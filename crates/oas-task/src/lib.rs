//! oas-task: on-disk **task bundle** persistence (N3, design `ENGINE-DESIGN.md`
//! §18.1 / §19.3 / §22.2).
//!
//! # Layout
//!
//! ```text
//! <tasks-dir>/<任务名>/
//!   meta.json      {"schema_version":1,"name":"<任务名>","expire":"daily"}
//!   flow.json      {"schema_version":1,"name":"...","steps":[...]}
//!   assets.json    {"schema_version":1,"design_resolution":[w,h],
//!                   "items":[{"id","kind","file","roi","threshold","rev"}]}
//!   images/a001.png, images/a002.png, ...   ← sequential names, NEVER the id
//! ```
//!
//! # Design rules enforced here (each one exists because it prevents a real bug)
//!
//! * **The asset id is never a path.** Ids may be Chinese; the image file name is
//!   always `images/aNNN.png`, so no Windows/Android/git encoding trap.
//! * **Duplicate ids are rejected before any write.**
//! * **Save-time reference validation**: assets and flows are cross-checked when
//!   they are saved, so a typo in `flow.json` cannot survive until the run that
//!   needs it (`oas_flow::Flow::validate_assets`).
//! * **Atomic writes** (tmp + rename): a crash mid-save never leaves a
//!   half-written `assets.json`/`flow.json` that the next load would misread.
//! * **`design_resolution` is set once** by the first template save; a later
//!   frame with different dimensions is an error, not a silently distorted crop.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use oas_types::Frame;

mod meta;
pub mod ocr_var;
pub mod package;
pub use meta::{
    validate_param_values, validate_variable_values, Capabilities, EntryRequires, ExitDecl, Meta,
    MetaEntry, ParamDecl, QuotaHint, ScheduleHint, Variable, CATEGORY_DEFAULT, DEFAULT_SCOPE,
    DEFAULT_VALUE_SOURCE, DEFAULT_EXIT_NAMES, EXIT_RESULTS, PARAM_DESCRIPTION_MAX_CHARS,
    PARAM_GROUP_MAX_CHARS, PARAM_LABEL_MAX_CHARS, PARAM_TYPES, TEMPLATE_VERSION,
    VARIABLE_SCOPES, VARIABLE_TYPES, VARIABLE_VALUE_SOURCES,
};

/// On-disk schema version for `meta.json` / `flow.json` / `assets.json`.
pub const SCHEMA_VERSION: u32 = 1;

/// NCC score above which a template counts as "found".
pub const DEFAULT_THRESHOLD: f32 = 0.85;

/// Asset/object kinds: recognize+act (`template`/`ocr_line`/`ocr_lines`/
/// `ocr_var`) and pure operation region (`region`, no recognition).
///
/// `ocr_var` = 「使用端OCR文字配置项」 (2026-11-06 参数→素材 redesign): an OCR
/// asset whose recognition text is changeable on the usage side
/// (`runtime_text` + `param_key`), with an optional ROI (empty = whole-screen
/// OCR).
pub const KINDS: [&str; 5] = ["template", "ocr_line", "ocr_lines", "ocr_var", "region"];

/// OCR asset kinds (recognition-based, no template pixels).
pub fn is_ocr_kind(kind: &str) -> bool {
    matches!(kind, "ocr_line" | "ocr_lines" | "ocr_var")
}

/// Recognition kinds that may act as a **node in an `op_region` chain**.
///
/// 2026-11-06 批次B (#4 model A): the chain is a **recognition gate only**; a
/// node's own operation area never becomes the action target. `region` assets
/// are pure fixed operation areas with no recognition, so they are **not** chain
/// nodes any more (referencing one is a validation error). Collections are
/// accepted by id (they have no `op_region` of their own) but are not `AssetDecl`
/// items, so they are not in this list.
pub const OP_REGION_NODE_KINDS: [&str; 4] = ["template", "ocr_line", "ocr_lines", "ocr_var"];

/// Maximum `op_region` chain depth (mirrors the flow region parser's bound).
pub const MAX_OP_REGION_DEPTH: usize = 16;

/// `runtime_text` (ocr_var default recognition text) max length, matching the
/// `text_expr`/OCR UI limit.
pub const RUNTIME_TEXT_MAX_CHARS: usize = 200;

/// Length limit of an ocr_var display `label`.
pub const OCR_VAR_LABEL_MAX_CHARS: usize = 80;

/// **Operation-region source** stored on one asset (`assets.json
/// items[].op_region`, 2026-11-06 参数→素材 redesign D2; 批次B #4 model A).
///
/// Two modes only (unlike the flow-level `oas_flow::OpRegion`, which also
/// carries `ocr`/2-value offsets):
/// * `self` — the current asset's own operation area = the historical runtime
///   behaviour (`action_shape`/`click_back`; template/collection shapes
///   translated by the match offset, OCR shapes translated by
///   `real line - authored roi`, else the real recognized bbox); this is the
///   default when the field is absent and it is **always** the final action
///   area (model A: the chain never contributes the click box);
/// * `asset` — recognize another asset as an additional **recognition gate**
///   (recursive chain; `pick` chooses among that asset's candidates: `score` =
///   best score (default), `nearest` = centre nearest to the previous node's
///   real rect). Every gate must be recognized; the action still lands on the
///   root asset's own `self` geometry.
///
/// The former `offset` mode was removed in 批次B (#4): "relative offset" was
/// just editing the current asset's own operation area, which is exactly
/// `self` + `action_shape`.
///
/// JSON: `{"mode":"self"}`, `{"mode":"asset","asset":"<id>","pick":"score|nearest"}`.
/// Validation (node existence/kind, referenced OCR expected text, cycle + depth,
/// terminal mode) lives in [`crate::validate_op_region_chain`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode")]
pub enum OpRegion {
    /// `{"mode":"self"}` — use this asset's own operation area (default).
    #[serde(rename = "self")]
    SelfRegion,
    /// `{"mode":"asset","asset":"<id>","pick":"score|nearest"}` — an extra
    /// recognition gate; never the action box (model A).
    #[serde(rename = "asset")]
    Asset {
        /// Referenced asset/collection id. A `region` item is **not** allowed as
        /// a chain node (model A: it has no recognition) and collections are
        /// always terminal.
        asset: String,
        /// Candidate pick rule for a multi-candidate node; absent = `score`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pick: Option<String>,
    },
}

/// Allowed `op_region.pick` values.
pub const OP_REGION_PICKS: [&str; 2] = ["score", "nearest"];

impl OpRegion {
    /// `self` (the default when the asset has no `op_region` field).
    pub fn self_region() -> Self {
        OpRegion::SelfRegion
    }

    /// `self` / `asset`.
    pub fn mode_str(&self) -> &'static str {
        match self {
            OpRegion::SelfRegion => "self",
            OpRegion::Asset { .. } => "asset",
        }
    }

    /// Effective pick rule (`score` when absent).
    pub fn pick(&self) -> &str {
        match self {
            OpRegion::Asset { pick, .. } => pick.as_deref().unwrap_or("score"),
            _ => "score",
        }
    }

    /// Referenced asset id for `mode=asset`.
    pub fn asset(&self) -> Option<&str> {
        match self {
            OpRegion::Asset { asset, .. } => Some(asset.as_str()),
            _ => None,
        }
    }

    /// `true` for the default mode (self).
    pub fn is_self(&self) -> bool {
        matches!(self, OpRegion::SelfRegion)
    }
}

impl Default for OpRegion {
    fn default() -> Self {
        OpRegion::SelfRegion
    }
}

// ---------------------------------------------------------------------------
// Lint (M0 unified task template, ENGINE-DESIGN §22.34.2)
// ---------------------------------------------------------------------------

/// One lint violation: a stable rule code, the JSON/file path it applies to and
/// a human-readable message. **Every issue carries a path** (the M0 acceptance
/// criterion; the CLI prints it and M1's create/save endpoints return it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LintIssue {
    /// `T1`/`T3`/`T4`/`T5`/`T6`/`T7` for the flow invariants, `META` for the
    /// `meta.json` contract, `FLOW` for flow parse/read errors.
    pub code: &'static str,
    pub path: String,
    pub msg: String,
}

impl LintIssue {
    pub fn new(code: &'static str, path: impl Into<String>, msg: impl Into<String>) -> Self {
        Self {
            code,
            path: path.into(),
            msg: msg.into(),
        }
    }
}

impl fmt::Display for LintIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}: {}", self.code, self.path, self.msg)
    }
}

// ---------------------------------------------------------------------------
// Public data model (serde: this is the on-disk contract)
// ---------------------------------------------------------------------------

/// Operation-region geometry. One canonical shape for click/drag sampling.
/// `kind=rect` uses `roi`; `kind=polygon` uses `points`.
/// `invert=true` means the effective region is the **whole screen minus shape**.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionShape {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roi: Option<[i32; 4]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub points: Option<Vec<[i32; 2]>>,
    #[serde(default)]
    pub invert: bool,
}

/// Default value of the **deprecated** `action_source` compatibility field.
///
/// The two-state operation-region model (`recognition` = follow the real
/// recognized bbox, `fixed` = independent operation region) is gone
/// (2026-11-06 事项6): the runtime only ever looked at whether `action_shape`
/// exists, so the field carried no engine semantics. Old `assets.json` files
/// still carry it; it is read for compatibility (any value), never written again
/// and never validated. See [`AssetDecl::action_source`].
///
/// Public so the editor's save handlers can construct an [`AssetDecl`] without
/// inventing a value for a field that is no longer persisted.
pub fn default_action_source() -> String {
    "recognition".to_string()
}

impl ActionShape {
    pub fn rect(roi: [i32; 4]) -> Self {
        Self {
            kind: "rect".to_string(),
            roi: Some(roi),
            points: None,
            invert: false,
        }
    }

    /// Bounding box of the selected shape (not of the inverted effective region).
    pub fn bbox(&self) -> Option<[i32; 4]> {
        if let Some(roi) = self.roi {
            return Some(roi);
        }
        let pts = self.points.as_ref()?;
        if pts.is_empty() {
            return None;
        }
        let mut min_x = i32::MAX;
        let mut min_y = i32::MAX;
        let mut max_x = i32::MIN;
        let mut max_y = i32::MIN;
        for p in pts {
            min_x = min_x.min(p[0]);
            min_y = min_y.min(p[1]);
            max_x = max_x.max(p[0]);
            max_y = max_y.max(p[1]);
        }
        Some([min_x, min_y, max_x - min_x + 1, max_y - min_y + 1])
    }

    pub fn contains(&self, x: i32, y: i32) -> bool {
        if self.kind == "rect" {
            if let Some([rx, ry, rw, rh]) = self.roi {
                return x >= rx && y >= ry && x < rx + rw && y < ry + rh;
            }
            return false;
        }
        let Some(pts) = self.points.as_ref() else {
            return false;
        };
        if pts.len() < 3 {
            return false;
        }
        // B-07: use the same f64 edge test as `oas_runtime::ClickShape`. The old
        // integer `(yj - yi).max(1)` formula rejected points inside triangles
        // with a negative-slope edge.
        let mut inside = false;
        let mut j = pts.len() - 1;
        for i in 0..pts.len() {
            let (xi, yi) = (pts[i][0], pts[i][1]);
            let (xj, yj) = (pts[j][0], pts[j][1]);
            if (yi > y) != (yj > y) {
                let denom = (yj - yi) as f64;
                let cross_x = (xj - xi) as f64 * (y - yi) as f64 / denom + xi as f64;
                if (x as f64) < cross_x {
                    inside = !inside;
                }
            }
            j = i;
        }
        inside
    }

    /// Convert to the runtime's exact operation geometry (same fields, same
    /// semantics). Used by `load_runtime_assets` so asset and collection shapes
    /// go through one translation.
    pub fn to_click_shape(&self) -> oas_runtime::ClickShape {
        oas_runtime::ClickShape {
            rect: self.roi.map(|r| oas_types::Rect {
                x: r[0],
                y: r[1],
                w: r[2],
                h: r[3],
            }),
            points: self.points.as_ref().map(|pts| {
                pts.iter()
                    .map(|p| oas_types::Point { x: p[0], y: p[1] })
                    .collect()
            }),
            invert: self.invert,
        }
    }
}

/// Shoelace area × 2 for an action polygon (`0` = collinear/degenerate).
fn action_polygon_area2(points: &[[i32; 2]]) -> i64 {
    let mut area2: i64 = 0;
    for i in 0..points.len() {
        let a = points[i];
        let b = points[(i + 1) % points.len()];
        area2 += a[0] as i64 * b[1] as i64 - b[0] as i64 * a[1] as i64;
    }
    area2
}

/// Validate one action shape.
///
/// `bounds` is the frame/design resolution when known; then rect and polygon
/// bboxes must stay inside it (B-05). A degenerate polygon (collinear, zero
/// area) is rejected as well. Callers that know the frame pass it so the save
/// and load paths reject identical geometry.
fn validate_action_shape(
    shape: &ActionShape,
    path: &str,
    bounds: Option<(u32, u32)>,
) -> Result<(), String> {
    match shape.kind.as_str() {
        "rect" => {
            let Some([x, y, w, h]) = shape.roi else {
                return Err(format!("{path}.roi is required for kind=rect"));
            };
            if x < 0 || y < 0 || w <= 0 || h <= 0 {
                return Err(format!(
                    "{path}.roi must satisfy x,y>=0 and w,h>0 (got [{x},{y},{w},{h}])"
                ));
            }
            if shape.points.is_some() {
                return Err(format!("{path}.points is only valid for kind=polygon"));
            }
            if let Some((fw, fh)) = bounds {
                if x as i64 + w as i64 > fw as i64 || y as i64 + h as i64 > fh as i64 {
                    return Err(format!(
                        "{path}.roi [{x},{y},{w},{h}] is outside the {fw}x{fh} frame"
                    ));
                }
            }
        }
        "polygon" => {
            let Some(pts) = shape.points.as_ref() else {
                return Err(format!("{path}.points is required for kind=polygon"));
            };
            if pts.len() < 3 {
                return Err(format!("{path}.points needs at least 3 vertices"));
            }
            if pts.iter().any(|p| p[0] < 0 || p[1] < 0) {
                return Err(format!("{path}.points must be non-negative"));
            }
            let raw: Vec<[i32; 2]> = pts.iter().map(|p| [p[0], p[1]]).collect();
            if polygon_self_intersects(&raw) {
                return Err(format!("{path}.points must not self-intersect"));
            }
            if action_polygon_area2(&raw) == 0 {
                return Err(format!(
                    "{path}.points has zero area (collinear/degenerate polygon)"
                ));
            }
            if shape.roi.is_some() {
                return Err(format!("{path}.roi is only valid for kind=rect"));
            }
            if let Some((fw, fh)) = bounds {
                let (mut min_x, mut min_y) = (i32::MAX, i32::MAX);
                let (mut max_x, mut max_y) = (i32::MIN, i32::MIN);
                for p in &raw {
                    min_x = min_x.min(p[0]);
                    min_y = min_y.min(p[1]);
                    max_x = max_x.max(p[0]);
                    max_y = max_y.max(p[1]);
                }
                if max_x as i64 >= fw as i64 || max_y as i64 >= fh as i64 {
                    return Err(format!(
                        "{path}.points bbox [{}..{}, {}..{}] is outside the {fw}x{fh} frame",
                        min_x, max_x, min_y, max_y
                    ));
                }
            }
        }
        other => return Err(format!("{path}.kind must be rect or polygon (got `{other}`)")),
    }
    Ok(())
}

/// Validate the `ocr_var`-only fields of one asset declaration.
///
/// Rules (2026-11-06 参数→素材 redesign): `runtime_text` = required non-blank
/// text (<= [`RUNTIME_TEXT_MAX_CHARS`], no control characters) = the default
/// recognition text; `param_key` = required valid parameter key; `label` =
/// optional non-blank display name (<= [`OCR_VAR_LABEL_MAX_CHARS`]); OCR pixel
/// provenance fields (`file`/`ocr_mode`/`ocr_source_roi`/`ocr_text`) are
/// forbidden. `roi` is optional (absent = whole-screen OCR); `polygon`
/// (2026-11-06 批次B, #6/#10) is allowed as a real recognition mask (its bbox
/// becomes the recognition ROI on save); `threshold`/`action_shape`/`op_region`
/// keep their normal meaning.
pub(crate) fn validate_ocr_var_fields(item: &AssetDecl, path: &str) -> Result<(), String> {
    for (field, present) in [
        ("file", item.file.is_some()),
        ("ocr_mode", item.ocr_mode.is_some()),
        ("ocr_source_roi", item.ocr_source_roi.is_some()),
        ("ocr_text", item.ocr_text.is_some()),
    ] {
        if present {
            return Err(format!(
                "{path}.{field} is not allowed for kind=ocr_var \
                 (use runtime_text/roi instead)"
            ));
        }
    }
    let Some(text) = item.runtime_text.as_deref() else {
        return Err(format!("{path}.runtime_text is required for kind=ocr_var"));
    };
    if text.trim().is_empty() {
        return Err(format!("{path}.runtime_text must not be blank"));
    }
    if text.chars().count() > RUNTIME_TEXT_MAX_CHARS {
        return Err(format!(
            "{path}.runtime_text must be <= {RUNTIME_TEXT_MAX_CHARS} characters (got {})",
            text.chars().count()
        ));
    }
    if text.chars().any(|c| c.is_control()) {
        return Err(format!("{path}.runtime_text must not contain control characters"));
    }
    let Some(key) = item.param_key.as_deref() else {
        return Err(format!("{path}.param_key is required for kind=ocr_var"));
    };
    if !meta::is_param_key(key) {
        return Err(format!(
            "{path}.param_key `{key}` must match [A-Za-z_][A-Za-z0-9_]* (<= 64 chars)"
        ));
    }
    if let Some(label) = item.label.as_deref() {
        if label.trim().is_empty() {
            return Err(format!("{path}.label must not be blank when present"));
        }
        if label.chars().count() > OCR_VAR_LABEL_MAX_CHARS {
            return Err(format!(
                "{path}.label must be <= {OCR_VAR_LABEL_MAX_CHARS} characters (got {})",
                label.chars().count()
            ));
        }
    }
    Ok(())
}

/// Structural validation of one `op_region` value (no chain traversal).
fn validate_op_region_shape(op: &OpRegion, path: &str) -> Result<(), String> {
    match op {
        OpRegion::SelfRegion => Ok(()),
        OpRegion::Asset { asset, pick } => {
            validate_id(asset).map_err(|e| format!("{path}.asset: {e}"))?;
            if let Some(pick) = pick {
                if !OP_REGION_PICKS.contains(&pick.as_str()) {
                    return Err(format!(
                        "{path}.pick `{pick}` must be one of {}",
                        OP_REGION_PICKS.join(", ")
                    ));
                }
            }
            Ok(())
        }
    }
}

/// Validate the whole `op_region` **chain** reachable from one asset.
///
/// `assets` is the would-be `assets.json` (the owner's new value included —
/// callers validate on the in-memory clone before writing). `owner_id` names the
/// asset whose new value is `op`; `None` means `op` is a free-standing value
/// (e.g. a server-side preview) and must terminate without any owner override.
///
/// Rules (2026-11-06 批次B, #4 model A):
/// * `mode=asset` follows the referenced asset's own `op_region`: the chain is a
///   **recognition gate only** and never contributes the action box;
/// * every referenced node must be recognizable: `template`/collection (NCC) or
///   an OCR asset with **expected text** (`ocr_var.runtime_text/param`, derived
///   `ocr_line.ocr_text`, plain `ocr_line`/`ocr_lines.ocr_text`); a plain OCR
///   node without `ocr_text` is a validation error;
/// * a `region` item is a pure fixed operation area with no recognition, so it
///   can **not** be a chain node (referencing it is an error);
/// * a collection is always terminal (no `op_region` of its own);
/// * cycles and chains deeper than [`MAX_OP_REGION_DEPTH`] are rejected;
/// * a chain must be resolvable: every referenced element must exist and every
///   leaf must be `self` (or a node without `op_region`, = self).
pub fn validate_op_region_chain(
    assets: &AssetsFile,
    owner_id: Option<&str>,
    op: &OpRegion,
    path: &str,
) -> Result<(), String> {
    validate_op_region_shape(op, path)?;
    let mut visited: BTreeSet<String> = BTreeSet::new();
    match owner_id {
        Some(owner) => {
            let owner_decl = assets
                .items
                .iter()
                .find(|a| a.id == owner)
                .ok_or_else(|| format!("{path}: owner asset `{owner}` is not declared"))?;
            if owner_decl.kind == "region" {
                return Err(format!(
                    "{path}: region assets are fixed operation areas and must not carry op_region"
                ));
            }
            walk_op_region_chain(assets, Some(owner), op, owner, 0, &mut visited, path)
        }
        None => match op {
            OpRegion::SelfRegion => Ok(()),
            OpRegion::Asset { .. } => Err(format!(
                "{path}: an op_region chain needs an owner asset (mode=asset without one)"
            )),
        },
    }
}

/// One DFS hop of [`validate_op_region_chain`]. `current` is the node being
/// resolved (`owner_id`'s new value is `owner_op`, every other node uses its
/// stored `op_region`).
fn walk_op_region_chain(
    assets: &AssetsFile,
    owner_id: Option<&str>,
    owner_op: &OpRegion,
    current: &str,
    depth: usize,
    visited: &mut BTreeSet<String>,
    path: &str,
) -> Result<(), String> {
    if depth > MAX_OP_REGION_DEPTH {
        return Err(format!(
            "{path}: op_region chain exceeds the max depth {MAX_OP_REGION_DEPTH} at `{current}`"
        ));
    }
    if !visited.insert(current.to_string()) {
        return Err(format!(
            "{path}: op_region chain cycle detected at `{current}` (visited: {})",
            visited.iter().cloned().collect::<Vec<_>>().join(" -> ")
        ));
    }
    if Some(current) == owner_id {
        // First hop (or a cycle back to the owner, already rejected above).
        return follow_op_region(assets, owner_id, owner_op, current, depth, visited, path);
    }
    let item = assets.items.iter().find(|a| a.id == current);
    let Some(item) = item else {
        if assets.collections.iter().any(|c| c.id == current) {
            // A collection has no op_region of its own: it is a valid terminal
            // recognition gate (NCC over its members).
            return Ok(());
        }
        return Err(format!(
            "{path}: op_region references `{current}`, which is not a declared asset or collection"
        ));
    };
    if item.kind == "region" {
        // 2026-11-06 批次B (#4 model A): a region is a pure fixed operation
        // area with no recognition, so it cannot be a recognition gate.
        return Err(format!(
            "{path}: region asset `{current}` cannot be an op_region chain node \
             (model A: chain nodes are recognition gates; region assets are fixed \
             operation areas only)"
        ));
    }
    if !OP_REGION_NODE_KINDS.contains(&item.kind.as_str()) {
        return Err(format!(
            "{path}: asset `{current}` has kind `{}`, which cannot be an op_region node",
            item.kind
        ));
    }
    // #3: every referenced OCR gate must be recognizable, i.e. it must carry the
    // expected text (plain ocr_line/ocr_lines: `ocr_text`; ocr_var:
    // runtime_text/param handled by its own contract).
    if is_ocr_kind(&item.kind) && item.kind != "ocr_var" {
        let has_text = item
            .ocr_text
            .as_deref()
            .map(|t| !t.trim().is_empty())
            .unwrap_or(false);
        if !has_text {
            return Err(format!(
                "{path}: OCR chain node `{current}` has no expected text (ocr_text); \
                 save an expected text on it (识别文字) or reference an ocr_var instead"
            ));
        }
    }
    match &item.op_region {
        None => Ok(()), // default self: the gate is recognized, chain ends here
        Some(op) => {
            let op_path = format!("{path} (node `{current}`)");
            validate_op_region_shape(op, &op_path)?;
            follow_op_region(assets, owner_id, op, current, depth, visited, path)
        }
    }
}

/// Shared tail of one chain hop: `self` terminates, `asset` recurses.
fn follow_op_region(
    assets: &AssetsFile,
    owner_id: Option<&str>,
    op: &OpRegion,
    current: &str,
    depth: usize,
    visited: &mut BTreeSet<String>,
    path: &str,
) -> Result<(), String> {
    match op {
        OpRegion::SelfRegion => Ok(()),
        OpRegion::Asset { asset, .. } => {
            if asset == current {
                return Err(format!("{path}: op_region of `{current}` references itself"));
            }
            walk_op_region_chain(assets, owner_id, op, asset, depth + 1, visited, path)
        }
    }
}

/// First asset/collection id whose `op_region` chain references `id`
/// (`mode=asset`), used to block a deletion that would leave a dangling chain.
fn op_region_referrer(assets: &AssetsFile, id: &str) -> Option<String> {
    assets
        .items
        .iter()
        .find(|a| a.op_region.as_ref().and_then(|op| op.asset()) == Some(id))
        .map(|a| a.id.clone())
}

/// Every `op_region` problem in one `assets.json`, with precise JSON paths.
///
/// Used by the load/lint path ([`TaskBundle::assets_shape_issues`]) and by the
/// save/update paths (which validate the would-be file before writing).
fn op_region_issues(assets: &AssetsFile) -> Vec<(String, String)> {
    let mut errs = Vec::new();
    for (i, item) in assets.items.iter().enumerate() {
        let p = format!("assets.json: items[{i}].op_region");
        if item.kind == "region" {
            if item.op_region.is_some() {
                errs.push((
                    p,
                    "region assets are fixed operation areas and must not carry op_region"
                        .to_string(),
                ));
            }
            continue;
        }
        let Some(op) = item.op_region.as_ref() else {
            continue;
        };
        if let Err(e) = validate_op_region_chain(assets, Some(&item.id), op, &p) {
            errs.push((p, e));
        }
    }
    errs
}

/// One declared asset, exactly as stored in `assets.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetDecl {
    /// Semantic name; Chinese allowed (`分组2`, `返回按钮`). Never used as a path.
    pub id: String,
    /// `template` | `ocr_line` | `ocr_lines` | `region`.
    pub kind: String,
    /// Template image, relative to the bundle root, always `images/aNNN.png`.
    /// `None` for OCR / region objects (they have no pixels).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// **Recognition region** `[x, y, w, h]` in frame coordinates.
    ///
    /// - template: source crop / template image origin;
    /// - OCR: ROI where recognition runs;
    /// - `region`: this is the operation region itself (no separate recognition).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roi: Option<[i32; 4]>,
    /// **Operation region** in frame coordinates (optional). When present the
    /// runtime uses this geometry (template: translated by the match offset;
    /// OCR/region: absolute); when absent it follows the **real recognized
    /// bbox**. This is the only operation-region switch now — the old
    /// `action_source` two-state rule is gone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_shape: Option<ActionShape>,
    /// **DEPRECATED compatibility field** (2026-11-06 事项6). Old `assets.json`
    /// files carry `"action_source": "recognition" | "fixed"`; it used to gate
    /// whether `action_shape` was allowed/required. The engine never read it and
    /// the two-state model is removed, so it is deserialized for old data,
    /// **never serialized again** and never validated. `action_shape` alone
    /// decides the operation region.
    #[serde(default = "default_action_source", skip_serializing)]
    pub action_source: String,
    /// v1.2.1 recognition provenance for `ocr_line` assets:
    /// `single` (default, rec only) or `lines` (came from a multi-line det+rec
    /// result and must run the same det+rec path at runtime).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ocr_mode: Option<String>,
    /// LEGACY provenance (pre-v1.2.3): original multi-line source region.
    /// Loading migrates it into `roi` (union with any user-enlarged ROI) and
    /// clears this field; runtime never reads it. Kept only for old assets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ocr_source_roi: Option<[i32; 4]>,
    /// Expected line text captured when this derived `ocr_line` was saved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ocr_text: Option<String>,
    /// NCC threshold for `template` assets; **minimum recognition confidence**
    /// (0~1) for `ocr_line`/`ocr_lines` assets. Templates default to
    /// [`DEFAULT_THRESHOLD`] on save; OCR defaults to `None` (any successful
    /// recognition is a hit, preserving pre-threshold behaviour).
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_threshold"
    )]
    pub threshold: Option<f32>,
    /// Optional polygon in **frame coordinates** at `design_resolution`
    /// (ENGINE §22.10). When present the runtime restricts `click` sampling to
    /// the polygon's **largest inscribed axis-aligned rectangle**; matching/OCR
    /// still use `roi` (= the polygon bounding box, computed on save).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub polygon: Option<Vec<[i32; 2]>>,
    /// **Default recognition text** of an `ocr_var` asset (2026-11-06 参数→素材
    /// redesign). The usage side may override it per run; when absent the
    /// runtime has nothing to recognize and fails loudly. Required for `kind:
    /// "ocr_var"`, forbidden for every other kind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_text: Option<String>,
    /// Optional display name of an `ocr_var` asset (also the derived parameter
    /// row's `label`; empty ⇒ the asset id is used). Forbidden for other kinds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// **Parameter key** of an `ocr_var` asset: the key the usage side edits in
    /// `values.json` (`tasks.<task>.values.<param_key>`) and the key of its
    /// derived `meta.params` row. Read-only after creation (Lead ruling 6).
    /// Required for `kind:"ocr_var"`, forbidden for every other kind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub param_key: Option<String>,
    /// **Operation-region chain** (2026-11-06 参数→素材 redesign D2; 批次B #4
    /// model A): absent = `self` (= historical `action_shape` behaviour). Present
    /// = recognize every referenced node as an extra **gate** at run time; the
    /// action box itself is always this asset's own self geometry. Not allowed on
    /// `kind:"region"` (its `roi` already is the operation area, and a region has
    /// no recognition so it cannot be a chain node either).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub op_region: Option<OpRegion>,
    /// Revision counter (1 on first save).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rev: Option<u32>,
    /// Frame ids where this asset is known to appear (v1.2.3 frame-asset link).
    /// Multiple positions are allowed; batch dry-run tests every listed frame.
    /// Empty is allowed for legacy assets, but new saves through the editor
    /// require at least one position (user 10-31).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub positions: Vec<u64>,
}

impl AssetDecl {
    /// Convenience constructor for a template asset (file is assigned on save).
    pub fn template(id: impl Into<String>, roi: [i32; 4]) -> Self {
        Self {
            id: id.into(),
            kind: "template".to_string(),
            file: None,
            roi: Some(roi),
            action_shape: None,
            action_source: "recognition".to_string(),
            ocr_mode: None,
            ocr_source_roi: None,
            ocr_text: None,
            threshold: None,
            polygon: None,
            runtime_text: None,
            label: None,
            param_key: None,
            op_region: None,
            rev: None,
            positions: Vec::new(),
        }
    }

    /// Convenience constructor for a `kind:"ocr_var"` asset.
    ///
    /// `roi` is optional: absent = whole-screen OCR (2026-11-06 redesign).
    pub fn ocr_var(
        id: impl Into<String>,
        param_key: impl Into<String>,
        runtime_text: impl Into<String>,
        roi: Option<[i32; 4]>,
    ) -> Self {
        Self {
            id: id.into(),
            kind: "ocr_var".to_string(),
            file: None,
            roi,
            action_shape: None,
            action_source: "recognition".to_string(),
            ocr_mode: None,
            ocr_source_roi: None,
            ocr_text: None,
            threshold: None,
            polygon: None,
            runtime_text: Some(runtime_text.into()),
            label: None,
            param_key: Some(param_key.into()),
            op_region: None,
            rev: None,
            positions: Vec::new(),
        }
    }

    /// `true` for the 使用端OCR文字配置项 kind.
    pub fn is_ocr_var(&self) -> bool {
        self.kind == "ocr_var"
    }

    /// Convenience constructor for a `kind:"region"` fixed operation area.
    pub fn region(id: impl Into<String>, roi: [i32; 4]) -> Self {
        let mut decl = Self::template(id, roi);
        decl.kind = "region".to_string();
        decl.file = None;
        decl
    }

    /// Convenience constructor for a single-line OCR asset (`ocr_line`).
    pub fn ocr_line(id: impl Into<String>, roi: [i32; 4]) -> Self {
        let mut decl = Self::template(id, roi);
        decl.kind = "ocr_line".to_string();
        decl.file = None;
        decl
    }

    /// Convenience constructor for a multi-line OCR asset (`ocr_lines`).
    pub fn ocr_lines(id: impl Into<String>, roi: [i32; 4]) -> Self {
        let mut decl = Self::template(id, roi);
        decl.kind = "ocr_lines".to_string();
        decl.file = None;
        decl
    }

    /// `op_region` with the documented default (`self`) applied.
    pub fn op_region_or_self(&self) -> OpRegion {
        self.op_region.clone().unwrap_or_default()
    }
}

/// v1.2.3: a logical target backed by one or more template assets. Flow steps
/// reference the collection id; oas-task expands it into the member templates
/// (click/click_target) before the runtime sees the flow.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetCollection {
    pub id: String,
    /// Template asset ids in this collection (at least one while referenced).
    pub members: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threshold: Option<f32>,
    /// Optional collection-level operation geometry; member shape wins.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_shape: Option<ActionShape>,
    /// **DEPRECATED compatibility field** (2026-11-06 事项6), exactly like
    /// [`AssetDecl::action_source`]: read from old data, never written again,
    /// never validated; `action_shape` alone decides the operation region.
    #[serde(default = "default_action_source", skip_serializing)]
    pub action_source: String,
}

/// `assets.json` payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetsFile {
    pub schema_version: u32,
    /// Frame size the ROIs were authored at; set by the first template save.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub design_resolution: Option<[u32; 2]>,
    #[serde(default)]
    pub items: Vec<AssetDecl>,
    /// Logical multi-image targets (flow references `collection.id`).
    #[serde(default)]
    pub collections: Vec<AssetCollection>,
}

impl Default for AssetsFile {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            design_resolution: None,
            items: Vec::new(),
            collections: Vec::new(),
        }
    }
}

/// v1.2.3: collapse the legacy two-ROI OCR model into one recognition ROI.
/// `ocr_source_roi` was provenance from the multi-line source; runtime and the
/// editor must both recognize on exactly one `roi`. Existing assets migrate
/// in-memory on every load and are written back in the unified shape on save.
///
/// Migration uses the **union** of the old line ROI and source ROI: the source
/// is usually larger, but a user may already have enlarged the recognition ROI
/// after authoring, and overwriting it with the older source would silently
/// shrink their fix. Union preserves both intents.
fn union_rect(a: [i32; 4], b: [i32; 4]) -> [i32; 4] {
    let x0 = a[0].min(b[0]);
    let y0 = a[1].min(b[1]);
    let x1 = (a[0] + a[2]).max(b[0] + b[2]);
    let y1 = (a[1] + a[3]).max(b[1] + b[3]);
    [x0, y0, (x1 - x0).max(1), (y1 - y0).max(1)]
}

fn normalize_legacy_ocr_roi(af: &mut AssetsFile) {
    for item in &mut af.items {
        if item.kind == "ocr_line" && item.ocr_mode.as_deref() == Some("lines") {
            if let Some(source) = item.ocr_source_roi.take() {
                item.roi = Some(match item.roi {
                    Some(roi) if roi[2] > 0 && roi[3] > 0 => union_rect(roi, source),
                    _ => source,
                });
            }
        }
    }
}

/// Convert one on-disk `op_region` into the dependency-light runtime type.
///
/// The task crate depends on `oas-runtime` (not the other way round), so the
/// runtime has its own `OpRegion` copy; this is the single conversion point and
/// runs only after [`validate_op_region_chain`] accepted the graph.
fn runtime_op_region(op: &OpRegion) -> oas_runtime::AssetOpRegion {
    match op {
        OpRegion::SelfRegion => oas_runtime::AssetOpRegion::SelfRegion,
        OpRegion::Asset { asset, pick } => oas_runtime::AssetOpRegion::Asset {
            asset: asset.clone(),
            pick: match pick.as_deref() {
                Some("nearest") => oas_runtime::OpRegionPick::Nearest,
                _ => oas_runtime::OpRegionPick::Score,
            },
        },
    }
}

// ---------------------------------------------------------------------------
// Validation helpers (also unit-tested directly)
// ---------------------------------------------------------------------------

/// §22.10 N10b-3: true when two non-adjacent polygon edges cross (bowtie etc.).
fn polygon_self_intersects(poly: &[[i32; 2]]) -> bool {
    let n = poly.len();
    if n < 4 {
        return false;
    }
    let cross = |o: [i32; 2], a: [i32; 2], b: [i32; 2]| -> i64 {
        (a[0] - o[0]) as i64 * (b[1] - o[1]) as i64
            - (a[1] - o[1]) as i64 * (b[0] - o[0]) as i64
    };
    let on_seg = |a: [i32; 2], b: [i32; 2], p: [i32; 2]| -> bool {
        p[0] >= a[0].min(b[0])
            && p[0] <= a[0].max(b[0])
            && p[1] >= a[1].min(b[1])
            && p[1] <= a[1].max(b[1])
    };
    for i in 0..n {
        let a1 = poly[i];
        let a2 = poly[(i + 1) % n];
        for j in 0..n {
            if j == i {
                continue;
            }
            let b1 = poly[j];
            let b2 = poly[(j + 1) % n];
            // adjacent edges share a vertex by construction — skip them
            if (i + 1) % n == j || (j + 1) % n == i {
                continue;
            }
            let d1 = cross(b1, b2, a1);
            let d2 = cross(b1, b2, a2);
            let d3 = cross(a1, a2, b1);
            let d4 = cross(a1, a2, b2);
            let proper = ((d1 > 0 && d2 < 0) || (d1 < 0 && d2 > 0))
                && ((d3 > 0 && d4 < 0) || (d3 < 0 && d4 > 0));
            if proper {
                return true;
            }
            // collinear touching/overlap
            if d1 == 0 && on_seg(b1, b2, a1) {
                return true;
            }
            if d2 == 0 && on_seg(b1, b2, a2) {
                return true;
            }
            if d3 == 0 && on_seg(a1, a2, b1) {
                return true;
            }
            if d4 == 0 && on_seg(a1, a2, b2) {
                return true;
            }
        }
    }
    false
}

/// Validate one **recognition polygon** and return its bbox `[x, y, w, h]`
/// (frame coordinates, inclusive pixel convention — `roi = polygon bbox`).
///
/// Shared by the save path (`save_asset_impl`) and the update path
/// (`update_asset_meta_v3`, 2026-11-06 批次B #6/#10) so both accept exactly the
/// same geometry. Rules (ENGINE §22.10 N10b-3):
/// * >= 3 points, no repeated adjacent point, non-zero area, no self-intersection;
/// * every point inside the frame/design bounds when known;
/// * `asset_id` is only used to make the error message actionable.
fn validate_recognition_polygon(
    poly: &[[i32; 2]],
    bounds: Option<(i32, i32)>,
    asset_id: &str,
) -> Result<[i32; 4], String> {
    if poly.len() < 3 {
        return Err(format!(
            "asset `{asset_id}` rejected: polygon needs >= 3 points (got {}); nothing written",
            poly.len()
        ));
    }
    let mut area2: i64 = 0;
    for i in 0..poly.len() {
        let a = poly[i];
        let b = poly[(i + 1) % poly.len()];
        if a == b {
            return Err(format!(
                "asset `{asset_id}` rejected: polygon has repeated adjacent point {a:?}; nothing written"
            ));
        }
        area2 += a[0] as i64 * b[1] as i64 - b[0] as i64 * a[1] as i64;
    }
    if area2 == 0 {
        return Err(format!(
            "asset `{asset_id}` rejected: polygon has zero area (collinear points); nothing written"
        ));
    }
    if polygon_self_intersects(poly) {
        return Err(format!(
            "asset `{asset_id}` rejected: polygon self-intersects (edges cross); nothing written"
        ));
    }
    let (mut min_x, mut min_y) = (i32::MAX, i32::MAX);
    let (mut max_x, mut max_y) = (i32::MIN, i32::MIN);
    for p in poly {
        min_x = min_x.min(p[0]);
        min_y = min_y.min(p[1]);
        max_x = max_x.max(p[0]);
        max_y = max_y.max(p[1]);
    }
    if min_x < 0 || min_y < 0 {
        return Err(format!(
            "asset `{asset_id}` rejected: polygon point is negative; nothing written"
        ));
    }
    if let Some((bw, bh)) = bounds {
        if max_x >= bw || max_y >= bh {
            return Err(format!(
                "asset `{asset_id}` rejected: polygon [{min_x}..{max_x}, {min_y}..{max_y}] \
                 is outside the {bw}x{bh} frame; nothing written"
            ));
        }
    }
    Ok([
        min_x,
        min_y,
        max_x - min_x + 1,
        max_y - min_y + 1,
    ])
}

fn is_id_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_' || ('\u{4e00}'..='\u{9fa5}').contains(&c)
}

fn is_id_char(c: char) -> bool {
    is_id_start(c) || c.is_ascii_digit()
}

/// Asset id grammar: `^[A-Za-z_\u4e00-\u9fa5][A-Za-z0-9_\u4e00-\u9fa5]*$`
/// (Chinese allowed; no leading digit, no spaces/punctuation).
pub fn validate_id(id: &str) -> Result<(), String> {
    if id.is_empty() {
        return Err("asset id is empty".to_string());
    }
    let mut chars = id.chars();
    let first = chars.next().unwrap();
    if !is_id_start(first) {
        return Err(format!(
            "asset id `{id}` must start with a letter, `_` or Chinese character \
             (not a digit, space or punctuation)"
        ));
    }
    for c in chars {
        if !is_id_char(c) {
            return Err(format!(
                "asset id `{id}` contains invalid character `{c}` \
                 (only letters, digits, `_`, Chinese are allowed)"
            ));
        }
    }
    Ok(())
}

/// Task name = one directory name under `<tasks-dir>`. Rejects anything that
/// could escape the tasks dir or is illegal on Windows.
pub fn validate_task_name(name: &str) -> Result<(), String> {
    const RESERVED: [&str; 22] = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
        "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    if name.is_empty() {
        return Err("task name is empty".to_string());
    }
    if name.trim() != name {
        return Err(format!("task name `{name}` has leading/trailing whitespace"));
    }
    if name == "." || name == ".." {
        return Err(format!("task name `{name}` is not a usable directory name"));
    }
    if name.chars().count() > 120 {
        return Err("task name is longer than 120 characters".to_string());
    }
    if name.ends_with('.') {
        return Err(format!("task name `{name}` must not end with `.`"));
    }
    if let Some(c) = name
        .chars()
        .find(|c| c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'))
    {
        return Err(format!("task name `{name}` contains invalid character `{c}`"));
    }
    let stem = name.split('.').next().unwrap_or("").to_ascii_uppercase();
    if RESERVED.contains(&stem.as_str()) {
        return Err(format!("task name `{name}` is a Windows reserved device name"));
    }
    Ok(())
}

fn validate_threshold(t: Option<f32>) -> Result<(), String> {
    if let Some(v) = t {
        if !v.is_finite() || !(0.0..=1.0).contains(&v) {
            return Err(format!("threshold {v} is out of range [0, 1]"));
        }
    }
    Ok(())
}

/// Serialize an f32 threshold as its **shortest human-readable** value.
///
/// `serde_json` widens `f32` to `f64`, so a plain derive writes
/// `0.8500000238418579`; the on-disk contract (and the editor UI) should show
/// `0.85`. Rounding to 6 decimals is lossless for any sane NCC threshold.
fn serialize_threshold<S>(v: &Option<f32>, s: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    match v {
        Some(x) => s.serialize_f64(((*x as f64) * 1_000_000.0).round() / 1_000_000.0),
        None => s.serialize_none(),
    }
}

/// Image paths in `assets.json` are always `images/aNNN.png` — never the id.
fn validate_image_rel_path(file: &str) -> Result<(), String> {
    if file.is_empty() {
        return Err("file path is empty".to_string());
    }
    if file.contains('\\') {
        return Err(format!(
            "file path `{file}` must use `/` separators (portable across Windows/Android)"
        ));
    }
    if file.starts_with('/') || Path::new(file).is_absolute() {
        return Err(format!("file path `{file}` must be relative to the bundle root"));
    }
    let mut parts = file.split('/');
    let dir = parts.next().unwrap_or("");
    if dir != "images" {
        return Err(format!("file path `{file}` must live under `images/`"));
    }
    let name = parts
        .next()
        .ok_or_else(|| format!("file path `{file}` must be `images/aNNN.png`"))?;
    if parts.next().is_some() {
        return Err(format!("file path `{file}` must be a single level under `images/`"));
    }
    let stem = name
        .strip_suffix(".png")
        .ok_or_else(|| format!("image `{name}` must end with `.png`"))?;
    let digits = stem
        .strip_prefix('a')
        .ok_or_else(|| format!("image `{name}` must be a sequential name aNNN.png (never the asset id)"))?;
    if digits.len() < 3 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!(
            "image `{name}` must be a sequential name aNNN.png (at least 3 digits, never the asset id)"
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Atomic file I/O
// ---------------------------------------------------------------------------

static TMP_SEQ: AtomicU64 = AtomicU64::new(0);

/// Write `bytes` to `path` atomically: write a sibling tmp file, then rename.
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("{}: has no parent directory", path.display()))?;
    std::fs::create_dir_all(parent)
        .map_err(|e| format!("create directory {}: {e}", parent.display()))?;
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("oas-task");
    let seq = TMP_SEQ.fetch_add(1, Ordering::Relaxed);
    let tmp = parent.join(format!(".{name}.tmp-{}-{seq}", std::process::id()));
    if let Err(e) = std::fs::write(&tmp, bytes) {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("write {}: {e}", tmp.display()));
    }
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("rename {} -> {}: {e}", tmp.display(), path.display()));
    }
    Ok(())
}

/// Crop an RGBA frame to `roi`, bounds-checked (loud error, never a silent clamp).
fn crop_rgba(frame: &Frame, roi: [i32; 4]) -> Result<Frame, String> {
    let [x, y, w, h] = roi;
    if w <= 0 || h <= 0 {
        return Err(format!("roi [x,y,w,h] = {roi:?} must have w > 0 and h > 0"));
    }
    if x < 0 || y < 0 {
        return Err(format!("roi [x,y,w,h] = {roi:?} must have x >= 0 and y >= 0"));
    }
    let (fw, fh) = (frame.width as i64, frame.height as i64);
    let (x1, y1) = (x as i64 + w as i64, y as i64 + h as i64);
    if x1 > fw || y1 > fh {
        return Err(format!(
            "roi [x,y,w,h] = {roi:?} is outside the {fw}x{fh} frame (right/bottom {x1}/{y1})"
        ));
    }
    let w = w as usize;
    let h = h as usize;
    let stride = frame.width as usize * 4;
    let mut rgba = Vec::with_capacity(w * h * 4);
    for row in 0..h {
        let start = (y as usize + row) * stride + x as usize * 4;
        rgba.extend_from_slice(&frame.rgba[start..start + w * 4]);
    }
    Ok(Frame::new(w as u32, h as u32, rgba))
}

// ---------------------------------------------------------------------------
// TaskBundle
// ---------------------------------------------------------------------------

/// A task bundle directory (`<tasks-dir>/<任务名>/`).
///
/// `open` is read-only and never creates anything; directories/files are created
/// lazily by `save_asset` / `save_flow` **after** all validation passed, so a
/// rejected save writes nothing.
#[derive(Debug, Clone)]
pub struct TaskBundle {
    root: PathBuf,
    /// v1.2.1: a pure global asset store (`assets/`) has no meta/flow; saves must
    /// not create task files or run flow cross-checks.
    assets_only: bool,
}

/// v1.2.3 U-01(B): a `call_task` step is a live reference. Before parsing, the
/// caller's raw JSON is expanded with the target task's current steps. This keeps
/// `flow.json` readable/reusable and makes target edits visible on the next run.
///
/// Runner Studio split §C.4: the target steps are wrapped in an in-memory
/// `with_params` scope (never written to disk) together with
/// - `params`: the call-site mapping (expression strings, evaluated by the
///   runtime in the **caller** scope),
/// - `defaults`: the target's `meta.params[].default` values,
/// - `block`: the target's declared keys when `inherit:"none"` (strict scope).
pub(crate) const MAX_CALL_DEPTH: usize = 8;

/// Apply `f` to every nested step array of one raw JSON step:
/// `then`/`else` (if), top-level `body` (loop/while) and `args.body`
/// (`with_params`). Used by finish stripping and call/collection expansion so
/// an authored `with_params` is traversed exactly like the generated one.
fn for_each_nested_steps_mut(
    step: &mut serde_json::Value,
    f: &mut dyn FnMut(&mut Vec<serde_json::Value>) -> Result<(), String>,
) -> Result<(), String> {
    for key in ["then", "else", "body"] {
        if let Some(arr) = step.get_mut(key).and_then(|x| x.as_array_mut()) {
            f(arr)?;
        }
    }
    if let Some(arr) = step
        .get_mut("args")
        .and_then(|a| a.get_mut("body"))
        .and_then(|x| x.as_array_mut())
    {
        f(arr)?;
    }
    Ok(())
}

fn strip_finish_steps(steps: &mut Vec<serde_json::Value>) {
    steps.retain(|s| s.get("op").and_then(|x| x.as_str()) != Some("finish"));
    for s in steps.iter_mut() {
        let _ = for_each_nested_steps_mut(s, &mut |arr| {
            strip_finish_steps(arr);
            Ok(())
        });
    }
}

/// `call_task.args.params`: child key -> expression string (§C.4.3).
///
/// Expressions are validated here so a typo fails at load with the call site in
/// the message instead of surfacing as a confusing path inside the target body.
fn parse_call_params_json(
    args: &serde_json::Value,
    task: &str,
) -> Result<BTreeMap<String, String>, String> {
    let Some(v) = args.get("params") else {
        return Ok(BTreeMap::new());
    };
    let obj = v.as_object().ok_or_else(|| {
        format!(
            "call_task `{task}`: args.params must be an object mapping \
             parameter key -> expression string"
        )
    })?;
    let mut out = BTreeMap::new();
    for (key, value) in obj {
        let src = value.as_str().ok_or_else(|| {
            format!("call_task `{task}`: args.params.{key} must be an expression string")
        })?;
        oas_flow::parse_expr(src)
            .map_err(|e| format!("call_task `{task}`: args.params.{key}: {e}"))?;
        out.insert(key.clone(), src.to_string());
    }
    Ok(out)
}

/// `call_task.args.inherit`: `"all"` (default) or `"none"` (§C.4.4).
fn parse_call_inherit_json(
    args: &serde_json::Value,
    task: &str,
) -> Result<oas_flow::CallInherit, String> {
    match args.get("inherit") {
        None => Ok(oas_flow::CallInherit::All),
        Some(serde_json::Value::String(s)) => match s.as_str() {
            "all" => Ok(oas_flow::CallInherit::All),
            "none" => Ok(oas_flow::CallInherit::None),
            other => Err(format!(
                "call_task `{task}`: args.inherit must be \"all\" or \"none\" (got `{other}`)"
            )),
        },
        Some(other) => Err(format!(
            "call_task `{task}`: args.inherit must be the string \"all\" or \"none\", got {other}"
        )),
    }
}

/// Read a call target's `meta.json` for `defaults`/`block` injection.
///
/// Strict by design (P1-2): a missing/unreadable/malformed target `meta.json`
/// is a **hard load error**, never a silent fallback to "no defaults". A
/// fallback would also leave `block` empty for `inherit:"none"`, i.e. it would
/// silently turn strict isolation into `inherit:"all"`, so the caller must be
/// told instead. The message names the call target and the specific problem.
fn load_call_target_meta(tasks_dir: &Path, task: &str) -> Result<Meta, String> {
    let path = tasks_dir.join(task).join("meta.json");
    match std::fs::read_to_string(&path) {
        Ok(text) => {
            Meta::from_json(&text).map_err(|e| format!("meta.json is invalid: {e}"))
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Err(format!(
            "meta.json not found ({}); a call target must be a \
             complete task bundle (meta.json + flow.json)",
            path.display()
        )),
        Err(e) => Err(format!("cannot read {}: {e}", path.display())),
    }
}

/// Stable 64-bit FNV-1a hash of the **expanded** flow JSON (P1-3).
///
/// FNV has no random seed, so the fingerprint is identical across processes;
/// it must not depend on `HashMap` iteration order. The JSON text is produced
/// by `serde_json::to_string` over a `serde_json::Value`, whose object maps are
/// sorted (`BTreeMap`), so equal flows always hash identically and any target
/// edit that changes the expanded flow changes the hash.
fn expanded_flow_fingerprint(text: &str) -> u64 {
    const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = FNV_OFFSET_BASIS;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

fn expand_call_steps(
    steps: &mut Vec<serde_json::Value>,
    tasks_dir: &Path,
    stack: &mut Vec<String>,
    depth: usize,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    if depth > MAX_CALL_DEPTH {
        return Err(format!("call_task nesting exceeds {MAX_CALL_DEPTH}"));
    }
    let mut out = Vec::with_capacity(steps.len());
    for mut step in steps.drain(..) {
        if step.get("op").and_then(|x| x.as_str()) == Some("call_task") {
            let args = step
                .get("args")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));
            let task = args
                .get("task")
                .and_then(|x| x.as_str())
                .ok_or_else(|| "call_task requires args.task".to_string())?
                .to_string();
            validate_task_name(&task)?;
            let params = parse_call_params_json(&args, &task)?;
            let inherit = parse_call_inherit_json(&args, &task)?;
            if stack.iter().any(|s| s == &task) {
                return Err(format!(
                    "call_task cycle: {} -> {task}",
                    stack.join(" -> ")
                ));
            }
            let caller = stack.last().cloned().unwrap_or_default();
            let at = format!("call_task `{task}` (called from `{caller}`)");
            let target_flow = tasks_dir.join(&task).join("flow.json");
            let text = std::fs::read_to_string(&target_flow)
                .map_err(|e| format!("{at}: read {}: {e}", target_flow.display()))?;
            let mut target: serde_json::Value = serde_json::from_str(&text)
                .map_err(|e| format!("{at}: target flow.json: {e}"))?;
            // Parse the target's own DSL now: a broken target then fails here,
            // naming the target/call site, instead of surfacing later as a bare
            // `steps[i].args.body[k]` path inside the expanded caller flow.
            oas_flow::Flow::from_json(&text)
                .map_err(|e| format!("{at}: target flow.json is invalid: {e}"))?;
            let mut target_steps = target
                .get_mut("steps")
                .and_then(|x| x.as_array_mut())
                .map(|a| std::mem::take(a))
                .ok_or_else(|| format!("{at}: target flow.json has no steps array"))?;
            // A task's terminal finishes are "return", not caller termination.
            strip_finish_steps(&mut target_steps);
            // P1-2: the target meta is mandatory, not a warning path. Without it
            // `defaults` and (for inherit:"none") `block` would silently be empty.
            let target_meta = load_call_target_meta(tasks_dir, &task)
                .map_err(|e| format!("{at}: {e}"))?;
            stack.push(task.clone());
            expand_call_steps(&mut target_steps, tasks_dir, stack, depth + 1, warnings)?;
            stack.pop();
            // §C.4.3 + V2.0: subtask defaults come from the child's effective
            // variables (scope=input), literal JSON values. A default that is not
            // a scalar is a broken target meta: fail the caller's load (P1-2),
            // never silently drop the default while the subtask still runs.
            // Runtime-scope variables are flow-local and are not injected.
            let mut defaults = serde_json::Map::new();
            let mut declared_keys = Vec::new();
            for var in target_meta.effective_variables() {
                if !var.is_input() {
                    continue;
                }
                declared_keys.push(var.key.clone());
                if let Some(default) = &var.default {
                    oas_flow::json_to_value(default).map_err(|e| {
                        format!(
                            "{at}: meta.variables `{}`.default is not a scalar value: {e}",
                            var.key
                        )
                    })?;
                    defaults.insert(var.key.clone(), default.clone());
                }
            }
            // `inherit:"all"` needs no block list: same-name values simply stay
            // visible in the shared parameter table (today's behaviour).
            let block = if inherit == oas_flow::CallInherit::None {
                declared_keys
            } else {
                Vec::new()
            };
            out.push(serde_json::json!({
                "op": "with_params",
                "args": {
                    "task": task,
                    "params": params,
                    "defaults": serde_json::Value::Object(defaults),
                    "block": block,
                    "body": target_steps,
                }
            }));
            continue;
        }
        for_each_nested_steps_mut(&mut step, &mut |arr| {
            expand_call_steps(arr, tasks_dir, stack, depth + 1, warnings)
        })?;
        out.push(step);
    }
    *steps = out;
    Ok(())
}

fn expand_collection_steps(
    steps: &mut Vec<serde_json::Value>,
    collections: &[AssetCollection],
    items: &[AssetDecl],
) -> Result<(), String> {
    for step in steps.iter_mut() {
        let op = step.get("op").and_then(|x| x.as_str()).unwrap_or("");
        if op == "click" {
            if let Some(args) = step.get_mut("args").and_then(|x| x.as_object_mut()) {
                let sel = args.get("sel").and_then(|x| x.as_str()).map(str::to_string);
                if let Some(sel) = sel {
                    if let Some(coll) = collections.iter().find(|c| c.id == sel) {
                        let first = coll.members.first().cloned().ok_or_else(|| {
                            format!("素材集合 `{sel}` 没有成员")
                        })?;
                        args.insert("sel".into(), serde_json::Value::String(first));
                        let mut alts: Vec<serde_json::Value> = args
                            .get("alts")
                            .and_then(|x| x.as_array())
                            .cloned()
                            .unwrap_or_default();
                        for m in coll.members.iter().skip(1) {
                            if !alts.iter().any(|a| a.as_str() == Some(m)) {
                                alts.push(serde_json::Value::String(m.clone()));
                            }
                        }
                        args.insert("alts".into(), serde_json::Value::Array(alts));
                    }
                }
            }
        } else if op == "click_target" {
            if let Some(args) = step.get_mut("args").and_then(|x| x.as_object_mut()) {
                if let Some(find) = args.get_mut("find").and_then(|x| x.as_array_mut()) {
                    let mut out = Vec::with_capacity(find.len());
                    for loc in find.drain(..) {
                        let sel = loc
                            .get("sel")
                            .and_then(|x| x.as_str())
                            .map(str::to_string);
                        let kind = loc.get("kind").and_then(|x| x.as_str()).unwrap_or("");
                        if let (Some(sel), "template") = (sel, kind) {
                            if let Some(coll) = collections.iter().find(|c| c.id == sel) {
                                // seq132: collection members may be any asset kind.
                                // Each member becomes the locator for ITS kind:
                                // template → template locator; OCR/ocr_var → OCR
                                // text locator (expected text or the ocr_var
                                // parameter expression); region → fixed region
                                // locator. Member order is preserved.
                                for m in &coll.members {
                                    let item = items.iter().find(|a| &a.id == m).ok_or_else(|| {
                                        format!(
                                            "素材集合 `{sel}` 的成员 `{m}` 不存在于 assets.json"
                                        )
                                    })?;
                                    let member_loc = match item.kind.as_str() {
                                        "template" => serde_json::json!({
                                            "kind": "template",
                                            "sel": m,
                                        }),
                                        "ocr_var" => {
                                            let key = item
                                                .param_key
                                                .clone()
                                                .filter(|k| !k.trim().is_empty())
                                                .ok_or_else(|| {
                                                    format!(
                                                        "素材集合 `{sel}` 的 ocr_var 成员 `{m}` 没有 param_key"
                                                    )
                                                })?;
                                            serde_json::json!({
                                                "kind": "ocr_text",
                                                "sel": m,
                                                "text_expr": format!("param('{key}')"),
                                            })
                                        }
                                        "ocr_line" | "ocr_lines" => {
                                            let text = item.ocr_text.clone().filter(|t| !t.trim().is_empty())
                                                .ok_or_else(|| {
                                                    format!(
                                                        "素材集合 `{sel}` 的 OCR 成员 `{m}` 没有期望文字 \
                                                         (ocr_text)，无法作为 click_target 集合成员；\
                                                         请给该素材设置识别文字或改用模板/ocr_var 成员"
                                                    )
                                                })?;
                                            serde_json::json!({
                                                "kind": "ocr_text",
                                                "sel": m,
                                                "text": text,
                                            })
                                        }
                                        "region" => {
                                            let roi = item.roi.ok_or_else(|| {
                                                format!(
                                                    "素材集合 `{sel}` 的 region 成员 `{m}` 没有 roi"
                                                )
                                            })?;
                                            serde_json::json!({
                                                "kind": "region",
                                                "roi": roi,
                                            })
                                        }
                                        other => {
                                            return Err(format!(
                                                "素材集合 `{sel}` 的成员 `{m}` kind=`{other}` \
                                                 不能作为 click_target 集合成员"
                                            ))
                                        }
                                    };
                                    out.push(member_loc);
                                }
                                continue;
                            }
                        }
                        out.push(loc);
                    }
                    *find = out;
                }
            }
        }
        for_each_nested_steps_mut(step, &mut |arr| {
            expand_collection_steps(arr, collections, items)
        })?;
    }
    Ok(())
}

impl TaskBundle {
    /// Open (or prepare to create) the bundle rooted at `root`.
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, String> {
        let root = root.into();
        if root.as_os_str().is_empty() {
            return Err("task bundle root is empty".to_string());
        }
        if root.exists() && !root.is_dir() {
            return Err(format!(
                "task bundle root {} exists but is not a directory",
                root.display()
            ));
        }
        Ok(Self {
            root,
            assets_only: false,
        })
    }

    /// Open a **global asset store** rooted at `root` (`assets.json` + `images/`).
    /// Unlike a task bundle it has no meta/flow and never writes them.
    pub fn open_asset_store(root: impl Into<PathBuf>) -> Result<Self, String> {
        let root = root.into();
        if root.as_os_str().is_empty() {
            return Err("asset store root is empty".to_string());
        }
        if root.exists() && !root.is_dir() {
            return Err(format!(
                "asset store root {} exists but is not a directory",
                root.display()
            ));
        }
        Ok(Self {
            root,
            assets_only: true,
        })
    }

    pub fn is_assets_only(&self) -> bool {
        self.assets_only
    }

    /// Open `<tasks_dir>/<name>`, validating `name` against path traversal.
    pub fn open_named(tasks_dir: impl AsRef<Path>, name: &str) -> Result<Self, String> {
        validate_task_name(name)?;
        Self::open(tasks_dir.as_ref().join(name))
    }

    /// Bundle root directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn meta_path(&self) -> PathBuf {
        self.root.join("meta.json")
    }

    pub fn flow_path(&self) -> PathBuf {
        self.root.join("flow.json")
    }

    pub fn assets_path(&self) -> PathBuf {
        self.root.join("assets.json")
    }

    pub fn images_dir(&self) -> PathBuf {
        self.root.join("images")
    }

    // -- loading -----------------------------------------------------------

    /// Load `assets.json`. A missing file means "no assets yet" (`Ok(default)`);
    /// anything malformed is a loud error with JSON paths.
    pub fn load_assets_file(&self) -> Result<AssetsFile, String> {
        let path = self.assets_path();
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(AssetsFile::default()),
            Err(e) => return Err(format!("read {}: {e}", path.display())),
        };
        let mut af: AssetsFile = serde_json::from_str(&text)
            .map_err(|e| format!("{}: invalid JSON: {e}", path.display()))?;
        normalize_legacy_ocr_roi(&mut af);
        let errs = self.assets_shape_errors(&af);
        if errs.is_empty() {
            Ok(af)
        } else {
            Err(errs.join("; "))
        }
    }

    /// Load `flow.json` without expanding collections/call_task. Used for
    /// reference checks (the expanded flow no longer contains the collection id).
    fn load_flow_raw(&self) -> Result<Option<oas_flow::Flow>, String> {
        let path = self.flow_path();
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(format!("read {}: {e}", path.display())),
        };
        oas_flow::Flow::from_json(&text)
            .map(Some)
            .map_err(|e| format!("flow.json: {e}"))
    }

    fn flow_references_id(&self, id: &str) -> bool {
        self.load_flow_raw()
            .ok()
            .flatten()
            .map(|f| f.referenced_assets().contains(id))
            .unwrap_or(false)
    }

    /// Save a multi-image collection. seq132: members may be **any** declared
    /// asset kind (`template` / `ocr_line` / `ocr_lines` / `ocr_var` / `region`);
    /// nested collections are rejected, and a member may belong to only one
    /// collection.
    pub fn save_collection(&self, coll: AssetCollection) -> Result<AssetCollection, String> {
        let mut af = self.load_assets_file()?;
        let id = coll.id.trim().to_string();
        if id.is_empty() {
            return Err("collection id 不能为空".into());
        }
        if af.items.iter().any(|a| a.id == id) {
            return Err(format!("`{id}` 已经是普通素材，不能再作为集合 id"));
        }
        let mut members = Vec::new();
        for m in &coll.members {
            if members.contains(m) {
                return Err(format!("集合成员重复：`{m}`"));
            }
            if af.collections.iter().any(|c| &c.id == m) {
                return Err(format!(
                    "集合成员 `{m}` 是另一个集合；素材集合不支持嵌套集合"
                ));
            }
            let item = af
                .items
                .iter()
                .find(|a| &a.id == m)
                .ok_or_else(|| format!("集合成员 `{m}` 不存在"))?;
            if !KINDS.contains(&item.kind.as_str()) {
                return Err(format!(
                    "集合成员 `{m}` 的 kind `{}` 不受支持",
                    item.kind
                ));
            }
            if af
                .collections
                .iter()
                .any(|c| c.id != id && c.members.iter().any(|x| x == m))
            {
                return Err(format!("素材 `{m}` 已经属于另一个集合"));
            }
            members.push(m.clone());
        }
        if members.is_empty() {
            return Err("素材集合至少需要一个成员".into());
        }
        // B-10: a collection-level operation shape is accepted by the API, so
        // it must be validated exactly like an asset shape (bounds from the
        // bundle design resolution) instead of being silently copied.
        // 2026-11-06 事项6: no two-state rule any more — a shape is always
        // allowed (and optional); `action_source` is a deprecated compat field
        // that is tolerated on input and never persisted.
        if let Some(shape) = coll.action_shape.as_ref() {
            validate_action_shape(
                shape,
                &format!("collection `{id}` action_shape"),
                af.design_resolution.map(|[w, h]| (w, h)),
            )?;
        }
        let saved = AssetCollection {
            id: id.clone(),
            members,
            threshold: coll.threshold,
            action_shape: coll.action_shape,
            action_source: default_action_source(),
        };
        if let Some(pos) = af.collections.iter().position(|c| c.id == id) {
            af.collections[pos] = saved.clone();
        } else {
            af.collections.push(saved.clone());
        }
        self.write_assets_file(&af)?;
        Ok(saved)
    }

    /// Delete a collection. Referenced collections are blocked; members remain
    /// ordinary assets (the editor asks the user to remove flow references first).
    pub fn delete_collection(&self, id: &str) -> Result<AssetCollection, String> {
        let mut af = self.load_assets_file()?;
        let pos = af
            .collections
            .iter()
            .position(|c| c.id == id)
            .ok_or_else(|| format!("素材集合 `{id}` 不存在"))?;
        if self.flow_references_id(id) {
            return Err(format!(
                "素材集合「{id}」仍被 flow 引用；请先在任务构建中删除/替换引用，再删除集合"
            ));
        }
        if let Some(owner) = op_region_referrer(&af, id) {
            return Err(format!(
                "素材集合「{id}」仍被素材「{owner}」的操作区链引用；请先修改该操作区，再删除集合"
            ));
        }
        let item = af.collections.remove(pos);
        self.write_assets_file(&af)?;
        Ok(item)
    }

    /// Load `flow.json`, if present.
    ///
    /// v1.2.3 U-01(B): expand `call_task` references against the sibling task
    /// directories before parsing, so the runtime only sees ordinary steps.
    ///
    /// Runner Studio split §C.4.3: each expanded call becomes an in-memory
    /// `with_params` scope carrying the target's `meta.params[].default` values
    /// (and a block list for `inherit:"none"`). A broken call target `meta.json`
    /// is a hard error (P1-2), never a silent "no defaults" fallback.
    pub fn load_flow(&self) -> Result<Option<oas_flow::Flow>, String> {
        self.load_flow_with_fingerprint()
            .map(|(flow, _fingerprint, _warnings)| flow)
    }

    /// Like [`Self::load_flow`] but also returns non-fatal expansion warnings.
    ///
    /// Since P1-2 the loader is strict about call target `meta.json`/defaults,
    /// so the vector is currently always empty; the API is kept for future
    /// non-fatal diagnostics and for callers that already print it.
    pub fn load_flow_with_warnings(
        &self,
    ) -> Result<(Option<oas_flow::Flow>, Vec<String>), String> {
        self.load_flow_with_fingerprint()
            .map(|(flow, _fingerprint, warnings)| (flow, warnings))
    }

    /// Like [`Self::load_flow`] but also returns a stable fingerprint of the
    /// **expanded** flow JSON (P1-3).
    ///
    /// The fingerprint covers the caller flow plus every expanded subtask body,
    /// mapping and default. `apps/oasd` stores it in the checkpoint so editing a
    /// called subtask (even when the parent's top-level step count is unchanged)
    /// invalidates an old checkpoint instead of silently skipping the call.
    /// `0` is reserved for "no flow.json".
    pub fn load_flow_with_fingerprint(
        &self,
    ) -> Result<(Option<oas_flow::Flow>, u64, Vec<String>), String> {
        let path = self.flow_path();
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Ok((None, 0, Vec::new()))
            }
            Err(e) => return Err(format!("read {}: {e}", path.display())),
        };
        let mut value: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| format!("flow.json: invalid JSON: {e}"))?;
        let mut warnings = Vec::new();
        if let Some(steps) = value.get_mut("steps").and_then(|x| x.as_array_mut()) {
            let tasks_dir = self
                .root
                .parent()
                .ok_or_else(|| format!("task bundle `{}` has no parent tasks dir", self.root.display()))?;
            let current_task = self
                .root
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            let mut stack = vec![current_task];
            expand_call_steps(steps, tasks_dir, &mut stack, 0, &mut warnings)?;
            let af = self.load_assets_file().unwrap_or_default();
            expand_collection_steps(steps, &af.collections, &af.items)?;
        }
        let expanded = serde_json::to_string(&value)
            .map_err(|e| format!("flow.json: serialize expanded flow: {e}"))?;
        let fingerprint = expanded_flow_fingerprint(&expanded);
        oas_flow::Flow::from_json(&expanded)
            .map(|flow| (Some(flow), fingerprint, warnings))
            .map_err(|e| format!("flow.json: {e}"))
    }

    /// Build the runtime [`oas_runtime::AssetSet`]: template PNGs are decoded to
    /// `GrayImage` and mapped with their `roi`/NCC `threshold`; OCR assets are
    /// registered by `roi` (+ their optional minimum recognition confidence);
    /// `region` objects become pure operation regions.
    pub fn load_runtime_assets(&self) -> Result<oas_runtime::AssetSet, String> {
        let af = self.load_assets_file()?;
        let mut set = oas_runtime::AssetSet::new();
        for (i, item) in af.items.iter().enumerate() {
            if item.kind == "region" {
                // Pure operation region: no pixels, no recognition.
                let roi = item
                    .action_shape
                    .as_ref()
                    .and_then(|s| s.bbox())
                    .or(item.roi)
                    .ok_or_else(|| {
                        format!(
                            "assets.json: items[{i}] region `{}` has no action_shape/roi",
                            item.id
                        )
                    })?;
                set.insert_region(
                    item.id.clone(),
                    oas_types::Rect {
                        x: roi[0],
                        y: roi[1],
                        w: roi[2],
                        h: roi[3],
                    },
                );
                let click_shape = match item.action_shape.as_ref() {
                    Some(shape) => shape.to_click_shape(),
                    None => oas_runtime::ClickShape {
                        rect: Some(oas_types::Rect {
                            x: roi[0],
                            y: roi[1],
                            w: roi[2],
                            h: roi[3],
                        }),
                        points: None,
                        invert: false,
                    },
                };
                set.set_click_shape(item.id.clone(), click_shape);
                continue;
            }
            if item.kind != "template" {
                // OCR assets (`ocr_line` / `ocr_lines` / `ocr_var`) carry **no
                // pixels**: the runtime only needs their ROI so `ocr(sel)` can
                // crop it. `ocr_line` → single-line rec (fast); `ocr_lines` /
                // `ocr_var` → det + rec (ENGINE-DESIGN §17.1; `ocr_var` needs the
                // real line bbox for candidates/real-rect, 2026-11-06 redesign).
                // A missing ROI is a loud load error, never "skip silently" —
                // except for `ocr_var`, where an absent ROI means whole-screen
                // OCR.
                let is_ocr_var = item.kind == "ocr_var";
                let rect = match item.roi {
                    Some(roi) => Some(oas_types::Rect {
                        x: roi[0],
                        y: roi[1],
                        w: roi[2],
                        h: roi[3],
                    }),
                    None if is_ocr_var => None,
                    None => {
                        return Err(format!(
                            "assets.json: items[{i}].roi: OCR asset `{}` has no roi \
                             (ocr(sel) needs the text region)",
                            item.id
                        ))
                    }
                };
                // B-T2 (`crates/oas-runtime`) adds the dedicated `ocr_var` table
                // (empty ROI = full screen, `param_key` expected text). Until
                // then an empty-ROI `ocr_var` gets no OCR entry here: running it
                // fails loudly as "not declared" instead of recognizing a wrong
                // region. A ROI-scoped `ocr_var` already behaves exactly like
                // `ocr_lines` (det + rec).
                // 2026-11-06 参数→素材: `ocr_var` gets its dedicated runtime table
                // (optional ROI = whole-screen OCR, `runtime_text`/`param_key`
                // contract), so it is NOT registered as a plain OCR asset.
                if is_ocr_var {
                    let runtime_text = item.runtime_text.clone().ok_or_else(|| {
                        format!(
                            "assets.json: items[{i}] ocr_var `{}` has no runtime_text",
                            item.id
                        )
                    })?;
                    let param_key = item.param_key.clone().ok_or_else(|| {
                        format!(
                            "assets.json: items[{i}] ocr_var `{}` has no param_key",
                            item.id
                        )
                    })?;
                    set.insert_ocr_var(item.id.clone(), rect, runtime_text, param_key);
                } else if let Some(rect) = rect {
                    if item.kind == "ocr_lines" {
                        set.insert_ocr_lines(item.id.clone(), rect);
                    } else {
                        set.insert_ocr(item.id.clone(), rect);
                    }
                }
                // OCR never runs NCC; its `threshold` is the **minimum
                // recognition confidence**. Absent = any successful read is a
                // hit (historical behaviour); `0` is stored as-is and means the
                // same at query time (`AssetSet::ocr_threshold` filters it out),
                // so the editor can clear a threshold by writing 0 (P1-4).
                if let Some(t) = item.threshold {
                    set.set_ocr_threshold(item.id.clone(), t);
                }
                // Operation region: fixed shapes are wired as a bbox for now;
                // recognition sources follow the OCR result at runtime.
                if let Some(shape) = &item.action_shape {
                    if let Some(action) = shape.bbox() {
                        set.set_click_back(
                            item.id.clone(),
                            oas_types::Rect {
                                x: action[0],
                                y: action[1],
                                w: action[2],
                                h: action[3],
                            },
                        );
                    }
                    set.set_click_shape(item.id.clone(), shape.to_click_shape());
                }
                // v1.2.3: one recognition ROI for derived OCR; det+rec runs on
                // the saved `roi` and matches `ocr_text`.
                if item.kind == "ocr_line"
                    && item.ocr_mode.as_deref() == Some("lines")
                    && item.ocr_text.is_some()
                {
                    let derived_roi = rect.ok_or_else(|| {
                        format!(
                            "assets.json: items[{i}].roi: derived ocr_line `{}` has no roi",
                            item.id
                        )
                    })?;
                    set.set_derived_ocr(item.id.clone(), derived_roi, item.ocr_text.clone().unwrap());
                }
                // 2026-11-06 批次B (#3): every OCR asset with a saved expected
                // text registers it, so plain `ocr_lines` (and single-line
                // `ocr_line`) can act as `op_region` recognition gates and their
                // editor candidates are filtered by the same text.
                if item.kind != "ocr_var" {
                    if let Some(text) = item
                        .ocr_text
                        .as_deref()
                        .map(str::trim)
                        .filter(|t| !t.is_empty())
                    {
                        set.set_ocr_expected_text(item.id.clone(), text.to_string());
                    }
                }
                // §22.10 N10b-2: OCR polygon mask (frame coordinates).
                if let Some(poly) = &item.polygon {
                    let pts: Vec<oas_types::Point> = poly
                        .iter()
                        .map(|p| oas_types::Point { x: p[0], y: p[1] })
                        .collect();
                    set.set_ocr_polygon(item.id.clone(), pts);
                }
                // 2026-11-06 参数→素材 D2: the operation-region chain is data for
                // the runtime resolver (validation already happened on load).
                if let Some(op) = &item.op_region {
                    set.insert_op_region(item.id.clone(), runtime_op_region(op));
                }
                continue;
            }
            let file = item.file.as_ref().ok_or_else(|| {
                format!(
                    "assets.json: items[{i}].file: template asset `{}` has no file",
                    item.id
                )
            })?;
            let roi = item.roi;
            let path = self.root.join(file);
            let bytes = std::fs::read(&path)
                .map_err(|e| format!("read template {}: {e}", path.display()))?;
            let frame = oas_codec::decode_png(&bytes)
                .map_err(|e| format!("decode template {}: {e}", path.display()))?;
            // The PNG is already the ROI crop; `roi` is the search window.
            let tpl = oas_runtime::Template {
                gray: oas_perceive::GrayImage::from_frame(&frame),
                roi: roi.map(|r| oas_types::Rect {
                    x: r[0],
                    y: r[1],
                    w: r[2],
                    h: r[3],
                }),
                threshold: item.threshold.unwrap_or(DEFAULT_THRESHOLD),
                click_polygon: item.polygon.as_ref().map(|pts| {
                    pts.iter()
                        .map(|p| oas_types::Point { x: p[0], y: p[1] })
                        .collect()
                }),
            };
            set.insert(item.id.clone(), tpl);
            if let Some(shape) = &item.action_shape {
                if let Some(back) = shape.bbox() {
                    set.set_click_back(
                        item.id.clone(),
                        oas_types::Rect {
                            x: back[0],
                            y: back[1],
                            w: back[2],
                            h: back[3],
                        },
                    );
                }
                set.set_click_shape(item.id.clone(), shape.to_click_shape());
            }
            // 2026-11-06 参数→素材 D2: operation-region chain data for the runtime.
            if let Some(op) = &item.op_region {
                set.insert_op_region(item.id.clone(), runtime_op_region(op));
            }
        }
        for coll in &af.collections {
            if coll.members.is_empty() {
                return Err(format!("素材集合 `{}` 没有成员", coll.id));
            }
            for m in &coll.members {
                if !af.items.iter().any(|a| &a.id == m) {
                    return Err(format!(
                        "素材集合 `{}` 的成员 `{m}` 不存在",
                        coll.id
                    ));
                }
            }
            // B-10: wire a collection-level shape as the fallback geometry.
            // The winning member's own shape still takes precedence in runtime.
            if let Some(shape) = &coll.action_shape {
                set.set_click_shape(coll.id.clone(), shape.to_click_shape());
                if let Some(back) = shape.bbox() {
                    set.set_click_back(
                        coll.id.clone(),
                        oas_types::Rect {
                            x: back[0],
                            y: back[1],
                            w: back[2],
                            h: back[3],
                        },
                    );
                }
            }
            set.insert_collection(coll.id.clone(), coll.members.clone(), coll.threshold);
        }
        Ok(set)
    }

    // -- saving ------------------------------------------------------------

    /// Save one **new** asset.
    ///
    /// `frame` is required for `template` assets: the ROI is cropped out of it
    /// and written as the next `images/aNNN.png`. Before anything is written:
    ///
    /// 1. id grammar / kind / threshold / ROI bounds are checked,
    /// 2. duplicate ids are rejected (use [`Self::update_asset`] to overwrite),
    /// 3. the design resolution is enforced,
    /// 4. if `flow.json` exists, the **would-be** item list (existing items plus
    ///    this one) is validated against the flow — a flow that references a
    ///    different (typo'd) id makes the save fail and write nothing.
    pub fn save_asset(&self, decl: AssetDecl, frame: Option<&Frame>) -> Result<AssetDecl, String> {
        self.save_asset_impl(decl, frame, false)
    }

    /// Overwrite an **existing** asset with the same id (`rev + 1`).
    ///
    /// Design: ENGINE-DESIGN §4 keeps a per-asset `rev` ("改图后能报哪张图变了")
    /// — an editor that could only *create* would force a rename on every
    /// re-crop. Update is therefore an **explicit second path**, never an
    /// implicit upsert. Every check of [`Self::save_asset`] still applies; on
    /// any failure **nothing is written** (old image + manifest stay untouched).
    pub fn update_asset(&self, decl: AssetDecl, frame: Option<&Frame>) -> Result<AssetDecl, String> {
        self.save_asset_impl(decl, frame, true)
    }

    /// v1.2: save several recognized candidates in one all-or-nothing batch.
    ///
    /// Each item still goes through [`Self::save_asset`] validation; if any item
    /// fails, the manifest is restored and images created by this batch are
    /// removed. Global uniqueness is checked by the caller (it needs all tasks +
    /// library), but duplicate IDs inside the batch are rejected here.
    pub fn save_assets_batch(
        &self,
        decls: Vec<AssetDecl>,
        frame: Option<&Frame>,
    ) -> Result<Vec<AssetDecl>, String> {
        if decls.is_empty() {
            return Err("save_assets_batch: no assets given".into());
        }
        let mut seen = std::collections::BTreeSet::new();
        for d in &decls {
            if !seen.insert(d.id.clone()) {
                return Err(format!("duplicate asset id `{}` in the same batch", d.id));
            }
        }
        let before_text = std::fs::read_to_string(self.assets_path()).ok();
        let before_files: std::collections::BTreeSet<String> = self
            .load_assets_file()
            .map(|af| af.items.into_iter().filter_map(|a| a.file).collect())
            .unwrap_or_default();
        let mut out = Vec::with_capacity(decls.len());
        for decl in decls {
            match self.save_asset(decl, frame) {
                Ok(saved) => out.push(saved),
                Err(e) => {
                    // rollback manifest + any image files created by this batch
                    if let Some(text) = before_text {
                        let _ = write_atomic(&self.assets_path(), text.as_bytes());
                    } else {
                        let _ = std::fs::remove_file(self.assets_path());
                    }
                    if let Ok(after) = self.load_assets_file() {
                        for item in after.items {
                            if let Some(file) = item.file {
                                if !before_files.contains(&file) {
                                    let _ = std::fs::remove_file(self.root.join(file));
                                }
                            }
                        }
                    }
                    return Err(format!("batch save rolled back: {e}"));
                }
            }
        }
        Ok(out)
    }

    /// v1.1 asset CRUD: delete one asset (never while the flow still references it).
    ///
    /// v1.2.3 collection rule: a collection referenced by a flow may keep at
    /// least one member; deleting a member removes it from the collection. An
    /// unreferenced collection is removed automatically when its last member is
    /// deleted.
    /// Reorder assets by id. Listed ids come first in the given order; any
    /// unspecified assets keep their relative order after them. This is the
    /// only persisted ordering (the `items` array order in `assets.json`).
    pub fn reorder_assets(&self, ids: &[String]) -> Result<Vec<String>, String> {
        if ids.is_empty() {
            return Err("`ids` must not be empty".into());
        }
        let mut af = self.load_assets_file()?;
        let mut seen = std::collections::BTreeSet::new();
        for id in ids {
            if !seen.insert(id.clone()) {
                return Err(format!("duplicate asset id in reorder list: `{id}`"));
            }
            if !af.items.iter().any(|a| &a.id == id) {
                return Err(format!("asset `{id}` does not exist"));
            }
        }
        let mut ordered: Vec<AssetDecl> = Vec::with_capacity(af.items.len());
        for id in ids {
            if let Some(pos) = af.items.iter().position(|a| &a.id == id) {
                ordered.push(af.items.remove(pos));
            }
        }
        ordered.append(&mut af.items);
        af.items = ordered;
        self.write_assets_file(&af)?;
        Ok(ids.to_vec())
    }

    pub fn delete_asset(&self, id: &str) -> Result<AssetDecl, String> {
        let mut af = self.load_assets_file()?;
        let pos = af
            .items
            .iter()
            .position(|a| a.id == id)
            .ok_or_else(|| format!("asset `{id}` does not exist"))?;
        if self.flow_references_id(id) {
            return Err(format!(
                "素材「{id}」仍被当前任务 flow 引用；请先在任务构建中删除/替换引用，再删除素材"
            ));
        }
        // D2 (2026-11-06 参数→素材): an asset used as an `op_region` chain node
        // must not be deleted while the chain still references it.
        if let Some(owner) = op_region_referrer(&af, id) {
            return Err(format!(
                "素材「{id}」仍被素材「{owner}」的操作区链引用；请先修改该操作区，再删除素材"
            ));
        }
        let mut remove_collections: Vec<usize> = Vec::new();
        for (ci, coll) in af.collections.iter_mut().enumerate() {
            if !coll.members.iter().any(|m| m == id) {
                continue;
            }
            let referenced = self.flow_references_id(&coll.id);
            if referenced && coll.members.len() <= 1 {
                return Err(format!(
                    "素材集合「{}」仍被 flow 引用，且只剩一个成员；至少保留一个成员才能删除",
                    coll.id
                ));
            }
            coll.members.retain(|m| m != id);
            if coll.members.is_empty() {
                remove_collections.push(ci);
            }
        }
        for ci in remove_collections.into_iter().rev() {
            af.collections.remove(ci);
        }
        let item = af.items.remove(pos);
        self.write_assets_file(&af)?;
        if let Some(file) = &item.file {
            let path = self.root.join(file);
            if path.starts_with(self.images_dir()) && path.is_file() {
                let _ = std::fs::remove_file(path);
            }
        }
        Ok(item)
    }

    /// v1.1 asset CRUD: rename an asset id (flow references must be updated first).
    pub fn rename_asset(&self, from: &str, to: &str) -> Result<AssetDecl, String> {
        validate_id(to)?;
        let mut af = self.load_assets_file()?;
        if af.items.iter().any(|a| a.id == to)
            || af.collections.iter().any(|c| c.id == to)
        {
            return Err(format!("asset `{to}` already exists"));
        }
        let pos = af
            .items
            .iter()
            .position(|a| a.id == from)
            .ok_or_else(|| format!("asset `{from}` does not exist"))?;
        if self.flow_references_id(from) {
            return Err(format!(
                "asset `{from}` is still referenced by flow.json; update the flow reference to `{to}` first"
            ));
        }
        for coll in af.collections.iter_mut() {
            for m in coll.members.iter_mut() {
                if m == from {
                    *m = to.to_string();
                }
            }
        }
        // D2: rewrite `op_region` chain references so a rename never leaves a
        // dangling chain behind.
        for item in af.items.iter_mut() {
            if let Some(OpRegion::Asset { asset, .. }) = item.op_region.as_mut() {
                if asset == from {
                    *asset = to.to_string();
                }
            }
        }
        af.items[pos].id = to.to_string();
        af.items[pos].rev = Some(af.items[pos].rev.unwrap_or(0) + 1);
        let item = af.items[pos].clone();
        self.write_assets_file(&af)?;
        Ok(item)
    }

    /// v1.1 asset CRUD: update OCR ROI / template-or-OCR threshold without
    /// re-cropping.
    ///
    /// `threshold` is the NCC threshold for `template` assets and the **minimum
    /// recognition confidence** for `ocr_line`/`ocr_lines` assets (`None` leaves
    /// the stored value untouched; `region` assets reject it).
    ///
    /// For an OCR asset `Some(0.0)` is accepted and means the same as "unset":
    /// no confidence filter. The editor can only clear a field by writing `0`
    /// (`None` = "no change"), so the runtime normalizes `0` to "unset" on every
    /// query (`AssetSet::ocr_threshold`). Templates keep `0` as a real NCC
    /// threshold (anything matches), which is their historical behaviour.
    ///
    /// Template ROI changes are rejected (the crop image would go stale); use
    /// `update_asset`/「保存素材覆盖」to re-crop a template.
    ///
    /// `keep_action_shape` (B-08b) is the editor's **识别区-only** switch. When
    /// `true` and the asset is a pure `region`, a `roi` update changes only the
    /// persisted ROI and leaves `action_shape` untouched: a region's operation
    /// polygon must survive a recognition-box move instead of being downgraded
    /// to `rect(roi)`. `false` keeps the historical B-08 synchronisation, so old
    /// callers/tests are unaffected.
    ///
    /// 2026-11-06 事项6: the old `action_source` two-state model is gone. A shape
    /// update is always accepted (any kind) and is validated geometrically only;
    /// there is no source to require, reject or use for clearing. Old data's
    /// `action_source` is read for compatibility and never written back.
    ///
    /// v2 (2026-11-06 参数→素材): also updates the ocr_var fields and the
    /// asset's `op_region` chain:
    /// * `runtime_text` / `label` apply to `kind:"ocr_var"` only and are
    ///   validated like the save path;
    /// * `param_key` is **read-only** after creation (Lead ruling 6) — a
    ///   different value is a loud error;
    /// * `op_region`: `None` = unchanged, `Some(None)` = clear (back to the
    ///   `self` default), `Some(Some(op))` = set. The whole chain is validated
    ///   against the would-be `assets.json` (node existence/kind, terminal mode,
    ///   cycle + depth) before anything is written.
    ///
    /// v3 (2026-11-06 批次B, #6/#10): adds the recognition `polygon` tri-state
    /// `None` = unchanged, `Some(None)` = clear, `Some(Some(points))` = set.
    /// The same [`validate_recognition_polygon`] rules as the save path apply;
    /// for non-template assets the recognition `roi` is re-derived from the
    /// polygon bbox (so mask and ROI can never disagree). A template update only
    /// accepts a polygon whose bbox equals its existing ROI (changing it needs a
    /// full overwrite/re-crop). `update_asset_meta_v2` stays as the
    /// polygon-less compatibility wrapper.
    #[allow(clippy::too_many_arguments)]
    pub fn update_asset_meta_v3(
        &self,
        id: &str,
        roi: Option<[i32; 4]>,
        action_shape: Option<ActionShape>,
        threshold: Option<f32>,
        keep_action_shape: bool,
        polygon: Option<Option<Vec<[i32; 2]>>>,
        op_region: Option<Option<OpRegion>>,
        runtime_text: Option<String>,
        label: Option<String>,
        param_key: Option<String>,
    ) -> Result<AssetDecl, String> {
        let mut af = self.load_assets_file()?;
        let pos = af
            .items
            .iter()
            .position(|a| a.id == id)
            .ok_or_else(|| format!("asset `{id}` does not exist"))?;
        let kind = af.items[pos].kind.clone();
        // B-05: same bounds source as the save path (`design_resolution`).
        let shape_bounds = af.design_resolution.map(|[w, h]| (w, h));
        if let Some(r) = roi {
            if r[2] <= 0 || r[3] <= 0 {
                return Err("roi width/height must be > 0".into());
            }
            if kind == "template" {
                return Err(
                    "template ROI update needs re-crop: save the asset again with 覆盖 (rev+1)"
                        .into(),
                );
            }
            // B-08b: even when the operation shape is kept, the persisted
            // region ROI must stay a frame-legal rect. The old B-08 sync
            // validated it implicitly through `rect(roi)`; keep that guarantee
            // on the ROI-only path instead of writing an unusable fallback.
            if kind == "region" {
                let [x, y, w, h] = r;
                if x < 0 || y < 0 {
                    return Err(format!(
                        "region roi [{x},{y},{w},{h}] must have x,y >= 0"
                    ));
                }
                if let Some((fw, fh)) = shape_bounds {
                    if x as i64 + w as i64 > fw as i64 || y as i64 + h as i64 > fh as i64 {
                        return Err(format!(
                            "region roi [{x},{y},{w},{h}] is outside the {fw}x{fh} frame"
                        ));
                    }
                }
            }
            af.items[pos].roi = Some(r);
            // B-08: a pure region's `roi` is a fallback alias of its operation
            // shape; keep them in sync instead of leaving a stale shape that
            // runtime would silently prefer. B-08b: an explicit
            // `keep_action_shape` (editor 识别区 edit) opts out, so a polygon
            // operation shape is never overwritten by the recognition box.
            if kind == "region" && action_shape.is_none() && !keep_action_shape {
                let shape = ActionShape::rect(r);
                validate_action_shape(&shape, "region roi", shape_bounds)?;
                af.items[pos].action_shape = Some(shape);
            }
        }
        let has_shape = action_shape.is_some();
        if let Some(shape) = action_shape {
            validate_action_shape(&shape, "action_shape", shape_bounds)?;
            if kind == "region" {
                // B-08: for a pure region the action_shape IS the operation
                // region; keep `roi` (the load fallback / bbox) in sync.
                let bbox = shape.bbox().ok_or_else(|| {
                    "region action_shape has no bbox (needs rect.roi or polygon.points)".to_string()
                })?;
                af.items[pos].roi = Some(bbox);
            }
            af.items[pos].action_shape = Some(shape);
        }
        if let Some(t) = threshold {
            if !(0.0..=1.0).contains(&t) {
                return Err("threshold must be 0..=1".into());
            }
            // `template` = NCC threshold; OCR = minimum recognition confidence.
            // A pure `region` has no recognition at all, so a threshold on it
            // would be silently meaningless.
            if kind == "region" {
                return Err("threshold does not apply to region assets".into());
            }
            af.items[pos].threshold = Some(t);
        }
        // --- recognition polygon (v3, #6/#10) --------------------------------
        let has_poly_update = polygon.is_some();
        if let Some(new_poly) = polygon {
            match new_poly {
                None => af.items[pos].polygon = None,
                Some(points) => {
                    if kind == "region" {
                        return Err(format!(
                            "asset `{id}`: region assets have no recognition polygon"
                        ));
                    }
                    let bounds = shape_bounds.map(|(w, h)| (w as i32, h as i32));
                    let bbox = validate_recognition_polygon(&points, bounds, id)?;
                    if kind == "template" {
                        // The template PNG is the crop of the existing ROI; a
                        // polygon edit that moves the recognition box would leave
                        // the crop stale, so require a full overwrite instead.
                        if af.items[pos].roi != Some(bbox) {
                            return Err(format!(
                                "asset `{id}`: template polygon change must keep the existing \
                                 ROI ({:?}); use 覆盖保存 to re-crop the template",
                                af.items[pos].roi
                            ));
                        }
                    } else {
                        // The recognition mask and the ROI can never disagree:
                        // the bbox is the mask's crop window (same rule as save).
                        af.items[pos].roi = Some(bbox);
                    }
                    af.items[pos].polygon = Some(points);
                }
            }
        }
        // --- ocr_var fields (runtime_text / label / param_key) ---------------
        if runtime_text.is_some() || label.is_some() || param_key.is_some() {
            if kind != "ocr_var" {
                return Err(format!(
                    "asset `{id}`: runtime_text/label/param_key are only valid for kind=ocr_var"
                ));
            }
            if let Some(new_key) = param_key.as_deref() {
                if af.items[pos].param_key.as_deref() != Some(new_key) {
                    return Err(format!(
                        "asset `{id}`: param_key is read-only after creation \
                         (changing it would drop the usage-side value)"
                    ));
                }
            }
            let mut probe = af.items[pos].clone();
            if let Some(text) = runtime_text.clone() {
                probe.runtime_text = Some(text);
            }
            if let Some(lbl) = label.clone() {
                probe.label = Some(lbl);
            }
            validate_ocr_var_fields(&probe, &format!("asset `{id}`"))?;
            af.items[pos] = probe;
        }
        // --- op_region chain --------------------------------------------------
        let has_op_update = op_region.is_some();
        if let Some(new_op) = op_region {
            if let Some(op) = new_op.as_ref() {
                validate_op_region_shape(op, &format!("asset `{id}` op_region"))?;
            }
            af.items[pos].op_region = new_op;
        }
        if roi.is_none()
            && !has_shape
            && threshold.is_none()
            && !has_poly_update
            && !has_op_update
            && runtime_text.is_none()
            && label.is_none()
            && param_key.is_none()
        {
            return Err(
                "nothing to update (pass roi, action_shape, threshold, polygon, op_region, \
                 runtime_text/label or param_key)"
                    .into(),
            );
        }
        // D2: the whole graph must stay valid after the change (node
        // existence/kind, terminal mode, cycle + depth); nothing written yet.
        if let Some((p, msg)) = op_region_issues(&af).into_iter().next() {
            return Err(format!("{p}: {msg}; nothing written"));
        }
        // Deprecated compat field: never persisted, normalized in memory.
        af.items[pos].action_source = default_action_source();
        af.items[pos].rev = Some(af.items[pos].rev.unwrap_or(0) + 1);
        let item = af.items[pos].clone();
        self.write_assets_file(&af)?;
        Ok(item)
    }

    /// Backward-compatible wrapper of [`Self::update_asset_meta_v3`] without the
    /// recognition-polygon argument (2026-11-06 批次B: serve-dev can switch to
    /// `update_asset_meta_v3` when the HTTP route accepts `polygon`).
    #[allow(clippy::too_many_arguments)]
    pub fn update_asset_meta_v2(
        &self,
        id: &str,
        roi: Option<[i32; 4]>,
        action_shape: Option<ActionShape>,
        threshold: Option<f32>,
        keep_action_shape: bool,
        op_region: Option<Option<OpRegion>>,
        runtime_text: Option<String>,
        label: Option<String>,
        param_key: Option<String>,
    ) -> Result<AssetDecl, String> {
        self.update_asset_meta_v3(
            id,
            roi,
            action_shape,
            threshold,
            keep_action_shape,
            None,
            op_region,
            runtime_text,
            label,
            param_key,
        )
    }

    /// Backward-compatible wrapper of [`Self::update_asset_meta_v2`] for the
    /// pre-2026-11-06 callers (P0b signature): it updates ROI / `action_shape` /
    /// threshold only. The 4th argument is the deprecated `action_source`; it is
    /// accepted and **ignored** (see P0b).
    pub fn update_asset_meta(
        &self,
        id: &str,
        roi: Option<[i32; 4]>,
        action_shape: Option<ActionShape>,
        _action_source: Option<String>,
        threshold: Option<f32>,
        keep_action_shape: bool,
    ) -> Result<AssetDecl, String> {
        self.update_asset_meta_v2(
            id,
            roi,
            action_shape,
            threshold,
            keep_action_shape,
            None,
            None,
            None,
            None,
        )
    }

    /// v1.2.3 frame-asset link: replace an asset's frame positions. Empty is
    /// allowed here so a user can clear positions before deleting a frame.
    pub fn update_asset_positions(
        &self,
        id: &str,
        positions: &[u64],
    ) -> Result<AssetDecl, String> {
        let mut af = self.load_assets_file()?;
        let pos = af
            .items
            .iter()
            .position(|a| a.id == id)
            .ok_or_else(|| format!("asset `{id}` does not exist"))?;
        af.items[pos].positions = positions.to_vec();
        af.items[pos].rev = Some(af.items[pos].rev.unwrap_or(0) + 1);
        let item = af.items[pos].clone();
        self.write_assets_file(&af)?;
        Ok(item)
    }

    fn save_asset_impl(
        &self,
        mut decl: AssetDecl,
        frame: Option<&Frame>,
        replace: bool,
    ) -> Result<AssetDecl, String> {
        validate_id(&decl.id).map_err(|e| format!("asset rejected: {e}"))?;
        if !KINDS.contains(&decl.kind.as_str()) {
            return Err(format!(
                "asset `{}` rejected: unknown kind `{}` (allowed: {})",
                decl.id,
                decl.kind,
                KINDS.join(", ")
            ));
        }
        validate_threshold(decl.threshold)
            .map_err(|e| format!("asset `{}` rejected: {e}", decl.id))?;
        // 2026-11-06 参数→素材: `ocr_var` field contract. runtime_text /
        // param_key / label are ocr_var-only; every other kind must not carry
        // them (a stale field would silently become dead data).
        if decl.kind == "ocr_var" {
            validate_ocr_var_fields(&decl, &format!("asset `{}`", decl.id))?;
        } else {
            for (field, present) in [
                ("runtime_text", decl.runtime_text.is_some()),
                ("param_key", decl.param_key.is_some()),
                ("label", decl.label.is_some()),
            ] {
                if present {
                    return Err(format!(
                        "asset `{}` rejected: {field} is only valid for kind=ocr_var",
                        decl.id
                    ));
                }
            }
        }

        // v1.2.1: OCR recognition provenance. A derived `ocr_line` saved from a
        // multi-line det+rec result must keep the source region + expected text
        // so runtime can reproduce the original recognition method.
        //
        // 2026-11-06 批次B (#3): `ocr_text` (expected text) is now also allowed
        // on a plain `ocr_lines` asset, so it can act as an `op_region`
        // recognition gate (the chain validator requires it for OCR nodes).
        if decl.ocr_mode.is_some() || decl.ocr_source_roi.is_some() {
            if decl.kind != "ocr_line" {
                return Err(format!(
                    "asset `{}` rejected: ocr_mode/ocr_source_roi are only valid \
                     for ocr_line assets",
                    decl.id
                ));
            }
            let mode = decl.ocr_mode.as_deref().unwrap_or("single");
            match mode {
                "single" => {
                    if decl.ocr_source_roi.is_some() {
                        return Err(format!(
                            "asset `{}` rejected: ocr_mode=single must not carry \
                             ocr_source_roi",
                            decl.id
                        ));
                    }
                }
                "lines" => {
                    // Legacy source is optional; recognition uses `roi` only
                    // (see normalize_legacy_ocr_roi).
                    if let Some(src) = decl.ocr_source_roi {
                        if src[2] <= 0 || src[3] <= 0 || src[0] < 0 || src[1] < 0 {
                            return Err(format!(
                                "asset `{}` rejected: ocr_source_roi must have x,y >= 0 and w,h > 0",
                                decl.id
                            ));
                        }
                    }
                    if decl.ocr_text.as_deref().unwrap_or("").trim().is_empty() {
                        return Err(format!(
                            "asset `{}` rejected: ocr_mode=lines requires non-empty ocr_text",
                            decl.id
                        ));
                    }
                }
                other => {
                    return Err(format!(
                        "asset `{}` rejected: unknown ocr_mode `{other}` \
                         (allowed: single, lines)",
                        decl.id
                    ))
                }
            }
        }
        if let Some(text) = decl.ocr_text.as_deref() {
            match decl.kind.as_str() {
                // Validated by the `ocr_mode=lines` branch above.
                "ocr_line" => {
                    if decl.ocr_mode.as_deref() != Some("lines") {
                        return Err(format!(
                            "asset `{}` rejected: ocr_line with ocr_text must use \
                             ocr_mode=lines",
                            decl.id
                        ));
                    }
                }
                "ocr_lines" => {
                    if text.trim().is_empty() {
                        return Err(format!(
                            "asset `{}` rejected: ocr_lines ocr_text must not be blank",
                            decl.id
                        ));
                    }
                }
                other => {
                    return Err(format!(
                        "asset `{}` rejected: ocr_text is only valid for ocr_line/ocr_lines \
                         assets (got kind `{other}`)",
                        decl.id
                    ))
                }
            }
        }

        // B-05: validate operation geometry against the frame the asset is
        // authored on (frame wins; otherwise the bundle design resolution).
        let mut af = self.load_assets_file()?;
        let shape_bounds = frame
            .map(|f| (f.width, f.height))
            .or(af.design_resolution.map(|[w, h]| (w, h)));
        // Matching uses a masked NCC inside the polygon; OCR uses the bbox crop
        // with outside-polygon pixels filled white; click uses the bbox.
        if let Some(poly) = &decl.polygon {
            let bounds = match frame {
                Some(f) => Some((f.width as i32, f.height as i32)),
                None => af.design_resolution.map(|[w, h]| (w as i32, h as i32)),
            };
            decl.roi = Some(validate_recognition_polygon(poly, bounds, &decl.id)?);
        }

        if decl.kind == "region" {
            if decl.file.is_some() {
                return Err(format!(
                    "asset `{}` rejected: region objects must not carry a file",
                    decl.id
                ));
            }
            if decl.roi.is_none() {
                return Err(format!(
                    "asset `{}` rejected: region requires roi [x,y,w,h] (operation region)",
                    decl.id
                ));
            }
            if decl.threshold.is_some() {
                return Err(format!(
                    "asset `{}` rejected: region objects must not carry a threshold",
                    decl.id
                ));
            }
            if decl.action_shape.is_none() {
                decl.action_shape = decl.roi.map(ActionShape::rect);
            }
        }
        // 2026-11-06 事项6: the `action_source` two-state model is removed. A
        // shape is always allowed and always optional for every kind (the
        // runtime follows the real recognized bbox when it is absent); only the
        // geometry itself is validated. `action_source` is a deprecated compat
        // field, tolerated on input and never persisted (see `AssetDecl`).
        if let Some(shape) = decl.action_shape.as_ref() {
            validate_action_shape(shape, "action_shape", shape_bounds)?;
        }
        // D2: structural `op_region` check; the full chain is validated on the
        // would-be assets.json below (needs the other assets).
        if let Some(op) = decl.op_region.as_ref() {
            validate_op_region_shape(op, &format!("asset `{}` op_region", decl.id))?;
        }
        // Deprecated compat field: normalized in memory, never persisted.
        decl.action_source = default_action_source();
        let existing = af.items.iter().position(|it| it.id == decl.id);        let old_file = existing.and_then(|i| af.items[i].file.clone());
        let rev = match (replace, existing) {
            (false, Some(i)) => {
                return Err(format!(
                    "asset id `{}` already exists (assets.json items[{i}]); nothing written - \
                     send `replace: true` to overwrite it (rev+1)",
                    decl.id
                ))
            }
            (true, None) => {
                return Err(format!(
                    "asset id `{}` does not exist; nothing written - drop `replace` to create it",
                    decl.id
                ))
            }
            (false, None) => 1,
            (true, Some(i)) => af.items[i].rev.unwrap_or(1).saturating_add(1),
        };

        // Validate + compute everything; nothing touches the disk before the
        // flow cross-check below passed.
        let mut pending_image: Option<(PathBuf, Vec<u8>)> = None;
        if decl.kind == "template" {
            let roi = decl.roi.ok_or_else(|| {
                format!("template asset `{}` requires roi [x,y,w,h]", decl.id)
            })?;
            let frame = frame.ok_or_else(|| {
                format!(
                    "template asset `{}` requires a captured frame (POST /capture first)",
                    decl.id
                )
            })?;
            let cropped = crop_rgba(frame, roi)?;
            if let Some([dw, dh]) = af.design_resolution {
                if (dw, dh) != (frame.width, frame.height) {
                    return Err(format!(
                        "design resolution mismatch: bundle was authored for {dw}x{dh}, \
                         frame is {}x{}; nothing written",
                        frame.width, frame.height
                    ));
                }
            } else {
                af.design_resolution = Some([frame.width, frame.height]);
            }
            let png = oas_codec::encode_png(&cropped)
                .map_err(|e| format!("encode template crop: {e}"))?;
            let rel = self.next_image_rel_path()?;
            let abs = self.root.join(&rel);
            decl.file = Some(rel);
            decl.threshold = Some(decl.threshold.unwrap_or(DEFAULT_THRESHOLD));
            pending_image = Some((abs, png));
        } else {
            // 2026-11-06 参数→素材: an `ocr_var`'s ROI is optional (absent =
            // whole-screen OCR); every other non-template kind still needs one.
            if let Some(roi) = decl.roi {
                let [x, y, w, h] = roi;
                if w <= 0 || h <= 0 || x < 0 || y < 0 {
                    return Err(format!(
                        "asset `{}` roi [x,y,w,h] = {roi:?} must have x,y >= 0 and w,h > 0",
                        decl.id
                    ));
                }
            } else if decl.kind != "ocr_var" {
                return Err(format!(
                    "asset `{}` of kind `{}` requires roi [x,y,w,h]",
                    decl.id, decl.kind
                ));
            }
            // OCR assets carry no template pixels: the id never becomes a path.
            decl.file = None;
        }
        decl.rev = Some(rev);

        // D2: validate the whole `op_region` graph on the would-be file (node
        // existence/kind, terminal mode, cycle + depth). Nothing is written yet.
        {
            let mut would_be = af.clone();
            match existing {
                Some(i) => would_be.items[i] = decl.clone(),
                None => would_be.items.push(decl.clone()),
            }
            if let Some((p, msg)) = op_region_issues(&would_be).into_iter().next() {
                return Err(format!("{p}: {msg}; nothing written"));
            }
        }

        // Save-time reference validation (the N3 "judge"): the flow must already
        // be satisfiable by the *would-be* asset list.
        let ids: BTreeSet<String> = af
            .items
            .iter()
            .map(|i| i.id.clone())
            .chain(std::iter::once(decl.id.clone()))
            .collect();
        if !self.assets_only && self.flow_path().is_file() {
            let flow = self.load_flow()?.ok_or_else(|| {
                "flow.json exists but could not be loaded; nothing written".to_string()
            })?;
            if let Err(errs) = flow.validate_assets(&ids) {
                let joined = errs
                    .iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<_>>()
                    .join("; ");
                return Err(format!("flow.json: {joined}; nothing written"));
            }
        }

        // ----- writes start here (all validation already passed) -----
        if !self.assets_only {
            self.ensure_meta()?;
        }
        if let Some((abs, png)) = pending_image.as_ref() {
            write_atomic(abs, png)?;
        }
        match existing {
            Some(i) => af.items[i] = decl.clone(),
            None => af.items.push(decl.clone()),
        }
        if let Err(e) = self.write_assets_file(&af) {
            // Do not leave an orphan image behind when the manifest write failed.
            if let Some((abs, _)) = pending_image {
                let _ = std::fs::remove_file(abs);
            }
            return Err(e);
        }
        // A successful overwrite may have moved the asset to a new `aNNN.png`:
        // drop the OLD image only after the manifest points at the new one.
        if let (true, Some(old)) = (replace, old_file) {
            if decl.file.as_deref() != Some(old.as_str()) {
                let _ = std::fs::remove_file(self.root.join(old));
            }
        }
        Ok(decl)
    }

    /// Save `flow.json`: parsed with `oas_flow::Flow::from_json` (so the whole
    /// DSL load-time validation applies) and cross-checked against declared
    /// assets before anything is written.
    pub fn save_flow(&self, flow_json: &str) -> Result<(), String> {
        let af = self.load_assets_file()?;
        let mut ids: BTreeSet<String> = af.items.iter().map(|i| i.id.clone()).collect();
        ids.extend(af.collections.into_iter().map(|c| c.id));
        self.save_flow_with_ids(flow_json, &ids)
    }

    /// v1.2.1: save a task flow validated against an **external** asset id set
    /// (typically the global asset store). `save_flow` remains the task-local
    /// compatibility path.
    pub fn save_flow_with_ids(
        &self,
        flow_json: &str,
        ids: &BTreeSet<String>,
    ) -> Result<(), String> {
        let flow =
            oas_flow::Flow::from_json(flow_json).map_err(|e| format!("flow.json rejected: {e}"))?;
        if let Err(errs) = flow.validate_assets(ids) {
            let joined = errs
                .iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join("; ");
            return Err(format!("flow.json: {joined}; nothing written"));
        }
        if !self.assets_only {
            self.ensure_meta()?;
        }
        let mut text = flow_json.trim_end().to_string();
        text.push('\n');
        write_atomic(&self.flow_path(), text.as_bytes())
    }

    /// Full bundle validation (N5 CLI). `flow.json` is optional; a present one
    /// must parse *and* only reference declared assets.
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errs = Vec::new();

        let meta_path = self.meta_path();
        match std::fs::read_to_string(&meta_path) {
            Ok(text) => match serde_json::from_str::<serde_json::Value>(&text) {
                Ok(v) => {
                    if v.get("schema_version").and_then(|x| x.as_u64())
                        != Some(SCHEMA_VERSION as u64)
                    {
                        errs.push(format!(
                            "meta.json: schema_version must be {SCHEMA_VERSION}"
                        ));
                    }
                    if v.get("name").and_then(|x| x.as_str()).is_none() {
                        errs.push("meta.json: missing string field `name`".to_string());
                    }
                    if v.get("expire").and_then(|x| x.as_str()).is_none() {
                        errs.push("meta.json: missing string field `expire`".to_string());
                    }
                }
                Err(e) => errs.push(format!("meta.json: invalid JSON: {e}")),
            },
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                errs.push(format!("meta.json: missing ({})", meta_path.display()))
            }
            Err(e) => errs.push(format!("meta.json: read failed: {e}")),
        }

        let af = match self.load_assets_file() {
            Ok(af) => Some(af),
            Err(e) => {
                errs.push(e);
                None
            }
        };

        match self.load_flow() {
            Ok(Some(flow)) => {
                if let Some(af) = af.as_ref() {
                    let ids: BTreeSet<String> = af.items.iter().map(|i| i.id.clone()).collect();
                    if let Err(refs) = flow.validate_assets(&ids) {
                        for e in refs {
                            errs.push(format!("flow.json: {e}"));
                        }
                    }
                }
            }
            Ok(None) => {}
            Err(e) => errs.push(e),
        }

        if errs.is_empty() {
            Ok(())
        } else {
            Err(errs)
        }
    }

    /// Load the typed M0 `meta.json`. A missing file is `Ok(None)` (callers that
    /// only need the flow stay tolerant); malformed JSON is a loud error.
    pub fn load_meta(&self) -> Result<Option<Meta>, String> {
        let path = self.meta_path();
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(format!("read {}: {e}", path.display())),
        };
        Meta::from_json(&text)
            .map(Some)
            .map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Lint the unified task-template contract (ENGINE §22.34.2, M0).
    ///
    /// Rules (every error carries a path, see [`LintIssue`]):
    /// * `META` — `meta.json` contract (exit result classes, params, …).
    /// * `T1`   — every path ends in `finish(<exit>)`, and every exit is declared
    ///            in `meta.exits`.
    /// * `T3`   — every `param('key')` is declared in `meta.params`.
    /// * `T4`   — every asset selector resolves to a declared asset (or `lib:`).
    /// * `T5`   — every declared asset is well-formed and its image exists.
    /// * `T6`   — the flow is DSL-only (no external-code keys).
    /// * `T7`   — only in [`Self::lint_with_calls`] (requires the tasks dir):
    ///            every `call_task` targets a readable sibling task and maps
    ///            only parameters both sides declare (Runner Studio §C.4.5).
    ///
    /// T2 (bounded loops) is enforced by the runtime step budget; `while` has
    /// only a condition (like a normal language), `loop` owns the numeric count.
    /// `flow.json` is optional (old tasks may not have one yet).
    pub fn lint(&self) -> Result<(), Vec<LintIssue>> {
        let mut issues = Vec::new();
        self.lint_into(&mut issues);
        if issues.is_empty() {
            Ok(())
        } else {
            Err(issues)
        }
    }

    /// Accumulate lint issues into `issues` (used by [`Self::lint`] and by
    /// `--all`, so one broken task cannot abort the whole report).
    pub fn lint_into(&self, issues: &mut Vec<LintIssue>) {
        self.lint_into_full(issues, None, None, None);
    }

    /// v1.2.1: lint a task whose flow may reference a **global** asset id set.
    pub fn lint_into_with_assets(
        &self,
        issues: &mut Vec<LintIssue>,
        external_assets: Option<&BTreeSet<String>>,
    ) {
        self.lint_into_full(issues, external_assets, None, None);
    }

    /// Lint one bundle **with** the cross-task `call_task` rule (T7).
    ///
    /// `tasks_dir` is required to resolve sibling task bundles; without it the
    /// self-contained rules still run ([`Self::lint`]), but T7 is skipped.
    pub fn lint_with_calls(&self, tasks_dir: &Path) -> Result<(), Vec<LintIssue>> {
        let mut issues = Vec::new();
        self.lint_into_with_calls(&mut issues, tasks_dir);
        if issues.is_empty() {
            Ok(())
        } else {
            Err(issues)
        }
    }

    /// [`Self::lint_into`] + T7 cross-task `call_task` checks.
    pub fn lint_into_with_calls(&self, issues: &mut Vec<LintIssue>, tasks_dir: &Path) {
        self.lint_into_full(issues, None, Some(tasks_dir), None);
    }

    /// T7 + the v1.2.1 global-asset override (used by the `oasd task lint` CLI).
    pub fn lint_into_with_calls_and_assets(
        &self,
        issues: &mut Vec<LintIssue>,
        tasks_dir: &Path,
        external_assets: Option<&BTreeSet<String>>,
    ) {
        self.lint_into_full(issues, external_assets, Some(tasks_dir), None);
    }

    /// seq129-fix: T7 + the global asset store's **full declarations**.
    ///
    /// The old `*_and_assets` entry point only receives the global *id set*,
    /// which is enough for T4 but not for asset-level rules: in global-asset mode
    /// a task's own `assets.json` may be empty while its flow (and `meta.variables`)
    /// reference a **global** `ocr_var`, so T8 would report a false
    /// "not a declared asset". This entry point takes the parsed global
    /// [`AssetsFile`] and merges it into the asset-level checks.
    ///
    /// Merge rule: task-local declarations win on an id clash (a task-local asset
    /// overrides the global one); otherwise global items/collections are appended.
    /// T5 keeps validating only the task-local file (global store validity is the
    /// caller's load error), and T4 accepts the union of local + global ids when
    /// `external_assets` is `None`.
    pub fn lint_into_with_calls_and_global_assets(
        &self,
        issues: &mut Vec<LintIssue>,
        tasks_dir: &Path,
        external_assets: Option<&BTreeSet<String>>,
        global_assets: Option<&AssetsFile>,
    ) {
        self.lint_into_full(issues, external_assets, Some(tasks_dir), global_assets);
    }

    /// Convenience form of [`Self::lint_into_with_calls_and_global_assets`].
    pub fn lint_with_calls_and_global_assets(
        &self,
        tasks_dir: &Path,
        global_assets: Option<&AssetsFile>,
    ) -> Result<(), Vec<LintIssue>> {
        let mut issues = Vec::new();
        self.lint_into_with_calls_and_global_assets(&mut issues, tasks_dir, None, global_assets);
        if issues.is_empty() {
            Ok(())
        } else {
            Err(issues)
        }
    }

    /// **T10 warnings** (V2.0, non-blocking; docs/VARIABLES-V2-DESIGN.md §2.2).
    ///
    /// Legacy `meta.params` rows are accepted for reading old tasks/packages, but
    /// new code reads `meta.variables`. This returns the advisory issues only
    /// (never part of [`Self::lint`], which is the blocking error channel): the
    /// caller (serve/CLI, V2.0b) may surface them as warnings.
    pub fn lint_warnings(&self) -> Vec<LintIssue> {
        match std::fs::read_to_string(self.meta_path()) {
            Ok(text) => match Meta::from_json(&text) {
                Ok(meta) => meta.legacy_params_warnings(),
                Err(_) => Vec::new(),
            },
            Err(_) => Vec::new(),
        }
    }

    fn lint_into_full(
        &self,
        issues: &mut Vec<LintIssue>,
        external_assets: Option<&BTreeSet<String>>,
        tasks_dir: Option<&Path>,
        global_assets: Option<&AssetsFile>,
    ) {
        // ---- meta.json ------------------------------------------------------
        let meta = match std::fs::read_to_string(self.meta_path()) {
            Ok(text) => match Meta::from_json(&text) {
                Ok(m) => {
                    issues.extend(m.lint_issues());
                    Some(m)
                }
                Err(e) => {
                    issues.push(LintIssue::new("META", "meta.json", e));
                    None
                }
            },
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                issues.push(LintIssue::new(
                    "META",
                    "meta.json",
                    format!("missing ({})", self.meta_path().display()),
                ));
                None
            }
            Err(e) => {
                issues.push(LintIssue::new(
                    "META",
                    "meta.json",
                    format!("read failed: {e}"),
                ));
                None
            }
        };

        // ---- assets.json (T5) ----------------------------------------------
        let assets = self.lint_assets(issues);
        // seq129-fix: global-mode tasks may reference assets from the global
        // store while their own `assets.json` is empty. Asset-level rules (T8)
        // need the merged view; T5 above stays local-only. Local declarations win
        // on an id clash (task-local overrides global); see the merge helper.
        let merged_assets: Option<AssetsFile> = match global_assets {
            Some(global) => Some(merge_global_assets_for_lint(assets.as_ref(), global)),
            None => assets.clone(),
        };

        // ---- flow.json (T1/T3/T4/T6) ---------------------------------------
        let flow_text = match std::fs::read_to_string(self.flow_path()) {
            Ok(t) => Some(t),
            Err(e) if e.kind() == io::ErrorKind::NotFound => None, // optional
            Err(e) => {
                issues.push(LintIssue::new(
                    "FLOW",
                    "flow.json",
                    format!("read failed: {e}"),
                ));
                None
            }
        };
        let Some(text) = flow_text else { return };
        // T6: the DSL has no external-code construct by construction; still scan
        // for hand-added keys (e.g. "code"/"script") BEFORE parsing so the report
        // is about the right rule.
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
            scan_forbidden_keys(&value, "$", issues);
        }
        let flow = match oas_flow::Flow::from_json(&text) {
            Ok(f) => f,
            Err(e) => {
                issues.push(LintIssue::new(
                    "FLOW",
                    format!("flow.json:{}", e.path),
                    e.msg,
                ));
                return;
            }
        };

        // T1: every path must end with finish(...), and each exit must be declared.
        if !flow.ends_with_finish() {
            issues.push(LintIssue::new(
                "T1",
                "flow.json:steps",
                "flow must end with an explicit finish(<exit>) on every path",
            ));
        }
        if let Some(meta) = &meta {
            let declared = meta.declared_exit_names();
            let listed = join_or_none(&declared);
            for use_site in flow.finish_uses() {
                if !declared.contains(&use_site.exit) {
                    issues.push(LintIssue::new(
                        "T1",
                        format!("flow.json:{}", use_site.path),
                        format!(
                            "finish exit `{}` is not declared in meta.exits \
                             (declared: {listed})",
                            use_site.exit
                        ),
                    ));
                }
            }
            // T3: every param('key') must be declared in meta.variables (V2.0:
            // `meta.params` no longer counts; T10 reports legacy rows separately).
            let params = meta.declared_input_variable_keys();
            let listed_params = join_or_none(&params);
            for site in flow.param_uses_at() {
                match site.key {
                    None => issues.push(LintIssue::new(
                        "T3",
                        format!("flow.json:{}", site.path),
                        "param() requires exactly one string-literal key \
                         (a computed key cannot be linted)"
                            .to_string(),
                    )),
                    Some(key) if !params.contains(&key) => {
                        issues.push(LintIssue::new(
                            "T3",
                            format!("flow.json:{}", site.path),
                            format!(
                                "param(`{key}`) is used but not declared in meta.variables \
                                 (declared: {listed_params})"
                            ),
                        ))
                    }
                    _ => {}
                }
            }
        }

        // T4: asset references must resolve (or be `lib:` selectors).
        // seq129-fix: in global mode the accepted id set is the union of the
        // task-local and global stores (items + collections); `lib:` stays
        // exempt inside `Flow::validate_assets`.
        let t4_ids: Option<BTreeSet<String>> = match external_assets {
            Some(ids) => Some(ids.clone()),
            None => match (&global_assets, &merged_assets) {
                (Some(_), Some(merged)) => Some(merged_asset_ids(merged)),
                _ => assets
                    .as_ref()
                    .map(|af| af.items.iter().map(|i| i.id.clone()).collect()),
            },
        };
        if let Some(ids) = t4_ids {
            if let Err(errs) = flow.validate_assets(&ids) {
                for e in errs {
                    issues.push(LintIssue::new(
                        "T4",
                        format!("flow.json:{}", e.path),
                        e.msg,
                    ));
                }
            }
        }

        // T7: `call_task` must reference a readable sibling task and only map
        // parameters both sides declare (§C.4.5). Requires the tasks directory,
        // so it is skipped by the self-contained `TaskBundle::lint()`.
        if let Some(tasks_dir) = tasks_dir {
            lint_call_tasks(&flow, meta.as_ref(), tasks_dir, issues);
        }

        // T8 (V2.0): every `value_source=ocr_text` variable must match its bound
        // ocr_var asset + the flow's reference/param-usage set. Lead ruling 4: an
        // **error** (publish blocking); the save-triggered sync heals it, lint
        // never rewrites. seq129-fix: the asset lookup uses the **merged**
        // local+global view, so a global ocr_var no longer reports a false T8.
        if let (Some(meta), Some(assets)) = (&meta, merged_assets.as_ref()) {
            issues.extend(crate::ocr_var::derived_variable_issues(meta, assets, &flow));
        }
    }

    /// T5: parse + shape-check `assets.json`. Returns the parsed file even when
    /// shape issues exist (the flow cross-check still has ids to work with).
    fn lint_assets(&self, issues: &mut Vec<LintIssue>) -> Option<AssetsFile> {
        let text = match std::fs::read_to_string(self.assets_path()) {
            Ok(t) => t,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Some(AssetsFile::default()),
            Err(e) => {
                issues.push(LintIssue::new(
                    "T5",
                    "assets.json",
                    format!("read failed: {e}"),
                ));
                return None;
            }
        };
        let af = match serde_json::from_str::<AssetsFile>(&text) {
            Ok(af) => af,
            Err(e) => {
                issues.push(LintIssue::new(
                    "T5",
                    "assets.json",
                    format!("invalid JSON: {e}"),
                ));
                return None;
            }
        };
        for (p, msg) in self.assets_shape_issues(&af) {
            issues.push(LintIssue::new("T5", p, msg));
        }
        Some(af)
    }

    // -- internals ---------------------------------------------------------

    /// Persist `assets.json` (pretty JSON + atomic write).
    ///
    /// Public since the 2026-11-06 参数→素材 migration: the sync/migration
    /// helpers in [`crate::ocr_var`] are pure (`&mut AssetsFile`), and the HTTP
    /// layer owns the backup/rollback around this write.
    pub fn write_assets_file(&self, af: &AssetsFile) -> Result<(), String> {
        let text = serde_json::to_string_pretty(af)
            .map_err(|e| format!("serialize assets.json: {e}"))?;
        write_atomic(&self.assets_path(), format!("{text}\n").as_bytes())
    }

    /// Persist `meta.json` **with a `meta.json.bak` backup** of the current
    /// file (atomic write; the backup is best-effort only when a file exists).
    ///
    /// This is the only writer used by the derived-parameter sync
    /// ([`crate::ocr_var::sync_ocr_var_params`]) and the migration API, so a
    /// failed save can always be rolled back from the backup.
    pub fn write_meta(&self, meta: &Meta) -> Result<(), String> {
        let path = self.meta_path();
        if path.is_file() {
            let bak = path.with_extension("json.bak");
            std::fs::copy(&path, &bak)
                .map_err(|e| format!("backup {}: {e}", bak.display()))?;
        }
        let text = serde_json::to_string_pretty(meta)
            .map_err(|e| format!("serialize meta.json: {e}"))?;
        write_atomic(&path, format!("{text}\n").as_bytes())
    }

    /// Create `meta.json` on the first save (name from the directory name).
    fn ensure_meta(&self) -> Result<(), String> {
        let path = self.meta_path();
        if path.is_file() {
            return Ok(());
        }
        let name = self
            .root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("task");
        let meta = serde_json::json!({
            "schema_version": SCHEMA_VERSION,
            "name": name,
            "expire": "daily",
        });
        let text = serde_json::to_string_pretty(&meta)
            .map_err(|e| format!("serialize meta.json: {e}"))?;
        write_atomic(&path, format!("{text}\n").as_bytes())
    }

    /// Next free `images/aNNN.png` (scans what is on disk, so an orphan never
    /// gets overwritten). The asset id is never involved.
    fn next_image_rel_path(&self) -> Result<String, String> {
        let dir = self.images_dir();
        let mut max = 0u32;
        match std::fs::read_dir(&dir) {
            Ok(entries) => {
                for entry in entries {
                    let entry = entry.map_err(|e| format!("read {}: {e}", dir.display()))?;
                    let name = entry.file_name().to_string_lossy().to_string();
                    if let Some(n) = parse_image_seq(&name) {
                        max = max.max(n);
                    }
                }
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("read {}: {e}", dir.display())),
        }
        Ok(format!("images/a{:03}.png", max.saturating_add(1)))
    }

    fn assets_shape_errors(&self, af: &AssetsFile) -> Vec<String> {
        self.assets_shape_issues(af)
            .into_iter()
            .map(|(path, msg)| format!("{path}: {msg}"))
            .collect()
    }

    /// Structured form of [`Self::assets_shape_errors`]: `(path, message)` pairs
    /// so the lint can carry a precise `assets.json: items[i].field` path.
    fn assets_shape_issues(&self, af: &AssetsFile) -> Vec<(String, String)> {
        let mut errs: Vec<(String, String)> = Vec::new();
        if af.schema_version != SCHEMA_VERSION {
            errs.push((
                "assets.json: schema_version".to_string(),
                format!(
                    "unsupported schema_version {} (expected {SCHEMA_VERSION})",
                    af.schema_version
                ),
            ));
        }
        if let Some([w, h]) = af.design_resolution {
            if w == 0 || h == 0 {
                errs.push((
                    "assets.json: design_resolution".to_string(),
                    "must be > 0".to_string(),
                ));
            }
        }
        let shape_bounds = af.design_resolution.map(|[w, h]| (w, h));
        let mut seen = BTreeSet::new();
        for (i, item) in af.items.iter().enumerate() {
            let p = format!("assets.json: items[{i}]");
            if let Err(e) = validate_id(&item.id) {
                errs.push((format!("{p}.id"), e));
            }
            if !seen.insert(item.id.clone()) {
                errs.push((format!("{p}.id"), format!("duplicate asset id `{}`", item.id)));
            }
            if !KINDS.contains(&item.kind.as_str()) {
                errs.push((
                    format!("{p}.kind"),
                    format!(
                        "unknown kind `{}` (allowed: {})",
                        item.kind,
                        KINDS.join(", ")
                    ),
                ));
            }
            if let Err(e) = validate_threshold(item.threshold) {
                errs.push((format!("{p}.threshold"), e));
            }
            // v1.2.1 OCR provenance (`ocr_mode`/`ocr_source_roi`) belongs to
            // `ocr_line`; 2026-11-06 批次B (#3): `ocr_text` is also valid on a
            // plain `ocr_lines` asset (expected text used as a chain gate).
            if item.ocr_mode.is_some() || item.ocr_source_roi.is_some() {
                if item.kind != "ocr_line" {
                    errs.push((
                        format!("{p}.ocr_mode"),
                        "ocr_mode/ocr_source_roi are only valid for ocr_line".to_string(),
                    ));
                }
                match item.ocr_mode.as_deref().unwrap_or("single") {
                    "single" => {
                        if item.ocr_source_roi.is_some() {
                            errs.push((
                                format!("{p}.ocr_source_roi"),
                                "ocr_mode=single must not carry ocr_source_roi".to_string(),
                            ));
                        }
                    }
                    "lines" => {
                        // v1.2.3: recognition ROI is the single `roi`; the old
                        // `ocr_source_roi` is migrated on load and no longer
                        // required for new saves.
                        if item.ocr_text.as_deref().unwrap_or("").trim().is_empty() {
                            errs.push((
                                format!("{p}.ocr_text"),
                                "ocr_mode=lines requires non-empty ocr_text".to_string(),
                            ));
                        }
                    }
                    other => errs.push((
                        format!("{p}.ocr_mode"),
                        format!("unknown ocr_mode `{other}` (allowed: single, lines)"),
                    )),
                }
            }
            if let Some(text) = item.ocr_text.as_deref() {
                match item.kind.as_str() {
                    "ocr_line" => {
                        if item.ocr_mode.as_deref() != Some("lines") {
                            errs.push((
                                format!("{p}.ocr_text"),
                                "ocr_line with ocr_text must use ocr_mode=lines".to_string(),
                            ));
                        }
                    }
                    "ocr_lines" => {
                        if text.trim().is_empty() {
                            errs.push((
                                format!("{p}.ocr_text"),
                                "ocr_lines ocr_text must not be blank".to_string(),
                            ));
                        }
                    }
                    other => errs.push((
                        format!("{p}.ocr_text"),
                        format!(
                            "ocr_text is only valid for ocr_line/ocr_lines assets (got kind `{other}`)"
                        ),
                    )),
                }
            }
            // 2026-11-06 参数→素材: `ocr_var` field contract (runtime_text /
            // param_key / label). Those fields are ocr_var-only; `op_region` is
            // valid on every recognizable kind (a `region` must not carry one —
            // checked by `op_region_issues` below).
            if item.kind == "ocr_var" {
                if let Err(e) = validate_ocr_var_fields(item, &p) {
                    errs.push((p.clone(), e));
                }
            } else {
                for (field, present) in [
                    ("runtime_text", item.runtime_text.is_some()),
                    ("param_key", item.param_key.is_some()),
                    ("label", item.label.is_some()),
                ] {
                    if present {
                        errs.push((
                            format!("{p}.{field}"),
                            format!("{field} is only valid for kind=ocr_var"),
                        ));
                    }
                }
            }
            // 2026-11-06 事项6: `action_source` is a deprecated compatibility
            // field — old values are tolerated and it is never validated or
            // written again. A shape is optional and always allowed.
            if let Some(shape) = item.action_shape.as_ref() {
                if let Err(e) =
                    validate_action_shape(shape, &format!("{p}.action_shape"), shape_bounds)
                {
                    errs.push((format!("{p}.action_shape"), e));
                }
            }
            match item.roi {
                Some([x, y, w, h]) => {
                    if x < 0 || y < 0 || w <= 0 || h <= 0 {
                        errs.push((
                            format!("{p}.roi"),
                            format!(
                                "[x,y,w,h] = {x},{y},{w},{h} must have x,y >= 0 and w,h > 0"
                            ),
                        ));
                    }
                }
                None => {
                    if item.kind == "region" {
                        errs.push((
                            format!("{p}.roi"),
                            "region object requires [x,y,w,h]".to_string(),
                        ));
                    }
                    // template without ROI is allowed in the GLOBAL store: old
                    // library templates were authored without an ROI and use
                    // full-frame search at runtime.
                }
            }
            match item.file.as_deref() {
                Some(file) => match validate_image_rel_path(file) {
                    Ok(()) => {
                        if !self.root.join(file).is_file() {
                            errs.push((
                                format!("{p}.file"),
                                format!("`{file}` does not exist"),
                            ));
                        }
                        if item.kind != "template" {
                            errs.push((
                                format!("{p}.file"),
                                format!("{} objects must not carry a file", item.kind),
                            ));
                        }
                    }
                    Err(e) => errs.push((format!("{p}.file"), e)),
                },
                None => {
                    if item.kind == "template" {
                        errs.push((
                            format!("{p}.file"),
                            "template asset requires an image file".to_string(),
                        ));
                    }
                }
            }
            if item.kind == "region" && item.threshold.is_some() {
                errs.push((
                    format!("{p}.threshold"),
                    "region objects must not carry a threshold".to_string(),
                ));
            }
        }
        // 2026-11-06 参数→素材 D2: `op_region` chains are validated as one graph
        // (node existence/kind, terminal mode, cycle + depth bound).
        errs.extend(op_region_issues(af));
        // `param_key` is unique per task (it is the `values.json` key of the
        // derived parameter row).
        let mut ocr_var_keys: BTreeMap<String, String> = BTreeMap::new();
        for item in af.items.iter().filter(|i| i.kind == "ocr_var") {
            let Some(key) = item.param_key.as_deref() else {
                continue;
            };
            if let Some(prev) = ocr_var_keys.insert(key.to_string(), item.id.clone()) {
                errs.push((
                    format!("assets.json: ocr_var `{}`.param_key", item.id),
                    format!("`{key}` is already used by ocr_var `{prev}`"),
                ));
            }
        }
        // B-10: collection-level operation geometry must obey the same contract
        // as per-asset geometry (otherwise it is saved but unusable at runtime).
        // 2026-11-06 事项6: no two-state check — the shape is optional and any
        // deprecated `action_source` value is tolerated.
        for (ci, coll) in af.collections.iter().enumerate() {
            let p = format!("assets.json: collections[{ci}]");
            if coll.members.is_empty() {
                errs.push((format!("{p}.members"), "集合至少需要一个成员".to_string()));
            }
            for (mi, m) in coll.members.iter().enumerate() {
                if af.collections.iter().any(|c| &c.id == m) {
                    errs.push((
                        format!("{p}.members[{mi}]"),
                        format!("`{m}` 是另一个集合；素材集合不支持嵌套集合"),
                    ));
                } else if !af.items.iter().any(|a| &a.id == m) {
                    errs.push((
                        format!("{p}.members[{mi}]"),
                        format!("成员 `{m}` 不存在于 assets.json items"),
                    ));
                }
            }
            if let Some(shape) = coll.action_shape.as_ref() {
                if let Err(e) =
                    validate_action_shape(shape, &format!("{p}.action_shape"), shape_bounds)
                {
                    errs.push((format!("{p}.action_shape"), e));
                }
            }
        }
        errs
    }
}

fn parse_image_seq(name: &str) -> Option<u32> {
    let stem = name.strip_prefix('a')?.strip_suffix(".png")?;
    if stem.is_empty() || !stem.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    stem.parse().ok()
}

/// List all valid task bundles under `tasks_dir` (sorted). A directory counts
/// as a bundle when it contains `meta.json`.
pub fn list_tasks(tasks_dir: &Path) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    match std::fs::read_dir(tasks_dir) {
        Ok(entries) => {
            for entry in entries {
                let entry = entry.map_err(|e| format!("read {}: {e}", tasks_dir.display()))?;
                let path = entry.path();
                if path.is_dir() && path.join("meta.json").is_file() {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        out.push(name.to_string());
                    }
                }
            }
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(out),
        Err(e) => return Err(format!("read {}: {e}", tasks_dir.display())),
    }
    out.sort();
    Ok(out)
}

/// T6: keys that would smuggle executable/foreign code into a flow. The DSL
/// parser already rejects unknown instructions; this catches hand-added args
/// that look like an external-code escape hatch.
const FORBIDDEN_FLOW_KEYS: [&str; 9] = [
    "code",
    "script",
    "eval",
    "exec",
    "python",
    "shell",
    "cmd",
    "powershell",
    "unsafe",
];

/// P1-1: parameter **names** are user data (`call_task.params.code` is a normal
/// mapping for a subtask parameter named `code`), so T6 must not treat keys
/// inside `args.params`/`args.defaults` as DSL keys. Values of `args.params` are
/// still evaluated expressions, so a blatant external-code escape such as
/// `"shell: rm -rf"` remains a T6 violation.
const CODE_ESCAPE_PREFIXES: [&str; 9] = [
    "code:",
    "script:",
    "eval:",
    "exec:",
    "python:",
    "shell:",
    "cmd:",
    "powershell:",
    "unsafe:",
];

fn looks_like_external_code_expression(text: &str) -> bool {
    let lowered = text.trim().to_ascii_lowercase();
    CODE_ESCAPE_PREFIXES.iter().any(|p| lowered.starts_with(p))
}

fn scan_forbidden_keys(
    value: &serde_json::Value,
    path: &str,
    issues: &mut Vec<LintIssue>,
) {
    match value {
        serde_json::Value::Object(map) => {
            // The maps under `args.params` / `args.defaults` are user-data maps:
            // their keys are parameter names, not DSL structure (P1-1).
            let is_param_map = path.ends_with(".args.params");
            let is_user_data_map =
                is_param_map || path.ends_with(".args.defaults");
            for (k, v) in map {
                let child = format!("{path}.{k}");
                if !is_user_data_map && FORBIDDEN_FLOW_KEYS.contains(&k.as_str()) {
                    issues.push(LintIssue::new(
                        "T6",
                        format!("flow.json:{child}"),
                        format!(
                            "key `{k}` is not part of the flow DSL: external code is \
                             not allowed (T6)"
                        ),
                    ));
                }
                if is_param_map {
                    if let Some(text) = v.as_str() {
                        if looks_like_external_code_expression(text) {
                            issues.push(LintIssue::new(
                                "T6",
                                format!("flow.json:{child}"),
                                format!(
                                    "parameter expression `{text}` looks like an \
                                     external-code escape; the flow DSL cannot execute \
                                     code (T6)"
                                ),
                            ));
                        }
                    }
                }
                scan_forbidden_keys(v, &child, issues);
            }
        }
        serde_json::Value::Array(items) => {
            for (i, v) in items.iter().enumerate() {
                scan_forbidden_keys(v, &format!("{path}[{i}]"), issues);
            }
        }
        _ => {}
    }
}

fn join_or_none(set: &BTreeSet<String>) -> String {
    if set.is_empty() {
        "(none)".to_string()
    } else {
        set.iter().cloned().collect::<Vec<_>>().join(", ")
    }
}

/// Every id a flow may reference from one [`AssetsFile`] (items + collections).
fn merged_asset_ids(af: &AssetsFile) -> BTreeSet<String> {
    let mut ids: BTreeSet<String> = af.items.iter().map(|i| i.id.clone()).collect();
    ids.extend(af.collections.iter().map(|c| c.id.clone()));
    ids
}

/// seq129-fix: merge one task's local [`AssetsFile`] over the global store's.
///
/// Merge rule: **task-local declarations win on an id clash** (a task-local asset
/// or collection overrides the global one with the same id); otherwise the global
/// store is the base and the local declarations are appended. Used only for
/// asset-level lint (T8/T4 known-id) — a local `assets.json` is never rewritten.
///
/// Public so the apps layer (install-time lint, etc.) can build the same merged
/// view for its own lint call site.
pub fn merge_global_assets_for_lint(
    local: Option<&AssetsFile>,
    global: &AssetsFile,
) -> AssetsFile {
    let mut merged = global.clone();
    let Some(local) = local else {
        return merged;
    };
    let local_item_ids: BTreeSet<&str> = local.items.iter().map(|i| i.id.as_str()).collect();
    merged
        .items
        .retain(|item| !local_item_ids.contains(item.id.as_str()));
    merged.items.extend(local.items.iter().cloned());
    let local_collection_ids: BTreeSet<&str> =
        local.collections.iter().map(|c| c.id.as_str()).collect();
    merged
        .collections
        .retain(|c| !local_collection_ids.contains(c.id.as_str()));
    merged.collections.extend(local.collections.iter().cloned());
    if merged.design_resolution.is_none() {
        merged.design_resolution = local.design_resolution;
    }
    merged
}

/// T7 (Runner Studio split §C.4.5): validate every `call_task` in the
/// **unexpanded** flow.
///
/// * the target task must exist and its `meta.json` must be readable and
///   parseable; its `flow.json` must be readable **and parse as a flow**
///   (missing vs. read-failure vs. parse-failure are distinguished);
/// * every mapping key must be declared by the target (`meta.params[].key`);
/// * mapping values may be constants (no `param(...)` call: no parent key to
///   check) or exactly `param('parentKey')`, whose parent key must be declared
///   by THIS task; computed `param(...)` keys are rejected because T7 cannot
///   verify their parent key statically (T3 stays untouched: `call_task` is a
///   leaf there, which is why mapping errors get the precise T7 message);
/// * `inherit` is already validated by the flow parser (`all`/`none` only).
///
/// Nested calls are not walked here: `oasd task lint --all` lints the target's
/// own flow, so a chain is covered by one issue per offending edge (and cycles
/// stay the loader's job, §C.4.3).
fn lint_call_tasks(
    flow: &oas_flow::Flow,
    parent_meta: Option<&Meta>,
    tasks_dir: &Path,
    issues: &mut Vec<LintIssue>,
) {
    for use_site in flow.call_task_uses() {
        let base = format!("flow.json:{}", use_site.path);
        if let Err(e) = validate_task_name(&use_site.task) {
            issues.push(LintIssue::new(
                "T7",
                base,
                format!("call_task `{}` 名称非法: {e}", use_site.task),
            ));
            continue;
        }
        let dir = tasks_dir.join(&use_site.task);
        if !dir.is_dir() {
            issues.push(LintIssue::new(
                "T7",
                base,
                format!(
                    "call_task `{}` 不存在: {} 不是任务目录",
                    use_site.task,
                    dir.display()
                ),
            ));
            continue;
        }

        // Target meta.json: the parameter schema.
        let target_meta_path = dir.join("meta.json");
        let target_meta = match std::fs::read_to_string(&target_meta_path) {
            Ok(text) => match Meta::from_json(&text) {
                Ok(meta) => Some(meta),
                Err(e) => {
                    issues.push(LintIssue::new(
                        "T7",
                        base.clone(),
                        format!("call_task `{}`: 无法解析目标 meta.json: {e}", use_site.task),
                    ));
                    None
                }
            },
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                issues.push(LintIssue::new(
                    "T7",
                    base.clone(),
                    format!(
                        "call_task `{}` 不存在或不是完整任务: 缺少 meta.json",
                        use_site.task
                    ),
                ));
                None
            }
            Err(e) => {
                issues.push(LintIssue::new(
                    "T7",
                    base.clone(),
                    format!(
                        "call_task `{}`: 无法读取目标 {}: {e}",
                        use_site.task,
                        target_meta_path.display()
                    ),
                ));
                None
            }
        };

        // Target flow.json: a task without one is not runnable, and since P2 a
        // target that exists but does not parse is reported here instead of
        // surfacing only at run/load time (with a target-less path).
        let target_flow_path = dir.join("flow.json");
        match std::fs::read_to_string(&target_flow_path) {
            Ok(text) => {
                if let Err(e) = oas_flow::Flow::from_json(&text) {
                    issues.push(LintIssue::new(
                        "T7",
                        base.clone(),
                        format!(
                            "call_task `{}`: 无法解析目标 flow.json: {e}",
                            use_site.task
                        ),
                    ));
                }
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => issues.push(LintIssue::new(
                "T7",
                base.clone(),
                format!(
                    "call_task `{}` 不存在或不是完整任务: 缺少 flow.json",
                    use_site.task
                ),
            )),
            Err(e) => issues.push(LintIssue::new(
                "T7",
                base.clone(),
                format!(
                    "call_task `{}`: 无法读取目标 {}: {e}",
                    use_site.task,
                    target_flow_path.display()
                ),
            )),
        }

        let target_keys = target_meta
            .as_ref()
            .map(Meta::declared_input_variable_keys);
        let parent_keys = parent_meta.map(Meta::declared_input_variable_keys);
        for (child, parent_ref) in &use_site.params {
            let param_path = format!("{base}.params.{child}");
            if let Some(target_keys) = &target_keys {
                if !target_keys.contains(child) {
                    issues.push(LintIssue::new(
                        "T7",
                        param_path.clone(),
                        format!(
                            "子任务 `{}` 未声明参数 `{child}` (declared: {})",
                            use_site.task,
                            join_or_none(target_keys)
                        ),
                    ));
                }
            }
            // A constant mapping (no `param(...)` call at all) is legal: it needs
            // no parent declaration. A computed `param(...)` key cannot be
            // checked statically, so it is rejected (P2).
            if use_site.computed_params.contains(child) {
                issues.push(LintIssue::new(
                    "T7",
                    param_path,
                    "call_task.params 只支持 param('key') 字面量引用或常量表达式 \
                     (计算表达式无法静态校验父 key)"
                        .to_string(),
                ));
                continue;
            }
            let Some(parent_key) = parent_ref else {
                continue;
            };
            if let Some(parent_keys) = &parent_keys {
                if !parent_keys.contains(parent_key) {
                    issues.push(LintIssue::new(
                        "T7",
                        param_path,
                        format!(
                            "父任务未声明参数 `{parent_key}` (declared: {})",
                            join_or_none(parent_keys)
                        ),
                    ));
                }
            }
        }
    }
}

/// Lint one task bundle `<tasks_dir>/<name>`.
///
/// Setup errors (invalid name / missing directory) are `Err`; contract
/// violations are the returned issues (`Ok(vec![])` = green).
pub fn lint_task(tasks_dir: &Path, name: &str) -> Result<Vec<LintIssue>, String> {
    let bundle = TaskBundle::open_named(tasks_dir, name)?;
    if !bundle.root().is_dir() {
        return Err(format!(
            "task `{name}` not found under {} (expected a directory with meta.json)",
            tasks_dir.display()
        ));
    }
    match bundle.lint_with_calls(tasks_dir) {
        Ok(()) => Ok(Vec::new()),
        Err(issues) => Ok(issues),
    }
}

/// Lint every directory under `tasks_dir` (sorted by name).
///
/// A directory without `meta.json` is reported as a broken bundle instead of
/// being silently skipped: `task lint --all` is the upgrade/verification gate.
/// Hidden directories (`.`-prefixed staging dirs) are ignored.
pub fn lint_tasks(tasks_dir: &Path) -> Result<Vec<(String, Vec<LintIssue>)>, String> {
    let entries = std::fs::read_dir(tasks_dir)
        .map_err(|e| format!("read {}: {e}", tasks_dir.display()))?;
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("read {}: {e}", tasks_dir.display()))?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name.starts_with('.') {
            continue; // staging dirs created by `oasd task new`
        }
        names.push(name.to_string());
    }
    names.sort();
    let mut out = Vec::with_capacity(names.len());
    for name in names {
        // Opened by path: the directory already exists, so a name that is not a
        // valid bundle name is reported by the lint (missing meta), not aborted.
        let bundle = TaskBundle::open(tasks_dir.join(&name))?;
        let issues = match bundle.lint_with_calls(tasks_dir) {
            Ok(()) => Vec::new(),
            Err(issues) => issues,
        };
        out.push((name, issues));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

    /// Self-cleaning temp dir (no `tempfile` dependency).
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "oas-task-test-{}-{tag}-{}",
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

    /// Deterministic non-flat RGBA frame.
    fn frame(w: u32, h: u32) -> Frame {
        let mut rgba = vec![0u8; (w as usize) * (h as usize) * 4];
        for y in 0..h {
            for x in 0..w {
                let i = ((y as usize) * (w as usize) + (x as usize)) * 4;
                rgba[i] = (x * 7) as u8;
                rgba[i + 1] = (y * 11) as u8;
                rgba[i + 2] = ((x + y) * 5) as u8;
                rgba[i + 3] = 255;
            }
        }
        Frame::new(w, h, rgba)
    }

    fn open(td: &TempDir) -> TaskBundle {
        TaskBundle::open(td.path().join("任务1")).expect("open bundle")
    }

    fn flow_json(sel: &str) -> String {
        format!(
            r#"{{"schema_version":1,"name":"任务1","steps":[{{"op":"click","args":{{"sel":"{sel}"}}}},{{"op":"finish","args":{{"exit":"done"}}}}]}}"#
        )
    }

    fn walk_files(dir: &Path, out: &mut Vec<PathBuf>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk_files(&path, out);
                } else {
                    out.push(path);
                }
            }
        }
    }

    #[test]
    fn reorder_assets_persists_items_order_and_rejects_unknown_ids() {
        let td = TempDir::new("reorder");
        let b = open(&td);
        let f = frame(32, 32);
        for id in ["甲", "乙", "丙"] {
            b.save_asset(AssetDecl::template(id, [0, 0, 8, 8]), Some(&f))
                .expect("save");
        }
        b.reorder_assets(&["丙".into(), "甲".into(), "乙".into()])
            .expect("reorder");
        let af = b.load_assets_file().expect("load");
        let ids: Vec<&str> = af.items.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids, vec!["丙", "甲", "乙"]);
        let before = std::fs::read(b.assets_path()).expect("assets");
        let err = b
            .reorder_assets(&["甲".into(), "不存在".into()])
            .expect_err("unknown id");
        assert!(err.contains("does not exist"), "{err}");
        assert_eq!(std::fs::read(b.assets_path()).expect("assets"), before);
    }

    #[test]
    fn collection_accepts_mixed_member_kinds_and_expands_by_kind() {
        // seq132: template + OCR + ocr_var + region members.
        let td = TempDir::new("collection-mixed");
        let b = open(&td);
        let f = frame(32, 32);
        b.save_asset(AssetDecl::template("成员模板", [0, 0, 8, 8]), Some(&f))
            .expect("template member");
        let mut ocr = AssetDecl::ocr_lines("成员OCR", [8, 0, 8, 8]);
        ocr.ocr_text = Some("开始".to_string());
        b.save_asset(ocr, None).expect("ocr_lines member");
        b.save_asset(
            AssetDecl::ocr_var("成员变量", "group", "御魂".to_string(), None),
            None,
        )
        .expect("ocr_var member");
        b.save_asset(AssetDecl::region("成员区域", [16, 16, 8, 8]), None)
            .expect("region member");
        b.save_collection(AssetCollection {
            id: "混合集合".into(),
            members: vec![
                "成员模板".into(),
                "成员OCR".into(),
                "成员变量".into(),
                "成员区域".into(),
            ],
            threshold: Some(0.8),
            action_shape: None,
            action_source: "recognition".into(),
        })
        .expect("mixed-kind collection must save");
        // Nested collections are rejected with an explicit message.
        let err = b
            .save_collection(AssetCollection {
                id: "嵌套集合".into(),
                members: vec!["混合集合".into()],
                threshold: None,
                action_shape: None,
                action_source: "recognition".into(),
            })
            .expect_err("nested collection must be rejected");
        assert!(err.contains("嵌套"), "unexpected: {err}");
        // A member may belong to only one collection.
        let err = b
            .save_collection(AssetCollection {
                id: "第二集合".into(),
                members: vec!["成员模板".into()],
                threshold: None,
                action_shape: None,
                action_source: "recognition".into(),
            })
            .expect_err("double membership must be rejected");
        assert!(err.contains("已经属于另一个集合"), "unexpected: {err}");
        // Lint/publish validation is clean for the mixed collection.
        b.save_flow(
            r#"{"schema_version":1,"name":"任务1","steps":[
                {"op":"finish","args":{"exit":"done"}}
            ]}"#,
        )
        .expect("flow");
        b.lint().expect("mixed collection must lint clean");
        // click_target expansion keeps the collection's member order and emits a
        // locator that fits each member kind.
        b.save_flow(
            r#"{"schema_version":1,"name":"任务1","steps":[
                {"op":"click_target","args":{"find":[{"kind":"template","sel":"混合集合"}]}},
                {"op":"finish","args":{"exit":"done"}}
            ]}"#,
        )
        .expect("flow with collection");
        let flow = b.load_flow().expect("load").expect("flow");
        let oas_flow::Step::ClickTarget { find, .. } = &flow.steps[0] else {
            panic!("expected click_target, got {:?}", flow.steps[0]);
        };
        assert_eq!(find.len(), 4, "collection expands to every member: {find:?}");
        let assets: Vec<&str> = find.iter().filter_map(|l| l.asset()).collect();
        assert_eq!(assets, vec!["成员模板", "成员OCR", "成员变量"]);
        // OCR member → text locator using its saved expected text.
        assert!(
            matches!(
                find[1].text_spec(),
                Some(oas_flow::TextSpec::Literal(text)) if text == "开始"
            ),
            "ocr_line member must expand to its text locator: {:?}",
            find[1]
        );
        // ocr_var member → param expression locator (runtime value wins).
        assert!(
            matches!(find[2].text_spec(), Some(oas_flow::TextSpec::Expr(_))),
            "ocr_var member must expand to the param expression: {:?}",
            find[2]
        );
        // region member → fixed region locator.
        assert!(
            matches!(find[3], oas_flow::Locator::Region { .. }),
            "region member must expand to a region locator: {:?}",
            find[3]
        );
        // Plain OCR member without expected text is a loud load error.
        b.save_asset(AssetDecl::ocr_lines("无期望文字", [0, 16, 8, 8]), None)
            .expect("plain ocr_lines");
        b.save_collection(AssetCollection {
            id: "第二集合".into(),
            members: vec!["无期望文字".into()],
            threshold: None,
            action_shape: None,
            action_source: "recognition".into(),
        })
        .expect("second collection");
        b.save_flow(
            r#"{"schema_version":1,"name":"任务1","steps":[
                {"op":"click_target","args":{"find":[{"kind":"template","sel":"第二集合"}]}},
                {"op":"finish","args":{"exit":"done"}}
            ]}"#,
        )
        .expect("save flow");
        let err = b.load_flow().expect_err("plain OCR member without text");
        assert!(err.contains("没有期望文字"), "unexpected: {err}");
    }

    #[test]
    fn collection_expands_into_member_template_locators() {
        let td = TempDir::new("collection");
        let b = open(&td);
        let f = frame(32, 32);
        b.save_asset(AssetDecl::template("成员甲", [0, 0, 8, 8]), Some(&f))
            .expect("save member a");
        b.save_asset(AssetDecl::template("成员乙", [8, 0, 8, 8]), Some(&f))
            .expect("save member b");
        b.save_collection(AssetCollection {
            id: "集合目标".into(),
            members: vec!["成员甲".into(), "成员乙".into()],
            threshold: Some(0.8),
            action_shape: None,
            action_source: "recognition".into(),
        })
        .expect("save collection");
        b.save_flow(
            r#"{"schema_version":1,"name":"任务1","steps":[
                {"op":"click_target","args":{"find":[{"kind":"template","sel":"集合目标"}]}},
                {"op":"finish","args":{"exit":"done"}}
            ]}"#,
        )
        .expect("save flow");
        let flow = b.load_flow().expect("load").expect("flow");
        let oas_flow::Step::ClickTarget { find, .. } = &flow.steps[0] else {
            panic!("expected click_target, got {:?}", flow.steps[0]);
        };
        assert_eq!(find.len(), 2, "collection expands to all members");
        let chosen: Vec<&str> = find.iter().filter_map(|l| l.asset()).collect();
        assert_eq!(chosen, vec!["成员甲", "成员乙"]);

        let err = b
            .delete_collection("集合目标")
            .expect_err("referenced collection must be blocked");
        assert!(err.contains("仍被"), "unexpected: {err}");

        // Member deletion is allowed while two members remain; the last member
        // of a referenced collection is blocked until the flow reference is gone.
        b.delete_asset("成员乙").expect("delete one member");
        let af = b.load_assets_file().expect("assets");
        assert_eq!(af.collections[0].members, vec!["成员甲"]);
        let err = b.delete_asset("成员甲").expect_err("last referenced member blocked");
        assert!(err.contains("最少") || err.contains("只剩"), "unexpected: {err}");
        b.save_flow(r#"{"schema_version":1,"name":"任务1","steps":[{"op":"finish","args":{"exit":"done"}}]}"#)
            .expect("remove collection reference");
        b.delete_asset("成员甲").expect("delete last member");
        let af = b.load_assets_file().expect("assets");
        assert!(af.collections.is_empty(), "empty unreferenced collection removed");
    }

    #[test]
    fn delete_asset_is_blocked_while_flow_references_it() {
        let td = TempDir::new("delete-ref");
        let b = open(&td);
        b.save_asset(AssetDecl::template("返回按钮", [0, 0, 8, 8]), Some(&frame(32, 32)))
            .expect("save asset");
        b.save_flow(&flow_json("返回按钮")).expect("save flow");
        let err = b.delete_asset("返回按钮").expect_err("referenced asset must be blocked");
        assert!(err.contains("仍被"), "unexpected: {err}");
        b.save_flow(r#"{"schema_version":1,"name":"任务1","steps":[{"op":"finish","args":{"exit":"done"}}]}"#)
            .expect("save flow without reference");
        b.delete_asset("返回按钮").expect("unreferenced asset deletes");
    }

    /// Write a raw bundle (no save validation): loader tests exercise exactly
    /// the hand-authored `call_task` JSON and the target `meta.params`.
    fn raw_task(td: &TempDir, name: &str, flow: &str, meta: Option<&str>) -> TaskBundle {
        let bundle = TaskBundle::open(td.path().join(name)).expect("open raw task");
        std::fs::create_dir_all(bundle.root()).expect("task dir");
        std::fs::write(bundle.flow_path(), flow).expect("write flow");
        if let Some(meta) = meta {
            std::fs::write(bundle.meta_path(), meta).expect("write meta");
        }
        bundle
    }

    fn with_params_of(step: &oas_flow::Step) -> &oas_flow::Step {
        let oas_flow::Step::WithParams { .. } = step else {
            panic!("expected with_params, got {step:?}");
        };
        step
    }

    fn finish_flow(name: &str) -> String {
        format!(
            r#"{{"schema_version":1,"name":"{name}","steps":[
                 {{"op":"log","args":{{"text":"'inside'"}}}},
                 {{"op":"finish","args":{{"exit":"done"}}}}]}}"#
        )
    }

    #[test]
    fn load_flow_inherit_none_blocks_every_declared_target_key() {
        let td = TempDir::new("calltask-none");
        raw_task(
            &td,
            "子任务",
            &finish_flow("子任务"),
            Some(
                r#"{"schema_version":1,"name":"子任务","params":[
                     {"key":"触发文字","type":"string","default":"默认"},
                     {"key":"max_rounds","type":"int","default":3}]}"#,
            ),
        );
        let caller = raw_task(
            &td,
            "调用者",
            r#"{"schema_version":1,"name":"调用者","steps":[
                 {"op":"call_task","args":{"task":"子任务","inherit":"none"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
            None,
        );
        let flow = caller.load_flow().expect("load").expect("flow");
        let oas_flow::Step::WithParams { block, defaults, .. } =
            with_params_of(&flow.steps[0])
        else {
            unreachable!()
        };
        assert_eq!(
            block,
            &vec!["触发文字".to_string(), "max_rounds".to_string()],
            "all declared target keys are blocked; declaration order is kept"
        );
        assert_eq!(defaults.len(), 2, "defaults still apply after blocking");
    }

    #[test]
    fn load_flow_nests_with_params_scopes_for_a_b_c() {
        let td = TempDir::new("calltask-nested");
        raw_task(
            &td,
            "C",
            &finish_flow("C"),
            Some(
                r#"{"schema_version":1,"name":"C","params":[
                     {"key":"c_level","type":"int","default":7}]}"#,
            ),
        );
        raw_task(
            &td,
            "B",
            r#"{"schema_version":1,"name":"B","steps":[
                 {"op":"call_task","args":{"task":"C","params":{"c_level":"param('b_in')"}}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
            Some(
                r#"{"schema_version":1,"name":"B","params":[
                     {"key":"b_in","type":"int","default":1}]}"#,
            ),
        );
        let a = raw_task(
            &td,
            "A",
            r#"{"schema_version":1,"name":"A","steps":[
                 {"op":"call_task","args":{"task":"B","params":{"b_in":"param('a_in')"}}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
            Some(
                r#"{"schema_version":1,"name":"A","params":[
                     {"key":"a_in","type":"int","default":2}]}"#,
            ),
        );

        let (flow, warnings) = a.load_flow_with_warnings().expect("load");
        let flow = flow.expect("flow");
        assert!(warnings.is_empty(), "{warnings:?}");
        let oas_flow::Step::WithParams { label: b_label, body: b_body, .. } =
            with_params_of(&flow.steps[0])
        else {
            unreachable!()
        };
        assert_eq!(b_label.as_deref(), Some("B"));
        assert_eq!(b_body.len(), 1, "B has one call step after expansion");
        let oas_flow::Step::WithParams {
            label: c_label,
            params: c_params,
            defaults: c_defaults,
            body: c_body,
            ..
        } = with_params_of(&b_body[0])
        else {
            unreachable!()
        };
        assert_eq!(c_label.as_deref(), Some("C"));
        assert_eq!(c_params.len(), 1);
        assert_eq!(c_params[0].0, "c_level");
        assert_eq!(c_defaults["c_level"], oas_flow::Value::Num(7.0));
        assert_eq!(c_body.len(), 1, "C's finish is stripped at every level");
        assert!(matches!(c_body[0], oas_flow::Step::Log { .. }));
    }

    #[test]
    fn load_flow_keeps_each_call_sites_mapping_independent() {
        let td = TempDir::new("calltask-sites");
        raw_task(
            &td,
            "子任务",
            &finish_flow("子任务"),
            Some(r#"{"schema_version":1,"name":"子任务","params":[
                 {"key":"p","type":"string","default":"d"}]}"#),
        );
        let caller = raw_task(
            &td,
            "调用者",
            r#"{"schema_version":1,"name":"调用者","steps":[
                 {"op":"call_task","args":{"task":"子任务","params":{"p":"param('one')"}}},
                 {"op":"call_task","args":{"task":"子任务","params":{"p":"param('two')"}}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
            None,
        );
        let flow = caller.load_flow().expect("load").expect("flow");
        let oas_flow::Step::WithParams { params: first, .. } =
            with_params_of(&flow.steps[0])
        else {
            unreachable!()
        };
        let oas_flow::Step::WithParams { params: second, .. } =
            with_params_of(&flow.steps[1])
        else {
            unreachable!()
        };
        assert_eq!(first[0].1, oas_flow::parse_expr("param('one')").unwrap());
        assert_eq!(second[0].1, oas_flow::parse_expr("param('two')").unwrap());
    }

    #[test]
    fn load_flow_fails_loudly_when_the_target_meta_is_missing_or_bad() {
        let td = TempDir::new("calltask-meta");
        raw_task(&td, "无meta", &finish_flow("无meta"), None);
        let caller = raw_task(
            &td,
            "调用者",
            r#"{"schema_version":1,"name":"调用者","steps":[
                 {"op":"call_task","args":{"task":"无meta"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
            None,
        );
        // P1-2: no more "no defaults + ignored warning": the call target must be
        // a complete bundle, otherwise inherit:none would silently degrade too.
        let err = caller.load_flow().expect_err("missing target meta must fail");
        assert!(err.contains("无meta"), "must name the target: {err}");
        assert!(err.contains("meta.json"), "must name the file: {err}");
        assert!(err.contains("call_task"), "must name the call: {err}");
        assert!(err.contains("调用者"), "must name the caller: {err}");

        raw_task(
            &td,
            "坏meta",
            &finish_flow("坏meta"),
            Some("{ this is not json"),
        );
        std::fs::write(
            caller.flow_path(),
            r#"{"schema_version":1,"name":"调用者","steps":[
                 {"op":"call_task","args":{"task":"坏meta"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        )
        .expect("rewrite caller flow");
        let err = caller.load_flow().expect_err("malformed target meta must fail");
        assert!(err.contains("坏meta"), "{err}");
        assert!(err.contains("meta.json is invalid"), "{err}");

        // A default that is not a flow value (array) is a hard error too: the
        // loader must not silently drop it and run the subtask with no default.
        raw_task(
            &td,
            "坏默认",
            &finish_flow("坏默认"),
            Some(
                r#"{"schema_version":1,"name":"坏默认","params":[
                     {"key":"k","type":"string","default":[1,2]}]}"#,
            ),
        );
        std::fs::write(
            caller.flow_path(),
            r#"{"schema_version":1,"name":"调用者","steps":[
                 {"op":"call_task","args":{"task":"坏默认"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        )
        .expect("rewrite caller flow");
        let err = caller.load_flow().expect_err("non-scalar default must fail");
        assert!(err.contains("坏默认"), "{err}");
        assert!(err.contains("default"), "{err}");
        assert!(err.contains("scalar"), "{err}");
    }

    #[test]
    fn load_flow_fingerprint_tracks_the_expanded_subtask_content() {
        let td = TempDir::new("calltask-fp");
        raw_task(
            &td,
            "子任务",
            &finish_flow("子任务"),
            Some(
                r#"{"schema_version":1,"name":"子任务","params":[
                     {"key":"k","type":"string","default":"d"}]}"#,
            ),
        );
        let caller = raw_task(
            &td,
            "调用者",
            r#"{"schema_version":1,"name":"调用者","steps":[
                 {"op":"call_task","args":{"task":"子任务","params":{"k":"param('p')"}}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
            Some(
                r#"{"schema_version":1,"name":"调用者","params":[
                     {"key":"p","type":"string","default":"v"}]}"#,
            ),
        );

        let (flow, first, warnings) = caller
            .load_flow_with_fingerprint()
            .expect("load with fingerprint");
        assert!(flow.is_some());
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_ne!(first, 0, "a present flow has a real fingerprint");
        let (_, second, _) = caller
            .load_flow_with_fingerprint()
            .expect("load again");
        assert_eq!(first, second, "fingerprint is stable across loads");

        // Edit the called subtask without changing its step count: the parent's
        // expanded flow changes, so the fingerprint must change (P1-3).
        let target = TaskBundle::open(td.path().join("子任务")).expect("target");
        std::fs::write(
            target.flow_path(),
            r#"{"schema_version":1,"name":"子任务","steps":[
                 {"op":"log","args":{"text":"'changed'"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        )
        .expect("rewrite target flow");
        let (_, changed, _) = caller
            .load_flow_with_fingerprint()
            .expect("load after subtask edit");
        assert_ne!(
            first, changed,
            "a subtask edit with the same step count must change the fingerprint"
        );

        // No flow.json at all is a distinct "no flow" identity, never a hash.
        let empty = TaskBundle::open(td.path().join("空任务")).expect("open");
        std::fs::create_dir_all(empty.root()).expect("dir");
        let (flow, fingerprint, _) = empty
            .load_flow_with_fingerprint()
            .expect("load empty task");
        assert!(flow.is_none());
        assert_eq!(fingerprint, 0);
    }

    #[test]
    fn load_flow_legacy_paramless_call_task_now_gets_subtask_defaults() {
        let td = TempDir::new("calltask-legacy");
        raw_task(
            &td,
            "子任务",
            &finish_flow("子任务"),
            Some(r#"{"schema_version":1,"name":"子任务","params":[
                 {"key":"触发文字","type":"string","default":"默认"}]}"#),
        );
        let caller = raw_task(
            &td,
            "调用者",
            r#"{"schema_version":1,"name":"调用者","steps":[
                 {"op":"call_task","args":{"task":"子任务"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
            None,
        );
        let flow = caller.load_flow().expect("load").expect("flow");
        let oas_flow::Step::WithParams {
            params,
            defaults,
            body,
            ..
        } = with_params_of(&flow.steps[0])
        else {
            unreachable!()
        };
        assert!(params.is_empty(), "the legacy call has no mapping");
        assert_eq!(
            defaults["触发文字"],
            oas_flow::Value::Str("默认".to_string()),
            "the old broken subtask default now runs"
        );
        assert_eq!(body.len(), 1);
    }

    #[test]
    fn load_flow_rejects_bad_call_task_args_with_the_call_in_the_message() {
        let td = TempDir::new("calltask-bad");
        raw_task(&td, "子任务", &finish_flow("子任务"), None);
        let caller = raw_task(
            &td,
            "调用者",
            r#"{"schema_version":1,"name":"调用者","steps":[
                 {"op":"call_task","args":{"task":"子任务"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
            None,
        );
        let bad_flows = [
            (
                r#"{"op":"call_task","args":{"task":"子任务","params":["x"]}}"#,
                "args.params must be an object",
            ),
            (
                r#"{"op":"call_task","args":{"task":"子任务","params":{"k":1}}}"#,
                "args.params.k must be an expression string",
            ),
            (
                r#"{"op":"call_task","args":{"task":"子任务","params":{"k":"param("}}}"#,
                "args.params.k",
            ),
            (
                r#"{"op":"call_task","args":{"task":"子任务","inherit":"sometimes"}}"#,
                "args.inherit",
            ),
            (
                r#"{"op":"call_task","args":{"task":"子任务","inherit":1}}"#,
                "args.inherit",
            ),
        ];
        for (step, expected) in bad_flows {
            let flow = format!(
                r#"{{"schema_version":1,"name":"调用者","steps":[{step},
                     {{"op":"finish","args":{{"exit":"done"}}}}]}}"#
            );
            std::fs::write(caller.flow_path(), flow).expect("write flow");
            let err = caller.load_flow().expect_err("must fail loudly");
            assert!(err.contains(expected), "expected {expected:?} in {err:?}");
            assert!(err.contains("子任务"), "must name the call: {err:?}");
        }
    }

    #[test]
    fn load_flow_keeps_hard_errors_for_missing_target_flow_and_cycles() {
        let td = TempDir::new("calltask-hard");
        // Target directory exists but has no flow.json: still a hard error.
        let target = TaskBundle::open(td.path().join("空目标")).expect("open");
        std::fs::create_dir_all(target.root()).expect("dir");
        std::fs::write(
            target.meta_path(),
            r#"{"schema_version":1,"name":"空目标","params":[]}"#,
        )
        .expect("meta");
        let caller = raw_task(
            &td,
            "调用者",
            r#"{"schema_version":1,"name":"调用者","steps":[
                 {"op":"call_task","args":{"task":"空目标"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
            None,
        );
        let err = caller.load_flow().expect_err("missing target flow");
        assert!(err.contains("空目标"), "{err}");
        assert!(err.contains("flow.json"), "{err}");

        // Cycles stay the loader's job (T7 never walks them). Both bundles are
        // complete (meta + flow) so the cycle check is what actually fires.
        raw_task(
            &td,
            "A",
            r#"{"schema_version":1,"name":"A","steps":[
                 {"op":"call_task","args":{"task":"B"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
            Some(r#"{"schema_version":1,"name":"A","params":[]}"#),
        );
        raw_task(
            &td,
            "B",
            r#"{"schema_version":1,"name":"B","steps":[
                 {"op":"call_task","args":{"task":"A"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
            Some(r#"{"schema_version":1,"name":"B","params":[]}"#),
        );
        let a = TaskBundle::open(td.path().join("A")).expect("open A");
        let err = a.load_flow().expect_err("cycle");
        assert!(err.contains("cycle"), "{err}");
    }

    #[test]
    fn load_flow_expands_call_task_into_a_with_params_scope() {
        let td = TempDir::new("calltask");
        raw_task(
            &td,
            "子任务",
            r#"{"schema_version":1,"name":"子任务","steps":[
                 {"op":"log","args":{"level":"info","text":"'inside'"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
            Some(
                r#"{"schema_version":1,"name":"子任务","params":[
                     {"key":"触发文字","type":"string","default":"默认"},
                     {"key":"max_rounds","type":"int","default":3},
                     {"key":"no_default","type":"string"}]}"#,
            ),
        );
        let caller = raw_task(
            &td,
            "调用者",
            r#"{"schema_version":1,"name":"调用者","steps":[
                 {"op":"call_task","args":{"task":"子任务","mode":"run",
                   "params":{"触发文字":"param('识别关键词')"}}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
            Some(
                r#"{"schema_version":1,"name":"调用者","params":[
                     {"key":"识别关键词","type":"string","default":"突破"}]}"#,
            ),
        );

        let (flow, warnings) = caller
            .load_flow_with_warnings()
            .expect("load with warnings");
        let flow = flow.expect("flow present");
        assert!(warnings.is_empty(), "{warnings:?}");

        assert_eq!(flow.steps.len(), 2, "scope + caller finish");
        let oas_flow::Step::WithParams {
            label,
            params,
            defaults,
            block,
            body,
        } = with_params_of(&flow.steps[0])
        else {
            unreachable!()
        };
        assert_eq!(label.as_deref(), Some("子任务"));
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].0, "触发文字");
        assert_eq!(
            params[0].1,
            oas_flow::parse_expr("param('识别关键词')").expect("expr")
        );
        // Only declared keys WITH a default are injected.
        assert_eq!(defaults.len(), 2, "{defaults:?}");
        assert_eq!(defaults["触发文字"], oas_flow::Value::Str("默认".into()));
        assert_eq!(defaults["max_rounds"], oas_flow::Value::Num(3.0));
        assert!(!defaults.contains_key("no_default"));
        // inherit defaults to "all" => no block list (same-name inheritance).
        assert!(block.is_empty(), "{block:?}");
        assert_eq!(body.len(), 1, "target finish must be stripped");
        assert!(
            matches!(body[0], oas_flow::Step::Log { .. }),
            "target step must be inside the scope: {body:?}"
        );
        assert!(
            matches!(flow.steps[1], oas_flow::Step::Finish { .. }),
            "caller finish must remain"
        );
        assert!(
            !flow
                .steps
                .iter()
                .any(|s| matches!(s, oas_flow::Step::CallTask { .. })),
            "no unexpanded call_task may reach the runtime"
        );
        // T1 is unaffected: the caller's own finish terminates the flow.
        assert!(flow.ends_with_finish());
    }

    #[test]
    fn load_flow_uses_child_variables_for_defaults_and_block() {
        // V2.0: subtask defaults/block come from the child's effective
        // variables (scope=input). Runtime-scope variables are flow-local and
        // must not be injected or block same-name inheritance.
        let td = TempDir::new("calltask-vars");
        raw_task(
            &td,
            "子任务",
            r#"{"schema_version":1,"name":"子任务","steps":[
                 {"op":"log","args":{"level":"info","text":"'inside'"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
            Some(
                r#"{"schema_version":1,"name":"子任务","variables":[
                     {"key":"触发文字","type":"string","default":"默认"},
                     {"key":"max_rounds","type":"int","default":3},
                     {"key":"no_default","type":"string"},
                     {"key":"battle_count","type":"int","scope":"runtime","default":0}]}"#,
            ),
        );
        let caller = raw_task(
            &td,
            "调用者",
            r#"{"schema_version":1,"name":"调用者","steps":[
                 {"op":"call_task","args":{"task":"子任务","inherit":"none",
                   "params":{"触发文字":"param('识别关键词')"}}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
            Some(
                r#"{"schema_version":1,"name":"调用者","variables":[
                     {"key":"识别关键词","type":"string","default":"突破"}]}"#,
            ),
        );
        let (flow, warnings) = caller
            .load_flow_with_warnings()
            .expect("load with warnings");
        let flow = flow.expect("flow present");
        assert!(warnings.is_empty(), "{warnings:?}");
        let oas_flow::Step::WithParams {
            defaults, block, ..
        } = with_params_of(&flow.steps[0])
        else {
            unreachable!()
        };
        assert_eq!(defaults.len(), 2, "{defaults:?}");
        assert_eq!(defaults["触发文字"], oas_flow::Value::Str("默认".into()));
        assert_eq!(defaults["max_rounds"], oas_flow::Value::Num(3.0));
        assert!(!defaults.contains_key("no_default"), "no default -> not injected");
        assert!(
            !defaults.contains_key("battle_count"),
            "runtime-scope variable must not be injected"
        );
        // inherit:"none" blocks exactly the child's input keys.
        let names: Vec<&str> = block.iter().map(|k| k.as_str()).collect();
        assert_eq!(names, vec!["触发文字", "max_rounds", "no_default"], "{block:?}");
    }

    #[test]
    fn id_grammar_accepts_chinese_and_rejects_bad_shapes() {
        for ok in ["分组2", "返回按钮", "_tmp", "a1", "Ab_c", "体力数字", "分组2_副本"] {
            assert!(validate_id(ok).is_ok(), "should accept {ok:?}");
        }
        for bad in [
            "",
            "2分组",
            "分 组",
            "返回-按钮",
            "返回按钮！",
            "分组\n",
            "a.b",
            "分组/2",
            "Ａ分组",
        ] {
            assert!(validate_id(bad).is_err(), "should reject {bad:?}");
        }
    }

    #[test]
    fn task_name_rejects_path_traversal_and_windows_traps() {
        for bad in ["", ".", "..", "../evil", "a/b", "a\\b", "a:b", "con", "con.txt", "任务.", " 任务"] {
            assert!(validate_task_name(bad).is_err(), "should reject {bad:?}");
        }
        for ok in ["任务1", "group_2", "a.b", "分组 2", "任务-1"] {
            assert!(validate_task_name(ok).is_ok(), "should accept {ok:?}");
        }
    }

    #[test]
    fn saves_assign_sequential_file_names_and_never_use_the_id_as_a_path() {
        let td = TempDir::new("seq");
        let b = open(&td);
        let f = frame(64, 48);

        let a1 = b
            .save_asset(AssetDecl::template("分组2", [0, 0, 10, 10]), Some(&f))
            .expect("first save");
        assert_eq!(a1.file.as_deref(), Some("images/a001.png"));
        assert_eq!(a1.rev, Some(1));

        let a2 = b
            .save_asset(AssetDecl::template("默认分组", [10, 10, 20, 15]), Some(&f))
            .expect("second save");
        assert_eq!(a2.file.as_deref(), Some("images/a002.png"));

        assert!(b.images_dir().join("a001.png").is_file());
        assert!(b.images_dir().join("a002.png").is_file());
        assert!(b.meta_path().is_file());

        // The Chinese id is stored in assets.json but is NOT a file name.
        let text = std::fs::read_to_string(b.assets_path()).expect("read assets.json");
        assert!(text.contains("\"分组2\""));
        assert!(!text.contains("分组2.png"));
        // f32 must not serialize as 0.8500000238418579 (on-disk contract says 0.85).
        assert!(
            text.contains("\"threshold\": 0.85"),
            "threshold should be human-readable in assets.json: {text}"
        );
        let mut files = Vec::new();
        walk_files(b.root(), &mut files);
        for f in &files {
            let name = f.file_name().unwrap().to_string_lossy().to_string();
            assert!(!name.contains("分组"), "id leaked into a path: {name}");
        }
    }

    #[test]
    fn update_asset_bumps_rev_swaps_pixels_and_removes_old_image() {
        let td = TempDir::new("upd");
        let b = open(&td);
        let f = frame(64, 48);
        let first = b
            .save_asset(AssetDecl::template("探索入口", [0, 0, 10, 10]), Some(&f))
            .expect("create");
        assert_eq!(first.rev, Some(1));
        let old_file = first.file.clone().expect("file");
        let old_bytes = std::fs::read(b.root().join(&old_file)).expect("old image");

        // A re-crop at a different ROI: same id, rev+1, NEW image, old file gone.
        let updated = b
            .update_asset(AssetDecl::template("探索入口", [4, 4, 12, 9]), Some(&f))
            .expect("update");
        assert_eq!(updated.rev, Some(2), "rev must bump");
        assert_ne!(
            updated.file.as_deref(),
            Some(old_file.as_str()),
            "a re-crop gets a new image file"
        );
        assert!(!b.root().join(&old_file).is_file(), "old image must be removed");
        let new_file = updated.file.as_deref().expect("new file");
        let new_bytes = std::fs::read(b.root().join(new_file)).expect("new image");
        assert_ne!(old_bytes, new_bytes, "pixels must be replaced");

        let af = b.load_assets_file().expect("assets");
        assert_eq!(af.items.len(), 1, "update must not duplicate the id");
        assert_eq!(af.items[0].id, "探索入口");
        assert_eq!(af.items[0].rev, Some(2));
        assert_eq!(af.items[0].roi, Some([4, 4, 12, 9]));
    }

    #[test]
    fn update_asset_works_for_ocr_rois_and_keeps_flow_refs_valid() {
        let td = TempDir::new("updocr");
        let b = open(&td);
        let f = frame(40, 30);
        b.save_asset(
            AssetDecl {
                kind: "ocr_line".into(),
                ..AssetDecl::template("探索入口", [0, 0, 20, 8])
            },
            Some(&f),
        )
        .expect("create ocr");
        b.save_flow(&flow_json("探索入口")).expect("flow");
        let flow_before = std::fs::read(b.flow_path()).expect("flow bytes");

        let updated = b
            .update_asset(
                AssetDecl {
                    kind: "ocr_line".into(),
                    ..AssetDecl::template("探索入口", [5, 5, 30, 10])
                },
                Some(&f),
            )
            .expect("update ocr");
        assert_eq!(updated.rev, Some(2));
        assert_eq!(updated.file, None, "OCR assets never own an image");
        assert_eq!(updated.roi, Some([5, 5, 30, 10]));
        assert_eq!(
            std::fs::read(b.flow_path()).expect("flow bytes"),
            flow_before,
            "updating an asset must not touch flow.json"
        );
    }

    #[test]
    fn ocr_asset_threshold_round_trips_into_the_runtime_assets() {
        let td = TempDir::new("ocrthr");
        let b = open(&td);
        let f = frame(40, 30);
        b.save_asset(
            AssetDecl {
                kind: "ocr_line".into(),
                ..AssetDecl::template("文字素材", [0, 0, 20, 8])
            },
            Some(&f),
        )
        .expect("create ocr");

        // A threshold on an OCR asset is its minimum recognition confidence.
        // The old guard rejected every non-template kind here.
        let updated = b
            .update_asset_meta("文字素材", None, None, None, Some(0.7), false)
            .expect("ocr threshold update");
        assert_eq!(updated.threshold, Some(0.7));
        let text = std::fs::read_to_string(b.assets_path()).expect("assets");
        assert!(text.contains("\"threshold\": 0.7"), "{text}");

        let set = b.load_runtime_assets().expect("runtime assets");
        assert_eq!(
            set.ocr_threshold("文字素材"),
            Some(0.7),
            "load must wire the OCR threshold into the runtime"
        );
        assert_eq!(
            set.threshold_for("文字素材"),
            Some(0.7),
            "the shared threshold lookup must see the OCR value"
        );
    }

    #[test]
    fn ocr_zero_threshold_update_is_accepted_and_loads_as_unset() {
        // P1-4: `None` means "leave unchanged", so the editor clears an OCR
        // threshold by writing 0. The write must be accepted and the runtime
        // must treat the loaded 0 exactly like a missing threshold.
        let td = TempDir::new("ocrzero");
        let b = open(&td);
        let f = frame(40, 30);
        b.save_asset(
            AssetDecl {
                kind: "ocr_line".into(),
                ..AssetDecl::template("文字素材", [0, 0, 20, 8])
            },
            Some(&f),
        )
        .expect("create ocr");
        b.update_asset_meta("文字素材", None, None, None, Some(0.7), false)
            .expect("set a real threshold first");

        let cleared = b
            .update_asset_meta("文字素材", None, None, None, Some(0.0), false)
            .expect("0 must be a valid threshold");
        assert_eq!(cleared.threshold, Some(0.0));
        let text = std::fs::read_to_string(b.assets_path()).expect("assets");
        assert!(text.contains("\"threshold\": 0.0"), "{text}");

        let set = b.load_runtime_assets().expect("runtime assets");
        assert_eq!(
            set.ocr_threshold("文字素材"),
            None,
            "0 must mean no confidence filter, exactly like a missing threshold"
        );
        assert_eq!(
            set.threshold_for("文字素材"),
            None,
            "0 must not surface as an explicit gate through threshold_for"
        );
    }

    #[test]
    fn update_asset_meta_threshold_is_range_checked_and_rejects_regions() {
        let td = TempDir::new("thrrange");
        let b = open(&td);
        let f = frame(40, 30);
        b.save_asset(
            AssetDecl {
                kind: "ocr_line".into(),
                ..AssetDecl::template("文字素材", [0, 0, 20, 8])
            },
            Some(&f),
        )
        .expect("create ocr");
        b.save_asset(
            AssetDecl {
                kind: "region".into(),
                ..AssetDecl::template("操作区域", [2, 2, 10, 10])
            },
            Some(&f),
        )
        .expect("create region");

        // Non-finite / out-of-range values stay rejected for every kind.
        for bad in [1.5f32, -0.1, f32::NAN] {
            let err = b
                .update_asset_meta("文字素材", None, None, None, Some(bad), false)
                .expect_err("bad threshold must be rejected");
            assert!(err.contains("0..=1"), "unexpected: {err}");
        }
        // A pure region has no recognition, so a threshold is meaningless.
        let before = std::fs::read(b.assets_path()).expect("assets bytes");
        let err = b
            .update_asset_meta("操作区域", None, None, None, Some(0.5), false)
            .expect_err("region threshold must be rejected");
        assert!(err.contains("does not apply to region"), "unexpected: {err}");
        assert_eq!(
            std::fs::read(b.assets_path()).expect("assets bytes"),
            before,
            "a rejected update must write nothing"
        );
    }

    #[test]
    fn template_threshold_update_still_works_alongside_ocr_thresholds() {
        let td = TempDir::new("thrtpl");
        let b = open(&td);
        let f = frame(40, 30);
        b.save_asset(AssetDecl::template("按钮", [0, 0, 10, 10]), Some(&f))
            .expect("create template");
        let updated = b
            .update_asset_meta("按钮", None, None, None, Some(0.75), false)
            .expect("template threshold update");
        assert_eq!(updated.threshold, Some(0.75));
        let set = b.load_runtime_assets().expect("runtime assets");
        assert_eq!(set.get("按钮").expect("template").threshold, 0.75);
        assert_eq!(set.threshold_for("按钮"), Some(0.75));
    }

    #[test]
    fn update_asset_of_a_missing_id_is_rejected_and_writes_nothing() {
        let td = TempDir::new("updmiss");
        let b = open(&td);
        let f = frame(32, 32);
        let err = b
            .update_asset(AssetDecl::template("探索入口", [0, 0, 8, 8]), Some(&f))
            .expect_err("must reject");
        assert!(err.contains("does not exist"), "unexpected: {err}");
        assert!(!b.assets_path().exists(), "nothing written");
    }

    #[test]
    fn failed_update_writes_nothing() {
        let td = TempDir::new("updfail");
        let b = open(&td);
        let f = frame(40, 30);
        b.save_asset(AssetDecl::template("探索入口", [0, 0, 10, 10]), Some(&f))
            .expect("create");
        let before_assets = std::fs::read(b.assets_path()).expect("assets");
        let mut before_files = Vec::new();
        walk_files(b.root(), &mut before_files);

        // ROI outside the frame => must fail before any write.
        let err = b
            .update_asset(AssetDecl::template("探索入口", [35, 25, 10, 10]), Some(&f))
            .expect_err("must fail");
        assert!(err.contains("out of bounds") || err.contains("roi"), "{err}");
        assert_eq!(std::fs::read(b.assets_path()).expect("assets"), before_assets);
        let mut after_files = Vec::new();
        walk_files(b.root(), &mut after_files);
        assert_eq!(before_files.len(), after_files.len(), "no orphan image");
    }

    #[test]
    fn polygon_asset_is_persisted_and_wired_into_the_runtime_template() {
        let td = TempDir::new("poly");
        let b = open(&td);
        let f = frame(64, 48);
        let mut decl = AssetDecl::template("多边形按钮", [10, 10, 21, 11]);
        decl.polygon = Some(vec![[10, 10], [30, 10], [30, 20], [10, 20]]);
        let saved = b.save_asset(decl, Some(&f)).expect("save polygon asset");
        // roi is DERIVED from the polygon bbox (inclusive pixel convention).
        assert_eq!(saved.roi, Some([10, 10, 21, 11]));
        assert_eq!(saved.rev, Some(1));
        let text = std::fs::read_to_string(b.assets_path()).expect("read assets");
        assert!(text.contains("\"polygon\""), "{text}");
        let set = b.load_runtime_assets().expect("runtime assets");
        let tpl = set.get("多边形按钮").expect("template");
        let poly = tpl.click_polygon.as_ref().expect("polygon wired");
        assert_eq!(poly.len(), 4);
        assert_eq!((poly[0].x, poly[0].y), (10, 10));
    }

    #[test]
    fn ocr_asset_polygon_is_wired_into_the_runtime_ocr_mask() {
        let td = TempDir::new("polyocr");
        let b = open(&td);
        let f = frame(64, 48);
        b.save_asset(AssetDecl::template("占位模板", [10, 10, 21, 11]), Some(&f))
            .expect("template sets design resolution");
        let mut decl = AssetDecl {
            id: "多边形文字".to_string(),
            kind: "ocr_lines".to_string(),
            file: None,
            roi: Some([10, 10, 21, 11]),
            action_shape: None,
            action_source: "recognition".to_string(),
            ocr_mode: None,
            ocr_source_roi: None,
            ocr_text: None,
            threshold: None,
            polygon: Some(vec![[10, 10], [30, 10], [30, 20], [10, 20]]),
            rev: None,
            positions: Vec::new(),
            ..AssetDecl::template("_", [0, 0, 1, 1])
        };        let saved = b.save_asset(decl.clone(), Some(&f)).expect("save ocr polygon");
        assert_eq!(saved.roi, Some([10, 10, 21, 11]));
        let set = b.load_runtime_assets().expect("runtime assets");
        let poly = set.ocr_polygon("多边形文字").expect("ocr polygon wired");
        assert_eq!(poly.len(), 4);
        assert_eq!((poly[2].x, poly[2].y), (30, 20));
        // OCR assets still carry no image file.
        decl = saved;
        assert!(decl.file.is_none());
    }

    #[test]
    fn action_shape_is_persisted_and_wired_into_the_runtime_operation_region() {
        // 2026-11-06 事项6 (P0b): an operation shape needs no `action_source`
        // state any more; it is optional, always allowed and is the only thing
        // that decides the operation region.
        let td = TempDir::new("back");
        let b = open(&td);
        let f = frame(64, 48);
        let mut decl = AssetDecl::template("点击区按钮", [10, 10, 21, 11]);
        decl.action_shape = Some(ActionShape::rect([8, 8, 30, 30]));
        let saved = b.save_asset(decl, Some(&f)).expect("save with action_shape");
        assert_eq!(
            saved.action_shape.as_ref().and_then(|s| s.bbox()),
            Some([8, 8, 30, 30])
        );
        // Deprecated compat field: in-memory value normalized, never persisted.
        assert_eq!(saved.action_source, "recognition");
        let text = std::fs::read_to_string(b.assets_path()).expect("read assets");
        assert!(text.contains("action_shape"), "{text}");
        assert!(
            !text.contains("action_source"),
            "the two-state field must not be written again: {text}"
        );
        assert!(!text.contains("action_roi"), "legacy field must be gone: {text}");
        let set = b.load_runtime_assets().expect("runtime assets");
        let back = set.click_back("点击区按钮").expect("click_back wired");
        assert_eq!((back.x, back.y, back.w, back.h), (8, 8, 30, 30));
    }

    #[test]
    fn polygon_action_shape_is_persisted_and_invert_is_kept() {
        let td = TempDir::new("polyshape");
        let b = open(&td);
        let f = frame(64, 48);
        let mut decl = AssetDecl::template("多边形操作区", [10, 10, 21, 11]);
        decl.action_shape = Some(ActionShape {
            kind: "polygon".into(),
            roi: None,
            points: Some(vec![[8, 8], [40, 8], [40, 30], [8, 30]]),
            invert: true,
        });
        let saved = b.save_asset(decl, Some(&f)).expect("save polygon shape");
        assert_eq!(saved.action_source, "recognition");
        let text = std::fs::read_to_string(b.assets_path()).expect("read");
        assert!(text.contains("\"polygon\""), "{text}");
        assert!(text.contains("\"invert\": true"), "{text}");
        assert!(!text.contains("action_source"), "{text}");
        let af = b.load_assets_file().expect("load");
        let shape = af.items[0].action_shape.as_ref().expect("shape");
        assert!(shape.invert);
        assert_eq!(shape.points.as_ref().map(|p| p.len()), Some(4));
        let set = b.load_runtime_assets().expect("runtime");
        let back = set.click_back("多边形操作区").expect("bbox fallback");
        assert_eq!((back.x, back.y, back.w, back.h), (8, 8, 33, 23));
    }

    #[test]
    fn self_intersecting_action_polygon_is_rejected() {
        let td = TempDir::new("polyself");
        let b = open(&td);
        let f = frame(64, 48);
        let mut decl = AssetDecl::template("自交多边形", [10, 10, 21, 11]);
        decl.action_shape = Some(ActionShape {
            kind: "polygon".into(),
            roi: None,
            points: Some(vec![[8, 8], [40, 30], [40, 8], [8, 30]]),
            invert: false,
        });
        let err = b.save_asset(decl, Some(&f)).expect_err("self-intersection must fail");
        assert!(err.contains("self-intersect"), "{err}");
    }

    #[test]
    fn action_shape_is_valid_for_ocr_and_region_derives_its_own_shape() {
        let td = TempDir::new("backbad");
        let b = open(&td);
        let f = frame(64, 48);
        // 2026-11-06 事项6: a shape with the (old default) recognition state is
        // no longer rejected — the shape itself decides the operation region.
        let ocr = AssetDecl {
            id: "文字".to_string(),
            kind: "ocr_lines".to_string(),
            file: None,
            roi: Some([10, 10, 21, 11]),
            action_shape: Some(ActionShape::rect([12, 12, 30, 30])),
            action_source: "recognition".to_string(),
            ocr_mode: None,
            ocr_source_roi: None,
            ocr_text: None,
            threshold: None,
            polygon: None,
            rev: None,
            positions: Vec::new(),
            ..AssetDecl::template("_", [0, 0, 1, 1])
        };
        b.save_asset(ocr.clone(), Some(&f))
            .expect("OCR shape without a fixed source is valid");
        let set = b.load_runtime_assets().expect("runtime assets");
        let op = set.click_back("文字").expect("OCR operation region");
        assert_eq!((op.x, op.y, op.w, op.h), (12, 12, 30, 30));

        let mut region = AssetDecl::template("占位", [1, 1, 2, 2]);
        region.id = "右侧列表".to_string();
        region.kind = "region".to_string();
        region.roi = Some([40, 10, 20, 20]);
        region.file = None;
        region.threshold = None;
        region.action_shape = None;
        region.action_source = "recognition".to_string();
        b.save_asset(region.clone(), Some(&f))
            .expect("pure region save");
        let af = b.load_assets_file().expect("assets");
        let saved_region = af.items.iter().find(|a| a.id == "右侧列表").expect("region");
        assert_eq!(
            saved_region.action_shape.as_ref().and_then(|s| s.bbox()),
            Some([40, 10, 20, 20])
        );
        let set = b.load_runtime_assets().expect("runtime assets");
        let r = set.region("右侧列表").expect("region wired");
        assert_eq!((r.x, r.y, r.w, r.h), (40, 10, 20, 20));
    }

    #[test]
    fn batch_save_is_all_or_nothing() {
        let td = TempDir::new("batchrollback");
        let b = open(&td);
        let ok = AssetDecl {
            id: "批次甲".to_string(),
            kind: "ocr_line".to_string(),
            file: None,
            roi: Some([1, 2, 30, 20]),
            action_shape: Some(ActionShape::rect([0, 0, 40, 30])),
            action_source: "fixed".to_string(),
            ocr_mode: None,
            ocr_source_roi: None,
            ocr_text: None,
            threshold: None,
            polygon: None,
            rev: None,
            positions: Vec::new(),
            ..AssetDecl::template("_", [0, 0, 1, 1])
        };
        let bad = AssetDecl {
            id: "批次乙".to_string(),
            kind: "region".to_string(),
            file: None,
            roi: None, // region requires roi
            action_shape: None,
            action_source: "recognition".to_string(),
            ocr_mode: None,
            ocr_source_roi: None,
            ocr_text: None,
            threshold: None,
            polygon: None,
            rev: None,
            positions: Vec::new(),
            ..AssetDecl::template("_", [0, 0, 1, 1])
        };
        let err = b
            .save_assets_batch(vec![ok, bad], None)
            .expect_err("bad batch must roll back");
        assert!(err.contains("rolled back"), "{err}");
        assert!(
            !b.assets_path().exists()
                || b.load_assets_file()
                    .map(|af| af.items.is_empty())
                    .unwrap_or(true),
            "no partial asset may remain"
        );
    }

    #[test]
    fn polygon_asset_validation_rejects_bad_shapes_without_writing() {        let td = TempDir::new("polybad");
        let b = open(&td);
        let f = frame(40, 30);
        let cases: Vec<Vec<[i32; 2]>> = vec![
            vec![[0, 0], [10, 10]],                                     // < 3 points
            vec![[0, 0], [10, 0], [20, 0]],                             // zero area
            vec![[0, 0], [-5, 10], [10, 10]],                           // negative
            vec![[0, 0], [100, 0], [100, 10], [0, 10]],                 // outside frame
            vec![[0, 0], [10, 10], [12, 0], [0, 12]],                   // self-intersecting (bowtie, non-zero area)
        ];
        for (i, poly) in cases.into_iter().enumerate() {
            let mut decl = AssetDecl::template(format!("坏多边形{i}"), [0, 0, 10, 10]);
            decl.polygon = Some(poly);
            let err = b.save_asset(decl, Some(&f)).expect_err("must reject");
            assert!(err.contains("nothing written"), "case {i}: {err}");
            assert!(!b.assets_path().exists(), "case {i} must write nothing");
        }
    }

    #[test]
    fn duplicate_id_is_rejected_without_mutation() {
        let td = TempDir::new("dup");
        let b = open(&td);
        let f = frame(32, 32);
        b.save_asset(AssetDecl::template("分组2", [0, 0, 8, 8]), Some(&f))
            .expect("first save");
        let before = std::fs::read(b.assets_path()).expect("read assets");
        let mut files_before = Vec::new();
        walk_files(b.root(), &mut files_before);

        let err = b
            .save_asset(AssetDecl::template("分组2", [8, 8, 8, 8]), Some(&f))
            .expect_err("duplicate must be rejected");
        assert!(err.contains("already exists"), "unexpected error: {err}");

        assert_eq!(std::fs::read(b.assets_path()).expect("read assets"), before);
        let mut files_after = Vec::new();
        walk_files(b.root(), &mut files_after);
        assert_eq!(files_before.len(), files_after.len());
    }

    #[test]
    fn template_requires_frame_roi_and_in_frame_bounds() {
        let td = TempDir::new("rules");
        let b = open(&td);
        let f = frame(40, 30);

        let mut no_roi = AssetDecl::template("分组2", [0, 0, 5, 5]);
        no_roi.roi = None;
        let err = b.save_asset(no_roi, Some(&f)).unwrap_err();
        assert!(err.contains("requires roi"), "unexpected: {err}");

        let err = b
            .save_asset(AssetDecl::template("分组2", [0, 0, 5, 5]), None)
            .unwrap_err();
        assert!(err.contains("captured frame"), "unexpected: {err}");

        let err = b
            .save_asset(AssetDecl::template("分组2", [35, 0, 10, 10]), Some(&f))
            .unwrap_err();
        assert!(err.contains("outside"), "unexpected: {err}");

        let err = b
            .save_asset(AssetDecl::template("分组2", [-1, 0, 5, 5]), Some(&f))
            .unwrap_err();
        assert!(err.contains("x >= 0"), "unexpected: {err}");

        let mut bad_threshold = AssetDecl::template("分组2", [0, 0, 5, 5]);
        bad_threshold.threshold = Some(1.5);
        let err = b.save_asset(bad_threshold, Some(&f)).unwrap_err();
        assert!(err.contains("out of range"), "unexpected: {err}");

        // Nothing was written by any of the rejected saves.
        assert!(!b.root().exists());
    }

    #[test]
    fn save_time_ref_validation_accepts_a_flow_that_references_the_new_id() {
        let td = TempDir::new("refok");
        let b = open(&td);
        std::fs::create_dir_all(b.root()).expect("root");
        // A flow already references 分组2, which is not declared yet: saving the
        // asset must satisfy the reference and succeed.
        std::fs::write(b.flow_path(), flow_json("分组2")).expect("seed flow");

        let saved = b
            .save_asset(AssetDecl::template("分组2", [0, 0, 8, 8]), Some(&frame(32, 32)))
            .expect("flow-referenced new asset must save");
        assert_eq!(saved.file.as_deref(), Some("images/a001.png"));
        assert!(b.assets_path().is_file());
        assert!(b.load_runtime_assets().is_ok());
    }

    #[test]
    fn save_time_ref_validation_blocks_typo_and_writes_nothing() {
        let td = TempDir::new("reftypo");
        let b = open(&td);
        std::fs::create_dir_all(b.root()).expect("root");
        // flow.json says 分组3, but the user is saving 分组2 (a typo upstream).
        std::fs::write(b.flow_path(), flow_json("分组3")).expect("seed flow");

        let err = b
            .save_asset(AssetDecl::template("分组2", [0, 0, 8, 8]), Some(&frame(32, 32)))
            .expect_err("flow referencing an unknown id must block the save");
        assert!(err.contains("分组3"), "error must name the missing id: {err}");
        assert!(err.contains("not declared"), "error must say it is undeclared: {err}");
        assert!(err.contains("assets"), "error must carry the JSON path: {err}");

        // NOTHING was written: no assets.json, no images, no meta.json.
        assert!(!b.assets_path().exists());
        assert!(!b.images_dir().exists());
        assert!(!b.meta_path().exists());
        assert_eq!(std::fs::read_dir(b.root()).unwrap().count(), 1);
    }

    #[test]
    fn save_flow_valid_then_invalid_writes_only_on_success() {
        let td = TempDir::new("flow");
        let b = open(&td);
        b.save_asset(AssetDecl::template("返回按钮", [0, 0, 8, 8]), Some(&frame(32, 32)))
            .expect("save asset");

        b.save_flow(&flow_json("返回按钮")).expect("valid flow");
        let before = std::fs::read(b.flow_path()).expect("read flow");
        assert!(b.load_flow().expect("load flow").is_some());

        let err = b.save_flow(&flow_json("返回按钮x")).unwrap_err();
        assert!(err.contains("not declared"), "unexpected: {err}");
        assert_eq!(std::fs::read(b.flow_path()).expect("read flow"), before);

        // A malformed flow is rejected by the DSL parser (load-time rules).
        let err = b
            .save_flow(r#"{"schema_version":1,"name":"x"}"#)
            .expect_err("missing steps must be rejected");
        assert!(err.contains("missing `steps`"), "unexpected: {err}");
        assert_eq!(std::fs::read(b.flow_path()).expect("read flow"), before);
    }

    #[test]
    fn save_flow_on_a_fresh_bundle_writes_nothing_when_it_fails() {
        let td = TempDir::new("freshflow");
        let b = open(&td);
        let err = b.save_flow(&flow_json("不存在")).unwrap_err();
        assert!(err.contains("not declared"), "unexpected: {err}");
        assert!(!b.flow_path().exists());
        assert!(!b.meta_path().exists());
    }

    #[test]
    fn legacy_source_roi_migrates_to_single_recognition_roi() {
        let td = TempDir::new("roimigrate");
        let b = open(&td);
        std::fs::create_dir_all(b.root()).expect("root");
        std::fs::write(
            b.assets_path(),
            r#"{"schema_version":1,"items":[
                 {"id":"召唤","kind":"ocr_line","roi":[10,20,30,40],
                  "ocr_mode":"lines","ocr_source_roi":[1,2,50,60],"ocr_text":"召唤"}
               ]}"#,
        )
        .expect("write legacy assets");
        let af = b.load_assets_file().expect("load");
        assert_eq!(af.items[0].roi, Some([1, 2, 50, 60]));
        assert_eq!(af.items[0].ocr_source_roi, None);
        let set = b.load_runtime_assets().expect("runtime assets");
        assert_eq!(set.derived_ocr("召唤"), Some((oas_types::Rect { x: 1, y: 2, w: 50, h: 60 }, "召唤".to_string())));
    }

    #[test]
    fn legacy_roi_migration_preserves_a_larger_manual_roi() {
        let td = TempDir::new("roimigrate2");
        let b = open(&td);
        std::fs::create_dir_all(b.root()).expect("root");
        std::fs::write(
            b.assets_path(),
            r#"{"schema_version":1,"items":[
                 {"id":"召唤","kind":"ocr_line","roi":[40,50,80,70],
                  "ocr_mode":"lines","ocr_source_roi":[1,2,50,60],"ocr_text":"召唤"}
               ]}"#,
        )
        .expect("write legacy assets");
        let af = b.load_assets_file().expect("load");
        // union([1,2,50,60], [40,50,80,70]) = [1,2,119,118]
        assert_eq!(af.items[0].roi, Some([1, 2, 119, 118]));
        assert_eq!(af.items[0].ocr_source_roi, None);
    }

    #[test]
    fn load_runtime_assets_maps_dimensions_threshold_and_roi() {
        let td = TempDir::new("runtime");
        let b = open(&td);
        let f = frame(64, 48);
        let tpl = AssetDecl {
            threshold: Some(0.9),
            ..AssetDecl::template("分组2", [10, 8, 20, 12])
        };
        b.save_asset(tpl, Some(&f)).expect("save template");

        // An OCR asset carries no pixels and must not enter the AssetSet.
        let mut ocr = AssetDecl::template("体力数字", [1, 2, 3, 4]);
        ocr.kind = "ocr_line".to_string();
        ocr.file = Some("images/ignored.png".to_string());
        let saved_ocr = b.save_asset(ocr, None).expect("save ocr asset");
        assert_eq!(saved_ocr.file, None);
        assert!(!b.images_dir().join("ignored.png").exists());

        let set = b.load_runtime_assets().expect("load runtime assets");
        assert_eq!(set.len(), 1);
        let tpl = set.get("分组2").expect("template present");
        assert_eq!((tpl.gray.width, tpl.gray.height), (20, 12));
        assert_eq!(tpl.threshold, 0.9);
        assert_eq!(
            tpl.roi,
            Some(oas_types::Rect {
                x: 10,
                y: 8,
                w: 20,
                h: 12
            })
        );
    }

    #[test]
    fn design_resolution_mismatch_is_rejected() {
        let td = TempDir::new("design");
        let b = open(&td);
        b.save_asset(AssetDecl::template("分组2", [0, 0, 8, 8]), Some(&frame(100, 50)))
            .expect("first save");
        let err = b
            .save_asset(AssetDecl::template("默认分组", [0, 0, 8, 8]), Some(&frame(200, 50)))
            .expect_err("different frame size must be rejected");
        assert!(
            err.contains("design resolution mismatch"),
            "unexpected error: {err}"
        );
        assert!(b.images_dir().join("a001.png").is_file());
        assert!(!b.images_dir().join("a002.png").exists());
        assert_eq!(b.load_assets_file().expect("assets").items.len(), 1);
        assert_eq!(
            b.load_assets_file().expect("assets").design_resolution,
            Some([100, 50])
        );
    }

    #[test]
    fn validate_accepts_a_good_bundle_and_flags_injected_corruption() {
        let td = TempDir::new("validate");
        let b = open(&td);
        b.save_asset(AssetDecl::template("返回按钮", [0, 0, 8, 8]), Some(&frame(32, 32)))
            .expect("save asset");
        b.save_flow(&flow_json("返回按钮")).expect("save flow");
        assert!(b.validate().is_ok(), "valid bundle: {:?}", b.validate());

        // INJECTION: assets.json references an image file that is not on disk.
        let injected = serde_json::json!({
            "schema_version": 1,
            "design_resolution": [32, 32],
            "items": [{
                "id": "返回按钮", "kind": "template", "file": "images/a999.png",
                "roi": [0, 0, 8, 8], "threshold": 0.85, "rev": 1
            }]
        });
        std::fs::write(b.assets_path(), serde_json::to_string(&injected).unwrap())
            .expect("inject");
        let errs = b.validate().expect_err("missing image must be flagged");
        assert!(
            errs.iter().any(|e| e.contains("does not exist")),
            "unexpected errors: {errs:?}"
        );
        assert!(b.load_runtime_assets().is_err());

        // INJECTION: duplicate ids in assets.json.
        let dup = serde_json::json!({
            "schema_version": 1,
            "items": [
                {"id": "返回按钮", "kind": "template", "file": "images/a001.png", "roi": [0, 0, 8, 8]},
                {"id": "返回按钮", "kind": "template", "file": "images/a001.png", "roi": [0, 0, 8, 8]}
            ]
        });
        std::fs::write(b.assets_path(), serde_json::to_string(&dup).unwrap()).expect("inject");
        let err = b.load_assets_file().expect_err("duplicate ids must be flagged");
        assert!(err.contains("duplicate asset id"), "unexpected: {err}");
    }

    #[test]
    fn corrupt_assets_json_is_a_loud_error_and_blocks_saves() {
        let td = TempDir::new("corrupt");
        let b = open(&td);
        std::fs::create_dir_all(b.root()).expect("root");
        std::fs::write(b.assets_path(), "{not json").expect("seed corruption");
        assert!(b.load_assets_file().is_err());
        let err = b
            .save_asset(AssetDecl::template("分组2", [0, 0, 8, 8]), Some(&frame(32, 32)))
            .unwrap_err();
        assert!(err.contains("invalid JSON"), "unexpected: {err}");
        assert!(!b.images_dir().exists());
    }

    #[test]
    fn atomic_writes_leave_no_tmp_files_behind() {
        let td = TempDir::new("atomic");
        let b = open(&td);
        b.save_asset(AssetDecl::template("分组2", [0, 0, 8, 8]), Some(&frame(32, 32)))
            .expect("save asset");
        b.save_flow(&flow_json("分组2")).expect("save flow");
        b.save_flow(&flow_json("分组2")).expect("overwrite flow");

        let mut files = Vec::new();
        walk_files(b.root(), &mut files);
        for f in &files {
            let name = f.file_name().unwrap().to_string_lossy().to_string();
            assert!(!name.contains(".tmp-"), "leftover tmp file: {}", f.display());
        }
        assert!(b.meta_path().is_file());
        assert!(b.flow_path().is_file());
        assert!(b.assets_path().is_file());
    }

    #[test]
    fn list_tasks_lists_bundles_and_tolerates_a_missing_tasks_dir() {
        let td = TempDir::new("list");
        let tasks_dir = td.path().join("tasks");
        std::fs::create_dir_all(&tasks_dir).expect("tasks dir");
        let b = TaskBundle::open_named(&tasks_dir, "任务1").expect("open named");
        b.save_asset(AssetDecl::template("分组2", [0, 0, 8, 8]), Some(&frame(32, 32)))
            .expect("save asset");
        let other = TaskBundle::open_named(&tasks_dir, "任务2").expect("open named");
        other
            .save_flow(&flow_json("不存在"))
            .expect_err("needs an asset");
        // 任务2 has no meta.json yet (its only save failed) => not a bundle.
        assert_eq!(list_tasks(&tasks_dir).expect("list"), vec!["任务1".to_string()]);
        assert!(list_tasks(&td.path().join("no-such-dir")).expect("missing dir").is_empty());
    }

    #[test]
    fn open_named_blocks_traversal_but_open_allows_any_existing_root() {
        let td = TempDir::new("open");
        assert!(TaskBundle::open_named(td.path(), "../evil").is_err());
        assert!(TaskBundle::open_named(td.path(), "a/b").is_err());
        let ok = TaskBundle::open_named(td.path(), "任务1").expect("valid name");
        assert_eq!(ok.root(), td.path().join("任务1"));
    }

    // ---- M0 lint: meta contract + T1/T3/T4/T5/T6 ----

    /// A legacy bundle exactly like the two shipped tasks: old-shape meta.json
    /// (schema/name/expire only), one template asset, flow ending in finish(done).
    fn write_legacy_bundle(td: &TempDir) -> TaskBundle {
        let b = open(td);
        b.save_asset(
            AssetDecl::template("返回按钮", [0, 0, 8, 8]),
            Some(&frame(32, 32)),
        )
        .expect("save asset");
        b.save_flow(&flow_json("返回按钮")).expect("save flow");
        b
    }

    fn write_flow_only_bundle(b: &TaskBundle, meta: &str, flow: &str) {
        std::fs::create_dir_all(b.root()).expect("root");
        std::fs::write(b.meta_path(), meta).expect("write meta");
        if !b.assets_path().exists() {
            std::fs::write(b.assets_path(), r#"{"schema_version":1,"items":[]}"#)
                .expect("write assets");
        }
        std::fs::write(b.flow_path(), flow).expect("write flow");
    }

    #[test]
    fn lint_passes_a_legacy_bundle_untouched() {
        let td = TempDir::new("lintlegacy");
        let b = write_legacy_bundle(&td);
        // meta.json is the OLD shape written by save_flow: no M0 fields at all.
        let meta_text = std::fs::read_to_string(b.meta_path()).expect("meta");
        assert!(
            !meta_text.contains("exits"),
            "fixture must stay legacy: {meta_text}"
        );
        b.lint()
            .unwrap_or_else(|issues| panic!("legacy bundle must lint green: {issues:?}"));

        // The typed view applies the M0 defaults (old tasks are untouched on disk).
        let meta = b.load_meta().expect("load meta").expect("meta present");
        assert_eq!(meta.category(), "expire");
        assert_eq!(meta.exits.len(), 2);
        assert!(meta.capabilities.checkpoint);
        assert!(meta.params.is_empty());
    }

    #[test]
    fn lint_flags_unknown_exit_result_in_meta() {
        let td = TempDir::new("lintbadresult");
        let b = write_legacy_bundle(&td);
        std::fs::write(
            b.meta_path(),
            r#"{"schema_version":1,"name":"任务1","exits":{"done":{"result":"continue"}}}"#,
        )
        .expect("meta");
        let issues = b.lint().expect_err("`continue` is not a result class");
        assert!(
            issues
                .iter()
                .any(|i| i.code == "META" && i.path == "meta.json:exits.done.result"),
            "{issues:?}"
        );
        assert!(
            issues.iter().any(|i| i.msg.contains("continue/stop")),
            "{issues:?}"
        );
    }

    #[test]
    fn lint_flags_an_exit_that_is_not_declared() {
        let td = TempDir::new("lintexit");
        let b = open(&td);
        write_flow_only_bundle(
            &b,
            r#"{"schema_version":1,"name":"任务1","exits":{"done":{"result":"success"}}}"#,
            r#"{"schema_version":1,"name":"任务1","steps":[
                 {"op":"finish","args":{"exit":"failed"}}]}"#,
        );
        let issues = b.lint().expect_err("`failed` is not declared");
        let t1: Vec<&LintIssue> = issues.iter().filter(|i| i.code == "T1").collect();
        assert_eq!(t1.len(), 1, "{issues:?}");
        assert!(
            t1[0].path.contains("steps[0].args.exit"),
            "path must point at the finish step: {}",
            t1[0].path
        );
        assert!(t1[0].msg.contains("failed"), "{}", t1[0].msg);
    }

    #[test]
    fn lint_flags_a_flow_that_does_not_end_with_finish() {
        let td = TempDir::new("lintend");
        let b = open(&td);
        write_flow_only_bundle(
            &b,
            r#"{"schema_version":1,"name":"任务1"}"#,
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"log","args":{"text":"'no finish'"}}]}"#,
        );
        let issues = b.lint().expect_err("must end with finish");
        assert!(
            issues
                .iter()
                .any(|i| i.code == "T1" && i.path == "flow.json:steps"),
            "{issues:?}"
        );
    }

    #[test]
    fn lint_flags_undeclared_and_dynamic_params() {
        let td = TempDir::new("lintparam");
        let b = open(&td);
        write_flow_only_bundle(
            &b,
            r#"{"schema_version":1,"name":"任务1","params":[]}"#,
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"if","args":{"cond":"param('max_rounds') > 0"},"then":[]},
                 {"op":"log","args":{"text":"param('a' + 'b')"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let issues = b.lint().expect_err("undeclared param");
        let t3: Vec<&LintIssue> = issues.iter().filter(|i| i.code == "T3").collect();
        assert_eq!(t3.len(), 2, "{issues:?}");
        assert!(
            t3.iter()
                .any(|i| i.path.contains("steps[0].args.cond") && i.msg.contains("max_rounds")),
            "{t3:?}"
        );
        assert!(
            t3.iter()
                .any(|i| i.path.contains("steps[1].args.text")
                    && i.msg.contains("string-literal")),
            "{t3:?}"
        );
    }

    #[test]
    fn lint_flags_undeclared_params_used_in_text_expr() {
        let td = TempDir::new("lintexpr");
        let b = open(&td);
        write_flow_only_bundle(
            &b,
            r#"{"schema_version":1,"name":"任务1","params":[
                 {"key":"k","type":"string","default":"完成"}]}"#,
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click_target","args":{
                   "find":[{"kind":"ocr_text","sel":"O","text_expr":"param('k')"}],
                   "requires":{"kind":"ocr_text","sel":"O",
                               "text_expr":"param('missing_guard')"}}},
                 {"op":"drag","args":{
                   "from":{"kind":"ocr_text","sel":"O","text_expr":"param('a' + 'b')"},
                   "direction":"up","distance":[1,2]}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let issues = b.lint().expect_err("T3 must see text_expr param uses");
        let t3: Vec<&LintIssue> = issues.iter().filter(|i| i.code == "T3").collect();
        assert_eq!(t3.len(), 2, "{issues:?}");
        assert!(
            t3.iter().any(|i| i.path.contains("steps[0].args.requires.text_expr")
                && i.msg.contains("missing_guard")),
            "{t3:?}"
        );
        assert!(
            t3.iter()
                .any(|i| i.path.contains("steps[1].args.from.text_expr")
                    && i.msg.contains("string-literal")),
            "a computed text_expr key must be reported as undecidable: {t3:?}"
        );
        // The declared key on the locator is NOT flagged.
        assert!(
            !t3.iter().any(|i| i.path.contains("find[0]")),
            "{t3:?}"
        );
    }

    #[test]
    fn lint_covers_long_press_find_for_t3_and_t4() {
        // v1.3 (OPERATION-REGION-DESIGN §6.1): `long_press.find[]` is the same
        // locator contract as click_target — T3 must see its text_expr param
        // uses and T4 must see its asset references.
        let td = TempDir::new("lintpress");
        let b = open(&td);
        write_flow_only_bundle(
            &b,
            r#"{"schema_version":1,"name":"任务1","params":[
                 {"key":"k","type":"string","default":"完成"}]}"#,
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"long_press","args":{"find":[
                    {"kind":"ocr_text","sel":"O_DECLARED","text_expr":"param('k')"},
                    {"kind":"ocr_text","sel":"O_MISSING","text_expr":"param('missing_key')"}]}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let issues = b.lint().expect_err("long_press lint issues expected");
        let t3: Vec<&LintIssue> = issues.iter().filter(|i| i.code == "T3").collect();
        assert_eq!(t3.len(), 1, "{issues:?}");
        assert!(
            t3[0]
                .path
                .contains("steps[0].args.find[1].text_expr")
                && t3[0].msg.contains("missing_key"),
            "{t3:?}"
        );
        // T4: both OCR selectors (and the missing one in particular) are
        // referenced assets, exactly like a click_target chain.
        let t4: Vec<&LintIssue> = issues.iter().filter(|i| i.code == "T4").collect();
        assert!(
            t4.iter().any(|i| i.msg.contains("O_DECLARED")),
            "long_press sel must be collected for T4: {t4:?}"
        );
        assert!(
            !t3.iter().any(|i| i.path.contains("find[0].text_expr")),
            "the declared key must not be flagged: {t3:?}"
        );
    }

    #[test]
    fn lint_collects_region_asset_for_t4() {
        // v1.4 A-plan (OPERATION-REGION-DESIGN §7): `region.asset` names a
        // whole-library operation-area asset, so T4 must validate it exists.
        let td = TempDir::new("lintregasset");
        let b = open(&td);
        write_flow_only_bundle(
            &b,
            r#"{"schema_version":1,"name":"任务1","params":[]}"#,
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click_target","args":{"find":[
                    {"kind":"ocr_text","sel":"O","text":"完成",
                     "region":{"mode":"nearest","asset":"I_MISSING"}}]}},
                 {"op":"drag","args":{
                    "from":{"kind":"ocr_text","sel":"O","text":"返回",
                            "region":{"mode":"asset","asset":"I_MISSING_2"}},
                    "direction":"up","distance":[1,2]}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let issues = b.lint().expect_err("T4 must see region.asset references");
        let t4: Vec<&LintIssue> = issues.iter().filter(|i| i.code == "T4").collect();
        assert!(
            t4.iter().any(|i| i.msg.contains("I_MISSING")),
            "click_target region.asset must be referenced for T4: {t4:?}"
        );
        assert!(
            t4.iter().any(|i| i.msg.contains("I_MISSING_2")),
            "drag region.asset must be referenced for T4: {t4:?}"
        );
        // region.asset does not introduce parameters (T3 untouched).
        assert!(
            !issues.iter().any(|i| i.code == "T3"),
            "region.asset must not create T3 issues: {issues:?}"
        );
    }

    #[test]
    fn lint_flags_asset_refs_and_missing_images() {
        let td = TempDir::new("lintassets");
        let b = open(&td);
        // T4: the flow references an asset that assets.json does not declare.
        write_flow_only_bundle(
            &b,
            r#"{"schema_version":1,"name":"任务1"}"#,
            &flow_json("不存在"),
        );
        let issues = b.lint().expect_err("undeclared asset reference");
        assert!(
            issues
                .iter()
                .any(|i| i.code == "T4" && i.msg.contains("不存在")),
            "{issues:?}"
        );

        // T5: a declared template whose image file is not on disk.
        let bad = serde_json::json!({
            "schema_version": 1,
            "items": [{
                "id": "返回按钮", "kind": "template",
                "file": "images/a999.png", "roi": [0, 0, 8, 8]
            }]
        });
        std::fs::write(b.assets_path(), serde_json::to_string(&bad).unwrap()).expect("assets");
        std::fs::write(b.flow_path(), flow_json("返回按钮")).expect("flow");
        let issues = b.lint().expect_err("missing image");
        assert!(
            issues
                .iter()
                .any(|i| i.code == "T5" && i.path == "assets.json: items[0].file"),
            "{issues:?}"
        );
    }

    #[test]
    fn lint_flags_external_code_keys() {
        let td = TempDir::new("lintcode");
        let b = open(&td);
        // The DSL parser ignores unknown args, so T6 must catch the escape hatch.
        write_flow_only_bundle(
            &b,
            r#"{"schema_version":1,"name":"任务1"}"#,
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"log","args":{"text":"'hi'","code":"os.system('x')"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let issues = b.lint().expect_err("external code must be rejected");
        assert!(
            issues
                .iter()
                .any(|i| i.code == "T6" && i.path.contains("args.code")),
            "{issues:?}"
        );
    }

    #[test]
    fn lint_t6_ignores_parameter_names_but_flags_code_expressions() {
        let td = TempDir::new("lintcode-params");
        let tasks = td.path().join("tasks");
        std::fs::create_dir_all(&tasks).expect("tasks dir");
        write_flow_only_bundle(
            &TaskBundle::open(tasks.join("子任务")).expect("target"),
            r#"{"schema_version":1,"name":"子任务","params":[
                 {"key":"code","type":"string","default":"x"}]}"#,
            r#"{"schema_version":1,"name":"子任务","steps":[
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let parent = TaskBundle::open(tasks.join("父任务")).expect("parent");
        write_flow_only_bundle(
            &parent,
            r#"{"schema_version":1,"name":"父任务","params":[
                 {"key":"parent_code","type":"string","default":"x"}]}"#,
            r#"{"schema_version":1,"name":"父任务","steps":[
                 {"op":"call_task","args":{"task":"子任务",
                   "params":{"code":"param('parent_code')"}}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        // P1-1: `code` is a legal subtask parameter name, not external code.
        let issues = lint_task(&tasks, "父任务").expect("lint");
        assert!(
            issues.iter().all(|i| i.code != "T6"),
            "parameter names must not participate in T6: {issues:?}"
        );
        assert!(issues.is_empty(), "{issues:?}");

        // A real external-code escape in the *value* is still T6.
        std::fs::write(
            parent.flow_path(),
            r#"{"schema_version":1,"name":"父任务","steps":[
                 {"op":"call_task","args":{"task":"子任务",
                   "params":{"code":"shell: rm -rf"}}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        )
        .expect("rewrite parent flow");
        let issues = lint_task(&tasks, "父任务").expect("lint");
        assert!(
            issues
                .iter()
                .any(|i| i.code == "T6"
                    && i.path.ends_with(".params.code")
                    && i.msg.contains("external-code")),
            "a shell expression value must stay a T6 violation: {issues:?}"
        );
    }

    #[test]
    fn lint_t7_accepts_a_valid_cross_task_mapping() {
        let td = TempDir::new("t7ok");
        let tasks = td.path().join("tasks");
        std::fs::create_dir_all(&tasks).expect("tasks dir");
        write_flow_only_bundle(
            &TaskBundle::open(tasks.join("子任务")).expect("target"),
            r#"{"schema_version":1,"name":"子任务","params":[
                 {"key":"trigger_text","type":"string","default":"默认"}]}"#,
            r#"{"schema_version":1,"name":"子任务","steps":[
                 {"op":"log","args":{"text":"param('trigger_text')"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        write_flow_only_bundle(
            &TaskBundle::open(tasks.join("父任务")).expect("parent"),
            r#"{"schema_version":1,"name":"父任务","params":[
                 {"key":"keyword","type":"string","default":"突破"}]}"#,
            r#"{"schema_version":1,"name":"父任务","steps":[
                 {"op":"call_task","args":{"task":"子任务",
                   "params":{"trigger_text":"param('keyword')"}}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let issues = lint_task(&tasks, "父任务").expect("lint");
        assert!(issues.is_empty(), "{issues:?}");
        // --all path uses the same rule.
        let all = lint_tasks(&tasks).expect("lint all");
        assert!(
            all.iter().all(|(_, issues)| issues.is_empty()),
            "{all:?}"
        );
    }

    #[test]
    fn t3_uses_variables_and_reports_legacy_params_as_warnings() {
        let td = TempDir::new("t3vars");
        let b = open(&td);
        // Declared V2 variable: green.
        write_flow_only_bundle(
            &b,
            r#"{"schema_version":1,"name":"任务1","variables":[
                 {"key":"k","type":"string","default":"完成"}]}"#,
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"log","args":{"text":"param('k')"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        b.lint().expect("declared variable is green");
        assert!(b.lint_warnings().is_empty(), "{:?}", b.lint_warnings());
        // Undeclared: T3 names meta.variables (not meta.params).
        write_flow_only_bundle(
            &b,
            r#"{"schema_version":1,"name":"任务1","variables":[
                 {"key":"other","type":"string","default":"x"}]}"#,
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"log","args":{"text":"param('k')"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let issues = b.lint().expect_err("undeclared variable");
        let t3: Vec<&LintIssue> = issues.iter().filter(|i| i.code == "T3").collect();
        assert_eq!(t3.len(), 1, "{issues:?}");
        assert!(t3[0].msg.contains("meta.variables"), "{}", t3[0]);
        assert!(!t3[0].msg.contains("meta.params"), "{}", t3[0]);
        // Legacy params-only task: lint stays green (in-memory bridge) but T10
        // warns non-blockingly.
        write_flow_only_bundle(
            &b,
            r#"{"schema_version":1,"name":"任务1","params":[
                 {"key":"k","type":"string","default":"完成"}]}"#,
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"log","args":{"text":"param('k')"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        b.lint().expect("legacy params bridge keeps lint green");
        let warnings = b.lint_warnings();
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert_eq!(warnings[0].code, "T10");
        assert_eq!(warnings[0].path, "meta.json:params");
    }

    #[test]
    fn lint_t7_uses_child_and_parent_variables() {
        let td = TempDir::new("t7vars");
        let tasks = td.path().join("tasks");
        std::fs::create_dir_all(&tasks).expect("tasks dir");
        write_flow_only_bundle(
            &TaskBundle::open(tasks.join("子任务")).expect("target"),
            r#"{"schema_version":1,"name":"子任务","variables":[
                 {"key":"trigger_text","type":"string","default":"默认"},
                 {"key":"child_count","type":"int","default":2},
                 {"key":"runtime_only","type":"int","scope":"runtime","default":0}]}"#,
            r#"{"schema_version":1,"name":"子任务","steps":[
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        write_flow_only_bundle(
            &TaskBundle::open(tasks.join("父任务")).expect("parent"),
            r#"{"schema_version":1,"name":"父任务","variables":[
                 {"key":"keyword","type":"string","default":"突破"}]}"#,
            r#"{"schema_version":1,"name":"父任务","steps":[
                 {"op":"call_task","args":{"task":"子任务","params":{
                   "trigger_text":"param('keyword')","child_count":"2"}}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let issues = lint_task(&tasks, "父任务").expect("lint");
        assert!(issues.is_empty(), "{issues:?}");
        // A mapping to a runtime-only child variable is not a valid input key.
        write_flow_only_bundle(
            &TaskBundle::open(tasks.join("父任务")).expect("parent"),
            r#"{"schema_version":1,"name":"父任务","variables":[
                 {"key":"keyword","type":"string","default":"突破"}]}"#,
            r#"{"schema_version":1,"name":"父任务","steps":[
                 {"op":"call_task","args":{"task":"子任务","params":{
                   "runtime_only":"1"}}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let issues = lint_task(&tasks, "父任务").expect("lint");
        let t7: Vec<&LintIssue> = issues.iter().filter(|i| i.code == "T7").collect();
        assert_eq!(t7.len(), 1, "{issues:?}");
        assert!(t7[0].msg.contains("runtime_only"), "{}", t7[0]);
    }

    #[test]
    fn lint_t7_flags_a_missing_target_task() {
        let td = TempDir::new("t7missing");
        let tasks = td.path().join("tasks");
        std::fs::create_dir_all(&tasks).expect("tasks dir");
        write_flow_only_bundle(
            &TaskBundle::open(tasks.join("父任务")).expect("parent"),
            r#"{"schema_version":1,"name":"父任务"}"#,
            r#"{"schema_version":1,"name":"父任务","steps":[
                 {"op":"call_task","args":{"task":"不存在"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let issues = lint_task(&tasks, "父任务").expect("lint");
        let t7: Vec<&LintIssue> = issues.iter().filter(|i| i.code == "T7").collect();
        assert_eq!(t7.len(), 1, "{issues:?}");
        assert!(t7[0].msg.contains("不存在"), "{:?}", t7[0]);
        assert!(t7[0].path.starts_with("flow.json:steps[0].args"), "{}", t7[0]);
    }

    #[test]
    fn lint_t7_flags_keys_each_side_has_not_declared() {
        let td = TempDir::new("t7keys");
        let tasks = td.path().join("tasks");
        std::fs::create_dir_all(&tasks).expect("tasks dir");
        write_flow_only_bundle(
            &TaskBundle::open(tasks.join("子任务")).expect("target"),
            r#"{"schema_version":1,"name":"子任务","params":[
                 {"key":"触发文字","type":"string","default":"默认"}]}"#,
            r#"{"schema_version":1,"name":"子任务","steps":[
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        write_flow_only_bundle(
            &TaskBundle::open(tasks.join("父任务")).expect("parent"),
            r#"{"schema_version":1,"name":"父任务","params":[]}"#,
            r#"{"schema_version":1,"name":"父任务","steps":[
                 {"op":"call_task","args":{"task":"子任务","params":{
                   "子任务没声明的":"param('父任务没声明的')"}}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let issues = lint_task(&tasks, "父任务").expect("lint");
        let t7: Vec<&LintIssue> = issues.iter().filter(|i| i.code == "T7").collect();
        assert_eq!(t7.len(), 2, "{issues:?}");
        assert!(
            t7.iter()
                .any(|i| i.msg.contains("子任务 `子任务` 未声明参数")
                    && i.path.ends_with(".params.子任务没声明的")),
            "{t7:?}"
        );
        assert!(
            t7.iter()
                .any(|i| i.msg.contains("父任务未声明参数") && i.msg.contains("父任务没声明的")),
            "{t7:?}"
        );
    }

    #[test]
    fn lint_t7_accepts_constant_mapping_but_rejects_computed_param_keys() {
        let td = TempDir::new("t7expr");
        let tasks = td.path().join("tasks");
        std::fs::create_dir_all(&tasks).expect("tasks dir");
        write_flow_only_bundle(
            &TaskBundle::open(tasks.join("子任务")).expect("target"),
            r#"{"schema_version":1,"name":"子任务","params":[
                 {"key":"触发文字","type":"string"}]}"#,
            r#"{"schema_version":1,"name":"子任务","steps":[
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let parent = TaskBundle::open(tasks.join("父任务")).expect("parent");
        write_flow_only_bundle(
            &parent,
            r#"{"schema_version":1,"name":"父任务"}"#,
            r#"{"schema_version":1,"name":"父任务","steps":[
                 {"op":"call_task","args":{"task":"子任务",
                   "params":{"触发文字":"'常量'"}}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        // P2: constants need no parent declaration and are legal.
        let issues = lint_task(&tasks, "父任务").expect("lint");
        assert!(issues.is_empty(), "constant mapping must pass: {issues:?}");

        // A computed param() key cannot be checked statically -> T7.
        std::fs::write(
            parent.flow_path(),
            r#"{"schema_version":1,"name":"父任务","steps":[
                 {"op":"call_task","args":{"task":"子任务",
                   "params":{"触发文字":"param('a' + 'b')"}}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        )
        .expect("rewrite parent flow");
        let issues = lint_task(&tasks, "父任务").expect("lint");
        let t7: Vec<&LintIssue> = issues.iter().filter(|i| i.code == "T7").collect();
        assert_eq!(t7.len(), 1, "{issues:?}");
        assert!(t7[0].msg.contains("只支持"), "{}", t7[0]);
        assert!(
            t7[0].path.ends_with(".params.触发文字"),
            "{}",
            t7[0]
        );
    }

    #[test]
    fn lint_t7_reports_an_unparseable_target_flow() {
        let td = TempDir::new("t7badflow");
        let tasks = td.path().join("tasks");
        std::fs::create_dir_all(tasks.join("坏目标")).expect("dir");
        std::fs::write(
            tasks.join("坏目标").join("meta.json"),
            r#"{"schema_version":1,"name":"坏目标","params":[]}"#,
        )
        .expect("meta");
        std::fs::write(
            tasks.join("坏目标").join("flow.json"),
            r#"{"schema_version":1,"name":"坏目标","steps":[{"op":"nope","args":{}}]}"#,
        )
        .expect("flow");
        write_flow_only_bundle(
            &TaskBundle::open(tasks.join("父任务")).expect("parent"),
            r#"{"schema_version":1,"name":"父任务"}"#,
            r#"{"schema_version":1,"name":"父任务","steps":[
                 {"op":"call_task","args":{"task":"坏目标"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        // P2: T7 parses the target flow, not just "is it readable".
        let issues = lint_task(&tasks, "父任务").expect("lint");
        let t7: Vec<&LintIssue> = issues.iter().filter(|i| i.code == "T7").collect();
        assert_eq!(t7.len(), 1, "{issues:?}");
        assert!(
            t7[0].msg.contains("无法解析目标 flow.json"),
            "{:?}",
            t7[0]
        );
        assert!(t7[0].msg.contains("nope"), "{:?}", t7[0]);
    }

    #[test]
    fn lint_t7_distinguishes_missing_meta_from_missing_flow() {
        let td = TempDir::new("t7broken");
        let tasks = td.path().join("tasks");
        std::fs::create_dir_all(&tasks).expect("tasks dir");
        std::fs::create_dir_all(tasks.join("只有flow")).expect("dir");
        std::fs::write(
            tasks.join("只有flow").join("flow.json"),
            r#"{"schema_version":1,"name":"只有flow","steps":[{"op":"finish","args":{"exit":"done"}}]}"#,
        )
        .expect("flow");
        std::fs::create_dir_all(tasks.join("只有meta")).expect("dir");
        std::fs::write(
            tasks.join("只有meta").join("meta.json"),
            r#"{"schema_version":1,"name":"只有meta"}"#,
        )
        .expect("meta");
        write_flow_only_bundle(
            &TaskBundle::open(tasks.join("父任务")).expect("parent"),
            r#"{"schema_version":1,"name":"父任务"}"#,
            r#"{"schema_version":1,"name":"父任务","steps":[
                 {"op":"call_task","args":{"task":"只有flow"}},
                 {"op":"call_task","args":{"task":"只有meta"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let issues = lint_task(&tasks, "父任务").expect("lint");
        let t7: Vec<&LintIssue> = issues.iter().filter(|i| i.code == "T7").collect();
        assert_eq!(t7.len(), 2, "{issues:?}");
        assert!(
            t7.iter()
                .any(|i| i.msg.contains("缺少 meta.json") && i.path.contains("steps[0]")),
            "{t7:?}"
        );
        assert!(
            t7.iter()
                .any(|i| i.msg.contains("缺少 flow.json") && i.path.contains("steps[1]")),
            "{t7:?}"
        );
    }

    #[test]
    fn lint_t7_stays_out_of_t3_and_out_of_the_self_contained_lint() {
        let td = TempDir::new("t7t3");
        let tasks = td.path().join("tasks");
        std::fs::create_dir_all(&tasks).expect("tasks dir");
        write_flow_only_bundle(
            &TaskBundle::open(tasks.join("子任务")).expect("target"),
            r#"{"schema_version":1,"name":"子任务","params":[
                 {"key":"k","type":"string"}]}"#,
            r#"{"schema_version":1,"name":"子任务","steps":[
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let parent = TaskBundle::open(tasks.join("父任务")).expect("parent");
        write_flow_only_bundle(
            &parent,
            r#"{"schema_version":1,"name":"父任务","params":[]}"#,
            r#"{"schema_version":1,"name":"父任务","steps":[
                 {"op":"call_task","args":{"task":"子任务","params":{"k":"param('parent_key')"}}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        // T3 must not flag the mapping's parent key: call_task is T7's leaf.
        let issues = lint_task(&tasks, "父任务").expect("lint");
        assert!(
            !issues.iter().any(|i| i.code == "T3"),
            "T3 must stay unchanged for call_task mappings: {issues:?}"
        );
        assert!(
            issues
                .iter()
                .any(|i| i.code == "T7" && i.msg.contains("父任务未声明参数")),
            "{issues:?}"
        );
        // Without a tasks dir the self-contained lint does not fire T7.
        parent.lint().unwrap_or_else(|issues| {
            panic!("T7 requires tasks_dir and must be skipped here: {issues:?}")
        });
    }

    #[test]
    fn lint_t7_rejects_an_invalid_target_name() {
        let td = TempDir::new("t7name");
        let tasks = td.path().join("tasks");
        std::fs::create_dir_all(&tasks).expect("tasks dir");
        write_flow_only_bundle(
            &TaskBundle::open(tasks.join("父任务")).expect("parent"),
            r#"{"schema_version":1,"name":"父任务"}"#,
            r#"{"schema_version":1,"name":"父任务","steps":[
                 {"op":"call_task","args":{"task":"../evil"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let issues = lint_task(&tasks, "父任务").expect("lint");
        let t7: Vec<&LintIssue> = issues.iter().filter(|i| i.code == "T7").collect();
        assert_eq!(t7.len(), 1, "{issues:?}");
        assert!(t7[0].msg.contains("名称非法"), "{:?}", t7[0]);
    }

    #[test]
    fn lint_tasks_reports_every_bundle_and_the_broken_one() {
        let td = TempDir::new("lintall");
        let tasks = td.path().join("tasks");
        std::fs::create_dir_all(&tasks).expect("tasks dir");
        let good = TaskBundle::open(tasks.join("好任务")).expect("open good");
        good.save_asset(
            AssetDecl::template("返回按钮", [0, 0, 8, 8]),
            Some(&frame(32, 32)),
        )
        .expect("asset");
        good.save_flow(&flow_json("返回按钮")).expect("flow");
        // A directory without meta.json is a broken bundle (reported, not skipped).
        let broken = tasks.join("坏任务");
        std::fs::create_dir_all(&broken).expect("broken dir");
        std::fs::write(broken.join("flow.json"), flow_json("x")).expect("broken flow");
        // A staging dir must be ignored.
        std::fs::create_dir_all(tasks.join(".oasd-new-1")).expect("staging dir");

        let all = lint_tasks(&tasks).expect("lint all");
        assert_eq!(all.len(), 2, "{all:?}");
        let get = |name: &str| {
            all.iter()
                .find(|(n, _)| n == name)
                .unwrap_or_else(|| panic!("{name} missing from {all:?}"))
        };
        assert!(get("好任务").1.is_empty(), "{:?}", get("好任务").1);
        assert!(
            get("坏任务").1.iter().any(|i| i.path == "meta.json"),
            "{:?}",
            get("坏任务").1
        );

        assert!(lint_task(&tasks, "好任务").expect("lint one").is_empty());
        assert!(lint_task(&tasks, "不存在").is_err(), "missing dir is a setup error");
        assert!(lint_task(&tasks, "../evil").is_err(), "traversal rejected");
    }

    // ---- B-05/B-07/B-08/B-10: exact action geometry validation -------------

    #[test]
    fn action_shape_contains_uses_the_runtime_f64_edge_test() {
        // B-07: the old `.max(1)` integer formula returned false for points
        // inside this triangle's negative-slope edge.
        let tri = ActionShape {
            kind: "polygon".into(),
            roi: None,
            points: Some(vec![[100, 100], [400, 100], [100, 400]]),
            invert: false,
        };
        assert!(tri.contains(120, 120));
        assert!(tri.contains(200, 180));
        assert!(!tri.contains(350, 350));
    }

    #[test]
    fn action_shape_out_of_bounds_and_zero_area_are_rejected_on_save() {
        // B-05: the save path must reject geometry the runtime would have to
        // clamp onto the screen edge.
        let td = TempDir::new("shape-bounds");
        let b = open(&td);
        let f = frame(64, 48);
        b.save_asset(AssetDecl::template("底图", [0, 0, 8, 8]), Some(&f))
            .expect("baseline template sets design_resolution");
        let before = std::fs::read_to_string(b.assets_path()).expect("assets bytes");
        let ocr = |id: &str, shape: ActionShape| AssetDecl {
            id: id.to_string(),
            kind: "ocr_line".to_string(),
            file: None,
            roi: Some([10, 10, 10, 10]),
            action_shape: Some(shape),
            action_source: "fixed".to_string(),
            ocr_mode: None,
            ocr_source_roi: None,
            ocr_text: None,
            threshold: None,
            polygon: None,
            rev: None,
            positions: Vec::new(),
            ..AssetDecl::template("_", [0, 0, 1, 1])
        };

        let err = b
            .save_asset(ocr("越界", ActionShape::rect([100, 100, 10, 10])), Some(&f))
            .expect_err("out-of-frame rect must be rejected");
        assert!(err.contains("outside the 64x48 frame"), "{err}");
        assert_eq!(
            std::fs::read_to_string(b.assets_path()).expect("assets bytes"),
            before,
            "rejected shape must not write assets.json"
        );

        let collinear = ActionShape {
            kind: "polygon".into(),
            roi: None,
            points: Some(vec![[10, 10], [20, 10], [30, 10]]),
            invert: false,
        };
        let err = b
            .save_asset(ocr("共线", collinear), Some(&f))
            .expect_err("collinear polygon must be rejected");
        assert!(err.contains("zero area"), "{err}");

        let legal = ActionShape {
            kind: "polygon".into(),
            roi: None,
            points: Some(vec![[5, 5], [30, 5], [5, 30]]),
            invert: false,
        };
        b.save_asset(ocr("合法", legal), Some(&f))
            .expect("in-frame non-degenerate polygon is valid");
    }

    #[test]
    fn action_shape_out_of_bounds_is_rejected_on_load_too() {
        // B-05: load must apply the same validation as save (no bypass by
        // editing assets.json by hand).
        let td = TempDir::new("shape-load");
        let b = open(&td);
        let f = frame(64, 48);
        b.save_asset(AssetDecl::template("底图", [0, 0, 8, 8]), Some(&f))
            .expect("baseline");
        let mut af = b.load_assets_file().expect("assets");
        af.items.push(AssetDecl {
            id: "手改越界".to_string(),
            kind: "ocr_line".to_string(),
            file: None,
            roi: Some([10, 10, 10, 10]),
            action_shape: Some(ActionShape::rect([100, 100, 10, 10])),
            action_source: "fixed".to_string(),
            ocr_mode: None,
            ocr_source_roi: None,
            ocr_text: None,
            threshold: None,
            polygon: None,
            rev: None,
            positions: Vec::new(),
            ..AssetDecl::template("_", [0, 0, 1, 1])
        });
        std::fs::write(
            b.assets_path(),
            serde_json::to_string_pretty(&af).expect("json"),
        )
        .expect("write assets");
        let err = b.load_assets_file().expect_err("load must reject the shape");
        assert!(err.contains("outside the 64x48 frame"), "{err}");
    }

    #[test]
    fn region_action_shape_update_is_allowed_and_syncs_roi() {
        // B-08: region objects failed every action_shape update; they must
        // accept it (region geometry IS the operation region).
        let td = TempDir::new("region-update");
        let b = open(&td);
        let f = frame(64, 48);
        b.save_asset(AssetDecl::template("底图", [0, 0, 8, 8]), Some(&f))
            .expect("baseline");
        let mut region = AssetDecl::template("列表", [10, 20, 30, 20]);
        region.kind = "region".to_string();
        region.file = None;
        region.threshold = None;
        region.action_shape = None;
        region.action_source = "recognition".to_string();
        b.save_asset(region, Some(&f)).expect("region save");

        let poly = ActionShape {
            kind: "polygon".into(),
            roi: None,
            points: Some(vec![[5, 5], [25, 5], [25, 25], [5, 25]]),
            invert: false,
        };
        let updated = b
            .update_asset_meta("列表", None, Some(poly), None, None, false)
            .expect("region action_shape update must be accepted");
        assert_eq!(updated.action_source, "recognition");
        assert_eq!(updated.roi, Some([5, 5, 21, 21]));
        assert_eq!(
            updated.action_shape.as_ref().and_then(|s| s.points.as_ref()).map(|p| p.len()),
            Some(4)
        );

        let err = b
            .update_asset_meta(
                "列表",
                None,
                Some(ActionShape::rect([100, 100, 10, 10])),
                None,
                None,
                false,
            )
            .expect_err("out-of-frame region update must fail");
        assert!(err.contains("outside"), "{err}");
        let af = b.load_assets_file().expect("assets");
        assert_eq!(af.items[1].roi, Some([5, 5, 21, 21]), "failed update writes nothing");

        // Updating only the region roi must re-derive the operation shape too.
        let updated = b
            .update_asset_meta("列表", Some([10, 10, 20, 10]), None, None, None, false)
            .expect("region roi update");
        assert_eq!(updated.roi, Some([10, 10, 20, 10]));
        assert_eq!(
            updated.action_shape.as_ref().and_then(|s| s.bbox()),
            Some([10, 10, 20, 10])
        );

        // P0b: the deprecated `action_source` argument is accepted but ignored —
        // it can neither reject the update nor clear the operation shape.
        let kept = b
            .update_asset_meta(
                "列表",
                Some([12, 12, 20, 10]),
                None,
                Some("recognition".into()),
                None,
                true,
            )
            .expect("deprecated action_source argument is ignored");
        assert_eq!(kept.roi, Some([12, 12, 20, 10]));
        assert_eq!(
            kept.action_shape.as_ref().and_then(|s| s.bbox()),
            Some([10, 10, 20, 10]),
            "an old payload's action_source must not clear the shape any more"
        );
        assert_eq!(kept.action_source, "recognition");
    }

    #[test]
    fn region_roi_save_with_keep_action_shape_preserves_the_polygon() {
        // B-08b (leftover 1): the editor's 识别区 save must not downgrade a
        // region's operation polygon to `rect(roi)`. `keep_action_shape: true`
        // means "only update the persisted ROI"; the operation shape is
        // untouched.
        let td = TempDir::new("region-keep-shape");
        let b = open(&td);
        let f = frame(200, 200);
        b.save_asset(AssetDecl::template("底图", [0, 0, 8, 8]), Some(&f))
            .expect("baseline");
        let poly = ActionShape {
            kind: "polygon".into(),
            roi: None,
            points: Some(vec![[5, 5], [25, 5], [25, 25], [5, 25]]),
            invert: false,
        };
        let mut region = AssetDecl::template("列表", [5, 5, 21, 21]);
        region.kind = "region".to_string();
        region.file = None;
        region.threshold = None;
        region.action_shape = Some(poly.clone());
        region.action_source = "fixed".to_string();
        b.save_asset(region, Some(&f)).expect("region save");

        // Move the recognition ROI only.
        let updated = b
            .update_asset_meta("列表", Some([40, 50, 30, 20]), None, None, None, true)
            .expect("region roi update with keep_action_shape");
        assert_eq!(updated.roi, Some([40, 50, 30, 20]));
        assert_eq!(
            updated.action_shape.as_ref().and_then(|s| s.points.clone()),
            poly.points,
            "the polygon operation shape must survive a ROI-only edit"
        );
        assert_eq!(updated.action_shape.as_ref().map(|s| s.kind.as_str()), Some("polygon"));
        assert_eq!(updated.action_source, "recognition");
        assert_eq!(updated.rev, Some(2));

        // Persisted AND wired: the runtime keeps the polygon (and its bbox) as
        // the operation region instead of following the new ROI rectangle.
        let set = b.load_runtime_assets().expect("runtime assets");
        let shape = set.click_shape("列表").expect("region click shape");
        let pts = shape.points.as_ref().expect("polygon points wired");
        assert_eq!(pts.len(), 4);
        assert_eq!((pts[2].x, pts[2].y), (25, 25));
        assert_eq!(shape.rect, None, "no rectangle may replace the polygon");
        assert_eq!(
            set.region("列表").map(|r| (r.x, r.y, r.w, r.h)),
            Some((5, 5, 21, 21)),
            "the region bbox still comes from action_shape, not the moved ROI"
        );

        // The ROI-only path must not be able to persist an unusable fallback.
        let err = b
            .update_asset_meta("列表", Some([200, 200, 10, 10]), None, None, None, true)
            .expect_err("out-of-frame region roi must be rejected even with keep_action_shape");
        assert!(err.contains("outside the 200x200 frame"), "{err}");
        let af = b.load_assets_file().expect("assets");
        assert_eq!(
            af.items[1].roi,
            Some([40, 50, 30, 20]),
            "a rejected ROI-only update writes nothing"
        );
    }

    #[test]
    fn region_roi_save_without_keep_action_shape_keeps_b08_rect_sync() {
        // Backward compatibility (B-08): absent/false `keep_action_shape` must
        // keep the old behaviour — a region ROI edit rewrites the operation
        // shape to `rect(roi)`.
        let td = TempDir::new("region-b08-compat");
        let b = open(&td);
        let f = frame(200, 200);
        b.save_asset(AssetDecl::template("底图", [0, 0, 8, 8]), Some(&f))
            .expect("baseline");
        let mut region = AssetDecl::template("列表", [5, 5, 21, 21]);
        region.kind = "region".to_string();
        region.file = None;
        region.threshold = None;
        region.action_shape = Some(ActionShape {
            kind: "polygon".into(),
            roi: None,
            points: Some(vec![[5, 5], [25, 5], [25, 25], [5, 25]]),
            invert: false,
        });
        region.action_source = "fixed".to_string();
        b.save_asset(region, Some(&f)).expect("region save");

        let updated = b
            .update_asset_meta("列表", Some([10, 10, 20, 10]), None, None, None, false)
            .expect("legacy B-08 region roi update");
        assert_eq!(
            updated.action_shape,
            Some(ActionShape::rect([10, 10, 20, 10])),
            "without the flag the historical rect(roi) sync stays in effect"
        );
        assert_eq!(updated.action_source, "recognition");
    }

    #[test]
    fn template_overwrite_with_translated_polygon_keeps_the_shape() {
        // Leftover 2: moving a template's ROI must move its recognition polygon
        // (pure translation), not replace it with `rect(new roi)`. The server
        // derives `roi` from the polygon bbox, so a same-size translation
        // round-trips the new ROI while every vertex keeps its relative shape.
        let td = TempDir::new("poly-shift");
        let b = open(&td);
        let f = frame(200, 200);
        // Non-rectangular 4-point shape whose bbox is exactly the ROI (30x30).
        let poly = vec![[10, 20], [39, 26], [39, 49], [10, 43]];
        let mut decl = AssetDecl::template("移动按钮", [10, 20, 30, 30]);
        decl.polygon = Some(poly.clone());
        let saved = b.save_asset(decl, Some(&f)).expect("save polygon template");
        assert_eq!(saved.roi, Some([10, 20, 30, 30]), "roi = polygon bbox");

        // UI payload: roi moved by (+40, +50), polygon translated by the same
        // delta (same point count, same shape, bbox = the new ROI).
        let moved: Vec<[i32; 2]> = poly.iter().map(|p| [p[0] + 40, p[1] + 50]).collect();
        let mut upd = AssetDecl::template("移动按钮", [50, 70, 30, 30]);
        upd.polygon = Some(moved.clone());
        let saved = b.update_asset(upd, Some(&f)).expect("overwrite moved polygon");
        assert_eq!(saved.roi, Some([50, 70, 30, 30]));
        assert_eq!(saved.rev, Some(2));
        assert_eq!(
            saved.polygon.as_ref(),
            Some(&moved),
            "every vertex must be translated, not replaced by a rectangle"
        );
        assert_eq!(saved.polygon.as_ref().map(|p| p.len()), Some(4));
        let set = b.load_runtime_assets().expect("runtime assets");
        let tpl = set.get("移动按钮").expect("template");
        let wired = tpl.click_polygon.as_ref().expect("polygon wired");
        assert_eq!(wired.len(), 4);
        assert_eq!((wired[1].x, wired[1].y), (79, 76));

        // Contract the UI translation check relies on: the polygon bbox wins
        // over the `roi` field, so a mismatched pair would silently move the
        // ROI. Guard it with an explicit assertion (documented behaviour).
        let mut mismatch = AssetDecl::template("移动按钮", [0, 0, 5, 5]);
        mismatch.polygon = Some(moved.clone());
        let saved = b.update_asset(mismatch, Some(&f)).expect("overwrite");
        assert_eq!(
            saved.roi,
            Some([50, 70, 30, 30]),
            "save derives roi from the polygon bbox (UI must keep them in sync)"
        );
    }

    #[test]
    fn out_of_frame_translated_template_polygon_is_rejected() {
        // The UI's translation fallback exists for exactly this case: a moved
        // polygon that leaves the frame must never be sent (oas-task rejects it
        // with a clear error instead of writing anything), so the rectangle is
        // the only safe payload after a shape-losing move.
        let td = TempDir::new("poly-oob");
        let b = open(&td);
        let f = frame(200, 200);
        let mut decl = AssetDecl::template("边缘按钮", [150, 150, 40, 40]);
        decl.polygon = Some(vec![[150, 150], [189, 150], [189, 189], [150, 189]]);
        b.save_asset(decl, Some(&f)).expect("save edge template");
        let before = std::fs::read(b.assets_path()).expect("assets bytes");

        let mut bad = AssetDecl::template("边缘按钮", [210, 210, 40, 40]);
        bad.polygon = Some(vec![[210, 210], [249, 210], [249, 249], [210, 249]]);
        let err = b
            .update_asset(bad, Some(&f))
            .expect_err("out-of-frame translated polygon must be rejected");
        assert!(err.contains("outside the 200x200 frame"), "{err}");
        assert_eq!(
            std::fs::read(b.assets_path()).expect("assets bytes"),
            before,
            "a rejected save writes nothing"
        );
    }

    #[test]
    fn collection_action_shape_is_validated_and_loaded() {
        // B-10: collection-level geometry was accepted without validation and
        // ignored by the runtime loader.
        // 2026-11-06 事项6 (P0b): no `action_source` two-state rule any more — a
        // shape is optional and always allowed; the deprecated source is
        // ignored and never persisted.
        let td = TempDir::new("collection-shape");
        let b = open(&td);
        let f = frame(64, 48);
        b.save_asset(AssetDecl::template("成员甲", [0, 0, 8, 8]), Some(&f))
            .expect("member a");
        b.save_asset(AssetDecl::template("成员乙", [8, 0, 8, 8]), Some(&f))
            .expect("member b");
        let coll = |shape: Option<ActionShape>, source: &str| AssetCollection {
            id: "集合目标".into(),
            members: vec!["成员甲".into(), "成员乙".into()],
            threshold: None,
            action_shape: shape,
            action_source: source.to_string(),
        };

        // Geometry itself is still validated on save.
        let err = b
            .save_collection(coll(Some(ActionShape::rect([100, 100, 10, 10])), "fixed"))
            .expect_err("out-of-frame collection shape must be rejected");
        assert!(err.contains("outside the 64x48 frame"), "{err}");
        // No shape + the old `fixed` source is now accepted (optional shape).
        let saved = b
            .save_collection(coll(None, "fixed"))
            .expect("a collection shape is optional; action_source has no effect");
        assert_eq!(saved.action_shape, None);
        assert_eq!(saved.action_source, "recognition", "compat field is normalized");
        // A shape + the old `recognition` source is accepted too.
        let saved = b
            .save_collection(coll(Some(ActionShape::rect([10, 10, 20, 20])), "recognition"))
            .expect("a collection shape needs no source");
        assert_eq!(saved.action_source, "recognition");
        let text = std::fs::read_to_string(b.assets_path()).expect("assets json");
        assert!(
            !text.contains("action_source"),
            "the deprecated field must not be written again: {text}"
        );
        let set = b.load_runtime_assets().expect("runtime assets");
        let shape = set
            .click_shape("集合目标")
            .expect("collection shape must be wired");
        assert_eq!(
            shape.rect.map(|r| (r.x, r.y, r.w, r.h)),
            Some((10, 10, 20, 20))
        );

        // Load path: a hand-edited out-of-frame collection shape is rejected.
        let mut af = b.load_assets_file().expect("assets");
        af.collections[0].action_shape = Some(ActionShape::rect([100, 100, 10, 10]));
        std::fs::write(
            b.assets_path(),
            serde_json::to_string_pretty(&af).expect("json"),
        )
        .expect("write assets");
        let err = b.load_assets_file().expect_err("load must validate collection shape");
        assert!(err.contains("outside the 64x48 frame"), "{err}");
    }

    #[test]
    fn old_assets_json_with_the_deprecated_action_source_loads() {
        // 2026-11-06 事项6 (P0b) compatibility: old files carry `action_source`,
        // including the two combinations the removed rule used to reject. They
        // must load, keep the same runtime geometry, and the next save must stop
        // writing the field.
        let td = TempDir::new("p0b-old-json");
        let b = open(&td);
        std::fs::create_dir_all(b.root()).expect("root");
        let f = frame(64, 48);
        // Creates `images/a001.png` + `design_resolution` the old JSON references.
        b.save_asset(AssetDecl::template("底图", [0, 0, 8, 8]), Some(&f))
            .expect("seed template image");
        let old = r#"{"schema_version":1,"design_resolution":[64,48],"items":[
            {"id":"识别带形状","kind":"ocr_line","roi":[10,10,20,10],"action_shape":{"kind":"rect","roi":[12,12,10,6],"invert":false},"action_source":"recognition"},
            {"id":"固定无形状","kind":"ocr_line","roi":[40,10,20,10],"action_source":"fixed"},
            {"id":"模板甲","kind":"template","file":"images/a001.png","roi":[0,0,8,8],"action_source":"fixed"}
        ],"collections":[
            {"id":"旧集合","members":["模板甲"],"action_shape":{"kind":"rect","roi":[1,1,4,4],"invert":false},"action_source":"recognition"}
        ]}"#;
        std::fs::write(b.assets_path(), old).expect("write old assets");
        let af = b.load_assets_file().expect("old data with action_source must load");
        assert_eq!(af.items.len(), 3);
        assert_eq!(af.items[0].action_source, "recognition");
        assert_eq!(af.items[1].action_source, "fixed");
        // Runtime geometry is unchanged: a present shape wins, absent = real bbox.
        let set = b.load_runtime_assets().expect("runtime assets");
        assert!(set.click_back("识别带形状").is_some());
        assert!(set.click_back("固定无形状").is_none());
        assert!(set.click_shape("旧集合").is_some());
        // Any later save rewrites the manifest without the deprecated field.
        b.save_asset(AssetDecl::template("模板乙", [0, 0, 8, 8]), Some(&f))
            .expect("save on top of old data");
        let text = std::fs::read_to_string(b.assets_path()).expect("assets");
        assert!(!text.contains("action_source"), "{text}");
        assert!(text.contains("action_shape"), "{text}");
    }

    // -----------------------------------------------------------------------
    // 2026-11-06 参数→素材 redesign (B-T1): ocr_var model + op_region chains
    // -----------------------------------------------------------------------

    #[test]
    fn ocr_var_round_trips_and_wires_the_runtime_ocr_asset() {
        let td = TempDir::new("ocr-var-roundtrip");
        let b = open(&td);
        let f = frame(64, 48);
        b.save_asset(AssetDecl::template("底图", [0, 0, 8, 8]), Some(&f))
            .expect("baseline template sets design resolution");
        let mut decl = AssetDecl::ocr_var("O_预设分组", "group", "御魂", Some([10, 10, 40, 20]));
        decl.label = Some("预设分组".to_string());
        decl.threshold = Some(0.6);
        decl.action_shape = Some(ActionShape::rect([12, 12, 30, 20]));
        decl.positions = vec![1];
        let saved = b.save_asset(decl, None).expect("save ocr_var");
        assert_eq!(saved.kind, "ocr_var");
        assert_eq!(saved.runtime_text.as_deref(), Some("御魂"));
        assert_eq!(saved.param_key.as_deref(), Some("group"));
        assert_eq!(saved.label.as_deref(), Some("预设分组"));
        assert_eq!(saved.roi, Some([10, 10, 40, 20]));
        // Persisted round-trip: the new fields are in assets.json; the OCR
        // pixel-provenance fields are absent (ocr_var forbids them).
        let af = b.load_assets_file().expect("load assets");
        let item = af
            .items
            .iter()
            .find(|a| a.id == "O_预设分组")
            .expect("saved item");
        assert_eq!(item.runtime_text.as_deref(), Some("御魂"));
        assert_eq!(item.param_key.as_deref(), Some("group"));
        assert_eq!(item.label.as_deref(), Some("预设分组"));
        assert_eq!(item.op_region, None, "absent op_region = self (default)");
        assert_eq!(item.ocr_mode, None);
        assert_eq!(item.ocr_text, None);
        let text = std::fs::read_to_string(b.assets_path()).expect("assets json");
        assert!(text.contains("\"runtime_text\""), "{text}");
        assert!(text.contains("\"param_key\""), "{text}");
        // Runtime wiring for a ROI-scoped ocr_var: dedicated `ocr_vars` table
        // (= forced det+rec), the minimum-confidence threshold and the operation
        // geometry (B-T2 added the `ocr_var` table; it is not a plain OCR asset).
        let set = b.load_runtime_assets().expect("runtime assets");
        assert!(set.is_ocr_asset("O_预设分组"));
        assert_eq!(
            set.ocr_var("O_预设分组")
                .and_then(|v| v.roi)
                .map(|r| (r.x, r.y, r.w, r.h)),
            Some((10, 10, 40, 20))
        );
        assert!(
            set.ocr_roi("O_预设分组").is_none(),
            "an ocr_var lives in its own runtime table"
        );
        assert_eq!(set.ocr_threshold("O_预设分组"), Some(0.6));
        assert_eq!(
            set.click_back("O_预设分组").map(|r| (r.x, r.y, r.w, r.h)),
            Some((12, 12, 30, 20))
        );
    }

    #[test]
    fn ocr_var_without_roi_saves_and_loads() {
        // 2026-11-06 redesign: an ocr_var's initial recognition region may be
        // empty (= whole-screen OCR). The runtime table for it is B-T2's job;
        // the task model must already accept and persist it.
        let td = TempDir::new("ocr-var-no-roi");
        let b = open(&td);
        let saved = b
            .save_asset(AssetDecl::ocr_var("O_全屏", "full", "开始", None), None)
            .expect("save ocr_var without roi");
        assert_eq!(saved.roi, None);
        let af = b.load_assets_file().expect("load");
        assert_eq!(af.items[0].roi, None);
        // Loading the runtime set is allowed (T1 has no full-screen OCR table).
        let set = b.load_runtime_assets().expect("runtime assets");
        assert!(set.ocr_roi("O_全屏").is_none());
    }

    #[test]
    fn ocr_var_field_validation_is_loud() {
        let td = TempDir::new("ocr-var-validate");
        let b = open(&td);
        let text_201 = "字".repeat(201);
        let label_81 = "标".repeat(81);
        let cases: Vec<(AssetDecl, &str)> = vec![
            (
                {
                    let mut d = AssetDecl::ocr_var("A1", "k1", "文字", None);
                    d.file = Some("images/a001.png".into());
                    d
                },
                "file is not allowed",
            ),
            (
                {
                    let mut d = AssetDecl::ocr_var("A2", "k2", "文字", None);
                    d.polygon = Some(vec![[0, 0], [0, 0], [1, 1]]);
                    d
                },
                "repeated adjacent point",
            ),
            (
                {
                    let mut d = AssetDecl::ocr_var("A3", "k3", "文字", None);
                    d.ocr_mode = Some("lines".into());
                    d
                },
                "ocr_mode is not allowed",
            ),
            (
                {
                    let mut d = AssetDecl::ocr_var("A4", "k4", "文字", None);
                    d.runtime_text = None;
                    d
                },
                "runtime_text is required",
            ),
            (
                AssetDecl::ocr_var("A5", "k5", "   ", None),
                "must not be blank",
            ),
            (
                AssetDecl::ocr_var("A6", "k6", text_201, None),
                "<= 200 characters",
            ),
            (
                AssetDecl::ocr_var("A7", "k7", "文\u{0007}字", None),
                "control characters",
            ),
            (
                {
                    let mut d = AssetDecl::ocr_var("A8", "文字键", "文字", None);
                    d.param_key = Some("文字键".into());
                    d
                },
                "must match [A-Za-z_]",
            ),
            (
                {
                    let mut d = AssetDecl::ocr_var("A9", "k9", "文字", None);
                    d.param_key = None;
                    d
                },
                "param_key is required",
            ),
            (
                {
                    let mut d = AssetDecl::ocr_var("A10", "k10", "文字", None);
                    d.label = Some(label_81);
                    d
                },
                "<= 80 characters",
            ),
            (
                {
                    let mut d = AssetDecl::template("A11", [0, 0, 8, 8]);
                    d.runtime_text = Some("文字".into());
                    d
                },
                "only valid for kind=ocr_var",
            ),
        ];
        for (decl, needle) in cases {
            let id = decl.id.clone();
            let err = b
                .save_asset(decl, None)
                .expect_err(&format!("{id} must be rejected"));
            assert!(err.contains(needle), "{id}: {err}");
        }
        // Nothing was written by the rejected saves.
        assert!(!b.assets_path().exists());
        // A legal ocr_var still saves afterwards.
        b.save_asset(AssetDecl::ocr_var("合法", "ok_key", "御魂", None), None)
            .expect("legal ocr_var");
    }

    #[test]
    fn duplicate_ocr_var_param_keys_are_rejected_on_load() {
        // `param_key` is the `values.json` key of the derived parameter row, so
        // it must be unique per task (two ocr_vars sharing it would corrupt the
        // Runner value mapping).
        let td = TempDir::new("ocr-var-key-collision");
        let b = open(&td);
        std::fs::create_dir_all(b.root()).expect("bundle root");
        std::fs::write(
            b.assets_path(),
            r#"{"schema_version":1,"design_resolution":[1280,720],"items":[
                {"id":"O_a","kind":"ocr_var","runtime_text":"甲","param_key":"group"},
                {"id":"O_b","kind":"ocr_var","runtime_text":"乙","param_key":"group"}
            ]}"#,
        )
        .expect("write assets");
        let err = b.load_assets_file().expect_err("duplicate param_key must fail");
        assert!(err.contains("already used by ocr_var"), "{err}");
    }

    #[test]
    fn op_region_json_supports_self_and_asset_modes_only() {
        let self_op: OpRegion = serde_json::from_str(r#"{"mode":"self"}"#).expect("self");
        assert!(self_op.is_self());
        assert_eq!(self_op.mode_str(), "self");
        let by_asset: OpRegion =
            serde_json::from_str(r#"{"mode":"asset","asset":"I_战斗按钮"}"#).expect("asset");
        assert_eq!(by_asset.asset(), Some("I_战斗按钮"));
        assert_eq!(by_asset.pick(), "score", "pick defaults to score");
        assert_eq!(by_asset.mode_str(), "asset");
        // Serialization stays compact: self is a unit variant, pick=None is
        // omitted.
        assert_eq!(
            serde_json::to_value(OpRegion::self_region()).expect("json"),
            serde_json::json!({"mode":"self"})
        );
        assert_eq!(
            serde_json::to_value(OpRegion::Asset {
                asset: "B".into(),
                pick: None
            })
            .expect("json"),
            serde_json::json!({"mode":"asset","asset":"B"})
        );
        // Unknown modes are load errors; 批次B (#4 model A) removed `offset`
        // entirely, so old offset JSON must fail loudly instead of silently
        // resolving as self.
        assert!(serde_json::from_str::<OpRegion>(r#"{"mode":"ocr"}"#).is_err());
        let offset_err = serde_json::from_str::<OpRegion>(r#"{"mode":"offset","offset":[1,2,3,4]}"#)
            .expect_err("offset mode was removed in 批次B");
        assert!(offset_err.to_string().contains("offset"), "{offset_err}");
    }

    #[test]
    fn op_region_chain_validation_rejects_cycles_depth_dangling_nodes_and_bad_gates() {
        let af: AssetsFile = serde_json::from_value(serde_json::json!({
            "schema_version": 1,
            "design_resolution": [1280, 720],
            "items": [
                {"id":"A","kind":"ocr_line","roi":[0,0,100,20],
                 "op_region":{"mode":"asset","asset":"B","pick":"nearest"}},
                {"id":"B","kind":"ocr_line","roi":[0,30,100,20],
                 "op_region":{"mode":"self"}},
                {"id":"R","kind":"region","roi":[200,200,50,50]},
                {"id":"D","kind":"ocr_line","roi":[0,60,100,20],
                 "op_region":{"mode":"asset","asset":"C"}},
                {"id":"C","kind":"ocr_lines","roi":[0,90,100,20]}
            ],
            "collections": []
        }))
        .expect("assets json");
        // Model A (#3): a referenced plain OCR gate needs an expected text.
        // Fill them for the "valid" cases below.
        let af = {
            let mut af = af;
            for item in &mut af.items {
                if item.kind == "ocr_line" {
                    item.ocr_mode = Some("lines".into());
                }
                if matches!(item.id.as_str(), "B" | "D" | "C" | "A") {
                    item.ocr_text = Some(format!("text-{}", item.id));
                }
            }
            af
        };
        let op_of = |id: &str| af.items.iter().find(|a| a.id == id).unwrap().op_region.clone();
        // A -> B:self and D -> C(self) are valid recognition gates.
        assert!(validate_op_region_chain(&af, Some("A"), &op_of("A").unwrap(), "test").is_ok());
        assert!(validate_op_region_chain(&af, Some("D"), &op_of("D").unwrap(), "test").is_ok());

        // Cycle: B points back to A.
        let mut cyc = af.clone();
        cyc.items[1].op_region = Some(OpRegion::Asset {
            asset: "A".into(),
            pick: None,
        });
        let err = validate_op_region_chain(&cyc, Some("A"), &op_of("A").unwrap(), "test")
            .expect_err("cycle");
        assert!(err.contains("cycle"), "{err}");

        // Dangling reference.
        let dangling = OpRegion::Asset {
            asset: "不存在".into(),
            pick: None,
        };
        let err = validate_op_region_chain(&af, Some("A"), &dangling, "test")
            .expect_err("dangling");
        assert!(err.contains("not a declared asset"), "{err}");

        // Self reference.
        let self_ref = OpRegion::Asset {
            asset: "A".into(),
            pick: None,
        };
        let err =
            validate_op_region_chain(&af, Some("A"), &self_ref, "test").expect_err("self ref");
        assert!(err.contains("references itself"), "{err}");

        // 批次B (#4 model A): a region is not a recognition gate and cannot be a
        // chain node.
        let to_region = OpRegion::Asset {
            asset: "R".into(),
            pick: None,
        };
        let err = validate_op_region_chain(&af, Some("A"), &to_region, "test")
            .expect_err("region node");
        assert!(
            err.contains("cannot be an op_region chain node"),
            "{err}"
        );

        // A region asset still must not carry op_region itself.
        let mut bad_region = af.clone();
        bad_region.items[2].op_region = Some(OpRegion::self_region());
        let err = validate_op_region_chain(&bad_region, Some("R"), &OpRegion::self_region(), "test")
            .expect_err("region op_region");
        assert!(err.contains("must not carry op_region"), "{err}");

        // #3: a referenced plain OCR node without expected text is a loud error.
        let mut no_text = af.clone();
        no_text.items[4].ocr_text = None;
        let err = validate_op_region_chain(&no_text, Some("D"), &op_of("D").unwrap(), "test")
            .expect_err("gate without expected text");
        assert!(err.contains("has no expected text"), "{err}");

        // Structural errors: invalid pick.
        let bad_pick = OpRegion::Asset {
            asset: "B".into(),
            pick: Some("best".into()),
        };
        assert!(validate_op_region_chain(&af, Some("A"), &bad_pick, "test")
            .expect_err("pick")
            .contains("pick"));

        // 18-node chain: the 17th hop exceeds MAX_OP_REGION_DEPTH.
        let mut items = Vec::new();
        for i in 0..18 {
            let op = if i == 17 {
                serde_json::json!({"mode":"self"})
            } else {
                serde_json::json!({"mode":"asset","asset":format!("E{}", i + 1)})
            };
            items.push(serde_json::json!({
                "id": format!("E{i}"),
                "kind": "ocr_line",
                "roi": [0, 0, 10, 10],
                "ocr_mode": "lines",
                "ocr_text": format!("gate-{i}"),
                "op_region": op
            }));
        }
        let deep: AssetsFile = serde_json::from_value(serde_json::json!({
            "schema_version": 1, "design_resolution": [1280, 720],
            "items": items, "collections": []
        }))
        .expect("deep assets");
        let deep_op = deep.items[0].op_region.clone().unwrap();
        let err = validate_op_region_chain(&deep, Some("E0"), &deep_op, "test")
            .expect_err("too deep");
        assert!(err.contains("max depth"), "{err}");

        // The load path applies the same graph rules to hand-written files.
        let td = TempDir::new("op-region-load");
        let b = open(&td);
        std::fs::create_dir_all(b.root()).expect("bundle root");
        std::fs::write(
            b.assets_path(),
            r#"{"schema_version":1,"design_resolution":[1280,720],"items":[
                {"id":"R","kind":"region","roi":[10,10,20,20],
                 "op_region":{"mode":"self"}}
            ]}"#,
        )
        .expect("write assets");
        let err = b.load_assets_file().expect_err("region with op_region must fail");
        assert!(err.contains("must not carry op_region"), "{err}");
    }

    #[test]
    fn op_region_save_delete_and_rename_keep_the_chain_consistent() {
        let td = TempDir::new("op-region-crud");
        let b = open(&td);
        b.save_asset(AssetDecl::ocr_var("B", "kb", "乙", Some([0, 0, 20, 10])), None)
            .expect("node B");
        let mut a = AssetDecl::ocr_var("A", "ka", "甲", Some([0, 20, 20, 10]));
        a.op_region = Some(OpRegion::Asset {
            asset: "B".into(),
            pick: None,
        });
        b.save_asset(a, None).expect("A with a chain");
        assert_eq!(
            b.load_assets_file().expect("load").items[1]
                .op_region
                .as_ref()
                .and_then(|op| op.asset()),
            Some("B")
        );
        // A dangling chain reference is rejected before anything is written.
        let mut c = AssetDecl::ocr_var("C", "kc", "丙", None);
        c.op_region = Some(OpRegion::Asset {
            asset: "NOPE".into(),
            pick: None,
        });
        let err = b.save_asset(c, None).expect_err("dangling chain");
        assert!(err.contains("not a declared asset"), "{err}");
        // A node used by a chain cannot be deleted; after clearing the chain it
        // can. Renaming the node rewrites the reference.
        let err = b.delete_asset("B").expect_err("referenced node");
        assert!(err.contains("操作区链引用"), "{err}");
        b.rename_asset("B", "B2").expect("rename node");
        assert_eq!(
            b.load_assets_file().expect("load").items[1]
                .op_region
                .as_ref()
                .and_then(|op| op.asset()),
            Some("B2")
        );
        b.update_asset_meta_v2("A", None, None, None, false, Some(None), None, None, None)
            .expect("clear A op_region");
        b.delete_asset("B2").expect("delete now-unreferenced node");
    }

    #[test]
    fn update_asset_meta_v2_updates_ocr_var_fields_and_op_region() {
        let td = TempDir::new("update-v2");
        let b = open(&td);
        b.save_asset(
            AssetDecl::ocr_var("O_x", "key_x", "旧文字", Some([0, 0, 30, 10])),
            None,
        )
        .expect("ocr_var");
        b.save_asset(AssetDecl::template("底图", [0, 40, 8, 8]), Some(&frame(64, 48)))
            .expect("baseline");
        // runtime_text + label update.
        let updated = b
            .update_asset_meta_v2(
                "O_x",
                None,
                None,
                None,
                false,
                None,
                Some("新文字".to_string()),
                Some("显示名".to_string()),
                None,
            )
            .expect("update ocr_var text");
        assert_eq!(updated.runtime_text.as_deref(), Some("新文字"));
        assert_eq!(updated.label.as_deref(), Some("显示名"));
        // param_key is read-only after creation.
        let err = b
            .update_asset_meta_v2(
                "O_x",
                None,
                None,
                None,
                false,
                None,
                None,
                None,
                Some("other_key".to_string()),
            )
            .expect_err("param_key is read-only");
        assert!(err.contains("read-only"), "{err}");
        // Blank runtime_text is rejected.
        let err = b
            .update_asset_meta_v2(
                "O_x",
                None,
                None,
                None,
                false,
                None,
                Some("   ".to_string()),
                None,
                None,
            )
            .expect_err("blank runtime_text");
        assert!(err.contains("must not be blank"), "{err}");
        // op_region set on a template node and cleared again.
        let updated = b
            .update_asset_meta_v2(
                "底图",
                None,
                None,
                None,
                false,
                Some(Some(OpRegion::Asset {
                    asset: "O_x".into(),
                    pick: Some("nearest".into()),
                })),
                None,
                None,
                None,
            )
            .expect("set template op_region");
        assert_eq!(updated.op_region.as_ref().and_then(|op| op.asset()), Some("O_x"));
        let updated = b
            .update_asset_meta_v2("底图", None, None, None, false, Some(None), None, None, None)
            .expect("clear op_region");
        assert_eq!(updated.op_region, None);
        // Unknown fields on a non-ocr_var asset are still rejected.
        let err = b
            .update_asset_meta_v2(
                "底图",
                None,
                None,
                None,
                false,
                None,
                Some("文字".to_string()),
                None,
                None,
            )
            .expect_err("runtime_text on a template");
        assert!(err.contains("only valid for kind=ocr_var"), "{err}");
    }

    #[test]
    fn update_asset_meta_v3_sets_clears_and_validates_the_recognition_polygon() {
        let td = TempDir::new("update-v3-polygon");
        let b = open(&td);
        b.save_asset(AssetDecl::template("底图", [0, 40, 8, 8]), Some(&frame(64, 48)))
            .expect("baseline template");
        b.save_asset(
            AssetDecl::ocr_lines("O_行", [0, 0, 20, 10]),
            None,
        )
        .expect("ocr_lines");
        // Set: the polygon bbox becomes the recognition ROI (same rule as save).
        let updated = b
            .update_asset_meta_v3(
                "O_行",
                None,
                None,
                None,
                false,
                Some(Some(vec![[2, 2], [18, 2], [18, 8], [2, 8]])),
                None,
                None,
                None,
                None,
            )
            .expect("set polygon");
        assert_eq!(updated.roi, Some([2, 2, 17, 7]));
        assert_eq!(
            updated.polygon,
            Some(vec![[2, 2], [18, 2], [18, 8], [2, 8]])
        );
        // Absent (`None`) leaves the stored polygon untouched.
        let updated = b
            .update_asset_meta_v3("O_行", None, None, Some(0.5), false, None, None, None, None, None)
            .expect("threshold update keeps polygon");
        assert_eq!(
            updated.polygon,
            Some(vec![[2, 2], [18, 2], [18, 8], [2, 8]])
        );
        // Explicit `Some(None)` clears it (roi keeps the last bbox).
        let updated = b
            .update_asset_meta_v3("O_行", None, None, None, false, Some(None), None, None, None, None)
            .expect("clear polygon");
        assert_eq!(updated.polygon, None);
        assert_eq!(updated.roi, Some([2, 2, 17, 7]));
        // Validation failures are loud and nothing is written.
        for (points, needle) in [
            (vec![[70, 0], [80, 0], [80, 10]], "outside the 64x48 frame"),
            (vec![[-1, 0], [5, 0], [5, 5]], "negative"),
            (vec![[0, 0], [10, 4], [10, 0], [0, 10]], "self-intersects"),
            (vec![[0, 0], [1, 1]], ">= 3 points"),
        ] {
            let err = b
                .update_asset_meta_v3(
                    "O_行",
                    None,
                    None,
                    None,
                    false,
                    Some(Some(points)),
                    None,
                    None,
                    None,
                    None,
                )
                .expect_err("invalid polygon");
            assert!(err.contains(needle), "{needle}: {err}");
        }
        // A region has no recognition polygon.
        b.save_asset(AssetDecl::region("R", [10, 10, 5, 5]), None)
            .expect("region");
        let err = b
            .update_asset_meta_v3(
                "R",
                None,
                None,
                None,
                false,
                Some(Some(vec![[10, 10], [14, 10], [14, 14]])),
                None,
                None,
                None,
                None,
            )
            .expect_err("region polygon");
        assert!(err.contains("no recognition polygon"), "{err}");
        // Template: same-ROI polygon is allowed (no re-crop needed), a moved one
        // is rejected with the re-crop hint.
        b.update_asset_meta_v3(
            "底图",
            None,
            None,
            None,
            false,
            Some(Some(vec![[0, 40], [7, 40], [7, 47], [0, 47]])),
            None,
            None,
            None,
            None,
        )
        .expect("template polygon with unchanged ROI");
        let err = b
            .update_asset_meta_v3(
                "底图",
                None,
                None,
                None,
                false,
                Some(Some(vec![[1, 41], [7, 41], [7, 47], [1, 47]])),
                None,
                None,
                None,
                None,
            )
            .expect_err("moved template polygon");
        assert!(err.contains("must keep the existing ROI"), "{err}");
    }

    #[test]
    fn ocr_var_polygon_saves_and_registers_the_runtime_mask() {
        let td = TempDir::new("ocr-var-polygon");
        let b = open(&td);
        let mut decl = AssetDecl::ocr_var("O_队伍", "team", "队伍1", None);
        decl.polygon = Some(vec![[10, 10], [40, 10], [40, 30], [10, 30]]);
        let saved = b.save_asset(decl, None).expect("ocr_var with polygon");
        // The polygon bbox becomes the recognition ROI on save.
        assert_eq!(saved.roi, Some([10, 10, 31, 21]));
        assert_eq!(
            saved.polygon,
            Some(vec![[10, 10], [40, 10], [40, 30], [10, 30]])
        );
        // Persisted round-trip + T5 accepts it.
        let af = b.load_assets_file().expect("assets load (T5)");
        let item = af.items.iter().find(|a| a.id == "O_队伍").expect("item");
        assert_eq!(item.roi, Some([10, 10, 31, 21]));
        assert_eq!(
            item.polygon,
            Some(vec![[10, 10], [40, 10], [40, 30], [10, 30]])
        );
        // Runtime wiring: the mask + the ocr_var ROI are registered.
        let set = b.load_runtime_assets().expect("runtime assets");
        assert!(set.is_ocr_asset("O_队伍"));
        assert_eq!(
            set.ocr_var("O_队伍")
                .and_then(|v| v.roi)
                .map(|r| (r.x, r.y, r.w, r.h)),
            Some((10, 10, 31, 21))
        );
        let pts = set.ocr_polygon("O_队伍").expect("polygon mask");
        assert_eq!(pts.len(), 4);
        assert_eq!((pts[0].x, pts[0].y), (10, 10));
    }

    #[test]
    fn ocr_lines_ocr_text_is_saved_as_the_expected_text_and_validated() {
        let td = TempDir::new("ocr-lines-text");
        let b = open(&td);
        let mut decl = AssetDecl::ocr_lines("O_行", [0, 0, 40, 20]);
        decl.ocr_text = Some("开始".to_string());
        b.save_asset(decl, None).expect("ocr_lines with ocr_text");
        let set = b.load_runtime_assets().expect("runtime assets");
        // Plain OCR gate text + candidate filter come from the same table.
        assert_eq!(set.ocr_expected_text("O_行"), Some("开始"));
        assert_eq!(
            set.ocr_roi("O_行").map(|r| (r.x, r.y, r.w, r.h)),
            Some((0, 0, 40, 20))
        );
        // Blank text and non-OCR kinds are rejected.
        let mut blank = AssetDecl::ocr_lines("O_空", [0, 0, 10, 10]);
        blank.ocr_text = Some("   ".to_string());
        let err = b.save_asset(blank, None).expect_err("blank ocr_text");
        assert!(err.contains("must not be blank"), "{err}");
        let mut tpl = AssetDecl::template("底图", [0, 0, 8, 8]);
        tpl.ocr_text = Some("开始".to_string());
        let err = b
            .save_asset(tpl, Some(&frame(64, 48)))
            .expect_err("template ocr_text");
        assert!(err.contains("only valid for ocr_line/ocr_lines"), "{err}");
    }

    #[test]
    fn write_meta_keeps_a_backup_and_round_trips() {
        let td = TempDir::new("write-meta");
        let b = open(&td);
        let mut meta =
            Meta::from_json(r#"{"schema_version":1,"name":"任务1"}"#).expect("meta");
        b.write_meta(&meta).expect("first write");
        let first = std::fs::read_to_string(b.meta_path()).expect("meta");
        meta.description = Some("新描述".to_string());
        b.write_meta(&meta).expect("second write");
        let bak = b.meta_path().with_extension("json.bak");
        assert!(bak.is_file(), "meta.json.bak must be created");
        assert_eq!(std::fs::read_to_string(&bak).expect("bak"), first);
        assert_eq!(
            b.load_meta()
                .expect("load")
                .expect("meta present")
                .description
                .as_deref(),
            Some("新描述")
        );
    }

    #[test]
    fn derived_parameter_rows_are_structurally_linted() {
        let meta = Meta::from_json(
            r#"{"schema_version":1,"name":"任务1","params":[
                {"key":"group","type":"int","default":3,"role":"user",
                 "asset_id":"O_group"}
            ]}"#,
        )
        .expect("meta");
        let issues = meta.lint_issues();
        assert!(issues.iter().any(|i| i.msg.contains("type=string")), "{issues:?}");
        assert!(issues.iter().any(|i| i.msg.contains("role=ocr_text")), "{issues:?}");
        assert!(
            issues.iter().any(|i| i.msg.contains("string default")),
            "{issues:?}"
        );
        // Old meta.json without asset_id keeps parsing (compat).
        let old = Meta::from_json(
            r#"{"schema_version":1,"name":"任务1","params":[
                {"key":"group","type":"string","default":"御魂","role":"ocr_text"}
            ]}"#,
        )
        .expect("old meta");
        assert_eq!(old.params[0].asset_id, None);
        assert!(old.lint_issues().is_empty(), "{:?}", old.lint_issues());
        // The derived marker only appears when it is set, so old files keep
        // their exact shape on round-trip.
        let text = serde_json::to_string(&old).expect("serialize old meta");
        assert!(!text.contains("asset_id"), "{text}");
        let derived = Meta::from_json(
            r#"{"schema_version":1,"name":"任务1","params":[
                {"key":"group","type":"string","default":"御魂","role":"ocr_text",
                 "asset_id":"O_group"}
            ]}"#,
        )
        .expect("derived meta");
        let text = serde_json::to_string(&derived).expect("serialize derived meta");
        assert!(text.contains("\"asset_id\":\"O_group\""), "{text}");
    }

    #[test]
    fn t8_lint_flags_derived_row_drift_in_a_bundle() {
        let td = TempDir::new("t8-lint");
        let b = open(&td);
        b.save_asset(
            AssetDecl::ocr_var("O_group", "group", "御魂", Some([0, 0, 40, 10])),
            None,
        )
        .expect("ocr_var");
        let flow = r#"{"schema_version":1,"name":"任务1","steps":[{"op":"click_target","args":{"find":[{"kind":"ocr_text","sel":"O_group","text_expr":"param('group')"}]}},{"op":"finish","args":{"exit":"done"}}]}"#;
        b.save_flow(flow).expect("flow");
        let meta = |default: &str| {
            Meta::from_json(&format!(
                r#"{{"schema_version":1,"name":"任务1","params":[
                    {{"key":"group","type":"string","default":"{default}",
                      "role":"ocr_text","label":"O_group","asset_id":"O_group"}}
                ]}}"#
            ))
            .expect("meta")
        };
        // Matching derived row: clean lint (no T8).
        b.write_meta(&meta("御魂")).expect("write meta");
        b.lint().expect("a synced bundle must lint clean");
        // Drifted default: T8 (Lead ruling 4: an error, publish blocking).
        b.write_meta(&meta("过期")).expect("write drifted meta");
        let issues = b.lint().expect_err("T8 must fail lint");
        let t8: Vec<_> = issues.iter().filter(|i| i.code == "T8").collect();
        assert_eq!(t8.len(), 1, "{issues:?}");
        assert_eq!(t8[0].path, "meta.json:params[0]");
    }

    #[test]
    fn global_assets_participate_in_t8_and_the_old_api_still_flags_them() {
        // seq129-fix: global-asset mode — the task's own assets.json is empty and
        // its flow/variable bind a GLOBAL ocr_var. The merged lint must be green;
        // the old (global-less) entry point keeps the historical T8/T4 errors.
        let td = TempDir::new("global-lint");
        let tasks = td.path().join("tasks");
        std::fs::create_dir_all(&tasks).expect("tasks dir");
        let b = TaskBundle::open(tasks.join("全局识别任务")).expect("bundle");
        write_flow_only_bundle(
            &b,
            r#"{"schema_version":1,"name":"全局识别任务","variables":[
                 {"key":"p","type":"string","value_source":"ocr_text",
                  "asset_id":"预设识别","default":"甲","runner_editable":true}]}"#,
            r#"{"schema_version":1,"name":"全局识别任务","steps":[
                 {"op":"click","args":{"sel":"预设识别"}},
                 {"op":"drag","args":{
                   "from":{"kind":"asset","sel":"预设识别"},
                   "direction":"up","distance":[10,20]}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        );
        let global: AssetsFile = serde_json::from_str(
            r#"{"schema_version":1,"items":[
                 {"id":"预设识别","kind":"ocr_var","runtime_text":"甲",
                  "param_key":"p"}]}"#,
        )
        .expect("global assets");
        // Merged: T8 sees the global ocr_var and T4 accepts the global id.
        let mut issues = Vec::new();
        b.lint_into_with_calls_and_global_assets(&mut issues, &tasks, None, Some(&global));
        assert!(
            issues.iter().all(|i| i.code != "T8"),
            "global ocr_var must satisfy T8: {issues:?}"
        );
        assert!(
            issues.iter().all(|i| i.code != "T4"),
            "global asset id must satisfy T4: {issues:?}"
        );
        assert!(issues.is_empty(), "{issues:?}");
        // The same bundle without the global view keeps the historical errors.
        let mut issues = Vec::new();
        b.lint_into_with_calls(&mut issues, &tasks);
        assert!(
            issues.iter().any(|i| i.code == "T8"),
            "without the global assets T8 must still flag the binding: {issues:?}"
        );
        assert!(
            issues.iter().any(|i| i.code == "T4"),
            "without the global assets T4 must still flag the reference: {issues:?}"
        );
        // Task-local declarations win over the global store on an id clash.
        let mut local_assets = AssetsFile::default();
        local_assets.items.push(serde_json::from_value(serde_json::json!({
            "id": "预设识别", "kind": "ocr_var",
            "runtime_text": "乙", "param_key": "p"
        }))
        .expect("local ocr_var"));
        b.write_assets_file(&local_assets).expect("write local assets");
        let mut issues = Vec::new();
        b.lint_into_with_calls_and_global_assets(&mut issues, &tasks, None, Some(&global));
        assert!(
            issues.iter().any(|i| i.code == "T8"),
            "a task-local override must win over the global declaration: {issues:?}"
        );
    }

    #[test]
    fn load_runtime_assets_wires_ocr_vars_and_op_region_chains() {
        // B-T2 boundary: the task model must feed the runtime's `ocr_vars` and
        // `op_regions` tables (the runtime cannot depend on oas-task).
        let td = TempDir::new("runtime-ocr-var");
        let b = open(&td);
        b.save_asset(
            AssetDecl::ocr_var("O_group", "group", "御魂", Some([0, 0, 40, 10])),
            None,
        )
        .expect("ocr_var");
        let mut tpl = AssetDecl::template("I_按钮", [0, 0, 8, 8]);
        tpl.op_region = Some(OpRegion::Asset {
            asset: "O_group".into(),
            pick: Some("nearest".into()),
        });
        b.save_asset(tpl, Some(&frame(64, 48)))
            .expect("template with an op_region chain");
        let set = b.load_runtime_assets().expect("runtime assets");
        let var = set.ocr_var("O_group").expect("ocr_var wired");
        assert_eq!(
            var.roi,
            Some(oas_types::Rect {
                x: 0,
                y: 0,
                w: 40,
                h: 10
            })
        );
        assert_eq!(var.runtime_text, "御魂");
        assert_eq!(var.param_key, "group");
        assert!(set.is_ocr_asset("O_group"));
        assert!(set.has_op_region_chain("I_按钮"));
        assert_eq!(
            set.op_region("I_按钮"),
            Some(&oas_runtime::AssetOpRegion::Asset {
                asset: "O_group".into(),
                pick: oas_runtime::OpRegionPick::Nearest,
            })
        );
        // An ocr_var with an empty ROI keeps `roi: None` (whole-screen OCR) and
        // is still recognized as an OCR asset.
        b.save_asset(AssetDecl::ocr_var("O_full", "full", "开始", None), None)
            .expect("full-screen ocr_var");
        let set = b.load_runtime_assets().expect("runtime assets");
        assert_eq!(set.ocr_var("O_full").and_then(|v| v.roi), None);
        assert!(set.is_ocr_asset("O_full"));
    }
}

