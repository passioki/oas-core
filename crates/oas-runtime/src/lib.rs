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

use std::collections::HashMap;
use std::time::{Duration, Instant};

use oas_flow::{eval, EvalContext, Expr, Flow, Step, Value};
use oas_perceive::{GrayImage, Matcher};
use oas_types::{Exit, Frame, InputSink, Outcome, Point, Rect, ScreenSource};

/// One declared asset: template pixels + optional search ROI + threshold.
pub struct Template {
    pub gray: GrayImage,
    pub roi: Option<Rect>,
    pub threshold: f32,
}

/// The task's declared assets (`assets/<Name>.json` in a later phase).
#[derive(Default)]
pub struct AssetSet {
    map: HashMap<String, Template>,
    /// OCR assets (`kind: ocr_line` / `ocr_lines`): they carry **no pixels**,
    /// only a frame ROI. `ocr(sel)` crops that ROI and hands it to the injected
    /// [`OcrProvider`] (P1-c, ENGINE-DESIGN §16.4/§22.4).
    ocr_rois: HashMap<String, Rect>,
}

impl AssetSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, id: impl Into<String>, tpl: Template) {
        self.map.insert(id.into(), tpl);
    }

    /// Declare an OCR asset (no template pixels, only an ROI).
    pub fn insert_ocr(&mut self, id: impl Into<String>, roi: Rect) {
        self.ocr_rois.insert(id.into(), roi);
    }

    pub fn get(&self, id: &str) -> Option<&Template> {
        self.map.get(id)
    }

    /// ROI of a declared OCR asset, if `id` is one.
    pub fn ocr_roi(&self, id: &str) -> Option<Rect> {
        self.ocr_rois.get(id).copied()
    }

    pub fn ocr_len(&self) -> usize {
        self.ocr_rois.len()
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

/// Recognise text inside a frame ROI (P1-c).
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
pub trait OcrProvider {
    fn recognize(&mut self, frame: &Frame, roi: Rect) -> Result<(String, f32), String>;
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
    /// Size of the frame `matcher` was built from (`click_point` needs it for
    /// the CP3 in-frame check).
    frame_size: Option<(u32, u32)>,
    events: Vec<StepEvent>,
    /// CD2 (§21) click audit: one row per SUCCESSFUL click, cleared per run.
    clicks: Vec<ClickRecord>,
    /// Seed resolved once at construction (HB5); `with_seed` pins it.
    run_seed: u64,
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
            frame_size: None,
            events: Vec::new(),
            clicks: Vec::new(),
            run_seed,
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
        let mut ctl = Ctl::Normal;
        match self.exec(&flow.steps, 0, &mut ctl) {
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
    fn record_click(&mut self, actual: Point, target: Point, sel: Option<String>) {
        self.clicks.push(ClickRecord {
            ts_ms: now_ms(),
            x: actual.x,
            y: actual.y,
            target_x: target.x,
            target_y: target.y,
            sel,
            seed: self.run_seed,
        });
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
    fn locate(&mut self, sel: &str) -> Result<(f32, Option<Point>), String> {
        if self.matcher.is_none() {
            self.refresh_frame()?;
        }
        let tpl = self
            .assets
            .get(sel)
            .ok_or_else(|| format!("selector `{sel}` is not declared in the task's assets"))?;
        let best = self
            .matcher
            .as_ref()
            .ok_or_else(|| "no frame available".to_string())?
            .match_template(&tpl.gray, tpl.roi);
        match best {
            Some(b) => Ok((b.score, Some(Point { x: b.x, y: b.y }))),
            None => Ok((0.0, None)),
        }
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

    /// Resolve an OCR asset's ROI and recognise it in the cached frame (P1-c).
    ///
    /// Failure semantics (all loud, never a silent `""`):
    /// * `sel` not declared / declared as a pixel template ⇒ error;
    /// * no [`OcrProvider`] attached ⇒ error;
    /// * the provider itself fails (empty ROI, out of bounds, ONNX error) ⇒ error.
    ///
    /// An **empty recognised text is not an error**: it maps to `None` (= `Null`
    /// in expressions), so `wait_until(ocr('X') == '分组2')` can keep polling and
    /// a missing text can never satisfy the comparison.
    fn ocr_run(&mut self, sel: &str) -> Result<(Option<String>, f32), String> {
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
        if self.frame.is_none() {
            self.refresh_frame()?;
        }
        let frame = self
            .frame
            .as_ref()
            .ok_or_else(|| "OCR: no frame available".to_string())?;
        let provider = self.ocr.as_mut().expect("provider checked above");
        let (text, score) = provider.recognize(frame, roi)?;
        Ok((if text.is_empty() { None } else { Some(text) }, score))
    }

    fn ocr_digit_value(&mut self, sel: &str) -> Result<Option<f64>, String> {
        match self.ocr_run(sel)? {
            (Some(text), _) => Ok(parse_digit(&text)),
            (None, _) => Ok(None),
        }
    }

    fn exec(&mut self, steps: &[Step], depth: usize, ctl: &mut Ctl) -> Result<(), String> {
        for step in steps {
            self.check_budget()?;
            self.steps += 1;
            if !matches!(ctl, Ctl::Normal) {
                return Ok(());
            }
            let t0 = Instant::now();
            match self.exec_one(step, depth, ctl) {
                Ok(detail) => {
                    let ms = t0.elapsed().as_secs_f64() * 1000.0;
                    let op = step_name(step);
                    self.log(depth, op, &detail, "ok", ms);
                    if matches!(ctl, Ctl::Finish(_)) {
                        return Ok(());
                    }
                }
                Err(e) => {
                    let ms = t0.elapsed().as_secs_f64() * 1000.0;
                    let op = step_name(step);
                    self.log(depth, op, &format!("error: {e}"), "failed", ms);
                    return Err(e);
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
            Step::While {
                cond,
                max_iters,
                body,
            } => {
                let mut iters = 0u32;
                loop {
                    self.check_budget()?;
                    if iters >= *max_iters {
                        return Err(format!("{EXHAUSTED}.iteration limit reached"));
                    }
                    if !matches!(eval(cond, self)?, Value::Bool(true)) {
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
                let th = self.assets.get(sel).map(|t| t.threshold).unwrap_or(0.9);
                if score >= th {
                    Ok(format!("appear {sel} score={score:.3} at {at:?}"))
                } else {
                    Err(format!("selector `{sel}` not found (best score {score:.3} < {th})"))
                }
            }
            Step::Click { sel } => {
                let (score, at) = self.locate(sel)?;
                let th = self.assets.get(sel).map(|t| t.threshold).unwrap_or(0.9);
                let Some(p) = at.filter(|_| score >= th) else {
                    return Err(format!("click target `{sel}` not found (best score {score:.3} < {th})"));
                };
                // HB1: the target box is the MATCHED TEMPLATE RECT, not an
                // 11x11 box at its top-left. Picking inside the real box is what
                // keeps the click distribution spread over the button; a fixed
                // box would defeat the whole anti-ban rule.
                let (tw, th_px) = self
                    .assets
                    .get(sel)
                    .map(|t| (t.gray.width, t.gray.height))
                    .ok_or_else(|| format!("selector `{sel}` is not declared in the task's assets"))?;
                let target = Rect {
                    x: p.x,
                    y: p.y,
                    w: tw as i32,
                    h: th_px as i32,
                };
                let actual = self.humanize.point_in(target, &mut self.rng);
                // Keep a humanised point inside the frame (a target touching an
                // edge can otherwise jitter off-screen). `locate` guarantees a
                // frame, so the missing-frame branch is a loud invariant break.
                let (w, h) = self
                    .frame_size
                    .ok_or_else(|| "click: no frame available".to_string())?;
                let actual = Point {
                    x: actual.x.clamp(0, w.saturating_sub(1) as i32),
                    y: actual.y.clamp(0, h.saturating_sub(1) as i32),
                };
                let delay = self.humanize.delay_ms(&mut self.rng);
                if delay > 0 {
                    std::thread::sleep(std::time::Duration::from_millis(delay as u64));
                }
                self.input.click(actual)?;
                // The screen may now be different: never let the next selector
                // match against this pre-click frame.
                self.invalidate_frame();
                // CD2/HB6: audit the click with the template-rect CENTRE as the
                // intended target and the humanised point as what actually went out.
                let centre = Point {
                    x: target.x + target.w / 2,
                    y: target.y + target.h / 2,
                };
                self.record_click(actual, centre, Some(sel.clone()));
                Ok(format!(
                    "clicked {sel} target=({},{}) point=({},{}) delay={delay}ms score={score:.3}",
                    p.x, p.y, actual.x, actual.y
                ))
            }
            Step::ClickPoint { x, y } => {
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
                // HB2: a raw coordinate click still gets +/- jitter_px of noise.
                let jittered = self.humanize.jitter_point(target, &mut self.rng);
                // ...but the ACTUAL point must stay on-screen.
                let actual = Point {
                    x: jittered.x.clamp(0, w.saturating_sub(1) as i32),
                    y: jittered.y.clamp(0, h.saturating_sub(1) as i32),
                };
                // HB3: randomised pre-click pause (not a machine-perfect rhythm).
                let delay = self.humanize.delay_ms(&mut self.rng);
                if delay > 0 {
                    std::thread::sleep(Duration::from_millis(delay as u64));
                }
                self.input.click(actual)?;
                self.invalidate_frame();
                // CD2: `click_point` has no asset id; the evaluated target is the
                // intent and the jittered point is what the device received.
                self.record_click(actual, target, None);
                // CP4/HB6: the log carries the TARGET *and* the actual point
                // (plus the jitter's delay and the frame it was checked against).
                Ok(format!(
                    "clicked_point target=({},{}) point=({},{}) delay={delay}ms frame={}x{}",
                    target.x, target.y, actual.x, actual.y, w, h
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
            Step::Ocr { sel } => {
                // P1-c: the standalone `ocr` step exists so an editor-authored
                // flow can PROVE what a selector reads without branching on it
                // (a plain expression has no step log). Empty text is reported,
                // not failed: the loud failure is only a missing provider/asset.
                let (text, score) = self.ocr_run(sel)?;
                Ok(match text {
                    Some(t) => format!("ocr {sel} = {t:?} score={score:.4}"),
                    None => format!("ocr {sel} = <no text> score={score:.4}"),
                })
            }
            Step::Log { level, text } => {
                let v = eval(text, self)?;
                Ok(format!("[{level}] {v:?}"))
            }
            Step::Finish { exit } => {
                *ctl = Ctl::Finish(*exit);
                Ok(format!("finish {exit:?}"))
            }
        }
    }
}

/// Marker error used to signal budget/loop exhaustion (mapped to Exhausted).
const EXHAUSTED: &str = "__exhausted__";

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
        Step::Ocr { .. } => "ocr",
        Step::Log { .. } => "log",
        Step::Finish { .. } => "finish",
    }
}

impl<S: ScreenSource, I: InputSink> EvalContext for Interpreter<S, I> {
    fn appear(&mut self, sel: &str) -> Result<bool, String> {
        let (score, _) = self.locate(sel)?;
        Ok(score >= self.assets.get(sel).map(|t| t.threshold).unwrap_or(0.9))
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
            },
        );
        set
    }

    fn mk(src: FakeScreen, budget: Budget) -> Interpreter<FakeScreen, FakeInput> {
        Interpreter::new(src, FakeInput::new().0, assets(), budget)
    }

    /// `assets()` plus two declared OCR assets (`ocr_line` kind → ROI only).
    fn assets_with_ocr() -> AssetSet {
        let mut set = assets();
        set.insert_ocr("O_TEXT", Rect { x: 4, y: 6, w: 20, h: 10 });
        set.insert_ocr("O_DIGITS", Rect { x: 4, y: 30, w: 30, h: 10 });
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
            Interpreter::new(src, input, assets(), Budget::default()),
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
        let mut interp = mk(FakeScreen::new(), Budget::default());
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"while","args":{"cond":"true","max_iters":3},
                  "body":[{"op":"log","args":{"text":"'spin'"}}]},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let r = interp.run(&f);
        assert_eq!(r.outcome(), Outcome::Exhausted);
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
}

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}
