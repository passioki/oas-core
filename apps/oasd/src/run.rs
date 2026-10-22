//! `oasd run` — execute one task bundle, live or offline, with a pinned seed
//! and machine-greppable output (N5 infrastructure).
//!
//! Design authority: `ENGINE-DESIGN.md` §6 (record/replay: assertions compare
//! the **event/action sequence**, never pixels), §19.3 (task bundle layout),
//! §20 HB5 (randomness must be seed-driven so a replay reproduces the click
//! sequence exactly) and §22.2 N5 (S1 run on device + replay).
//!
//! # CLI contract (the S1 judge greps these lines)
//!
//! ```text
//! seed=<n>                    always printed, also for live runs (HB5)
//! frame_source=live|replay    always printed
//! step seq=<n> depth=<n> op=<op> result=<ok|failed> elapsed_ms=<ms> detail=<detail>
//! outcome=Done|Failed|Exhausted reason=<...>
//! clicks=[(x,y),...]          the ACTUAL points handed to InputSink, in order
//! ```
//!
//! Extra informational lines: `task=`, `assets=`, `device=`, `replay_frames=`,
//! `input=`, `budget_steps=`/`budget_wall_s=`, `record=`, `record_dir=`,
//! `events_out=`, `clicks_out=<path> count=<N>`, `swipes=`, and `hint=` after a
//! failure (actionable next step).
//!
//! Exit code: `0` iff `outcome=Done`; `1` for `Failed`/`Exhausted`; `2` for
//! usage / setup errors (bad flags, missing bundle, no adb device, ...).
//!
//! # Modes
//!
//! * `--replay <dir>`: offline [`ReplaySource`] — **never** touches adb. Input
//!   is a dry sink that only records the points (a replay must not tap).
//! * live (default): first deduped adb device; input is `AdbInput` unless
//!   `--dry-run` (then the same dry sink is used, so a dry run shows what
//!   *would* be clicked without sending a single tap).
//! * `--record <dir>` (live only): wraps the screen source so **every** grab is
//!   persisted via `oas_obs::Recorder::capture_frame`; on finish the bundle is
//!   a frame-exact recording of this run and `meta.json` is stamped with the
//!   seed the Lead can replay with. Replaying it with the same seed must
//!   reproduce the identical click sequence (HB5 / §6).
//! * `--events-out <file.jsonl>`: one JSON object per [`StepEvent`].
//! * `--clicks-out <file.jsonl>`: the CD2 click audit (§21) — one
//!   `{ts_ms,x,y,target_x,target_y,sel,miss,locator,seed}` object per successful
//!   click, i.e. the data source of the always-on click-distribution view. Written
//!   even when the outcome is not `Done` (an empty file still witnesses "there were
//!   no clicks"), and the parent directory is created if missing. `locator` is
//!   `template` / `ocr_text` / `coord_memory` / `point` (ENGINE §22.11).

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::rc::Rc;

use oas_adapter_pc::Adb;
use oas_obs::{Recorder, ReplaySource};
use oas_runtime::{Budget, ClickRecord, Interpreter, RunResult, StepEvent};
use oas_task::TaskBundle;
use oas_types::{Frame, InputSink, Outcome, Point, ScreenSource};

pub const USAGE: &str = "usage: oasd run --task <bundle-dir> [--replay <bundle-dir>] \
                         [--seed <u64>] [--dry-run] [--record <bundle-dir>] \
                         [--events-out <file.jsonl>] [--clicks-out <file.jsonl>] \
                         [--model <rec.onnx>] [--det-model <det.onnx>] \
                         [--budget-steps N] [--budget-wall S]";

const DEFAULT_MAX_STEPS: u32 = 5000;
const DEFAULT_MAX_WALL_S: f64 = 300.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameSource {
    Live,
    Replay,
}

impl FrameSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::Replay => "replay",
        }
    }
}

/// Parsed `oasd run` arguments.
#[derive(Debug, Clone)]
pub struct RunOptions {
    pub task: Option<PathBuf>,
    pub replay: Option<PathBuf>,
    /// `None` / `Some(0)` => pick `now_ms()` (design §20: seed 0 = auto).
    pub seed: Option<u64>,
    pub dry_run: bool,
    pub record: Option<PathBuf>,
    pub events_out: Option<PathBuf>,
    pub clicks_out: Option<PathBuf>,
    /// PP-OCRv6 recogniser for in-flow `ocr()` (P1-c). Only meaningful with
    /// `--features ocr`; the feature-less build rejects it loudly.
    pub model: Option<PathBuf>,
    /// Text detector — required by `ocr_lines` assets (det + rec, §17.1).
    pub det_model: Option<PathBuf>,
    pub budget_steps: u32,
    pub budget_wall_s: f64,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            task: None,
            replay: None,
            seed: None,
            dry_run: false,
            record: None,
            events_out: None,
            clicks_out: None,
            model: None,
            det_model: None,
            budget_steps: DEFAULT_MAX_STEPS,
            budget_wall_s: DEFAULT_MAX_WALL_S,
        }
    }
}

impl RunOptions {
    fn parse(rest: &[String]) -> Result<Self, String> {
        let mut o = Self::default();
        let mut i = 0usize;
        while i < rest.len() {
            match rest[i].as_str() {
                "--task" => o.task = Some(PathBuf::from(take_value(rest, &mut i, "--task")?)),
                "--replay" => {
                    o.replay = Some(PathBuf::from(take_value(rest, &mut i, "--replay")?))
                }
                "--seed" => {
                    let v = take_value(rest, &mut i, "--seed")?;
                    o.seed = Some(v.parse::<u64>().map_err(|_| {
                        format!("--seed must be an unsigned integer (u64), got `{v}`")
                    })?);
                }
                "--dry-run" => {
                    o.dry_run = true;
                    i += 1;
                }
                "--record" => o.record = Some(PathBuf::from(take_value(rest, &mut i, "--record")?)),
                "--events-out" => {
                    o.events_out = Some(PathBuf::from(take_value(rest, &mut i, "--events-out")?))
                }
                "--clicks-out" => {
                    o.clicks_out = Some(PathBuf::from(take_value(rest, &mut i, "--clicks-out")?))
                }
                "--model" => o.model = Some(PathBuf::from(take_value(rest, &mut i, "--model")?)),
                "--det-model" => {
                    o.det_model = Some(PathBuf::from(take_value(rest, &mut i, "--det-model")?))
                }
                "--budget-steps" => {
                    let v = take_value(rest, &mut i, "--budget-steps")?;
                    let n = v
                        .parse::<u32>()
                        .map_err(|_| format!("--budget-steps must be a positive integer, got `{v}`"))?;
                    if n == 0 {
                        return Err("--budget-steps must be >= 1 (0 would exhaust instantly)".into());
                    }
                    o.budget_steps = n;
                }
                "--budget-wall" => {
                    let v = take_value(rest, &mut i, "--budget-wall")?;
                    let s = v
                        .parse::<f64>()
                        .map_err(|_| format!("--budget-wall must be a number of seconds, got `{v}`"))?;
                    if !s.is_finite() || s <= 0.0 {
                        return Err(format!("--budget-wall must be > 0, got `{v}`"));
                    }
                    o.budget_wall_s = s;
                }
                other => return Err(format!("unknown run argument `{other}`\n{USAGE}")),
            }
        }
        Ok(o)
    }
}

fn take_value(rest: &[String], i: &mut usize, flag: &str) -> Result<String, String> {
    let v = rest
        .get(*i + 1)
        .ok_or_else(|| format!("{flag} needs a value\n{USAGE}"))?;
    *i += 2;
    Ok(v.clone())
}

/// Attach the in-flow OCR provider when `--model` / `--det-model` are given.
///
/// Feature-gated on purpose: `ort`/`oas-ocr` must not leak into the default
/// build (ENGINE-DESIGN §22.4 "trait + feature 门控"). The feature-less twin
/// below rejects the flags loudly instead of ignoring them (a flow that needs OCR
/// would otherwise fail much later with a confusing message).
///
/// Both OCR paths are wired (§17.1): rec always; det too when `--det-model` is
/// passed, which is what `ocr_lines` assets need (det + rec).
#[cfg(feature = "ocr")]
fn attach_ocr<S: ScreenSource, I: InputSink>(
    interp: Interpreter<S, I>,
    model: Option<&Path>,
    det_model: Option<&Path>,
) -> Result<Interpreter<S, I>, String> {
    match (model, det_model) {
        (Some(rec), det) => {
            let mut provider = crate::ocr::OnnxOcr::from_file(rec)
                .map_err(|e| format!("--model {}: {e}", rec.display()))?;
            if let Some(det) = det {
                provider = provider
                    .with_det(det)
                    .map_err(|e| format!("--det-model {}: {e}", det.display()))?;
            }
            Ok(interp.with_ocr(Box::new(provider)))
        }
        (None, Some(det)) => Err(format!(
            "--det-model {} needs --model <rec.onnx> too \
             (det alone finds boxes, it cannot recognise text)",
            det.display()
        )),
        (None, None) => Ok(interp),
    }
}

#[cfg(not(feature = "ocr"))]
fn attach_ocr<S: ScreenSource, I: InputSink>(
    interp: Interpreter<S, I>,
    model: Option<&Path>,
    det_model: Option<&Path>,
) -> Result<Interpreter<S, I>, String> {
    if model.is_none() && det_model.is_none() {
        return Ok(interp);
    }
    Err(
        "--model/--det-model not supported: this build has no OCR \
         (rebuild with `--features ocr`) - refusing to ignore them"
            .into(),
    )
}

/// Structured result of one run (also what the unit tests assert on).
#[derive(Debug)]
#[allow(dead_code)] // fields are read by the tests; the binary only uses `exit_code()`
pub struct RunReport {
    pub seed: u64,
    pub frame_source: FrameSource,
    pub events: Vec<StepEvent>,
    pub result: RunResult,
    /// Actual points handed to the `InputSink`, in order (HB6 / §21 CD2).
    pub clicks: Vec<Point>,
}

impl RunReport {
    pub fn outcome(&self) -> Outcome {
        self.result.outcome()
    }

    /// 0 iff `Done`; 1 for `Failed`/`Exhausted`.
    pub fn exit_code(&self) -> u8 {
        match self.outcome() {
            Outcome::Done => 0,
            _ => 1,
        }
    }
}

/// CLI entry point: parse, execute, print. Exit 0 only for `Done`.
pub fn run_cmd(rest: &[String]) -> ExitCode {
    if rest.iter().any(|a| a == "-h" || a == "--help") {
        println!("{USAGE}");
        return ExitCode::from(2);
    }
    let opts = match RunOptions::parse(rest) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("run: {e}");
            return ExitCode::from(2);
        }
    };
    let mut print = |line: &str| println!("{line}");
    match execute(&opts, &mut print) {
        Ok(report) => ExitCode::from(report.exit_code()),
        Err(e) => {
            eprintln!("run failed: {e}");
            ExitCode::from(2)
        }
    }
}

/// Run once. Setup errors are `Err`; run outcomes (Done/Failed/Exhausted) are
/// part of the returned report so the caller can choose the exit code.
pub fn execute(opts: &RunOptions, out: &mut dyn FnMut(&str)) -> Result<RunReport, String> {
    let task_dir = opts
        .task
        .as_deref()
        .ok_or_else(|| format!("--task <bundle-dir> is required\n{USAGE}"))?;
    if opts.record.is_some() && opts.replay.is_some() {
        return Err("--record is live-only and cannot be combined with --replay — the replay \
                    bundle already contains frames; drop --record or --replay"
            .into());
    }

    // HB5: resolve + print the seed BEFORE anything that can fail, so even a
    // failed run tells you which seed to replay with.
    let seed = match opts.seed {
        Some(s) if s != 0 => s,
        _ => now_ms(),
    };
    let frame_source = if opts.replay.is_some() {
        FrameSource::Replay
    } else {
        FrameSource::Live
    };
    out(&format!("seed={seed}"));
    out(&format!("frame_source={}", frame_source.as_str()));

    // ---- task bundle (§19.3) ------------------------------------------------
    let bundle = TaskBundle::open(task_dir)
        .map_err(|e| format!("task bundle `{}`: {e}", task_dir.display()))?;
    let flow = bundle
        .load_flow()
        .map_err(|e| format!("task bundle `{}`: {e}", task_dir.display()))?
        .ok_or_else(|| {
            format!(
                "task bundle `{}` has no flow.json — build the flow in the editor \
                 (POST /task/save_flow) or pass the bundle directory that contains it",
                task_dir.display()
            )
        })?;
    let assets = bundle
        .load_runtime_assets()
        .map_err(|e| format!("task bundle `{}`: {e}", task_dir.display()))?;
    out(&format!("task={}", task_dir.display()));
    out(&format!("assets={}", assets.len()));

    // ---- coordinate memory for `click_target` (§22.11) ----------------------
    // Replay and dry runs are READ-ONLY: a replay must never pollute the task's
    // remembered points. The file lives next to the bundle, not in git.
    let coords = Rc::new(RefCell::new(crate::coords::FileCoordStore::open(
        &task_dir.join("state"),
        opts.replay.is_none() && !opts.dry_run,
    )));
    out(&format!(
        "coords={} writable={}",
        coords.borrow().len(),
        coords.borrow().is_writable()
    ));

    // ---- optional recorder (wraps the source; §6 record every frame) --------
    let recorder: Option<Rc<RefCell<Recorder>>> = match &opts.record {
        Some(dir) => {
            ensure_fresh_recording(dir)?;
            let rec = Recorder::create(
                dir,
                format!("oasd run task={} seed={seed}", task_dir.display()),
            )
            .map_err(|e| format!("record dir `{}`: {e}", dir.display()))?;
            out(&format!("record={}", dir.display()));
            Some(Rc::new(RefCell::new(rec)))
        }
        None => None,
    };

    // ---- screen source ------------------------------------------------------
    let mut live: Option<(Adb, String)> = None;
    let mut src: Box<dyn ScreenSource> = match &opts.replay {
        Some(replay_dir) => {
            let replay = ReplaySource::open(replay_dir).map_err(|e| {
                format!(
                    "replay bundle `{}`: {e} — record one with \
                     `oasd run --task <bundle> --record <dir>` first",
                    replay_dir.display()
                )
            })?;
            out(&format!("replay_frames={}", replay.len()));
            Box::new(replay)
        }
        None => {
            let adb = Adb::from_env();
            let devices = adb.devices().map_err(|e| {
                format!(
                    "no adb device via `{}` ({e}); attach the emulator or pass --replay <bundle>",
                    adb.exe.display()
                )
            })?;
            let dev = devices.first().ok_or_else(|| {
                format!(
                    "no adb device attached (ADB_PATH={}); attach the emulator or pass \
                     --replay <bundle>",
                    adb.exe.display()
                )
            })?;
            out(&format!("device={}", dev.serial));
            live = Some((adb.clone(), dev.serial.clone()));
            Box::new(adb.screen(&dev.serial))
        }
    };
    if let Some(rec) = &recorder {
        src = Box::new(RecordingScreen {
            inner: src,
            rec: rec.clone(),
        });
    }

    // ---- input sink ---------------------------------------------------------
    // Live non-dry runs send real taps; replay and --dry-run use a dry sink and
    // only log the points (so N5 replay/dry runs never touch the device).
    let clicks: Rc<RefCell<Vec<Point>>> = Rc::new(RefCell::new(Vec::new()));
    let swipes: Rc<RefCell<Vec<(Point, Point, u32)>>> = Rc::new(RefCell::new(Vec::new()));
    let inner_input: Option<Box<dyn InputSink>> = match (&live, opts.dry_run) {
        (Some((adb, serial)), false) => {
            out("input=adb");
            Some(Box::new(adb.input(serial)))
        }
        (Some(_), true) => {
            out("input=dry-run");
            None
        }
        (None, _) => {
            out("input=dry (replay)");
            None
        }
    };
    let input = LoggedInput {
        inner: inner_input,
        clicks: clicks.clone(),
        swipes: swipes.clone(),
    };

    // ---- run ----------------------------------------------------------------
    let budget = Budget {
        max_wall_s: opts.budget_wall_s,
        max_steps: opts.budget_steps,
    };
    out(&format!(
        "budget_steps={} budget_wall_s={}",
        budget.max_steps, budget.max_wall_s
    ));
    let interp = Interpreter::new(DynScreen(src), input, assets, budget).with_seed(seed);
    let interp = attach_ocr(interp, opts.model.as_deref(), opts.det_model.as_deref())?;
    let mut interp = interp.with_coord_store(Box::new(crate::coords::SharedStore(coords.clone())));
    out(&format!(
        "ocr={}",
        match (opts.model.as_deref(), opts.det_model.as_deref()) {
            (Some(rec), det) => {
                let det = det.map(|d| format!(" det={}", d.display())).unwrap_or_default();
                format!("on model={}{det}", rec.display())
            }
            (None, Some(_)) => "on (det only)".to_string(),
            (None, None) => "off".to_string(),
        }
    ));
    let result = interp.run(&flow);
    let events = interp.events().to_vec();
    // CD2 (§21): snapshot the click audit BEFORE the interpreter is dropped.
    // The rows are the interpreter's own report of what it actually clicked.
    let click_rows = interp.clicks().to_vec();
    drop(interp); // release the recording source's Rc clone before finishing
    // Persist learned points (no-op for replay/dry). A write failure is a loud
    // warning, not a run failure: the flow itself already succeeded.
    let flush_result = coords.borrow_mut().flush();
    match flush_result {
        Ok(()) => {
            let n = coords.borrow().len();
            out(&format!("coords_final={n}"));
        }
        Err(e) => out(&format!("coords_warn={e}")),
    }

    for e in &events {
        out(&format!(
            "step seq={} depth={} op={} result={} elapsed_ms={:.1} detail={}",
            e.seq, e.depth, e.op, e.result, e.elapsed_ms, e.detail
        ));
    }

    // ---- finish the recording bundle (frames + events + seed in meta) -------
    if let Some(rec) = recorder {
        {
            let mut r = rec.borrow_mut();
            for e in &events {
                r.event(
                    e.seq as u32 + 1,
                    &e.op,
                    &format!(
                        "result={} elapsed_ms={:.1} {}",
                        e.result, e.elapsed_ms, e.detail
                    ),
                );
            }
        }
        let rec = match Rc::try_unwrap(rec) {
            Ok(r) => r.into_inner(),
            Err(_) => {
                return Err("internal error: recorder still shared after the run".to_string())
            }
        };
        let meta = rec.finish().map_err(|e| format!("record finish: {e}"))?;
        let dir = opts
            .record
            .as_deref()
            .expect("recorder only exists when --record was given");
        stamp_meta(dir, seed, task_dir, frame_source.as_str())?;
        out(&format!("record_dir={} frames={}", dir.display(), meta.frames));
    }

    // ---- summary ------------------------------------------------------------
    let outcome = result.outcome();
    let reason = match &result {
        RunResult::Finished(exit) => format!("{exit:?}"),
        RunResult::Failed(m) | RunResult::Exhausted(m) => m.clone(),
    };
    out(&format!("outcome={} reason={}", outcome_str(outcome), reason));
    for hint in hints_for(&result) {
        out(&format!("hint={hint}"));
    }

    let clicked = clicks.borrow().clone();
    out(&format!("clicks={}", format_clicks(&clicked)));
    out(&format!(
        "misses={}",
        click_rows.iter().filter(|r| r.miss).count()
    ));
    out(&format!("swipes={}", swipes.borrow().len()));

    if let Some(path) = &opts.events_out {
        write_events_jsonl(path, &events)?;
        out(&format!(
            "events_out={} lines={}",
            path.display(),
            events.len()
        ));
    }

    // CD2 (§21): the persistent click-distribution input. Written even when the
    // outcome != Done on purpose: "0 clicks" is evidence too.
    if let Some(path) = &opts.clicks_out {
        write_clicks_jsonl(path, &click_rows)?;
        out(&format!(
            "clicks_out={} count={}",
            path.display(),
            click_rows.len()
        ));
    }

    Ok(RunReport {
        seed,
        frame_source,
        events,
        result,
        clicks: clicked,
    })
}

// ---------------------------------------------------------------------------
// Port wrappers
// ---------------------------------------------------------------------------

/// `Interpreter` is generic; these wrappers keep one concrete code path for all
/// four (live/replay) x (real/dry input) combinations.
struct DynScreen(Box<dyn ScreenSource>);

impl ScreenSource for DynScreen {
    fn grab(&mut self) -> Result<Frame, String> {
        self.0.grab()
    }
}

/// ScreenSource wrapper used by `--record`: persists **every** frame the
/// interpreter grabs, so the bundle is a frame-exact recording of this run.
struct RecordingScreen {
    inner: Box<dyn ScreenSource>,
    rec: Rc<RefCell<Recorder>>,
}

impl ScreenSource for RecordingScreen {
    fn grab(&mut self) -> Result<Frame, String> {
        let frame = self.inner.grab()?;
        self.rec.borrow_mut().capture_frame(&frame)?;
        Ok(frame)
    }
}

/// InputSink wrapper that records the ACTUAL points (HB6) and forwards them to
/// the real sink when there is one. With no inner sink it is the dry sink used
/// by `--replay` / `--dry-run`.
struct LoggedInput {
    inner: Option<Box<dyn InputSink>>,
    clicks: Rc<RefCell<Vec<Point>>>,
    swipes: Rc<RefCell<Vec<(Point, Point, u32)>>>,
}

impl InputSink for LoggedInput {
    fn click(&mut self, p: Point) -> Result<(), String> {
        self.clicks.borrow_mut().push(p);
        match &mut self.inner {
            Some(sink) => sink.click(p),
            None => Ok(()),
        }
    }

    fn swipe(&mut self, from: Point, to: Point, ms: u32) -> Result<(), String> {
        self.swipes.borrow_mut().push((from, to, ms));
        match &mut self.inner {
            Some(sink) => sink.swipe(from, to, ms),
            None => Ok(()),
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn outcome_str(o: Outcome) -> &'static str {
    match o {
        Outcome::Done => "Done",
        Outcome::Failed => "Failed",
        Outcome::Exhausted => "Exhausted",
    }
}

fn format_clicks(points: &[Point]) -> String {
    let inner: Vec<String> = points
        .iter()
        .map(|p| format!("({},{})", p.x, p.y))
        .collect();
    format!("[{}]", inner.join(","))
}

/// Actionable `hint=` lines: a failed S1 run should tell the operator the next
/// step without reading this source file.
fn hints_for(result: &RunResult) -> Vec<&'static str> {
    let msg = match result {
        RunResult::Finished(_) => return Vec::new(),
        RunResult::Failed(m) | RunResult::Exhausted(m) => m.as_str(),
    };
    let mut hints = Vec::new();
    if msg.contains("replay exhausted") {
        hints.push(
            "the replay bundle ran out of frames mid-flow — record a longer run with --record \
             (the recording only contains the grabs of the run that produced it)",
        );
    }
    if msg.contains("not declared") {
        hints.push(
            "flow.json references an asset id that assets.json does not declare — re-save the \
             asset/flow in the editor so save-time reference validation runs",
        );
    }
    if msg.contains("not found") && !msg.contains("replay") {
        hints.push(
            "a template selector did not match this frame — verify the ROI/template in the \
             editor (试识别) and that the game is on the expected screen",
        );
    }
    if msg.contains("outside the frame") {
        hints.push(
            "the flow computes a point outside the frame — fix that literal/expression in \
             flow.json (the interpreter refuses off-screen clicks)",
        );
    }
    hints
}

/// A recording must be a single run: silently appending/overwriting frames from
/// a previous run would corrupt the frame order the replay depends on.
fn ensure_fresh_recording(dir: &Path) -> Result<(), String> {
    let frames = dir.join("frames");
    let has_frames = std::fs::read_dir(&frames)
        .map(|mut it| it.next().is_some())
        .unwrap_or(false);
    if has_frames {
        return Err(format!(
            "record dir `{}` already contains frames — a recording must be one run; \
             delete it or pick a new directory",
            dir.display()
        ));
    }
    Ok(())
}

/// Add the run's seed (HB5) + the replay command to the bundle's `meta.json`.
///
/// `oas_obs::Meta` is the on-disk contract for record/replay and is deliberately
/// not widened here; unknown JSON fields are ignored by `read_meta`, so a
/// replay of this bundle can recover the exact seed.
fn stamp_meta(dir: &Path, seed: u64, task: &Path, frame_source: &str) -> Result<(), String> {
    let path = dir.join("meta.json");
    let text =
        std::fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let mut v: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("parse {}: {e}", path.display()))?;
    let obj = v
        .as_object_mut()
        .ok_or_else(|| format!("{}: expected a JSON object", path.display()))?;
    obj.insert("seed".to_string(), serde_json::json!(seed));
    obj.insert(
        "task".to_string(),
        serde_json::json!(task.display().to_string()),
    );
    obj.insert(
        "frame_source".to_string(),
        serde_json::json!(frame_source),
    );
    obj.insert(
        "replay_hint".to_string(),
        serde_json::json!(format!(
            "oasd run --task {} --replay {} --seed {seed}",
            task.display(),
            dir.display()
        )),
    );
    let pretty = serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?;
    std::fs::write(&path, format!("{pretty}\n"))
        .map_err(|e| format!("write {}: {e}", path.display()))
}

/// JSONL: one JSON object per [`StepEvent`] (the click-distribution tooling and
/// the §6 sequence assertions read this stream).
fn write_events_jsonl(path: &Path, events: &[StepEvent]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("create {}: {e}", parent.display()))?;
        }
    }
    let mut text = String::new();
    for e in events {
        let v = serde_json::json!({
            "seq": e.seq,
            "depth": e.depth,
            "op": e.op,
            "result": e.result,
            "elapsed_ms": e.elapsed_ms,
            "detail": e.detail,
        });
        text.push_str(&v.to_string());
        text.push('\n');
    }
    std::fs::write(path, text).map_err(|e| format!("write {}: {e}", path.display()))
}

/// JSONL click audit (CD2, §21): one object per successful click. The field set
/// is exactly [`ClickRecord`]'s (the §21 row `ts_ms,x,y,sel,seed` plus the
/// audit columns `target_x,target_y` that §19 CP4/HB6 requires). Written to the
/// parent directory, created on demand — a run must never be lost because the
/// output folder did not exist yet.
fn write_clicks_jsonl(path: &Path, rows: &[ClickRecord]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| {
                format!(
                    "--clicks-out: cannot create directory `{}` for `{}` ({e}) — \
                     pass a path in an existing writable directory",
                    parent.display(),
                    path.display()
                )
            })?;
        }
    }
    let mut text = String::new();
    for r in rows {
        // `sel` is a validated asset id, but go through serde_json so any
        // future id (Chinese, quotes) can never produce a broken JSONL line.
        let sel = match &r.sel {
            Some(s) => serde_json::to_string(s)
                .map_err(|e| format!("--clicks-out: serialize sel: {e}"))?,
            None => "null".to_string(),
        };
        text.push_str(&format!(
            r#"{{"ts_ms":{},"x":{},"y":{},"target_x":{},"target_y":{},"sel":{},"miss":{},"locator":{},"seed":{}}}"#,
            r.ts_ms,
            r.x,
            r.y,
            r.target_x,
            r.target_y,
            sel,
            r.miss,
            serde_json::to_string(&r.locator)
                .map_err(|e| format!("--clicks-out: serialize locator: {e}"))?,
            r.seed
        ));
        text.push('\n');
    }
    std::fs::write(path, text).map_err(|e| {
        format!(
            "--clicks-out: cannot write `{}` ({e}) — check that the path is writable \
             and is not a directory",
            path.display()
        )
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::sync::atomic::{AtomicU64, Ordering};

    const FRAME_W: u32 = 200;
    const FRAME_H: u32 = 120;
    /// Literal target of the fixture click: jitter must stay within ±6 px.
    const TARGET: Point = Point { x: 100, y: 60 };

    /// click_point literal + log + finish (the N5 unit fixture from task-3).
    const TEST_FLOW: &str = r#"{"schema_version":1,"name":"n5-run-test","steps":[
        {"op":"click_point","args":{"x":100,"y":60}},
        {"op":"log","args":{"level":"info","text":"'after click'"}},
        {"op":"finish","args":{"exit":"done"}}]}"#;

    // ---- fixture helpers ----------------------------------------------------

    static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

    /// Self-cleaning temp dir (no `tempfile` dependency).
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "oasd-run-test-{}-{tag}-{}",
                std::process::id(),
                TEST_SEQ.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).expect("create temp dir");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Deterministic, non-flat RGBA frame (flat frames are unusable for NCC).
    fn frame(variant: u8) -> Frame {
        let (w, h) = (FRAME_W as usize, FRAME_H as usize);
        let mut rgba = vec![0u8; w * h * 4];
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) * 4;
                rgba[i] = ((x * 3 + y * 7 + variant as usize * 11) % 251) as u8;
                rgba[i + 1] = ((x * 5 + y * 3) % 241) as u8;
                rgba[i + 2] = ((x ^ y) as u8).wrapping_add(variant);
                rgba[i + 3] = 255;
            }
        }
        Frame::new(FRAME_W, FRAME_H, rgba)
    }

    /// Task bundle: a real `flow.json` written through the real N3 save path.
    fn write_task_bundle(dir: &Path) {
        let bundle = TaskBundle::open(dir).expect("open task bundle");
        bundle.save_flow(TEST_FLOW).expect("save flow");
    }

    /// Replay bundle: 2 PNG frames persisted with the new `capture_frame` API.
    fn write_replay_bundle(dir: &Path) {
        let mut rec = Recorder::create(dir, "oasd-run-test").expect("create replay bundle");
        rec.capture_frame(&frame(1)).expect("frame 0");
        rec.capture_frame(&frame(2)).expect("frame 1");
        rec.finish().expect("finish replay bundle");
    }

    fn fixture(tag: &str) -> (TempDir, PathBuf, PathBuf) {
        let td = TempDir::new(tag);
        let task = td.path().join("task");
        let replay = td.path().join("replay");
        write_task_bundle(&task);
        write_replay_bundle(&replay);
        (td, task, replay)
    }

    fn replay_options(task: &Path, replay: &Path, seed: u64) -> RunOptions {
        RunOptions {
            task: Some(task.to_path_buf()),
            replay: Some(replay.to_path_buf()),
            seed: Some(seed),
            ..RunOptions::default()
        }
    }

    fn run_capture(opts: &RunOptions) -> (RunReport, Vec<String>) {
        let mut lines = Vec::new();
        let report = execute(opts, &mut |l: &str| lines.push(l.to_string()))
            .unwrap_or_else(|e| panic!("run should succeed: {e}"));
        (report, lines)
    }

    fn line_with<'a>(lines: &'a [String], prefix: &str) -> &'a str {
        lines
            .iter()
            .find(|l| l.starts_with(prefix))
            .map(|s| s.as_str())
            .unwrap_or_else(|| panic!("no line starting with `{prefix}` in {lines:?}"))
    }

    // ---- tests --------------------------------------------------------------

    #[test]
    fn same_seed_replay_produces_identical_actual_clicks() {
        // HB5 / §6: replay with the SAME seed must reproduce the action
        // sequence exactly. This is the N5 judge's offline half.
        let (_td, task, replay) = fixture("seed-exact");
        let (r1, lines1) = run_capture(&replay_options(&task, &replay, 42));
        let (r2, lines2) = run_capture(&replay_options(&task, &replay, 42));
        assert_eq!(line_with(&lines2, "seed="), "seed=42");

        assert_eq!(r1.outcome(), Outcome::Done, "events: {:?}", r1.events);
        assert_eq!(r1.exit_code(), 0);
        assert_eq!(r1.clicks, r2.clicks, "same seed must replay the same points");
        assert_eq!(r1.clicks.len(), 1, "the fixture clicks exactly once");

        // §6: assert the EVENT/ACTION SEQUENCE, never pixels.
        let seq = |r: &RunReport| -> Vec<(usize, String, String, String)> {
            r.events
                .iter()
                .map(|e| (e.seq, e.op.clone(), e.result.clone(), e.detail.clone()))
                .collect()
        };
        assert_eq!(seq(&r1), seq(&r2), "same seed must replay the same events");

        // Machine-greppable anchors (the contract the Lead greps).
        assert_eq!(line_with(&lines1, "seed="), "seed=42");
        assert_eq!(
            line_with(&lines1, "frame_source="),
            "frame_source=replay"
        );
        let step_lines: Vec<&String> = lines1
            .iter()
            .filter(|l| l.starts_with("step seq="))
            .collect();
        assert_eq!(step_lines.len(), r1.events.len());
        for (line, e) in step_lines.iter().zip(&r1.events) {
            assert!(
                line.starts_with(&format!(
                    "step seq={} depth={} op={} result={} elapsed_ms=",
                    e.seq, e.depth, e.op, e.result
                )),
                "bad step line format: {line}"
            );
            assert!(line.ends_with(&format!("detail={}", e.detail)), "{line}");
        }
        assert!(lines1.iter().any(|l| l == "outcome=Done reason=Done"));
        assert_eq!(
            line_with(&lines1, "clicks="),
            format!("clicks={}", format_clicks(&r1.clicks))
        );
    }

    #[test]
    fn actual_clicks_stay_within_jitter_and_inside_the_frame() {
        let (_td, task, replay) = fixture("bounded");
        let (r, _lines) = run_capture(&replay_options(&task, &replay, 7));
        assert_eq!(r.outcome(), Outcome::Done);
        assert_eq!(r.clicks.len(), 1);
        let p = r.clicks[0];
        assert!(
            (p.x - TARGET.x).abs() <= 6 && (p.y - TARGET.y).abs() <= 6,
            "HB2: {p:?} is not within ±6 px of {TARGET:?}"
        );
        assert!(
            (0..FRAME_W as i32).contains(&p.x) && (0..FRAME_H as i32).contains(&p.y),
            "actual point {p:?} must stay inside the {FRAME_W}x{FRAME_H} frame"
        );
    }

    #[test]
    fn different_seeds_keep_the_target_but_move_the_point() {
        // Deterministic property (fixed seed list => no flakiness): the target
        // stays the same, the anti-ban jitter does not collapse to one point.
        // HB7 allows 1~3% misses, so a missed click is skipped here (its point is
        // deliberately outside the jitter neighbourhood).
        let (_td, task, replay) = fixture("seeds");
        let mut points = Vec::new();
        for seed in 1..=8u64 {
            let (r, _lines) = run_capture(&replay_options(&task, &replay, seed));
            assert_eq!(r.outcome(), Outcome::Done, "seed {seed}");
            assert_eq!(r.clicks.len(), 1, "seed {seed}");
            let missed = r
                .events
                .iter()
                .any(|e| e.op == "click_point" && e.detail.contains("miss=true"));
            if missed {
                continue;
            }
            let p = r.clicks[0];
            assert!(
                (p.x - TARGET.x).abs() <= 6 && (p.y - TARGET.y).abs() <= 6,
                "seed {seed}: {p:?} left the ±6 px neighbourhood of {TARGET:?}"
            );
            assert!(
                (0..FRAME_W as i32).contains(&p.x) && (0..FRAME_H as i32).contains(&p.y),
                "seed {seed}: {p:?} left the frame"
            );
            points.push((p.x, p.y));
        }
        let distinct: HashSet<(i32, i32)> = points.iter().copied().collect();
        assert!(
            distinct.len() >= 2,
            "seeds produced a single non-miss point {points:?} — jitter is not seed-driven"
        );
    }

    #[test]
    fn replay_mode_never_touches_the_device() {
        // Point ADB_PATH at a path that cannot exist: if the replay path spawns
        // adb at all, this run fails. (The Lead's rule: replay/dry tests must
        // not tap the live MuMu.)
        let (_td, task, replay) = fixture("no-device");
        let saved = std::env::var_os("ADB_PATH");
        let decoy = _td.path().join("definitely-not-adb.exe");
        std::env::set_var("ADB_PATH", &decoy);
        let result = execute(&replay_options(&task, &replay, 5), &mut |_: &str| {});
        match saved {
            Some(v) => std::env::set_var("ADB_PATH", v),
            None => std::env::remove_var("ADB_PATH"),
        }
        let report = result.expect("replay must not need adb");
        assert_eq!(report.outcome(), Outcome::Done);
        assert!(!decoy.exists());
    }

    #[test]
    fn events_out_is_one_json_object_per_step() {
        let (td, task, replay) = fixture("jsonl");
        let out_path = td.path().join("out/events.jsonl");
        let mut opts = replay_options(&task, &replay, 11);
        opts.events_out = Some(out_path.clone());
        let (r, lines) = run_capture(&opts);

        let text = std::fs::read_to_string(&out_path).expect("events.jsonl written");
        let rows: Vec<serde_json::Value> = text
            .lines()
            .map(|l| serde_json::from_str(l).expect("every JSONL line is valid JSON"))
            .collect();
        assert_eq!(rows.len(), r.events.len());
        for (row, e) in rows.iter().zip(&r.events) {
            assert_eq!(row["seq"].as_u64(), Some(e.seq as u64));
            assert_eq!(row["depth"].as_u64(), Some(e.depth as u64));
            assert_eq!(row["op"].as_str(), Some(e.op.as_str()));
            assert_eq!(row["result"].as_str(), Some(e.result.as_str()));
            assert!(row["elapsed_ms"].as_f64().is_some());
            assert_eq!(row["detail"].as_str(), Some(e.detail.as_str()));
        }
        assert_eq!(rows.last().unwrap()["op"].as_str(), Some("finish"));
        assert!(line_with(&lines, "events_out=").starts_with(&format!(
            "events_out={} lines={}",
            out_path.display(),
            r.events.len()
        )));
    }

    /// Parse a clicks JSONL file into values (each line must be one JSON object).
    fn read_clicks_jsonl(path: &Path) -> Vec<serde_json::Value> {
        let text = std::fs::read_to_string(path).expect("clicks file written");
        text.lines()
            .map(|l| serde_json::from_str(l).expect("every clicks JSONL line is valid JSON"))
            .collect()
    }

    #[test]
    fn clicks_out_writes_one_cd2_row_per_click_and_is_seed_reproducible() {
        // §21 CD2: `oasd run --clicks-out` persists the click audit that the
        // click-distribution view (CD3/CD4) reads. Two same-seed replays must
        // produce identical files except for the wall-clock `ts_ms`.
        let (td, task, replay) = fixture("clicks-out");
        let a = td.path().join("runs/a.clicks.jsonl");
        let b = td.path().join("runs/b.clicks.jsonl");
        let mut o1 = replay_options(&task, &replay, 42);
        o1.clicks_out = Some(a.clone());
        let (r1, lines1) = run_capture(&o1);
        let mut o2 = replay_options(&task, &replay, 42);
        o2.clicks_out = Some(b.clone());
        let (r2, _lines2) = run_capture(&o2);

        assert_eq!(r1.outcome(), Outcome::Done);
        assert_eq!(
            line_with(&lines1, "clicks_out="),
            format!("clicks_out={} count=1", a.display()),
            "the count must be machine-greppable (CD4 anchors on it)"
        );
        // The parent directory did not exist: the writer created it.
        let rows1 = read_clicks_jsonl(&a);
        let rows2 = read_clicks_jsonl(&b);
        assert_eq!(rows1.len(), 1, "the fixture clicks exactly once");
        assert_eq!(rows1.len(), r1.clicks.len());
        assert_eq!(rows1[0]["sel"], serde_json::Value::Null);
        assert_eq!(rows1[0]["seed"].as_u64(), Some(42));
        let (x, y) = (
            rows1[0]["x"].as_i64().expect("x") as i32,
            rows1[0]["y"].as_i64().expect("y") as i32,
        );
        assert_eq!(
            (x, y),
            (r1.clicks[0].x, r1.clicks[0].y),
            "CD2 x/y must be the ACTUAL points from the run"
        );
        assert!(
            (x - TARGET.x).abs() <= 6 && (y - TARGET.y).abs() <= 6,
            "HB2: ({x},{y}) is not within ±6 px of {TARGET:?}"
        );
        assert_eq!(rows1[0]["target_x"].as_i64(), Some(TARGET.x as i64));
        assert_eq!(rows1[0]["target_y"].as_i64(), Some(TARGET.y as i64));
        assert!(
            rows1[0]["ts_ms"].as_u64().unwrap_or(0) > 1_600_000_000_000,
            "ts_ms must be epoch milliseconds: {}",
            rows1[0]["ts_ms"]
        );

        // Same seed => byte-identical rows apart from the wall-clock timestamp.
        let strip_ts = |rows: Vec<serde_json::Value>| -> Vec<serde_json::Value> {
            rows.into_iter()
                .map(|mut r| {
                    r["ts_ms"] = serde_json::json!(0);
                    r
                })
                .collect()
        };
        assert_eq!(
            strip_ts(rows1),
            strip_ts(rows2),
            "same seed must reproduce the same CD2 rows (except ts_ms)"
        );
        assert_eq!(
            r1.clicks, r2.clicks,
            "same seed must replay the same actual points"
        );

        // Replay mode still never touches adb: no `device=` line is printed and
        // the frame source is the replay bundle.
        assert_eq!(line_with(&lines1, "frame_source="), "frame_source=replay");
        assert_eq!(line_with(&lines1, "input="), "input=dry (replay)");
        assert!(
            !lines1.iter().any(|l| l.starts_with("device=")),
            "replay must not open an adb device: {lines1:?}"
        );
    }

    #[test]
    fn clicks_out_is_written_even_when_the_run_fails() {
        // "Write even when outcome != Done": a failed run still leaves evidence
        // (here: an empty file = there were no clicks), and the count is printed.
        let td = TempDir::new("clicks-out-fail");
        let task = td.path().join("task");
        let replay = td.path().join("replay");
        let bundle = TaskBundle::open(&task).expect("open task bundle");
        bundle
            .save_flow(
                r#"{"schema_version":1,"name":"n4-fail","steps":[
                     {"op":"click_point","args":{"x":10000,"y":60}},
                     {"op":"finish","args":{"exit":"done"}}]}"#,
            )
            .expect("save flow");
        write_replay_bundle(&replay);
        let out_path = td.path().join("nested/runs/fail.clicks.jsonl");
        let mut opts = replay_options(&task, &replay, 3);
        opts.clicks_out = Some(out_path.clone());
        let (r, lines) = run_capture(&opts);

        assert_eq!(r.outcome(), Outcome::Failed, "off-screen click must fail");
        assert_eq!(r.exit_code(), 1);
        let text = std::fs::read_to_string(&out_path).expect("clicks file written on failure");
        assert_eq!(text, "", "no successful click => no rows");
        assert_eq!(
            line_with(&lines, "clicks_out="),
            format!("clicks_out={} count=0", out_path.display())
        );
    }

    #[test]
    fn recording_wrapper_persists_every_grab_and_replays_frame_exact() {
        struct FakeScreen {
            frames: Vec<Frame>,
            idx: usize,
        }
        impl ScreenSource for FakeScreen {
            fn grab(&mut self) -> Result<Frame, String> {
                let f = self
                    .frames
                    .get(self.idx)
                    .ok_or("fake screen exhausted")?
                    .clone();
                self.idx += 1;
                Ok(f)
            }
        }

        let td = TempDir::new("record");
        let rec_dir = td.path().join("bundle");
        let rec = Rc::new(RefCell::new(
            Recorder::create(&rec_dir, "wrapper-test").expect("recorder"),
        ));
        let originals = vec![frame(3), frame(4), frame(5)];
        let mut src = RecordingScreen {
            inner: Box::new(FakeScreen {
                frames: originals.clone(),
                idx: 0,
            }),
            rec: rec.clone(),
        };
        for _ in 0..3 {
            src.grab().expect("grab through the recording wrapper");
        }
        assert_eq!(rec.borrow().frames(), 3, "every grab must be persisted");
        drop(src);

        let rec = match Rc::try_unwrap(rec) {
            Ok(r) => r.into_inner(),
            Err(_) => panic!("recorder must be unique after the source is dropped"),
        };
        let meta = rec.finish().expect("finish");
        assert_eq!(meta.frames, 3);
        assert_eq!((meta.width, meta.height), (FRAME_W, FRAME_H));

        let mut replay = ReplaySource::open(&rec_dir).expect("open replay");
        assert_eq!(replay.len(), 3);
        for (i, want) in originals.iter().enumerate() {
            let got = replay.grab().expect("grab replay");
            assert_eq!(
                oas_codec::frame_hash(&got),
                oas_codec::frame_hash(want),
                "recorded frame {i} must replay byte-identically"
            );
        }
        assert!(replay.grab().is_err(), "replay must fail loudly when exhausted");
    }

    /// Run the fixture flow against an arbitrary screen source with a pinned
    /// seed, collecting the actual input points — the same machinery `execute`
    /// uses, minus the CLI/reporting layer.
    fn run_bundle<S: ScreenSource>(
        bundle: &TaskBundle,
        src: S,
        seed: u64,
    ) -> (RunResult, Vec<Point>) {
        let flow = bundle.load_flow().expect("load flow").expect("flow present");
        let assets = bundle.load_runtime_assets().expect("load assets");
        let clicks: Rc<RefCell<Vec<Point>>> = Rc::new(RefCell::new(Vec::new()));
        let input = LoggedInput {
            inner: None,
            clicks: clicks.clone(),
            swipes: Rc::new(RefCell::new(Vec::new())),
        };
        let mut interp =
            Interpreter::new(src, input, assets, Budget::default()).with_seed(seed);
        let result = interp.run(&flow);
        let points = clicks.borrow().clone();
        (result, points)
    }

    #[test]
    fn recorded_run_then_replay_reproduces_the_same_clicks() {
        // The N5 judge chain, offline: a run whose every grab is recorded must
        // replay — with the same seed — to the IDENTICAL click sequence. The
        // "live" pass is simulated with a frame bundle, so no device is needed.
        let td = TempDir::new("record-replay-chain");
        let task = td.path().join("task");
        let live_frames = td.path().join("live_frames");
        let rec_dir = td.path().join("recorded");
        write_task_bundle(&task);
        write_replay_bundle(&live_frames);
        let bundle = TaskBundle::open(&task).expect("open task");

        // (1) baseline run over the "live" frames.
        let (r1, clicks_live) = run_bundle(
            &bundle,
            ReplaySource::open(&live_frames).expect("open live frames"),
            42,
        );
        assert_eq!(r1.outcome(), Outcome::Done);

        // (2) the same run with the `--record` wrapper around the source.
        let rec = Rc::new(RefCell::new(
            Recorder::create(&rec_dir, "recorded-run").expect("recorder"),
        ));
        let recorded_src = RecordingScreen {
            inner: Box::new(ReplaySource::open(&live_frames).expect("open live frames")),
            rec: rec.clone(),
        };
        let (r2, clicks_recorded) = run_bundle(&bundle, recorded_src, 42);
        assert_eq!(r2.outcome(), Outcome::Done);
        assert_eq!(
            clicks_recorded, clicks_live,
            "recording must not perturb the action sequence"
        );
        assert_eq!(
            rec.borrow().frames(),
            1,
            "the flow's one grab must be persisted"
        );
        let rec = match Rc::try_unwrap(rec) {
            Ok(r) => r.into_inner(),
            Err(_) => panic!("recorder must be unique after the source is dropped"),
        };
        rec.finish().expect("finish recording");

        // (3) replay that recording through the real CLI path: same seed =>
        // identical actual points (the S1 judge's assertion).
        let (r3, lines) = run_capture(&replay_options(&task, &rec_dir, 42));
        assert_eq!(r3.outcome(), Outcome::Done);
        assert_eq!(
            r3.clicks, clicks_live,
            "replay of the recording must reproduce the live clicks"
        );
        assert_eq!(line_with(&lines, "frame_source="), "frame_source=replay");
        assert_eq!(line_with(&lines, "seed="), "seed=42");
    }

    #[test]
    fn stamped_meta_stays_readable_by_oas_obs() {
        // `stamp_meta` adds seed/task/replay_hint to meta.json (HB5: the seed
        // must live in the bundle); `oas_obs::read_meta` must keep reading it
        // (the on-disk Meta contract is not widened, unknown fields ignored).
        let td = TempDir::new("stamped-meta");
        let rec_dir = td.path().join("rec");
        let mut rec = Recorder::create(&rec_dir, "test").expect("recorder");
        rec.capture_frame(&frame(1)).expect("frame");
        rec.finish().expect("finish");
        let task = td.path().join("task");
        stamp_meta(&rec_dir, 42, &task, "live").expect("stamp meta");

        let meta = oas_obs::read_meta(&rec_dir).expect("meta must stay readable");
        assert_eq!(meta.frames, 1);
        assert_eq!((meta.width, meta.height), (FRAME_W, FRAME_H));
        assert_eq!(meta.source, "test");
        let text = std::fs::read_to_string(rec_dir.join("meta.json")).expect("read meta");
        assert!(text.contains("\"seed\": 42"), "seed missing from meta: {text}");
        assert!(text.contains("\"replay_hint\""), "replay hint missing: {text}");
        assert!(text.contains("--replay"), "replay hint must be actionable: {text}");
    }

    #[test]
    fn record_with_replay_is_an_actionable_error() {
        let (td, task, replay) = fixture("record-replay");
        let mut opts = replay_options(&task, &replay, 1);
        opts.record = Some(td.path().join("rec"));
        let err = execute(&opts, &mut |_: &str| {}).expect_err("must be rejected");
        assert!(err.contains("--record"), "{err}");
        assert!(err.contains("--replay"), "{err}");
        assert!(err.contains("live-only"), "{err}");
    }

    #[test]
    fn record_dir_with_existing_frames_is_rejected() {
        let (td, _task, _replay) = fixture("stale-record");
        let rec_dir = td.path().join("replay"); // already has frames
        let err = ensure_fresh_recording(&rec_dir).expect_err("stale frames must be rejected");
        assert!(err.contains("already contains frames"), "{err}");
        // A fresh dir is accepted (and is not created by the check).
        let fresh = td.path().join("fresh-rec");
        assert!(ensure_fresh_recording(&fresh).is_ok());
        assert!(!fresh.exists(), "the freshness check must not create the dir");
    }

    #[test]
    fn missing_flow_and_missing_replay_are_actionable() {
        let td = TempDir::new("errors");
        let task = td.path().join("task");
        std::fs::create_dir_all(&task).expect("task dir");
        let opts = RunOptions {
            task: Some(task.clone()),
            replay: Some(td.path().join("no-such-replay")),
            seed: Some(1),
            ..RunOptions::default()
        };
        let err = execute(&opts, &mut |_: &str| {}).expect_err("no flow.json must be an error");
        assert!(err.contains("flow.json"), "{err}");
        assert!(err.contains("editor"), "{err}");

        // With a flow present, the missing replay bundle names the fix.
        write_task_bundle(&task);
        let err = execute(&opts, &mut |_: &str| {}).expect_err("missing replay bundle");
        assert!(err.contains("replay bundle"), "{err}");
        assert!(err.contains("--record"), "{err}");
    }

    #[test]
    fn seed_zero_means_auto_and_is_still_printed() {
        let (_td, task, replay) = fixture("seed-auto");
        let mut opts = replay_options(&task, &replay, 0);
        opts.seed = Some(0);
        let (r, lines) = run_capture(&opts);
        assert_ne!(r.seed, 0, "seed 0 must resolve to a concrete seed");
        assert_eq!(line_with(&lines, "seed="), format!("seed={}", r.seed));
    }

    #[test]
    fn cli_parsing_covers_every_flag_and_rejects_bad_input() {
        let args: Vec<String> = [
            "--task",
            "tasks/S1切预设",
            "--replay",
            "_verify/rec1",
            "--seed",
            "9",
            "--dry-run",
            "--record",
            "rec",
            "--events-out",
            "out/e.jsonl",
            "--clicks-out",
            "runs/s1.clicks.jsonl",
            "--budget-steps",
            "7",
            "--budget-wall",
            "1.5",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let o = RunOptions::parse(&args).expect("parse all flags");
        assert_eq!(o.task.as_deref(), Some(Path::new("tasks/S1切预设")));
        assert_eq!(o.replay.as_deref(), Some(Path::new("_verify/rec1")));
        assert_eq!(o.seed, Some(9));
        assert!(o.dry_run);
        assert_eq!(o.record.as_deref(), Some(Path::new("rec")));
        assert_eq!(o.events_out.as_deref(), Some(Path::new("out/e.jsonl")));
        assert_eq!(
            o.clicks_out.as_deref(),
            Some(Path::new("runs/s1.clicks.jsonl"))
        );
        assert_eq!(o.budget_steps, 7);
        assert_eq!(o.budget_wall_s, 1.5);

        for bad in [
            vec!["--task"],
            vec!["--nope"],
            vec!["--seed", "abc"],
            vec!["--budget-steps", "0"],
            vec!["--budget-wall", "NaN"],
            vec!["--clicks-out"],
        ] {
            let bad: Vec<String> = bad.iter().map(|s| s.to_string()).collect();
            assert!(
                RunOptions::parse(&bad).is_err(),
                "should reject {bad:?}"
            );
        }
    }
}
