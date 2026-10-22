//! Long-run statistics & alerts (ENGINE §22.19 纵向⑤).
//!
//! Aggregates the artifacts a supervisor run already leaves on disk:
//!   * `*.clicks.jsonl`  — CD2 click audit (clicks / misses / locators)
//!   * `*.events.jsonl`  — step events (failed steps / retries / popup dismissals)
//!   * queue report JSON — `{results:[{outcome, attempts, clicks, misses}]}`
//!   * `shots/*.png`     — failure screenshots
//!
//! Output: a JSON summary, an optional self-contained HTML dashboard, and
//! `ALERT` lines for the failure conditions.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde::{Deserialize, Serialize};

pub const USAGE: &str = "usage: oasd stats --runs <dir> [--report <file.json>] [--html <file.html>] \
                         [--alert-fail-rate 0.10] [--alert-failures 0]";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClickStats {
    pub files: u32,
    pub clicks: u64,
    pub misses: u64,
    pub distinct: u64,
    pub locators: BTreeMap<String, u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QueueStats {
    pub reports: u32,
    pub tasks: u32,
    pub done: u32,
    pub failed: u32,
    pub attempts: u64,
    pub total_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EventStats {
    pub files: u32,
    pub steps: u64,
    pub failed_steps: u64,
    pub retries: u64,
    pub popup_interrupts: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ShotStats {
    pub count: u32,
    pub files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Summary {
    pub schema_version: u32,
    pub generated_ms: u128,
    pub runs_dir: String,
    pub clicks: ClickStats,
    pub queue: QueueStats,
    pub events: EventStats,
    pub shots: ShotStats,
    pub alerts: Vec<String>,
}

impl Summary {
    pub fn alert_count(&self) -> usize {
        self.alerts.len()
    }
}

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn read_lines(path: &Path) -> Vec<serde_json::Value> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .collect()
}

/// Aggregate every artifact under `runs_dir`.
pub fn collect(runs_dir: &Path) -> Result<Summary, String> {
    let entries = std::fs::read_dir(runs_dir).map_err(|e| {
        format!(
            "runs dir `{}`: {e} (pass --runs <dir>)",
            runs_dir.display()
        )
    })?;
    let mut s = Summary {
        schema_version: 1,
        generated_ms: now_ms(),
        runs_dir: runs_dir.display().to_string(),
        ..Summary::default()
    };
    let mut distinct: BTreeSet<(i64, i64)> = BTreeSet::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            continue;
        }
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        if name.ends_with(".clicks.jsonl") {
            s.clicks.files += 1;
            for v in read_lines(&path) {
                s.clicks.clicks += 1;
                if v.get("miss").and_then(|m| m.as_bool()).unwrap_or(false) {
                    s.clicks.misses += 1;
                }
                if let (Some(x), Some(y)) = (
                    v.get("x").and_then(|x| x.as_i64()),
                    v.get("y").and_then(|y| y.as_i64()),
                ) {
                    distinct.insert((x, y));
                }
                let loc = v
                    .get("locator")
                    .and_then(|l| l.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                *s.clicks.locators.entry(loc).or_insert(0) += 1;
            }
        } else if name.ends_with(".events.jsonl") {
            s.events.files += 1;
            for v in read_lines(&path) {
                s.events.steps += 1;
                let result = v.get("result").and_then(|r| r.as_str()).unwrap_or("");
                let op = v.get("op").and_then(|o| o.as_str()).unwrap_or("");
                if result == "failed" {
                    s.events.failed_steps += 1;
                } else if result == "retry" {
                    s.events.retries += 1;
                }
                if op == "interrupt" {
                    s.events.popup_interrupts += 1;
                }
            }
        } else if name.ends_with(".json") {
            // Queue report: {schema_version, results:[{outcome, attempts, ...}]}
            if let Ok(text) = std::fs::read_to_string(&path) {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Some(results) = v.get("results").and_then(|r| r.as_array()) {
                        s.queue.reports += 1;
                        for r in results {
                            s.queue.tasks += 1;
                            match r.get("outcome").and_then(|o| o.as_str()) {
                                Some("done") => s.queue.done += 1,
                                _ => s.queue.failed += 1,
                            }
                            s.queue.attempts += r.get("attempts").and_then(|a| a.as_u64()).unwrap_or(1);
                            let start = r.get("started_ms").and_then(|x| x.as_u64()).unwrap_or(0);
                            let end = r.get("finished_ms").and_then(|x| x.as_u64()).unwrap_or(0);
                            s.queue.total_ms += end.saturating_sub(start);
                        }
                    }
                }
            }
        }
    }
    s.clicks.distinct = distinct.len() as u64;

    let shots_dir = runs_dir.join("shots");
    if let Ok(entries) = std::fs::read_dir(&shots_dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().map(|e| e == "png").unwrap_or(false) {
                s.shots.count += 1;
                s.shots
                    .files
                    .push(p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default());
            }
        }
        s.shots.files.sort();
    }
    Ok(s)
}

/// Compute alerts from the summary + thresholds.
pub fn alerts_for(mut s: Summary, alert_fail_rate: f64, alert_failures: u32) -> Summary {
    if s.clicks.clicks > 0 {
        let rate = s.clicks.misses as f64 / s.clicks.clicks as f64;
        if rate > alert_fail_rate {
            s.alerts.push(format!(
                "click miss rate {:.2}% > {:.2}%",
                rate * 100.0,
                alert_fail_rate * 100.0
            ));
        }
    }
    if s.queue.failed > alert_failures {
        s.alerts.push(format!(
            "queue failed tasks {} > {}",
            s.queue.failed, alert_failures
        ));
    }
    if s.events.failed_steps > 0 {
        s.alerts.push(format!(
            "{} failed step(s) in event logs",
            s.events.failed_steps
        ));
    }
    if s.shots.count > 0 {
        s.alerts.push(format!(
            "{} failure screenshot(s) present (see shots/)",
            s.shots.count
        ));
    }
    s
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Self-contained HTML dashboard.
pub fn render_html(s: &Summary) -> String {
    let mut loc_rows = String::new();
    for (k, v) in &s.clicks.locators {
        loc_rows.push_str(&format!("<tr><td>{}</td><td>{v}</td></tr>", esc(k)));
    }
    if loc_rows.is_empty() {
        loc_rows.push_str("<tr><td colspan=2>（无点击记录）</td></tr>");
    }
    let mut alerts = String::new();
    for a in &s.alerts {
        alerts.push_str(&format!("<li class=\"alert\">{}</li>", esc(a)));
    }
    if alerts.is_empty() {
        alerts.push_str("<li class=\"ok\">无告警</li>");
    }
    let mut shots = String::new();
    for f in &s.shots.files {
        shots.push_str(&format!("<li>{}</li>", esc(f)));
    }
    if shots.is_empty() {
        shots.push_str("<li>（无失败截图）</li>");
    }
    format!(
        r#"<!doctype html>
<html lang="zh"><meta charset="utf-8"><title>OAS 运行统计</title>
<style>
body{{background:#0f1016;color:#e8e8ee;font-family:-apple-system,Segoe UI,Microsoft YaHei,sans-serif;margin:24px}}
h1{{font-size:20px;margin:0 0 4px}} .muted{{color:#9aa0aa;font-size:12px}}
table{{border-collapse:collapse;margin:8px 0 18px;min-width:420px}}
th,td{{border:1px solid #2a2d38;padding:6px 12px;text-align:left;font-size:13px}}
th{{background:#1a1d27;color:#ffd54a}}
.cards{{display:flex;gap:12px;flex-wrap:wrap}}
.card{{background:#171a23;border:1px solid #2a2d38;border-radius:8px;padding:10px 14px;min-width:150px}}
.card .v{{font-size:22px;font-weight:600;color:#7ee0c0}}
.alert{{color:#ff6b6b}} .ok{{color:#7ee0c0}}
</style>
<h1>OAS 运行统计</h1>
<div class="muted">runs_dir: {} · generated_ms: {}</div>
<div class="cards">
<div class="card"><div class="muted">点击</div><div class="v">{}</div></div>
<div class="card"><div class="muted">手误 (miss)</div><div class="v">{}</div></div>
<div class="card"><div class="muted">distinct 点</div><div class="v">{}</div></div>
<div class="card"><div class="muted">队列任务 done/failed</div><div class="v">{}/{}</div></div>
<div class="card"><div class="muted">失败步骤</div><div class="v">{}</div></div>
<div class="card"><div class="muted">重试</div><div class="v">{}</div></div>
<div class="card"><div class="muted">失败截图</div><div class="v">{}</div></div>
</div>
<h2>告警</h2><ul>{}</ul>
<h2>点击定位分布 (locator)</h2>
<table><tr><th>locator</th><th>次数</th></tr>{}</table>
<h2>失败截图</h2><ul>{}</ul>
</html>"#,
        esc(&s.runs_dir),
        s.generated_ms,
        s.clicks.clicks,
        s.clicks.misses,
        s.clicks.distinct,
        s.queue.done,
        s.queue.failed,
        s.events.failed_steps,
        s.events.retries,
        s.shots.count,
        alerts,
        loc_rows,
        shots
    )
}

#[derive(Debug, Clone)]
pub struct StatsOptions {
    pub runs: PathBuf,
    pub report: Option<PathBuf>,
    pub html: Option<PathBuf>,
    pub alert_fail_rate: f64,
    pub alert_failures: u32,
}

impl Default for StatsOptions {
    fn default() -> Self {
        Self {
            runs: PathBuf::from("runs"),
            report: None,
            html: None,
            alert_fail_rate: 0.10,
            alert_failures: 0,
        }
    }
}

fn take(rest: &[String], i: &mut usize, flag: &str) -> Result<String, String> {
    if *i + 1 >= rest.len() {
        return Err(format!("{flag} requires a value"));
    }
    *i += 2;
    Ok(rest[*i - 1].clone())
}

impl StatsOptions {
    pub fn parse(rest: &[String]) -> Result<Self, String> {
        let mut o = Self::default();
        let mut i = 0usize;
        while i < rest.len() {
            match rest[i].as_str() {
                "--runs" => o.runs = PathBuf::from(take(rest, &mut i, "--runs")?),
                "--report" => o.report = Some(PathBuf::from(take(rest, &mut i, "--report")?)),
                "--html" => o.html = Some(PathBuf::from(take(rest, &mut i, "--html")?)),
                "--alert-fail-rate" => {
                    let v = take(rest, &mut i, "--alert-fail-rate")?;
                    o.alert_fail_rate = v
                        .parse()
                        .map_err(|_| format!("--alert-fail-rate must be a number, got `{v}`"))?;
                }
                "--alert-failures" => {
                    let v = take(rest, &mut i, "--alert-failures")?;
                    o.alert_failures = v
                        .parse()
                        .map_err(|_| format!("--alert-failures must be an integer, got `{v}`"))?;
                }
                other => return Err(format!("unknown option `{other}`\n{USAGE}")),
            }
        }
        Ok(o)
    }
}

pub fn stats_cmd(rest: &[String]) -> ExitCode {
    if rest.iter().any(|a| a == "-h" || a == "--help") {
        println!("{USAGE}");
        return ExitCode::from(2);
    }
    let opts = match StatsOptions::parse(rest) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("stats: {e}");
            return ExitCode::from(2);
        }
    };
    let summary = match collect(&opts.runs) {
        Ok(s) => alerts_for(s, opts.alert_fail_rate, opts.alert_failures),
        Err(e) => {
            eprintln!("stats: {e}");
            return ExitCode::from(2);
        }
    };
    println!(
        "clicks files={} clicks={} misses={} distinct={}",
        summary.clicks.files, summary.clicks.clicks, summary.clicks.misses, summary.clicks.distinct
    );
    println!(
        "queue reports={} tasks={} done={} failed={} attempts={} total_ms={}",
        summary.queue.reports,
        summary.queue.tasks,
        summary.queue.done,
        summary.queue.failed,
        summary.queue.attempts,
        summary.queue.total_ms
    );
    println!(
        "events files={} steps={} failed={} retries={} popups={}",
        summary.events.files,
        summary.events.steps,
        summary.events.failed_steps,
        summary.events.retries,
        summary.events.popup_interrupts
    );
    println!("shots count={}", summary.shots.count);
    for (k, v) in &summary.clicks.locators {
        println!("locator {k}={v}");
    }
    if let Some(path) = &opts.report {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        match serde_json::to_string_pretty(&summary) {
            Ok(text) => {
                if let Err(e) = std::fs::write(path, text) {
                    eprintln!("stats: write `{}`: {e}", path.display());
                    return ExitCode::from(2);
                }
                println!("report={}", path.display());
            }
            Err(e) => {
                eprintln!("stats: serialize: {e}");
                return ExitCode::from(2);
            }
        }
    }
    if let Some(path) = &opts.html {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Err(e) = std::fs::write(path, render_html(&summary)) {
            eprintln!("stats: write `{}`: {e}", path.display());
            return ExitCode::from(2);
        }
        println!("html={}", path.display());
    }
    if summary.alerts.is_empty() {
        println!("alerts=0");
        ExitCode::SUCCESS
    } else {
        for a in &summary.alerts {
            println!("ALERT {a}");
        }
        println!("alerts={}", summary.alerts.len());
        ExitCode::from(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "oas-stats-{tag}-{}-{}",
            std::process::id(),
            now_ms()
        ));
        std::fs::create_dir_all(&p).expect("tmpdir");
        p
    }

    #[test]
    fn aggregates_clicks_events_queue_and_shots() {
        let dir = tmpdir("agg");
        std::fs::write(
            dir.join("a.clicks.jsonl"),
            "{\"x\":1,\"y\":2,\"miss\":false,\"locator\":\"template\"}\n\
             {\"x\":3,\"y\":4,\"miss\":true,\"locator\":\"point\"}\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("b.events.jsonl"),
            "{\"op\":\"click\",\"result\":\"retry\"}\n\
             {\"op\":\"click\",\"result\":\"failed\"}\n\
             {\"op\":\"interrupt\",\"result\":\"ok\"}\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("queue.json"),
            r#"{"schema_version":1,"results":[
                {"outcome":"done","attempts":1,"started_ms":100,"finished_ms":200},
                {"outcome":"failed","attempts":2,"started_ms":200,"finished_ms":350}]}"#,
        )
        .unwrap();
        std::fs::create_dir_all(dir.join("shots")).unwrap();
        std::fs::write(dir.join("shots/fail-seed1-1.png"), b"x").unwrap();
        std::fs::write(dir.join("shots/fail-seed2-2.png"), b"x").unwrap();

        let s = collect(&dir).expect("collect");
        assert_eq!(s.clicks.files, 1);
        assert_eq!(s.clicks.clicks, 2);
        assert_eq!(s.clicks.misses, 1);
        assert_eq!(s.clicks.distinct, 2);
        assert_eq!(s.clicks.locators.get("template"), Some(&1));
        assert_eq!(s.events.steps, 3);
        assert_eq!(s.events.failed_steps, 1);
        assert_eq!(s.events.retries, 1);
        assert_eq!(s.events.popup_interrupts, 1);
        assert_eq!(s.queue.reports, 1);
        assert_eq!(s.queue.tasks, 2);
        assert_eq!(s.queue.done, 1);
        assert_eq!(s.queue.failed, 1);
        assert_eq!(s.queue.attempts, 3);
        assert_eq!(s.queue.total_ms, 250);
        assert_eq!(s.shots.count, 2);

        let s = alerts_for(s, 0.10, 0);
        assert_eq!(s.alerts.len(), 4, "{:?}", s.alerts);
        let html = render_html(&s);
        assert!(html.contains("OAS 运行统计"));
        assert!(html.contains("失败截图"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_runs_dir_is_an_actionable_error() {
        let e = collect(Path::new(r"Z:\definitely\no\such\runs")).unwrap_err();
        assert!(e.contains("runs dir"), "{e}");
    }

    #[test]
    fn no_alerts_when_clean() {
        let dir = tmpdir("clean");
        std::fs::write(
            dir.join("a.clicks.jsonl"),
            "{\"x\":1,\"y\":2,\"miss\":false,\"locator\":\"template\"}\n",
        )
        .unwrap();
        let s = alerts_for(collect(&dir).unwrap(), 0.10, 0);
        assert!(s.alerts.is_empty(), "{:?}", s.alerts);
        std::fs::remove_dir_all(&dir).ok();
    }
}
