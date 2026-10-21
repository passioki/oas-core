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

use oas_flow::{eval, EvalContext, Flow, Step, Value};
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
}

impl AssetSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, id: impl Into<String>, tpl: Template) {
        self.map.insert(id.into(), tpl);
    }

    pub fn get(&self, id: &str) -> Option<&Template> {
        self.map.get(id)
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
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
    events: Vec<StepEvent>,
    started: Instant,
    steps: u32,
}

impl<S: ScreenSource, I: InputSink> Interpreter<S, I> {
    pub fn new(src: S, input: I, assets: AssetSet, budget: Budget) -> Self {
        Self {
            src,
            input,
            assets,
            budget,
            matcher: None,
            events: Vec::new(),
            started: Instant::now(),
            steps: 0,
        }
    }

    pub fn events(&self) -> &[StepEvent] {
        &self.events
    }

    pub fn run(&mut self, flow: &Flow) -> RunResult {
        self.started = Instant::now();
        self.events.clear();
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

    /// Grab a frame and cache its matcher (integral images) for this frame.
    fn refresh_frame(&mut self) -> Result<(), String> {
        let frame: Frame = self.src.grab()?;
        let gray = GrayImage::from_frame(&frame);
        self.matcher = Some(Matcher::new(gray));
        Ok(())
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
                    self.matcher = None; // next eval grabs a fresh frame
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
                self.input.click(p)?;
                Ok(format!("clicked {sel} at ({},{}) score={score:.3}", p.x, p.y))
            }
            Step::Ocr { sel } => Err(format!(
                "OCR is not wired yet (P1-c): step ocr(`{sel}`) cannot run"
            )),
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
        Err(format!("OCR is not wired yet (P1-c): ocr(`{sel}`)"))
    }

    fn ocr_digit(&mut self, sel: &str) -> Result<Option<f64>, String> {
        Err(format!("OCR is not wired yet (P1-c): ocr_digit(`{sel}`)"))
    }

    fn page(&mut self) -> Result<Option<String>, String> {
        Ok(None) // page recognition lands in a later phase
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oas_flow::Flow;

    /// Fake screen: one 64x48 frame with a bright 10x10 block at (12,10).
    struct FakeScreen {
        frame: Frame,
        grabs: usize,
    }

    impl FakeScreen {
        fn new() -> Self {
            let (w, h) = (64u32, 48u32);
            let mut rgba = vec![30u8; (w * h * 4) as usize];
            // same patterned 10x10 tile as the asset, pasted at (12,10)
            for y in 0..10u32 {
                for x in 0..10u32 {
                    let v = (((x * 7 + y * 13) % 251) + 3) as u8;
                    let i = (((10 + y) * w + (12 + x)) * 4) as usize;
                    rgba[i] = v;
                    rgba[i + 1] = v;
                    rgba[i + 2] = v;
                    rgba[i + 3] = 255;
                }
            }
            Self {
                frame: Frame::new(w, h, rgba),
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

    struct FakeInput {
        clicks: Vec<Point>,
    }

    impl InputSink for FakeInput {
        fn click(&mut self, p: Point) -> Result<(), String> {
            self.clicks.push(p);
            Ok(())
        }
        fn swipe(&mut self, _f: Point, _t: Point, _ms: u32) -> Result<(), String> {
            Ok(())
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
        Interpreter::new(src, FakeInput { clicks: vec![] }, assets(), budget)
    }

    fn flow(json: &str) -> Flow {
        Flow::from_json(json).expect("flow should load")
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
        // the click landed on the bright block
        let click = &interp.events()[1];
        assert!(click.detail.contains("at (12,10)"), "{}", click.detail);
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

    #[test]
    fn ocr_is_not_silently_zero() {
        let mut interp = mk(FakeScreen::new(), Budget::default());
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"if","args":{"cond":"ocr('I_BRIGHT') == '1'"},
                  "then":[{"op":"log","args":{"text":"'one'"}}]},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        match interp.run(&f) {
            RunResult::Failed(msg) => assert!(msg.contains("OCR is not wired"), "{msg}"),
            other => panic!("expected Failed (OCR not wired), got {other:?}"),
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
}
