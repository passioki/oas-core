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

use std::collections::BTreeSet;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use oas_types::Frame;

mod meta;
pub use meta::{
    Capabilities, EntryRequires, ExitDecl, Meta, MetaEntry, ParamDecl, QuotaHint, ScheduleHint,
    CATEGORY_DEFAULT, DEFAULT_EXIT_NAMES, EXIT_RESULTS, PARAM_TYPES, TEMPLATE_VERSION,
};

/// On-disk schema version for `meta.json` / `flow.json` / `assets.json`.
pub const SCHEMA_VERSION: u32 = 1;

/// NCC score above which a template counts as "found".
pub const DEFAULT_THRESHOLD: f32 = 0.85;

/// Asset/object kinds: recognize+act (`template`/`ocr_line`/`ocr_lines`) and
/// pure operation region (`region`, no recognition).
pub const KINDS: [&str; 4] = ["template", "ocr_line", "ocr_lines", "region"];

// ---------------------------------------------------------------------------
// Lint (M0 unified task template, ENGINE-DESIGN §22.34.2)
// ---------------------------------------------------------------------------

/// One lint violation: a stable rule code, the JSON/file path it applies to and
/// a human-readable message. **Every issue carries a path** (the M0 acceptance
/// criterion; the CLI prints it and M1's create/save endpoints return it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LintIssue {
    /// `T1`/`T3`/`T4`/`T5`/`T6` for the flow invariants, `META` for the
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

fn default_action_source() -> String {
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
    /// **Operation region** in frame coordinates. `action_source=fixed` requires
    /// this; `recognition` leaves it absent and uses the real recognized bbox.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_shape: Option<ActionShape>,
    /// `recognition` (follow real OCR/template bbox) or `fixed` (independent
    /// operation region). Persisted explicitly; no legacy inference.
    #[serde(default = "default_action_source")]
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
    /// NCC threshold; defaults to [`DEFAULT_THRESHOLD`] for templates.
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
            rev: None,
            positions: Vec::new(),
        }
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
    #[serde(default = "default_action_source")]
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
const MAX_CALL_DEPTH: usize = 8;

fn strip_finish_steps(steps: &mut Vec<serde_json::Value>) {
    steps.retain(|s| s.get("op").and_then(|x| x.as_str()) != Some("finish"));
    for s in steps.iter_mut() {
        for key in ["then", "else", "body"] {
            if let Some(arr) = s.get_mut(key).and_then(|x| x.as_array_mut()) {
                strip_finish_steps(arr);
            }
        }
    }
}

fn expand_call_steps(
    steps: &mut Vec<serde_json::Value>,
    tasks_dir: &Path,
    stack: &mut Vec<String>,
    depth: usize,
) -> Result<(), String> {
    if depth > MAX_CALL_DEPTH {
        return Err(format!("call_task nesting exceeds {MAX_CALL_DEPTH}"));
    }
    let mut out = Vec::with_capacity(steps.len());
    for mut step in steps.drain(..) {
        if step.get("op").and_then(|x| x.as_str()) == Some("call_task") {
            let task = step
                .get("args")
                .and_then(|a| a.get("task"))
                .and_then(|x| x.as_str())
                .ok_or_else(|| "call_task requires args.task".to_string())?
                .to_string();
            validate_task_name(&task)?;
            if stack.iter().any(|s| s == &task) {
                return Err(format!(
                    "call_task cycle: {} -> {task}",
                    stack.join(" -> ")
                ));
            }
            let target_flow = tasks_dir.join(&task).join("flow.json");
            let text = std::fs::read_to_string(&target_flow)
                .map_err(|e| format!("call_task `{task}`: read {}: {e}", target_flow.display()))?;
            let mut target: serde_json::Value = serde_json::from_str(&text)
                .map_err(|e| format!("call_task `{task}`: flow.json: {e}"))?;
            let mut target_steps = target
                .get_mut("steps")
                .and_then(|x| x.as_array_mut())
                .map(|a| std::mem::take(a))
                .ok_or_else(|| format!("call_task `{task}`: flow.json has no steps array"))?;
            // A task's terminal finishes are "return", not caller termination.
            strip_finish_steps(&mut target_steps);
            stack.push(task.clone());
            expand_call_steps(&mut target_steps, tasks_dir, stack, depth + 1)?;
            stack.pop();
            out.extend(target_steps);
            continue;
        }
        for key in ["then", "else", "body"] {
            if let Some(arr) = step.get_mut(key).and_then(|x| x.as_array_mut()) {
                expand_call_steps(arr, tasks_dir, stack, depth + 1)?;
            }
        }
        out.push(step);
    }
    *steps = out;
    Ok(())
}

fn expand_collection_steps(
    steps: &mut Vec<serde_json::Value>,
    collections: &[AssetCollection],
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
                                for m in &coll.members {
                                    let mut next = loc.clone();
                                    if let Some(obj) = next.as_object_mut() {
                                        obj.insert(
                                            "sel".into(),
                                            serde_json::Value::String(m.clone()),
                                        );
                                    }
                                    out.push(next);
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
        for key in ["then", "else", "body"] {
            if let Some(arr) = step.get_mut(key).and_then(|x| x.as_array_mut()) {
                expand_collection_steps(arr, collections)?;
            }
        }
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

    /// Save a multi-image collection. Members must be existing template assets
    /// and a template may belong to only one collection in this version.
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
            let item = af
                .items
                .iter()
                .find(|a| &a.id == m)
                .ok_or_else(|| format!("集合成员 `{m}` 不存在"))?;
            if item.kind != "template" {
                return Err(format!("集合成员 `{m}` 必须是模板图（template）"));
            }
            if af
                .collections
                .iter()
                .any(|c| c.id != id && c.members.iter().any(|x| x == m))
            {
                return Err(format!("模板 `{m}` 已经属于另一个集合"));
            }
            members.push(m.clone());
        }
        if members.is_empty() {
            return Err("素材集合至少需要一个成员".into());
        }
        // B-10: a collection-level operation shape is accepted by the API, so
        // it must be validated exactly like an asset shape (bounds from the
        // bundle design resolution) instead of being silently copied.
        if coll.action_source != "recognition" && coll.action_source != "fixed" {
            return Err(format!(
                "集合 `{id}`: action_source must be recognition or fixed (got `{}`)",
                coll.action_source
            ));
        }
        match (coll.action_source.as_str(), coll.action_shape.as_ref()) {
            ("recognition", Some(_)) => {
                return Err(format!(
                    "集合 `{id}`: action_source=recognition must not carry action_shape"
                ))
            }
            ("fixed", None) => {
                return Err(format!(
                    "集合 `{id}`: action_source=fixed requires action_shape"
                ))
            }
            ("fixed", Some(shape)) => validate_action_shape(
                shape,
                &format!("collection `{id}` action_shape"),
                af.design_resolution.map(|[w, h]| (w, h)),
            )?,
            _ => {}
        }
        let saved = AssetCollection {
            id: id.clone(),
            members,
            threshold: coll.threshold,
            action_shape: coll.action_shape,
            action_source: coll.action_source,
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
        let item = af.collections.remove(pos);
        self.write_assets_file(&af)?;
        Ok(item)
    }

    /// Load `flow.json`, if present.
    ///
    /// v1.2.3 U-01(B): expand `call_task` references against the sibling task
    /// directories before parsing, so the runtime only sees ordinary steps.
    pub fn load_flow(&self) -> Result<Option<oas_flow::Flow>, String> {
        let path = self.flow_path();
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(format!("read {}: {e}", path.display())),
        };
        let mut value: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| format!("flow.json: invalid JSON: {e}"))?;
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
            expand_call_steps(steps, tasks_dir, &mut stack, 0)?;
            let collections = self
                .load_assets_file()
                .map(|af| af.collections)
                .unwrap_or_default();
            expand_collection_steps(steps, &collections)?;
        }
        let expanded = serde_json::to_string(&value)
            .map_err(|e| format!("flow.json: serialize expanded flow: {e}"))?;
        oas_flow::Flow::from_json(&expanded)
            .map(Some)
            .map_err(|e| format!("flow.json: {e}"))
    }

    /// Build the runtime [`oas_runtime::AssetSet`]: template PNGs are decoded to
    /// `GrayImage`, `roi`/`threshold` are mapped. OCR assets have no pixels and
    /// are skipped (their ROI is consumed by the OCR primitives directly).
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
                // OCR assets (`ocr_line` / `ocr_lines`) carry **no pixels**: the
                // runtime only needs their ROI so `ocr(sel)` can crop it.
                // `ocr_line` → single-line rec (fast); `ocr_lines` → det + rec
                // (ENGINE-DESIGN §17.1: both paths are kept, the kind picks one).
                // A missing ROI is a loud load error, never "skip silently".
                let roi = item.roi.ok_or_else(|| {
                    format!(
                        "assets.json: items[{i}].roi: OCR asset `{}` has no roi \
                         (ocr(sel) needs the text region)",
                        item.id
                    )
                })?;
                let rect = oas_types::Rect {
                    x: roi[0],
                    y: roi[1],
                    w: roi[2],
                    h: roi[3],
                };
                if item.kind == "ocr_lines" {
                    set.insert_ocr_lines(item.id.clone(), rect);
                } else {
                    set.insert_ocr(item.id.clone(), rect);
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
                    set.set_derived_ocr(item.id.clone(), rect, item.ocr_text.clone().unwrap());
                }
                // §22.10 N10b-2: OCR polygon mask (frame coordinates).
                if let Some(poly) = &item.polygon {
                    let pts: Vec<oas_types::Point> = poly
                        .iter()
                        .map(|p| oas_types::Point { x: p[0], y: p[1] })
                        .collect();
                    set.set_ocr_polygon(item.id.clone(), pts);
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
        af.items[pos].id = to.to_string();
        af.items[pos].rev = Some(af.items[pos].rev.unwrap_or(0) + 1);
        let item = af.items[pos].clone();
        self.write_assets_file(&af)?;
        Ok(item)
    }

    /// v1.1 asset CRUD: update OCR ROI / template threshold without re-cropping.
    ///
    /// Template ROI changes are rejected (the crop image would go stale); use
    /// `update_asset`/「保存素材覆盖」to re-crop a template.
    pub fn update_asset_meta(
        &self,
        id: &str,
        roi: Option<[i32; 4]>,
        action_shape: Option<ActionShape>,
        action_source: Option<String>,
        threshold: Option<f32>,
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
            af.items[pos].roi = Some(r);
            // B-08: a pure region's `roi` is a fallback alias of its operation
            // shape; keep them in sync instead of leaving a stale shape that
            // runtime would silently prefer.
            if kind == "region" && action_shape.is_none() {
                let shape = ActionShape::rect(r);
                validate_action_shape(&shape, "region roi", shape_bounds)?;
                af.items[pos].action_shape = Some(shape);
                af.items[pos].action_source = "fixed".to_string();
            }
        }
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
            af.items[pos].action_source = "fixed".to_string();
        }
        let had_source = action_source.is_some();
        if let Some(source) = action_source {
            if source != "recognition" && source != "fixed" {
                return Err("action_source must be recognition or fixed".into());
            }
            if kind == "region" && source == "recognition" {
                return Err(
                    "region objects always have a fixed action_shape; recognition is invalid".into(),
                );
            }
            if source == "recognition" {
                af.items[pos].action_shape = None;
            }
            af.items[pos].action_source = source;
        }
        if let Some(t) = threshold {
            if !(0.0..=1.0).contains(&t) {
                return Err("threshold must be 0..=1".into());
            }
            if kind != "template" {
                return Err("threshold only applies to template assets".into());
            }
            af.items[pos].threshold = Some(t);
        }
        if roi.is_none() && af.items[pos].action_shape.is_none() && threshold.is_none() && !had_source {
            return Err("nothing to update (pass roi, action_shape/action_source and/or threshold)".into());
        }
        af.items[pos].rev = Some(af.items[pos].rev.unwrap_or(0) + 1);
        let item = af.items[pos].clone();
        self.write_assets_file(&af)?;
        Ok(item)
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
        // v1.2.1: OCR recognition provenance. A derived `ocr_line` saved from a
        // multi-line det+rec result must keep the source region + expected text
        // so runtime can reproduce the original recognition method.
        if decl.ocr_mode.is_some() || decl.ocr_source_roi.is_some() || decl.ocr_text.is_some() {
            if decl.kind != "ocr_line" {
                return Err(format!(
                    "asset `{}` rejected: ocr_mode/ocr_source_roi/ocr_text are only valid \
                     for ocr_line assets",
                    decl.id
                ));
            }
            let mode = decl.ocr_mode.as_deref().unwrap_or("single");
            match mode {
                "single" => {
                    if decl.ocr_source_roi.is_some() || decl.ocr_text.is_some() {
                        return Err(format!(
                            "asset `{}` rejected: ocr_mode=single must not carry \
                             ocr_source_roi/ocr_text",
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

        let mut af = self.load_assets_file()?;
        // B-05: validate operation geometry against the frame the asset is
        // authored on (frame wins; otherwise the bundle design resolution).
        let shape_bounds = frame
            .map(|f| (f.width, f.height))
            .or(af.design_resolution.map(|[w, h]| (w, h)));
        // Matching uses a masked NCC inside the polygon; OCR uses the bbox crop
        // with outside-polygon pixels filled white; click uses the bbox.
        if let Some(poly) = &decl.polygon {
            if poly.len() < 3 {
                return Err(format!(
                    "asset `{}` rejected: polygon needs >= 3 points (got {}); nothing written",
                    decl.id,
                    poly.len()
                ));
            }
            let mut area2: i64 = 0;
            for i in 0..poly.len() {
                let a = poly[i];
                let b = poly[(i + 1) % poly.len()];
                if a == b {
                    return Err(format!(
                        "asset `{}` rejected: polygon has repeated adjacent point {:?}; nothing written",
                        decl.id, a
                    ));
                }
                area2 += a[0] as i64 * b[1] as i64 - b[0] as i64 * a[1] as i64;
            }
            if area2 == 0 {
                return Err(format!(
                    "asset `{}` rejected: polygon has zero area (collinear points); nothing written",
                    decl.id
                ));
            }
            if polygon_self_intersects(poly) {
                return Err(format!(
                    "asset `{}` rejected: polygon self-intersects (edges cross); nothing written",
                    decl.id
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
                    "asset `{}` rejected: polygon point is negative; nothing written",
                    decl.id
                ));
            }
            let bounds = match frame {
                Some(f) => Some((f.width as i32, f.height as i32)),
                None => af.design_resolution.map(|[w, h]| (w as i32, h as i32)),
            };
            if let Some((bw, bh)) = bounds {
                if max_x >= bw || max_y >= bh {
                    return Err(format!(
                        "asset `{}` rejected: polygon [{}..{}, {}..{}] is outside the {bw}x{bh} frame; \
                         nothing written",
                        decl.id, min_x, max_x, min_y, max_y
                    ));
                }
            }
            decl.roi = Some([min_x, min_y, max_x - min_x + 1, max_y - min_y + 1]);
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
            decl.action_source = "fixed".to_string();
        }
        if decl.action_source != "recognition" && decl.action_source != "fixed" {
            return Err(format!(
                "asset `{}` rejected: action_source must be recognition or fixed",
                decl.id
            ));
        }
        match (decl.action_source.as_str(), decl.action_shape.as_ref()) {
            ("recognition", Some(_)) => {
                return Err(format!(
                    "asset `{}` rejected: action_source=recognition must not carry action_shape",
                    decl.id
                ))
            }
            ("fixed", None) => {
                return Err(format!(
                    "asset `{}` rejected: action_source=fixed requires action_shape",
                    decl.id
                ))
            }
            ("fixed", Some(shape)) => {
                validate_action_shape(shape, "action_shape", shape_bounds)?
            }
            _ => {}
        }
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
            let roi = decl.roi.ok_or_else(|| {
                format!(
                    "asset `{}` of kind `{}` requires roi [x,y,w,h]",
                    decl.id, decl.kind
                )
            })?;
            let [x, y, w, h] = roi;
            if w <= 0 || h <= 0 || x < 0 || y < 0 {
                return Err(format!(
                    "asset `{}` roi [x,y,w,h] = {roi:?} must have x,y >= 0 and w,h > 0",
                    decl.id
                ));
            }
            // OCR assets carry no template pixels: the id never becomes a path.
            decl.file = None;
        }
        decl.rev = Some(rev);

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
        self.lint_into_with_assets(issues, None);
    }

    /// v1.2.1: lint a task whose flow may reference a **global** asset id set.
    pub fn lint_into_with_assets(
        &self,
        issues: &mut Vec<LintIssue>,
        external_assets: Option<&BTreeSet<String>>,
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
            // T3: every param('key') must be declared in meta.params.
            let params = meta.declared_param_keys();
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
                                "param(`{key}`) is used but not declared in meta.params \
                                 (declared: {listed_params})"
                            ),
                        ))
                    }
                    _ => {}
                }
            }
        }

        // T4: asset references must resolve (or be `lib:` selectors).
        let t4_ids: Option<BTreeSet<String>> = match external_assets {
            Some(ids) => Some(ids.clone()),
            None => assets
                .as_ref()
                .map(|af| af.items.iter().map(|i| i.id.clone()).collect()),
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

    fn write_assets_file(&self, af: &AssetsFile) -> Result<(), String> {
        let text = serde_json::to_string_pretty(af)
            .map_err(|e| format!("serialize assets.json: {e}"))?;
        write_atomic(&self.assets_path(), format!("{text}\n").as_bytes())
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
            if item.ocr_mode.is_some()
                || item.ocr_source_roi.is_some()
                || item.ocr_text.is_some()
            {
                if item.kind != "ocr_line" {
                    errs.push((
                        format!("{p}.ocr_mode"),
                        "ocr provenance is only valid for ocr_line".to_string(),
                    ));
                }
                match item.ocr_mode.as_deref().unwrap_or("single") {
                    "single" => {}
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
            if item.action_source != "recognition" && item.action_source != "fixed" {
                errs.push((
                    format!("{p}.action_source"),
                    format!("must be recognition or fixed (got `{}`)", item.action_source),
                ));
            }
            if item.action_source == "recognition" && item.action_shape.is_some() {
                errs.push((
                    format!("{p}.action_shape"),
                    "action_source=recognition must not carry action_shape".to_string(),
                ));
            }
            if item.action_source == "fixed" && item.action_shape.is_none() {
                errs.push((
                    format!("{p}.action_shape"),
                    "action_source=fixed requires action_shape".to_string(),
                ));
            }
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
        // B-10: collection-level operation geometry must obey the same contract
        // as per-asset geometry (otherwise it is saved but unusable at runtime).
        for (ci, coll) in af.collections.iter().enumerate() {
            let p = format!("assets.json: collections[{ci}]");
            if coll.action_source != "recognition" && coll.action_source != "fixed" {
                errs.push((
                    format!("{p}.action_source"),
                    format!(
                        "must be recognition or fixed (got `{}`)",
                        coll.action_source
                    ),
                ));
            }
            if coll.action_source == "recognition" && coll.action_shape.is_some() {
                errs.push((
                    format!("{p}.action_shape"),
                    "action_source=recognition must not carry action_shape".to_string(),
                ));
            }
            if coll.action_source == "fixed" && coll.action_shape.is_none() {
                errs.push((
                    format!("{p}.action_shape"),
                    "action_source=fixed requires action_shape".to_string(),
                ));
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

fn scan_forbidden_keys(
    value: &serde_json::Value,
    path: &str,
    issues: &mut Vec<LintIssue>,
) {
    match value {
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                let child = format!("{path}.{k}");
                if FORBIDDEN_FLOW_KEYS.contains(&k.as_str()) {
                    issues.push(LintIssue::new(
                        "T6",
                        format!("flow.json:{child}"),
                        format!(
                            "key `{k}` is not part of the flow DSL: external code is \
                             not allowed (T6)"
                        ),
                    ));
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
    match bundle.lint() {
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
        let issues = match bundle.lint() {
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

    #[test]
    fn load_flow_expands_call_task_and_strips_target_finish() {
        let td = TempDir::new("calltask");
        let target = TaskBundle::open(td.path().join("子任务")).expect("open target");
        target
            .save_flow(
                r#"{"schema_version":1,"name":"子任务","steps":[
                    {"op":"log","args":{"level":"info","text":"'inside'"}},
                    {"op":"finish","args":{"exit":"done"}}
                ]}"#,
            )
            .expect("save target");

        let caller = TaskBundle::open(td.path().join("调用者")).expect("open caller");
        caller
            .save_flow(
                r#"{"schema_version":1,"name":"调用者","steps":[
                    {"op":"call_task","args":{"task":"子任务","mode":"run"}},
                    {"op":"finish","args":{"exit":"done"}}
                ]}"#,
            )
            .expect("save caller");

        let flow = caller.load_flow().expect("load").expect("flow");
        assert_eq!(flow.steps.len(), 2, "target finish must be stripped");
        assert!(
            matches!(flow.steps[0], oas_flow::Step::Log { .. }),
            "target steps must be expanded: {:?}",
            flow.steps
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
        let td = TempDir::new("back");
        let b = open(&td);
        let f = frame(64, 48);
        let mut decl = AssetDecl::template("点击区按钮", [10, 10, 21, 11]);
        decl.action_source = "fixed".into();
        decl.action_shape = Some(ActionShape::rect([8, 8, 30, 30]));
        let saved = b.save_asset(decl, Some(&f)).expect("save with action_shape");
        assert_eq!(
            saved.action_shape.as_ref().and_then(|s| s.bbox()),
            Some([8, 8, 30, 30])
        );
        assert_eq!(saved.action_source, "fixed");
        let text = std::fs::read_to_string(b.assets_path()).expect("read assets");
        assert!(text.contains("action_shape"), "{text}");
        assert!(!text.contains("action_roi"), "legacy field must be gone: {text}");
        let set = b.load_runtime_assets().expect("runtime assets");
        let back = set.click_back("点击区按钮").expect("click_back wired");
        assert_eq!((back.x, back.y, back.w, back.h), (8, 8, 30, 30));
    }

    #[test]
    fn fixed_polygon_action_shape_is_persisted_and_invert_is_kept() {
        let td = TempDir::new("polyshape");
        let b = open(&td);
        let f = frame(64, 48);
        let mut decl = AssetDecl::template("多边形操作区", [10, 10, 21, 11]);
        decl.action_source = "fixed".into();
        decl.action_shape = Some(ActionShape {
            kind: "polygon".into(),
            roi: None,
            points: Some(vec![[8, 8], [40, 8], [40, 30], [8, 30]]),
            invert: true,
        });
        let saved = b.save_asset(decl, Some(&f)).expect("save polygon shape");
        assert_eq!(saved.action_source, "fixed");
        let text = std::fs::read_to_string(b.assets_path()).expect("read");
        assert!(text.contains("\"polygon\""), "{text}");
        assert!(text.contains("\"invert\": true"), "{text}");
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
        decl.action_source = "fixed".into();
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
    fn fixed_shape_is_valid_for_ocr_and_region_derives_its_own_shape() {
        let td = TempDir::new("backbad");
        let b = open(&td);
        let f = frame(64, 48);
        let ocr = AssetDecl {
            id: "文字".to_string(),
            kind: "ocr_lines".to_string(),
            file: None,
            roi: Some([10, 10, 21, 11]),
            action_shape: Some(ActionShape::rect([12, 12, 30, 30])),
            action_source: "fixed".to_string(),
            ocr_mode: None,
            ocr_source_roi: None,
            ocr_text: None,
            threshold: None,
            polygon: None,
            rev: None,
            positions: Vec::new(),
        };
        b.save_asset(ocr.clone(), Some(&f))
            .expect("OCR fixed shape is valid");
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
        assert_eq!(saved_region.action_source, "fixed");
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
            .update_asset_meta("列表", None, Some(poly), None, None)
            .expect("region action_shape update must be accepted");
        assert_eq!(updated.action_source, "fixed");
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
            )
            .expect_err("out-of-frame region update must fail");
        assert!(err.contains("outside"), "{err}");
        let af = b.load_assets_file().expect("assets");
        assert_eq!(af.items[1].roi, Some([5, 5, 21, 21]), "failed update writes nothing");

        // Updating only the region roi must re-derive the operation shape too.
        let updated = b
            .update_asset_meta("列表", Some([10, 10, 20, 10]), None, None, None)
            .expect("region roi update");
        assert_eq!(updated.roi, Some([10, 10, 20, 10]));
        assert_eq!(
            updated.action_shape.as_ref().and_then(|s| s.bbox()),
            Some([10, 10, 20, 10])
        );

        let err = b
            .update_asset_meta("列表", None, None, Some("recognition".into()), None)
            .expect_err("region cannot switch to recognition");
        assert!(err.contains("region"), "{err}");
    }

    #[test]
    fn collection_action_shape_is_validated_and_loaded() {
        // B-10: collection-level geometry was accepted without validation and
        // ignored by the runtime loader.
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

        let err = b
            .save_collection(coll(
                Some(ActionShape::rect([100, 100, 10, 10])),
                "fixed",
            ))
            .expect_err("out-of-frame collection shape must be rejected");
        assert!(err.contains("outside the 64x48 frame"), "{err}");
        let err = b
            .save_collection(coll(Some(ActionShape::rect([10, 10, 20, 20])), "recognition"))
            .expect_err("recognition must not carry a shape");
        assert!(err.contains("must not carry"), "{err}");
        let err = b
            .save_collection(coll(None, "fixed"))
            .expect_err("fixed requires a shape");
        assert!(err.contains("requires action_shape"), "{err}");

        b.save_collection(coll(Some(ActionShape::rect([10, 10, 20, 20])), "fixed"))
            .expect("valid collection shape");
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
}

