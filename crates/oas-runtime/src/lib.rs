//! oas-runtime: the flow interpreter (P1-b).
//!
//! Executes a validated [`oas_flow::Flow`] against a [`ScreenSource`] +
//! [`InputSink`], emitting one **structured event per step** (step / op / detail
//! / result / elapsed) and enforcing a hard [`Budget`].
//!
//! Design rules that exist because the old engine got them wrong:
//! * **Exhaustion is not success.** Budget/loop exhaustion returns
//!   `RunResult::Exhausted`, which maps to `Outcome::Exhausted`, never `Done`.
//! * **Missing selectors fail loudly** instead of being skipped.
//! * **OCR is not stubbed to 0/""**: until P1-c wires ONNX, `ocr()` returns an
//!   explicit error, so a flow that needs OCR cannot silently pass.

pub mod humanize;

use std::collections::{BTreeMap, HashMap};
use std::time::{Duration, Instant};

use oas_flow::{
    eval, DragAxis, DragTarget, EvalContext, Expect, Expr, Flow, InterruptRule, Locator, Step, Value,
};
use oas_perceive::{GrayImage, Matcher};
use oas_types::{Exit, Frame, InputSink, Outcome, Point, Rect, ScreenSource};

/// One declared asset: template pixels + optional search ROI + threshold.
#[derive(Clone)]
pub struct Template {
    pub gray: GrayImage,
    pub roi: Option<Rect>,
    pub threshold: f32,
    /// Optional polygon (FRAME coordinates at authoring time, ENGINE §22.10).
    /// When present, `click` samples inside its **largest inscribed axis-aligned
    /// rectangle** instead of the full matched template rect — the polygon is
    /// used for precision/selection, while the click area stays rectangular.
    pub click_polygon: Option<Vec<Point>>,
}

/// The task's declared assets (`assets/<Name>.json` in a later phase).
#[derive(Default, Clone)]
pub struct AssetSet {
    map: HashMap<String, Template>,
    /// OCR assets (`kind: ocr_line` / `ocr_lines`): they carry **no pixels**,
    /// only a frame ROI. `ocr(sel)` crops that ROI and hands it to the injected
    /// [`OcrProvider`] (ENGINE-DESIGN §17.1: rec is the fast path for one line;
    /// `ocr_lines` regions go through **det + rec**).
    ocr: HashMap<String, (Rect, bool)>,
    /// Optional polygon mask per OCR asset (§22.10 N10b-2): OCR crops the bbox
    /// but only pixels INSIDE this polygon are kept; outside is filled white.
    ocr_polygons: HashMap<String, Vec<Point>>,
    /// Optional explicit **operation region** (`action_roi`, old OAS `roiBack`)
    /// per recognizable object: frame coords at design resolution; translated by
    /// the match offset on use. OCR objects use it directly.
    click_back: HashMap<String, Rect>,
    /// Pure operation regions (`kind: region`, v1.2): no recognition, the rect is
    /// the operation region itself.
    regions: HashMap<String, Rect>,
    /// Derived OCR lines (v1.2.1): saved from a multi-line det+rec result. Runtime
    /// must re-run det+rec on `source_roi` and match `expected` text, instead of
    /// degrading to a single-line rec on the tight line crop.
    derived_ocr: HashMap<String, (Rect, String)>,
    /// v1.2.3 template collections: collection id -> member template ids.
    collections: HashMap<String, Vec<String>>,
    collection_thresholds: HashMap<String, f32>,
}

impl AssetSet {
    /// Attach a polygon mask to an OCR asset (frame coordinates).
    pub fn set_ocr_polygon(&mut self, id: impl Into<String>, polygon: Vec<Point>) {
        self.ocr_polygons.insert(id.into(), polygon);
    }

    pub fn ocr_polygon(&self, id: &str) -> Option<&[Point]> {
        self.ocr_polygons.get(id).map(|v| v.as_slice())
    }

    /// Attach an explicit operation region to a recognizable object (v1.2).
    pub fn set_click_back(&mut self, id: impl Into<String>, rect: Rect) {
        self.click_back.insert(id.into(), rect);
    }

    pub fn click_back(&self, id: &str) -> Option<Rect> {
        self.click_back.get(id).copied()
    }

    /// Declare a pure operation region (`kind: region`, v1.2).
    pub fn insert_region(&mut self, id: impl Into<String>, rect: Rect) {
        self.regions.insert(id.into(), rect);
    }

    pub fn region(&self, id: &str) -> Option<Rect> {
        self.regions.get(id).copied()
    }

    /// Register a multi-line-derived OCR line: source ROI + expected text.
    pub fn set_derived_ocr(&mut self, id: impl Into<String>, source_roi: Rect, expected: String) {
        self.derived_ocr.insert(id.into(), (source_roi, expected));
    }

    pub fn derived_ocr(&self, id: &str) -> Option<(Rect, String)> {
        self.derived_ocr.get(id).cloned()
    }

    /// v1.2.3: declare a template collection. Runtime tries members in order and
    /// uses the winning member's action_roi.
    pub fn insert_collection(
        &mut self,
        id: impl Into<String>,
        members: Vec<String>,
        threshold: Option<f32>,
    ) {
        let id = id.into();
        if let Some(t) = threshold {
            self.collection_thresholds.insert(id.clone(), t);
        }
        self.collections.insert(id, members);
    }

    pub fn collection_members(&self, id: &str) -> Option<&[String]> {
        self.collections.get(id).map(|v| v.as_slice())
    }

    /// Threshold for a plain template or a collection (collection override wins).
    pub fn threshold_for(&self, id: &str) -> Option<f32> {
        self.collection_thresholds
            .get(id)
            .copied()
            .or_else(|| self.map.get(id).map(|t| t.threshold))
    }
}

impl AssetSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, id: impl Into<String>, tpl: Template) {
        self.map.insert(id.into(), tpl);
    }

    /// Declare a **single-line** OCR asset (rec fast path).
    pub fn insert_ocr(&mut self, id: impl Into<String>, roi: Rect) {
        self.ocr.insert(id.into(), (roi, false));
    }

    /// Declare a **multi-line** OCR asset (`kind: ocr_lines`) — det + rec.
    pub fn insert_ocr_lines(&mut self, id: impl Into<String>, roi: Rect) {
        self.ocr.insert(id.into(), (roi, true));
    }

    pub fn get(&self, id: &str) -> Option<&Template> {
        self.map.get(id)
    }

    /// ROI of a declared OCR asset, if `id` is one.
    pub fn ocr_roi(&self, id: &str) -> Option<Rect> {
        self.ocr.get(id).map(|(roi, _)| *roi)
    }

    /// `true` when this OCR asset must go through det + rec (`ocr_lines`).
    pub fn ocr_is_multiline(&self, id: &str) -> bool {
        self.ocr.get(id).map(|(_, multi)| *multi).unwrap_or(false)
    }

    pub fn ocr_len(&self) -> usize {
        self.ocr.len()
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

/// One detected text line (det + rec): box in **frame** coordinates.
#[derive(Debug, Clone, PartialEq)]
pub struct OcrLine {
    pub rect: Rect,
    pub text: String,
    pub score: f32,
}

/// Point-in-polygon (ray casting, even-odd). Boundary points are unspecified.
pub fn point_in_polygon(p: Point, polygon: &[Point]) -> bool {    if polygon.len() < 3 {
        return false;
    }
    let (px, py) = (p.x as f64, p.y as f64);
    let mut inside = false;
    let n = polygon.len();
    for i in 0..n {
        let a = polygon[i];
        let b = polygon[(i + 1) % n];
        if ((a.y as f64) > py) != ((b.y as f64) > py) {
            let x = (b.x - a.x) as f64 * (py - a.y as f64) / ((b.y - a.y) as f64)
                + a.x as f64;
            if px < x {
                inside = !inside;
            }
        }
    }
    inside
}

/// OCR mask helper (§22.10 N10b-2): copy the frame and fill every pixel inside
/// the OCR bbox but OUTSIDE the polygon with white, so only polygon content is
/// recognised.
pub fn mask_frame_for_ocr(frame: &Frame, roi: Rect, polygon: &[Point]) -> Frame {
    let mut rgba = frame.rgba.clone();
    let x0 = roi.x.max(0);
    let y0 = roi.y.max(0);
    let x1 = (roi.x + roi.w).min(frame.width as i32);
    let y1 = (roi.y + roi.h).min(frame.height as i32);
    for y in y0..y1 {
        for x in x0..x1 {
            if !point_in_polygon(Point { x, y }, polygon) {
                let i = (((y as u32) * frame.width + (x as u32)) * 4) as usize;
                rgba[i] = 255;
                rgba[i + 1] = 255;
                rgba[i + 2] = 255;
                rgba[i + 3] = 255;
            }
        }
    }
    Frame::new(frame.width, frame.height, rgba)
}

/// Largest axis-aligned rectangle **inscribed in the polygon** (ENGINE §22.10).///
/// Implementation: rasterise the polygon's bounding box to a boolean mask
/// (pixel centres), then run the classic maximal-rectangle-in-histogram scan.
/// Boxes are small (a UI button), so this is exact-on-pixels and fast.
pub fn largest_inscribed_rect(polygon: &[Point]) -> Option<Rect> {
    if polygon.len() < 3 {
        return None;
    }
    let (mut min_x, mut min_y) = (i32::MAX, i32::MAX);
    let (mut max_x, mut max_y) = (i32::MIN, i32::MIN);
    for p in polygon {
        min_x = min_x.min(p.x);
        min_y = min_y.min(p.y);
        max_x = max_x.max(p.x);
        max_y = max_y.max(p.y);
    }
    let w = (max_x - min_x + 1) as usize;
    let h = (max_y - min_y + 1) as usize;
    if w == 0 || h == 0 {
        return None;
    }
    let mut mask = vec![false; w * h];
    for j in 0..h {
        for i in 0..w {
            if point_in_polygon(
                Point {
                    x: min_x + i as i32,
                    y: min_y + j as i32,
                },
                polygon,
            ) {
                mask[j * w + i] = true;
            }
        }
    }

    // Maximal rectangle in a binary matrix (height histogram + stack).
    let mut heights = vec![0usize; w];
    let mut best: Option<(usize, usize, usize, usize)> = None; // x, y, w, h
    let mut best_area = 0usize;
    for j in 0..h {
        for i in 0..w {
            heights[i] = if mask[j * w + i] { heights[i] + 1 } else { 0 };
        }
        let mut stack: Vec<usize> = Vec::new();
        for i in 0..=w {
            let cur = if i == w { 0 } else { heights[i] };
            while let Some(&top) = stack.last() {
                if heights[top] > cur {
                    let height = heights[top];
                    stack.pop();
                    let left = stack.last().map(|&x| x + 1).unwrap_or(0);
                    let width = i - left;
                    let area = height * width;
                    if area > best_area {
                        best_area = area;
                        best = Some((left, j + 1 - height, width, height));
                    }
                } else {
                    break;
                }
            }
            stack.push(i);
        }
    }
    best.map(|(x, y, ww, hh)| Rect {
        x: min_x + x as i32,
        y: min_y + y as i32,
        w: ww as i32,
        h: hh as i32,
    })
}
///
/// The runtime deliberately does **not** depend on `oas-ocr`: the ONNX engine is
/// optional (`onnx` feature) and only the binary crate wires it in. This trait is
/// the seam, so the interpreter can be unit-tested offline with a fake provider
/// and built **without** ONNX at all.
///
/// Contract (ENGINE-DESIGN §3.2 "识别统一返回结构化结果，不许静默 0"):
/// * an empty string means "nothing recognised"; the interpreter maps it to
///   `Value::Null`, never to a success-looking `""`;
/// * a provider error is a **loud** run failure, not a skipped step.
///
/// There are two paths on purpose (ENGINE-DESIGN §17.1 — the user explicitly
/// kept both): `recognize` is the fast single-line rec primitive; `recognize_lines`
/// is det + per-line rec for a whole region. `ocr_line` assets call the former,
/// `ocr_lines` assets the latter.
pub trait OcrProvider {
    fn recognize(&mut self, frame: &Frame, roi: Rect) -> Result<(String, f32), String>;

    /// det + rec: every text line inside `roi`, boxes in FRAME coordinates.
    fn recognize_lines(&mut self, frame: &Frame, roi: Rect) -> Result<Vec<OcrLine>, String>;
}

/// Persistent "coordinate memory" store for the lowest-priority locator of a
/// `click_target` chain (ENGINE §22.11).
///
/// The runtime never touches the filesystem: the binary crate implements this
/// over `tasks/<task>/state/coords.json`, decides whether writes are allowed
/// (replay = read-only) and handles design-resolution scaling.
pub trait CoordStore {
    fn get(&mut self, key: &str, frame_w: u32, frame_h: u32) -> Option<Point>;
    fn put(&mut self, key: &str, point: Point, frame_w: u32, frame_h: u32);
}

/// Progress hook for checkpointing (ENGINE §22.16): called after each successful
/// TOP-LEVEL step with its index, so the driver can persist "next step to run".
pub trait ProgressSink {
    fn step_done(&mut self, index: usize, op: &str);

    /// v1.2.2: persist runtime variables after each successful top-level step.
    /// Default is a no-op so existing sinks stay compatible.
    fn vars_changed(&mut self, _vars: &BTreeMap<String, Value>) {}
}

#[derive(Debug, Clone, Copy)]
pub struct Budget {
    pub max_wall_s: f64,
    pub max_steps: u32,
}

impl Default for Budget {
    fn default() -> Self {
        Self {
            max_wall_s: 300.0,
            max_steps: 5000,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StepEvent {
    pub seq: usize,
    pub depth: usize,
    pub op: String,
    pub detail: String,
    pub result: String,
    pub elapsed_ms: f64,
}

/// One click-audit row: the **actual** point handed to the [`InputSink`] plus the
/// target it was derived from (design doc §21 CD2, §19 CP4, §20 HB6).
///
/// `x`/`y` are what really got clicked; `target_x`/`target_y` are the intent
/// (a template rect's centre for `click`, the evaluated point for `click_point`).
/// Keeping both is what lets the click-distribution view answer "why did the
/// click land here?" — and `seed` makes the jitter reproducible.
#[derive(Debug, Clone, PartialEq)]
pub struct ClickRecord {
    /// Epoch milliseconds when the click was accepted.
    pub ts_ms: u128,
    /// ACTUAL clicked point.
    pub x: i32,
    /// ACTUAL clicked point.
    pub y: i32,
    /// Target point: template-rect centre (`click`) / evaluated target (`click_point`).
    pub target_x: i32,
    /// Target point: template-rect centre (`click`) / evaluated target (`click_point`).
    pub target_y: i32,
    /// Asset id for a `click` step; `None` for a raw `click_point`.
    pub sel: Option<String>,
    /// HB7: `true` when this click MISSED the target box (human error). The row
    /// still records the real point so the distribution view can colour misses.
    pub miss: bool,
    /// Which locator resolved this click (§22.11): `template` / `ocr_text` /
    /// `coord_memory` / `point` (raw `click_point`).
    pub locator: String,
    /// Human-readable match evidence for THIS click, e.g. `template:预设:0.987`,
    /// `ocr_text:分组列表:分组2:1.000`, `coord_memory:分组2记忆`, `point`.
    /// Written to the CD2 JSONL so every click's match path is auditable.
    pub match_info: String,
    /// The run's seed (HB5: the audit is reproducible).
    pub seed: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RunResult {
    Finished(Exit),
    Failed(String),
    /// Hard limits hit (wall time / step count / loop iterations).
    Exhausted(String),
}

impl RunResult {
    /// ★ The single mapping point to the 3-state outcome.
    pub fn outcome(&self) -> Outcome {
        match self {
            RunResult::Finished(exit) => exit.to_outcome(),
            RunResult::Failed(_) => Outcome::Failed,
            RunResult::Exhausted(_) => Outcome::Exhausted,
        }
    }
}

enum Ctl {
    Normal,
    Break,
    Continue,
    Finish(Exit),
}

/// Result of one OCR asset read (`ocr` step / expression).
enum OcrRead {
    /// `ocr_line`: one recognised line (or `None` when nothing was read).
    Single(Option<String>, f32),
    /// `ocr_lines`: det + rec, one entry per text line.
    Lines(Vec<OcrLine>),
}

pub struct Interpreter<S: ScreenSource, I: InputSink> {
    src: S,
    input: I,
    assets: AssetSet,
    budget: Budget,
    matcher: Option<Matcher>,
    /// RGBA frame the cached matcher was built from. Kept so in-flow OCR can
    /// crop the **same** frame a selector just looked at (P1-c); dropped by
    /// [`Self::invalidate_frame`] exactly like the matcher.
    frame: Option<Frame>,
    /// Injected OCR engine. `None` means "this run cannot do OCR": `ocr()` must
    /// fail loudly instead of returning `""` (ENGINE-DESIGN §3.2).
    ocr: Option<Box<dyn OcrProvider>>,
    /// Injected coordinate-memory store for `click_target` fallbacks (§22.11).
    coords: Option<Box<dyn CoordStore>>,
    /// Known popups dismissed before every step (§22.15), copied from the flow.
    interrupts: Vec<InterruptRule>,
    /// Checkpoint hook (§22.16); `None` = no persistence.
    progress: Option<Box<dyn ProgressSink>>,
    /// Resume offset: skip this many TOP-LEVEL steps (§22.16 checkpoint recovery).
    start_step: usize,
    /// Size of the frame `matcher` was built from (`click_point` needs it for
    /// the CP3 in-frame check).
    frame_size: Option<(u32, u32)>,
    events: Vec<StepEvent>,
    /// CD2 (§21) click audit: one row per SUCCESSFUL click, cleared per run.
    clicks: Vec<ClickRecord>,
    /// Seed resolved once at construction (HB5); `with_seed` pins it.
    run_seed: u64,
    /// Injected task parameters for `param('key')` (M0 unified template):
    /// `meta.params[].default` merged with `oasd run --param k=v` overrides.
    params: BTreeMap<String, Value>,
    /// v1.2 runtime variables (`set_var`/`inc_var`/`ocr_to_var`), scoped to one
    /// flow run. Not persisted in checkpoints (intentionally simple).
    vars: BTreeMap<String, Value>,
    /// Matched line rect from the last OCR read (v1.2.1 derived ocr_line), used
    /// to translate the saved operation region to the new line position.
    last_ocr_rect: Option<Rect>,
    /// v1.2.3: for each collection selector, which member matched last.
    last_set_member: HashMap<String, String>,
    started: Instant,
    humanize: crate::humanize::Humanize,
    rng: crate::humanize::Rng,
    steps: u32,
}

impl<S: ScreenSource, I: InputSink> Interpreter<S, I> {
    pub fn new(src: S, input: I, assets: AssetSet, budget: Budget) -> Self {
        let humanize = crate::humanize::Humanize::default();
        // Per-run seed: random for the game (HB1/HB5), persisted by the
        // recorder so a replay can use with_seed() and reproduce exactly.
        // A FIXED seed every run would make the click sequence identical
        // across runs, which is itself a bot tell. Resolved ONCE here so the
        // CD2 click audit records the same seed the RNG actually uses.
        let run_seed = humanize.resolved_seed(now_ms());
        Self {
            src,
            input,
            assets,
            budget,
            matcher: None,
            frame: None,
            ocr: None,
            coords: None,
            interrupts: Vec::new(),
            progress: None,
            start_step: 0,
            frame_size: None,
            events: Vec::new(),
            clicks: Vec::new(),
            run_seed,
            params: BTreeMap::new(),
            vars: BTreeMap::new(),
            last_ocr_rect: None,
            last_set_member: HashMap::new(),
            started: Instant::now(),
            humanize,
            rng: crate::humanize::Rng::new(run_seed),
            steps: 0,
        }
    }

    pub fn events(&self) -> &[StepEvent] {
        &self.events
    }

    /// CD2 (§21) click audit for the LAST/current run: actual point, target,
    /// selector and the run seed, one row per successful `click`/`click_point`.
    pub fn clicks(&self) -> &[ClickRecord] {
        &self.clicks
    }

    /// Last frame the interpreter looked at (for failure screenshots, §22.11 审计).
    pub fn last_frame(&self) -> Option<Frame> {
        self.frame.clone()
    }

    /// The seed this run actually used (HB5). Unlike [`Self::seed_now`] (which
    /// exposes the mutable RNG state the recorder peeks at), this is the
    /// immutable per-run seed recorded in the audit.
    pub fn run_seed(&self) -> u64 {
        self.run_seed
    }

    /// Pin the RNG seed (replays use the seed recorded in the bundle).
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.rng = crate::humanize::Rng::new(seed);
        self.run_seed = seed;
        self
    }

    /// Attach the OCR engine used by `ocr(sel)` / `ocr_digit(sel)` / `ocr` steps
    /// (P1-c). Without it, any OCR accessor is a **loud** failure — it must
    /// never look like "recognised nothing" (ENGINE-DESIGN §3.2).
    pub fn with_ocr(mut self, provider: Box<dyn OcrProvider>) -> Self {
        self.ocr = Some(provider);
        self
    }

    /// Attach the coordinate-memory store used by `click_target`'s last locator.
    pub fn with_coord_store(mut self, store: Box<dyn CoordStore>) -> Self {
        self.coords = Some(store);
        self
    }

    /// v1.2.2: restore variables from a checkpoint before resuming.
    pub fn with_vars(mut self, vars: BTreeMap<String, Value>) -> Self {
        self.vars = vars;
        self
    }

    /// v1.2.2: snapshot of runtime variables for checkpoint persistence.
    pub fn vars(&self) -> &BTreeMap<String, Value> {
        &self.vars
    }

    /// Attach a checkpoint hook (§22.16): after every successful top-level step.
    pub fn with_progress_sink(mut self, sink: Box<dyn ProgressSink>) -> Self {
        self.progress = Some(sink);
        self
    }

    /// Resume from a checkpoint: skip the first `n` top-level steps (§22.16).
    pub fn with_start_step(mut self, n: usize) -> Self {
        self.start_step = n;
        self
    }

    /// Override the humanisation policy (HB1~HB3). Offline simulations use
    /// `delay_min_ms == delay_max_ms == 0` so 1000 runs do not spend ~500ms per
    /// run in HB3 sleeps; the click **distribution** stays the same (HB1/HB2 are
    /// untouched).
    pub fn with_humanize(mut self, humanize: crate::humanize::Humanize) -> Self {
        self.humanize = humanize;
        self
    }

    /// Inject the task parameters visible to `param('key')` (M0 unified task
    /// template). The driver passes `meta.params[].default` values merged with
    /// the `--param k=v` CLI overrides; a call with no value is a **loud**
    /// runtime failure, never a silent null/zero.
    pub fn with_params(mut self, params: BTreeMap<String, Value>) -> Self {
        self.params = params;
        self
    }

    /// Add one parameter value (convenience for callers/tests).
    pub fn with_param(mut self, key: impl Into<String>, value: Value) -> Self {
        self.params.insert(key.into(), value);
        self
    }

    /// Current RNG seed state (record it into the bundle meta).
    pub fn seed_now(&self) -> u64 {
        self.rng.peek()
    }

    pub fn run(&mut self, flow: &Flow) -> RunResult {
        self.started = Instant::now();
        self.events.clear();
        // A rerun is a NEW run: its click audit must not carry rows from the
        // previous one (same lifecycle as `events`).
        self.clicks.clear();
        self.steps = 0;
        self.interrupts = flow.interrupts.clone();
        // ENGINE §22.16: top-level steps are executed one by one so a checkpoint
        // can be persisted after each success (and `--resume` can skip the prefix).
        let start = self.start_step.min(flow.steps.len());
        let mut ctl = Ctl::Normal;
        let mut outcome: Result<(), String> = Ok(());
        for (i, step) in flow.steps.iter().enumerate().skip(start) {
            match self.exec(std::slice::from_ref(step), 0, &mut ctl) {
                Ok(()) => {
                    if let Some(sink) = self.progress.as_mut() {
                        sink.step_done(i, step_name(step));
                        let vars = self.vars.clone();
                        sink.vars_changed(&vars);
                    }
                    if !matches!(ctl, Ctl::Normal) {
                        break;
                    }
                }
                Err(e) => {
                    outcome = Err(e);
                    break;
                }
            }
        }
        match outcome {
            Ok(()) => match ctl {
                Ctl::Finish(exit) => RunResult::Finished(exit),
                _ => RunResult::Failed(
                    "flow ended without finish(<exit>) - an explicit exit is required".into(),
                ),
            },
            Err(e) if e.starts_with(EXHAUSTED) => {
                let detail = e[EXHAUSTED.len()..].trim_start_matches('.').trim().to_string();
                RunResult::Exhausted(detail)
            }
            Err(e) => RunResult::Failed(e),
        }
    }

    fn log(&mut self, depth: usize, op: &str, detail: &str, result: &str, elapsed_ms: f64) {
        let seq = self.events.len();
        self.events.push(StepEvent {
            seq,
            depth,
            op: op.to_string(),
            detail: detail.to_string(),
            result: result.to_string(),
            elapsed_ms,
        });
    }

    /// CD2 (§21): audit a click **after** the input sink accepted it — a click
    /// that failed (off-screen target / missing template) is not a click and must
    /// not appear in the distribution, or the heatmap would show points the game
    /// never received.
    fn record_click(
        &mut self,
        actual: Point,
        target: Point,
        sel: Option<String>,
        miss: bool,
        locator: &str,
        match_info: String,
    ) {
        self.clicks.push(ClickRecord {
            ts_ms: now_ms(),
            x: actual.x,
            y: actual.y,
            target_x: target.x,
            target_y: target.y,
            sel,
            miss,
            locator: locator.to_string(),
            match_info,
            seed: self.run_seed,
        });
    }

    /// Clamp -> HB3 delay -> tap -> invalidate -> CD2 audit. The three click
    /// instructions share this (DRY-3: same sequence in click/click_point/
    /// click_target), and it returns the ACTUAL point + delay for the step log.
    fn perform_click(
        &mut self,
        plan: crate::humanize::ClickPlan,
        centre: Point,
        sel: Option<String>,
        locator: &str,
        match_info: String,
        w: u32,
        h: u32,
    ) -> Result<(Point, u32), String> {
        let actual = Point {
            x: plan.point.x.clamp(0, w.saturating_sub(1) as i32),
            y: plan.point.y.clamp(0, h.saturating_sub(1) as i32),
        };
        let delay = self.humanize.delay_ms(&mut self.rng);
        if delay > 0 {
            std::thread::sleep(std::time::Duration::from_millis(delay as u64));
        }
        self.input.click(actual)?;
        // The screen may now be different: never let the next selector match
        // against this pre-click frame.
        self.invalidate_frame();
        self.record_click(actual, centre, sel, plan.miss, locator, match_info);
        Ok((actual, delay))
    }

    /// ENGINE §22.11 补充层: poll the screen after a click until `expect` holds.
    ///
    /// Each poll invalidates the cached frame and grabs a fresh one (a frame-gated
    /// replay therefore consumes one frame per poll — exactly how waiting is
    /// modelled offline). Timeout = loud step failure: "clicked but nothing
    /// changed" must never count as success.
    fn verify_expect(&mut self, expect: &Expect) -> Result<(), String> {
        let timeout = std::time::Duration::from_millis(expect.timeout_ms() as u64);
        let start = std::time::Instant::now();
        let mut last;
        loop {
            self.invalidate_frame();
            last = match expect {
                Expect::Appear { sel, .. } => {
                    let (score, at) = self.locate(sel)?;
                    let th = self.assets.threshold_for(sel).unwrap_or(0.9);
                    if at.is_some() && score >= th {
                        return Ok(());
                    }
                    format!("template `{sel}` best score {score:.3} < {th}")
                }
                Expect::Vanish { sel, .. } => {
                    let (score, at) = self.locate(sel)?;
                    let th = self.assets.threshold_for(sel).unwrap_or(0.9);
                    if !(at.is_some() && score >= th) {
                        return Ok(());
                    }
                    format!("template `{sel}` still present (score {score:.3})")
                }
                Expect::OcrText { sel, text, .. } => {
                    let read = self.ocr_read(sel)?;
                    let ok = match &read {
                        OcrRead::Lines(lines) => {
                            lines.iter().any(|l| l.text.trim() == text.trim())
                        }
                        OcrRead::Single(Some(t), _) => t.trim() == text.trim(),
                        OcrRead::Single(None, _) => false,
                    };
                    if ok {
                        return Ok(());
                    }
                    format!("ocr_text `{sel}` does not read {text:?} yet")
                }
            };
            let elapsed = start.elapsed();
            if elapsed >= timeout {
                return Err(format!(
                    "expect {} not met after {}ms ({last})",
                    expect.kind(),
                    expect.timeout_ms()
                ));
            }
            std::thread::sleep((timeout - elapsed).min(std::time::Duration::from_millis(50)));
        }
    }

    /// Pre-click page guard (`requires`): when unmet the click must NOT happen.
    /// Uses the same polling engine as `expect`, but a distinct error prefix.
    fn require_guard(&mut self, requires: Option<&Expect>) -> Result<bool, String> {
        match requires {
            None => Ok(false),
            Some(e) => {
                self.verify_expect(e)
                    .map_err(|err| format!("page guard failed: {err}"))?;
                Ok(true)
            }
        }
    }

    /// Run `expect` when present; returns whether a verification ran (for logs).
    fn verify_if_present(&mut self, expect: Option<&Expect>) -> Result<bool, String> {
        match expect {
            Some(e) => {
                self.verify_expect(e)?;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// `click_target`: after ANY locator succeeds, refresh the remembered point
    /// for the chain's `coord_memory` key (§22.11) — the store may be read-only.
    fn remember_target(&mut self, find: &[Locator], p: Point, w: u32, h: u32) {
        if let Some(store) = self.coords.as_mut() {
            for loc in find {
                if let Locator::CoordMemory { key } = loc {
                    store.put(key, p, w, h);
                    break;
                }
            }
        }
    }

    /// Grab a frame and cache its matcher (integral images) for this frame.
    fn refresh_frame(&mut self) -> Result<(), String> {
        let frame: Frame = self.src.grab()?;
        self.frame_size = Some((frame.width, frame.height));
        let gray = GrayImage::from_frame(&frame);
        self.matcher = Some(Matcher::new(gray));
        self.frame = Some(frame);
        Ok(())
    }

    /// Discard the cached frame. Called after every successful click: the click
    /// may have changed the screen, so the **next** selector must not match
    /// against the pre-click pixels (that stale-frame bug is exactly what this
    /// rewrite exists to kill: it produces "clicked the old position" runs).
    /// It also discards the cached RGBA frame used by in-flow OCR.
    fn invalidate_frame(&mut self) {
        self.matcher = None;
        self.frame = None;
    }

    fn check_budget(&self) -> Result<(), String> {
        if self.steps >= self.budget.max_steps {
            return Err(format!(
                "{EXHAUSTED}.step budget exceeded (max_steps={})",
                self.budget.max_steps
            ));
        }
        let elapsed = self.started.elapsed().as_secs_f64();
        if elapsed > self.budget.max_wall_s {
            return Err(format!("{EXHAUSTED}.wall-time budget exceeded ({elapsed:.1}s > {:.1}s)", self.budget.max_wall_s));
        }
        Ok(())
    }

    /// Score + best position of `sel` in the current frame.
    ///
    /// Borrow note: the frame/matcher is refreshed first (needs `&mut self`),
    /// then the asset and matcher are borrowed **disjointly** for the match, so
    /// no unsafe code is needed.
    /// Template-pixel mask from the asset polygon (frame coords → template coords).
    fn template_mask(&self, tpl: &Template) -> Option<Vec<bool>> {
        let poly = tpl.click_polygon.as_ref()?;
        let roi = tpl.roi?;
        let tw = tpl.gray.width as i32;
        let th = tpl.gray.height as i32;
        let mut mask = vec![false; (tw * th) as usize];
        for j in 0..th {
            for i in 0..tw {
                if point_in_polygon(Point { x: roi.x + i, y: roi.y + j }, poly) {
                    mask[(j * tw + i) as usize] = true;
                }
            }
        }
        Some(mask)
    }

    /// §22.29 H4：在后续 `frames - 1` 帧上继续匹配同一目标；位置漂移超过
    /// `tolerance_px` 或分数低于阈值 ⇒ 响亮失败（不点击）。
    fn verify_stable_frames(
        &mut self,
        sel: &str,
        first: Point,
        threshold: f32,
        frames: u8,
    ) -> Result<(), String> {
        const TOLERANCE_PX: i32 = 4;
        for i in 1..frames {
            self.refresh_frame()?;
            let (score, at) = self.locate(sel)?;
            if score < threshold {
                return Err(format!(
                    "多帧投票失败：第 {} 帧未达到阈值（{score:.3} < {threshold:.3}）",
                    i + 1
                ));
            }
            let p = at.ok_or_else(|| {
                format!("多帧投票失败：第 {} 帧未找到 `{sel}`", i + 1)
            })?;
            if (p.x - first.x).abs() > TOLERANCE_PX || (p.y - first.y).abs() > TOLERANCE_PX {
                return Err(format!(
                    "多帧投票失败：第 {} 帧位置漂移（首帧 ({},{})，本帧 ({},{})，容差 {}px）",
                    i + 1,
                    first.x,
                    first.y,
                    p.x,
                    p.y,
                    TOLERANCE_PX
                ));
            }
        }
        Ok(())
    }

    /// §22.31 H5：在 `rect` 周边 `expand` 像素内寻找一个已声明模板（用于
    /// 「OCR 文字 + 图标」互证：同文案不同按钮时只有带图标的那个通过）。
    fn template_in_region(
        &mut self,
        sel: &str,
        rect: Rect,
        expand: i32,
    ) -> Result<bool, String> {
        if self.matcher.is_none() {
            self.refresh_frame()?;
        }
        let Some(tpl) = self.assets.get(sel) else {
            return Err(format!(
                "require_template `{sel}` is not declared in the task's assets"
            ));
        };
        let th = tpl.threshold;
        let (fw, fh) = self
            .frame_size
            .ok_or_else(|| "require_template: no frame available".to_string())?;
        let (fw, fh) = (fw as i32, fh as i32);
        let x0 = (rect.x - expand).max(0);
        let y0 = (rect.y - expand).max(0);
        let x1 = (rect.x + rect.w + expand).min(fw);
        let y1 = (rect.y + rect.h + expand).min(fh);
        let w = (x1 - x0).max(0);
        let h = (y1 - y0).max(0);
        if w < tpl.gray.width as i32 || h < tpl.gray.height as i32 {
            return Ok(false);
        }
        let matcher = self
            .matcher
            .as_ref()
            .ok_or_else(|| "require_template: no frame available".to_string())?;
        Ok(matcher
            .match_template(&tpl.gray, Some(Rect { x: x0, y: y0, w, h }))
            .map(|b| b.score >= th)
            .unwrap_or(false))
    }

    fn locate(&mut self, sel: &str) -> Result<(f32, Option<Point>), String> {
        if let Some(members) = self.assets.collection_members(sel).map(|m| m.to_vec()) {
            if members.is_empty() {
                return Err(format!("selector `{sel}` is an empty template collection"));
            }
            if self.matcher.is_none() {
                self.refresh_frame()?;
            }
            let mut best: Option<(f32, Option<Point>, String)> = None;
            for m in members {
                let (score, at) = self.locate_one(&m)?;
                if best.as_ref().map(|b| score > b.0).unwrap_or(true) {
                    best = Some((score, at, m));
                }
            }
            let (score, at, member) = best.ok_or_else(|| format!("collection `{sel}` has no members"))?;
            self.last_set_member.insert(sel.to_string(), member);
            return Ok((score, at));
        }
        self.locate_one(sel)
    }

    fn locate_one(&mut self, sel: &str) -> Result<(f32, Option<Point>), String> {
        if self.matcher.is_none() {
            self.refresh_frame()?;
        }
        let tpl = self
            .assets
            .get(sel)
            .ok_or_else(|| format!("selector `{sel}` is not declared in the task's assets"))?;
        let matcher = self
            .matcher
            .as_ref()
            .ok_or_else(|| "no frame available".to_string())?;
        // §22.20: search the authored ROI first (fast); when that is not a
        // confident hit, fall back to a WHOLE-FRAME pyramid search so a UI that
        // shifted (or was re-laid-out) is still found — no ROI tuning needed.
        // §22.10 N10b-1: with a polygon, match only INSIDE it (masked NCC) so
        // background/animation outside the polygon cannot hurt the score.
        let roi_best = match self.template_mask(tpl) {
            Some(mask) => matcher.match_template_masked(&tpl.gray, &mask, tpl.roi),
            None => matcher.match_template(&tpl.gray, tpl.roi),
        };
        if let Some(b) = &roi_best {
            if b.score >= 0.95 {
                return Ok((b.score, Some(Point { x: b.x, y: b.y })));
            }
        }
        let full_best = matcher.match_template_full(&tpl.gray);
        match (roi_best, full_best) {
            (Some(a), Some(b)) => {
                if b.score > a.score {
                    Ok((b.score, Some(Point { x: b.x, y: b.y })))
                } else {
                    Ok((a.score, Some(Point { x: a.x, y: a.y })))
                }
            }
            (Some(a), None) => Ok((a.score, Some(Point { x: a.x, y: a.y }))),
            (None, Some(b)) => Ok((b.score, Some(Point { x: b.x, y: b.y }))),
            (None, None) => Ok((0.0, None)),
        }
    }

    /// Actual click box for a matched template. Explicit user decision [10-22
    /// 23:0x]: the click area is the matched template's **bounding rectangle**
    /// (the same rect before/after the polygon feature); the polygon now only
    /// affects MATCHING (mask, §22.10 N10b), never the click area shape.
    fn click_box_for(&self, sel: &str, p: Point) -> Result<Rect, String> {
        // v1.2.3: a collection click uses the winning member's geometry
        // (recognition ROI + operation region); the collection id has no pixels.
        let effective_sel: &str = if self.assets.collection_members(sel).is_some() {
            self.last_set_member
                .get(sel)
                .map(|s| s.as_str())
                .ok_or_else(|| format!("collection `{sel}` has no matched member yet"))?
        } else {
            sel
        };
        let t = self.assets.get(effective_sel).ok_or_else(|| {
            format!("selector `{effective_sel}` is not declared in the task's assets")
        })?;
        // §22.25: explicit click area (`roi_back`) wins, translated by the match
        // offset; without it we keep the matched template bbox (§22.20 decision).
        if let Some(back) = self.assets.click_back(effective_sel) {
            let roi = t.roi.ok_or_else(|| {
                format!(
                    "selector `{effective_sel}` has a roi_back but no roi to translate it \
                     (a click area only makes sense together with an authored roi)"
                )
            })?;
            return Ok(Rect {
                x: back.x + (p.x - roi.x),
                y: back.y + (p.y - roi.y),
                w: back.w,
                h: back.h,
            });
        }
        Ok(Rect {
            x: p.x,
            y: p.y,
            w: t.gray.width as i32,
            h: t.gray.height as i32,
        })
    }

    /// Evaluate one coordinate expression (`click_point`/`swipe`): it must be a
    /// finite number. A string/bool is a loud error, not a silent miss.
    fn eval_coord(&mut self, e: &Expr, what: &str) -> Result<f64, String> {
        match eval(e, self)? {
            Value::Num(n) if n.is_finite() => Ok(n),
            Value::Num(n) => Err(format!("{what} must be a finite number, got {n}")),
            other => Err(format!("{what} must be a number, got {other:?}")),
        }
    }

    /// Resolve an OCR asset and read it from the cached frame.
    ///
    /// Path selection follows the asset `kind` (ENGINE-DESIGN §17.1: the user
    /// kept **both** rec and det):
    /// * `ocr_line`  → `recognize` (fast single-line rec, 12~20ms);
    /// * `ocr_lines` → `recognize_lines` (det + rec, every line in the region).
    ///
    /// Failure semantics (all loud, never a silent `""`):
    /// * `sel` not declared / declared as a pixel template ⇒ error;
    /// * no [`OcrProvider`] attached ⇒ error;
    /// * the provider itself fails (empty ROI, out of bounds, ONNX error,
    ///   `ocr_lines` without a det model) ⇒ error.
    ///
    /// An **empty recognised text is not an error**: it maps to `None` (= `Null`
    /// in expressions), so `wait_until(ocr('X') == …)` can keep polling and a
    /// missing text can never satisfy the comparison.
    /// OCR an explicit frame rectangle (v1.1 inline `ocr_text` locators).
    ///
    /// Same loud failure semantics as [`Self::ocr_read`]: no provider = error,
    /// provider failure = error; empty text maps to `None` for the single path.
    fn ocr_read_rect(
        &mut self,
        roi: oas_types::Rect,
        multiline: bool,
    ) -> Result<OcrRead, String> {
        if self.ocr.is_none() {
            return Err(
                "OCR is not available for this run: no OCR provider attached \
                 (build with `--features ocr` and pass `--model <rec.onnx>`) - \
                 refusing to fake inline ocr_text"
                    .into(),
            );
        }
        if self.frame.is_none() {
            self.refresh_frame()?;
        }
        let frame = self
            .frame
            .as_ref()
            .ok_or_else(|| "OCR: no frame available".to_string())?;
        let provider = self.ocr.as_mut().expect("provider checked above");
        if multiline {
            Ok(OcrRead::Lines(provider.recognize_lines(frame, roi)?))
        } else {
            let (text, score) = provider.recognize(frame, roi)?;
            Ok(OcrRead::Single(
                if text.is_empty() { None } else { Some(text) },
                score,
            ))
        }
    }

    /// Resolve a `drag` endpoint to a frame rectangle.
    ///
    /// `region`/`point` need no assets; `ocr_text` reads the line's bbox from an
    /// OCR asset or an inline ROI. A single-line OCR target has no line bbox by
    /// design, so it is rejected with a clear message.
    fn drag_target_rect(&mut self, t: &DragTarget) -> Result<oas_types::Rect, String> {
        match t {
            DragTarget::Region { roi } => Ok(oas_types::Rect {
                x: roi[0],
                y: roi[1],
                w: roi[2],
                h: roi[3],
            }),
            DragTarget::Point { x, y } => Ok(oas_types::Rect {
                x: *x,
                y: *y,
                w: 1,
                h: 1,
            }),
            DragTarget::OcrText { sel, roi, text } => {
                let read = if let Some(roi) = roi {
                    self.ocr_read_rect(
                        oas_types::Rect {
                            x: roi[0],
                            y: roi[1],
                            w: roi[2],
                            h: roi[3],
                        },
                        true,
                    )?
                } else {
                    let sel = sel
                        .clone()
                        .ok_or_else(|| "ocr_text drag target needs sel or roi".to_string())?;
                    self.ocr_read(&sel)?
                };
                match read {
                    OcrRead::Lines(lines) => lines
                        .iter()
                        .find(|l| l.text.trim() == text.trim())
                        .map(|l| l.rect)
                        .ok_or_else(|| {
                            format!("drag ocr_text: no line equals {text:?} in the region")
                        }),
                    OcrRead::Single(..) => Err(
                        "drag ocr_text target must use ocr_lines/inline det (single-line has no bbox)"
                            .into(),
                    ),
                }
            }
        }
    }

    /// Random point inside `rect`, inset by `margin` fraction (never edge-exact).
    fn random_point_in(&mut self, rect: oas_types::Rect, margin: f32) -> Point {
        let w = rect.w.max(1);
        let h = rect.h.max(1);
        let mx = ((w as f32 * margin).floor() as i32).min(w / 2);
        let my = ((h as f32 * margin).floor() as i32).min(h / 2);
        let span_x = (w - 2 * mx).max(1);
        let span_y = (h - 2 * my).max(1);
        let dx = (self.rng.unit_f64() * span_x as f64).floor() as i32;
        let dy = (self.rng.unit_f64() * span_y as f64).floor() as i32;
        Point {
            x: rect.x + mx + dx.min(span_x - 1),
            y: rect.y + my + dy.min(span_y - 1),
        }
    }

    fn ocr_read(&mut self, sel: &str) -> Result<OcrRead, String> {

        let roi = match self.assets.ocr_roi(sel) {
            Some(r) => r,
            None if self.assets.get(sel).is_some() => {
                return Err(format!(
                    "selector `{sel}` is a template asset, not an OCR asset \
                     (kind must be `ocr_line`/`ocr_lines`)"
                ))
            }
            None => {
                return Err(format!(
                    "OCR selector `{sel}` is not declared in the task's assets"
                ))
            }
        };
        if self.ocr.is_none() {
            return Err(format!(
                "OCR is not available for this run: no OCR provider attached \
                 (build with `--features ocr` and pass `--model <rec.onnx>`) - \
                 refusing to fake `ocr({sel})`"
            ));
        }
        let multiline = self.assets.ocr_is_multiline(sel);
        let ocr_polygon = self.assets.ocr_polygon(sel).map(|p| p.to_vec());
        if self.frame.is_none() {
            self.refresh_frame()?;
        }
        let frame = self
            .frame
            .as_ref()
            .ok_or_else(|| "OCR: no frame available".to_string())?;
        // v1.2.1: a line saved from a multi-line det+rec result must use the SAME
        // source region + det+rec path at runtime. Falling back to single-line
        // rec on the tight line crop can change the recognized text.
        if let Some((source, expected)) = self.assets.derived_ocr(sel) {
            let provider = self.ocr.as_mut().expect("provider checked above");
            let lines = provider.recognize_lines(frame, source)?;
            let hit = lines
                .iter()
                .filter(|l| l.text.trim() == expected.trim())
                .max_by(|a, b| a.score.partial_cmp(&b.score).unwrap_or(std::cmp::Ordering::Equal));
            let Some(line) = hit else {
                self.last_ocr_rect = None;
                return Err(format!(
                    "derived ocr_line `{sel}`: no line equals {:?} in original source ROI {:?} \
                     ({} lines found) - refusing to use single-line rec",
                    expected,
                    source,
                    lines.len()
                ));
            };
            self.last_ocr_rect = Some(line.rect);
            return Ok(OcrRead::Single(Some(line.text.clone()), line.score));
        }
        self.last_ocr_rect = None;
        // §22.10 N10b-2: polygon OCR mask = bbox crop, outside-polygon pixels
        // filled white so neighbour text / background never reaches the engine.
        let masked_frame;
        let frame: &Frame = if let Some(poly) = &ocr_polygon {
            masked_frame = mask_frame_for_ocr(frame, roi, poly);
            &masked_frame
        } else {
            frame
        };
        let provider = self.ocr.as_mut().expect("provider checked above");
        if multiline {
            Ok(OcrRead::Lines(provider.recognize_lines(frame, roi)?))
        } else {
            let (text, score) = provider.recognize(frame, roi)?;
            Ok(OcrRead::Single(
                if text.is_empty() { None } else { Some(text) },
                score,
            ))
        }
    }

    /// Value view of [`Self::ocr_read`]: `ocr()` returns **one string**.
    ///
    /// For `ocr_lines` the lines are joined with `\n` and the score is the
    /// **minimum** line score — never an optimistic average (a bad line must
    /// lower the reported confidence).
    fn ocr_run(&mut self, sel: &str) -> Result<(Option<String>, f32), String> {
        match self.ocr_read(sel)? {
            OcrRead::Single(text, score) => Ok((text, score)),
            OcrRead::Lines(lines) if lines.is_empty() => Ok((None, 0.0)),
            OcrRead::Lines(lines) => {
                let text = lines
                    .iter()
                    .map(|l| l.text.clone())
                    .collect::<Vec<_>>()
                    .join("\n");
                let score = lines
                    .iter()
                    .map(|l| l.score)
                    .fold(f32::INFINITY, f32::min);
                Ok((Some(text), score))
            }
        }
    }

    fn ocr_digit_value(&mut self, sel: &str) -> Result<Option<f64>, String> {
        match self.ocr_run(sel)? {
            (Some(text), _) => Ok(parse_digit(&text)),
            (None, _) => Ok(None),
        }
    }

    /// Resolve one known popup if any `interrupts` rule matches (§22.15).
    ///
    /// A rule is `{detect, action?}`: `detect` proves the popup is present, then
    /// `action` is clicked (defaults to `detect`). This is what lets an invitation
    /// popup be **accepted** (action = 「接受」) instead of blindly dismissed.
    /// Returns `true` when a popup was resolved (caller re-checks, bounded).
    fn handle_interrupts(&mut self, depth: usize) -> Result<bool, String> {
        let rules = self.interrupts.clone();
        for rule in &rules {
            let (score, at) = self.locate(&rule.detect)?;
            let th = self.assets.threshold_for(&rule.detect).unwrap_or(0.9);
            let Some(_detected) = at.filter(|_| score >= th) else {
                continue;
            };
            let action_sel = rule.action.clone().unwrap_or_else(|| rule.detect.clone());
            let (ascore, aat) = self.locate(&action_sel)?;
            let ath = self
                .assets
                .get(&action_sel)
                .map(|t| t.threshold)
                .unwrap_or(0.9);
            let Some(ap) = aat.filter(|_| ascore >= ath) else {
                return Err(format!(
                    "interrupt: popup `{}` detected (score {score:.3}) but action `{action_sel}` \
                     not found (best {ascore:.3} < {ath})",
                    rule.detect
                ));
            };
            let target = self.click_box_for(&action_sel, ap)?;
            let (w, h) = self
                .frame_size
                .ok_or_else(|| "interrupt: no frame available".to_string())?;
            let plan = self.humanize.template_click(target, w, &mut self.rng);
            let centre = Point {
                x: target.x + target.w / 2,
                y: target.y + target.h / 2,
            };
            let (actual, delay) = self.perform_click(
                plan,
                centre,
                Some(action_sel.clone()),
                "interrupt",
                format!(
                    "interrupt:{}:{}->{}:{ascore:.3}",
                    rule.detect, rule.detect, action_sel
                ),
                w,
                h,
            )?;
            self.log(
                depth,
                "interrupt",
                &format!(
                    "resolved popup detect=`{}` action=`{action_sel}` score={ascore:.3} point=({},{}) delay={delay}ms",
                    rule.detect, actual.x, actual.y
                ),
                "ok",
                0.0,
            );
            return Ok(true);
        }
        Ok(false)
    }

    fn exec(&mut self, steps: &[Step], depth: usize, ctl: &mut Ctl) -> Result<(), String> {
        for step in steps {
            self.check_budget()?;
            self.steps += 1;
            if !matches!(ctl, Ctl::Normal) {
                return Ok(());
            }
            // ENGINE §22.15: dismiss known popups BEFORE the step runs. Bounded
            // so a permanently-matching "popup" cannot spin forever.
            let mut dismissed = 0usize;
            while self.handle_interrupts(depth)? {
                dismissed += 1;
                if dismissed >= MAX_INTERRUPTS_PER_STEP {
                    return Err(format!(
                        "popup handling did not converge after {MAX_INTERRUPTS_PER_STEP} dismissals \
                         (check the `interrupts` rules)"
                    ));
                }
            }
            // ENGINE §22.16: step-level retry with backoff. The policy is only
            // attached to action steps; control-flow steps never re-run.
            let policy = step_retry(step);
            let mut attempt: u32 = 0;
            loop {
                let t0 = Instant::now();
                match self.exec_one(step, depth, ctl) {
                    Ok(detail) => {
                        let ms = t0.elapsed().as_secs_f64() * 1000.0;
                        let op = step_name(step);
                        if attempt > 0 {
                            self.log(
                                depth,
                                op,
                                &format!("{detail} (retried {attempt})"),
                                "ok",
                                ms,
                            );
                        } else {
                            self.log(depth, op, &detail, "ok", ms);
                        }
                        if matches!(ctl, Ctl::Finish(_)) {
                            return Ok(());
                        }
                        break;
                    }
                    Err(e) => {
                        let ms = t0.elapsed().as_secs_f64() * 1000.0;
                        let op = step_name(step);
                        let Some(policy) = policy else {
                            self.log(depth, op, &format!("error: {e}"), "failed", ms);
                            return Err(e);
                        };
                        if attempt >= policy.times {
                            self.log(
                                depth,
                                op,
                                &format!(
                                    "error: {e} (retries exhausted: {}/{})",
                                    attempt, policy.times
                                ),
                                "failed",
                                ms,
                            );
                            return Err(e);
                        }
                        attempt += 1;
                        let delay = policy.delay_for_attempt(attempt);
                        self.log(
                            depth,
                            op,
                            &format!(
                                "error: {e} (retry {attempt}/{} after {delay}ms)",
                                policy.times
                            ),
                            "retry",
                            ms,
                        );
                        if delay > 0 {
                            std::thread::sleep(Duration::from_millis(delay as u64));
                        }
                        self.invalidate_frame(); // a retry must re-grab
                        self.check_budget()?;
                    }
                }
            }
        }
        Ok(())
    }

    fn exec_one(&mut self, step: &Step, depth: usize, ctl: &mut Ctl) -> Result<String, String> {
        match step {
            Step::If {
                cond,
                then,
                otherwise,
            } => {
                let v = eval(cond, self)?;
                let taken = matches!(v, Value::Bool(true));
                let body = if taken { then } else { otherwise };
                self.exec(body, depth + 1, ctl)?;
                Ok(format!("cond -> {taken}"))
            }
            Step::Loop { n, body } => {
                let count = match eval(n, self)? {
                    Value::Num(x) => x.max(0.0) as u32,
                    other => return Err(format!("loop n must be a number, got {other:?}")),
                };
                if count > self.budget.max_steps {
                    return Err(format!("{EXHAUSTED}.iteration limit reached"));
                }
                let mut done = 0u32;
                for _ in 0..count {
                    self.exec(body, depth + 1, ctl)?;
                    if matches!(ctl, Ctl::Break) {
                        *ctl = Ctl::Normal;
                        break;
                    }
                    if matches!(ctl, Ctl::Continue) {
                        *ctl = Ctl::Normal;
                        continue;
                    }
                    if matches!(ctl, Ctl::Finish(_)) {
                        return Ok(format!("loop interrupted after {done}"));
                    }
                    done += 1;
                }
                Ok(format!("loop {done}/{count}"))
            }
            Step::While { cond, body } => {
                let mut iters = 0u32;
                loop {
                    self.check_budget()?;
                    let value = eval(cond, self)?;
                    let truthy = match value {
                        Value::Bool(b) => b,
                        // Common-language truthiness: 0 is false, nonzero is true.
                        Value::Num(n) => n != 0.0,
                        other => {
                            return Err(format!(
                                "while condition must be bool or number, got {other:?}"
                            ))
                        }
                    };
                    if !truthy {
                        break;
                    }
                    iters += 1;
                    self.exec(body, depth + 1, ctl)?;
                    if matches!(ctl, Ctl::Break) {
                        *ctl = Ctl::Normal;
                        break;
                    }
                    if matches!(ctl, Ctl::Continue) {
                        *ctl = Ctl::Normal;
                    }
                    if matches!(ctl, Ctl::Finish(_)) {
                        return Ok(format!("while interrupted after {iters}"));
                    }
                }
                Ok(format!("while {iters} iteration(s)"))
            }
            Step::Break => {
                *ctl = Ctl::Break;
                Ok("break".into())
            }
            Step::Continue => {
                *ctl = Ctl::Continue;
                Ok("continue".into())
            }
            Step::Wait { seconds } => {
                let s = *seconds;
                if s < 0.0 {
                    return Err("wait seconds must be >= 0".into());
                }
                std::thread::sleep(Duration::from_secs_f64(s));
                Ok(format!("waited {s}s"))
            }
            Step::WaitUntil { cond, timeout_s } => {
                let deadline = Instant::now() + Duration::from_secs_f64(*timeout_s);
                loop {
                    if matches!(eval(cond, self)?, Value::Bool(true)) {
                        return Ok("condition met".into());
                    }
                    if Instant::now() >= deadline {
                        return Err(format!("wait_until timed out after {timeout_s}s"));
                    }
                    std::thread::sleep(Duration::from_millis(20));
                    self.invalidate_frame(); // next eval grabs a fresh frame
                }
            }
            Step::Appear { sel } => {
                let (score, at) = self.locate(sel)?;
                let th = self.assets.threshold_for(sel).unwrap_or(0.9);
                if score >= th {
                    Ok(format!("appear {sel} score={score:.3} at {at:?}"))
                } else {
                    Err(format!("selector `{sel}` not found (best score {score:.3} < {th})"))
                }
            }
            Step::DismissPopup {
                sel,
                action,
                timeout_ms,
                required,
                ..
            } => {
                let deadline = Instant::now() + Duration::from_millis(*timeout_ms as u64);
                loop {
                    let (score, at) = self.locate(sel)?;
                    let th = self.assets.threshold_for(sel).unwrap_or(0.9);
                    if let Some(_detected) = at.filter(|_| score >= th) {
                        // §22.15: `action` selects the button (e.g. 接受); when
                        // omitted the detected template itself is clicked.
                        let action_sel = action.clone().unwrap_or_else(|| sel.clone());
                        let (ascore, aat) = self.locate(&action_sel)?;
                        let ath = self
                            .assets
                            .get(&action_sel)
                            .map(|t| t.threshold)
                            .unwrap_or(0.9);
                        let Some(ap) = aat.filter(|_| ascore >= ath) else {
                            return Err(format!(
                                "dismiss_popup: popup `{sel}` detected but action `{action_sel}` \
                                 not found (best {ascore:.3} < {ath})"
                            ));
                        };
                        let target = self.click_box_for(&action_sel, ap)?;
                        let (w, h) = self
                            .frame_size
                            .ok_or_else(|| "dismiss_popup: no frame available".to_string())?;
                        let plan = self.humanize.template_click(target, w, &mut self.rng);
                        let centre = Point {
                            x: target.x + target.w / 2,
                            y: target.y + target.h / 2,
                        };
                        let (actual, delay) = self.perform_click(
                            plan,
                            centre,
                            Some(action_sel.clone()),
                            "dismiss_popup",
                            format!("dismiss_popup:{sel}->{action_sel}:{ascore:.3}"),
                            w,
                            h,
                        )?;
                        return Ok(format!(
                            "resolved popup `{sel}` action=`{action_sel}` score={ascore:.3} \
                             point=({},{}) delay={delay}ms",
                            actual.x, actual.y
                        ));
                    }
                    if *timeout_ms == 0 || Instant::now() >= deadline {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                    self.invalidate_frame(); // next poll grabs a fresh frame
                }
                if *required {
                    Err(format!(
                        "dismiss_popup: `{sel}` not found within {timeout_ms}ms"
                    ))
                } else {
                    Ok(format!("no popup `{sel}`"))
                }
            }
            Step::Click {
                sel,
                alts,
                requires,
                expect,
                stable_frames,
                ..
            } => {
                let guarded = self.require_guard(requires.as_ref())?;
                // ENGINE §22.11 补充层 "多模板变体": try `sel` first, then the
                // alternatives in declaration order; the first one reaching its
                // own threshold wins and its id is what gets audited.
                let mut candidates: Vec<&str> = Vec::with_capacity(1 + alts.len());
                candidates.push(sel.as_str());
                // INJECTED(I18) removed
                candidates.extend(alts.iter().map(|s| s.as_str()));
                let mut evidence: Vec<String> = Vec::new();
                let mut hit: Option<(&str, f32, Point)> = None;
                for cand in &candidates {
                    let (score, at) = self.locate(cand)?;
                    let th = self.assets.threshold_for(*cand).unwrap_or(0.9);
                    if let Some(p) = at.filter(|_| score >= th) {
                        hit = Some((cand, score, p));
                        break;
                    }
                    evidence.push(format!("{cand}: {score:.3} < {th}"));
                }
                let Some((hit_sel, score, p)) = hit else {
                    return Err(format!(
                        "click target `{sel}` not found (no variant matched: {})",
                        evidence.join(", ")
                    ));
                };
                // §22.29 H4 多帧投票：连续 N 帧都命中同一位置才允许点击，
                // 专治动画/过场/淡入期间的抖动或误匹配。
                let vote_frames = stable_frames.unwrap_or(1);
                if vote_frames > 1 {
                    let th = self
                        .assets
                        .get(hit_sel)
                        .map(|t| t.threshold)
                        .unwrap_or(0.9);
                    self.verify_stable_frames(hit_sel, p, th, vote_frames)?;
                }

                // HB1: the target box is the MATCHED TEMPLATE RECT, not an
                // 11x11 box at its top-left. Picking inside the real box is what
                // keeps the click distribution spread over the button; a fixed
                // box would defeat the whole anti-ban rule.
                //
                // ENGINE §22.10: if the asset has a polygon, the CLICK AREA is
                // the largest axis-aligned rectangle inscribed in that polygon
                // (translated by the match offset). Matching still uses the
                // whole template.
                let target = self.click_box_for(hit_sel, p)?;
                // HB1/HB7: 20% bell / 5% uniform / 75% comet (hand-side tail)
                // inside the target rect; with 1~3% probability the click MISSES
                // the box (still performed, audited as `miss=true`).
                let (w, h) = self
                    .frame_size
                    .ok_or_else(|| "click: no frame available".to_string())?;
                let plan = self.humanize.template_click(target, w, &mut self.rng);
                let centre = Point {
                    x: target.x + target.w / 2,
                    y: target.y + target.h / 2,
                };
                let match_info = format!("template:{hit_sel}:{score:.3}");
                let (actual, delay) = self.perform_click(
                    plan,
                    centre,
                    Some(hit_sel.to_string()),
                    "template",
                    match_info,
                    w,
                    h,
                )?;
                let verified = self.verify_if_present(expect.as_ref())?;
                let miss = if plan.miss { " miss=true" } else { "" };
                let voted = if vote_frames > 1 {
                    format!(" voted={vote_frames}")
                } else {
                    String::new()
                };
                Ok(format!(
                    "clicked {hit_sel} target=({},{}) point=({},{}){miss} delay={delay}ms score={score:.3} verified={verified} guard={guarded}{voted}",
                    p.x, p.y, actual.x, actual.y
                ))
            }
            Step::ClickPoint { x, y, requires, expect, .. } => {
                let guarded = self.require_guard(requires.as_ref())?;
                // CP2: both coordinates are expressions (literal or computed).
                let tx = self.eval_coord(x, "x")?;
                let ty = self.eval_coord(y, "y")?;
                let target = Point {
                    x: tx.round() as i32,
                    y: ty.round() as i32,
                };
                // CP3 needs the frame dimensions; grab one lazily if nothing has
                // looked at the screen yet in this run.
                if self.matcher.is_none() {
                    self.refresh_frame()?;
                }
                let (w, h) = self
                    .frame_size
                    .ok_or_else(|| "click_point: no frame available".to_string())?;
                // CP3: an off-screen click must fail loudly - never "clicked
                // outside the screen, counted as success".
                if target.x < 0 || target.y < 0 || target.x >= w as i32 || target.y >= h as i32 {
                    return Err(format!(
                        "click_point ({},{}) is outside the frame ({}x{})",
                        target.x, target.y, w, h
                    ));
                }
                // HB2/HB7: a raw coordinate click gets +/- jitter_px of noise and
                // can miss by up to ~2*miss_px with the 1~3% per-click chance.
                let plan = self.humanize.point_click(target, &mut self.rng);
                let (actual, delay) = self.perform_click(
                    plan,
                    target,
                    None,
                    "point",
                    "point".to_string(),
                    w,
                    h,
                )?;
                let verified = self.verify_if_present(expect.as_ref())?;
                // CP4/HB6: the log carries the TARGET *and* the actual point
                // (plus the jitter's delay and the frame it was checked against).
                let miss = if plan.miss { " miss=true" } else { "" };
                Ok(format!(
                    "clicked_point target=({},{}) point=({},{}){miss} delay={delay}ms frame={}x{} verified={verified} guard={guarded}",
                    target.x, target.y, actual.x, actual.y, w, h
                ))
            }
            Step::ClickTarget {
                find,
                requires,
                expect,
                ..
            } => {
                let guarded = self.require_guard(requires.as_ref())?;
                // ENGINE §22.11: fixed priority image > text > memory; every
                // locator failure is kept as evidence, and ALL failing is a loud
                // step failure (never a silent skip).
                if self.frame_size.is_none() {
                    self.refresh_frame()?;
                }
                let (w, h) = self
                    .frame_size
                    .ok_or_else(|| "click_target: no frame available".to_string())?;
                let mut order: Vec<&Locator> = find.iter().collect();
                order.sort_by_key(|l| l.priority());
                let mut evidence: Vec<String> = Vec::new();
                for loc in order {
                    match loc {
                        Locator::Template { sel, threshold } => match self.locate(sel) {
                            Ok((score, at)) => {
                                let th = threshold.unwrap_or_else(|| {
                                    self.assets.threshold_for(sel).unwrap_or(0.9)
                                });
                                let Some(p) = at.filter(|_| score >= th) else {
                                    evidence.push(format!(
                                        "template `{sel}`: best score {score:.3} < {th}"
                                    ));
                                    continue;
                                };
                                let target = self.click_box_for(sel, p)?;
                                let plan = self.humanize.template_click(target, w, &mut self.rng);
                                let centre = Point {
                                    x: target.x + target.w / 2,
                                    y: target.y + target.h / 2,
                                };
                                let match_info = format!("template:{sel}:{score:.3}");
                                let (actual, _delay) = self.perform_click(
                                    plan,
                                    centre,
                                    Some(sel.clone()),
                                    "template",
                                    match_info,
                                    w,
                                    h,
                                )?;
                                self.remember_target(find, actual, w, h);
                                let verified = self.verify_if_present(expect.as_ref())?;
                                let miss = if plan.miss { " miss=true" } else { "" };
                                return Ok(format!(
                                    "clicked_target locator=template sel={sel} score={score:.3} point=({},{}){miss} verified={verified} guard={guarded}",
                                    actual.x, actual.y
                                ));
                            }
                            Err(e) => evidence.push(format!("template `{sel}`: {e}")),
                        },
                        Locator::OcrText {
                            sel,
                            text,
                            require_template,
                        } => match self.ocr_read(sel) {
                            Ok(OcrRead::Lines(lines)) => {
                                let hit = lines
                                    .iter()
                                    .filter(|l| l.text.trim() == text.trim())
                                    .max_by(|a, b| {
                                        a.score
                                            .partial_cmp(&b.score)
                                            .unwrap_or(std::cmp::Ordering::Equal)
                                    });
                                match hit {
                                    Some(line) => {
                                        if let Some(rt) = require_template {
                                            match self.template_in_region(rt, line.rect, 16) {
                                                Ok(true) => {}
                                                Ok(false) => {
                                                    evidence.push(format!(
                                                        "ocr_text `{sel}`: line {:?} matched but \
                                                         required template `{rt}` not found near it",
                                                        line.text
                                                    ));
                                                    continue;
                                                }
                                                Err(e) => return Err(e),
                                            }
                                        }
                                        let plan =
                                            self.humanize.template_click(line.rect, w, &mut self.rng);
                                        let centre = Point {
                                            x: line.rect.x + line.rect.w / 2,
                                            y: line.rect.y + line.rect.h / 2,
                                        };
                                        let match_info =
                                            format!("ocr_text:{sel}:{}:{:.3}", line.text, line.score);
                                        let (actual, _delay) = self.perform_click(
                                            plan,
                                            centre,
                                            Some(sel.clone()),
                                            "ocr_text",
                                            match_info,
                                            w,
                                            h,
                                        )?;
                                        self.remember_target(find, actual, w, h);
                                        let verified = self.verify_if_present(expect.as_ref())?;
                                        let miss = if plan.miss { " miss=true" } else { "" };
                                        return Ok(format!(
                                            "clicked_target locator=ocr_text sel={sel} text={:?} score={:.3} point=({},{}){miss} verified={verified} guard={guarded}",
                                            line.text, line.score, actual.x, actual.y
                                        ));
                                    }
                                    None => evidence.push(format!(
                                        "ocr_text `{sel}`: no line equals {:?} ({} lines found)",
                                        text,
                                        lines.len()
                                    )),
                                }
                            }
                            Ok(OcrRead::Single(Some(t), score)) => {
                                if t.trim() == text.trim() {
                                    let recognition = self
                                        .assets
                                        .ocr_roi(sel)
                                        .ok_or_else(|| format!("ocr_text `{sel}`: OCR asset has no roi"))?;
                                    // v1.2: the saved operation region (defaults to the
                                    // recognition ROI) is where the click lands. This is
                                    // what lets an OCR line's action area be larger than
                                    // the text box without per-action offsets.
                                    let mut rect = self.assets.click_back(sel).unwrap_or(recognition);
                                    // v1.2.1: a multi-line-derived line follows its
                                    // matched line position; translate the saved
                                    // operation region by the detection delta.
                                    if self.assets.derived_ocr(sel).is_some() {
                                        if let Some(matched) = self.last_ocr_rect {
                                            let saved_line = recognition;
                                            rect.x += matched.x - saved_line.x;
                                            rect.y += matched.y - saved_line.y;
                                        }
                                    }
                                    if let Some(rt) = require_template {
                                        match self.template_in_region(rt, recognition, 16) {
                                            Ok(true) => {}
                                            Ok(false) => {
                                                evidence.push(format!(
                                                    "ocr_text `{sel}`: {:?} matched but required \
                                                     template `{rt}` not found near it",
                                                    t
                                                ));
                                                continue;
                                            }
                                            Err(e) => return Err(e),
                                        }
                                    }
                                    let plan = self.humanize.template_click(rect, w, &mut self.rng);
                                    let centre = Point {
                                        x: rect.x + rect.w / 2,
                                        y: rect.y + rect.h / 2,
                                    };
                                    let match_info =
                                        format!("ocr_text:{sel}:{t}:{score:.3}");
                                    let (actual, _delay) = self.perform_click(
                                        plan,
                                        centre,
                                        Some(sel.clone()),
                                        "ocr_text",
                                        match_info,
                                        w,
                                        h,
                                    )?;
                                    self.remember_target(find, actual, w, h);
                                    let verified = self.verify_if_present(expect.as_ref())?;
                                    let miss = if plan.miss { " miss=true" } else { "" };
                                    return Ok(format!(
                                        "clicked_target locator=ocr_text sel={sel} text={:?} score={score:.3} point=({},{}){miss} verified={verified} guard={guarded}",
                                        t, actual.x, actual.y
                                    ));
                                }
                                evidence.push(format!(
                                    "ocr_text `{sel}`: text {:?} != {:?}",
                                    t, text
                                ));
                            }
                            Ok(OcrRead::Single(None, score)) => evidence.push(format!(
                                "ocr_text `{sel}`: no text (score {score:.3})"
                            )),
                            Err(e) => evidence.push(format!("ocr_text `{sel}`: {e}")),
                        },
                        Locator::OcrTextInline {
                            roi,
                            text,
                            require_template,
                        } => {
                            let rect = oas_types::Rect {
                                x: roi[0],
                                y: roi[1],
                                w: roi[2],
                                h: roi[3],
                            };
                            match self.ocr_read_rect(rect, true) {
                                Ok(OcrRead::Lines(lines)) => {
                                    let hit = lines
                                        .iter()
                                        .filter(|l| l.text.trim() == text.trim())
                                        .max_by(|a, b| {
                                            a.score
                                                .partial_cmp(&b.score)
                                                .unwrap_or(std::cmp::Ordering::Equal)
                                        });
                                    match hit {
                                        Some(line) => {
                                            if let Some(rt) = require_template {
                                                match self.template_in_region(rt, line.rect, 16) {
                                                    Ok(true) => {}
                                                    Ok(false) => {
                                                        evidence.push(format!(
                                                            "ocr_text inline: line {:?} matched but \
                                                             required template `{rt}` not found near it",
                                                            line.text
                                                        ));
                                                        continue;
                                                    }
                                                    Err(e) => return Err(e),
                                                }
                                            }
                                            let plan = self
                                                .humanize
                                                .template_click(line.rect, w, &mut self.rng);
                                            let centre = Point {
                                                x: line.rect.x + line.rect.w / 2,
                                                y: line.rect.y + line.rect.h / 2,
                                            };
                                            let match_info = format!(
                                                "ocr_text:inline:{}:{:.3}",
                                                line.text, line.score
                                            );
                                            let (actual, _delay) = self.perform_click(
                                                plan,
                                                centre,
                                                None,
                                                "ocr_text",
                                                match_info,
                                                w,
                                                h,
                                            )?;
                                            self.remember_target(find, actual, w, h);
                                            let verified =
                                                self.verify_if_present(expect.as_ref())?;
                                            let miss =
                                                if plan.miss { " miss=true" } else { "" };
                                            return Ok(format!(
                                                "clicked_target locator=ocr_text inline text={:?} \
                                                 score={:.3} point=({},{}){miss} verified={verified} guard={guarded}",
                                                line.text, line.score, actual.x, actual.y
                                            ));
                                        }
                                        None => evidence.push(format!(
                                            "ocr_text inline: no line equals {:?} ({} lines found in roi {:?})",
                                            text,
                                            lines.len(),
                                            roi
                                        )),
                                    }
                                }
                                Ok(OcrRead::Single(Some(t), score)) => {
                                    if t.trim() == text.trim() {
                                        let plan = self.humanize.template_click(rect, w, &mut self.rng);
                                        let centre = Point {
                                            x: rect.x + rect.w / 2,
                                            y: rect.y + rect.h / 2,
                                        };
                                        let (actual, _delay) = self.perform_click(
                                            plan,
                                            centre,
                                            None,
                                            "ocr_text",
                                            format!("ocr_text:inline:{t}:{score:.3}"),
                                            w,
                                            h,
                                        )?;
                                        self.remember_target(find, actual, w, h);
                                        let verified = self.verify_if_present(expect.as_ref())?;
                                        let miss = if plan.miss { " miss=true" } else { "" };
                                        return Ok(format!(
                                            "clicked_target locator=ocr_text inline text={t:?} \
                                             score={score:.3} point=({},{}){miss} verified={verified} guard={guarded}",
                                            actual.x, actual.y
                                        ));
                                    }
                                    evidence.push(format!(
                                        "ocr_text inline: text {t:?} != {text:?}"
                                    ));
                                }
                                Ok(OcrRead::Single(None, score)) => evidence.push(format!(
                                    "ocr_text inline: no text in roi {roi:?} (score {score:.3})"
                                )),
                                Err(e) => evidence.push(format!("ocr_text inline: {e}")),
                            }
                        }
                        Locator::Region { roi } => {
                            if self.matcher.is_none() {
                                self.refresh_frame()?;
                            }
                            let (fw, fh) = self
                                .frame_size
                                .ok_or_else(|| "region click: no frame available".to_string())?;
                            let rect = oas_types::Rect {
                                x: roi[0],
                                y: roi[1],
                                w: roi[2],
                                h: roi[3],
                            };
                            if rect.x < 0
                                || rect.y < 0
                                || rect.x + rect.w > fw as i32
                                || rect.y + rect.h > fh as i32
                            {
                                evidence.push(format!(
                                    "region {:?} is outside the frame ({}x{})",
                                    roi, fw, fh
                                ));
                                continue;
                            }
                            let p = self.random_point_in(rect, 0.15);
                            let plan = self.humanize.point_click(p, &mut self.rng);
                            let (actual, _delay) = self.perform_click(
                                plan,
                                p,
                                None,
                                "region",
                                format!("region:({},{},{},{})", roi[0], roi[1], roi[2], roi[3]),
                                fw,
                                fh,
                            )?;
                            self.remember_target(find, actual, fw, fh);
                            let verified = self.verify_if_present(expect.as_ref())?;
                            let miss = if plan.miss { " miss=true" } else { "" };
                            return Ok(format!(
                                "clicked_target locator=region roi={roi:?} point=({},{}){miss} verified={verified} guard={guarded}",
                                actual.x, actual.y
                            ));
                        }
                        Locator::RegionRef { sel } => {
                            let rect = self.assets.region(sel).ok_or_else(|| {
                                format!(
                                    "region `{sel}` is not declared as a task region asset \
                                     (kind: region)"
                                )
                            })?;
                            if self.matcher.is_none() {
                                self.refresh_frame()?;
                            }
                            let (fw, fh) = self
                                .frame_size
                                .ok_or_else(|| "region click: no frame available".to_string())?;
                            if rect.x < 0
                                || rect.y < 0
                                || rect.x + rect.w > fw as i32
                                || rect.y + rect.h > fh as i32
                            {
                                evidence.push(format!(
                                    "region `{sel}` rect ({},{},{},{}) is outside the frame ({}x{})",
                                    rect.x, rect.y, rect.w, rect.h, fw, fh
                                ));
                                continue;
                            }
                            let p = self.random_point_in(rect, 0.15);
                            let plan = self.humanize.point_click(p, &mut self.rng);
                            let (actual, _delay) = self.perform_click(
                                plan,
                                p,
                                Some(sel.clone()),
                                "region",
                                format!(
                                    "region:{sel}:({},{},{},{})",
                                    rect.x, rect.y, rect.w, rect.h
                                ),
                                fw,
                                fh,
                            )?;
                            self.remember_target(find, actual, fw, fh);
                            let verified = self.verify_if_present(expect.as_ref())?;
                            let miss = if plan.miss { " miss=true" } else { "" };
                            return Ok(format!(
                                "clicked_target locator=region sel={sel} point=({},{}){miss} verified={verified} guard={guarded}",
                                actual.x, actual.y
                            ));
                        }
                        Locator::CoordMemory { key } => {
                            let Some(store) = self.coords.as_mut() else {
                                evidence.push("coord_memory: no store attached".to_string());
                                continue;
                            };
                            match store.get(key, w, h) {
                                Some(p) => {
                                    let plan = self.humanize.point_click(p, &mut self.rng);
                                    let (actual, _delay) = self.perform_click(
                                        plan,
                                        p,
                                        None,
                                        "coord_memory",
                                        format!("coord_memory:{key}"),
                                        w,
                                        h,
                                    )?;
                                    let verified = self.verify_if_present(expect.as_ref())?;
                                    let miss = if plan.miss { " miss=true" } else { "" };
                                    return Ok(format!(
                                        "clicked_target locator=coord_memory key={key} point=({},{}){miss} verified={verified} guard={guarded}",
                                        actual.x, actual.y
                                    ));
                                }
                                None => {
                                    evidence.push(format!("coord_memory `{key}`: no remembered point"))
                                }
                            }
                        }
                    }
                }
                Err(format!(
                    "click_target: all locators failed: [{}]",
                    evidence.join("; ")
                ))
            }
            Step::Swipe {
                from_x,
                from_y,
                to_x,
                to_y,
                ms,
            } => {
                let fx = self.eval_coord(from_x, "swipe from_x")?;
                let fy = self.eval_coord(from_y, "swipe from_y")?;
                let tx = self.eval_coord(to_x, "swipe to_x")?;
                let ty = self.eval_coord(to_y, "swipe to_y")?;
                let from = Point {
                    x: fx.round() as i32,
                    y: fy.round() as i32,
                };
                let to = Point {
                    x: tx.round() as i32,
                    y: ty.round() as i32,
                };
                // Same CP3 posture as click_point: both endpoints must be
                // on-screen, otherwise the swipe is a loud failure.
                if self.matcher.is_none() {
                    self.refresh_frame()?;
                }
                let (w, h) = self
                    .frame_size
                    .ok_or_else(|| "swipe: no frame available".to_string())?;
                let outside = |p: Point| {
                    p.x < 0 || p.y < 0 || p.x >= w as i32 || p.y >= h as i32
                };
                if outside(from) || outside(to) {
                    return Err(format!(
                        "swipe ({},{})->({},{}) is outside the frame ({}x{})",
                        from.x, from.y, to.x, to.y, w, h
                    ));
                }
                // Deterministic (no jitter/delay): see Step::Swipe docs.
                self.input.swipe(from, to, *ms)?;
                self.invalidate_frame();
                Ok(format!(
                    "swiped from=({},{}) to=({},{}) ms={} frame={}x{}",
                    from.x, from.y, to.x, to.y, ms, w, h
                ))
            }
            Step::Drag {
                from,
                to,
                direction,
                distance,
                duration_ms,
                margin,
            } => {
                let src_rect = self.drag_target_rect(from)?;
                if self.matcher.is_none() {
                    self.refresh_frame()?;
                }
                let (w, h) = self
                    .frame_size
                    .ok_or_else(|| "drag: no frame available".to_string())?;
                let start = self.random_point_in(src_rect, *margin);
                let (ux, uy) = match direction {
                    DragAxis::Up => (0, -1),
                    DragAxis::Down => (0, 1),
                    DragAxis::Left => (-1, 0),
                    DragAxis::Right => (1, 0),
                };
                let inside = |p: Point| {
                    p.x >= 0 && p.y >= 0 && p.x < w as i32 && p.y < h as i32
                };
                if !inside(start) {
                    return Err(format!(
                        "drag start ({},{}) is outside the frame ({}x{})",
                        start.x, start.y, w, h
                    ));
                }
                let end = if let Some(target) = to {
                    let rect = self.drag_target_rect(target)?;
                    // Random end inside the target region, but only accept it when
                    // it really lies in `direction` (v1.1 方向性要求).
                    let mut picked = None;
                    for _ in 0..24 {
                        let p = self.random_point_in(rect, *margin);
                        let dx = p.x - start.x;
                        let dy = p.y - start.y;
                        let dot = dx * ux + dy * uy;
                        let dist2 = dx * dx + dy * dy;
                        if dot > 0 && dist2 >= 20 * 20 && inside(p) {
                            picked = Some(p);
                            break;
                        }
                    }
                    picked.ok_or_else(|| {
                        format!(
                            "drag target in direction `{}` is unreachable from ({},{}) (no point with positive {}-ward distance)",
                            direction.as_str(),
                            start.x,
                            start.y,
                            direction.as_str()
                        )
                    })?
                } else {
                    let range = distance.ok_or_else(|| "drag needs to or distance".to_string())?;
                    let span = (range[1] - range[0]).max(1) as f64;
                    let len = range[0] + (self.rng.unit_f64() * span).round() as i32;
                    let p = Point {
                        x: start.x + ux * len,
                        y: start.y + uy * len,
                    };
                    if !inside(p) {
                        return Err(format!(
                            "drag distance {len} in direction `{}` from ({},{}) leaves the frame ({}x{})",
                            direction.as_str(),
                            start.x,
                            start.y,
                            w,
                            h
                        ));
                    }
                    p
                };
                let span = (duration_ms[1] - duration_ms[0] + 1) as f64;
                let ms = duration_ms[0] + (self.rng.unit_f64() * span) as u32;
                if ms == 0 {
                    return Err("drag duration must be > 0".into());
                }
                self.input.swipe(start, end, ms)?;
                self.invalidate_frame();
                Ok(format!(
                    "dragged direction={} from=({},{}) to=({},{}) ms={} randomized=true frame={}x{}",
                    direction.as_str(),
                    start.x,
                    start.y,
                    end.x,
                    end.y,
                    ms,
                    w,
                    h
                ))
            }
            Step::SetVar { name, value } => {
                let v = eval(value, self)?;
                self.vars.insert(name.clone(), v.clone());
                Ok(format!("set_var {name}={v:?}"))
            }
            Step::IncVar { name, by } => {
                let by_v = eval(by, self)?;
                let delta = match by_v {
                    Value::Num(n) => n,
                    other => return Err(format!("inc_var({name}): `by` must be a number, got {other:?}")),
                };
                let current = match self.vars.get(name) {
                    Some(Value::Num(n)) => *n,
                    Some(other) => {
                        return Err(format!(
                            "inc_var({name}): variable exists but is not a number: {other:?}"
                        ))
                    }
                    None => 0.0,
                };
                let next = current + delta;
                self.vars.insert(name.clone(), Value::Num(next));
                Ok(format!("inc_var {name} += {delta} -> {next}"))
            }
            Step::OcrToVar { sel, var } => {
                let text = match self.ocr_read(sel)? {
                    OcrRead::Single(Some(t), _) => t,
                    OcrRead::Single(None, _) => String::new(),
                    OcrRead::Lines(lines) => {
                        lines.iter().map(|l| l.text.clone()).collect::<Vec<_>>().join("\n")
                    }
                };
                self.vars.insert(var.clone(), Value::Str(text.clone()));
                Ok(format!("ocr_to_var {sel} -> {var} = {text:?}"))
            }
            Step::Ocr { sel } => {
                // The standalone `ocr` step exists so an editor-authored flow can
                // PROVE what a selector reads without branching on it (a plain
                // expression has no step log). `ocr_lines` assets log EVERY line
                // (det + rec), so a multi-line region stays inspectable (§17.1).
                // Empty text is reported, not failed: the loud failure is only a
                // missing provider / asset / det model.
                match self.ocr_read(sel)? {
                    OcrRead::Single(text, score) => Ok(match text {
                        Some(t) => format!("ocr {sel} = {t:?} score={score:.4}"),
                        None => format!("ocr {sel} = <no text> score={score:.4}"),
                    }),
                    OcrRead::Lines(lines) => {
                        let parts: Vec<String> = lines
                            .iter()
                            .map(|l| {
                                format!(
                                    "{:?}@({},{},{},{})={:.4}",
                                    l.text, l.rect.x, l.rect.y, l.rect.w, l.rect.h, l.score
                                )
                            })
                            .collect();
                        Ok(format!(
                            "ocr {sel} = {} line(s): {}",
                            lines.len(),
                            parts.join(" | ")
                        ))
                    }
                }
            }
            Step::Log { level, text } => {
                let v = eval(text, self)?;
                Ok(format!("[{level}] {v:?}"))
            }
            Step::CallTask { task } => Err(format!(
                "call_task `{task}` was not expanded by the task loader; \
                 run the flow through oasd/TaskBundle::load_flow"
            )),
            Step::Finish { exit } => {
                *ctl = Ctl::Finish(*exit);
                Ok(format!("finish {exit:?}"))
            }
        }
    }
}

/// Marker error used to signal budget/loop exhaustion (mapped to Exhausted).
const EXHAUSTED: &str = "__exhausted__";

/// §22.15: a step may be preceded by at most this many popup dismissals before
/// the run fails loudly (a rule that always matches would otherwise spin).
const MAX_INTERRUPTS_PER_STEP: usize = 5;

/// ENGINE §22.16: the retry policy of a step (action steps only).
fn step_retry(step: &Step) -> Option<oas_flow::RetryPolicy> {
    match step {
        Step::Click { retry, .. }
        | Step::ClickPoint { retry, .. }
        | Step::ClickTarget { retry, .. }
        | Step::DismissPopup { retry, .. } => *retry,
        _ => None,
    }
}

fn step_name(s: &Step) -> &'static str {
    match s {
        Step::If { .. } => "if",
        Step::Loop { .. } => "loop",
        Step::While { .. } => "while",
        Step::Break => "break",
        Step::Continue => "continue",
        Step::Wait { .. } => "wait",
        Step::WaitUntil { .. } => "wait_until",
        Step::Appear { .. } => "appear",
        Step::Click { .. } => "click",
        Step::ClickPoint { .. } => "click_point",
        Step::Swipe { .. } => "swipe",
        Step::Drag { .. } => "drag",
        Step::Ocr { .. } => "ocr",
        Step::SetVar { .. } => "set_var",
        Step::IncVar { .. } => "inc_var",
        Step::OcrToVar { .. } => "ocr_to_var",
        Step::DismissPopup { .. } => "dismiss_popup",
        Step::ClickTarget { .. } => "click_target",
        Step::CallTask { .. } => "call_task",
        Step::Log { .. } => "log",
        Step::Finish { .. } => "finish",
    }
}

impl<S: ScreenSource, I: InputSink> EvalContext for Interpreter<S, I> {
    fn appear(&mut self, sel: &str) -> Result<bool, String> {
        // "出现素材" covers every asset kind (user 10-31): OCR assets are
        // visible when their text can be read; template/collection assets use
        // the normal locate path.
        if self.assets.ocr_roi(sel).is_some() {
            if self.ocr.is_none() {
                return Err(format!(
                    "OCR is not available for appear({sel}): no OCR provider attached"
                ));
            }
            return match self.ocr_read(sel) {
                Ok(OcrRead::Single(text, _)) => Ok(text.map(|t| !t.trim().is_empty()).unwrap_or(false)),
                Ok(OcrRead::Lines(lines)) => Ok(!lines.is_empty()),
                // A multi-line-derived line that is not found means "not visible".
                Err(e) if e.starts_with("derived ocr_line") => Ok(false),
                Err(e) => Err(e),
            };
        }
        let (score, _) = self.locate(sel)?;
        Ok(score >= self.assets.threshold_for(sel).unwrap_or(0.9))
    }

    fn ocr(&mut self, sel: &str) -> Result<Option<String>, String> {
        Ok(self.ocr_run(sel)?.0)
    }

    fn ocr_digit(&mut self, sel: &str) -> Result<Option<f64>, String> {
        self.ocr_digit_value(sel)
    }

    fn page(&mut self) -> Result<Option<String>, String> {
        Ok(None) // page recognition lands in a later phase
    }

    fn text_visible(&mut self, text: &str) -> Result<bool, String> {
        let target = text.trim();
        if target.is_empty() {
            return Ok(false);
        }
        if self.ocr.is_none() {
            return Err("text_visible() requires OCR (no OCR provider attached)".to_string());
        }
        if self.frame.is_none() {
            self.refresh_frame()?;
        }
        let frame = self
            .frame
            .as_ref()
            .ok_or_else(|| "text_visible(): no frame available".to_string())?
            .clone();
        let roi = oas_types::Rect {
            x: 0,
            y: 0,
            w: frame.width as i32,
            h: frame.height as i32,
        };
        let provider = self.ocr.as_mut().expect("checked above");
        let lines = provider.recognize_lines(&frame, roi)?;
        Ok(lines.iter().any(|l| l.text.trim() == target))
    }

    fn param(&mut self, key: &str) -> Result<Value, String> {
        self.params.get(key).cloned().ok_or_else(|| {
            format!(
                "param('{key}') has no value: declare it in meta.params (with a default) \
                 or pass --param {key}=<value>"
            )
        })
    }

    fn var(&mut self, name: &str) -> Result<Value, String> {
        self.vars.get(name).cloned().ok_or_else(|| {
            format!(
                "var('{name}') is not set: use set_var/inc_var/ocr_to_var earlier in the flow"
            )
        })
    }
}

/// `ocr_digit(sel)` helper: the **first run of digits** in the recognised text,
/// e.g. `"12/30" -> 12`, `"体力: 3.5" -> 3.5`, `"无" -> None`.
fn parse_digit(text: &str) -> Option<f64> {
    let start = text.find(|c: char| c.is_ascii_digit())?;
    let rest = &text[start..];
    let end = rest
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(rest.len());
    rest[..end].parse::<f64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use oas_flow::Flow;

    /// A 64x48 frame with the patterned 10x10 tile pasted at `(tx, ty)`.
    fn frame_with_tile(tx: u32, ty: u32) -> Frame {
        let (w, h) = (64u32, 48u32);
        let mut rgba = vec![30u8; (w * h * 4) as usize];
        for y in 0..10u32 {
            for x in 0..10u32 {
                let v = (((x * 7 + y * 13) % 251) + 3) as u8;
                let i = (((ty + y) * w + (tx + x)) * 4) as usize;
                rgba[i] = v;
                rgba[i + 1] = v;
                rgba[i + 2] = v;
                rgba[i + 3] = 255;
            }
        }
        Frame::new(w, h, rgba)
    }

    /// Fake screen: one 64x48 frame with a bright 10x10 block at (12,10).
    struct FakeScreen {
        frame: Frame,
        grabs: usize,
    }

    impl FakeScreen {
        fn new() -> Self {
            Self {
                frame: frame_with_tile(12, 10),
                grabs: 0,
            }
        }
    }

    impl ScreenSource for FakeScreen {
        fn grab(&mut self) -> Result<Frame, String> {
            self.grabs += 1;
            Ok(self.frame.clone())
        }
    }

    /// Screen whose grabs return successive frames (the last one repeats), so a
    /// test can prove that a click forces the next selector to re-grab.
    struct TwoFrameScreen {
        frames: Vec<Frame>,
        idx: usize,
    }

    impl ScreenSource for TwoFrameScreen {
        fn grab(&mut self) -> Result<Frame, String> {
            let i = self.idx.min(self.frames.len() - 1);
            self.idx += 1;
            Ok(self.frames[i].clone())
        }
    }

    /// Input sink whose recorded clicks/swipes can be inspected after `run()`.
    #[derive(Clone, Default)]
    struct SharedInput {
        clicks: std::rc::Rc<std::cell::RefCell<Vec<Point>>>,
        swipes: std::rc::Rc<std::cell::RefCell<Vec<(Point, Point, u32)>>>,
    }

    struct FakeInput {
        shared: SharedInput,
    }

    impl FakeInput {
        fn new() -> (Self, SharedInput) {
            let shared = SharedInput::default();
            (
                Self {
                    shared: shared.clone(),
                },
                shared,
            )
        }
    }

    impl InputSink for FakeInput {
        fn click(&mut self, p: Point) -> Result<(), String> {
            self.shared.clicks.borrow_mut().push(p);
            Ok(())
        }
        fn swipe(&mut self, from: Point, to: Point, ms: u32) -> Result<(), String> {
            self.shared.swipes.borrow_mut().push((from, to, ms));
            Ok(())
        }
    }

    /// Fake OCR provider (P1-c): scripted `(text, score)` replies, one per call;
    /// the last reply repeats. Records the ROI of every call so a test can prove
    /// the interpreter used the **declared asset's** ROI.
    struct FakeOcr {
        replies: std::collections::VecDeque<(String, f32)>,
        rois: std::rc::Rc<std::cell::RefCell<Vec<Rect>>>,
    }

    impl FakeOcr {
        fn new(replies: &[(&str, f32)]) -> (Self, std::rc::Rc<std::cell::RefCell<Vec<Rect>>>) {
            let rois = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
            (
                Self {
                    replies: replies
                        .iter()
                        .map(|(t, s)| (t.to_string(), *s))
                        .collect(),
                    rois: rois.clone(),
                },
                rois,
            )
        }
    }

    impl OcrProvider for FakeOcr {
        fn recognize(&mut self, _frame: &Frame, roi: Rect) -> Result<(String, f32), String> {
            self.rois.borrow_mut().push(roi);
            if let Some(reply) = self.replies.pop_front() {
                if self.replies.is_empty() {
                    self.replies.push_back(reply.clone());
                }
                Ok(reply)
            } else {
                Err("FakeOcr has no scripted reply".into())
            }
        }

        fn recognize_lines(&mut self, _frame: &Frame, _roi: Rect) -> Result<Vec<OcrLine>, String> {
            Err("FakeOcr only supports single-line rec".into())
        }
    }

    /// Fake det+rec provider for `ocr_lines` assets.
    struct FakeDetOcr {
        lines: Vec<OcrLine>,
        rois: std::rc::Rc<std::cell::RefCell<Vec<Rect>>>,
    }

    impl FakeDetOcr {
        fn new(lines: Vec<OcrLine>) -> (Self, std::rc::Rc<std::cell::RefCell<Vec<Rect>>>) {
            let rois = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
            (
                Self {
                    lines,
                    rois: rois.clone(),
                },
                rois,
            )
        }
    }

    impl OcrProvider for FakeDetOcr {
        fn recognize(&mut self, _frame: &Frame, _roi: Rect) -> Result<(String, f32), String> {
            Err("FakeDetOcr only supports det+rec".into())
        }

        fn recognize_lines(&mut self, _frame: &Frame, roi: Rect) -> Result<Vec<OcrLine>, String> {
            self.rois.borrow_mut().push(roi);
            Ok(self.lines.clone())
        }
    }

    fn line(text: &str, score: f32, rect: Rect) -> OcrLine {
        OcrLine {
            rect,
            text: text.to_string(),
            score,
        }
    }

    fn assets() -> AssetSet {
        let (w, h) = (10u32, 10u32);
        // NOTE: a FLAT template has zero variance => NCC is undefined (and our
        // matcher correctly returns None). Use a patterned 10x10 tile instead.
        let pattern: Vec<u8> = (0..(w * h))
            .map(|i| (((i % w) * 7 + (i / w) * 13) % 251 + 3) as u8)
            .collect();
        let gray = GrayImage::new(w, h, pattern);
        let mut set = AssetSet::new();
        set.insert(
            "I_BRIGHT",
            Template {
                gray,
                roi: None,
                threshold: 0.8,
                click_polygon: None,
            },
        );
        // a dark template that cannot match anything in our frame
        let dark = GrayImage::new(w, h, (0..(w * h)).map(|i| (i % 7) as u8).collect());
        set.insert(
            "I_NEVER",
            Template {
                gray: dark,
                roi: Some(Rect { x: 0, y: 0, w: 64, h: 48 }),
                threshold: 0.9,
                click_polygon: None,
            },
        );
        set
    }

    /// Test default: misses disabled, otherwise a random 1~3% click in unit
    /// tests would make bounded-hit assertions flaky.
    fn no_miss() -> crate::humanize::Humanize {
        crate::humanize::Humanize {
            miss_chance_min: 0.0,
            miss_chance_max: 0.0,
            ..crate::humanize::Humanize::default()
        }
    }

    fn mk(src: FakeScreen, budget: Budget) -> Interpreter<FakeScreen, FakeInput> {
        Interpreter::new(src, FakeInput::new().0, assets(), budget).with_humanize(no_miss())
    }

    /// `assets()` plus two declared OCR assets (`ocr_line` kind → ROI only).
    fn assets_with_ocr() -> AssetSet {
        let mut set = assets();
        set.insert_ocr("O_TEXT", Rect { x: 4, y: 6, w: 20, h: 10 });
        set.insert_ocr("O_DIGITS", Rect { x: 4, y: 30, w: 30, h: 10 });
        set.insert_ocr_lines("O_LIST", Rect { x: 0, y: 0, w: 64, h: 48 });
        set
    }

    /// OCR provider whose text is derived from the **frame itself** (`f<tag>`,
    /// tag = `rgba[0]`). This is what makes "did the interpreter drop the cached
    /// frame?" observable.
    struct FrameTagOcr {
        calls: std::rc::Rc<std::cell::RefCell<usize>>,
    }

    impl OcrProvider for FrameTagOcr {
        fn recognize(&mut self, frame: &Frame, _roi: Rect) -> Result<(String, f32), String> {
            *self.calls.borrow_mut() += 1;
            Ok((format!("f{}", frame.rgba[0]), 0.9))
        }

        fn recognize_lines(&mut self, frame: &Frame, roi: Rect) -> Result<Vec<OcrLine>, String> {
            let (text, score) = self.recognize(frame, roi)?;
            Ok(vec![OcrLine {
                rect: roi,
                text,
                score,
            }])
        }
    }

    fn tagged_frame(tag: u8) -> Frame {
        let mut frame = frame_with_tile(12, 10);
        frame.rgba[0] = tag;
        frame
    }

    type OcrInterp = Interpreter<FakeScreen, FakeInput>;
    type OcrRois = std::rc::Rc<std::cell::RefCell<Vec<Rect>>>;

    /// Interpreter with a scripted [`FakeOcr`] attached.
    fn mk_ocr(replies: &[(&str, f32)]) -> (OcrInterp, OcrRois) {
        let (input, _) = FakeInput::new();
        let (fake, rois) = FakeOcr::new(replies);
        let interp = Interpreter::new(
            FakeScreen::new(),
            input,
            assets_with_ocr(),
            Budget::default(),
        )
        .with_ocr(Box::new(fake));
        (interp, rois)
    }

    fn mk_inspect<S: ScreenSource>(src: S) -> (Interpreter<S, FakeInput>, SharedInput) {
        let (input, shared) = FakeInput::new();
        (
            Interpreter::new(src, input, assets(), Budget::default()).with_humanize(no_miss()),
            shared,
        )
    }

    fn flow(json: &str) -> Flow {
        Flow::from_json(json).expect("flow should load")
    }

    /// Parse the `key=(a,b)` tuple out of a step detail.
    fn detail_tuple(detail: &str, key: &str) -> (i32, i32) {
        let marker = format!("{key}=(");
        let i = detail
            .find(&marker)
            .unwrap_or_else(|| panic!("no `{marker}` in `{detail}`"))
            + marker.len();
        let rest = &detail[i..];
        let end = rest.find(')').expect("closing paren");
        let mut it = rest[..end].split(',');
        let x = it.next().expect("x").trim().parse().expect("numeric x");
        let y = it.next().expect("y").trim().parse().expect("numeric y");
        (x, y)
    }

    fn detail_point(detail: &str) -> Point {
        let (x, y) = detail_tuple(detail, "point");
        Point { x, y }
    }

    fn detail_delay_ms(detail: &str) -> u32 {
        let i = detail.find("delay=").expect("delay field") + "delay=".len();
        let rest = &detail[i..];
        let end = rest.find("ms").expect("ms suffix");
        rest[..end].parse().expect("numeric delay")
    }

    #[test]
    fn straight_line_logs_each_step_and_finishes() {
        let mut interp = mk(FakeScreen::new(), Budget::default());
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"appear","args":{"sel":"I_BRIGHT"}},
                 {"op":"click","args":{"sel":"I_BRIGHT"}},
                 {"op":"log","args":{"text":"'clicked'"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let r = interp.run(&f);
        assert_eq!(r, RunResult::Finished(Exit::Done));
        assert_eq!(r.outcome(), Outcome::Done);
        let ops: Vec<&str> = interp.events().iter().map(|e| e.op.as_str()).collect();
        assert_eq!(ops, vec!["appear", "click", "log", "finish"]);
        assert!(interp.events().iter().all(|e| e.result == "ok"));
        let click = &interp.events()[1];
        // HB1: the target box is the matched 10x10 template rect at (12,10) ...
        assert_eq!(detail_tuple(&click.detail, "target"), (12, 10));
        // ... and the ACTUAL humanised point lies inside that template rect
        // (not in the old hard-coded 11x11 box hugging the match's top-left).
        let p = detail_point(&click.detail);
        assert!(
            (12..22).contains(&p.x) && (10..20).contains(&p.y),
            "actual point {p:?} must be inside the matched template rect: {}",
            click.detail
        );
        assert!((30..=120).contains(&detail_delay_ms(&click.detail)));
    }

    #[test]
    fn collection_locates_best_member_and_uses_its_click_box() {
        let mut set = assets();
        let base = set.get("I_BRIGHT").expect("bright template").clone();
        set.insert(
            "I_BRIGHT_ROI",
            Template {
                roi: Some(Rect {
                    x: 12,
                    y: 10,
                    w: 10,
                    h: 10,
                }),
                ..base
            },
        );
        set.insert_collection(
            "C_BRIGHT",
            vec!["I_NEVER".to_string(), "I_BRIGHT_ROI".to_string()],
            None,
        );
        set.set_click_back(
            "I_BRIGHT_ROI",
            Rect {
                x: 14,
                y: 12,
                w: 6,
                h: 6,
            },
        );
        let mut interp = Interpreter::new(
            FakeScreen::new(),
            FakeInput::new().0,
            set,
            Budget::default(),
        )
        .with_humanize(no_miss());
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click","args":{"sel":"C_BRIGHT"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let click = &interp.events()[0];
        let p = detail_point(&click.detail);
        assert!(
            (14..20).contains(&p.x) && (12..18).contains(&p.y),
            "collection click must use the winning member action_roi: {}",
            click.detail
        );
    }

    #[test]
    fn missing_click_target_fails_loudly() {
        let mut interp = mk(FakeScreen::new(), Budget::default());
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click","args":{"sel":"I_NEVER"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let r = interp.run(&f);
        match &r {
            RunResult::Failed(msg) => assert!(msg.contains("not found"), "{msg}"),
            other => panic!("expected Failed, got {other:?}"),
        }
        assert_eq!(r.outcome(), Outcome::Failed);
        // the failure is visible in the event log with the failing step
        let last = interp.events().last().expect("event");
        assert_eq!(last.op, "click");
        assert_eq!(last.result, "failed");
    }

    #[test]
    fn undeclared_selector_is_an_error_not_a_skip() {
        let mut interp = mk(FakeScreen::new(), Budget::default());
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click","args":{"sel":"I_MISSING"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        match interp.run(&f) {
            RunResult::Failed(msg) => {
                assert!(msg.contains("not declared"), "{msg}")
            }
            other => panic!("expected Failed, got {other:?}"),
        }
    }

    #[test]
    fn while_exhaustion_is_exhausted_not_success() {
        let mut interp = mk(
            FakeScreen::new(),
            Budget {
                max_wall_s: 60.0,
                max_steps: 5,
            },
        );
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"while","args":{"cond":"true"},
                  "body":[{"op":"log","args":{"text":"'spin'"}}]},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let r = interp.run(&f);
        assert_eq!(r.outcome(), Outcome::Exhausted);
    }

    #[test]
    fn appear_supports_ocr_assets() {
        let (mut interp, _) = mk_ocr(&[("完成", 0.9)]);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"if","args":{"cond":"appear('O_TEXT')"},
                  "then":[{"op":"finish","args":{"exit":"done"}}],
                  "else":[{"op":"finish","args":{"exit":"failed"}}]}]}"#,
        );
        assert_eq!(interp.run(&f).outcome(), Outcome::Done);
    }

    #[test]
    fn loop_zero_or_negative_runs_zero_iterations() {
        for n in ["0", "-3"] {
            let mut interp = mk(FakeScreen::new(), Budget::default());
            let raw = r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"loop","args":{"n":"__N__"},
                  "body":[{"op":"click","args":{"sel":"I_MISSING"}}]},
                 {"op":"finish","args":{"exit":"done"}}]}"#
                .replace("__N__", n);
            let f = flow(&raw);
            assert_eq!(
                interp.run(&f).outcome(),
                Outcome::Done,
                "loop n={n} must not enter body"
            );
        }
    }

    #[test]
    fn while_numeric_zero_does_not_enter_loop() {
        let mut interp = mk(FakeScreen::new(), Budget::default());
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"while","args":{"cond":"0"},
                  "body":[{"op":"log","args":{"text":"'must not run'"}}]},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f).outcome(), Outcome::Done);
    }

    #[test]
    fn step_budget_is_exhausted_not_success() {
        let mut interp = mk(
            FakeScreen::new(),
            Budget {
                max_wall_s: 60.0,
                max_steps: 3,
            },
        );
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"log","args":{"text":"1"}},
                 {"op":"log","args":{"text":"2"}},
                 {"op":"log","args":{"text":"3"}},
                 {"op":"log","args":{"text":"4"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f).outcome(), Outcome::Exhausted);
    }

    #[test]
    fn wait_until_timeout_fails() {
        let mut interp = mk(FakeScreen::new(), Budget::default());
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"wait_until","args":{"cond":"appear('I_NEVER')","timeout_s":0.2}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        match interp.run(&f) {
            RunResult::Failed(msg) => assert!(msg.contains("timed out"), "{msg}"),
            other => panic!("expected Failed, got {other:?}"),
        }
    }

    // ---- OCR wiring (P1-c · ENGINE-DESIGN §16.4 / §22.4) ----

    /// The provider is the only source of text: the step log and the expression
    /// evaluator both read the asset's declared ROI (no hard-coded coordinates).
    #[test]
    fn ocr_reads_the_declared_roi_and_reports_text() {
        let (mut interp, rois) = mk_ocr(&[("分组2", 0.9987)]);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"ocr","args":{"sel":"O_TEXT"}},
                 {"op":"log","args":{"text":"ocr('O_TEXT')"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(
            interp.run(&f),
            RunResult::Finished(Exit::Done),
            "{:?}",
            interp.events()
        );
        let ev = &interp.events()[0];
        assert_eq!(ev.op, "ocr");
        assert!(ev.detail.contains("分组2"), "{}", ev.detail);
        assert!(ev.detail.contains("0.9987"), "{}", ev.detail);
        assert!(
            interp.events()[1].detail.contains("分组2"),
            "{:?}",
            interp.events()[1]
        );
        // Both calls used exactly the declared OCR asset ROI.
        assert_eq!(
            rois.borrow().as_slice(),
            &[Rect { x: 4, y: 6, w: 20, h: 10 }; 2]
        );
    }

    /// Empty recognition is `Null`, never `""`: it must not satisfy a comparison.
    #[test]
    fn ocr_empty_text_is_null_and_never_matches() {
        let (mut interp, _) = mk_ocr(&[("", 0.0)]);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"log","args":{"text":"ocr('O_TEXT')"}},
                 {"op":"if","args":{"cond":"ocr('O_TEXT') == '分组2'"},
                  "then":[{"op":"log","args":{"text":"'matched'"}}],
                  "else":[{"op":"log","args":{"text":"'no match'"}}]},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        assert!(
            interp.events()[0].detail.contains("Null"),
            "{:?}",
            interp.events()[0]
        );
        // Nested steps are logged before their parent `if` event.
        assert!(
            interp.events()[1].detail.contains("no match"),
            "{:?}",
            interp.events()[1]
        );
        assert!(
            interp.events()[2].detail.contains("cond -> false"),
            "if cond must be false: {:?}",
            interp.events()[2]
        );
    }

    /// `wait_until(ocr(...) == ...)` polls **fresh frames**: the interpreter must
    /// drop the cached frame between attempts, otherwise a replay would keep
    /// OCR-ing one frame forever and the wait could never see the next screen.
    #[test]
    fn wait_until_ocr_polls_fresh_frames() {
        let src = TwoFrameScreen {
            frames: vec![tagged_frame(1), tagged_frame(2)],
            idx: 0,
        };
        let (input, _) = FakeInput::new();
        let calls = std::rc::Rc::new(std::cell::RefCell::new(0usize));
        let mut interp = Interpreter::new(src, input, assets_with_ocr(), Budget::default())
            .with_ocr(Box::new(FrameTagOcr {
                calls: calls.clone(),
            }));
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"wait_until","args":{"cond":"ocr('O_TEXT') == 'f2'","timeout_s":0.3}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(
            interp.run(&f),
            RunResult::Finished(Exit::Done),
            "{:?}",
            interp.events()
        );
        assert!(*calls.borrow() >= 2, "must poll at least twice");
    }

    /// `ocr_digit` takes the first number in the recognised text ("12/30" → 12).
    #[test]
    fn ocr_digit_parses_the_first_number() {
        let (mut interp, _) = mk_ocr(&[("12/30", 0.97)]);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"log","args":{"text":"ocr_digit('O_DIGITS')"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        assert!(
            interp.events()[0].detail.contains("12.0"),
            "{:?}",
            interp.events()[0]
        );
    }

    /// P1-c regression guard: without a provider `ocr()` must FAIL LOUDLY — it
    /// must never degrade to `""`/`Null` (that would make an OCR-dependent flow
    /// silently pass, the exact bug this rewrite exists to kill).
    #[test]
    fn ocr_without_a_provider_fails_loudly() {
        let (input, _) = FakeInput::new();
        let mut interp =
            Interpreter::new(FakeScreen::new(), input, assets_with_ocr(), Budget::default());
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"if","args":{"cond":"ocr('O_TEXT') == '1'"},
                  "then":[{"op":"log","args":{"text":"'one'"}}]},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        match interp.run(&f) {
            RunResult::Failed(msg) => assert!(
                msg.contains("no OCR provider") && msg.contains("refusing to fake"),
                "{msg}"
            ),
            other => panic!("expected Failed (no provider), got {other:?}"),
        }
    }

    #[test]
    fn ocr_on_a_template_asset_is_a_loud_error() {
        let (mut interp, _) = mk_ocr(&[("x", 0.5)]);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"ocr","args":{"sel":"I_BRIGHT"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        match interp.run(&f) {
            RunResult::Failed(msg) => assert!(msg.contains("not an OCR asset"), "{msg}"),
            other => panic!("expected Failed, got {other:?}"),
        }
    }

    #[test]
    fn ocr_undeclared_asset_is_a_loud_error() {
        let (mut interp, _) = mk_ocr(&[("x", 0.5)]);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"ocr","args":{"sel":"O_MISSING"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        match interp.run(&f) {
            RunResult::Failed(msg) => assert!(msg.contains("not declared"), "{msg}"),
            other => panic!("expected Failed, got {other:?}"),
        }
    }

    // ---- multi-line OCR (`ocr_lines` => det + rec · ENGINE-DESIGN §17.1) ----

    #[test]
    fn text_visible_searches_full_screen_multiline_ocr() {
        let (input, _) = FakeInput::new();
        let (fake, _rois) = FakeDetOcr::new(vec![
            line("町中", 0.9999, Rect { x: 4, y: 6, w: 40, h: 10 }),
            line("庭院", 0.9998, Rect { x: 4, y: 20, w: 30, h: 10 }),
        ]);
        let mut interp = Interpreter::new(FakeScreen::new(), input, assets_with_ocr(), Budget::default())
            .with_ocr(Box::new(fake));
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"if","args":{"cond":"text_visible('町中')"},
                  "then":[{"op":"finish","args":{"exit":"done"}}],
                  "else":[{"op":"finish","args":{"exit":"failed"}}]}]}"#,
        );
        assert_eq!(interp.run(&f).outcome(), Outcome::Done);
    }

    #[test]
    fn ocr_lines_asset_uses_det_and_logs_every_line() {
        let (input, _) = FakeInput::new();
        let (fake, rois) = FakeDetOcr::new(vec![
            line("默认分组", 0.9999, Rect { x: 4, y: 6, w: 40, h: 10 }),
            line("分组2", 0.9998, Rect { x: 4, y: 20, w: 30, h: 10 }),
        ]);
        let mut interp = Interpreter::new(FakeScreen::new(), input, assets_with_ocr(), Budget::default())
            .with_ocr(Box::new(fake));
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"ocr","args":{"sel":"O_LIST"}},
                 {"op":"log","args":{"text":"ocr('O_LIST')"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(
            interp.run(&f),
            RunResult::Finished(Exit::Done),
            "{:?}",
            interp.events()
        );
        let ev = &interp.events()[0];
        assert!(ev.detail.contains("2 line(s)"), "{}", ev.detail);
        assert!(
            ev.detail.contains("默认分组") && ev.detail.contains("分组2"),
            "{}",
            ev.detail
        );
        // The expression view joins lines with `\n` (Debug escapes it).
        assert!(
            interp.events()[1].detail.contains("默认分组\\n分组2"),
            "{:?}",
            interp.events()[1]
        );
        // Two reads: the `ocr` step and the `log(ocr(...))` expression.
        assert_eq!(
            rois.borrow().as_slice(),
            &[Rect { x: 0, y: 0, w: 64, h: 48 }; 2]
        );
    }

    /// An `ocr_lines` asset must NEVER go through the single-line rec path —
    /// FakeOcr (rec-only) errors, which proves `ocr_read` branched correctly.
    #[test]
    fn ocr_lines_never_uses_the_single_line_path() {
        let (mut interp, _) = mk_ocr(&[("should-not-be-used", 0.5)]);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"ocr","args":{"sel":"O_LIST"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        match interp.run(&f) {
            RunResult::Failed(msg) => assert!(msg.contains("single-line rec"), "{msg}"),
            other => panic!("expected Failed (det path), got {other:?}"),
        }
    }

    #[test]
    fn flow_without_finish_is_a_failure() {
        let mut interp = mk(FakeScreen::new(), Budget::default());
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"log","args":{"text":"'no exit'"}}]}"#,
        );
        match interp.run(&f) {
            RunResult::Failed(msg) => assert!(msg.contains("finish"), "{msg}"),
            other => panic!("expected Failed, got {other:?}"),
        }
    }

    // ---- click_point (design doc §19 CP1-CP4, §20 HB2/HB3/HB6) ----

    #[test]
    fn click_point_literal_is_humanised_bounded_and_audited() {
        let (mut interp, input) = mk_inspect(FakeScreen::new());
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click_point","args":{"x":20,"y":20}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let r = interp.run(&f);
        assert_eq!(r, RunResult::Finished(Exit::Done), "events: {:?}", interp.events());
        let ev = &interp.events()[0];
        assert_eq!(ev.op, "click_point");
        assert_eq!(ev.result, "ok");
        // CP4: the exact requested target and the frame it was validated against
        assert_eq!(detail_tuple(&ev.detail, "target"), (20, 20));
        assert!(ev.detail.contains("frame=64x48"), "{}", ev.detail);
        // HB2: actual point within +/-6px of the target, and on-screen
        let actual = detail_point(&ev.detail);
        let clicks = input.clicks.borrow();
        assert_eq!(clicks.len(), 1, "exactly one click");
        assert_eq!(clicks[0], actual, "HB6: the log must carry the ACTUAL point");
        assert!((actual.x - 20).abs() <= 6 && (actual.y - 20).abs() <= 6, "{actual:?}");
        assert!(
            (0..64).contains(&actual.x) && (0..48).contains(&actual.y),
            "actual point must stay on-screen: {actual:?}"
        );
        // HB3: randomised pre-click delay inside the documented band
        let delay = detail_delay_ms(&ev.detail);
        assert!((30..=120).contains(&delay), "delay {delay} out of range");
    }

    #[test]
    fn click_point_evaluates_expression_coordinates() {
        // CP2: coordinates are expressions (arithmetic, then rounded to i32).
        let (mut interp, input) = mk_inspect(FakeScreen::new());
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click_point","args":{"x":"15 + 5","y":"4 * 5"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let ev = &interp.events()[0];
        assert_eq!(detail_tuple(&ev.detail, "target"), (20, 20), "{}", ev.detail);
        let actual = detail_point(&ev.detail);
        assert!((actual.x - 20).abs() <= 6 && (actual.y - 20).abs() <= 6, "{actual:?}");
        assert_eq!(input.clicks.borrow().len(), 1);
    }

    #[test]
    fn click_point_out_of_frame_fails_loudly_without_clicking() {
        let (mut interp, input) = mk_inspect(FakeScreen::new());
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click_point","args":{"x":64,"y":20}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let r = interp.run(&f);
        match &r {
            RunResult::Failed(msg) => {
                assert!(msg.contains("outside the frame"), "{msg}");
                assert!(msg.contains("64x48"), "{msg}");
            }
            other => panic!("expected Failed, got {other:?}"),
        }
        assert_eq!(r.outcome(), Outcome::Failed);
        // CP3: no InputSink click may happen for an off-screen target
        assert!(input.clicks.borrow().is_empty(), "off-screen click must not be sent");
        let last = interp.events().last().expect("event");
        assert_eq!(last.op, "click_point");
        assert_eq!(last.result, "failed");
    }

    #[test]
    fn click_point_negative_coordinate_fails_loudly() {
        let (mut interp, input) = mk_inspect(FakeScreen::new());
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click_point","args":{"x":20,"y":-1}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let r = interp.run(&f);
        match &r {
            RunResult::Failed(msg) => assert!(msg.contains("outside the frame"), "{msg}"),
            other => panic!("expected Failed, got {other:?}"),
        }
        assert!(input.clicks.borrow().is_empty());
    }

    #[test]
    fn click_invalidates_frame_cache() {
        // Frame 1 has the tile at (12,10); after the first click the game shows
        // frame 2, where the same tile is at (40,20). The second click must
        // match against the NEW frame, not the cached pre-click one.
        let src = TwoFrameScreen {
            frames: vec![frame_with_tile(12, 10), frame_with_tile(40, 20)],
            idx: 0,
        };
        let (mut interp, _input) = mk_inspect(src);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click","args":{"sel":"I_BRIGHT"}},
                 {"op":"click","args":{"sel":"I_BRIGHT"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let events = interp.events();
        assert_eq!(detail_tuple(&events[0].detail, "target"), (12, 10), "{}", events[0].detail);
        assert_eq!(
            detail_tuple(&events[1].detail, "target"),
            (40, 20),
            "second click used a STALE frame: {}",
            events[1].detail
        );
    }

    #[test]
    fn click_point_jitter_cannot_leave_the_frame() {
        // A target on the far corner: HB2 jitter would push the point off-screen
        // for some seeds; the interpreter must clamp the ACTUAL point.
        let mut saw_would_leave = false;
        for seed in 1u64..=16 {
            let (mut interp, input) = mk_inspect(FakeScreen::new());
            interp = interp.with_seed(seed);
            let f = flow(
                r#"{"schema_version":1,"name":"t","steps":[
                     {"op":"click_point","args":{"x":63,"y":47}},
                     {"op":"finish","args":{"exit":"done"}}]}"#,
            );
            assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done), "seed {seed}");
            let clicks = input.clicks.borrow();
            let p = clicks[0];
            assert!(
                (0..64).contains(&p.x) && (0..48).contains(&p.y),
                "seed {seed}: click {p:?} left the 64x48 frame"
            );
            // reproduce the raw jitter: this seed genuinely wanted to go off-screen
            let mut rng = crate::humanize::Rng::new(seed);
            let raw = crate::humanize::Humanize::default().jitter_point(Point { x: 63, y: 47 }, &mut rng);
            if raw.x >= 64 || raw.y >= 48 || raw.x < 0 || raw.y < 0 {
                saw_would_leave = true;
            }
        }
        assert!(saw_would_leave, "test needs at least one seed whose raw jitter leaves the frame");
    }

    // ---- HB7 misses (1~3% per click, [10-22 19:0x] spec) ----

    #[test]
    fn click_miss_is_outside_the_template_box_and_logged() {
        let (input, shared) = FakeInput::new();
        let mut interp = Interpreter::new(FakeScreen::new(), input, assets(), Budget::default())
            .with_humanize(crate::humanize::Humanize {
                miss_chance_min: 1.0,
                miss_chance_max: 1.0,
                ..crate::humanize::Humanize::default()
            });
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click","args":{"sel":"I_BRIGHT"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let ev = &interp.events()[0];
        assert!(ev.detail.contains("miss=true"), "{}", ev.detail);
        let p = detail_point(&ev.detail);
        // The matched template sits at (12,10) with size 10x10.
        let inside = (12..22).contains(&p.x) && (10..20).contains(&p.y);
        assert!(!inside, "HB7 miss must land outside the template rect: {p:?}");
        let rows = interp.clicks();
        assert!(rows[0].miss, "audit row must flag the miss: {rows:?}");
        assert_eq!(shared.clicks.borrow().as_slice(), &[p], "HB6: the log carries the actual point");
    }

    #[test]
    fn click_point_miss_is_logged_and_audited() {
        let (input, _shared) = FakeInput::new();
        let mut interp = Interpreter::new(FakeScreen::new(), input, assets(), Budget::default())
            .with_humanize(crate::humanize::Humanize {
                miss_chance_min: 1.0,
                miss_chance_max: 1.0,
                ..crate::humanize::Humanize::default()
            });
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click_point","args":{"x":32,"y":24}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let ev = &interp.events()[0];
        assert!(ev.detail.contains("miss=true"), "{}", ev.detail);
        let p = detail_point(&ev.detail);
        let far = (p.x - 32).abs() + (p.y - 24).abs() > 8;
        assert!(far, "HB7 miss should be visibly off target: {p:?}");
        assert!(interp.clicks()[0].miss);
    }

    // ---- §22.10 polygon click area (largest inscribed rectangle) ----

    #[test]
    fn polygon_laar_finds_a_rectangle_inside_a_rectangle_polygon() {
        let poly = vec![
            Point { x: 100, y: 100 },
            Point { x: 142, y: 100 },
            Point { x: 142, y: 132 },
            Point { x: 100, y: 132 },
        ];
        let r = largest_inscribed_rect(&poly).expect("laar");
        assert!(r.w >= 39 && r.h >= 29, "{r:?}");
        assert!(r.x >= 100 && r.y >= 100 && r.x + r.w <= 142 && r.y + r.h <= 132, "{r:?}");
    }

    #[test]
    fn polygon_laar_finds_the_largest_arm_of_an_l_shape() {
        let poly = vec![
            Point { x: 0, y: 0 },
            Point { x: 60, y: 0 },
            Point { x: 60, y: 20 },
            Point { x: 20, y: 20 },
            Point { x: 20, y: 60 },
            Point { x: 0, y: 60 },
        ];
        let r = largest_inscribed_rect(&poly).expect("laar");
        assert!(r.w * r.h >= 1100, "{r:?} area {}", r.w * r.h);
        let centre = Point {
            x: r.x + r.w / 2,
            y: r.y + r.h / 2,
        };
        assert!(point_in_polygon(centre, &poly));
    }

    #[test]
    fn polygon_laar_rejects_degenerate_polygons() {
        assert!(largest_inscribed_rect(&[]).is_none());
        assert!(largest_inscribed_rect(&[Point { x: 0, y: 0 }, Point { x: 1, y: 1 }]).is_none());
    }

    #[test]
    fn click_area_is_the_matched_template_bbox_even_with_a_polygon() {
        // Explicit user decision [10-22 23:0x]: polygon only affects MATCHING;
        // the click area is the matched template's bounding rectangle.
        let pattern: Vec<u8> = (0..100)
            .map(|i| (((i % 10) * 7 + (i / 10) * 13) % 251 + 3) as u8)
            .collect();
        let mut assets = assets();
        assets.insert(
            "I_POLY",
            Template {
                gray: GrayImage::new(10, 10, pattern),
                roi: Some(Rect {
                    x: 12,
                    y: 10,
                    w: 10,
                    h: 10,
                }),
                threshold: 0.8,
                click_polygon: Some(vec![
                    Point { x: 12, y: 10 },
                    Point { x: 22, y: 10 },
                    Point { x: 17, y: 20 },
                ]),
            },
        );
        let uniform_only = crate::humanize::Humanize {
            bell_weight: 0.0,
            uniform_weight: 1.0,
            comet_weight: 0.0,
            miss_chance_min: 0.0,
            miss_chance_max: 0.0,
            ..crate::humanize::Humanize::default()
        };
        for seed in 1..=50u64 {
            let (input, shared) = FakeInput::new();
            let mut interp = Interpreter::new(
                FakeScreen::new(),
                input,
                assets.clone(),
                Budget::default(),
            )
            .with_humanize(uniform_only)
            .with_seed(seed);
            let f = flow(
                r#"{"schema_version":1,"name":"t","steps":[
                     {"op":"click","args":{"sel":"I_POLY"}},
                     {"op":"finish","args":{"exit":"done"}}]}"#,
            );
            assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done), "seed {seed}");
            let clicks = shared.clicks.borrow();
            let p = clicks[0];
            assert!(
                p.x >= 12 && p.x < 22 && p.y >= 10 && p.y < 20,
                "seed {seed}: click must stay inside the matched template bbox, got {p:?}"
            );
        }
    }

    #[test]
    fn click_finds_a_template_that_shifted_outside_its_roi() {
        // §22.20: the authored ROI has nothing, but the same template sits at
        // (40,30) elsewhere => whole-frame fallback finds it and clicks there.
        let pattern: Vec<u8> = (0..100)
            .map(|i| (((i % 10) * 7 + (i / 10) * 13) % 251 + 3) as u8)
            .collect();
        let mut assets = assets();
        assets.insert(
            "I_SHIFTED",
            Template {
                gray: GrayImage::new(10, 10, pattern),
                roi: Some(Rect {
                    x: 12,
                    y: 10,
                    w: 10,
                    h: 10,
                }),
                threshold: 0.8,
                click_polygon: None,
            },
        );
        let screen = TwoFrameScreen {
            frames: vec![frame_with_tile(40, 30)],
            idx: 0,
        };
        let (input, shared) = FakeInput::new();
        let mut interp = Interpreter::new(screen, input, assets, Budget::default())
            .with_humanize(no_miss())
            .with_seed(5);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click","args":{"sel":"I_SHIFTED"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let clicks = shared.clicks.borrow();
        let p = clicks[0];
        assert!(
            p.x >= 40 && p.x < 50 && p.y >= 30 && p.y < 40,
            "click should follow the shifted template to (40,30), got {p:?}"
        );
    }

    #[test]
    fn roi_back_expands_the_click_area_and_translates_with_the_match() {
        // §22.25: recognition box = 10x10 at (12,10); click box = 20x20 at (10,8)
        // (a bigger "safe" area, like the old OAS roiBack). Clicks must sample the
        // click box, not the matched template bbox.
        let pattern: Vec<u8> = (0..100)
            .map(|i| (((i % 10) * 7 + (i / 10) * 13) % 251 + 3) as u8)
            .collect();
        let mut assets = assets();
        assets.insert(
            "I_BACK",
            Template {
                gray: GrayImage::new(10, 10, pattern.clone()),
                roi: Some(Rect {
                    x: 12,
                    y: 10,
                    w: 10,
                    h: 10,
                }),
                threshold: 0.8,
                click_polygon: None,
            },
        );
        assets.set_click_back(
            "I_BACK",
            Rect {
                x: 10,
                y: 8,
                w: 20,
                h: 20,
            },
        );
        let uniform_only = crate::humanize::Humanize {
            bell_weight: 0.0,
            uniform_weight: 1.0,
            comet_weight: 0.0,
            miss_chance_min: 0.0,
            miss_chance_max: 0.0,
            ..crate::humanize::Humanize::default()
        };
        let mut outside_front = 0;
        for seed in 1..=50u64 {
            let (input, shared) = FakeInput::new();
            let mut interp = Interpreter::new(
                FakeScreen::new(),
                input,
                assets.clone(),
                Budget::default(),
            )
            .with_humanize(uniform_only)
            .with_seed(seed);
            let f = flow(
                r#"{"schema_version":1,"name":"t","steps":[
                     {"op":"click","args":{"sel":"I_BACK"}},
                     {"op":"finish","args":{"exit":"done"}}]}"#,
            );
            assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done), "seed {seed}");
            let p = shared.clicks.borrow()[0];
            assert!(
                p.x >= 10 && p.x < 30 && p.y >= 8 && p.y < 28,
                "seed {seed}: click must stay inside roi_back (10,8,20,20), got {p:?}"
            );
            if !(p.x >= 12 && p.x < 22 && p.y >= 10 && p.y < 20) {
                outside_front += 1;
            }
        }
        assert!(
            outside_front > 0,
            "click box must actually be used (all 50 clicks landed in the template bbox)"
        );
    }

    #[test]
    fn roi_back_translates_when_the_template_moved() {
        // Template authored at (12,10) but the frame has it at (40,30): the click
        // box (10,8,20,20) must move by the same (+28,+20) offset => (38,28,20,20).
        let pattern: Vec<u8> = (0..100)
            .map(|i| (((i % 10) * 7 + (i / 10) * 13) % 251 + 3) as u8)
            .collect();
        let mut assets = assets();
        assets.insert(
            "I_BACK_SHIFT",
            Template {
                gray: GrayImage::new(10, 10, pattern),
                roi: Some(Rect {
                    x: 12,
                    y: 10,
                    w: 10,
                    h: 10,
                }),
                threshold: 0.8,
                click_polygon: None,
            },
        );
        assets.set_click_back(
            "I_BACK_SHIFT",
            Rect {
                x: 10,
                y: 8,
                w: 20,
                h: 20,
            },
        );
        let screen = TwoFrameScreen {
            frames: vec![frame_with_tile(40, 30)],
            idx: 0,
        };
        let (input, shared) = FakeInput::new();
        let mut interp = Interpreter::new(screen, input, assets, Budget::default())
            .with_humanize(no_miss())
            .with_seed(7);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click","args":{"sel":"I_BACK_SHIFT"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let p = shared.clicks.borrow()[0];
        assert!(
            p.x >= 38 && p.x < 58 && p.y >= 28 && p.y < 48,
            "click must follow the translated roi_back (38,28,20,20), got {p:?}"
        );
    }

    #[test]
    fn stable_frames_requires_the_same_position_on_two_frames() {
        // §22.29 H4: two consecutive frames with the target at the same place
        // => one click. A moving target => loud failure and NO click.
        let pattern: Vec<u8> = (0..100)
            .map(|i| (((i % 10) * 7 + (i / 10) * 13) % 251 + 3) as u8)
            .collect();
        let mut assets = assets();
        assets.insert(
            "I_VOTE",
            Template {
                gray: GrayImage::new(10, 10, pattern),
                roi: Some(Rect {
                    x: 12,
                    y: 10,
                    w: 10,
                    h: 10,
                }),
                threshold: 0.8,
                click_polygon: None,
            },
        );
        let stable = frame_with_tile(12, 10);
        let screen = TwoFrameScreen {
            frames: vec![stable.clone(), stable],
            idx: 0,
        };
        let (input, shared) = FakeInput::new();
        let mut interp = Interpreter::new(screen, input, assets.clone(), Budget::default())
            .with_humanize(no_miss())
            .with_seed(41);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click","args":{"sel":"I_VOTE","stable_frames":2}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        assert_eq!(shared.clicks.borrow().len(), 1);

        // moving target between the two frames
        let moving = TwoFrameScreen {
            frames: vec![frame_with_tile(12, 10), frame_with_tile(40, 30)],
            idx: 0,
        };
        let (input, shared) = FakeInput::new();
        let mut interp = Interpreter::new(moving, input, assets, Budget::default())
            .with_humanize(no_miss())
            .with_seed(42);
        let res = interp.run(&f);
        match res {
            RunResult::Failed(e) => assert!(e.contains("多帧投票失败"), "{e}"),
            other => panic!("expected Failed, got {other:?}"),
        }
        assert!(
            shared.clicks.borrow().is_empty(),
            "voting failure must never click"
        );
    }

    // ---- §22.31 H5: OCR text + template cross-check ----

    struct FixedLineOcr {
        rect: Rect,
        text: String,
    }

    impl OcrProvider for FixedLineOcr {
        fn recognize(&mut self, _frame: &Frame, _roi: Rect) -> Result<(String, f32), String> {
            Ok((self.text.clone(), 0.99))
        }

        fn recognize_lines(&mut self, _frame: &Frame, _roi: Rect) -> Result<Vec<OcrLine>, String> {
            Ok(vec![line(&self.text, 0.99, self.rect)])
        }
    }

    fn run_cross_check(frame: Frame, require_template: &str) -> (RunResult, Vec<Point>) {
        let mut assets = assets_with_ocr();
        assets.insert_ocr_lines("O_CONFIRM", Rect { x: 0, y: 0, w: 64, h: 48 });
        let screen = TwoFrameScreen {
            frames: vec![frame],
            idx: 0,
        };
        let (input, shared) = FakeInput::new();
        let mut interp = Interpreter::new(screen, input, assets, Budget::default())
            .with_humanize(no_miss())
            .with_ocr(Box::new(FixedLineOcr {
                rect: Rect { x: 0, y: 0, w: 30, h: 20 },
                text: "确认".to_string(),
            }))
            .with_seed(51);
        let f = flow(
            &r#"{"schema_version":1,"name":"t","steps":[{"op":"click_target","args":{"find":[{"kind":"ocr_text","sel":"O_CONFIRM","text":"确认","require_template":"__RT__"}]}},{"op":"finish","args":{"exit":"done"}}]}"#
                .replace("__RT__", require_template),
        );
        let res = interp.run(&f);
        let clicks = shared.clicks.borrow().clone();
        (res, clicks)
    }

    #[test]
    fn ocr_text_requires_the_cross_check_template() {
        // Positive: the frame contains I_BRIGHT near the OCR line => click.
        let (res, clicks) = run_cross_check(frame_with_tile(12, 10), "I_BRIGHT");
        assert_eq!(res, RunResult::Finished(Exit::Done));
        assert_eq!(clicks.len(), 1);
        let p = clicks[0];
        assert!(
            p.x >= 0 && p.x < 30 && p.y >= 0 && p.y < 20,
            "click should land in the OCR line rect, got {p:?}"
        );

        // Negative: same text, but the required icon template is absent => no click.
        let mut blank = Frame::new(64, 48, vec![10u8; 64 * 48 * 4]);
        for i in (3..blank.rgba.len()).step_by(4) {
            blank.rgba[i] = 255;
        }
        let (res, clicks) = run_cross_check(blank, "I_NEVER");
        match res {
            RunResult::Failed(e) => {
                assert!(e.contains("require") || e.contains("required template"), "{e}")
            }
            other => panic!("expected Failed, got {other:?}"),
        }
        assert!(clicks.is_empty(), "cross-check failure must never click");
    }

    // ---- §22.11 click_target locator chain ----

    struct FakeStore {
        items: std::collections::HashMap<String, Point>,
        puts: usize,
        writable: bool,
    }

    impl FakeStore {
        fn new(items: &[(&str, (i32, i32))], writable: bool) -> Self {
            Self {
                items: items
                    .iter()
                    .map(|(k, (x, y))| {
                        (k.to_string(), Point { x: *x, y: *y })
                    })
                    .collect(),
                puts: 0,
                writable,
            }
        }
    }

    impl CoordStore for FakeStore {
        fn get(&mut self, key: &str, _w: u32, _h: u32) -> Option<Point> {
            self.items.get(key).copied()
        }

        fn put(&mut self, key: &str, p: Point, _w: u32, _h: u32) {
            if self.writable {
                self.items.insert(key.to_string(), p);
                self.puts += 1;
            }
        }
    }

    /// Interpreter owns `Box<dyn CoordStore>`, so the test keeps an Rc to inspect.
    struct SharedFakeStore(std::rc::Rc<std::cell::RefCell<FakeStore>>);

    impl CoordStore for SharedFakeStore {
        fn get(&mut self, key: &str, w: u32, h: u32) -> Option<Point> {
            self.0.borrow_mut().get(key, w, h)
        }
        fn put(&mut self, key: &str, p: Point, w: u32, h: u32) {
            self.0.borrow_mut().put(key, p, w, h)
        }
    }

    fn chain_assets() -> AssetSet {
        let mut a = assets();
        a.insert_ocr_lines(
            "O_LIST",
            Rect {
                x: 40,
                y: 24,
                w: 40,
                h: 24,
            },
        );
        a
    }

    fn click_target_flow(find: &str) -> Flow {
        flow(&format!(
            r#"{{"schema_version":1,"name":"t","steps":[
                 {{"op":"click_target","args":{{"find":{find}}}}},
                 {{"op":"finish","args":{{"exit":"done"}}}}]}}"#
        ))
    }

    #[test]
    fn click_target_prefers_template_even_when_declared_last_and_updates_memory() {
        let (ocr, _rois) = FakeDetOcr::new(vec![line(
            "返回",
            0.99,
            Rect {
                x: 40,
                y: 24,
                w: 20,
                h: 12,
            },
        )]);
        let store = std::rc::Rc::new(std::cell::RefCell::new(FakeStore::new(
            &[("返回按钮", (50, 40))],
            true,
        )));
        let (input, _shared) = FakeInput::new();
        let mut interp = Interpreter::new(FakeScreen::new(), input, chain_assets(), Budget::default())
            .with_humanize(no_miss())
            .with_ocr(Box::new(ocr))
            .with_coord_store(Box::new(SharedFakeStore(store.clone())))
            .with_seed(5);
        let f = click_target_flow(
            r#"[{"kind":"coord_memory","key":"返回按钮"},
                {"kind":"ocr_text","sel":"O_LIST","text":"返回"},
                {"kind":"template","sel":"I_BRIGHT"}]"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let rows = interp.clicks();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].locator, "template");
        assert_eq!(rows[0].sel.as_deref(), Some("I_BRIGHT"));
        // Any locator success refreshes the remembered point for the chain.
        assert_eq!(store.borrow().puts, 1);
    }

    #[test]
    fn click_target_falls_back_to_ocr_text() {
        let (ocr, rois) = FakeDetOcr::new(vec![line(
            "返回",
            0.9,
            Rect {
                x: 40,
                y: 24,
                w: 20,
                h: 12,
            },
        )]);
        let (input, shared) = FakeInput::new();
        let mut interp = Interpreter::new(FakeScreen::new(), input, chain_assets(), Budget::default())
            .with_humanize(no_miss())
            .with_ocr(Box::new(ocr))
            .with_seed(6);
        let f = click_target_flow(
            r#"[{"kind":"template","sel":"I_NEVER"},
                {"kind":"ocr_text","sel":"O_LIST","text":"返回"}]"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let rows = interp.clicks();
        assert_eq!(rows[0].locator, "ocr_text");
        assert!(
            rows[0].match_info.starts_with("ocr_text:O_LIST:返回:"),
            "match_info must carry the OCR evidence: {}",
            rows[0].match_info
        );
        let p = shared.clicks.borrow()[0];
        assert!(
            p.x >= 40 && p.x < 60 && p.y >= 24 && p.y < 36,
            "OCR fallback must click inside the matched line box: {p:?}"
        );
        assert_eq!(rois.borrow().len(), 1, "OCR must have been asked once");
    }

    #[test]
    fn click_target_inline_ocr_roi_clicks_matching_line_without_asset() {
        // v1.1: whole-screen OCR lines can be clicked with an inline roi+text;
        // no OCR asset is declared (chain_assets only has O_LIST/I_BRIGHT).
        let (ocr, rois) = FakeDetOcr::new(vec![line(
            "个人突破",
            0.97,
            Rect {
                x: 40,
                y: 24,
                w: 36,
                h: 14,
            },
        )]);
        let (input, shared) = FakeInput::new();
        let mut interp = Interpreter::new(FakeScreen::new(), input, chain_assets(), Budget::default())
            .with_humanize(no_miss())
            .with_ocr(Box::new(ocr))
            .with_seed(7);
        let f = click_target_flow(
            r#"[{"kind":"ocr_text","roi":[5,1,60,40],"text":"个人突破"}]"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let rows = interp.clicks();
        assert_eq!(rows[0].locator, "ocr_text");
        assert!(rows[0].sel.is_none(), "inline locator has no asset");
        assert!(
            rows[0].match_info.starts_with("ocr_text:inline:个人突破:"),
            "match_info must say inline: {}",
            rows[0].match_info
        );
        let p = shared.clicks.borrow()[0];
        assert!(
            p.x >= 40 && p.x < 76 && p.y >= 24 && p.y < 38,
            "inline OCR click must land in the matched line, got {p:?}"
        );
        assert_eq!(rois.borrow().len(), 1, "inline OCR must ask the provider once");
    }

    #[test]
    fn click_target_falls_back_to_coord_memory_and_respects_read_only_store() {
        let (ocr, _rois) = FakeDetOcr::new(vec![line(
            "分组2",
            0.9,
            Rect {
                x: 40,
                y: 24,
                w: 20,
                h: 12,
            },
        )]);
        let store = std::rc::Rc::new(std::cell::RefCell::new(FakeStore::new(
            &[("预设", (50, 40))],
            false, // replay-like read-only store: put() is a no-op
        )));
        let (input, shared) = FakeInput::new();
        let mut interp = Interpreter::new(FakeScreen::new(), input, chain_assets(), Budget::default())
            .with_humanize(no_miss())
            .with_ocr(Box::new(ocr))
            .with_coord_store(Box::new(SharedFakeStore(store.clone())))
            .with_seed(7);
        let f = click_target_flow(
            r#"[{"kind":"template","sel":"I_NEVER"},
                {"kind":"ocr_text","sel":"O_LIST","text":"不存在的文字"},
                {"kind":"coord_memory","key":"预设"}]"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let rows = interp.clicks();
        assert_eq!(rows[0].locator, "coord_memory");
        assert!(
            rows[0].match_info.starts_with("coord_memory:预设"),
            "match_info must carry the memory key: {}",
            rows[0].match_info
        );
        let p = shared.clicks.borrow()[0];
        assert!(
            (p.x - 50).abs() <= 7 && (p.y - 40).abs() <= 7,
            "memory fallback should jitter around the remembered point: {p:?}"
        );
        assert_eq!(store.borrow().puts, 0, "read-only store must not be written");
    }

    #[test]
    fn click_target_all_locators_failing_is_a_loud_error() {
        let (ocr, _rois) = FakeDetOcr::new(vec![line(
            "分组2",
            0.9,
            Rect {
                x: 40,
                y: 24,
                w: 20,
                h: 12,
            },
        )]);
        let store = std::rc::Rc::new(std::cell::RefCell::new(FakeStore::new(&[], false)));
        let (input, _shared) = FakeInput::new();
        let mut interp = Interpreter::new(FakeScreen::new(), input, chain_assets(), Budget::default())
            .with_humanize(no_miss())
            .with_ocr(Box::new(ocr))
            .with_coord_store(Box::new(SharedFakeStore(store)))
            .with_seed(8);
        let f = click_target_flow(
            r#"[{"kind":"template","sel":"I_NEVER"},
                {"kind":"ocr_text","sel":"O_LIST","text":"没有这个"},
                {"kind":"coord_memory","key":"空记忆"}]"#,
        );
        match interp.run(&f) {
            RunResult::Failed(msg) => {
                assert!(msg.contains("all locators failed"), "{msg}");
                assert!(msg.contains("no remembered point"), "evidence missing: {msg}");
                assert!(msg.contains("template `I_NEVER`"), "evidence missing: {msg}");
            }
            other => panic!("expected a loud failure, got {other:?}"),
        }
    }

    // ---- §22.11 补充层: post-click verification (`expect`) ----

    fn frame_flat() -> Frame {
        Frame::new(64, 48, vec![30u8; (64 * 48 * 4) as usize])
    }

    #[test]
    fn click_point_expect_appear_polls_until_the_template_appears() {
        let screen = TwoFrameScreen {
            frames: vec![frame_flat(), frame_with_tile(12, 10)],
            idx: 0,
        };
        let (input, shared) = FakeInput::new();
        let mut interp = Interpreter::new(screen, input, assets(), Budget::default())
            .with_humanize(no_miss())
            .with_seed(3);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click_point","args":{"x":20,"y":20,
                   "expect":{"kind":"appear","sel":"I_BRIGHT","timeout_ms":800}}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        assert_eq!(shared.clicks.borrow().len(), 1);
        let detail = interp
            .events()
            .iter()
            .find(|e| e.op == "click_point")
            .map(|e| e.detail.clone())
            .expect("step event");
        assert!(detail.contains("verified=true"), "{detail}");
    }

    #[test]
    fn click_point_expect_appear_times_out_loudly() {
        let (input, _shared) = FakeInput::new();
        let mut interp = Interpreter::new(FakeScreen::new(), input, assets(), Budget::default())
            .with_humanize(no_miss())
            .with_seed(4);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click_point","args":{"x":20,"y":20,
                   "expect":{"kind":"appear","sel":"I_NEVER","timeout_ms":120}}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        match interp.run(&f) {
            RunResult::Failed(msg) => {
                assert!(msg.contains("expect appear not met after 120ms"), "{msg}");
            }
            other => panic!("expected a loud timeout failure, got {other:?}"),
        }
    }

    #[test]
    fn click_point_expect_vanish_passes_after_the_popup_is_gone() {
        let screen = TwoFrameScreen {
            frames: vec![frame_with_tile(12, 10), frame_flat()],
            idx: 0,
        };
        let (input, _shared) = FakeInput::new();
        let mut interp = Interpreter::new(screen, input, assets(), Budget::default())
            .with_humanize(no_miss())
            .with_seed(6);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click_point","args":{"x":20,"y":20,
                   "expect":{"kind":"vanish","sel":"I_BRIGHT","timeout_ms":800}}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let detail = interp
            .events()
            .iter()
            .find(|e| e.op == "click_point")
            .map(|e| e.detail.clone())
            .expect("step event");
        assert!(detail.contains("verified=true"), "{detail}");
    }

    #[test]
    fn click_point_expect_ocr_text_uses_the_ocr_provider() {
        let mut a = assets();
        a.insert_ocr(
            "O_MSG",
            Rect {
                x: 0,
                y: 0,
                w: 64,
                h: 48,
            },
        );
        let (ocr, _rois) = FakeOcr::new(&[("完成", 0.9)]);
        let (input, _shared) = FakeInput::new();
        let mut interp = Interpreter::new(FakeScreen::new(), input, a, Budget::default())
            .with_humanize(no_miss())
            .with_ocr(Box::new(ocr))
            .with_seed(7);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click_point","args":{"x":20,"y":20,
                   "expect":{"kind":"ocr_text","sel":"O_MSG","text":"完成","timeout_ms":500}}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let detail = interp
            .events()
            .iter()
            .find(|e| e.op == "click_point")
            .map(|e| e.detail.clone())
            .expect("step event");
        assert!(detail.contains("verified=true"), "{detail}");
    }

    // ---- §22.11 补充层: pre-click page guard (`requires`) ----

    #[test]
    fn click_requires_guard_fails_before_any_click() {
        let (input, shared) = FakeInput::new();
        let mut interp = Interpreter::new(FakeScreen::new(), input, assets(), Budget::default())
            .with_humanize(no_miss())
            .with_seed(9);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click_point","args":{"x":20,"y":20,
                   "requires":{"kind":"appear","sel":"I_NEVER","timeout_ms":120}}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        match interp.run(&f) {
            RunResult::Failed(msg) => {
                assert!(msg.contains("page guard failed"), "{msg}");
                assert!(msg.contains("expect appear not met"), "{msg}");
            }
            other => panic!("expected a loud guard failure, got {other:?}"),
        }
        assert!(
            shared.clicks.borrow().is_empty(),
            "an unmet page guard must NOT click"
        );
    }

    #[test]
    fn click_requires_guard_polls_then_clicks_on_the_right_page() {
        let screen = TwoFrameScreen {
            frames: vec![frame_flat(), frame_with_tile(12, 10)],
            idx: 0,
        };
        let (input, shared) = FakeInput::new();
        let mut interp = Interpreter::new(screen, input, assets(), Budget::default())
            .with_humanize(no_miss())
            .with_seed(10);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click_point","args":{"x":20,"y":20,
                   "requires":{"kind":"appear","sel":"I_BRIGHT","timeout_ms":500}}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        assert_eq!(shared.clicks.borrow().len(), 1, "guard met => click happens");
        let detail = interp
            .events()
            .iter()
            .find(|e| e.op == "click_point")
            .map(|e| e.detail.clone())
            .expect("step event");
        assert!(detail.contains("guard=true"), "{detail}");
    }

    // ---- §22.11 补充层: multi-template variants (`alts`) ----

    #[test]
    fn click_alts_falls_back_to_the_next_template_variant() {
        let (input, shared) = FakeInput::new();
        let mut interp = Interpreter::new(FakeScreen::new(), input, assets(), Budget::default())
            .with_humanize(no_miss())
            .with_seed(11);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click","args":{"sel":"I_NEVER","alts":["I_BRIGHT"]}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let rows = interp.clicks();
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].sel.as_deref(),
            Some("I_BRIGHT"),
            "the audited id must be the VARIANT that matched"
        );
        assert!(
            rows[0].match_info.starts_with("template:I_BRIGHT:"),
            "match_info must carry the matched variant + score: {}",
            rows[0].match_info
        );
        let p = shared.clicks.borrow()[0];
        assert!(
            p.x >= 12 && p.x < 22 && p.y >= 10 && p.y < 20,
            "variant click must land in the matched tile: {p:?}"
        );
    }

    #[test]
    fn click_alts_all_failing_reports_every_score() {
        let (input, _shared) = FakeInput::new();
        let mut interp = Interpreter::new(FakeScreen::new(), input, assets(), Budget::default())
            .with_humanize(no_miss())
            .with_seed(12);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click","args":{"sel":"I_NEVER","alts":["I_NEVER"]}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        match interp.run(&f) {
            RunResult::Failed(msg) => {
                assert!(msg.contains("not found"), "{msg}");
                assert!(msg.contains("I_NEVER:"), "per-variant evidence missing: {msg}");
            }
            other => panic!("expected a loud failure, got {other:?}"),
        }
    }

    // ---- §22.15 popup / interrupt handling ----

    #[test]
    fn on_interrupt_dismisses_a_popup_before_the_step_runs() {
        let screen = TwoFrameScreen {
            frames: vec![frame_with_tile(12, 10), frame_flat()],
            idx: 0,
        };
        let (input, shared) = FakeInput::new();
        let mut interp = Interpreter::new(screen, input, assets(), Budget::default())
            .with_humanize(no_miss())
            .with_seed(13);
        let f = flow(
            r#"{"schema_version":1,"name":"t","interrupts":[{"sel":"I_BRIGHT"}],
                 "steps":[{"op":"click_point","args":{"x":20,"y":20}},
                          {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let rows = interp.clicks();
        assert_eq!(rows.len(), 2, "one dismiss + one real click");
        assert_eq!(rows[0].locator, "interrupt");
        assert!(rows[0].match_info.starts_with("interrupt:I_BRIGHT:"));
        assert_eq!(rows[1].locator, "point");
        assert!(
            interp.events().iter().any(|e| e.op == "interrupt"),
            "the dismissal must be visible in the step log"
        );
    }

    #[test]
    fn on_interrupt_that_never_disappears_fails_loudly() {
        // FakeScreen always shows the tile => the rule matches forever.
        let (input, shared) = FakeInput::new();
        let mut interp = Interpreter::new(FakeScreen::new(), input, assets(), Budget::default())
            .with_humanize(no_miss())
            .with_seed(14);
        let f = flow(
            r#"{"schema_version":1,"name":"t","interrupts":[{"sel":"I_BRIGHT"}],
                 "steps":[{"op":"click_point","args":{"x":20,"y":20}},
                          {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        match interp.run(&f) {
            RunResult::Failed(msg) => {
                assert!(msg.contains("did not converge"), "{msg}");
            }
            other => panic!("expected a bounded-popup failure, got {other:?}"),
        }
        assert_eq!(shared.clicks.borrow().len(), MAX_INTERRUPTS_PER_STEP);
    }

    #[test]
    fn dismiss_popup_step_clicks_when_present() {
        let (input, shared) = FakeInput::new();
        let mut interp = Interpreter::new(FakeScreen::new(), input, assets(), Budget::default())
            .with_humanize(no_miss())
            .with_seed(15);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"dismiss_popup","args":{"sel":"I_BRIGHT","timeout_ms":0,"required":true}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        assert_eq!(shared.clicks.borrow().len(), 1);
        let rows = interp.clicks();
        assert_eq!(rows[0].locator, "dismiss_popup");
        assert!(
            rows[0].match_info.starts_with("dismiss_popup:I_BRIGHT->I_BRIGHT:"),
            "match_info: {}",
            rows[0].match_info
        );
    }

    #[test]
    fn dismiss_popup_absent_is_ok_when_not_required() {
        let (input, shared) = FakeInput::new();
        let mut interp = Interpreter::new(FakeScreen::new(), input, assets(), Budget::default())
            .with_humanize(no_miss())
            .with_seed(16);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"dismiss_popup","args":{"sel":"I_NEVER","timeout_ms":0}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        assert!(shared.clicks.borrow().is_empty(), "absent popup => no click");
    }

    #[test]
    fn dismiss_popup_absent_required_fails_loudly() {
        let (input, _shared) = FakeInput::new();
        let mut interp = Interpreter::new(FakeScreen::new(), input, assets(), Budget::default())
            .with_humanize(no_miss())
            .with_seed(17);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"dismiss_popup","args":{"sel":"I_NEVER","timeout_ms":100,"required":true}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        match interp.run(&f) {
            RunResult::Failed(msg) => {
                assert!(msg.contains("dismiss_popup: `I_NEVER` not found"), "{msg}");
            }
            other => panic!("expected a loud failure, got {other:?}"),
        }
    }

    // ---- §22.15 popup / interrupt handling: ACCEPT vs DISMISS ----

    /// Popup frame with TWO distinct tiles: a placeholder "popup" at (12,10) and
    /// the "accept" button at (40,30) (10x10 each).
    fn frame_two_tiles() -> Frame {
        let (w, h) = (64u32, 48u32);
        let mut rgba = vec![30u8; (w * h * 4) as usize];
        let paint = |rgba: &mut [u8], ox: u32, oy: u32, mul: u32| {
            for y in 0..10u32 {
                for x in 0..10u32 {
                    let v = (((x * mul + y * (mul + 3)) % 251) + 3) as u8;
                    let i = (((oy + y) * w + (ox + x)) * 4) as usize;
                    rgba[i] = v;
                    rgba[i + 1] = v;
                    rgba[i + 2] = v;
                    rgba[i + 3] = 255;
                }
            }
        };
        paint(&mut rgba, 12, 10, 7);
        paint(&mut rgba, 40, 30, 11);
        Frame::new(w, h, rgba)
    }

    fn tile_pattern(mul: u32) -> Vec<u8> {
        (0..100u32)
            .map(|i| {
                let (x, y) = (i % 10, i / 10);
                (((x * mul + y * (mul + 3)) % 251) + 3) as u8
            })
            .collect()
    }

    fn two_tile_assets() -> AssetSet {
        let mut set = AssetSet::new();
        set.insert(
            "POPUP",
            Template {
                gray: GrayImage::new(10, 10, tile_pattern(7)),
                roi: Some(Rect {
                    x: 12,
                    y: 10,
                    w: 10,
                    h: 10,
                }),
                threshold: 0.8,
                click_polygon: None,
            },
        );
        set.insert(
            "ACCEPT",
            Template {
                gray: GrayImage::new(10, 10, tile_pattern(11)),
                roi: Some(Rect {
                    x: 40,
                    y: 30,
                    w: 10,
                    h: 10,
                }),
                threshold: 0.8,
                click_polygon: None,
            },
        );
        set
    }

    #[test]
    fn on_interrupt_accepts_an_invitation_popup_via_action_template() {
        let screen = TwoFrameScreen {
            frames: vec![frame_two_tiles(), frame_flat()],
            idx: 0,
        };
        let (input, _shared) = FakeInput::new();
        let mut interp =
            Interpreter::new(screen, input, two_tile_assets(), Budget::default())
                .with_humanize(no_miss())
                .with_seed(18);
        let f = flow(
            r#"{"schema_version":1,"name":"t",
                 "interrupts":[{"detect":"POPUP","action":"ACCEPT"}],
                 "steps":[{"op":"click_point","args":{"x":20,"y":20}},
                          {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let rows = interp.clicks();
        assert_eq!(rows.len(), 2, "one accept + one real click");
        assert_eq!(rows[0].locator, "interrupt");
        assert!(
            rows[0].x >= 40 && rows[0].x < 50 && rows[0].y >= 30 && rows[0].y < 40,
            "must click the ACTION (accept) button, got ({},{})",
            rows[0].x,
            rows[0].y
        );
        assert!(
            rows[0].match_info.contains("POPUP->ACCEPT"),
            "match_info: {}",
            rows[0].match_info
        );
    }

    #[test]
    fn interrupt_detected_without_its_action_is_a_loud_error() {
        let (input, _shared) = FakeInput::new();
        let mut interp = Interpreter::new(FakeScreen::new(), input, assets(), Budget::default())
            .with_humanize(no_miss())
            .with_seed(19);
        let f = flow(
            r#"{"schema_version":1,"name":"t",
                 "interrupts":[{"detect":"I_BRIGHT","action":"I_NEVER"}],
                 "steps":[{"op":"click_point","args":{"x":20,"y":20}},
                          {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        match interp.run(&f) {
            RunResult::Failed(msg) => {
                assert!(msg.contains("action `I_NEVER` not found"), "{msg}");
            }
            other => panic!("expected a loud failure, got {other:?}"),
        }
    }

    // ---- §22.16 step-level retry with backoff ----

    #[test]
    fn click_retry_succeeds_on_the_next_frame() {
        let screen = TwoFrameScreen {
            frames: vec![frame_flat(), frame_with_tile(12, 10)],
            idx: 0,
        };
        let (input, shared) = FakeInput::new();
        let mut interp = Interpreter::new(screen, input, assets(), Budget::default())
            .with_humanize(no_miss())
            .with_seed(20);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click","args":{"sel":"I_BRIGHT",
                   "retry":{"times":1,"delay_ms":0}}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        assert_eq!(shared.clicks.borrow().len(), 1);
        let events = interp.events();
        assert_eq!(
            events.iter().filter(|e| e.result == "retry").count(),
            1,
            "exactly one retry event: {events:?}"
        );
        let ok = events
            .iter()
            .find(|e| e.op == "click" && e.result == "ok")
            .expect("successful click event");
        assert!(ok.detail.contains("(retried 1)"), "{}", ok.detail);
    }

    #[test]
    fn click_retry_exhaustion_fails_with_evidence() {
        let (input, _shared) = FakeInput::new();
        let mut interp = Interpreter::new(FakeScreen::new(), input, assets(), Budget::default())
            .with_humanize(no_miss())
            .with_seed(21);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click","args":{"sel":"I_NEVER",
                   "retry":{"times":2,"delay_ms":0}}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        match interp.run(&f) {
            RunResult::Failed(msg) => assert!(msg.contains("not found"), "{msg}"),
            other => panic!("expected a loud failure, got {other:?}"),
        }
        let events = interp.events();
        assert_eq!(events.iter().filter(|e| e.result == "retry").count(), 2);
        assert!(events.last().map(|e| e.result == "failed" && e.detail.contains("retries exhausted")).unwrap_or(false), "{events:?}");
    }

    // ---- §22.16 checkpoint recovery: start_step + progress sink ----

    struct RecordingSink {
        steps: std::rc::Rc<std::cell::RefCell<Vec<(usize, String)>>>,
    }

    impl ProgressSink for RecordingSink {
        fn step_done(&mut self, index: usize, op: &str) {
            self.steps
                .borrow_mut()
                .push((index, op.to_string()));
        }
    }

    #[test]
    fn start_step_skips_the_checkpointed_prefix() {
        let (input, shared) = FakeInput::new();
        let mut interp = Interpreter::new(FakeScreen::new(), input, assets(), Budget::default())
            .with_humanize(no_miss())
            .with_seed(22)
            .with_start_step(1);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click_point","args":{"x":20,"y":20}},
                 {"op":"log","args":{"level":"info","text":"'resumed'"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        assert!(
            shared.clicks.borrow().is_empty(),
            "step 0 must be skipped when resuming"
        );
        assert!(
            interp.events().iter().any(|e| e.op == "log"),
            "step 1 must still run"
        );
    }

    #[test]
    fn progress_sink_reports_each_successful_top_level_step() {
        let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let (input, _shared) = FakeInput::new();
        let mut interp = Interpreter::new(FakeScreen::new(), input, assets(), Budget::default())
            .with_humanize(no_miss())
            .with_seed(23)
            .with_progress_sink(Box::new(RecordingSink { steps: seen.clone() }));
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click_point","args":{"x":20,"y":20}},
                 {"op":"log","args":{"level":"info","text":"'x'"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let got = seen.borrow().clone();
        assert_eq!(
            got,
            vec![
                (0, "click_point".to_string()),
                (1, "log".to_string()),
                (2, "finish".to_string())
            ]
        );
    }

    // ---- §22.10 N10b: polygon masks (match + OCR) ----

    #[test]
    fn mask_matching_ignores_changed_background_outside_the_polygon() {
        // Template tile 10x10 at (12,10); polygon = top/apex triangle. Pixels
        // OUTSIDE the polygon are changed in the frame; masked matching must not
        // care (plain matching would score much lower).
        let pattern: Vec<u8> = (0..100)
            .map(|i| (((i % 10) * 7 + (i / 10) * 13) % 251 + 3) as u8)
            .collect();
        let mut frame = frame_with_tile(12, 10);
        let poly = vec![
            Point { x: 12, y: 10 },
            Point { x: 22, y: 10 },
            Point { x: 17, y: 20 },
        ];
        for j in 0..10u32 {
            for i in 0..10u32 {
                let p = Point {
                    x: 12 + i as i32,
                    y: 10 + j as i32,
                };
                if !point_in_polygon(p, &poly) {
                    let idx = (((10 + j) * frame.width + (12 + i)) * 4) as usize;
                    frame.rgba[idx] = 255 - frame.rgba[idx];
                }
            }
        }
        let gray = GrayImage::from_frame(&frame);
        let tpl = GrayImage::new(10, 10, pattern.clone());
        let roi = Rect {
            x: 12,
            y: 10,
            w: 10,
            h: 10,
        };
        let mut mask = vec![false; 100];
        for j in 0..10i32 {
            for i in 0..10i32 {
                if point_in_polygon(
                    Point {
                        x: 12 + i,
                        y: 10 + j,
                    },
                    &poly,
                ) {
                    mask[(j * 10 + i) as usize] = true;
                }
            }
        }
        let plain = oas_perceive::match_template(&gray, &tpl, Some(roi))
            .map(|b| b.score)
            .unwrap_or(-2.0);
        let masked = oas_perceive::Matcher::from_ref(&gray)
            .match_template_masked(&tpl, &mask, Some(roi))
            .map(|b| b.score)
            .unwrap_or(-2.0);
        assert!(plain < 0.99, "plain NCC should drop with changed pixels: {plain}");
        assert!(masked > 0.99, "masked NCC should ignore outside pixels: {masked}");

        let mut assets = AssetSet::new();
        assets.insert(
            "I_MASK",
            Template {
                gray: tpl,
                roi: Some(roi),
                threshold: 0.995,
                click_polygon: Some(poly),
            },
        );
        let screen = TwoFrameScreen {
            frames: vec![frame],
            idx: 0,
        };
        let (input, shared) = FakeInput::new();
        let mut interp = Interpreter::new(screen, input, assets, Budget::default())
            .with_humanize(no_miss())
            .with_seed(31);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click","args":{"sel":"I_MASK"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let p = shared.clicks.borrow()[0];
        assert!(
            p.x >= 12 && p.x < 22 && p.y >= 10 && p.y < 20,
            "click must stay inside the matched template: {p:?}"
        );
    }

    struct ProbeOcr;
    impl OcrProvider for ProbeOcr {
        fn recognize(&mut self, frame: &Frame, _roi: Rect) -> Result<(String, f32), String> {
            let at = |x: u32, y: u32| -> u8 {
                frame.rgba[((y * frame.width + x) * 4) as usize]
            };
            // inside polygon (10,5) unchanged; outside (30,5) must be filled white.
            assert_eq!(at(10, 5), 100, "inside-polygon pixel must survive masking");
            assert_eq!(at(30, 5), 255, "outside-polygon pixel must be filled white");
            Ok(("ok".to_string(), 1.0))
        }
        fn recognize_lines(&mut self, _frame: &Frame, _roi: Rect) -> Result<Vec<OcrLine>, String> {
            Err("single-line only".into())
        }
    }

    #[test]
    fn ocr_polygon_mask_fills_pixels_outside_the_polygon() {
        let (w, h) = (64u32, 48u32);
        let mut rgba = vec![10u8; (w * h * 4) as usize];
        for i in (3..rgba.len()).step_by(4) {
            rgba[i] = 255;
        }
        let idx = |x: u32, y: u32| ((y * w + x) * 4) as usize;
        rgba[idx(10, 5)] = 100; // inside polygon
        rgba[idx(30, 5)] = 50; // outside polygon
        let frame = Frame::new(w, h, rgba);
        let mut assets = AssetSet::new();
        assets.insert_ocr(
            "O_MASK",
            Rect {
                x: 0,
                y: 0,
                w: 32,
                h: 16,
            },
        );
        assets.set_ocr_polygon(
            "O_MASK",
            vec![
                Point { x: 0, y: 0 },
                Point { x: 20, y: 0 },
                Point { x: 0, y: 15 },
            ],
        );
        let screen = TwoFrameScreen {
            frames: vec![frame],
            idx: 0,
        };
        let (input, _shared) = FakeInput::new();
        let mut interp = Interpreter::new(screen, input, assets, Budget::default())
            .with_humanize(no_miss())
            .with_ocr(Box::new(ProbeOcr))
            .with_seed(32);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"ocr","args":{"sel":"O_MASK"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
    }

    // ---- CD2 click audit (§21) ----

    #[test]
    fn click_point_records_one_cd2_row_with_the_evaluated_target() {
        let (mut interp, input) = mk_inspect(FakeScreen::new());
        interp = interp.with_seed(99);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click_point","args":{"x":20,"y":20}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let rows = interp.clicks();
        assert_eq!(rows.len(), 1, "one successful click => exactly one row: {rows:?}");
        let r = &rows[0];
        // target = the evaluated (pre-jitter) point, x/y = the ACTUAL point.
        assert_eq!((r.target_x, r.target_y), (20, 20), "{r:?}");
        assert_eq!(r.sel, None, "click_point has no asset id");
        assert_eq!(r.seed, 99, "the audit must carry the seeded run seed");
        let actual = input.clicks.borrow()[0];
        assert_eq!(
            (r.x, r.y),
            (actual.x, actual.y),
            "CD2 x/y must be the point actually handed to InputSink"
        );
        assert!(
            (r.x - 20).abs() <= 6 && (r.y - 20).abs() <= 6,
            "HB2: {r:?} is not within ±6 px of the target"
        );
        assert!(
            r.ts_ms > 1_600_000_000_000,
            "ts_ms must be epoch milliseconds, got {}",
            r.ts_ms
        );
    }

    #[test]
    fn click_template_records_the_asset_id_and_the_template_centre() {
        let (mut interp, _input) = mk_inspect(FakeScreen::new());
        interp = interp.with_seed(5);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click","args":{"sel":"I_BRIGHT"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let rows = interp.clicks();
        assert_eq!(rows.len(), 1);
        let r = &rows[0];
        assert_eq!(r.sel.as_deref(), Some("I_BRIGHT"));
        // The 10x10 template matches at (12,10) => its rect centre is (17,15).
        assert_eq!((r.target_x, r.target_y), (17, 15), "{r:?}");
        // The actual point is a humanised point inside that template rect.
        assert!(
            (12..22).contains(&r.x) && (10..20).contains(&r.y),
            "actual point ({},{}) left the matched template rect",
            r.x,
            r.y
        );
        assert_eq!(r.seed, 5);
    }

    #[test]
    fn failed_click_records_no_audit_row() {
        let (mut interp, input) = mk_inspect(FakeScreen::new());
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click","args":{"sel":"I_NEVER"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert!(matches!(interp.run(&f), RunResult::Failed(_)));
        assert!(input.clicks.borrow().is_empty(), "no InputSink click may happen");
        assert!(
            interp.clicks().is_empty(),
            "a failed click must not appear in the click distribution: {:?}",
            interp.clicks()
        );
    }

    #[test]
    fn clicks_are_cleared_when_the_same_interpreter_runs_again() {
        let (mut interp, _input) = mk_inspect(FakeScreen::new());
        interp = interp.with_seed(3);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click_point","args":{"x":20,"y":20}},
                 {"op":"click_point","args":{"x":30,"y":25}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        assert_eq!(interp.clicks().len(), 2);
        assert_eq!(interp.clicks()[0].seed, 3);
        // A rerun is a new run: same lifecycle as `events` (cleared, not appended).
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        assert_eq!(
            interp.clicks().len(),
            2,
            "rerun must not append to the previous run's click audit"
        );
    }

    #[test]
    fn run_seed_is_resolved_once_and_pinned_by_with_seed() {
        // Construction resolves seed 0 into a concrete seed (HB5) ...
        let (interp, _input) = mk_inspect(FakeScreen::new());
        assert_ne!(interp.run_seed(), 0, "seed 0 must resolve at construction");
        // ... and seed_now() (the recorder's view) is unchanged: initial state.
        assert_eq!(interp.run_seed(), interp.seed_now());
        // with_seed pins BOTH the RNG and the recorded run seed.
        let (pinned, _input2) = mk_inspect(FakeScreen::new());
        let pinned = pinned.with_seed(1234);
        assert_eq!(pinned.run_seed(), 1234);
        assert_eq!(pinned.seed_now(), 1234);
    }

    // ---- swipe (§3.2 action class) ----

    #[test]
    fn swipe_executes_in_frame_and_logs_endpoints() {
        let (mut interp, input) = mk_inspect(FakeScreen::new());
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"swipe","args":{"from":[10,20],"to":["40 + 10","30"],"ms":400}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let ev = &interp.events()[0];
        assert_eq!(ev.op, "swipe");
        assert_eq!(ev.result, "ok");
        assert_eq!(
            ev.detail,
            "swiped from=(10,20) to=(50,30) ms=400 frame=64x48"
        );
        // deterministic: the endpoints are passed through untouched
        assert_eq!(
            input.swipes.borrow().as_slice(),
            &[(Point { x: 10, y: 20 }, Point { x: 50, y: 30 }, 400)]
        );
    }

    #[test]
    fn swipe_out_of_frame_endpoint_fails_without_input() {
        let (mut interp, input) = mk_inspect(FakeScreen::new());
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"swipe","args":{"from":[10,20],"to":[64,20]}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let r = interp.run(&f);
        match &r {
            RunResult::Failed(msg) => {
                assert!(msg.contains("outside the frame"), "{msg}");
                assert!(msg.contains("64x48"), "{msg}");
            }
            other => panic!("expected Failed, got {other:?}"),
        }
        assert_eq!(r.outcome(), Outcome::Failed);
        assert!(input.swipes.borrow().is_empty(), "off-screen swipe must not be sent");
        let last = interp.events().last().expect("event");
        assert_eq!(last.op, "swipe");
        assert_eq!(last.result, "failed");
    }

    // ---- v1.1 拟人拖动 / 独立区域 ----

    #[test]
    fn drag_region_randomizes_endpoints_and_respects_direction() {
        let (mut interp, input) = mk_inspect(FakeScreen::new());
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"drag","args":{
                   "from":{"kind":"region","roi":[5,20,20,20]},
                   "direction":"right",
                   "distance":[10,20],
                   "duration_ms":[100,120],
                   "margin":0.0
                 }},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let swipes = input.swipes.borrow().clone();
        assert_eq!(swipes.len(), 1, "exactly one swipe");
        let (from, to, ms) = swipes[0];
        assert!(from.x >= 5 && from.x < 25 && from.y >= 20 && from.y < 40, "{from:?}");
        assert!(to.x > from.x, "direction right: {from:?} -> {to:?}");
        let dist = to.x - from.x;
        assert!((10..=20).contains(&dist), "distance {dist} outside [10,20]");
        assert!((100..=120).contains(&ms), "duration {ms} outside [100,120]");
        let ev = interp
            .events()
            .iter()
            .rev()
            .find(|e| e.op == "drag")
            .expect("drag event");
        assert!(ev.detail.contains("direction=right"), "{}", ev.detail);
        assert!(ev.detail.contains("randomized=true"), "{}", ev.detail);
    }

    #[test]
    fn drag_to_target_rejects_wrong_direction() {
        let (mut interp, input) = mk_inspect(FakeScreen::new());
        // source right side, target left side, direction right => unreachable.
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"drag","args":{
                   "from":{"kind":"region","roi":[40,10,10,10]},
                   "to":{"kind":"region","roi":[5,10,10,10]},
                   "direction":"right"
                 }},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        match interp.run(&f) {
            RunResult::Failed(msg) => assert!(msg.contains("unreachable"), "{msg}"),
            other => panic!("expected Failed, got {other:?}"),
        }
        assert!(input.swipes.borrow().is_empty(), "invalid drag must not swipe");
    }

    #[test]
    fn region_locator_clicks_a_random_point_inside_roi() {
        let (mut interp, input) = mk_inspect(FakeScreen::new());
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click_target","args":{"find":[{"kind":"region","roi":[10,20,30,20]}]}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let clicks = input.clicks.borrow().clone();
        assert_eq!(clicks.len(), 1);
        let p = clicks[0];
        assert!(
            p.x >= 6 && p.x <= 44 && p.y >= 16 && p.y <= 44,
            "click should be inside region (humanize jitter allowed): {p:?}"
        );
        assert_eq!(interp.clicks()[0].locator, "region");
    }

    #[test]
    fn ocr_asset_action_roi_controls_click_landing() {
        let mut a = assets();
        a.insert_ocr(
            "O_LINE",
            Rect {
                x: 10,
                y: 10,
                w: 20,
                h: 10,
            },
        );
        a.set_click_back(
            "O_LINE",
            Rect {
                x: 5,
                y: 5,
                w: 60,
                h: 30,
            },
        );
        let (input, shared) = FakeInput::new();
        let mut interp = Interpreter::new(FakeScreen::new(), input, a, Budget::default())
            .with_humanize(no_miss())
            .with_ocr(Box::new(FixedLineOcr {
                rect: Rect {
                    x: 10,
                    y: 10,
                    w: 20,
                    h: 10,
                },
                text: "确认".to_string(),
            }))
            .with_seed(11);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click_target","args":{"find":[{"kind":"ocr_text","sel":"O_LINE","text":"确认"}]}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let clicks = shared.clicks.borrow().clone();
        assert_eq!(clicks.len(), 1);
        let p = clicks[0];
        assert!(
            p.x >= 0 && p.x <= 70 && p.y >= 0 && p.y <= 40,
            "OCR click should land in the operation region (jitter allowed): {p:?}"
        );
        assert_eq!(interp.clicks()[0].locator, "ocr_text");
    }

    #[test]
    fn derived_ocr_line_uses_original_source_roi_and_translates_action_region() {
        let mut a = assets();
        // Saved line frame: recognition line bbox + operation region.
        a.insert_ocr(
            "O_DERIVED",
            Rect {
                x: 10,
                y: 10,
                w: 20,
                h: 8,
            },
        );
        a.set_click_back(
            "O_DERIVED",
            Rect {
                x: 5,
                y: 5,
                w: 30,
                h: 20,
            },
        );
        // Original multi-line source region; det+rec must run on THIS region.
        a.set_derived_ocr(
            "O_DERIVED",
            Rect {
                x: 0,
                y: 0,
                w: 60,
                h: 40,
            },
            "确认".to_string(),
        );
        let (ocr, rois) = FakeDetOcr::new(vec![line(
            "确认",
            0.98,
            Rect {
                x: 10,
                y: 18, // line moved down 8px vs saved line bbox
                w: 20,
                h: 8,
            },
        )]);
        let (input, shared) = FakeInput::new();
        let mut interp = Interpreter::new(FakeScreen::new(), input, a, Budget::default())
            .with_humanize(no_miss())
            .with_ocr(Box::new(ocr))
            .with_seed(14);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click_target","args":{"find":[{"kind":"ocr_text","sel":"O_DERIVED","text":"确认"}]}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        // Provider was asked with the ORIGINAL source ROI, not the tight line ROI.
        let seen = rois.borrow().clone();
        assert_eq!(seen.len(), 1);
        assert_eq!(
            (seen[0].x, seen[0].y, seen[0].w, seen[0].h),
            (0, 0, 60, 40)
        );
        // Operation region translated by (matched line y - saved line y) = +8.
        let clicks = shared.clicks.borrow().clone();
        assert_eq!(clicks.len(), 1);
        let p = clicks[0];
        assert!(
            p.x >= 0 && p.x <= 40 && p.y >= 8 && p.y <= 38,
            "click should land in translated operation region: {p:?}"
        );
    }

    #[test]
    fn saved_region_ref_clicks_inside_the_operation_region() {
        let mut a = assets();
        a.insert_region(
            "R_LIST",
            Rect {
                x: 10,
                y: 20,
                w: 30,
                h: 20,
            },
        );
        let (input, shared) = FakeInput::new();
        let mut interp = Interpreter::new(FakeScreen::new(), input, a, Budget::default())
            .with_humanize(no_miss())
            .with_seed(12);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click_target","args":{"find":[{"kind":"region","sel":"R_LIST"}]}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let clicks = shared.clicks.borrow().clone();
        assert_eq!(clicks.len(), 1);
        let p = clicks[0];
        assert!(p.x >= 6 && p.x <= 44 && p.y >= 16 && p.y <= 44, "{p:?}");
        assert_eq!(interp.clicks()[0].locator, "region");
    }

    #[test]
    fn variables_can_count_across_a_loop() {
        let (mut interp, _input) = mk_inspect(FakeScreen::new());
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"set_var","args":{"name":"n","value":"0"}},
                 {"op":"loop","args":{"n":"3"},"body":[
                   {"op":"inc_var","args":{"name":"n"}},
                   {"op":"log","args":{"text":"var('n')"}}
                 ]},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        assert_eq!(interp.vars.get("n"), Some(&Value::Num(3.0)));
    }

    #[test]
    fn ocr_to_var_exposes_recognized_text() {
        let (input, _shared) = FakeInput::new();
        let mut a = assets();
        a.insert_ocr(
            "O_LINE",
            Rect {
                x: 10,
                y: 10,
                w: 20,
                h: 10,
            },
        );
        let mut interp = Interpreter::new(FakeScreen::new(), input, a, Budget::default())
            .with_humanize(no_miss())
            .with_ocr(Box::new(FixedLineOcr {
                rect: Rect {
                    x: 10,
                    y: 10,
                    w: 20,
                    h: 10,
                },
                text: "确认".to_string(),
            }))
            .with_seed(13);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"ocr_to_var","args":{"sel":"O_LINE","var":"answer"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        assert_eq!(
            interp.vars.get("answer"),
            Some(&Value::Str("确认".to_string()))
        );
    }

    // ---- param() injection (M0 unified task template) ----

    #[test]
    fn param_values_are_injected_into_expressions() {
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"log","args":{"text":"param('max_rounds') * 2"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let mut interp = mk(FakeScreen::new(), Budget::default()).with_params(
            std::collections::BTreeMap::from([("max_rounds".to_string(), Value::Num(3.0))]),
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let detail = &interp.events()[0].detail;
        assert!(detail.contains("Num(6.0)"), "detail: {detail}");
    }

    #[test]
    fn with_param_adds_one_value_and_can_be_chained() {
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"log","args":{"text":"param('flag') == true and param('mode') == 'fast'"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let mut interp = mk(FakeScreen::new(), Budget::default())
            .with_param("flag", Value::Bool(true))
            .with_param("mode", Value::Str("fast".to_string()));
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        assert!(interp.events()[0].detail.contains("true"));
    }

    #[test]
    fn missing_param_value_fails_the_run_loudly() {
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"log","args":{"text":"param('nope')"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let mut interp = mk(FakeScreen::new(), Budget::default());
        match interp.run(&f) {
            RunResult::Failed(msg) => {
                assert!(msg.contains("nope"), "must name the missing key: {msg}");
                assert!(msg.contains("param"), "must mention param(): {msg}");
            }
            other => panic!("expected Failed, got {other:?}"),
        }
    }
}

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}
