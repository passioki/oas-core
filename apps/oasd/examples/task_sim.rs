//! Simulate a **whole task** N times offline and aggregate every click.
//!
//! ```text
//! cargo run -p oasd --example task_sim -- \
//!   --task <bundle> --replay <dir> --repeats 1000 --seed-base 1 --clicks-out <out.jsonl>
//! ```
//!
//! Why: the click-distribution view must answer "across 1000 runs of the SAME
//! task, where do the clicks land?" — one-ROI heatmaps are not enough. This runs
//! the **real interpreter** against a recorded bundle, one different seed per
//! run, and every successful click becomes one CD2 row (`x`,`y` actual +
//! `target_x`,`target_y` intent + `sel` + `seed` + `run`).
//!
//! The workflow is offline (`ReplaySource` + no-op input): nothing is tapped.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use oas_obs::ReplaySource;
use oas_runtime::humanize::Humanize;
use oas_runtime::{Budget, Interpreter, RunResult};
use oas_task::TaskBundle;
use oas_types::{Frame, InputSink, Point, ScreenSource};

/// Pre-decoded replay: every frame is decoded ONCE, then cloned per run. The
/// naive version (open `ReplaySource` per run) spends minutes re-decoding the
/// same 8 PNGs 1000 times.
struct VecScreen {
    frames: std::rc::Rc<Vec<Frame>>,
    idx: usize,
}

impl ScreenSource for VecScreen {
    fn grab(&mut self) -> Result<Frame, String> {
        match self.frames.get(self.idx) {
            Some(f) => {
                self.idx += 1;
                Ok(f.clone())
            }
            None => Err(format!(
                "replay exhausted ({} frame(s) consumed)",
                self.frames.len()
            )),
        }
    }
}

#[derive(Default)]
struct NoopInput;

impl InputSink for NoopInput {
    fn click(&mut self, _p: Point) -> Result<(), String> {
        Ok(())
    }
    fn swipe(&mut self, _from: Point, _to: Point, _ms: u32) -> Result<(), String> {
        Ok(())
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("task_sim: {e}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<(), String> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let mut task: Option<PathBuf> = None;
    let mut replay: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut repeats = 1000usize;
    let mut seed_base = 1u64;
    let mut shard = 0usize;
    let mut shards = 1usize;
    let mut i = 0usize;
    while i < a.len() {
        match a[i].as_str() {
            "--task" => task = Some(PathBuf::from(take(&a, &mut i, "--task")?)),
            "--replay" => replay = Some(PathBuf::from(take(&a, &mut i, "--replay")?)),
            "--clicks-out" => out = Some(PathBuf::from(take(&a, &mut i, "--clicks-out")?)),
            "--repeats" => {
                repeats = take(&a, &mut i, "--repeats")?
                    .parse()
                    .map_err(|_| "--repeats must be a positive integer".to_string())?
            }
            "--seed-base" => {
                seed_base = take(&a, &mut i, "--seed-base")?
                    .parse()
                    .map_err(|_| "--seed-base must be a u64".to_string())?
            }
            // Sharding: run k = shard, shard+shards, ... so N processes can cover
            // the same 1..repeats seed range in parallel (outputs are merged).
            "--shard" => {
                shard = take(&a, &mut i, "--shard")?
                    .parse()
                    .map_err(|_| "--shard must be an integer".to_string())?
            }
            "--shards" => {
                shards = take(&a, &mut i, "--shards")?
                    .parse()
                    .map_err(|_| "--shards must be a positive integer".to_string())?
            }
            "--miss-chance" => {
                return Err(
                    "--miss-chance was removed: [10-22] the user kept the bell only \
                     (no miss clicks)"
                        .into(),
                )
            }
            "--miss-px" => {
                return Err(
                    "--miss-px was removed: [10-22] the user kept the bell only \
                     (no miss clicks)"
                        .into(),
                )
            }
            other => {
                return Err(format!(
                    "unknown argument `{other}`\nusage: --task <bundle> --replay <dir> \
                     --repeats N --seed-base S --clicks-out <file> [--shard i --shards n]"
                ))
            }
        }
    }
    let task = task.ok_or("--task <bundle> is required")?;
    let replay = replay.ok_or("--replay <dir> is required")?;
    let out = out.ok_or("--clicks-out <file> is required")?;
    if repeats == 0 {
        return Err("--repeats must be > 0".into());
    }
    if shards == 0 || shard >= shards {
        return Err("--shards must be > 0 and --shard must be in 0..shards".into());
    }

    let bundle = TaskBundle::open(&task).map_err(|e| format!("task bundle: {e}"))?;
    let flow = bundle
        .load_flow()
        .map_err(|e| format!("task bundle: {e}"))?
        .ok_or("task bundle has no flow.json")?;

    let assets = bundle
        .load_runtime_assets()
        .map_err(|e| format!("assets: {e}"))?;

    // Decode the recorded frames ONCE; every run gets a fresh cursor over them.
    let mut replay_src = ReplaySource::open(&replay).map_err(|e| format!("replay: {e}"))?;
    let mut frames: Vec<Frame> = Vec::new();
    loop {
        match replay_src.grab() {
            Ok(f) => frames.push(f),
            Err(_) => break,
        }
    }
    if frames.is_empty() {
        return Err("replay has no frames".into());
    }
    let frames = std::rc::Rc::new(frames);

    let mut file = std::fs::File::create(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let mut outcomes: BTreeMap<String, usize> = BTreeMap::new();
    let mut per_sel: BTreeMap<String, (usize, BTreeSet<(i32, i32)>)> = BTreeMap::new();
    let mut all_points: BTreeSet<(i32, i32)> = BTreeSet::new();
    let mut total_clicks = 0usize;

    for k in (shard..repeats).step_by(shards) {
        let seed = seed_base.wrapping_add(k as u64);
        let mut interp = Interpreter::new(
            VecScreen {
                frames: frames.clone(),
                idx: 0,
            },
            NoopInput,
            assets.clone(),
            Budget::default(),
        )
        .with_seed(seed)
        // HB3 sleeps would cost ~500ms per run; a distribution simulation only
        // needs HB1/HB2, which stay exactly as in production.
        .with_humanize(Humanize {
            delay_min_ms: 0,
            delay_max_ms: 0,
            ..Humanize::default()
        });
        let result = interp.run(&flow);
        let key = match &result {
            RunResult::Finished(_) => "Done".to_string(),
            RunResult::Exhausted(m) => format!("Exhausted({m})"),
            RunResult::Failed(m) => format!("Failed({m})"),
        };
        *outcomes.entry(key).or_insert(0) += 1;

        for c in interp.clicks() {
            total_clicks += 1;
            let sel = c.sel.clone().unwrap_or_else(|| "<click_point>".to_string());
            let e = per_sel.entry(sel.clone()).or_insert_with(|| (0, BTreeSet::new()));
            e.0 += 1;
            e.1.insert((c.x, c.y));
            all_points.insert((c.x, c.y));
            writeln!(
                file,
                "{{\"ts_ms\":{},\"x\":{},\"y\":{},\"target_x\":{},\"target_y\":{},\"sel\":{},\"seed\":{},\"run\":{}}}",
                c.ts_ms,
                c.x,
                c.y,
                c.target_x,
                c.target_y,
                json_opt(&c.sel),
                c.seed,
                k
            )
            .map_err(|e| format!("write {}: {e}", out.display()))?;
        }
        if k % 100 == 0 {
            eprintln!("run {k}/{repeats} …");
        }
    }

    println!("task={}", task.display());
    println!("repeats={repeats} seed_base={seed_base}");
    println!("clicks_total={total_clicks} distinct_pixels={}", all_points.len());
    for (k, v) in &outcomes {
        println!("outcome[{k}]={v}");
    }
    for (sel, (n, pts)) in &per_sel {
        println!("sel[{sel}]: clicks={n} distinct={}", pts.len());
    }
    println!("out={}", out.display());
    Ok(())
}

fn take(a: &[String], i: &mut usize, flag: &str) -> Result<String, String> {
    let v = a.get(*i + 1).ok_or_else(|| format!("{flag} needs a value"))?;
    *i += 2;
    Ok(v.clone())
}

fn json_opt(sel: &Option<String>) -> String {
    match sel {
        None => "null".to_string(),
        Some(s) => format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),
    }
}
