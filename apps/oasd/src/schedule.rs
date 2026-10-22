//! 调度器 v3 主循环 / CLI（ENGINE-DESIGN §22.34 · DELIVERY-PLAN M2 · N37）。
//!
//! Replaces the v1 `every_s` plan (`ENGINE §22.33`) with the v3 player config:
//! `scheduler.json` (schema_version 3) = calendar / guards / entries. The pure
//! scheduling matrix lives in [`crate::scheduler`]; this module owns all IO:
//! config/state files, the single-instance lock, execution via `run::execute`,
//! crash recovery, audit events and reports.
//!
//! # CLI
//! ```text
//! oasd scheduler [run] --config <scheduler.json> [--state <file>] [--tasks-dir tasks]
//!              [--once] [--max-rounds N] [--dry-run] [--status] [--report <file.json>]
//!              [--library <dir>] [--model rec.onnx] [--det-model det.onnx]
//!              [--replay <bundle>] [--clicks-dir <dir>] [--shots-dir <dir>] [--seed N]
//!              [--keep-going] [--tz-offset-min M] [--tick-ms MS] [--events <file.jsonl>]
//! oasd scheduler reset-memory --tasks a,b [--all] [--include-checkpoint]
//!              [--config <scheduler.json>] [--state <file>] [--tasks-dir tasks]
//! ```
//! `--plan` is accepted as an alias of `--config` (v1 flag spelling) and the
//! old `--max-runs` as an alias of `--max-rounds`. `oasd schedule ...` is kept
//! as the historical entry point (`schedule_cmd`), so old start scripts still
//! dispatch here; a v1 plan file is rejected with a migration hint.
//!
//! # State file (`scheduler.state.json`)
//! Atomic (tmp + rename) writes of **runtime state only**:
//! ```json
//! {
//!   "schema_version": 3,
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
//!   }
//! }
//! ```
//! There is deliberately **no "completed" flag** for temp tasks and no
//! cross-pass "already ran" memory (§22.34.3): the only memory is runtime
//! counters, quota counters and cooldown. The `running` marker is written
//! before every execution; if the process dies, the next start sees a dirty
//! `running` entry and re-runs it with `--resume` (periodic) or resets the temp
//! task state and starts fresh (temp — no completion memory by design).
//!
//! # Audit events (`scheduler.events.jsonl`)
//! Append-only JSONL next to the state file (`<config>.events.jsonl` by
//! default): `state_recovered`, `pass`, `run_finished`, `temp_state_reset`,
//! `alert`, `wake`, `stop`, `memory_reset`. `--report` additionally writes the
//! full machine-readable summary of the session.
//!
//! # Reset memory
//! `reset-memory` clears, for the selected tasks: ① the scheduler entry state
//! (attempts/cooldown/quota counters) and ② `tasks/<id>/state/` (checkpoint,
//! learned coordinates — the task's memory), and appends a `memory_reset`
//! audit event. It refuses to run while a scheduler holds the lock, and leaves
//! run statistics (`runs/ *.clicks.jsonl`) untouched. Checkpoint clearing is
//! **on by default** (`--include-checkpoint` is the explicit spelling);
//! `--keep-checkpoint` keeps `checkpoint.json` while clearing the rest.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write as IoWrite;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use oas_runtime::RunResult;

use crate::run::{self, RunOptions, RunReport};
use crate::scheduler::{
    self, classify_signal, clear_running, cycle_ids, entry_next_due_ms, format_local_datetime,
    guard_reason, mark_running, next_wake, plan_pass, quota_remaining, quota_used, record_result,
    roll_cycles, should_retry, validate_scheduler_with_offset, Entry, GuardBlock, Lifecycle,
    PassPlan, ResultClass, RunMode, RunSignal, Scheduler, SchedulerState, DEFAULT_UTC_OFFSET_MIN,
    MAX_SLEEP_MS, SCHEDULE_SCHEMA_VERSION,
};

/// Default player config (§22.34 / M3 `config::SchedulerConfig::plan`).
pub const DEFAULT_CONFIG: &str = "scheduler.json";
pub const DEFAULT_TASKS_DIR: &str = "tasks";
pub const DEFAULT_MAX_SLEEP_MS: u64 = MAX_SLEEP_MS as u64;

pub const USAGE: &str = "usage: oasd scheduler [run] [--config <scheduler.json>] [--state <file>] \
                         [--tasks-dir tasks] [--once] [--max-rounds N] [--dry-run] [--status] \
                         [--report <file.json>] [--library <dir>] [--model <rec.onnx>] \
                         [--det-model <det.onnx>] [--replay <bundle>] [--clicks-dir <dir>] \
                         [--shots-dir <dir>] [--seed N] [--keep-going] [--tz-offset-min M] \
                         [--tick-ms MS] [--events <file.jsonl>]
       oasd scheduler reset-memory --tasks <a,b> [--all] [--include-checkpoint] \
                         [--config <scheduler.json>] [--state <file>] [--tasks-dir tasks]";

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SchedulerOptions {
    /// `scheduler.json` v3 (`--config` / `--plan`).
    pub config: PathBuf,
    /// `scheduler.state.json` (default: `<config stem>.state.json`).
    pub state: Option<PathBuf>,
    pub tasks_dir: PathBuf,
    pub library: PathBuf,
    pub model: Option<PathBuf>,
    pub det_model: Option<PathBuf>,
    pub replay: Option<PathBuf>,
    pub clicks_dir: Option<PathBuf>,
    pub shots_dir: Option<PathBuf>,
    pub seed: Option<u64>,
    /// One pass, then exit (no sleeping).
    pub once: bool,
    /// Maximum passes in this session.
    pub max_rounds: Option<u64>,
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
            shots_dir: None,
            seed: None,
            once: false,
            max_rounds: None,
            dry_run: false,
            status: false,
            report: None,
            events: None,
            keep_going: false,
            tz_offset_min: None,
            max_sleep_ms: DEFAULT_MAX_SLEEP_MS,
            loop_interval_s: None,
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
                "--shots-dir" => {
                    o.shots_dir = Some(PathBuf::from(take_value(rest, &mut i, "--shots-dir")?))
                }
                "--seed" => {
                    o.seed = Some(parse_u64(&take_value(rest, &mut i, "--seed")?, "--seed")?)
                }
                "--max-rounds" | "--max-runs" => {
                    let v = parse_u64(
                        &take_value(rest, &mut i, "--max-rounds")?,
                        "--max-rounds",
                    )?;
                    if v == 0 {
                        return Err("--max-rounds must be >= 1".to_string());
                    }
                    o.max_rounds = Some(v);
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
    if !path.is_file() {
        return Ok(SchedulerState::new());
    }
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("scheduler state: read `{}`: {e}", path.display()))?;
    let state: SchedulerState = serde_json::from_str(&text)
        .map_err(|e| format!("scheduler state `{}`: invalid JSON: {e}", path.display()))?;
    if state.schema_version != SCHEDULE_SCHEMA_VERSION {
        return Err(format!(
            "scheduler state `{}`: schema_version {} unsupported (expect {}); \
             delete it (memory reset) or point --state elsewhere",
            path.display(),
            state.schema_version,
            SCHEDULE_SCHEMA_VERSION
        ));
    }
    Ok(state)
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
    for entry in &sched.entries {
        if metas.contains_key(&entry.task) {
            continue;
        }
        let dir = opts.tasks_dir.join(&entry.task);
        if !dir.is_dir() {
            return Err(format!(
                "scheduler config: entry `{}` task `{}` not found under `{}`",
                entry.entry_id,
                entry.task,
                opts.tasks_dir.display()
            ));
        }
        let meta_path = dir.join("meta.json");
        let text = std::fs::read_to_string(&meta_path).map_err(|e| {
            format!(
                "scheduler config: entry `{}` task `{}`: meta.json: {e} \
                 (not a task bundle)",
                entry.entry_id, entry.task
            )
        })?;
        let exits = scheduler::parse_meta_exits(&text)
            .map_err(|e| format!("scheduler config: task `{}`: {e}", entry.task))?;
        metas.insert(entry.task.clone(), exits);
    }
    Ok(Loaded { sched, metas })
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

/// Is a process with `pid` alive? Used only for stale-lock detection.
fn process_alive(pid: u32) -> bool {
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

fn lock_held_by_live_process(state_path: &Path) -> Option<u32> {
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SchedulerReport {
    pub schema_version: u32,
    pub started_ms: u64,
    pub finished_ms: u64,
    pub dry_run: bool,
    pub mode: String,
    pub passes: u64,
    /// Entry executions (one per planned entry per pass, retries included).
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
}

impl SchedulerReport {
    pub fn all_ok(&self) -> bool {
        self.fatals == 0 && self.blocked == 0 && self.failures == 0
    }
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

impl<'a> Session<'a> {
    fn run_entry(
        &mut self,
        entry: &Entry,
        planned: &scheduler::PlannedRun,
        pass: u64,
        out: &mut dyn FnMut(&str),
    ) -> Result<ResultClass, String> {
        let meta = meta_exits_for(&self.loaded.metas, &entry.task);
        let mut want_resume = planned.resume;
        let mut attempt: u32 = 0;
        let mut attempts: u32 = 0;
        let mut clicks_total: usize = 0;
        let exec_started = now_ms();

        let (class, exit, outcome, error) = loop {
            attempt += 1;
            attempts += 1;

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

            let resume = if want_resume {
                let ckpt = task_checkpoint_path(&self.opts.tasks_dir, &entry.task);
                if ckpt.is_file() {
                    true
                } else {
                    out(&format!(
                        "scheduler_resume_skipped entry={} task={} reason=no_checkpoint",
                        entry.entry_id, entry.task
                    ));
                    false
                }
            } else {
                false
            };

            mark_running(&mut self.state, entry, pass, attempt, now_ms(), resume);
            self.save_state(out)?;

            let seq = self.seq;
            self.seq += 1;
            let clicks_out = self.opts.clicks_dir.as_ref().map(|dir| {
                dir.join(format!(
                    "{seq:05}-{}-p{pass}-a{attempt}.clicks.jsonl",
                    entry.task
                ))
            });
            let run_opts = RunOptions {
                task: Some(self.opts.tasks_dir.join(&entry.task)),
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
                params: entry_run_params(entry),
                ..RunOptions::default()
            };
            out(&format!(
                "scheduler_run entry={} task={} pass={pass} attempt={attempt} resume={resume} lifecycle={}",
                entry.entry_id,
                entry.task,
                entry.lifecycle.as_str()
            ));
            let started = now_ms();
            let result = run::execute(&run_opts, out);
            let _attempt_duration_ms = now_ms().saturating_sub(started);

            let (class, exit, outcome, error): (ResultClass, String, String, Option<String>) =
                match result {
                    Ok(report) => {
                        let signal = signal_of(&report);
                        let class = classify_signal(signal, &meta);
                        clicks_total += report.clicks.len();
                        (
                            class,
                            signal.name().to_string(),
                            format!("{:?}", report.outcome()),
                            None,
                        )
                    }
                    Err(e) => {
                        out(&format!(
                            "scheduler_run_error entry={} task={} attempt={attempt}: {e}",
                            entry.entry_id, entry.task
                        ));
                        let class = classify_signal(RunSignal::RunFailed, &meta);
                        (class, "run_error".to_string(), "error".to_string(), Some(e))
                    }
                };
            if should_retry(class, attempt, entry) {
                out(&format!(
                    "scheduler_retry entry={} task={} attempt={attempt} class={}",
                    entry.entry_id,
                    entry.task,
                    class.as_str()
                ));
                // Retry after a failure: resume from the checkpoint when the
                // run left one (periodic only; temp state is reset above).
                want_resume = entry.lifecycle == Lifecycle::Periodic;
                continue;
            }
            break (class, exit, outcome, error);
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
            stats.ran += 1;
            let class = self.run_entry(&entry, planned, pass, out)?;
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
            json!({
                "entry_id": entry.entry_id,
                "task": entry.task,
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
    let mut choices: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for entry in &sched.entries {
        choices
            .entry(entry.task.clone())
            .or_default()
            .push(entry.entry_id.clone());
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

    Ok(json!({
        "schema_version": SCHEDULE_SCHEMA_VERSION,
        "generated_ms": now,
        "generated_local": format_local_datetime(now, offset),
        "config_path": opts.config.display().to_string(),
        "state_path": state_path.display().to_string(),
        "events_path": opts.events_path().display().to_string(),
        "utc_offset_min": offset,
        "mode": sched.mode.as_str(),
        "loop_interval_s": sched.loop_interval_s,
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
    let mut state = load_state(&state_path)?;
    let now = now_ms() as i64;

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
    };
    // A marker that no longer maps to an enabled entry can never be finished:
    // drop it loudly instead of leaving the state dirty forever.
    if let Some(stale) = session.recovery.clone() {
        let still_configured = session
            .sched()
            .entry(&stale.entry_id)
            .map(|e| e.enabled)
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
        "scheduler_start config={} state={} tasks_dir={} mode={} once={} max_rounds={}",
        opts.config.display(),
        state_path.display(),
        opts.tasks_dir.display(),
        session.sched().mode.as_str(),
        opts.once,
        opts.max_rounds
            .map(|v| v.to_string())
            .unwrap_or_else(|| "-".to_string())
    ));
    session.audit(json!({
        "event": "scheduler_start",
        "config": opts.config.display().to_string(),
        "mode": session.sched().mode.as_str(),
        "once": opts.once,
        "max_rounds": opts.max_rounds,
        "utc_offset_min": session.sched().calendar.utc_offset_min,
    }));
    session.save_state(out)?;

    let cli_max = opts.max_rounds;
    let loop_rounds = session.sched().loop_rounds;
    let mut session_passes: u64 = 0;
    let stopped_reason: String;
    loop {
        session_passes += 1;
        let pass_no = session.state.passes + 1;
        let now = now_ms() as i64;
        let calendar = session.sched().calendar;
        let roll = roll_cycles(&mut session.state, &calendar, now);
        if roll.changed {
            session.save_state(out)?;
        }
        let plan = session.plan_now(now);
        let stats = session.execute_pass(pass_no, now, &plan, out)?;

        session.state.last_pass_ms = now as u64;
        session.state.passes = session.state.passes.saturating_add(1);
        session.report.passes += 1;
        session.save_state(out)?;

        // -- stopping rules --------------------------------------------------
        let mut reason: Option<&'static str> = None;
        if stats.fatal_stop {
            reason = Some("fatal");
        } else if let Some(max) = cli_max {
            if session_passes >= max {
                reason = Some("max_rounds");
            }
        }
        if reason.is_none() && loop_rounds > 0 && session_passes >= loop_rounds {
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
        if let Some(r) = reason {
            stopped_reason = r.to_string();
            break;
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
        if sleep_ms > 0 {
            std::thread::sleep(Duration::from_millis(sleep_ms));
        }
    }

    session.state.last_stop = stopped_reason.clone();
    session.save_state(out)?;
    session.audit(json!({"event": "stop", "reason": stopped_reason, "passes": session.report.passes}));

    let now = now_ms() as i64;
    session.report.stopped_reason = stopped_reason.clone();
    session.report.finished_ms = now_ms();
    session.report.entries = build_entry_reports(&session.sched(), &session.state, now);
    session.report.next_wake_ms = Some(next_wake(&session.sched(), &session.state, now).at_ms);
    out(&format!(
        "scheduler_stop reason={stopped_reason} passes={} runs={} successes={} resources={} blocked={} failures={} fatals={}",
        session.report.passes,
        session.report.runs,
        session.report.successes,
        session.report.resources,
        session.report.blocked,
        session.report.failures,
        session.report.fatals
    ));
    out(&format!(
        "scheduler_summary runs={} successes={} resources={} blocked={} failures={} fatals={} dry_run=false",
        session.report.runs,
        session.report.successes,
        session.report.resources,
        session.report.blocked,
        session.report.failures,
        session.report.fatals
    ));
    if let Some(path) = &opts.report {
        write_report(path, &session.report)?;
    }
    Ok(session.report)
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
            for entry in &sched.entries {
                targets.insert(entry.task.clone());
            }
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

    let mut state = load_state(&state_path)?;
    let mut report = ResetReport {
        tasks: targets.iter().cloned().collect(),
        state_path: state_path.display().to_string(),
        events_path: events_path.display().to_string(),
        ..ResetReport::default()
    };

    // ① Scheduler entry state (attempts/cooldown/quota AND a dirty running
    // marker: an offline crash residue is exactly "memory").
    let mut clear_ids: Vec<String> = Vec::new();
    for (entry_id, st) in &state.entries {
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
        if targets.contains(&running_task) {
            state.running = None;
            state_changed = true;
            out(&format!(
                "reset_memory_cleared_dirty_running={}",
                running.entry_id
            ));
        }
    }
    if state_changed {
        write_json_atomic(&state_path, &state)
            .map_err(|e| format!("reset-memory: state: {e}"))?;
    }
    report.entries = clear_ids;

    // ② Task state/ (checkpoint + learned coords + counters) — memory, not stats.
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

    // ③ Audit event (fatal on failure: a reset without an audit trail is not
    // acceptable — §22.34.5 "审计 memory_reset").
    append_jsonl(
        &events_path,
        &json!({
            "ts_ms": now_ms(),
            "event": "memory_reset",
            "tasks": report.tasks,
            "entry_ids": report.entries,
            "files_removed": files_removed,
            "checkpoint_kept": opts.keep_checkpoint,
            "state_path": report.state_path,
        }),
    )
    .map_err(|e| format!("reset-memory: audit: {e}"))?;

    out(&format!("reset_memory_tasks={}", report.tasks.join(",")));
    out(&format!("reset_memory_entries={}", report.entries.len()));
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
    let opts = match SchedulerOptions::parse(rest) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("scheduler: {e}");
            return ExitCode::from(2);
        }
    };
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

/// Entry `params` -> `RunOptions.params` (`--param k=v` equivalent).
///
/// Config values are JSON scalars (`3`, `true`, `"fast"`); `run.rs` parses the
/// string forms and merges the bundle's `meta.params[].default` underneath.
/// Non-scalar values are passed as compact JSON (treated as a string parameter).
fn entry_run_params(entry: &Entry) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for (key, value) in &entry.params {
        let text = match value {
            Value::String(s) => s.clone(),
            Value::Bool(b) => b.to_string(),
            Value::Number(n) => n.to_string(),
            other => serde_json::to_string(other).unwrap_or_default(),
        };
        out.insert(key.clone(), text);
    }
    out
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
            ..SchedulerOptions::default()
        }
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
        opts.max_rounds = Some(2);
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
        assert_eq!(v["schema_version"], 3);
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
        std::fs::write(
            &ckpt,
            r#"{"schema_version":1,"flow_name":"resume-flow","steps":2,"next_step":1}"#,
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
        opts.max_rounds = Some(5);
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
        assert!(err.contains("schema_version must be 3"), "{err}");
        assert!(err.contains("v3") || err.contains("§22.34"), "{err}");
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
            "--max-rounds",
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
        assert_eq!(o.max_rounds, Some(3));
        assert_eq!(o.report, Some(PathBuf::from("r.json")));
        assert_eq!(o.events, Some(PathBuf::from("e.jsonl")));
        assert_eq!(o.tz_offset_min, Some(480));
        assert_eq!(o.max_sleep_ms, 500);
        assert_eq!(o.loop_interval_s, Some(2));
        assert_eq!(o.model, Some(PathBuf::from("rec.onnx")));
        assert_eq!(o.det_model, Some(PathBuf::from("det.onnx")));
        // `--plan` and `--max-runs` are accepted as v1-compatible aliases.
        let alias: Vec<String> = ["--plan", "p.json", "--max-runs", "2"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let o = SchedulerOptions::parse(&alias).expect("alias parse");
        assert_eq!(o.config, PathBuf::from("p.json"));
        assert_eq!(o.max_rounds, Some(2));
        assert!(SchedulerOptions::parse(&["--nope".to_string()]).is_err());
        assert!(SchedulerOptions::parse(&["--max-rounds".to_string(), "0".to_string()]).is_err());
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
        assert_eq!(v["schema_version"], 3);
        assert_eq!(v["runs"], 1);
        assert_eq!(v["successes"], 1);
        assert_eq!(v["entries"][0]["entry_id"], "e1");
        assert_eq!(v["entries"][0]["quota_used"], 1);
        assert_eq!(v["stopped_reason"], "once");
        assert_eq!(report.entries[0].quota_remaining, Some(0));
        std::fs::remove_dir_all(&td).ok();
    }
}
