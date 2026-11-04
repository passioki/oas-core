//! 调度器主循环 / CLI（ENGINE-DESIGN §22.34 · `docs/SCHEDULER-RUNNER-PLAN.md`）。
//!
//! Replaces the v1 `every_s` plan (`ENGINE §22.33`) with the v4 player config:
//! `scheduler.json` (schema_version 4) = calendar / guards / queues / entries.
//! Schema v3 documents are still read (no `queues` / `kind` → empty queue list,
//! `kind = "task"`) and are upgraded to v4 in memory; a v3 state file is
//! migrated on load and copied to `<state>.v3.bak` before the first v4 write
//! (`scheduler_state_backup path=...`). The pure scheduling matrix lives in
//! [`crate::scheduler`]; this module owns all IO: config/state files, the
//! single-instance lock, execution via `run::execute`, crash recovery, audit
//! events and reports.
//!
//! # CLI
//! ```text
//! oasd scheduler [run] --plan <scheduler.json> [--state <file>] [--tasks-dir tasks]
//!              [--once] [--max-passes N] [--dry-run] [--status] [--report <file.json>]
//!              [--library <dir>] [--model rec.onnx] [--det-model det.onnx]
//!              [--replay <bundle>] [--clicks-dir <dir>] [--runs-dir <dir>] [--shots-dir <dir>] [--seed N]
//!              [--keep-going] [--tz-offset-min M] [--tick-ms MS] [--events <file.jsonl>]
//! oasd scheduler reset-memory --tasks a,b [--all] [--include-checkpoint]
//!              [--plan <scheduler.json>] [--state <file>] [--tasks-dir tasks]
//! ```
//! **`--plan` selects the schedule plan** (`scheduler.json`). `--config` is the
//! **global live deployment config** (`config/live.json`): `main.rs` consumes it
//! before dispatching any subcommand, so in a normal `oasd` invocation it never
//! reaches this parser. Two distinct failure modes follow:
//! * `oasd scheduler --config scheduler.json` fails before the subcommand runs:
//!   `OasConfig` is `deny_unknown_fields`, so the plan file is rejected as a
//!   live config with `config error: config JSON: unknown field 'config'` and
//!   exit code 2 — it does **not** silently run the default plan;
//! * `oasd scheduler --config config/live.json` loads a valid live config, the
//!   flag is stripped here, and the scheduler parser silently falls back to the
//!   default `scheduler.json` (that is the "silent" half of the trap).
//! `--plan` remains accepted as an alias of the parser's `--config` field
//! (`SchedulerOptions::config`) for API callers. `--max-passes` is the primary
//! spelling of the L3 pass cap; `--max-rounds`/`--max-runs` are deprecated
//! aliases. `oasd schedule ...` is kept as the historical entry point
//! (`schedule_cmd`); a v1 plan file is rejected with a migration hint.
//!
//! # State file (`scheduler.state.json`)
//! Atomic (tmp + rename) writes of **runtime state only**:
//! ```json
//! {
//!   "schema_version": 4,
//!   "created_ms": 1760000000000,
//!   "last_pass_ms": 1760000000000,
//!   "passes": 12,
//!   "running": null,                       // {"entry_id","task","pass","attempt","since_ms","resume"}
//!   "cycle": {"day": "2026-10-23", "week": "2026-10-19"},
//!   "last_stop": "max_rounds",
//!   "entries": {
//!     "e1": {"task":"任务1","runs":3,"successes":3,"resources":0,"blocked":0,
//!            "failures":0,"last_result":"success","last_outcome":"Done",
//!            "last_exit":"done","last_attempts":1,"last_run_ms":0,
//!            "last_duration_ms":1234,"cooldown_until_ms":0,
//!            "quota_day":{"cycle":"2026-10-23","used":1},
//!            "quota_week":{"cycle":"2026-10-19","used":1}}
//!   },
//!   "queues": {                            // schema v4; absent in a v3 state
//!     "q1": {"total_rounds":2,"rounds_completed":1,"empty_rounds":0,
//!            "finished":false,
//!            "current_round":{"index":2,"member_index":1,"started_ms":0},
//!            "member_results":[{"round":2,"member_index":0,"task":"任务1",
//!                               "class":"success","exit":"done","outcome":"Done",
//!                               "attempts":1,"duration_ms":1200,
//!                               "finished_ms":1760000000000}]}
//!   }
//! }
//! ```
//! There is deliberately **no "completed" flag** for temp tasks and no
//! cross-pass "already ran" memory (§22.34.3): the only memory is runtime
//! counters, quota counters and cooldown. The `running` marker is written
//! before every execution; if the process dies, the next start sees a dirty
//! `running` entry and re-runs it with `--resume` (periodic) or resets the temp
//! task state and starts fresh (temp — no completion memory by design). Queue
//! rounds additionally persist `current_round{index, member_index}` so a
//! restart resumes the round without double-counting it (plan §二 rule 8).
//! A closed round stages its entry-level result in `pending_entry_record` in
//! the **same atomic write** as the round close; the next write applies
//! `record_result` and clears the stage. A crash between the two writes is
//! replayed exactly once at startup (`queue_entry_record_recovered`), so the
//! entry's quota/statistics can no longer be lost after the round was counted.
//! Every queue transition (round begin, each member record, round end) is
//! written with `save_state` = atomic tmp+rename, so the persisted state is
//! always a consistent snapshot; a member that finished but was not recorded
//! yet simply re-runs (at-least-once, the round is still counted once).
//!
//! # Audit events (`scheduler.events.jsonl`)
//! Append-only JSONL next to the state file (`<config>.events.jsonl` by
//! default): `scheduler_start`, `state_recovered`, `recovery_dropped`, `pass`,
//! `run_finished`, `temp_state_reset`, `alert`, `wake`, `stop`, `memory_reset`,
//! plus the queue-round events (P2): `queue_round_started`,
//! `queue_round_resumed` (restart continues `current_round`),
//! `queue_member_started`, `queue_member_finished` (the `skipped` flag marks a
//! member that failed the lightweight precheck and was recorded with
//! `attempts = 0`), `queue_round_finished`, `empty_round` (a round where no
//! member executed — lazy counting, rule 5), `queue_finished` (rule 6) and
//! `queue_missing` (defence in depth). `queue_entry_record_recovered` records
//! the startup replay of a staged entry result (P1-2). `scheduler_lease_busy`
//! records each contiguous device-lease conflict (PR6 P1-5: the pass is skipped
//! and retried, never failed). `--report` additionally writes the full
//! machine-readable summary of the session.
//!
//! # Run entities (P4)
//! Every execution attempt writes one immutable, first-class run entity to
//! `<runs-dir>/scheduler/<run_id>.json` (`--runs-dir`, default `runs`; the
//! directory is created on demand). A task entry writes one file per attempt
//! (retries included); a queue entry writes one file per member attempt.
//! `run_id = <started_ms>-<seq:04>` is process-unique and collision-checked on
//! disk; writes are atomic pretty JSON + newline (`write_json_atomic`). The
//! `run_finished` audit event additionally carries `exit_code`, `run_file`
//! (the last attempt's file, relative to `--runs-dir`, e.g.
//! `scheduler/1760000000000-0000.json`) and `run_file_error` (`null` unless
//! that attempt's entity could not be written). A write failure stays
//! **non-fatal** (the run already happened and state must still be saved) but
//! is observable: a `scheduler_run_file_error` log line + stderr, an `alert`
//! event with `kind=run_file_write` (audit log + `SchedulerReport::alerts`),
//! the additive `SchedulerReport::run_file_errors` counter, and a summary line
//! from the CLI. The legacy `*.clicks.jsonl` files and `GET /runs` are
//! untouched.
//!
//! # Execution (task entries vs queue entries, P2)
//! `plan_pass` filters `entries[]` in order; `execute_pass` dispatches on
//! `entry.kind`:
//! * `task` → [`Session::run_entry`]: one L2 run per pass, `entry.retry`
//!   extra attempts on `retryable`.
//! * `queue` → [`Session::execute_queue_round`]: one L4 **round** per pass.
//!   It opens (or, after a crash, resumes) `state.queues[qid].current_round`,
//!   runs the members from `member_index` to the tail serially, records each
//!   member (`state.queues[qid].member_results`), and closes the round with the
//!   pure lazy-count rule (`queue_end_round`: `rounds_completed` only when a
//!   member actually executed, otherwise `empty_rounds`). Every state
//!   transition is written with `save_state` (atomic tmp+rename) **before** the
//!   next side effect, so a crash resumes the round without double-counting it
//!   (plan §二 rule 8). A member gets `1 + entry.retry` attempts (the same
//!   `entry.retry` as a task entry; `0` = one attempt); retries sleep
//!   `queue.between_ms` and resume from `state/checkpoint.json` when it exists
//!   (same `resume_on_retry` default as `oasd queue`). The round's entry-level
//!   result (quota/cooldown/statistics) is recorded once with `record_result`:
//!   >=1 member `success`/`resource` → `success`; executed but all failed →
//!   `retryable`; empty round → `blocked`; any member `fatal` → `fatal` (and
//!   the round aborts like `fail_fast`, so `--keep-going` keeps its meaning).
//!
//! # Reset memory
//! `reset-memory` clears, for the selected tasks: ① the scheduler entry state
//! (attempts/cooldown/quota counters) — including the queue entry that schedules
//! a selected queue, whose `EntryState.task` is the queue_id; ② the round state
//! of every queue containing a selected task (rounds/finished/`member_results`,
//! and a dirty `running` marker pointing at the queue entry or one of its
//! members); and ③ `tasks/<id>/state/` (checkpoint, learned coordinates — the
//! task's memory). It appends a `memory_reset` audit event carrying the reset
//! queue ids under `queues_reset`, refuses to run while a scheduler holds the
//! lock, and leaves run statistics (`runs/ *.clicks.jsonl`) untouched.
//! Checkpoint clearing is **on by default** (`--include-checkpoint` is the
//! explicit spelling); `--keep-checkpoint` keeps `checkpoint.json` while
//! clearing the rest.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write as IoWrite;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use oas_runtime::RunResult;

use crate::device_lease::DeviceLease;
use crate::run::{self, RunOptions, RunReport};
use crate::scheduler::{
    self, classify_signal, clear_running, cycle_ids, entry_next_due_ms, format_local_datetime,
    guard_reason, next_wake, plan_pass, quota_remaining, quota_used, record_result, roll_cycles,
    should_retry, validate_scheduler_with_offset, Entry, EntryKind, GuardBlock, Lifecycle, PassPlan,
    ResultClass, RunMode, RunSignal, Scheduler, SchedulerState, DEFAULT_UTC_OFFSET_MIN, MAX_SLEEP_MS,
    SCHEDULE_SCHEMA_VERSION, STATE_SCHEMA_VERSION,
};

/// Default player config (§22.34 / M3 `config::SchedulerConfig::plan`).
pub const DEFAULT_CONFIG: &str = "scheduler.json";
pub const DEFAULT_TASKS_DIR: &str = "tasks";
pub const DEFAULT_RUNS_DIR: &str = "runs";
pub const DEFAULT_MAX_SLEEP_MS: u64 = MAX_SLEEP_MS as u64;

/// Subdirectory of `--runs-dir` that holds the first-class Run entities:
/// `<runs_dir>/scheduler/<run_id>.json` (P4). The `scheduler/` prefix keeps the
/// legacy click JSONL files at the root untouched (`GET /runs` compatibility).
pub(crate) const SCHEDULER_RUN_DIR: &str = "scheduler";
/// Schema version of one run entity file (P4; independent of the plan/state
/// schema versions because run files are append-only immutable history).
pub(crate) const RUN_SCHEMA_VERSION: u32 = 1;

pub const USAGE: &str = "usage: oasd scheduler [run] --plan <scheduler.json> [--state <file>] \
                         [--tasks-dir tasks] [--once] [--max-passes N] [--dry-run] [--status] \
                         [--report <file.json>] [--library <dir>] [--model <rec.onnx>] \
                         [--det-model <det.onnx>] [--replay <bundle>] [--clicks-dir <dir>] \
                         [--runs-dir <dir>] [--shots-dir <dir>] [--seed N] [--keep-going] \
                         [--tz-offset-min M] [--tick-ms MS] [--events <file.jsonl>]
       oasd scheduler reset-memory --tasks <a,b> [--all] [--include-checkpoint] \
                         [--plan <scheduler.json>] [--state <file>] [--tasks-dir tasks]
       note: --plan is the SCHEDULE plan (scheduler.json). --config is the GLOBAL live
             deployment config (config/live.json, consumed by main.rs before the
             subcommand) and is NOT the schedule file: pointing it at scheduler.json
             fails with `config error ... unknown field 'config'` (exit 2).
             --max-passes is the pass cap; --max-rounds/--max-runs are deprecated aliases.";

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// Scripted attempt result for the deterministic executor seam (tests only).
///
/// Production always goes through `run::execute` + `classify_signal`; tests
/// inject one of these per `(task, attempt)` so a queue round can be exercised
/// without a device/replay (P2 task D).
#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TestExecResult {
    pub class: ResultClass,
    pub exit: String,
    pub outcome: String,
    pub clicks: usize,
    pub error: Option<String>,
    /// Mirrors `RunReport::exit_code()` for the scripted attempt (0 = Done).
    pub exit_code: u8,
}

#[cfg(test)]
impl TestExecResult {
    pub(crate) fn new(class: ResultClass, exit: &str, outcome: &str) -> Self {
        TestExecResult {
            class,
            exit: exit.to_string(),
            outcome: outcome.to_string(),
            clicks: 0,
            error: None,
            exit_code: if outcome == "Done" { 0 } else { 1 },
        }
    }

    pub(crate) fn with_clicks(mut self, clicks: usize) -> Self {
        self.clicks = clicks;
        self
    }
}

/// One call observed by the test seam (`calls()` asserts execution order).
#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TestExecCall {
    pub task: String,
    pub attempt: u32,
    pub resume: bool,
}

/// Test-only scripted executor: task name → per-attempt results (the last entry
/// repeats), plus a set of members that fail the lightweight precheck.
#[cfg(test)]
#[derive(Debug, Clone, Default)]
pub(crate) struct TestExec {
    script: BTreeMap<String, Vec<TestExecResult>>,
    skip: BTreeSet<String>,
    calls: std::sync::Arc<std::sync::Mutex<Vec<TestExecCall>>>,
    /// `state.running` snapshot observed by the seam right before every attempt
    /// (asserts `mark_running` named the current member task; B5 test gap).
    observed_running: std::sync::Arc<
        std::sync::Mutex<Vec<Option<scheduler::RunningEntry>>>,
    >,
}

#[cfg(test)]
impl TestExec {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn with(mut self, task: &str, steps: Vec<TestExecResult>) -> Self {
        self.script.insert(task.to_string(), steps);
        self
    }

    /// Make this member fail the "task still exists" precheck (empty-round
    /// test): it is recorded with `attempts = 0` and never executed.
    pub(crate) fn skip_member(mut self, task: &str) -> Self {
        self.skip.insert(task.to_string());
        self
    }

    fn next(&self, task: &str, attempt: u32, resume: bool) -> Option<TestExecResult> {
        self.calls.lock().expect("test exec calls").push(TestExecCall {
            task: task.to_string(),
            attempt,
            resume,
        });
        let steps = self.script.get(task)?;
        if steps.is_empty() {
            return None;
        }
        let idx = (attempt as usize).saturating_sub(1).min(steps.len() - 1);
        Some(steps[idx].clone())
    }

    fn skips(&self, task: &str) -> bool {
        self.skip.contains(task)
    }

    pub(crate) fn calls(&self) -> Vec<TestExecCall> {
        self.calls.lock().expect("test exec calls").clone()
    }

    /// Record the `state.running` marker the production seam just wrote (the
    /// engine call is replaced by the script, the bookkeeping is not).
    fn observe_running(&self, running: Option<scheduler::RunningEntry>) {
        self.observed_running
            .lock()
            .expect("test exec running")
            .push(running);
    }

    pub(crate) fn observed_running(&self) -> Vec<Option<scheduler::RunningEntry>> {
        self.observed_running
            .lock()
            .expect("test exec running")
            .clone()
    }
}

#[derive(Debug, Clone)]
pub struct SchedulerOptions {
    /// The schedule plan `scheduler.json` (schema v4; v3 read as compat).
    ///
    /// CLI spelling: `--plan`. (`--config` is consumed globally by `main.rs` as
    /// the live deployment config and never reaches this field in a normal
    /// `oasd` invocation; the parser still accepts it as an alias for API
    /// callers such as `serve.rs`.)
    pub config: PathBuf,
    /// `scheduler.state.json` (default: `<config stem>.state.json`).
    pub state: Option<PathBuf>,
    pub tasks_dir: PathBuf,
    pub library: PathBuf,
    pub model: Option<PathBuf>,
    pub det_model: Option<PathBuf>,
    pub replay: Option<PathBuf>,
    pub clicks_dir: Option<PathBuf>,
    /// Root for the first-class Run entities (P4): each attempt writes
    /// `<runs_dir>/scheduler/<run_id>.json` (default `runs`). Independent of
    /// `clicks_dir` so the click audit can live elsewhere.
    pub runs_dir: PathBuf,
    pub shots_dir: Option<PathBuf>,
    pub seed: Option<u64>,
    /// One pass, then exit (no sleeping).
    pub once: bool,
    /// L3 pass cap for this session (`--max-passes`; the deprecated aliases are
    /// `--max-rounds`/`--max-runs`). **Not** a queue Round (L4): see
    /// `docs/SCHEDULER-RUNNER-PLAN.md` §一 for the four layers.
    pub max_passes: Option<u64>,
    /// Preview the pass plan: no state writes, no lock, no execution.
    pub dry_run: bool,
    /// Print the machine-readable status JSON (panel contract) and exit.
    pub status: bool,
    pub report: Option<PathBuf>,
    pub events: Option<PathBuf>,
    /// `fatal` does not stop the scheduler (diagnostics/long runs).
    pub keep_going: bool,
    /// Override the local UTC offset (minutes); default: config / env / +480.
    pub tz_offset_min: Option<i32>,
    /// Upper bound for waiting sleeps (v1 `--tick-ms` compatibility).
    pub max_sleep_ms: u64,
    /// Override `config.loop_interval_s`.
    pub loop_interval_s: Option<u64>,
    /// Deterministic executor seam (tests only). Production leaves it `None`
    /// and `run_task_attempt` calls `run::execute` + `classify_signal`; tests
    /// script `(task, attempt)` results via this field.
    #[cfg(test)]
    pub(crate) test_exec: Option<TestExec>,
    /// Runner values layer (design §C.3 priority 3) source: `None` (production)
    /// resolves `config/values.json` next to the live `--config` file through
    /// [`crate::runner_config::runner_values_path`]. Tests pin a temp path here
    /// so they never read the operator's real values store.
    pub values_path: Option<PathBuf>,
    /// Cooperative stop token (design §A.6.2, PR6). `None` = no external stop:
    /// the CLI runs until a stopping rule fires. The Runner's embedded scheduler
    /// sets it and `POST /runner/scheduler/stop` stores `true`, so the loop
    /// exits between passes with `stopped_reason = "api_stop"` **without**
    /// interrupting an attempt that is already executing (single active
    /// instance). The check is also done in ≤200 ms slices of the inter-pass
    /// sleep, so a stop request is honoured in ≤5 s even during a long wait.
    pub stop: Option<Arc<AtomicBool>>,
    /// Device lease path (PR6, design §A.6.4). `None` = no lease (tests / pure
    /// CLI without a live `--config`).
    ///
    /// When set, the lease is acquired **immediately before each pass** and
    /// released when the pass ends ([`DeviceLease`] Drop), so an idle/sleeping
    /// scheduler does not block a Runner temp run or a Studio trial. A live
    /// holder makes the pass wait (`scheduler_lease_busy` log + audit) and be
    /// retried after [`LEASE_BUSY_RETRY_MS`]: it never aborts the session and
    /// never writes a dirty `running` marker.
    pub lease_path: Option<PathBuf>,
}

impl Default for SchedulerOptions {
    fn default() -> Self {
        SchedulerOptions {
            config: PathBuf::from(DEFAULT_CONFIG),
            state: None,
            tasks_dir: PathBuf::from(DEFAULT_TASKS_DIR),
            library: PathBuf::from(crate::library::DEFAULT_DIR),
            model: None,
            det_model: None,
            replay: None,
            clicks_dir: None,
            runs_dir: PathBuf::from(DEFAULT_RUNS_DIR),
            shots_dir: None,
            seed: None,
            once: false,
            max_passes: None,
            dry_run: false,
            status: false,
            report: None,
            events: None,
            keep_going: false,
            tz_offset_min: None,
            max_sleep_ms: DEFAULT_MAX_SLEEP_MS,
            loop_interval_s: None,
            #[cfg(test)]
            test_exec: None,
            values_path: None,
            stop: None,
            lease_path: None,
        }
    }
}

fn take_value(rest: &[String], i: &mut usize, flag: &str) -> Result<String, String> {
    if *i + 1 >= rest.len() {
        return Err(format!("{flag} requires a value"));
    }
    *i += 2;
    Ok(rest[*i - 1].clone())
}

fn parse_u64(s: &str, flag: &str) -> Result<u64, String> {
    s.parse()
        .map_err(|_| format!("{flag} must be a non-negative integer, got `{s}`"))
}

impl SchedulerOptions {
    pub fn parse(rest: &[String]) -> Result<Self, String> {
        let mut o = Self::default();
        let mut i = 0usize;
        while i < rest.len() {
            match rest[i].as_str() {
                "--config" | "--plan" | "--scheduler-config" => {
                    o.config = PathBuf::from(take_value(rest, &mut i, "--config")?)
                }
                "--state" => {
                    o.state = Some(PathBuf::from(take_value(rest, &mut i, "--state")?))
                }
                "--tasks-dir" => {
                    o.tasks_dir = PathBuf::from(take_value(rest, &mut i, "--tasks-dir")?)
                }
                "--library" => {
                    o.library = PathBuf::from(take_value(rest, &mut i, "--library")?)
                }
                "--model" => o.model = Some(PathBuf::from(take_value(rest, &mut i, "--model")?)),
                "--det-model" => {
                    o.det_model = Some(PathBuf::from(take_value(rest, &mut i, "--det-model")?))
                }
                "--replay" => {
                    o.replay = Some(PathBuf::from(take_value(rest, &mut i, "--replay")?))
                }
                "--clicks-dir" => {
                    o.clicks_dir = Some(PathBuf::from(take_value(rest, &mut i, "--clicks-dir")?))
                }
                "--runs-dir" => {
                    o.runs_dir = PathBuf::from(take_value(rest, &mut i, "--runs-dir")?)
                }
                "--shots-dir" => {
                    o.shots_dir = Some(PathBuf::from(take_value(rest, &mut i, "--shots-dir")?))
                }
                "--seed" => {
                    o.seed = Some(parse_u64(&take_value(rest, &mut i, "--seed")?, "--seed")?)
                }
                "--max-passes" | "--max-rounds" | "--max-runs" => {
                    let flag = rest[i].clone();
                    let v = parse_u64(&take_value(rest, &mut i, &flag)?, &flag)?;
                    if v == 0 {
                        return Err(format!("{flag} must be >= 1"));
                    }
                    o.max_passes = Some(v);
                }
                "--tz-offset-min" => {
                    let raw = take_value(rest, &mut i, "--tz-offset-min")?;
                    let v: i32 = raw.parse().map_err(|_| {
                        format!("--tz-offset-min must be an integer (minutes), got `{raw}`")
                    })?;
                    if !(-720..=840).contains(&v) {
                        return Err(format!(
                            "--tz-offset-min must be -720..=840, got {v}"
                        ));
                    }
                    o.tz_offset_min = Some(v);
                }
                "--tick-ms" => {
                    let v = parse_u64(&take_value(rest, &mut i, "--tick-ms")?, "--tick-ms")?;
                    if v == 0 {
                        return Err("--tick-ms must be >= 1".to_string());
                    }
                    o.max_sleep_ms = v.max(1);
                }
                "--loop-interval-s" => {
                    let v = parse_u64(
                        &take_value(rest, &mut i, "--loop-interval-s")?,
                        "--loop-interval-s",
                    )?;
                    if v > 86_400 {
                        return Err("--loop-interval-s must be <= 86400".to_string());
                    }
                    o.loop_interval_s = Some(v);
                }
                "--events" => {
                    o.events = Some(PathBuf::from(take_value(rest, &mut i, "--events")?))
                }
                "--once" => {
                    o.once = true;
                    i += 1;
                }
                "--dry-run" => {
                    o.dry_run = true;
                    i += 1;
                }
                "--status" => {
                    o.status = true;
                    i += 1;
                }
                "--report" => {
                    o.report = Some(PathBuf::from(take_value(rest, &mut i, "--report")?))
                }
                "--keep-going" => {
                    o.keep_going = true;
                    i += 1;
                }
                other => return Err(format!("unknown option `{other}`\n{USAGE}")),
            }
        }
        Ok(o)
    }

    pub fn state_path(&self) -> PathBuf {
        self.state
            .clone()
            .unwrap_or_else(|| sibling_with_suffix(&self.config, "state.json"))
    }

    pub fn events_path(&self) -> PathBuf {
        self.events
            .clone()
            .unwrap_or_else(|| sibling_with_suffix(&self.config, "events.jsonl"))
    }
}

/// `dir/scheduler.json` + `"state.json"` -> `dir/scheduler.state.json`.
fn sibling_with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "scheduler".to_string());
    let name = format!("{stem}.{suffix}");
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.join(name),
        _ => PathBuf::from(name),
    }
}

// ---------------------------------------------------------------------------
// Small IO helpers
// ---------------------------------------------------------------------------

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn create_parent(path: &Path) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("create `{}`: {e}", dir.display()))?;
        }
    }
    Ok(())
}

/// Atomic JSON write (tmp + rename): a crash never leaves a half-written file.
fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    create_parent(path)?;
    let text = serde_json::to_string_pretty(value)
        .map_err(|e| format!("serialize `{}`: {e}", path.display()))?;
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "state.json".to_string());
    let tmp = path.with_file_name(format!("{file_name}.tmp"));
    std::fs::write(&tmp, text + "\n")
        .map_err(|e| format!("write `{}`: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("rename `{}`: {e}", path.display()))
}

/// Append one audit JSONL row (creates parents). Best-effort callers warn.
fn append_jsonl(path: &Path, value: &Value) -> Result<(), String> {
    create_parent(path)?;
    let line = serde_json::to_string(value)
        .map_err(|e| format!("serialize audit event: {e}"))?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| format!("open `{}`: {e}", path.display()))?;
    writeln!(file, "{line}").map_err(|e| format!("append `{}`: {e}", path.display()))
}

fn load_state(path: &Path) -> Result<SchedulerState, String> {
    load_state_migrating(path).map(|(state, _)| state)
}

/// `<state>.v3.bak` (`scheduler.state.json` -> `scheduler.state.json.v3.bak`).
fn state_v3_backup_path(state_path: &Path) -> PathBuf {
    let mut os = state_path.as_os_str().to_os_string();
    os.push(".v3.bak");
    PathBuf::from(os)
}

/// Copy the pre-migration v3 state to `<state>.v3.bak` and log the path.
///
/// Called only when [`load_state_migrating`] reported a v3 migration and just
/// **before** the first v4 write (plan §四 "读 3 自动迁移 + 备份 state"). An
/// existing `.v3.bak` is overwritten (the latest pre-upgrade snapshot wins).
/// `Err` aborts before v4 is written, leaving the original v3 file untouched.
fn backup_state_v3(state_path: &Path, out: &mut dyn FnMut(&str)) -> Result<PathBuf, String> {
    let backup = state_v3_backup_path(state_path);
    std::fs::copy(state_path, &backup).map_err(|e| {
        format!(
            "scheduler state: backup `{}` -> `{}`: {e}",
            state_path.display(),
            backup.display()
        )
    })?;
    out(&format!("scheduler_state_backup path={}", backup.display()));
    Ok(backup)
}

/// [`load_state`] plus a migration note.
///
/// Schema v3 (`scheduler.state.json` before queues) is accepted and upgraded in
/// memory to v4 with an empty `queues` map — `#[serde(default)]` already
/// produces it, the version bump is the migration. Any other version is an
/// error; the caller decides whether the next save rewrites v4 (the daemon
/// saves right after startup, `reset-memory` only writes when it changed
/// something, `--status`/`--dry-run` never write).
///
/// The migration is **in memory only**: a caller that will write v4 must call
/// [`backup_state_v3`] first, while the file on disk is still v3.
fn load_state_migrating(path: &Path) -> Result<(SchedulerState, Option<String>), String> {
    if !path.is_file() {
        return Ok((SchedulerState::new(), None));
    }
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("scheduler state: read `{}`: {e}", path.display()))?;
    let mut state: SchedulerState = serde_json::from_str(&text)
        .map_err(|e| format!("scheduler state `{}`: invalid JSON: {e}", path.display()))?;
    let note = match state.schema_version {
        STATE_SCHEMA_VERSION => None,
        3 => {
            state.schema_version = STATE_SCHEMA_VERSION;
            Some(format!(
                "scheduler state `{}`: schema_version 3 migrated to {STATE_SCHEMA_VERSION} \
                 (queue state starts empty)",
                path.display()
            ))
        }
        other => {
            return Err(format!(
                "scheduler state `{}`: schema_version {other} unsupported \
                 (expect 3 or {STATE_SCHEMA_VERSION}); delete it (memory reset) \
                 or point --state elsewhere",
                path.display()
            ));
        }
    };
    Ok((state, note))
}

fn default_utc_offset() -> i32 {
    std::env::var("OAS_TZ_OFFSET_MIN")
        .ok()
        .and_then(|v| v.trim().parse::<i32>().ok())
        .filter(|v| (-720..=840).contains(v))
        .unwrap_or(DEFAULT_UTC_OFFSET_MIN)
}

fn read_config_text(opts: &SchedulerOptions) -> Result<String, String> {
    std::fs::read_to_string(&opts.config).map_err(|e| {
        format!(
            "scheduler config: read `{}`: {e} — create a v3 scheduler.json \
             (see ENGINE-DESIGN §22.34.4) or pass --config/--plan <file>",
            opts.config.display()
        )
    })
}

/// Load + validate the config only (no task IO) — used by `--status` too.
fn load_scheduler(opts: &SchedulerOptions) -> Result<Scheduler, String> {
    let text = read_config_text(opts)?;
    let resolved_default = opts.tz_offset_min.unwrap_or_else(default_utc_offset);
    let mut sched = validate_scheduler_with_offset(&text, resolved_default)?;
    if let Some(cli) = opts.tz_offset_min {
        sched.calendar.utc_offset_min = cli;
    }
    if let Some(interval) = opts.loop_interval_s {
        sched.loop_interval_s = interval;
    }
    Ok(sched)
}

/// Config + task existence + `meta.exits` (fail fast before any write).
struct Loaded {
    sched: Scheduler,
    /// task -> exit name -> result class (M0 contract; empty when undeclared).
    metas: BTreeMap<String, BTreeMap<String, ResultClass>>,
}

fn load_full(opts: &SchedulerOptions) -> Result<Loaded, String> {
    let sched = load_scheduler(opts)?;
    let mut metas = BTreeMap::new();
    // Task entries and queue members both name task bundles; a queue entry's
    // `task` is a queue_id, never a bundle (rule 2/3).
    for entry in &sched.entries {
        if entry.kind != EntryKind::Task || metas.contains_key(&entry.task) {
            continue;
        }
        let exits = load_task_metas(
            &opts.tasks_dir,
            &entry.task,
            &format!("entry `{}`", entry.entry_id),
        )?;
        metas.insert(entry.task.clone(), exits);
    }
    for queue in &sched.queues {
        for member in &queue.members {
            if metas.contains_key(member) {
                continue;
            }
            let exits = load_task_metas(
                &opts.tasks_dir,
                member,
                &format!("queue `{}` member", queue.queue_id),
            )?;
            metas.insert(member.clone(), exits);
        }
    }
    Ok(Loaded { sched, metas })
}

/// Fail fast when a referenced task bundle is missing/unreadable (before any
/// byte of state is written) and collect its `meta.exits`.
fn load_task_metas(
    tasks_dir: &Path,
    task: &str,
    ctx: &str,
) -> Result<BTreeMap<String, ResultClass>, String> {
    let dir = tasks_dir.join(task);
    if !dir.is_dir() {
        return Err(format!(
            "scheduler config: {ctx} task `{task}` not found under `{}`",
            tasks_dir.display()
        ));
    }
    let meta_path = dir.join("meta.json");
    let text = std::fs::read_to_string(&meta_path).map_err(|e| {
        format!(
            "scheduler config: {ctx} task `{task}`: meta.json: {e} \
             (not a task bundle)"
        )
    })?;
    scheduler::parse_meta_exits(&text)
        .map_err(|e| format!("scheduler config: task `{task}`: {e}"))
}

/// Collect `meta.exits` for one task (tolerated when absent).
fn meta_exits_for(
    metas: &BTreeMap<String, BTreeMap<String, ResultClass>>,
    task: &str,
) -> BTreeMap<String, ResultClass> {
    metas.get(task).cloned().unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Single instance lock
// ---------------------------------------------------------------------------

struct SchedulerLock {
    path: PathBuf,
}

impl SchedulerLock {
    fn lock_path(state_path: &Path) -> PathBuf {
        state_path.with_extension("lock")
    }

    /// Acquire the lock file (`create_new`) or reclaim a stale one.
    fn acquire(state_path: &Path) -> Result<Self, String> {
        let path = Self::lock_path(state_path);
        create_parent(&path)?;
        let pid = std::process::id();
        for attempt in 0..2 {
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(mut file) => {
                    let payload = json!({"pid": pid, "started_ms": now_ms()});
                    let _ = write!(file, "{payload}");
                    return Ok(SchedulerLock { path });
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    let holder = read_lock_pid(&path);
                    if let Some(other) = holder {
                        // Any live holder blocks us — including our own pid
                        // (a second thread in this process must not steal it).
                        if process_alive(other) {
                            return Err(format!(
                                "another scheduler is already running (pid {other}, lock `{}`); \
                                 stop it before starting a second one",
                                path.display()
                            ));
                        }
                    }
                    // Stale lock (dead pid / unreadable): reclaim exactly once.
                    let _ = std::fs::remove_file(&path);
                    if attempt == 1 {
                        return Err(format!(
                            "lock `{}`: could not reclaim a stale lock",
                            path.display()
                        ));
                    }
                }
                Err(e) => return Err(format!("lock `{}`: {e}", path.display())),
            }
        }
        Err(format!("lock `{}`: unreachable", path.display()))
    }
}

impl Drop for SchedulerLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn read_lock_pid(path: &Path) -> Option<u32> {
    let text = std::fs::read_to_string(path).ok()?;
    let value: Value = serde_json::from_str(&text).ok()?;
    value
        .get("pid")
        .and_then(|v| v.as_u64())
        .and_then(|v| u32::try_from(v).ok())
}

/// Is a process with `pid` alive? Used for stale-lock / stale-lease detection.
pub(crate) fn process_alive(pid: u32) -> bool {
    #[cfg(windows)]
    {
        let filter = format!("PID eq {pid}");
        match std::process::Command::new("tasklist")
            .args(["/FI", &filter, "/FO", "CSV", "/NH"])
            .output()
        {
            Ok(out) => {
                let text = String::from_utf8_lossy(&out.stdout);
                // Matching CSV rows contain `,"<pid>",`; the "no tasks" notice
                // does not (and is locale-dependent, so do not match on it).
                text.contains(&format!(",\"{pid}\","))
            }
            Err(_) => true, // cannot verify => assume alive (never steal a lock)
        }
    }
    #[cfg(unix)]
    {
        match std::process::Command::new("kill")
            .arg("-0")
            .arg(pid.to_string())
            .status()
        {
            Ok(status) => status.success(),
            Err(_) => true,
        }
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = pid;
        true
    }
}

/// Pid of a live scheduler holding `<state_path>.lock` (`None` when the lock is
/// absent or stale). Read-only: never creates, steals or removes the lock; the
/// lock path comes from the same [`SchedulerLock::lock_path`] used by
/// [`SchedulerLock::acquire`], so it is never hardcoded in callers.
pub fn lock_held_by_live_process(state_path: &Path) -> Option<u32> {
    let path = SchedulerLock::lock_path(state_path);
    if !path.is_file() {
        return None;
    }
    match read_lock_pid(&path) {
        Some(pid) if process_alive(pid) => Some(pid),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Report
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EntryReport {
    pub entry_id: String,
    pub task: String,
    pub lifecycle: String,
    pub enabled: bool,
    pub runs: u64,
    pub successes: u64,
    pub resources: u64,
    pub blocked: u64,
    pub failures: u64,
    pub last_result: String,
    pub last_outcome: String,
    pub last_exit: String,
    pub last_run_ms: u64,
    pub cooldown_until_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quota_per: Option<String>,
    pub quota_times: u32,
    pub quota_used: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quota_remaining: Option<u32>,
    pub next_due_ms: Option<i64>,
}

/// One completed queue round, summarised from the persisted `member_results`
/// (P2 `--report`). Only rounds with at least one recorded member can be
/// summarised: an **empty** round leaves no member result by definition (its
/// evidence is `empty_rounds` + the `empty_round` audit event).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct QueueRoundReport {
    /// Round ordinal (see `scheduler::QueueRoundState::index`).
    pub round: u64,
    /// Members that actually executed (`attempts >= 1`).
    pub executed: u64,
    /// Members present in `member_results` for this round (skipped ones have
    /// `attempts = 0`).
    pub recorded: u64,
    pub members: Vec<scheduler::QueueMemberResult>,
}

/// Per-queue summary in `scheduler-report.json` (P2; additive to the report,
/// `#[serde(default)]` on [`SchedulerReport::queues`] keeps older reports/readers
/// compatible).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct QueueReport {
    pub queue_id: String,
    pub name: String,
    pub total_rounds: u64,
    pub on_failure: String,
    pub between_ms: u64,
    pub rounds_completed: u64,
    pub empty_rounds: u64,
    /// Rule 6 (`finished` is sticky until reset-memory / the UI).
    pub finished: bool,
    pub current_round: Option<scheduler::QueueRoundState>,
    pub last_round: Option<QueueRoundReport>,
    pub next_due_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SchedulerReport {
    /// Report schema version. By the existing convention it tracks
    /// [`SCHEDULE_SCHEMA_VERSION`] (P1 bumped both 3→4 together); P2 only
    /// *adds* `queues[]` with `#[serde(default)]`, so it stays 4 and an older
    /// reader simply ignores the new key (additive change, no break).
    pub schema_version: u32,
    pub started_ms: u64,
    pub finished_ms: u64,
    pub dry_run: bool,
    pub mode: String,
    pub passes: u64,
    /// Entry executions (one per planned entry per pass, retries included);
    /// a queue entry counts **one per round**, not per member.
    pub runs: u64,
    pub successes: u64,
    pub resources: u64,
    pub blocked: u64,
    pub failures: u64,
    pub fatals: u64,
    pub stopped_reason: String,
    pub alerts: Vec<String>,
    pub config_path: String,
    pub state_path: String,
    pub next_wake_ms: Option<i64>,
    pub entries: Vec<EntryReport>,
    /// P2 queue summaries (additive; absent in older reports / older readers).
    #[serde(default)]
    pub queues: Vec<QueueReport>,
    /// P4 additive: run entity writes that failed during this session.
    ///
    /// A failure is **not** a run failure (the attempt already happened and the
    /// state was still saved); this counter exists so the lost history is
    /// visible in `--report`/the CLI summary instead of only in the scheduler
    /// log. `0` on a healthy session.
    #[serde(default)]
    pub run_file_errors: u32,
}

impl SchedulerReport {
    pub fn all_ok(&self) -> bool {
        self.fatals == 0 && self.blocked == 0 && self.failures == 0
    }
}

// ---------------------------------------------------------------------------
// First-class Run entities (P4)
// ---------------------------------------------------------------------------

/// One immutable run entity: `<runs_dir>/scheduler/<run_id>.json`.
///
/// Written once per **attempt** (task entry) / **member attempt** (queue
/// entry) by [`Session::write_run_entity`]. `queue_id` / `round` /
/// `member_index` are present only for queue members; `error` / `clicks_file`
/// only when the attempt produced them. The file name comes from `run_id`
/// (`<started_ms>-<seq:04>`): unique within the process, collision-checked on
/// disk, sortable by `started_ms`, and safe as a file name by construction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SchedulerRun {
    #[serde(default)]
    pub schema_version: u32,
    #[serde(default)]
    pub run_id: String,
    /// Always `"scheduler"` for these files (future producers reuse the dir).
    #[serde(default)]
    pub source: String,
    /// `"task"` (task entry attempt) or `"queue_member"` (queue member attempt).
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub entry_id: String,
    /// Task entry: the task name. Queue member: the member task name (never the
    /// queue_id — that lives in `queue_id`).
    #[serde(default)]
    pub task: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queue_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub round: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member_index: Option<usize>,
    #[serde(default)]
    pub attempt: u32,
    #[serde(default)]
    pub resume: bool,
    #[serde(default)]
    pub started_ms: u64,
    #[serde(default)]
    pub finished_ms: u64,
    #[serde(default)]
    pub duration_ms: u64,
    /// [`ResultClass::as_str`] (`success` / `resource` / `blocked` / ...).
    #[serde(default)]
    pub class: String,
    /// Exit signal name (the existing `run_finished` `exit` string).
    #[serde(default)]
    pub exit: String,
    /// `RunReport::exit_code()` (0 = Done, 1 = Failed/Exhausted; a setup error
    /// without a report is recorded as 1).
    #[serde(default)]
    pub exit_code: u8,
    /// `Outcome` debug string (`Done` / `Failed` / `Exhausted`; `error` for a
    /// setup error, matching the `run_finished` event).
    #[serde(default)]
    pub outcome: String,
    #[serde(default)]
    pub clicks: usize,
    /// Failure text (only when the attempt failed with an error).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Click audit file, when `--clicks-dir` produced one for this attempt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clicks_file: Option<String>,
}

impl SchedulerRun {
    /// A file is only a run entity when it carries the mandatory identity
    /// fields; arbitrary/hand-edited JSON under `scheduler/` is skipped (a bad
    /// file never fails the list/detail/stats call). `schema_version >= 1`
    /// keeps a future additive version readable instead of silently ignored.
    pub(crate) fn is_run_entity(&self) -> bool {
        self.schema_version >= 1
            && !self.run_id.is_empty()
            && !self.task.is_empty()
            && (self.kind == "task" || self.kind == "queue_member")
    }
}

/// `<runs_dir>/scheduler` — the run entity directory (P4).
pub(crate) fn scheduler_runs_dir(runs_dir: &Path) -> PathBuf {
    runs_dir.join(SCHEDULER_RUN_DIR)
}

/// Reference to the run entity written by the most recent attempt.
///
/// The `run_finished` audit event summarises a whole entry execution while run
/// files are per attempt; the last attempt is the representative one for the
/// additive `exit_code` / `run_file` event fields.
#[derive(Debug, Clone)]
struct RunRef {
    /// Path relative to `runs_dir` (`scheduler/<run_id>.json`, `/` separated).
    run_file: String,
    exit_code: u8,
}

/// Queue-member identity of one attempt (`kind = "queue_member"`).
#[derive(Debug, Clone, Copy)]
struct MemberRunContext<'a> {
    queue_id: &'a str,
    round: u64,
    member_index: usize,
}

// ---------------------------------------------------------------------------
// Session: main loop state
// ---------------------------------------------------------------------------

struct Session<'a> {
    opts: &'a SchedulerOptions,
    loaded: Loaded,
    state: SchedulerState,
    state_path: PathBuf,
    events_path: PathBuf,
    report: SchedulerReport,
    /// Crash-recovery marker loaded from the state file (consumed after the
    /// entry's recovered execution).
    recovery: Option<scheduler::RunningEntry>,
    seq: u64,
    /// Monotonic run entity sequence (`<started_ms>-<seq:04>`); independent of
    /// the clicks-file `seq` so a run id never depends on `--clicks-dir`.
    run_seq: u64,
    /// Run entity of the most recent attempt (cleared at the start of every
    /// planned entry execution so a previous entry can never leak into the
    /// aggregate `run_finished` event).
    last_run: Option<RunRef>,
    /// Error of the most recent attempt's run-entity write (`Some` only when
    /// that attempt failed to persist its entity). Cleared together with
    /// `last_run` and by every successful write, so the aggregate event can
    /// tell "no attempt ran" (`run_file: null`, `run_file_error: null`) apart
    /// from "the evidence was lost" (`run_file: null`, `run_file_error: "..."`).
    last_run_error: Option<String>,
}

impl<'a> Session<'a> {
    fn sched(&self) -> &Scheduler {
        &self.loaded.sched
    }

    fn save_state(&self, out: &mut dyn FnMut(&str)) -> Result<(), String> {
        write_json_atomic(&self.state_path, &self.state)
            .map_err(|e| format!("scheduler state: {e}"))
            .map_err(|e| {
                out(&format!("scheduler_state_error={e}"));
                e
            })
    }

    fn audit(&self, mut event: Value) {
        if let Some(obj) = event.as_object_mut() {
            obj.entry("ts_ms".to_string()).or_insert(json!(now_ms()));
        }
        if let Err(e) = append_jsonl(&self.events_path, &event) {
            // Audit is best-effort during runs; reset-memory makes it fatal.
            eprintln!("scheduler: audit warn: {e}");
        }
    }

    fn alert(&mut self, out: &mut dyn FnMut(&str), kind: &str, entry_id: &str, message: &str) {
        let line = format!(
            "ALERT level=warn kind={kind} entry={entry_id} message={message}"
        );
        out(&line);
        self.report.alerts.push(format!("{kind}:{entry_id}:{message}"));
        self.audit(json!({
            "event": "alert",
            "kind": kind,
            "entry_id": entry_id,
            "message": message,
        }));
    }

    /// Next globally unique, filename-safe run id: `<started_ms>-<seq:04>`.
    ///
    /// `run_seq` is process-local; the on-disk existence check upgrades that to
    /// "unique unless two live processes write the same millisecond **and** the
    /// same sequence" — there is only one active scheduler instance by design
    /// (`docs/SCHEDULER-RUNNER-PLAN.md` §三), and a restart after a crash finds
    /// the previous files and skips the colliding sequence number.
    fn next_run_id(&mut self, started_ms: u64) -> String {
        loop {
            let seq = self.run_seq;
            self.run_seq += 1;
            let id = format!("{started_ms}-{seq:04}");
            let path = self
                .opts
                .runs_dir
                .join(SCHEDULER_RUN_DIR)
                .join(format!("{id}.json"));
            if !path.exists() {
                return id;
            }
        }
    }

    /// Exit code used by the aggregate `run_finished` event when no run entity
    /// was written (an empty queue round, or a run-file write failure):
    /// completed classes are 0, everything else 1 (`RunReport::exit_code`
    /// mapping).
    fn class_exit_code(class: ResultClass) -> u8 {
        match class {
            ResultClass::Success | ResultClass::Resource => 0,
            _ => 1,
        }
    }

    /// Persist one first-class Run entity (`<runs_dir>/scheduler/<run_id>.json`)
    /// and remember its relative path for the aggregate `run_finished` event.
    ///
    /// The write is atomic (`write_json_atomic`: pretty JSON + newline, tmp +
    /// rename). A write failure is loud but **not fatal**: the run already
    /// happened and the scheduler state must still be persisted/cleared; the
    /// event then carries `run_file: null` **and** `run_file_error: "<detail>"`
    /// (never a path to a missing file). The failure is surfaced as an `alert`
    /// event/`report.alerts` entry (`kind=run_file_write`) and counted in
    /// `report.run_file_errors` (P1-3), so it never degrades into a silent
    /// "no run history" state.
    #[allow(clippy::too_many_arguments)]
    fn write_run_entity(
        &mut self,
        entry: &Entry,
        task: &str,
        attempt: u32,
        resume: bool,
        member: Option<MemberRunContext<'_>>,
        result: &AttemptResult,
        clicks_file: Option<String>,
        out: &mut dyn FnMut(&str),
    ) -> Option<RunRef> {
        let run_id = self.next_run_id(result.started_ms);
        let run_file = format!("{SCHEDULER_RUN_DIR}/{run_id}.json");
        let (kind, queue_id, round, member_index) = match member {
            Some(m) => (
                "queue_member",
                Some(m.queue_id.to_string()),
                Some(m.round),
                Some(m.member_index),
            ),
            None => ("task", None, None, None),
        };
        let record = SchedulerRun {
            schema_version: RUN_SCHEMA_VERSION,
            run_id: run_id.clone(),
            source: "scheduler".to_string(),
            kind: kind.to_string(),
            entry_id: entry.entry_id.clone(),
            task: task.to_string(),
            queue_id,
            round,
            member_index,
            attempt,
            resume,
            started_ms: result.started_ms,
            finished_ms: result.finished_ms,
            duration_ms: result.duration_ms,
            class: result.class.as_str().to_string(),
            exit: result.exit.clone(),
            exit_code: result.exit_code,
            outcome: result.outcome.clone(),
            clicks: result.clicks,
            error: result.error.clone(),
            clicks_file,
        };
        let path = self
            .opts
            .runs_dir
            .join(SCHEDULER_RUN_DIR)
            .join(format!("{run_id}.json"));
        match write_json_atomic(&path, &record) {
            Ok(()) => {
                self.last_run_error = None;
                out(&format!(
                    "scheduler_run_file run_id={run_id} file={run_file} class={} exit_code={}",
                    record.class, record.exit_code
                ));
                Some(RunRef {
                    run_file,
                    exit_code: result.exit_code,
                })
            }
            Err(e) => {
                out(&format!(
                    "scheduler_run_file_error run_id={run_id} file={run_file} error={e}"
                ));
                eprintln!("scheduler: run file warn: {e}");
                // P1-3: a lost run entity is never silent. The attempt already
                // ran, so this stays non-fatal (execution/state continue); the
                // alert + counter only make the lost evidence observable.
                self.report.run_file_errors += 1;
                self.last_run_error =
                    Some(format!("run_id={run_id} file={run_file} error={e}"));
                self.alert(
                    out,
                    "run_file_write",
                    &entry.entry_id,
                    &format!(
                        "entry={} task={task} attempt={attempt} run_id={run_id} \
                         file={run_file} error={e}",
                        entry.entry_id
                    ),
                );
                None
            }
        }
    }

    /// `(exit_code, run_file, run_file_error)` of the most recent attempt for
    /// the aggregate `run_finished` event. `run_file` is `None` when that
    /// attempt produced no run entity; `run_file_error` distinguishes a write
    /// failure (`Some`) from "no attempt ran" (`None`). `exit_code` falls back
    /// to the entry class.
    fn last_run_event_fields(
        &self,
        class: ResultClass,
    ) -> (u8, Option<String>, Option<String>) {
        match &self.last_run {
            Some(r) => (r.exit_code, Some(r.run_file.clone()), None),
            None => (Self::class_exit_code(class), None, self.last_run_error.clone()),
        }
    }

    fn plan_now(&self, now: i64) -> PassPlan {
        plan_pass(
            self.sched(),
            &self.state,
            now,
            self.recovery.as_ref(),
        )
    }

    /// Log a pass plan (`scheduler_plan` / `scheduler_skip` lines).
    fn log_plan(&self, plan: &PassPlan, out: &mut dyn FnMut(&str)) {
        for p in &plan.runnable {
            out(&format!(
                "scheduler_plan entry={} task={} lifecycle={} reason={} resume={}",
                p.entry_id,
                p.task,
                p.lifecycle.as_str(),
                p.reason,
                p.resume
            ));
        }
        for s in &plan.skipped {
            out(&format!(
                "scheduler_skip entry={} task={} reason={}",
                s.entry_id, s.task, s.reason
            ));
        }
    }

    /// Replay a round whose entry-level result was staged by `queue_end_round`
    /// but never applied (P1-2 exactly-once window).
    ///
    /// The queue may already be `finished`, so `plan_pass` will never trigger it
    /// again: this must run at startup, before the first pass. The mutation is
    /// persisted by the caller's first `save_state` (always before the first
    /// pass); a crash before that write simply replays the record on the next
    /// start — the flag is cleared in the same write as `record_result`, so the
    /// accounting is applied **exactly once**.
    ///
    /// A record whose entry disappeared (or stopped being a queue entry) cannot
    /// be attributed any more: it is dropped **loudly** (log + audit) instead of
    /// silently double-applying or bricking the daemon on corrupt state.
    ///
    /// The staged class is not trusted blindly: it is recomputed from the round's
    /// persisted `member_results` (P1-1's source of truth). The staged value is
    /// still written to the audit as `staged_class` for traceability.
    fn recover_pending_entry_records(&mut self, out: &mut dyn FnMut(&str)) {
        let pending: Vec<(String, scheduler::PendingEntryRecord)> = self
            .state
            .queues
            .iter()
            .filter_map(|(queue_id, st)| {
                st.pending_entry_record
                    .clone()
                    .map(|rec| (queue_id.clone(), rec))
            })
            .collect();
        for (queue_id, rec) in pending {
            let entry = self.sched().entry(&rec.entry_id).cloned();
            let Some(entry) = entry else {
                self.drop_pending_entry_record(&queue_id, &rec, "entry_missing", out);
                continue;
            };
            if entry.kind != EntryKind::Queue {
                self.drop_pending_entry_record(&queue_id, &rec, "entry_not_queue", out);
                continue;
            }
            let queue = self.sched().queue(&queue_id).cloned();
            let Some(queue) = queue else {
                self.drop_pending_entry_record(&queue_id, &rec, "queue_missing", out);
                continue;
            };
            // P1-1/P1-2: recompute the class from the persisted member results
            // (the same source of truth a resumed round uses) instead of
            // trusting the staged string. No rows for the round means the state
            // is corrupt/hand-edited: drop loudly, never invent a round result.
            let summary = self
                .state
                .queues
                .get(&queue_id)
                .map(|st| scheduler::queue_round_summary(&queue, st, rec.round));
            let Some(summary) = summary else {
                self.drop_pending_entry_record(&queue_id, &rec, "queue_state_missing", out);
                continue;
            };
            if summary.recorded_members == 0 {
                self.drop_pending_entry_record(&queue_id, &rec, "round_results_missing", out);
                continue;
            }
            let class = summary.class();
            let (exit, outcome) = summary.exit_outcome();
            let now = now_ms() as i64;
            let cycles = cycle_ids(&self.sched().calendar, now);
            let effects = record_result(
                &mut self.state,
                &entry,
                &cycles,
                now,
                class,
                exit,
                outcome,
                summary.attempts,
                rec.duration_ms,
            );
            if let Some(st) = self.state.queues.get_mut(&queue_id) {
                st.pending_entry_record = None;
            }
            // The round is closed, so a dirty marker that still names this entry
            // is stale (the original process died before clearing it).
            let running_was_this_entry = self
                .state
                .running
                .as_ref()
                .map(|r| r.entry_id == rec.entry_id)
                .unwrap_or(false);
            if running_was_this_entry {
                self.state.running = None;
                if self
                    .recovery
                    .as_ref()
                    .map(|r| r.entry_id == rec.entry_id)
                    .unwrap_or(false)
                {
                    self.recovery = None;
                }
            }
            out(&format!(
                "scheduler_queue_entry_record_recovered entry={} queue={queue_id} round={} class={} attempts={} clicks={} quota_used={} cooldown_until_ms={}",
                rec.entry_id,
                rec.round,
                class.as_str(),
                summary.attempts,
                summary.clicks,
                effects
                    .quota_used
                    .map(|used| used.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                effects.cooldown_until_ms
            ));
            self.audit(json!({
                "event": "queue_entry_record_recovered",
                "applied": true,
                "entry_id": rec.entry_id,
                "queue_id": queue_id,
                "round": rec.round,
                "class": class.as_str(),
                "staged_class": rec.class,
                "exit": exit,
                "outcome": outcome,
                "attempts": summary.attempts,
                "clicks": summary.clicks,
                "duration_ms": rec.duration_ms,
                "quota_used": effects.quota_used,
                "cooldown_until_ms": effects.cooldown_until_ms,
                "finished_ms": rec.finished_ms,
            }));
        }
    }

    /// Drop a staged entry result that can no longer be attributed (loudly).
    fn drop_pending_entry_record(
        &mut self,
        queue_id: &str,
        rec: &scheduler::PendingEntryRecord,
        reason: &str,
        out: &mut dyn FnMut(&str),
    ) {
        if let Some(st) = self.state.queues.get_mut(queue_id) {
            st.pending_entry_record = None;
        }
        out(&format!(
            "scheduler_queue_entry_record_dropped entry={} queue={queue_id} round={} reason={reason}",
            rec.entry_id, rec.round
        ));
        self.audit(json!({
            "event": "queue_entry_record_recovered",
            "applied": false,
            "reason": reason,
            "entry_id": rec.entry_id,
            "queue_id": queue_id,
            "round": rec.round,
            "class": rec.class,
        }));
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct PassStats {
    ran: u64,
    fatal_stop: bool,
}

// ---------------------------------------------------------------------------
// Paths for the run
// ---------------------------------------------------------------------------

fn task_checkpoint_path(tasks_dir: &Path, task: &str) -> PathBuf {
    tasks_dir.join(task).join("state").join("checkpoint.json")
}

/// Remove the task's `state/` contents (checkpoint, learned coords, counters).
/// Returns the removed file names. Missing dir = no-op.
fn clear_task_state_dir(tasks_dir: &Path, task: &str) -> Result<Vec<String>, String> {
    let dir = tasks_dir.join(task).join("state");
    if !dir.exists() {
        return Ok(Vec::new());
    }
    if !dir.is_dir() {
        std::fs::remove_file(&dir).map_err(|e| format!("remove `{}`: {e}", dir.display()))?;
        return Ok(vec!["state".to_string()]);
    }
    let mut removed = Vec::new();
    let entries =
        std::fs::read_dir(&dir).map_err(|e| format!("read `{}`: {e}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("read `{}`: {e}", dir.display()))?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            std::fs::remove_dir_all(&path)
                .map_err(|e| format!("remove `{}`: {e}", path.display()))?;
        } else {
            std::fs::remove_file(&path).map_err(|e| format!("remove `{}`: {e}", path.display()))?;
        }
        removed.push(name);
    }
    Ok(removed)
}

fn ensure_dir(path: &Path) -> Result<(), String> {
    std::fs::create_dir_all(path).map_err(|e| format!("create `{}`: {e}", path.display()))
}

// ---------------------------------------------------------------------------
// Execution of one planned entry
// ---------------------------------------------------------------------------

fn signal_of(report: &RunReport) -> RunSignal {
    match &report.result {
        RunResult::Finished(exit) => RunSignal::Exit(*exit),
        RunResult::Failed(_) => RunSignal::RunFailed,
        RunResult::Exhausted(_) => RunSignal::Exhausted,
    }
}

/// The outcome of **one execution attempt** (L2). Production = `run::execute`
/// + `classify_signal`; tests script it through the `test_exec` seam so a queue
/// round is deterministic without a device/replay (P2 task D).
///
/// P4: also carries the per-attempt timing and the report exit code so exactly
/// one immutable run entity can be written per attempt.
#[derive(Debug, Clone)]
struct AttemptResult {
    class: ResultClass,
    exit: String,
    outcome: String,
    clicks: usize,
    error: Option<String>,
    /// `RunReport::exit_code()` (0 = Done, 1 = Failed/Exhausted; a setup error
    /// without a report is 1).
    exit_code: u8,
    started_ms: u64,
    finished_ms: u64,
    duration_ms: u64,
}

impl<'a> Session<'a> {
    /// Execute **one attempt** of `task` for `entry` — the single execution
    /// seam shared by task entries and queue members (P2 task D: one helper,
    /// not two).
    ///
    /// Writes the `running` marker with the **executed task name** (for a queue
    /// entry that is the member, not the queue_id — `reset-memory` and
    /// `--status` match on it), saves state, logs `scheduler_run`, then calls
    /// the engine (`run::execute` + `classify_signal`). Under `#[cfg(test)]` a
    /// scripted result from `SchedulerOptions::test_exec` replaces only the
    /// engine call: the marker/state/log bookkeeping stays identical.
    ///
    /// P4: every attempt (retries and scripted test attempts included) writes
    /// one run entity `<runs_dir>/scheduler/<run_id>.json` and leaves its
    /// reference in `Session::last_run` for the aggregate `run_finished` event.
    fn run_task_attempt(
        &mut self,
        entry: &Entry,
        task: &str,
        pass: u64,
        attempt: u32,
        resume: bool,
        member: Option<MemberRunContext<'_>>,
        out: &mut dyn FnMut(&str),
    ) -> Result<AttemptResult, String> {
        // Resolve the explicit param layers before touching scheduler state: a
        // corrupt Runner values store is a loud config error and must fail fast
        // without leaving a dirty `running` marker behind (same contract as an
        // invalid `scheduler.json`).
        let params = attempt_run_params(&self.opts, task, entry)?;
        let meta = meta_exits_for(&self.loaded.metas, task);
        scheduler::mark_running_for(
            &mut self.state,
            &entry.entry_id,
            task,
            pass,
            attempt,
            now_ms(),
            resume,
        );
        self.save_state(out)?;

        // Test seam: capture the marker that was just persisted so a test can
        // assert `mark_running` named the member (not the queue_id).
        #[cfg(test)]
        if let Some(exec) = &self.opts.test_exec {
            exec.observe_running(self.state.running.clone());
        }

        let seq = self.seq;
        self.seq += 1;
        let clicks_out = self.opts.clicks_dir.as_ref().map(|dir| {
            dir.join(format!("{seq:05}-{task}-p{pass}-a{attempt}.clicks.jsonl"))
        });
        out(&format!(
            "scheduler_run entry={} task={task} pass={pass} attempt={attempt} resume={resume} lifecycle={}",
            entry.entry_id,
            entry.lifecycle.as_str()
        ));

        let started_ms = now_ms();

        #[cfg(test)]
        if let Some(scripted) = self
            .opts
            .test_exec
            .as_ref()
            .and_then(|exec| exec.next(task, attempt, resume))
        {
            let finished_ms = now_ms();
            let result = AttemptResult {
                class: scripted.class,
                exit: scripted.exit,
                outcome: scripted.outcome,
                clicks: scripted.clicks,
                error: scripted.error,
                exit_code: scripted.exit_code,
                started_ms,
                finished_ms,
                duration_ms: finished_ms.saturating_sub(started_ms),
            };
            // The scripted seam never runs `run::execute`, so no click audit
            // file exists: `clicks_file` stays absent (the run entity is still
            // written, exactly like production).
            self.last_run =
                self.write_run_entity(entry, task, attempt, resume, member, &result, None, out);
            return Ok(result);
        }

        // Priority §C.3 layers 2+3 were resolved above (`params`): Runner task
        // values underneath, entry params on top; no CLI layer on this path, and
        // `meta.params[].default` is merged last inside `run::execute`.
        let run_opts = RunOptions {
            task: Some(self.opts.tasks_dir.join(task)),
            replay: self.opts.replay.clone(),
            seed: self
                .opts
                .seed
                .map(|s| s.saturating_add(u64::from(attempt.saturating_sub(1)))),
            clicks_out,
            shots_dir: self.opts.shots_dir.clone(),
            library: self.opts.library.clone(),
            resume,
            model: self.opts.model.clone(),
            det_model: self.opts.det_model.clone(),
            params,
            // The pass already holds the device lease (PR6 P1-5): the attempt
            // must not acquire it again (a second acquire is a same-pid
            // conflict by design).
            skip_lease: true,
            ..RunOptions::default()
        };
        let result = match run::execute(&run_opts, out) {
            Ok(report) => {
                let signal = signal_of(&report);
                AttemptResult {
                    class: classify_signal(signal, &meta),
                    exit: signal.name().to_string(),
                    outcome: format!("{:?}", report.outcome()),
                    clicks: report.clicks.len(),
                    error: None,
                    exit_code: report.exit_code(),
                    started_ms,
                    finished_ms: 0, // filled below, after the engine returned
                    duration_ms: 0,
                }
            }
            Err(e) => {
                out(&format!(
                    "scheduler_run_error entry={} task={task} attempt={attempt}: {e}",
                    entry.entry_id
                ));
                AttemptResult {
                    class: classify_signal(RunSignal::RunFailed, &meta),
                    exit: "run_error".to_string(),
                    outcome: "error".to_string(),
                    clicks: 0,
                    error: Some(e),
                    exit_code: 1,
                    started_ms,
                    finished_ms: 0,
                    duration_ms: 0,
                }
            }
        };
        let finished_ms = now_ms();
        let result = AttemptResult {
            finished_ms,
            duration_ms: finished_ms.saturating_sub(started_ms),
            ..result
        };
        // A click audit file exists only when the engine got far enough to write
        // it (setup errors may abort earlier); never claim a missing file.
        let clicks_file = run_opts
            .clicks_out
            .as_ref()
            .filter(|p| p.is_file())
            .map(|p| p.display().to_string());
        self.last_run =
            self.write_run_entity(entry, task, attempt, resume, member, &result, clicks_file, out);
        Ok(result)
    }

    /// Resolve `resume` for one attempt: only when requested, and only when a
    /// checkpoint actually exists (degrade loudly to a fresh run otherwise).
    fn attempt_resume(
        &self,
        entry: &Entry,
        task: &str,
        want_resume: bool,
        out: &mut dyn FnMut(&str),
    ) -> bool {
        if !want_resume {
            return false;
        }
        if task_checkpoint_path(&self.opts.tasks_dir, task).is_file() {
            true
        } else {
            out(&format!(
                "scheduler_resume_skipped entry={} task={task} reason=no_checkpoint",
                entry.entry_id
            ));
            false
        }
    }

    fn run_entry(
        &mut self,
        entry: &Entry,
        planned: &scheduler::PlannedRun,
        pass: u64,
        out: &mut dyn FnMut(&str),
    ) -> Result<ResultClass, String> {
        let mut want_resume = planned.resume;
        let mut attempts: u32 = 0;
        let mut clicks_total: usize = 0;
        let exec_started = now_ms();
        // The run_finished event must never reference the previous entry's file.
        self.last_run = None;
        self.last_run_error = None;

        let (class, exit, outcome, error) = loop {
            attempts += 1;
            let attempt = attempts;

            // Temp rule: execution state is reset before *and* after every run
            // (§22.34.3) — retries included, so a stale checkpoint never leaks.
            if entry.lifecycle == Lifecycle::Temp {
                let removed = clear_task_state_dir(&self.opts.tasks_dir, &entry.task)
                    .map_err(|e| format!("temp state reset: {e}"))?;
                out(&format!(
                    "scheduler_temp_reset_before entry={} task={} removed={}",
                    entry.entry_id,
                    entry.task,
                    removed.len()
                ));
                self.audit(json!({
                    "event": "temp_state_reset",
                    "phase": "before",
                    "entry_id": entry.entry_id,
                    "task": entry.task,
                    "removed": removed,
                }));
            }

            let resume = self.attempt_resume(entry, &entry.task, want_resume, out);
            let result =
                self.run_task_attempt(entry, &entry.task, pass, attempt, resume, None, out)?;
            clicks_total += result.clicks;
            if should_retry(result.class, attempt, entry) {
                out(&format!(
                    "scheduler_retry entry={} task={} attempt={attempt} class={}",
                    entry.entry_id,
                    entry.task,
                    result.class.as_str()
                ));
                // Retry after a failure: resume from the checkpoint when the
                // run left one (periodic only; temp state is reset above).
                want_resume = entry.lifecycle == Lifecycle::Periodic;
                continue;
            }
            break (result.class, result.exit, result.outcome, result.error);
        };

        let total_duration = now_ms().saturating_sub(exec_started);

        // A completed execution is never "dirty" any more.
        clear_running(&mut self.state);
        let now = now_ms() as i64;
        let cycles = cycle_ids(&self.sched().calendar, now);
        let effects = record_result(
            &mut self.state,
            entry,
            &cycles,
            now,
            class,
            &exit,
            &outcome,
            attempts,
            total_duration,
        );

        if entry.lifecycle == Lifecycle::Temp {
            let removed = clear_task_state_dir(&self.opts.tasks_dir, &entry.task)
                .map_err(|e| format!("temp state reset (after): {e}"))?;
            out(&format!(
                "scheduler_temp_reset_after entry={} task={} removed={}",
                entry.entry_id,
                entry.task,
                removed.len()
            ));
            self.audit(json!({
                "event": "temp_state_reset",
                "phase": "after",
                "entry_id": entry.entry_id,
                "task": entry.task,
                "removed": removed,
            }));
        }

        self.save_state(out)?;

        if let Some(recovered) = &self.recovery {
            if recovered.entry_id == entry.entry_id {
                self.recovery = None;
            }
        }

        out(&format!(
            "scheduler_run_done entry={} task={} pass={pass} attempts={attempts} class={} exit={} outcome={} clicks={} duration_ms={} cooldown_until_ms={}",
            entry.entry_id,
            entry.task,
            class.as_str(),
            exit,
            outcome,
            clicks_total,
            total_duration,
            effects.cooldown_until_ms
        ));
        if let Some(err) = &error {
            out(&format!(
                "scheduler_run_detail entry={} error={err}",
                entry.entry_id
            ));
        }
        let (run_exit_code, run_file, run_file_error) = self.last_run_event_fields(class);
        self.audit(json!({
            "event": "run_finished",
            "entry_id": entry.entry_id,
            "task": entry.task,
            "pass": pass,
            "attempts": attempts,
            "class": class.as_str(),
            "exit": exit,
            "outcome": outcome,
            "clicks": clicks_total,
            "duration_ms": total_duration,
            "cooldown_until_ms": effects.cooldown_until_ms,
            "quota_used": effects.quota_used,
            // P4 additive fields (the per-attempt run entity of the last attempt;
            // `run_file` is relative to `--runs-dir`). `run_file_error` is
            // non-null only when that attempt's entity could not be written
            // (P1-3), so consumers can tell "no attempt" from "lost evidence".
            "exit_code": run_exit_code,
            "run_file": run_file,
            "run_file_error": run_file_error,
        }));

        self.report.runs += 1;
        match class {
            ResultClass::Success => self.report.successes += 1,
            ResultClass::Resource => self.report.resources += 1,
            ResultClass::Blocked => {
                self.report.blocked += 1;
                self.alert(
                    out,
                    "task_blocked",
                    &entry.entry_id,
                    &format!(
                        "task={} exit={exit} cooldown_s={}",
                        entry.task, entry.cooldown_s
                    ),
                );
            }
            ResultClass::Retryable => self.report.failures += 1,
            ResultClass::Fatal => {
                self.report.fatals += 1;
                self.alert(
                    out,
                    "task_fatal",
                    &entry.entry_id,
                    &format!("task={} exit={exit}", entry.task),
                );
            }
        }

        Ok(class)
    }

    /// Rule 4 lightweight member precheck: the bundle must still exist.
    ///
    /// `load_full` already fail-fasts on a missing member at startup; this
    /// catches a bundle deleted while the daemon runs. A skipped member is
    /// recorded with `attempts = 0` (lazy counting then treats the round as
    /// empty, rule 5). Tests inject skips through the same seam
    /// (`TestExec::skip_member`) to exercise the empty-round path
    /// deterministically.
    fn member_skip_reason(&self, member: &str) -> Option<String> {
        #[cfg(test)]
        if let Some(exec) = &self.opts.test_exec {
            if exec.skips(member) {
                return Some("test_injected".to_string());
            }
        }
        if !self
            .opts
            .tasks_dir
            .join(member)
            .join("meta.json")
            .is_file()
        {
            return Some("task_missing".to_string());
        }
        None
    }

    /// Run one queue member with `1 + entry.retry` attempts (queue.rs parity:
    /// `entry.retry = 0` ⇒ exactly one attempt). Only `retryable` retries (the
    /// same `should_retry` as a task entry); between attempts it sleeps
    /// `queue.between_ms` and resumes from `tasks/<member>/state/checkpoint.json`
    /// when one exists (`oasd queue` `resume_on_retry` default = true).
    fn run_queue_member(
        &mut self,
        entry: &Entry,
        queue: &scheduler::Queue,
        member: &str,
        pass: u64,
        round: u64,
        member_index: usize,
        resume_first: bool,
        out: &mut dyn FnMut(&str),
    ) -> Result<scheduler::QueueMemberOutcome, String> {
        let started = now_ms();
        let mut attempts: u32 = 0;
        let mut clicks: usize = 0;
        let mut want_resume = resume_first;
        loop {
            attempts += 1;
            let resume = self.attempt_resume(entry, member, want_resume, out);
            if attempts == 1 {
                self.audit(json!({
                    "event": "queue_member_started",
                    "entry_id": entry.entry_id,
                    "queue_id": queue.queue_id,
                    "round": round,
                    "task": member,
                    "pass": pass,
                    "resume": resume,
                    "max_attempts": entry.max_attempts(),
                }));
            }
            let member_ctx = MemberRunContext {
                queue_id: &queue.queue_id,
                round,
                member_index,
            };
            let result = self.run_task_attempt(
                entry,
                member,
                pass,
                attempts,
                resume,
                Some(member_ctx),
                out,
            )?;
            clicks += result.clicks;
            if should_retry(result.class, attempts, entry) {
                out(&format!(
                    "scheduler_queue_member_retry entry={} queue={} round={round} task={member} attempt={attempts} class={}",
                    entry.entry_id,
                    queue.queue_id,
                    result.class.as_str()
                ));
                // queue.rs `resume_on_retry` default: resume on retry when the
                // failed attempt left a checkpoint.
                want_resume = true;
                if queue.between_ms > 0 {
                    std::thread::sleep(Duration::from_millis(queue.between_ms));
                }
                continue;
            }
            let duration = now_ms().saturating_sub(started);
            return Ok(scheduler::QueueMemberOutcome::new(
                result.class,
                &result.exit,
                &result.outcome,
                attempts,
                duration,
            )
            .with_clicks(clicks));
        }
    }

    /// Execute one queue **round** for a `kind = "queue"` entry (L4; plan §二
    /// rules 3/5/6/7/8 and §三 P2).
    ///
    /// 1. resolve the queue (`entry.task` is the queue_id) — a missing queue is
    ///    recorded as `blocked` loudly, never a panic;
    /// 2. `queue_should_finish` ⇒ skip (rule 6);
    /// 3. resume `current_round` (restart) or open a new one (rule 8);
    /// 4. run the members from `member_index` serially, recording each one and
    ///    saving state before the next side effect;
    /// 5. close the round with the pure lazy-count rule (`queue_end_round`) and
    ///    record one entry-level result through `record_result` (quota/cooldown
    ///    live on the entry, rule 2).
    fn execute_queue_round(
        &mut self,
        entry: &Entry,
        planned: &scheduler::PlannedRun,
        pass: u64,
        out: &mut dyn FnMut(&str),
    ) -> Result<ResultClass, String> {
        // The run_finished event must never reference the previous entry's file
        // (an empty round has no run entity at all).
        self.last_run = None;
        self.last_run_error = None;
        let Some(queue) = entry
            .queue_id()
            .and_then(|queue_id| self.sched().queue(queue_id))
            .cloned()
        else {
            // Validation rejects an unknown queue reference; this is defence in
            // depth. Record a blocked result (with cooldown) instead of running.
            out(&format!(
                "scheduler_queue_error entry={} queue={} reason=queue_missing",
                entry.entry_id, entry.task
            ));
            self.audit(json!({
                "event": "queue_missing",
                "entry_id": entry.entry_id,
                "queue_id": entry.task,
                "pass": pass,
            }));
            self.alert(
                out,
                "queue_missing",
                &entry.entry_id,
                &format!("queue={} missing from queues[]", entry.task),
            );
            scheduler::clear_running(&mut self.state);
            let now = now_ms() as i64;
            let cycles = cycle_ids(&self.sched().calendar, now);
            record_result(
                &mut self.state,
                entry,
                &cycles,
                now,
                ResultClass::Blocked,
                "queue_missing",
                "error",
                0,
                0,
            );
            self.save_state(out)?;
            self.report.runs += 1;
            self.report.blocked += 1;
            return Ok(ResultClass::Blocked);
        };
        let queue_id = queue.queue_id.clone();

        // 1) Ensure the per-queue state exists (config mirror) and apply rule 6.
        {
            let st = self
                .state
                .queues
                .entry(queue_id.clone())
                .or_insert_with(|| scheduler::QueueState {
                    total_rounds: queue.total_rounds,
                    ..scheduler::QueueState::default()
                });
            if scheduler::queue_should_finish(&queue, st) {
                out(&format!(
                    "scheduler_queue_skip entry={} queue={queue_id} reason=queue_finished",
                    entry.entry_id
                ));
                return Ok(ResultClass::Blocked);
            }
        }

        // 2) Resume the open round (rule 8) or open a new one. Re-beginning an
        //    open round would be a state-machine error (`queue_begin_round`
        //    rejects it) and would double-count the round after a crash.
        let open_round = self
            .state
            .queues
            .get(&queue_id)
            .and_then(|st| st.current_round.clone());
        let round_index: u64;
        if let Some(round) = open_round {
            round_index = round.index;
            out(&format!(
                "scheduler_queue_round_resume entry={} queue={queue_id} round={round_index} member_index={} pass={pass}",
                entry.entry_id, round.member_index
            ));
            self.audit(json!({
                "event": "queue_round_resumed",
                "entry_id": entry.entry_id,
                "queue_id": queue_id,
                "round": round_index,
                "member_index": round.member_index,
                "pass": pass,
            }));
        } else {
            round_index = {
                let st = self
                    .state
                    .queues
                    .get_mut(&queue_id)
                    .ok_or_else(|| format!("queue round: `{queue_id}` state vanished"))?;
                scheduler::queue_begin_round(&queue, st, now_ms() as i64)
                    .map_err(|e| format!("queue round: {e}"))?
            };
            self.save_state(out)?;
            out(&format!(
                "scheduler_queue_round_start entry={} queue={queue_id} round={round_index} members={} pass={pass}",
                entry.entry_id,
                queue.members.len()
            ));
            self.audit(json!({
                "event": "queue_round_started",
                "entry_id": entry.entry_id,
                "queue_id": queue_id,
                "round": round_index,
                "members": queue.members.len(),
                "pass": pass,
            }));
        }

        let member_total = queue.members.len();
        let start_index = {
            let st = self
                .state
                .queues
                .get(&queue_id)
                .ok_or_else(|| format!("queue round: `{queue_id}` state vanished"))?;
            scheduler::queue_next_member_index(&queue, st)
        };
        if start_index > 0 {
            out(&format!(
                "scheduler_queue_round_resume_from entry={} queue={queue_id} round={round_index} member_index={start_index}",
                entry.entry_id
            ));
        }

        let round_started = now_ms();
        // P1-1: the round's aggregates come from the **whole persisted round**
        // (members recorded before a crash) plus every member this process
        // records below. Classifying from process-local counters alone lost a
        // prior member's class: `success` + later failure became `retryable`
        // (no quota) and a recorded `fatal` disappeared.
        let mut summary = self
            .state
            .queues
            .get(&queue_id)
            .map(|st| scheduler::queue_round_summary(&queue, st, round_index))
            .ok_or_else(|| format!("queue round: `{queue_id}` state vanished"))?;
        let mut aborted = summary.aborted;
        if aborted {
            // The crash happened after a member that already decided the round
            // (a `fatal` member, or a `fail_fast` failure) was recorded but
            // before the round was closed. Do not run the remaining members:
            // the in-process path aborts at that member too (rule 7).
            out(&format!(
                "scheduler_queue_round_abort entry={} queue={queue_id} round={round_index} reason={} resumed=true",
                entry.entry_id,
                if summary.fatal { "fatal" } else { "fail_fast" }
            ));
        }

        // 3) Members from the persisted cursor to the tail, serially (rule 3/10).
        for member_index in start_index..member_total {
            if aborted {
                // The round was already decided before this process started
                // (logged above); never run another member.
                break;
            }
            let member = queue.members[member_index].clone();
            let skip_reason = self.member_skip_reason(&member);
            let skipped = skip_reason.is_some();
            let outcome: scheduler::QueueMemberOutcome = match skip_reason {
                Some(reason) => {
                    out(&format!(
                        "scheduler_queue_member_skip entry={} queue={queue_id} round={round_index} member={member_index} task={member} reason={reason}",
                        entry.entry_id
                    ));
                    // No execution (rule 4 precheck): attempts = 0 keeps rule 5
                    // lazy counting honest.
                    scheduler::QueueMemberOutcome::new(
                        ResultClass::Blocked,
                        "skipped",
                        &reason,
                        0,
                        0,
                    )
                }
                None => {
                    // Resume only the member the dirty `running` marker names
                    // (the one that was executing when the process died). Other
                    // members start fresh even when the *round* is resumed, so a
                    // stale checkpoint from an earlier round is never replayed.
                    let resume_member = planned.resume
                        && self
                            .state
                            .running
                            .as_ref()
                            .map(|r| r.entry_id == entry.entry_id && r.task == member)
                            .unwrap_or(false);
                    let outcome = self.run_queue_member(
                        entry,
                        &queue,
                        &member,
                        pass,
                        round_index,
                        member_index,
                        resume_member,
                        out,
                    )?;
                    outcome
                }
            };

            // Record the member (advances `member_index`) and clear the running
            // marker in the **same atomic write**: a crash before this point
            // re-runs the member; the round is still counted exactly once.
            {
                let st = self
                    .state
                    .queues
                    .get_mut(&queue_id)
                    .ok_or_else(|| format!("queue round: `{queue_id}` state vanished"))?;
                scheduler::queue_record_member(st, &member, &outcome, now_ms() as i64)
                    .map_err(|e| format!("queue round: {e}"))?;
            }
            scheduler::clear_running(&mut self.state);
            self.save_state(out)?;

            // P1-1: recompute the round aggregates from the persisted results
            // (never from process-local counters); this is the single source of
            // truth for the abort decision and the final entry-level class.
            summary = {
                let st = self
                    .state
                    .queues
                    .get(&queue_id)
                    .ok_or_else(|| format!("queue round: `{queue_id}` state vanished"))?;
                scheduler::queue_round_summary(&queue, st, round_index)
            };
            aborted = summary.aborted;

            self.audit(json!({
                "event": "queue_member_finished",
                "entry_id": entry.entry_id,
                "queue_id": queue_id,
                "round": round_index,
                "member_index": member_index,
                "task": member,
                "class": outcome.class.as_str(),
                "exit": outcome.exit.clone(),
                "outcome": outcome.outcome.clone(),
                "attempts": outcome.attempts,
                "clicks": outcome.clicks,
                "duration_ms": outcome.duration_ms,
                "skipped": skipped,
                "pass": pass,
            }));
            out(&format!(
                "scheduler_queue_member_done entry={} queue={queue_id} round={round_index} member={member_index} task={member} attempts={} class={} exit={} outcome={} clicks={} duration_ms={} skipped={skipped}",
                entry.entry_id,
                outcome.attempts,
                outcome.class.as_str(),
                outcome.exit,
                outcome.outcome,
                outcome.clicks,
                outcome.duration_ms
            ));

            if aborted {
                out(&format!(
                    "scheduler_queue_round_abort entry={} queue={queue_id} round={round_index} reason={}",
                    entry.entry_id,
                    if summary.fatal { "fatal" } else { "fail_fast" }
                ));
                break;
            }
            // Pause between member executions (never after the last one).
            if member_index + 1 < member_total && queue.between_ms > 0 {
                std::thread::sleep(Duration::from_millis(queue.between_ms));
            }
        }

        // 4) Close the round with lazy counting (rule 5): only a round where at
        //    least one member actually executed consumes `total_rounds`; a fully
        //    skipped round only bumps `empty_rounds` (no quota burn, no spin).
        //    The entry-level class is decided from the whole round and staged in
        //    the same atomic write (P1-2 exactly-once window).
        let class = summary.class();
        let (exit, outcome) = summary.exit_outcome();
        let round_duration = now_ms().saturating_sub(round_started);
        let end = {
            let st = self
                .state
                .queues
                .get_mut(&queue_id)
                .ok_or_else(|| format!("queue round: `{queue_id}` state vanished"))?;
            let end = scheduler::queue_end_round(&queue, st, summary.executed_any())
                .map_err(|e| format!("queue round: {e}"))?;
            // `record_result` below clears the stage in the next save; a crash
            // in between is replayed exactly once at startup.
            st.pending_entry_record = Some(scheduler::PendingEntryRecord {
                entry_id: entry.entry_id.clone(),
                round: end.index,
                class: class.as_str().to_string(),
                exit: exit.to_string(),
                outcome: outcome.to_string(),
                attempts: summary.attempts,
                duration_ms: round_duration,
                finished_ms: now_ms(),
            });
            end
        };
        self.save_state(out)?;

        out(&format!(
            "scheduler_queue_round_done entry={} queue={queue_id} round={round_index} members={member_total} executed={} empty={} aborted={aborted} rounds_completed={} empty_rounds={} finished={} duration_ms={round_duration}",
            entry.entry_id,
            summary.executed_members,
            !summary.executed_any(),
            end.rounds_completed,
            end.empty_rounds,
            end.finished
        ));
        self.audit(json!({
            "event": "queue_round_finished",
            "entry_id": entry.entry_id,
            "queue_id": queue_id,
            "round": end.index,
            "members": member_total,
            "executed_members": summary.executed_members,
            "executed": summary.executed_any(),
            "empty": !summary.executed_any(),
            "aborted": aborted,
            "rounds_completed": end.rounds_completed,
            "empty_rounds": end.empty_rounds,
            "finished": end.finished,
            "duration_ms": round_duration,
            "pass": pass,
        }));
        if !summary.executed_any() {
            out(&format!(
                "scheduler_queue_empty_round entry={} queue={queue_id} round={round_index} empty_rounds={}",
                entry.entry_id, end.empty_rounds
            ));
            self.audit(json!({
                "event": "empty_round",
                "entry_id": entry.entry_id,
                "queue_id": queue_id,
                "round": end.index,
                "empty_rounds": end.empty_rounds,
                "pass": pass,
            }));
        }
        if end.finished {
            out(&format!(
                "scheduler_queue_finished entry={} queue={queue_id} rounds_completed={} total_rounds={}",
                entry.entry_id, end.rounds_completed, queue.total_rounds
            ));
            self.audit(json!({
                "event": "queue_finished",
                "entry_id": entry.entry_id,
                "queue_id": queue_id,
                "rounds_completed": end.rounds_completed,
                "total_rounds": queue.total_rounds,
                "pass": pass,
            }));
        }

        // 5) One entry-level result per round (P2 task C7). The queue itself has
        //    no quota/cooldown (rule 2); they live on the entry. Clear the
        //    pending stage in the **same atomic write** as `record_result`, so
        //    the result is applied exactly once across crashes.
        scheduler::clear_running(&mut self.state);
        let now = now_ms() as i64;
        let cycles = cycle_ids(&self.sched().calendar, now);
        let effects = record_result(
            &mut self.state,
            entry,
            &cycles,
            now,
            class,
            exit,
            outcome,
            summary.attempts,
            round_duration,
        );
        if let Some(st) = self.state.queues.get_mut(&queue_id) {
            st.pending_entry_record = None;
        }
        self.save_state(out)?;

        if let Some(recovered) = &self.recovery {
            if recovered.entry_id == entry.entry_id {
                self.recovery = None;
            }
        }

        out(&format!(
            "scheduler_run_done entry={} task={} pass={pass} attempts={} class={} exit={exit} outcome={outcome} clicks={} duration_ms={round_duration} cooldown_until_ms={}",
            entry.entry_id,
            queue_id,
            summary.attempts,
            class.as_str(),
            summary.clicks,
            effects.cooldown_until_ms
        ));
        // P4 additive fields: the last executed member attempt's run entity is
        // the representative file for this round (all per-member attempts are
        // independently listed by `GET /runs/scheduler`). `run_file_error` is
        // non-null only when that attempt's entity could not be written (P1-3);
        // an empty round leaves both null.
        let (run_exit_code, run_file, run_file_error) = self.last_run_event_fields(class);
        self.audit(json!({
            "event": "run_finished",
            "entry_id": entry.entry_id,
            "task": queue_id,
            "kind": "queue",
            "pass": pass,
            "round": end.index,
            "attempts": summary.attempts,
            "class": class.as_str(),
            "exit": exit,
            "outcome": outcome,
            "clicks": summary.clicks,
            "duration_ms": round_duration,
            "cooldown_until_ms": effects.cooldown_until_ms,
            "quota_used": effects.quota_used,
            "exit_code": run_exit_code,
            "run_file": run_file,
            "run_file_error": run_file_error,
        }));

        self.report.runs += 1;
        match class {
            ResultClass::Success => self.report.successes += 1,
            ResultClass::Resource => self.report.resources += 1,
            ResultClass::Blocked => {
                self.report.blocked += 1;
                self.alert(
                    out,
                    "queue_blocked",
                    &entry.entry_id,
                    &format!("queue={queue_id} empty_round={}", !summary.executed_any()),
                );
            }
            ResultClass::Retryable => self.report.failures += 1,
            ResultClass::Fatal => {
                self.report.fatals += 1;
                self.alert(
                    out,
                    "queue_fatal",
                    &entry.entry_id,
                    &format!("queue={queue_id} round={round_index}"),
                );
            }
        }

        Ok(class)
    }

    fn execute_pass(
        &mut self,
        pass: u64,
        now: i64,
        plan: &PassPlan,
        out: &mut dyn FnMut(&str),
    ) -> Result<PassStats, String> {
        let mut stats = PassStats::default();
        let guard_txt = plan
            .guard
            .map(|g| g.as_str())
            .unwrap_or("none");
        out(&format!(
            "scheduler_pass pass={pass} runnable={} skipped={} guard={guard_txt}",
            plan.runnable.len(),
            plan.skipped.len()
        ));
        self.audit(json!({
            "event": "pass",
            "pass": pass,
            "runnable": plan.runnable.len(),
            "skipped": plan.skipped.len(),
            "guard": plan.guard.map(|g| g.as_str()),
            "cycles": {"day": plan.cycles.day, "week": plan.cycles.week},
        }));
        if let Some(block) = plan.guard {
            let reason = guard_reason(self.sched(), now, block);
            out(&format!(
                "scheduler_guard blocked={} reason={reason}",
                block.as_str()
            ));
            let id = format!("guard:{}", block.as_str());
            self.alert(out, "guard_blocked", &id, &reason);
            return Ok(stats);
        }
        self.log_plan(plan, out);
        for planned in &plan.runnable {
            let Some(entry) = self
                .sched()
                .entry(&planned.entry_id)
                .cloned()
            else {
                continue;
            };
            // One planned entry per pass. A task entry runs one L2 execution; a
            // queue entry runs one L4 **round** (counted as one `stats.ran`).
            stats.ran += 1;
            let class = match entry.kind {
                EntryKind::Task => self.run_entry(&entry, planned, pass, out)?,
                EntryKind::Queue => self.execute_queue_round(&entry, planned, pass, out)?,
            };
            if class == ResultClass::Fatal && !self.opts.keep_going {
                out(&format!(
                    "scheduler_fatal_stop entry={} (use --keep-going to continue)",
                    entry.entry_id
                ));
                stats.fatal_stop = true;
                break;
            }
        }
        Ok(stats)
    }
}

// ---------------------------------------------------------------------------
// Report/status helpers
// ---------------------------------------------------------------------------

fn build_entry_reports(
    sched: &Scheduler,
    state: &SchedulerState,
    now: i64,
) -> Vec<EntryReport> {
    let cycles = cycle_ids(&sched.calendar, now);
    sched
        .entries
        .iter()
        .map(|entry| {
            let st = state.entries.get(&entry.entry_id);
            let used = quota_used(state, entry, &cycles);
            EntryReport {
                entry_id: entry.entry_id.clone(),
                task: entry.task.clone(),
                lifecycle: entry.lifecycle.as_str().to_string(),
                enabled: entry.enabled,
                runs: st.map(|s| s.runs).unwrap_or(0),
                successes: st.map(|s| s.successes).unwrap_or(0),
                resources: st.map(|s| s.resources).unwrap_or(0),
                blocked: st.map(|s| s.blocked).unwrap_or(0),
                failures: st.map(|s| s.failures).unwrap_or(0),
                last_result: st.map(|s| s.last_result.clone()).unwrap_or_default(),
                last_outcome: st.map(|s| s.last_outcome.clone()).unwrap_or_default(),
                last_exit: st.map(|s| s.last_exit.clone()).unwrap_or_default(),
                last_run_ms: st.map(|s| s.last_run_ms).unwrap_or(0),
                cooldown_until_ms: st.map(|s| s.cooldown_until_ms).unwrap_or(0),
                quota_per: entry.quota.as_ref().map(|q| q.per.as_str().to_string()),
                quota_times: entry.quota.as_ref().map(|q| q.times).unwrap_or(0),
                quota_used: used,
                quota_remaining: quota_remaining(state, entry, &cycles),
                next_due_ms: entry_next_due_ms(sched, state, entry, now),
            }
        })
        .collect()
}

fn write_report(path: &Path, report: &SchedulerReport) -> Result<(), String> {
    write_json_atomic(path, report).map_err(|e| format!("--report: {e}"))
}

/// Summarise the newest round present in `member_results` (P2 `--report` /
/// `--status`). `None` when no member has been recorded yet.
///
/// The `member_results` ring buffer is capped (`MAX_QUEUE_MEMBER_RESULTS`), so
/// the newest round is always complete for queues with <= 200 members.
fn last_round_report(results: &[scheduler::QueueMemberResult]) -> Option<QueueRoundReport> {
    let round = results.iter().map(|m| m.round).max()?;
    let members: Vec<scheduler::QueueMemberResult> = results
        .iter()
        .filter(|m| m.round == round)
        .cloned()
        .collect();
    let executed = members.iter().filter(|m| m.attempts > 0).count() as u64;
    Some(QueueRoundReport {
        round,
        executed,
        recorded: members.len() as u64,
        members,
    })
}

/// Build the per-queue summary used by both `--report` and `--status`
/// (P2: "`--report` 与 status 输出同步").
fn build_queue_reports(sched: &Scheduler, state: &SchedulerState, now: i64) -> Vec<QueueReport> {
    sched
        .queues
        .iter()
        .map(|q| {
            let st = state.queues.get(&q.queue_id);
            let finished = st.map(|s| scheduler::queue_should_finish(q, s)).unwrap_or(false);
            // No state yet = never started = due whenever the entry allows it.
            let next_due = match st {
                Some(s) => scheduler::queue_next_due_ms(q, s, now),
                None => Some(now),
            };
            QueueReport {
                queue_id: q.queue_id.clone(),
                name: q.name.clone(),
                total_rounds: q.total_rounds,
                on_failure: q.on_failure.as_str().to_string(),
                between_ms: q.between_ms,
                rounds_completed: st.map(|s| s.rounds_completed).unwrap_or(0),
                empty_rounds: st.map(|s| s.empty_rounds).unwrap_or(0),
                finished,
                current_round: st.and_then(|s| s.current_round.clone()),
                last_round: st.and_then(|s| last_round_report(&s.member_results)),
                next_due_ms: next_due,
            }
        })
        .collect()
}

/// Machine-readable status for the editor panel (`GET /scheduler`).
///
/// Available as a public function so `serve.rs` (M1 panel) can render the same
/// data without shelling out to the CLI. It never writes anything and never
/// takes the lock: an operator can poll it while the daemon runs.
pub fn status_json(opts: &SchedulerOptions) -> Result<Value, String> {
    let sched = load_scheduler(opts)?;
    let state_path = opts.state_path();
    let state = load_state(&state_path)?;
    let now = now_ms() as i64;
    let plan = plan_pass(&sched, &state, now, state.running.as_ref());
    let wake = next_wake(&sched, &state, now);
    let cycles = cycle_ids(&sched.calendar, now);
    let offset = sched.calendar.utc_offset_min;

    let runnable: BTreeSet<String> = plan.runnable.iter().map(|p| p.entry_id.clone()).collect();
    let mut blocked: BTreeMap<String, String> = BTreeMap::new();
    for s in &plan.skipped {
        blocked.insert(s.entry_id.clone(), s.reason.clone());
    }
    if let Some(block) = plan.guard {
        for e in &sched.entries {
            if e.enabled {
                blocked.insert(e.entry_id.clone(), block.as_str().to_string());
            }
        }
    }

    let entries: Vec<Value> = sched
        .entries
        .iter()
        .map(|entry| {
            let st = state.entries.get(&entry.entry_id);
            let used = quota_used(&state, entry, &cycles);
            // P2: queue progress on the entry itself (rule 5/6/8) so the panel
            // does not have to join `entries[]` with `queues[]` by queue_id.
            let queue = entry.queue_id().map(|queue_id| {
                let qst = state.queues.get(queue_id);
                let queue = sched.queue(queue_id);
                json!({
                    "queue_id": queue_id,
                    "total_rounds": queue.map(|q| q.total_rounds).unwrap_or(0),
                    "rounds_completed": qst.map(|s| s.rounds_completed).unwrap_or(0),
                    "empty_rounds": qst.map(|s| s.empty_rounds).unwrap_or(0),
                    "finished": queue
                        .map(|q| qst.map(|s| scheduler::queue_should_finish(q, s)).unwrap_or(false))
                        .unwrap_or(false),
                    "current_round": qst.and_then(|s| s.current_round.as_ref()).map(|r| json!({
                        "index": r.index,
                        "member_index": r.member_index,
                        "started_ms": r.started_ms,
                    })),
                    "last_round": qst
                        .and_then(|s| last_round_report(&s.member_results))
                        .and_then(|r| serde_json::to_value(r).ok()),
                })
            });
            json!({
                "entry_id": entry.entry_id,
                "task": entry.task,
                "kind": entry.kind.as_str(),
                "queue_id": entry.queue_id(),
                "queue": queue,
                "enabled": entry.enabled,
                "lifecycle": entry.lifecycle.as_str(),
                "windows": entry.windows.iter().map(|w| json!({
                    "days": w.days,
                    "start": w.start,
                    "end": w.end,
                })).collect::<Vec<_>>(),
                "quota": entry.quota.as_ref().map(|q| json!({
                    "per": q.per.as_str(),
                    "times": q.times,
                    "used": used,
                    "remaining": q.times.saturating_sub(used),
                    "cycle": if q.per == scheduler::QuotaPer::Week { cycles.week.clone() } else { cycles.day.clone() },
                })),
                "params": entry.params,
                "retry": entry.retry,
                "cooldown_s": entry.cooldown_s,
                "runnable_now": runnable.contains(&entry.entry_id),
                "blocked_reason": blocked.get(&entry.entry_id),
                "next_due_ms": entry_next_due_ms(&sched, &state, entry, now),
                "next_due_local": entry_next_due_ms(&sched, &state, entry, now)
                    .map(|at| format_local_datetime(at, offset)),
                "cooldown_until_ms": st.map(|s| s.cooldown_until_ms).unwrap_or(0),
                "cooldown_until_local": st
                    .map(|s| s.cooldown_until_ms)
                    .filter(|v| *v > 0)
                    .map(|v| format_local_datetime(v as i64, offset)),
                "runs": st.map(|s| s.runs).unwrap_or(0),
                "successes": st.map(|s| s.successes).unwrap_or(0),
                "resources": st.map(|s| s.resources).unwrap_or(0),
                "blocked": st.map(|s| s.blocked).unwrap_or(0),
                "failures": st.map(|s| s.failures).unwrap_or(0),
                "last_result": st.map(|s| s.last_result.clone()).unwrap_or_default(),
                "last_outcome": st.map(|s| s.last_outcome.clone()).unwrap_or_default(),
                "last_exit": st.map(|s| s.last_exit.clone()).unwrap_or_default(),
                "last_run_ms": st.map(|s| s.last_run_ms).unwrap_or(0),
            })
        })
        .collect();

    // Tasks for the "reset memory" multi-select box (deduped, entries grouped).
    // Queue members are tasks too (they are not scheduler entries of their own).
    let mut choices: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for entry in &sched.entries {
        if entry.kind == EntryKind::Task {
            choices
                .entry(entry.task.clone())
                .or_default()
                .push(entry.entry_id.clone());
        }
    }
    for queue in &sched.queues {
        for member in &queue.members {
            choices.entry(member.clone()).or_default();
        }
    }
    let task_choices: Vec<Value> = choices
        .iter()
        .map(|(task, entry_ids)| {
            let dir = opts.tasks_dir.join(task);
            json!({
                "task": task,
                "entry_ids": entry_ids,
                "bundle_exists": dir.is_dir(),
                "state_dir_exists": dir.join("state").is_dir(),
            })
        })
        .collect();

    // Read-only queue view for P3 (`docs/SCHEDULER-RUNNER-PLAN.md` §二 rules
    // 5/6/8). `--status` never writes queue state; it only reads it.
    let queues: Vec<Value> = sched
        .queues
        .iter()
        .map(|q| {
            let st = state.queues.get(&q.queue_id);
            let finished = st.map(|s| scheduler::queue_should_finish(q, s)).unwrap_or(false);
            // No state yet = never started = due whenever the entry allows it.
            let next_due = match st {
                Some(s) => scheduler::queue_next_due_ms(q, s, now),
                None => Some(now),
            };
            let rounds_completed = st.map(|s| s.rounds_completed).unwrap_or(0);
            json!({
                "queue_id": q.queue_id,
                "name": q.name,
                "members": q.members,
                "total_rounds": q.total_rounds,
                "rounds_completed": rounds_completed,
                "rounds_remaining": if q.total_rounds == 0 {
                    None
                } else {
                    Some(q.total_rounds.saturating_sub(rounds_completed))
                },
                "on_failure": q.on_failure.as_str(),
                "between_ms": q.between_ms,
                "empty_rounds": st.map(|s| s.empty_rounds).unwrap_or(0),
                "finished": finished,
                "current_round": st.and_then(|s| s.current_round.as_ref()).map(|r| json!({
                    "index": r.index,
                    "member_index": r.member_index,
                    "started_ms": r.started_ms,
                })),
                "last_round": st
                    .and_then(|s| last_round_report(&s.member_results))
                    .and_then(|r| serde_json::to_value(r).ok()),
                "member_results": st.map(|s| s.member_results.iter().map(|m| json!({
                    "round": m.round,
                    "member_index": m.member_index,
                    "task": m.task,
                    "class": m.class,
                    "exit": m.exit,
                    "outcome": m.outcome,
                    "clicks": m.clicks,
                    "attempts": m.attempts,
                    "duration_ms": m.duration_ms,
                    "finished_ms": m.finished_ms,
                })).collect::<Vec<_>>()).unwrap_or_default(),
                "next_due_ms": next_due,
                "next_due_local": next_due.map(|at| format_local_datetime(at, offset)),
            })
        })
        .collect();

    Ok(json!({
        "schema_version": SCHEDULE_SCHEMA_VERSION,
        "state_schema_version": state.schema_version,
        "generated_ms": now,
        "generated_local": format_local_datetime(now, offset),
        "config_path": opts.config.display().to_string(),
        "state_path": state_path.display().to_string(),
        "events_path": opts.events_path().display().to_string(),
        "utc_offset_min": offset,
        "mode": sched.mode.as_str(),
        "loop_interval_s": sched.loop_interval_s,
        // Compatibility key: the value is the L3 **pass** cap (new spelling
        // `max_passes`); no duplicate `max_passes` key is emitted.
        "loop_rounds": sched.loop_rounds,
        "calendar": {
            "day_reset": scheduler::format_hhmm(sched.calendar.day_reset_min),
            "week_reset": {
                "weekday": sched.calendar.week_reset_weekday,
                "time": scheduler::format_hhmm(sched.calendar.week_reset_min),
            },
        },
        "cycles": {"day": cycles.day, "week": cycles.week},
        "guards": {
            "blocked": plan.guard.map(|g| g.as_str()),
            "blackout_reason": plan.guard
                .filter(|g| *g == GuardBlock::Blackout)
                .map(|g| guard_reason(&sched, now, g)),
            "active_hours": sched.active_hours.iter().map(|w| json!({
                "days": w.days,
                "start": w.start,
                "end": w.end,
            })).collect::<Vec<_>>(),
            "blackouts": sched.blackouts.iter().map(|b| json!({
                "days": b.window.days,
                "start": b.window.start,
                "end": b.window.end,
                "reason": b.reason,
            })).collect::<Vec<_>>(),
        },
        "passes": state.passes,
        "last_pass_ms": state.last_pass_ms,
        "last_stop": state.last_stop,
        "running": state.running.as_ref().map(|r| json!({
            "entry_id": r.entry_id,
            "task": r.task,
            "pass": r.pass,
            "attempt": r.attempt,
            "since_ms": r.since_ms,
            "since_local": format_local_datetime(r.since_ms as i64, offset),
            "resume": r.resume,
        })),
        "plan": {
            "runnable": plan.runnable.iter().map(|p| json!({
                "entry_id": p.entry_id,
                "task": p.task,
                "lifecycle": p.lifecycle.as_str(),
                "reason": p.reason,
                "resume": p.resume,
            })).collect::<Vec<_>>(),
            "skipped": plan.skipped.iter().map(|s| json!({
                "entry_id": s.entry_id,
                "task": s.task,
                "reason": s.reason,
            })).collect::<Vec<_>>(),
        },
        "next_wake_ms": wake.at_ms,
        "next_wake_local": format_local_datetime(wake.at_ms, offset),
        "next_wake_reasons": wake.reasons,
        "queues": queues,
        "entries": entries,
        "task_choices": task_choices,
    }))
}

/// Print the status JSON (used by `--status`).
pub fn print_status(opts: &SchedulerOptions) -> Result<(), String> {
    let value = status_json(opts)?;
    let text = serde_json::to_string_pretty(&value)
        .map_err(|e| format!("status: serialize: {e}"))?;
    println!("{text}");
    Ok(())
}

// ---------------------------------------------------------------------------
// Main loop
// ---------------------------------------------------------------------------

fn plan_dry_run(
    sched: &Scheduler,
    state: &SchedulerState,
    now: i64,
    out: &mut dyn FnMut(&str),
) -> PassPlan {
    let plan = plan_pass(sched, state, now, state.running.as_ref());
    if let Some(block) = plan.guard {
        out(&format!(
            "scheduler_guard blocked={} reason={}",
            block.as_str(),
            guard_reason(sched, now, block)
        ));
    }
    for p in &plan.runnable {
        out(&format!(
            "scheduler_plan entry={} task={} lifecycle={} reason={} resume={}",
            p.entry_id,
            p.task,
            p.lifecycle.as_str(),
            p.reason,
            p.resume
        ));
    }
    for s in &plan.skipped {
        out(&format!(
            "scheduler_skip entry={} task={} reason={}",
            s.entry_id, s.task, s.reason
        ));
    }
    let wake = next_wake(sched, state, now);
    out(&format!(
        "scheduler_dry_run runnable={} skipped={} next_wake_ms={} reasons={}",
        plan.runnable.len(),
        plan.skipped.len(),
        wake.at_ms,
        wake.reasons.join(",")
    ));
    plan
}

/// Minimum wait before retrying a pass whose device lease is held elsewhere
/// (PR6, design §A.6.4). `sleep_for` may return 1 ms when a runnable entry is
/// due right now; without this floor a held lease would spin and spam both the
/// log ring and the audit file.
const LEASE_BUSY_RETRY_MS: u64 = 500;

/// Acquire the per-pass device lease (PR6 P1-5/P1-6).
///
/// `None` when [`SchedulerOptions::lease_path`] is unset (tests / pure CLI).
/// The returned [`DeviceLease`] is dropped when the pass ends, so the device is
/// free while the scheduler sleeps; an in-flight attempt is protected because
/// `execute_pass` runs entirely inside the guard's lifetime.
fn acquire_pass_lease(opts: &SchedulerOptions) -> Result<Option<DeviceLease>, String> {
    match opts.lease_path.as_ref() {
        Some(path) => DeviceLease::acquire(path, "runner-scheduler", "scheduler").map(Some),
        None => Ok(None),
    }
}

/// Run the scheduler. Returns the session report; `Err` = setup/config error
/// (exit code 2), run failures are recorded in the report instead.
pub fn run_scheduler(
    opts: &SchedulerOptions,
    out: &mut dyn FnMut(&str),
) -> Result<SchedulerReport, String> {
    let started = now_ms();
    if opts.status {
        print_status(opts)?;
        let mut report = SchedulerReport {
            schema_version: SCHEDULE_SCHEMA_VERSION,
            started_ms: started,
            finished_ms: now_ms(),
            stopped_reason: "status".to_string(),
            config_path: opts.config.display().to_string(),
            state_path: opts.state_path().display().to_string(),
            ..SchedulerReport::default()
        };
        report.mode = "status".to_string();
        return Ok(report);
    }

    // Fail fast before anything is written (task existence + meta.exits).
    let loaded = load_full(opts)?;
    let sched = loaded.sched;
    let state_path = opts.state_path();
    let events_path = opts.events_path();
    let (mut state, state_migrated) = load_state_migrating(&state_path)?;
    let now = now_ms() as i64;
    if let Some(note) = &state_migrated {
        out(&format!("scheduler_state_migrated note={note}"));
    }

    let mut report = SchedulerReport {
        schema_version: SCHEDULE_SCHEMA_VERSION,
        started_ms: started,
        dry_run: opts.dry_run,
        mode: sched.mode.as_str().to_string(),
        config_path: opts.config.display().to_string(),
        state_path: state_path.display().to_string(),
        ..SchedulerReport::default()
    };

    // -- dry-run: pure preview, never writes state and never takes the lock ----
    if opts.dry_run {
        out(&format!(
            "scheduler_start config={} tasks_dir={} mode={} dry_run=true",
            opts.config.display(),
            opts.tasks_dir.display(),
            sched.mode.as_str()
        ));
        let mut plan_state = state.clone();
        roll_cycles(&mut plan_state, &sched.calendar, now);
        let plan = plan_dry_run(&sched, &plan_state, now, out);
        report.next_wake_ms = Some(next_wake(&sched, &plan_state, now).at_ms);
        report.entries = build_entry_reports(&sched, &plan_state, now);
        report.queues = build_queue_reports(&sched, &plan_state, now);
        report.stopped_reason = "dry_run".to_string();
        report.finished_ms = now_ms();
        out(&format!(
            "scheduler_summary dry_run=true runnable={} skipped={} passes=0",
            plan.runnable.len(),
            plan.skipped.len()
        ));
        if let Some(path) = &opts.report {
            write_report(path, &report)?;
        }
        return Ok(report);
    }

    // -- single instance lock ------------------------------------------------
    let _lock = SchedulerLock::acquire(&state_path)
        .map_err(|e| format!("scheduler: {e}"))?;

    if state.created_ms == 0 {
        state.created_ms = now as u64;
    }
    if let Some(dir) = &opts.clicks_dir {
        ensure_dir(dir).map_err(|e| format!("--clicks-dir: {e}"))?;
    }

    // Crash recovery: a dirty `running` marker means the previous process died
    // mid-execution. Periodic entries resume from their checkpoint; temp entries
    // have no completion memory and start fresh (state reset before execution).
    let recovery = state.running.clone();
    let mut session = Session {
        opts,
        loaded: Loaded { sched, metas: loaded.metas },
        state,
        state_path: state_path.clone(),
        events_path: events_path.clone(),
        report,
        recovery: recovery.clone(),
        seq: 0,
        run_seq: 0,
        last_run: None,
        last_run_error: None,
    };
    // P1-2 exactly-once: a round that was closed on disk but whose entry-level
    // `record_result` was not saved must be replayed before the first pass (the
    // queue may already be `finished`, so `plan_pass` can never trigger it). The
    // replay happens in memory here; the `save_state` below — always before the
    // first pass — persists the applied result and the cleared flag atomically.
    session.recover_pending_entry_records(out);
    // A marker that no longer maps to an enabled entry can never be finished:
    // drop it loudly instead of leaving the state dirty forever. A finished
    // queue entry counts as "not configured any more" (rule 6: it will never be
    // triggered again, so its recovery marker could never be consumed).
    if let Some(stale) = session.recovery.clone() {
        let still_configured = session
            .sched()
            .entry(&stale.entry_id)
            .map(|e| {
                e.enabled
                    && !(e.kind == EntryKind::Queue
                        && session
                            .sched()
                            .queue(&e.task)
                            .map(|q| scheduler::queue_finished(q, &session.state))
                            .unwrap_or(false))
            })
            .unwrap_or(false);
        if !still_configured {
            out(&format!(
                "scheduler_recover_dropped entry={} task={} reason=entry_missing_or_disabled",
                stale.entry_id, stale.task
            ));
            session.audit(json!({
                "event": "recovery_dropped",
                "entry_id": stale.entry_id,
                "task": stale.task,
                "reason": "entry_missing_or_disabled",
            }));
            session.state.running = None;
            session.recovery = None;
        }
    }
    if let Some(rec) = &session.recovery {
        out(&format!(
            "scheduler_recover entry={} task={} pass={} attempt={} resume={}",
            rec.entry_id, rec.task, rec.pass, rec.attempt, rec.resume
        ));
        session.audit(json!({
            "event": "state_recovered",
            "entry_id": rec.entry_id,
            "task": rec.task,
            "pass": rec.pass,
            "attempt": rec.attempt,
            "since_ms": rec.since_ms,
        }));
    }
    out(&format!(
        "scheduler_start config={} state={} tasks_dir={} mode={} once={} max_passes={}",
        opts.config.display(),
        state_path.display(),
        opts.tasks_dir.display(),
        session.sched().mode.as_str(),
        opts.once,
        opts.max_passes
            .map(|v| v.to_string())
            .unwrap_or_else(|| "-".to_string())
    ));
    session.audit(json!({
        "event": "scheduler_start",
        "config": opts.config.display().to_string(),
        "mode": session.sched().mode.as_str(),
        "once": opts.once,
        "max_passes": opts.max_passes,
        "utc_offset_min": session.sched().calendar.utc_offset_min,
    }));
    if state_migrated.is_some() {
        // Plan §四 risk table: "读 3 自动迁移 + 备份 state". The copy happens
        // before the first v4 write below; a failed copy aborts without
        // touching the v3 file.
        backup_state_v3(&state_path, out)?;
    }
    session.save_state(out)?;

    let cli_max_passes = opts.max_passes;
    let config_max_passes = session.sched().loop_rounds;
    let mut session_passes: u64 = 0;
    // Log/audit a lease conflict once per contiguous busy streak, not on every
    // 500 ms retry (a Studio trial can hold the device for minutes).
    let mut lease_busy_logged = false;
    let stopped_reason: String = 'passes: loop {
        // Cooperative stop (PR6, design §A.6.2): check **before** starting new
        // work, so a stop request that arrives between passes never begins
        // another attempt. An attempt that is already running is never
        // interrupted — this is a single-active-instance scheduler.
        if stop_requested(opts) {
            out("scheduler_stop_requested source=api");
            break "api_stop".to_string();
        }
        let now = now_ms() as i64;
        // PR6 P1-5/P1-6: hold the device lease for exactly one pass. Acquire
        // before planning/executing (the in-flight attempt is protected by the
        // guard's lifetime); Drop at the end of the pass frees the device while
        // the scheduler sleeps. A conflict is a transient condition: skip this
        // pass, keep `state` clean, and retry after the normal sleep floor.
        let pass_lease = match acquire_pass_lease(opts) {
            Ok(lease) => {
                lease_busy_logged = false;
                lease
            }
            Err(e) => {
                if !lease_busy_logged {
                    lease_busy_logged = true;
                    out(&format!(
                        "scheduler_lease_busy owner=runner-scheduler mode=scheduler error={e}"
                    ));
                    session.audit(json!({
                        "event": "scheduler_lease_busy",
                        "owner": "runner-scheduler",
                        "mode": "scheduler",
                        "error": e,
                    }));
                }
                let wake = next_wake(&session.sched(), &session.state, now);
                let sleep_ms = sleep_for(
                    session.sched(),
                    opts,
                    now,
                    wake.at_ms,
                    session.report.passes,
                    0,
                )
                .max(LEASE_BUSY_RETRY_MS);
                out(&format!("scheduler_lease_retry sleep_ms={sleep_ms}"));
                if !interruptible_sleep(opts, sleep_ms) {
                    out("scheduler_stop_requested source=api");
                    break 'passes "api_stop".to_string();
                }
                continue;
            }
        };
        session_passes += 1;
        let pass_no = session.state.passes + 1;
        let calendar = session.sched().calendar;
        let roll = roll_cycles(&mut session.state, &calendar, now);
        if roll.changed {
            session.save_state(out)?;
        }
        let plan = session.plan_now(now);
        let stats = session.execute_pass(pass_no, now, &plan, out)?;
        // The attempt(s) are done: release the device before the state save and
        // the inter-pass sleep, so the Runner/Studio can use it immediately.
        drop(pass_lease);

        session.state.last_pass_ms = now as u64;
        session.state.passes = session.state.passes.saturating_add(1);
        session.report.passes += 1;
        session.save_state(out)?;

        // -- stopping rules --------------------------------------------------
        // Historical reason strings `max_rounds`/`loop_rounds` are kept (they
        // are persisted in `state.last_stop` and reported); both mean "L3 pass
        // cap reached", not a queue Round.
        let mut reason: Option<&'static str> = None;
        if stats.fatal_stop {
            reason = Some("fatal");
        } else if let Some(max) = cli_max_passes {
            if session_passes >= max {
                reason = Some("max_rounds");
            }
        }
        if reason.is_none() && config_max_passes > 0 && session_passes >= config_max_passes {
            reason = Some("loop_rounds");
        }
        if reason.is_none() && opts.once {
            reason = Some("once");
        }
        if reason.is_none()
            && session.sched().mode == RunMode::Stop
            && stats.ran > 0
        {
            reason = Some("stop");
        }
        if reason.is_none() && !session.sched().entries.iter().any(|e| e.enabled) {
            reason = Some("no_enabled_entries");
        }
        // (P1's `queue_executor_pending` stop-mode convergence rule is gone:
        // P2 executes queue rounds, so a queue-only stop-mode config stops with
        // `reason=stop` after the round instead of pretending nothing can run.)
        if let Some(r) = reason {
            break r.to_string();
        }
        // A stop request that arrived while the pass was executing is honoured
        // here: the pass (including the running attempt) already finished.
        if stop_requested(opts) {
            out("scheduler_stop_requested source=api");
            break "api_stop".to_string();
        }

        // -- wait for the next opportunity -----------------------------------
        let wake = next_wake(&session.sched(), &session.state, now);
        let sleep_ms = sleep_for(
            session.sched(),
            opts,
            now,
            wake.at_ms,
            session.report.passes,
            stats.ran,
        );
        out(&format!(
            "scheduler_wake pass={pass_no} next_wake_ms={} next_wake_local={} reasons={} sleep_ms={}",
            wake.at_ms,
            format_local_datetime(wake.at_ms, session.sched().calendar.utc_offset_min),
            wake.reasons.join(","),
            sleep_ms
        ));
        session.audit(json!({
            "event": "wake",
            "pass": pass_no,
            "next_wake_ms": wake.at_ms,
            "reasons": wake.reasons,
            "sleep_ms": sleep_ms,
        }));
        if sleep_ms > 0 && !interruptible_sleep(opts, sleep_ms) {
            out("scheduler_stop_requested source=api");
            break 'passes "api_stop".to_string();
        }
    };

    session.state.last_stop = stopped_reason.clone();
    session.save_state(out)?;
    session.audit(json!({"event": "stop", "reason": stopped_reason, "passes": session.report.passes}));

    let now = now_ms() as i64;
    session.report.stopped_reason = stopped_reason.clone();
    session.report.finished_ms = now_ms();
    session.report.entries = build_entry_reports(&session.sched(), &session.state, now);
    session.report.queues = build_queue_reports(&session.sched(), &session.state, now);
    session.report.next_wake_ms = Some(next_wake(&session.sched(), &session.state, now).at_ms);
    out(&format!(
        "scheduler_stop reason={stopped_reason} passes={} runs={} successes={} resources={} blocked={} failures={} fatals={} run_file_errors={}",
        session.report.passes,
        session.report.runs,
        session.report.successes,
        session.report.resources,
        session.report.blocked,
        session.report.failures,
        session.report.fatals,
        session.report.run_file_errors
    ));
    out(&format!(
        "scheduler_summary runs={} successes={} resources={} blocked={} failures={} fatals={} run_file_errors={} dry_run=false",
        session.report.runs,
        session.report.successes,
        session.report.resources,
        session.report.blocked,
        session.report.failures,
        session.report.fatals,
        session.report.run_file_errors
    ));
    if let Some(path) = &opts.report {
        write_report(path, &session.report)?;
    }
    Ok(session.report)
}

/// Granularity of the inter-pass sleep while a stop token is configured
/// (design §A.6.2: `POST /runner/scheduler/stop` must exit in ≤ 5 s, so the
/// blocking wait is split into slices that re-check the token).
const STOP_POLL_INTERVAL_MS: u64 = 200;

/// Has a cooperative stop been requested (Runner `POST /runner/scheduler/stop`)?
fn stop_requested(opts: &SchedulerOptions) -> bool {
    opts.stop
        .as_ref()
        .map(|token| token.load(Ordering::SeqCst))
        .unwrap_or(false)
}

/// Sleep `total_ms` in ≤ [`STOP_POLL_INTERVAL_MS`] slices.
///
/// Returns `false` when the stop token was set (the caller then stops with
/// `stopped_reason = "api_stop"`). An in-flight attempt is never interrupted:
/// this only ever runs *between* passes.
fn interruptible_sleep(opts: &SchedulerOptions, total_ms: u64) -> bool {
    // No stop token => keep the historical single sleep (no extra wakeups).
    if opts.stop.is_none() {
        std::thread::sleep(Duration::from_millis(total_ms));
        return true;
    }
    let mut remaining = total_ms;
    while remaining > 0 {
        if stop_requested(opts) {
            return false;
        }
        let chunk = remaining.min(STOP_POLL_INTERVAL_MS);
        std::thread::sleep(Duration::from_millis(chunk));
        remaining -= chunk;
    }
    !stop_requested(opts)
}

/// How long to sleep between passes.
///
/// * loop mode, something ran: the configured loop interval (0 = no sleep).
/// * loop mode, nothing ran: sleep up to the next useful instant (interval caps
///   it so temp/periodic changes are noticed).
/// * stop mode: wait for the next `next_wake` (capped by `--tick-ms`). We only
///   get here when a pass had nothing runnable — a pass that ran something stops.
fn sleep_for(
    sched: &Scheduler,
    opts: &SchedulerOptions,
    now: i64,
    wake_at: i64,
    _passes: u64,
    ran: u64,
) -> u64 {
    let delta = (wake_at - now).max(1) as u64;
    match sched.mode {
        RunMode::Loop => {
            let interval = sched.loop_interval_s.saturating_mul(1000);
            if interval == 0 {
                // 0 = "next pass immediately" (tests/long-loop). Still avoid a
                // busy spin when there is nothing to run at all.
                if ran > 0 {
                    0
                } else {
                    delta.min(1_000)
                }
            } else if ran > 0 {
                interval
            } else {
                interval.min(delta)
            }
        }
        RunMode::Stop => delta.min(opts.max_sleep_ms.max(1)),
    }
}

// ---------------------------------------------------------------------------
// reset-memory
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct ResetOptions {
    pub config: PathBuf,
    pub state: Option<PathBuf>,
    pub tasks_dir: PathBuf,
    pub tasks: Vec<String>,
    pub all: bool,
    pub include_checkpoint: bool,
    pub keep_checkpoint: bool,
    pub events: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ResetReport {
    pub tasks: Vec<String>,
    pub entries: Vec<String>,
    /// Queue ids whose round state and queue-entry state were cleared (schema
    /// v4, plan §二 rule 6/8). Always serialized (CLI + serve report contract).
    pub queues: Vec<String>,
    pub files_removed: u64,
    pub state_path: String,
    pub events_path: String,
}

impl ResetOptions {
    pub fn parse(rest: &[String]) -> Result<Self, String> {
        let mut o = ResetOptions {
            config: PathBuf::from(DEFAULT_CONFIG),
            tasks_dir: PathBuf::from(DEFAULT_TASKS_DIR),
            include_checkpoint: true,
            ..ResetOptions::default()
        };
        let mut i = 0usize;
        while i < rest.len() {
            match rest[i].as_str() {
                "--tasks" => {
                    let raw = take_value(rest, &mut i, "--tasks")?;
                    o.tasks = raw
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                }
                "--config" | "--plan" | "--scheduler-config" => {
                    o.config = PathBuf::from(take_value(rest, &mut i, "--config")?)
                }
                "--state" => {
                    o.state = Some(PathBuf::from(take_value(rest, &mut i, "--state")?))
                }
                "--tasks-dir" => {
                    o.tasks_dir = PathBuf::from(take_value(rest, &mut i, "--tasks-dir")?)
                }
                "--events" => {
                    o.events = Some(PathBuf::from(take_value(rest, &mut i, "--events")?))
                }
                "--all" => {
                    o.all = true;
                    i += 1;
                }
                "--include-checkpoint" => {
                    o.include_checkpoint = true;
                    i += 1;
                }
                "--keep-checkpoint" => {
                    o.keep_checkpoint = true;
                    i += 1;
                }
                other => return Err(format!("unknown reset-memory option `{other}`\n{USAGE}")),
            }
        }
        if o.tasks.is_empty() && !o.all {
            return Err(format!(
                "reset-memory: pass --tasks <a,b> and/or --all\n{USAGE}"
            ));
        }
        Ok(o)
    }

    fn state_path(&self) -> PathBuf {
        self.state
            .clone()
            .unwrap_or_else(|| sibling_with_suffix(&self.config, "state.json"))
    }
}

/// Clear scheduler memory for the selected tasks (CLI + panel callable).
///
/// Requires the scheduler to be stopped (lock free). Returns a summary; the
/// caller prints it. `Err` = refusal/IO error (nothing is deleted on refusal).
pub fn reset_memory(
    opts: &ResetOptions,
    out: &mut dyn FnMut(&str),
) -> Result<ResetReport, String> {
    let state_path = opts.state_path();
    let events_path = opts
        .events
        .clone()
        .unwrap_or_else(|| sibling_with_suffix(&opts.config, "events.jsonl"));

    // Refuse while a live scheduler holds the lock.
    if let Some(pid) = lock_held_by_live_process(&state_path) {
        return Err(format!(
            "reset-memory: scheduler is running (pid {pid}); stop it before resetting memory"
        ));
    }
    // Reclaim a stale lock file so the next scheduler start is not blocked.
    let lock_path = SchedulerLock::lock_path(&state_path);
    if lock_path.is_file() {
        let _ = std::fs::remove_file(&lock_path);
        out(&format!(
            "reset_memory_stale_lock_removed={}",
            lock_path.display()
        ));
    }

    // Optional config: used to map entry ids -> tasks and for --all.
    let sched: Option<Scheduler> = if opts.config.is_file() {
        Some(load_scheduler(&SchedulerOptions {
            config: opts.config.clone(),
            state: opts.state.clone(),
            tasks_dir: opts.tasks_dir.clone(),
            ..SchedulerOptions::default()
        })?)
    } else {
        None
    };

    let mut targets: BTreeSet<String> = BTreeSet::new();
    for task in &opts.tasks {
        oas_task::validate_task_name(task)
            .map_err(|e| format!("reset-memory: --tasks: {e}"))?;
        targets.insert(task.clone());
    }
    if opts.all {
        if let Some(sched) = &sched {
            // Task entries AND queue members are tasks (a queue entry's `task`
            // is a queue_id, never a bundle).
            targets.extend(sched.task_names());
        }
        if targets.is_empty() {
            // No config: fall back to every task bundle under --tasks-dir.
            if let Ok(list) = oas_task::list_tasks(&opts.tasks_dir) {
                targets.extend(list);
            }
        }
    }
    if targets.is_empty() {
        return Err("reset-memory: nothing selected (no matching tasks)".to_string());
    }

    let (mut state, state_migrated) = load_state_migrating(&state_path)?;
    let mut report = ResetReport {
        tasks: targets.iter().cloned().collect(),
        state_path: state_path.display().to_string(),
        events_path: events_path.display().to_string(),
        ..ResetReport::default()
    };

    // Queues touched by the selection + the ids of the queue entries that
    // schedule them. A queue entry's `EntryState.task` is the queue_id (not a
    // bundle name), so it can never be matched through `targets`.
    let mut reset_queues: Vec<String> = Vec::new();
    let mut queue_entry_ids: BTreeSet<String> = BTreeSet::new();
    if let Some(sched) = &sched {
        for queue in &sched.queues {
            let hit = opts.all || queue.members.iter().any(|m| targets.contains(m));
            if !hit {
                continue;
            }
            reset_queues.push(queue.queue_id.clone());
            for entry in &sched.entries {
                if entry.kind == EntryKind::Queue && entry.task == queue.queue_id {
                    queue_entry_ids.insert(entry.entry_id.clone());
                }
            }
        }
    }

    // ① Scheduler entry state (attempts/cooldown/quota AND a dirty running
    // marker: an offline crash residue is exactly "memory"). Queue entries are
    // matched by entry_id (their task is a queue_id, not a bundle name).
    let mut clear_ids: Vec<String> = Vec::new();
    for (entry_id, st) in &state.entries {
        if queue_entry_ids.contains(entry_id) {
            clear_ids.push(entry_id.clone());
            continue;
        }
        let task = if st.task.is_empty() {
            sched
                .as_ref()
                .and_then(|s| s.entry(entry_id))
                .map(|e| e.task.clone())
                .unwrap_or_default()
        } else {
            st.task.clone()
        };
        if targets.contains(&task) {
            clear_ids.push(entry_id.clone());
        }
    }
    clear_ids.sort();
    clear_ids.dedup();
    let mut state_changed = !clear_ids.is_empty();
    for id in &clear_ids {
        state.entries.remove(id);
    }
    if let Some(running) = state.running.clone() {
        let running_task = if running.task.is_empty() {
            sched
                .as_ref()
                .and_then(|s| s.entry(&running.entry_id))
                .map(|e| e.task.clone())
                .unwrap_or_default()
        } else {
            running.task.clone()
        };
        // The running marker points either at the queue entry itself (P2) or
        // at one of its member tasks; both are memory of the reset queue.
        let points_at_queue_entry = queue_entry_ids.contains(&running.entry_id);
        let points_at_queue = reset_queues
            .iter()
            .any(|q| q == &running.task || q == &running_task);
        if points_at_queue_entry || points_at_queue || targets.contains(&running_task) {
            state.running = None;
            state_changed = true;
            out(&format!(
                "reset_memory_cleared_dirty_running={}",
                running.entry_id
            ));
        }
    }
    // ② Queue round state is memory too (plan §二 rules 6/8): drop the whole
    // `state.queues[qid]` entry (counters / finished / open round /
    // member_results) for every selected queue. A missing entry is a no-op.
    for queue_id in &reset_queues {
        if state.queues.remove(queue_id).is_some() {
            state_changed = true;
        }
    }
    report.queues = reset_queues.clone();
    report.entries = clear_ids;

    if state_changed {
        if state_migrated.is_some() {
            // Keep the pre-migration v3 file before the first v4 write here too
            // (the daemon path does the same in `run_scheduler`).
            backup_state_v3(&state_path, out)?;
        }
        write_json_atomic(&state_path, &state)
            .map_err(|e| format!("reset-memory: state: {e}"))?;
    }

    // ③ Task state/ (checkpoint + learned coords + counters) — memory, not stats.
    let mut files_removed: u64 = 0;
    for task in &targets {
        let dir = opts.tasks_dir.join(task).join("state");
        if !dir.exists() {
            out(&format!("reset_memory_task={task} state_dir=absent"));
            continue;
        }
        if opts.keep_checkpoint && dir.is_dir() {
            let mut removed_names = Vec::new();
            let entries = std::fs::read_dir(&dir)
                .map_err(|e| format!("reset-memory: read `{}`: {e}", dir.display()))?;
            for entry in entries {
                let entry =
                    entry.map_err(|e| format!("reset-memory: read `{}`: {e}", dir.display()))?;
                if entry.file_name().to_string_lossy() == "checkpoint.json" {
                    continue;
                }
                let path = entry.path();
                if path.is_dir() {
                    std::fs::remove_dir_all(&path).map_err(|e| {
                        format!("reset-memory: remove `{}`: {e}", path.display())
                    })?;
                } else {
                    std::fs::remove_file(&path).map_err(|e| {
                        format!("reset-memory: remove `{}`: {e}", path.display())
                    })?;
                }
                files_removed += 1;
                removed_names.push(entry.file_name().to_string_lossy().to_string());
            }
            out(&format!(
                "reset_memory_task={task} kept=checkpoint.json removed={}",
                removed_names.len()
            ));
        } else {
            // Both checkpoint and counters are cleared by default (§22.34.5).
            let removed = clear_task_state_dir(&opts.tasks_dir, task)
                .map_err(|e| format!("reset-memory: {e}"))?;
            files_removed += removed.len() as u64;
            out(&format!(
                "reset_memory_task={task} removed={} files={}",
                removed.len(),
                removed.join(",")
            ));
        }
    }
    report.files_removed = files_removed;

    // ④ Audit event (fatal on failure: a reset without an audit trail is not
    // acceptable — §22.34.5 "审计 memory_reset"). `queues` stays for backward
    // compatibility; `queues_reset` names the queue-state cleanup explicitly.
    append_jsonl(
        &events_path,
        &json!({
            "ts_ms": now_ms(),
            "event": "memory_reset",
            "tasks": report.tasks,
            "entry_ids": report.entries,
            "queues": report.queues,
            "queues_reset": report.queues,
            "files_removed": files_removed,
            "checkpoint_kept": opts.keep_checkpoint,
            "state_path": report.state_path,
        }),
    )
    .map_err(|e| format!("reset-memory: audit: {e}"))?;

    out(&format!("reset_memory_tasks={}", report.tasks.join(",")));
    out(&format!("reset_memory_entries={}", report.entries.len()));
    out(&format!("reset_memory_queues={}", report.queues.join(",")));
    out(&format!("reset_memory_files_removed={files_removed}"));
    out(&format!("reset_memory_state={}", report.state_path));
    out(&format!("reset_memory_audit={}", report.events_path));
    Ok(report)
}

pub fn reset_memory_cmd(rest: &[String]) -> ExitCode {
    if rest.iter().any(|a| a == "-h" || a == "--help") {
        println!("{USAGE}");
        return ExitCode::from(2);
    }
    let opts = match ResetOptions::parse(rest) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("scheduler reset-memory: {e}");
            return ExitCode::from(2);
        }
    };
    let mut out = |line: &str| println!("{line}");
    match reset_memory(&opts, &mut out) {
        Ok(_) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("scheduler reset-memory: {e}");
            ExitCode::from(2)
        }
    }
}

// ---------------------------------------------------------------------------
// CLI entry point (`oasd schedule` / `oasd scheduler`)
// ---------------------------------------------------------------------------

pub fn schedule_cmd(rest: &[String]) -> ExitCode {
    if rest.first().map(|s| s.as_str()) == Some("reset-memory") {
        return reset_memory_cmd(&rest[1..]);
    }
    if rest.iter().any(|a| a == "-h" || a == "--help") {
        println!("{USAGE}");
        return ExitCode::from(2);
    }
    let mut opts = match SchedulerOptions::parse(rest) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("scheduler: {e}");
            return ExitCode::from(2);
        }
    };
    // PR6 P1-6: the legacy CLI scheduler protects the device too. The same
    // `config/device.lease` path Studio/Runner use is derived from the live
    // `--config` (already loaded globally by `main.rs`); without one the lease
    // is disabled, exactly like `oasd run`.
    opts.lease_path = crate::device_lease::lease_path_from_config();
    let mut out = |line: &str| println!("{line}");
    match run_scheduler(&opts, &mut out) {
        Ok(report) => {
            if opts.status || opts.dry_run {
                // status prints JSON itself; dry-run is a preview, never "failed".
                return ExitCode::SUCCESS;
            }
            if report.all_ok() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        Err(e) => {
            eprintln!("scheduler: {e}");
            ExitCode::from(2)
        }
    }
}

/// Alias for `oasd scheduler ...` (main.rs dispatches both spellings here).
pub fn scheduler_cmd(rest: &[String]) -> ExitCode {
    schedule_cmd(rest)
}

/// Where the Runner values layer comes from: the test seam when set, otherwise
/// the live `config/values.json` next to `--config` (design §C.3 layer 3).
fn values_path_of(opts: &SchedulerOptions) -> PathBuf {
    opts.values_path
        .clone()
        .unwrap_or_else(crate::runner_config::runner_values_path)
}

/// Explicit `RunOptions.params` layers for one attempt (design §C.3):
/// **entry params (layer 2) over Runner values (layer 3)**.
///
/// The bundle's `meta.params[].default` (layer 4) is merged under both inside
/// `run::execute`; a CLI layer does not exist on the scheduler path. Keeping the
/// merge in one function is what makes the priority matrix directly unit-testable.
fn attempt_run_params(
    opts: &SchedulerOptions,
    task: &str,
    entry: &Entry,
) -> Result<run::ParamMap, String> {
    let mut params = run::runner_values_layer(&values_path_of(opts), task)?;
    params.extend(entry_run_params(entry));
    Ok(params)
}

/// Scheduler entry `params` (design §C.3 layer 2) -> `RunOptions.params`.
///
/// Directly clones the JSON values: the pre-PR3 version stringified every scalar
/// (`"007"` -> `"007"` text, `3` -> `"3"`), so `run.rs` had to guess the type
/// again and `"007"` became `Num(7.0)`. Entry params keep their JSON type now;
/// only the CLI `--param` text is parsed. `run.rs` merges Runner values below
/// and `meta.params[].default` last.
fn entry_run_params(entry: &Entry) -> run::ParamMap {
    entry.params.clone()
}

// ---------------------------------------------------------------------------
// Integration tests: synthetic task bundles + replay (no emulator/device)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use oas_obs::Recorder;
    use oas_task::TaskBundle;
    use oas_types::Frame;

    fn tmpdir(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "oas-scheduler-{tag}-{}-{}",
            std::process::id(),
            now_ms()
        ));
        std::fs::create_dir_all(&p).expect("tmpdir");
        p
    }

    fn frame(v: u8) -> Frame {
        let (w, h) = (64u32, 48u32);
        let mut rgba = vec![v; (w * h * 4) as usize];
        for i in (3..rgba.len()).step_by(4) {
            rgba[i] = 255;
        }
        Frame::new(w, h, rgba)
    }

    /// Create `tasks/<name>/{meta.json,flow.json}` (default meta first, then the
    /// optional override so tests can declare `exits`).
    fn write_task(td: &Path, name: &str, flow: &str, meta_override: Option<&str>) {
        let dir = td.join(name);
        let bundle = TaskBundle::open(&dir).expect("open task");
        bundle.save_flow(flow).expect("save flow");
        if let Some(meta) = meta_override {
            std::fs::write(dir.join("meta.json"), meta).expect("write meta");
        }
    }

    fn write_replay(dir: &Path) {
        let mut rec = Recorder::create(dir, "scheduler-test").expect("recorder");
        rec.capture_frame(&frame(10)).expect("f0");
        rec.capture_frame(&frame(20)).expect("f1");
        rec.finish().expect("finish");
    }

    const OK_FLOW: &str = r#"{"schema_version":1,"name":"ok","steps":[
         {"op":"click_point","args":{"x":10,"y":10}},
         {"op":"finish","args":{"exit":"done"}}]}"#;

    const RESUME_FLOW: &str = r#"{"schema_version":1,"name":"resume-flow","steps":[
         {"op":"click_point","args":{"x":10,"y":10}},
         {"op":"finish","args":{"exit":"done"}}]}"#;

    fn base_opts(td: &Path, config: PathBuf, replay: Option<PathBuf>) -> SchedulerOptions {
        SchedulerOptions {
            config,
            tasks_dir: td.to_path_buf(),
            replay,
            once: true,
            tz_offset_min: Some(0),
            library: td.join("library"),
            clicks_dir: Some(td.join("clicks")),
            runs_dir: td.join("runs"),
            // Hermetic: never read the operator's real `config/values.json`.
            // The path does not exist in the tempdir => empty values layer;
            // tests that exercise values override this seam explicitly.
            values_path: Some(td.join("config").join("values.json")),
            ..SchedulerOptions::default()
        }
    }

    // -- P2 queue-round test helpers (deterministic executor seam) ----------

    /// One scripted attempt result (see `SchedulerOptions::test_exec`).
    fn step(class: ResultClass) -> TestExecResult {
        match class {
            ResultClass::Success => TestExecResult::new(class, "done", "Done").with_clicks(2),
            ResultClass::Resource => TestExecResult::new(class, "no_resource", "Failed"),
            ResultClass::Blocked => TestExecResult::new(class, "window_closed", "Blocked"),
            ResultClass::Retryable => TestExecResult::new(class, "failed", "Failed"),
            ResultClass::Fatal => TestExecResult::new(class, "exhausted", "Fatal"),
        }
    }

    fn queue_opts(td: &Path, config: PathBuf, exec: TestExec) -> SchedulerOptions {
        let mut o = base_opts(td, config, None);
        o.test_exec = Some(exec);
        o
    }

    /// A v4 queue config with one `kind=queue` entry; `members`/`total_rounds`/
    /// `on_failure`/`between_ms`/`retry`/`quota` are all explicit so every test
    /// is independent of the defaults.
    fn queue_config_text(
        members: &[&str],
        total_rounds: u64,
        on_failure: &str,
        retry: u32,
        entry_extra: &str,
        config_extra: &str,
    ) -> String {
        let members: Vec<String> = members.iter().map(|m| format!("\"{m}\"")).collect();
        format!(
            r#"{{"schema_version":4,
                "config":{{"mode":"loop","loop_interval_s":0{config_extra}}},
                "queues":[{{"queue_id":"q1","members":[{}],"total_rounds":{total_rounds},
                           "on_failure":"{on_failure}","between_ms":0}}],
                "entries":[{{"entry_id":"qe","task":"q1","kind":"queue","retry":{retry}{entry_extra}}}]}}"#,
            members.join(",")
        )
    }

    fn exec_two(first: ResultClass, second: ResultClass) -> TestExec {
        TestExec::new()
            .with("成员1", vec![step(first)])
            .with("成员2", vec![step(second)])
    }

    fn write_config(td: &Path, text: &str) -> PathBuf {
        let path = td.join("scheduler.json");
        std::fs::write(&path, text).expect("write config");
        path
    }

    fn state_of(opts: &SchedulerOptions) -> SchedulerState {
        load_state(&opts.state_path()).expect("load state")
    }

    // -- periodic: quota + state survives a restart -------------------------

    #[test]
    fn periodic_once_runs_and_quota_blocks_the_second_scan() {
        let td = tmpdir("periodic");
        let replay = td.join("replay");
        write_task(&td, "任务1", OK_FLOW, None);
        write_replay(&replay);
        let config = write_config(
            &td,
            &format!(
                r#"{{"schema_version":3,
                    "config":{{"calendar":{{"day_reset":"05:00"}}}},
                    "entries":[{{"entry_id":"e1","task":"任务1","lifecycle":"periodic",
                                 "quota":{{"per":"day","times":1}}}}]}}"#
            ),
        );
        let opts = base_opts(&td, config, Some(replay));
        let mut lines = Vec::new();
        let r1 = run_scheduler(&opts, &mut |l| lines.push(l.to_string())).expect("run 1");
        assert_eq!(r1.runs, 1, "{r1:?}");
        assert_eq!(r1.successes, 1, "{r1:?}");
        assert_eq!(r1.stopped_reason, "once");
        assert!(lines.iter().any(|l| l.contains("scheduler_run_done")));
        assert!(
            lines
                .iter()
                .any(|l| l.contains("scheduler_plan entry=e1") && l.contains("reason=periodic")),
            "planned: {lines:?}"
        );
        let state_path = opts.state_path();
        assert!(state_path.is_file(), "state must be persisted");
        let state = state_of(&opts);
        assert_eq!(state.passes, 1);
        assert_eq!(state.entries["e1"].runs, 1);
        assert_eq!(state.entries["e1"].successes, 1);
        assert_eq!(state.entries["e1"].quota_day.used, 1);
        assert!(state.running.is_none());
        // Click audit evidence exists.
        let clicks: Vec<_> = std::fs::read_dir(td.join("clicks"))
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".clicks.jsonl"))
            .collect();
        assert_eq!(clicks.len(), 1, "one click audit file per run");

        // Second --once scan: the quota is full, nothing may run.
        let mut lines2 = Vec::new();
        let r2 = run_scheduler(&opts, &mut |l| lines2.push(l.to_string())).expect("run 2");
        assert_eq!(r2.runs, 0, "quota must survive a restart: {r2:?}");
        assert_eq!(state_of(&opts).passes, 2);
        assert!(
            lines2
                .iter()
                .any(|l| l.contains("scheduler_skip entry=e1") && l.contains("quota_exhausted")),
            "{lines2:?}"
        );
        std::fs::remove_dir_all(&td).ok();
    }

    // -- temp: every pass + state reset before/after ------------------------

    #[test]
    fn temp_runs_every_pass_and_resets_its_state_before_and_after() {
        let td = tmpdir("temp");
        let replay = td.join("replay");
        write_task(&td, "临时", OK_FLOW, None);
        write_replay(&replay);
        // Stale task memory that a temp task must NOT carry across passes.
        let state_dir = td.join("临时").join("state");
        std::fs::create_dir_all(&state_dir).unwrap();
        std::fs::write(state_dir.join("checkpoint.json"), "{}").unwrap();
        std::fs::write(state_dir.join("coords.json"), "{}").unwrap();
        let config = write_config(
            &td,
            r#"{"schema_version":3,
                "config":{"mode":"loop","loop_interval_s":0,"calendar":{"day_reset":"05:00"}},
                "entries":[{"entry_id":"t1","task":"临时","lifecycle":"temp"}]}"#,
        );
        let mut opts = base_opts(&td, config, Some(replay));
        opts.once = false;
        opts.max_passes = Some(2);
        let mut lines = Vec::new();
        let report = run_scheduler(&opts, &mut |l| lines.push(l.to_string())).expect("run");
        assert_eq!(report.passes, 2, "two loop passes: {report:?}");
        assert_eq!(report.runs, 2, "temp runs once per pass: {report:?}");
        assert_eq!(report.stopped_reason, "max_rounds");
        assert!(
            lines
                .iter()
                .any(|l| l.contains("scheduler_temp_reset_before") && l.contains("removed=2")),
            "the stale state dir must be reset before the run: {lines:?}"
        );
        assert!(lines.iter().any(|l| l.contains("scheduler_temp_reset_after")));
        // After the run the task state dir is empty (no completion memory).
        let left: Vec<_> = std::fs::read_dir(&state_dir).unwrap().collect();
        assert!(left.is_empty(), "temp memory must be reset after the run");
        let state = state_of(&opts);
        assert_eq!(state.entries["t1"].runs, 2, "no cross-pass completion memory");
        assert_eq!(state.entries["t1"].quota_day.used, 0, "temp has no quota");
        std::fs::remove_dir_all(&td).ok();
    }

    #[test]
    fn temp_runs_again_on_a_restarted_scheduler() {
        let td = tmpdir("temp-restart");
        let replay = td.join("replay");
        write_task(&td, "临时", OK_FLOW, None);
        write_replay(&replay);
        let config = write_config(
            &td,
            r#"{"schema_version":3,"entries":[{"entry_id":"t1","task":"临时","lifecycle":"temp"}]}"#,
        );
        let opts = base_opts(&td, config, Some(replay));
        let r1 = run_scheduler(&opts, &mut |_| {}).expect("run 1");
        assert_eq!(r1.runs, 1);
        // A second process (restart) has no "already ran" memory for temp.
        let r2 = run_scheduler(&opts, &mut |_| {}).expect("run 2");
        assert_eq!(r2.runs, 1, "temp must run again on the next start: {r2:?}");
        assert_eq!(state_of(&opts).entries["t1"].runs, 2);
        std::fs::remove_dir_all(&td).ok();
    }

    // -- dry-run / status ---------------------------------------------------

    #[test]
    fn dry_run_plans_without_writing_state_or_lock() {
        let td = tmpdir("dry");
        let replay = td.join("replay");
        write_task(&td, "任务1", OK_FLOW, None);
        write_replay(&replay);
        let config = write_config(
            &td,
            r#"{"schema_version":3,"entries":[{"entry_id":"e1","task":"任务1","lifecycle":"periodic"}]}"#,
        );
        let mut opts = base_opts(&td, config, Some(replay));
        opts.dry_run = true;
        let mut lines = Vec::new();
        let report = run_scheduler(&opts, &mut |l| lines.push(l.to_string())).expect("dry run");
        assert!(report.dry_run);
        assert_eq!(report.runs, 0);
        assert_eq!(report.stopped_reason, "dry_run");
        assert!(lines.iter().any(|l| l.contains("scheduler_dry_run")));
        assert!(
            !opts.state_path().exists(),
            "dry-run must not write the state file"
        );
        assert!(
            !SchedulerLock::lock_path(&opts.state_path()).exists(),
            "dry-run must not take the lock"
        );
        assert!(
            !td.join("clicks").exists(),
            "dry-run must not create output dirs"
        );
        std::fs::remove_dir_all(&td).ok();
    }

    #[test]
    fn config_max_passes_caps_the_session_passes() {
        let td = tmpdir("max-passes");
        let replay = td.join("replay");
        write_task(&td, "临时", OK_FLOW, None);
        write_replay(&replay);
        // New spelling in the document: accepted and effective (L3 pass cap).
        let config = write_config(
            &td,
            r#"{"schema_version":4,
                "config":{"mode":"loop","loop_interval_s":0,"max_passes":2},
                "entries":[{"entry_id":"t1","task":"临时","lifecycle":"temp"}]}"#,
        );
        let mut opts = base_opts(&td, config, Some(replay));
        opts.once = false;
        let report = run_scheduler(&opts, &mut |_| {}).expect("run");
        assert_eq!(report.passes, 2, "max_passes is the session pass cap: {report:?}");
        assert_eq!(report.runs, 2, "the temp entry runs each capped pass");
        assert_eq!(
            report.stopped_reason, "loop_rounds",
            "historical stop-reason string for the pass cap"
        );
        std::fs::remove_dir_all(&td).ok();
    }

    #[test]
    fn status_json_exposes_the_panel_contract() {
        let td = tmpdir("status");
        let replay = td.join("replay");
        write_task(&td, "任务1", OK_FLOW, None);
        write_replay(&replay);
        let config = write_config(
            &td,
            r#"{"schema_version":3,
                "config":{"calendar":{"day_reset":"05:00","week_reset":{"weekday":0,"time":"05:00"}}},
                "entries":[{"entry_id":"e1","task":"任务1","lifecycle":"periodic",
                             "quota":{"per":"day","times":2},
                             "windows":[{"days":[],"start":"00:00","end":"00:00"}]},
                           {"entry_id":"t1","task":"任务1","lifecycle":"temp"}]}"#,
        );
        let opts = base_opts(&td, config, Some(replay));
        let r = run_scheduler(&opts, &mut |_| {}).expect("run");
        assert_eq!(r.runs, 2, "periodic + temp both ran");

        let v = status_json(&opts).expect("status");
        assert_eq!(v["schema_version"], 4);
        assert_eq!(v["state_schema_version"], 4);
        assert_eq!(v["entries"].as_array().unwrap().len(), 2);
        assert!(v["cycles"]["day"].is_string());
        assert!(v["next_wake_ms"].is_i64());
        assert!(v["calendar"]["day_reset"].is_string());
        assert!(v["task_choices"].as_array().unwrap().len() == 1);
        assert_eq!(v["task_choices"][0]["task"], "任务1");
        assert_eq!(v["task_choices"][0]["entry_ids"].as_array().unwrap().len(), 2);
        let e1 = &v["entries"][0];
        assert_eq!(e1["entry_id"], "e1");
        assert_eq!(e1["lifecycle"], "periodic");
        assert_eq!(e1["quota"]["used"], 1);
        assert_eq!(e1["quota"]["remaining"], 1);
        assert_eq!(v["entries"][1]["lifecycle"], "temp");
        assert!(v["plan"]["runnable"].is_array());
        assert!(v["guards"]["blocked"].is_null());
        std::fs::remove_dir_all(&td).ok();
    }

    // -- crash recovery -----------------------------------------------------

    #[test]
    fn dirty_running_state_resumes_a_periodic_entry_from_its_checkpoint() {
        let td = tmpdir("recover");
        let replay = td.join("replay");
        write_task(&td, "恢复", RESUME_FLOW, None);
        write_replay(&replay);
        let config = write_config(
            &td,
            r#"{"schema_version":3,
                "config":{"calendar":{"day_reset":"00:00"},
                          "guards":{"active_hours":[]}},
                "entries":[{"entry_id":"e1","task":"恢复","lifecycle":"periodic",
                             "windows":[{"days":[],"start":"00:00","end":"00:00"}]}]}"#,
        );
        let opts = base_opts(&td, config, Some(replay));
        let state_path = opts.state_path();
        // Simulate a crash mid-run: dirty running marker + a checkpoint that
        // says the first of two steps already completed.
        let state = SchedulerState {
            schema_version: SCHEDULE_SCHEMA_VERSION,
            created_ms: 1,
            running: Some(scheduler::RunningEntry {
                entry_id: "e1".to_string(),
                task: "恢复".to_string(),
                pass: 1,
                attempt: 1,
                since_ms: 1,
                resume: true,
            }),
            ..SchedulerState::new()
        };
        write_json_atomic(&state_path, &state).unwrap();
        let ckpt = task_checkpoint_path(&td, "恢复");
        std::fs::create_dir_all(ckpt.parent().unwrap()).unwrap();
        // P1-3: a checkpoint now carries the expanded-flow fingerprint.
        let fingerprint = TaskBundle::open(td.join("恢复"))
            .expect("open task")
            .load_flow_with_fingerprint()
            .expect("load flow")
            .1;
        std::fs::write(
            &ckpt,
            format!(
                r#"{{"schema_version":1,"flow_name":"resume-flow","steps":2,"next_step":1,"flow_fingerprint":{fingerprint}}}"#
            ),
        )
        .unwrap();

        let mut lines = Vec::new();
        let report = run_scheduler(&opts, &mut |l| lines.push(l.to_string())).expect("recover");
        assert_eq!(report.runs, 1, "{report:?}");
        assert_eq!(report.successes, 1, "resume must finish the task: {report:?}");
        assert!(
            lines
                .iter()
                .any(|l| l.contains("scheduler_recover entry=e1")),
            "{lines:?}"
        );
        assert!(
            lines
                .iter()
                .any(|l| l.contains("scheduler_run ") && l.contains("resume=true")),
            "recovery must pass --resume: {lines:?}"
        );
        assert!(
            lines.iter().any(|l| l.contains("resume_from_step=1")),
            "run must actually resume from step 1: {lines:?}"
        );
        let state = state_of(&opts);
        assert!(state.running.is_none(), "recovery clears the dirty marker");
        assert_eq!(state.entries["e1"].runs, 1);
        // Audit trail records the recovery.
        let events = std::fs::read_to_string(opts.events_path()).unwrap();
        assert!(events.contains("\"state_recovered\""), "{events}");
        std::fs::remove_dir_all(&td).ok();
    }

    // -- result classification: fatal / blocked -----------------------------

    #[test]
    fn fatal_exit_stops_the_scheduler_and_writes_an_alert() {
        let td = tmpdir("fatal");
        let replay = td.join("replay");
        let fatal_flow = r#"{"schema_version":1,"name":"fatal","steps":[
             {"op":"finish","args":{"exit":"failed"}}]}"#;
        write_task(
            &td,
            "致命",
            fatal_flow,
            Some(
                r#"{"schema_version":1,"name":"致命","expire":"daily",
                    "exits":{"failed":{"result":"fatal"}}}"#,
            ),
        );
        write_task(&td, "正常", OK_FLOW, None);
        write_replay(&replay);
        let config = write_config(
            &td,
            r#"{"schema_version":3,
                "entries":[{"entry_id":"boom","task":"致命","lifecycle":"temp"},
                           {"entry_id":"ok","task":"正常","lifecycle":"temp"}]}"#,
        );
        let mut opts = base_opts(&td, config, Some(replay));
        opts.max_passes = Some(5);
        let mut lines = Vec::new();
        let report = run_scheduler(&opts, &mut |l| lines.push(l.to_string())).expect("run");
        assert_eq!(report.fatals, 1);
        assert_eq!(
            report.runs, 1,
            "fatal stops the pass before the next entry: {report:?}"
        );
        assert_eq!(report.stopped_reason, "fatal");
        assert!(
            lines
                .iter()
                .any(|l| l.starts_with("ALERT level=warn kind=task_fatal")),
            "{lines:?}"
        );
        assert!(report.alerts.iter().any(|a| a.contains("task_fatal")));
        std::fs::remove_dir_all(&td).ok();
    }

    #[test]
    fn keep_going_overrides_fatal() {
        let td = tmpdir("keep-going");
        let replay = td.join("replay");
        let fatal_flow = r#"{"schema_version":1,"name":"fatal","steps":[
             {"op":"finish","args":{"exit":"failed"}}]}"#;
        write_task(
            &td,
            "致命",
            fatal_flow,
            Some(
                r#"{"schema_version":1,"name":"致命","expire":"daily",
                    "exits":{"failed":"fatal"}}"#,
            ),
        );
        write_task(&td, "正常", OK_FLOW, None);
        write_replay(&replay);
        let config = write_config(
            &td,
            r#"{"schema_version":3,
                "entries":[{"entry_id":"boom","task":"致命","lifecycle":"temp"},
                           {"entry_id":"ok","task":"正常","lifecycle":"temp"}]}"#,
        );
        let mut opts = base_opts(&td, config, Some(replay));
        opts.keep_going = true;
        let report = run_scheduler(&opts, &mut |_| {}).expect("run");
        assert_eq!(report.fatals, 1);
        assert_eq!(report.runs, 2, "--keep-going continues with the next entry");
        assert_eq!(report.stopped_reason, "once");
        std::fs::remove_dir_all(&td).ok();
    }

    #[test]
    fn blocked_exit_sets_a_cooldown_and_alerts() {
        let td = tmpdir("blocked");
        let replay = td.join("replay");
        let flow = r#"{"schema_version":1,"name":"blocked","steps":[
             {"op":"finish","args":{"exit":"no_resource"}}]}"#;
        write_task(
            &td,
            "阻塞",
            flow,
            Some(
                r#"{"schema_version":1,"name":"阻塞","expire":"daily",
                    "exits":{"no_resource":{"result":"blocked"}}}"#,
            ),
        );
        write_replay(&replay);
        let config = write_config(
            &td,
            r#"{"schema_version":3,
                "config":{"defaults":{"retry":0,"cooldown":600}},
                "entries":[{"entry_id":"b1","task":"阻塞","lifecycle":"periodic",
                             "quota":{"per":"day","times":5}}]}"#,
        );
        let opts = base_opts(&td, config, Some(replay));
        let mut lines = Vec::new();
        let r1 = run_scheduler(&opts, &mut |l| lines.push(l.to_string())).expect("run 1");
        assert_eq!(r1.blocked, 1, "{r1:?}");
        assert!(r1.alerts.iter().any(|a| a.contains("task_blocked")));
        let state = state_of(&opts);
        assert!(
            state.entries["b1"].cooldown_until_ms > 0,
            "blocked => cooldown"
        );
        assert_eq!(
            state.entries["b1"].quota_day.used, 0,
            "a blocked run must not consume quota"
        );
        // Immediate second scan is filtered by the cooldown.
        let mut lines2 = Vec::new();
        let r2 = run_scheduler(&opts, &mut |l| lines2.push(l.to_string())).expect("run 2");
        assert_eq!(r2.runs, 0, "{r2:?}");
        assert!(
            lines2.iter().any(|l| l.contains("reason=cooldown")),
            "{lines2:?}"
        );
        std::fs::remove_dir_all(&td).ok();
    }

    #[test]
    fn retryable_exit_uses_the_retry_budget_then_cools_down() {
        let td = tmpdir("retry");
        let replay = td.join("replay");
        let flow = r#"{"schema_version":1,"name":"retry","steps":[
             {"op":"finish","args":{"exit":"failed"}}]}"#;
        write_task(&td, "重试", flow, None);
        write_replay(&replay);
        let config = write_config(
            &td,
            r#"{"schema_version":3,
                "config":{"defaults":{"retry":1,"cooldown":60}},
                "entries":[{"entry_id":"r1","task":"重试","lifecycle":"temp","retry":1}]}"#,
        );
        let opts = base_opts(&td, config, Some(replay));
        let mut lines = Vec::new();
        let report = run_scheduler(&opts, &mut |l| lines.push(l.to_string())).expect("run");
        assert_eq!(report.failures, 1, "{report:?}");
        let attempts: Vec<_> = lines
            .iter()
            .filter(|l| l.contains("scheduler_run entry=r1"))
            .collect();
        assert_eq!(attempts.len(), 2, "1 initial + 1 retry: {lines:?}");
        assert!(lines.iter().any(|l| l.contains("scheduler_retry")));
        std::fs::remove_dir_all(&td).ok();
    }

    // -- entry params (M0 param contract) ----------------------------------

    #[test]
    fn entry_params_are_injected_into_the_run() {
        let td = tmpdir("params");
        let replay = td.join("replay");
        let param_flow = r#"{"schema_version":1,"name":"params","steps":[
             {"op":"click_point","args":{"x":"param('x')","y":10}},
             {"op":"finish","args":{"exit":"done"}}]}"#;
        write_task(
            &td,
            "参数",
            param_flow,
            Some(
                r#"{"schema_version":1,"name":"参数","expire":"daily",
                    "params":[{"key":"x","type":"int","default":10}],
                    "exits":{"done":{"result":"success"}}}"#,
            ),
        );
        write_replay(&replay);
        let config = write_config(
            &td,
            r#"{"schema_version":3,
                "entries":[{"entry_id":"p1","task":"参数","lifecycle":"temp",
                             "params":{"x":20}}]}"#,
        );
        let opts = base_opts(&td, config, Some(replay));
        let mut lines = Vec::new();
        let report = run_scheduler(&opts, &mut |l| lines.push(l.to_string())).expect("run");
        assert_eq!(report.successes, 1, "{report:?}");
        assert!(
            lines
                .iter()
                .any(|l| l.contains("params=") && l.contains("cli_params=1")),
            "scheduler must pass the entry params as --param values: {lines:?}"
        );
        let clicks = std::fs::read_dir(td.join("clicks"))
            .expect("clicks dir")
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .find(|p| {
                p.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .ends_with(".clicks.jsonl")
            })
            .expect("click audit written");
        let text = std::fs::read_to_string(clicks).expect("clicks read");
        let row: Value =
            serde_json::from_str(text.lines().next().expect("one row")).unwrap();
        assert_eq!(
            row["target_x"], 20,
            "entry params must override the meta default: {row}"
        );
        assert_eq!(row["target_y"], 10);
        std::fs::remove_dir_all(&td).ok();
    }

    #[test]
    fn entry_params_failing_the_schema_check_fail_the_entry_not_the_scheduler() {
        // P1: the post-merge schema re-check inside `run::execute` is a per-entry
        // setup error. The scheduler must record it as this entry's failure and
        // keep the pass alive (only a corrupt values store may abort the loop).
        let td = tmpdir("params-schema-reject");
        let replay = td.join("replay");
        let flow = r#"{"schema_version":1,"name":"params","steps":[
             {"op":"log","args":{"level":"info","text":"param('x')"}},
             {"op":"finish","args":{"exit":"done"}}]}"#;
        write_task(
            &td,
            "参数",
            flow,
            Some(
                r#"{"schema_version":1,"name":"参数",
                    "params":[{"key":"x","type":"int","default":10}],
                    "exits":{"done":{"result":"success"}}}"#,
            ),
        );
        write_replay(&replay);
        let config = write_config(
            &td,
            r#"{"schema_version":3,
                "entries":[{"entry_id":"bad","task":"参数","lifecycle":"temp",
                             "params":{"x":"not-an-int"}}]}"#,
        );
        let opts = base_opts(&td, config, Some(replay));
        let mut lines = Vec::new();
        let report = run_scheduler(&opts, &mut |l| lines.push(l.to_string())).expect("scheduler");
        assert_eq!(report.successes, 0, "{report:?}");
        assert_eq!(report.failures, 1, "{report:?}");
        assert!(
            lines.iter().any(|l| l.contains("scheduler_run_error")
                && l.contains("task config:")
                && l.contains("x")),
            "the entry must surface the schema error: {lines:?}"
        );
        assert!(
            lines.iter().any(|l| l.contains("scheduler_run_done") && l.contains("class=retryable")),
            "{lines:?}"
        );
        // The failed attempt is not left as a dirty `running` marker.
        let state = state_of(&opts);
        assert!(state.running.is_none(), "running marker: {:?}", state.running);
        std::fs::remove_dir_all(&td).ok();
    }

    // -- PR3: JSON type fidelity + four-layer priority (§C.3) --------------

    const TYPED_PARAM_FLOW: &str = r#"{"schema_version":1,"name":"类型","steps":[
         {"op":"log","args":{"level":"info","text":"param('code')"}},
         {"op":"log","args":{"level":"info","text":"param('n')"}},
         {"op":"log","args":{"level":"info","text":"param('flag')"}},
         {"op":"log","args":{"level":"info","text":"param('from_values')"}},
         {"op":"finish","args":{"exit":"done"}}]}"#;
    const TYPED_PARAM_META: &str = r#"{"schema_version":1,"name":"类型",
         "params":[
            {"key":"code","type":"string","default":"000"},
            {"key":"n","type":"int","default":5},
            {"key":"flag","type":"bool","default":false},
            {"key":"from_values","type":"string","default":"meta_fallback"}],
         "exits":{"done":{"result":"success"},"failed":{"result":"retryable"}}}"#;

    /// Write `config/values.json` for `task` and return the seam path.
    fn write_runner_values(td: &Path, task: &str, values_json: &str) -> PathBuf {
        let path = td.join("config").join("values.json");
        let values: Value = serde_json::from_str(values_json).expect("values JSON");
        let values = values.as_object().expect("values object").clone();
        crate::runner_config::set_task_values(&path, task, values, 1).expect("write values");
        path
    }

    fn typed_entry_config(params: &str) -> String {
        format!(
            r#"{{"schema_version":3,
                "entries":[{{"entry_id":"tp","task":"类型","lifecycle":"temp"{params}}}]}}"#
        )
    }

    fn entry_with_params(params: BTreeMap<String, Value>) -> Entry {
        Entry {
            entry_id: "tp".to_string(),
            task: "类型".to_string(),
            kind: EntryKind::Task,
            enabled: true,
            lifecycle: Lifecycle::Temp,
            windows: Vec::new(),
            quota: None,
            params,
            retry: 0,
            cooldown_s: 0,
        }
    }

    #[test]
    fn entry_params_keep_json_scalar_types_end_to_end() {
        // R7 regression: entry `{"code":"007"}` must reach param() as Str("007"),
        // not be stringified and re-guessed into Num(7.0).
        let td = tmpdir("params-typed");
        let replay = td.join("replay");
        write_task(&td, "类型", TYPED_PARAM_FLOW, Some(TYPED_PARAM_META));
        write_replay(&replay);
        let config = write_config(
            &td,
            &typed_entry_config(r#", "params":{"code":"007","n":3,"flag":true}"#),
        );
        let mut opts = base_opts(&td, config, Some(replay));
        opts.values_path = Some(td.join("config").join("values.json"));
        let mut lines = Vec::new();
        let report = run_scheduler(&opts, &mut |l| lines.push(l.to_string())).expect("run");
        assert_eq!(report.successes, 1, "{report:?}");
        assert!(
            lines.iter().any(|l| l.contains(r#"Str("007")"#)),
            "string must reach the runtime as a string: {lines:?}"
        );
        assert!(
            lines.iter().any(|l| l.contains("Num(3.0)")),
            "number must reach the runtime as a number: {lines:?}"
        );
        assert!(
            lines.iter().any(|l| l.contains("Bool(true)")),
            "bool must reach the runtime as a bool: {lines:?}"
        );
        assert!(
            !lines.iter().any(|l| l.contains("Num(7.0)")),
            "007 must never be re-guessed as 7.0: {lines:?}"
        );
        assert!(
            lines
                .iter()
                .any(|l| l.contains("params=") && l.contains("cli_params=3")),
            "explicit layer count: {lines:?}"
        );
        std::fs::remove_dir_all(&td).ok();
    }

    #[test]
    fn four_layer_priority_entry_then_values_then_default() {
        // meta default: code "000", n 5, flag false, from_values "meta_fallback"
        // values:       code "999", n 99, flag true, from_values "v"
        // entry:        code "007", n 3
        // => entry wins for code/n, values win for flag/from_values.
        let td = tmpdir("params-layers-e2e");
        let replay = td.join("replay");
        write_task(&td, "类型", TYPED_PARAM_FLOW, Some(TYPED_PARAM_META));
        write_replay(&replay);
        let values_path = write_runner_values(
            &td,
            "类型",
            r#"{"code":"999","n":99,"flag":true,"from_values":"v"}"#,
        );
        let config = write_config(
            &td,
            &typed_entry_config(r#", "params":{"code":"007","n":3}"#),
        );
        let mut opts = base_opts(&td, config, Some(replay));
        opts.values_path = Some(values_path);
        let mut lines = Vec::new();
        let report = run_scheduler(&opts, &mut |l| lines.push(l.to_string())).expect("run");
        assert_eq!(report.successes, 1, "{report:?}");
        let has = |s: &str| lines.iter().any(|l| l.contains(s));
        assert!(has(r#"Str("007")"#), "entry beats values: {lines:?}");
        assert!(has("Num(3.0)"), "entry beats values: {lines:?}");
        assert!(has("Bool(true)"), "values beat default: {lines:?}");
        assert!(has(r#"Str("v")"#), "values-only key reaches param(): {lines:?}");
        assert!(!has(r#"Str("999")"#), "values must not beat entry: {lines:?}");
        assert!(!has("Num(99.0)"), "values must not beat entry: {lines:?}");
        assert!(
            !has(r#"Str("meta_fallback")"#),
            "values must beat the default: {lines:?}"
        );
        std::fs::remove_dir_all(&td).ok();
    }

    #[test]
    fn runner_values_apply_when_the_entry_has_no_params() {
        let td = tmpdir("params-values-only");
        let replay = td.join("replay");
        write_task(&td, "类型", TYPED_PARAM_FLOW, Some(TYPED_PARAM_META));
        write_replay(&replay);
        let values_path = write_runner_values(&td, "类型", r#"{"code":"abc","n":42}"#);
        let config = write_config(&td, &typed_entry_config(""));
        let mut opts = base_opts(&td, config, Some(replay));
        opts.values_path = Some(values_path);
        let mut lines = Vec::new();
        let report = run_scheduler(&opts, &mut |l| lines.push(l.to_string())).expect("run");
        assert_eq!(report.successes, 1, "{report:?}");
        let has = |s: &str| lines.iter().any(|l| l.contains(s));
        assert!(has(r#"Str("abc")"#), "{lines:?}");
        assert!(has("Num(42.0)"), "{lines:?}");
        // Unset key still falls back to the declared default.
        assert!(has("Bool(false)"), "{lines:?}");
        assert!(has(r#"Str("meta_fallback")"#), "{lines:?}");
        std::fs::remove_dir_all(&td).ok();
    }

    #[test]
    fn attempt_params_merge_is_type_preserving_and_loud_on_corrupt_values() {
        let td = tmpdir("params-merge-unit");
        let values_path = write_runner_values(&td, "任务1", r#"{"code":"999","n":99}"#);
        let opts = SchedulerOptions {
            values_path: Some(values_path.clone()),
            ..SchedulerOptions::default()
        };
        let entry = entry_with_params(BTreeMap::from([
            ("code".to_string(), json!("007")),
            ("flag".to_string(), json!(true)),
        ]));
        let merged = attempt_run_params(&opts, "任务1", &entry).expect("merge");
        assert_eq!(merged["code"], json!("007"), "entry wins over values");
        assert_eq!(merged["n"], json!(99), "values survive under entry");
        assert_eq!(merged["flag"], json!(true));

        // A task without an entry in values.json gets only the entry layer.
        let other = attempt_run_params(&opts, "任务2", &entry).expect("merge");
        assert_eq!(other.len(), 2, "{other:?}");
        assert!(other.get("n").is_none(), "{other:?}");

        // A corrupt store is a loud error carrying the task (never a silent
        // empty layer: runner_config's contract).
        std::fs::write(&values_path, "{ not json").expect("corrupt values");
        let err = attempt_run_params(&opts, "任务1", &entry).expect_err("must be loud");
        assert!(err.contains("任务1"), "{err}");
        assert!(err.contains("invalid JSON"), "{err}");
        std::fs::remove_dir_all(&td).ok();
    }

    // -- guards (deterministic: a full-day blackout) ------------------------

    #[test]
    fn guard_blackout_blocks_the_whole_pass() {
        let td = tmpdir("guard");
        let replay = td.join("replay");
        write_task(&td, "任务1", OK_FLOW, None);
        write_replay(&replay);
        let config = write_config(
            &td,
            r#"{"schema_version":3,
                "config":{"guards":{"blackouts":[
                    {"days":[],"start":"00:00","end":"00:00","reason":"维护"}]}},
                "entries":[{"entry_id":"e1","task":"任务1","lifecycle":"temp"}]}"#,
        );
        let opts = base_opts(&td, config, Some(replay));
        let mut lines = Vec::new();
        let report = run_scheduler(&opts, &mut |l| lines.push(l.to_string())).expect("run");
        assert_eq!(report.runs, 0, "guards block every entry: {report:?}");
        assert!(
            lines
                .iter()
                .any(|l| l.contains("scheduler_guard blocked=blackout") && l.contains("维护")),
            "{lines:?}"
        );
        assert!(lines.iter().any(|l| l.starts_with("ALERT")), "{lines:?}");
        assert!(report.alerts.iter().any(|a| a.contains("guard_blocked")));
        std::fs::remove_dir_all(&td).ok();
    }

    // -- config fail-fast ---------------------------------------------------

    #[test]
    fn missing_task_fails_fast_without_writing_state() {
        let td = tmpdir("missing-task");
        let config = write_config(
            &td,
            r#"{"schema_version":3,"entries":[{"entry_id":"e1","task":"不存在","lifecycle":"periodic"}]}"#,
        );
        let opts = base_opts(&td, config, None);
        let err = run_scheduler(&opts, &mut |_| {}).expect_err("must fail fast");
        assert!(err.contains("not found"), "{err}");
        assert!(
            !opts.state_path().exists(),
            "invalid config must not write state"
        );
        std::fs::remove_dir_all(&td).ok();
    }

    #[test]
    fn v1_plan_is_rejected_with_a_migration_hint() {
        let td = tmpdir("v1");
        write_task(&td, "任务1", OK_FLOW, None);
        let config = write_config(
            &td,
            r#"{"schema_version":1,"jobs":[{"task":"任务1","every_s":3600}]}"#,
        );
        let err = run_scheduler(&base_opts(&td, config, None), &mut |_| {})
            .expect_err("v1 must be rejected");
        assert!(err.contains("schema_version must be 4"), "{err}");
        assert!(err.contains("v4") || err.contains("§22.34"), "{err}");
        std::fs::remove_dir_all(&td).ok();
    }

    // -- lock ---------------------------------------------------------------

    #[test]
    fn single_instance_lock_blocks_a_second_scheduler() {
        let td = tmpdir("lock");
        let replay = td.join("replay");
        write_task(&td, "任务1", OK_FLOW, None);
        write_replay(&replay);
        let config = write_config(
            &td,
            r#"{"schema_version":3,"entries":[{"entry_id":"e1","task":"任务1","lifecycle":"periodic"}]}"#,
        );
        let opts = base_opts(&td, config, Some(replay));
        let lock_path = SchedulerLock::lock_path(&opts.state_path());
        std::fs::write(&lock_path, format!(r#"{{"pid":{},"started_ms":0}}"#, std::process::id()))
            .unwrap();
        let err = run_scheduler(&opts, &mut |_| {}).expect_err("lock must block");
        assert!(err.contains("already running"), "{err}");
        std::fs::remove_file(&lock_path).unwrap();
        let report = run_scheduler(&opts, &mut |_| {}).expect("lock reclaimed");
        assert_eq!(report.runs, 1);
        assert!(!lock_path.exists(), "clean exit releases the lock");
        std::fs::remove_dir_all(&td).ok();
    }

    // -- PR6: cooperative stop (Runner `POST /runner/scheduler/stop`) -------

    #[test]
    fn scheduler_options_default_has_no_stop_token() {
        assert!(SchedulerOptions::default().stop.is_none());
    }

    /// One looping plan + scripted executor: the first pass runs immediately,
    /// then the loop waits `loop_interval_s=1` — plenty of time to request a
    /// stop (the sleep is sliced at 200 ms for exactly this test).
    fn stop_test_opts(td: &Path) -> SchedulerOptions {
        write_task(td, "任务1", OK_FLOW, None);
        let config = write_config(
            td,
            r#"{"schema_version":4,
                "config":{"mode":"loop","loop_interval_s":1},
                "entries":[{"entry_id":"e1","task":"任务1","lifecycle":"temp"}]}"#,
        );
        let mut opts = base_opts(td, config, None);
        opts.once = false;
        opts.test_exec = Some(TestExec::new().with("任务1", vec![step(ResultClass::Success)]));
        opts
    }

    #[test]
    fn preset_stop_token_stops_before_the_first_pass_with_api_stop() {
        let td = tmpdir("stop-preset");
        let mut opts = stop_test_opts(&td);
        opts.stop = Some(Arc::new(AtomicBool::new(true)));
        let mut lines = Vec::new();
        let report = run_scheduler(&opts, &mut |l| lines.push(l.to_string())).expect("run");
        assert_eq!(report.stopped_reason, "api_stop", "{report:?}");
        assert_eq!(report.passes, 0, "no pass may start after a preset stop");
        assert_eq!(report.runs, 0);
        let state = state_of(&opts);
        assert_eq!(state.passes, 0);
        assert_eq!(state.last_stop, "api_stop");
        assert!(state.running.is_none(), "no dirty running marker");
        assert!(
            lines.iter().any(|l| l == "scheduler_stop_requested source=api"),
            "{lines:?}"
        );
        assert!(
            lines.iter().any(|l| l.contains("scheduler_stop reason=api_stop")),
            "{lines:?}"
        );
        assert!(
            !SchedulerLock::lock_path(&opts.state_path()).exists(),
            "the lock must be released on an api_stop exit"
        );
        std::fs::remove_dir_all(&td).ok();
    }

    #[test]
    fn stop_token_set_while_running_exits_between_passes_without_dirty_state() {
        let td = tmpdir("stop-running");
        let mut opts = stop_test_opts(&td);
        let stop = Arc::new(AtomicBool::new(false));
        opts.stop = Some(stop.clone());
        let (tx, rx) = std::sync::mpsc::channel::<String>();
        let report = std::thread::scope(|scope| {
            let handle = scope.spawn(|| {
                let mut emit = |line: &str| {
                    let _ = tx.send(line.to_string());
                };
                run_scheduler(&opts, &mut emit)
            });
            // Wait until the first attempt actually ran: this proves the stop
            // is honoured *after* the attempt, never in the middle of one.
            let t0 = std::time::Instant::now();
            let mut ran = false;
            while !ran && t0.elapsed() < Duration::from_secs(5) {
                if let Ok(line) = rx.recv_timeout(Duration::from_millis(100)) {
                    if line.starts_with("scheduler_run ") {
                        ran = true;
                    }
                }
            }
            assert!(ran, "the scripted attempt never ran");
            stop.store(true, Ordering::SeqCst);
            handle.join().expect("scheduler thread").expect("run")
        });
        assert_eq!(report.stopped_reason, "api_stop", "{report:?}");
        assert!(report.passes >= 1, "{report:?}");
        assert!(report.runs >= 1, "{report:?}");
        let state = state_of(&opts);
        assert_eq!(state.last_stop, "api_stop");
        assert!(
            state.running.is_none(),
            "stop must not leave a dirty running marker"
        );
        assert!(
            !SchedulerLock::lock_path(&opts.state_path()).exists(),
            "the lock must be released on an api_stop exit"
        );
        std::fs::remove_dir_all(&td).ok();
    }

    // -- PR6: per-pass device lease (P1-5/P1-6) -----------------------------

    #[test]
    fn options_default_has_no_lease_path() {
        assert!(SchedulerOptions::default().lease_path.is_none());
    }

    #[test]
    fn acquire_pass_lease_is_a_loud_conflict_while_held() {
        let td = tmpdir("lease-helper");
        let path = td.join("config").join("device.lease");
        let opts = SchedulerOptions {
            lease_path: Some(path.clone()),
            ..SchedulerOptions::default()
        };
        let first = acquire_pass_lease(&opts)
            .expect("first acquire")
            .expect("lease path configured");
        assert_eq!(
            crate::device_lease::read_lease(&path).map(|i| i.mode),
            Some("scheduler".to_string())
        );
        let err = acquire_pass_lease(&opts).expect_err("a live holder must block the next pass");
        assert!(err.contains("Runner 正在调度"), "{err}");
        drop(first);
        assert!(!path.exists(), "dropping the pass lease releases the file");
        assert!(
            acquire_pass_lease(&opts).expect("released").is_some(),
            "the device is usable again"
        );
        std::fs::remove_dir_all(&td).ok();
    }

    #[test]
    fn scheduler_pass_holds_the_lease_and_idles_without_it() {
        let td = tmpdir("lease-per-pass");
        write_task(&td, "任务1", OK_FLOW, None);
        let config = write_config(
            &td,
            r#"{"schema_version":4,
                "config":{"mode":"loop","loop_interval_s":1},
                "entries":[{"entry_id":"e1","task":"任务1","lifecycle":"temp"}]}"#,
        );
        let mut opts = base_opts(&td, config, None);
        opts.once = false;
        let lease_path = td.join("config").join("device.lease");
        opts.lease_path = Some(lease_path.clone());
        let stop = Arc::new(AtomicBool::new(false));
        opts.stop = Some(stop.clone());
        opts.test_exec = Some(TestExec::new().with("任务1", vec![step(ResultClass::Success)]));
        let held_during_attempt = Arc::new(AtomicBool::new(false));
        let idle_acquire = Arc::new(AtomicBool::new(false));
        let held_flag = held_during_attempt.clone();
        let idle_flag = idle_acquire.clone();
        let lp = lease_path.clone();
        let report = std::thread::scope(|scope| {
            let handle = scope.spawn(|| {
                let mut emit = |line: &str| {
                    if line.starts_with("scheduler_run ") {
                        if let Some(info) = crate::device_lease::read_lease(&lp) {
                            if info.owner == "runner-scheduler" && info.mode == "scheduler" {
                                held_flag.store(true, Ordering::SeqCst);
                            }
                        }
                    }
                    if line.starts_with("scheduler_wake ") {
                        // The pass is over: the device must be free, so a temp
                        // run (same pid) can acquire it now.
                        if let Ok(lease) = DeviceLease::acquire(&lp, "runner", "temp-run") {
                            idle_flag.store(true, Ordering::SeqCst);
                            drop(lease);
                        }
                        stop.store(true, Ordering::SeqCst);
                    }
                };
                run_scheduler(&opts, &mut emit)
            });
            handle.join().expect("scheduler thread").expect("run")
        });
        assert!(
            held_during_attempt.load(Ordering::SeqCst),
            "the running attempt must hold owner=runner-scheduler/mode=scheduler: {report:?}"
        );
        assert!(
            idle_acquire.load(Ordering::SeqCst),
            "the inter-pass sleep must not hold the device lease"
        );
        assert!(report.passes >= 1, "{report:?}");
        assert!(report.runs >= 1, "{report:?}");
        assert_eq!(report.stopped_reason, "api_stop", "{report:?}");
        assert!(
            !lease_path.exists(),
            "the pass lease is released after the stop"
        );
        std::fs::remove_dir_all(&td).ok();
    }

    #[test]
    fn scheduler_attempt_does_not_double_acquire_the_pass_lease() {
        let td = tmpdir("lease-attempt");
        write_task(&td, "任务1", OK_FLOW, None);
        let replay = td.join("replay");
        write_replay(&replay);
        let config = write_config(
            &td,
            r#"{"schema_version":4,
                "config":{"mode":"stop"},
                "entries":[{"entry_id":"e1","task":"任务1","lifecycle":"temp"}]}"#,
        );
        let mut opts = base_opts(&td, config, Some(replay));
        let lease_path = td.join("config").join("device.lease");
        opts.lease_path = Some(lease_path.clone());
        let report = run_scheduler(&opts, &mut |_| {}).expect("real replay run");
        assert_eq!(report.runs, 1, "{report:?}");
        assert_eq!(
            report.successes, 1,
            "the attempt must run under the pass lease with skip_lease=true: {report:?}"
        );
        assert!(!lease_path.exists(), "the pass lease is released after the run");
        std::fs::remove_dir_all(&td).ok();
    }

    #[test]
    fn scheduler_lease_conflict_skips_the_pass_without_dirty_state() {
        let td = tmpdir("lease-conflict");
        write_task(&td, "任务1", OK_FLOW, None);
        let config = write_config(
            &td,
            r#"{"schema_version":4,
                "config":{"mode":"loop","loop_interval_s":1},
                "entries":[{"entry_id":"e1","task":"任务1","lifecycle":"temp"}]}"#,
        );
        let mut opts = base_opts(&td, config, None);
        opts.once = false;
        let lease_path = td.join("config").join("device.lease");
        opts.lease_path = Some(lease_path.clone());
        let stop = Arc::new(AtomicBool::new(false));
        opts.stop = Some(stop.clone());
        let exec = TestExec::new().with("任务1", vec![step(ResultClass::Success)]);
        opts.test_exec = Some(exec.clone());
        // A live Studio trial: same pid + scheduler-style JSON, but never
        // dropped by the scheduler (it models another surface's lease).
        let external =
            DeviceLease::acquire(&lease_path, "studio", "editor-run").expect("external lease");
        let (tx, rx) = std::sync::mpsc::channel::<String>();
        let report = std::thread::scope(|scope| {
            let handle = scope.spawn(|| {
                let mut emit = |line: &str| {
                    let _ = tx.send(line.to_string());
                };
                run_scheduler(&opts, &mut emit)
            });
            let mut saw = false;
            let t0 = std::time::Instant::now();
            while !saw && t0.elapsed() < Duration::from_secs(5) {
                if let Ok(line) = rx.recv_timeout(Duration::from_millis(100)) {
                    if line.starts_with("scheduler_lease_busy ") {
                        saw = true;
                    }
                }
            }
            assert!(saw, "the scheduler never reported scheduler_lease_busy");
            stop.store(true, Ordering::SeqCst);
            handle.join().expect("scheduler thread").expect("run")
        });
        assert_eq!(report.stopped_reason, "api_stop", "{report:?}");
        assert_eq!(report.passes, 0, "a busy lease must skip the pass: {report:?}");
        assert_eq!(report.runs, 0);
        assert!(exec.calls().is_empty(), "the scripted attempt must not run");
        let state = state_of(&opts);
        assert_eq!(state.passes, 0, "a skipped pass is not counted");
        assert!(state.running.is_none(), "no dirty running marker");
        assert_eq!(state.last_stop, "api_stop");
        assert!(lease_path.exists(), "the external holder keeps the lease");
        let events = std::fs::read_to_string(opts.events_path()).expect("events");
        assert!(events.contains("scheduler_lease_busy"), "{events}");
        assert!(
            !SchedulerLock::lock_path(&opts.state_path()).exists(),
            "the scheduler lock is released on an api_stop exit"
        );
        drop(external);
        assert!(!lease_path.exists(), "dropping the external lease frees it");
        std::fs::remove_dir_all(&td).ok();
    }

    // -- reset-memory -------------------------------------------------------

    #[test]
    fn reset_memory_clears_entry_state_and_task_state_and_audits() {
        let td = tmpdir("reset");
        let replay = td.join("replay");
        write_task(&td, "任务1", OK_FLOW, None);
        write_replay(&replay);
        let config = write_config(
            &td,
            r#"{"schema_version":3,
                "entries":[{"entry_id":"e1","task":"任务1","lifecycle":"periodic",
                             "quota":{"per":"day","times":1}}]}"#,
        );
        let mut opts = base_opts(&td, config.clone(), Some(replay));
        opts.once = true;
        let r = run_scheduler(&opts, &mut |_| {}).expect("run");
        assert_eq!(r.runs, 1);
        // Pretend the task left a checkpoint + learned coords.
        let task_state = td.join("任务1").join("state");
        std::fs::create_dir_all(&task_state).unwrap();
        std::fs::write(task_state.join("checkpoint.json"), "{}").unwrap();
        std::fs::write(task_state.join("coords.json"), "{}").unwrap();
        let before = state_of(&opts);
        assert_eq!(before.entries["e1"].quota_day.used, 1);

        let reset = ResetOptions {
            config: config.clone(),
            tasks_dir: td.clone(),
            tasks: vec!["任务1".to_string()],
            include_checkpoint: true,
            ..ResetOptions::default()
        };
        let mut lines = Vec::new();
        let report = reset_memory(&reset, &mut |l| lines.push(l.to_string())).expect("reset");
        assert_eq!(report.entries, vec!["e1".to_string()]);
        assert!(report.files_removed >= 2, "{report:?}");
        assert!(lines.iter().any(|l| l.contains("reset_memory_tasks=任务1")));

        // ① scheduler entry state gone ② task state/ emptied.
        let after = state_of(&opts);
        assert!(!after.entries.contains_key("e1"));
        assert_eq!(
            std::fs::read_dir(&task_state).unwrap().count(),
            0,
            "checkpoint + coords must be cleared by default"
        );
        // ③ audit event written.
        let events = std::fs::read_to_string(opts.events_path()).unwrap();
        assert!(events.contains("\"event\":\"memory_reset\""), "{events}");
        assert!(events.contains("任务1"), "{events}");

        // Quota is reset: a new scheduler run executes again.
        let r2 = run_scheduler(&opts, &mut |_| {}).expect("run after reset");
        assert_eq!(r2.runs, 1, "reset must clear the quota counter: {r2:?}");
        std::fs::remove_dir_all(&td).ok();
    }

    #[test]
    fn reset_memory_refuses_while_the_scheduler_is_running() {
        let td = tmpdir("reset-running");
        let config = write_config(
            &td,
            r#"{"schema_version":3,"entries":[{"entry_id":"e1","task":"任务1"}]}"#,
        );
        let reset = ResetOptions {
            config,
            tasks_dir: td.clone(),
            tasks: vec!["任务1".to_string()],
            include_checkpoint: true,
            ..ResetOptions::default()
        };
        let lock_path = SchedulerLock::lock_path(&reset.state_path());
        std::fs::write(&lock_path, format!(r#"{{"pid":{}}}"#, std::process::id())).unwrap();
        let err = reset_memory(&reset, &mut |_| {}).expect_err("must refuse");
        assert!(err.contains("scheduler is running"), "{err}");
        std::fs::remove_file(&lock_path).ok();
        std::fs::remove_dir_all(&td).ok();
    }

    #[test]
    fn reset_memory_keep_checkpoint_flag_keeps_checkpoint_only() {
        let td = tmpdir("reset-keep");
        write_task(&td, "任务1", OK_FLOW, None);
        let dir = td.join("任务1").join("state");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("checkpoint.json"), "{}").unwrap();
        std::fs::write(dir.join("coords.json"), "{}").unwrap();
        let reset = ResetOptions {
            config: td.join("scheduler.json"),
            tasks_dir: td.clone(),
            tasks: vec!["任务1".to_string()],
            include_checkpoint: true,
            keep_checkpoint: true,
            ..ResetOptions::default()
        };
        reset_memory(&reset, &mut |_| {}).expect("reset");
        assert!(dir.join("checkpoint.json").is_file(), "checkpoint kept");
        assert!(!dir.join("coords.json").exists(), "other memory cleared");
        std::fs::remove_dir_all(&td).ok();
    }

    // -- CLI parsing --------------------------------------------------------

    #[test]
    fn options_parse_all_documented_flags() {
        let raw: Vec<String> = [
            "--config",
            "cfg/scheduler.json",
            "--state",
            "st.json",
            "--tasks-dir",
            "tasks",
            "--library",
            "lib",
            "--replay",
            "rep",
            "--clicks-dir",
            "clicks",
            "--shots-dir",
            "shots",
            "--seed",
            "7",
            "--once",
            "--max-passes",
            "3",
            "--dry-run",
            "--status",
            "--report",
            "r.json",
            "--events",
            "e.jsonl",
            "--keep-going",
            "--tz-offset-min",
            "480",
            "--tick-ms",
            "500",
            "--loop-interval-s",
            "2",
            "--model",
            "rec.onnx",
            "--det-model",
            "det.onnx",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let o = SchedulerOptions::parse(&raw).expect("parse");
        assert_eq!(o.config, PathBuf::from("cfg/scheduler.json"));
        assert_eq!(o.state, Some(PathBuf::from("st.json")));
        assert_eq!(o.tasks_dir, PathBuf::from("tasks"));
        assert_eq!(o.library, PathBuf::from("lib"));
        assert_eq!(o.replay, Some(PathBuf::from("rep")));
        assert_eq!(o.clicks_dir, Some(PathBuf::from("clicks")));
        assert_eq!(o.shots_dir, Some(PathBuf::from("shots")));
        assert_eq!(o.seed, Some(7));
        assert!(o.once && o.dry_run && o.status && o.keep_going);
        assert_eq!(o.max_passes, Some(3));
        assert_eq!(o.report, Some(PathBuf::from("r.json")));
        assert_eq!(o.events, Some(PathBuf::from("e.jsonl")));
        assert_eq!(o.tz_offset_min, Some(480));
        assert_eq!(o.max_sleep_ms, 500);
        assert_eq!(o.loop_interval_s, Some(2));
        assert_eq!(o.model, Some(PathBuf::from("rec.onnx")));
        assert_eq!(o.det_model, Some(PathBuf::from("det.onnx")));
        // `--plan` is the schedule-file spelling and `--max-passes` is the
        // primary pass-cap flag; `--max-rounds`/`--max-runs` stay as deprecated
        // aliases (all three spellings must be equivalent).
        let alias: Vec<String> = ["--plan", "p.json", "--max-runs", "2"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let o = SchedulerOptions::parse(&alias).expect("alias parse");
        assert_eq!(o.config, PathBuf::from("p.json"));
        assert_eq!(o.max_passes, Some(2));
        for spelling in ["--max-passes", "--max-rounds", "--max-runs"] {
            let argv = [spelling.to_string(), "2".to_string()];
            let parsed = SchedulerOptions::parse(&argv).expect(spelling);
            assert_eq!(parsed.max_passes, Some(2), "{spelling} is the pass cap");
        }
        assert!(SchedulerOptions::parse(&["--nope".to_string()]).is_err());
        for spelling in ["--max-passes", "--max-rounds", "--max-runs"] {
            let err = SchedulerOptions::parse(&[spelling.to_string(), "0".to_string()])
                .expect_err("0 is not a pass cap");
            assert!(err.contains(spelling), "{spelling}: {err}");
        }
    }

    #[test]
    fn default_paths_are_m3_compatible() {
        let o = SchedulerOptions::default();
        assert_eq!(o.config, PathBuf::from("scheduler.json"));
        assert_eq!(o.state_path(), PathBuf::from("scheduler.state.json"));
        assert_eq!(o.events_path(), PathBuf::from("scheduler.events.jsonl"));
        assert_eq!(
            sibling_with_suffix(Path::new("cfg/live-scheduler.json"), "lock"),
            PathBuf::from("cfg/live-scheduler.lock")
        );
    }

    #[test]
    fn reset_options_parse() {
        let raw: Vec<String> = ["--tasks", "a,b", "--all", "--include-checkpoint"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let o = ResetOptions::parse(&raw).expect("parse");
        assert_eq!(o.tasks, vec!["a".to_string(), "b".to_string()]);
        assert!(o.all && o.include_checkpoint && !o.keep_checkpoint);
        assert!(ResetOptions::parse(&[]).is_err(), "a selector is required");
        let keep: Vec<String> = ["--tasks", "a", "--include-checkpoint", "--keep-checkpoint"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let parsed = ResetOptions::parse(&keep).expect("keep-checkpoint wins over include");
        assert!(parsed.keep_checkpoint && parsed.include_checkpoint);
    }

    // -- report -------------------------------------------------------------

    #[test]
    fn report_contains_session_and_entry_details() {
        let td = tmpdir("report");
        let replay = td.join("replay");
        write_task(&td, "任务1", OK_FLOW, None);
        write_replay(&replay);
        let config = write_config(
            &td,
            r#"{"schema_version":3,
                "entries":[{"entry_id":"e1","task":"任务1","lifecycle":"periodic",
                             "quota":{"per":"day","times":1}}]}"#,
        );
        let mut opts = base_opts(&td, config, Some(replay));
        opts.report = Some(td.join("report.json"));
        let report = run_scheduler(&opts, &mut |_| {}).expect("run");
        assert!(opts.report.as_ref().unwrap().is_file());
        let raw = std::fs::read_to_string(opts.report.as_ref().unwrap()).unwrap();
        let v: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(v["schema_version"], 4);
        assert_eq!(v["runs"], 1);
        assert_eq!(v["successes"], 1);
        assert_eq!(v["entries"][0]["entry_id"], "e1");
        assert_eq!(v["entries"][0]["quota_used"], 1);
        assert_eq!(v["stopped_reason"], "once");
        assert!(
            v["queues"].as_array().map(|a| a.is_empty()).unwrap_or(false),
            "task-only configs report an empty queue list: {v}"
        );
        assert_eq!(report.entries[0].quota_remaining, Some(0));

        // Backward compatibility: an older report has no `queues` key; the
        // `#[serde(default)]` field reads as an empty list (additive P2 change).
        let mut old = serde_json::to_value(&report).expect("report to json");
        old.as_object_mut().expect("report object").remove("queues");
        let reparsed: SchedulerReport =
            serde_json::from_value(old).expect("older report still parses");
        assert!(reparsed.queues.is_empty());
        assert_eq!(reparsed.runs, report.runs);
        std::fs::remove_dir_all(&td).ok();
    }

    // -- state schema v3 -> v4 migration ------------------------------------

    #[test]
    fn v3_state_migrates_to_v4_with_empty_queues() {
        let td = tmpdir("state-v3");
        let state_path = td.join("scheduler.state.json");
        // Exactly what the previous release wrote: schema 3, no `queues`.
        let v3 = r#"{"schema_version":3,"created_ms":1,"passes":7,
                     "last_stop":"once","cycle":{"day":"2026-10-23","week":"2026-10-19"},
                     "entries":{"e1":{"task":"任务1","runs":2,"successes":2,
                                      "quota_day":{"cycle":"2026-10-23","used":2}}}}"#;
        std::fs::write(&state_path, v3).unwrap();

        let (state, note) = load_state_migrating(&state_path).expect("v3 state loads");
        assert_eq!(state.schema_version, 4, "v3 is upgraded on load");
        assert!(state.queues.is_empty(), "v3 had no queue state");
        assert_eq!(state.passes, 7, "old fields survive the migration");
        assert_eq!(state.entries["e1"].runs, 2);
        assert_eq!(state.entries["e1"].quota_day.used, 2);
        let note = note.expect("migration is announced");
        assert!(note.contains("migrated"), "{note}");

        // Saving always writes v4 (the daemon saves right after startup).
        write_json_atomic(&state_path, &state).unwrap();
        let text = std::fs::read_to_string(&state_path).unwrap();
        let v: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["schema_version"], 4);
        assert_eq!(v["entries"]["e1"]["runs"], 2);
        std::fs::remove_dir_all(&td).ok();
    }

    #[test]
    fn v3_state_migration_backs_up_the_original_before_writing_v4() {
        let td = tmpdir("state-backup");
        let config = write_config(&td, r#"{"schema_version":3,"entries":[]}"#);
        let opts = base_opts(&td, config, None);
        let state_path = opts.state_path();
        let v3 = r#"{"schema_version":3,"created_ms":1,"passes":7,
                     "last_stop":"once","entries":{"e1":{"task":"任务1","runs":2}}}"#;
        std::fs::write(&state_path, v3).unwrap();
        let mut lines = Vec::new();
        run_scheduler(&opts, &mut |l| lines.push(l.to_string())).expect("run");
        let backup = state_v3_backup_path(&state_path);
        assert_eq!(
            std::fs::read_to_string(&backup).expect("v3 backup exists"),
            v3,
            "the backup is the exact pre-migration v3 file"
        );
        assert!(
            lines
                .iter()
                .any(|l| l.contains("scheduler_state_backup path=") && l.contains("v3.bak")),
            "{lines:?}"
        );
        let migrated: Value =
            serde_json::from_str(&std::fs::read_to_string(&state_path).unwrap()).unwrap();
        assert_eq!(migrated["schema_version"], 4, "the state was rewritten as v4");
        assert_eq!(migrated["passes"], 8, "the migrated counters continue");
        std::fs::remove_dir_all(&td).ok();
    }

    #[test]
    fn v4_state_roundtrips_queue_rounds() {
        let td = tmpdir("state-v4");
        let state_path = td.join("scheduler.state.json");
        let mut state = SchedulerState::new();
        state.queues.insert(
            "q1".to_string(),
            scheduler::QueueState {
                total_rounds: 2,
                rounds_completed: 1,
                empty_rounds: 3,
                finished: false,
                current_round: Some(scheduler::QueueRoundState {
                    index: 2,
                    member_index: 1,
                    started_ms: 500,
                }),
                pending_entry_record: None,
                member_results: vec![scheduler::QueueMemberResult {
                    round: 2,
                    member_index: 0,
                    task: "任务1".to_string(),
                    class: "success".to_string(),
                    exit: "done".to_string(),
                    outcome: "Done".to_string(),
                    clicks: 0,
                    attempts: 1,
                    duration_ms: 10,
                    finished_ms: 600,
                }],
            },
        );
        write_json_atomic(&state_path, &state).expect("write v4");
        let (loaded, note) = load_state_migrating(&state_path).expect("load v4");
        assert!(note.is_none(), "v4 needs no migration");
        assert_eq!(loaded, state, "queue state round-trips exactly");
        assert_eq!(
            loaded.queues["q1"].current_round.as_ref().unwrap().member_index,
            1,
            "the restart cursor survives"
        );
        std::fs::remove_dir_all(&td).ok();
    }

    #[test]
    fn unsupported_state_schema_is_rejected() {
        let td = tmpdir("state-v5");
        let state_path = td.join("scheduler.state.json");
        std::fs::write(&state_path, r#"{"schema_version":5}"#).unwrap();
        let err = load_state(&state_path).expect_err("v5 unsupported");
        assert!(err.contains("schema_version 5 unsupported"), "{err}");
        std::fs::write(&state_path, r#"{"schema_version":2}"#).unwrap();
        assert!(load_state(&state_path).is_err(), "v2 stays unsupported");
        std::fs::remove_dir_all(&td).ok();
    }

    // -- queue schema in the IO layer (status / fail-fast / reset) -----------

    #[test]
    fn status_json_exposes_queue_state_read_only() {
        let td = tmpdir("status-queue");
        write_task(&td, "任务1", OK_FLOW, None);
        let config = write_config(
            &td,
            r#"{"schema_version":4,
                "queues":[{"queue_id":"q1","name":"日常","members":["任务1"],
                           "total_rounds":2,"on_failure":"fail_fast","between_ms":250}],
                "entries":[{"entry_id":"qe","task":"q1","kind":"queue","lifecycle":"temp"}]}"#,
        );
        let opts = base_opts(&td, config, None);
        let mut state = SchedulerState::new();
        state.queues.insert(
            "q1".to_string(),
            scheduler::QueueState {
                total_rounds: 2,
                rounds_completed: 1,
                empty_rounds: 0,
                finished: false,
                current_round: Some(scheduler::QueueRoundState {
                    index: 2,
                    member_index: 0,
                    started_ms: 1,
                }),
                pending_entry_record: None,
                member_results: vec![scheduler::QueueMemberResult {
                    round: 1,
                    member_index: 0,
                    task: "任务1".to_string(),
                    class: "success".to_string(),
                    exit: "done".to_string(),
                    outcome: "Done".to_string(),
                    clicks: 0,
                    attempts: 1,
                    duration_ms: 5,
                    finished_ms: 10,
                }],
            },
        );
        write_json_atomic(&opts.state_path(), &state).unwrap();

        let v = status_json(&opts).expect("status");
        assert_eq!(v["state_schema_version"], 4);
        let q = &v["queues"][0];
        assert_eq!(q["queue_id"], "q1");
        assert_eq!(q["name"], "日常");
        assert_eq!(q["total_rounds"], 2);
        assert_eq!(q["rounds_completed"], 1);
        assert_eq!(q["on_failure"], "fail_fast");
        assert_eq!(q["between_ms"], 250);
        assert_eq!(q["current_round"]["index"], 2);
        assert_eq!(q["current_round"]["member_index"], 0);
        assert_eq!(q["member_results"][0]["task"], "任务1");
        assert_eq!(q["finished"], false);
        assert!(!q["next_due_ms"].is_null(), "an open round is due now");
        // P2 status additions: last-round summary + rounds_remaining.
        assert_eq!(q["last_round"]["round"], 1);
        assert_eq!(q["last_round"]["executed"], 1);
        assert_eq!(q["last_round"]["members"][0]["task"], "任务1");
        assert_eq!(q["last_round"]["members"][0]["clicks"], 0);
        assert_eq!(q["rounds_remaining"], 1);
        // The queue entry is labelled and carries the queue_id (not a task name)
        // plus its P2 round progress (rule 5/6/8).
        assert_eq!(v["entries"][0]["kind"], "queue");
        assert_eq!(v["entries"][0]["queue_id"], "q1");
        assert_eq!(v["entries"][0]["queue"]["rounds_completed"], 1);
        assert_eq!(v["entries"][0]["queue"]["current_round"]["index"], 2);
        assert_eq!(v["entries"][0]["queue"]["finished"], false);
        assert_eq!(v["entries"][0]["queue"]["last_round"]["round"], 1);

        // Queue members are tasks for the reset-memory multi-select.
        let tasks: Vec<&str> = v["task_choices"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["task"].as_str().unwrap())
            .collect();
        assert_eq!(tasks, vec!["任务1"], "{tasks:?}");

        // A queue that never ran reports "due now" instead of null.
        std::fs::remove_file(opts.state_path()).unwrap();
        let fresh = status_json(&opts).expect("status without state");
        assert_eq!(fresh["queues"][0]["rounds_completed"], 0);
        assert_eq!(fresh["queues"][0]["finished"], false);
        assert!(fresh["queues"][0]["last_round"].is_null());
        assert!(!fresh["queues"][0]["next_due_ms"].is_null());
        std::fs::remove_dir_all(&td).ok();
    }

    #[test]
    fn invalid_queue_config_writes_no_state() {
        let td = tmpdir("queue-invalid");
        let config = write_config(
            &td,
            r#"{"schema_version":4,
                "queues":[{"queue_id":"q1","members":["不存在"]}],
                "entries":[{"entry_id":"qe","task":"q1","kind":"queue"}]}"#,
        );
        let opts = base_opts(&td, config, None);
        let err = run_scheduler(&opts, &mut |_| {}).expect_err("missing member fails fast");
        assert!(err.contains("queue `q1` member"), "{err}");
        assert!(err.contains("not found"), "{err}");
        assert!(
            !opts.state_path().exists(),
            "invalid config must not write state"
        );
        std::fs::remove_dir_all(&td).ok();
    }

    // -- queue rounds (P2): executor, lazy counting, recovery, policies ------

    #[test]
    fn queue_round_runs_two_members_and_counts_one_round() {
        let td = tmpdir("q-round");
        write_task(&td, "成员1", OK_FLOW, None);
        write_task(&td, "成员2", OK_FLOW, None);
        let config = write_config(
            &td,
            &queue_config_text(
                &["成员1", "成员2"],
                2,
                "continue",
                0,
                r#","quota":{"per":"day","times":2}"#,
                "",
            ),
        );
        let mut opts = queue_opts(
            &td,
            config,
            exec_two(ResultClass::Success, ResultClass::Success),
        );
        let report_path = td.join("report.json");
        opts.report = Some(report_path.clone());
        let mut lines = Vec::new();
        let report = run_scheduler(&opts, &mut |l| lines.push(l.to_string())).expect("run");

        // One queue round = ONE entry run (members are not entry runs).
        assert_eq!(report.runs, 1, "{report:?}");
        assert_eq!(report.successes, 1, "{report:?}");
        assert_eq!(report.stopped_reason, "once");
        let state = state_of(&opts);
        let q = &state.queues["q1"];
        assert_eq!(q.rounds_completed, 1);
        assert_eq!(q.empty_rounds, 0);
        assert!(!q.finished);
        assert_eq!(q.member_results.len(), 2);
        assert_eq!(q.member_results[0].task, "成员1");
        assert_eq!(q.member_results[1].task, "成员2");
        assert_eq!(q.member_results[0].clicks, 2, "member clicks are recorded");
        assert_eq!(
            state.entries["qe"].quota_day.used, 1,
            "one successful round consumes one entry quota"
        );
        // Members ran serially in queue order.
        let calls = opts.test_exec.as_ref().unwrap().calls();
        assert_eq!(
            calls.iter().map(|c| c.task.as_str()).collect::<Vec<_>>(),
            vec!["成员1", "成员2"],
            "{calls:?}"
        );
        // Events are complete (`queue_round_started` ... `queue_round_finished`).
        let events = std::fs::read_to_string(opts.events_path()).unwrap();
        for event in [
            "queue_round_started",
            "queue_member_started",
            "queue_member_finished",
            "queue_round_finished",
            "run_finished",
        ] {
            assert!(
                events.contains(&format!("\"event\":\"{event}\"")),
                "missing {event}: {events}"
            );
        }
        assert!(events.contains("\"queue_id\":\"q1\""), "{events}");
        // Report carries the queue summary (P2: --report + status synced).
        assert_eq!(report.queues.len(), 1);
        assert_eq!(report.queues[0].queue_id, "q1");
        assert_eq!(report.queues[0].rounds_completed, 1);
        let last = report.queues[0].last_round.as_ref().expect("last round");
        assert_eq!(last.round, 1);
        assert_eq!(last.executed, 2);
        assert_eq!(last.recorded, 2);
        // ... and the same shape is serialized to `--report`.
        let raw: Value =
            serde_json::from_str(&std::fs::read_to_string(&report_path).unwrap()).unwrap();
        assert_eq!(raw["schema_version"], 4);
        assert_eq!(raw["queues"][0]["queue_id"], "q1");
        assert_eq!(raw["queues"][0]["rounds_completed"], 1);
        assert_eq!(raw["queues"][0]["last_round"]["round"], 1);
        assert_eq!(raw["queues"][0]["last_round"]["members"][1]["task"], "成员2");
        assert_eq!(raw["queues"][0]["last_round"]["members"][0]["clicks"], 2);
        assert!(lines.iter().any(|l| l.contains("scheduler_queue_round_done")));
        std::fs::remove_dir_all(&td).ok();
    }

    #[test]
    fn queue_member_failure_continues_by_default() {
        let td = tmpdir("q-continue");
        write_task(&td, "成员1", OK_FLOW, None);
        write_task(&td, "成员2", OK_FLOW, None);
        let config = write_config(
            &td,
            &queue_config_text(
                &["成员1", "成员2"],
                0,
                "continue",
                0,
                "",
                "",
            ),
        );
        let opts = queue_opts(
            &td,
            config,
            exec_two(ResultClass::Retryable, ResultClass::Success),
        );
        let report = run_scheduler(&opts, &mut |_| {}).expect("run");
        assert_eq!(report.runs, 1, "{report:?}");
        assert_eq!(report.successes, 1, "one member succeeded: {report:?}");
        assert_eq!(report.failures, 0);
        let state = state_of(&opts);
        let q = &state.queues["q1"];
        assert_eq!(q.rounds_completed, 1, "the round still counts (rule 5)");
        assert_eq!(q.member_results.len(), 2);
        assert_eq!(q.member_results[0].class, "retryable");
        assert_eq!(q.member_results[1].class, "success");
        assert_eq!(
            opts.test_exec.as_ref().unwrap().calls().len(),
            2,
            "continue policy runs the next member"
        );
        assert_eq!(state.entries["qe"].last_result, "success");
        std::fs::remove_dir_all(&td).ok();
    }

    #[test]
    fn queue_fail_fast_aborts_the_round_but_still_counts_it() {
        let td = tmpdir("q-failfast");
        write_task(&td, "成员1", OK_FLOW, None);
        write_task(&td, "成员2", OK_FLOW, None);
        let config = write_config(
            &td,
            &queue_config_text(&["成员1", "成员2"], 0, "fail_fast", 0, "", ""),
        );
        let opts = queue_opts(
            &td,
            config,
            exec_two(ResultClass::Retryable, ResultClass::Success),
        );
        let report = run_scheduler(&opts, &mut |_| {}).expect("run");
        assert_eq!(report.runs, 1, "{report:?}");
        assert_eq!(
            report.failures, 1,
            "executed but all executed members failed -> retryable: {report:?}"
        );
        let state = state_of(&opts);
        let q = &state.queues["q1"];
        assert_eq!(q.rounds_completed, 1, "fail_fast still counts the round (rule 7)");
        assert_eq!(q.member_results.len(), 1, "the second member was not run");
        assert_eq!(q.member_results[0].class, "retryable");
        assert_eq!(
            opts.test_exec.as_ref().unwrap().calls().len(),
            1,
            "fail_fast aborts the remaining members"
        );
        let events = std::fs::read_to_string(opts.events_path()).unwrap();
        assert!(
            events.contains("\"event\":\"queue_round_finished\"")
                && events.contains("\"aborted\":true"),
            "{events}"
        );
        assert!(
            state.entries["qe"].cooldown_until_ms > 0,
            "retryable puts the entry into cooldown"
        );
        std::fs::remove_dir_all(&td).ok();
    }

    #[test]
    fn queue_empty_round_does_not_consume_total_rounds() {
        let td = tmpdir("q-empty");
        write_task(&td, "成员1", OK_FLOW, None);
        write_task(&td, "成员2", OK_FLOW, None);
        let config = write_config(
            &td,
            &queue_config_text(&["成员1", "成员2"], 2, "continue", 0, "", ""),
        );
        // Both members fail the rule-4 precheck (simulated mid-run deletion).
        let exec = TestExec::new().skip_member("成员1").skip_member("成员2");
        let opts = queue_opts(&td, config, exec);
        let report = run_scheduler(&opts, &mut |_| {}).expect("run");
        assert_eq!(report.runs, 1, "{report:?}");
        assert_eq!(report.blocked, 1, "an empty round is blocked: {report:?}");
        let state = state_of(&opts);
        let q = &state.queues["q1"];
        assert_eq!(q.rounds_completed, 0, "lazy counting: no total_rounds burned");
        assert_eq!(q.empty_rounds, 1);
        assert!(!q.finished, "an empty round cannot finish the queue");
        assert_eq!(q.member_results.len(), 2, "skipped members are recorded");
        assert!(q.member_results.iter().all(|m| m.attempts == 0));
        assert!(
            opts.test_exec.as_ref().unwrap().calls().is_empty(),
            "skipped members are never executed"
        );
        let events = std::fs::read_to_string(opts.events_path()).unwrap();
        assert!(events.contains("\"event\":\"empty_round\""), "{events}");
        assert_eq!(state.entries["qe"].last_result, "blocked");
        std::fs::remove_dir_all(&td).ok();
    }

    #[test]
    fn queue_finishes_at_total_rounds_and_the_plan_skips_it() {
        let td = tmpdir("q-finish");
        write_task(&td, "成员1", OK_FLOW, None);
        let config = write_config(
            &td,
            &queue_config_text(&["成员1"], 1, "continue", 0, "", ""),
        );
        let mut opts = queue_opts(
            &td,
            config,
            TestExec::new().with("成员1", vec![step(ResultClass::Success)]),
        );
        opts.once = false;
        opts.max_passes = Some(2);
        let mut lines = Vec::new();
        let report = run_scheduler(&opts, &mut |l| lines.push(l.to_string())).expect("run");
        assert_eq!(report.passes, 2, "{report:?}");
        assert_eq!(report.runs, 1, "the second pass plans nothing");
        assert_eq!(report.stopped_reason, "max_rounds");
        let state = state_of(&opts);
        let q = &state.queues["q1"];
        assert_eq!(q.rounds_completed, 1);
        assert!(q.finished, "rule 6: total_rounds reached");
        assert_eq!(q.current_round, None, "the round was closed");
        assert!(
            lines.iter().any(|l| l.contains("scheduler_skip entry=qe")
                && l.contains("queue_finished")),
            "a finished queue is planned as skipped: {lines:?}"
        );
        assert!(report.queues[0].finished);
        std::fs::remove_dir_all(&td).ok();
    }

    #[test]
    fn queue_resumes_an_open_round_after_a_crash() {
        let td = tmpdir("q-resume");
        write_task(&td, "成员1", OK_FLOW, None);
        write_task(&td, "成员2", OK_FLOW, None);
        let config = write_config(
            &td,
            &queue_config_text(&["成员1", "成员2"], 2, "continue", 0, "", ""),
        );
        let opts = queue_opts(
            &td,
            config,
            exec_two(ResultClass::Success, ResultClass::Success),
        );
        // Crash residue: round 1 is open at member 1 (member 0 was recorded),
        // plus the dirty `running` marker of the member that was executing.
        let mut state = SchedulerState::new();
        state.queues.insert(
            "q1".to_string(),
            scheduler::QueueState {
                total_rounds: 2,
                rounds_completed: 0,
                empty_rounds: 0,
                finished: false,
                current_round: Some(scheduler::QueueRoundState {
                    index: 1,
                    member_index: 1,
                    started_ms: 1,
                }),
                pending_entry_record: None,
                member_results: vec![scheduler::QueueMemberResult {
                    round: 1,
                    member_index: 0,
                    task: "成员1".to_string(),
                    class: "success".to_string(),
                    exit: "done".to_string(),
                    outcome: "Done".to_string(),
                    clicks: 1,
                    attempts: 1,
                    duration_ms: 1,
                    finished_ms: 1,
                }],
            },
        );
        state.running = Some(scheduler::RunningEntry {
            entry_id: "qe".to_string(),
            task: "成员2".to_string(),
            pass: 1,
            attempt: 1,
            since_ms: 1,
            resume: false,
        });
        write_json_atomic(&opts.state_path(), &state).unwrap();

        let mut lines = Vec::new();
        let report = run_scheduler(&opts, &mut |l| lines.push(l.to_string())).expect("run");
        assert_eq!(report.runs, 1, "{report:?}");
        let after = state_of(&opts);
        let q = &after.queues["q1"];
        assert_eq!(q.current_round, None);
        assert_eq!(q.rounds_completed, 1, "the resumed round is counted exactly once");
        assert_eq!(q.member_results.len(), 2, "member 0 was not re-run");
        assert_eq!(q.member_results[1].task, "成员2");
        let calls = opts.test_exec.as_ref().unwrap().calls();
        assert_eq!(calls.len(), 1, "only the remaining member ran: {calls:?}");
        assert_eq!(calls[0].task, "成员2");
        assert!(after.running.is_none(), "the running marker is cleared");
        assert!(
            lines.iter().any(|l| l.contains("scheduler_queue_round_resume")),
            "{lines:?}"
        );
        let events = std::fs::read_to_string(opts.events_path()).unwrap();
        assert!(events.contains("\"event\":\"queue_round_resumed\""), "{events}");
        assert!(
            !events.contains("\"event\":\"queue_round_started\""),
            "a resumed round must not begin a new one: {events}"
        );
        std::fs::remove_dir_all(&td).ok();
    }

    #[test]
    fn queue_member_retries_one_plus_entry_retry_times() {
        let td = tmpdir("q-retry");
        write_task(&td, "成员1", OK_FLOW, None);
        let config = write_config(
            &td,
            &queue_config_text(&["成员1"], 0, "continue", 1, "", ""),
        );
        let opts = queue_opts(
            &td,
            config,
            TestExec::new().with(
                "成员1",
                vec![step(ResultClass::Retryable), step(ResultClass::Success)],
            ),
        );
        let report = run_scheduler(&opts, &mut |_| {}).expect("run");
        assert_eq!(report.runs, 1, "{report:?}");
        assert_eq!(report.successes, 1);
        let state = state_of(&opts);
        let q = &state.queues["q1"];
        assert_eq!(q.member_results.len(), 1);
        assert_eq!(q.member_results[0].attempts, 2, "1 + entry.retry = 2 attempts");
        assert_eq!(q.member_results[0].class, "success", "the retry succeeded");
        let calls = opts.test_exec.as_ref().unwrap().calls();
        assert_eq!(
            calls.iter().map(|c| c.attempt).collect::<Vec<_>>(),
            vec![1, 2],
            "{calls:?}"
        );
        std::fs::remove_dir_all(&td).ok();
    }

    #[test]
    fn queue_member_retry_resumes_from_the_checkpoint() {
        let td = tmpdir("q-resume-retry");
        write_task(&td, "成员1", OK_FLOW, None);
        let state_dir = td.join("成员1").join("state");
        std::fs::create_dir_all(&state_dir).unwrap();
        std::fs::write(state_dir.join("checkpoint.json"), "{}").unwrap();
        let config = write_config(
            &td,
            &queue_config_text(&["成员1"], 0, "continue", 1, "", ""),
        );
        let opts = queue_opts(
            &td,
            config,
            TestExec::new().with(
                "成员1",
                vec![step(ResultClass::Retryable), step(ResultClass::Success)],
            ),
        );
        run_scheduler(&opts, &mut |_| {}).expect("run");
        let calls = opts.test_exec.as_ref().unwrap().calls();
        assert_eq!(calls.len(), 2, "{calls:?}");
        assert!(!calls[0].resume, "the first attempt starts fresh");
        assert!(calls[1].resume, "the retry resumes from the checkpoint");
        std::fs::remove_dir_all(&td).ok();
    }

    #[test]
    fn queue_entry_quota_limits_rounds_per_day() {
        let td = tmpdir("q-quota");
        write_task(&td, "成员1", OK_FLOW, None);
        let config = write_config(
            &td,
            &queue_config_text(
                &["成员1"],
                0,
                "continue",
                0,
                r#","quota":{"per":"day","times":1}"#,
                "",
            ),
        );
        let mut opts = queue_opts(
            &td,
            config,
            TestExec::new().with("成员1", vec![step(ResultClass::Success)]),
        );
        opts.once = false;
        opts.max_passes = Some(2);
        let mut lines = Vec::new();
        let report = run_scheduler(&opts, &mut |l| lines.push(l.to_string())).expect("run");
        assert_eq!(report.passes, 2, "{report:?}");
        assert_eq!(report.runs, 1, "quota blocks the second round: {report:?}");
        let state = state_of(&opts);
        assert_eq!(state.queues["q1"].rounds_completed, 1);
        assert_eq!(state.entries["qe"].quota_day.used, 1);
        assert!(
            lines.iter().any(|l| l.contains("scheduler_skip entry=qe")
                && l.contains("quota_exhausted")),
            "{lines:?}"
        );
        std::fs::remove_dir_all(&td).ok();
    }

    #[test]
    fn stop_mode_runs_one_queue_round_then_stops() {
        let td = tmpdir("q-stop");
        write_task(&td, "成员1", OK_FLOW, None);
        write_task(&td, "成员2", OK_FLOW, None);
        let mut text = queue_config_text(&["成员1", "成员2"], 0, "continue", 0, "", "");
        text = text.replace(r#""mode":"loop""#, r#""mode":"stop""#);
        let config = write_config(&td, &text);
        let mut opts = queue_opts(
            &td,
            config,
            exec_two(ResultClass::Success, ResultClass::Success),
        );
        opts.once = false;
        let report = run_scheduler(&opts, &mut |_| {}).expect("run");
        assert_eq!(report.stopped_reason, "stop", "{report:?}");
        assert_eq!(report.passes, 1);
        assert_eq!(report.runs, 1);
        let state = state_of(&opts);
        assert_eq!(state.queues["q1"].rounds_completed, 1);
        assert_eq!(state.last_stop, "stop");
        std::fs::remove_dir_all(&td).ok();
    }

    #[test]
    fn queue_rounds_are_counted_lazily_across_passes() {
        let td = tmpdir("q-lazy");
        write_task(&td, "成员1", OK_FLOW, None);
        let config = write_config(
            &td,
            &queue_config_text(&["成员1"], 2, "continue", 0, "", ""),
        );
        let mut opts = queue_opts(
            &td,
            config,
            TestExec::new().with("成员1", vec![step(ResultClass::Success)]),
        );
        opts.once = false;
        opts.max_passes = Some(3);
        let report = run_scheduler(&opts, &mut |_| {}).expect("run");
        assert_eq!(report.passes, 3, "{report:?}");
        assert_eq!(report.runs, 2, "exactly total_rounds rounds ran: {report:?}");
        assert_eq!(report.stopped_reason, "max_rounds");
        let state = state_of(&opts);
        let q = &state.queues["q1"];
        assert_eq!(q.rounds_completed, 2);
        assert_eq!(q.empty_rounds, 0);
        assert!(q.finished);
        assert_eq!(q.member_results.len(), 2);
        std::fs::remove_dir_all(&td).ok();
    }

    #[test]
    fn queue_fatal_member_stops_the_daemon() {
        let td = tmpdir("q-fatal");
        write_task(&td, "成员1", OK_FLOW, None);
        write_task(&td, "成员2", OK_FLOW, None);
        let config = write_config(
            &td,
            &queue_config_text(&["成员1", "成员2"], 0, "continue", 0, "", ""),
        );
        let opts = queue_opts(
            &td,
            config,
            exec_two(ResultClass::Fatal, ResultClass::Success),
        );
        let mut lines = Vec::new();
        let report = run_scheduler(&opts, &mut |l| lines.push(l.to_string())).expect("run");
        assert_eq!(report.stopped_reason, "fatal", "{report:?}");
        assert_eq!(report.fatals, 1);
        assert_eq!(
            opts.test_exec.as_ref().unwrap().calls().len(),
            1,
            "a fatal member always aborts the round (fail_fast semantics)"
        );
        let state = state_of(&opts);
        assert_eq!(state.queues["q1"].rounds_completed, 1, "the executed round counts");
        assert_eq!(state.entries["qe"].last_result, "fatal");
        assert!(
            lines.iter().any(|l| l.contains("scheduler_queue_round_abort")
                && l.contains("reason=fatal")),
            "{lines:?}"
        );
        std::fs::remove_dir_all(&td).ok();
    }

    #[test]
    fn reset_memory_reopens_a_finished_queue() {
        let td = tmpdir("q-reset-reopen");
        write_task(&td, "成员1", OK_FLOW, None);
        let config = write_config(
            &td,
            &queue_config_text(&["成员1"], 1, "continue", 0, "", ""),
        );
        let opts = queue_opts(
            &td,
            config.clone(),
            TestExec::new().with("成员1", vec![step(ResultClass::Success)]),
        );
        run_scheduler(&opts, &mut |_| {}).expect("first run");
        assert!(state_of(&opts).queues["q1"].finished);

        let reset = ResetOptions {
            config,
            tasks_dir: td.clone(),
            tasks: vec!["成员1".to_string()],
            include_checkpoint: true,
            ..ResetOptions::default()
        };
        reset_memory(&reset, &mut |_| {}).expect("reset");
        assert!(!state_of(&opts).queues.contains_key("q1"));

        let report = run_scheduler(&opts, &mut |_| {}).expect("second run");
        assert_eq!(report.runs, 1, "reset-memory makes the queue runnable again");
        let q = &state_of(&opts).queues["q1"];
        assert_eq!(q.rounds_completed, 1);
        assert!(q.finished);
        std::fs::remove_dir_all(&td).ok();
    }

    #[test]
    fn queue_fatal_option_keeps_going_with_keep_going() {
        // `--keep-going` must keep the session alive after a fatal member; the
        // round itself is still aborted (fail_fast semantics).
        let td = tmpdir("q-fatal-keep");
        write_task(&td, "成员1", OK_FLOW, None);
        write_task(&td, "成员2", OK_FLOW, None);
        let config = write_config(
            &td,
            &queue_config_text(&["成员1", "成员2"], 0, "continue", 0, "", ""),
        );
        let mut opts = queue_opts(
            &td,
            config,
            exec_two(ResultClass::Fatal, ResultClass::Success),
        );
        opts.keep_going = true;
        let report = run_scheduler(&opts, &mut |_| {}).expect("run");
        assert_ne!(report.stopped_reason, "fatal", "{report:?}");
        assert_eq!(report.fatals, 1);
        assert_eq!(opts.test_exec.as_ref().unwrap().calls().len(), 1);
        std::fs::remove_dir_all(&td).ok();
    }

    #[test]
    fn reset_memory_resets_queue_state_and_audits_it() {
        let td = tmpdir("reset-queue");
        let config = write_config(
            &td,
            r#"{"schema_version":4,
                "queues":[{"queue_id":"q1","members":["任务1"],"total_rounds":1}],
                "entries":[{"entry_id":"qe","task":"q1","kind":"queue","lifecycle":"temp"}]}"#,
        );
        let opts = base_opts(&td, config.clone(), None);
        let mut state = SchedulerState::new();
        state.queues.insert(
            "q1".to_string(),
            scheduler::QueueState {
                total_rounds: 1,
                rounds_completed: 1,
                empty_rounds: 0,
                finished: true,
                current_round: None,
                pending_entry_record: None,
                member_results: vec![scheduler::QueueMemberResult {
                    round: 1,
                    member_index: 0,
                    task: "任务1".to_string(),
                    class: "success".to_string(),
                    exit: "done".to_string(),
                    outcome: "Done".to_string(),
                    clicks: 0,
                    attempts: 1,
                    duration_ms: 1,
                    finished_ms: 1,
                }],
            },
        );
        // P2-style residue: the queue entry has its own EntryState (its `task`
        // is the queue_id) and a dirty running marker pointing at it.
        state.entries.insert(
            "qe".to_string(),
            scheduler::EntryState {
                task: "q1".to_string(),
                runs: 1,
                ..scheduler::EntryState::default()
            },
        );
        state.running = Some(scheduler::RunningEntry {
            entry_id: "qe".to_string(),
            task: "q1".to_string(),
            pass: 1,
            attempt: 1,
            since_ms: 1,
            resume: false,
        });
        write_json_atomic(&opts.state_path(), &state).unwrap();

        let reset = ResetOptions {
            config,
            tasks_dir: td.clone(),
            tasks: vec!["任务1".to_string()],
            include_checkpoint: true,
            ..ResetOptions::default()
        };
        let mut lines = Vec::new();
        let report = reset_memory(&reset, &mut |l| lines.push(l.to_string())).expect("reset");
        assert_eq!(report.queues, vec!["q1".to_string()]);
        assert_eq!(
            report.entries,
            vec!["qe".to_string()],
            "the queue entry's own EntryState is memory too"
        );
        let after = state_of(&opts);
        assert!(
            !after.queues.contains_key("q1"),
            "the whole queue state entry is removed"
        );
        assert!(!after.entries.contains_key("qe"), "queue entry state gone");
        assert!(after.running.is_none(), "dirty running marker cleared");
        assert!(
            lines.iter().any(|l| l.contains("reset_memory_queues=q1")),
            "{lines:?}"
        );
        assert!(
            lines.iter().any(|l| l.contains("reset_memory_cleared_dirty_running=qe")),
            "{lines:?}"
        );
        let events = std::fs::read_to_string(opts.events_path()).unwrap();
        assert!(events.contains("\"queues\":[\"q1\"]"), "{events}");
        assert!(events.contains("\"queues_reset\":[\"q1\"]"), "{events}");
        std::fs::remove_dir_all(&td).ok();
    }

    #[test]
    fn reset_memory_backs_up_a_v3_state_before_writing_v4() {
        let td = tmpdir("reset-v3-backup");
        let config = write_config(
            &td,
            r#"{"schema_version":3,"entries":[{"entry_id":"e1","task":"任务1"}]}"#,
        );
        let opts = base_opts(&td, config.clone(), None);
        let state_path = opts.state_path();
        let v3 = r#"{"schema_version":3,"passes":3,
                     "entries":{"e1":{"task":"任务1","runs":1}}}"#;
        std::fs::write(&state_path, v3).unwrap();
        let reset = ResetOptions {
            config,
            tasks_dir: td.clone(),
            tasks: vec!["任务1".to_string()],
            include_checkpoint: true,
            ..ResetOptions::default()
        };
        let mut lines = Vec::new();
        reset_memory(&reset, &mut |l| lines.push(l.to_string())).expect("reset");
        let backup = state_v3_backup_path(&state_path);
        assert_eq!(
            std::fs::read_to_string(&backup).unwrap(),
            v3,
            "the backup is the exact pre-migration v3 file"
        );
        assert!(
            lines
                .iter()
                .any(|l| l.contains("scheduler_state_backup path=") && l.contains("v3.bak")),
            "{lines:?}"
        );
        let migrated: Value =
            serde_json::from_str(&std::fs::read_to_string(&state_path).unwrap()).unwrap();
        assert_eq!(migrated["schema_version"], 4);
        std::fs::remove_dir_all(&td).ok();
    }

    // -- P1/P2 review fixes (resume classification, exactly-once, gates) -----

    #[test]
    fn queue_resume_after_success_plus_failure_classifies_success() {
        // P1-1: member 0 succeeded before the crash; the only member this
        // process runs fails. The whole round is still a success (quota is
        // consumed, no retryable/cooldown).
        let td = tmpdir("q-resume-class");
        write_task(&td, "成员1", OK_FLOW, None);
        write_task(&td, "成员2", OK_FLOW, None);
        let config = write_config(
            &td,
            &queue_config_text(
                &["成员1", "成员2"],
                0,
                "continue",
                0,
                r#","quota":{"per":"day","times":2}"#,
                "",
            ),
        );
        let opts = queue_opts(
            &td,
            config,
            TestExec::new().with("成员2", vec![step(ResultClass::Retryable)]),
        );
        let mut state = SchedulerState::new();
        state.queues.insert(
            "q1".to_string(),
            scheduler::QueueState {
                current_round: Some(scheduler::QueueRoundState {
                    index: 1,
                    member_index: 1,
                    started_ms: 1,
                }),
                member_results: vec![scheduler::QueueMemberResult {
                    round: 1,
                    member_index: 0,
                    task: "成员1".to_string(),
                    class: "success".to_string(),
                    exit: "done".to_string(),
                    outcome: "Done".to_string(),
                    clicks: 2,
                    attempts: 1,
                    duration_ms: 1,
                    finished_ms: 1,
                }],
                ..scheduler::QueueState::default()
            },
        );
        state.running = Some(scheduler::RunningEntry {
            entry_id: "qe".to_string(),
            task: "成员2".to_string(),
            pass: 1,
            attempt: 1,
            since_ms: 1,
            resume: false,
        });
        write_json_atomic(&opts.state_path(), &state).unwrap();

        let mut lines = Vec::new();
        let report = run_scheduler(&opts, &mut |l| lines.push(l.to_string())).expect("run");
        assert_eq!(report.runs, 1, "{report:?}");
        assert_eq!(report.successes, 1, "{report:?}");
        assert_eq!(report.failures, 0, "the round is not retryable: {report:?}");
        let after = state_of(&opts);
        let q = &after.queues["q1"];
        assert_eq!(q.current_round, None);
        assert_eq!(q.rounds_completed, 1, "the resumed round counts once");
        assert_eq!(q.member_results.len(), 2);
        assert_eq!(q.member_results[1].class, "retryable");
        let e = &after.entries["qe"];
        assert_eq!(
            e.last_result, "success",
            "classification must consider the persisted member 0"
        );
        assert_eq!(e.quota_day.used, 1, "a successful round consumes quota");
        assert_eq!(e.cooldown_until_ms, 0, "success sets no cooldown");
        assert_eq!(
            opts.test_exec.as_ref().unwrap().calls().len(),
            1,
            "only the remaining member runs"
        );
        let events = std::fs::read_to_string(opts.events_path()).unwrap();
        assert!(
            events.contains("\"event\":\"queue_round_finished\"")
                && events.contains("\"executed_members\":2")
                && events.contains("\"executed\":true"),
            "the resumed round reports both members: {events}"
        );
        assert!(
            events.contains("\"event\":\"run_finished\"")
                && events.contains("\"attempts\":2")
                && events.contains("\"clicks\":2"),
            "run_finished aggregates the whole round: {events}"
        );
        std::fs::remove_dir_all(&td).ok();
    }

    #[test]
    fn queue_resume_after_a_recorded_fatal_stays_fatal() {
        // P1-1: a fatal member was recorded before the crash; the remaining
        // member must not run and the round must not become a success.
        let td = tmpdir("q-resume-fatal");
        write_task(&td, "成员1", OK_FLOW, None);
        write_task(&td, "成员2", OK_FLOW, None);
        let config = write_config(
            &td,
            &queue_config_text(&["成员1", "成员2"], 0, "continue", 0, "", ""),
        );
        let opts = queue_opts(
            &td,
            config,
            TestExec::new().with("成员2", vec![step(ResultClass::Success)]),
        );
        let mut state = SchedulerState::new();
        state.queues.insert(
            "q1".to_string(),
            scheduler::QueueState {
                current_round: Some(scheduler::QueueRoundState {
                    index: 1,
                    member_index: 1,
                    started_ms: 1,
                }),
                member_results: vec![scheduler::QueueMemberResult {
                    round: 1,
                    member_index: 0,
                    task: "成员1".to_string(),
                    class: "fatal".to_string(),
                    exit: "exhausted".to_string(),
                    outcome: "Fatal".to_string(),
                    clicks: 0,
                    attempts: 1,
                    duration_ms: 1,
                    finished_ms: 1,
                }],
                ..scheduler::QueueState::default()
            },
        );
        write_json_atomic(&opts.state_path(), &state).unwrap();

        let mut lines = Vec::new();
        let report = run_scheduler(&opts, &mut |l| lines.push(l.to_string())).expect("run");
        assert!(
            opts.test_exec.as_ref().unwrap().calls().is_empty(),
            "remaining members must not run after a recorded fatal"
        );
        assert_eq!(report.fatals, 1, "{report:?}");
        assert_eq!(report.stopped_reason, "fatal");
        let after = state_of(&opts);
        let q = &after.queues["q1"];
        assert_eq!(q.current_round, None);
        assert_eq!(q.rounds_completed, 1, "the executed round still counts once");
        assert_eq!(q.member_results.len(), 1, "the second member never ran");
        assert_eq!(after.entries["qe"].last_result, "fatal");
        assert_eq!(
            after.entries["qe"].quota_day.used, 0,
            "fatal consumes no quota"
        );
        assert!(
            lines.iter().any(|l| l.contains("scheduler_queue_round_abort")
                && l.contains("reason=fatal")
                && l.contains("resumed=true")),
            "{lines:?}"
        );
        let events = std::fs::read_to_string(opts.events_path()).unwrap();
        assert!(events.contains("\"aborted\":true"), "{events}");
        std::fs::remove_dir_all(&td).ok();
    }

    #[test]
    fn queue_resume_after_a_recorded_failure_aborts_the_fail_fast_round() {
        // P1-1: the `aborted` flag is rebuilt from the persisted round too; a
        // fail_fast round does not run the remaining members after a restart.
        let td = tmpdir("q-resume-failfast");
        write_task(&td, "成员1", OK_FLOW, None);
        write_task(&td, "成员2", OK_FLOW, None);
        let config = write_config(
            &td,
            &queue_config_text(&["成员1", "成员2"], 0, "fail_fast", 0, "", ""),
        );
        let opts = queue_opts(
            &td,
            config,
            TestExec::new().with("成员2", vec![step(ResultClass::Success)]),
        );
        let mut state = SchedulerState::new();
        state.queues.insert(
            "q1".to_string(),
            scheduler::QueueState {
                current_round: Some(scheduler::QueueRoundState {
                    index: 1,
                    member_index: 1,
                    started_ms: 1,
                }),
                member_results: vec![scheduler::QueueMemberResult {
                    round: 1,
                    member_index: 0,
                    task: "成员1".to_string(),
                    class: "retryable".to_string(),
                    exit: "failed".to_string(),
                    outcome: "Failed".to_string(),
                    clicks: 0,
                    attempts: 1,
                    duration_ms: 1,
                    finished_ms: 1,
                }],
                ..scheduler::QueueState::default()
            },
        );
        write_json_atomic(&opts.state_path(), &state).unwrap();

        let report = run_scheduler(&opts, &mut |_| {}).expect("run");
        assert!(
            opts.test_exec.as_ref().unwrap().calls().is_empty(),
            "fail_fast abort survives the restart"
        );
        assert_eq!(report.failures, 1, "{report:?}");
        let after = state_of(&opts);
        let q = &after.queues["q1"];
        assert_eq!(q.rounds_completed, 1);
        assert_eq!(q.member_results.len(), 1);
        assert_eq!(after.entries["qe"].last_result, "retryable");
        assert!(
            after.entries["qe"].cooldown_until_ms > 0,
            "retryable puts the entry into cooldown"
        );
        std::fs::remove_dir_all(&td).ok();
    }

    #[test]
    fn pending_entry_record_is_recovered_exactly_once_at_startup() {
        // P1-2: the round was closed (the queue is already finished) but the
        // entry-level result was never applied. The next start must apply it
        // once — plan_pass can never trigger a finished queue again.
        let td = tmpdir("q-pending-record");
        write_task(&td, "成员1", OK_FLOW, None);
        let config = write_config(
            &td,
            &queue_config_text(
                &["成员1"],
                1,
                "continue",
                0,
                r#","quota":{"per":"day","times":2}"#,
                "",
            ),
        );
        let opts = queue_opts(&td, config, TestExec::new());
        let finished_ms = now_ms();
        let mut state = SchedulerState::new();
        state.queues.insert(
            "q1".to_string(),
            scheduler::QueueState {
                total_rounds: 1,
                rounds_completed: 1,
                empty_rounds: 0,
                finished: true,
                current_round: None,
                member_results: vec![scheduler::QueueMemberResult {
                    round: 1,
                    member_index: 0,
                    task: "成员1".to_string(),
                    class: "success".to_string(),
                    exit: "done".to_string(),
                    outcome: "Done".to_string(),
                    clicks: 2,
                    attempts: 1,
                    duration_ms: 7,
                    finished_ms,
                }],
                pending_entry_record: Some(scheduler::PendingEntryRecord {
                    entry_id: "qe".to_string(),
                    round: 1,
                    // Deliberately wrong staged class: recovery must recompute
                    // it from `member_results` (which say success).
                    class: "retryable".to_string(),
                    exit: "queue_failed".to_string(),
                    outcome: "Failed".to_string(),
                    attempts: 1,
                    duration_ms: 7,
                    finished_ms,
                }),
            },
        );
        state.running = Some(scheduler::RunningEntry {
            entry_id: "qe".to_string(),
            task: "成员1".to_string(),
            pass: 1,
            attempt: 1,
            since_ms: 1,
            resume: false,
        });
        write_json_atomic(&opts.state_path(), &state).unwrap();

        let mut lines = Vec::new();
        let report = run_scheduler(&opts, &mut |l| lines.push(l.to_string())).expect("run");
        assert_eq!(report.runs, 0, "nothing executed in this session");
        assert!(opts.test_exec.as_ref().unwrap().calls().is_empty());
        let after = state_of(&opts);
        let e = &after.entries["qe"];
        assert_eq!(e.runs, 1, "the staged result is applied exactly once");
        assert_eq!(e.successes, 1);
        assert_eq!(
            e.last_result, "success",
            "the class is recomputed from member_results, not taken from the stage"
        );
        assert_eq!(e.last_exit, "queue_done");
        assert_eq!(e.quota_day.used, 1, "the recovered round consumes its quota");
        assert_eq!(e.cooldown_until_ms, 0, "a success sets no cooldown");
        assert!(
            after.queues["q1"].pending_entry_record.is_none(),
            "the stage is cleared in the same write"
        );
        assert!(after.running.is_none(), "the stale running marker is cleared");
        assert_eq!(
            after.queues["q1"].rounds_completed, 1,
            "the round was already counted and is not re-run"
        );
        let raw: Value =
            serde_json::from_str(&std::fs::read_to_string(opts.state_path()).unwrap()).unwrap();
        assert!(
            raw["queues"]["q1"].get("pending_entry_record").is_none(),
            "the cleared stage is not serialized: {raw}"
        );
        assert!(
            lines.iter().any(|l| l
                .contains("scheduler_queue_entry_record_recovered")
                && l.contains("entry=qe")
                && l.contains("class=success")
                && l.contains("quota_used=1")),
            "{lines:?}"
        );
        let events = std::fs::read_to_string(opts.events_path()).unwrap();
        assert!(
            events.contains("\"event\":\"queue_entry_record_recovered\"")
                && events.contains("\"applied\":true")
                && events.contains("\"staged_class\":\"retryable\""),
            "the recomputed class and the staged value are both audited: {events}"
        );

        // A second start must not record it again.
        let report2 = run_scheduler(&opts, &mut |_| {}).expect("second run");
        assert_eq!(report2.runs, 0);
        let after2 = state_of(&opts);
        assert_eq!(after2.entries["qe"].runs, 1, "exactly once, not twice");
        assert_eq!(after2.entries["qe"].quota_day.used, 1);
        std::fs::remove_dir_all(&td).ok();
    }

    #[test]
    fn queue_between_gate_delays_the_next_round() {
        // P2-1: the inter-round pause is enforced by plan_pass, so status
        // (`runnable_now`) and `next_due_ms` stay consistent.
        let td = tmpdir("q-between");
        write_task(&td, "成员1", OK_FLOW, None);
        let config = write_config(
            &td,
            r#"{"schema_version":4,
                "config":{"mode":"loop","loop_interval_s":0},
                "queues":[{"queue_id":"q1","members":["成员1"],"total_rounds":0,
                           "on_failure":"continue","between_ms":600000}],
                "entries":[{"entry_id":"qe","task":"q1","kind":"queue","retry":0}]}"#,
        );
        let opts = queue_opts(
            &td,
            config,
            TestExec::new().with("成员1", vec![step(ResultClass::Success)]),
        );
        let now = now_ms();
        let mut state = SchedulerState::new();
        state.queues.insert(
            "q1".to_string(),
            scheduler::QueueState {
                rounds_completed: 1,
                member_results: vec![scheduler::QueueMemberResult {
                    round: 1,
                    member_index: 0,
                    task: "成员1".to_string(),
                    class: "success".to_string(),
                    exit: "done".to_string(),
                    outcome: "Done".to_string(),
                    clicks: 1,
                    attempts: 1,
                    duration_ms: 5,
                    finished_ms: now,
                }],
                ..scheduler::QueueState::default()
            },
        );
        write_json_atomic(&opts.state_path(), &state).unwrap();

        let mut lines = Vec::new();
        let r1 = run_scheduler(&opts, &mut |l| lines.push(l.to_string())).expect("run 1");
        assert_eq!(r1.runs, 0, "the between_ms gate blocks the next round: {r1:?}");
        assert!(opts.test_exec.as_ref().unwrap().calls().is_empty());
        assert!(
            lines.iter().any(|l| l.contains("scheduler_skip entry=qe")
                && l.contains("reason=queue_between")),
            "{lines:?}"
        );
        assert_eq!(state_of(&opts).queues["q1"].rounds_completed, 1);

        let v = status_json(&opts).expect("status");
        assert_eq!(v["entries"][0]["runnable_now"], false, "{v}");
        assert_eq!(v["entries"][0]["blocked_reason"], "queue_between");
        let due = v["entries"][0]["next_due_ms"].as_i64().expect("next due");
        assert!(
            due > v["generated_ms"].as_i64().unwrap(),
            "the pause is in the future: {v}"
        );
        assert_eq!(
            v["queues"][0]["next_due_ms"].as_i64(),
            Some(due),
            "entry and queue report the same due instant"
        );

        // Time advanced past the pause: the following pass runs the round.
        let mut state = state_of(&opts);
        state.queues.get_mut("q1").unwrap().member_results[0].finished_ms =
            now.saturating_sub(600_100);
        write_json_atomic(&opts.state_path(), &state).unwrap();
        let r2 = run_scheduler(&opts, &mut |_| {}).expect("run 2");
        assert_eq!(r2.runs, 1, "{r2:?}");
        assert_eq!(opts.test_exec.as_ref().unwrap().calls().len(), 1);
        let after = state_of(&opts);
        assert_eq!(after.queues["q1"].rounds_completed, 2);
        assert_eq!(after.queues["q1"].member_results.len(), 2);
        std::fs::remove_dir_all(&td).ok();
    }

    #[test]
    fn queue_member_execution_marks_running_with_the_member_task() {
        // B5 test gap: `state.running.task` must name the member that is about
        // to run (not the queue_id) and must be cleared afterwards.
        let td = tmpdir("q-mark-running");
        write_task(&td, "成员1", OK_FLOW, None);
        write_task(&td, "成员2", OK_FLOW, None);
        let config = write_config(
            &td,
            &queue_config_text(&["成员1", "成员2"], 0, "continue", 0, "", ""),
        );
        let opts = queue_opts(
            &td,
            config,
            exec_two(ResultClass::Success, ResultClass::Success),
        );
        run_scheduler(&opts, &mut |_| {}).expect("run");
        let observed = opts.test_exec.as_ref().unwrap().observed_running();
        assert_eq!(observed.len(), 2, "one marker per attempt");
        let first = observed[0].as_ref().expect("running marker before member 1");
        assert_eq!(
            first.task, "成员1",
            "the marker names the member, not the queue_id"
        );
        assert_eq!(first.entry_id, "qe");
        assert_eq!(first.pass, 1);
        assert_eq!(first.attempt, 1);
        assert!(!first.resume);
        let second = observed[1].as_ref().expect("running marker before member 2");
        assert_eq!(second.task, "成员2");
        assert_eq!(second.attempt, 1);
        assert!(
            state_of(&opts).running.is_none(),
            "the marker is cleared after the round"
        );
        std::fs::remove_dir_all(&td).ok();
    }

    #[test]
    fn queue_member_missing_at_runtime_is_recorded_and_skipped() {
        // B5 test gap: the production rule-4 precheck (`member_skip_reason`
        // -> `task_missing`), exercised without the startup fail-fast and
        // without the `skip_member` test injection.
        let td = tmpdir("q-member-missing");
        write_task(&td, "成员1", OK_FLOW, None);
        write_task(&td, "成员2", OK_FLOW, None);
        let config = write_config(
            &td,
            &queue_config_text(&["成员1", "成员2"], 0, "continue", 0, "", ""),
        );
        let opts = queue_opts(
            &td,
            config,
            TestExec::new().with("成员1", vec![step(ResultClass::Success)]),
        );
        // Load while both bundles exist (startup fail-fast is happy), then
        // delete the second member: only the runtime precheck can see it.
        let loaded = load_full(&opts).expect("load");
        std::fs::remove_dir_all(td.join("成员2")).expect("remove member 2");
        let state_path = opts.state_path();
        let events_path = opts.events_path();
        let mut session = Session {
            opts: &opts,
            loaded,
            state: SchedulerState::new(),
            state_path,
            events_path,
            report: SchedulerReport::default(),
            recovery: None,
            seq: 0,
            run_seq: 0,
            last_run: None,
            last_run_error: None,
        };
        let entry = session.sched().entry("qe").cloned().expect("queue entry");
        let planned = scheduler::PlannedRun {
            entry_index: 0,
            entry_id: "qe".to_string(),
            task: "q1".to_string(),
            lifecycle: Lifecycle::Periodic,
            resume: false,
            reason: "periodic".to_string(),
        };
        let mut lines = Vec::new();
        let class = session
            .execute_queue_round(&entry, &planned, 1, &mut |l| lines.push(l.to_string()))
            .expect("round");
        assert_eq!(class, ResultClass::Success, "member 1 still succeeded");
        let q = &session.state.queues["q1"];
        assert_eq!(q.member_results.len(), 2);
        let skipped = &q.member_results[1];
        assert_eq!(skipped.task, "成员2");
        assert_eq!(skipped.class, "blocked");
        assert_eq!(skipped.exit, "skipped");
        assert_eq!(skipped.outcome, "task_missing");
        assert_eq!(skipped.attempts, 0, "the missing member never executed");
        assert_eq!(q.rounds_completed, 1, "member 1 executed, so the round counts");
        assert_eq!(q.current_round, None);
        assert!(q.pending_entry_record.is_none());
        assert_eq!(
            opts.test_exec.as_ref().unwrap().calls().len(),
            1,
            "only the existing member ran"
        );
        assert!(
            lines.iter().any(|l| l.contains("scheduler_queue_member_skip")
                && l.contains("reason=task_missing")),
            "{lines:?}"
        );
        let events = std::fs::read_to_string(&opts.events_path()).unwrap();
        assert!(
            events.contains("\"event\":\"queue_member_finished\"")
                && events.contains("\"skipped\":true"),
            "{events}"
        );
        std::fs::remove_dir_all(&td).ok();
    }

    // -- P4: first-class Run entities (`runs/scheduler/<run_id>.json`) --------

    /// Every run entity file under `<runs_dir>/scheduler`, parsed and sorted by
    /// `started_ms` (then `run_id`) so assertions are independent of readdir
    /// order.
    fn run_entities(opts: &SchedulerOptions) -> Vec<serde_json::Value> {
        let dir = opts.runs_dir.join(SCHEDULER_RUN_DIR);
        let mut runs: Vec<serde_json::Value> = std::fs::read_dir(&dir)
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .filter(|e| e.file_name().to_string_lossy().ends_with(".json"))
                    .map(|e| {
                        serde_json::from_str(
                            &std::fs::read_to_string(e.path()).expect("read run entity"),
                        )
                        .expect("run entity JSON")
                    })
                    .collect()
            })
            .unwrap_or_default();
        runs.sort_by(|a, b| {
            a["started_ms"]
                .as_u64()
                .cmp(&b["started_ms"].as_u64())
                .then_with(|| a["run_id"].as_str().cmp(&b["run_id"].as_str()))
        });
        runs
    }

    /// Raw file names under `<runs_dir>/scheduler` (atomic-write residue check).
    fn run_file_names(opts: &SchedulerOptions) -> Vec<String> {
        let dir = opts.runs_dir.join(SCHEDULER_RUN_DIR);
        let mut names: Vec<String> = std::fs::read_dir(&dir)
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    }

    /// One `kind=task` entry for `任务1`.
    fn task_entry_config(retry: u32, lifecycle: &str) -> String {
        format!(
            r#"{{"schema_version":4,"config":{{"mode":"stop"}},
                "entries":[{{"entry_id":"e1","task":"任务1","lifecycle":"{lifecycle}",
                            "retry":{retry}}}]}}"#
        )
    }

    #[test]
    fn task_entry_writes_one_run_entity_per_attempt() {
        let td = tmpdir("p4-task-retry");
        write_task(&td, "任务1", OK_FLOW, None);
        let config = write_config(&td, &task_entry_config(1, "periodic"));
        let opts = queue_opts(
            &td,
            config,
            TestExec::new().with(
                "任务1",
                vec![
                    step(ResultClass::Retryable),
                    step(ResultClass::Success).with_clicks(3),
                ],
            ),
        );
        let report = run_scheduler(&opts, &mut |_l| {}).expect("scheduler");
        assert_eq!(report.runs, 1, "{report:?}");
        assert_eq!(report.successes, 1, "the final attempt succeeded: {report:?}");

        let runs = run_entities(&opts);
        assert_eq!(runs.len(), 2, "one run entity per attempt: {runs:?}");
        // Attempt 1: retryable failure, exit code 1, no clicks.
        assert_eq!(runs[0]["schema_version"], 1);
        assert_eq!(runs[0]["source"], "scheduler");
        assert_eq!(runs[0]["kind"], "task");
        assert_eq!(runs[0]["entry_id"], "e1");
        assert_eq!(runs[0]["task"], "任务1");
        assert_eq!(runs[0]["attempt"], 1);
        assert_eq!(runs[0]["resume"], false);
        assert_eq!(runs[0]["class"], "retryable");
        assert_eq!(runs[0]["exit"], "failed");
        assert_eq!(runs[0]["exit_code"], 1);
        assert_eq!(runs[0]["outcome"], "Failed");
        assert_eq!(runs[0]["clicks"], 0);
        assert!(
            runs[0].get("queue_id").is_none()
                && runs[0].get("round").is_none()
                && runs[0].get("member_index").is_none(),
            "task entries carry no queue fields: {runs:?}"
        );
        assert!(runs[0].get("clicks_file").is_none());
        // Attempt 2: success, exit code 0 and the scripted click count.
        assert_eq!(runs[1]["attempt"], 2);
        assert_eq!(runs[1]["class"], "success");
        assert_eq!(runs[1]["exit"], "done");
        assert_eq!(runs[1]["exit_code"], 0);
        assert_eq!(runs[1]["outcome"], "Done");
        assert_eq!(runs[1]["clicks"], 3);
        let started = runs[1]["started_ms"].as_u64().expect("started_ms");
        let finished = runs[1]["finished_ms"].as_u64().expect("finished_ms");
        assert!(finished >= started, "{runs:?}");
        assert_eq!(
            runs[1]["duration_ms"].as_u64(),
            Some(finished - started),
            "duration_ms must be finished-started"
        );
        // Unique ids and no atomic-write temp residue.
        assert_ne!(runs[0]["run_id"], runs[1]["run_id"]);
        let names = run_file_names(&opts);
        assert_eq!(names.len(), 2, "only the two run entities: {names:?}");
        assert!(
            names.iter().all(|n| n.ends_with(".json") && !n.contains(".tmp")),
            "{names:?}"
        );
        for run in &runs {
            let id = run["run_id"].as_str().expect("run_id");
            assert!(
                id.chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')),
                "filename-safe run_id: {id}"
            );
            assert!(names.contains(&format!("{id}.json")), "{id} / {names:?}");
        }
        std::fs::remove_dir_all(&td).ok();
    }

    #[test]
    fn task_entry_run_entity_captures_error_text_and_exit_code() {
        let td = tmpdir("p4-task-error");
        write_task(&td, "任务1", OK_FLOW, None);
        let config = write_config(&td, &task_entry_config(0, "periodic"));
        let mut scripted = TestExecResult::new(ResultClass::Fatal, "run_error", "error");
        scripted.error = Some("task bundle vanished".to_string());
        scripted.exit_code = 1;
        let opts = queue_opts(
            &td,
            config,
            TestExec::new().with("任务1", vec![scripted]),
        );
        let report = run_scheduler(&opts, &mut |_l| {}).expect("scheduler");
        assert_eq!(report.fatals, 1, "{report:?}");

        let runs = run_entities(&opts);
        assert_eq!(runs.len(), 1, "{runs:?}");
        assert_eq!(runs[0]["class"], "fatal");
        assert_eq!(runs[0]["exit"], "run_error");
        assert_eq!(runs[0]["exit_code"], 1);
        assert_eq!(runs[0]["outcome"], "error");
        assert_eq!(runs[0]["error"], "task bundle vanished");
        assert_eq!(runs[0]["clicks"], 0);
        std::fs::remove_dir_all(&td).ok();
    }

    #[test]
    fn queue_member_run_entities_carry_queue_round_and_member_index() {
        let td = tmpdir("p4-queue-members");
        write_task(&td, "成员1", OK_FLOW, None);
        write_task(&td, "成员2", OK_FLOW, None);
        let config = write_config(
            &td,
            &queue_config_text(&["成员1", "成员2"], 1, "continue", 0, "", ""),
        );
        let opts = queue_opts(
            &td,
            config,
            exec_two(ResultClass::Success, ResultClass::Success),
        );
        let report = run_scheduler(&opts, &mut |_l| {}).expect("scheduler");
        assert_eq!(report.runs, 1, "{report:?}");

        let runs = run_entities(&opts);
        assert_eq!(runs.len(), 2, "one run entity per member attempt: {runs:?}");
        for (i, run) in runs.iter().enumerate() {
            assert_eq!(run["kind"], "queue_member");
            assert_eq!(run["entry_id"], "qe");
            assert_eq!(run["queue_id"], "q1");
            assert_eq!(run["round"], 1);
            assert_eq!(run["member_index"], i);
            assert_eq!(run["task"], ["成员1", "成员2"][i]);
            assert_eq!(run["attempt"], 1);
            assert_eq!(run["class"], "success");
            assert_eq!(run["exit_code"], 0);
            assert_eq!(run["clicks"], 2);
        }
        assert_ne!(runs[0]["run_id"], runs[1]["run_id"]);
        std::fs::remove_dir_all(&td).ok();
    }

    #[test]
    fn run_finished_event_carries_exit_code_and_run_file_without_losing_fields() {
        let td = tmpdir("p4-event");
        write_task(&td, "任务1", OK_FLOW, None);
        let config = write_config(&td, &task_entry_config(0, "periodic"));
        let opts = queue_opts(
            &td,
            config,
            TestExec::new().with("任务1", vec![step(ResultClass::Success)]),
        );
        let report = run_scheduler(&opts, &mut |_l| {}).expect("scheduler");
        assert_eq!(
            report.run_file_errors, 0,
            "a healthy session never counts a write failure: {report:?}"
        );

        let runs = run_entities(&opts);
        assert_eq!(runs.len(), 1, "{runs:?}");
        let events = std::fs::read_to_string(opts.events_path()).expect("events");
        let finished = events
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("event JSON"))
            .find(|v| v["event"] == "run_finished")
            .expect("run_finished event");
        assert_eq!(finished["exit_code"], 0, "{finished}");
        assert_eq!(
            finished["run_file"],
            format!(
                "{SCHEDULER_RUN_DIR}/{}.json",
                runs[0]["run_id"].as_str().expect("run_id")
            )
        );
        assert!(
            finished.get("run_file_error").is_some() && finished["run_file_error"].is_null(),
            "a successful write reports run_file_error: null: {finished}"
        );
        // Existing fields must still be present (additive change only).
        for key in [
            "entry_id",
            "task",
            "pass",
            "attempts",
            "class",
            "exit",
            "outcome",
            "clicks",
            "duration_ms",
            "cooldown_until_ms",
            "quota_used",
        ] {
            assert!(finished.get(key).is_some(), "missing `{key}`: {finished}");
        }
        assert_eq!(finished["task"], "任务1");
        assert_eq!(finished["attempts"], 1);
        std::fs::remove_dir_all(&td).ok();
    }

    #[test]
    fn run_file_write_failure_alerts_and_counts_without_failing_the_run() {
        // P1-3: a lost run entity is observable (alert event + report counter +
        // `run_file_error` on the aggregate event) but never changes the run's
        // outcome or stops execution/state persistence.
        let td = tmpdir("p4-run-write-fail");
        write_task(&td, "任务1", OK_FLOW, None);
        let config = write_config(&td, &task_entry_config(0, "periodic"));
        let mut opts = queue_opts(
            &td,
            config,
            TestExec::new().with("任务1", vec![step(ResultClass::Success)]),
        );
        opts.report = Some(td.join("scheduler-report.json"));
        // Cross-platform "unwritable": a *file* where `<runs_dir>/scheduler/`
        // must be created makes `create_dir_all` fail inside write_json_atomic.
        std::fs::create_dir_all(&opts.runs_dir).expect("runs dir");
        std::fs::write(opts.runs_dir.join(SCHEDULER_RUN_DIR), "not a directory")
            .expect("blocker file");

        let mut lines = Vec::new();
        let report = run_scheduler(&opts, &mut |l| lines.push(l.to_string())).expect("scheduler");
        // The run itself still succeeded and its state was persisted/cleared.
        assert_eq!(report.runs, 1, "{report:?}");
        assert_eq!(report.successes, 1, "{report:?}");
        assert!(
            report.all_ok(),
            "a lost run file is not a run failure: {report:?}"
        );
        assert_eq!(report.run_file_errors, 1, "{report:?}");
        assert_eq!(
            report.alerts.len(),
            1,
            "one run_file_write alert: {:?}",
            report.alerts
        );
        let alert = &report.alerts[0];
        assert!(alert.starts_with("run_file_write:e1:"), "{alert}");
        assert!(
            alert.contains("task=任务1") && alert.contains("run_id=") && alert.contains("error="),
            "the alert names the task/run/error: {alert}"
        );
        assert!(
            lines
                .iter()
                .any(|l| l.starts_with("scheduler_run_file_error")),
            "{lines:?}"
        );
        assert!(
            lines
                .iter()
                .any(|l| l.starts_with("ALERT level=warn kind=run_file_write")),
            "the write failure is an ALERT line: {lines:?}"
        );
        assert!(
            lines
                .iter()
                .any(|l| l.starts_with("scheduler_summary") && l.contains("run_file_errors=1")),
            "the CLI summary counts the write failure: {lines:?}"
        );
        assert_eq!(
            state_of(&opts).running,
            None,
            "state was still cleared/saved"
        );
        assert!(
            !opts.runs_dir.join(SCHEDULER_RUN_DIR).is_dir(),
            "no run dir/entity was produced"
        );
        // The exit report (`--report`) exposes the additive counter + alert.
        let report_text =
            std::fs::read_to_string(opts.report.as_ref().expect("report path")).expect("report");
        let report_json: serde_json::Value =
            serde_json::from_str(&report_text).expect("report JSON");
        assert_eq!(report_json["run_file_errors"], 1, "{report_json}");
        assert!(
            report_json["alerts"][0]
                .as_str()
                .unwrap_or_default()
                .starts_with("run_file_write:e1:"),
            "{report_json}"
        );

        let events = std::fs::read_to_string(opts.events_path()).expect("events");
        let parsed: Vec<serde_json::Value> = events
            .lines()
            .map(|line| serde_json::from_str(line).expect("event JSON"))
            .collect();
        let alert_event = parsed
            .iter()
            .find(|v| v["event"] == "alert" && v["kind"] == "run_file_write")
            .expect("run_file_write alert event");
        assert_eq!(alert_event["entry_id"], "e1");
        assert!(
            alert_event["message"]
                .as_str()
                .unwrap_or_default()
                .contains("任务1"),
            "{alert_event}"
        );
        let finished = parsed
            .iter()
            .find(|v| v["event"] == "run_finished")
            .expect("run_finished event");
        assert!(finished["run_file"].is_null(), "{finished}");
        let err = finished["run_file_error"]
            .as_str()
            .expect("run_file_error names the write failure");
        assert!(
            err.contains("run_id=") && err.contains("scheduler/") && err.contains("error="),
            "{err}"
        );
        assert_eq!(
            finished["exit_code"], 0,
            "the run itself still succeeded: {finished}"
        );
        std::fs::remove_dir_all(&td).ok();
    }

    #[test]
    fn run_ids_are_unique_across_scheduler_restarts() {
        let td = tmpdir("p4-unique");
        write_task(&td, "任务1", OK_FLOW, None);
        // `temp` runs on every pass, so two independent sessions both write.
        let config = write_config(&td, &task_entry_config(0, "temp"));
        let opts = queue_opts(
            &td,
            config,
            TestExec::new().with("任务1", vec![step(ResultClass::Success)]),
        );
        run_scheduler(&opts, &mut |_l| {}).expect("run 1");
        run_scheduler(&opts, &mut |_l| {}).expect("run 2");
        let runs = run_entities(&opts);
        assert_eq!(runs.len(), 2, "{runs:?}");
        let ids: BTreeSet<&str> = runs
            .iter()
            .map(|r| r["run_id"].as_str().expect("run_id"))
            .collect();
        assert_eq!(ids.len(), 2, "run ids must stay unique: {ids:?}");
        assert_eq!(run_file_names(&opts).len(), 2);
        std::fs::remove_dir_all(&td).ok();
    }
}
