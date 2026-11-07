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

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use oas_flow::{
    eval, DragAxis, DragShape, DragTarget, EvalContext, Expect, Expr, Flow, InterruptRule, Locator,
    OpRegion, Step, TextSpec, Value,
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

/// Operation-region shape used by click sampling.
#[derive(Clone, Debug, PartialEq)]
pub struct ClickShape {
    pub rect: Option<Rect>,
    pub points: Option<Vec<Point>>,
    pub invert: bool,
}

impl ClickShape {
    /// Axis-aligned bbox of the shape (frame coordinates).
    ///
    /// Math is done in `i64` (P1-4): with `i32` points, `x1 - x0 + 1` can
    /// overflow at `i32::MAX`, which used to panic in debug and wrap in
    /// release. A bbox that cannot be represented back as an `i32` rect
    /// returns `None`; [`Self::sample`] then fails loudly instead of inventing
    /// a point.
    pub fn bbox(&self) -> Option<Rect> {
        if let Some(r) = self.rect {
            return Some(r);
        }
        let pts = self.points.as_ref()?;
        if pts.is_empty() {
            return None;
        }
        let mut x0 = i32::MAX;
        let mut y0 = i32::MAX;
        let mut x1 = i32::MIN;
        let mut y1 = i32::MIN;
        for p in pts {
            x0 = x0.min(p.x);
            y0 = y0.min(p.y);
            x1 = x1.max(p.x);
            y1 = y1.max(p.y);
        }
        let w = x1 as i64 - x0 as i64 + 1;
        let h = y1 as i64 - y0 as i64 + 1;
        if w <= 0 || h <= 0 || w > i32::MAX as i64 || h > i32::MAX as i64 {
            return None;
        }
        Some(Rect {
            x: x0,
            y: y0,
            w: w as i32,
            h: h as i32,
        })
    }

    pub fn contains(&self, p: Point) -> bool {
        if let Some(r) = self.rect {
            // i64 keeps `r.x + r.w` from overflowing when a rect touches
            // i32::MAX (P1-4); the comparison is otherwise identical.
            let px = p.x as i64;
            let py = p.y as i64;
            return px >= r.x as i64
                && py >= r.y as i64
                && px < r.x as i64 + r.w as i64
                && py < r.y as i64 + r.h as i64;
        }
        let Some(pts) = self.points.as_ref() else {
            return false;
        };
        if pts.len() < 3 {
            return false;
        }
        let mut inside = false;
        let mut j = pts.len() - 1;
        for i in 0..pts.len() {
            let (xi, yi) = (pts[i].x, pts[i].y);
            let (xj, yj) = (pts[j].x, pts[j].y);
            if (yi > p.y) != (yj > p.y) {
                // f64 subtraction avoids an `i32` overflow in `xj - xi` for
                // extreme (programmatic) coordinates; integer frame coords keep
                // exactly the same result.
                let denom = yj as f64 - yi as f64;
                let cross_x = (xj as f64 - xi as f64) * (p.y as f64 - yi as f64) / denom
                    + xi as f64;
                if (p.x as f64) < cross_x {
                    inside = !inside;
                }
            }
            j = i;
        }
        inside
    }

    pub fn effective_contains(&self, p: Point) -> bool {
        let inside = self.contains(p);
        if self.invert {
            !inside
        } else {
            inside
        }
    }

    /// Sample one integer point that the **effective** operation region contains.
    ///
    /// Unlike the old silent implementation this is a loud `Result`:
    /// * an empty effective region (e.g. a full-screen `invert` shape) is an
    ///   error, never "click the excluded area";
    /// * a degenerate polygon (collinear points, zero area, fewer than 3 points)
    ///   is an error, never the bbox centre;
    /// * the deterministic scan fallback only returns a point that
    ///   [`Self::effective_contains`] accepts.
    pub fn sample(&self, w: u32, h: u32, rng: &mut crate::humanize::Rng) -> Result<Point, String> {
        if w == 0 || h == 0 {
            return Err("exact operation shape needs a non-empty screen".to_string());
        }
        // Defense in depth (P1-4): the flow parser already rejects these, but
        // ClickShape is also built from asset action_shapes and tests. A
        // degenerate geometry is a loud error, never a panic or a silent
        // bbox/centre fallback.
        if let Some(r) = self.rect {
            if r.w <= 0 || r.h <= 0 {
                return Err(format!(
                    "operation shape rect has non-positive size {}x{}",
                    r.w, r.h
                ));
            }
        }
        if let Some(pts) = self.points.as_ref() {
            if pts.is_empty() {
                return Err("exact operation shape has no geometry".to_string());
            }
        }
        let bbox = self.bbox().ok_or_else(|| {
            "operation shape bbox does not fit the i32 coordinate range".to_string()
        })?;
        let in_frame = |p: Point| p.x >= 0 && p.y >= 0 && p.x < w as i32 && p.y < h as i32;
        if self.invert {
            // 1) Random rejection sampling (fast for normal popups).
            for _ in 0..4096 {
                let p = Point {
                    x: rng.range_i32(0, w.saturating_sub(1) as i32),
                    y: rng.range_i32(0, h.saturating_sub(1) as i32),
                };
                if !self.contains(p) {
                    return Ok(p);
                }
            }
            // 2) Deterministic proof that the effective region is empty. Only
            //    pays the full-frame scan when random sampling already failed.
            for y in 0..h as i32 {
                for x in 0..w as i32 {
                    let p = Point { x, y };
                    if !self.contains(p) {
                        return Ok(p);
                    }
                }
            }
            return Err(format!(
                "inverted operation shape leaves no clickable point on the {}x{} screen",
                w, h
            ));
        }
        if let Some(pts) = self.points.as_ref() {
            if pts.len() < 3 {
                return Err(format!(
                    "operation polygon needs at least 3 points (got {})",
                    pts.len()
                ));
            }
            if polygon_area2(pts) == 0 {
                return Err("operation polygon has zero area (collinear points)".to_string());
            }
        }
        // P1-4: intersect the shape bbox with the frame before sampling. A
        // shape that is entirely off-screen is a loud failure, not "sample the
        // bbox centre". Computed in i64 so no edge can wrap. Invert is exempt:
        // its effective region is the whole frame minus the shape, so an
        // off-screen excluded shape is still fully sampleable.
        let bx0 = bbox.x as i64;
        let by0 = bbox.y as i64;
        let bx1 = bbox.x as i64 + bbox.w as i64 - 1;
        let by1 = bbox.y as i64 + bbox.h as i64 - 1;
        let x0 = bx0.max(0);
        let y0 = by0.max(0);
        let x1 = bx1.min(w as i64 - 1);
        let y1 = by1.min(h as i64 - 1);
        if x0 > x1 || y0 > y1 {
            return Err(format!(
                "operation shape has no clickable point: bbox [{},{},{}x{}] does not intersect the {}x{} frame",
                bbox.x, bbox.y, bbox.w, bbox.h, w, h
            ));
        }
        // 1) Random inside the bbox.
        for _ in 0..4096 {
            let p = Point {
                x: rng.range_i32(bbox.x, bbox.x.saturating_add(bbox.w).saturating_sub(1)),
                y: rng.range_i32(bbox.y, bbox.y.saturating_add(bbox.h).saturating_sub(1)),
            };
            if in_frame(p) && self.contains(p) {
                return Ok(p);
            }
        }
        // 2) Deterministic scan inside the bbox ∩ screen. Catches thin polygons
        //    that random sampling can miss without ever inventing a point.
        for y in y0 as i32..=y1 as i32 {
            for x in x0 as i32..=x1 as i32 {
                let p = Point { x, y };
                if self.contains(p) {
                    return Ok(p);
                }
            }
        }
        Err(format!(
            "operation shape has no clickable point inside the {}x{} frame",
            w, h
        ))
    }

    /// Return the shape shifted by `(dx, dy)` (frame coordinates).
    ///
    /// Used by the OCR self-geometry rule (2026-11-06 批次B #4/Q1): an OCR
    /// asset's authored `action_shape` follows the real recognized line by
    /// `real.topLeft - roi.topLeft`, exactly like a template shape follows its
    /// match. `invert` is a frame-wide exclusion and is left untouched by
    /// callers, but shifting it is still lossless (points move with the frame).
    pub fn translated(&self, dx: i32, dy: i32) -> ClickShape {
        if dx == 0 && dy == 0 {
            return self.clone();
        }
        let mut out = self.clone();
        if let Some(r) = out.rect.as_mut() {
            r.x = r.x.saturating_add(dx);
            r.y = r.y.saturating_add(dy);
        }
        if let Some(pts) = out.points.as_mut() {
            for p in pts {
                p.x = p.x.saturating_add(dx);
                p.y = p.y.saturating_add(dy);
            }
        }
        out
    }
}

/// One resolved `drag` endpoint (v1.2.5 O-2).
///
/// `rect` is the historical bbox (the margin domain and the fallback when the
/// flow carries no `shape`); `shape` is the optional exact operation shape,
/// converted field-for-field into the same [`ClickShape`] the click path uses.
struct DragEndpoint {
    rect: Rect,
    shape: Option<ClickShape>,
}

impl DragEndpoint {
    fn shape_label(&self) -> &'static str {
        match self.shape.as_ref() {
            None => "none",
            Some(s) if s.rect.is_some() => "rect",
            Some(_) => "polygon",
        }
    }

    fn invert(&self) -> bool {
        self.shape.as_ref().is_some_and(|s| s.invert)
    }

    /// `margin` only has the documented "inset the candidate rectangle" meaning
    /// (EDITOR-UX-PLAN §15.3), so it exists **only** on the bbox path: no shape,
    /// or a **non-inverted rect** shape (which is sampled from `endpoint.rect`,
    /// not from the shape). Those keep `margin` active with no extra detail
    /// token. A polygon or an inverted shape samples its own effective region,
    /// where an inset has no defined meaning, so `margin` is ignored and the
    /// step detail records `margin=ignored`.
    fn ignores_margin(&self) -> bool {
        match self.shape.as_ref() {
            None => false,
            Some(s) => s.rect.is_none() || s.invert,
        }
    }
}

/// One resolved `long_press` target (v1.3 operation-region P1).
///
/// `find[]` is resolved exactly like `click_target`'s redundancy chain, but the
/// action is a hold instead of a tap: this struct carries the point to hold plus
/// the provenance needed for the step log.
struct PressTarget {
    point: Point,
    locator: &'static str,
    sel: Option<String>,
    match_info: String,
    region: Option<OpRegion>,
}


/// v1.2.5 (O-2): a flow `region` drag shape is the same geometry as an asset
/// `action_shape`, so drag sampling and click sampling share one algorithm.
/// `oas_task::ActionShape::to_click_shape` does the identical mapping.
impl From<&DragShape> for ClickShape {
    fn from(shape: &DragShape) -> Self {
        ClickShape {
            rect: shape.roi.map(|r| Rect {
                x: r[0],
                y: r[1],
                w: r[2],
                h: r[3],
            }),
            points: shape.points.as_ref().map(|pts| {
                pts.iter()
                    .map(|p| Point { x: p[0], y: p[1] })
                    .collect()
            }),
            invert: shape.invert,
        }
    }
}

/// Shoelace area × 2. `0` means the vertices are collinear/degenerate.
fn polygon_area2(points: &[Point]) -> i64 {
    let mut area2: i64 = 0;
    for i in 0..points.len() {
        let a = points[i];
        let b = points[(i + 1) % points.len()];
        area2 += a.x as i64 * b.y as i64 - b.x as i64 * a.y as i64;
    }
    area2
}

/// Maximum `op_region` chain depth (mirrors `oas_task::MAX_OP_REGION_DEPTH`;
/// a runtime `visited` set is the primary cycle guard, this is the belt-and-
/// braces bound).
pub const MAX_OP_REGION_CHAIN_DEPTH: usize = 16;

/// Runtime view of one `kind:"ocr_var"` asset (使用端OCR文字配置项,
/// 2026-11-06 参数→素材 redesign).
///
/// It reuses the OCR machinery: the recognition text is
/// `params[param_key]` (runner value) over `runtime_text` (asset default), and
/// the read is always det + rec so every hit has a real line bbox. `roi=None`
/// means whole-screen OCR.
#[derive(Debug, Clone, PartialEq)]
pub struct OcrVar {
    pub roi: Option<Rect>,
    pub runtime_text: String,
    pub param_key: String,
}

/// Candidate pick rule of an `op_region` `asset` hop (Lead ruling 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpRegionPick {
    /// Highest score wins (template best match / OCR highest confidence).
    Score,
    /// Centre nearest to the previous node's **real** recognized rect centre;
    /// tied distance breaks on the higher score.
    Nearest,
}

/// Operation-region chain source stored on one asset (`assets.json
/// items[].op_region`, 2026-11-06 参数→素材 redesign D2).
///
/// This is the runtime, dependency-light copy of `oas_task::OpRegion` (the
/// task crate depends on this crate, so the conversion happens in
/// `TaskBundle::load_runtime_assets`). Named `AssetOpRegion` to avoid clashing
/// with the flow-level `oas_flow::OpRegion` (the v1.3 `region` step field).
///
/// Chain semantics (model B, restored 2026-11-06 seq127 #2/#3): `self` = this
/// node's own operation area; `asset` = recognize the referenced node and use
/// **its** `op_region` recursively — the final operation box is the LAST
/// referenced node's own self geometry. The former `offset` mode was removed.
#[derive(Debug, Clone, PartialEq)]
pub enum AssetOpRegion {
    /// Use the node's own operation area (historical `click_box_for`/`click_back`
    /// semantics); the default when the asset has no `op_region`.
    SelfRegion,
    /// Recognize `asset` as an extra gate and continue with *its* `op_region`.
    Asset { asset: String, pick: OpRegionPick },
}

impl AssetOpRegion {
    pub fn mode_str(&self) -> &'static str {
        match self {
            AssetOpRegion::SelfRegion => "self",
            AssetOpRegion::Asset { .. } => "asset",
        }
    }
}

/// One resolved hop of an `op_region` chain (diagnostics / step log).
#[derive(Debug, Clone, PartialEq)]
pub struct OpRegionChainStep {
    /// Asset/collection id of this hop.
    pub asset: String,
    /// `self` / `asset` / `region` / `collection` / `default`.
    pub mode: &'static str,
    /// This node's **real** recognized rect (None for fixed terminals).
    pub real: Option<Rect>,
    pub score: Option<f32>,
    pub text: Option<String>,
    /// seq134: how many single-source candidates this hop had, and which one was
    /// picked (multi-candidate recognition is now observable in the chain log).
    pub candidate_count: usize,
    pub candidate_index: usize,
    /// The operation box this hop produces. For a terminal `self` hop this is
    /// the final click area (model B: the last referenced node's own geometry).
    pub action_box: Rect,
}

/// A fully resolved `op_region` chain: the final operation box (+ exact shape)
/// from the **terminal self node** and every recognized hop for the log.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedOpRegion {
    pub rect: Rect,
    pub shape: Option<ClickShape>,
    pub chain: Vec<OpRegionChainStep>,
}

impl ResolvedOpRegion {
    /// One-line chain summary for the step log
    /// (`chain=[I_A:self -> I_B:offset -> R:region]`).
    pub fn log_suffix(&self) -> String {
        let ids: Vec<String> = self
            .chain
            .iter()
            .map(|s| format!("{}:{}", s.asset, s.mode))
            .collect();
        format!(" region_chain=[{}]", ids.join(" -> "))
    }
}

/// One resolved editor dry-run (`POST /task/asset/test_action`, B-T3b).
///
/// The editor shows `steps` (the recognition chain), the final `action_box` and
/// a concrete `point` (inside the effective operation shape) for its canvas
/// preview; the same `point` is what a `real` request taps.
#[derive(Debug, Clone, PartialEq)]
pub struct PreviewAction {
    /// Root target asset/collection id.
    pub target: String,
    /// The final operation box = the **last referenced node's own self
    /// geometry** (model B); with a `self` root this is the root's own area.
    pub action_box: Rect,
    /// Exact operation shape of the final hop (polygon / invert aware).
    pub shape: Option<ClickShape>,
    /// A concrete point inside the effective operation shape (dry preview and
    /// the real tap).
    pub point: Point,
    /// Root's real recognized rect (None for a fixed `region` target).
    pub real: Option<Rect>,
    pub score: Option<f32>,
    /// Every hop (root first) for the request's `steps` field.
    pub steps: Vec<OpRegionChainStep>,
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
    /// Exact operation geometry (v0.1): polygon / invert aware. `click_back`
    /// remains the bbox fallback for old tests and simple rectangles.
    click_shapes: HashMap<String, ClickShape>,
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
    /// Optional **minimum recognition confidence** (0~1) per OCR asset. Absent
    /// means "any successful read counts" — the historical rule, preserved so
    /// existing assets keep their behaviour. When present, an OCR result whose
    /// score is below this value is treated as a miss, exactly like a template
    /// below its NCC threshold.
    ocr_thresholds: HashMap<String, f32>,
    /// `kind:"ocr_var"` assets (使用端OCR文字配置项): recognition text comes from
    /// the injected `params[param_key]` over `runtime_text`; `roi=None` means
    /// whole-screen OCR. Kept in a parallel table so the historical
    /// `insert_ocr`/`ocr_roi` contract is untouched.
    ocr_vars: HashMap<String, OcrVar>,
    /// Expected text of **plain** OCR assets (`ocr_line`/`ocr_lines` with a
    /// saved `ocr_text`, 2026-11-06 批次B #3): lets a non-ocr_var OCR asset act
    /// as an `op_region` recognition gate and filters its editor candidates.
    ocr_expected: HashMap<String, String>,
    /// Operation-region chain per asset (`assets.json items[].op_region`); an
    /// absent/`SelfRegion` entry keeps the historical click geometry.
    op_regions: HashMap<String, AssetOpRegion>,
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

    pub fn set_click_shape(&mut self, id: impl Into<String>, shape: ClickShape) {
        self.click_shapes.insert(id.into(), shape);
    }

    pub fn click_shape(&self, id: &str) -> Option<&ClickShape> {
        self.click_shapes.get(id)
    }

    /// Translate a template's exact operation shape from authoring coordinates
    /// into current-frame coordinates by `matched - roi` (the same translation
    /// the runtime click path applies). Exposed so the editor's collection
    /// `real=1` test samples the identical shape instead of re-implementing it.
    /// OCR/region/collection-level shapes are absolute: callers pass the id
    /// that actually owns the shape, and assets without an roi are unchanged.
    pub fn translate_click_shape(
        &self,
        mut shape: ClickShape,
        id: &str,
        matched: Point,
    ) -> ClickShape {
        if let Some(t) = self.map.get(id) {
            if let Some(roi) = t.roi {
                let dx = matched.x - roi.x;
                let dy = matched.y - roi.y;
                if dx != 0 || dy != 0 {
                    if let Some(r) = shape.rect.as_mut() {
                        r.x += dx;
                        r.y += dy;
                    }
                    if let Some(pts) = shape.points.as_mut() {
                        for p in pts {
                            p.x += dx;
                            p.y += dy;
                        }
                    }
                }
            }
        }
        shape
    }

    /// Declare a pure operation region (`kind: region`, v1.2).
    pub fn insert_region(&mut self, id: impl Into<String>, rect: Rect) {
        self.regions.insert(id.into(), rect);
    }

    pub fn region(&self, id: &str) -> Option<Rect> {
        self.regions.get(id).copied()
    }

    /// Register a multi-line-derived OCR line: recognition ROI + expected text.
    pub fn set_derived_ocr(&mut self, id: impl Into<String>, recognition_roi: Rect, expected: String) {
        self.derived_ocr.insert(id.into(), (recognition_roi, expected));
    }

    /// Declare one `kind:"ocr_var"` asset (2026-11-06 参数→素材 redesign).
    pub fn insert_ocr_var(
        &mut self,
        id: impl Into<String>,
        roi: Option<Rect>,
        runtime_text: String,
        param_key: String,
    ) {
        self.ocr_vars.insert(
            id.into(),
            OcrVar {
                roi,
                runtime_text,
                param_key,
            },
        );
    }

    /// The `ocr_var` declaration, if `id` is one.
    pub fn ocr_var(&self, id: &str) -> Option<&OcrVar> {
        self.ocr_vars.get(id)
    }

    /// Declare one asset's operation-region chain.
    pub fn insert_op_region(&mut self, id: impl Into<String>, op: AssetOpRegion) {
        self.op_regions.insert(id.into(), op);
    }

    /// The asset's declared operation-region chain, if any.
    pub fn op_region(&self, id: &str) -> Option<&AssetOpRegion> {
        self.op_regions.get(id)
    }

    /// `true` when the asset declares a chain that replaces the historical
    /// geometry (`op_region` present and not `self`). Old assets return `false`,
    /// so the old click path stays byte-identical.
    pub fn has_op_region_chain(&self, id: &str) -> bool {
        self.op_regions
            .get(id)
            .is_some_and(|op| !matches!(op, AssetOpRegion::SelfRegion))
    }

    /// `true` for every OCR-backed asset kind (`ocr_line` / `ocr_lines` /
    /// `ocr_var`, including derived lines). The single kind test for OCR, since
    /// an `ocr_var` may have no ROI and therefore not appear in the `ocr` table.
    pub fn is_ocr_asset(&self, id: &str) -> bool {
        self.ocr.contains_key(id)
            || self.ocr_vars.contains_key(id)
            || self.derived_ocr.contains_key(id)
    }

    /// The expected recognition text of an `ocr_var`: the injected parameter value
    /// (`params[param_key]`, Runner value) wins over the asset default
    /// `runtime_text`. Both blank/missing is a loud error — never "match
    /// anything" (Lead ruling 12: one expected-text helper for editor + runtime).
    pub fn ocr_var_expected_text(
        &self,
        id: &str,
        params: &BTreeMap<String, Value>,
    ) -> Result<String, String> {
        let Some(var) = self.ocr_vars.get(id) else {
            return Err(format!("`{id}` is not an ocr_var asset"));
        };
        if let Some(value) = params.get(&var.param_key) {
            return match value {
                Value::Str(s) if !s.trim().is_empty() => Ok(s.clone()),
                Value::Str(_) => Err(format!(
                    "ocr_var `{id}`: parameter `{}` is an empty string \
                     (usage-side override must not be blank)",
                    var.param_key
                )),
                other => Err(format!(
                    "ocr_var `{id}`: parameter `{}` must be a string, got {other:?}",
                    var.param_key
                )),
            };
        }
        if var.runtime_text.trim().is_empty() {
            return Err(format!(
                "ocr_var `{id}` has no recognition text (runtime_text is blank and \
                 parameter `{}` was not injected)",
                var.param_key
            ));
        }
        Ok(var.runtime_text.clone())
    }

    pub fn derived_ocr(&self, id: &str) -> Option<(Rect, String)> {
        self.derived_ocr.get(id).cloned()
    }

    /// Register the expected text of a plain OCR asset (`ocr_lines`/`ocr_line`
    /// with a saved `ocr_text`, 2026-11-06 批次B #3). Such an asset can then act
    /// as an `op_region` recognition gate and its editor candidates are filtered
    /// by the same text.
    pub fn set_ocr_expected_text(&mut self, id: impl Into<String>, expected: String) {
        self.ocr_expected.insert(id.into(), expected);
    }

    pub fn ocr_expected_text(&self, id: &str) -> Option<&str> {
        self.ocr_expected.get(id).map(String::as_str)
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

    /// Declare a **minimum recognition confidence** for an OCR asset.
    ///
    /// `assets.json` stores this in the same `threshold` field used for template
    /// NCC. Absent and `0` both mean "recognized = hit" (no confidence filter):
    /// the editor can only clear a threshold by writing `0` (`update_asset_meta`
    /// reads `None` as "leave unchanged"), so both spellings must be equivalent
    /// (P1-4). The value is stored verbatim; [`Self::ocr_threshold`] normalizes
    /// it on every read.
    pub fn set_ocr_threshold(&mut self, id: impl Into<String>, threshold: f32) {
        self.ocr_thresholds.insert(id.into(), threshold);
    }

    /// Minimum recognition confidence of an OCR asset, if one is **active**.
    ///
    /// `None` covers both "never declared" and an explicit `0`: `0 = unset =
    /// no confidence filter`. This is the single normalization point every
    /// caller (click / appear / expect / drag / ocr_text) relies on.
    pub fn ocr_threshold(&self, id: &str) -> Option<f32> {
        self.ocr_thresholds.get(id).copied().filter(|t| *t > 0.0)
    }

    /// Threshold for a plain template, a collection (collection override wins)
    /// or an OCR asset (explicit minimum recognition confidence).
    ///
    /// OCR assets return `Some` only when the stored value is **> 0**; `0` and
    /// "never declared" both yield `None`. Callers then apply their kind-specific
    /// fallback: `0.0` for OCR presence/click (never reject) and the historical
    /// `0.9` for templates/collections.
    pub fn threshold_for(&self, id: &str) -> Option<f32> {
        self.collection_thresholds
            .get(id)
            .copied()
            .or_else(|| self.map.get(id).map(|t| t.threshold))
            .or_else(|| self.ocr_threshold(id))
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

// ---------------------------------------------------------------------------
// v1.3 operation region (docs/OPERATION-REGION-DESIGN.md §6.1)
// ---------------------------------------------------------------------------

/// Centre of a rect (`i64` math, clamped back into `i32`).
fn rect_centre(r: Rect) -> Point {
    Point {
        x: (r.x as i64 + r.w as i64 / 2).clamp(i32::MIN as i64, i32::MAX as i64) as i32,
        y: (r.y as i64 + r.h as i64 / 2).clamp(i32::MIN as i64, i32::MAX as i64) as i32,
    }
}

/// Is `inner` fully inside `outer` (used to label an OCR line as `domain=roi`)?
fn rect_inside_rect(outer: Rect, inner: Rect) -> bool {
    inner.x >= outer.x
        && inner.y >= outer.y
        && inner.x + inner.w <= outer.x + outer.w
        && inner.y + inner.h <= outer.y + outer.h
}

/// Matching OCR line candidates for one expected text, declaration order kept.
///
/// `min` is the asset's declared OCR confidence (None = no threshold). Shared by
/// `click_target` (both OCR forms), `drag` endpoints and `long_press`; the
/// public wrapper [`matching_ocr_lines`] is also used by the editor candidate
/// API (Lead ruling 12: one expected-text/filter rule for editor + runtime).
fn ocr_line_candidates<'a>(
    lines: &'a [OcrLine],
    text: &str,
    min: Option<f32>,
) -> Vec<&'a OcrLine> {
    // Historical fuzzy helper (unit tests pin its exact semantics). Runtime
    // interactions use the strict single-source engine (seq134 #1).
    lines
        .iter()
        .filter(|l| {
            ocr_text_matches(text, l.text.as_str()) && min.map(|m| l.score >= m).unwrap_or(true)
        })
        .collect()
}

/// Public view of [`ocr_line_candidates`]: every OCR line matching `expected`
/// (exact/normalized/fuzzy with #11 digit protection) at/above the optional
/// minimum confidence. Runtime click/wait/chain keep this fuzzy rule.
///
/// The editor candidate API uses the stricter [`matching_ocr_lines_strict`]
/// (normalized equality, no 1-edit tolerance), so a digit-variant row can never
/// be offered as a candidate; both helpers share the same normalization.
pub fn matching_ocr_lines<'a>(
    lines: &'a [OcrLine],
    expected: &str,
    min: Option<f32>,
) -> Vec<&'a OcrLine> {
    ocr_line_candidates(lines, expected, min)
}

/// Strict-CJK candidate filter (2026-11-06 批次A, #3/#11): every OCR line whose
/// normalized text **exactly equals** `expected` at/above the optional minimum
/// confidence. No 1-edit tolerance, so 「队伍1」 never yields 「队伍2」/「队伍3」
/// and 「召唤」 does not yield 「召喚」.
///
/// Editor candidates use this stricter rule; runtime click/wait/chain keep the
/// fuzzy [`matching_ocr_lines`]. The public API is additive: the old signature
/// and behavior stay intact (serve keeps compiling).
pub fn matching_ocr_lines_strict<'a>(
    lines: &'a [OcrLine],
    expected: &str,
    min: Option<f32>,
) -> Vec<&'a OcrLine> {
    lines
        .iter()
        .filter(|line| {
            ocr_text_matches_strict(expected, line.text.as_str())
                && min.map(|m| line.score >= m).unwrap_or(true)
        })
        .collect()
}

/// `region.mode="nearest"`: the candidate whose bbox centre is nearest to
/// `reference` (squared distance; ties keep declaration order).
fn nearest_ocr_line<'a>(
    candidates: &[&'a OcrLine],
    reference: Point,
) -> Option<&'a OcrLine> {
    let mut best: Option<(&'a OcrLine, i64)> = None;
    for &line in candidates {
        let c = rect_centre(line.rect);
        let dx = c.x as i64 - reference.x as i64;
        let dy = c.y as i64 - reference.y as i64;
        let dist2 = dx * dx + dy * dy;
        match best {
            Some((_, best_dist)) if dist2 >= best_dist => {}
            _ => best = Some((line, dist2)),
        }
    }
    best.map(|(line, _)| line)
}

/// Highest-score OCR line, ties keep declaration order (seq134: `drag ocr_text`
/// picks the highest candidate, not the first detected line).
fn pick_line_score_first<'a>(candidates: &[&'a OcrLine]) -> Option<&'a OcrLine> {
    let mut best: Option<&'a OcrLine> = None;
    for &line in candidates {
        let better = match best {
            None => true,
            Some(b) => line.score > b.score,
        };
        if better {
            best = Some(line);
        }
    }
    best
}

/// Shift (and optionally resize) a base bbox by an `offset` region.
///
/// Arithmetic is `i64` + clamp (same P1-4 posture as `random_point_in`): the
/// parser bounds every value at 1e6, so this can never overflow an `i32`.
fn offset_region_rect(base: Rect, dx: i32, dy: i32, w: Option<i32>, h: Option<i32>) -> Rect {
    let width = w.unwrap_or(base.w).max(1);
    let height = h.unwrap_or(base.h).max(1);
    Rect {
        x: (base.x as i64 + dx as i64).clamp(i32::MIN as i64, i32::MAX as i64) as i32,
        y: (base.y as i64 + dy as i64).clamp(i32::MIN as i64, i32::MAX as i64) as i32,
        w: width,
        h: height,
    }
}

/// Clamp a point into the frame (same policy as `perform_click`).
fn clamp_to_frame(p: Point, w: u32, h: u32) -> Point {
    Point {
        x: p.x.clamp(0, w.saturating_sub(1) as i32),
        y: p.y.clamp(0, h.saturating_sub(1) as i32),
    }
}

/// One-line step-log suffix for an operation-region override.
///
/// Empty when the flow has no `region` (old flows keep byte-identical step
/// details); otherwise ` region=<mode>` plus ` asset=<id>` for the v1.4 A-plan.
fn region_log(region: Option<&OpRegion>) -> String {
    let Some(region) = region else {
        return String::new();
    };
    let mut s = format!(" region={}", region.mode_str());
    if let Some(asset) = region.asset() {
        s.push_str(&format!(" asset={asset}"));
    }
    s
}

/// OCR text comparison. OCR models often emit a traditional/variant CJK glyph
/// for the same on-screen word (e.g. 召喚 vs 召唤). Exact equality made a saved
/// asset fail even though the workbench showed the text. Normalize whitespace
/// and allow one substitution/insertion/deletion for multi-character strings.
///
/// 2026-11-06 批次A (#11): **ASCII digit protection**. After normalization the
/// ordered ASCII digit runs of both sides must be identical; otherwise the
/// strings never match (so 「队伍1」 no longer matches 「队伍2」/「队伍3」/
/// 「队伍12」 through the 1-edit tolerance). Full-width digits (０..９) are folded
/// to ASCII during normalization, so a font that renders `1` as `１` still
/// matches; CJK variant glyphs (`召喚`/`召唤`) keep the 1-edit tolerance.
pub fn ocr_text_matches(expected: &str, actual: &str) -> bool {
    let a = ocr_normalize(expected);
    let b = ocr_normalize(actual);
    if a == b {
        return true;
    }
    // Digit protection (batch A / #11): never fuzzy-match across different
    // ASCII digit groups. This runs before the length/edit-distance gate so
    // 队伍1 vs 队伍12 is rejected even though it is a 1-edit pair.
    if ocr_digit_runs(&a) != ocr_digit_runs(&b) {
        return false;
    }
    let ac: Vec<char> = a.chars().collect();
    let bc: Vec<char> = b.chars().collect();
    if ac.len() < 2 || bc.len() < 2 || ac.len().abs_diff(bc.len()) > 1 {
        return false;
    }
    // Levenshtein distance with a cap of 1.
    let n = ac.len();
    let m = bc.len();
    let mut prev: Vec<usize> = (0..=m).collect();
    for i in 1..=n {
        let mut cur = vec![i; m + 1];
        for j in 1..=m {
            let cost = usize::from(ac[i - 1] != bc[j - 1]);
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        prev = cur;
    }
    prev[m] <= 1
}

/// Shared OCR text normalization (fuzzy matcher + strict candidate filter):
/// drop whitespace and ASCII punctuation, lowercase, and fold full-width ASCII
/// digits (U+FF10..U+FF19) to ASCII digits.
fn ocr_normalize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_whitespace() || c.is_ascii_punctuation() {
            continue;
        }
        for lower in c.to_lowercase() {
            let folded = match lower {
                '\u{FF10}'..='\u{FF19}' => {
                    char::from_u32('0' as u32 + (lower as u32 - '\u{FF10}' as u32))
                        .unwrap_or(lower)
                }
                other => other,
            };
            out.push(folded);
        }
    }
    out
}

/// Ordered ASCII digit runs of an already-normalized string (「第1队第2层」 →
/// `["1", "2"]`). Used by the #11 digit protection; pure-text strings produce an
/// empty list on both sides and keep the historical fuzzy behavior.
fn ocr_digit_runs(normalized: &str) -> Vec<&str> {
    let mut runs: Vec<&str> = Vec::new();
    let mut start: Option<usize> = None;
    for (i, c) in normalized.char_indices() {
        if c.is_ascii_digit() {
            if start.is_none() {
                start = Some(i);
            }
        } else if let Some(s) = start.take() {
            runs.push(&normalized[s..i]);
        }
    }
    if let Some(s) = start {
        runs.push(&normalized[s..]);
    }
    runs
}

/// Strict OCR text comparison (2026-11-06 批次A, #3/#11): normalized **exact**
/// equality, no 1-edit tolerance. Used by [`matching_ocr_lines_strict`] (editor
/// candidates); runtime click/wait keep the fuzzy [`ocr_text_matches`].
fn ocr_text_matches_strict(expected: &str, actual: &str) -> bool {
    ocr_normalize(expected) == ocr_normalize(actual)
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

/// Template-pixel mask from an asset polygon (frame coords → template coords).
/// Public so the editor/batch tests can use the exact same matching semantics.
pub fn template_polygon_mask(tpl: &Template) -> Option<Vec<bool>> {
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

/// One template match: authored ROI (masked when a polygon exists) fast path,
/// then whole-frame pyramid fallback; returns the best score + point.
/// Shared by the interpreter, `/task/asset/match`, and batch frame tests.
pub fn locate_template(matcher: &Matcher, tpl: &Template) -> (f32, Option<Point>) {
    let roi_best = match template_polygon_mask(tpl) {
        Some(mask) => matcher.match_template_masked(&tpl.gray, &mask, tpl.roi),
        None => matcher.match_template(&tpl.gray, tpl.roi),
    };
    if let Some(b) = &roi_best {
        if b.score >= 0.95 {
            return (b.score, Some(Point { x: b.x, y: b.y }));
        }
    }
    let full_best = matcher.match_template_full(&tpl.gray);
    match (roi_best, full_best) {
        (Some(a), Some(b)) if b.score > a.score => (b.score, Some(Point { x: b.x, y: b.y })),
        (Some(a), _) => (a.score, Some(Point { x: a.x, y: a.y })),
        (None, Some(b)) => (b.score, Some(Point { x: b.x, y: b.y })),
        (None, None) => (0.0, None),
    }
}

/// Threshold a selector's candidates must reach.
///
/// * template / collection: the declared NCC threshold, else `0.9`;
/// * OCR (`ocr_line` / `ocr_lines`, incl. derived lines): the asset's explicit
///   minimum confidence when **> 0**, else **0** — an explicit `0` means
///   "unset = no confidence filter", exactly like a missing value.
///
/// Public single source of truth: the interpreter's click path and the editor's
/// candidate list (`GET /task/asset/candidates`) must agree byte-for-byte on
/// what counts as a candidate.
pub fn asset_threshold(assets: &AssetSet, sel: &str) -> f32 {
    if assets.is_ocr_asset(sel) {
        match assets.ocr_threshold(sel) {
            Some(t) if t > 0.0 => t,
            _ => 0.0,
        }
    } else {
        assets.threshold_for(sel).unwrap_or(0.9)
    }
}

/// Operation box **and** exact shape of one template match, in current-frame
/// coordinates. `matched` is the template top-left position returned by the
/// matcher; the shape is authored relative to the template `roi`, so it is
/// translated by `matched - roi` (the same translation the runtime click and
/// the editor's collection test apply).
///
/// Resolution order is the runtime click path's, unchanged: translated
/// `action_shape` bbox → translated `click_back` → matched template bbox. An
/// inverted shape covers the whole frame. A template that is not declared (or
/// whose shape cannot be translated because it has no `roi`) is a loud error,
/// exactly like `click_box_for`.
///
/// Extracted from [`Interpreter::click_box_for`] so the editor candidate API can
/// report the operation area of **every** candidate without re-implementing the
/// geometry (a copy would drift from the engine).
pub fn template_operation_geometry(
    assets: &AssetSet,
    id: &str,
    matched: Point,
    frame_size: Option<(u32, u32)>,
) -> Result<(Rect, Option<ClickShape>), String> {
    let t = assets
        .get(id)
        .ok_or_else(|| format!("selector `{id}` is not declared in the task's assets"))?;
    if let Some(shape) = assets.click_shape(id) {
        let translated = assets.translate_click_shape(shape.clone(), id, matched);
        if shape.invert {
            let (w, h) = frame_size
                .ok_or_else(|| "inverted operation region needs a screen size".to_string())?;
            return Ok((
                Rect {
                    x: 0,
                    y: 0,
                    w: w as i32,
                    h: h as i32,
                },
                Some(translated),
            ));
        }
        if let Some(back) = shape.bbox() {
            let roi = t.roi.ok_or_else(|| {
                format!("selector `{id}` has an operation shape but no roi to translate it")
            })?;
            return Ok((
                Rect {
                    x: back.x + (matched.x - roi.x),
                    y: back.y + (matched.y - roi.y),
                    w: back.w,
                    h: back.h,
                },
                Some(translated),
            ));
        }
    }
    if let Some(back) = assets.click_back(id) {
        let roi = t.roi.ok_or_else(|| {
            format!(
                "selector `{id}` has a roi_back but no roi to translate it \
                 (a click area only makes sense together with an authored roi)"
            )
        })?;
        return Ok((
            Rect {
                x: back.x + (matched.x - roi.x),
                y: back.y + (matched.y - roi.y),
                w: back.w,
                h: back.h,
            },
            None,
        ));
    }
    Ok((
        Rect {
            x: matched.x,
            y: matched.y,
            w: t.gray.width as i32,
            h: t.gray.height as i32,
        },
        None,
    ))
}

/// **Self-operation geometry** of one declared asset at a given **real
/// recognized rect** (`real`), shared by the runtime click paths and the editor
/// candidate API.
///
/// 2026-11-06 (#4/Q1 + seq127 #2/#3): this is the single-node `self` geometry
/// used by every chain hop, the candidate `action_box`, and the plain click path:
/// * `region` → its fixed `roi` (+ its exact shape), `real` ignored;
/// * template / collection → [`template_operation_geometry`] with
///   `matched = real.topLeft` (translated `action_shape` → translated
///   `click_back` → match bbox). For a **collection candidate** pass the winning
///   member's id (`AssetMatchCandidate::member`), not the collection id;
/// * OCR (`ocr_line` / `ocr_lines` / `ocr_var` / derived line) → the stored
///   `action_shape`/`click_back` translated by `real.topLeft - authored_roi`
///   (it follows the recognized line, exactly like a template follows its
///   match). An empty ROI (whole-screen `ocr_var`) has no authored origin, so its
///   stored coordinates stay **absolute**; no shape/back → `real` itself.
///
/// This is the exact geometry `resolve_asset_op_region` returns and
/// `preview_asset_action`/`click_box_for` use, so serve can compute candidate
/// `action_box` values without copying the rules. Errors are loud (undeclared
/// asset, invert without a frame size, template shape without an roi).
pub fn self_operation_geometry(
    assets: &AssetSet,
    id: &str,
    real: Rect,
    frame_size: Option<(u32, u32)>,
) -> Result<(Rect, Option<ClickShape>), String> {
    if let Some(rect) = assets.region(id) {
        return Ok((rect, assets.click_shape(id).cloned()));
    }
    if assets.is_ocr_asset(id) {
        let authored_roi = assets
            .ocr_var(id)
            .and_then(|v| v.roi)
            .or_else(|| assets.ocr_roi(id));
        let (dx, dy) = authored_roi
            .map(|roi| (real.x.saturating_sub(roi.x), real.y.saturating_sub(roi.y)))
            .unwrap_or((0, 0));
        if let Some(shape) = assets.click_shape(id) {
            if shape.invert {
                let (w, h) = frame_size.ok_or_else(|| {
                    format!("inverted operation region of `{id}` needs a screen size")
                })?;
                return Ok((
                    Rect {
                        x: 0,
                        y: 0,
                        w: w as i32,
                        h: h as i32,
                    },
                    Some(shape.clone()),
                ));
            }
            let translated = shape.translated(dx, dy);
            if let Some(bbox) = translated.bbox() {
                return Ok((bbox, Some(translated)));
            }
        }
        if let Some(back) = assets.click_back(id) {
            return Ok((
                Rect {
                    x: back.x.saturating_add(dx),
                    y: back.y.saturating_add(dy),
                    w: back.w,
                    h: back.h,
                },
                None,
            ));
        }
        return Ok((real, None));
    }
    template_operation_geometry(
        assets,
        id,
        Point {
            x: real.x,
            y: real.y,
        },
        frame_size,
    )
}

/// One recognition candidate of a template asset or a template collection, with
/// the operation geometry the runtime click path would use for that exact match.
///
/// This is the editor `GET /task/asset/candidates` payload source **and** the
/// engine's `region.asset mode=nearest` candidate source: both must enumerate
/// the same regions, so there is exactly one implementation.
/// Which search domain produced one recognition candidate (seq134).
///
/// * `Roi` — the asset's authored ROI (image ROI fast path, or an OCR line inside
///   the authored recognition ROI);
/// * `Screen` — the whole frame without a declared ROI (template without `roi`,
///   `ocr_var` without `roi`);
/// * `ScreenFallback` — the OCR runtime fallback: the declared ROI had no strict
///   expected-text match, so the whole frame was recognized (seq134 #2);
/// * `Region` — a fixed `region` asset (no recognition at all).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandidateDomain {
    Roi,
    Screen,
    ScreenFallback,
    Region,
}

impl CandidateDomain {
    pub fn as_str(&self) -> &'static str {
        match self {
            CandidateDomain::Roi => "roi",
            CandidateDomain::Screen => "screen",
            CandidateDomain::ScreenFallback => "screen_fallback",
            CandidateDomain::Region => "region",
        }
    }
}

/// One unified recognition candidate: image (template/collection member), text
/// (OCR line), or fixed region (seq134 single-source shape).
///
/// Sequence is: [`asset_match_candidates`] for templates, [`Interpreter::recognition_candidates`]
/// for every runtime path (editor preview/test_action/chain/operation).
#[derive(Debug, Clone, PartialEq)]
pub struct AssetMatchCandidate {
    /// The asset that produced the pixels: the asset id itself for a single
    /// template, or the member id inside a collection.
    pub member: String,
    /// Declared kind of `member` (`template` / `ocr_line` / `ocr_lines` /
    /// `ocr_var` / derived / `region`) — UI and chain steps display it.
    pub member_kind: String,
    pub score: f32,
    /// The **real recognition region** in frame coordinates: the matched
    /// template's bbox (`x`/`y` = template top-left, `w`/`h` = template size),
    /// or the recognized OCR line bbox.
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    /// Match/line bbox centre (the `nearest` selection reference).
    pub centre: Point,
    /// Candidate operation area in frame coordinates (see
    /// [`template_operation_geometry`] / [`self_operation_geometry`]).
    pub action_box: Rect,
    /// Exact operation shape in current-frame coordinates, when the asset has
    /// one (polygon / invert aware).
    pub action_shape: Option<ClickShape>,
    /// Recognized text for OCR candidates (`None` for image/region ones).
    pub text: Option<String>,
    /// Which search domain produced this candidate (ROI / whole screen / fallback
    /// / fixed region) — the editor shows the fallback hint.
    pub domain: CandidateDomain,
    /// `score >=` the producing asset/member threshold (seq134: pass is a pure
    /// threshold verdict, never an expected-text verdict).
    pub pass: bool,
}

/// Every NCC match candidate of `id` at or above the selector threshold.
///
/// * template asset → candidates of that template;
/// * template collection → candidates of every member template, concatenated in
///   **member declaration order** (the runtime `nearest` path relies on that
///   order for ties).
///
/// Search window is the **whole frame** (2026-11-06 seq127 #1): the editor
/// candidate API and the `op_region` chain picker enumerate every occurrence of
/// the template, so a saved `roi` never hides additional matches (the plain
/// click best path keeps its ROI + pyramid fallback via
/// [`locate_template`]). The full-frame search is coarse-to-fine (4x pyramid +
/// full-resolution refine) so it stays fast on 1280x720.
///
/// Members without usable operation geometry are skipped, exactly like the
/// runtime candidate path. Use [`sort_asset_match_candidates`] for the editor's
/// stable display order.
pub fn asset_match_candidates(
    assets: &AssetSet,
    matcher: &Matcher,
    id: &str,
) -> Result<Vec<AssetMatchCandidate>, String> {
    let mut templates: Vec<(String, &Template)> = Vec::new();
    if assets.collection_members(id).is_some() {
        let members = assets
            .collection_members(id)
            .map(|m| m.to_vec())
            .unwrap_or_default();
        if members.is_empty() {
            return Err(format!("operation asset `{id}` is an empty collection"));
        }
        for m in members {
            if let Some(t) = assets.get(&m) {
                templates.push((m, t));
            }
        }
    } else if let Some(t) = assets.get(id) {
        templates.push((id.to_string(), t));
    }
    if templates.is_empty() {
        return Err(format!(
            "operation asset `{id}` is not a template or template collection"
        ));
    }
    let threshold = asset_threshold(assets, id);
    // The matcher owns the whole-frame gray image, so an inverted operation
    // shape can resolve to the full frame exactly like `click_box_for` does.
    let frame_size = Some((
        matcher.image().width,
        matcher.image().height,
    ));
    let mut out: Vec<AssetMatchCandidate> = Vec::new();
    for (cid, tpl) in templates {
        // 2026-11-06 seq127 #1: the editor candidate API / chain picker enumerate
        // **every occurrence in the whole frame** for templates and collections;
        // the asset `roi` is only a hint for the historical single-best click
        // path (`locate_template`), never a limit here.
        let found = match template_polygon_mask(tpl) {
            Some(mask) => matcher.match_template_masked_all_full(&tpl.gray, &mask, threshold),
            None => matcher.match_template_all_full(&tpl.gray, threshold),
        };
        for cand in found {
            let pos = Point {
                x: cand.x,
                y: cand.y,
            };
            let Ok((action_box, action_shape)) =
                template_operation_geometry(assets, &cid, pos, frame_size)
            else {
                continue; // a member without usable geometry is not a candidate
            };
            out.push(AssetMatchCandidate {
                member: cid.clone(),
                member_kind: assets
                    .ocr_var(&cid)
                    .map(|_| "ocr_var")
                    .or_else(|| assets.ocr_roi(&cid).map(|_| "ocr_line"))
                    .or_else(|| assets.derived_ocr(&cid).map(|_| "derived_ocr"))
                    .unwrap_or("template")
                    .to_string(),
                score: cand.score,
                x: cand.x,
                y: cand.y,
                w: tpl.gray.width as i32,
                h: tpl.gray.height as i32,
                centre: Point {
                    x: cand.x + tpl.gray.width as i32 / 2,
                    y: cand.y + tpl.gray.height as i32 / 2,
                },
                action_box,
                action_shape,
                text: None,
                domain: CandidateDomain::Screen,
                pass: cand.score >= asset_threshold(assets, &cid),
            });
        }
    }
    Ok(out)
}

/// Stable editor order for [`AssetMatchCandidate`]s: score descending, then
/// `y` ascending, then `x` ascending (the same deterministic order
/// [`oas_perceive`]'s `*_all` matchers use inside one template).
///
/// The sort is **stable** on purpose: candidates that tie on all three keys keep
/// the collection member declaration order from [`asset_match_candidates`].
pub fn sort_asset_match_candidates(candidates: &mut [AssetMatchCandidate]) {
    candidates.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.y.cmp(&b.y))
            .then(a.x.cmp(&b.x))
    });
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

/// Streaming observer for editor run logs: called for every `StepEvent`.
pub trait StepObserver {
    fn step(&mut self, event: &StepEvent);
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

/// Single owner of every screenshot-derived value used by one interpreter.
///
/// ALL screen reads go through this cache so we never mix pixels from different
/// moments inside one logical observation (e.g. two `appear()` calls in a
/// condition) and never evaluate a selector against a frame captured before a
/// click/wait/swipe/drag.
///
/// Lifecycle:
/// - `refresh(src)`: grab a fresh frame and gray matcher.
/// - `ensure(src)`: grab only when no observation is cached (used inside one
///   evaluation, so multiple selectors share the same pixels).
/// - `invalidate()`: drop it after any input that may change the screen.
pub struct ScreenCache {
    frame: Option<Frame>,
    matcher: Option<Matcher>,
    size: Option<(u32, u32)>,
    /// Monotonic grab counter (diagnostics / stale-frame assertions).
    seq: u64,
}

impl ScreenCache {
    fn new() -> Self {
        Self {
            frame: None,
            matcher: None,
            size: None,
            seq: 0,
        }
    }

    /// Grab a fresh frame, replacing any previous observation.
    fn refresh<S: ScreenSource>(&mut self, src: &mut S) -> Result<(), String> {
        let frame: Frame = src.grab()?;
        self.seq = self.seq.saturating_add(1);
        self.size = Some((frame.width, frame.height));
        self.matcher = Some(Matcher::new(GrayImage::from_frame(&frame)));
        self.frame = Some(frame);
        Ok(())
    }

    /// Use the cached observation if present; grab only when missing.
    fn ensure<S: ScreenSource>(&mut self, src: &mut S) -> Result<(), String> {
        if self.frame.is_none() || self.matcher.is_none() {
            self.refresh(src)?;
        }
        Ok(())
    }

    /// Drop pixels after any action that may change the screen.
    fn invalidate(&mut self) {
        self.matcher = None;
        self.frame = None;
    }

    fn size(&self) -> Option<(u32, u32)> {
        self.size
    }

    fn seq(&self) -> u64 {
        self.seq
    }
}

/// One hop of an asset-level `op_region` chain, already recognized (private:
/// [`Interpreter::resolve_asset_op_region`] is the public entry point).
///
/// `box_`/`shape` are the **self-mode** operation geometry of this node (what the
/// historical `click_box_for` path would click for it); `real` is its real
/// recognized rect (None for a fixed `region` terminal, which needs none).
struct ChainNodeHit {
    asset: String,
    kind: &'static str,
    real: Option<Rect>,
    box_: Rect,
    shape: Option<ClickShape>,
    score: Option<f32>,
    text: Option<String>,
}

/// seq134 best-match result (single source for every interaction path).
#[derive(Debug, Clone, PartialEq)]
pub struct BestMatch {
    pub member: String,
    pub member_kind: String,
    pub score: f32,
    /// Position used by the historical click path: matched template top-left for
    /// templates, the ROI centre for a region, and a `(0,0)` marker for OCR
    /// (OCR geometry follows [`Interpreter::last_ocr_rect`]).
    pub at: Option<Point>,
    pub real: Option<Rect>,
    pub domain: CandidateDomain,
    pub text: Option<String>,
}

pub struct Interpreter<S: ScreenSource, I: InputSink> {
    src: S,
    input: I,
    assets: AssetSet,
    budget: Budget,
    /// Cooperative stop for editor-triggered runs (`POST /task/run/stop`).
    cancel: Option<Arc<AtomicBool>>,
    /// All screenshot-derived state; only this cache may own pixels/matcher.
    screen: ScreenCache,
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
    /// Editor-run diagnostic: log each `appear(sel)` result. Off for tests/CLI
    /// unless explicitly enabled, so it does not flood normal logs.
    trace_appear: bool,
    events: Vec<StepEvent>,
    /// Optional streaming sink: the editor shows step logs while a run is
    /// still inside a long `while` (before `run()` returns).
    observer: Option<Box<dyn StepObserver>>,
    /// CD2 (§21) click audit: one row per SUCCESSFUL click, cleared per run.
    clicks: Vec<ClickRecord>,
    /// Seed resolved once at construction (HB5); `with_seed` pins it.
    run_seed: u64,
    /// Injected task parameters for `param('key')` (M0 unified template):
    /// `meta.params[].default` merged with `oasd run --param k=v` overrides.
    /// `with_params` scopes temporarily replace entries and restore them after.
    params: BTreeMap<String, Value>,
    /// Active subtask labels (innermost last), used only to make a missing
    /// `param()` error name the subtask that needs it (Runner Studio §C.4.4).
    subtask_stack: Vec<String>,
    /// v1.2 runtime variables (`set_var`/`inc_var`/`ocr_to_var`), scoped to one
    /// flow run. Not persisted in checkpoints (intentionally simple).
    vars: BTreeMap<String, Value>,
    /// Matched line rect from the last OCR read (ordinary `ocr_line`/`ocr_lines`,
    /// derived and `ocr_var` all set it to the recognized line bbox, seq134 #2).
    last_ocr_rect: Option<Rect>,
    /// seq134: per-frame det+rec line cache. Key `(asset id, full-frame?)`; the
    /// same frame + same OCR asset runs det+rec exactly once, shared by collection
    /// members, `test_action`, chain nodes and interaction paths.
    ocr_lines_cache: HashMap<(String, bool), Vec<OcrLine>>,
    /// v1.2.3: for each collection selector, which member matched last.
    last_set_member: HashMap<String, String>,
    started: Instant,
    humanize: crate::humanize::Humanize,
    rng: crate::humanize::Rng,
    steps: u32,
    /// Op name of the step currently executing (set by [`Self::exec_one`]).
    /// Used purely as error context when a `text_expr` cannot be resolved
    /// (参数 v2): the message must name the op and the parameter key. Empty
    /// before the first step.
    current_op: &'static str,
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
            cancel: None,
            screen: ScreenCache::new(),
            ocr: None,
            coords: None,
            interrupts: Vec::new(),
            progress: None,
            start_step: 0,
            events: Vec::new(),
            observer: None,
            trace_appear: false,
            clicks: Vec::new(),
            run_seed,
            params: BTreeMap::new(),
            subtask_stack: Vec::new(),
            vars: BTreeMap::new(),
            last_ocr_rect: None,
            ocr_lines_cache: HashMap::new(),
            last_set_member: HashMap::new(),
            started: Instant::now(),
            humanize,
            rng: crate::humanize::Rng::new(run_seed),
            steps: 0,
            current_op: "",
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
        self.screen.frame.clone()
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

    /// Cooperative cancellation: checked before every step (and each loop
    /// iteration via `check_budget`).
    pub fn with_cancel(mut self, cancel: Arc<AtomicBool>) -> Self {
        self.cancel = Some(cancel);
        self
    }

    pub fn with_observer(mut self, observer: Box<dyn StepObserver>) -> Self {
        self.observer = Some(observer);
        self
    }

    /// Enable per-`appear()` diagnostics in the step log (editor runs).
    pub fn with_trace_appear(mut self, trace: bool) -> Self {
        self.trace_appear = trace;
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
        if let Some(observer) = self.observer.as_mut() {
            let event = self.events[seq].clone();
            observer.step(&event);
        }
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

    /// v1.3: perform one **long press** at `point` for `ms`.
    ///
    /// Same clamp posture as [`Self::perform_click`] (a point outside the frame
    /// is clamped, not silently accepted), the same humanized pre-action delay,
    /// and the frame is invalidated afterwards because the screen may change.
    /// No click-audit row is recorded: the CD2 heatmap is about taps, and a hold
    /// is a different gesture.
    fn perform_long_press(
        &mut self,
        point: Point,
        ms: u32,
        w: u32,
        h: u32,
    ) -> Result<Point, String> {
        let actual = clamp_to_frame(point, w, h);
        let delay = self.humanize.delay_ms(&mut self.rng);
        if delay > 0 {
            std::thread::sleep(std::time::Duration::from_millis(delay as u64));
        }
        self.input.long_press(actual, ms)?;
        // The screen may now be different: never let the next selector match
        // against this pre-gesture frame.
        self.invalidate_frame();
        Ok(actual)
    }

    /// ENGINE §22.11 补充层: poll the screen after a click until `expect` holds.
    ///
    /// Each poll invalidates the cached frame and grabs a fresh one (a frame-gated
    /// replay therefore consumes one frame per poll — exactly how waiting is
    /// modelled offline). Timeout = loud step failure: "clicked but nothing
    /// changed" must never count as success.
    ///
    /// 参数 v2 (Q8=A): a `text_expr` OCR expectation is resolved **once here**,
    /// before polling starts; every poll reuses that string.
    fn verify_expect(&mut self, expect: &Expect) -> Result<(), String> {
        let resolved_text: Option<String> = match expect {
            Expect::OcrText { text: spec, .. } => {
                let site = format!("{} {}", self.current_op, expect.kind());
                Some(self.resolve_text(spec, &site)?)
            }
            Expect::Appear { .. } | Expect::Vanish { .. } => None,
        };
        let timeout = std::time::Duration::from_millis(expect.timeout_ms() as u64);
        let start = std::time::Instant::now();
        let mut last;
        loop {
            self.reobserve_screen()?;
            // `selector_threshold` is the **single presence policy** here:
            // * OCR: explicit minimum confidence, else 0 — an explicit `0` is
            //   byte-for-byte equivalent to "unset" and must never inherit the
            //   0.9 template fallback (P1-4);
            // * template/collection: declared threshold, else the historical 0.9.
            last = match expect {
                Expect::Appear { sel, .. } => {
                    let (score, at) = self.locate(sel)?;
                    let th = self.selector_threshold(sel);
                    if at.is_some() && score >= th {
                        return Ok(());
                    }
                    format!("selector `{sel}` best score {score:.3} < {th}")
                }
                Expect::Vanish { sel, .. } => {
                    let (score, at) = self.locate(sel)?;
                    let th = self.selector_threshold(sel);
                    if !(at.is_some() && score >= th) {
                        return Ok(());
                    }
                    format!("selector `{sel}` still present (score {score:.3})")
                }
                Expect::OcrText { sel, .. } => {
                    let Some(text) = resolved_text.as_deref() else {
                        // Only the OcrText arm sets `resolved_text`; unreachable.
                        return Err(format!(
                            "internal: {} has no resolved text_expr",
                            expect.kind()
                        ));
                    };
                    let read = self.ocr_read(sel)?;
                    // An explicitly declared OCR threshold also gates text
                    // verification: a recognized-but-low-confidence line is not
                    // evidence that the expected text is really on screen.
                    let min = self.assets.ocr_threshold(sel);
                    let ok = match &read {
                        OcrRead::Lines(lines) => lines.iter().any(|l| {
                            ocr_text_matches(text, l.text.as_str())
                                && min.map(|m| l.score >= m).unwrap_or(true)
                        }),
                        OcrRead::Single(Some(t), score) => {
                            ocr_text_matches(text, t)
                                && min.map(|m| *score >= m).unwrap_or(true)
                        }
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
        self.screen.refresh(&mut self.src)
    }

    /// Ensure there is a frame, but reuse the current observation if present.
    /// Used *inside* one logical read (e.g. two selectors in one condition).
    fn observe_screen(&mut self) -> Result<(), String> {
        self.screen.ensure(&mut self.src)
    }

    /// Start a NEW observation: discard pixels from any previous action and
    /// grab the live screen. This is the only API steps should use when they
    /// decide something about the current screen (conditions, clicks, OCR).
    fn reobserve_screen(&mut self) -> Result<(), String> {
        // seq134: a new observation invalidates cached OCR lines too.
        self.ocr_lines_cache.clear();
        self.screen.refresh(&mut self.src)
    }

    /// Discard the cached frame. Called after every successful click/swipe/drag:
    /// the screen may have changed, so the next selector must not match against
    /// the pre-action pixels.
    fn invalidate_frame(&mut self) {
        self.ocr_lines_cache.clear();
        self.screen.invalidate();
    }

    fn check_budget(&self) -> Result<(), String> {
        if let Some(cancel) = &self.cancel {
            if cancel.load(Ordering::Relaxed) {
                return Err("cancelled by user (POST /task/run/stop)".to_string());
            }
        }
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
        self.observe_screen()?;
        let Some(tpl) = self.assets.get(sel) else {
            return Err(format!(
                "require_template `{sel}` is not declared in the task's assets"
            ));
        };
        let th = tpl.threshold;
        let (fw, fh) = self
            .screen.size()
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
            .screen.matcher
            .as_ref()
            .ok_or_else(|| "require_template: no frame available".to_string())?;
        Ok(matcher
            .match_template(&tpl.gray, Some(Rect { x: x0, y: y0, w, h }))
            .map(|b| b.score >= th)
            .unwrap_or(false))
    }

    fn locate(&mut self, sel: &str) -> Result<(f32, Option<Point>), String> {
        let best = self.recognize_best(sel)?;
        Ok((best.score, best.at))
    }

    // ------------------------------------------------------------------
    // seq134: ONE recognition engine (candidates → pick → best)
    // ------------------------------------------------------------------

    fn ocr_asset_kind(&self, id: &str) -> &'static str {
        // Declared kind first (the editor's `member_kind` must match assets.json):
        // a plain ocr_line with saved `ocr_text` also has a derived OCR entry, so
        // `derived_ocr` must NOT shadow `ocr_roi` here.
        if self.assets.ocr_var(id).is_some() {
            "ocr_var"
        } else if self.assets.ocr_roi(id).is_some() {
            if self.assets.ocr_is_multiline(id) {
                "ocr_lines"
            } else {
                "ocr_line"
            }
        } else if self.assets.derived_ocr(id).is_some() {
            "derived_ocr"
        } else if self.assets.region(id).is_some() {
            "region"
        } else if self.assets.get(id).is_some() {
            "template"
        } else {
            "unknown"
        }
    }

    /// Expected text of one OCR asset (single rule, seq134):
    /// `ocr_var` = params override > runtime_text; derived = saved line text;
    /// plain = the saved `ocr_text` (None = the "no expected text" exception).
    fn ocr_asset_expected(&self, id: &str) -> Result<Option<String>, String> {
        let expected = if self.assets.ocr_var(id).is_some() {
            Some(self.assets.ocr_var_expected_text(id, &self.params)?)
        } else if let Some((_, expected)) = self.assets.derived_ocr(id) {
            Some(expected)
        } else {
            self.assets.ocr_expected_text(id).map(str::to_string)
        };
        Ok(expected.filter(|e| !e.trim().is_empty()))
    }

    fn ocr_asset_declared_roi(&self, id: &str) -> Option<Rect> {
        // Derived OCR uses its SAVED SOURCE ROI for det+rec (v1.2.3: exactly one
        // recognition ROI); the asset's own narrow line ROI is only click geometry.
        self.assets
            .ocr_var(id)
            .and_then(|v| v.roi)
            .or_else(|| self.assets.derived_ocr(id).map(|(r, _)| r))
            .or_else(|| self.assets.ocr_roi(id))
    }

    /// Det+rec lines of one OCR asset with the seq134 domain/mask rules:
    /// * `full_frame == false` → declared ROI (whole frame for `ocr_var` without
    ///   ROI) **with** the asset polygon mask;
    /// * `full_frame == true` → whole frame, **never** the ROI polygon.
    fn ocr_lines_for(&mut self, id: &str, full_frame: bool) -> Result<Vec<OcrLine>, String> {
        self.observe_screen()?;
        let key = (id.to_string(), full_frame);
        if let Some(lines) = self.ocr_lines_cache.get(&key) {
            return Ok(lines.clone());
        }
        let frame = self
            .screen
            .frame
            .clone()
            .ok_or_else(|| format!("OCR `{id}`: no frame available"))?;
        let frame_rect = Rect {
            x: 0,
            y: 0,
            w: frame.width as i32,
            h: frame.height as i32,
        };
        let declared = self.ocr_asset_declared_roi(id);
        let roi = if full_frame {
            frame_rect
        } else {
            declared.unwrap_or(frame_rect)
        };
        let polygon = if full_frame {
            None
        } else {
            self.assets.ocr_polygon(id).map(|p| p.to_vec())
        };
        let masked;
        let frame_ref: &Frame = match polygon.as_ref() {
            Some(poly) => {
                masked = mask_frame_for_ocr(&frame, roi, poly);
                &masked
            }
            None => &frame,
        };
        if self.ocr.is_none() {
            return Err(format!(
                "OCR is not available for this run: no OCR provider attached \
                 (build with `--features ocr` and pass `--model <rec.onnx>`) - \
                 refusing to fake `ocr({id})`"
            ));
        }
        let provider = self.ocr.as_mut().expect("provider checked above");
        let lines = provider.recognize_lines(frame_ref, roi)?;
        self.ocr_lines_cache.insert(key, lines.clone());
        Ok(lines)
    }

    fn ocr_candidate_from_line(
        &self,
        id: &str,
        line: &OcrLine,
        domain: CandidateDomain,
        frame_size: Option<(u32, u32)>,
    ) -> Result<AssetMatchCandidate, String> {
        let (action_box, action_shape) =
            self_operation_geometry(&self.assets, id, line.rect, frame_size)?;
        Ok(AssetMatchCandidate {
            member: id.to_string(),
            member_kind: self.ocr_asset_kind(id).to_string(),
            score: line.score,
            x: line.rect.x,
            y: line.rect.y,
            w: line.rect.w,
            h: line.rect.h,
            centre: rect_centre(line.rect),
            action_box,
            action_shape,
            text: Some(line.text.clone()),
            domain,
            pass: line.score >= asset_threshold(&self.assets, id),
        })
    }

    /// seq134: strict expected-text candidates for a **flow locator** (`click_target`
    /// `text` / `drag ocr_text`): declared ROI first, then whole-frame fallback in
    /// the same frame (domain `screen_fallback`), exactly like [`Self::recognize_best`].
    fn ocr_target_candidates(
        &mut self,
        sel: &str,
        expected: &str,
    ) -> Result<Vec<AssetMatchCandidate>, String> {
        let declared = self.ocr_asset_declared_roi(sel);
        let threshold = self.assets.ocr_threshold(sel);
        let frame_size = self.screen.size();
        let roi_lines = self.ocr_lines_for(sel, false)?;
        let mut domain = if declared.is_some() {
            CandidateDomain::Roi
        } else {
            CandidateDomain::Screen
        };
        let mut matched: Vec<OcrLine> = matching_ocr_lines_strict(&roi_lines, expected, threshold)
            .into_iter()
            .cloned()
            .collect();
        if matched.is_empty() && declared.is_some() {
            let full = self.ocr_lines_for(sel, true)?;
            matched = matching_ocr_lines_strict(&full, expected, threshold)
                .into_iter()
                .cloned()
                .collect();
            domain = CandidateDomain::ScreenFallback;
        }
        let mut out = Vec::new();
        for line in &matched {
            out.push(self.ocr_candidate_from_line(sel, line, domain, frame_size)?);
        }
        sort_asset_match_candidates(&mut out);
        Ok(out)
    }

    fn ocr_recognition_candidates(&mut self, id: &str) -> Result<Vec<AssetMatchCandidate>, String> {
        let expected = self.ocr_asset_expected(id)?;
        let threshold = self.assets.ocr_threshold(id);
        let declared = self.ocr_asset_declared_roi(id);
        let frame_size = self.screen.size();
        let mut out = Vec::new();
        match expected.as_deref() {
            Some(expected) => {
                // seq134 #3: with expected text the candidates come from the WHOLE
                // FRAME (strict normalized equality); ROI hits are annotated.
                let lines = self.ocr_lines_for(id, true)?;
                for line in matching_ocr_lines_strict(&lines, expected, threshold) {
                    let domain = match declared {
                        None => CandidateDomain::Screen,
                        Some(roi) if rect_inside_rect(roi, line.rect) => CandidateDomain::Roi,
                        Some(_) => CandidateDomain::ScreenFallback,
                    };
                    out.push(self.ocr_candidate_from_line(id, line, domain, frame_size)?);
                }
            }
            None => {
                // seq134 #4: plain OCR without expected text stays in its ROI.
                let lines = self.ocr_lines_for(id, false)?;
                let domain = if declared.is_some() {
                    CandidateDomain::Roi
                } else {
                    CandidateDomain::Screen
                };
                for line in lines
                    .iter()
                    .filter(|l| threshold.map(|m| l.score >= m).unwrap_or(true))
                {
                    out.push(self.ocr_candidate_from_line(id, line, domain, frame_size)?);
                }
            }
        }
        sort_asset_match_candidates(&mut out);
        Ok(out)
    }

    /// seq134: THE candidate entry point (preview/test_action, chains,
    /// `region.asset`, drag, operation candidates). Image/text/region/collection
    /// members all share this structure and [`sort_asset_match_candidates`].
    pub fn recognition_candidates(&mut self, id: &str) -> Result<Vec<AssetMatchCandidate>, String> {
        if let Some(real) = self.assets.region(id) {
            let frame_size = self.screen.size();
            let (action_box, action_shape) =
                self_operation_geometry(&self.assets, id, real, frame_size)?;
            return Ok(vec![AssetMatchCandidate {
                member: id.to_string(),
                member_kind: "region".to_string(),
                score: 1.0,
                x: real.x,
                y: real.y,
                w: real.w,
                h: real.h,
                centre: rect_centre(real),
                action_box,
                action_shape,
                text: None,
                domain: CandidateDomain::Region,
                pass: true,
            }]);
        }
        if let Some(members) = self.assets.collection_members(id).map(|m| m.to_vec()) {
            if members.is_empty() {
                return Err(format!("collection `{id}` has no members"));
            }
            let mut out = Vec::new();
            for m in members {
                out.extend(self.recognition_candidates(&m)?);
            }
            sort_asset_match_candidates(&mut out);
            return Ok(out);
        }
        if self.assets.get(id).is_some() {
            self.observe_screen()?;
            let matcher = self
                .screen
                .matcher
                .as_ref()
                .ok_or_else(|| format!("asset `{id}`: no frame available"))?;
            let mut out = asset_match_candidates(&self.assets, matcher, id)?;
            sort_asset_match_candidates(&mut out);
            return Ok(out);
        }
        if self.assets.is_ocr_asset(id) {
            return self.ocr_recognition_candidates(id);
        }
        Err(format!("asset `{id}` is not declared in the task's assets"))
    }

    /// seq134: THE best entry point. Template: ROI(≥0.95)→whole-frame pyramid.
    /// OCR with expected text: ROI strict → whole-frame strict fallback in the same
    /// frame; OCR without expected text: ROI only; region fixed; collection picks
    /// the best member. The OCR line bbox becomes `last_ocr_rect`.
    pub fn recognize_best(&mut self, sel: &str) -> Result<BestMatch, String> {
        if let Some(roi) = self.assets.region(sel) {
            return Ok(BestMatch {
                member: sel.to_string(),
                member_kind: "region".to_string(),
                score: 1.0,
                at: Some(rect_centre(roi)),
                real: Some(roi),
                domain: CandidateDomain::Region,
                text: None,
            });
        }
        if let Some(members) = self.assets.collection_members(sel).map(|m| m.to_vec()) {
            if members.is_empty() {
                return Err(format!("selector `{sel}` is an empty template collection"));
            }
            self.observe_screen()?;
            let mut best: Option<BestMatch> = None;
            let mut first_error: Option<String> = None;
            for m in members {
                match self.recognize_best(&m) {
                    Ok(hit) => {
                        if best.as_ref().map(|b| hit.score > b.score).unwrap_or(true) {
                            best = Some(hit);
                        }
                    }
                    Err(e) => {
                        if first_error.is_none() {
                            first_error = Some(e);
                        }
                    }
                }
            }
            let Some(best) = best else {
                return Err(first_error
                    .unwrap_or_else(|| format!("collection `{sel}` has no members")));
            };
            self.last_set_member
                .insert(sel.to_string(), best.member.clone());
            return Ok(best);
        }
        if self.assets.get(sel).is_some() {
            return self.recognize_template_best(sel);
        }
        if !self.assets.is_ocr_asset(sel) {
            return Err(format!("selector `{sel}` is not declared in the task's assets"));
        }
        self.recognize_ocr_best(sel)
    }

    fn recognize_template_best(&mut self, sel: &str) -> Result<BestMatch, String> {
        self.observe_screen()?;
        self.last_ocr_rect = None;
        let tpl = self
            .assets
            .get(sel)
            .ok_or_else(|| format!("selector `{sel}` is not declared in the task's assets"))?;
        let matcher = self
            .screen
            .matcher
            .as_ref()
            .ok_or_else(|| "no frame available".to_string())?;
        let (score, at) = locate_template(matcher, tpl);
        let real = at.map(|p| Rect {
            x: p.x,
            y: p.y,
            w: tpl.gray.width as i32,
            h: tpl.gray.height as i32,
        });
        Ok(BestMatch {
            member: sel.to_string(),
            member_kind: "template".to_string(),
            score,
            at,
            real,
            domain: CandidateDomain::Screen,
            text: None,
        })
    }

    fn recognize_ocr_best(&mut self, sel: &str) -> Result<BestMatch, String> {
        let expected = self.ocr_asset_expected(sel)?;
        let threshold = self.assets.ocr_threshold(sel);
        let declared = self.ocr_asset_declared_roi(sel);
        if let Some(expected) = expected.as_deref() {
            let roi_lines = self.ocr_lines_for(sel, false)?;
            let mut first_roi_lines = roi_lines.clone();
            let mut lines: Vec<(OcrLine, CandidateDomain)> =
                matching_ocr_lines_strict(&roi_lines, expected, threshold)
                    .into_iter()
                    .cloned()
                    .map(|line| {
                        let domain = match declared {
                            None => CandidateDomain::Screen,
                            Some(_) => CandidateDomain::Roi,
                        };
                        (line, domain)
                    })
                    .collect();
            if lines.is_empty() && declared.is_some() {
                let full = self.ocr_lines_for(sel, true)?;
                first_roi_lines.extend(full.iter().cloned());
                lines = matching_ocr_lines_strict(&full, expected, threshold)
                    .into_iter()
                    .cloned()
                    .map(|line| (line, CandidateDomain::ScreenFallback))
                    .collect();
            }
            if lines.is_empty() {
                self.last_ocr_rect = None;
                let found = first_roi_lines
                    .iter()
                    .map(|l| format!("{:?}({:.3})", l.text, l.score))
                    .collect::<Vec<_>>()
                    .join(", ");
                let label = if self.assets.derived_ocr(sel).is_some() {
                    "derived ocr_line"
                } else if self.assets.ocr_var(sel).is_some() {
                    "ocr_var"
                } else {
                    "OCR"
                };
                return Err(format!(
                    "{label} `{sel}`: no line equals {expected:?} in recognition ROI \
                     {declared:?} or the whole frame (strict match, threshold {threshold:?}; \
                     lines found: {found})"
                ));
            }
            let idx = lines
                .iter()
                .enumerate()
                .max_by(|(_, (a, _)), (_, (b, _))| {
                    a.score
                        .partial_cmp(&b.score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(i, _)| i)
                .unwrap_or(0);
            let (line, domain) = lines.swap_remove(idx);
            self.last_ocr_rect = Some(line.rect);
            return Ok(BestMatch {
                member: sel.to_string(),
                member_kind: self.ocr_asset_kind(sel).to_string(),
                score: line.score,
                at: Some(Point { x: 0, y: 0 }),
                real: Some(line.rect),
                domain,
                text: Some(line.text),
            });
        }
        // No expected text: ROI-only (seq134 #4). A single-line `ocr_line` keeps
        // the historical rec-only read (no line bbox); `ocr_lines` uses det+rec
        // and the highest-score line becomes the real recognized row.
        if !self.assets.ocr_is_multiline(sel) {
            self.observe_screen()?;
            self.last_ocr_rect = None;
            let frame = self
                .screen
                .frame
                .clone()
                .ok_or_else(|| format!("OCR `{sel}`: no frame available"))?;
            let frame_rect = Rect {
                x: 0,
                y: 0,
                w: frame.width as i32,
                h: frame.height as i32,
            };
            let roi = declared.unwrap_or(frame_rect);
            let polygon = self.assets.ocr_polygon(sel).map(|p| p.to_vec());
            let masked;
            let frame_ref: &Frame = match polygon.as_ref() {
                Some(poly) => {
                    masked = mask_frame_for_ocr(&frame, roi, poly);
                    &masked
                }
                None => &frame,
            };
            let provider = self.ocr.as_mut().ok_or_else(|| {
                format!(
                    "OCR is not available for this run: no OCR provider attached \
                     (build with `--features ocr` and pass `--model <rec.onnx>`) - \
                     refusing to fake `ocr({sel})`"
                )
            })?;
            let (text, score) = provider.recognize(frame_ref, roi)?;
            return Ok(BestMatch {
                member: sel.to_string(),
                member_kind: self.ocr_asset_kind(sel).to_string(),
                score,
                at: if text.is_empty() {
                    None
                } else {
                    Some(Point { x: 0, y: 0 })
                },
                real: None,
                domain: if declared.is_some() {
                    CandidateDomain::Roi
                } else {
                    CandidateDomain::Screen
                },
                text: if text.is_empty() { None } else { Some(text) },
            });
        }
        let lines = self.ocr_lines_for(sel, false)?;
        let best = lines.iter().max_by(|a, b| {
            a.score
                .partial_cmp(&b.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        match best {
            Some(line) => {
                self.last_ocr_rect = Some(line.rect);
                Ok(BestMatch {
                    member: sel.to_string(),
                    member_kind: self.ocr_asset_kind(sel).to_string(),
                    score: line.score,
                    at: Some(Point { x: 0, y: 0 }),
                    real: Some(line.rect),
                    domain: if declared.is_some() {
                        CandidateDomain::Roi
                    } else {
                        CandidateDomain::Screen
                    },
                    text: Some(line.text.clone()),
                })
            }
            None => {
                self.last_ocr_rect = None;
                Ok(BestMatch {
                    member: sel.to_string(),
                    member_kind: self.ocr_asset_kind(sel).to_string(),
                    score: 0.0,
                    at: None,
                    real: None,
                    domain: CandidateDomain::Roi,
                    text: None,
                })
            }
        }
    }

    /// Return the operation shape in *screen* coordinates. Template shapes are
    /// authored relative to the template ROI, so they must be translated by the
    /// current match offset before validation/sampling.
    fn translated_click_shape(&self, sel: &str, matched: Point) -> Option<ClickShape> {
        if self.assets.collection_members(sel).is_some() {
            // A collection-level shape is an absolute fallback; the winning
            // member's own shape wins and is translated by its match offset.
            if let Some(member) = self.last_set_member.get(sel) {
                if let Some(shape) = self.assets.click_shape(member) {
                    return Some(self.translate_shape(shape.clone(), member, matched));
                }
            }
            return self.assets.click_shape(sel).cloned();
        }
        let shape = self.assets.click_shape(sel)?.clone();
        Some(self.translate_shape(shape, sel, matched))
    }

    /// Translate a template's operation shape by `matched - roi` (OCR/region
    /// shapes are absolute and therefore never translated). Delegates to the
    /// public [`AssetSet::translate_click_shape`] so the editor test and the
    /// runtime click path share one implementation.
    fn translate_shape(&self, shape: ClickShape, sel: &str, matched: Point) -> ClickShape {
        self.assets.translate_click_shape(shape, sel, matched)
    }

    /// Re-sample a click plan if it landed outside an exact operation shape.
    ///
    /// **Loud failure** (B-03): when the shape's effective region is empty or
    /// degenerate, sampling returns `Err` and the step fails instead of silently
    /// clicking the bbox centre / the excluded area.
    fn adjust_plan_for_shape(
        &mut self,
        sel: &str,
        plan: crate::humanize::ClickPlan,
        w: u32,
        h: u32,
        matched: Point,
    ) -> Result<crate::humanize::ClickPlan, String> {
        let Some(shape) = self.translated_click_shape(sel, matched) else {
            return Ok(plan);
        };
        let in_frame = plan.point.x >= 0
            && plan.point.y >= 0
            && plan.point.x < w as i32
            && plan.point.y < h as i32;
        if in_frame && shape.effective_contains(plan.point) {
            return Ok(plan);
        }
        let point = shape.sample(w, h, &mut self.rng)?;
        Ok(crate::humanize::ClickPlan {
            point,
            miss: false,
        })
    }

    /// v1.4: re-sample a click plan against an **already translated** operation
    /// shape (the `region.asset` A-plan path, where the shape comes from
    /// [`Self::operation_region_for_asset`] rather than the OCR asset's own
    /// `action_shape`).
    fn adjust_plan_for_explicit_shape(
        &mut self,
        plan: crate::humanize::ClickPlan,
        shape: Option<&ClickShape>,
        w: u32,
        h: u32,
    ) -> Result<crate::humanize::ClickPlan, String> {
        let Some(shape) = shape else {
            return Ok(plan);
        };
        let in_frame = plan.point.x >= 0
            && plan.point.y >= 0
            && plan.point.x < w as i32
            && plan.point.y < h as i32;
        if in_frame && shape.effective_contains(plan.point) {
            return Ok(plan);
        }
        let point = shape.sample(w, h, &mut self.rng)?;
        Ok(crate::humanize::ClickPlan {
            point,
            miss: false,
        })
    }

    /// True when `p` is inside the **effective** operation shape and on screen.
    /// Used by the region path, whose point-`click` jitter must never leave a
    /// narrow region (B-04).
    fn click_shape_accepts(&self, shape: &ClickShape, p: Point, w: u32, h: u32) -> bool {
        p.x >= 0
            && p.y >= 0
            && p.x < w as i32
            && p.y < h as i32
            && shape.effective_contains(p)
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
        // seq132: a region member's fixed ROI is its click box.
        if let Some(roi) = self.assets.region(effective_sel) {
            return Ok(roi);
        }
        // OCR assets have no template pixels; click their own self operation
        // area. 2026-11-06 批次B (#4/Q1): an authored `roi` makes the stored
        // `action_shape`/`click_back` follow the recognized line by
        // `real.topLeft - roi.topLeft`; without a recognized line yet (or with no
        // roi, e.g. whole-screen ocr_var) the authored coordinates stay absolute.
        if self.assets.is_ocr_asset(effective_sel) {
            let authored_roi = self
                .assets
                .ocr_var(effective_sel)
                .and_then(|v| v.roi)
                .or_else(|| self.assets.ocr_roi(effective_sel));
            if let Some(real) = self.last_ocr_rect.or(authored_roi) {
                let (rect, _shape) =
                    self.ocr_self_operation_geometry(effective_sel, real, self.screen.size())?;
                return Ok(rect);
            }
            if let Some(shape) = self.assets.click_shape(effective_sel) {
                if shape.invert {
                    let (w, h) = self.screen.size().ok_or_else(|| {
                        "inverted OCR operation region needs a screen size".to_string()
                    })?;
                    return Ok(Rect {
                        x: 0,
                        y: 0,
                        w: w as i32,
                        h: h as i32,
                    });
                }
                if let Some(b) = shape.bbox() {
                    return Ok(b);
                }
            }
            if let Some(back) = self.assets.click_back(effective_sel) {
                return Ok(back);
            }
            if let Some(r) = self.last_ocr_rect {
                return Ok(r);
            }
            return self
                .assets
                .ocr_roi(effective_sel)
                .ok_or_else(|| format!("OCR selector `{effective_sel}` has no roi"));
        }
        // Template: exact operation shape (translated) → explicit click area
        // (`roi_back`, translated) → matched bbox. Extracted so the editor
        // candidate API reports the identical geometry.
        let (rect, _shape) =
            template_operation_geometry(&self.assets, effective_sel, p, self.screen.size())?;
        Ok(rect)
    }

    /// seq132: resolve the winning member's **real rect + self geometry** for an
    /// `op_region` chain root (mixed collections included). Kept next to
    /// [`Self::click_geometry_for`] so both use the same per-kind dispatch.
    fn chain_geometry_for(
        &self,
        sel: &str,
        matched: Point,
    ) -> Result<(Rect, Option<ClickShape>, Rect), String> {
        let asset = self.effective_asset(sel);
        if let Some(roi) = self.assets.region(asset) {
            return Ok((roi, self.assets.click_shape(asset).cloned(), roi));
        }
        if self.assets.is_ocr_asset(asset) {
            let real = self
                .last_ocr_rect
                .or_else(|| self.assets.ocr_var(asset).and_then(|v| v.roi))
                .or_else(|| self.assets.ocr_roi(asset))
                .ok_or_else(|| {
                    format!("`{asset}`: op_region chain needs the real recognized rect")
                })?;
            let (box_, shape) =
                self.ocr_self_operation_geometry(asset, real, self.screen.size())?;
            return Ok((box_, shape, real));
        }
        let real = self
            .root_template_real_rect(asset, matched)
            .ok_or_else(|| format!("`{asset}`: op_region chain needs the real matched rect"))?;
        let (box_, shape) = self.click_geometry_for(asset, matched)?;
        Ok((box_, shape, real))
    }

    /// Click geometry (rect + exact operation shape) from **one source**.
    ///
    /// seq130 fix (#5): the sampled target rect and the shape that constrains the
    /// final click point must never disagree. For an OCR asset whose authored
    /// `roi` is set, the stored `action_shape` follows the recognized line by
    /// `real - roi` (Q1), so **both** must come from
    /// [`Self::ocr_self_operation_geometry`]; the older
    /// [`Self::translated_click_shape`] returned the authored absolute shape for
    /// OCR (it only translates templates), which snapped the click back to the
    /// authored area. Template/collection keep the historical
    /// `click_box_for` + translated shape; a full-screen ocr_var (no authored
    /// roi) keeps the absolute shape (Q1 documented limit).
    /// seq132: the asset that actually owns the geometry for `sel` — the winning
    /// member for a collection, otherwise `sel` itself.
    fn effective_asset<'a>(&'a self, sel: &'a str) -> &'a str {
        if self.assets.collection_members(sel).is_some() {
            self.last_set_member
                .get(sel)
                .map(|s| s.as_str())
                .unwrap_or(sel)
        } else {
            sel
        }
    }

    /// seq132: rect + score + member for a mixed collection candidate, used by the
    /// runtime candidate API/preview and (with the caller's OCR lines) by serve.
    /// The exact same ordering is used everywhere (`sort_asset_match_candidates`).
    pub fn collection_candidates(
        &mut self,
        id: &str,
        _strict_ocr_match: bool,
    ) -> Result<Vec<AssetMatchCandidate>, String> {
        // seq134: the runtime candidate list is the single-source enumeration.
        return self.recognition_candidates(id);
    }

    /// seq134: `region.asset` resolution through the single-source engine.
    fn operation_region_single_source(
        &mut self,
        asset_id: &str,
        region: &OpRegion,
        reference: Option<Point>,
        site: &str,
    ) -> Result<(Rect, Option<ClickShape>), String> {
        let err = |what: &str| format!("{site}: operation asset `{asset_id}` {what}");
        if !self.assets.region(asset_id).is_some()
            && self.assets.get(asset_id).is_none()
            && self.assets.collection_members(asset_id).is_none()
            && !self.assets.is_ocr_asset(asset_id)
        {
            return Err(err("is not declared in the task's assets"));
        }
        let candidates = self
            .recognition_candidates(asset_id)
            .map_err(|e| format!("{site}: {e}"))?;
        if candidates.is_empty() {
            return Err(err("has no match candidate >= its threshold"));
        }
        if matches!(region, OpRegion::Nearest { .. }) {
            let reference = reference.ok_or_else(|| {
                err("needs a reference point for mode=nearest (matched OCR line)")
            })?;
            let idx = Self::pick_candidate(&candidates, Some(reference))
                .ok_or_else(|| err("has no candidate"))?;
            let best = &candidates[idx];
            return Ok((best.action_box, best.action_shape.clone()));
        }
        // mode=asset: one best match through recognize_best (template ROI→full,
        // OCR ROI→whole-frame strict fallback, mixed collection best member).
        let best = self.recognize_best(asset_id)?;
        let Some(at) = best.at else {
            return Err(err("has no match"));
        };
        let score = best.score;
        let threshold = self.selector_threshold(asset_id);
        if score < threshold {
            return Err(err(&format!(
                "best match score {score:.3} < threshold {threshold:.3}"
            )));
        }
        if self.assets.is_ocr_asset(asset_id) {
            let real = best
                .real
                .ok_or_else(|| err("has no recognized OCR line rect"))?;
            let (box_, shape) = self.ocr_self_operation_geometry(asset_id, real, self.screen.size())?;
            return Ok((box_, shape));
        }
        let (rect, shape) = self.click_geometry_for(asset_id, at)?;
        Ok((rect, shape))
    }
    fn click_geometry_for(
        &self,
        sel: &str,
        matched: Point,
    ) -> Result<(Rect, Option<ClickShape>), String> {
        let effective_sel: &str = if self.assets.collection_members(sel).is_some() {
            self.last_set_member
                .get(sel)
                .map(|s| s.as_str())
                .ok_or_else(|| format!("collection `{sel}` has no matched member yet"))?
        } else {
            sel
        };
        // seq132: a region member / direct region asset is a fixed operation
        // area (absolute shape, no recognition).
        if let Some(roi) = self.assets.region(effective_sel) {
            return Ok((roi, self.assets.click_shape(effective_sel).cloned()));
        }
        if self.assets.is_ocr_asset(effective_sel) {
            let authored_roi = self
                .assets
                .ocr_var(effective_sel)
                .and_then(|v| v.roi)
                .or_else(|| self.assets.ocr_roi(effective_sel));
            // The normal click path sets `last_ocr_rect` from the recognized line;
            // then rect and shape are translated together (roi=None => absolute).
            if let Some(real) = self.last_ocr_rect.or(authored_roi) {
                return self.ocr_self_operation_geometry(
                    effective_sel,
                    real,
                    self.screen.size(),
                );
            }
            // No recognized rect yet: the same fallbacks `click_box_for` uses.
            let rect = self.click_box_for(sel, matched)?;
            let shape = self.assets.click_shape(effective_sel).cloned();
            return Ok((rect, shape));
        }
        let rect = self.click_box_for(sel, matched)?;
        let shape = self.translated_click_shape(sel, matched);
        Ok((rect, shape))
    }


    /// v1.4 A-plan: the operation region + exact shape of a **whole-library**
    /// operation-area asset (`region.asset`).
    ///
    /// * template / collection: `mode=asset` → one best match; `mode=nearest` →
    ///   the multi-candidate whose **match bbox centre** is nearest to
    ///   `reference` (the matched parameter-OCR line centre; ties pick the higher
    ///   score);
    /// * fixed region asset → its region (+ declared exact shape), no matching;
    /// * OCR asset → its declared operation area (`action_shape` / `click_back`
    ///   / `ocr_roi`), no candidates;
    /// * undeclared / no match / no reference → **loud error** prefixed with
    ///   `site` (never a silent fallback).
    fn operation_region_for_asset(
        &mut self,
        asset_id: &str,
        region: &OpRegion,
        reference: Option<Point>,
        site: &str,
    ) -> Result<(Rect, Option<ClickShape>), String> {
        if asset_id.trim().is_empty() {
            return Err(format!("{site}: `region.asset` is empty"));
        }
        // Fixed region asset: no matching needed.
        if let Some(rect) = self.assets.region(asset_id) {
            return Ok((rect, self.assets.click_shape(asset_id).cloned()));
        }
        // seq134: OCR targets are position candidates too (asset/nearest unified).
        return self.operation_region_single_source(asset_id, region, reference, site);
    }

    /// v1.4 A-plan: when `region` carries a whole-library `asset`, resolve the
    /// operation region/shape to use **instead of** the OCR-line geometry.
    ///
    /// `reference` is the matched parameter-OCR line bbox centre (used by
    /// `mode=nearest`). Returns `None` when the region has no `asset`, keeping
    /// the v1.3/historical behaviour untouched.
    fn region_asset_target(
        &mut self,
        region: &Option<OpRegion>,
        reference: Point,
        site: &str,
    ) -> Result<Option<(Rect, Option<ClickShape>)>, String> {
        let Some(region) = region.as_ref() else {
            return Ok(None);
        };
        let Some(asset_id) = region.asset() else {
            return Ok(None);
        };
        let asset_id = asset_id.to_string();
        let reference = match region {
            OpRegion::Nearest { .. } => Some(reference),
            _ => None,
        };
        Ok(Some(
            self.operation_region_for_asset(&asset_id, region, reference, site)?,
        ))
    }

    /// One hop of an asset-level `op_region` chain, already recognized.
    ///
    /// `box_`/`shape` are the **self-mode** operation geometry of this node (what
    /// Resolve the asset-level `op_region` chain of `target` when it has one
    /// (2026-11-06 参数→素材 redesign D2/B16).
    ///
    /// Returns `None` when the target declares no chain (absent/`self`): the
    /// Resolve the root asset's operation region through its `op_region` chain
    /// (**model B**, restored 2026-11-06 seq127 #2/#3).
    ///
    /// User semantics: *"when 引用素材 is selected, the operation area is the LAST
    /// referenced asset's own operation area; only when 当前素材操作区(self) is
    /// selected is it the asset's own area."* So:
    /// * root `op_region=self` (or no `op_region`) → the chain-less historical
    ///   path: this function returns `Ok(None)` and the caller keeps its old
    ///   geometry byte-identical;
    /// * root `op_region=asset` → recognize the referenced gate (all-frame
    ///   multi-candidate + `pick`: score / nearest-to-previous-real-centre),
    ///   follow that gate's own `op_region`, and stop at the first node whose
    ///   `op_region` is absent/self: **that terminal node's own self geometry**
    ///   (`ChainNodeHit.box_`/`shape`, i.e. translated `action_shape` /
    ///   `click_back` / real matched bbox) is the final operation box.
    ///
    /// Preserved from the model-A round: `offset` is gone, `region` cannot be a
    /// chain node, any gate miss/undeclared is a loud error (no click, no
    /// fallback), cycles and depth cap apply.
    ///
    /// The caller has already recognized the target, so its real rect and its
    /// self-mode box/shape are passed in; they are the fallback/terminal geometry
    /// when the chain ends at the root itself.
    pub fn resolve_asset_op_region(
        &mut self,
        target: &str,
        root_real: Rect,
        root_self_box: Rect,
        root_self_shape: Option<ClickShape>,
        site: &str,
    ) -> Result<Option<ResolvedOpRegion>, String> {
        if !self.assets.has_op_region_chain(target) {
            return Ok(None);
        }
        self.observe_screen()?;
        let kind = self.asset_kind_str(target);
        let mut visited: BTreeSet<String> = BTreeSet::new();
        visited.insert(target.to_string());
        let mut steps: Vec<OpRegionChainStep> = Vec::new();
        let root = ChainNodeHit {
            asset: target.to_string(),
            kind,
            real: Some(root_real),
            box_: root_self_box,
            shape: root_self_shape,
            score: None,
            text: None,
        };
        // Model B: the walk terminates at the last referenced node's self geometry.
        self.walk_op_region_chain(root, 1, 0, 0, &mut visited, &mut steps, site)
            .map(Some)
    }

    /// Classification used by the chain log (`template`/`collection`/`ocr`/
    /// `region`).
    fn asset_kind_str(&self, id: &str) -> &'static str {
        if self.assets.region(id).is_some() {
            "region"
        } else if self.assets.collection_members(id).is_some() {
            "collection"
        } else if self.assets.ocr_var(id).is_some()
            || self.assets.ocr_roi(id).is_some()
            || self.assets.derived_ocr(id).is_some()
        {
            "ocr"
        } else {
            "template"
        }
    }

    /// The **real** recognized rect of a template/collection match at `matched`
    /// (template top-left). Collections resolve through their winning member.
    fn root_template_real_rect(&self, sel: &str, matched: Point) -> Option<Rect> {
        let tpl_id = if self.assets.get(sel).is_some() {
            sel
        } else {
            self.last_set_member.get(sel).map(|s| s.as_str())?
        };
        let tpl = self.assets.get(tpl_id)?;
        Some(Rect {
            x: matched.x,
            y: matched.y,
            w: tpl.gray.width as i32,
            h: tpl.gray.height as i32,
        })
    }

    /// One-shot editor preview (B-T3b; model B restored in seq127 #2/#3):
    /// recognize the target once (or take the `candidate_index`-th candidate),
    /// walk its asset-level `op_region` chain, and return the **last referenced
    /// node's own operation box** + a concrete point inside it.
    ///
    /// Read-only: nothing is clicked or persisted. `POST /task/asset/test_action`
    /// uses this for both the dry preview and (through the returned point) the
    /// real tap. The geometry is the **same** chain resolver the runtime click
    /// paths use, so the editor can never disagree with the engine.
    pub fn preview_asset_action(
        &mut self,
        target: &str,
        candidate_index: Option<usize>,
    ) -> Result<PreviewAction, String> {
        self.reobserve_screen()?;
        let (w, h) = self
            .screen
            .size()
            .ok_or_else(|| "test_action: no frame available".to_string())?;
        let root = self.preview_root_candidate(target, candidate_index, "test_action")?;
        let root_real = root.real;
        let score = root.score;
        let root_box = root.box_;
        let root_shape = root.shape.clone();
        let mut steps: Vec<OpRegionChainStep> = Vec::new();
        // seq132: for a collection the winning member owns geometry + chain.
        let chain_owner = if self.assets.collection_members(target).is_some() {
            root.asset.clone()
        } else {
            target.to_string()
        };
        let (action_box, shape) =
            if self.assets.has_op_region_chain(&chain_owner) && root.kind != "region" {
                let mut visited: BTreeSet<String> = BTreeSet::new();
                visited.insert(chain_owner.clone());
                let resolved =
                    self.walk_op_region_chain(root, 1, 0, 0, &mut visited, &mut steps, "test_action")?;
                // Model B: the final box is the terminal node's own self geometry.
                steps = resolved.chain;
                (resolved.rect, resolved.shape)
            } else {
                steps.push(OpRegionChainStep {
                    asset: root.asset.clone(),
                    mode: root.kind,
                    real: root.real,
                    score: root.score,
                    text: root.text.clone(),
                    action_box: root_box,
                    candidate_count: 1,
                    candidate_index: 0,
                });
                (root_box, root_shape)
            };
        let point = match shape.as_ref() {
            Some(shape) => shape.sample(w, h, &mut self.rng)?,
            None => {
                let cx = action_box.x.saturating_add(action_box.w / 2);
                let cy = action_box.y.saturating_add(action_box.h / 2);
                Point {
                    x: cx.clamp(0, (w as i32).saturating_sub(1)),
                    y: cy.clamp(0, (h as i32).saturating_sub(1)),
                }
            }
        };
        Ok(PreviewAction {
            target: target.to_string(),
            action_box,
            shape,
            point,
            real: root_real,
            score,
            steps,
        })
    }

    /// Root recognition for [`Self::preview_asset_action`].
    ///
    /// `candidate_index` uses the **candidates API order** (stable: score desc →
    /// y asc → x asc) so the editor can test the exact candidate it displays;
    /// `None` = the best candidate. A `region` target has exactly one candidate.
/// seq134: THE candidate picker (single source for the op_region chain,
/// `region.asset nearest`, drag endpoints and preview selection).
///
/// * `pick_ref == None` → highest score, ties keep candidate order;
/// * `pick_ref == Some(reference)` → candidate centre nearest to `reference`
///   (squared distance), ties break on higher score, then candidate order.
    pub fn pick_candidate(candidates: &[AssetMatchCandidate], pick_ref: Option<Point>) -> Option<usize> {
    let mut best: Option<(usize, f32, i64)> = None;
    for (i, c) in candidates.iter().enumerate() {
        let dist = pick_ref
            .map(|r| {
                let dx = c.centre.x as i64 - r.x as i64;
                let dy = c.centre.y as i64 - r.y as i64;
                dx * dx + dy * dy
            })
            .unwrap_or(0);
        let better = match best {
            None => true,
            Some((_, best_score, best_dist)) => match pick_ref {
                None => c.score > best_score,
                Some(_) => dist < best_dist || (dist == best_dist && c.score > best_score),
            },
        };
        if better {
            best = Some((i, c.score, dist));
        }
    }
    best.map(|(i, _, _)| i)
}

/// seq134: editor/`test_action`/chain single-source candidate → chain hit.
/// Candidate order and `candidate_index` come from
/// [`Interpreter::recognition_candidates`], the same enumeration the editor API
/// consumes.
    fn candidate_to_chain_hit(
        target: &str,
        candidates: &[AssetMatchCandidate],
        idx: usize,
        site: &str,
    ) -> Result<ChainNodeHit, String> {
    let Some(c) = candidates.get(idx).cloned() else {
        return Err(format!(
            "{site}: candidate_index {idx} is out of range (asset `{target}` has {} candidates)",
            candidates.len()
        ));
    };
    let kind = match c.member_kind.as_str() {
        "region" => "region",
        "template" => "template",
        _ => "ocr",
    };
    Ok(ChainNodeHit {
        asset: c.member.clone(),
        kind,
        real: Some(Rect {
            x: c.x,
            y: c.y,
            w: c.w,
            h: c.h,
        }),
        box_: c.action_box,
        shape: c.action_shape,
        score: Some(c.score),
        text: c.text,
    })
}

    fn preview_root_candidate(
        &mut self,
        target: &str,
        candidate_index: Option<usize>,
        site: &str,
    ) -> Result<ChainNodeHit, String> {
        let candidates = self
            .recognition_candidates(target)
            .map_err(|e| format!("{site}: {e}"))?;
        if candidates.is_empty() {
            return Err(format!(
                "{site}: asset `{target}` has no candidate at/above its threshold"
            ));
        }
        let idx = candidate_index.unwrap_or(0);
        let mut hit = Self::candidate_to_chain_hit(target, &candidates, idx, site)?;
        if hit.kind == "region" {
            // A fixed region has no recognition rect (historical preview contract).
            hit.real = None;
        }
        Ok(hit)
    }

    /// One DFS hop of [`Self::resolve_asset_op_region`] (model B: the terminal
    /// `self` node's own geometry is the final operation box; every `asset` hop
    /// recognizes the next node and recurses).
    fn walk_op_region_chain(
        &mut self,
        node: ChainNodeHit,
        candidate_count: usize,
        candidate_index: usize,
        depth: usize,
        visited: &mut BTreeSet<String>,
        steps: &mut Vec<OpRegionChainStep>,
        site: &str,
    ) -> Result<ResolvedOpRegion, String> {
        if depth > MAX_OP_REGION_CHAIN_DEPTH {
            return Err(format!(
                "{site}: op_region chain exceeds the max depth \
                 {MAX_OP_REGION_CHAIN_DEPTH} at `{}`",
                node.asset
            ));
        }
        let op = self.assets.op_region(&node.asset).cloned();
        match op {
            None | Some(AssetOpRegion::SelfRegion) => {
                // Terminal gate: the node was recognized; the chain ends here.
                steps.push(OpRegionChainStep {
                    asset: node.asset,
                    mode: node.kind,
                    real: node.real,
                    score: node.score,
                    text: node.text,
                    action_box: node.box_,
                    candidate_count,
                    candidate_index,
                });
                Ok(ResolvedOpRegion {
                    rect: node.box_,
                    shape: node.shape,
                    chain: std::mem::take(steps),
                })
            }
            Some(AssetOpRegion::Asset { asset, pick }) => {
                steps.push(OpRegionChainStep {
                    asset: node.asset.clone(),
                    mode: "asset",
                    real: node.real,
                    score: node.score,
                    text: node.text,
                    action_box: node.box_,
                    candidate_count,
                    candidate_index,
                });
                if !visited.insert(asset.clone()) {
                    return Err(format!(
                        "{site}: op_region chain cycle detected at `{asset}` \
                         (visited: {})",
                        visited.iter().cloned().collect::<Vec<_>>().join(" -> ")
                    ));
                }
                let real = node.real.ok_or_else(|| {
                    format!(
                        "{site}: op_region chain hop from `{}` needs a real recognized rect",
                        node.asset
                    )
                })?;
                let pick_ref = match pick {
                    OpRegionPick::Nearest => Some(rect_centre(real)),
                    OpRegionPick::Score => None,
                };
                let (next, next_count, next_index) =
                    self.recognize_chain_node(&asset, pick_ref, site)?;
                self.walk_op_region_chain(
                    next,
                    next_count,
                    next_index,
                    depth + 1,
                    visited,
                    steps,
                    site,
                )
            }
        }
    }

    /// Recognize one chain node (beyond the root) and resolve its self-mode
    /// geometry. Model B: when this node terminates the chain (no `op_region` /
    /// `self`), its self geometry **is** the final operation box. `pick_ref` =
    /// the previous node's real-rect centre for `pick=nearest`.
    fn recognize_chain_node(
        &mut self,
        asset: &str,
        pick_ref: Option<Point>,
        site: &str,
    ) -> Result<(ChainNodeHit, usize, usize), String> {
        // seq134: fixed `region` nodes cannot be chain nodes (no recognition).
        if self.assets.region(asset).is_some() {
            return Err(format!(
                "{site}: region asset `{asset}` cannot be an op_region chain node \
                 (chain nodes must be recognizable; a region has no recognition)"
            ));
        }
        // seq134: a plain OCR chain node still needs an expected text (the asset
        // loader/validator requires `ocr_text`); without it the gate is loud.
        if self.assets.is_ocr_asset(asset)
            && !matches!(
                self.ocr_asset_expected(asset)?,
                Some(_)
            )
        {
            return Err(format!(
                "{site}: op_region chain node `{asset}` is an OCR asset without an \
                 expected text (set 识别文字/ocr_text on it or reference an ocr_var)"
            ));
        }
        // seq134: ONE candidate enumeration for template / OCR / ocr_var / 混合集合;
        // a collection node enumerates EVERY member (incl. OCR/region), not only
        // template members.
        let candidates = self
            .recognition_candidates(asset)
            .map_err(|e| format!("{site}: {e}"))?;
        if candidates.is_empty() {
            if self.assets.is_ocr_asset(asset) {
                return Err(format!(
                    "{site}: op_region chain node `{asset}`: no line matching its expected text \
                     (strict, all lines at/above the OCR threshold)"
                ));
            }
            return Err(format!(
                "{site}: op_region chain node `{asset}` has no match at/above its threshold"
            ));
        }
        let idx = Self::pick_candidate(&candidates, pick_ref).ok_or_else(|| {
            format!("{site}: op_region chain node `{asset}` has no candidate")
        })?;
        let chosen = &candidates[idx];
        let kind = if self.assets.collection_members(asset).is_some() {
            "collection"
        } else if self.assets.is_ocr_asset(asset) {
            "ocr"
        } else {
            "template"
        };
        let hit = ChainNodeHit {
            asset: chosen.member.clone(),
            kind,
            real: Some(Rect {
                x: chosen.x,
                y: chosen.y,
                w: chosen.w,
                h: chosen.h,
            }),
            box_: chosen.action_box,
            shape: chosen.action_shape.clone(),
            score: Some(chosen.score),
            text: chosen.text.clone(),
        };
        Ok((hit, candidates.len(), idx))
    }



    /// Shared OCR self-geometry (chain gates, editor preview, root click paths).
    fn ocr_self_operation_geometry(
        &self,
        sel: &str,
        real: Rect,
        frame_size: Option<(u32, u32)>,
    ) -> Result<(Rect, Option<ClickShape>), String> {
        // Single source of truth: the public model-A helper (2026-11-06 批次B),
        // also used by serve's candidate `action_box` so the editor and the
        // engine can never disagree.
        self_operation_geometry(&self.assets, sel, real, frame_size)
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

    /// Resolve the expected OCR text of one `text` / `text_expr` site (参数 v2,
    /// PARAM-V2-DESIGN §3.2.4).
    ///
    /// * a [`TextSpec::Literal`] is returned byte-for-byte (old flows unchanged);
    /// * a [`TextSpec::Expr`] is evaluated **once when the step starts** — never
    ///   per poll (Q8=A) — and must produce a **non-empty string**. `Null`,
    ///   empty, number and bool are **loud** errors: a missing/blank parameter
    ///   must never degrade into "match anything" or click the wrong target.
    ///
    /// `site` names the op + field (`click_target args.find[0].text_expr`) and a
    /// `param('key')` expression also names its key, so the failure is
    /// actionable without a debugger.
    fn resolve_text(&mut self, spec: &TextSpec, site: &str) -> Result<String, String> {
        match spec {
            TextSpec::Literal(s) => Ok(s.clone()),
            TextSpec::Expr(expr) => {
                let what = match spec.param_key() {
                    Some(key) => format!("text_expr param('{key}')"),
                    None => "text_expr".to_string(),
                };
                let value = eval(expr, self).map_err(|e| format!("{site}: {what}: {e}"))?;
                match value {
                    Value::Str(s) if !s.trim().is_empty() => Ok(s),
                    Value::Str(_) => {
                        Err(format!("{site}: {what} resolved to an empty string"))
                    }
                    other => Err(format!(
                        "{site}: {what} must be a string, got {other:?}"
                    )),
                }
            }
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
        self.observe_screen()?;
        let frame = self
            .screen.frame
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

    /// Resolve a `drag` endpoint to its bbox rectangle + optional exact shape.
    ///
    /// `region`/`point` need no assets; `ocr_text` reads the line's bbox from an
    /// OCR asset or an inline ROI. A single-line OCR target has no line bbox by
    /// design, so it is rejected with a clear message.
    ///
    /// v1.2.5 (O-2): only a `region` target can carry a `shape`; `point` and
    /// `ocr_text` keep `shape: None` and thus their historical behaviour.
    ///
    /// 参数 v2: an `ocr_text` endpoint may use `text_expr`; it is resolved
    /// **once here**, before any frame is read or dragged (failing to resolve
    /// aborts the step with no swipe). `site` names the endpoint in that error.
    /// seq129: resolve one `{"kind":"asset"}` drag endpoint by reusing the
    /// **click asset path**: recognize the asset (template / collection /
    /// OCR / ocr_var) or take a fixed `region` asset's ROI, resolve its real
    /// recognized rect, then its **operation area** exactly like `Step::Click`:
    ///
    /// * an asset-level `op_region` chain wins (model B: the terminal node's own
    ///   self geometry, via [`Self::resolve_asset_op_region`]);
    /// * otherwise `self_operation_geometry` (`action_shape`/`click_back`/
    ///   matched bbox; OCR assets follow the recognized line, Q1 rules).
    ///
    /// A recognition miss, an undeclared asset or unusable geometry is a loud
    /// error carrying the asset id — the drag step then sends **no** swipe.
    fn drag_asset_endpoint(&mut self, sel: &str, site: &str) -> Result<DragEndpoint, String> {
        // A pure `region` asset has no recognition: its declared ROI is the
        // endpoint area (an explicit shape, if authored, is already absolute).
        if let Some(roi) = self.assets.region(sel) {
            return Ok(DragEndpoint {
                rect: roi,
                shape: self.assets.click_shape(sel).cloned(),
            });
        }
        let (score, at) = self
            .locate(sel)
            .map_err(|e| format!("{site}: drag asset `{sel}`: {e}"))?;
        let threshold = self.selector_threshold(sel);
        let Some(p) = at.filter(|_| score >= threshold) else {
            return Err(format!(
                "{site}: drag asset `{sel}` not found (score {score:.3} < threshold {threshold:.3})"
            ));
        };
        // seq132: mixed collections dispatch per winning member kind; the
        // member's own op_region chain applies exactly like a click.
        let (mut rect, mut shape) = self.click_geometry_for(sel, p)?;
        let chain_sel = self.effective_asset(sel).to_string();
        if self.assets.has_op_region_chain(&chain_sel) {
            let (root_box, root_shape, real) = self.chain_geometry_for(&chain_sel, p)?;
            if let Some(resolved) = self.resolve_asset_op_region(
                &chain_sel,
                real,
                root_box,
                root_shape,
                site,
            )? {
                rect = resolved.rect;
                shape = resolved.shape;
            }
        }
        Ok(DragEndpoint { rect, shape })
    }

    /// Resolve one `drag` endpoint (`from` / `to`) to the rectangle + optional
    /// exact operation shape that [`Self::sample_drag_point`] samples.
    ///
    /// 参数 v2: an `ocr_text` endpoint may use `text_expr`; it is resolved
    /// **once here**, before any frame is read or dragged (failing to resolve
    /// aborts the step with no swipe). `site` names the endpoint in that error.
    /// seq129: an `asset` endpoint reuses the click asset path
    /// ([`Self::drag_asset_endpoint`]).
    fn drag_target(&mut self, t: &DragTarget, site: &str) -> Result<DragEndpoint, String> {
        match t {
            DragTarget::Asset { sel } => self.drag_asset_endpoint(sel, site),
            DragTarget::Region { roi, shape } => Ok(DragEndpoint {
                rect: Rect {
                    x: roi[0],
                    y: roi[1],
                    w: roi[2],
                    h: roi[3],
                },
                shape: shape.as_ref().map(ClickShape::from),
            }),
            DragTarget::Point { x, y } => Ok(DragEndpoint {
                rect: Rect {
                    x: *x,
                    y: *y,
                    w: 1,
                    h: 1,
                },
                shape: None,
            }),
            DragTarget::OcrText {
                sel,
                roi,
                text,
                region,
            } => {
                let text = self.resolve_text(text, &format!("{site}.text_expr"))?;
                // v1.3 `region.mode="nearest"` reference: the OCR **recognition
                // ROI** centre (`sel`'s ocr_roi, or the inline `roi`).
                // seq134: strict expected text with ROI→whole-frame fallback for a
                // named OCR asset; an inline det ROI has no asset/threshold.
                let reference: Point;
                let line: OcrLine = if let Some(roi) = roi {
                    let r = Rect {
                        x: roi[0],
                        y: roi[1],
                        w: roi[2],
                        h: roi[3],
                    };
                    reference = rect_centre(r);
                    let read = self.ocr_read_rect(r, true)?;
                    let lines = match read {
                        OcrRead::Lines(lines) => lines,
                        OcrRead::Single(..) => {
                            return Err(
                                "drag ocr_text target must use ocr_lines/inline det (single-line has no bbox)"
                                    .into(),
                            )
                        }
                    };
                    let matches = matching_ocr_lines_strict(&lines, &text, None);
                    match region {
                        Some(OpRegion::Nearest { .. }) => nearest_ocr_line(&matches, reference),
                        _ => pick_line_score_first(&matches),
                    }
                    .cloned()
                    .ok_or_else(|| {
                        format!("drag ocr_text: no line equals {text:?} in the region")
                    })?
                } else {
                    let sel_s = sel
                        .clone()
                        .ok_or_else(|| "ocr_text drag target needs sel or roi".to_string())?;
                    reference = rect_centre(
                        self.ocr_asset_declared_roi(&sel_s)
                            .ok_or_else(|| format!("ocr_text `{sel_s}`: OCR asset has no roi"))?,
                    );
                    let cands = self.ocr_target_candidates(&sel_s, &text)?;
                    let picked = match region {
                        Some(OpRegion::Nearest { .. }) => {
                            Self::pick_candidate(&cands, Some(reference))
                        }
                        _ => Self::pick_candidate(&cands, None),
                    }
                    .ok_or_else(|| {
                        format!("drag ocr_text: no line equals {text:?} in the region")
                    })?;
                    let c = &cands[picked];
                    let rect = Rect {
                        x: c.x,
                        y: c.y,
                        w: c.w,
                        h: c.h,
                    };
                    self.last_ocr_rect = Some(rect);
                    OcrLine {
                        rect,
                        text: c.text.clone().unwrap_or_default(),
                        score: c.score,
                    }
                };
                // v1.3 `region` (OPERATION-REGION-DESIGN §6.1): default = the
                // matched line bbox (historical drag behaviour); `asset` uses the
                // asset operation area/shape when declared.
                // v1.4 A-plan: a whole-library `region.asset` replaces the geometry
                // (nearest reference = the matched param-OCR line bbox centre).
                let asset_target = self.region_asset_target(
                    region,
                    rect_centre(line.rect),
                    &format!("{site}.region.asset"),
                )?;
                // 2026-11-06 参数→素材: an asset-level `op_region` chain replaces
                // the geometry only when the flow carries no legacy `region`.
                // 2026-11-06 批次B (#4/Q1) + seq127 #2/#3: the final operation
                // box is the terminal chain node's own self geometry; this root
                // box is only the fallback when the chain ends at the root.
                let chain = if region.is_none() {
                    match sel.as_deref() {
                        Some(s) if self.assets.has_op_region_chain(s) => {
                            let (self_box, self_shape) = self
                                .ocr_self_operation_geometry(s, line.rect, self.screen.size())?;
                            self.resolve_asset_op_region(
                                s,
                                line.rect,
                                self_box,
                                self_shape,
                                "drag ocr_text",
                            )?
                        }
                        _ => None,
                    }
                } else {
                    None
                };
                let (rect, shape) = if let Some(r) = &chain {
                    (r.rect, r.shape.clone())
                } else if let Some((rect, shape)) = asset_target {
                    (rect, shape)
                } else {
                    match region {
                        None => (line.rect, None),
                        Some(OpRegion::Asset { .. }) => {
                            let asset_rect = sel.as_deref().and_then(|s| self.assets.click_back(s));
                            let asset_shape = sel
                                .as_deref()
                                .and_then(|s| self.assets.click_shape(s))
                                .cloned();
                            (asset_rect.unwrap_or(line.rect), asset_shape)
                        }
                        Some(OpRegion::Ocr) | Some(OpRegion::Nearest { .. }) => (line.rect, None),
                        Some(OpRegion::Offset { dx, dy, w, h }) => {
                            (offset_region_rect(line.rect, *dx, *dy, *w, *h), None)
                        }
                    }
                };
                Ok(DragEndpoint { rect, shape })
            }
        }
    }

    /// v1.3: resolve a `long_press` `find[]` chain to the point that must be
    /// held — **no gesture is performed here**.
    ///
    /// The redundancy chain mirrors `click_target` (image > OCR > region >
    /// memory) and reuses the same OCR-candidate / operation-region helpers, so
    /// the two actions agree on what "the target" means. Keep them in sync when
    /// a locator kind changes. Errors: locator failures are collected as
    /// evidence; low-level layout errors (e.g. a broken asset) propagate.
    fn resolve_press_target(
        &mut self,
        find: &[Locator],
        resolved_text: &[Option<String>],
        w: u32,
        h: u32,
    ) -> Result<PressTarget, String> {
        let mut order: Vec<(usize, &Locator)> = find.iter().enumerate().collect();
        order.sort_by_key(|(_, l)| l.priority());
        let mut evidence: Vec<String> = Vec::new();
        for (idx, loc) in order {
            // Resolved OCR text of THIS locator ("" for non-OCR kinds).
            let text: &str = resolved_text[idx].as_deref().unwrap_or_default();
            match loc {
                Locator::Template { sel, threshold } => match self.locate(sel) {
                    Ok((score, at)) => {
                        let th = threshold.unwrap_or_else(|| self.selector_threshold(sel));
                        let Some(p) = at.filter(|_| score >= th) else {
                            evidence.push(format!(
                                "template `{sel}`: best score {score:.3} < {th}"
                            ));
                            continue;
                        };
                        let (rect, root_shape) = self.click_geometry_for(sel, p)?;
                        // 2026-11-06 参数→素材: an asset-level `op_region` chain
                        // replaces the historical geometry for this locator.
                        let plan_shape = root_shape.clone();
                        // seq132: a collection's winning member owns the chain.
                        let chain_sel = self.effective_asset(sel).to_string();
                        let chain = if self.assets.has_op_region_chain(&chain_sel) {
                            let (box_, shape, real) = self.chain_geometry_for(&chain_sel, p)?;
                            self.resolve_asset_op_region(
                                &chain_sel,
                                real,
                                box_,
                                shape,
                                "long_press template",
                            )?
                        } else {
                            None
                        };
                        let (target, chain_shape) = match &chain {
                            Some(r) => (r.rect, r.shape.clone()),
                            None => (rect, None),
                        };
                        let plan = self.humanize.template_click(target, w, &mut self.rng);
                        let plan = if let Some(shape) = chain_shape.as_ref() {
                            self.adjust_plan_for_explicit_shape(plan, Some(shape), w, h)?
                        } else if chain.is_some() {
                            plan
                        } else if let Some(shape) = plan_shape.as_ref() {
                            // seq130: same-source rect+shape (OCR Q1).
                            self.adjust_plan_for_explicit_shape(plan, Some(shape), w, h)?
                        } else {
                            plan
                        };
                        let chain_info = chain
                            .as_ref()
                            .map(|c| c.log_suffix())
                            .unwrap_or_default();
                        return Ok(PressTarget {
                            point: clamp_to_frame(plan.point, w, h),
                            locator: "template",
                            sel: Some(sel.clone()),
                            match_info: format!("template:{sel}:{score:.3}{chain_info}"),
                            region: None,
                        });
                    }
                    Err(e) => evidence.push(format!("template `{sel}`: {e}")),
                },
                Locator::OcrText {
                    sel,
                    require_template,
                    region,
                    ..
                } => match self.ocr_read(sel) {
                    Ok(OcrRead::Lines(lines)) => {
                        let min = self.assets.ocr_threshold(sel);
                        let mut matched: Vec<OcrLine> =
                    matching_ocr_lines_strict(&lines, text, min).into_iter().cloned().collect();
                let mut lines = lines;
                if matched.is_empty() && self.ocr_asset_declared_roi(sel).is_some() {
                    if let Ok(full) = self.ocr_lines_for(sel, true) {
                        lines = full;
                        matched = matching_ocr_lines_strict(&lines, text, min)
                            .into_iter()
                            .cloned()
                            .collect();
                    }
                }
                let candidates: Vec<&OcrLine> = matched.iter().collect();
                        let hit = match region {
                            Some(OpRegion::Nearest { .. }) => {
                                let roi = self.assets.ocr_roi(sel).ok_or_else(|| {
                                    format!("ocr_text `{sel}`: OCR asset has no roi")
                                })?;
                                nearest_ocr_line(&candidates, rect_centre(roi))
                            }
                            _ => candidates.iter().copied().max_by(|a, b| {
                                a.score
                                    .partial_cmp(&b.score)
                                    .unwrap_or(std::cmp::Ordering::Equal)
                            }),
                        };
                        let Some(line) = hit else {
                            evidence.push(format!(
                                "ocr_text `{sel}`: no line equals {text:?} ({} lines found)",
                                lines.len()
                            ));
                            continue;
                        };
                        if let Some(rt) = require_template {
                            match self.template_in_region(rt, line.rect, 16) {
                                Ok(true) => {}
                                Ok(false) => {
                                    evidence.push(format!(
                                        "ocr_text `{sel}`: line {:?} matched but required \
                                         template `{rt}` not found near it",
                                        line.text
                                    ));
                                    continue;
                                }
                                Err(e) => return Err(e),
                            }
                        }
                        let centre = Point {
                            x: line.rect.x + line.rect.w / 2,
                            y: line.rect.y + line.rect.h / 2,
                        };
                        // v1.4 A-plan: a whole-library `region.asset` replaces the
                        // geometry; the parameter OCR line above still had to match.
                        let asset_target = self.region_asset_target(
                            region,
                            centre,
                            &format!("long_press args.find[{idx}].region.asset"),
                        )?;
                        // 2026-11-06 批次B (#4/Q1) + seq127 #2/#3: the chain's
                        // terminal node wins; this root self geometry is only the
                        // fallback when the chain ends at the root.
                        let (self_box, self_shape) =
                            self.ocr_self_operation_geometry(sel, line.rect, Some((w, h)))?;
                        let default_target = self_box;
                        // 2026-11-06 参数→素材: asset-level `op_region` chain
                        // (only without a legacy flow `region`).
                        let chain = if region.is_none() && self.assets.has_op_region_chain(sel) {
                            self.resolve_asset_op_region(
                                sel,
                                line.rect,
                                default_target,
                                self_shape.clone(),
                                "long_press ocr_text",
                            )?
                        } else {
                            None
                        };
                        let (target, explicit_shape, shape_aware) =
                            if let Some(r) = &chain {
                                (r.rect, r.shape.clone(), false)
                            } else if let Some((rect, shape)) = asset_target {
                                (rect, shape, false)
                            } else {
                                let (t, _aware) = match region {
                                    None | Some(OpRegion::Asset { .. }) => (default_target, true),
                                    Some(OpRegion::Ocr)
                                    | Some(OpRegion::Nearest { .. }) => (line.rect, false),
                                    Some(OpRegion::Offset { dx, dy, w, h }) => {
                                        (offset_region_rect(line.rect, *dx, *dy, *w, *h), false)
                                    }
                                };
                                (t, self_shape.clone(), false)
                            };
                        let plan = self.humanize.template_click(target, w, &mut self.rng);
                        let plan = if explicit_shape.is_some() {
                            self.adjust_plan_for_explicit_shape(
                                plan,
                                explicit_shape.as_ref(),
                                w,
                                h,
                            )?
                        } else if shape_aware {
                            self.adjust_plan_for_shape(sel, plan, w, h, centre)?
                        } else {
                            plan
                        };
                        let chain_info = chain
                            .as_ref()
                            .map(|c| c.log_suffix())
                            .unwrap_or_default();
                        return Ok(PressTarget {
                            point: clamp_to_frame(plan.point, w, h),
                            locator: "ocr_text",
                            sel: Some(sel.clone()),
                            match_info: format!(
                                "ocr_text:{sel}:{}:{:.3}{chain_info}",
                                line.text, line.score
                            ),
                            region: region.clone(),
                        });
                    }
                    Ok(OcrRead::Single(Some(t), score)) => {
                        if !(ocr_text_matches_strict(text, &t) && self.ocr_score_ok(sel, score)) {
                            evidence.push(format!(
                                "ocr_text `{sel}`: text {t:?} matched but score {score:.3} \
                                 is below the declared OCR threshold"
                            ));
                            continue;
                        }
                        let recognition = if let Some(r) = self.assets.ocr_roi(sel) {
                            r
                        } else if let Some(r) = self.assets.ocr_var(sel).and_then(|v| v.roi) {
                            r
                        } else if let Some(r) = self.last_ocr_rect {
                            r
                        } else {
                            return Err(format!("ocr_text `{sel}`: OCR asset has no roi"));
                        };
                        // 2026-11-06 批次B (#4/Q1) + seq127 #2/#3: the chain's
                        // terminal node wins; this root self geometry is only the
                        // fallback when the chain ends at the root.
                        let real = self.last_ocr_rect.unwrap_or(recognition);
                        let (self_box, self_shape) =
                            self.ocr_self_operation_geometry(sel, real, Some((w, h)))?;
                        let default_target = self_box;
                        let marker = Point {
                            x: recognition.x,
                            y: recognition.y,
                        };
                        // v1.4 A-plan: whole-library operation asset (single-line
                        // OCR has no line bbox: the recognition ROI is the reference).
                        let asset_target = self.region_asset_target(
                            region,
                            rect_centre(recognition),
                            &format!("long_press args.find[{idx}].region.asset"),
                        )?;
                        // 2026-11-06 参数→素材: asset-level `op_region` chain
                        // (only without a legacy flow `region`); model B: the
                        // terminal node's own geometry is the final operation box.
                        let chain = if region.is_none() && self.assets.has_op_region_chain(sel) {
                            self.resolve_asset_op_region(
                                sel,
                                real,
                                default_target,
                                self_shape.clone(),
                                "long_press ocr_text",
                            )?
                        } else {
                            None
                        };
                        let (target, explicit_shape, shape_aware) =
                            if let Some(r) = &chain {
                                (r.rect, r.shape.clone(), false)
                            } else if let Some((rect, shape)) = asset_target {
                                (rect, shape, false)
                            } else {
                                let (t, _aware) = match region {
                                    None | Some(OpRegion::Asset { .. }) => (default_target, true),
                                    Some(OpRegion::Ocr)
                                    | Some(OpRegion::Nearest { .. }) => (recognition, false),
                                    Some(OpRegion::Offset { dx, dy, w, h }) => {
                                        (offset_region_rect(recognition, *dx, *dy, *w, *h), false)
                                    }
                                };
                                (t, self_shape.clone(), false)
                            };
                        if let Some(rt) = require_template {
                            match self.template_in_region(rt, recognition, 16) {
                                Ok(true) => {}
                                Ok(false) => {
                                    evidence.push(format!(
                                        "ocr_text `{sel}`: {t:?} matched but required template \
                                         `{rt}` not found near it"
                                    ));
                                    continue;
                                }
                                Err(e) => return Err(e),
                            }
                        }
                        let plan = self.humanize.template_click(target, w, &mut self.rng);
                        let plan = if explicit_shape.is_some() {
                            self.adjust_plan_for_explicit_shape(
                                plan,
                                explicit_shape.as_ref(),
                                w,
                                h,
                            )?
                        } else if shape_aware {
                            self.adjust_plan_for_shape(sel, plan, w, h, marker)?
                        } else {
                            plan
                        };
                        return Ok(PressTarget {
                            point: clamp_to_frame(plan.point, w, h),
                            locator: "ocr_text",
                            sel: Some(sel.clone()),
                            match_info: format!(
                                "ocr_text:{sel}:{t}:{score:.3}{}",
                                chain
                                    .as_ref()
                                    .map(|c| c.log_suffix())
                                    .unwrap_or_default()
                            ),
                            region: region.clone(),
                        });
                    }
                    Ok(OcrRead::Single(None, score)) => evidence.push(format!(
                        "ocr_text `{sel}`: no text (score {score:.3})"
                    )),
                    Err(e) => evidence.push(format!("ocr_text `{sel}`: {e}")),
                },
                Locator::OcrTextInline {
                    roi,
                    require_template,
                    region,
                    ..
                } => {
                    let rect = Rect {
                        x: roi[0],
                        y: roi[1],
                        w: roi[2],
                        h: roi[3],
                    };
                    match self.ocr_read_rect(rect, true) {
                        Ok(OcrRead::Lines(lines)) => {
                            let candidates = ocr_line_candidates(&lines, text, None);
                            let hit = match region {
                                Some(OpRegion::Nearest { .. }) => {
                                    nearest_ocr_line(&candidates, rect_centre(rect))
                                }
                                _ => candidates.iter().copied().max_by(|a, b| {
                                    a.score
                                        .partial_cmp(&b.score)
                                        .unwrap_or(std::cmp::Ordering::Equal)
                                }),
                            };
                            let Some(line) = hit else {
                                evidence.push(format!(
                                    "ocr_text inline: no line equals {text:?} \
                                     ({} lines found in roi {roi:?})",
                                    lines.len()
                                ));
                                continue;
                            };
                            if let Some(rt) = require_template {
                                match self.template_in_region(rt, line.rect, 16) {
                                    Ok(true) => {}
                                    Ok(false) => {
                                        evidence.push(format!(
                                            "ocr_text inline: line {:?} matched but required \
                                             template `{rt}` not found near it",
                                            line.text
                                        ));
                                        continue;
                                    }
                                    Err(e) => return Err(e),
                                }
                            }
                            // v1.4 A-plan: inline form (reference = matched line centre).
                            let asset_target = self.region_asset_target(
                                region,
                                rect_centre(line.rect),
                                &format!("long_press args.find[{idx}].region.asset"),
                            )?;
                            let target = if let Some((rect, shape)) = asset_target {
                                let plan = self.humanize.template_click(rect, w, &mut self.rng);
                                let plan = self.adjust_plan_for_explicit_shape(
                                    plan,
                                    shape.as_ref(),
                                    w,
                                    h,
                                )?;
                                return Ok(PressTarget {
                                    point: clamp_to_frame(plan.point, w, h),
                                    locator: "ocr_text",
                                    sel: None,
                                    match_info: format!(
                                        "ocr_text:inline:{}:{:.3}",
                                        line.text, line.score
                                    ),
                                    region: region.clone(),
                                });
                            } else {
                                match region {
                                    None
                                    | Some(OpRegion::Asset { .. })
                                    | Some(OpRegion::Ocr)
                                    | Some(OpRegion::Nearest { .. }) => line.rect,
                                    Some(OpRegion::Offset { dx, dy, w, h }) => {
                                        offset_region_rect(line.rect, *dx, *dy, *w, *h)
                                    }
                                }
                            };
                            let plan = self.humanize.template_click(target, w, &mut self.rng);
                            return Ok(PressTarget {
                                point: clamp_to_frame(plan.point, w, h),
                                locator: "ocr_text",
                                sel: None,
                                match_info: format!(
                                    "ocr_text:inline:{}:{:.3}",
                                    line.text, line.score
                                ),
                                region: region.clone(),
                            });
                        }
                        Ok(OcrRead::Single(Some(t), score)) => {
                            if ocr_text_matches_strict(text, &t) {
                                // v1.4 A-plan: inline single-line reference = rect centre.
                                let asset_target = self.region_asset_target(
                                    region,
                                    rect_centre(rect),
                                    &format!("long_press args.find[{idx}].region.asset"),
                                )?;
                                if let Some((rect, shape)) = asset_target {
                                    let plan =
                                        self.humanize.template_click(rect, w, &mut self.rng);
                                    let plan = self.adjust_plan_for_explicit_shape(
                                        plan,
                                        shape.as_ref(),
                                        w,
                                        h,
                                    )?;
                                    return Ok(PressTarget {
                                        point: clamp_to_frame(plan.point, w, h),
                                        locator: "ocr_text",
                                        sel: None,
                                        match_info: format!(
                                            "ocr_text:inline:{t}:{score:.3}"
                                        ),
                                        region: region.clone(),
                                    });
                                }
                                let target = match region {
                                    Some(OpRegion::Offset { dx, dy, w, h }) => {
                                        offset_region_rect(rect, *dx, *dy, *w, *h)
                                    }
                                    _ => rect,
                                };
                                let plan =
                                    self.humanize.template_click(target, w, &mut self.rng);
                                return Ok(PressTarget {
                                    point: clamp_to_frame(plan.point, w, h),
                                    locator: "ocr_text",
                                    sel: None,
                                    match_info: format!("ocr_text:inline:{t}:{score:.3}"),
                                    region: region.clone(),
                                });
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
                    self.observe_screen()?;
                    let (fw, fh) = self
                        .screen
                        .size()
                        .ok_or_else(|| "long_press region: no frame available".to_string())?;
                    let rect = Rect {
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
                    return Ok(PressTarget {
                        point: clamp_to_frame(plan.point, fw, fh),
                        locator: "region",
                        sel: None,
                        match_info: format!(
                            "region:({},{},{},{})",
                            roi[0], roi[1], roi[2], roi[3]
                        ),
                        region: None,
                    });
                }
                Locator::RegionRef { sel } => {
                    let rect = self.assets.region(sel).ok_or_else(|| {
                        format!(
                            "region `{sel}` is not declared as a task region asset (kind: region)"
                        )
                    })?;
                    let exact_shape = self.assets.click_shape(sel).cloned();
                    self.observe_screen()?;
                    let (fw, fh) = self
                        .screen
                        .size()
                        .ok_or_else(|| "long_press region: no frame available".to_string())?;
                    if exact_shape.is_none()
                        && (rect.x < 0
                            || rect.y < 0
                            || rect.x + rect.w > fw as i32
                            || rect.y + rect.h > fh as i32)
                    {
                        evidence.push(format!(
                            "region `{sel}` rect ({},{},{},{}) is outside the frame ({}x{})",
                            rect.x, rect.y, rect.w, rect.h, fw, fh
                        ));
                        continue;
                    }
                    let p = match exact_shape.as_ref() {
                        Some(shape) => shape
                            .sample(fw, fh, &mut self.rng)
                            .map_err(|e| format!("region `{sel}` exact shape: {e}"))?,
                        None => self.random_point_in(rect, 0.15),
                    };
                    let mut plan = self.humanize.point_click(p, &mut self.rng);
                    if let Some(shape) = exact_shape.as_ref() {
                        // B-04: never let the jitter leave a narrow region.
                        if !self.click_shape_accepts(shape, plan.point, fw, fh) {
                            let mut accepted = None;
                            for _ in 0..8 {
                                let q = shape.sample(fw, fh, &mut self.rng)?;
                                let candidate = self.humanize.point_click(q, &mut self.rng);
                                if self.click_shape_accepts(shape, candidate.point, fw, fh) {
                                    accepted = Some(candidate);
                                    break;
                                }
                            }
                            plan = accepted.unwrap_or(crate::humanize::ClickPlan {
                                point: p,
                                miss: false,
                            });
                        }
                    }
                    return Ok(PressTarget {
                        point: clamp_to_frame(plan.point, fw, fh),
                        locator: "region",
                        sel: Some(sel.clone()),
                        match_info: format!(
                            "region:{sel}:({},{},{},{})",
                            rect.x, rect.y, rect.w, rect.h
                        ),
                        region: None,
                    });
                }
                Locator::CoordMemory { key } => {
                    let Some(store) = self.coords.as_mut() else {
                        evidence.push("coord_memory: no store attached".to_string());
                        continue;
                    };
                    match store.get(key, w, h) {
                        Some(p) => {
                            let plan = self.humanize.point_click(p, &mut self.rng);
                            return Ok(PressTarget {
                                point: clamp_to_frame(plan.point, w, h),
                                locator: "coord_memory",
                                sel: None,
                                match_info: format!("coord_memory:{key}"),
                                region: None,
                            });
                        }
                        None => {
                            evidence.push(format!("coord_memory `{key}`: no remembered point"))
                        }
                    }
                }
            }
        }
        Err(format!(
            "all locators failed: [{}]",
            evidence.join("; ")
        ))
    }

    /// Sample one drag endpoint (v1.2.5 O-2).
    ///
    /// * no shape, or a **non-inverted rect** shape: the historical bbox path
    ///   `random_point_in(rect, margin)`, bit-for-bit unchanged. This is the
    ///   **only** case where `margin` is applied (P2-2).
    /// * polygon or inverted shape: the same [`ClickShape::sample`] the click
    ///   path uses (point-in-polygon; full screen minus the shape for invert).
    ///   `margin` has no defined meaning there and is ignored.
    ///
    /// Sampling failures stay **loud** — they never fall back to the bbox or its
    /// centre (that was the §E-72 P1-3 defect on the click path). In particular
    /// a polygon/invert shape whose bbox misses the frame fails with
    /// `... does not intersect the ... frame` and suppresses the swipe.
    fn sample_drag_point(
        &mut self,
        endpoint: &DragEndpoint,
        margin: f32,
        w: u32,
        h: u32,
    ) -> Result<Point, String> {
        let exact = endpoint
            .shape
            .as_ref()
            .filter(|s| s.rect.is_none() || s.invert);
        match exact {
            None => Ok(self.random_point_in(endpoint.rect, margin)),
            Some(shape) => shape.sample(w, h, &mut self.rng),
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
        // i64 + clamp (P1-4): a pathological rect near i32::MAX must not panic
        // in debug / wrap in release. The caller's on-screen check turns a
        // clamped off-screen point into a loud failure.
        let x = rect.x as i64 + mx as i64 + dx.min(span_x - 1) as i64;
        let y = rect.y as i64 + my as i64 + dy.min(span_y - 1) as i64;
        Point {
            x: x.clamp(i32::MIN as i64, i32::MAX as i64) as i32,
            y: y.clamp(i32::MIN as i64, i32::MAX as i64) as i32,
        }
    }

    /// seq134: OCR read through the ONE recognition engine (strict expected text,
    /// ROI→whole-frame fallback for expected OCR, ROI-only for plain OCR without
    /// expected text). Every interaction (`click`/`click_target`/`long_press`/
    /// `appear`/`wait_until`/drag/`ocr()` value reads) uses this.
    fn ocr_read(&mut self, sel: &str) -> Result<OcrRead, String> {
        if self.assets.ocr_var(sel).is_some() {
            let expected = self.assets.ocr_var_expected_text(sel, &self.params)?;
            let best = match self.recognize_ocr_best(sel) {
                Ok(b) => b,
                Err(e) => {
                    return Err(format!(
                        "ocr_var `{sel}`: no line equals {expected:?} in its ROI or the whole \
                         frame ({e})"
                    ))
                }
            };
            return Ok(OcrRead::Single(best.text, best.score));
        }
        if self.assets.get(sel).is_some() {
            return Err(format!(
                "selector `{sel}` is a template asset, not an OCR asset \
                 (kind must be `ocr_line`/`ocr_lines`)"
            ));
        }
        if !self.assets.is_ocr_asset(sel) {
            return Err(format!(
                "OCR selector `{sel}` is not declared in the task's assets"
            ));
        }
        let expected = self.ocr_asset_expected(sel)?;
        if expected.is_some() {
            let best = self.recognize_ocr_best(sel)?;
            return Ok(OcrRead::Single(best.text, best.score));
        }
        // Plain OCR without expected text: its declared ROI only (seq134 #4). A
        // single-line `ocr_line` keeps the historical rec-only read; `ocr_lines`
        // uses det+rec and the highest-score line is the recognized row.
        if !self.assets.ocr_is_multiline(sel) {
            self.observe_screen()?;
            self.last_ocr_rect = None;
            let frame = self
                .screen
                .frame
                .clone()
                .ok_or_else(|| format!("OCR `{sel}`: no frame available"))?;
            let frame_rect = Rect {
                x: 0,
                y: 0,
                w: frame.width as i32,
                h: frame.height as i32,
            };
            let roi = self.ocr_asset_declared_roi(sel).unwrap_or(frame_rect);
            let polygon = self.assets.ocr_polygon(sel).map(|p| p.to_vec());
            let masked;
            let frame_ref: &Frame = match polygon.as_ref() {
                Some(poly) => {
                    masked = mask_frame_for_ocr(&frame, roi, poly);
                    &masked
                }
                None => &frame,
            };
            let provider = self.ocr.as_mut().ok_or_else(|| {
                format!(
                    "OCR is not available for this run: no OCR provider attached \
                     (build with `--features ocr` and pass `--model <rec.onnx>`) - \
                     refusing to fake `ocr({sel})`"
                )
            })?;
            let (text, score) = provider.recognize(frame_ref, roi)?;
            return Ok(OcrRead::Single(
                if text.is_empty() { None } else { Some(text) },
                score,
            ));
        }
        let lines = self.ocr_lines_for(sel, false)?;
        let best = lines.iter().max_by(|a, b| {
            a.score
                .partial_cmp(&b.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        self.last_ocr_rect = best.map(|l| l.rect);
        if self.assets.ocr_is_multiline(sel) {
            Ok(OcrRead::Lines(lines))
        } else {
            let (text, score) = best
                .map(|l| (Some(l.text.clone()), l.score))
                .unwrap_or((None, 0.0));
            Ok(OcrRead::Single(text, score))
        }
    }

    /// Value-path view of [`Self::ocr_read`]: applies the asset's **explicit**
    /// minimum recognition confidence.
    ///
    /// `ocr()`, `ocr_digit()` and `ocr_to_var` return recognized text, so a
    /// low-confidence read must not silently become a value (leftover 3). When
    /// the asset declares a non-zero `threshold` and the reported score is
    /// below it, the read is a **miss**, exactly like a template below its NCC
    /// threshold: `ocr()`/`ocr_digit()` yield `None` and `ocr_to_var` stores an
    /// empty string. This is deliberately *not* a hard error — `wait_until(ocr(
    /// ...) == ...)` is the documented way to wait for a value, and aborting the
    /// run on the first low-confidence poll would break it. The `ocr`
    /// diagnostic step still logs the raw text + score, so a filtered read stays
    /// visible. Unset/`0` keeps the historical "recognized = value" behaviour;
    /// a read with no text at all is already `None` and passes unchanged.
    fn ocr_value_read(&mut self, sel: &str) -> Result<OcrRead, String> {
        let read = self.ocr_read(sel)?;
        let Some(min) = self.assets.ocr_threshold(sel) else {
            return Ok(read);
        };
        match read {
            OcrRead::Single(Some(_), score) if score < min => Ok(OcrRead::Single(None, score)),
            // The value joins every line, so the value's confidence is the
            // LOWEST line score (the same score `ocr()` reports/logs).
            OcrRead::Lines(lines) if !lines.is_empty() && lines.iter().any(|l| l.score < min) => {
                Ok(OcrRead::Lines(Vec::new()))
            }
            other => Ok(other),
        }
    }

    /// Value view of [`Self::ocr_read`]: `ocr()` returns **one string**.
    ///
    /// For `ocr_lines` the lines are joined with `\n` and the score is the
    /// **minimum** line score — never an optimistic average (a bad line must
    /// lower the reported confidence). An explicit OCR threshold is enforced by
    /// [`Self::ocr_value_read`] before the text reaches the flow.
    fn ocr_run(&mut self, sel: &str) -> Result<(Option<String>, f32), String> {
        match self.ocr_value_read(sel)? {
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

    /// Threshold for a selector (see [`asset_threshold`], the public single
    /// source of truth shared with the editor candidate API).
    ///
    /// * template / collection: the declared NCC threshold, else 0.9;
    /// * OCR (`ocr_line` / `ocr_lines`, incl. derived lines): the asset's
    ///   explicit minimum confidence when **> 0**, else **0** — an explicit `0`
    ///   means "unset = no confidence filter", exactly like a missing value, and
    ///   a successful read is a hit (P1-4). Setting `t > 0` makes a read below
    ///   it a miss, with the same `score >= threshold` rule templates use.
    fn selector_threshold(&self, sel: &str) -> f32 {
        asset_threshold(&self.assets, sel)
    }

    /// `true` when an OCR read at `score` passes the asset's **optional**
    /// minimum confidence. No declared/active threshold (absent or `0`) = always
    /// pass (historical "recognized = hit" semantics).
    fn ocr_score_ok(&self, sel: &str, score: f32) -> bool {
        match self.assets.ocr_threshold(sel) {
            Some(min) => score >= min,
            None => true,
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
            let th = self.selector_threshold(&rule.detect);
            let Some(_detected) = at.filter(|_| score >= th) else {
                continue;
            };
            let action_sel = rule.action.clone().unwrap_or_else(|| rule.detect.clone());
            let (ascore, aat) = self.locate(&action_sel)?;
            let ath = self.selector_threshold(&action_sel);
            let Some(ap) = aat.filter(|_| ascore >= ath) else {
                return Err(format!(
                    "interrupt: popup `{}` detected (score {score:.3}) but action `{action_sel}` \
                     not found (best {ascore:.3} < {ath})",
                    rule.detect
                ));
            };
            let (target, action_shape) = self.click_geometry_for(&action_sel, ap)?;
            let (w, h) = self
                .screen.size()
                .ok_or_else(|| "interrupt: no frame available".to_string())?;
            let plan = self.humanize.template_click(target, w, &mut self.rng);
            let plan = self.adjust_plan_for_explicit_shape(plan, action_shape.as_ref(), w, h)?;
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
        // 参数 v2: error context for `text_expr` resolution (op + key).
        self.current_op = step_name(step);
        match step {
            Step::If {
                cond,
                then,
                otherwise,
            } => {
                // A condition must describe the CURRENT screen, not a frame
                // cached before a wait/transition.
                self.reobserve_screen()?;
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
                    // A while condition describes the CURRENT screen. Drop any
                    // frame cached by the previous body/condition so appear()
                    // grabs a live frame; otherwise a post-click/transition
                    // condition can keep evaluating pre-transition pixels.
                    self.reobserve_screen()?;
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
                    // Detailed per-iteration condition evidence: when a loop
                    // does not exit, this shows which appear()/selector terms
                    // were false without guessing from the final state.
                    self.log(
                        depth + 1,
                        "while_cond",
                        &format!(
                            "iteration={iters} value={value:?} truthy={truthy} frame_seq={} cond={cond:?}",
                            self.screen.seq()
                        ),
                        "ok",
                        0.0,
                    );
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
                // The next selector/condition must re-grab: a wait is often
                // used exactly because the screen is transitioning.
                self.invalidate_frame();
                Ok(format!("waited {s}s"))
            }
            Step::Key { keycode } => {
                self.input
                    .key(*keycode)
                    .map_err(|e| format!("key {keycode}: {e}"))?;
                self.invalidate_frame();
                Ok(format!("key {keycode}"))
            }
            Step::RestartApp { package } => {
                self.input
                    .restart_app(package)
                    .map_err(|e| format!("restart_app {package}: {e}"))?;
                self.invalidate_frame();
                Ok(format!("restart_app {package}"))
            }
            Step::WaitUntil { cond, timeout_s } => {
                let deadline = Instant::now() + Duration::from_secs_f64(*timeout_s);
                loop {
                    // First check must also be fresh; only poll frames after.
                    self.reobserve_screen()?;
                    if matches!(eval(cond, self)?, Value::Bool(true)) {
                        return Ok("condition met".into());
                    }
                    if Instant::now() >= deadline {
                        return Err(format!("wait_until timed out after {timeout_s}s"));
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
            }
            Step::Appear { sel } => {
                self.reobserve_screen()?;
                let (score, at) = self.locate(sel)?;
                // seq134: `appear` must actually RECOGNIZE something (an empty OCR
                // read has score 0/None and must not pass even without a threshold).
                let th = self.selector_threshold(sel);
                if at.is_some() && score >= th {
                    Ok(format!("appear {sel} score={score:.3} at {at:?}"))
                } else {
                    Err(format!(
                        "selector `{sel}` not found (recognized={}, best score {score:.3} < {th})",
                        at.is_some()
                    ))
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
                    self.reobserve_screen()?;
                    let (score, at) = self.locate(sel)?;
                    let th = self.selector_threshold(sel);
                    if let Some(_detected) = at.filter(|_| score >= th) {
                        // §22.15: `action` selects the button (e.g. 接受); when
                        // omitted the detected template itself is clicked.
                        let action_sel = action.clone().unwrap_or_else(|| sel.clone());
                        let (ascore, aat) = self.locate(&action_sel)?;
                        let ath = self.selector_threshold(&action_sel);
                        let Some(ap) = aat.filter(|_| ascore >= ath) else {
                            return Err(format!(
                                "dismiss_popup: popup `{sel}` detected but action `{action_sel}` \
                                 not found (best {ascore:.3} < {ath})"
                            ));
                        };
                        let (target, action_shape) = self.click_geometry_for(&action_sel, ap)?;
                        let (w, h) = self
                            .screen.size()
                            .ok_or_else(|| "dismiss_popup: no frame available".to_string())?;
                        let plan = self.humanize.template_click(target, w, &mut self.rng);
                        let plan = self.adjust_plan_for_explicit_shape(plan, action_shape.as_ref(), w, h)?;
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
                self.reobserve_screen()?;
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
                    // seq134: ONE recognition path for templates, collections AND
                    // OCR (`locate` → `recognize_best`): strict expected text,
                    // ROI→whole-frame OCR fallback, `last_ocr_rect` = recognized
                    // line for every OCR kind.
                    let (score, at) = self.locate(cand)?;
                    let th = self.selector_threshold(cand);
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
                if vote_frames > 1 && self.assets.ocr_roi(hit_sel).is_none() {
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
                //
                // seq130 fix: rect and shape come from ONE source (OCR assets
                // translate both with the recognized line, Q1).
                let (target, same_source_shape) = self.click_geometry_for(hit_sel, p)?;
                // 2026-11-06 参数→素材: an asset-level `op_region` chain replaces
                // the historical geometry. Model B (批次B #4 + seq127 #2/#3): the
                // chain walks to the last referenced node and uses its own area.
                // Root real rect: OCR = its matched line bbox (`last_ocr_rect`),
                // template/collection = matched bbox.
                let mut chain = None;
                // seq132: a collection's winning member owns the chain.
                let chain_sel = self.effective_asset(hit_sel).to_string();
                if self.assets.has_op_region_chain(&chain_sel) {
                    let (root_box, root_shape, real) = self.chain_geometry_for(&chain_sel, p)?;
                    chain = self.resolve_asset_op_region(
                        &chain_sel,
                        real,
                        root_box,
                        root_shape,
                        "click",
                    )?;
                }
                let (target, chain_shape) = match &chain {
                    Some(r) => (r.rect, r.shape.clone()),
                    None => (target, None),
                };
                // HB1/HB7: 20% bell / 5% uniform / 75% comet (hand-side tail)
                // inside the target rect; with 1~3% probability the click MISSES
                // the box (still performed, audited as `miss=true`).
                let (w, h) = self
                    .screen.size()
                    .ok_or_else(|| "click: no frame available".to_string())?;
                let plan = self.humanize.template_click(target, w, &mut self.rng);
                let plan = if let Some(shape) = chain_shape.as_ref() {
                    self.adjust_plan_for_explicit_shape(plan, Some(shape), w, h)?
                } else if chain.is_some() {
                    plan
                } else if let Some(shape) = same_source_shape.as_ref() {
                    self.adjust_plan_for_explicit_shape(plan, Some(shape), w, h)?
                } else {
                    plan
                };
                let centre = Point {
                    x: target.x + target.w / 2,
                    y: target.y + target.h / 2,
                };
                let match_info = if self.assets.is_ocr_asset(hit_sel) {
                    format!("ocr:{hit_sel}:{score:.3}")
                } else {
                    format!("template:{hit_sel}:{score:.3}")
                };
                let (actual, delay) = self.perform_click(
                    plan,
                    centre,
                    Some(hit_sel.to_string()),
                    if self.assets.is_ocr_asset(hit_sel) { "ocr" } else { "template" },
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
                let report_origin = if self.assets.is_ocr_asset(hit_sel) {
                    Point { x: target.x, y: target.y }
                } else {
                    p
                };
                let chain_info = chain
                    .as_ref()
                    .map(|c| c.log_suffix())
                    .unwrap_or_default();
                Ok(format!(
                    "clicked {hit_sel} target=({},{}) point=({},{}){miss} delay={delay}ms score={score:.3} verified={verified} guard={guarded}{voted}{chain_info}",
                    report_origin.x, report_origin.y, actual.x, actual.y
                ))
            }
            Step::ClickPoint { x, y, requires, expect, .. } => {
                self.reobserve_screen()?;
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
                self.observe_screen()?;
                let (w, h) = self
                    .screen.size()
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
                self.reobserve_screen()?;
                // 参数 v2 (PARAM-V2-DESIGN §3.2.4): resolve every locator
                // `text_expr` ONCE at step entry, before the redundancy chain
                // runs. A missing / empty / non-string value aborts the step
                // here — it must never fall through to a lower-priority locator
                // (that could click a remembered coordinate instead).
                let mut resolved_text: Vec<Option<String>> = Vec::with_capacity(find.len());
                for (j, loc) in find.iter().enumerate() {
                    resolved_text.push(match loc.text_spec() {
                        None => None,
                        Some(spec) => Some(self.resolve_text(
                            spec,
                            &format!("click_target args.find[{j}].text_expr"),
                        )?),
                    });
                }
                let guarded = self.require_guard(requires.as_ref())?;
                // ENGINE §22.11: fixed priority image > text > memory; every
                // locator failure is kept as evidence, and ALL failing is a loud
                // step failure (never a silent skip).
                if self.screen.size().is_none() {
                    self.observe_screen()?;
                }
                let (w, h) = self
                    .screen.size()
                    .ok_or_else(|| "click_target: no frame available".to_string())?;
                let mut order: Vec<(usize, &Locator)> = find.iter().enumerate().collect();
                order.sort_by_key(|(_, l)| l.priority());
                let mut evidence: Vec<String> = Vec::new();
                for (idx, loc) in order {
                    // The resolved OCR text of THIS locator ("" for the non-OCR
                    // kinds, which never read it). Parser guarantees a `TextSpec`
                    // on every `ocr_text` locator, so it is always `Some` there.
                    let text: &str = resolved_text[idx].as_deref().unwrap_or_default();
                    match loc {
                        Locator::Template { sel, threshold } => match self.locate(sel) {
                            Ok((score, at)) => {
                                // An inline locator threshold wins; otherwise use
                                // the same selector policy as click/appear, so an
                                // OCR id named here gets 0 (0 = unset, P1-4) and
                                // a template/collection gets its 0.9 fallback.
                                let th = threshold
                                    .unwrap_or_else(|| self.selector_threshold(sel));
                                let Some(p) = at.filter(|_| score >= th) else {
                                    evidence.push(format!(
                                        "template `{sel}`: best score {score:.3} < {th}"
                                    ));
                                    continue;
                                };
                                // 2026-11-06 参数→素材: an explicit asset-level
                                // `op_region` chain replaces the historical
                                // geometry; absent/`self` keeps the old path
                                // byte-identical.
                                let (root_box, root_shape) = self.click_geometry_for(sel, p)?;
                                let plan_shape = root_shape.clone();
                                // seq132: a collection's winning member owns the chain.
                                let chain_sel = self.effective_asset(sel).to_string();
                                let chain = if self.assets.has_op_region_chain(&chain_sel) {
                                    let (box_, shape, real) =
                                        self.chain_geometry_for(&chain_sel, p)?;
                                    self.resolve_asset_op_region(
                                        &chain_sel,
                                        real,
                                        box_,
                                        shape,
                                        "click_target template",
                                    )?
                                } else {
                                    None
                                };
                                let (target, chain_shape) = match &chain {
                                    Some(r) => (r.rect, r.shape.clone()),
                                    None => (root_box, None),
                                };
                                let plan = self.humanize.template_click(target, w, &mut self.rng);
                                let plan = if let Some(shape) = chain_shape.as_ref() {
                                    self.adjust_plan_for_explicit_shape(
                                        plan,
                                        Some(shape),
                                        w,
                                        h,
                                    )?
                                } else if chain.is_some() {
                                    plan
                                } else if let Some(shape) = plan_shape.as_ref() {
                                    // seq130: same-source rect+shape (OCR Q1).
                                    self.adjust_plan_for_explicit_shape(
                                        plan,
                                        Some(shape),
                                        w,
                                        h,
                                    )?
                                } else {
                                    plan
                                };
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
                                let chain_info = chain
                                    .as_ref()
                                    .map(|c| c.log_suffix())
                                    .unwrap_or_default();
                                return Ok(format!(
                                    "clicked_target locator=template sel={sel} score={score:.3} point=({},{}){miss} verified={verified} guard={guarded}{chain_info}",
                                    actual.x, actual.y
                                ));
                            }
                            Err(e) => evidence.push(format!("template `{sel}`: {e}")),
                        },
                        Locator::OcrText {
                            sel,
                            require_template,
                            region,
                            ..
                        } => match self.ocr_read(sel) {
                            Ok(OcrRead::Lines(lines)) => {
                                // An explicitly declared OCR minimum confidence
                                // filters candidate lines: a matched-text line
                                // below it is not evidence, exactly like a
                                // template below its NCC threshold.
                                let min = self.assets.ocr_threshold(sel);
                                // v1.3 `region`: candidate filtering is shared
                                // with drag / long_press. The default pick stays
                                // "highest score" (historical click behaviour).
                                let mut matched: Vec<OcrLine> =
                    matching_ocr_lines_strict(&lines, text, min).into_iter().cloned().collect();
                let mut lines = lines;
                if matched.is_empty() && self.ocr_asset_declared_roi(sel).is_some() {
                    if let Ok(full) = self.ocr_lines_for(sel, true) {
                        lines = full;
                        matched = matching_ocr_lines_strict(&lines, text, min)
                            .into_iter()
                            .cloned()
                            .collect();
                    }
                }
                let candidates: Vec<&OcrLine> = matched.iter().collect();
                                let hit = match region {
                                    Some(OpRegion::Nearest { .. }) => {
                                        let roi = self.assets.ocr_roi(sel).ok_or_else(|| {
                                            format!("ocr_text `{sel}`: OCR asset has no roi")
                                        })?;
                                        nearest_ocr_line(&candidates, rect_centre(roi))
                                    }
                                    _ => candidates.iter().copied().max_by(|a, b| {
                                        a.score
                                            .partial_cmp(&b.score)
                                            .unwrap_or(std::cmp::Ordering::Equal)
                                    }),
                                };
                                let region_info = region_log(region.as_ref());
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
                                        let centre = Point {
                                            x: line.rect.x + line.rect.w / 2,
                                            y: line.rect.y + line.rect.h / 2,
                                        };
                                        // 2026-11-06 批次B (#4/Q1): the recognized
                                        // line is the real rect; the operation box
                                        // is the asset's own self geometry, whose
                                        // stored `action_shape` follows this delta.
                                        self.last_ocr_rect = Some(line.rect);
                                        let (self_box, self_shape) = self
                                            .ocr_self_operation_geometry(
                                                sel,
                                                line.rect,
                                                Some((w, h)),
                                            )?;
                                        // v1.4 A-plan: a whole-library `region.asset`
                                        // replaces the operation geometry; the param OCR
                                        // line above still had to match.
                                        let asset_target = self.region_asset_target(
                                            region,
                                            centre,
                                            &format!(
                                                "click_target args.find[{idx}].region.asset"
                                            ),
                                        )?;
                                        // B-02: an explicit operation region
                                        // (action_shape / click_back) must win over
                                        // the recognized line box, including invert.
                                        let default_target = self_box;
                                        // 2026-11-06 参数→素材: an asset-level
                                        // `op_region` chain (only when the flow
                                        // carries no legacy `region`: flow region
                                        // keeps priority) replaces the geometry.
                                        let chain = if region.is_none() {
                                            self.resolve_asset_op_region(
                                                sel,
                                                line.rect,
                                                default_target,
                                                self_shape.clone(),
                                                "click_target ocr_text",
                                            )?
                                        } else {
                                            None
                                        };
                                        // v1.3 (OPERATION-REGION-DESIGN §6.1): an
                                        // explicit `region` overrides the asset
                                        // operation area; `mode=asset` keeps it.
                                        let (target, explicit_shape, shape_aware) =
                                            if let Some(r) = &chain {
                                                (r.rect, r.shape.clone(), false)
                                            } else if let Some((rect, shape)) = asset_target {
                                                (rect, shape, false)
                                            } else {
                                                let (t, _aware) = match region {
                                                    None | Some(OpRegion::Asset { .. }) => {
                                                        (default_target, true)
                                                    }
                                                    Some(OpRegion::Ocr)
                                                    | Some(OpRegion::Nearest { .. }) => {
                                                        (line.rect, false)
                                                    }
                                                    Some(OpRegion::Offset { dx, dy, w, h }) => (
                                                        offset_region_rect(
                                                            line.rect, *dx, *dy, *w, *h,
                                                        ),
                                                        false,
                                                    ),
                                                };
                                                (t, self_shape.clone(), false)
                                            };
                                        let plan =
                                            self.humanize.template_click(target, w, &mut self.rng);
                                        let plan = if explicit_shape.is_some() {
                                            self.adjust_plan_for_explicit_shape(
                                                plan,
                                                explicit_shape.as_ref(),
                                                w,
                                                h,
                                            )?
                                        } else if shape_aware {
                                            self.adjust_plan_for_shape(sel, plan, w, h, centre)?
                                        } else {
                                            plan
                                        };
                                        let centre = Point {
                                            x: target.x + target.w / 2,
                                            y: target.y + target.h / 2,
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
                                        let chain_info = chain
                                            .as_ref()
                                            .map(|c| c.log_suffix())
                                            .unwrap_or_default();
                                        return Ok(format!(
                                            "clicked_target locator=ocr_text sel={sel} text={:?} score={:.3} point=({},{}){miss} verified={verified} guard={guarded}{region_info}{chain_info}",
                                            line.text, line.score, actual.x, actual.y
                                        ));
                                    }
                                    None => {
                                        // Distinguish "wrong text" from
                                        // "right text, too low confidence" so a
                                        // declared OCR threshold is debuggable.
                                        let matched_best = lines
                                            .iter()
                                            .filter(|l| ocr_text_matches(text, l.text.as_str()))
                                            .map(|l| l.score)
                                            .fold(None::<f32>, |acc, s| {
                                                Some(acc.map_or(s, |a| a.max(s)))
                                            });
                                        match (min, matched_best) {
                                            (Some(m), Some(s)) => evidence.push(format!(
                                                "ocr_text `{sel}`: line matched but best score \
                                                 {s:.3} < {m}"
                                            )),
                                            _ => evidence.push(format!(
                                                "ocr_text `{sel}`: no line equals {:?} ({} lines found)",
                                                text,
                                                lines.len()
                                            )),
                                        }
                                    }
                                }
                            }
                            Ok(OcrRead::Single(Some(t), score)) => {
                                if ocr_text_matches_strict(text, &t) && !self.ocr_score_ok(sel, score) {
                                    let min = self.assets.ocr_threshold(sel).unwrap_or(0.0);
                                    evidence.push(format!(
                                        "ocr_text `{sel}`: text {t:?} matched but score \
                                         {score:.3} < {min}"
                                    ));
                                    continue;
                                }
                                if ocr_text_matches_strict(text, &t) {
                                    // 2026-11-06 参数→素材: an `ocr_var` may have no
                                    // ROI (whole-screen OCR); its real recognized
                                    // line bbox (`last_ocr_rect`, set by
                                    // `recognize_best`) is then the operation basis.
                                    let recognition = if let Some(r) = self.assets.ocr_roi(sel) {
                                        r
                                    } else if let Some(r) =
                                        self.assets.ocr_var(sel).and_then(|v| v.roi)
                                    {
                                        r
                                    } else if let Some(r) = self.last_ocr_rect {
                                        r
                                    } else {
                                        return Err(format!(
                                            "ocr_text `{sel}`: OCR asset has no roi"
                                        ));
                                    };
                                    // 2026-11-06 批次B (#4/Q1): the operation box
                                    // is the asset's own self geometry; with an
                                    // authored ROI the stored `action_shape`
                                    // follows `real - roi` (templates translate
                                    // the same way). `roi` absent = absolute.
                                    let real = self.last_ocr_rect.unwrap_or(recognition);
                                    let (self_box, self_shape) = self
                                        .ocr_self_operation_geometry(sel, real, Some((w, h)))?;
                                    let default_target = self_box;
                                    // v1.3 region override (single-line OCR has no
                                    // per-line bbox: the recognition ROI IS its box).
                                    // v1.4 A-plan: `region.asset` takes priority.
                                    let asset_target = self.region_asset_target(
                                        region,
                                        rect_centre(recognition),
                                        &format!("click_target args.find[{idx}].region.asset"),
                                    )?;
                                    let marker = Point {
                                        x: recognition.x,
                                        y: recognition.y,
                                    };
                                    // 2026-11-06 参数→素材: an asset-level
                                    // `op_region` chain applies only without a
                                    // legacy flow `region` (flow region wins).
                                    let chain = if region.is_none()
                                        && self.assets.has_op_region_chain(sel)
                                    {
                                        self.resolve_asset_op_region(
                                            sel,
                                            real,
                                            default_target,
                                            self_shape.clone(),
                                            "click_target ocr_text",
                                        )?
                                    } else {
                                        None
                                    };
                                    let (target, explicit_shape, shape_aware) =
                                        if let Some(r) = &chain {
                                            (r.rect, r.shape.clone(), false)
                                        } else if let Some((rect, shape)) = asset_target {
                                            (rect, shape, false)
                                        } else {
                                            let (t, _aware) = match region {
                                                None | Some(OpRegion::Asset { .. }) => {
                                                    (default_target, true)
                                                }
                                                Some(OpRegion::Ocr)
                                                | Some(OpRegion::Nearest { .. }) => {
                                                    (recognition, false)
                                                }
                                                Some(OpRegion::Offset { dx, dy, w, h }) => (
                                                    offset_region_rect(
                                                        recognition, *dx, *dy, *w, *h,
                                                    ),
                                                    false,
                                                ),
                                            };
                                            (t, self_shape.clone(), false)
                                        };
                                    let region_info = region_log(region.as_ref());
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
                                    let plan = self.humanize.template_click(target, w, &mut self.rng);
                                    let plan = if explicit_shape.is_some() {
                                        self.adjust_plan_for_explicit_shape(
                                            plan,
                                            explicit_shape.as_ref(),
                                            w,
                                            h,
                                        )?
                                    } else if shape_aware {
                                        self.adjust_plan_for_shape(sel, plan, w, h, marker)?
                                    } else {
                                        plan
                                    };
                                    let centre = Point {
                                        x: target.x + target.w / 2,
                                        y: target.y + target.h / 2,
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
                                    let chain_info = chain
                                        .as_ref()
                                        .map(|c| c.log_suffix())
                                        .unwrap_or_default();
                                    return Ok(format!(
                                        "clicked_target locator=ocr_text sel={sel} text={:?} score={score:.3} point=({},{}){miss} verified={verified} guard={guarded}{region_info}{chain_info}",
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
                            require_template,
                            region,
                            ..
                        } => {
                            let rect = oas_types::Rect {
                                x: roi[0],
                                y: roi[1],
                                w: roi[2],
                                h: roi[3],
                            };
                            let region_info = region_log(region.as_ref());
                            match self.ocr_read_rect(rect, true) {
                                Ok(OcrRead::Lines(lines)) => {
                                    // v1.3 `region`: shared candidate selection;
                                    // `nearest` uses the inline ROI centre.
                                    let candidates = ocr_line_candidates(&lines, text, None);
                                    let hit = match region {
                                        Some(OpRegion::Nearest { .. }) => {
                                            nearest_ocr_line(&candidates, rect_centre(rect))
                                        }
                                        _ => candidates.iter().copied().max_by(|a, b| {
                                            a.score
                                                .partial_cmp(&b.score)
                                                .unwrap_or(std::cmp::Ordering::Equal)
                                        }),
                                    };
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
                                            // v1.4 A-plan: a whole-library
                                            // `region.asset` replaces the geometry.
                                            let asset_target = self.region_asset_target(
                                                region,
                                                rect_centre(line.rect),
                                                &format!(
                                                    "click_target args.find[{idx}].region.asset"
                                                ),
                                            )?;
                                            let (target, explicit_shape) =
                                                if let Some((rect, shape)) = asset_target {
                                                    (rect, shape)
                                                } else {
                                                    let target = match region {
                                                        None
                                                        | Some(OpRegion::Asset { .. })
                                                        | Some(OpRegion::Ocr)
                                                        | Some(OpRegion::Nearest { .. }) => {
                                                            line.rect
                                                        }
                                                        Some(OpRegion::Offset { dx, dy, w, h }) => {
                                                            offset_region_rect(
                                                                line.rect, *dx, *dy, *w, *h,
                                                            )
                                                        }
                                                    };
                                                    (target, None)
                                                };
                                            let plan = self
                                                .humanize
                                                .template_click(target, w, &mut self.rng);
                                            let plan = self.adjust_plan_for_explicit_shape(
                                                plan,
                                                explicit_shape.as_ref(),
                                                w,
                                                h,
                                            )?;
                                            let centre = Point {
                                                x: target.x + target.w / 2,
                                                y: target.y + target.h / 2,
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
                                                 score={:.3} point=({},{}){miss} verified={verified} guard={guarded}{region_info}",
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
                                    if ocr_text_matches_strict(text, &t) {
                                        // v1.4 A-plan: inline single-line reference =
                                        // the inline ROI centre.
                                        let asset_target = self.region_asset_target(
                                            region,
                                            rect_centre(rect),
                                            &format!(
                                                "click_target args.find[{idx}].region.asset"
                                            ),
                                        )?;
                                        let target = match asset_target {
                                            Some((rect, _shape)) => rect,
                                            None => match region {
                                                Some(OpRegion::Offset { dx, dy, w, h }) => {
                                                    offset_region_rect(rect, *dx, *dy, *w, *h)
                                                }
                                                _ => rect,
                                            },
                                        };
                                        let plan =
                                            self.humanize.template_click(target, w, &mut self.rng);
                                        let centre = Point {
                                            x: target.x + target.w / 2,
                                            y: target.y + target.h / 2,
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
                                             score={score:.3} point=({},{}){miss} verified={verified} guard={guarded}{region_info}",
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
                            self.observe_screen()?;
                            let (fw, fh) = self
                                .screen.size()
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
                            let exact_shape = self.assets.click_shape(sel).cloned();
                            self.observe_screen()?;
                            let (fw, fh) = self
                                .screen.size()
                                .ok_or_else(|| "region click: no frame available".to_string())?;
                            if exact_shape.is_none()
                                && (rect.x < 0
                                    || rect.y < 0
                                    || rect.x + rect.w > fw as i32
                                    || rect.y + rect.h > fh as i32)
                            {
                                evidence.push(format!(
                                    "region `{sel}` rect ({},{},{},{}) is outside the frame ({}x{})",
                                    rect.x, rect.y, rect.w, rect.h, fw, fh
                                ));
                                continue;
                            }
                            let p = match exact_shape.as_ref() {
                                Some(shape) => shape.sample(fw, fh, &mut self.rng).map_err(|e| {
                                    format!("region `{sel}` exact shape: {e}")
                                })?,
                                None => self.random_point_in(rect, 0.15),
                            };
                            // B-04: `point_click` adds ±jitter_px (and a deliberate
                            // miss can land much farther). A narrow region would
                            // otherwise click outside its shape. Re-sample until
                            // the jittered point is still accepted; fall back to
                            // the un-jittered sampled point (never outside).
                            let mut plan = self.humanize.point_click(p, &mut self.rng);
                            if let Some(shape) = exact_shape.as_ref() {
                                if !self.click_shape_accepts(shape, plan.point, fw, fh) {
                                    let mut accepted = None;
                                    for _ in 0..8 {
                                        let q = shape.sample(fw, fh, &mut self.rng)?;
                                        let candidate = self.humanize.point_click(q, &mut self.rng);
                                        if self.click_shape_accepts(shape, candidate.point, fw, fh) {
                                            accepted = Some(candidate);
                                            break;
                                        }
                                    }
                                    plan = accepted.unwrap_or(crate::humanize::ClickPlan {
                                        point: p,
                                        miss: false,
                                    });
                                }
                            }
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
            // v1.3 long press (docs/OPERATION-REGION-DESIGN.md §6.1): same
            // locator chain / guard / verification as `click_target`, but the
            // action is a hold. `hold_range` (humanized) wins over `hold_ms`.
            Step::LongPress {
                find,
                hold_ms,
                hold_range,
                requires,
                expect,
            } => {
                self.reobserve_screen()?;
                // Same `text_expr` contract as click_target: resolve every
                // locator text ONCE at step entry; a bad value aborts before any
                // gesture (it must never fall through to a lower-priority kind).
                let mut resolved_text: Vec<Option<String>> = Vec::with_capacity(find.len());
                for (j, loc) in find.iter().enumerate() {
                    resolved_text.push(match loc.text_spec() {
                        None => None,
                        Some(spec) => Some(self.resolve_text(
                            spec,
                            &format!("long_press args.find[{j}].text_expr"),
                        )?),
                    });
                }
                let guarded = self.require_guard(requires.as_ref())?;
                if self.screen.size().is_none() {
                    self.observe_screen()?;
                }
                let (w, h) = self
                    .screen
                    .size()
                    .ok_or_else(|| "long_press: no frame available".to_string())?;
                // Hold duration: `hold_range` (random humanized) wins over the
                // fixed `hold_ms`. Both sides are 100..=10000 (parser-enforced).
                let ms = match hold_range {
                    Some([lo, hi]) => {
                        let span = hi.saturating_sub(*lo) as f64;
                        let picked = *lo
                            + (self.rng.unit_f64() * (span + 1.0)).floor() as u32;
                        picked.min(*hi)
                    }
                    None => *hold_ms,
                };
                let target = self
                    .resolve_press_target(find, &resolved_text, w, h)
                    .map_err(|e| format!("long_press: {e}"))?;
                let actual = self.perform_long_press(target.point, ms, w, h)?;
                let verified = self.verify_if_present(expect.as_ref())?;
                let region_info = region_log(target.region.as_ref());
                let sel_info = target
                    .sel
                    .as_deref()
                    .map(|s| format!(" sel={s}"))
                    .unwrap_or_default();
                return Ok(format!(
                    "held locator={}{sel_info} ms={ms} point=({},{}) frame={}x{} \
                     verified={verified} guard={guarded} match={}{region_info}",
                    target.locator,
                    actual.x,
                    actual.y,
                    w,
                    h,
                    target.match_info
                ));
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
                self.observe_screen()?;
                let (w, h) = self
                    .screen.size()
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
            // v1.3 multi-segment curve (docs/OPERATION-REGION-DESIGN.md §6.1):
            // N points → N-1 independent `input swipe` calls. Every segment must
            // stay on-screen; the total duration is randomized and split evenly
            // (>= 1ms/segment). NOT one continuous gesture by design.
            Step::CurveSwipe {
                points,
                duration_ms,
            } => {
                self.observe_screen()?;
                let (w, h) = self
                    .screen
                    .size()
                    .ok_or_else(|| "curve_swipe: no frame available".to_string())?;
                if points.len() < 2 {
                    return Err("curve_swipe needs at least 2 points".to_string());
                }
                for (i, p) in points.iter().enumerate() {
                    if p[0] < 0 || p[1] < 0 || p[0] >= w as i32 || p[1] >= h as i32 {
                        return Err(format!(
                            "curve_swipe point[{i}]=({},{}) is outside the frame ({}x{})",
                            p[0], p[1], w, h
                        ));
                    }
                }
                let span = duration_ms[1].saturating_sub(duration_ms[0]) as f64;
                let total = (duration_ms[0] as f64
                    + (self.rng.unit_f64() * (span + 1.0)).floor())
                .min(duration_ms[1] as f64) as u32;
                let segments = points.len() - 1;
                let seg_ms = (total / segments as u32).max(1);
                for i in 0..segments {
                    let from = Point {
                        x: points[i][0],
                        y: points[i][1],
                    };
                    let to = Point {
                        x: points[i + 1][0],
                        y: points[i + 1][1],
                    };
                    self.input.swipe(from, to, seg_ms)?;
                }
                self.invalidate_frame();
                Ok(format!(
                    "curve_swipe points={} segments={} total_ms={} seg_ms={} frame={}x{} \
                     (each segment is an independent swipe gesture)",
                    points.len(),
                    segments,
                    seg_ms.saturating_mul(segments as u32),
                    seg_ms,
                    w,
                    h
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
                self.reobserve_screen()?;
                let src = self.drag_target(from, "drag args.from")?;
                self.observe_screen()?;
                let (w, h) = self
                    .screen.size()
                    .ok_or_else(|| "drag: no frame available".to_string())?;
                let start = self
                    .sample_drag_point(&src, *margin, w, h)
                    .map_err(|e| format!("drag from target: {e}"))?;
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
                let mut dst: Option<DragEndpoint> = None;
                let end = if let Some(target) = to {
                    let ep = self.drag_target(target, "drag args.to")?;
                    // Random end inside the target region, but only accept it when
                    // it really lies in `direction` (v1.1 方向性要求). With an
                    // exact shape the candidate already lies inside the shape, so
                    // only the direction/distance filter can reject it.
                    let mut picked = None;
                    for _ in 0..24 {
                        let p = self
                            .sample_drag_point(&ep, *margin, w, h)
                            .map_err(|e| format!("drag to target: {e}"))?;
                        let dx = p.x - start.x;
                        let dy = p.y - start.y;
                        let dot = dx * ux + dy * uy;
                        let dist2 = dx * dx + dy * dy;
                        if dot > 0 && dist2 >= 20 * 20 && inside(p) {
                            picked = Some(p);
                            break;
                        }
                    }
                    let target_end = picked.ok_or_else(|| {
                        format!(
                            "drag target in direction `{}` is unreachable from ({},{}) (no point with positive {}-ward distance, target shape={}{})",
                            direction.as_str(),
                            start.x,
                            start.y,
                            direction.as_str(),
                            ep.shape_label(),
                            if ep.invert() { ", invert=true" } else { "" }
                        )
                    })?;
                    dst = Some(ep);
                    target_end
                } else {
                    // P2-3 (documented semantics, intentionally unchanged):
                    // without `to`, the end point is *derived* from
                    // `start + direction × random distance`. `from.shape`
                    // constrains the sampled START only; the derived end is
                    // constrained by the frame, not by the shape. With an
                    // invert shape this can re-enter the excluded area (e.g.
                    // the only free row drags up). Do NOT clamp it to the
                    // shape: distance mode is deliberately path-free.
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
                // v1.2.5 (O-2): record which endpoints used an exact shape so a
                // flow can be audited; polygon/invert sampling ignores `margin`
                // (the pure-bbox detail stays byte-identical).
                let margin_ignored = src.ignores_margin()
                    || dst.as_ref().is_some_and(DragEndpoint::ignores_margin);
                Ok(format!(
                    "dragged direction={} from=({},{}) to=({},{}) ms={} randomized=true frame={}x{} from_shape={} from_invert={} to_shape={} to_invert={}{}",
                    direction.as_str(),
                    start.x,
                    start.y,
                    end.x,
                    end.y,
                    ms,
                    w,
                    h,
                    src.shape_label(),
                    src.invert(),
                    dst.as_ref().map_or("none", DragEndpoint::shape_label),
                    dst.as_ref().is_some_and(DragEndpoint::invert),
                    if margin_ignored { " margin=ignored" } else { "" }
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
                // Leftover 3: a value assignment must honour the asset's
                // explicit minimum confidence too (below it = loud failure,
                // never a variable filled with low-confidence text).
                let text = match self.ocr_value_read(sel)? {
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
                self.reobserve_screen()?;
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
            // Runner Studio split §C.4.3: a parameter scope generated by the
            // task loader around an expanded `call_task` (advanced flows may
            // author it directly).
            //
            // Resolution order (§C.4.4, high -> low):
            // 1. call-site `params` mapping evaluated in the CALLER scope,
            // 2. same-name inheritance from the caller (`inherit:"all"`),
            // 3. subtask `meta.params[].default` (`defaults`),
            // 4. loud failure naming the subtask.
            //
            // The caller's parameter table is restored on success AND failure;
            // `break`/`continue`/`finish` in the body keep propagating through
            // the shared `ctl` unchanged (the target's `finish` steps are
            // stripped by the loader, so a subtask cannot end the parent flow).
            Step::WithParams {
                label,
                params,
                defaults,
                block,
                body,
            } => {
                let snapshot = self.params.clone();
                // 2) Evaluate every mapping against the CALLER scope first: a
                // mapping may reference a parent key, never a previous entry of
                // the same mapping object.
                let mut overrides: BTreeMap<String, Value> = BTreeMap::new();
                let mut eval_error: Option<String> = None;
                for (key, expr) in params {
                    match eval(expr, self) {
                        Ok(value) => {
                            overrides.insert(key.clone(), value);
                        }
                        Err(e) => {
                            eval_error = Some(e);
                            break;
                        }
                    }
                }
                if let Some(e) = eval_error {
                    self.params = snapshot;
                    return Err(e);
                }
                // 3) `inherit:"none"` hides the caller's same-name values; the
                // snapshot restores them afterwards (also for nested scopes).
                for key in block {
                    self.params.remove(key);
                }
                for (key, value) in defaults {
                    if !self.params.contains_key(key) && !overrides.contains_key(key) {
                        self.params.insert(key.clone(), value.clone());
                    }
                }
                // 1) Explicit mappings win over both inheritance and defaults.
                for (key, value) in overrides {
                    self.params.insert(key, value);
                }
                if let Some(label) = label {
                    self.subtask_stack.push(label.clone());
                }
                let result = self.exec(body, depth + 1, ctl);
                if label.is_some() {
                    self.subtask_stack.pop();
                }
                self.params = snapshot;
                result.map(|_| match label {
                    Some(label) => {
                        format!("subtask `{label}` ({} step(s))", body.len())
                    }
                    None => format!("scope ({} step(s))", body.len()),
                })
            }
            Step::CallTask { task, .. } => Err(format!(
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
        Step::Key { .. } => "key",
        Step::RestartApp { .. } => "restart_app",
        Step::WaitUntil { .. } => "wait_until",
        Step::Appear { .. } => "appear",
        Step::Click { .. } => "click",
        Step::ClickPoint { .. } => "click_point",
        Step::Swipe { .. } => "swipe",
        Step::Drag { .. } => "drag",
        Step::LongPress { .. } => "long_press",
        Step::CurveSwipe { .. } => "curve_swipe",
        Step::Ocr { .. } => "ocr",
        Step::SetVar { .. } => "set_var",
        Step::IncVar { .. } => "inc_var",
        Step::OcrToVar { .. } => "ocr_to_var",
        Step::DismissPopup { .. } => "dismiss_popup",
        Step::ClickTarget { .. } => "click_target",
        Step::CallTask { .. } => "call_task",
        Step::WithParams { .. } => "with_params",
        Step::Log { .. } => "log",
        Step::Finish { .. } => "finish",
    }
}

impl<S: ScreenSource, I: InputSink> EvalContext for Interpreter<S, I> {
    fn appear(&mut self, sel: &str) -> Result<bool, String> {
        // "出现素材" covers every asset kind (user 10-31): OCR assets are
        // visible when their text can be read; template/collection assets use
        // the normal locate path.
        let result = if self.assets.is_ocr_asset(sel) {
            if self.ocr.is_none() {
                Err(format!(
                    "OCR is not available for appear({sel}): no OCR provider attached"
                ))
            } else {
                match self.ocr_read(sel) {
                    Ok(OcrRead::Single(text, score)) => Ok(text
                        .map(|t| !t.trim().is_empty())
                        .unwrap_or(false)
                        && self.ocr_score_ok(sel, score)),
                    Ok(OcrRead::Lines(lines)) => {
                        // Presence uses the best line (the same score
                        // `recognize_best` reports); an explicit OCR threshold must
                        // be met by that best line.
                        let best = lines
                            .iter()
                            .map(|l| l.score)
                            .fold(f32::MIN, f32::max);
                        Ok(!lines.is_empty() && self.ocr_score_ok(sel, best))
                    }
                    // A multi-line-derived line that is not found means "not visible".
                    Err(e) if e.starts_with("derived ocr_line") => Ok(false),
                    Err(e) => Err(e),
                }
            }
        } else {
            match self.locate(sel) {
                Ok((score, _)) => Ok(score >= self.assets.threshold_for(sel).unwrap_or(0.9)),
                Err(e) => Err(e),
            }
        };
        if self.trace_appear {
            let detail = match &result {
                Ok(v) => format!("sel={sel} -> {v} frame_seq={}", self.screen.seq()),
                Err(e) => format!("sel={sel} -> ERROR {e:?} frame_seq={}", self.screen.seq()),
            };
            self.log(
                0,
                "appear_trace",
                &detail,
                if result.is_ok() { "ok" } else { "failed" },
                0.0,
            );
        }
        result
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
        self.observe_screen()?;
        let frame = self
            .screen.frame
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
        Ok(lines.iter().any(|l| ocr_text_matches(target, l.text.as_str())))
    }

    fn param(&mut self, key: &str) -> Result<Value, String> {
        if let Some(value) = self.params.get(key) {
            return Ok(value.clone());
        }
        // §C.4.4 #4: inside a subtask the historical message would tell the user
        // to change the PARENT's CLI parameters, which is misleading. Name the
        // subtask and point at the two real fixes (declaration or mapping).
        Err(match self.subtask_stack.last() {
            Some(label) => format!(
                "param('{key}') has no value in subtask '{label}': declare it in that \
                 task's meta.params (with a default), map it with call_task.params, \
                 or pass --param {key}=<value>"
            ),
            None => format!(
                "param('{key}') has no value: declare it in meta.params (with a default) \
                 or pass --param {key}=<value>"
            ),
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
        /// v1.3 long-press gestures, kept distinct from swipes so a test can
        /// prove the interpreter called `long_press` (and not a click/swipe).
        long_presses: std::rc::Rc<std::cell::RefCell<Vec<(Point, u32)>>>,
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
        fn long_press(&mut self, p: Point, ms: u32) -> Result<(), String> {
            self.shared.long_presses.borrow_mut().push((p, ms));
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

    /// det+rec provider that records the FRAME it was handed (not only the ROI),
    /// so a test can prove the *derived* OCR path applied the polygon mask before
    /// calling the engine — a ROI-only assertion cannot catch an unmasked frame.
    struct FrameProbeDetOcr {
        lines: Vec<OcrLine>,
        rois: std::rc::Rc<std::cell::RefCell<Vec<Rect>>>,
        frames: std::rc::Rc<std::cell::RefCell<Vec<Frame>>>,
    }

    impl FrameProbeDetOcr {
        fn new(
            lines: Vec<OcrLine>,
        ) -> (
            Self,
            std::rc::Rc<std::cell::RefCell<Vec<Rect>>>,
            std::rc::Rc<std::cell::RefCell<Vec<Frame>>>,
        ) {
            let rois = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
            let frames = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
            (
                Self {
                    lines,
                    rois: rois.clone(),
                    frames: frames.clone(),
                },
                rois,
                frames,
            )
        }
    }

    impl OcrProvider for FrameProbeDetOcr {
        fn recognize(&mut self, _frame: &Frame, _roi: Rect) -> Result<(String, f32), String> {
            Err("FrameProbeDetOcr only supports det+rec".into())
        }

        fn recognize_lines(&mut self, frame: &Frame, roi: Rect) -> Result<Vec<OcrLine>, String> {
            self.rois.borrow_mut().push(roi);
            self.frames.borrow_mut().push(frame.clone());
            Ok(self.lines.clone())
        }
    }

    #[test]
    fn screen_cache_shares_one_observation_until_invalidated() {
        let mut src = FakeScreen::new();
        let mut cache = ScreenCache::new();
        cache.ensure(&mut src).expect("first observation");
        assert_eq!(src.grabs, 1);
        let first_seq = cache.seq();
        // Multiple selectors inside one condition must reuse the same pixels.
        cache.ensure(&mut src).expect("same observation");
        assert_eq!(src.grabs, 1);
        assert_eq!(cache.seq(), first_seq);
        // Any screen-changing action invalidates; next read grabs a new frame.
        cache.invalidate();
        assert!(cache.frame.is_none());
        cache.ensure(&mut src).expect("fresh observation");
        assert_eq!(src.grabs, 2);
        assert!(cache.seq() > first_seq);
        // Explicit re-observation always grabs.
        cache.refresh(&mut src).expect("forced observation");
        assert_eq!(src.grabs, 3);
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

    /// Interpreter over **caller-provided** assets + a scripted [`FakeOcr`],
    /// returning the recorded clicks too. Used by the OCR-threshold tests, which
    /// must attach `threshold` to an OCR asset before the run starts.
    fn mk_ocr_with(
        assets: AssetSet,
        replies: &[(&str, f32)],
    ) -> (OcrInterp, OcrRois, SharedInput) {
        let (input, shared) = FakeInput::new();
        let (fake, rois) = FakeOcr::new(replies);
        let interp = Interpreter::new(FakeScreen::new(), input, assets, Budget::default())
            .with_humanize(no_miss())
            .with_ocr(Box::new(fake))
            .with_seed(5);
        (interp, rois, shared)
    }

    /// Same as [`mk_ocr_with`] but with a det+rec provider (`ocr_lines`).
    fn mk_det_ocr_with(assets: AssetSet, lines: Vec<OcrLine>) -> (OcrInterp, SharedInput) {
        let (input, shared) = FakeInput::new();
        let (fake, _rois) = FakeDetOcr::new(lines);
        let interp = Interpreter::new(FakeScreen::new(), input, assets, Budget::default())
            .with_humanize(no_miss())
            .with_ocr(Box::new(fake))
            .with_seed(5);
        (interp, shared)
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

    // ---- seq132: mixed collections (template / OCR / ocr_var / region) ----

    fn mixed_collection_assets() -> AssetSet {
        let mut set = assets();
        set.insert_ocr_var(
            "O_GROUP",
            Some(Rect {
                x: 0,
                y: 0,
                w: 64,
                h: 48,
            }),
            "御魂".to_string(),
            "group".to_string(),
        );
        set.insert_region(
            "R_MIX",
            Rect {
                x: 40,
                y: 4,
                w: 10,
                h: 10,
            },
        );
        set.insert_collection(
            "C_MIX",
            vec![
                "I_NEVER".to_string(),
                "O_GROUP".to_string(),
                "R_MIX".to_string(),
                "I_BRIGHT".to_string(),
            ],
            None,
        );
        set
    }

    fn mixed_collection_line() -> OcrLine {
        chain_line(20, 20, 10, 5, "御魂", 0.97)
    }

    fn mk_mixed_collection_interp() -> (Interpreter<FakeScreen, FakeInput>, SharedInput) {
        let (ocr, _rois) = FakeDetOcr::new(vec![mixed_collection_line()]);
        let (input, shared) = FakeInput::new();
        let interp = Interpreter::new(FakeScreen::new(), input, mixed_collection_assets(), Budget::default())
            .with_humanize(no_miss())
            .with_ocr(Box::new(ocr))
            .with_seed(3);
        (interp, shared)
    }

    #[test]
    fn mixed_collection_locate_records_the_region_member() {
        let mut interp = Interpreter::new(
            FakeScreen::new(),
            FakeInput::new().0,
            mixed_collection_assets(),
            Budget::default(),
        )
        .with_humanize(no_miss());
        // No OCR provider here: the region member always wins (score 1.0).
        let (score, _at) = interp.locate("C_MIX").expect("locate mixed");
        assert_eq!(score, 1.0);
        assert_eq!(
            interp.last_set_member.get("C_MIX").map(String::as_str),
            Some("R_MIX"),
            "region member must be the recorded winner"
        );
    }

    #[test]
    fn click_on_mixed_collection_region_member_clicks_its_roi() {
        let mut set = assets();
        set.insert_region(
            "R_MIX",
            Rect {
                x: 30,
                y: 20,
                w: 10,
                h: 10,
            },
        );
        set.insert_collection(
            "C_MIX",
            vec!["I_NEVER".to_string(), "R_MIX".to_string()],
            None,
        );
        let (input, shared) = FakeInput::new();
        let mut interp = Interpreter::new(FakeScreen::new(), input, set, Budget::default())
            .with_humanize(no_miss());
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click","args":{"sel":"C_MIX"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let p = shared.clicks.borrow()[0];
        assert!(
            inside(
                Rect {
                    x: 30,
                    y: 20,
                    w: 10,
                    h: 10
                },
                p
            ),
            "region member click must land in its ROI, got {p:?}"
        );
    }

    #[test]
    fn click_on_mixed_collection_ocr_member_uses_q1_geometry() {
        let mut set = assets();
        set.insert_ocr_var("O_GROUP", None, "御魂".to_string(), "group".to_string());
        set.insert_collection(
            "C_MIX",
            vec!["I_NEVER".to_string(), "O_GROUP".to_string()],
            None,
        );
        let (mut interp, _rois, shared) = {
            let (ocr, rois) = FakeDetOcr::new(vec![mixed_collection_line()]);
            let (input, shared) = FakeInput::new();
            let interp = Interpreter::new(FakeScreen::new(), input, set, Budget::default())
                .with_humanize(no_miss())
                .with_ocr(Box::new(ocr))
                .with_seed(5);
            (interp, rois, shared)
        };
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click","args":{"sel":"C_MIX"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let p = shared.clicks.borrow()[0];
        let line = Rect {
            x: 20,
            y: 20,
            w: 10,
            h: 5,
        };
        assert!(
            inside(line, p),
            "OCR member click must use the recognized line rect {line:?}, got {p:?}"
        );
    }

    #[test]
    fn mixed_collection_candidates_enumerate_every_member_kind() {
        let (mut interp, _shared) = mk_mixed_collection_interp();
        let candidates = interp.collection_candidates("C_MIX", true).expect("mixed candidates");
        let members: Vec<&str> = candidates.iter().map(|c| c.member.as_str()).collect();
        assert!(members.contains(&"I_BRIGHT"), "{members:?}");
        assert!(members.contains(&"O_GROUP"), "{members:?}");
        assert!(members.contains(&"R_MIX"), "{members:?}");
        // Stable order: score desc → y asc → x asc.
        for pair in candidates.windows(2) {
            let (a, b) = (&pair[0], &pair[1]);
            assert!(
                a.score > b.score || (a.score == b.score && a.y <= b.y),
                "candidates out of order: {candidates:?}"
            );
        }
        let region = candidates.iter().find(|c| c.member == "R_MIX").expect("region candidate");
        assert_eq!(region.score, 1.0);
        assert_eq!(
            (region.x, region.y, region.w, region.h),
            (40, 4, 10, 10)
        );
        let ocr = candidates.iter().find(|c| c.member == "O_GROUP").expect("ocr candidate");
        assert!((ocr.score - 0.97).abs() < 1e-6, "{ocr:?}");
        assert_eq!(
            (ocr.action_box.x, ocr.action_box.y, ocr.action_box.w, ocr.action_box.h),
            (20, 20, 10, 5),
            "OCR candidate operation box must be the recognized line (Q1)"
        );
    }

    #[test]
    fn preview_mixed_collection_candidate_index_matches_the_candidate_list() {
        let (mut interp, _shared) = mk_mixed_collection_interp();
        let candidates = interp.collection_candidates("C_MIX", true).expect("candidates");
        for idx in [0usize, 1] {
            let preview = interp
                .preview_asset_action("C_MIX", Some(idx))
                .expect("mixed preview");
            let want = &candidates[idx];
            assert_eq!(
                preview.steps[0].asset, want.member,
                "candidate_index {idx} must select the same member"
            );
            assert_eq!(preview.action_box, want.action_box);
            assert!(inside(preview.action_box, preview.point));
        }
        assert!(
            interp.preview_asset_action("C_MIX", Some(candidates.len())).is_err(),
            "out-of-range candidate_index must be a loud error"
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
    fn ocr_text_matches_variant_glyphs_but_not_unrelated_text() {
        assert!(ocr_text_matches("召唤", "召喚"));
        assert!(ocr_text_matches("图鉴", "图鑑"));
        assert!(ocr_text_matches(" 召 唤 ", "召喚"));
        assert!(!ocr_text_matches("召唤", "取消"));
        assert!(!ocr_text_matches("确定", "确定按钮"));
    }

    #[test]
    fn ocr_text_matches_protects_ascii_digits() {
        // #11: the old 1-edit tolerance made same-length digit variants match.
        assert!(!ocr_text_matches("队伍1", "队伍2"));
        assert!(!ocr_text_matches("队伍1", "队伍3"));
        assert!(!ocr_text_matches("队伍1", "队伍12"));
        assert!(!ocr_text_matches("队伍12", "队伍1"));
        assert!(!ocr_text_matches("队伍2", "队伍1"));
        // Same digits: equality + the historical CJK/noise tolerance still work.
        assert!(ocr_text_matches("队伍1", "队伍1"));
        assert!(ocr_text_matches("队伍 1", "队伍1"));
        assert!(ocr_text_matches("队伍1", "队伍1 "));
        assert!(ocr_text_matches("队伍1号", "队伍1 号"));
        // Full-width digits are folded to ASCII before the digit guard.
        assert!(ocr_text_matches("队伍１", "队伍1"));
        assert!(!ocr_text_matches("队伍１", "队伍2"));
        // Multiple digit groups are compared in order.
        assert!(ocr_text_matches("第1队第2层", "第1队第2层"));
        assert!(!ocr_text_matches("第1队第2层", "第1队第3层"));
        assert!(!ocr_text_matches("第1队第2层", "第2队第1层"));
        // Pure text (no digits on either side) behavior is unchanged.
        assert!(ocr_text_matches("召唤", "召喚"));
        assert!(ocr_text_matches("图鉴", "图鑑"));
        assert!(!ocr_text_matches("召唤", "取消"));
        assert!(!ocr_text_matches("确定", "确定按钮"));
        // An ASCII digit run never fuzzy-matches its absence (CJK numeral).
        assert!(!ocr_text_matches("队伍1", "队伍一"));
        // Empty boundaries keep the historical rule: only equal empties match.
        assert!(ocr_text_matches("", ""));
        assert!(!ocr_text_matches("", "队伍1"));
        assert!(!ocr_text_matches("队伍1", ""));
    }

    #[test]
    fn matching_ocr_lines_strict_is_normalized_exact_equality() {
        let lines = vec![
            chain_line(0, 0, 10, 10, "队伍1", 1.0),
            chain_line(0, 12, 10, 10, "队伍2", 0.99),
            chain_line(0, 24, 10, 10, "队伍1 ", 0.98),
            chain_line(0, 36, 10, 10, "队伍1满", 0.97),
            chain_line(0, 48, 10, 10, "召唤", 0.96),
            chain_line(0, 60, 10, 10, "召喚", 0.95),
            chain_line(0, 72, 10, 10, "队伍１", 0.94),
        ];
        let texts = |c: Vec<&OcrLine>| -> Vec<String> {
            c.into_iter().map(|l| l.text.clone()).collect()
        };
        // Strict: normalized equality only (whitespace/full-width folded), so the
        // 队伍2/队伍1满/半角数字 variants are NOT candidates.
        assert_eq!(
            texts(matching_ocr_lines_strict(&lines, "队伍1", None)),
            vec!["队伍1", "队伍1 ", "队伍１"]
        );
        // Fuzzy (runtime rule + digit guard): one extra insertion is tolerated,
        // but 队伍2 is rejected by the digit protection.
        assert_eq!(
            texts(matching_ocr_lines(&lines, "队伍1", None)),
            vec!["队伍1", "队伍1 ", "队伍1满", "队伍１"]
        );
        // CJK variant glyphs: strict rejects 召喚, fuzzy keeps it.
        assert_eq!(
            texts(matching_ocr_lines_strict(&lines, "召唤", None)),
            vec!["召唤"]
        );
        assert_eq!(
            texts(matching_ocr_lines(&lines, "召唤", None)),
            vec!["召唤", "召喚"]
        );
        // The minimum-confidence filter applies to both modes.
        assert_eq!(matching_ocr_lines_strict(&lines, "队伍1", Some(0.99)).len(), 1);
        assert_eq!(matching_ocr_lines(&lines, "队伍1", Some(0.99)).len(), 1);
        // Empty input/empty expected are safe.
        assert!(matching_ocr_lines_strict(&[], "队伍1", None).is_empty());
        assert!(matching_ocr_lines_strict(&lines, "", None).is_empty());
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
                 {"op":"if","args":{"cond":"text_visible('不存在') or text_visible('町中')"},
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
        // seq134: det+rec line cache — the `ocr` step and the `log(ocr(...))`
        // expression share ONE recognition call in the same frame.
        assert_eq!(
            rois.borrow().as_slice(),
            &[Rect { x: 0, y: 0, w: 64, h: 48 }]
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
    fn exact_shape_translates_when_the_template_moved() {
        // Same shift as roi_back test, but through click_shape: the authored
        // operation shape must move with the matched template.
        let pattern: Vec<u8> = (0..100)
            .map(|i| (((i % 10) * 7 + (i / 10) * 13) % 251 + 3) as u8)
            .collect();
        let mut assets = assets();
        assets.insert(
            "I_SHAPE_SHIFT",
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
        assets.set_click_shape(
            "I_SHAPE_SHIFT",
            ClickShape {
                rect: Some(Rect {
                    x: 10,
                    y: 8,
                    w: 20,
                    h: 20,
                }),
                points: None,
                invert: false,
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
                 {"op":"click","args":{"sel":"I_SHAPE_SHIFT"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let p = shared.clicks.borrow()[0];
        assert!(
            p.x >= 38 && p.x < 58 && p.y >= 28 && p.y < 48,
            "click must follow the translated action shape (38,28,20,20), got {p:?}"
        );
    }

    #[test]
    fn exact_polygon_shape_translates_when_the_template_moved() {
        // B-01: the translation must apply to polygon shapes too, not just rect.
        let pattern: Vec<u8> = (0..100)
            .map(|i| (((i % 10) * 7 + (i / 10) * 13) % 251 + 3) as u8)
            .collect();
        let mut assets = assets();
        assets.insert(
            "I_SHAPE_POLY",
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
        assets.set_click_shape(
            "I_SHAPE_POLY",
            ClickShape {
                rect: None,
                points: Some(vec![
                    Point { x: 10, y: 8 },
                    Point { x: 30, y: 8 },
                    Point { x: 30, y: 28 },
                    Point { x: 10, y: 28 },
                ]),
                invert: false,
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
                 {"op":"click","args":{"sel":"I_SHAPE_POLY"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let p = shared.clicks.borrow()[0];
        assert!(
            p.x >= 38 && p.x < 58 && p.y >= 28 && p.y < 48,
            "polygon must translate with the match (shift +28,+20), got {p:?}"
        );
    }

    #[test]
    fn public_self_operation_geometry_matches_the_click_rules() {
        // 2026-11-06: the public helper serve uses for candidate `action_box`
        // must apply the exact same self-geometry/Q1 rules as the engine.
        let mut a = AssetSet::new();
        // (1) OCR with an authored ROI: the shape follows the recognized line.
        a.insert_ocr_var(
            "O_move",
            Some(Rect {
                x: 10,
                y: 10,
                w: 30,
                h: 12,
            }),
            "御魂".to_string(),
            "k1".to_string(),
        );
        a.set_click_shape(
            "O_move",
            ClickShape {
                rect: Some(Rect {
                    x: 12,
                    y: 11,
                    w: 20,
                    h: 8,
                }),
                points: None,
                invert: false,
            },
        );
        let (rect, shape) = self_operation_geometry(
            &a,
            "O_move",
            Rect {
                x: 14,
                y: 15,
                w: 30,
                h: 12,
            },
            Some((64, 48)),
        )
        .expect("ocr roi-follow geometry");
        assert_eq!(
            rect,
            Rect {
                x: 16,
                y: 16,
                w: 20,
                h: 8
            }
        );
        assert!(shape.is_some());

        // (2) Whole-screen ocr_var: absolute stored shape (no authored origin).
        a.insert_ocr_var("O_abs", None, "御魂".to_string(), "k2".to_string());
        a.set_click_shape(
            "O_abs",
            ClickShape {
                rect: Some(Rect {
                    x: 5,
                    y: 6,
                    w: 20,
                    h: 8,
                }),
                points: None,
                invert: false,
            },
        );
        let (rect, _) = self_operation_geometry(
            &a,
            "O_abs",
            Rect {
                x: 30,
                y: 30,
                w: 10,
                h: 5,
            },
            None,
        )
        .expect("ocr absolute geometry");
        assert_eq!(
            rect,
            Rect {
                x: 5,
                y: 6,
                w: 20,
                h: 8
            }
        );

        // (3) Template: translated shape / click_back / match bbox, like the
        // runtime template click path (`matched = real.topLeft`).
        let pattern: Vec<u8> = (0..100)
            .map(|i| (((i % 10) * 7 + (i / 10) * 13) % 251 + 3) as u8)
            .collect();
        a.insert(
            "T_shift",
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
        a.set_click_back(
            "T_shift",
            Rect {
                x: 20,
                y: 20,
                w: 6,
                h: 6,
            },
        );
        let (rect, _) = self_operation_geometry(
            &a,
            "T_shift",
            Rect {
                x: 36,
                y: 30,
                w: 10,
                h: 10,
            },
            None,
        )
        .expect("template geometry");
        // delta = matched(36,30) - roi(12,10) = (+24,+20)
        assert_eq!(
            rect,
            Rect {
                x: 44,
                y: 40,
                w: 6,
                h: 6
            }
        );

        // (4) Region: its fixed rect, `real` ignored.
        a.insert_region("R_fixed", Rect { x: 7, y: 8, w: 9, h: 4 });
        let (rect, _) = self_operation_geometry(
            &a,
            "R_fixed",
            Rect {
                x: 1,
                y: 2,
                w: 3,
                h: 4,
            },
            None,
        )
        .expect("region geometry");
        assert_eq!(
            rect,
            Rect {
                x: 7,
                y: 8,
                w: 9,
                h: 4
            }
        );

        // Undeclared ids are loud errors.
        assert!(self_operation_geometry(&a, "NOPE", rect, None).is_err());
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

    // ---- 参数 v2 (PARAM-V2-DESIGN §3.2.4): `text_expr` OCR text at runtime ----

    /// OCR interpreter over `chain_assets()` whose det+rec provider always
    /// returns one line `text` at `rect`.
    fn mk_chain_det_for(text: &str, rect: Rect) -> (OcrInterp, SharedInput) {
        mk_det_ocr_with(chain_assets(), vec![line(text, 0.95, rect)])
    }

    #[test]
    fn click_target_text_expr_clicks_the_line_named_by_the_parameter() {
        let (interp, shared) = mk_chain_det_for(
            "挑战",
            Rect {
                x: 40,
                y: 24,
                w: 20,
                h: 12,
            },
        );
        let mut interp = interp.with_param("关键词", Value::Str("挑战".to_string()));
        let f = click_target_flow(
            r#"[{"kind":"ocr_text","sel":"O_LIST","text_expr":"param('关键词')"}]"#,
        );
        assert_eq!(
            interp.run(&f),
            RunResult::Finished(Exit::Done),
            "{:?}",
            interp.events()
        );
        let rows = interp.clicks();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].locator, "ocr_text");
        assert!(
            rows[0].match_info.starts_with("ocr_text:O_LIST:挑战:"),
            "match_info must carry the resolved text: {}",
            rows[0].match_info
        );
        let p = shared.clicks.borrow()[0];
        assert!(
            p.x >= 40 && p.x < 60 && p.y >= 24 && p.y < 36,
            "must click inside the matched line: {p:?}"
        );
    }

    #[test]
    fn click_target_text_expr_missing_parameter_fails_loudly_without_clicking() {
        // A failing resolution must NEVER fall through to `coord_memory` (that
        // would click a remembered point with the wrong expectation).
        let (interp, shared) = mk_chain_det_for(
            "挑战",
            Rect {
                x: 40,
                y: 24,
                w: 20,
                h: 12,
            },
        );
        let store = std::rc::Rc::new(std::cell::RefCell::new(FakeStore::new(
            &[("返回按钮", (50, 40))],
            false,
        )));
        let mut interp = interp.with_coord_store(Box::new(SharedFakeStore(store)));
        let f = click_target_flow(
            r#"[{"kind":"ocr_text","sel":"O_LIST","text_expr":"param('关键词')"},
                {"kind":"coord_memory","key":"返回按钮"}]"#,
        );
        match interp.run(&f) {
            RunResult::Failed(msg) => {
                assert!(msg.contains("click_target"), "must name the op: {msg}");
                assert!(msg.contains("关键词"), "must name the key: {msg}");
                assert!(msg.contains("has no value"), "{msg}");
            }
            other => panic!("expected a loud failure, got {other:?}"),
        }
        assert!(
            shared.clicks.borrow().is_empty(),
            "no click may happen when the text_expr cannot be resolved"
        );
        assert!(interp.clicks().is_empty());
    }

    #[test]
    fn click_target_text_expr_rejects_non_string_and_empty_values() {
        for value in [
            Value::Null,
            Value::Num(3.0),
            Value::Bool(true),
            Value::Str("   ".to_string()),
        ] {
            let (interp, shared) = mk_chain_det_for(
                "挑战",
                Rect {
                    x: 40,
                    y: 24,
                    w: 20,
                    h: 12,
                },
            );
            let mut interp = interp.with_param("关键词", value.clone());
            let f = click_target_flow(
                r#"[{"kind":"ocr_text","sel":"O_LIST","text_expr":"param('关键词')"}]"#,
            );
            match interp.run(&f) {
                RunResult::Failed(msg) => {
                    assert!(msg.contains("click_target"), "{value:?}: {msg}");
                    assert!(msg.contains("关键词"), "{value:?}: {msg}");
                    let expected = if matches!(value, Value::Str(_)) {
                        "empty string"
                    } else {
                        "must be a string"
                    };
                    assert!(msg.contains(expected), "{value:?}: {msg}");
                }
                other => panic!("expected a loud failure for {value:?}, got {other:?}"),
            }
            assert!(
                shared.clicks.borrow().is_empty(),
                "{value:?}: no click may happen"
            );
        }
    }

    #[test]
    fn click_target_inline_text_expr_uses_the_parameter() {
        let (interp, shared) = mk_chain_det_for(
            "个人突破",
            Rect {
                x: 40,
                y: 24,
                w: 36,
                h: 14,
            },
        );
        let mut interp = interp.with_param("关键词", Value::Str("个人突破".to_string()));
        let f = click_target_flow(
            r#"[{"kind":"ocr_text","roi":[5,1,60,40],"text_expr":"param('关键词')"}]"#,
        );
        assert_eq!(
            interp.run(&f),
            RunResult::Finished(Exit::Done),
            "{:?}",
            interp.events()
        );
        let rows = interp.clicks();
        assert_eq!(rows.len(), 1);
        assert!(rows[0].sel.is_none(), "inline locator has no asset");
        assert!(
            rows[0].match_info.starts_with("ocr_text:inline:个人突破:"),
            "{}",
            rows[0].match_info
        );
        let p = shared.clicks.borrow()[0];
        assert!(
            p.x >= 40 && p.x < 76 && p.y >= 24 && p.y < 38,
            "inline text_expr click must land in the matched line: {p:?}"
        );
    }

    #[test]
    fn requires_guard_text_expr_gates_the_click() {
        // Satisfied: the guard is read through the parameter and the click runs.
        let (interp, rois, shared) = mk_ocr_with(assets_with_ocr(), &[("挑战", 0.95)]);
        let mut interp = interp.with_param("要求", Value::Str("挑战".to_string()));
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click_point","args":{"x":20,"y":20,
                   "requires":{"kind":"ocr_text","sel":"O_TEXT",
                               "text_expr":"param('要求')","timeout_ms":300}}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(
            interp.run(&f),
            RunResult::Finished(Exit::Done),
            "{:?}",
            interp.events()
        );
        assert_eq!(shared.clicks.borrow().len(), 1, "guard passed => click runs");
        assert_eq!(rois.borrow().len(), 1, "guard must read OCR once");

        // Unsatisfied: the guard fails loudly and the click must NOT happen.
        let (interp, _rois, shared) = mk_ocr_with(assets_with_ocr(), &[("挑战", 0.95)]);
        let mut interp = interp.with_param("要求", Value::Str("别的".to_string()));
        match interp.run(&f) {
            RunResult::Failed(msg) => {
                assert!(msg.contains("page guard failed"), "{msg}");
                assert!(msg.contains("别的"), "{msg}");
            }
            other => panic!("expected a loud guard failure, got {other:?}"),
        }
        assert!(
            shared.clicks.borrow().is_empty(),
            "an unmet requires guard must suppress the click"
        );
    }

    #[test]
    fn click_point_expect_text_expr_verifies_after_the_click() {
        let (interp, _rois, shared) = mk_ocr_with(assets_with_ocr(), &[("完成", 0.95)]);
        let mut interp = interp.with_param("期望", Value::Str("完成".to_string()));
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click_point","args":{"x":20,"y":20,
                   "expect":{"kind":"ocr_text","sel":"O_TEXT",
                             "text_expr":"param('期望')","timeout_ms":300}}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(
            interp.run(&f),
            RunResult::Finished(Exit::Done),
            "{:?}",
            interp.events()
        );
        assert_eq!(shared.clicks.borrow().len(), 1);
        let detail = interp
            .events()
            .iter()
            .find(|e| e.op == "click_point")
            .map(|e| e.detail.clone())
            .expect("click_point event");
        assert!(detail.contains("verified=true"), "{detail}");
    }

    #[test]
    fn expect_text_expr_non_string_is_a_loud_error() {
        let (interp, _rois, shared) = mk_ocr_with(assets_with_ocr(), &[("完成", 0.95)]);
        let mut interp = interp.with_param("期望", Value::Num(1.0));
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click_point","args":{"x":20,"y":20,
                   "expect":{"kind":"ocr_text","sel":"O_TEXT",
                             "text_expr":"param('期望')","timeout_ms":100}}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        match interp.run(&f) {
            RunResult::Failed(msg) => {
                assert!(msg.contains("click_point"), "must name the op: {msg}");
                assert!(msg.contains("期望"), "must name the key: {msg}");
                assert!(msg.contains("must be a string"), "{msg}");
            }
            other => panic!("expected a loud failure, got {other:?}"),
        }
        // The click itself is the post-click `expect`'s precondition; what must
        // NOT happen is a successful verification.
        assert_eq!(shared.clicks.borrow().len(), 1);
    }

    #[test]
    fn expect_text_expr_is_resolved_once_not_per_poll() {
        // First OCR reply feeds the `text_expr` (完成); every poll afterwards
        // reads 别的. Resolve-once (Q8=A) therefore never matches, while a
        // per-poll re-evaluation would have switched the expectation to 别的 and
        // succeeded — the timeout message proves which happened.
        let (interp, rois, _shared) = mk_ocr_with(assets_with_ocr(), &[("完成", 0.95), ("别的", 0.95)]);
        let mut interp = interp;
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click_point","args":{"x":20,"y":20,
                   "expect":{"kind":"ocr_text","sel":"O_TEXT",
                             "text_expr":"ocr('O_TEXT')","timeout_ms":200}}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        match interp.run(&f) {
            RunResult::Failed(msg) => {
                assert!(
                    msg.contains("完成"),
                    "the FIRST read must stay the expectation: {msg}"
                );
            }
            other => panic!("expected a timeout (no re-evaluation), got {other:?}"),
        }
        assert!(
            rois.borrow().len() >= 3,
            "resolution + at least two polls must have read OCR: {}",
            rois.borrow().len()
        );
    }

    #[test]
    fn drag_ocr_text_text_expr_resolves_the_endpoint() {
        let rect = Rect {
            x: 10,
            y: 10,
            w: 20,
            h: 12,
        };
        let (interp, shared) = mk_chain_det_for("返回", rect);
        let mut interp = interp.with_param("起点", Value::Str("返回".to_string()));
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"drag","args":{
                   "from":{"kind":"ocr_text","sel":"O_LIST","text_expr":"param('起点')"},
                   "direction":"down","distance":[5,10],"duration_ms":[100,120],"margin":0.0}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(
            interp.run(&f),
            RunResult::Finished(Exit::Done),
            "{:?}",
            interp.events()
        );
        let swipes = shared.swipes.borrow();
        assert_eq!(swipes.len(), 1, "one drag must be sent");
        let (from, to, _ms) = swipes[0];
        assert!(
            (10..30).contains(&from.x) && (10..22).contains(&from.y),
            "drag start must come from the OCR line found via the parameter: {from:?}"
        );
        assert_eq!(to.x, from.x, "direction `down` keeps x");
        assert!(
            (5..=10).contains(&(to.y - from.y)),
            "distance [5,10] must be respected: {from:?} -> {to:?}"
        );
    }

    #[test]
    fn drag_ocr_text_text_expr_without_a_match_fails_without_swiping() {
        let rect = Rect {
            x: 10,
            y: 10,
            w: 20,
            h: 12,
        };
        let (interp, shared) = mk_chain_det_for("返回", rect);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"drag","args":{
                   "from":{"kind":"ocr_text","sel":"O_LIST","text_expr":"param('起点')"},
                   "direction":"down","distance":[5,10],"duration_ms":[100,120],"margin":0.0}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let mut interp = interp;
        match interp.run(&f) {
            RunResult::Failed(msg) => {
                assert!(msg.contains("drag"), "must name the op: {msg}");
                assert!(msg.contains("起点"), "must name the key: {msg}");
            }
            other => panic!("expected a loud failure, got {other:?}"),
        }
        assert!(
            shared.swipes.borrow().is_empty(),
            "no swipe may happen when the drag endpoint cannot be resolved"
        );
    }

    // ---- v1.3 operation region / long_press / curve_swipe ----

    /// OCR interpreter whose `O_LIST` asset also declares a small `click_back`
    /// operation area at `(10,30,15,10)`; the det+rec provider returns `lines`.
    fn mk_chain_det_with_back(lines: Vec<OcrLine>) -> (OcrInterp, SharedInput) {
        let mut assets = chain_assets();
        assets.set_click_back(
            "O_LIST",
            Rect {
                x: 10,
                y: 30,
                w: 15,
                h: 10,
            },
        );
        mk_det_ocr_with(assets, lines)
    }

    #[test]
    fn region_ocr_forces_the_line_bbox_over_the_asset_action_area() {
        let line_rect = Rect {
            x: 40,
            y: 24,
            w: 20,
            h: 12,
        };
        let back = Rect {
            x: 10,
            y: 30,
            w: 15,
            h: 10,
        };
        let f_plain = click_target_flow(r#"[{"kind":"ocr_text","sel":"O_LIST","text":"返回"}]"#);
        let f_ocr = click_target_flow(
            r#"[{"kind":"ocr_text","sel":"O_LIST","text":"返回","region":{"mode":"ocr"}}]"#,
        );

        // No region: the asset operation area (click_back) wins (historical).
        let (mut interp, shared) = mk_chain_det_with_back(vec![line("返回", 0.95, line_rect)]);
        assert_eq!(interp.run(&f_plain), RunResult::Finished(Exit::Done));
        let p = shared.clicks.borrow()[0];
        assert!(
            (back.x..back.x + back.w).contains(&p.x) && (back.y..back.y + back.h).contains(&p.y),
            "no region => asset click_back area, got {p:?}"
        );

        // mode=ocr: the matched line bbox wins even though click_back exists.
        let (mut interp, shared) = mk_chain_det_with_back(vec![line("返回", 0.95, line_rect)]);
        assert_eq!(interp.run(&f_ocr), RunResult::Finished(Exit::Done));
        let p = shared.clicks.borrow()[0];
        assert!(
            (line_rect.x..line_rect.x + line_rect.w).contains(&p.x)
                && (line_rect.y..line_rect.y + line_rect.h).contains(&p.y),
            "region=ocr => matched line bbox, got {p:?}"
        );
        let detail = interp
            .events()
            .iter()
            .find(|e| e.op == "click_target")
            .map(|e| e.detail.clone())
            .expect("click_target event");
        assert!(detail.contains("region=ocr"), "{detail}");
    }

    #[test]
    fn region_asset_makes_a_drag_endpoint_use_the_asset_operation_area() {
        let line_rect = Rect {
            x: 40,
            y: 24,
            w: 20,
            h: 12,
        };
        let back = Rect {
            x: 10,
            y: 30,
            w: 20,
            h: 10,
        };
        let plain = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"drag","args":{
                   "from":{"kind":"ocr_text","sel":"O_LIST","text":"返回"},
                   "direction":"up","distance":[5,10],"duration_ms":[100,120],"margin":0.0}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let with_asset = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"drag","args":{
                   "from":{"kind":"ocr_text","sel":"O_LIST","text":"返回",
                           "region":{"mode":"asset"}},
                   "direction":"up","distance":[5,10],"duration_ms":[100,120],"margin":0.0}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let build = || mk_chain_det_with_back(vec![line("返回", 0.95, line_rect)]);

        // No region: drag keeps the historical line-bbox endpoint.
        let (mut interp, shared) = build();
        assert_eq!(interp.run(&plain), RunResult::Finished(Exit::Done));
        let start = shared.swipes.borrow()[0].0;
        assert!(
            (line_rect.x..line_rect.x + line_rect.w).contains(&start.x)
                && (line_rect.y..line_rect.y + line_rect.h).contains(&start.y),
            "drag without region must use the line bbox, got {start:?}"
        );

        // region=asset: the declared asset operation area is used (new for drag).
        let (mut interp, shared) = build();
        assert_eq!(interp.run(&with_asset), RunResult::Finished(Exit::Done));
        let start = shared.swipes.borrow()[0].0;
        assert!(
            (back.x..back.x + back.w).contains(&start.x)
                && (back.y..back.y + back.h).contains(&start.y),
            "drag region=asset must use click_back, got {start:?}"
        );
    }

    #[test]
    fn region_nearest_click_picks_the_line_nearest_to_the_ocr_roi_centre() {
        // O_LIST ocr_roi is (40,24,40,24) => centre (60,36).
        let near = Rect {
            x: 60,
            y: 40,
            w: 10,
            h: 10,
        }; // centre (65,45), d2=106
        let far = Rect {
            x: 40,
            y: 24,
            w: 20,
            h: 12,
        }; // centre (50,30), d2=136
        let lines = vec![line("返回", 0.99, far), line("返回", 0.90, near)];
        let plain = click_target_flow(r#"[{"kind":"ocr_text","sel":"O_LIST","text":"返回"}]"#);
        let nearest = click_target_flow(
            r#"[{"kind":"ocr_text","sel":"O_LIST","text":"返回","region":{"mode":"nearest"}}]"#,
        );

        // Default: highest score wins (far line) — historical behaviour.
        let (mut interp, shared) = mk_det_ocr_with(chain_assets(), lines.clone());
        assert_eq!(interp.run(&plain), RunResult::Finished(Exit::Done));
        let p = shared.clicks.borrow()[0];
        assert!(
            (far.x..far.x + far.w).contains(&p.x) && (far.y..far.y + far.h).contains(&p.y),
            "default pick must stay max-score, got {p:?}"
        );

        // nearest: the line whose centre is closest to the ROI centre.
        let (mut interp, shared) = mk_det_ocr_with(chain_assets(), lines);
        assert_eq!(interp.run(&nearest), RunResult::Finished(Exit::Done));
        let p = shared.clicks.borrow()[0];
        assert!(
            (near.x..near.x + near.w).contains(&p.x) && (near.y..near.y + near.h).contains(&p.y),
            "region=nearest must pick the nearest line, got {p:?}"
        );
        let detail = interp
            .events()
            .iter()
            .find(|e| e.op == "click_target")
            .map(|e| e.detail.clone())
            .expect("click_target event");
        assert!(detail.contains("region=nearest"), "{detail}");
    }

    #[test]
    fn region_offset_shifts_and_keeps_the_line_size() {
        let line_rect = Rect {
            x: 40,
            y: 24,
            w: 20,
            h: 12,
        };
        // 2-value offset: shift only, the line size is kept.
        let (mut interp, shared) =
            mk_det_ocr_with(chain_assets(), vec![line("返回", 0.95, line_rect)]);
        let shifted = click_target_flow(
            r#"[{"kind":"ocr_text","sel":"O_LIST","text":"返回",
                 "region":{"mode":"offset","offset":[-20,-10]}}]"#,
        );
        assert_eq!(interp.run(&shifted), RunResult::Finished(Exit::Done));
        let p = shared.clicks.borrow()[0];
        assert!(
            (20..40).contains(&p.x) && (14..26).contains(&p.y),
            "offset [-20,-10] => box (20,14,20,12), got {p:?}"
        );

        // 4-value offset: shift + resize.
        let (mut interp, shared) =
            mk_det_ocr_with(chain_assets(), vec![line("返回", 0.95, line_rect)]);
        let resized = click_target_flow(
            r#"[{"kind":"ocr_text","sel":"O_LIST","text":"返回",
                 "region":{"mode":"offset","offset":[5,5,10,8]}}]"#,
        );
        assert_eq!(interp.run(&resized), RunResult::Finished(Exit::Done));
        let p = shared.clicks.borrow()[0];
        assert!(
            (45..55).contains(&p.x) && (29..37).contains(&p.y),
            "offset [5,5,10,8] => box (45,29,10,8), got {p:?}"
        );
    }

    #[test]
    fn region_nearest_inline_uses_the_inline_roi_centre() {
        // Inline roi (0,0,64,48) => centre (32,24).
        let near = Rect {
            x: 40,
            y: 24,
            w: 20,
            h: 12,
        }; // centre (50,30) d2=360
        let far = Rect {
            x: 10,
            y: 10,
            w: 10,
            h: 10,
        }; // centre (15,15) d2=370
        let lines = vec![line("返回", 0.90, near), line("返回", 0.99, far)];
        let f = click_target_flow(
            r#"[{"kind":"ocr_text","roi":[0,0,64,48],"text":"返回",
                 "region":{"mode":"nearest"}}]"#,
        );
        let (mut interp, shared) = mk_det_ocr_with(chain_assets(), lines);
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let p = shared.clicks.borrow()[0];
        assert!(
            (near.x..near.x + near.w).contains(&p.x) && (near.y..near.y + near.h).contains(&p.y),
            "inline nearest must use the inline roi centre, got {p:?}"
        );
    }

    #[test]
    fn long_press_defaults_to_3000ms_on_the_ocr_line() {
        let line_rect = Rect {
            x: 40,
            y: 24,
            w: 20,
            h: 12,
        };
        let (mut interp, shared) = mk_chain_det_for("长按", line_rect);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"long_press","args":{
                   "find":[{"kind":"ocr_text","sel":"O_LIST","text":"长按"}]}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(
            interp.run(&f),
            RunResult::Finished(Exit::Done),
            "{:?}",
            interp.events()
        );
        let presses = shared.long_presses.borrow();
        assert_eq!(presses.len(), 1, "exactly one hold");
        let (p, ms) = presses[0];
        assert_eq!(ms, 3000, "default hold_ms");
        assert!(
            (line_rect.x..line_rect.x + line_rect.w).contains(&p.x)
                && (line_rect.y..line_rect.y + line_rect.h).contains(&p.y),
            "hold must be inside the matched line: {p:?}"
        );
        assert!(shared.clicks.borrow().is_empty(), "no tap may be sent");
        let detail = interp
            .events()
            .iter()
            .find(|e| e.op == "long_press")
            .map(|e| e.detail.clone())
            .expect("long_press event");
        assert!(detail.contains("held locator=ocr_text"), "{detail}");
        assert!(detail.contains("ms=3000"), "{detail}");
    }

    #[test]
    fn long_press_hold_range_wins_over_hold_ms() {
        let line_rect = Rect {
            x: 40,
            y: 24,
            w: 20,
            h: 12,
        };
        let ranged = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"long_press","args":{
                   "find":[{"kind":"ocr_text","sel":"O_LIST","text":"长按"}],
                   "hold_ms":800,"hold_range":[2500,3500]}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let (mut interp, shared) = mk_chain_det_for("长按", line_rect);
        assert_eq!(interp.run(&ranged), RunResult::Finished(Exit::Done));
        let (_, ms) = shared.long_presses.borrow()[0];
        assert!(
            (2500..=3500).contains(&ms),
            "hold_range must win and stay in range, got {ms}"
        );

        let fixed = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"long_press","args":{
                   "find":[{"kind":"ocr_text","sel":"O_LIST","text":"长按"}],
                   "hold_ms":800}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let (mut interp, shared) = mk_chain_det_for("长按", line_rect);
        assert_eq!(interp.run(&fixed), RunResult::Finished(Exit::Done));
        let (_, ms) = shared.long_presses.borrow()[0];
        assert_eq!(ms, 800, "fixed hold_ms without a range");
    }

    #[test]
    fn long_press_text_expr_and_region_nearest_pick_the_expected_line() {
        // O_LIST ocr_roi centre (60,36): the near line must be chosen.
        let near = Rect {
            x: 60,
            y: 40,
            w: 10,
            h: 10,
        };
        let far = Rect {
            x: 40,
            y: 24,
            w: 20,
            h: 12,
        };
        let lines = vec![line("长按", 0.99, far), line("长按", 0.90, near)];
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"long_press","args":{
                   "find":[{"kind":"ocr_text","sel":"O_LIST","text_expr":"param('k')",
                            "region":{"mode":"nearest"}}]}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let (interp, shared) = mk_det_ocr_with(chain_assets(), lines.clone());
        let mut interp = interp.with_param("k", Value::Str("长按".to_string()));
        assert_eq!(
            interp.run(&f),
            RunResult::Finished(Exit::Done),
            "{:?}",
            interp.events()
        );
        let (p, _ms) = shared.long_presses.borrow()[0];
        assert!(
            (near.x..near.x + near.w).contains(&p.x) && (near.y..near.y + near.h).contains(&p.y),
            "nearest + text_expr must hold the nearest line, got {p:?}"
        );

        // A missing parameter fails loudly BEFORE any gesture.
        let (interp, shared) = mk_det_ocr_with(chain_assets(), lines);
        let mut interp = interp;
        match interp.run(&f) {
            RunResult::Failed(msg) => {
                assert!(msg.contains("long_press"), "must name the op: {msg}");
                assert!(msg.contains("'k'"), "must name the key: {msg}");
            }
            other => panic!("expected a loud failure, got {other:?}"),
        }
        assert!(
            shared.long_presses.borrow().is_empty(),
            "no hold may happen when the text_expr cannot be resolved"
        );
    }

    #[test]
    fn long_press_template_target_and_all_locators_failed_are_loud() {
        // Template chain: hold inside the matched I_BRIGHT box (12,10,10,10).
        let (input, shared) = FakeInput::new();
        let mut interp = Interpreter::new(FakeScreen::new(), input, assets(), Budget::default())
            .with_humanize(no_miss())
            .with_seed(5);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"long_press","args":{"find":[{"kind":"template","sel":"I_BRIGHT"}]}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let (p, ms) = shared.long_presses.borrow()[0];
        assert_eq!(ms, 3000);
        assert!(
            (12..22).contains(&p.x) && (10..20).contains(&p.y),
            "hold must be inside the matched template box: {p:?}"
        );

        // All locators failing is a loud failure and sends no gesture.
        let (input, shared) = FakeInput::new();
        let mut interp = Interpreter::new(
            FakeScreen::new(),
            input,
            chain_assets(),
            Budget::default(),
        )
        .with_humanize(no_miss())
        .with_seed(5);
        let f_fail = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"long_press","args":{"find":[
                    {"kind":"template","sel":"I_NEVER"},
                    {"kind":"coord_memory","key":"空记忆"}]}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        match interp.run(&f_fail) {
            RunResult::Failed(msg) => {
                assert!(msg.contains("all locators failed"), "{msg}");
                assert!(msg.contains("I_NEVER"), "{msg}");
            }
            other => panic!("expected a loud failure, got {other:?}"),
        }
        assert!(shared.long_presses.borrow().is_empty());
    }

    #[test]
    fn curve_swipe_sends_one_independent_swipe_per_segment() {
        let (mut interp, shared) = mk_inspect(FakeScreen::new());
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"curve_swipe","args":{
                   "points":[[10,10],[20,20],[30,15]],"duration_ms":[600,600]}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let swipes = shared.swipes.borrow();
        assert_eq!(swipes.len(), 2, "3 points => 2 segments");
        assert_eq!(swipes[0].0, Point { x: 10, y: 10 });
        assert_eq!(swipes[0].1, Point { x: 20, y: 20 });
        assert_eq!(swipes[1].0, Point { x: 20, y: 20 });
        assert_eq!(swipes[1].1, Point { x: 30, y: 15 });
        assert_eq!(swipes[0].2, 300, "600ms total split over 2 segments");
        assert_eq!(swipes[1].2, 300);
        let detail = interp
            .events()
            .iter()
            .find(|e| e.op == "curve_swipe")
            .map(|e| e.detail.clone())
            .expect("curve_swipe event");
        assert!(detail.contains("segments=2"), "{detail}");
        assert!(detail.contains("total_ms=600"), "{detail}");
        assert!(detail.contains("independent"), "{detail}");

        // Random total duration stays inside the declared range.
        let (mut interp, shared) = mk_inspect(FakeScreen::new());
        let ranged = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"curve_swipe","args":{
                   "points":[[10,10],[20,20],[30,15]],"duration_ms":[600,1000]}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&ranged), RunResult::Finished(Exit::Done));
        let swipes = shared.swipes.borrow();
        let total = swipes[0].2 + swipes[1].2;
        assert!((600..=1000).contains(&total), "total {total} out of range");
        assert_eq!(swipes[0].2, swipes[1].2, "evenly split");
    }

    #[test]
    fn curve_swipe_out_of_frame_point_is_a_loud_failure() {
        let (mut interp, shared) = mk_inspect(FakeScreen::new());
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"curve_swipe","args":{"points":[[10,10],[200,20]]}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        match interp.run(&f) {
            RunResult::Failed(msg) => {
                assert!(msg.contains("point[1]"), "{msg}");
                assert!(msg.contains("outside the frame"), "{msg}");
            }
            other => panic!("expected a loud failure, got {other:?}"),
        }
        assert!(shared.swipes.borrow().is_empty(), "no segment may be sent");
    }

    // ---- v1.4 A-plan: `region.asset` (whole-library operation-area asset) ----

    /// 64x48 frame with the patterned 10x10 tile stamped **twice**: an exact copy
    /// at `(ax,ay)` (NCC 1.0, the "best match") and a slightly perturbed copy at
    /// `(bx,by)` (lower NCC, still above `I_BRIGHT`'s 0.8 threshold).
    fn frame_with_two_tiles(ax: u32, ay: u32, bx: u32, by: u32) -> Frame {
        let mut f = frame_with_tile(ax, ay);
        for y in 0..10u32 {
            for x in 0..10u32 {
                let mut v = (((x * 7 + y * 13) % 251) + 3) as u8;
                if x == 3 && y == 4 {
                    v = v.wrapping_add(70);
                }
                if x == 6 && y == 7 {
                    v = v.wrapping_sub(65);
                }
                let i = (((by + y) * 64 + (bx + x)) * 4) as usize;
                f.rgba[i] = v;
                f.rgba[i + 1] = v;
                f.rgba[i + 2] = v;
                f.rgba[i + 3] = 255;
            }
        }
        f
    }

    fn two_tile_frame() -> Frame {
        frame_with_two_tiles(12, 10, 40, 6)
    }

    /// The scripted "parameter OCR line": its bbox centre (42,9) is near the
    /// perturbed tile at (40,6) and far from the exact tile at (12,10).
    fn param_ocr_line() -> OcrLine {
        line(
            "行",
            0.95,
            Rect {
                x: 36,
                y: 4,
                w: 12,
                h: 10,
            },
        )
    }

    /// Interpreter over `frame` with `O_REF` (ocr_lines, whole frame) + the
    /// caller's assets; the det+rec provider always returns `lines`.
    fn mk_aplan(
        mut assets: AssetSet,
        lines: Vec<OcrLine>,
        frame: Frame,
    ) -> (OcrInterp, SharedInput) {
        assets.insert_ocr_lines(
            "O_REF",
            Rect {
                x: 0,
                y: 0,
                w: 64,
                h: 48,
            },
        );
        let (input, shared) = FakeInput::new();
        let (fake, _rois) = FakeDetOcr::new(lines);
        let interp = Interpreter::new(
            FakeScreen { frame, grabs: 0 },
            input,
            assets,
            Budget::default(),
        )
        .with_humanize(no_miss())
        .with_ocr(Box::new(fake))
        .with_seed(5);
        (interp, shared)
    }

    #[test]
    fn region_asset_mode_asset_clicks_the_best_operation_asset_match() {
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click_target","args":{"find":[
                   {"kind":"ocr_text","sel":"O_REF","text":"行",
                    "region":{"mode":"asset","asset":"I_BRIGHT"}}]}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let (mut interp, shared) = mk_aplan(assets(), vec![param_ocr_line()], two_tile_frame());
        assert_eq!(
            interp.run(&f),
            RunResult::Finished(Exit::Done),
            "{:?}",
            interp.events()
        );
        let p = shared.clicks.borrow()[0];
        assert!(
            (12..22).contains(&p.x) && (10..20).contains(&p.y),
            "mode=asset must click the BEST match (exact tile at 12,10), got {p:?}"
        );
        let detail = interp
            .events()
            .iter()
            .find(|e| e.op == "click_target")
            .map(|e| e.detail.clone())
            .expect("click_target event");
        assert!(detail.contains("region=asset asset=I_BRIGHT"), "{detail}");
    }

    #[test]
    fn region_asset_mode_nearest_clicks_the_candidate_nearest_the_param_line() {
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click_target","args":{"find":[
                   {"kind":"ocr_text","sel":"O_REF","text":"行",
                    "region":{"mode":"nearest","asset":"I_BRIGHT"}}]}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let (mut interp, shared) = mk_aplan(assets(), vec![param_ocr_line()], two_tile_frame());
        assert_eq!(
            interp.run(&f),
            RunResult::Finished(Exit::Done),
            "{:?}",
            interp.events()
        );
        let p = shared.clicks.borrow()[0];
        assert!(
            (40..50).contains(&p.x) && (6..16).contains(&p.y),
            "mode=nearest must click the candidate nearest the OCR line, got {p:?}"
        );
        let detail = interp
            .events()
            .iter()
            .find(|e| e.op == "click_target")
            .map(|e| e.detail.clone())
            .expect("click_target event");
        assert!(
            detail.contains("region=nearest asset=I_BRIGHT"),
            "{detail}"
        );
    }

    #[test]
    fn region_asset_accepts_a_fixed_region_or_an_ocr_asset_operation_area() {
        // Fixed region asset: no matching needed.
        let mut fixed_assets = assets();
        fixed_assets.insert_region(
            "R_OP",
            Rect {
                x: 5,
                y: 30,
                w: 20,
                h: 10,
            },
        );
        let f_region = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click_target","args":{"find":[
                   {"kind":"ocr_text","sel":"O_REF","text":"行",
                    "region":{"mode":"asset","asset":"R_OP"}}]}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let (mut interp, shared) = mk_aplan(fixed_assets, vec![param_ocr_line()], two_tile_frame());
        assert_eq!(interp.run(&f_region), RunResult::Finished(Exit::Done));
        let p = shared.clicks.borrow()[0];
        assert!(
            (5..25).contains(&p.x) && (30..40).contains(&p.y),
            "fixed region asset must be clicked directly, got {p:?}"
        );

        // OCR asset: its declared operation area (click_back) is used.
        let mut ocr_assets = assets();
        ocr_assets.insert_ocr_lines(
            "O_OP",
            Rect {
                x: 0,
                y: 0,
                w: 10,
                h: 10,
            },
        );
        ocr_assets.set_click_back(
            "O_OP",
            Rect {
                x: 8,
                y: 8,
                w: 12,
                h: 8,
            },
        );
        let f_ocr = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click_target","args":{"find":[
                   {"kind":"ocr_text","sel":"O_REF","text":"行",
                    "region":{"mode":"nearest","asset":"O_OP"}}]}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let (mut interp, shared) = mk_aplan(ocr_assets, vec![param_ocr_line()], two_tile_frame());
        assert_eq!(interp.run(&f_ocr), RunResult::Finished(Exit::Done));
        let p = shared.clicks.borrow()[0];
        // seq134: the OCR operation asset follows the recognized line's translated
        // click_back (Q1), so a line at x≈36 gives the [44,12,12,8] area.
        assert!(
            (44..56).contains(&p.x) && (12..20).contains(&p.y),
            "OCR operation asset must follow the recognized line geometry, got {p:?}"
        );
    }

    #[test]
    fn region_asset_without_candidates_or_undeclared_fails_loudly() {
        // No match candidate on a flat frame => loud failure, no click.
        let flat = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click_target","args":{"find":[
                   {"kind":"ocr_text","sel":"O_REF","text":"行",
                    "region":{"mode":"nearest","asset":"I_BRIGHT"}}]}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let (mut interp, shared) = mk_aplan(assets(), vec![param_ocr_line()], frame_flat());
        match interp.run(&flat) {
            RunResult::Failed(msg) => {
                assert!(msg.contains("operation asset `I_BRIGHT`"), "{msg}");
                assert!(msg.contains("no match candidate"), "{msg}");
            }
            other => panic!("expected a loud failure, got {other:?}"),
        }
        assert!(shared.clicks.borrow().is_empty(), "no click may happen");

        // Undeclared operation asset => loud failure.
        let undeclared = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click_target","args":{"find":[
                   {"kind":"ocr_text","sel":"O_REF","text":"行",
                    "region":{"mode":"asset","asset":"I_MISSING"}}]}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let (mut interp, shared) = mk_aplan(assets(), vec![param_ocr_line()], two_tile_frame());
        match interp.run(&undeclared) {
            RunResult::Failed(msg) => {
                assert!(msg.contains("operation asset `I_MISSING`"), "{msg}");
                assert!(msg.contains("not declared"), "{msg}");
            }
            other => panic!("expected a loud failure, got {other:?}"),
        }
        assert!(shared.clicks.borrow().is_empty());
    }

    #[test]
    fn region_asset_works_on_inline_ocr_locators() {
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click_target","args":{"find":[
                   {"kind":"ocr_text","roi":[0,0,64,48],"text":"行",
                    "region":{"mode":"nearest","asset":"I_BRIGHT"}}]}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let (mut interp, shared) = mk_aplan(assets(), vec![param_ocr_line()], two_tile_frame());
        assert_eq!(
            interp.run(&f),
            RunResult::Finished(Exit::Done),
            "{:?}",
            interp.events()
        );
        let p = shared.clicks.borrow()[0];
        assert!(
            (40..50).contains(&p.x) && (6..16).contains(&p.y),
            "inline OCR + region.asset nearest must use the nearest candidate, got {p:?}"
        );
    }

    #[test]
    fn region_asset_works_on_drag_endpoints_and_long_press() {
        let drag = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"drag","args":{
                   "from":{"kind":"ocr_text","sel":"O_REF","text":"行",
                           "region":{"mode":"nearest","asset":"I_BRIGHT"}},
                   "direction":"up","distance":[5,10],"duration_ms":[100,120],"margin":0.0}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let (mut interp, shared) = mk_aplan(assets(), vec![param_ocr_line()], two_tile_frame());
        assert_eq!(
            interp.run(&drag),
            RunResult::Finished(Exit::Done),
            "{:?}",
            interp.events()
        );
        let start = shared.swipes.borrow()[0].0;
        assert!(
            (40..50).contains(&start.x) && (6..16).contains(&start.y),
            "drag start must come from the nearest operation candidate, got {start:?}"
        );

        let press = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"long_press","args":{"find":[
                   {"kind":"ocr_text","sel":"O_REF","text":"行",
                    "region":{"mode":"nearest","asset":"I_BRIGHT"}}]}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let (mut interp, shared) = mk_aplan(assets(), vec![param_ocr_line()], two_tile_frame());
        assert_eq!(
            interp.run(&press),
            RunResult::Finished(Exit::Done),
            "{:?}",
            interp.events()
        );
        let (p, _ms) = shared.long_presses.borrow()[0];
        assert!(
            (40..50).contains(&p.x) && (6..16).contains(&p.y),
            "long_press must hold the nearest operation candidate, got {p:?}"
        );
        let detail = interp
            .events()
            .iter()
            .find(|e| e.op == "long_press")
            .map(|e| e.detail.clone())
            .expect("long_press event");
        assert!(
            detail.contains("region=nearest asset=I_BRIGHT"),
            "{detail}"
        );
    }

    #[test]
    fn operation_region_for_asset_helper_covers_template_region_ocr_and_errors() {
        let mut assets = assets();
        assets.insert_region(
            "R_OP",
            Rect {
                x: 1,
                y: 2,
                w: 3,
                h: 4,
            },
        );
        assets.insert_ocr_lines(
            "O_OP",
            Rect {
                x: 0,
                y: 0,
                w: 10,
                h: 10,
            },
        );
        assets.set_click_back(
            "O_OP",
            Rect {
                x: 8,
                y: 8,
                w: 12,
                h: 8,
            },
        );
        let (mut interp, _shared) = mk_aplan(assets, vec![param_ocr_line()], two_tile_frame());

        // Template: mode=asset -> best match (exact tile at 12,10).
        let (rect, _shape) = interp
            .operation_region_for_asset(
                "I_BRIGHT",
                &OpRegion::Asset { asset: None },
                None,
                "site",
            )
            .expect("best template match");
        assert_eq!(
            rect,
            Rect {
                x: 12,
                y: 10,
                w: 10,
                h: 10
            }
        );

        // Template: mode=nearest -> candidate nearest the reference point.
        let (rect, _shape) = interp
            .operation_region_for_asset(
                "I_BRIGHT",
                &OpRegion::Nearest {
                    asset: Some("I_BRIGHT".to_string()),
                },
                Some(Point { x: 42, y: 9 }),
                "site",
            )
            .expect("nearest template match");
        assert_eq!(
            rect,
            Rect {
                x: 40,
                y: 6,
                w: 10,
                h: 10
            }
        );

        // Fixed region asset.
        let (rect, _shape) = interp
            .operation_region_for_asset(
                "R_OP",
                &OpRegion::Asset {
                    asset: Some("R_OP".to_string()),
                },
                None,
                "site",
            )
            .expect("region asset");
        assert_eq!(
            rect,
            Rect {
                x: 1,
                y: 2,
                w: 3,
                h: 4
            }
        );

        // OCR asset -> seq134: OCR targets are position candidates now; the
        // candidate's action_box follows the recognized line (Q1 translation of
        // click_back), never the absolute declared area.
        let (rect, _shape) = interp
            .operation_region_for_asset(
                "O_OP",
                &OpRegion::Asset {
                    asset: Some("O_OP".to_string()),
                },
                None,
                "site",
            )
            .expect("ocr asset");
        assert_eq!(
            rect,
            Rect {
                x: 44,
                y: 12,
                w: 12,
                h: 8
            }
        );

        // Undeclared asset.
        let err = interp
            .operation_region_for_asset(
                "I_MISSING",
                &OpRegion::Asset {
                    asset: Some("I_MISSING".to_string()),
                },
                None,
                "site",
            )
            .expect_err("undeclared must fail");
        assert!(err.contains("site:"), "{err}");
        assert!(err.contains("not declared"), "{err}");

        // mode=nearest without a reference point.
        let err = interp
            .operation_region_for_asset("I_BRIGHT", &OpRegion::Nearest { asset: None }, None, "site")
            .expect_err("nearest needs a reference");
        assert!(err.contains("reference"), "{err}");
    }

    #[test]
    fn operation_region_for_asset_collection_candidates_span_members() {
        let mut assets = assets();
        let tpl = assets.get("I_BRIGHT").cloned().expect("I_BRIGHT");
        assets.insert("I_BRIGHT2", tpl);
        assets.insert_collection(
            "C_TILES",
            vec!["I_BRIGHT".to_string(), "I_BRIGHT2".to_string()],
            Some(0.8),
        );
        let (mut interp, _shared) = mk_aplan(assets, vec![param_ocr_line()], two_tile_frame());

        // nearest across the collection's members: reference near (40,6).
        let (rect, _shape) = interp
            .operation_region_for_asset(
                "C_TILES",
                &OpRegion::Nearest {
                    asset: Some("C_TILES".to_string()),
                },
                Some(Point { x: 42, y: 9 }),
                "site",
            )
            .expect("collection nearest");
        assert_eq!(
            rect,
            Rect {
                x: 40,
                y: 6,
                w: 10,
                h: 10
            }
        );

        // mode=asset uses the collection's single best match (exact tile).
        let (rect, _shape) = interp
            .operation_region_for_asset(
                "C_TILES",
                &OpRegion::Asset {
                    asset: Some("C_TILES".to_string()),
                },
                None,
                "site",
            )
            .expect("collection best");
        assert_eq!(
            rect,
            Rect {
                x: 12,
                y: 10,
                w: 10,
                h: 10
            }
        );
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

    // ---- v1.2.5 (O-2): drag endpoints with an inline operation shape ----

    /// Triangle used by the O-2 drag tests: screen 64x48, `[[4,4],[40,4],[4,40]]`.
    fn triangle_shape() -> ClickShape {
        ClickShape {
            rect: None,
            points: Some(vec![
                Point { x: 4, y: 4 },
                Point { x: 40, y: 4 },
                Point { x: 4, y: 40 },
            ]),
            invert: false,
        }
    }

    #[test]
    fn drag_from_polygon_samples_inside_polygon() {
        let shape = triangle_shape();
        for i in 0..32 {
            let (mut interp, input) = mk_inspect(FakeScreen::new());
            let f = flow(
                r#"{"schema_version":1,"name":"t","steps":[
                     {"op":"drag","args":{
                       "from":{"kind":"region","roi":[0,0,64,48],
                               "shape":{"kind":"polygon","points":[[4,4],[40,4],[4,40]],"invert":false}},
                       "direction":"right","distance":[5,10],"duration_ms":[100,120],"margin":0.0
                     }},
                     {"op":"finish","args":{"exit":"done"}}]}"#,
            );
            assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done), "run {i}");
            let swipes = input.swipes.borrow().clone();
            assert_eq!(swipes.len(), 1, "run {i}: exactly one swipe");
            let (from, _to, _ms) = swipes[0];
            assert!(
                shape.contains(from),
                "run {i}: drag start {from:?} must lie inside the polygon"
            );
            let ev = interp
                .events()
                .iter()
                .rev()
                .find(|e| e.op == "drag")
                .expect("drag event");
            assert!(ev.detail.contains("from_shape=polygon"), "{}", ev.detail);
            assert!(ev.detail.contains("from_invert=false"), "{}", ev.detail);
            assert!(
                ev.detail.contains("margin=ignored"),
                "polygon sampling ignores margin: {}",
                ev.detail
            );
        }
    }

    #[test]
    fn drag_invert_shape_start_is_outside_excluded_shape() {
        // Invert excludes the right band [40,0,24,48]; any start is x<40, so the
        // right-ward distance endpoint can never leave the 64x48 frame.
        for i in 0..16 {
            let (mut interp, input) = mk_inspect(FakeScreen::new());
            let f = flow(
                r#"{"schema_version":1,"name":"t","steps":[
                     {"op":"drag","args":{
                       "from":{"kind":"region","roi":[40,0,24,48],
                               "shape":{"kind":"rect","roi":[40,0,24,48],"invert":true}},
                       "direction":"right","distance":[5,10],"duration_ms":[100,120]
                     }},
                     {"op":"finish","args":{"exit":"done"}}]}"#,
            );
            assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done), "run {i}");
            let swipes = input.swipes.borrow().clone();
            assert_eq!(swipes.len(), 1, "run {i}: exactly one swipe");
            let (from, _to, _ms) = swipes[0];
            assert!(
                from.x < 40,
                "run {i}: inverted drag start {from:?} fell into the excluded rect"
            );
            let ev = interp
                .events()
                .iter()
                .rev()
                .find(|e| e.op == "drag")
                .expect("drag event");
            assert!(ev.detail.contains("from_invert=true"), "{}", ev.detail);
            assert!(ev.detail.contains("from_shape=rect"), "{}", ev.detail);
            assert!(ev.detail.contains("margin=ignored"), "{}", ev.detail);
        }
    }

    #[test]
    fn drag_to_polygon_shape_samples_endpoint_inside_target_shape() {
        // Target triangle lives on the right (x>=30); a right-ward drag from the
        // left region always satisfies dot>0 and dist>=20, so every run works.
        let target = ClickShape {
            rect: None,
            points: Some(vec![
                Point { x: 30, y: 0 },
                Point { x: 63, y: 0 },
                Point { x: 63, y: 40 },
            ]),
            invert: false,
        };
        for i in 0..16 {
            let (mut interp, input) = mk_inspect(FakeScreen::new());
            let f = flow(
                r#"{"schema_version":1,"name":"t","steps":[
                     {"op":"drag","args":{
                       "from":{"kind":"region","roi":[0,0,10,10]},
                       "to":{"kind":"region","roi":[30,0,34,41],
                             "shape":{"kind":"polygon","points":[[30,0],[63,0],[63,40]],"invert":false}},
                       "direction":"right","duration_ms":[100,120]
                     }},
                     {"op":"finish","args":{"exit":"done"}}]}"#,
            );
            assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done), "run {i}");
            let swipes = input.swipes.borrow().clone();
            assert_eq!(swipes.len(), 1, "run {i}: exactly one swipe");
            let (from, to, _ms) = swipes[0];
            assert!(to.x > from.x, "run {i}: direction right {from:?} -> {to:?}");
            assert!(
                target.contains(to),
                "run {i}: drag end {to:?} must lie inside the target polygon"
            );
            let ev = interp
                .events()
                .iter()
                .rev()
                .find(|e| e.op == "drag")
                .expect("drag event");
            assert!(ev.detail.contains("to_shape=polygon"), "{}", ev.detail);
            assert!(ev.detail.contains("margin=ignored"), "{}", ev.detail);
        }
    }

    #[test]
    fn drag_invert_shape_covering_screen_fails_loudly_without_swipe() {
        let (mut interp, input) = mk_inspect(FakeScreen::new());
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"drag","args":{
                   "from":{"kind":"region","roi":[0,0,64,48],
                           "shape":{"kind":"rect","roi":[0,0,64,48],"invert":true}},
                   "direction":"right","distance":[5,10]
                 }},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        match interp.run(&f) {
            RunResult::Failed(msg) => {
                assert!(msg.contains("no clickable point"), "{msg}");
                assert!(msg.contains("drag from target"), "{msg}");
            }
            other => panic!("expected Failed, got {other:?}"),
        }
        assert!(
            input.swipes.borrow().is_empty(),
            "a drag with no valid shape point must never swipe (no bbox/centre fallback)"
        );
    }

    // ---- seq129: drag endpoints from task assets (same path as a click) ----

    /// Interpreter over one custom frame + AssetSet (no OCR provider).
    fn drag_interp_with(
        frame: Frame,
        set: AssetSet,
    ) -> (Interpreter<TwoFrameScreen, FakeInput>, SharedInput) {
        let (input, shared) = FakeInput::new();
        let interp = Interpreter::new(
            TwoFrameScreen {
                frames: vec![frame],
                idx: 0,
            },
            input,
            set,
            Budget::default(),
        )
        .with_humanize(no_miss());
        (interp, shared)
    }

    /// Same 10x10 patterned tile the `assets()` fixture uses.
    fn tile_template(roi: Option<Rect>) -> Template {
        let pattern: Vec<u8> = (0..100)
            .map(|i| (((i % 10) * 7 + (i / 10) * 13) % 251 + 3) as u8)
            .collect();
        Template {
            gray: GrayImage::new(10, 10, pattern),
            roi,
            threshold: 0.8,
            click_polygon: None,
        }
    }

    #[test]
    fn drag_from_template_asset_uses_its_matched_box() {
        let (mut interp, input) = mk_inspect(FakeScreen::new());
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"drag","args":{
                   "from":{"kind":"asset","sel":"I_BRIGHT"},
                   "direction":"right","distance":[5,10],
                   "duration_ms":[100,120],"margin":0.0}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let swipes = input.swipes.borrow().clone();
        assert_eq!(swipes.len(), 1, "exactly one swipe");
        let (from, to, ms) = swipes[0];
        let matched = Rect {
            x: 12,
            y: 10,
            w: 10,
            h: 10,
        };
        assert!(
            inside(matched, from),
            "drag start must be sampled from the asset matched box {matched:?}, got {from:?}"
        );
        assert!(to.x > from.x, "direction right: {from:?} -> {to:?}");
        assert!((100..=120).contains(&ms), "duration {ms}");
        let ev = interp
            .events()
            .iter()
            .rev()
            .find(|e| e.op == "drag")
            .expect("drag event");
        assert_eq!(ev.result, "ok");
        assert!(ev.detail.contains("direction=right"), "{}", ev.detail);
        assert!(ev.detail.contains("randomized=true"), "{}", ev.detail);
        assert!(ev.detail.contains("from_shape=none"), "{}", ev.detail);
    }

    #[test]
    fn drag_to_template_asset_endpoint_lands_inside_it_in_direction() {
        // Target tile is far to the right so the 24-sample direction window is
        // comfortably satisfiable.
        let (mut interp, input) = drag_interp_with(frame_with_tile(40, 10), assets());
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"drag","args":{
                   "from":{"kind":"region","roi":[0,0,5,5]},
                   "to":{"kind":"asset","sel":"I_BRIGHT"},
                   "direction":"right","duration_ms":[100,120],"margin":0.0}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let swipes = input.swipes.borrow().clone();
        assert_eq!(swipes.len(), 1, "exactly one swipe");
        let (from, to, _ms) = swipes[0];
        let target = Rect {
            x: 40,
            y: 10,
            w: 10,
            h: 10,
        };
        assert!(to.x > from.x, "direction right: {from:?} -> {to:?}");
        assert!(
            inside(target, to),
            "drag end must be sampled from the `to` asset box {target:?}, got {to:?}"
        );
        let ev = interp
            .events()
            .iter()
            .rev()
            .find(|e| e.op == "drag")
            .expect("drag event");
        assert!(ev.detail.contains("to_shape=none"), "{}", ev.detail);
    }

    #[test]
    fn drag_from_template_asset_honours_its_action_shape() {
        // Template matched at (12,10) with authored roi (0,0): the action_shape
        // rect (6,5,4,3) is translated to (18,15,4,3) — the drag start must stay
        // inside that exact shape, not the whole matched box.
        let mut set = AssetSet::new();
        set.insert(
            "I_BRIGHT",
            tile_template(Some(Rect {
                x: 0,
                y: 0,
                w: 64,
                h: 48,
            })),
        );
        set.set_click_shape(
            "I_BRIGHT",
            ClickShape {
                rect: Some(Rect {
                    x: 6,
                    y: 5,
                    w: 4,
                    h: 3,
                }),
                points: None,
                invert: false,
            },
        );
        let expected = Rect {
            x: 18,
            y: 15,
            w: 4,
            h: 3,
        };
        for seed in 0..12 {
            let (input, shared) = FakeInput::new();
            let mut interp = Interpreter::new(
                FakeScreen::new(),
                input,
                set.clone(),
                Budget::default(),
            )
            .with_humanize(no_miss())
            .with_seed(seed);
            let f = flow(
                r#"{"schema_version":1,"name":"t","steps":[
                     {"op":"drag","args":{
                       "from":{"kind":"asset","sel":"I_BRIGHT"},
                       "direction":"right","distance":[5,10],"margin":0.0}},
                     {"op":"finish","args":{"exit":"done"}}]}"#,
            );
            assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done), "seed {seed}");
            let swipes = shared.swipes.borrow().clone();
            assert_eq!(swipes.len(), 1, "seed {seed}");
            assert!(
                inside(expected, swipes[0].0),
                "seed {seed}: start {:?} outside the translated action_shape {expected:?}",
                swipes[0].0
            );
            let ev = interp
                .events()
                .iter()
                .rev()
                .find(|e| e.op == "drag")
                .expect("drag event");
            assert!(ev.detail.contains("from_shape=rect"), "{}", ev.detail);
        }
    }

    #[test]
    fn drag_asset_endpoint_miss_fails_loudly_and_never_swipes() {
        let (mut interp, input) = mk_inspect(FakeScreen::new());
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"drag","args":{
                   "from":{"kind":"asset","sel":"I_NEVER"},
                   "direction":"up","distance":[10,20]}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        match interp.run(&f) {
            RunResult::Failed(msg) => {
                assert!(msg.contains("I_NEVER"), "asset id must be in the error: {msg}");
                assert!(msg.contains("not found"), "{msg}");
                assert!(msg.contains("drag args.from"), "{msg}");
            }
            other => panic!("expected Failed, got {other:?}"),
        }
        assert!(
            input.swipes.borrow().is_empty(),
            "an unrecognized asset endpoint must never swipe"
        );
        let last = interp.events().last().expect("event");
        assert_eq!(last.op, "drag");
        assert_eq!(last.result, "failed");
    }

    #[test]
    fn drag_from_ocr_var_asset_uses_the_recognized_line_rect() {
        let (mut interp, _rois, shared) = mk_det_interp(
            ocr_var_assets(),
            vec![chain_line(20, 20, 10, 5, "御魂", 0.97)],
        );
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"drag","args":{
                   "from":{"kind":"asset","sel":"O_group"},
                   "direction":"right","distance":[5,10],"margin":0.0}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let swipes = shared.swipes.borrow().clone();
        assert_eq!(swipes.len(), 1);
        let line = Rect {
            x: 20,
            y: 20,
            w: 10,
            h: 5,
        };
        assert!(
            inside(line, swipes[0].0),
            "OCR asset endpoint must use the recognized line rect {line:?}, got {:?}",
            swipes[0].0
        );
    }

    #[test]
    fn drag_from_region_asset_uses_its_fixed_roi() {
        let mut set = assets();
        let region = Rect {
            x: 10,
            y: 20,
            w: 10,
            h: 10,
        };
        set.insert_region("R_fixed", region);
        let (mut interp, input) = drag_interp_with(frame_with_tile(12, 10), set);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"drag","args":{
                   "from":{"kind":"asset","sel":"R_fixed"},
                   "direction":"right","distance":[5,10],"margin":0.0}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let swipes = input.swipes.borrow().clone();
        assert_eq!(swipes.len(), 1);
        assert!(
            inside(region, swipes[0].0),
            "region asset endpoint must be its declared ROI {region:?}, got {:?}",
            swipes[0].0
        );
    }

    #[test]
    fn drag_from_asset_op_region_chain_lands_on_the_terminal_box() {
        // Root template declares op_region -> ocr_var gate; model B: the drag
        // endpoint is the gate's own recognized line box, not the root box.
        let mut set = AssetSet::new();
        set.insert("I_ROOT", tile_template(None));
        set.insert_ocr_var("O_GATE", None, "乙".to_string(), "pb".to_string());
        set.insert_op_region(
            "I_ROOT",
            AssetOpRegion::Asset {
                asset: "O_GATE".to_string(),
                pick: OpRegionPick::Score,
            },
        );
        let (ocr, _rois) = FakeDetOcr::new(vec![chain_line(50, 30, 8, 4, "乙", 0.97)]);
        let (input, shared) = FakeInput::new();
        let mut interp = Interpreter::new(
            FakeScreen::new(),
            input,
            set,
            Budget::default(),
        )
        .with_humanize(no_miss())
        .with_ocr(Box::new(ocr));
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"drag","args":{
                   "from":{"kind":"asset","sel":"I_ROOT"},
                   "direction":"left","distance":[5,10],"margin":0.0}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let swipes = shared.swipes.borrow().clone();
        assert_eq!(swipes.len(), 1);
        let terminal = Rect {
            x: 50,
            y: 30,
            w: 8,
            h: 4,
        };
        let root_box = Rect {
            x: 12,
            y: 10,
            w: 10,
            h: 10,
        };
        assert!(
            inside(terminal, swipes[0].0),
            "chain drag start must be inside the terminal box {terminal:?}, got {:?}",
            swipes[0].0
        );
        assert!(
            !inside(root_box, swipes[0].0),
            "the root box must not provide the drag geometry once a chain is declared"
        );
    }

    #[test]
    fn drag_from_collection_asset_uses_the_winning_member_box() {
        let mut set = assets();
        let mut member = set.get("I_BRIGHT").expect("template").clone();
        member.roi = Some(Rect {
            x: 12,
            y: 10,
            w: 10,
            h: 10,
        });
        set.insert("I_BRIGHT_ROI", member);
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
        let (mut interp, input) = drag_interp_with(frame_with_tile(12, 10), set);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"drag","args":{
                   "from":{"kind":"asset","sel":"C_BRIGHT"},
                   "direction":"right","distance":[5,10],"margin":0.0}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let swipes = input.swipes.borrow().clone();
        assert_eq!(swipes.len(), 1);
        let box_ = Rect {
            x: 14,
            y: 12,
            w: 6,
            h: 6,
        };
        assert!(
            inside(box_, swipes[0].0),
            "collection endpoint must use the winning member's operation area {box_:?}, got {:?}",
            swipes[0].0
        );
    }

    #[test]
    fn drag_polygon_shape_outside_frame_fails_loudly_without_swipe() {
        // P1-4 runtime defense: the parser bounds coordinates (<= 1e6) but a
        // shape bbox can still sit entirely outside the current frame. That is
        // a loud failure, not a bbox fallback (and must not panic on the scan).
        let (mut interp, input) = mk_inspect(FakeScreen::new());
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"drag","args":{
                   "from":{"kind":"region","roi":[0,0,64,48],
                           "shape":{"kind":"polygon",
                                    "points":[[900000,900000],[900010,900000],[900000,900010]],
                                    "invert":false}},
                   "direction":"right","distance":[5,10]
                 }},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        match interp.run(&f) {
            RunResult::Failed(msg) => {
                assert!(msg.contains("does not intersect"), "{msg}");
                assert!(msg.contains("drag from target"), "{msg}");
            }
            other => panic!("expected Failed, got {other:?}"),
        }
        assert!(
            input.swipes.borrow().is_empty(),
            "an off-frame shape must never swipe (no bbox/centre fallback)"
        );
    }

    #[test]
    fn drag_region_bbox_near_i32_max_fails_loudly_without_panic() {
        // P1-4 defense on the bbox path: a region roi near i32::MAX used to
        // overflow `rect.x + mx + dx` (debug panic). The point is clamped and
        // the existing on-screen check turns it into a loud failure.
        let (mut interp, input) = mk_inspect(FakeScreen::new());
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"drag","args":{
                   "from":{"kind":"region","roi":[2147483600,0,100,10]},
                   "direction":"right","distance":[5,10]
                 }},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        match interp.run(&f) {
            RunResult::Failed(msg) => {
                assert!(msg.contains("outside the frame"), "{msg}");
            }
            other => panic!("expected Failed, got {other:?}"),
        }
        assert!(
            input.swipes.borrow().is_empty(),
            "an off-frame bbox must never swipe"
        );
    }

    #[test]
    fn drag_distance_endpoint_is_not_constrained_by_from_shape() {
        // P2-3 documented semantics: with `to=None` the end point is derived
        // from start + direction × random distance. The invert shape below
        // excludes y 0..=46, so the effective region is exactly row y=47; an
        // `up` distance drag necessarily starts at y=47 and ends at y<47 —
        // inside the excluded shape. That is the established distance semantics
        // (the shape constrains the sampled START, not the derived path end),
        // pinned here so nobody "fixes" it by clamping to the shape.
        let excluded = ClickShape {
            rect: Some(Rect {
                x: 0,
                y: 0,
                w: 64,
                h: 47,
            }),
            points: None,
            invert: true,
        };
        for i in 0..8 {
            let (mut interp, input) = mk_inspect(FakeScreen::new());
            let f = flow(
                r#"{"schema_version":1,"name":"t","steps":[
                     {"op":"drag","args":{
                       "from":{"kind":"region","roi":[0,0,64,48],
                               "shape":{"kind":"rect","roi":[0,0,64,47],"invert":true}},
                       "direction":"up","distance":[5,10],"duration_ms":[100,120]
                     }},
                     {"op":"finish","args":{"exit":"done"}}]}"#,
            );
            assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done), "run {i}");
            let swipes = input.swipes.borrow().clone();
            assert_eq!(swipes.len(), 1, "run {i}: exactly one swipe");
            let (from, to, _ms) = swipes[0];
            assert_eq!(from.y, 47, "run {i}: the only free row is y=47: {from:?}");
            assert!(
                to.y < 47,
                "run {i}: distance end {to:?} is derived, not shape-sampled"
            );
            assert!(
                excluded.contains(to),
                "run {i}: distance end may re-enter the excluded shape: {to:?}"
            );
            assert!(
                !excluded.effective_contains(to),
                "run {i}: effective region still excludes the end point: {to:?}"
            );
            let ev = interp
                .events()
                .iter()
                .rev()
                .find(|e| e.op == "drag")
                .expect("drag event");
            assert!(ev.detail.contains("from_invert=true"), "{}", ev.detail);
            assert!(ev.detail.contains("margin=ignored"), "{}", ev.detail);
        }
    }

    #[test]
    fn drag_to_invert_shape_samples_endpoint_outside_excluded_rect() {
        // P2-9: `to` also supports invert. The target shape excludes
        // x 20..=29, so the effective target region is x<20 or x>=30. Every
        // sampled end must land in one of those bands and never inside the
        // excluded rect (a plain-bbox sample would land exactly there).
        let excluded = ClickShape {
            rect: Some(Rect {
                x: 20,
                y: 0,
                w: 10,
                h: 48,
            }),
            points: None,
            invert: true,
        };
        for i in 0..16 {
            let (mut interp, input) = mk_inspect(FakeScreen::new());
            let f = flow(
                r#"{"schema_version":1,"name":"t","steps":[
                     {"op":"drag","args":{
                       "from":{"kind":"region","roi":[0,0,3,48]},
                       "to":{"kind":"region","roi":[20,0,10,48],
                             "shape":{"kind":"rect","roi":[20,0,10,48],"invert":true}},
                       "direction":"right","duration_ms":[100,120]
                     }},
                     {"op":"finish","args":{"exit":"done"}}]}"#,
            );
            assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done), "run {i}");
            let swipes = input.swipes.borrow().clone();
            assert_eq!(swipes.len(), 1, "run {i}: exactly one swipe");
            let (from, to, _ms) = swipes[0];
            assert!(to.x > from.x, "run {i}: direction right {from:?} -> {to:?}");
            assert!(
                !excluded.contains(to),
                "run {i}: to-invert end {to:?} fell into the excluded rect"
            );
            assert!(
                excluded.effective_contains(to),
                "run {i}: to-invert end {to:?} must be in the effective region"
            );
            let ev = interp
                .events()
                .iter()
                .rev()
                .find(|e| e.op == "drag")
                .expect("drag event");
            assert!(ev.detail.contains("to_shape=rect"), "{}", ev.detail);
            assert!(ev.detail.contains("to_invert=true"), "{}", ev.detail);
            assert!(ev.detail.contains("margin=ignored"), "{}", ev.detail);
        }
    }

    #[test]
    fn drag_region_without_shape_keeps_bbox_margin_behavior() {
        // bbox [4,4,40,30], margin 0.25 => inset x=10, y=7 => x in [14,33],
        // y in [11,26]. The endpoint only moves down 5..8px (<=34), still in frame.
        for i in 0..32 {
            let (mut interp, input) = mk_inspect(FakeScreen::new());
            let f = flow(
                r#"{"schema_version":1,"name":"t","steps":[
                     {"op":"drag","args":{
                       "from":{"kind":"region","roi":[4,4,40,30]},
                       "direction":"down","distance":[5,8],"duration_ms":[100,120],"margin":0.25
                     }},
                     {"op":"finish","args":{"exit":"done"}}]}"#,
            );
            assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done), "run {i}");
            let swipes = input.swipes.borrow().clone();
            let (from, _to, _ms) = swipes[0];
            assert!(
                (14..=33).contains(&from.x) && (11..=26).contains(&from.y),
                "run {i}: bbox margin lost, start {from:?}"
            );
            let ev = interp
                .events()
                .iter()
                .rev()
                .find(|e| e.op == "drag")
                .expect("drag event");
            assert!(ev.detail.contains("from_shape=none"), "{}", ev.detail);
            assert!(
                !ev.detail.contains("margin=ignored"),
                "the pure-bbox path keeps margin: {}",
                ev.detail
            );
        }
        // A non-inverted rect shape is exactly the bbox path, so the documented
        // margin inset must still apply (only polygon/invert ignore it).
        for i in 0..8 {
            let (mut interp, input) = mk_inspect(FakeScreen::new());
            let f = flow(
                r#"{"schema_version":1,"name":"t","steps":[
                     {"op":"drag","args":{
                       "from":{"kind":"region","roi":[4,4,40,30],
                               "shape":{"kind":"rect","roi":[4,4,40,30],"invert":false}},
                       "direction":"down","distance":[5,8],"duration_ms":[100,120],"margin":0.25
                     }},
                     {"op":"finish","args":{"exit":"done"}}]}"#,
            );
            assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done), "run {i}");
            let (from, _to, _ms) = input.swipes.borrow()[0];
            assert!(
                (14..=33).contains(&from.x) && (11..=26).contains(&from.y),
                "run {i}: rect shape must keep the bbox margin, start {from:?}"
            );
            let ev = interp
                .events()
                .iter()
                .rev()
                .find(|e| e.op == "drag")
                .expect("drag event");
            assert!(ev.detail.contains("from_shape=rect"), "{}", ev.detail);
            assert!(!ev.detail.contains("margin=ignored"), "{}", ev.detail);
        }
    }

    #[test]
    fn drag_wrong_direction_with_shape_still_errors() {
        let (mut interp, input) = mk_inspect(FakeScreen::new());
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"drag","args":{
                   "from":{"kind":"region","roi":[40,10,10,10],
                           "shape":{"kind":"rect","roi":[40,10,10,10],"invert":false}},
                   "to":{"kind":"region","roi":[5,10,10,10],
                         "shape":{"kind":"rect","roi":[5,10,10,10],"invert":false}},
                   "direction":"right"
                 }},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        match interp.run(&f) {
            RunResult::Failed(msg) => {
                assert!(msg.contains("unreachable"), "{msg}");
                assert!(msg.contains("target shape=rect"), "{msg}");
            }
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
    fn click_ocr_asset_uses_action_roi_not_template_lookup() {
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
                 {"op":"click","args":{"sel":"O_LINE"}},
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
        assert_eq!(interp.clicks()[0].locator, "ocr");
    }

    // ---- seq130 #5: OCR click geometry must follow the recognized line (Q1) ----

    /// 1280x720 blank frame (the user asset lives in full-screen coordinates).
    fn blank_frame_1280x720() -> Frame {
        Frame::new(1280, 720, vec![40u8; 1280 * 720 * 4])
    }

    /// The user's asset shape (runs/editor-切换预设御魂 clicks.jsonl): `ocr_var`
    /// with an authored ROI and an authored `action_shape` rect far above the
    /// lines it recognizes.
    fn user_preset_assets(roi: Option<Rect>) -> AssetSet {
        let mut set = AssetSet::new();
        set.insert_ocr_var("O_预设", roi, "分组7".to_string(), "group".to_string());
        set.set_click_shape(
            "O_预设",
            ClickShape {
                rect: Some(Rect {
                    x: 1057,
                    y: 72,
                    w: 120,
                    h: 50,
                }),
                points: None,
                invert: false,
            },
        );
        set
    }

    /// The recognized line (y=245): authored ROI top is y=84, so the translated
    /// operation shape is (1057+15, 72+161, 120, 50) = (1072, 233, 120, 50).
    fn user_preset_line() -> OcrLine {
        chain_line(1100, 245, 100, 40, "分组7", 0.97)
    }

    fn mk_big_ocr_interp(
        set: AssetSet,
        lines: Vec<OcrLine>,
        seed: u64,
    ) -> (Interpreter<TwoFrameScreen, FakeInput>, SharedInput) {
        let (ocr, _rois) = FakeDetOcr::new(lines);
        let (input, shared) = FakeInput::new();
        let interp = Interpreter::new(
            TwoFrameScreen {
                frames: vec![blank_frame_1280x720()],
                idx: 0,
            },
            input,
            set,
            Budget::default(),
        )
        .with_humanize(no_miss())
        .with_ocr(Box::new(ocr))
        .with_seed(seed);
        (interp, shared)
    }

    const USER_TRANSLATED_SHAPE: Rect = Rect {
        x: 1072,
        y: 233,
        w: 120,
        h: 50,
    };
    const USER_AUTHORED_SHAPE: Rect = Rect {
        x: 1057,
        y: 72,
        w: 120,
        h: 50,
    };

    fn assert_on_the_translated_line(seed: u64, p: Point) {
        assert!(
            inside(USER_TRANSLATED_SHAPE, p),
            "seed {seed}: click {p:?} must follow the recognized line into \
             {USER_TRANSLATED_SHAPE:?} (authored shape {USER_AUTHORED_SHAPE:?} is stale)"
        );
        assert!(
            !inside(USER_AUTHORED_SHAPE, p),
            "seed {seed}: click {p:?} must not fall back to the authored absolute shape \
             {USER_AUTHORED_SHAPE:?}"
        );
    }

    #[test]
    fn ocr_var_click_follows_the_recognized_line_not_the_authored_shape() {
        for seed in 0..16 {
            let (mut interp, shared) = mk_big_ocr_interp(
                user_preset_assets(Some(Rect {
                    x: 1085,
                    y: 84,
                    w: 160,
                    h: 546,
                })),
                vec![user_preset_line()],
                seed,
            );
            let f = flow(
                r#"{"schema_version":1,"name":"t","steps":[
                     {"op":"click","args":{"sel":"O_预设"}},
                     {"op":"finish","args":{"exit":"done"}}]}"#,
            );
            assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done), "seed {seed}");
            let clicks = shared.clicks.borrow().clone();
            assert_eq!(clicks.len(), 1, "seed {seed}");
            assert_on_the_translated_line(seed, clicks[0]);
        }
    }

    #[test]
    fn ocr_var_click_target_locator_follows_the_recognized_line() {
        for seed in 0..16 {
            let (mut interp, shared) = mk_big_ocr_interp(
                user_preset_assets(Some(Rect {
                    x: 1085,
                    y: 84,
                    w: 160,
                    h: 546,
                })),
                vec![user_preset_line()],
                seed,
            );
            let f = flow(
                r#"{"schema_version":1,"name":"t","steps":[
                     {"op":"click_target","args":{"find":[
                       {"kind":"ocr_text","sel":"O_预设","text":"分组7"}]}},
                     {"op":"finish","args":{"exit":"done"}}]}"#,
            );
            assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done), "seed {seed}");
            let clicks = shared.clicks.borrow().clone();
            assert_eq!(clicks.len(), 1, "seed {seed}");
            assert_on_the_translated_line(seed, clicks[0]);
        }
    }

    #[test]
    fn ocr_var_long_press_follows_the_recognized_line() {
        for seed in 0..16 {
            let (mut interp, shared) = mk_big_ocr_interp(
                user_preset_assets(Some(Rect {
                    x: 1085,
                    y: 84,
                    w: 160,
                    h: 546,
                })),
                vec![user_preset_line()],
                seed,
            );
            let f = flow(
                r#"{"schema_version":1,"name":"t","steps":[
                     {"op":"long_press","args":{"find":[
                       {"kind":"ocr_text","sel":"O_预设","text":"分组7"}]}},
                     {"op":"finish","args":{"exit":"done"}}]}"#,
            );
            assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done), "seed {seed}");
            let presses = shared.long_presses.borrow().clone();
            assert_eq!(presses.len(), 1, "seed {seed}");
            assert_on_the_translated_line(seed, presses[0].0);
        }
    }

    #[test]
    fn ocr_var_without_a_roi_keeps_the_absolute_action_shape() {
        // Q1 documented limit: a whole-screen ocr_var has no authored origin, so
        // the stored action_shape stays absolute (no translation).
        for seed in 0..8 {
            let (mut interp, shared) = mk_big_ocr_interp(
                user_preset_assets(None),
                vec![user_preset_line()],
                seed,
            );
            let f = flow(
                r#"{"schema_version":1,"name":"t","steps":[
                     {"op":"click","args":{"sel":"O_预设"}},
                     {"op":"finish","args":{"exit":"done"}}]}"#,
            );
            assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done), "seed {seed}");
            let clicks = shared.clicks.borrow().clone();
            assert_eq!(clicks.len(), 1, "seed {seed}");
            let p = clicks[0];
            assert!(
                inside(USER_AUTHORED_SHAPE, p),
                "seed {seed}: roi=None keeps the absolute shape {USER_AUTHORED_SHAPE:?}, got {p:?}"
            );
        }
    }

    #[test]
    fn click_target_template_locator_also_accepts_ocr_assets() {
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
                 {"op":"click_target","args":{"find":[{"kind":"template","sel":"O_LINE"}]}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let clicks = shared.clicks.borrow().clone();
        assert_eq!(clicks.len(), 1);
        let p = clicks[0];
        assert!(
            p.x >= 0 && p.x <= 70 && p.y >= 0 && p.y <= 40,
            "OCR template-locator click should land in the action region: {p:?}"
        );
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
        // Provider was asked with the saved recognition ROI, not the tight line ROI.
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

    // ---- derived OCR polygon mask (OCR_LINES §22.10 N10b-2 parity) ----

    /// Triangle used by the derived-OCR mask tests: (10,5) is INSIDE it, while
    /// (30,5) is inside the 32x16 source ROI but OUTSIDE the polygon.
    fn probe_triangle() -> Vec<Point> {
        vec![
            Point { x: 0, y: 0 },
            Point { x: 20, y: 0 },
            Point { x: 0, y: 15 },
        ]
    }

    /// 64x48 probe frame: outside-polygon pixels carry a sentinel 50, so a test
    /// can tell "raw frame" (50) from "polygon-masked frame" (255).
    fn derived_polygon_probe_frame() -> Frame {
        let (w, h) = (64u32, 48u32);
        let mut rgba = vec![10u8; (w * h * 4) as usize];
        for i in (3..rgba.len()).step_by(4) {
            rgba[i] = 255;
        }
        let idx = |x: u32, y: u32| ((y * w + x) * 4) as usize;
        rgba[idx(10, 5)] = 100; // inside the triangle: must survive any masking
        rgba[idx(30, 5)] = 50; // outside the triangle: white iff masked
        Frame::new(w, h, rgba)
    }

    fn frame_pixel(frame: &Frame, x: u32, y: u32) -> u8 {
        frame.rgba[((y * frame.width + x) * 4) as usize]
    }

    /// A derived line (`ocr_mode=lines`, saved from a multi-line det+rec result)
    /// must honour the asset's `ocr_polygon` exactly like the plain OCR path: the
    /// mask has to be applied to the frame BEFORE the provider sees it. The probe
    /// provider clones the frame it receives, so the previous implementation
    /// (raw frame straight into `recognize_lines`) fails this test: pixel (30,5)
    /// would still be 50 there, only masking turns it into 255.
    #[test]
    fn derived_ocr_with_polygon_masks_the_frame_before_det_rec() {
        let source = Rect {
            x: 0,
            y: 0,
            w: 32,
            h: 16,
        };
        let mut a = assets();
        a.insert_ocr(
            "O_DERIVED_MASK",
            Rect {
                x: 10,
                y: 5,
                w: 20,
                h: 8,
            },
        );
        a.set_derived_ocr("O_DERIVED_MASK", source, "确认".to_string());
        a.set_ocr_polygon("O_DERIVED_MASK", probe_triangle());
        let matched = line(
            "确认",
            0.97,
            Rect {
                x: 10,
                y: 5,
                w: 20,
                h: 8,
            },
        );
        let (ocr, rois, frames) = FrameProbeDetOcr::new(vec![matched.clone()]);
        let screen = TwoFrameScreen {
            frames: vec![derived_polygon_probe_frame()],
            idx: 0,
        };
        let (input, _shared) = FakeInput::new();
        let mut interp = Interpreter::new(screen, input, a, Budget::default())
            .with_humanize(no_miss())
            .with_ocr(Box::new(ocr))
            .with_seed(41);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"ocr","args":{"sel":"O_DERIVED_MASK"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        // Return semantics unchanged: det+rec still runs on the saved SOURCE ROI.
        assert_eq!(rois.borrow().as_slice(), &[source]);
        let seen = frames.borrow();
        assert_eq!(seen.len(), 1, "exactly one det+rec call");
        assert_eq!(
            frame_pixel(&seen[0], 10, 5),
            100,
            "polygon content must survive the mask"
        );
        assert_eq!(
            frame_pixel(&seen[0], 30, 5),
            255,
            "outside-polygon pixel must reach det+rec white (mask applied)"
        );
        // `last_ocr_rect` still comes from the provider's matched line.
        assert_eq!(interp.last_ocr_rect, Some(matched.rect));
    }

    /// No polygon => the new branch is a no-op: the raw frame must reach det+rec
    /// unchanged (sentinel 50 survives), so assets that never declared a polygon
    /// keep their exact previous behaviour.
    #[test]
    fn derived_ocr_without_polygon_keeps_the_raw_frame() {
        let source = Rect {
            x: 0,
            y: 0,
            w: 32,
            h: 16,
        };
        let mut a = assets();
        a.insert_ocr(
            "O_DERIVED_RAW",
            Rect {
                x: 10,
                y: 5,
                w: 20,
                h: 8,
            },
        );
        a.set_derived_ocr("O_DERIVED_RAW", source, "确认".to_string());
        let matched = line(
            "确认",
            0.96,
            Rect {
                x: 10,
                y: 5,
                w: 20,
                h: 8,
            },
        );
        let (ocr, rois, frames) = FrameProbeDetOcr::new(vec![matched.clone()]);
        let screen = TwoFrameScreen {
            frames: vec![derived_polygon_probe_frame()],
            idx: 0,
        };
        let (input, _shared) = FakeInput::new();
        let mut interp = Interpreter::new(screen, input, a, Budget::default())
            .with_humanize(no_miss())
            .with_ocr(Box::new(ocr))
            .with_seed(43);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"ocr","args":{"sel":"O_DERIVED_RAW"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        assert_eq!(rois.borrow().as_slice(), &[source]);
        let seen = frames.borrow();
        assert_eq!(seen.len(), 1, "exactly one det+rec call");
        assert_eq!(frame_pixel(&seen[0], 10, 5), 100);
        assert_eq!(
            frame_pixel(&seen[0], 30, 5),
            50,
            "no polygon => raw frame, never masked"
        );
        assert_eq!(interp.last_ocr_rect, Some(matched.rect));
    }

    /// The mask must not soften failure semantics: with a polygon declared, a
    /// derived line whose expected text is missing is still a loud run failure
    /// (no `Null` fallback) and `last_ocr_rect` stays cleared.
    #[test]
    fn derived_ocr_with_polygon_still_fails_loudly_when_the_line_is_missing() {
        let mut a = assets();
        a.insert_ocr(
            "O_DERIVED_MASK",
            Rect {
                x: 10,
                y: 5,
                w: 20,
                h: 8,
            },
        );
        a.set_derived_ocr(
            "O_DERIVED_MASK",
            Rect {
                x: 0,
                y: 0,
                w: 32,
                h: 16,
            },
            "确认".to_string(),
        );
        a.set_ocr_polygon("O_DERIVED_MASK", probe_triangle());
        let (ocr, _rois, _frames) = FrameProbeDetOcr::new(vec![line(
            "取消",
            0.90,
            Rect {
                x: 10,
                y: 5,
                w: 20,
                h: 8,
            },
        )]);
        let screen = TwoFrameScreen {
            frames: vec![derived_polygon_probe_frame()],
            idx: 0,
        };
        let (input, _shared) = FakeInput::new();
        let mut interp = Interpreter::new(screen, input, a, Budget::default())
            .with_humanize(no_miss())
            .with_ocr(Box::new(ocr))
            .with_seed(44);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"ocr","args":{"sel":"O_DERIVED_MASK"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        match interp.run(&f) {
            RunResult::Failed(msg) => {
                assert!(msg.contains("derived ocr_line `O_DERIVED_MASK`"), "{msg}");
                assert!(msg.contains("no line equals"), "{msg}");
                assert!(
                    msg.contains("取消"),
                    "failure must list what det+rec did find: {msg}"
                );
            }
            other => panic!("expected Failed, got {other:?}"),
        }
        assert_eq!(
            interp.last_ocr_rect, None,
            "a missing derived line clears the last rect"
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

    // ---- with_params scopes (Runner Studio split §C.4.3/§C.4.4) ----

    fn details(interp: &Interpreter<FakeScreen, FakeInput>) -> Vec<String> {
        interp.events().iter().map(|e| e.detail.clone()).collect()
    }

    #[test]
    fn with_params_maps_a_parent_value_into_the_subtask() {
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"with_params","args":{"task":"子任务",
                   "params":{"child":"param('parent')"},
                   "body":[{"op":"log","args":{"text":"param('child')"}}]}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let mut interp = mk(FakeScreen::new(), Budget::default())
            .with_param("parent", Value::Str("突破".to_string()));
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let details = details(&interp);
        assert!(
            details.iter().any(|d| d.contains("突破")),
            "mapped value must be visible: {details:?}"
        );
        assert!(
            details.iter().any(|d| d.contains("subtask `子任务`")),
            "the scope must log/label the subtask: {details:?}"
        );
        // The scope restores the caller's table: `child` must not leak.
        assert_eq!(
            interp.params,
            BTreeMap::from([("parent".to_string(), Value::Str("突破".to_string()))])
        );
    }

    #[test]
    fn with_params_same_name_inheritance_wins_over_the_subtask_default() {
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"with_params","args":{"task":"子任务",
                   "defaults":{"k":"默认"},
                   "body":[{"op":"log","args":{"text":"param('k')"}}]}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let mut interp = mk(FakeScreen::new(), Budget::default())
            .with_param("k", Value::Str("父".to_string()));
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let details = details(&interp);
        assert!(details.iter().any(|d| d.contains("父")), "{details:?}");
        assert!(
            !details.iter().any(|d| d.contains("默认")),
            "default must not override an inherited value: {details:?}"
        );
    }

    #[test]
    fn with_params_uses_the_subtask_default_when_the_parent_has_no_such_key() {
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"with_params","args":{"task":"子任务",
                   "defaults":{"k":"子默认"},
                   "body":[{"op":"log","args":{"text":"param('k')"}}]}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let mut interp = mk(FakeScreen::new(), Budget::default());
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let details = details(&interp);
        assert!(details.iter().any(|d| d.contains("子默认")), "{details:?}");
    }

    #[test]
    fn with_params_mapping_overrides_same_name_inheritance() {
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"with_params","args":{"task":"子任务",
                   "params":{"k":"param('other')"},
                   "defaults":{"k":"默认"},
                   "body":[{"op":"log","args":{"text":"param('k')"}}]}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let mut interp = mk(FakeScreen::new(), Budget::default())
            .with_param("k", Value::Str("父".to_string()))
            .with_param("other", Value::Str("映射".to_string()));
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let details = details(&interp);
        assert!(details.iter().any(|d| d.contains("映射")), "{details:?}");
    }

    #[test]
    fn with_params_mapping_is_evaluated_in_the_caller_scope_before_overriding() {
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"with_params","args":{"task":"子任务",
                   "params":{"a":"param('a') * 3"},
                   "body":[{"op":"log","args":{"text":"param('a')"}}]}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let mut interp = mk(FakeScreen::new(), Budget::default()).with_param("a", Value::Num(2.0));
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let details = details(&interp);
        assert!(details.iter().any(|d| d.contains("Num(6.0)")), "{details:?}");
    }

    #[test]
    fn with_params_inherit_none_blocks_the_parent_value_and_uses_the_default() {
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"with_params","args":{"task":"子任务",
                   "block":["k"],"defaults":{"k":"子默认"},
                   "body":[{"op":"log","args":{"text":"param('k')"}}]}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let mut interp = mk(FakeScreen::new(), Budget::default())
            .with_param("k", Value::Str("父".to_string()));
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let details = details(&interp);
        assert!(details.iter().any(|d| d.contains("子默认")), "{details:?}");
        assert!(!details.iter().any(|d| d.contains("父")), "{details:?}");
        // The blocked caller value is restored after the scope.
        assert_eq!(interp.params["k"], Value::Str("父".to_string()));
    }

    #[test]
    fn with_params_inherit_none_without_default_fails_loudly_with_the_label() {
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"with_params","args":{"task":"子任务",
                   "block":["k"],
                   "body":[{"op":"log","args":{"text":"param('k')"}}]}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let mut interp = mk(FakeScreen::new(), Budget::default())
            .with_param("k", Value::Str("父".to_string()));
        match interp.run(&f) {
            RunResult::Failed(msg) => {
                assert!(msg.contains("subtask '子任务'"), "must name it: {msg}");
                assert!(msg.contains("k"), "must name the key: {msg}");
                assert!(
                    msg.contains("call_task.params"),
                    "must suggest the mapping fix: {msg}"
                );
            }
            other => panic!("expected Failed, got {other:?}"),
        }
        // Failure must restore the caller's table too.
        assert_eq!(interp.params["k"], Value::Str("父".to_string()));
        assert!(interp.subtask_stack.is_empty(), "label stack must unwind");
    }

    #[test]
    fn with_params_multiple_call_sites_are_independent() {
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"with_params","args":{"task":"子任务",
                   "params":{"child":"param('one')"},
                   "body":[{"op":"set_var","args":{"name":"a","value":"param('child')"}}]}},
                 {"op":"with_params","args":{"task":"子任务",
                   "params":{"child":"param('two')"},
                   "body":[{"op":"set_var","args":{"name":"b","value":"param('child')"}}]}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let mut interp = mk(FakeScreen::new(), Budget::default())
            .with_param("one", Value::Str("甲".to_string()))
            .with_param("two", Value::Str("乙".to_string()));
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        assert_eq!(interp.vars["a"], Value::Str("甲".to_string()));
        assert_eq!(interp.vars["b"], Value::Str("乙".to_string()));
    }

    #[test]
    fn with_params_nested_scopes_read_the_outer_scope() {
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"with_params","args":{"task":"B",
                   "params":{"x":"param('root')"},
                   "body":[
                     {"op":"with_params","args":{"task":"C",
                       "params":{"y":"param('x')"},
                       "body":[{"op":"log","args":{"text":"param('y')"}}]}}
                   ]}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let mut interp = mk(FakeScreen::new(), Budget::default())
            .with_param("root", Value::Str("根值".to_string()));
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let details = details(&interp);
        assert!(details.iter().any(|d| d.contains("根值")), "{details:?}");
        assert_eq!(
            interp.params,
            BTreeMap::from([("root".to_string(), Value::Str("根值".to_string()))])
        );
    }

    #[test]
    fn with_params_restores_the_caller_scope_after_success() {
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"with_params","args":{"task":"子任务",
                   "defaults":{"sub_only":1},
                   "body":[{"op":"log","args":{"text":"'x'"}}]}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let mut interp = mk(FakeScreen::new(), Budget::default())
            .with_param("k", Value::Str("父".to_string()));
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        assert_eq!(
            interp.params,
            BTreeMap::from([("k".to_string(), Value::Str("父".to_string()))]),
            "subtask defaults may not leak into the caller"
        );
    }

    #[test]
    fn with_params_finish_in_the_body_still_ends_the_parent_flow() {
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"with_params","args":{"task":"子任务","body":[
                   {"op":"set_var","args":{"name":"hit","value":"1"}},
                   {"op":"finish","args":{"exit":"failed"}}
                 ]}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let mut interp = mk(FakeScreen::new(), Budget::default())
            .with_param("k", Value::Str("父".to_string()));
        // Loader-generated scopes never contain finish (target finishes are
        // stripped); a hand-authored one keeps the normal control-flow semantics.
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Failed));
        assert_eq!(interp.vars["hit"], Value::Num(1.0));
        assert_eq!(interp.params.len(), 1, "params restored after finish");
    }

    #[test]
    fn with_params_break_inside_a_loop_scope_breaks_the_loop() {
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"set_var","args":{"name":"n","value":"0"}},
                 {"op":"loop","args":{"n":"3"},"body":[
                   {"op":"with_params","args":{"task":"子任务","body":[
                     {"op":"inc_var","args":{"name":"n"}},
                     {"op":"break"}
                   ]}}
                 ]},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let mut interp = mk(FakeScreen::new(), Budget::default());
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        assert_eq!(interp.vars["n"], Value::Num(1.0), "break must leave the loop");
    }

    #[test]
    fn with_params_continue_inside_a_loop_scope_continues_the_loop() {
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"set_var","args":{"name":"n","value":"0"}},
                 {"op":"loop","args":{"n":"3"},"body":[
                   {"op":"with_params","args":{"task":"子任务","body":[
                     {"op":"inc_var","args":{"name":"n"}},
                     {"op":"continue"}
                   ]}}
                 ]},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let mut interp = mk(FakeScreen::new(), Budget::default());
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        assert_eq!(interp.vars["n"], Value::Num(3.0), "continue runs all iterations");
    }

    #[test]
    fn with_params_without_a_label_keeps_the_legacy_error_text() {
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"with_params","args":{"body":[
                   {"op":"log","args":{"text":"param('nope')"}}]}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let mut interp = mk(FakeScreen::new(), Budget::default());
        match interp.run(&f) {
            RunResult::Failed(msg) => {
                assert!(msg.contains("param('nope') has no value"), "{msg}");
                assert!(msg.contains("declare it in meta.params"), "{msg}");
                assert!(!msg.contains("subtask"), "no label => old message: {msg}");
            }
            other => panic!("expected Failed, got {other:?}"),
        }
    }

    #[test]
    fn with_params_empty_scope_keeps_the_flat_namespace_behaviour() {
        // Loader output for a legacy `{"op":"call_task","args":{"task":"X"}}`:
        // no mapping/defaults/block, so the parent table is simply shared.
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"with_params","args":{"task":"子任务","body":[
                   {"op":"log","args":{"text":"param('shared')"}}]}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let mut interp = mk(FakeScreen::new(), Budget::default())
            .with_param("shared", Value::Str("共享".to_string()));
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let details = details(&interp);
        assert!(details.iter().any(|d| d.contains("共享")), "{details:?}");
    }

    // ---- B-02/B-03/B-04/B-12: exact operation geometry on OCR/region paths ----

    #[test]
    fn click_target_ocr_lines_uses_the_action_polygon_not_the_line_rect() {
        // B-02: `ocr_lines` used to click the recognized line rect even when the
        // asset carried an explicit polygon operation region.
        let mut a = assets();
        a.insert_ocr_lines(
            "O_LINES",
            Rect {
                x: 0,
                y: 0,
                w: 64,
                h: 48,
            },
        );
        a.set_click_shape(
            "O_LINES",
            ClickShape {
                rect: None,
                points: Some(vec![
                    Point { x: 4, y: 4 },
                    Point { x: 12, y: 4 },
                    Point { x: 12, y: 12 },
                    Point { x: 4, y: 12 },
                ]),
                invert: false,
            },
        );
        let (ocr, _rois) = FakeDetOcr::new(vec![line(
            "确认",
            0.99,
            Rect {
                x: 30,
                y: 30,
                w: 10,
                h: 10,
            },
        )]);
        let (input, shared) = FakeInput::new();
        let mut interp = Interpreter::new(FakeScreen::new(), input, a, Budget::default())
            .with_humanize(no_miss())
            .with_ocr(Box::new(ocr))
            .with_seed(5);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click_target","args":{"find":[{"kind":"ocr_text","sel":"O_LINES","text":"确认"}]}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let p = shared.clicks.borrow()[0];
        // 2026-11-06 批次B (#4/Q1): with an authored ROI (0,0,64,48) the stored
        // action polygon follows the recognized line by
        // `real.topLeft - roi.topLeft` = (30,30), so the click lands in the
        // translated polygon (34,34)..(42,42) — never the line rect itself and
        // never the stale absolute authoring position.
        assert!(
            p.x >= 34 && p.x < 42 && p.y >= 34 && p.y < 42,
            "click must land in the translated action polygon (34,34,8,8), got {p:?}"
        );
    }

    #[test]
    fn click_target_ocr_single_invert_clicks_outside_the_excluded_shape() {
        // B-02: the old OCR click_target path ignored `invert` and clicked the
        // recognition/action bbox — i.e. inside the excluded area here.
        let mut a = assets();
        a.insert_ocr(
            "O_INVERT",
            Rect {
                x: 0,
                y: 0,
                w: 64,
                h: 48,
            },
        );
        a.set_click_shape(
            "O_INVERT",
            ClickShape {
                rect: Some(Rect {
                    x: 0,
                    y: 0,
                    w: 32,
                    h: 48,
                }),
                points: None,
                invert: true,
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
            .with_seed(5);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click_target","args":{"find":[{"kind":"ocr_text","sel":"O_INVERT","text":"确认"}]}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let p = shared.clicks.borrow()[0];
        assert!(
            p.x >= 32,
            "invert must click outside the excluded x<32 shape, got {p:?}"
        );
    }

    #[test]
    fn click_target_ocr_single_full_screen_invert_fails_without_clicking() {
        // B-02/B-03: invert must reach the OCR path, and an empty effective area
        // must fail loudly instead of clicking the excluded rect.
        let mut a = assets();
        a.insert_ocr(
            "O_FULL",
            Rect {
                x: 0,
                y: 0,
                w: 64,
                h: 48,
            },
        );
        a.set_click_shape(
            "O_FULL",
            ClickShape {
                rect: Some(Rect {
                    x: 0,
                    y: 0,
                    w: 64,
                    h: 48,
                }),
                points: None,
                invert: true,
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
            .with_seed(5);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click_target","args":{"find":[{"kind":"ocr_text","sel":"O_FULL","text":"确认"}]}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        match interp.run(&f) {
            RunResult::Failed(msg) => assert!(
                msg.contains("no clickable point"),
                "must explain the empty effective region: {msg}"
            ),
            other => panic!("full-screen invert must fail, got {other:?}"),
        }
        assert!(
            shared.clicks.borrow().is_empty(),
            "an empty effective region must never produce a click"
        );
    }

    #[test]
    fn full_screen_invert_shape_fails_loudly_without_clicking() {
        // Same B-03 guarantee on the template `click` path.
        let pattern: Vec<u8> = (0..100)
            .map(|i| (((i % 10) * 7 + (i / 10) * 13) % 251 + 3) as u8)
            .collect();
        let mut a = assets();
        a.insert(
            "I_FULL_INVERT",
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
        a.set_click_shape(
            "I_FULL_INVERT",
            ClickShape {
                rect: Some(Rect {
                    x: 0,
                    y: 0,
                    w: 64,
                    h: 48,
                }),
                points: None,
                invert: true,
            },
        );
        let (input, shared) = FakeInput::new();
        let mut interp = Interpreter::new(FakeScreen::new(), input, a, Budget::default())
            .with_humanize(no_miss())
            .with_seed(5);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click","args":{"sel":"I_FULL_INVERT"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        match interp.run(&f) {
            RunResult::Failed(msg) => assert!(
                msg.contains("no clickable point"),
                "must explain the empty effective region: {msg}"
            ),
            other => panic!("full-screen invert must fail, got {other:?}"),
        }
        assert!(shared.clicks.borrow().is_empty());
    }

    #[test]
    fn narrow_region_exact_shape_never_jitters_outside() {
        // B-04: point_click adds ±6 px of jitter; a 1px-wide (or very thin)
        // region must still be clicked strictly inside the shape.
        for roi in [
            Rect {
                x: 10,
                y: 20,
                w: 1,
                h: 20,
            },
            Rect {
                x: 10,
                y: 20,
                w: 12,
                h: 2,
            },
            Rect {
                x: 10,
                y: 20,
                w: 2,
                h: 12,
            },
        ] {
            let mut a = assets();
            a.insert_region("R_NARROW", roi);
            a.set_click_shape(
                "R_NARROW",
                ClickShape {
                    rect: Some(roi),
                    points: None,
                    invert: false,
                },
            );
            let (input, shared) = FakeInput::new();
            let mut interp = Interpreter::new(FakeScreen::new(), input, a, Budget::default())
                .with_humanize(crate::humanize::Humanize {
                    miss_chance_min: 0.0,
                    miss_chance_max: 0.0,
                    ..crate::humanize::Humanize::default()
                })
                .with_seed(9);
            let f = flow(
                r#"{"schema_version":1,"name":"t","steps":[
                     {"op":"click_target","args":{"find":[{"kind":"region","sel":"R_NARROW"}]}},
                     {"op":"finish","args":{"exit":"done"}}]}"#,
            );
            assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
            let p = shared.clicks.borrow()[0];
            assert!(
                p.x >= roi.x
                    && p.x < roi.x + roi.w
                    && p.y >= roi.y
                    && p.y < roi.y + roi.h,
                "jittered region click {p:?} must stay inside {roi:?}"
            );
        }
    }

    #[test]
    fn appear_and_dismiss_popup_use_the_ocr_selector_threshold() {
        // B-12: OCR assets have no template score; a successful read must count
        // for `appear`/`dismiss_popup` exactly like it does for `click`.
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"appear","args":{"sel":"O_TEXT"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let (mut interp, _) = mk_ocr(&[("完成", 0.4)]);
        assert_eq!(interp.run(&f).outcome(), Outcome::Done);

        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"dismiss_popup","args":{"sel":"O_TEXT","timeout_ms":0,"required":true}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let (input, shared) = FakeInput::new();
        let (fake_ocr, _rois) = FakeOcr::new(&[("完成", 0.4)]);
        let mut interp = Interpreter::new(
            FakeScreen::new(),
            input,
            assets_with_ocr(),
            Budget::default(),
        )
        .with_humanize(no_miss())
        .with_ocr(Box::new(fake_ocr))
        .with_seed(5);
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        assert_eq!(
            shared.clicks.borrow().len(),
            1,
            "dismiss_popup must click the OCR action region"
        );
    }

    // ---- OCR minimum-confidence threshold (same `threshold` field as NCC) ----

    /// `assets_with_ocr()` with an explicit minimum confidence on `O_TEXT`.
    fn assets_with_ocr_threshold(threshold: f32) -> AssetSet {
        let mut set = assets_with_ocr();
        set.set_ocr_threshold("O_TEXT", threshold);
        set
    }

    #[test]
    fn ocr_asset_without_threshold_accepts_a_low_confidence_read() {
        // Historical default: any successful read is a hit. A 0.02-confidence
        // read must still satisfy `appear` and `click`.
        let (mut interp, _, _) = mk_ocr_with(assets_with_ocr(), &[("完成", 0.02)]);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"appear","args":{"sel":"O_TEXT"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f).outcome(), Outcome::Done);

        let (mut interp, _, shared) = mk_ocr_with(assets_with_ocr(), &[("完成", 0.02)]);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click","args":{"sel":"O_TEXT"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        assert_eq!(shared.clicks.borrow().len(), 1, "a read counts as a hit");
    }

    #[test]
    fn ocr_asset_threshold_rejects_a_low_confidence_read() {
        // 0.4 < 0.9: the read is a miss, with score + threshold in the evidence.
        let (mut interp, _, _) = mk_ocr_with(assets_with_ocr_threshold(0.9), &[("完成", 0.4)]);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"appear","args":{"sel":"O_TEXT"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        match interp.run(&f) {
            RunResult::Failed(msg) => {
                assert!(msg.contains("0.400"), "score must be in the evidence: {msg}");
                assert!(msg.contains("0.9"), "threshold must be in the evidence: {msg}");
            }
            other => panic!("appear must reject 0.4 below threshold 0.9, got {other:?}"),
        }

        // The click path used to hardcode 0.0 for OCR; it must use the asset
        // threshold now and never emit a click for a below-threshold read.
        let (mut interp, _, shared) =
            mk_ocr_with(assets_with_ocr_threshold(0.9), &[("完成", 0.4)]);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click","args":{"sel":"O_TEXT"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        match interp.run(&f) {
            RunResult::Failed(msg) => {
                assert!(msg.contains("0.400 < 0.9"), "{msg}");
                assert!(msg.contains("no variant matched"), "{msg}");
            }
            other => panic!("click must reject 0.4 below threshold 0.9, got {other:?}"),
        }
        assert!(
            shared.clicks.borrow().is_empty(),
            "a below-threshold OCR read must never click"
        );
    }

    #[test]
    fn ocr_asset_threshold_passes_a_confident_read() {
        // 0.4 >= 0.3 proves the OCR threshold is used instead of the template
        // 0.9 default (which would have rejected this read).
        let (mut interp, _, shared) =
            mk_ocr_with(assets_with_ocr_threshold(0.3), &[("完成", 0.4)]);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click","args":{"sel":"O_TEXT"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        assert_eq!(shared.clicks.borrow().len(), 1);
        let detail = interp
            .events()
            .iter()
            .find(|e| e.op == "click")
            .map(|e| e.detail.clone())
            .expect("click event");
        assert!(detail.contains("score=0.400"), "{detail}");
    }

    #[test]
    fn ocr_threshold_gates_condition_appear_and_ocr_text_locator() {
        let if_flow = || {
            flow(
                r#"{"schema_version":1,"name":"t","steps":[
                     {"op":"if","args":{"cond":"appear('O_TEXT')"},
                      "then":[{"op":"finish","args":{"exit":"done"}}],
                      "else":[{"op":"finish","args":{"exit":"failed"}}]}]}"#,
            )
        };
        let (mut interp, _, _) = mk_ocr_with(assets_with_ocr_threshold(0.9), &[("完成", 0.4)]);
        assert_eq!(
            interp.run(&if_flow()).outcome(),
            Outcome::Failed,
            "appear('O_TEXT') must be false below the OCR threshold"
        );
        let (mut interp, _, _) = mk_ocr_with(assets_with_ocr_threshold(0.3), &[("完成", 0.4)]);
        assert_eq!(interp.run(&if_flow()).outcome(), Outcome::Done);

        let target_flow = || {
            flow(
                r#"{"schema_version":1,"name":"t","steps":[
                     {"op":"click_target","args":{"find":[
                        {"kind":"ocr_text","sel":"O_TEXT","text":"完成"}]}},
                     {"op":"finish","args":{"exit":"done"}}]}"#,
            )
        };
        let (mut interp, _, shared) =
            mk_ocr_with(assets_with_ocr_threshold(0.9), &[("完成", 0.4)]);
        match interp.run(&target_flow()) {
            RunResult::Failed(msg) => {
                assert!(msg.contains("matched but score 0.400 < 0.9"), "{msg}");
            }
            other => panic!("ocr_text must not click below threshold, got {other:?}"),
        }
        assert!(shared.clicks.borrow().is_empty());

        let (mut interp, _, shared) =
            mk_ocr_with(assets_with_ocr_threshold(0.3), &[("完成", 0.4)]);
        assert_eq!(interp.run(&target_flow()), RunResult::Finished(Exit::Done));
        assert_eq!(shared.clicks.borrow().len(), 1);
    }

    #[test]
    fn ocr_threshold_applies_to_expect_appear() {
        // `expect appear` uses the same selector threshold as click/appear:
        // the explicit OCR minimum confidence, else 0 (never 0.9).
        let expect_flow = || {
            flow(
                r#"{"schema_version":1,"name":"t","steps":[
                     {"op":"click_point","args":{"x":5,"y":5,
                       "expect":{"kind":"appear","sel":"O_TEXT","timeout_ms":1}}},
                     {"op":"finish","args":{"exit":"done"}}]}"#,
            )
        };
        let (mut interp, _, _) = mk_ocr_with(assets_with_ocr_threshold(0.9), &[("完成", 0.4)]);
        match interp.run(&expect_flow()) {
            RunResult::Failed(msg) => {
                assert!(msg.contains("expect appear not met"), "{msg}");
                assert!(msg.contains("0.400 < 0.9"), "{msg}");
            }
            other => panic!("expect appear must honour the OCR threshold, got {other:?}"),
        }
        let (mut interp, _, _) = mk_ocr_with(assets_with_ocr_threshold(0.3), &[("完成", 0.4)]);
        assert_eq!(interp.run(&expect_flow()), RunResult::Finished(Exit::Done));
    }

    // ---- OCR threshold on VALUE paths (ocr()/ocr_digit()/ocr_to_var) ----
    // Leftover 3: an OCR asset can declare `threshold` (minimum confidence) but
    // the value paths used to ignore it, so a thresholded asset still handed
    // low-confidence text to the flow. These tests pin all three behaviours:
    // explicit threshold hides the value, unset/0 keeps the old result, and an
    // explicit threshold accepts a confident read.

    #[test]
    fn ocr_value_paths_skip_a_read_below_the_explicit_threshold() {
        // A below-threshold read is a miss, not a value: ocr()/ocr_digit() are
        // `Null`, ocr_to_var stores "", and the low-confidence text never leaks.
        let (mut interp, _, _) = mk_ocr_with(assets_with_ocr_threshold(0.9), &[("完成", 0.4)]);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"log","args":{"text":"ocr('O_TEXT')"}},
                 {"op":"log","args":{"text":"ocr_digit('O_TEXT')"}},
                 {"op":"ocr_to_var","args":{"sel":"O_TEXT","var":"answer"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let details: Vec<&str> = interp.events().iter().map(|e| e.detail.as_str()).collect();
        assert!(
            details.first().is_some_and(|d| d.contains("Null")),
            "ocr() must yield no value below the threshold: {details:?}"
        );
        assert!(
            details.get(1).is_some_and(|d| d.contains("Null")),
            "ocr_digit() must yield no value below the threshold: {details:?}"
        );
        assert!(
            !details.iter().any(|d| d.contains("完成")),
            "the below-threshold text must never reach a value: {details:?}"
        );
        assert!(
            details.iter().any(|d| d.contains("ocr_to_var") && d.contains("\"\"")),
            "ocr_to_var must store the empty (unrecognized) result: {details:?}"
        );
        assert_eq!(
            interp.vars.get("answer"),
            Some(&Value::Str(String::new())),
            "a below-threshold read must not become a variable value"
        );
    }

    #[test]
    fn wait_until_ocr_value_polls_past_a_below_threshold_read() {
        // Why the value path treats a low read as a miss instead of a hard
        // error: `wait_until(ocr(...) == ...)` is the documented way to wait for
        // a value, so it must keep polling until the read is confident enough.
        let (mut interp, _, _) = mk_ocr_with(
            assets_with_ocr_threshold(0.8),
            &[("完成", 0.3), ("完成", 0.9)],
        );
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"wait_until","args":{"cond":"ocr('O_TEXT') == '完成'","timeout_s":0.3}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(
            interp.run(&f),
            RunResult::Finished(Exit::Done),
            "the 0.3 read must be skipped and the 0.9 read accepted: {:?}",
            interp.events()
        );
    }

    #[test]
    fn ocr_value_paths_without_a_threshold_keep_low_confidence_text() {
        // Backward compatibility: absent and explicit 0 both mean "no filter",
        // so the historical 0.02-confidence read is still returned.
        for threshold in [None, Some(0.0f32)] {
            let mut set = assets_with_ocr();
            if let Some(t) = threshold {
                set.set_ocr_threshold("O_TEXT", t);
            }
            let (mut interp, _, _) = mk_ocr_with(set, &[("12/30", 0.02)]);
            let f = flow(
                r#"{"schema_version":1,"name":"t","steps":[
                     {"op":"log","args":{"text":"ocr('O_TEXT')"}},
                     {"op":"log","args":{"text":"ocr_digit('O_TEXT')"}},
                     {"op":"ocr_to_var","args":{"sel":"O_TEXT","var":"answer"}},
                     {"op":"finish","args":{"exit":"done"}}]}"#,
            );
            assert_eq!(
                interp.run(&f),
                RunResult::Finished(Exit::Done),
                "threshold {threshold:?} must not filter a successful read"
            );
            let details: Vec<&str> = interp.events().iter().map(|e| e.detail.as_str()).collect();
            assert!(
                details.iter().any(|d| d.contains("12/30")),
                "ocr() must return the low-confidence text: {details:?}"
            );
            assert!(
                details.iter().any(|d| d.contains("12.0")),
                "ocr_digit() must parse the low-confidence text: {details:?}"
            );
            assert_eq!(
                interp.vars.get("answer"),
                Some(&Value::Str("12/30".to_string())),
                "ocr_to_var must store the low-confidence text at {threshold:?}"
            );
        }
    }

    #[test]
    fn ocr_value_paths_accept_a_confident_read_at_the_explicit_threshold() {
        // 0.4 >= 0.3 (and below the 0.9 template default): proves the value
        // paths compare against the OCR minimum confidence, not 0.9.
        let (mut interp, _, _) = mk_ocr_with(assets_with_ocr_threshold(0.3), &[("12/30", 0.4)]);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"log","args":{"text":"ocr('O_TEXT')"}},
                 {"op":"log","args":{"text":"ocr_digit('O_TEXT')"}},
                 {"op":"ocr_to_var","args":{"sel":"O_TEXT","var":"answer"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let details: Vec<&str> = interp.events().iter().map(|e| e.detail.as_str()).collect();
        assert!(details.iter().any(|d| d.contains("12/30")), "{details:?}");
        assert!(details.iter().any(|d| d.contains("12.0")), "{details:?}");
        assert_eq!(
            interp.vars.get("answer"),
            Some(&Value::Str("12/30".to_string()))
        );
    }

    #[test]
    fn ocr_lines_value_read_uses_the_lowest_line_score_for_the_threshold() {
        // The value joins every line, so the LOWEST line score is its
        // confidence (unlike presence/click, which use the best line).
        let lines = vec![
            line("确认", 0.95, Rect { x: 2, y: 2, w: 10, h: 8 }),
            line("取消", 0.40, Rect { x: 2, y: 20, w: 10, h: 8 }),
        ];
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"log","args":{"text":"ocr('O_LIST')"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );

        let mut strict = assets_with_ocr();
        strict.set_ocr_threshold("O_LIST", 0.9);
        let (mut interp, _) = mk_det_ocr_with(strict, lines.clone());
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let detail = &interp.events()[0].detail;
        assert!(
            detail.contains("Null"),
            "a joined value with a below-threshold line is unrecognized: {detail}"
        );
        assert!(
            !detail.contains("确认") && !detail.contains("取消"),
            "the below-threshold lines must not leak into the value: {detail}"
        );

        let mut loose = assets_with_ocr();
        loose.set_ocr_threshold("O_LIST", 0.3);
        let (mut interp, _) = mk_det_ocr_with(loose, lines);
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let detail = &interp.events()[0].detail;
        assert!(
            detail.contains("确认") && detail.contains("取消"),
            "the confident joined value must be returned: {detail}"
        );

        // No text at all is an empty result (`Null`), never a threshold failure.
        let mut empty = assets_with_ocr();
        empty.set_ocr_threshold("O_LIST", 0.9);
        let (mut interp, _) = mk_det_ocr_with(empty, Vec::new());
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        assert!(
            interp.events()[0].detail.contains("Null"),
            "zero detected lines stay an empty value: {:?}",
            interp.events()[0]
        );
    }

    #[test]
    fn ocr_value_threshold_gate_leaves_templates_and_collections_alone() {
        // The new gate reads `ocr_threshold` only: template/collection
        // thresholds and `ocr()`'s loud "not an OCR asset" errors are unchanged.
        let mut set = assets_with_ocr_threshold(0.9);
        set.insert_collection("C_MIX", vec!["I_BRIGHT".to_string()], Some(0.7));
        let (mut interp, _, _) = mk_ocr_with(set, &[("完成", 0.95)]);
        assert_eq!(interp.assets.threshold_for("C_MIX"), Some(0.7));
        assert_eq!(interp.assets.ocr_threshold("C_MIX"), None);
        assert_eq!(interp.assets.ocr_threshold("I_BRIGHT"), None);

        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"log","args":{"text":"ocr('I_BRIGHT')"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        match interp.run(&f) {
            RunResult::Failed(msg) => {
                assert!(msg.contains("template asset, not an OCR asset"), "{msg}")
            }
            other => panic!("ocr() on a template must stay a loud error, got {other:?}"),
        }
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"log","args":{"text":"ocr('C_MIX')"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        match interp.run(&f) {
            RunResult::Failed(msg) => assert!(msg.contains("not declared"), "{msg}"),
            other => panic!("ocr() on a collection must stay a loud error, got {other:?}"),
        }
    }

    #[test]
    fn ocr_lines_threshold_uses_the_best_line_score() {
        // `recognize_best` reports the best line of an `ocr_lines` asset; the
        // threshold compares against that same best score.
        let lines = vec![
            line("确认", 0.95, Rect { x: 2, y: 2, w: 10, h: 8 }),
            line("取消", 0.40, Rect { x: 2, y: 20, w: 10, h: 8 }),
        ];
        let mut pass = assets_with_ocr();
        pass.set_ocr_threshold("O_LIST", 0.9);
        let (mut interp, _) = mk_det_ocr_with(pass, lines.clone());
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"appear","args":{"sel":"O_LIST"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(
            interp.run(&f).outcome(),
            Outcome::Done,
            "best line 0.95 >= 0.9"
        );

        let mut fail = assets_with_ocr();
        fail.set_ocr_threshold("O_LIST", 0.99);
        let (mut interp, _) = mk_det_ocr_with(fail, lines);
        assert_eq!(
            interp.run(&f).outcome(),
            Outcome::Failed,
            "best line 0.95 < 0.99"
        );
    }

    #[test]
    fn derived_ocr_line_threshold_uses_the_matched_line_score() {
        let roi = Rect { x: 0, y: 0, w: 64, h: 48 };
        let lines = vec![line("确认", 0.6, Rect { x: 4, y: 4, w: 20, h: 10 })];
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"appear","args":{"sel":"O_DERIVED"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );

        let mut strict = assets();
        strict.insert_ocr_lines("O_DERIVED", roi);
        strict.set_derived_ocr("O_DERIVED", roi, "确认".to_string());
        strict.set_ocr_threshold("O_DERIVED", 0.9);
        let (mut interp, _) = mk_det_ocr_with(strict, lines.clone());
        assert_eq!(
            interp.run(&f).outcome(),
            Outcome::Failed,
            "derived line 0.6 < 0.9"
        );

        let mut loose = assets();
        loose.insert_ocr_lines("O_DERIVED", roi);
        loose.set_derived_ocr("O_DERIVED", roi, "确认".to_string());
        loose.set_ocr_threshold("O_DERIVED", 0.5);
        let (mut interp, _) = mk_det_ocr_with(loose, lines);
        assert_eq!(
            interp.run(&f).outcome(),
            Outcome::Done,
            "derived line 0.6 >= 0.5"
        );
    }

    #[test]
    fn ocr_zero_threshold_equals_unset_for_click_appear_expect_and_click_target() {
        // P1-4: the editor can only clear a threshold by *writing* 0
        // (`update_asset_meta` treats `None` as "no change"), so 0 must behave
        // exactly like "unset" on every recognition path. The read is 0.02 —
        // far below the 0.9 template default — so any fallback to 0.9 fails.
        const CLICK: &str = r#"{"schema_version":1,"name":"t","steps":[
             {"op":"click","args":{"sel":"O_TEXT"}},
             {"op":"finish","args":{"exit":"done"}}]}"#;
        const APPEAR: &str = r#"{"schema_version":1,"name":"t","steps":[
             {"op":"appear","args":{"sel":"O_TEXT"}},
             {"op":"finish","args":{"exit":"done"}}]}"#;
        const COND_APPEAR: &str = r#"{"schema_version":1,"name":"t","steps":[
             {"op":"if","args":{"cond":"appear('O_TEXT')"},
              "then":[{"op":"finish","args":{"exit":"done"}}],
              "else":[{"op":"finish","args":{"exit":"failed"}}]}]}"#;
        const EXPECT_APPEAR: &str = r#"{"schema_version":1,"name":"t","steps":[
             {"op":"click_point","args":{"x":5,"y":5,
               "expect":{"kind":"appear","sel":"O_TEXT","timeout_ms":1}}},
             {"op":"finish","args":{"exit":"done"}}]}"#;
        const EXPECT_OCR_TEXT: &str = r#"{"schema_version":1,"name":"t","steps":[
             {"op":"click_point","args":{"x":5,"y":5,
               "expect":{"kind":"ocr_text","sel":"O_TEXT","text":"完成","timeout_ms":1}}},
             {"op":"finish","args":{"exit":"done"}}]}"#;
        const CLICK_TARGET: &str = r#"{"schema_version":1,"name":"t","steps":[
             {"op":"click_target","args":{"find":[
                {"kind":"ocr_text","sel":"O_TEXT","text":"完成"}]}},
             {"op":"finish","args":{"exit":"done"}}]}"#;

        for (label, steps, expected_clicks) in [
            ("click", CLICK, 1usize),
            ("appear", APPEAR, 0),
            ("if appear(...)", COND_APPEAR, 0),
            ("expect appear", EXPECT_APPEAR, 1),
            ("expect ocr_text", EXPECT_OCR_TEXT, 1),
            ("click_target ocr_text", CLICK_TARGET, 1),
        ] {
            for threshold in [None, Some(0.0f32)] {
                let mut set = assets_with_ocr();
                if let Some(t) = threshold {
                    set.set_ocr_threshold("O_TEXT", t);
                }
                let (mut interp, _, shared) = mk_ocr_with(set, &[("完成", 0.02)]);
                assert_eq!(
                    interp.run(&flow(steps)),
                    RunResult::Finished(Exit::Done),
                    "{label} must pass with threshold {threshold:?} (0.02 read)"
                );
                assert_eq!(
                    shared.clicks.borrow().len(),
                    expected_clicks,
                    "{label} click count with threshold {threshold:?}"
                );
            }
        }
    }

    #[test]
    fn ocr_expect_appear_and_vanish_distinguish_unset_zero_and_0_9() {
        // The same 0.02 read, three threshold spellings:
        // * unset / 0 -> no confidence filter -> counts as PRESENT;
        // * 0.9       -> below the minimum  -> counts as ABSENT.
        // `expect appear` must therefore accept unset/0 and reject 0.9, while
        // `expect vanish` must do the exact opposite.
        fn expect_run(kind: &str, threshold: Option<f32>) -> RunResult {
            let mut set = assets_with_ocr();
            if let Some(t) = threshold {
                set.set_ocr_threshold("O_TEXT", t);
            }
            let (mut interp, _, _) = mk_ocr_with(set, &[("完成", 0.02)]);
            let f = flow(&format!(
                r#"{{"schema_version":1,"name":"t","steps":[
                     {{"op":"click_point","args":{{"x":5,"y":5,
                       "expect":{{"kind":"{kind}","sel":"O_TEXT","timeout_ms":1}}}}}},
                     {{"op":"finish","args":{{"exit":"done"}}}}]}}"#
            ));
            interp.run(&f)
        }
        let done = |r: RunResult| r == RunResult::Finished(Exit::Done);

        for threshold in [None, Some(0.0f32)] {
            assert!(
                done(expect_run("appear", threshold)),
                "expect appear must accept the 0.02 read at {threshold:?}"
            );
            assert!(
                !done(expect_run("vanish", threshold)),
                "expect vanish must fail while the 0.02 read is present at {threshold:?}"
            );
        }

        match expect_run("appear", Some(0.9)) {
            RunResult::Failed(msg) => {
                assert!(msg.contains("expect appear not met"), "{msg}");
                assert!(msg.contains("0.020 < 0.9"), "{msg}");
            }
            other => panic!("expect appear must reject 0.02 below 0.9, got {other:?}"),
        }
        assert!(
            done(expect_run("vanish", Some(0.9))),
            "expect vanish must treat a below-threshold read as absent"
        );
    }

    #[test]
    fn ocr_zero_threshold_is_normalized_to_unset_in_public_queries() {
        // One normalization point: `ocr_threshold` hides 0, `threshold_for`
        // forwards that, and `selector_threshold` turns it into OCR's 0.0.
        let mut set = assets_with_ocr();
        set.set_ocr_threshold("O_TEXT", 0.0);
        set.set_ocr_threshold("O_DIGITS", 0.4);
        set.insert_collection("C_MIX", vec!["I_NEVER".to_string()], Some(0.7));
        let interp = Interpreter::new(
            FakeScreen::new(),
            FakeInput::new().0,
            set,
            Budget::default(),
        );
        assert_eq!(interp.assets.ocr_threshold("O_TEXT"), None, "0 = unset");
        assert_eq!(
            interp.assets.threshold_for("O_TEXT"),
            None,
            "0 must never surface as an explicit gate"
        );
        assert_eq!(interp.selector_threshold("O_TEXT"), 0.0);
        assert_eq!(interp.assets.ocr_threshold("O_DIGITS"), Some(0.4));
        assert_eq!(interp.selector_threshold("O_DIGITS"), 0.4);
        // Templates and collections keep their historical semantics.
        assert_eq!(interp.selector_threshold("I_BRIGHT"), 0.8);
        assert_eq!(interp.selector_threshold("I_NEVER"), 0.9);
        assert_eq!(interp.selector_threshold("C_MIX"), 0.7);
    }

    #[test]
    fn template_and_collection_thresholds_are_unchanged() {
        let mut set = assets_with_ocr();
        set.set_ocr_threshold("O_TEXT", 0.85);
        set.insert_collection("C_MIX", vec!["I_NEVER".to_string()], Some(0.7));
        let interp = Interpreter::new(
            FakeScreen::new(),
            FakeInput::new().0,
            set,
            Budget::default(),
        );
        // Templates still use their own NCC threshold (0.8 / 0.9 in `assets()`).
        assert_eq!(interp.selector_threshold("I_BRIGHT"), 0.8);
        assert_eq!(interp.selector_threshold("I_NEVER"), 0.9);
        // A collection override still wins.
        assert_eq!(interp.selector_threshold("C_MIX"), 0.7);
        // OCR: explicit threshold when set, 0.0 (never reject) otherwise.
        assert_eq!(interp.selector_threshold("O_TEXT"), 0.85);
        assert_eq!(interp.selector_threshold("O_DIGITS"), 0.0);
        // `threshold_for` reports a value only when one is active (> 0).
        assert_eq!(interp.assets.threshold_for("I_BRIGHT"), Some(0.8));
        assert_eq!(interp.assets.threshold_for("C_MIX"), Some(0.7));
        assert_eq!(interp.assets.threshold_for("O_TEXT"), Some(0.85));
        assert_eq!(interp.assets.threshold_for("O_DIGITS"), None);
    }

    // =======================================================================
    // 2026-11-06 参数→素材 redesign (B-T2): ocr_var reads + op_region chains
    // =======================================================================

    fn chain_line(x: i32, y: i32, w: i32, h: i32, text: &str, score: f32) -> OcrLine {
        OcrLine {
            rect: Rect { x, y, w, h },
            text: text.to_string(),
            score,
        }
    }

    /// Interpreter with a scripted det+rec provider (every call returns `lines`).
    fn mk_det_interp(
        set: AssetSet,
        lines: Vec<OcrLine>,
    ) -> (OcrInterp, OcrRois, SharedInput) {
        let (ocr, rois) = FakeDetOcr::new(lines);
        let (input, shared) = FakeInput::new();
        let interp = Interpreter::new(FakeScreen::new(), input, set, Budget::default())
            .with_humanize(no_miss())
            .with_seed(7)
            .with_ocr(Box::new(ocr));
        (interp, rois, shared)
    }

    fn params(pairs: &[(&str, &str)]) -> BTreeMap<String, Value> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), Value::Str((*v).to_string())))
            .collect()
    }

    fn inside(rect: Rect, p: Point) -> bool {
        (rect.x..rect.x + rect.w).contains(&p.x) && (rect.y..rect.y + rect.h).contains(&p.y)
    }

    fn ocr_var_assets() -> AssetSet {
        let mut set = AssetSet::new();
        set.insert_ocr_var("O_group", None, "御魂".to_string(), "group".to_string());
        set
    }

    #[test]
    fn ocr_var_expected_text_is_single_sourced() {
        let set = ocr_var_assets();
        assert_eq!(
            set.ocr_var_expected_text("O_group", &BTreeMap::new())
                .expect("runtime_text default"),
            "御魂"
        );
        assert_eq!(
            set.ocr_var_expected_text("O_group", &params(&[("group", "结界")]))
                .expect("param override"),
            "结界"
        );
        let blank = params(&[("group", "   ")]);
        assert!(set
            .ocr_var_expected_text("O_group", &blank)
            .expect_err("blank override")
            .contains("empty string"));
        let numeric = BTreeMap::from([("group".to_string(), Value::Num(3.0))]);
        assert!(set
            .ocr_var_expected_text("O_group", &numeric)
            .expect_err("non-string override")
            .contains("must be a string"));
        assert!(set.ocr_var_expected_text("NOPE", &BTreeMap::new()).is_err());
        // `is_ocr_asset` covers ocr_var even without a ROI.
        assert!(set.is_ocr_asset("O_group"));
    }

    #[test]
    fn ocr_var_reads_full_screen_with_the_param_override_and_clicks_the_real_line() {
        let (interp, rois, shared) = mk_det_interp(
            ocr_var_assets(),
            vec![
                chain_line(4, 30, 20, 10, "御魂", 0.99), // runtime_text line
                chain_line(10, 5, 24, 12, "结界", 0.90), // param-override line
            ],
        );
        let mut interp = interp.with_params(params(&[("group", "结界")]));
        let f = click_target_flow(
            r#"[{"kind":"ocr_text","sel":"O_group","text_expr":"param('group')"}]"#,
        );
        assert_eq!(
            interp.run(&f),
            RunResult::Finished(Exit::Done),
            "{:?}",
            interp.events()
        );
        let clicks = shared.clicks.borrow();
        assert_eq!(clicks.len(), 1);
        assert!(
            inside(Rect { x: 10, y: 5, w: 24, h: 12 }, clicks[0]),
            "must click the override-matched real line: {:?}",
            clicks[0]
        );
        // Whole-screen OCR: the provider was called with the whole frame.
        assert_eq!(rois.borrow()[0], Rect { x: 0, y: 0, w: 64, h: 48 });
        let detail = interp
            .events()
            .iter()
            .find(|e| e.op == "click_target")
            .map(|e| e.detail.clone())
            .expect("click_target event");
        assert!(detail.contains("text=\"结界\""), "{detail}");
    }

    #[test]
    fn ocr_var_read_defaults_to_runtime_text() {
        let (mut interp, _rois, shared) = mk_det_interp(
            ocr_var_assets(),
            vec![
                chain_line(4, 30, 20, 10, "御魂", 0.99),
                chain_line(10, 5, 24, 12, "结界", 0.90),
            ],
        );
        // No params: the literal locator text exercises the asset's runtime_text
        // (the flow-level `text_expr` resolution is covered elsewhere).
        let f = click_target_flow(r#"[{"kind":"ocr_text","sel":"O_group","text":"御魂"}]"#);
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        let clicks = shared.clicks.borrow();
        assert!(inside(Rect { x: 4, y: 30, w: 20, h: 10 }, clicks[0]));
    }

    #[test]
    fn ocr_var_below_threshold_is_loud_and_never_clicks() {
        let mut set = ocr_var_assets();
        set.set_ocr_threshold("O_group", 0.95);
        let (mut interp, _rois, shared) =
            mk_det_interp(set, vec![chain_line(4, 30, 20, 10, "御魂", 0.90)]);
        let f = click_target_flow(r#"[{"kind":"ocr_text","sel":"O_group","text":"御魂"}]"#);
        match interp.run(&f) {
            RunResult::Failed(msg) => {
                assert!(msg.contains("no line equals"), "{msg}");
            }
            other => panic!("expected a loud failure, got {other:?}"),
        }
        assert!(shared.clicks.borrow().is_empty(), "no click on a miss");
    }

    #[test]
    fn chain_gate_and_candidates_search_the_whole_frame_not_the_asset_roi() {
        // seq127 #1 + #2/#3 combined: the gate template's saved ROI only covers
        // the first tile, but the chain `nearest` pick must see the second tile
        // (full-frame candidates) and the terminal box is that second tile.
        let pattern: Vec<u8> = (0..100)
            .map(|i| (((i % 10) * 7 + (i / 10) * 13) % 251 + 3) as u8)
            .collect();
        let mut set = AssetSet::new();
        set.insert(
            "T_gate",
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
        set.insert_ocr_var("O_a", None, "甲".to_string(), "pa".to_string());
        set.insert_op_region(
            "O_a",
            AssetOpRegion::Asset {
                asset: "T_gate".to_string(),
                pick: OpRegionPick::Nearest,
            },
        );
        let frame = frame_with_two_tiles(12, 10, 40, 30);
        let (ocr, _rois) = FakeDetOcr::new(vec![chain_line(35, 32, 10, 6, "甲", 0.99)]);
        let (input, shared) = FakeInput::new();
        let mut interp = Interpreter::new(
            TwoFrameScreen {
                frames: vec![frame],
                idx: 0,
            },
            input,
            set,
            Budget::default(),
        )
        .with_humanize(no_miss())
        .with_ocr(Box::new(ocr))
        .with_seed(9);
        let f = click_target_flow(r#"[{"kind":"ocr_text","sel":"O_a","text":"甲"}]"#);
        assert_eq!(
            interp.run(&f),
            RunResult::Finished(Exit::Done),
            "{:?}",
            interp.events()
        );
        let p = shared.clicks.borrow()[0];
        let outside_roi = Rect {
            x: 40,
            y: 30,
            w: 10,
            h: 10,
        };
        let inside_roi = Rect {
            x: 12,
            y: 10,
            w: 10,
            h: 10,
        };
        assert!(
            inside(outside_roi, p) && !inside(inside_roi, p),
            "nearest must pick the full-frame match at {outside_roi:?}, got {p:?}"
        );
    }

    #[test]
    fn model_b_terminal_node_action_shape_is_translated_like_a_template() {
        // Model B: the LAST referenced node's own `action_shape` is translated by
        // its match offset (historical template semantics), not the root's.
        let pattern: Vec<u8> = (0..100)
            .map(|i| (((i % 10) * 7 + (i / 10) * 13) % 251 + 3) as u8)
            .collect();
        let mut set = AssetSet::new();
        set.insert(
            "T_gate",
            Template {
                gray: GrayImage::new(10, 10, pattern),
                roi: Some(Rect {
                    x: 0,
                    y: 0,
                    w: 64,
                    h: 48,
                }),
                threshold: 0.8,
                click_polygon: None,
            },
        );
        set.set_click_shape(
            "T_gate",
            ClickShape {
                rect: Some(Rect {
                    x: 5,
                    y: 5,
                    w: 6,
                    h: 4,
                }),
                points: None,
                invert: false,
            },
        );
        set.insert_ocr_var("O_a", None, "甲".to_string(), "pa".to_string());
        set.insert_op_region(
            "O_a",
            AssetOpRegion::Asset {
                asset: "T_gate".to_string(),
                pick: OpRegionPick::Score,
            },
        );
        let (ocr, _rois) = FakeDetOcr::new(vec![chain_line(2, 2, 10, 5, "甲", 0.99)]);
        let (input, shared) = FakeInput::new();
        let mut interp = Interpreter::new(
            TwoFrameScreen {
                frames: vec![frame_with_tile(12, 10)],
                idx: 0,
            },
            input,
            set,
            Budget::default(),
        )
        .with_humanize(no_miss())
        .with_ocr(Box::new(ocr))
        .with_seed(11);
        let f = click_target_flow(r#"[{"kind":"ocr_text","sel":"O_a","text":"甲"}]"#);
        assert_eq!(
            interp.run(&f),
            RunResult::Finished(Exit::Done),
            "{:?}",
            interp.events()
        );
        // Gate matched at (12,10) with authored roi (0,0): translated action_shape
        // = (12+5, 10+5, 6, 4) = (17,15,6,4).
        assert!(
            inside(
                Rect {
                    x: 17,
                    y: 15,
                    w: 6,
                    h: 4
                },
                shared.clicks.borrow()[0]
            ),
            "terminal action_shape must be translated: {:?}",
            shared.clicks.borrow()[0]
        );
    }

    #[test]
    fn model_b_chain_click_lands_on_the_terminal_node_geometry() {
        // 2026-11-06 seq127 #2/#3 (model B): when 引用素材 is used, the click
        // lands on the LAST referenced node's own operation area — not on the
        // root's. Score pick here -> the far high-score 乙 line.
        let (mut interp, _rois, shared) =
            mk_det_interp(asset_chain_setup(OpRegionPick::Score), asset_chain_lines());
        let f = click_target_flow(r#"[{"kind":"ocr_text","sel":"O_a","text":"甲"}]"#);
        assert_eq!(
            interp.run(&f),
            RunResult::Finished(Exit::Done),
            "{:?}",
            interp.events()
        );
        let terminal = Rect {
            x: 50,
            y: 40,
            w: 10,
            h: 5,
        };
        assert!(
            inside(terminal, shared.clicks.borrow()[0]),
            "model B must click the terminal node's box {terminal:?}, got {:?}",
            shared.clicks.borrow()[0]
        );
        let detail = interp
            .events()
            .iter()
            .find(|e| e.op == "click_target")
            .map(|e| e.detail.clone())
            .expect("click_target event");
        assert!(
            detail.contains("region_chain=[O_a:asset -> O_b:ocr]"),
            "{detail}"
        );
    }

    #[test]
    fn ocr_self_geometry_follows_the_recognized_line_when_a_roi_is_authored() {
        // 批次B (#4/Q1): with an authored ROI, the stored `action_shape` follows
        // the real recognized line by `real.topLeft - roi.topLeft`.
        let mut set = AssetSet::new();
        let authored_roi = Rect {
            x: 10,
            y: 10,
            w: 30,
            h: 12,
        };
        set.insert_ocr_var(
            "O_group",
            Some(authored_roi),
            "御魂".to_string(),
            "group".to_string(),
        );
        set.set_click_shape(
            "O_group",
            ClickShape {
                rect: Some(Rect {
                    x: 12,
                    y: 11,
                    w: 20,
                    h: 8,
                }),
                points: None,
                invert: false,
            },
        );
        // Recognized line moved by (+4, +5) from the authored ROI.
        let (mut interp, _rois, shared) =
            mk_det_interp(set, vec![chain_line(14, 15, 30, 12, "御魂", 0.97)]);
        let f = click_target_flow(r#"[{"kind":"ocr_text","sel":"O_group","text":"御魂"}]"#);
        assert_eq!(
            interp.run(&f),
            RunResult::Finished(Exit::Done),
            "{:?}",
            interp.events()
        );
        let expected = Rect {
            x: 12 + 4,
            y: 11 + 5,
            w: 20,
            h: 8,
        };
        assert!(
            inside(expected, shared.clicks.borrow()[0]),
            "OCR action_shape must follow the line: {expected:?}, got {:?}",
            shared.clicks.borrow()[0]
        );
    }

    #[test]
    fn ocr_self_geometry_stays_absolute_without_an_authored_roi() {
        // Whole-screen ocr_var: there is no authored origin to follow, so the
        // stored absolute `action_shape` is used as-is.
        let mut set = AssetSet::new();
        set.insert_ocr_var("O_full", None, "御魂".to_string(), "group".to_string());
        set.set_click_shape(
            "O_full",
            ClickShape {
                rect: Some(Rect {
                    x: 5,
                    y: 6,
                    w: 20,
                    h: 8,
                }),
                points: None,
                invert: false,
            },
        );
        let (mut interp, _rois, shared) =
            mk_det_interp(set, vec![chain_line(30, 30, 10, 5, "御魂", 0.97)]);
        let f = click_target_flow(r#"[{"kind":"ocr_text","sel":"O_full","text":"御魂"}]"#);
        assert_eq!(
            interp.run(&f),
            RunResult::Finished(Exit::Done),
            "{:?}",
            interp.events()
        );
        assert!(
            inside(
                Rect {
                    x: 5,
                    y: 6,
                    w: 20,
                    h: 8
                },
                shared.clicks.borrow()[0]
            ),
            "no roi => absolute action shape, got {:?}",
            shared.clicks.borrow()[0]
        );
    }

    /// Root ocr_var `O_a` references `O_b` through its `op_region`; provider lines
    /// contain the root line and two `乙` lines for the chained node.
    fn asset_chain_setup(pick: OpRegionPick) -> AssetSet {
        let mut set = AssetSet::new();
        set.insert_ocr_var("O_a", None, "甲".to_string(), "pa".to_string());
        set.insert_ocr_var("O_b", None, "乙".to_string(), "pb".to_string());
        set.insert_op_region(
            "O_a",
            AssetOpRegion::Asset {
                asset: "O_b".to_string(),
                pick,
            },
        );
        set
    }

    fn asset_chain_lines() -> Vec<OcrLine> {
        vec![
            chain_line(2, 2, 10, 5, "甲", 0.99),
            chain_line(6, 2, 10, 5, "乙", 0.80),   // nearest the root centre
            chain_line(50, 40, 10, 5, "乙", 0.95), // highest score but far away
        ]
    }

    #[test]
    fn op_region_chain_nearest_picks_the_closest_node_candidate() {
        let (mut interp, _rois, shared) =
            mk_det_interp(asset_chain_setup(OpRegionPick::Nearest), asset_chain_lines());
        let f = click_target_flow(r#"[{"kind":"ocr_text","sel":"O_a","text":"甲"}]"#);
        assert_eq!(
            interp.run(&f),
            RunResult::Finished(Exit::Done),
            "{:?}",
            interp.events()
        );
        let clicks = shared.clicks.borrow();
        assert!(
            inside(Rect { x: 6, y: 2, w: 10, h: 5 }, clicks[0]),
            "model B: nearest terminal candidate box: {:?}",
            clicks[0]
        );
        let detail = interp
            .events()
            .iter()
            .find(|e| e.op == "click_target")
            .map(|e| e.detail.clone())
            .expect("click_target event");
        assert!(
            detail.contains("region_chain=[O_a:asset -> O_b:ocr]"),
            "{detail}"
        );
        // `nearest` must pick the gate candidate closest to the root line centre,
        // even though the far 乙 line has the higher score.
        let preview = interp
            .preview_asset_action("O_a", None)
            .expect("preview gate pick");
        assert_eq!(preview.steps.len(), 2);
        assert_eq!(
            preview.steps[1].real,
            Some(Rect {
                x: 6,
                y: 2,
                w: 10,
                h: 5
            })
        );
    }

    #[test]
    fn op_region_chain_score_picks_the_best_scored_node_candidate() {
        let (mut interp, _rois, shared) =
            mk_det_interp(asset_chain_setup(OpRegionPick::Score), asset_chain_lines());
        let f = click_target_flow(r#"[{"kind":"ocr_text","sel":"O_a","text":"甲"}]"#);
        assert_eq!(
            interp.run(&f),
            RunResult::Finished(Exit::Done),
            "{:?}",
            interp.events()
        );
        let clicks = shared.clicks.borrow();
        assert!(
            inside(Rect { x: 50, y: 40, w: 10, h: 5 }, clicks[0]),
            "model B: highest-score terminal candidate box: {:?}",
            clicks[0]
        );
        let preview = interp
            .preview_asset_action("O_a", None)
            .expect("preview gate pick");
        assert_eq!(
            preview.steps[1].real,
            Some(Rect {
                x: 50,
                y: 40,
                w: 10,
                h: 5
            }),
            "score gate pick"
        );
    }

    #[test]
    fn region_asset_cannot_be_a_chain_node_at_run_time() {
        // 2026-11-06 批次B (#4): a region has no recognition, so it can never be
        // a chain node (its own op_region would also be refused). Save-time
        // validation rejects it; the runtime is the loud backup for old data.
        let mut set = ocr_var_assets();
        set.insert_region("R_fixed", Rect { x: 40, y: 5, w: 12, h: 9 });
        set.insert_op_region(
            "O_group",
            AssetOpRegion::Asset {
                asset: "R_fixed".to_string(),
                pick: OpRegionPick::Score,
            },
        );
        let (mut interp, _rois, shared) =
            mk_det_interp(set, vec![chain_line(4, 30, 20, 10, "御魂", 0.99)]);
        let f = click_target_flow(r#"[{"kind":"ocr_text","sel":"O_group","text":"御魂"}]"#);
        match interp.run(&f) {
            RunResult::Failed(msg) => {
                assert!(msg.contains("cannot be an op_region chain node"), "{msg}");
            }
            other => panic!("expected a loud region-node failure, got {other:?}"),
        }
        assert!(shared.clicks.borrow().is_empty(), "no click on a bad gate");
    }

    #[test]
    fn op_region_chain_template_terminal_uses_the_gate_box() {
        let mut set = assets();
        set.insert_ocr_var("O_group", None, "御魂".to_string(), "group".to_string());
        set.insert_op_region(
            "O_group",
            AssetOpRegion::Asset {
                asset: "I_BRIGHT".to_string(),
                pick: OpRegionPick::Score,
            },
        );
        let (mut interp, _rois, shared) =
            mk_det_interp(set, vec![chain_line(4, 30, 20, 10, "御魂", 0.99)]);
        let f = click_target_flow(r#"[{"kind":"ocr_text","sel":"O_group","text":"御魂"}]"#);
        assert_eq!(
            interp.run(&f),
            RunResult::Finished(Exit::Done),
            "{:?}",
            interp.events()
        );
        // Model B: the terminal template node's own matched bbox is the click
        // area (`assets()`'s tile template matches at 12,10 in FakeScreen).
        assert!(
            inside(
                Rect {
                    x: 12,
                    y: 10,
                    w: 10,
                    h: 10
                },
                shared.clicks.borrow()[0]
            ),
            "the terminal template node must provide the operation box: {:?}",
            shared.clicks.borrow()[0]
        );
    }

    #[test]
    fn op_region_chain_missing_node_fails_without_clicking() {
        let (mut interp, _rois, shared) = mk_det_interp(
            asset_chain_setup(OpRegionPick::Score),
            vec![chain_line(2, 2, 10, 5, "甲", 0.99)], // no 乙 line at all
        );
        let f = click_target_flow(r#"[{"kind":"ocr_text","sel":"O_a","text":"甲"}]"#);
        match interp.run(&f) {
            RunResult::Failed(msg) => {
                assert!(msg.contains("no line matching"), "{msg}");
            }
            other => panic!("expected a loud chain failure, got {other:?}"),
        }
        assert!(shared.clicks.borrow().is_empty(), "no click on a chain miss");
    }

    #[test]
    fn op_region_chain_cycle_fails_loudly_at_run_time() {
        let mut set = AssetSet::new();
        set.insert_ocr_var("O_a", None, "甲".to_string(), "pa".to_string());
        set.insert_ocr_var("O_b", None, "乙".to_string(), "pb".to_string());
        set.insert_op_region(
            "O_a",
            AssetOpRegion::Asset {
                asset: "O_b".to_string(),
                pick: OpRegionPick::Score,
            },
        );
        set.insert_op_region(
            "O_b",
            AssetOpRegion::Asset {
                asset: "O_a".to_string(),
                pick: OpRegionPick::Score,
            },
        );
        let (mut interp, _rois, shared) = mk_det_interp(
            set,
            vec![
                chain_line(2, 2, 10, 5, "甲", 0.99),
                chain_line(4, 4, 10, 5, "乙", 0.99),
            ],
        );
        let f = click_target_flow(r#"[{"kind":"ocr_text","sel":"O_a","text":"甲"}]"#);
        match interp.run(&f) {
            RunResult::Failed(msg) => {
                assert!(msg.contains("cycle"), "{msg}");
            }
            other => panic!("expected a cycle failure, got {other:?}"),
        }
        assert!(shared.clicks.borrow().is_empty());
    }

    #[test]
    fn flow_region_keeps_priority_over_an_asset_op_region_chain() {
        let mut set = assets();
        set.insert_ocr_var("O_group", None, "御魂".to_string(), "group".to_string());
        set.insert_op_region(
            "O_group",
            AssetOpRegion::Asset {
                asset: "I_BRIGHT".to_string(),
                pick: OpRegionPick::Score,
            },
        );
        let (mut interp, _rois, shared) =
            mk_det_interp(set, vec![chain_line(10, 10, 30, 12, "御魂", 0.97)]);
        // The legacy flow-level `region` must win over the asset chain.
        let f = click_target_flow(
            r#"[{"kind":"ocr_text","sel":"O_group","text":"御魂",
                 "region":{"mode":"offset","offset":[1,2,6,4]}}]"#,
        );
        assert_eq!(
            interp.run(&f),
            RunResult::Finished(Exit::Done),
            "{:?}",
            interp.events()
        );
        let expected = Rect {
            x: 10 + 1,
            y: 10 + 2,
            w: 6,
            h: 4,
        };
        assert!(
            inside(expected, shared.clicks.borrow()[0]),
            "flow region box {expected:?} must win, got {:?}",
            shared.clicks.borrow()[0]
        );
    }

    #[test]
    fn long_press_uses_the_asset_op_region_chain_terminal_box() {
        let mut set = assets();
        set.insert_ocr_var("O_group", None, "御魂".to_string(), "group".to_string());
        set.insert_op_region(
            "O_group",
            AssetOpRegion::Asset {
                asset: "I_BRIGHT".to_string(),
                pick: OpRegionPick::Score,
            },
        );
        // The root line is placed away from the template gate's match (12,10) so
        // this test really proves model B (terminal box), not an overlap.
        let (mut interp, _rois, shared) =
            mk_det_interp(set, vec![chain_line(10, 34, 20, 10, "御魂", 0.97)]);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"long_press","args":{
                   "find":[{"kind":"ocr_text","sel":"O_group","text":"御魂"}]}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(
            interp.run(&f),
            RunResult::Finished(Exit::Done),
            "{:?}",
            interp.events()
        );
        let presses = shared.long_presses.borrow();
        assert_eq!(presses.len(), 1);
        assert!(
            inside(
                Rect {
                    x: 12,
                    y: 10,
                    w: 10,
                    h: 10
                },
                presses[0].0
            ),
            "model B hold point = terminal template box: {:?}",
            presses[0].0
        );
    }

    #[test]
    fn recognition_candidates_covers_templates_ocr_and_region() {
        // seq134 single source: templates, OCR (strict expected text + threshold),
        // and fixed regions all enumerate here; no asset-kind-specific helper.
        let mut tpl_interp = Interpreter::new(
            FakeScreen::new(),
            FakeInput::new().0,
            assets(),
            Budget::default(),
        )
        .with_humanize(no_miss());
        let tpl = tpl_interp
            .recognition_candidates("I_BRIGHT")
            .expect("template candidates");
        assert_eq!(tpl.len(), 1);
        assert_eq!((tpl[0].x, tpl[0].y, tpl[0].w, tpl[0].h), (12, 10, 10, 10));
        assert!(tpl[0].score >= 0.8);
        assert!(tpl[0].text.is_none());
        assert_eq!(tpl[0].member_kind, "template");

        let lines = vec![
            chain_line(10, 5, 20, 10, "开始", 0.90),
            chain_line(10, 30, 20, 10, "结束", 0.99),
            chain_line(40, 5, 20, 10, "开始", 0.50), // below threshold
        ];
        let mut ocr_set = AssetSet::new();
        ocr_set.insert_ocr_lines("O_list", Rect { x: 0, y: 0, w: 64, h: 48 });
        ocr_set.set_ocr_threshold("O_list", 0.6);
        ocr_set.set_ocr_expected_text("O_list", "开始".to_string());
        let (mut ocr_interp, _rois, _shared) = mk_det_interp(ocr_set, lines.clone());
        let ocr = ocr_interp
            .recognition_candidates("O_list")
            .expect("ocr candidates");
        assert_eq!(ocr.len(), 1, "{ocr:?}");
        assert_eq!(ocr[0].text.as_deref(), Some("开始"));
        assert_eq!(
            Rect { x: ocr[0].x, y: ocr[0].y, w: ocr[0].w, h: ocr[0].h },
            Rect { x: 10, y: 5, w: 20, h: 10 }
        );
        assert_eq!(ocr[0].member_kind, "ocr_lines");

        // No expected text: every ROI line at/above the OCR threshold, sorted
        // score desc → y → x (the 0.50 line is filtered).
        let mut plain_set = AssetSet::new();
        plain_set.insert_ocr_lines("O_list", Rect { x: 0, y: 0, w: 64, h: 48 });
        plain_set.set_ocr_threshold("O_list", 0.6);
        let (mut plain_interp, _rois, _shared) = mk_det_interp(plain_set, lines);
        let all = plain_interp
            .recognition_candidates("O_list")
            .expect("all lines");
        assert_eq!(all.len(), 2, "{all:?}");
        assert_eq!(all[0].text.as_deref(), Some("结束"));

        let mut region_set = AssetSet::new();
        region_set.insert_region("R", Rect { x: 5, y: 6, w: 7, h: 8 });
        let mut region_interp = Interpreter::new(
            FakeScreen::new(),
            FakeInput::new().0,
            region_set,
            Budget::default(),
        )
        .with_humanize(no_miss());
        let region = region_interp
            .recognition_candidates("R")
            .expect("region candidate");
        assert_eq!(region.len(), 1);
        assert_eq!((region[0].x, region[0].y, region[0].w, region[0].h), (5, 6, 7, 8));
        assert_eq!(region[0].member_kind, "region");

        // Missing data sources are loud errors (undeclared asset / no OCR provider).
        let mut empty_interp = Interpreter::new(
            FakeScreen::new(),
            FakeInput::new().0,
            AssetSet::new(),
            Budget::default(),
        );
        assert!(empty_interp.recognition_candidates("NOPE").is_err());
        let mut no_provider_set = AssetSet::new();
        no_provider_set.insert_ocr_lines("O_list", Rect { x: 0, y: 0, w: 64, h: 48 });
        let mut no_provider = Interpreter::new(
            FakeScreen::new(),
            FakeInput::new().0,
            no_provider_set,
            Budget::default(),
        );
        assert!(no_provider.recognition_candidates("O_list").is_err());
    }

    #[test]
    fn preview_asset_action_model_b_uses_the_terminal_box() {
        let (mut interp, _rois, _shared) =
            mk_det_interp(asset_chain_setup(OpRegionPick::Score), asset_chain_lines());
        let preview = interp
            .preview_asset_action("O_a", None)
            .expect("preview");
        // Model B: the action box is the TERMINAL node's own geometry (the far
        // high-score 乙 line picked by `preset = score`).
        assert_eq!(
            preview.action_box,
            Rect {
                x: 50,
                y: 40,
                w: 10,
                h: 5
            }
        );
        assert!(inside(preview.action_box, preview.point));
        // The root's real line is still reported as `real` (the recognition).
        assert_eq!(
            preview.real,
            Some(Rect {
                x: 2,
                y: 2,
                w: 10,
                h: 5
            })
        );
        assert_eq!(preview.steps.len(), 2);
        assert_eq!(preview.steps[0].mode, "asset");
        assert_eq!(preview.steps[1].mode, "ocr");
        assert_eq!(
            preview.steps[1].real,
            Some(Rect {
                x: 50,
                y: 40,
                w: 10,
                h: 5
            })
        );
    }

    #[test]
    fn plain_ocr_lines_chain_gate_uses_its_expected_text() {
        // 2026-11-06 批次B (#3): a plain `ocr_lines` asset with a saved expected
        // text is a valid recognition gate (det+rec, multi-candidate pick).
        let mut set = AssetSet::new();
        set.insert_ocr_var("O_a", None, "甲".to_string(), "pa".to_string());
        set.insert_ocr_lines(
            "O_gate",
            Rect {
                x: 0,
                y: 0,
                w: 64,
                h: 48,
            },
        );
        set.set_ocr_expected_text("O_gate", "乙".to_string());
        set.insert_op_region(
            "O_a",
            AssetOpRegion::Asset {
                asset: "O_gate".to_string(),
                pick: OpRegionPick::Score,
            },
        );
        let (mut interp, _rois, shared) = mk_det_interp(
            set,
            vec![
                chain_line(2, 2, 10, 5, "甲", 0.99),
                chain_line(20, 20, 10, 5, "乙", 0.95),
            ],
        );
        let f = click_target_flow(r#"[{"kind":"ocr_text","sel":"O_a","text":"甲"}]"#);
        assert_eq!(
            interp.run(&f),
            RunResult::Finished(Exit::Done),
            "{:?}",
            interp.events()
        );
        assert!(
            inside(
                Rect {
                    x: 20,
                    y: 20,
                    w: 10,
                    h: 5
                },
                shared.clicks.borrow()[0]
            ),
            "model B: terminal plain-OCR gate box, got {:?}",
            shared.clicks.borrow()[0]
        );
        let preview = interp
            .preview_asset_action("O_a", None)
            .expect("preview plain-OCR gate");
        assert_eq!(preview.steps.len(), 2);
        assert_eq!(preview.steps[1].mode, "ocr");
        assert_eq!(preview.steps[1].text.as_deref(), Some("乙"));
        assert_eq!(
            preview.steps[1].real,
            Some(Rect {
                x: 20,
                y: 20,
                w: 10,
                h: 5
            })
        );
    }

    #[test]
    fn plain_ocr_chain_gate_without_expected_text_fails_loudly() {
        // #3: save-time validation requires `ocr_text`, but the runtime must also
        // fail loudly on hand-edited/global-store data instead of guessing.
        let mut set = AssetSet::new();
        set.insert_ocr_var("O_a", None, "甲".to_string(), "pa".to_string());
        set.insert_ocr_lines(
            "O_gate",
            Rect {
                x: 0,
                y: 0,
                w: 64,
                h: 48,
            },
        );
        set.insert_op_region(
            "O_a",
            AssetOpRegion::Asset {
                asset: "O_gate".to_string(),
                pick: OpRegionPick::Score,
            },
        );
        let (mut interp, _rois, shared) = mk_det_interp(
            set,
            vec![
                chain_line(2, 2, 10, 5, "甲", 0.99),
                chain_line(20, 20, 10, 5, "乙", 0.95),
            ],
        );
        let f = click_target_flow(r#"[{"kind":"ocr_text","sel":"O_a","text":"甲"}]"#);
        match interp.run(&f) {
            RunResult::Failed(msg) => {
                assert!(msg.contains("without an expected text"), "{msg}");
            }
            other => panic!("expected a loud gate failure, got {other:?}"),
        }
        assert!(shared.clicks.borrow().is_empty());
    }

    #[test]
    fn ocr_var_polygon_masks_the_frame_before_det_rec() {
        // 批次B (#6/#10): an ocr_var may now carry a recognition polygon; the
        // bbox pixels OUTSIDE it must be whitened before det+rec, exactly like the
        // plain/derived OCR paths.
        let roi = Rect {
            x: 10,
            y: 5,
            w: 20,
            h: 8,
        };
        let mut set = AssetSet::new();
        set.insert_ocr_var("O_mask", Some(roi), "确认".to_string(), "k".to_string());
        set.set_ocr_polygon("O_mask", probe_triangle());
        let matched = line("确认", 0.97, roi);
        let (ocr, rois, frames) = FrameProbeDetOcr::new(vec![matched.clone()]);
        let screen = TwoFrameScreen {
            frames: vec![derived_polygon_probe_frame()],
            idx: 0,
        };
        let (input, shared) = FakeInput::new();
        let mut interp = Interpreter::new(screen, input, set, Budget::default())
            .with_humanize(no_miss())
            .with_ocr(Box::new(ocr))
            .with_seed(43);
        let f = click_target_flow(r#"[{"kind":"ocr_text","sel":"O_mask","text":"确认"}]"#);
        assert_eq!(interp.run(&f), RunResult::Finished(Exit::Done));
        assert_eq!(rois.borrow().as_slice(), &[roi]);
        let seen = frames.borrow();
        assert_eq!(seen.len(), 1, "exactly one det+rec call");
        // (10,5) is inside the probe triangle → survives; (29,5) is inside the
        // bbox but outside the triangle → becomes white.
        assert_eq!(frame_pixel(&seen[0], 10, 5), 100);
        assert_eq!(frame_pixel(&seen[0], 29, 5), 255);
        assert_eq!(shared.clicks.borrow().len(), 1);
    }

    #[test]
    fn preview_single_line_ocr_target_enumerates_det_rec_candidates() {
        // 批次B (#3): the editor preview for a plain `ocr_line` target must use
        // det+rec (multi-candidate), so `candidate_index >= 1` is meaningful and
        // matches the candidates API. Runtime click keeps the single-read path.
        let mut set = AssetSet::new();
        set.insert_ocr(
            "O_ONE",
            Rect {
                x: 0,
                y: 0,
                w: 64,
                h: 48,
            },
        );
        let second = Rect {
            x: 40,
            y: 30,
            w: 10,
            h: 5,
        };
        let (mut interp, _rois, _shared) = mk_det_interp(
            set,
            vec![
                chain_line(10, 5, 20, 10, "甲", 0.99),
                chain_line(40, 30, 10, 5, "乙", 0.90),
            ],
        );
        let first = interp
            .preview_asset_action("O_ONE", None)
            .expect("best candidate");
        assert_eq!(first.real, Some(Rect { x: 10, y: 5, w: 20, h: 10 }));
        let idx1 = interp
            .preview_asset_action("O_ONE", Some(1))
            .expect("second candidate");
        assert_eq!(idx1.real, Some(second));
        assert!(interp.preview_asset_action("O_ONE", Some(9)).is_err());
    }

    #[test]
    fn preview_ocr_candidates_are_strict_matches_in_candidate_order() {
        // seq134 A1/A3: OCR candidates are the strict expected-text matches over the
        // WHOLE frame; non-matching lines are not candidates at all (the seq133
        // "all det lines, pass-first" model was retired by the seq134 ruling).
        let (mut interp, _rois, _shared) = mk_det_interp(
            ocr_var_assets(),
            vec![
                chain_line(10, 10, 20, 10, "御魂", 0.90),
                chain_line(10, 40, 20, 10, "御魂", 0.80),
                chain_line(10, 30, 20, 10, "结束", 0.99),
            ],
        );
        let idx0 = interp.preview_asset_action("O_group", None).expect("index 0");
        assert_eq!(idx0.real, Some(Rect { x: 10, y: 10, w: 20, h: 10 }));
        assert_eq!(idx0.steps[0].text.as_deref(), Some("御魂"));
        let idx1 = interp
            .preview_asset_action("O_group", Some(1))
            .expect("index 1");
        assert_eq!(idx1.real, Some(Rect { x: 10, y: 40, w: 20, h: 10 }));
        // Only the 2 strict matches are candidates: index 2 is out of range.
        assert!(interp.preview_asset_action("O_group", Some(2)).is_err());
    }

    #[test]
    fn preview_asset_action_honours_candidate_index_and_region_targets() {
        let (mut interp, _rois, _shared) = mk_det_interp(
            ocr_var_assets(),
            vec![
                chain_line(4, 30, 20, 10, "御魂", 0.90),
                chain_line(10, 5, 24, 12, "御魂", 0.99),
            ],
        );
        // Best candidate = highest score.
        let best = interp
            .preview_asset_action("O_group", None)
            .expect("best preview");
        assert_eq!(
            best.real,
            Some(Rect {
                x: 10,
                y: 5,
                w: 24,
                h: 12
            })
        );
        // candidate_index follows the candidates-API order (score desc).
        let second = interp
            .preview_asset_action("O_group", Some(1))
            .expect("second candidate");
        assert_eq!(
            second.real,
            Some(Rect {
                x: 4,
                y: 30,
                w: 20,
                h: 10
            })
        );
        assert!(interp.preview_asset_action("O_group", Some(9)).is_err());

        let mut region_set = AssetSet::new();
        region_set.insert_region("R_fixed", Rect { x: 5, y: 6, w: 7, h: 8 });
        let (input, _shared) = FakeInput::new();
        let mut region_interp = Interpreter::new(
            FakeScreen::new(),
            input,
            region_set,
            Budget::default(),
        )
        .with_humanize(no_miss());
        let region = region_interp
            .preview_asset_action("R_fixed", None)
            .expect("region preview");
        assert_eq!(
            region.action_box,
            Rect {
                x: 5,
                y: 6,
                w: 7,
                h: 8
            }
        );
        assert_eq!(region.steps[0].mode, "region");
        assert_eq!(region.real, None);
        assert!(inside(region.action_box, region.point));
    }

    #[test]
    fn resolve_asset_op_region_returns_none_for_assets_without_a_chain() {
        let (mut interp, _rois, _shared) = mk_det_interp(
            {
                let mut set = ocr_var_assets();
                // Explicit self = default = historical path.
                set.insert_op_region("O_group", AssetOpRegion::SelfRegion);
                set
            },
            vec![chain_line(4, 30, 20, 10, "御魂", 0.99)],
        );
        assert!(!interp.assets.has_op_region_chain("O_group"));
        let resolved = interp
            .resolve_asset_op_region(
                "O_group",
                Rect { x: 4, y: 30, w: 20, h: 10 },
                Rect { x: 4, y: 30, w: 20, h: 10 },
                None,
                "test",
            )
            .expect("resolution");
        assert!(resolved.is_none(), "self chain must keep the old path");
        assert!(!interp.assets.has_op_region_chain("NOT_DECLARED"));
    }

    // ------------------------------------------------------------------
    // seq134 single-source recognition tests
    // ------------------------------------------------------------------

    /// det+rec provider that only returns the lines inside the requested ROI
    /// (the real provider contract) and records every ROI it saw.
    struct RoiFilterDetOcr {
        lines: Vec<OcrLine>,
        rois: std::rc::Rc<std::cell::RefCell<Vec<Rect>>>,
    }

    impl RoiFilterDetOcr {
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

    impl OcrProvider for RoiFilterDetOcr {
        fn recognize(&mut self, _frame: &Frame, _roi: Rect) -> Result<(String, f32), String> {
            Err("RoiFilterDetOcr only supports det+rec".into())
        }

        fn recognize_lines(&mut self, _frame: &Frame, roi: Rect) -> Result<Vec<OcrLine>, String> {
            self.rois.borrow_mut().push(roi);
            Ok(self
                .lines
                .iter()
                .filter(|l| rect_inside_rect(roi, l.rect))
                .cloned()
                .collect())
        }
    }

    #[test]
    fn image_and_text_multi_candidate_orders_are_isomorphic() {
        // Two exact-ish tiles in the frame (as in seq127) and two identical OCR
        // lines at the same visual positions: both paths must expose the same
        // multi-candidate shape/order (score desc → y → x).
        let frame = frame_with_two_tiles(12, 10, 40, 6);
        let mut set = ocr_var_assets();
        let mut tpl = assets().get("I_BRIGHT").expect("I_BRIGHT").clone();
        tpl.threshold = 0.5; // capture both tile occurrences
        set.insert("TPL_MULTI", tpl);
        let (ocr, _rois) = FakeDetOcr::new(vec![
            chain_line(12, 10, 10, 10, "御魂", 0.99),
            chain_line(40, 6, 10, 10, "御魂", 0.97),
        ]);
        let (input, _shared) = FakeInput::new();
        let mut interp = Interpreter::new(
            TwoFrameScreen {
                frames: vec![frame],
                idx: 0,
            },
            input,
            set,
            Budget::default(),
        )
        .with_humanize(no_miss())
        .with_ocr(Box::new(ocr));
        let image = interp.recognition_candidates("TPL_MULTI").expect("image candidates");
        let text = interp.recognition_candidates("O_group").expect("text candidates");
        assert!(image.len() >= 2, "{image:?}");
        assert!(text.len() >= 2, "{text:?}");
        for list in [&image, &text] {
            for pair in list.windows(2) {
                let (a, b) = (&pair[0], &pair[1]);
                assert!(
                    a.score > b.score || (a.score == b.score && a.y <= b.y),
                    "candidates must be score desc → y: {list:?}"
                );
            }
        }
        assert_eq!((image[0].x, image[0].y), (12, 10), "{image:?}");
        assert_eq!((text[0].x, text[0].y), (12, 10), "{text:?}");
        assert_eq!(image[0].member_kind, "template");
        assert_eq!(text[0].member_kind, "ocr_var");
        assert_eq!(text[0].text.as_deref(), Some("御魂"));
        // P0 cross-path consistency: `test_action`/chain candidate_index uses the
        // SAME list/order as `recognition_candidates`.
        for idx in [0usize, 1] {
            let preview = interp.preview_asset_action("O_group", Some(idx)).expect("preview");
            assert_eq!(
                preview.real,
                Some(Rect {
                    x: text[idx].x,
                    y: text[idx].y,
                    w: text[idx].w,
                    h: text[idx].h
                }),
                "candidate_index {idx} must select the same OCR candidate"
            );
            assert_eq!(preview.steps[0].score, Some(text[idx].score));
        }
    }

    #[test]
    fn ocr_roi_miss_falls_back_to_the_whole_frame() {
        let mut set = ocr_var_assets();
        set.insert_ocr_var(
            "O_tight",
            Some(Rect {
                x: 0,
                y: 0,
                w: 5,
                h: 5,
            }),
            "御魂".to_string(),
            "p_tight".to_string(),
        );
        let wanted = Rect {
            x: 40,
            y: 30,
            w: 10,
            h: 5,
        };
        let (ocr, rois) = RoiFilterDetOcr::new(vec![chain_line(40, 30, 10, 5, "御魂", 0.91)]);
        let (input, _shared) = FakeInput::new();
        let mut interp = Interpreter::new(
            TwoFrameScreen {
                frames: vec![frame_with_tile(12, 10)],
                idx: 0,
            },
            input,
            set,
            Budget::default(),
        )
        .with_humanize(no_miss())
        .with_ocr(Box::new(ocr));
        let best = interp.recognize_best("O_tight").expect("whole-frame fallback");
        assert_eq!(best.domain, CandidateDomain::ScreenFallback);
        assert_eq!(best.real, Some(wanted));
        assert!((best.score - 0.91).abs() < 1e-6);
        // ROI was tried first, then the whole frame in the SAME observation.
        let seen: Vec<Rect> = rois.borrow().clone();
        assert_eq!(seen.len(), 2, "ROI then fallback: {seen:?}");
        assert_eq!(
            seen[0],
            Rect {
                x: 0,
                y: 0,
                w: 5,
                h: 5
            }
        );
        assert_eq!(
            seen[1],
            Rect {
                x: 0,
                y: 0,
                w: 64,
                h: 48
            }
        );
        // Candidates are the same single-source full-frame enumeration; a match
        // outside the declared ROI is annotated as the fallback domain.
        let cands = interp.recognition_candidates("O_tight").expect("candidates");
        assert_eq!(cands.len(), 1, "{cands:?}");
        assert_eq!(cands[0].domain, CandidateDomain::ScreenFallback);
        let preview = interp.preview_asset_action("O_tight", None).expect("preview");
        assert_eq!(preview.real, Some(wanted));
    }

    #[test]
    fn plain_ocr_without_expected_text_stays_in_its_roi() {
        let mut set = assets_with_ocr();
        let declared = Rect {
            x: 0,
            y: 0,
            w: 10,
            h: 10,
        };
        set.insert_ocr_lines("O_LIMIT", declared);
        let (ocr, rois) = RoiFilterDetOcr::new(vec![
            chain_line(2, 2, 5, 5, "内", 0.90),
            chain_line(40, 30, 10, 5, "外", 0.99),
        ]);
        let (input, _shared) = FakeInput::new();
        let mut interp = Interpreter::new(
            TwoFrameScreen {
                frames: vec![frame_with_tile(12, 10)],
                idx: 0,
            },
            input,
            set,
            Budget::default(),
        )
        .with_humanize(no_miss())
        .with_ocr(Box::new(ocr));
        let cands = interp.recognition_candidates("O_LIMIT").expect("candidates");
        assert_eq!(cands.len(), 1, "only the in-ROI line: {cands:?}");
        assert_eq!(cands[0].text.as_deref(), Some("内"));
        assert_eq!(cands[0].domain, CandidateDomain::Roi);
        // seq134 #4: no full-frame retry for a no-expected plain OCR asset.
        assert_eq!(rois.borrow().as_slice(), &[declared]);
    }

    #[test]
    fn strict_expected_text_rejects_near_variants_at_runtime() {
        let mut set = AssetSet::new();
        set.insert_ocr_var("O_strict", None, "队伍1".to_string(), "p".to_string());
        let (ocr, _rois) = RoiFilterDetOcr::new(vec![
            chain_line(0, 0, 10, 5, "队伍2", 0.99),
            chain_line(0, 10, 10, 5, "队伍3", 0.98),
        ]);
        let (input, _shared) = FakeInput::new();
        let mut interp = Interpreter::new(
            TwoFrameScreen {
                frames: vec![frame_with_tile(12, 10)],
                idx: 0,
            },
            input,
            set,
            Budget::default(),
        )
        .with_humanize(no_miss())
        .with_ocr(Box::new(ocr));
        assert!(
            interp.recognition_candidates("O_strict").unwrap().is_empty(),
            "strict must not accept 队伍2/队伍3 for 队伍1"
        );
        assert!(interp.recognize_best("O_strict").is_err());
    }

    #[test]
    fn collection_chain_node_enumerates_every_member_kind() {
        let mut set = assets();
        set.insert_ocr_var("O_group", None, "御魂".to_string(), "p".to_string());
        set.insert_region(
            "R_MIX",
            Rect {
                x: 30,
                y: 20,
                w: 10,
                h: 10,
            },
        );
        set.insert_collection(
            "C_MIX",
            vec!["I_BRIGHT".to_string(), "O_group".to_string(), "R_MIX".to_string()],
            None,
        );
        set.insert_op_region(
            "I_NEVER",
            AssetOpRegion::Asset {
                asset: "C_MIX".to_string(),
                pick: OpRegionPick::Score,
            },
        );
        let (mut interp, _rois, _shared) = mk_det_interp(
            set,
            vec![chain_line(4, 4, 20, 10, "御魂", 0.97)],
        );
        let all = interp.recognition_candidates("C_MIX").expect("collection candidates");
        assert!(all.iter().any(|c| c.member_kind == "template"), "{all:?}");
        assert!(all.iter().any(|c| c.member_kind == "ocr_var"), "{all:?}");
        assert!(all.iter().any(|c| c.member_kind == "region"), "{all:?}");
        let resolved = interp
            .resolve_asset_op_region(
                "I_NEVER",
                Rect {
                    x: 12,
                    y: 10,
                    w: 10,
                    h: 10,
                },
                Rect {
                    x: 12,
                    y: 10,
                    w: 10,
                    h: 10,
                },
                None,
                "test",
            )
            .expect("chain resolution")
            .expect("some chain");
        let last = resolved.chain.last().expect("terminal step");
        assert_eq!(last.asset, "I_BRIGHT");
        assert!(
            last.candidate_count > 1,
            "collection chain node must record all-member candidates: {last:?}"
        );
    }

    #[test]
    fn appear_requires_a_recognized_line_even_without_threshold() {
        let (mut interp, _rois) = mk_ocr(&[("", 0.5)]);
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"appear","args":{"sel":"O_TEXT"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert!(
            matches!(interp.run(&f), RunResult::Failed(_)),
            "an empty OCR read must not satisfy appear: {:?}",
            interp.events()
        );
    }

    #[test]
    fn drag_ocr_text_picks_the_highest_score_line() {
        let mut set = assets_with_ocr();
        set.insert_ocr_lines(
            "O_DRAG",
            Rect {
                x: 0,
                y: 0,
                w: 64,
                h: 48,
            },
        );
        let (mut interp, _rois, _shared) = mk_det_interp(
            set,
            vec![
                chain_line(10, 10, 20, 5, "甲", 0.5),
                chain_line(40, 20, 20, 5, "甲", 0.95),
            ],
        );
        let t = DragTarget::OcrText {
            sel: Some("O_DRAG".to_string()),
            roi: None,
            text: oas_flow::TextSpec::Literal("甲".to_string()),
            region: None,
        };
        let endpoint = interp.drag_target(&t, "test").expect("drag endpoint");
        assert_eq!(
            endpoint.rect,
            Rect {
                x: 40,
                y: 20,
                w: 20,
                h: 5
            },
            "drag ocr_text must pick the highest score, not the first line"
        );
    }

    #[test]
    fn plain_ocr_click_follows_the_recognized_line() {
        let mut set = assets_with_ocr();
        set.insert_ocr_lines(
            "O_LINES",
            Rect {
                x: 0,
                y: 0,
                w: 64,
                h: 48,
            },
        );
        let (mut interp, _rois, shared) = mk_det_interp(
            set,
            vec![
                chain_line(10, 10, 20, 5, "甲", 0.90),
                chain_line(40, 20, 20, 5, "甲", 0.96),
            ],
        );
        let f = flow(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click","args":{"sel":"O_LINES"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        assert_eq!(
            interp.run(&f),
            RunResult::Finished(Exit::Done),
            "{:?}",
            interp.events()
        );
        let p = shared.clicks.borrow()[0];
        assert!(
            inside(
                Rect {
                    x: 40,
                    y: 20,
                    w: 20,
                    h: 5
                },
                p
            ),
            "plain OCR click must follow the highest recognized line, got {p:?}"
        );
    }
}

#[cfg(test)]
mod click_shape_tests {
    use super::*;

    #[test]
    fn polygon_and_invert_samples_are_effective() {
        let mut rng = crate::humanize::Rng::new(7);
        let poly = ClickShape {
            rect: None,
            points: Some(vec![
                Point { x: 100, y: 100 },
                Point { x: 400, y: 100 },
                Point { x: 400, y: 300 },
                Point { x: 100, y: 300 },
            ]),
            invert: false,
        };
        for _ in 0..64 {
            let p = poly.sample(640, 360, &mut rng).expect("polygon sample");
            assert!(poly.contains(p), "sample outside polygon: {p:?}");
        }

        let inverted = ClickShape {
            rect: Some(Rect { x: 100, y: 100, w: 100, h: 100 }),
            points: None,
            invert: true,
        };
        for _ in 0..64 {
            let p = inverted.sample(640, 360, &mut rng).expect("invert sample");
            assert!(!inverted.contains(p), "sample inside excluded rect: {p:?}");
        }
    }

    #[test]
    fn polygon_contains_handles_negative_slope_edges() {
        let tri = ClickShape {
            rect: None,
            points: Some(vec![
                Point { x: 100, y: 100 },
                Point { x: 400, y: 100 },
                Point { x: 100, y: 400 },
            ]),
            invert: false,
        };
        assert!(tri.contains(Point { x: 120, y: 120 }));
        assert!(tri.contains(Point { x: 200, y: 180 }));
        assert!(!tri.contains(Point { x: 350, y: 350 }));
    }

    #[test]
    fn empty_invert_and_degenerate_polygon_samples_are_loud_errors() {
        // B-03: a full-screen invert has no effective point; the old code
        // returned `None`/bbox centre and the click still happened.
        let mut rng = crate::humanize::Rng::new(3);
        let full = ClickShape {
            rect: Some(Rect {
                x: 0,
                y: 0,
                w: 64,
                h: 48,
            }),
            points: None,
            invert: true,
        };
        let err = full
            .sample(64, 48, &mut rng)
            .expect_err("empty effective region must fail");
        assert!(err.contains("no clickable point"), "{err}");

        // Collinear polygon: bbox exists but no pixel is inside.
        let collinear = ClickShape {
            rect: None,
            points: Some(vec![
                Point { x: 10, y: 10 },
                Point { x: 20, y: 10 },
                Point { x: 30, y: 10 },
            ]),
            invert: false,
        };
        let err = collinear
            .sample(640, 360, &mut rng)
            .expect_err("degenerate polygon must fail");
        assert!(err.contains("zero area"), "{err}");

        let too_few = ClickShape {
            rect: None,
            points: Some(vec![Point { x: 10, y: 10 }, Point { x: 20, y: 20 }]),
            invert: false,
        };
        let err = too_few
            .sample(640, 360, &mut rng)
            .expect_err("2-point polygon must fail");
        assert!(err.contains("at least 3"), "{err}");
    }

    #[test]
    fn bbox_and_contains_handle_i32_limits_without_overflow() {
        // P1-4: `rect.x + rect.w` used to overflow for a rect at i32::MAX.
        let at_max = ClickShape {
            rect: Some(Rect {
                x: i32::MAX,
                y: 0,
                w: 1,
                h: 1,
            }),
            points: None,
            invert: false,
        };
        assert_eq!(
            at_max.bbox(),
            Some(Rect {
                x: i32::MAX,
                y: 0,
                w: 1,
                h: 1
            })
        );
        assert!(at_max.contains(Point { x: i32::MAX, y: 0 }));
        assert!(!at_max.contains(Point { x: i32::MIN, y: 0 }));
        assert!(!at_max.contains(Point { x: i32::MAX, y: 1 }));

        // A polygon spanning 2^31 pixels cannot be expressed as an i32 rect:
        // bbox must be None (no `x1 - x0 + 1` overflow/panic), and `contains`
        // must still answer without integer overflow.
        let wide = ClickShape {
            rect: None,
            points: Some(vec![
                Point { x: 0, y: 0 },
                Point { x: i32::MAX, y: 0 },
                Point { x: i32::MAX, y: i32::MAX },
            ]),
            invert: false,
        };
        assert!(wide.bbox().is_none(), "2^31-wide bbox cannot fit i32");
        assert!(wide.contains(Point {
            x: i32::MAX - 1,
            y: 1
        }));
        assert!(!wide.contains(Point { x: 0, y: i32::MAX }));
    }

    #[test]
    fn sample_off_frame_and_overflowing_bbox_are_loud_errors() {
        // P1-4: sample must reject geometry it cannot represent or that is
        // entirely off-screen, loudly and without panicking.
        let mut rng = crate::humanize::Rng::new(11);
        let off_frame = ClickShape {
            rect: Some(Rect {
                x: i32::MAX,
                y: 0,
                w: 1,
                h: 1,
            }),
            points: None,
            invert: false,
        };
        let err = off_frame
            .sample(64, 48, &mut rng)
            .expect_err("off-frame rect must fail");
        assert!(err.contains("does not intersect"), "{err}");

        let overflowing = ClickShape {
            rect: None,
            points: Some(vec![
                Point { x: 0, y: 0 },
                Point { x: i32::MAX, y: 0 },
                Point { x: i32::MAX, y: i32::MAX },
            ]),
            invert: false,
        };
        let err = overflowing
            .sample(64, 48, &mut rng)
            .expect_err("overflowing bbox must fail");
        assert!(err.contains("does not fit the i32"), "{err}");

        // A degenerate rect is a loud error too (parser rejects it, runtime
        // defense keeps programmatic shapes honest).
        let degenerate = ClickShape {
            rect: Some(Rect {
                x: 0,
                y: 0,
                w: 0,
                h: 10,
            }),
            points: None,
            invert: false,
        };
        let err = degenerate
            .sample(64, 48, &mut rng)
            .expect_err("zero-width rect must fail");
        assert!(err.contains("non-positive size"), "{err}");
    }
}

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}
