//! Task queue / supervisor (ENGINE §22.17 纵向③).
//!
//! Runs the same `oasd run` pipeline over an ordered list of task bundles:
//! per-task attempts with backoff, optional checkpoint resume on retry, rounds
//! for long-running supervision, and a machine-readable report.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use oas_types::Outcome;

use crate::run::{self, RunOptions};

pub const USAGE: &str = "usage: oasd queue --tasks <dir1>,<dir2>,… [--replay <dir>] \
                         [--max-attempts N] [--between-ms M] [--rounds N] \
                         [--report <file.json>] [--clicks-dir <dir>] [--shots-dir <dir>] \
                         [--seed <u64>] [--fail-fast] [--no-resume-on-retry] \
                         [--model <rec.onnx>] [--det-model <det.onnx>]";

#[derive(Debug, Clone)]
pub struct QueueOptions {
    pub tasks: Vec<PathBuf>,
    pub replay: Option<PathBuf>,
    /// Attempts per task (>= 1).
    pub max_attempts: u32,
    pub between_ms: u64,
    pub rounds: u32,
    pub report: Option<PathBuf>,
    pub clicks_dir: Option<PathBuf>,
    pub shots_dir: Option<PathBuf>,
    pub seed: Option<u64>,
    pub fail_fast: bool,
    /// On retry, resume from the task's checkpoint when one exists.
    pub resume_on_retry: bool,
    pub model: Option<PathBuf>,
    pub det_model: Option<PathBuf>,
}

impl Default for QueueOptions {
    fn default() -> Self {
        Self {
            tasks: Vec::new(),
            replay: None,
            max_attempts: 1,
            between_ms: 500,
            rounds: 1,
            report: None,
            clicks_dir: None,
            shots_dir: None,
            seed: None,
            fail_fast: false,
            resume_on_retry: true,
            model: None,
            det_model: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub task: String,
    pub round: u32,
    pub attempts: u32,
    pub outcome: String,
    pub exit_code: u8,
    pub clicks: usize,
    pub misses: usize,
    pub started_ms: u128,
    pub finished_ms: u128,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueReport {
    pub schema_version: u32,
    pub started_ms: u128,
    pub finished_ms: u128,
    pub results: Vec<TaskResult>,
}

impl QueueReport {
    pub fn all_done(&self) -> bool {
        !self.results.is_empty() && self.results.iter().all(|r| r.outcome == "done")
    }

    pub fn failed_count(&self) -> usize {
        self.results.iter().filter(|r| r.outcome != "done").count()
    }
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn take_value(rest: &[String], i: &mut usize, flag: &str) -> Result<String, String> {
    if *i + 1 >= rest.len() {
        return Err(format!("{flag} requires a value"));
    }
    *i += 2;
    Ok(rest[*i - 1].clone())
}

impl QueueOptions {
    pub fn parse(rest: &[String]) -> Result<Self, String> {
        let mut o = Self::default();
        let mut i = 0usize;
        while i < rest.len() {
            match rest[i].as_str() {
                "--tasks" => {
                    let v = take_value(rest, &mut i, "--tasks")?;
                    o.tasks = v
                        .split(',')
                        .map(|s| s.trim())
                        .filter(|s| !s.is_empty())
                        .map(PathBuf::from)
                        .collect();
                    if o.tasks.is_empty() {
                        return Err("--tasks needs at least one bundle directory".into());
                    }
                }
                "--replay" => o.replay = Some(PathBuf::from(take_value(rest, &mut i, "--replay")?)),
                "--max-attempts" => {
                    let v = take_value(rest, &mut i, "--max-attempts")?;
                    let n: u32 = v
                        .parse()
                        .map_err(|_| format!("--max-attempts must be an integer, got `{v}`"))?;
                    if n == 0 || n > 20 {
                        return Err(format!("--max-attempts must be 1..=20, got {n}"));
                    }
                    o.max_attempts = n;
                }
                "--between-ms" => {
                    let v = take_value(rest, &mut i, "--between-ms")?;
                    o.between_ms = v
                        .parse()
                        .map_err(|_| format!("--between-ms must be an integer, got `{v}`"))?;
                }
                "--rounds" => {
                    let v = take_value(rest, &mut i, "--rounds")?;
                    let n: u32 = v
                        .parse()
                        .map_err(|_| format!("--rounds must be an integer, got `{v}`"))?;
                    if n == 0 {
                        return Err("--rounds must be >= 1".into());
                    }
                    o.rounds = n;
                }
                "--report" => o.report = Some(PathBuf::from(take_value(rest, &mut i, "--report")?)),
                "--clicks-dir" => {
                    o.clicks_dir = Some(PathBuf::from(take_value(rest, &mut i, "--clicks-dir")?))
                }
                "--shots-dir" => {
                    o.shots_dir = Some(PathBuf::from(take_value(rest, &mut i, "--shots-dir")?))
                }
                "--seed" => {
                    let v = take_value(rest, &mut i, "--seed")?;
                    o.seed = Some(
                        v.parse()
                            .map_err(|_| format!("--seed must be a u64, got `{v}`"))?,
                    );
                }
                "--fail-fast" => {
                    o.fail_fast = true;
                    i += 1;
                }
                "--no-resume-on-retry" => {
                    o.resume_on_retry = false;
                    i += 1;
                }
                "--model" => o.model = Some(PathBuf::from(take_value(rest, &mut i, "--model")?)),
                "--det-model" => {
                    o.det_model = Some(PathBuf::from(take_value(rest, &mut i, "--det-model")?))
                }
                other => {
                    return Err(format!(
                        "unknown option `{other}`\n{USAGE}"
                    ))
                }
            }
        }
        if o.tasks.is_empty() {
            return Err(format!("--tasks is required\n{USAGE}"));
        }
        Ok(o)
    }
}

/// Run the queue once (all rounds) and return the report.
pub fn execute_queue(
    opts: &QueueOptions,
    out: &mut dyn FnMut(&str),
) -> Result<QueueReport, String> {
    let started = now_ms();
    let mut results: Vec<TaskResult> = Vec::new();
    if let Some(dir) = &opts.clicks_dir {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("--clicks-dir: create `{}` ({e})", dir.display()))?;
    }
    'rounds: for round in 0..opts.rounds {
        for task in &opts.tasks {
            let stem = task
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "task".to_string());
            let task_started = now_ms();
            let mut attempt = 0u32;
            let mut last_error: Option<String> = None;
            loop {
                attempt += 1;
                let seed = opts
                    .seed
                    .map(|s| s.saturating_add((attempt as u64).saturating_sub(1)));
                let clicks_out = opts.clicks_dir.as_ref().map(|dir| {
                    dir.join(format!("{stem}-r{round}-a{attempt}.clicks.jsonl"))
                });
                let resume = opts.resume_on_retry
                    && attempt > 1
                    && task.join("state").join("checkpoint.json").exists();
                let run_opts = RunOptions {
                    task: Some(task.clone()),
                    replay: opts.replay.clone(),
                    seed,
                    clicks_out,
                    shots_dir: opts.shots_dir.clone(),
                    resume,
                    model: opts.model.clone(),
                    det_model: opts.det_model.clone(),
                    ..RunOptions::default()
                };
                out(&format!(
                    "queue_task task={} round={round} attempt={attempt} resume={resume}",
                    task.display()
                ));
                match run::execute(&run_opts, out) {
                    Ok(report) => {
                        let done = report.outcome() == Outcome::Done;
                        let misses = report
                            .events
                            .iter()
                            .filter(|e| e.detail.contains("miss=true"))
                            .count();
                        out(&format!(
                            "queue_task_done task={} attempt={attempt} outcome={:?} clicks={} misses={misses}",
                            task.display(),
                            report.outcome(),
                            report.clicks.len()
                        ));
                        if done || attempt >= opts.max_attempts {
                            results.push(TaskResult {
                                task: task.display().to_string(),
                                round,
                                attempts: attempt,
                                outcome: format!("{:?}", report.outcome()).to_lowercase(),
                                exit_code: report.exit_code(),
                                clicks: report.clicks.len(),
                                misses,
                                started_ms: task_started,
                                finished_ms: now_ms(),
                                error: None,
                            });
                            break;
                        }
                        last_error = Some("not done".to_string());
                    }
                    Err(e) => {
                        out(&format!("queue_task_error task={} attempt={attempt}: {e}", task.display()));
                        last_error = Some(e);
                        results.push(TaskResult {
                            task: task.display().to_string(),
                            round,
                            attempts: attempt,
                            outcome: "failed".to_string(),
                            exit_code: 1,
                            clicks: 0,
                            misses: 0,
                            started_ms: task_started,
                            finished_ms: now_ms(),
                            error: last_error.clone(),
                        });
                        break;
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(opts.between_ms));
            }
            let _ = &last_error;
            if opts.fail_fast {
                if let Some(r) = results.last() {
                    if r.outcome != "done" {
                        out("queue_fail_fast=1");
                        break 'rounds;
                    }
                }
            }
            if opts.between_ms > 0 {
                std::thread::sleep(std::time::Duration::from_millis(opts.between_ms));
            }
        }
    }
    let report = QueueReport {
        schema_version: 1,
        started_ms: started,
        finished_ms: now_ms(),
        results,
    };
    if let Some(path) = &opts.report {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("--report: create `{}` ({e})", dir.display()))?;
        }
        let text = serde_json::to_string_pretty(&report)
            .map_err(|e| format!("--report: serialize ({e})"))?;
        std::fs::write(path, text)
            .map_err(|e| format!("--report: write `{}` ({e})", path.display()))?;
        out(&format!("queue_report={}", path.display()));
    }
    out(&format!(
        "queue_done tasks={} failed={} all_done={}",
        report.results.len(),
        report.failed_count(),
        report.all_done()
    ));
    Ok(report)
}

pub fn queue_cmd(rest: &[String]) -> ExitCode {
    if rest.iter().any(|a| a == "-h" || a == "--help") {
        println!("{USAGE}");
        return ExitCode::from(2);
    }
    let opts = match QueueOptions::parse(rest) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("queue: {e}");
            return ExitCode::from(2);
        }
    };
    let mut print = |line: &str| println!("{line}");
    match execute_queue(&opts, &mut print) {
        Ok(report) => {
            if report.all_done() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        Err(e) => {
            eprintln!("queue failed: {e}");
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
            "oas-queue-{tag}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
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
        let mut rec = Recorder::create(dir, "queue-test").expect("recorder");
        rec.capture_frame(&frame(10)).expect("f0");
        rec.capture_frame(&frame(20)).expect("f1");
        rec.finish().expect("finish");
    }

    #[test]
    fn queue_runs_two_tasks_and_reports_done() {
        let td = tmpdir("ok");
        let a = td.join("a");
        let b = td.join("b");
        let replay = td.join("replay");
        let ok_flow = r#"{"schema_version":1,"name":"ok","steps":[
             {"op":"click_point","args":{"x":10,"y":10}},
             {"op":"finish","args":{"exit":"done"}}]}"#;
        write_task(&a, ok_flow);
        write_task(&b, ok_flow);
        write_replay(&replay);
        let opts = QueueOptions {
            tasks: vec![a.clone(), b.clone()],
            replay: Some(replay),
            report: Some(td.join("report.json")),
            clicks_dir: Some(td.join("clicks")),
            between_ms: 0,
            ..QueueOptions::default()
        };
        let mut lines = Vec::new();
        let report = execute_queue(&opts, &mut |l| lines.push(l.to_string())).expect("queue");
        assert_eq!(report.results.len(), 2);
        assert!(report.all_done(), "{report:?}");
        assert!(lines.iter().any(|l| l.contains("queue_task task=")));
        assert!(opts.report.as_ref().unwrap().exists());
        let raw = std::fs::read_to_string(opts.report.as_ref().unwrap()).unwrap();
        assert!(raw.contains("\"results\""));
        std::fs::remove_dir_all(&td).ok();
    }

    #[test]
    fn queue_retries_until_max_attempts_then_fails() {
        let td = tmpdir("fail");
        let task = td.join("bad");
        let replay = td.join("replay");
        let bad_flow = r#"{"schema_version":1,"name":"bad","steps":[
             {"op":"click_point","args":{"x":10000,"y":60}},
             {"op":"finish","args":{"exit":"done"}}]}"#;
        write_task(&task, bad_flow);
        write_replay(&replay);
        let opts = QueueOptions {
            tasks: vec![task],
            replay: Some(replay),
            max_attempts: 2,
            between_ms: 0,
            ..QueueOptions::default()
        };
        let mut lines = Vec::new();
        let report = execute_queue(&opts, &mut |l| lines.push(l.to_string())).expect("queue");
        assert_eq!(report.results.len(), 1);
        assert_eq!(report.results[0].attempts, 2, "max_attempts honored");
        assert_eq!(report.results[0].outcome, "failed");
        assert!(!report.all_done());
        assert!(lines.iter().any(|l| l.contains("queue_task_done")));
        std::fs::remove_dir_all(&td).ok();
    }
}
