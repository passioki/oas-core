//! 纵向⑥：周期调度/守护运行（ENGINE §22.33）。
//!
//! Reads a JSON plan of periodic jobs and runs due tasks **serially** through the
//! normal `run::execute` path. State (next run time / last outcome) is persisted
//! so a restarted daemon does not spin off the same job immediately.
//!
//! Usage:
//!   oasd schedule --plan schedule.json [--state file] [--tasks-dir tasks]
//!                 [--once] [--max-runs N] [--tick-ms 1000] [--dry-run]
//!                 [--report out.json] [--library <dir>]
//!                 [--model rec.onnx] [--det-model det.onnx]
//!                 [--clicks-dir dir] [--shots-dir dir] [--seed N]
//!
//! This is the "production loop" piece: queue runs a fixed list once, schedule
//! keeps due jobs running on their period (state survives restarts).

use std::path::PathBuf;
use std::process::ExitCode;

use serde::{Deserialize, Serialize};

use crate::run::{self, RunOptions};

pub const USAGE: &str = "usage: oasd schedule --plan <plan.json> [--state <file>] \
                         [--tasks-dir tasks] [--once] [--max-runs N] [--tick-ms 1000] \
                         [--dry-run] [--report <file.json>] [--library <dir>] \
                         [--model <rec.onnx>] [--det-model <det.onnx>] \
                         [--clicks-dir <dir>] [--shots-dir <dir>] [--seed N]";

pub const DEFAULT_TICK_MS: u64 = 1000;
pub const MAX_RETRIES: u32 = 5;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleJob {
    pub task: String,
    /// 周期（秒）。1s 是下限；长时间挂机通常 600~7200。
    pub every_s: u64,
    #[serde(default)]
    pub retries: u32,
    #[serde(default)]
    pub retry_delay_s: u64,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulePlan {
    pub schema_version: u32,
    pub jobs: Vec<ScheduleJob>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct JobState {
    pub task: String,
    #[serde(default)]
    pub next_run_ms: u64,
    #[serde(default)]
    pub last_run_ms: u64,
    #[serde(default)]
    pub last_outcome: String,
    #[serde(default)]
    pub runs: u64,
    #[serde(default)]
    pub failures: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScheduleState {
    pub schema_version: u32,
    #[serde(default)]
    pub jobs: Vec<JobState>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct JobReport {
    pub task: String,
    pub runs: u64,
    pub successes: u64,
    pub failures: u64,
    pub last_outcome: String,
    pub next_run_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScheduleReport {
    pub schema_version: u32,
    pub started_ms: u64,
    pub finished_ms: u64,
    pub dry_run: bool,
    pub runs: u64,
    pub successes: u64,
    pub failures: u64,
    pub jobs: Vec<JobReport>,
}

impl ScheduleReport {
    pub fn all_ok(&self) -> bool {
        self.failures == 0
    }
}

#[derive(Debug, Clone)]
pub struct ScheduleOptions {
    pub plan: PathBuf,
    pub state: Option<PathBuf>,
    pub tasks_dir: PathBuf,
    pub once: bool,
    pub max_runs: Option<u64>,
    pub tick_ms: u64,
    pub dry_run: bool,
    pub report: Option<PathBuf>,
    pub library: PathBuf,
    pub model: Option<PathBuf>,
    pub det_model: Option<PathBuf>,
    pub clicks_dir: Option<PathBuf>,
    pub shots_dir: Option<PathBuf>,
    pub seed: Option<u64>,
    /// 可选的录制回放（测试/离线调度）；None = 实机。
    pub replay: Option<PathBuf>,
}

impl Default for ScheduleOptions {
    fn default() -> Self {
        Self {
            plan: PathBuf::from("schedule.json"),
            state: None,
            tasks_dir: PathBuf::from("tasks"),
            once: false,
            max_runs: None,
            tick_ms: DEFAULT_TICK_MS,
            dry_run: false,
            report: None,
            library: PathBuf::from(crate::library::DEFAULT_DIR),
            model: None,
            det_model: None,
            clicks_dir: None,
            shots_dir: None,
            seed: None,
            replay: None,
        }
    }
}

impl ScheduleOptions {
    pub fn parse(rest: &[String]) -> Result<Self, String> {
        let mut o = Self::default();
        let mut i = 0usize;
        while i < rest.len() {
            let flag = rest[i].as_str();
            match flag {
                "--plan" => o.plan = PathBuf::from(take_value(rest, &mut i, "--plan")?),
                "--state" => o.state = Some(PathBuf::from(take_value(rest, &mut i, "--state")?)),
                "--tasks-dir" => {
                    o.tasks_dir = PathBuf::from(take_value(rest, &mut i, "--tasks-dir")?)
                }
                "--tick-ms" => {
                    o.tick_ms = parse_u64(&take_value(rest, &mut i, "--tick-ms")?, "--tick-ms")?
                }
                "--max-runs" => {
                    o.max_runs = Some(parse_u64(
                        &take_value(rest, &mut i, "--max-runs")?,
                        "--max-runs",
                    )?)
                }
                "--report" => o.report = Some(PathBuf::from(take_value(rest, &mut i, "--report")?)),
                "--library" => {
                    o.library = PathBuf::from(take_value(rest, &mut i, "--library")?)
                }
                "--model" => o.model = Some(PathBuf::from(take_value(rest, &mut i, "--model")?)),
                "--det-model" => {
                    o.det_model = Some(PathBuf::from(take_value(rest, &mut i, "--det-model")?))
                }
                "--replay" => o.replay = Some(PathBuf::from(take_value(rest, &mut i, "--replay")?)),
                "--clicks-dir" => {
                    o.clicks_dir = Some(PathBuf::from(take_value(rest, &mut i, "--clicks-dir")?))
                }
                "--shots-dir" => {
                    o.shots_dir = Some(PathBuf::from(take_value(rest, &mut i, "--shots-dir")?))
                }
                "--seed" => {
                    o.seed = Some(parse_u64(&take_value(rest, &mut i, "--seed")?, "--seed")?)
                }
                "--once" => {
                    o.once = true;
                    i += 1;
                }
                "--dry-run" => {
                    o.dry_run = true;
                    i += 1;
                }
                other => return Err(format!("unknown option `{other}`\n{USAGE}")),
            }
        }
        if o.tick_ms == 0 {
            return Err("--tick-ms must be > 0".to_string());
        }
        Ok(o)
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

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Parse + validate a schedule plan. Every check fails loudly with no writes.
pub fn parse_plan(text: &str) -> Result<SchedulePlan, String> {
    let plan: SchedulePlan =
        serde_json::from_str(text).map_err(|e| format!("schedule plan: invalid JSON: {e}"))?;
    if plan.schema_version != 1 {
        return Err(format!(
            "schedule plan: unsupported schema_version {} (expected 1)",
            plan.schema_version
        ));
    }
    if plan.jobs.is_empty() {
        return Err("schedule plan: `jobs` must not be empty".to_string());
    }
    let mut seen = std::collections::BTreeSet::new();
    for (i, job) in plan.jobs.iter().enumerate() {
        oas_task::validate_task_name(&job.task)
            .map_err(|e| format!("schedule plan: jobs[{i}].task: {e}"))?;
        if job.every_s == 0 {
            return Err(format!(
                "schedule plan: jobs[{i}].every_s must be >= 1 (got 0)"
            ));
        }
        if job.retries > MAX_RETRIES {
            return Err(format!(
                "schedule plan: jobs[{i}].retries must be <= {MAX_RETRIES} (got {})",
                job.retries
            ));
        }
        if !seen.insert(job.task.clone()) {
            return Err(format!(
                "schedule plan: duplicate task `{}` (jobs[{i}])",
                job.task
            ));
        }
    }
    Ok(plan)
}

fn state_for<'a>(state: &'a ScheduleState, task: &str) -> Option<&'a JobState> {
    state.jobs.iter().find(|j| j.task == task)
}

fn state_for_mut<'a>(state: &'a mut ScheduleState, task: &str) -> &'a mut JobState {
    if let Some(idx) = state.jobs.iter().position(|j| j.task == task) {
        return &mut state.jobs[idx];
    }
    state.jobs.push(JobState {
        task: task.to_string(),
        ..JobState::default()
    });
    let last = state.jobs.len() - 1;
    &mut state.jobs[last]
}

/// Persist state atomically (tmp + rename).
fn write_state(path: &std::path::Path, state: &ScheduleState) -> Result<(), String> {
    let text = serde_json::to_string_pretty(state)
        .map_err(|e| format!("schedule state: serialize: {e}"))?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("schedule state: create `{}`: {e}", dir.display()))?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, text + "\n")
        .map_err(|e| format!("schedule state: write `{}`: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path)
        .map_err(|e| format!("schedule state: rename to `{}`: {e}", path.display()))
}

fn load_state(path: &std::path::Path) -> Result<ScheduleState, String> {
    if !path.is_file() {
        return Ok(ScheduleState::default());
    }
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("schedule state: read `{}`: {e}", path.display()))?;
    serde_json::from_str(&text)
        .map_err(|e| format!("schedule state: parse `{}`: {e}", path.display()))
}

fn state_path(opts: &ScheduleOptions) -> PathBuf {
    opts.state.clone().unwrap_or_else(|| {
        let mut p = opts.plan.clone();
        let name = p
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "schedule.json".to_string());
        p.set_file_name(format!("{name}.state.json"));
        p
    })
}

/// Which jobs are due now (enabled, next_run <= now), earliest first.
pub fn due_jobs(
    plan: &SchedulePlan,
    state: &ScheduleState,
    now: u64,
) -> Vec<usize> {
    let mut due: Vec<(u64, usize)> = Vec::new();
    for (i, job) in plan.jobs.iter().enumerate() {
        if !job.enabled {
            continue;
        }
        let next = state_for(state, &job.task)
            .map(|s| s.next_run_ms)
            .unwrap_or(0);
        if next <= now {
            due.push((next, i));
        }
    }
    due.sort();
    due.into_iter().map(|(_, i)| i).collect()
}

fn advance(state: &mut ScheduleState, job: &ScheduleJob, now: u64, outcome: &str) {
    let st = state_for_mut(state, &job.task);
    st.last_run_ms = now;
    st.last_outcome = outcome.to_string();
    st.runs += 1;
    if outcome != "done" {
        st.failures += 1;
    }
    st.next_run_ms = now.saturating_add(job.every_s.saturating_mul(1000));
}

/// Run the scheduler until `--once` / `--max-runs` / no jobs. Returns a report.
pub fn run_schedule(
    opts: &ScheduleOptions,
    out: &mut dyn FnMut(&str),
) -> Result<ScheduleReport, String> {
    let plan_text = std::fs::read_to_string(&opts.plan)
        .map_err(|e| format!("schedule plan: read `{}`: {e}", opts.plan.display()))?;
    let plan = parse_plan(&plan_text)?;
    // Fail fast when a job points at a missing task directory.
    for job in &plan.jobs {
        let task_dir = opts.tasks_dir.join(&job.task);
        if !task_dir.is_dir() {
            return Err(format!(
                "schedule plan: job task `{}` not found under `{}`",
                job.task,
                opts.tasks_dir.display()
            ));
        }
    }
    if let Some(dir) = &opts.clicks_dir {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("--clicks-dir: create `{}`: {e}", dir.display()))?;
    }
    let path = state_path(opts);
    let mut state = load_state(&path)?;
    if state.schema_version == 0 {
        state.schema_version = 1;
    }
    // New jobs start due immediately; existing jobs keep their next_run.
    let now0 = now_ms();
    for job in &plan.jobs {
        if state_for(&state, &job.task).is_none() {
            let st = state_for_mut(&mut state, &job.task);
            st.next_run_ms = now0;
        }
    }

    let started = now_ms();
    let mut report = ScheduleReport {
        schema_version: 1,
        started_ms: started,
        dry_run: opts.dry_run,
        ..ScheduleReport::default()
    };
    let mut ran: u64 = 0;
    loop {
        let now = now_ms();
        let due = due_jobs(&plan, &state, now);
        if due.is_empty() {
            if opts.once || opts.max_runs.map(|m| ran >= m).unwrap_or(false) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(opts.tick_ms));
            continue;
        }
        let mut progressed = false;
        for idx in due {
            if let Some(max) = opts.max_runs {
                if ran >= max {
                    break;
                }
            }
            let job = &plan.jobs[idx];
            let task_dir = opts.tasks_dir.join(&job.task);
            if opts.dry_run {
                out(&format!(
                    "schedule_dry_run task={} every_s={} next_run_ms={}",
                    job.task, job.every_s, now
                ));
                ran += 1;
                progressed = true;
                continue;
            }
            let mut outcome = "failed".to_string();
            let mut attempts = 0u32;
            loop {
                attempts += 1;
                let seed = opts
                    .seed
                    .map(|s| s.saturating_add((attempts as u64).saturating_sub(1)));
                let clicks_out = opts.clicks_dir.as_ref().map(|dir| {
                    dir.join(format!(
                        "{}-{}-a{}.clicks.jsonl",
                        job.task,
                        now_ms(),
                        attempts
                    ))
                });
                let run_opts = RunOptions {
                    task: Some(task_dir.clone()),
                    replay: opts.replay.clone(),
                    seed,
                    clicks_out,
                    shots_dir: opts.shots_dir.clone(),
                    library: opts.library.clone(),
                    model: opts.model.clone(),
                    det_model: opts.det_model.clone(),
                    ..RunOptions::default()
                };
                out(&format!(
                    "schedule_job task={} attempt={attempts} every_s={}",
                    job.task, job.every_s
                ));
                match run::execute(&run_opts, out) {
                    Ok(r) => {
                        outcome = format!("{:?}", r.outcome()).to_lowercase();
                        out(&format!(
                            "schedule_job_done task={} attempt={attempts} outcome={outcome} clicks={}",
                            job.task,
                            r.clicks.len()
                        ));
                        if outcome == "done" || attempts > job.retries {
                            break;
                        }
                    }
                    Err(e) => {
                        out(&format!(
                            "schedule_job_error task={} attempt={attempts}: {e}",
                            job.task
                        ));
                        outcome = "failed".to_string();
                        if attempts > job.retries {
                            break;
                        }
                    }
                }
                if job.retry_delay_s > 0 {
                    std::thread::sleep(std::time::Duration::from_secs(job.retry_delay_s));
                }
            }
            let now2 = now_ms();
            advance(&mut state, job, now2, &outcome);
            write_state(&path, &state)?;
            report.runs += 1;
            if outcome == "done" {
                report.successes += 1;
            } else {
                report.failures += 1;
            }
            ran += 1;
            progressed = true;
        }
        if opts.once || opts.dry_run {
            break;
        }
        if let Some(max) = opts.max_runs {
            if ran >= max {
                break;
            }
        }
        if !progressed {
            std::thread::sleep(std::time::Duration::from_millis(opts.tick_ms));
        }
    }

    // Per-job rollup from the final state.
    for job in &plan.jobs {
        if let Some(st) = state_for(&state, &job.task) {
            report.jobs.push(JobReport {
                task: job.task.clone(),
                runs: st.runs,
                successes: st.runs.saturating_sub(st.failures),
                failures: st.failures,
                last_outcome: st.last_outcome.clone(),
                next_run_ms: st.next_run_ms,
            });
        }
    }
    report.finished_ms = now_ms();
    // Dry-run is a pure preview: never write state (otherwise a preview would
    // push the real next_run into the future).
    if !opts.dry_run {
        write_state(&path, &state)?;
    }
    if let Some(path) = &opts.report {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("--report: create `{}`: {e}", dir.display()))?;
        }
        let text = serde_json::to_string_pretty(&report)
            .map_err(|e| format!("--report: serialize: {e}"))?;
        std::fs::write(path, text + "\n")
            .map_err(|e| format!("--report: write `{}`: {e}", path.display()))?;
    }
    Ok(report)
}

pub fn schedule_cmd(rest: &[String]) -> ExitCode {
    if rest.iter().any(|a| a == "-h" || a == "--help") {
        println!("{USAGE}");
        return ExitCode::from(2);
    }
    let opts = match ScheduleOptions::parse(rest) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("schedule: {e}");
            return ExitCode::from(2);
        }
    };
    let mut lines = std::io::stdout();
    let mut out = |line: &str| {
        use std::io::Write;
        let _ = writeln!(lines, "{line}");
    };
    match run_schedule(&opts, &mut out) {
        Ok(report) => {
            println!(
                "schedule runs={} successes={} failures={} dry_run={}",
                report.runs, report.successes, report.failures, report.dry_run
            );
            if report.all_ok() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(e) => {
            eprintln!("schedule: {e}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oas_obs::Recorder;
    use oas_task::TaskBundle;
    use oas_types::Frame;

    fn tmpdir(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "oas-schedule-{tag}-{}-{}",
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

    fn write_task(dir: &std::path::Path, flow: &str) {
        let b = TaskBundle::open(dir).expect("open task");
        b.save_flow(flow).expect("save flow");
    }

    fn write_replay(dir: &std::path::Path) {
        let mut rec = Recorder::create(dir, "schedule-test").expect("recorder");
        rec.capture_frame(&frame(10)).expect("f0");
        rec.capture_frame(&frame(20)).expect("f1");
        rec.finish().expect("finish");
    }

    #[test]
    fn plan_validation_rejects_bad_entries() {
        for bad in [
            r#"{"schema_version":1,"jobs":[]}"#,
            r#"{"schema_version":1,"jobs":[{"task":"a","every_s":0}]}"#,
            r#"{"schema_version":1,"jobs":[{"task":"a","every_s":60,"retries":6}]}"#,
            r#"{"schema_version":1,"jobs":[{"task":"a","every_s":60},{"task":"a","every_s":120}]}"#,
            r#"{"schema_version":2,"jobs":[{"task":"a","every_s":60}]}"#,
        ] {
            let err = parse_plan(bad).expect_err("must reject");
            assert!(!err.is_empty(), "rejection must explain: {bad}");
        }
        let ok = parse_plan(
            r#"{"schema_version":1,"jobs":[{"task":"任务1","every_s":600,"retries":1}]}"#,
        )
        .expect("valid plan");
        assert_eq!(ok.jobs.len(), 1);
        assert!(ok.jobs[0].enabled);
    }

    #[test]
    fn due_jobs_and_advance_follow_the_period() {
        let plan = parse_plan(
            r#"{"schema_version":1,"jobs":[
                 {"task":"任务1","every_s":600},
                 {"task":"任务2","every_s":60,"enabled":false}]}"#,
        )
        .expect("plan");
        let mut state = ScheduleState {
            schema_version: 1,
            jobs: vec![JobState {
                task: "任务1".to_string(),
                next_run_ms: 1000,
                ..JobState::default()
            }],
        };
        assert_eq!(due_jobs(&plan, &state, 999), Vec::<usize>::new());
        assert_eq!(due_jobs(&plan, &state, 1000), vec![0]);
        advance(&mut state, &plan.jobs[0], 5000, "done");
        let st = state_for(&state, "任务1").expect("state");
        assert_eq!(st.next_run_ms, 5000 + 600_000);
        assert_eq!(st.runs, 1);
        assert_eq!(st.failures, 0);
        assert_eq!(due_jobs(&plan, &state, 5000), Vec::<usize>::new());
    }

    #[test]
    fn once_runs_a_due_job_then_respects_state_on_restart() {
        let td = tmpdir("once");
        let task = td.join("任务1");
        let replay = td.join("replay");
        let ok_flow = r#"{"schema_version":1,"name":"ok","steps":[
             {"op":"click_point","args":{"x":10,"y":10}},
             {"op":"finish","args":{"exit":"done"}}]}"#;
        write_task(&task, ok_flow);
        write_replay(&replay);
        let plan_path = td.join("schedule.json");
        std::fs::write(
            &plan_path,
            r#"{"schema_version":1,"jobs":[{"task":"任务1","every_s":3600}]}"#,
        )
        .expect("plan file");
        let report_path = td.join("report.json");
        let mut opts = ScheduleOptions {
            plan: plan_path.clone(),
            tasks_dir: td.clone(),
            once: true,
            max_runs: Some(1),
            replay: Some(replay),
            report: Some(report_path.clone()),
            ..ScheduleOptions::default()
        };
        let mut lines = Vec::new();
        let r1 = run_schedule(&opts, &mut |l| lines.push(l.to_string()))
            .expect("first schedule run");
        assert_eq!(r1.runs, 1, "{r1:?}");
        assert_eq!(r1.successes, 1, "{r1:?}");
        assert!(lines.iter().any(|l| l.contains("schedule_job task=任务1")));
        assert!(report_path.exists());
        let state_path = plan_path.with_extension("json.state.json");
        assert!(state_path.exists(), "state file must be persisted");

        // Second --once scan: state says next run is an hour away => runs 0.
        opts.report = None;
        let r2 = run_schedule(&opts, &mut |_| {}).expect("second schedule run");
        assert_eq!(r2.runs, 0, "job must not be re-run before its period: {r2:?}");
        std::fs::remove_dir_all(&td).ok();
    }
}
