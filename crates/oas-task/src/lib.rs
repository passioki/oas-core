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
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use oas_types::Frame;

/// On-disk schema version for `meta.json` / `flow.json` / `assets.json`.
pub const SCHEMA_VERSION: u32 = 1;

/// NCC score above which a template counts as "found".
pub const DEFAULT_THRESHOLD: f32 = 0.85;

/// Asset kinds allowed by §18.1 (`template` / `ocr_line` / `ocr_lines`).
pub const KINDS: [&str; 3] = ["template", "ocr_line", "ocr_lines"];

// ---------------------------------------------------------------------------
// Public data model (serde: this is the on-disk contract)
// ---------------------------------------------------------------------------

/// One declared asset, exactly as stored in `assets.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetDecl {
    /// Semantic name; Chinese allowed (`分组2`, `返回按钮`). Never used as a path.
    pub id: String,
    /// `template` | `ocr_line` | `ocr_lines`.
    pub kind: String,
    /// Template image, relative to the bundle root, always `images/aNNN.png`.
    /// `None` for OCR assets (they have no pixels).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// `[x, y, w, h]` in frame coordinates at the bundle's `design_resolution`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roi: Option<[i32; 4]>,
    /// Optional **click area** (`roi_back`, frame coordinates). Borrowed from the
    /// old OAS annotator (`roiFront`/`roiBack`): `roi` is the recognition/crop box,
    /// this is the click-sampling box. Missing => click the matched template bbox
    /// (the default since §22.20).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roi_back: Option<[i32; 4]>,
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
}

impl AssetDecl {
    /// Convenience constructor for a template asset (file is assigned on save).
    pub fn template(id: impl Into<String>, roi: [i32; 4]) -> Self {
        Self {
            id: id.into(),
            kind: "template".to_string(),
            file: None,
            roi: Some(roi),
            roi_back: None,
            threshold: None,
            polygon: None,
            rev: None,
        }
    }
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
}

impl Default for AssetsFile {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            design_resolution: None,
            items: Vec::new(),
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
        Ok(Self { root })
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
        let af: AssetsFile = serde_json::from_str(&text)
            .map_err(|e| format!("{}: invalid JSON: {e}", path.display()))?;
        let errs = self.assets_shape_errors(&af);
        if errs.is_empty() {
            Ok(af)
        } else {
            Err(errs.join("; "))
        }
    }

    /// Load `flow.json`, if present.
    pub fn load_flow(&self) -> Result<Option<oas_flow::Flow>, String> {
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

    /// Build the runtime [`oas_runtime::AssetSet`]: template PNGs are decoded to
    /// `GrayImage`, `roi`/`threshold` are mapped. OCR assets have no pixels and
    /// are skipped (their ROI is consumed by the OCR primitives directly).
    pub fn load_runtime_assets(&self) -> Result<oas_runtime::AssetSet, String> {
        let af = self.load_assets_file()?;
        let mut set = oas_runtime::AssetSet::new();
        for (i, item) in af.items.iter().enumerate() {
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
            let roi = item.roi.ok_or_else(|| {
                format!(
                    "assets.json: items[{i}].roi: template asset `{}` has no roi",
                    item.id
                )
            })?;
            let path = self.root.join(file);
            let bytes = std::fs::read(&path)
                .map_err(|e| format!("read template {}: {e}", path.display()))?;
            let frame = oas_codec::decode_png(&bytes)
                .map_err(|e| format!("decode template {}: {e}", path.display()))?;
            // The PNG is already the ROI crop; `roi` is the search window.
            let tpl = oas_runtime::Template {
                gray: oas_perceive::GrayImage::from_frame(&frame),
                roi: Some(oas_types::Rect {
                    x: roi[0],
                    y: roi[1],
                    w: roi[2],
                    h: roi[3],
                }),
                threshold: item.threshold.unwrap_or(DEFAULT_THRESHOLD),
                click_polygon: item.polygon.as_ref().map(|pts| {
                    pts.iter()
                        .map(|p| oas_types::Point { x: p[0], y: p[1] })
                        .collect()
                }),
            };
            set.insert(item.id.clone(), tpl);
            if let Some(back) = item.roi_back {
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

        let mut af = self.load_assets_file()?;

        // §22.10: optional polygon => validate it and derive `roi` = bounding box.
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

        // §22.25 `roi_back`: optional explicit click area (old OAS roiBack).
        // Only templates have a click target; bounds must fit the design frame.
        if let Some(back) = decl.roi_back {
            if decl.kind != "template" {
                return Err(format!(
                    "asset `{}` rejected: roi_back is only valid for template assets \
                     (OCR assets have no click target); nothing written",
                    decl.id
                ));
            }
            let [bx, by, bw, bh] = back;
            if bw <= 0 || bh <= 0 || bx < 0 || by < 0 {
                return Err(format!(
                    "asset `{}` rejected: roi_back [x,y,w,h] = {back:?} must have \
                     x,y >= 0 and w,h > 0; nothing written",
                    decl.id
                ));
            }
            let bounds = match frame {
                Some(f) => Some((f.width as i32, f.height as i32)),
                None => af.design_resolution.map(|[w, h]| (w as i32, h as i32)),
            };
            if let Some((fw, fh)) = bounds {
                if bx + bw > fw || by + bh > fh {
                    return Err(format!(
                        "asset `{}` rejected: roi_back [x,y,w,h] = {back:?} is outside \
                         the {fw}x{fh} frame; nothing written",
                        decl.id
                    ));
                }
            }
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
        if self.flow_path().is_file() {
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
        self.ensure_meta()?;
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
        let flow =
            oas_flow::Flow::from_json(flow_json).map_err(|e| format!("flow.json rejected: {e}"))?;
        let af = self.load_assets_file()?;
        let ids: BTreeSet<String> = af.items.iter().map(|i| i.id.clone()).collect();
        if let Err(errs) = flow.validate_assets(&ids) {
            let joined = errs
                .iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join("; ");
            return Err(format!("flow.json: {joined}; nothing written"));
        }
        self.ensure_meta()?;
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
        let mut errs = Vec::new();
        if af.schema_version != SCHEMA_VERSION {
            errs.push(format!(
                "assets.json: unsupported schema_version {} (expected {SCHEMA_VERSION})",
                af.schema_version
            ));
        }
        if let Some([w, h]) = af.design_resolution {
            if w == 0 || h == 0 {
                errs.push("assets.json: design_resolution must be > 0".to_string());
            }
        }
        let mut seen = BTreeSet::new();
        for (i, item) in af.items.iter().enumerate() {
            let p = format!("assets.json: items[{i}]");
            if let Err(e) = validate_id(&item.id) {
                errs.push(format!("{p}.id: {e}"));
            }
            if !seen.insert(item.id.clone()) {
                errs.push(format!("{p}.id: duplicate asset id `{}`", item.id));
            }
            if !KINDS.contains(&item.kind.as_str()) {
                errs.push(format!(
                    "{p}.kind: unknown kind `{}` (allowed: {})",
                    item.kind,
                    KINDS.join(", ")
                ));
            }
            if let Err(e) = validate_threshold(item.threshold) {
                errs.push(format!("{p}.threshold: {e}"));
            }
            match item.roi {
                Some([x, y, w, h]) => {
                    if x < 0 || y < 0 || w <= 0 || h <= 0 {
                        errs.push(format!(
                            "{p}.roi: [x,y,w,h] = {x},{y},{w},{h} must have x,y >= 0 and w,h > 0"
                        ));
                    }
                }
                None => {
                    if item.kind == "template" {
                        errs.push(format!("{p}.roi: template asset requires [x,y,w,h]"));
                    }
                }
            }
            match item.file.as_deref() {
                Some(file) => match validate_image_rel_path(file) {
                    Ok(()) => {
                        if !self.root.join(file).is_file() {
                            errs.push(format!("{p}.file: `{file}` does not exist"));
                        }
                    }
                    Err(e) => errs.push(format!("{p}.file: {e}")),
                },
                None => {
                    if item.kind == "template" {
                        errs.push(format!("{p}.file: template asset requires an image file"));
                    }
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
            roi_back: None,
            threshold: None,
            polygon: Some(vec![[10, 10], [30, 10], [30, 20], [10, 20]]),
            rev: None,
        };
        let saved = b.save_asset(decl.clone(), Some(&f)).expect("save ocr polygon");
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
    fn roi_back_is_persisted_and_wired_into_the_runtime_click_area() {
        let td = TempDir::new("back");
        let b = open(&td);
        let f = frame(64, 48);
        let mut decl = AssetDecl::template("点击区按钮", [10, 10, 21, 11]);
        decl.roi_back = Some([8, 8, 30, 30]);
        let saved = b.save_asset(decl, Some(&f)).expect("save with roi_back");
        assert_eq!(saved.roi_back, Some([8, 8, 30, 30]));
        let text = std::fs::read_to_string(b.assets_path()).expect("read assets");
        assert!(text.contains("roi_back"), "{text}");
        let set = b.load_runtime_assets().expect("runtime assets");
        let back = set.click_back("点击区按钮").expect("click_back wired");
        assert_eq!((back.x, back.y, back.w, back.h), (8, 8, 30, 30));
    }

    #[test]
    fn roi_back_is_rejected_on_ocr_assets_and_outside_the_frame() {
        let td = TempDir::new("backbad");
        let b = open(&td);
        let f = frame(64, 48);
        let mut ocr = AssetDecl {
            id: "文字".to_string(),
            kind: "ocr_lines".to_string(),
            file: None,
            roi: Some([10, 10, 21, 11]),
            roi_back: Some([8, 8, 30, 30]),
            threshold: None,
            polygon: None,
            rev: None,
        };
        let err = b.save_asset(ocr.clone(), Some(&f)).expect_err("ocr must reject");
        assert!(err.contains("roi_back is only valid for template"), "{err}");
        ocr.kind = "template".to_string();
        ocr.roi_back = Some([50, 40, 30, 30]); // outside 64x48
        let err = b.save_asset(ocr, Some(&f)).expect_err("outside must reject");
        assert!(err.contains("outside"), "{err}");
        assert!(!b.assets_path().exists());
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
}
