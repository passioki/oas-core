//! TaskPackage (`.oaspkg`) build / read / verify (Runner Studio split §B).
//!
//! A package is a **plain zip** with a fixed, small contract:
//!
//! ```text
//! manifest.json          required, package metadata + file digests
//! checksums.sha256       required, one `<sha256>  <relative/path>` line per file
//! task/                  the published task bundle (meta/flow/assets/images)
//! deps/<task_id>/        optional embedded call_task targets
//! assets/                optional snapshot of the referenced global assets
//! README.md              optional human release note
//! ```
//!
//! Design rules enforced here (each prevents a real failure mode):
//! * **no runtime state is ever packaged** (`state/`, `*.clicks.jsonl`, `*.tmp`,
//!   `*.bak`, `.backup`); `validate_for_publish` additionally refuses to publish
//!   a bundle that has `state/` or click logs;
//! * **zip-slip is rejected twice** (on read and on extract): absolute paths,
//!   drive letters, backslashes, `.`/`..` components and empty components;
//! * **bounded extraction** (2000 entries / 512 MiB declared total / 256 MiB per
//!   entry, at most 100:1 compression ratio per entry, no encrypted entries);
//!   each entry is read through `Read::take(declared + 1)`, so a lying header
//!   cannot make the decompressor allocate more than it declared;
//! * **no silent flow-op downgrade**: `engine.flow_ops` is derived from the raw
//!   `call_task.args.{params,inherit}` of the authored flow (design §C.4.6), not
//!   from the generated in-memory `with_params`;
//! * **reproducible builds** (`--reproducible`): fixed zip timestamp, zero
//!   `created_at_ms`, no wall-clock in the manifest → identical input gives an
//!   identical archive hash;
//! * **zip entry names are always forward-slash, relative paths** so a package
//!   built on Windows installs on Linux/Android unchanged.
//!
//! All IO uses `std::fs`; no Windows-only API is used, so the same code runs on
//! the future Android/Tauri packaging layer.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    validate_task_name, AssetsFile, LintIssue, Meta, TaskBundle, MAX_CALL_DEPTH, SCHEMA_VERSION,
    TEMPLATE_VERSION,
};

/// On-disk `manifest.package_schema` supported by this engine.
///
/// V2.0 (`docs/VARIABLES-V2-DESIGN.md` §2.5/§6.3): 1 → 2 because
/// `params_schema` was replaced by `variables_schema`. Old packages are refused
/// (no dual-read path); V2.0b implements the install-time message.
pub const PACKAGE_SCHEMA: u32 = 2;

/// Package suffix. It is still a zip; the suffix only keeps double-click
/// extractors and file pickers honest.
pub const PACKAGE_EXTENSION: &str = "oaspkg";

pub const MANIFEST_FILE: &str = "manifest.json";
pub const CHECKSUMS_FILE: &str = "checksums.sha256";

/// Flow operations this engine can execute. A package whose manifest declares
/// anything outside this list is refused by the Runner installer (design
/// §C.4.6). Only the call-semantics gates are listed: the rest of the DSL is
/// covered by `flow.schema_version`.
///
/// 参数 v2 (PARAM-V2-DESIGN §3.6): `text_expr` on an OCR locator / guard /
/// drag endpoint gets its own marker because a pre-v2 Runner's parser requires
/// the literal `text` field — it would fail mid-task instead of at install
/// time. `expect.text_expr` covers both `requires` and `expect` guards (they
/// share the engine's `Expect` object).
///
/// v1.3 operation region (OPERATION-REGION-DESIGN §6.1): `long_press` and
/// `curve_swipe` are new instructions, and an authored `region` operation-area
/// override on an OCR locator / drag endpoint gets its own marker. A pre-v1.3
/// Runner's locator parser ignores `region` (a silently different action
/// area), so packages using it must be refused at install time.
///
/// v1.4 A-plan (OPERATION-REGION-DESIGN §7): `region.asset` names a
/// **whole-library operation-area asset**; a pre-v1.4 Runner would silently
/// ignore that key and act on the wrong geometry, hence dedicated markers.
///
/// seq129: a `drag` endpoint may be a task **asset** (`{"kind":"asset","sel":..}`).
/// A pre-seq129 Runner's drag parser rejects the unknown kind with a load error,
/// but the install gate should refuse the package up front, so it gets its own
/// marker too.
pub const SUPPORTED_FLOW_OPS: [&str; 15] = [
    "call_task",
    "call_task.params",
    "call_task.inherit.none",
    "with_params",
    "click_target.text_expr",
    "expect.text_expr",
    "drag.text_expr",
    "long_press",
    "curve_swipe",
    "click_target.region",
    "drag.region",
    "click_target.region_asset",
    "drag.region_asset",
    "drag.asset",
    "navigate",
];

/// Engine version this crate records in `manifest.engine.min_engine` and checks
/// at install time (design §B.2/§B.4).
pub const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Refuse absurd archives before allocating/decompressing them (design §A.7:
/// at most 2000 entries).
pub const MAX_PACKAGE_ENTRIES: usize = 2000;
/// Refuse archives whose declared uncompressed payload exceeds this.
pub const MAX_PACKAGE_TOTAL_BYTES: u64 = 512 * 1024 * 1024;
/// Refuse a single entry larger than this (the total cap still applies).
pub const MAX_PACKAGE_ENTRY_BYTES: u64 = 256 * 1024 * 1024;
/// Refuse an entry whose declared size is more than this many times its
/// compressed size (design §A.7: compression ratio > 100:1 is refused).
pub const MAX_COMPRESSION_RATIO: u64 = 100;
/// Entries smaller than this are exempt from the ratio check: deflate overhead
/// on a tiny, legitimately compressed file can exceed 100:1, and the absolute
/// damage is bounded by this constant.
pub const MIN_RATIO_CHECK_BYTES: u64 = 1024 * 1024;

// ---------------------------------------------------------------------------
// Manifest (on-disk contract, §B.2)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub package_schema: u32,
    pub package_id: String,
    pub task_id: String,
    pub task_version: u32,
    pub name: String,
    pub created_at_ms: u64,
    pub tool: ToolInfo,
    pub source: SourceInfo,
    pub engine: EngineInfo,
    pub files: Vec<FileEntry>,
    /// V2.0 variable schema snapshot (`meta.variables` + call-site mapping).
    pub variables_schema: VariablesSchema,
    pub dependencies: Vec<Dependency>,
    pub asset_refs: Vec<AssetRef>,
    pub platform: Platform,
    /// DEPRECATED (seq137 #3): map requirements are not a package dependency —
    /// the map is a local action-library resource. The field is kept only so old
    /// packages still load; new builds leave it empty.
    #[serde(default, skip_serializing_if = "MapRequirements::is_empty")]
    pub map_requirements: MapRequirements,
    /// v1 has no signing; the field exists so a future package can be signed
    /// without a schema change. Always `null` in Phase 1.
    pub signature: Option<serde_json::Value>,
}

/// seq136 D1: `navigate` target requirements (literal node ids).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapRequirements {
    #[serde(default)]
    pub nodes: Vec<String>,
}

impl MapRequirements {
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolInfo {
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git: Option<String>,
    pub dirty: bool,
    pub surface: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceInfo {
    pub repo: String,
    pub commit: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EngineInfo {
    pub template_version: u32,
    pub min_engine: String,
    pub flow_ops: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
}

/// **Deprecated** (V2.0a compat): the pre-V2 `params_schema` snapshot shape.
///
/// `package_schema=2` packages use [`VariablesSchema`] instead; this type is
/// kept only so the intermediate serve build keeps compiling (V2.0b deletes it).
#[deprecated(note = "V2.0: packages use VariablesSchema (package_schema=2)")]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParamsSchema {
    /// Snapshot of `meta.params` (raw JSON array, never a second authority).
    pub own: serde_json::Value,
    /// One row per `call_task` site in the **authored root flow**.
    #[serde(default)]
    pub call_deps: Vec<CallDepSchema>,
}

/// V2.0 manifest variable snapshot (design §2.5).
///
/// Like the old `params_schema.own` this is a **snapshot, not a second
/// authority**: after install the task's `meta.variables` wins.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VariablesSchema {
    /// Effective variables of the published task (`meta.variables`, or the
    /// in-memory import of a legacy `meta.params`).
    #[serde(default)]
    pub own: Vec<crate::meta::Variable>,
    /// One row per `call_task` site in the **authored root flow**.
    #[serde(default)]
    pub call_deps: Vec<CallDepSchema>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CallDepSchema {
    pub child: String,
    /// child variable key -> raw parent expression string.
    #[serde(default)]
    pub mapping: BTreeMap<String, String>,
    /// `"all"` (default) or `"none"`.
    pub inherit: String,
    /// Child effective input-variable keys snapshot, in declaration order.
    #[serde(default)]
    pub child_variables: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dependency {
    pub task_id: String,
    pub version: u32,
    /// `true` = the task directory is embedded under `deps/`.
    pub included: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetRef {
    pub id: String,
    pub kind: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Platform {
    pub os: String,
    pub needs_adb: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub design_resolution: Option<[u32; 2]>,
}

// ---------------------------------------------------------------------------
// Build options / outcome / errors
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct BuildOptions {
    pub include_deps: bool,
    pub reproducible: bool,
    /// Overwrite an existing `<task>-v<version>.oaspkg`.
    pub force: bool,
    /// Root that contains sibling task bundles. Defaults to the task's parent.
    pub tasks_dir: PathBuf,
    /// Global asset store (`assets.json` + `images/`). Defaults to
    /// `<tasks_dir>/../assets`.
    pub assets_dir: PathBuf,
    pub tool_version: String,
    pub git: Option<String>,
    pub dirty: bool,
    pub surface: String,
    /// Absolute repository root recorded as `source.repo`.
    pub repo: String,
    /// Commit hash recorded as `source.commit` (may be empty).
    pub commit: String,
    /// DEPRECATED (seq137 #3): no longer set by publish (map is not a package
    /// dependency; runtime validates lazily). Kept for API compatibility.
    pub map_nodes: Option<BTreeSet<String>>,
}

impl BuildOptions {
    /// Production defaults: sibling tasks dir + sibling global asset store.
    pub fn new(task_dir: &Path) -> Self {
        let tasks_dir = task_dir
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let assets_dir = tasks_dir
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(|p| p.join("assets"))
            .unwrap_or_else(|| PathBuf::from("assets"));
        Self {
            include_deps: false,
            reproducible: false,
            force: false,
            tasks_dir,
            assets_dir,
            tool_version: env!("CARGO_PKG_VERSION").to_string(),
            git: None,
            dirty: false,
            surface: "studio".to_string(),
            repo: String::new(),
            commit: String::new(),
            map_nodes: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BuildOutcome {
    /// Final archive path.
    pub path: PathBuf,
    /// SHA-256 of the final archive (what the user copies/compares).
    pub sha256: String,
    pub manifest: Manifest,
    pub warnings: Vec<String>,
}

/// Build failures distinguish "already exists" (HTTP 409 / `--force`) from
/// validation errors (HTTP 400), instead of collapsing everything into a string.
#[derive(Debug, Clone, PartialEq)]
pub enum PackageError {
    /// The target `.oaspkg` already exists and `force` was not set.
    Exists(PathBuf),
    /// Publish-time validation (lint T1/T3/T4/T5/T6/T7 + package rules).
    Validation(Vec<LintIssue>),
    Other(String),
}

impl fmt::Display for PackageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PackageError::Exists(path) => write!(
                f,
                "package already exists: {} (use --force or bump meta.version)",
                path.display()
            ),
            PackageError::Validation(issues) => {
                let joined = issues
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("; ");
                write!(f, "{joined}")
            }
            PackageError::Other(msg) => f.write_str(msg),
        }
    }
}

impl std::error::Error for PackageError {}

impl From<String> for PackageError {
    fn from(value: String) -> Self {
        PackageError::Other(value)
    }
}

pub type PackageResult<T> = Result<T, PackageError>;

// ---------------------------------------------------------------------------
// Small helpers
// ---------------------------------------------------------------------------

static WRITE_SEQ: AtomicU64 = AtomicU64::new(0);

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn hex(bytes: &[u8]) -> String {
    const TABLE: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(TABLE[(b >> 4) as usize] as char);
        out.push(TABLE[(b & 0x0f) as usize] as char);
    }
    out
}

/// SHA-256 of a byte slice, lowercase hex.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex(&hasher.finalize())
}

/// Alias kept explicit for call sites that read like the design text.
pub fn sha256_bytes(bytes: &[u8]) -> String {
    sha256_hex(bytes)
}

/// SHA-256 of a file, lowercase hex. Streams the file (no full read).
pub fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = std::fs::File::open(path)
        .map_err(|e| format!("open {}: {e}", path.display()))?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher)
        .map_err(|e| format!("read {}: {e}", path.display()))?;
    Ok(hex(&hasher.finalize()))
}

fn is_sha256_hex(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Validate one archive-relative entry path and reject zip-slip shapes.
///
/// Rules: non-empty, relative, forward slashes only, no Windows drive prefix,
/// no empty/`.`/`..` component, no control/Windows-illegal character.
pub fn validate_archive_path(path: &str) -> Result<(), String> {
    validate_archive_path_inner(path, false)
}

fn validate_archive_path_inner(path: &str, allow_dir: bool) -> Result<(), String> {
    if path.is_empty() {
        return Err("package entry path is empty".to_string());
    }
    if path.contains('\\') {
        return Err(format!(
            "package entry `{path}` uses a backslash; zip entries must use `/`"
        ));
    }
    if path.starts_with('/') {
        return Err(format!("package entry `{path}` is absolute"));
    }
    let bytes = path.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        return Err(format!(
            "package entry `{path}` starts with a Windows drive/ADS prefix"
        ));
    }
    let trimmed = if allow_dir && path.ends_with('/') {
        &path[..path.len() - 1]
    } else {
        path
    };
    if trimmed.is_empty() {
        return Err(format!("package entry `{path}` has an empty path"));
    }
    for component in trimmed.split('/') {
        if component.is_empty() {
            return Err(format!("package entry `{path}` has an empty component"));
        }
        if component == "." || component == ".." {
            return Err(format!(
                "package entry `{path}` contains `{component}`; path traversal is refused"
            ));
        }
        for ch in component.chars() {
            if ch.is_control() || matches!(ch, '<' | '>' | ':' | '"' | '|' | '?' | '*') {
                return Err(format!(
                    "package entry `{path}` contains the illegal character `{ch}`"
                ));
            }
        }
    }
    Ok(())
}

/// Convert a Windows or Unix path into a package-relative `/` path.
fn relative_slash_path(path: &Path) -> Result<String, String> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::Normal(os) => parts.push(
                os.to_str()
                    .ok_or_else(|| format!("path `{}` is not valid UTF-8", path.display()))?
                    .to_string(),
            ),
            std::path::Component::CurDir => {}
            _ => {
                return Err(format!(
                    "path `{}` is not a plain relative path",
                    path.display()
                ))
            }
        }
    }
    Ok(parts.join("/"))
}

fn platform_os() -> String {
    let os = std::env::consts::OS;
    let arch = match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        other => other,
    };
    format!("{os}-{arch}")
}

// ---------------------------------------------------------------------------
// Flow-op derivation (design §C.4.6)
// ---------------------------------------------------------------------------

/// Does an authored `ocr_text` locator / guard / drag endpoint use the v2
/// `text_expr` text source?
///
/// Requires `kind:"ocr_text"` **and** a `text_expr` key: a `text_expr` on any
/// other kind is ignored by the parser (and therefore not a capability this
/// flow actually uses), so it must not gate the package.
fn has_ocr_text_expr(v: &serde_json::Value) -> bool {
    v.get("kind").and_then(|k| k.as_str()) == Some("ocr_text")
        && v.get("text_expr").is_some()
}

/// `requires` / `expect` sub-objects of a click-family step (§3.6).
fn has_expect_text_expr(args: &serde_json::Value) -> bool {
    ["requires", "expect"].iter().any(|key| {
        args.get(*key)
            .map(has_ocr_text_expr)
            .unwrap_or(false)
    })
}

/// Does an authored OCR locator / drag endpoint carry the v1.3 `region`
/// operation-area override? Only `kind:"ocr_text"` accepts it at parse time, so
/// a `region` on any other kind cannot be a real capability.
fn has_region_override(v: &serde_json::Value) -> bool {
    v.get("kind").and_then(|k| k.as_str()) == Some("ocr_text") && v.get("region").is_some()
}

/// Does an authored OCR locator / drag endpoint carry a v1.4 `region.asset`
/// (whole-library operation-area asset)? Only `asset` / `nearest` modes allow it
/// at parse time.
fn has_region_asset(v: &serde_json::Value) -> bool {
    has_region_override(v)
        && v.get("region")
            .and_then(|r| r.get("asset"))
            .is_some()
}

/// seq129: does an authored drag endpoint use the `asset` kind?
fn has_asset_endpoint(v: &serde_json::Value) -> bool {
    v.get("kind").and_then(|k| k.as_str()) == Some("asset")
}

fn walk_flow_ops(
    steps: &[serde_json::Value],
    out: &mut BTreeSet<String>,
) {
    for step in steps {
        let op = step.get("op").and_then(|v| v.as_str()).unwrap_or("");
        let args = step.get("args");
        match op {
            "call_task" => {
                out.insert("call_task".to_string());
                let params_nonempty = args
                    .and_then(|a| a.get("params"))
                    .and_then(|p| p.as_object())
                    .map(|p| !p.is_empty())
                    .unwrap_or(false);
                if params_nonempty {
                    out.insert("call_task.params".to_string());
                }
                let inherit_none = args
                    .and_then(|a| a.get("inherit"))
                    .and_then(|v| v.as_str())
                    == Some("none");
                if inherit_none {
                    out.insert("call_task.inherit.none".to_string());
                }
            }
            "with_params" => {
                out.insert("with_params".to_string());
            }
            // 参数 v2: `text_expr` OCR text source (PARAM-V2-DESIGN §3.6).
            // v1.3: an authored `region` override on a find[] OCR locator
            // (OPERATION-REGION-DESIGN §6.1).
            "click_target" => {
                if let Some(args) = args {
                    let finds: Vec<&serde_json::Value> = args
                        .get("find")
                        .and_then(|f| f.as_array())
                        .map(|finds| finds.iter().collect())
                        .unwrap_or_default();
                    if finds.iter().any(|loc| has_ocr_text_expr(loc)) {
                        out.insert("click_target.text_expr".to_string());
                    }
                    if finds.iter().any(|loc| has_region_override(loc)) {
                        out.insert("click_target.region".to_string());
                    }
                    // v1.4 A-plan: whole-library operation-area asset.
                    if finds.iter().any(|loc| has_region_asset(loc)) {
                        out.insert("click_target.region_asset".to_string());
                    }
                    if has_expect_text_expr(args) {
                        out.insert("expect.text_expr".to_string());
                    }
                }
            }
            // v1.3 long press: the op itself is the capability gate; its
            // `find[]` chain may carry `text_expr`/`region` but an old Runner
            // cannot run `long_press` at all (the op marker covers both).
            "long_press" => {
                out.insert("long_press".to_string());
            }
            "click" | "click_point" => {
                if let Some(args) = args {
                    if has_expect_text_expr(args) {
                        out.insert("expect.text_expr".to_string());
                    }
                }
            }
            "drag" => {
                if let Some(args) = args {
                    let targets: Vec<&serde_json::Value> = ["from", "to"]
                        .iter()
                        .filter_map(|key| args.get(*key))
                        .collect();
                    if targets.iter().any(|t| has_ocr_text_expr(t)) {
                        out.insert("drag.text_expr".to_string());
                    }
                    if targets.iter().any(|t| has_region_override(t)) {
                        out.insert("drag.region".to_string());
                    }
                    // v1.4 A-plan: whole-library operation-area asset.
                    if targets.iter().any(|t| has_region_asset(t)) {
                        out.insert("drag.region_asset".to_string());
                    }
                    // seq129: asset endpoints need a seq129+ Runner.
                    if targets.iter().any(|t| has_asset_endpoint(t)) {
                        out.insert("drag.asset".to_string());
                    }
                }
            }
            // v1.3 multi-segment curve swipe: new instruction.
            "curve_swipe" => {
                out.insert("curve_swipe".to_string());
            }
            // seq136 D1: navigate needs a D1 engine and a global map.
            "navigate" => {
                out.insert("navigate".to_string());
            }
            _ => {}
        }
        for key in ["then", "else", "body"] {
            if let Some(nested) = step.get(key).and_then(|v| v.as_array()) {
                walk_flow_ops(nested, out);
            }
        }
        if let Some(nested) = args.and_then(|a| a.get("body")).and_then(|v| v.as_array()) {
            walk_flow_ops(nested, out);
        }
    }
}

/// Derive the manifest `engine.flow_ops` from the **authored** flow JSON.
///
/// `with_params` is normally generated in memory and never written to disk; a
/// hand-authored one is still reported so the gate cannot be bypassed by
/// authoring it directly. `call_task.params` / `call_task.inherit.none` come
/// from the raw args — exactly the fields a pre-PR4 Runner would silently
/// ignore (design §C.4.6 correction).
pub fn derive_flow_ops(flow_json: &str) -> Result<Vec<String>, String> {
    let value: serde_json::Value =
        serde_json::from_str(flow_json).map_err(|e| format!("flow.json: invalid JSON: {e}"))?;
    let steps = value
        .get("steps")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "flow.json: missing `steps` array".to_string())?;
    let mut out = BTreeSet::new();
    walk_flow_ops(steps, &mut out);
    Ok(out.into_iter().collect())
}

/// Union of [`derive_flow_ops`] over several flow files (root + embedded deps).
pub fn derive_flow_ops_for_files(paths: &[PathBuf]) -> Result<Vec<String>, String> {
    let mut out = BTreeSet::new();
    for path in paths {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("read {}: {e}", path.display()))?;
        out.extend(derive_flow_ops(&text)?);
    }
    Ok(out.into_iter().collect())
}

/// Are every declared flow ops supported by this engine? Returns the offending
/// ops (empty = supported).
pub fn unsupported_flow_ops(ops: &[String]) -> Vec<String> {
    ops.iter()
        .filter(|op| !SUPPORTED_FLOW_OPS.contains(&op.as_str()))
        .cloned()
        .collect()
}

/// Compare two dotted versions numerically (`0.10.0 >= 0.9.9`), ignoring a
/// leading `v` and any `-pre`/`+build` suffix on each component. Used by the
/// `manifest.engine.min_engine` install gate; deliberately simple (the workspace
/// version is plain semver and pre-release ordering is out of scope).
pub fn version_at_least(current: &str, minimum: &str) -> bool {
    fn numeric_parts(version: &str) -> Vec<u64> {
        let raw = version.trim().trim_start_matches('v');
        raw.split('.')
            .map(|part| {
                let digits: String = part
                    .chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect();
                digits.parse::<u64>().unwrap_or(0)
            })
            .collect()
    }
    let mut current = numeric_parts(current);
    let mut minimum = numeric_parts(minimum);
    let len = current.len().max(minimum.len());
    current.resize(len, 0);
    minimum.resize(len, 0);
    current >= minimum
}

// ---------------------------------------------------------------------------
// Checksums
// ---------------------------------------------------------------------------

/// Render `<sha256>  <path>` lines (two spaces, sorted, trailing newline).
pub fn checksums_text(checksums: &BTreeMap<String, String>) -> String {
    let mut out = String::new();
    for (path, sha) in checksums {
        out.push_str(sha);
        out.push_str("  ");
        out.push_str(path);
        out.push('\n');
    }
    out
}

/// Parse `checksums.sha256`. Every line must be `<64 hex>  <relative path>`;
/// duplicates and traversal paths are refused.
pub fn parse_checksums(text: &str) -> Result<BTreeMap<String, String>, String> {
    let mut out = BTreeMap::new();
    for (idx, line) in text.lines().enumerate() {
        let line = line.trim_end_matches('\r');
        if line.trim().is_empty() {
            continue;
        }
        let mut parts = line.splitn(2, char::is_whitespace);
        let sha = parts.next().unwrap_or("").trim();
        let path = parts.next().unwrap_or("").trim_start();
        if !is_sha256_hex(sha) {
            return Err(format!(
                "checksums.sha256 line {}: expected a 64-character lowercase sha256, got `{sha}`",
                idx + 1
            ));
        }
        if path.is_empty() {
            return Err(format!(
                "checksums.sha256 line {}: missing file path",
                idx + 1
            ));
        }
        validate_archive_path(path).map_err(|e| format!("checksums.sha256 line {}: {e}", idx + 1))?;
        if path == MANIFEST_FILE || path == CHECKSUMS_FILE {
            return Err(format!(
                "checksums.sha256 must not list `{path}` (the manifest and the checksum file are \
                 not payload)"
            ));
        }
        if out.insert(path.to_string(), sha.to_ascii_lowercase()).is_some() {
            return Err(format!(
                "checksums.sha256 lists `{path}` more than once"
            ));
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Build
// ---------------------------------------------------------------------------

fn collect_files_from(
    root: &Path,
    prefix: &str,
    out: &mut BTreeMap<String, Vec<u8>>,
    skipped: &mut usize,
) -> Result<(), String> {
    // Count runtime/backup artifacts at the bundle root so the publish report can
    // say they were left out (they are never packaged).
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if is_runtime_artifact_name(&name, is_dir) {
                *skipped += 1;
            }
        }
    }
    for name in ["meta.json", "flow.json", "assets.json"] {
        let path = root.join(name);
        if path.is_file() {
            let bytes =
                std::fs::read(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
            out.insert(format!("{prefix}{name}"), bytes);
        }
    }
    let images = root.join("images");
    if images.is_dir() {
        collect_tree_files(&images, &format!("{prefix}images"), out, skipped)?;
    }
    Ok(())
}

fn is_runtime_artifact_name(name: &str, is_dir: bool) -> bool {
    if is_dir {
        return name == "state" || name == ".backup" || name.ends_with(".backup");
    }
    name.ends_with(".clicks.jsonl")
        || name.ends_with(".tmp")
        || name.ends_with(".bak")
        || name.ends_with(".backup")
}

fn collect_tree_files(
    root: &Path,
    prefix: &str,
    out: &mut BTreeMap<String, Vec<u8>>,
    skipped: &mut usize,
) -> Result<(), String> {
    let mut entries: Vec<_> = std::fs::read_dir(root)
        .map_err(|e| format!("read {}: {e}", root.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("read {}: {e}", root.display()))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let file_type = entry
            .file_type()
            .map_err(|e| format!("stat {}: {e}", path.display()))?;
        if file_type.is_symlink() {
            *skipped += 1;
            continue;
        }
        if file_type.is_dir() {
            if is_runtime_artifact_name(&name, true) {
                *skipped += 1;
                continue;
            }
            collect_tree_files(&path, &format!("{prefix}/{name}"), out, skipped)?;
        } else if file_type.is_file() {
            if is_runtime_artifact_name(&name, false) {
                *skipped += 1;
                continue;
            }
            let bytes =
                std::fs::read(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
            out.insert(format!("{prefix}/{name}"), bytes);
        } else {
            *skipped += 1;
        }
    }
    Ok(())
}

/// Parse the global asset store's `assets.json` (design: `--assets-dir`).
///
/// Returns `Ok(None)` when the store is not initialized (no `assets.json`), which
/// keeps the legacy task-local-only mode.
pub fn global_assets_file(assets_dir: &Path) -> Result<Option<AssetsFile>, String> {
    let path = assets_dir.join("assets.json");
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("read {}: {e}", path.display())),
    };
    let file: AssetsFile = serde_json::from_str(&text)
        .map_err(|e| format!("global assets `{}` is invalid JSON: {e}", path.display()))?;
    Ok(Some(file))
}

/// IDs that a flow may legitimately reference from the global asset store:
/// asset ids **and** collection ids (a flow references the collection id).
pub fn global_asset_ids(assets_dir: &Path) -> Result<Option<BTreeSet<String>>, String> {
    Ok(global_assets_file(assets_dir)?.map(|file| {
        let mut ids: BTreeSet<String> = file.items.into_iter().map(|a| a.id).collect();
        ids.extend(file.collections.into_iter().map(|c| c.id));
        ids
    }))
}

/// Publish-time validation. Returns every issue (lint + package rules) instead
/// of short-circuiting, so the Studio can show one complete list. On success it
/// returns non-fatal warnings (currently: an empty `state/` directory, which is
/// excluded from the package but must not block publishing).
///
/// `assets_dir` selects global-asset T4 mode when `assets/assets.json` exists;
/// otherwise the task-local assets are authoritative (bare-directory mode).
pub fn validate_for_publish(
    task_dir: &Path,
    tasks_dir: &Path,
    assets_dir: Option<&Path>,
) -> Result<Vec<String>, Vec<LintIssue>> {
    let mut issues = Vec::new();
    let mut warnings = Vec::new();
    let task_name = task_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();
    if let Err(e) = validate_task_name(&task_name) {
        issues.push(LintIssue::new("PKG", "task-dir", e));
    }
    if !task_dir.join("meta.json").is_file() {
        issues.push(LintIssue::new(
            "META",
            "meta.json",
            format!("missing ({})", task_dir.join("meta.json").display()),
        ));
    }
    if !task_dir.join("flow.json").is_file() {
        issues.push(LintIssue::new(
            "PKG",
            "flow.json",
            "missing; a publishable task must have a flow.json",
        ));
    }
    scan_runtime_artifacts(task_dir, "task", &mut issues, &mut warnings);
    if !issues.is_empty() {
        // A missing meta/flow cannot be linted meaningfully (the lint would
        // repeat the same missing-file errors with different codes).
        return Err(issues);
    }

    let global_assets = match assets_dir {
        Some(dir) => match global_assets_file(dir) {
            Ok(file) => file,
            Err(e) => {
                issues.push(LintIssue::new("PKG", "assets/assets.json", e));
                None
            }
        },
        None => None,
    };
    let external = global_assets.as_ref().map(|file| {
        let mut ids: BTreeSet<String> = file.items.iter().map(|a| a.id.clone()).collect();
        ids.extend(file.collections.iter().map(|c| c.id.clone()));
        ids
    });
    let bundle = match TaskBundle::open(task_dir.to_path_buf()) {
        Ok(bundle) => bundle,
        Err(e) => {
            issues.push(LintIssue::new("PKG", "task-dir", e));
            return Err(issues);
        }
    };
    bundle.lint_into_with_calls_and_global_assets(
        &mut issues,
        tasks_dir,
        external.as_ref(),
        global_assets.as_ref(),
    );
    if issues.is_empty() {
        Ok(warnings)
    } else {
        Err(issues)
    }
}

/// Is the directory present but empty? A failing `read_dir` is treated as "not
/// empty" so an unreadable `state/` still blocks the publish.
fn dir_is_empty(path: &Path) -> bool {
    std::fs::read_dir(path)
        .map(|mut entries| entries.next().is_none())
        .unwrap_or(false)
}

fn scan_runtime_artifacts(
    root: &Path,
    prefix: &str,
    issues: &mut Vec<LintIssue>,
    warnings: &mut Vec<String>,
) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let rel = format!("{prefix}/{name}");
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            if name == "state" {
                // An empty runtime-state dir is harmless (it is never packaged),
                // but real state files must not silently disappear into a
                // published package.
                if dir_is_empty(&path) {
                    warnings.push(format!(
                        "`{rel}` is empty and is not packaged (runtime state is never published)"
                    ));
                } else {
                    issues.push(LintIssue::new(
                        "PKG",
                        rel,
                        "runtime state directory must not be published (move it out of the task or delete it)",
                    ));
                }
                continue;
            }
            if name == ".backup" || name.ends_with(".backup") {
                issues.push(LintIssue::new(
                    "PKG",
                    rel,
                    "runtime state directory must not be published (move it out of the task or delete it)",
                ));
                continue;
            }
            scan_runtime_artifacts(&path, &rel, issues, warnings);
        } else if file_type.is_file() && name.ends_with(".clicks.jsonl") {
            issues.push(LintIssue::new(
                "PKG",
                rel,
                "runtime click log must not be published",
            ));
        }
    }
}

struct DepTask {
    id: String,
    version: u32,
    dir: PathBuf,
    flow_text: String,
    meta: Meta,
}

fn read_dep_task(tasks_dir: &Path, task_id: &str, at: &str) -> Result<DepTask, String> {
    validate_task_name(task_id)?;
    let dir = tasks_dir.join(task_id);
    let meta_path = dir.join("meta.json");
    let text = std::fs::read_to_string(&meta_path)
        .map_err(|e| format!("{at}: read {}: {e}", meta_path.display()))?;
    let meta = Meta::from_json(&text)
        .map_err(|e| format!("{at}: meta.json is invalid: {e}"))?;
    let flow_path = dir.join("flow.json");
    let flow_text = std::fs::read_to_string(&flow_path)
        .map_err(|e| format!("{at}: read {}: {e}", flow_path.display()))?;
    Ok(DepTask {
        id: task_id.to_string(),
        version: meta.version.unwrap_or(1),
        dir,
        flow_text,
        meta,
    })
}

fn walk_dependencies(
    task_id: &str,
    flow_text: &str,
    tasks_dir: &Path,
    stack: &mut Vec<String>,
    depth: usize,
    out: &mut BTreeMap<String, DepTask>,
) -> Result<(), String> {
    if depth > MAX_CALL_DEPTH {
        return Err(format!("call_task nesting exceeds {MAX_CALL_DEPTH}"));
    }
    let flow = oas_flow::Flow::from_json(flow_text)
        .map_err(|e| format!("task `{task_id}`: flow.json: {e}"))?;
    for use_site in flow.call_task_uses() {
        let child = use_site.task;
        if stack.iter().any(|s| s == &child) {
            return Err(format!(
                "call_task cycle: {} -> {child}",
                stack.join(" -> ")
            ));
        }
        if depth + 1 > MAX_CALL_DEPTH {
            return Err(format!(
                "call_task nesting exceeds {MAX_CALL_DEPTH} (at `{child}`)"
            ));
        }
        if out.contains_key(&child) {
            continue;
        }
        let at = format!("call_task `{child}` (called from `{task_id}`)");
        let dep = read_dep_task(tasks_dir, &child, &at)?;
        let next_flow = dep.flow_text.clone();
        out.insert(child.clone(), dep);
        stack.push(child.clone());
        walk_dependencies(&child, &next_flow, tasks_dir, stack, depth + 1, out)?;
        stack.pop();
    }
    Ok(())
}

/// One raw `call_task` site from the authored root flow.
fn collect_call_deps(
    flow_json: &str,
    tasks_dir: &Path,
) -> Result<Vec<CallDepSchema>, String> {
    let value: serde_json::Value =
        serde_json::from_str(flow_json).map_err(|e| format!("flow.json: {e}"))?;
    let steps = value
        .get("steps")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "flow.json: missing `steps` array".to_string())?;
    let mut out = Vec::new();
    walk_call_deps(steps, tasks_dir, &mut out)?;
    Ok(out)
}

fn walk_call_deps(
    steps: &[serde_json::Value],
    tasks_dir: &Path,
    out: &mut Vec<CallDepSchema>,
) -> Result<(), String> {
    for step in steps {
        let op = step.get("op").and_then(|v| v.as_str()).unwrap_or("");
        let args = step.get("args");
        if op == "call_task" {
            let child = args
                .and_then(|a| a.get("task"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| "call_task requires args.task".to_string())?
                .to_string();
            let mut mapping = BTreeMap::new();
            if let Some(params) = args.and_then(|a| a.get("params")) {
                let obj = params.as_object().ok_or_else(|| {
                    format!("call_task `{child}`: args.params must be an object")
                })?;
                for (key, value) in obj {
                    let src = value.as_str().ok_or_else(|| {
                        format!("call_task `{child}`: args.params.{key} must be a string expression")
                    })?;
                    mapping.insert(key.clone(), src.to_string());
                }
            }
            let inherit = args
                .and_then(|a| a.get("inherit"))
                .and_then(|v| v.as_str())
                .unwrap_or("all")
                .to_string();
            let child_variables = read_dep_task(tasks_dir, &child, "variables_schema")
                .map(|dep| {
                    dep.meta
                        .effective_variables()
                        .into_iter()
                        .filter(|v| v.is_input())
                        .map(|v| v.key)
                        .collect()
                })
                .unwrap_or_default();
            out.push(CallDepSchema {
                child,
                mapping,
                inherit,
                child_variables,
            });
        }
        for key in ["then", "else", "body"] {
            if let Some(nested) = step.get(key).and_then(|v| v.as_array()) {
                walk_call_deps(nested, tasks_dir, out)?;
            }
        }
        if let Some(nested) = args.and_then(|a| a.get("body")).and_then(|v| v.as_array()) {
            walk_call_deps(nested, tasks_dir, out)?;
        }
    }
    Ok(())
}

struct AssetPlan {
    design_resolution: Option<[u32; 2]>,
    refs: Vec<AssetRef>,
    payload: BTreeMap<String, Vec<u8>>,
}

fn plan_assets(
    referenced: &BTreeSet<String>,
    assets_dir: &Path,
    warnings: &mut Vec<String>,
) -> Result<AssetPlan, String> {
    let mut payload = BTreeMap::new();
    let mut refs = Vec::new();
    let mut plain_refs: Vec<String> = Vec::new();
    for id in referenced {
        if id.starts_with("lib:") {
            warnings.push(format!(
                "global library selector `{id}` is not packaged in Phase 1 (the Runner must \
                 provide it locally)"
            ));
        } else {
            plain_refs.push(id.clone());
        }
    }
    let store_path = assets_dir.join("assets.json");
    let Ok(text) = std::fs::read_to_string(&store_path) else {
        if !plain_refs.is_empty() {
            warnings.push(format!(
                "global asset store is not initialized at {}; {} referenced asset(s) were not \
                 snapshotted (task-local assets are still packaged)",
                assets_dir.display(),
                plain_refs.len()
            ));
        }
        return Ok(AssetPlan {
            design_resolution: None,
            refs,
            payload,
        });
    };
    let store: AssetsFile = serde_json::from_str(&text)
        .map_err(|e| format!("global assets `{}` is invalid JSON: {e}", store_path.display()))?;
    let mut selected_items: BTreeSet<String> = BTreeSet::new();
    let mut selected_collections: BTreeSet<String> = BTreeSet::new();
    for id in &plain_refs {
        if store.items.iter().any(|a| &a.id == id) {
            selected_items.insert(id.clone());
            continue;
        }
        if let Some(collection) = store.collections.iter().find(|c| &c.id == id) {
            selected_collections.insert(id.clone());
            for member in &collection.members {
                if !store.items.iter().any(|a| &a.id == member) {
                    return Err(format!(
                        "global collection `{id}` references missing asset `{member}`"
                    ));
                }
                selected_items.insert(member.clone());
            }
            continue;
        }
        return Err(format!(
            "flow references global asset `{id}`, but it is not declared in {}",
            store_path.display()
        ));
    }
    let mut items = Vec::new();
    for item in &store.items {
        if !selected_items.contains(&item.id) {
            continue;
        }
        let sha = if let Some(file) = item.file.as_deref() {
            validate_archive_path(file)
                .map_err(|e| format!("global asset `{}`: {e}", item.id))?;
            let src = assets_dir.join(file);
            let bytes =
                std::fs::read(&src).map_err(|e| format!("read global asset {}: {e}", src.display()))?;
            payload.insert(format!("assets/{file}"), bytes.clone());
            sha256_bytes(&bytes)
        } else {
            let canonical = serde_json::to_vec(item)
                .map_err(|e| format!("serialize global asset `{}`: {e}", item.id))?;
            sha256_bytes(&canonical)
        };
        refs.push(AssetRef {
            id: item.id.clone(),
            kind: item.kind.clone(),
            sha256: sha,
        });
        items.push(item.clone());
    }
    let collections: Vec<_> = store
        .collections
        .iter()
        .filter(|c| selected_collections.contains(&c.id))
        .cloned()
        .collect();
    refs.sort_by(|a, b| a.id.cmp(&b.id));
    if !items.is_empty() || !collections.is_empty() {
        let snapshot = AssetsFile {
            schema_version: SCHEMA_VERSION,
            design_resolution: store.design_resolution,
            items,
            collections,
        };
        let mut bytes = serde_json::to_vec_pretty(&snapshot)
            .map_err(|e| format!("serialize assets snapshot: {e}"))?;
        bytes.push(b'\n');
        payload.insert("assets/assets.json".to_string(), bytes);
    }
    Ok(AssetPlan {
        design_resolution: store.design_resolution,
        refs,
        payload,
    })
}

fn read_design_resolution(task_dir: &Path) -> Option<[u32; 2]> {
    let text = std::fs::read_to_string(task_dir.join("assets.json")).ok()?;
    let file: AssetsFile = serde_json::from_str(&text).ok()?;
    file.design_resolution
}

fn resolve_output_path(out: &Path, task_id: &str, version: u32) -> Result<PathBuf, String> {
    if out.as_os_str().is_empty() {
        return Err("output path is empty".to_string());
    }
    let looks_like_file = !out.is_dir()
        && out
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case(PACKAGE_EXTENSION))
            .unwrap_or(false);
    if looks_like_file {
        Ok(out.to_path_buf())
    } else {
        Ok(out.join(format!("{task_id}-v{version}.{PACKAGE_EXTENSION}")))
    }
}

fn write_zip(
    path: &Path,
    entries: &BTreeMap<String, Vec<u8>>,
    reproducible: bool,
) -> Result<(), String> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("create {}: {e}", parent.display()))?;
    }
    let file = std::fs::File::create(path)
        .map_err(|e| format!("create {}: {e}", path.display()))?;
    let mut zip = zip::ZipWriter::new(file);
    let mut options = zip::write::FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o644);
    if reproducible {
        options = options.last_modified_time(zip::DateTime::default());
    }
    for (name, bytes) in entries {
        zip.start_file(name.as_str(), options)
            .map_err(|e| format!("zip start `{name}`: {e}"))?;
        zip.write_all(bytes)
            .map_err(|e| format!("zip write `{name}`: {e}"))?;
    }
    zip.finish().map_err(|e| format!("zip finish: {e}"))?;
    Ok(())
}

/// Low-level writer for tooling/tests that already assembled a full entry map
/// (including `manifest.json` / `checksums.sha256`). Entry paths are written
/// verbatim, so callers must validate them; `build_package` is the safe path.
pub fn write_archive(
    path: &Path,
    entries: &BTreeMap<String, Vec<u8>>,
    reproducible: bool,
) -> Result<(), String> {
    write_zip(path, entries, reproducible)
}

/// Build one `.oaspkg`.
///
/// `out` may be a directory (default file name `<task_id>-v<version>.oaspkg`) or
/// an explicit `*.oaspkg` path. An existing target is refused unless
/// `opts.force`.
pub fn build_package(
    task_dir: &Path,
    out: &Path,
    opts: &BuildOptions,
) -> PackageResult<BuildOutcome> {
    if !task_dir.is_dir() {
        return Err(PackageError::Other(format!(
            "task directory `{}` does not exist",
            task_dir.display()
        )));
    }
    let task_name = task_dir
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| PackageError::Other("task directory name is not UTF-8".to_string()))?;
    validate_task_name(task_name)?;

    let meta_path = task_dir.join("meta.json");
    let meta_text = std::fs::read_to_string(&meta_path)
        .map_err(|e| PackageError::Other(format!("read {}: {e}", meta_path.display())))?;
    let meta = Meta::from_json(&meta_text)
        .map_err(|e| PackageError::Other(format!("{}: {e}", meta_path.display())))?;
    let task_id = meta.task_id().to_string();
    validate_task_name(&task_id)?;
    let version = meta.version.unwrap_or(1);
    if version == 0 {
        return Err(PackageError::Other(
            "meta.version must be >= 1 for a publishable task".to_string(),
        ));
    }

    let mut warnings = Vec::new();
    if task_id != task_name {
        warnings.push(format!(
            "manifest task_id `{task_id}` differs from the bundle directory `{task_name}`"
        ));
    }
    match validate_for_publish(task_dir, &opts.tasks_dir, Some(&opts.assets_dir)) {
        Ok(extra) => warnings.extend(extra),
        Err(issues) => return Err(PackageError::Validation(issues)),
    }

    let flow_path = task_dir.join("flow.json");
    let root_flow_text = std::fs::read_to_string(&flow_path)
        .map_err(|e| PackageError::Other(format!("read {}: {e}", flow_path.display())))?;

    let mut deps = BTreeMap::new();
    let mut stack = vec![task_name.to_string()];
    walk_dependencies(
        task_name,
        &root_flow_text,
        &opts.tasks_dir,
        &mut stack,
        0,
        &mut deps,
    )?;

    // Validate every resolved dependency bundle (T1/META/T3..T7), not just the
    // root: an embedded dep with a broken flow would fail on the Runner.
    // seq129-fix: pass the parsed global store too, so a dep that binds a global
    // ocr_var does not get a false T8.
    let global_assets = global_assets_file(&opts.assets_dir)?;
    let external = global_assets.as_ref().map(|file| {
        let mut ids: BTreeSet<String> = file.items.iter().map(|a| a.id.clone()).collect();
        ids.extend(file.collections.iter().map(|c| c.id.clone()));
        ids
    });
    for dep in deps.values() {
        let bundle = TaskBundle::open(dep.dir.clone())?;
        let mut issues = Vec::new();
        bundle.lint_into_with_calls_and_global_assets(
            &mut issues,
            &opts.tasks_dir,
            external.as_ref(),
            global_assets.as_ref(),
        );
        if !issues.is_empty() {
            let prefixed = issues
                .into_iter()
                .map(|mut issue| {
                    issue.path = format!("deps/{}/{}", dep.id, issue.path);
                    issue
                })
                .collect();
            return Err(PackageError::Validation(prefixed));
        }
    }

    // ---- payload: task/ -------------------------------------------------
    let mut payload: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let mut skipped = 0usize;
    collect_files_from(task_dir, "task/", &mut payload, &mut skipped)?;
    let readme = task_dir.join("README.md");
    if readme.is_file() {
        let bytes = std::fs::read(&readme)
            .map_err(|e| PackageError::Other(format!("read {}: {e}", readme.display())))?;
        payload.insert("README.md".to_string(), bytes);
    }

    // ---- payload: deps/<id>/ -------------------------------------------
    if opts.include_deps {
        for dep in deps.values() {
            collect_files_from(
                &dep.dir,
                &format!("deps/{}/", dep.id),
                &mut payload,
                &mut skipped,
            )?;
        }
    } else if !deps.is_empty() {
        warnings.push(format!(
            "{} dependency task(s) are declared but not embedded; the Runner must install them",
            deps.len()
        ));
    }
    if skipped > 0 {
        warnings.push(format!(
            "{skipped} runtime/backup artifact(s) were excluded from the package"
        ));
    }

    // ---- flow_ops -------------------------------------------------------
    let mut flow_ops = BTreeSet::new();
    flow_ops.extend(derive_flow_ops(&root_flow_text)?);
    if opts.include_deps {
        for dep in deps.values() {
            flow_ops.extend(derive_flow_ops(&dep.flow_text)?);
        }
    }
    let flow_ops: Vec<String> = flow_ops.into_iter().collect();

    // ---- payload: assets/ snapshot -------------------------------------
    let mut referenced: BTreeSet<String> = oas_flow::Flow::from_json(&root_flow_text)
        .map_err(|e| PackageError::Other(format!("flow.json: {e}")))?
        .referenced_assets();
    if opts.include_deps {
        for dep in deps.values() {
            let flow = oas_flow::Flow::from_json(&dep.flow_text)
                .map_err(|e| PackageError::Other(format!("deps/{}/flow.json: {e}", dep.id)))?;
            referenced.extend(flow.referenced_assets());
        }
    }
    let asset_plan = plan_assets(&referenced, &opts.assets_dir, &mut warnings)?;
    payload.extend(asset_plan.payload);

    // ---- bounded payload ------------------------------------------------
    // The reader counts every zip entry, including `manifest.json` and
    // `checksums.sha256`; keep the writer's check on the same total so a package
    // it builds can always be read back.
    let total_entries = payload.len() + 2;
    if total_entries > MAX_PACKAGE_ENTRIES {
        return Err(PackageError::Other(format!(
            "package has {total_entries} entries ({} payload + 2 control); the limit is \
             {MAX_PACKAGE_ENTRIES}",
            payload.len()
        )));
    }
    let mut total = 0u64;
    for (path, bytes) in &payload {
        let len = bytes.len() as u64;
        if len > MAX_PACKAGE_ENTRY_BYTES {
            return Err(PackageError::Other(format!(
                "package entry `{path}` is {len} bytes; per-entry limit is {MAX_PACKAGE_ENTRY_BYTES}"
            )));
        }
        total = total
            .checked_add(len)
            .ok_or_else(|| PackageError::Other("package size overflow".to_string()))?;
        validate_archive_path(path)?;
    }
    if total > MAX_PACKAGE_TOTAL_BYTES {
        return Err(PackageError::Other(format!(
            "package payload is {total} bytes; the limit is {MAX_PACKAGE_TOTAL_BYTES}"
        )));
    }

    // ---- checksums + manifest ------------------------------------------
    let mut checksums = BTreeMap::new();
    let mut files = Vec::new();
    for (path, bytes) in &payload {
        let sha = sha256_bytes(bytes);
        checksums.insert(path.clone(), sha.clone());
        files.push(FileEntry {
            path: path.clone(),
            sha256: sha,
            bytes: bytes.len() as u64,
        });
    }
    let checksums_text = checksums_text(&checksums);
    // Package identity is the *payload* digest (the checksums file), because a
    // package cannot contain its own final archive hash without a fixed-point.
    // The archive hash is returned separately in `BuildOutcome.sha256`.
    let content_sha = sha256_bytes(checksums_text.as_bytes());
    let package_id = format!("pkg-{task_id}-{version}-{}", &content_sha[..12]);

    let flow_ops_for_manifest = flow_ops.clone();
    let manifest = Manifest {
        package_schema: PACKAGE_SCHEMA,
        package_id,
        task_id: task_id.clone(),
        task_version: version,
        name: meta.name.clone(),
        created_at_ms: if opts.reproducible { 0 } else { now_ms() },
        tool: ToolInfo {
            version: opts.tool_version.clone(),
            git: opts.git.clone(),
            dirty: opts.dirty,
            surface: opts.surface.clone(),
        },
        source: SourceInfo {
            repo: opts.repo.clone(),
            commit: opts.commit.clone(),
        },
        engine: EngineInfo {
            template_version: meta.template_version.unwrap_or(TEMPLATE_VERSION),
            min_engine: ENGINE_VERSION.to_string(),
            flow_ops: flow_ops_for_manifest,
        },
        files,
        variables_schema: VariablesSchema {
            own: meta.effective_variables(),
            call_deps: collect_call_deps(&root_flow_text, &opts.tasks_dir)?,
        },
        dependencies: deps
            .values()
            .map(|dep| Dependency {
                task_id: dep.id.clone(),
                version: dep.version,
                included: opts.include_deps,
            })
            .collect(),
        asset_refs: asset_plan.refs,
        platform: Platform {
            os: platform_os(),
            needs_adb: true,
            design_resolution: asset_plan
                .design_resolution
                .or_else(|| read_design_resolution(task_dir)),
        },
        map_requirements: MapRequirements {
            nodes: opts
                .map_nodes
                .clone()
                .map(|s| s.into_iter().collect())
                .unwrap_or_default(),
        },
        signature: None,
    };
    let manifest_text = serde_json::to_string_pretty(&manifest)
        .map_err(|e| PackageError::Other(format!("serialize manifest: {e}")))?;

    let mut entries = payload;
    entries.insert(MANIFEST_FILE.to_string(), format!("{manifest_text}\n").into_bytes());
    entries.insert(CHECKSUMS_FILE.to_string(), checksums_text.into_bytes());

    // ---- write the archive atomically ----------------------------------
    let target = resolve_output_path(out, &task_id, version)?;
    if target.exists() && !opts.force {
        return Err(PackageError::Exists(target));
    }
    let parent = target
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)
        .map_err(|e| PackageError::Other(format!("create {}: {e}", parent.display())))?;
    let seq = WRITE_SEQ.fetch_add(1, Ordering::Relaxed);
    let tmp = parent.join(format!(
        ".{}.tmp-{}-{seq}",
        target
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("package"),
        std::process::id()
    ));
    if let Err(e) = write_zip(&tmp, &entries, opts.reproducible) {
        let _ = std::fs::remove_file(&tmp);
        return Err(PackageError::Other(e));
    }
    if target.exists() {
        if let Err(e) = std::fs::remove_file(&target) {
            let _ = std::fs::remove_file(&tmp);
            return Err(PackageError::Other(format!(
                "remove existing {}: {e}",
                target.display()
            )));
        }
    }
    if let Err(e) = std::fs::rename(&tmp, &target) {
        let _ = std::fs::remove_file(&tmp);
        return Err(PackageError::Other(format!(
            "rename {} -> {}: {e}",
            tmp.display(),
            target.display()
        )));
    }
    let sha256 = sha256_file(&target)?;
    // §B.3: publish next to the package with a `sha256sum -c` compatible sidecar.
    // A sidecar failure must not invalidate an otherwise good package.
    let file_name = target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| format!("{task_id}-v{version}.{PACKAGE_EXTENSION}"));
    let sidecar = target.with_extension(format!("{PACKAGE_EXTENSION}.sha256"));
    if let Err(e) = std::fs::write(&sidecar, format!("{sha256}  {file_name}\n")) {
        warnings.push(format!(
            "package built, but writing {} failed: {e}",
            sidecar.display()
        ));
    }
    Ok(BuildOutcome {
        path: target,
        sha256,
        manifest,
        warnings,
    })
}

// ---------------------------------------------------------------------------
// Read / verify / extract
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct PackageArchive {
    pub manifest: Manifest,
    pub checksums: BTreeMap<String, String>,
    /// Payload files only (`manifest.json` / `checksums.sha256` are excluded).
    pub files: BTreeMap<String, Vec<u8>>,
}

impl PackageArchive {
    /// Reconstruct the `checksums.sha256` text from the parsed map.
    pub fn checksums_text(&self) -> String {
        checksums_text(&self.checksums)
    }
}

/// One entry's name/size policy, shared by the read and the extract paths.
///
/// The limits are checked against the zip **header** (declared sizes) before any
/// decompression, so a malicious archive is refused without allocating its
/// payload; the read path additionally caps the actual decompressed output.
fn check_entry_limits(name: &str, declared: u64, compressed: u64) -> Result<(), String> {
    if declared > MAX_PACKAGE_ENTRY_BYTES {
        return Err(format!(
            "package entry `{name}` declares {declared} bytes; per-entry limit is \
             {MAX_PACKAGE_ENTRY_BYTES}"
        ));
    }
    if declared >= MIN_RATIO_CHECK_BYTES {
        // `compressed_size == 0` on a non-empty declared entry is a lie; treat it
        // as 1 byte so the ratio check still fires.
        let compressed = compressed.max(1);
        if declared > compressed.saturating_mul(MAX_COMPRESSION_RATIO) {
            return Err(format!(
                "package entry `{name}` has a compression ratio above {MAX_COMPRESSION_RATIO}:1 \
                 (declares {declared} bytes from {compressed} compressed bytes); refusing a \
                 possible zip bomb"
            ));
        }
    }
    Ok(())
}

/// `ZipArchive::by_index` with an explicit encrypted-entry refusal.
///
/// zip 0.6.6 has no `ZipFile::encrypted()` accessor; its reader refuses an
/// encrypted entry by returning `PASSWORD_REQUIRED` from `by_index`. That is the
/// equivalent API judgement, so it is mapped to a deliberate package error
/// instead of a generic zip error.
fn by_index_checked<'a>(
    archive: &'a mut zip::ZipArchive<std::fs::File>,
    idx: usize,
) -> Result<zip::read::ZipFile<'a>, String> {
    match archive.by_index(idx) {
        Ok(entry) => Ok(entry),
        Err(zip::result::ZipError::UnsupportedArchive(msg))
            if msg == zip::result::ZipError::PASSWORD_REQUIRED =>
        {
            Err(format!(
                "package entry {idx} is encrypted; encrypted packages are not supported"
            ))
        }
        Err(e) => Err(format!("zip entry {idx}: {e}")),
    }
}

/// Metadata-only pre-scan of a zip: entry count, declared sizes, compression
/// ratio and encryption. No payload is decompressed, so this is cheap; it is
/// run by both [`read_package`] and [`extract_to`] so neither path depends on
/// the other's limits.
fn check_zip_limits(zip_path: &Path) -> Result<(), String> {
    let file = std::fs::File::open(zip_path)
        .map_err(|e| format!("open package {}: {e}", zip_path.display()))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| format!("{} is not a readable zip: {e}", zip_path.display()))?;
    if archive.len() > MAX_PACKAGE_ENTRIES {
        return Err(format!(
            "package has {} entries; the limit is {MAX_PACKAGE_ENTRIES}",
            archive.len()
        ));
    }
    let mut total = 0u64;
    for idx in 0..archive.len() {
        let entry = by_index_checked(&mut archive, idx)?;
        let name = entry.name().to_string();
        let is_dir = entry.is_dir() || name.ends_with('/');
        validate_archive_path_inner(&name, is_dir)?;
        if is_dir {
            continue;
        }
        check_entry_limits(&name, entry.size(), entry.compressed_size())?;
        total = total
            .checked_add(entry.size())
            .ok_or_else(|| "package size overflow".to_string())?;
        if total > MAX_PACKAGE_TOTAL_BYTES {
            return Err(format!(
                "package declares {total} bytes; the limit is {MAX_PACKAGE_TOTAL_BYTES}"
            ));
        }
    }
    Ok(())
}

fn payload_map_from_zip(
    zip_path: &Path,
) -> Result<(Manifest, BTreeMap<String, String>, BTreeMap<String, Vec<u8>>), String> {
    check_zip_limits(zip_path)?;
    let file = std::fs::File::open(zip_path)
        .map_err(|e| format!("open package {}: {e}", zip_path.display()))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| format!("{} is not a readable zip: {e}", zip_path.display()))?;
    if archive.len() > MAX_PACKAGE_ENTRIES {
        return Err(format!(
            "package has {} entries; the limit is {MAX_PACKAGE_ENTRIES}",
            archive.len()
        ));
    }
    let mut entries: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let mut total = 0u64;
    for idx in 0..archive.len() {
        let mut entry = by_index_checked(&mut archive, idx)?;
        let name = entry.name().to_string();
        let is_dir = entry.is_dir() || name.ends_with('/');
        validate_archive_path_inner(&name, is_dir)?;
        if is_dir {
            continue;
        }
        let declared = entry.size();
        // Defensive re-check: `check_zip_limits` already scanned the metadata of
        // this same archive; keeping the policy next to the read makes the
        // bound impossible to regress by deleting the pre-scan.
        check_entry_limits(&name, declared, entry.compressed_size())?;
        total = total
            .checked_add(declared)
            .ok_or_else(|| "package size overflow".to_string())?;
        if total > MAX_PACKAGE_TOTAL_BYTES {
            return Err(format!(
                "package declares {total} bytes; the limit is {MAX_PACKAGE_TOTAL_BYTES}"
            ));
        }
        // Never read past the declared size: a zip bomb must be refused after at
        // most `declared + 1` bytes instead of being fully materialized.
        let mut bytes = Vec::with_capacity(declared.min(64 * 1024) as usize);
        let mut limited = entry.by_ref().take(declared + 1);
        limited
            .read_to_end(&mut bytes)
            .map_err(|e| format!("read package entry `{name}`: {e}"))?;
        if bytes.len() as u64 > declared {
            return Err(format!(
                "package entry `{name}` produced more than its declared {declared} bytes; \
                 refusing a possible zip bomb"
            ));
        }
        if bytes.len() as u64 != declared {
            return Err(format!(
                "package entry `{name}` declared {declared} bytes but produced {}",
                bytes.len()
            ));
        }
        if entries.insert(name.clone(), bytes).is_some() {
            return Err(format!("package contains duplicate entry `{name}`"));
        }
    }
    let manifest_bytes = entries
        .remove(MANIFEST_FILE)
        .ok_or_else(|| format!("package is missing `{MANIFEST_FILE}`"))?;
    let manifest: Manifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|e| format!("{MANIFEST_FILE}: invalid JSON: {e}"))?;
    if manifest.package_schema != PACKAGE_SCHEMA {
        return Err(format!(
            "unsupported package_schema {} (this engine supports {PACKAGE_SCHEMA}); \
             re-publish the task with the V2 variable model",
            manifest.package_schema
        ));
    }
    validate_task_name(&manifest.task_id)?;
    if manifest.task_version == 0 {
        return Err("manifest.task_version must be >= 1".to_string());
    }
    let mut dependency_ids = BTreeSet::new();
    for dep in &manifest.dependencies {
        validate_task_name(&dep.task_id)?;
        if dep.version == 0 {
            return Err(format!(
                "manifest.dependencies `{}` has version 0",
                dep.task_id
            ));
        }
        if !dependency_ids.insert(dep.task_id.clone()) {
            return Err(format!(
                "manifest.dependencies lists `{}` more than once",
                dep.task_id
            ));
        }
    }
    let checksums_bytes = entries
        .remove(CHECKSUMS_FILE)
        .ok_or_else(|| format!("package is missing `{CHECKSUMS_FILE}`"))?;
    let checksums_text = String::from_utf8(checksums_bytes)
        .map_err(|e| format!("{CHECKSUMS_FILE} is not UTF-8: {e}"))?;
    let checksums = parse_checksums(&checksums_text)?;
    Ok((manifest, checksums, entries))
}

/// Read + fully verify one package **without extracting it**.
///
/// Verification: every zip entry path is safe, every checksum matches, the
/// checksum set equals the payload set, and `manifest.files` agrees with both.
pub fn read_package(zip_path: &Path) -> Result<PackageArchive, String> {
    let (manifest, checksums, files) = payload_map_from_zip(zip_path)?;

    let payload_paths: BTreeSet<&String> = files.keys().collect();
    let listed_paths: BTreeSet<&String> = checksums.keys().collect();
    if payload_paths != listed_paths {
        let missing: Vec<&String> = listed_paths.difference(&payload_paths).copied().collect();
        let extra: Vec<&String> = payload_paths.difference(&listed_paths).copied().collect();
        return Err(format!(
            "checksums/payload mismatch: missing from package {:?}, not listed in checksums {:?}",
            missing, extra
        ));
    }
    for (path, expected) in &checksums {
        let bytes = files
            .get(path)
            .ok_or_else(|| format!("checksums lists `{path}` but the package is missing it"))?;
        let actual = sha256_bytes(bytes);
        if &actual != expected {
            return Err(format!(
                "checksum mismatch for `{path}`: expected {expected}, got {actual}"
            ));
        }
    }

    let manifest_paths: BTreeSet<&String> =
        manifest.files.iter().map(|f| &f.path).collect();
    if manifest_paths.len() != manifest.files.len() {
        return Err("manifest.files contains duplicate paths".to_string());
    }
    if manifest_paths != listed_paths {
        return Err(format!(
            "manifest.files does not match checksums.sha256 (manifest {}, checksums {})",
            manifest.files.len(),
            checksums.len()
        ));
    }
    for entry in &manifest.files {
        let bytes = files
            .get(&entry.path)
            .ok_or_else(|| format!("manifest lists `{}` but the package is missing it", entry.path))?;
        if entry.bytes != bytes.len() as u64 {
            return Err(format!(
                "manifest.files bytes mismatch for `{}`: manifest {}, actual {}",
                entry.path,
                entry.bytes,
                bytes.len()
            ));
        }
        let sha = checksums
            .get(&entry.path)
            .ok_or_else(|| format!("manifest lists `{}` but checksums does not", entry.path))?;
        if &entry.sha256 != sha {
            return Err(format!(
                "manifest/checksums sha256 disagree for `{}`",
                entry.path
            ));
        }
    }
    Ok(PackageArchive {
        manifest,
        checksums,
        files,
    })
}

fn walk_extracted_files(
    root: &Path,
    current: &Path,
    out: &mut BTreeSet<String>,
) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    let entries = match std::fs::read_dir(current) {
        Ok(entries) => entries,
        Err(e) => {
            errors.push(format!("read {}: {e}", current.display()));
            return Err(errors);
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => {
                errors.push(format!("read {}: {e}", current.display()));
                continue;
            }
        };
        let path = entry.path();
        let file_type = match entry.file_type() {
            Ok(t) => t,
            Err(e) => {
                errors.push(format!("stat {}: {e}", path.display()));
                continue;
            }
        };
        if file_type.is_symlink() {
            errors.push(format!(
                "`{}` is a symlink; packages may only contain regular files",
                path.display()
            ));
            continue;
        }
        if file_type.is_dir() {
            if let Err(mut nested) = walk_extracted_files(root, &path, out) {
                errors.append(&mut nested);
            }
            continue;
        }
        match path.strip_prefix(root).map(relative_slash_path) {
            Ok(Ok(rel)) => {
                out.insert(rel);
            }
            Ok(Err(e)) => errors.push(e),
            Err(e) => errors.push(format!("strip prefix {}: {e}", path.display())),
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Verify an unpacked package directory against its `checksums.sha256`.
///
/// Exact set comparison: a missing file **and** an extra file are both errors.
/// `manifest.json` and `checksums.sha256` are not payload and are skipped.
pub fn verify_checksums(dir: &Path) -> Result<(), Vec<String>> {
    let checksum_path = dir.join(CHECKSUMS_FILE);
    let text = match std::fs::read_to_string(&checksum_path) {
        Ok(text) => text,
        Err(e) => return Err(vec![format!("read {}: {e}", checksum_path.display())]),
    };
    let checksums = match parse_checksums(&text) {
        Ok(map) => map,
        Err(e) => return Err(vec![e]),
    };
    let mut actual = BTreeSet::new();
    if let Err(errors) = walk_extracted_files(dir, dir, &mut actual) {
        return Err(errors);
    }
    actual.remove(MANIFEST_FILE);
    actual.remove(CHECKSUMS_FILE);
    let expected: BTreeSet<String> = checksums.keys().cloned().collect();
    let mut errors = Vec::new();
    for missing in expected.difference(&actual) {
        errors.push(format!("file `{missing}` is listed but missing on disk"));
    }
    for extra in actual.difference(&expected) {
        errors.push(format!("file `{extra}` exists but is not listed in {CHECKSUMS_FILE}"));
    }
    for (path, sha) in &checksums {
        let file = dir.join(path);
        if !file.is_file() {
            continue;
        }
        match sha256_file(&file) {
            Ok(actual) if &actual == sha => {}
            Ok(actual) => errors.push(format!(
                "checksum mismatch for `{path}`: expected {sha}, got {actual}"
            )),
            Err(e) => errors.push(e),
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Extract a verified package into `dest`.
///
/// `dest` must not exist (or be empty): the installer always uses a fresh
/// staging directory, and refusing to merge into a non-empty directory is one
/// more zip-slip/overwrite guard. Entry paths are validated again here, and the
/// zip bomb limits are re-checked on this path (not only inside `read_package`).
pub fn extract_to(zip_path: &Path, dest: &Path) -> Result<(), String> {
    check_zip_limits(zip_path)?;
    let archive = read_package(zip_path)?;
    if dest.exists() {
        if !dest.is_dir() {
            return Err(format!(
                "extract destination `{}` exists and is not a directory",
                dest.display()
            ));
        }
        let mut entries = std::fs::read_dir(dest)
            .map_err(|e| format!("read {}: {e}", dest.display()))?;
        if entries.next().is_some() {
            return Err(format!(
                "extract destination `{}` is not empty; refusing to overwrite",
                dest.display()
            ));
        }
    } else {
        std::fs::create_dir_all(dest)
            .map_err(|e| format!("create {}: {e}", dest.display()))?;
    }
    for (rel, bytes) in &archive.files {
        validate_archive_path(rel)?;
        let out_path = dest.join(rel);
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("create {}: {e}", parent.display()))?;
        }
        std::fs::write(&out_path, bytes)
            .map_err(|e| format!("write {}: {e}", out_path.display()))?;
    }
    let manifest = serde_json::to_string_pretty(&archive.manifest)
        .map_err(|e| format!("serialize manifest: {e}"))?;
    std::fs::write(dest.join(MANIFEST_FILE), format!("{manifest}\n"))
        .map_err(|e| format!("write {MANIFEST_FILE}: {e}"))?;
    std::fs::write(dest.join(CHECKSUMS_FILE), archive.checksums_text())
        .map_err(|e| format!("write {CHECKSUMS_FILE}: {e}"))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            static SEQ: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "oas-package-test-{}-{tag}-{}",
                std::process::id(),
                SEQ.fetch_add(1, Ordering::Relaxed)
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

    /// Create `<tasks>/<id>/meta.json` + `flow.json` with the given raw params
    /// array and steps array. Both arguments are JSON text.
    fn make_task(tasks: &Path, id: &str, version: u32, params: &str, steps: &str) -> PathBuf {
        let dir = tasks.join(id);
        std::fs::create_dir_all(&dir).expect("create task dir");
        let meta = format!(
            r#"{{"schema_version":1,"template_version":1,"task_id":"{id}","name":"{id}","version":{version},"params":{params},"exits":{{"done":{{"result":"success"}},"failed":{{"result":"retryable"}}}}}}"#
        );
        std::fs::write(dir.join("meta.json"), meta).expect("write meta");
        let flow = format!(r#"{{"schema_version":1,"name":"{id}","steps":{steps}}}"#);
        std::fs::write(dir.join("flow.json"), flow).expect("write flow");
        dir
    }

    const FINISH_DONE: &str = r#"[{"op":"finish","args":{"exit":"done"}}]"#;
    const CALL_B_WITH_PARAMS: &str = r#"[
        {"op":"call_task","args":{"task":"B","params":{"x":"param('y')"},"inherit":"none"}},
        {"op":"finish","args":{"exit":"done"}}
    ]"#;

    fn opts(tasks: &Path, include_deps: bool) -> BuildOptions {
        let mut o = BuildOptions::new(&tasks.join("A"));
        o.tasks_dir = tasks.to_path_buf();
        o.assets_dir = tasks
            .parent()
            .map(|p| p.join("assets"))
            .unwrap_or_else(|| tasks.join("assets"));
        o.include_deps = include_deps;
        o.reproducible = true;
        o.tool_version = "0.1.0".to_string();
        o.git = Some("abc1234".to_string());
        o.dirty = false;
        o.repo = "D:/repo".to_string();
        o.commit = "abc1234".to_string();
        o
    }

    fn build(tasks: &Path, id: &str, out: &Path, include_deps: bool) -> BuildOutcome {
        build_package(&tasks.join(id), out, &opts(tasks, include_deps)).expect("build package")
    }

    fn write_raw_zip(path: &Path, entries: &[(&str, &[u8])]) {
        let file = std::fs::File::create(path).expect("create zip");
        let mut zip = zip::ZipWriter::new(file);
        for (name, bytes) in entries {
            zip.start_file(*name, zip::write::FileOptions::default())
                .expect("start zip entry");
            zip.write_all(bytes).expect("write zip entry");
        }
        zip.finish().expect("finish zip");
    }

    fn rewrite_archive(
        archive: &PackageArchive,
        path: &Path,
        manifest: Option<&Manifest>,
        checksums: Option<&BTreeMap<String, String>>,
        extra: &[(&str, &[u8])],
    ) {
        let mut entries: Vec<(String, Vec<u8>)> = archive
            .files
            .iter()
            .map(|(p, b)| (p.clone(), b.clone()))
            .collect();
        for (name, bytes) in extra {
            entries.push(((*name).to_string(), bytes.to_vec()));
        }
        entries.push((
            MANIFEST_FILE.to_string(),
            serde_json::to_vec_pretty(manifest.unwrap_or(&archive.manifest)).unwrap(),
        ));
        entries.push((
            CHECKSUMS_FILE.to_string(),
            checksums_text(checksums.unwrap_or(&archive.checksums)).into_bytes(),
        ));
        let file = std::fs::File::create(path).expect("create rewritten zip");
        let mut zip = zip::ZipWriter::new(file);
        for (name, bytes) in entries {
            zip.start_file(name.as_str(), zip::write::FileOptions::default())
                .expect("start rewritten entry");
            zip.write_all(&bytes).expect("write rewritten entry");
        }
        zip.finish().expect("finish rewritten zip");
    }

    #[test]
    fn navigate_flow_op_and_map_requirements_are_additive() {
        assert!(SUPPORTED_FLOW_OPS.contains(&"navigate"));
        let ops = derive_flow_ops(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"navigate","args":{"target":"御魂十层"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        )
        .expect("derive flow ops");
        assert!(ops.contains(&"navigate".to_string()), "{ops:?}");
        assert!(unsupported_flow_ops(&ops).is_empty());
        // Old Runner gate: the op is recognized by this build (the *old* Runner
        // rejects it via its own copy of SUPPORTED_FLOW_OPS).
        assert!(unsupported_flow_ops(&["navigate".to_string()]).is_empty());
        // Manifest field is additive + default-empty.
        let req = MapRequirements::default();
        assert!(req.is_empty());
        assert_eq!(
            serde_json::to_value(&req).expect("json"),
            serde_json::json!({"nodes": []})
        );
        let parsed: MapRequirements =
            serde_json::from_value(serde_json::json!({"nodes":["庭院","御魂十层"]}))
                .expect("parse");
        assert_eq!(parsed.nodes, vec!["庭院", "御魂十层"]);
        // Old manifest without the field still deserializes (default empty).
        let manifest = serde_json::json!({
            "package_schema": 2, "package_id":"p","task_id":"t","task_version":1,
            "name":"n","created_at_ms":0,
            "tool":{"version":"1","dirty":false,"surface":"studio"},
            "source":{"repo":"r","commit":"c"},
            "engine":{"template_version":1,"min_engine":"0.1.0","flow_ops":[]},
            "files":[],
            "variables_schema":{"own":[],"call_deps":[]},
            "dependencies":[], "asset_refs":[],
            "platform":{"os":"windows","needs_adb":true,"design_resolution":null},
            "signature":null
        });
        let m: Manifest = serde_json::from_value(manifest).expect("old manifest");
        assert!(m.map_requirements.is_empty());
        let value = serde_json::to_value(&m).expect("json");
        assert!(
            value.get("map_requirements").is_none()
                || value["map_requirements"].is_null(),
            "empty map_requirements is skipped in serialization: {value}"
        );
    }

    #[test]
    fn manifest_fields_and_flow_ops_cover_call_semantics() {
        let td = TempDir::new("manifest");
        let tasks = td.path().join("tasks");
        std::fs::create_dir_all(&tasks).unwrap();
        make_task(
            &tasks,
            "A",
            3,
            r#"[{"key":"y","type":"string","default":"突破"}]"#,
            CALL_B_WITH_PARAMS,
        );
        make_task(
            &tasks,
            "B",
            2,
            r#"[{"key":"x","type":"int","default":1}]"#,
            FINISH_DONE,
        );
        let out = td.path().join("out");
        let outcome = build(&tasks, "A", &out, true);

        let m = &outcome.manifest;
        assert_eq!(m.package_schema, PACKAGE_SCHEMA);
        assert_eq!(m.task_id, "A");
        assert_eq!(m.task_version, 3);
        assert_eq!(m.name, "A");
        assert!(m.package_id.starts_with("pkg-A-3-"), "{}", m.package_id);
        assert_eq!(m.created_at_ms, 0, "reproducible build has no wall clock");
        assert_eq!(m.tool.surface, "studio");
        assert_eq!(m.engine.template_version, 1);
        assert_eq!(m.engine.min_engine, env!("CARGO_PKG_VERSION"));
        assert_eq!(
            m.engine.flow_ops,
            vec![
                "call_task".to_string(),
                "call_task.inherit.none".to_string(),
                "call_task.params".to_string()
            ],
            "flow_ops must come from raw call_task args"
        );
        assert_eq!(m.dependencies.len(), 1);
        assert_eq!(m.dependencies[0].task_id, "B");
        assert_eq!(m.dependencies[0].version, 2);
        assert!(m.dependencies[0].included);
        assert_eq!(m.variables_schema.own[0].key, "y");
        let dep = &m.variables_schema.call_deps[0];
        assert_eq!(dep.child, "B");
        assert_eq!(dep.mapping["x"], "param('y')");
        assert_eq!(dep.inherit, "none");
        assert_eq!(dep.child_variables, vec!["x".to_string()]);
        assert!(m.signature.is_none());
        assert_eq!(m.platform.os, platform_os());
        assert!(m.platform.needs_adb);
        let paths: Vec<&str> = m.files.iter().map(|f| f.path.as_str()).collect();
        assert!(paths.contains(&"task/meta.json"), "{paths:?}");
        assert!(paths.contains(&"task/flow.json"), "{paths:?}");
        assert!(paths.contains(&"deps/B/flow.json"), "{paths:?}");
        assert!(!paths.iter().any(|p| p.starts_with("manifest")), "{paths:?}");
    }

    #[test]
    fn manifest_uses_v2_variables_schema_and_package_schema_2() {
        let td = TempDir::new("variables-schema");
        let tasks = td.path().join("tasks");
        std::fs::create_dir_all(&tasks).unwrap();
        make_task(&tasks, "A", 1, "[]", CALL_B_WITH_PARAMS);
        // Overwrite A with a V2 variables meta (no legacy params).
        std::fs::write(
            tasks.join("A").join("meta.json"),
            r#"{"schema_version":1,"name":"A","version":1,
                "variables":[
                  {"key":"y","type":"string","default":"突破","runner_editable":true},
                  {"key":"max_rounds","type":"int","default":3,"min":1,"max":99}
                ],
                "exits":{"done":{"result":"success"},"failed":{"result":"retryable"}}}"#,
        )
        .unwrap();
        make_task(&tasks, "B", 1, "[]", FINISH_DONE);
        std::fs::write(
            tasks.join("B").join("meta.json"),
            r#"{"schema_version":1,"name":"B","version":1,
                "variables":[{"key":"x","type":"string","default":"甲"}],
                "exits":{"done":{"result":"success"},"failed":{"result":"retryable"}}}"#,
        )
        .unwrap();
        let outcome = build(&tasks, "A", &td.path().join("out"), false);
        let m = &outcome.manifest;
        assert_eq!(m.package_schema, 2, "V2 package schema");
        let own: Vec<&str> = m
            .variables_schema
            .own
            .iter()
            .map(|v| v.key.as_str())
            .collect();
        assert_eq!(own, vec!["y", "max_rounds"]);
        assert_eq!(m.variables_schema.own[1].kind, "int");
        assert_eq!(m.variables_schema.own[1].min, Some(1.0));
        let dep = &m.variables_schema.call_deps[0];
        assert_eq!(dep.child, "B");
        assert_eq!(dep.child_variables, vec!["x".to_string()]);
        // Legacy params-only tasks are imported into the snapshot (bridge).
        let td2 = TempDir::new("variables-schema-legacy");
        let tasks2 = td2.path().join("tasks");
        std::fs::create_dir_all(&tasks2).unwrap();
        make_task(
            &tasks2,
            "L",
            1,
            r#"[{"key":"max_rounds","type":"int","default":3}]"#,
            FINISH_DONE,
        );
        let outcome2 = build(&tasks2, "L", &td2.path().join("out"), false);
        assert_eq!(outcome2.manifest.variables_schema.own[0].key, "max_rounds");
        assert_eq!(
            outcome2.manifest.variables_schema.own[0].value_source(),
            "literal"
        );
    }

    #[test]
    fn package_schema_1_is_rejected_with_a_republish_message() {
        let td = TempDir::new("old-schema");
        let tasks = td.path().join("tasks");
        std::fs::create_dir_all(&tasks).unwrap();
        make_task(&tasks, "A", 1, "[]", FINISH_DONE);
        let outcome = build(&tasks, "A", &td.path().join("out"), false);
        let archive = read_package(&outcome.path).expect("read");
        let mut old = archive.manifest.clone();
        old.package_schema = 1;
        let path = td.path().join("old.oaspkg");
        rewrite_archive(&archive, &path, Some(&old), None, &[]);
        let err = read_package(&path).expect_err("schema=1 must be refused");
        assert!(err.contains("unsupported package_schema 1"), "{err}");
        assert!(err.contains("re-publish"), "{err}");
    }

    #[test]
    fn build_read_extract_verify_roundtrip() {
        let td = TempDir::new("roundtrip");
        let tasks = td.path().join("tasks");
        std::fs::create_dir_all(&tasks).unwrap();
        make_task(&tasks, "A", 1, "[]", FINISH_DONE);
        let out = td.path().join("out");
        let outcome = build(&tasks, "A", &out, false);

        let archive = read_package(&outcome.path).expect("read package");
        assert_eq!(archive.manifest.package_id, outcome.manifest.package_id);
        assert_eq!(archive.checksums.len(), archive.files.len());
        assert!(archive.files.contains_key("task/flow.json"));
        assert!(!archive.files.contains_key(MANIFEST_FILE));
        assert!(!archive.files.contains_key(CHECKSUMS_FILE));

        let dest = td.path().join("extract");
        extract_to(&outcome.path, &dest).expect("extract");
        verify_checksums(&dest).expect("checksums verify");
        assert!(dest.join("task").join("meta.json").is_file());
        assert_eq!(sha256_file(&outcome.path).unwrap(), outcome.sha256);
        let sidecar = outcome.path.with_extension("oaspkg.sha256");
        let sidecar_text = std::fs::read_to_string(&sidecar).expect("sidecar");
        assert!(sidecar_text.contains(&outcome.sha256), "{sidecar_text}");
    }

    #[test]
    fn reproducible_builds_have_identical_archive_hash() {
        let td = TempDir::new("reproducible");
        let tasks = td.path().join("tasks");
        std::fs::create_dir_all(&tasks).unwrap();
        make_task(&tasks, "A", 2, "[]", FINISH_DONE);
        let first = td.path().join("first.oaspkg");
        let second = td.path().join("second.oaspkg");
        let a = build_package(&tasks.join("A"), &first, &opts(&tasks, false)).unwrap();
        let b = build_package(&tasks.join("A"), &second, &opts(&tasks, false)).unwrap();
        assert_eq!(a.sha256, b.sha256);
        assert_eq!(a.manifest.package_id, b.manifest.package_id);
        assert_eq!(a.manifest.created_at_ms, 0);
    }

    #[test]
    fn zip_slip_paths_are_rejected() {
        let td = TempDir::new("zip-slip");
        for (i, name) in ["../evil.txt", "/absolute.txt", "C:/drive.txt", "a\\b.txt"]
            .iter()
            .enumerate()
        {
            let path = td.path().join(format!("bad-{i}.oaspkg"));
            write_raw_zip(&path, &[(name, b"x")]);
            let err = read_package(&path).expect_err("zip-slip must be rejected");
            assert!(
                err.contains("traversal")
                    || err.contains("absolute")
                    || err.contains("drive")
                    || err.contains("backslash"),
                "unexpected error for `{name}`: {err}"
            );
        }
    }

    #[test]
    fn checksum_missing_and_extra_files_are_rejected() {
        let td = TempDir::new("checksums");
        let tasks = td.path().join("tasks");
        std::fs::create_dir_all(&tasks).unwrap();
        make_task(&tasks, "A", 1, "[]", FINISH_DONE);
        let outcome = build(&tasks, "A", &td.path().join("out"), false);
        let archive = read_package(&outcome.path).unwrap();

        // Missing: remove one checksum line but leave the file in the zip.
        let mut missing = archive.checksums.clone();
        missing.remove("task/meta.json");
        let missing_path = td.path().join("missing.oaspkg");
        rewrite_archive(&archive, &missing_path, None, Some(&missing), &[]);
        let err = read_package(&missing_path).expect_err("missing checksum row");
        assert!(err.contains("not listed in checksums"), "{err}");

        // Extra: add a zip entry that no checksum lists.
        let extra_path = td.path().join("extra.oaspkg");
        rewrite_archive(
            &archive,
            &extra_path,
            None,
            None,
            &[("task/extra.txt", b"surprise")],
        );
        let err = read_package(&extra_path).expect_err("extra file");
        assert!(err.contains("not listed in checksums"), "{err}");
    }

    #[test]
    fn manifest_files_must_match_checksums_and_bytes() {
        let td = TempDir::new("manifest-files");
        let tasks = td.path().join("tasks");
        std::fs::create_dir_all(&tasks).unwrap();
        make_task(&tasks, "A", 1, "[]", FINISH_DONE);
        let outcome = build(&tasks, "A", &td.path().join("out"), false);
        let archive = read_package(&outcome.path).unwrap();

        let mut broken = archive.manifest.clone();
        broken.files.retain(|f| f.path != "task/flow.json");
        let path = td.path().join("broken-manifest.oaspkg");
        rewrite_archive(&archive, &path, Some(&broken), None, &[]);
        let err = read_package(&path).expect_err("manifest/checksums mismatch");
        assert!(err.contains("manifest.files does not match"), "{err}");
    }

    #[test]
    fn runtime_artifacts_are_excluded_and_state_blocks_publish() {
        let td = TempDir::new("runtime");
        let tasks = td.path().join("tasks");
        std::fs::create_dir_all(&tasks).unwrap();
        let dir = make_task(&tasks, "A", 1, "[]", FINISH_DONE);
        std::fs::write(dir.join("run.tmp"), b"tmp").unwrap();
        std::fs::write(dir.join("meta.json.bak"), b"backup").unwrap();
        std::fs::write(dir.join("x.clicks.jsonl"), b"{}").unwrap();
        // `x.clicks.jsonl` makes publish invalid by design, so validate first
        // and then remove it to check that tmp/bak are merely excluded.
        let issues = validate_for_publish(&dir, &tasks, None).expect_err("clicks must block");
        assert!(
            issues.iter().any(|i| i.msg.contains("click log")),
            "{issues:?}"
        );
        std::fs::remove_file(dir.join("x.clicks.jsonl")).unwrap();
        std::fs::create_dir_all(dir.join("state")).unwrap();
        std::fs::write(dir.join("state").join("checkpoint.json"), b"{}").unwrap();
        let issues = validate_for_publish(&dir, &tasks, None).expect_err("state must block");
        assert!(
            issues.iter().any(|i| i.msg.contains("runtime state")),
            "{issues:?}"
        );
        std::fs::remove_dir_all(dir.join("state")).unwrap();

        let outcome = build(&tasks, "A", &td.path().join("out"), false);
        let archive = read_package(&outcome.path).unwrap();
        assert!(
            !archive.files.keys().any(|p| p.contains(".tmp") || p.contains(".bak")),
            "{:?}",
            archive.files.keys()
        );
        assert!(outcome.warnings.iter().any(|w| w.contains("excluded")), "{:?}", outcome.warnings);
    }

    #[test]
    fn include_deps_embeds_the_transitive_call_closure() {
        let td = TempDir::new("deps");
        let tasks = td.path().join("tasks");
        std::fs::create_dir_all(&tasks).unwrap();
        make_task(
            &tasks,
            "A",
            1,
            "[]",
            r#"[{"op":"call_task","args":{"task":"B"}},{"op":"finish","args":{"exit":"done"}}]"#,
        );
        make_task(
            &tasks,
            "B",
            2,
            "[]",
            r#"[{"op":"call_task","args":{"task":"C"}},{"op":"finish","args":{"exit":"done"}}]"#,
        );
        make_task(&tasks, "C", 4, "[]", FINISH_DONE);

        let with = build(&tasks, "A", &td.path().join("with"), true);
        let archive = read_package(&with.path).unwrap();
        assert!(archive.files.contains_key("deps/B/flow.json"));
        assert!(archive.files.contains_key("deps/C/flow.json"));
        let deps: Vec<(String, bool)> = with
            .manifest
            .dependencies
            .iter()
            .map(|d| (d.task_id.clone(), d.included))
            .collect();
        assert_eq!(
            deps,
            vec![("B".to_string(), true), ("C".to_string(), true)]
        );
        assert_eq!(
            with.manifest.engine.flow_ops,
            vec!["call_task".to_string()],
            "B/C add no new ops"
        );

        let without = build(&tasks, "A", &td.path().join("without"), false);
        let archive = read_package(&without.path).unwrap();
        assert!(!archive.files.keys().any(|p| p.starts_with("deps/")));
        assert!(without.manifest.dependencies.iter().all(|d| !d.included));
    }

    #[test]
    fn existing_package_is_refused_without_force() {
        let td = TempDir::new("force");
        let tasks = td.path().join("tasks");
        std::fs::create_dir_all(&tasks).unwrap();
        make_task(&tasks, "A", 1, "[]", FINISH_DONE);
        let target = td.path().join("A-v1.oaspkg");
        let first = build_package(&tasks.join("A"), &target, &opts(&tasks, false)).unwrap();
        let err = build_package(&tasks.join("A"), &target, &opts(&tasks, false))
            .expect_err("must refuse existing package");
        match err {
            PackageError::Exists(path) => assert_eq!(path, target),
            other => panic!("expected Exists, got {other:?}"),
        }
        let mut forced = opts(&tasks, false);
        forced.force = true;
        let second = build_package(&tasks.join("A"), &target, &forced).unwrap();
        assert_eq!(first.manifest.package_id, second.manifest.package_id);
        assert_eq!(first.sha256, second.sha256);
    }

    #[test]
    fn call_cycles_and_depth_are_rejected() {
        let td = TempDir::new("cycle");
        let tasks = td.path().join("tasks");
        std::fs::create_dir_all(&tasks).unwrap();
        make_task(
            &tasks,
            "A",
            1,
            "[]",
            r#"[{"op":"call_task","args":{"task":"B"}},{"op":"finish","args":{"exit":"done"}}]"#,
        );
        make_task(
            &tasks,
            "B",
            1,
            "[]",
            r#"[{"op":"call_task","args":{"task":"A"}},{"op":"finish","args":{"exit":"done"}}]"#,
        );
        let err = build_package(
            &tasks.join("A"),
            &td.path().join("cycle"),
            &opts(&tasks, true),
        )
        .expect_err("cycle must be rejected");
        assert!(err.to_string().contains("cycle"), "{err}");

        let td2 = TempDir::new("depth");
        let tasks2 = td2.path().join("tasks");
        std::fs::create_dir_all(&tasks2).unwrap();
        for i in 0..10 {
            let id = format!("T{i}");
            let steps = if i == 9 {
                FINISH_DONE.to_string()
            } else {
                format!(
                    r#"[{{"op":"call_task","args":{{"task":"T{}"}}}},{{"op":"finish","args":{{"exit":"done"}}}}]"#,
                    i + 1
                )
            };
            make_task(&tasks2, &id, 1, "[]", &steps);
        }
        let err = build_package(
            &tasks2.join("T0"),
            &td2.path().join("depth"),
            &opts(&tasks2, true),
        )
        .expect_err("depth must be rejected");
        assert!(err.to_string().contains("exceeds 8"), "{err}");
    }

    #[test]
    fn flow_ops_are_derived_from_raw_args_not_generated_scopes() {
        let legacy = r#"{"schema_version":1,"name":"A","steps":[
            {"op":"call_task","args":{"task":"B"}},
            {"op":"finish","args":{"exit":"done"}}
        ]}"#;
        assert_eq!(derive_flow_ops(legacy).unwrap(), vec!["call_task".to_string()]);

        let hand_written = r#"{"schema_version":1,"name":"A","steps":[
            {"op":"with_params","args":{"body":[
                {"op":"finish","args":{"exit":"done"}}
            ]}}
        ]}"#;
        assert_eq!(
            derive_flow_ops(hand_written).unwrap(),
            vec!["with_params".to_string()]
        );
        assert!(unsupported_flow_ops(&["call_task".into()]).is_empty());
        assert_eq!(
            unsupported_flow_ops(&["call_task".into(), "future_op".into()]),
            vec!["future_op".to_string()]
        );
    }

    #[test]
    fn drag_asset_endpoint_flow_op_is_derived_and_supported() {
        let flow = r#"{"schema_version":1,"name":"A","steps":[
            {"op":"drag","args":{
              "from":{"kind":"asset","sel":"I_START"},
              "to":{"kind":"asset","sel":"I_END"},
              "direction":"right","distance":[10,20]}},
            {"op":"finish","args":{"exit":"done"}}]}"#;
        assert_eq!(derive_flow_ops(flow).unwrap(), vec!["drag.asset".to_string()]);
        // The current engine supports the marker (install gate passes).
        let derived = derive_flow_ops(flow).unwrap();
        assert!(unsupported_flow_ops(&derived).is_empty());
        assert!(SUPPORTED_FLOW_OPS.contains(&"drag.asset"));
        // A Runner that predates seq129 does not know `drag.asset`; the install
        // gate rejects it instead of failing mid-task.
        let pre_seq129_known = [
            "call_task",
            "with_params",
            "drag.text_expr",
            "drag.region",
            "drag.region_asset",
        ];
        let rejected: Vec<&str> = derived
            .iter()
            .map(String::as_str)
            .filter(|op| !pre_seq129_known.contains(op))
            .collect();
        assert_eq!(rejected, vec!["drag.asset"]);
        // Old region/point endpoints do not trigger the new marker.
        let legacy = r#"{"schema_version":1,"name":"A","steps":[
            {"op":"drag","args":{
              "from":{"kind":"region","roi":[0,0,10,10]},
              "direction":"up","distance":[1,2]}},
            {"op":"finish","args":{"exit":"done"}}]}"#;
        assert!(derive_flow_ops(legacy).unwrap().is_empty());
    }

    #[test]
    fn text_expr_flow_ops_are_derived_from_the_raw_flow() {
        // ---- raw-flow derivation (unit) ----
        let text_expr_flow = r#"{"schema_version":1,"name":"A","steps":[
            {"op":"click_target","args":{
              "find":[{"kind":"ocr_text","sel":"O_BTN","text_expr":"param('k')"}],
              "requires":{"kind":"ocr_text","sel":"O_BTN","text_expr":"param('g')"},
              "expect":{"kind":"ocr_text","sel":"O_BTN","text":"完成"}}},
            {"op":"drag","args":{
              "from":{"kind":"ocr_text","sel":"O_ROW","text_expr":"param('row')"},
              "direction":"up","distance":[10,20]}},
            {"op":"finish","args":{"exit":"done"}}]}"#;
        assert_eq!(
            derive_flow_ops(text_expr_flow).unwrap(),
            vec![
                "click_target.text_expr".to_string(),
                "drag.text_expr".to_string(),
                "expect.text_expr".to_string(),
            ]
        );

        // A literal-only flow keeps the historical (empty) op set: publishing an
        // old task must not start gating on the new capability.
        let literal_flow = r#"{"schema_version":1,"name":"A","steps":[
            {"op":"click_target","args":{"find":[{"kind":"ocr_text","sel":"O","text":"完成"}]}},
            {"op":"click","args":{"sel":"S","expect":{"kind":"ocr_text","sel":"O","text":"完成"}}},
            {"op":"drag","args":{"from":{"kind":"region","roi":[0,0,10,10]},
              "direction":"up","distance":[1,2]}},
            {"op":"finish","args":{"exit":"done"}}]}"#;
        assert!(derive_flow_ops(literal_flow).unwrap().is_empty());

        // Nested blocks are walked too (and only the real op marker is added).
        let nested = r#"{"schema_version":1,"name":"A","steps":[
            {"op":"if","args":{"cond":"true"},
             "then":[{"op":"click_target","args":{"find":[
                 {"kind":"ocr_text","sel":"O","text_expr":"param('k')"}]}}],
             "else":[]},
            {"op":"finish","args":{"exit":"done"}}]}"#;
        assert_eq!(
            derive_flow_ops(nested).unwrap(),
            vec!["click_target.text_expr".to_string()]
        );

        // This engine understands the new markers (install gate: empty = no
        // offending ops) ...
        let derived = derive_flow_ops(text_expr_flow).unwrap();
        assert!(unsupported_flow_ops(&derived).is_empty());
        // ... while a pre-v2 Runner (the old 4-op set) must refuse the package
        // instead of failing mid-task.
        let pre_v2 = [
            "call_task",
            "call_task.params",
            "call_task.inherit.none",
            "with_params",
        ];
        let rejected: Vec<&str> = derived
            .iter()
            .map(String::as_str)
            .filter(|op| !pre_v2.iter().any(|known| known == op))
            .collect();
        assert_eq!(
            rejected,
            vec![
                "click_target.text_expr",
                "drag.text_expr",
                "expect.text_expr"
            ],
            "a pre-v2 Runner must reject every text_expr marker"
        );
    }

    #[test]
    fn text_expr_authentic_package_manifest_advertises_the_new_ops() {
        let td = TempDir::new("textexpr");
        let tasks = td.path().join("tasks");
        let assets = td.path().join("assets");
        std::fs::create_dir_all(&tasks).unwrap();
        std::fs::create_dir_all(assets.join("images")).unwrap();
        std::fs::write(
            assets.join("assets.json"),
            r#"{"schema_version":1,"design_resolution":[1280,720],"items":[
                {"id":"O_BTN","kind":"ocr_lines","roi":[0,0,10,10]},
                {"id":"O_ROW","kind":"ocr_lines","roi":[0,0,10,10]}]}"#,
        )
        .unwrap();
        make_task(
            &tasks,
            "T",
            1,
            r#"[{"key":"k","type":"string","default":"完成"},
                 {"key":"g","type":"string","default":"准备"},
                 {"key":"row","type":"string","default":"返回"}]"#,
            r#"[{"op":"click_target","args":{
                  "find":[{"kind":"ocr_text","sel":"O_BTN","text_expr":"param('k')"}],
                  "requires":{"kind":"ocr_text","sel":"O_BTN","text_expr":"param('g')"}}},
                {"op":"drag","args":{
                  "from":{"kind":"ocr_text","sel":"O_ROW","text_expr":"param('row')"},
                  "direction":"up","distance":[10,20]}},
                {"op":"finish","args":{"exit":"done"}}]"#,
        );
        let outcome = build(&tasks, "T", &td.path().join("out"), false);
        assert_eq!(
            outcome.manifest.engine.flow_ops,
            vec![
                "click_target.text_expr".to_string(),
                "drag.text_expr".to_string(),
                "expect.text_expr".to_string(),
            ],
            "the published manifest must advertise text_expr"
        );
    }

    #[test]
    fn operation_region_and_action_ops_are_derived_from_the_raw_flow() {
        // v1.3 (OPERATION-REGION-DESIGN §6.1): the new instructions and the
        // authored `region` overrides get their own install-gate markers.
        let flow_json = r#"{"schema_version":1,"name":"A","steps":[
            {"op":"click_target","args":{"find":[
                {"kind":"ocr_text","sel":"O_BTN","text":"完成",
                 "region":{"mode":"nearest"}}]}},
            {"op":"drag","args":{
              "from":{"kind":"ocr_text","sel":"O_ROW","text":"返回",
                      "region":{"mode":"offset","offset":[1,2]}},
              "direction":"up","distance":[10,20]}},
            {"op":"long_press","args":{"find":[{"kind":"ocr_text","sel":"O_BTN","text":"完成"}]}},
            {"op":"curve_swipe","args":{"points":[[1,1],[10,10]]}},
            {"op":"finish","args":{"exit":"done"}}]}"#;
        assert_eq!(
            derive_flow_ops(flow_json).unwrap(),
            vec![
                "click_target.region".to_string(),
                "curve_swipe".to_string(),
                "drag.region".to_string(),
                "long_press".to_string(),
            ]
        );
        // This engine understands them: the install gate reports no offender.
        assert!(unsupported_flow_ops(&derive_flow_ops(flow_json).unwrap()).is_empty());

        // A flow without region/long_press/curve_swipe keeps the old op set —
        // old packages must not start gating on the new markers.
        let plain = r#"{"schema_version":1,"name":"A","steps":[
            {"op":"click_target","args":{"find":[
                {"kind":"ocr_text","sel":"O","text":"完成"}]}},
            {"op":"drag","args":{"from":{"kind":"region","roi":[0,0,10,10]},
              "direction":"up","distance":[1,2]}},
            {"op":"finish","args":{"exit":"done"}}]}"#;
        assert!(derive_flow_ops(plain).unwrap().is_empty());

        // A pre-v1.3 Runner (the 7-op v2 set) must refuse every new marker.
        let pre_v13 = [
            "call_task",
            "call_task.params",
            "call_task.inherit.none",
            "with_params",
            "click_target.text_expr",
            "expect.text_expr",
            "drag.text_expr",
        ];
        let derived = derive_flow_ops(flow_json).unwrap();
        let rejected: Vec<&str> = derived
            .iter()
            .map(String::as_str)
            .filter(|op| !pre_v13.iter().any(|known| known == op))
            .collect();
        assert_eq!(
            rejected,
            vec![
                "click_target.region",
                "curve_swipe",
                "drag.region",
                "long_press"
            ],
            "a pre-v1.3 Runner must reject the new markers"
        );

        // `long_press` with text_expr/region in its find[] needs only the
        // `long_press` marker (an old Runner cannot run the op at all).
        let press_only = r#"{"schema_version":1,"name":"A","steps":[
            {"op":"long_press","args":{"find":[
                {"kind":"ocr_text","sel":"O","text_expr":"param('k')",
                 "region":{"mode":"ocr"}}]}},
            {"op":"finish","args":{"exit":"done"}}]}"#;
        assert_eq!(
            derive_flow_ops(press_only).unwrap(),
            vec!["long_press".to_string()]
        );
    }

    #[test]
    fn region_asset_flow_ops_are_derived_and_gated() {
        // v1.4 A-plan (OPERATION-REGION-DESIGN §7): `region.asset` is a new
        // install-gate capability on click_target / drag OCR locators. Both the
        // v1.3 `region` marker and the new `region_asset` marker are emitted.
        let flow_json = r#"{"schema_version":1,"name":"A","steps":[
            {"op":"click_target","args":{"find":[
                {"kind":"ocr_text","sel":"O_BTN","text":"完成",
                 "region":{"mode":"nearest","asset":"I_TILE"}}]}},
            {"op":"drag","args":{
              "from":{"kind":"ocr_text","sel":"O_ROW","text":"返回",
                      "region":{"mode":"asset","asset":"I_TILE"}},
              "direction":"up","distance":[10,20]}},
            {"op":"long_press","args":{"find":[
                {"kind":"ocr_text","sel":"O_BTN","text":"完成",
                 "region":{"mode":"asset","asset":"I_TILE"}}]}},
            {"op":"finish","args":{"exit":"done"}}]}"#;
        assert_eq!(
            derive_flow_ops(flow_json).unwrap(),
            vec![
                "click_target.region".to_string(),
                "click_target.region_asset".to_string(),
                "drag.region".to_string(),
                "drag.region_asset".to_string(),
                "long_press".to_string(),
            ]
        );
        // This engine understands them (install gate reports no offender).
        assert!(unsupported_flow_ops(&derive_flow_ops(flow_json).unwrap()).is_empty());

        // A v1.3 Runner (the 11-op set without the asset markers) must refuse a
        // package that uses `region.asset`: it would silently ignore the key.
        let pre_v14 = [
            "call_task",
            "call_task.params",
            "call_task.inherit.none",
            "with_params",
            "click_target.text_expr",
            "expect.text_expr",
            "drag.text_expr",
            "long_press",
            "curve_swipe",
            "click_target.region",
            "drag.region",
        ];
        let derived = derive_flow_ops(flow_json).unwrap();
        let rejected: Vec<&str> = derived
            .iter()
            .map(String::as_str)
            .filter(|op| !pre_v14.iter().any(|known| known == op))
            .collect();
        assert_eq!(
            rejected,
            vec!["click_target.region_asset", "drag.region_asset"],
            "a pre-v1.4 Runner must reject the region_asset markers"
        );

        // A region without `asset` keeps the v1.3 marker only (no new gate).
        let no_asset = r#"{"schema_version":1,"name":"A","steps":[
            {"op":"click_target","args":{"find":[
                {"kind":"ocr_text","sel":"O","text":"完成",
                 "region":{"mode":"nearest"}}]}},
            {"op":"finish","args":{"exit":"done"}}]}"#;
        assert_eq!(
            derive_flow_ops(no_asset).unwrap(),
            vec!["click_target.region".to_string()]
        );
    }

    #[test]
    fn region_asset_package_manifest_advertises_the_new_markers() {
        let td = TempDir::new("regionasset");
        let tasks = td.path().join("tasks");
        let assets = td.path().join("assets");
        std::fs::create_dir_all(&tasks).unwrap();
        std::fs::create_dir_all(assets.join("images")).unwrap();
        std::fs::write(assets.join("images").join("a001.png"), b"png-bytes").unwrap();
        std::fs::write(
            assets.join("assets.json"),
            r#"{"schema_version":1,"design_resolution":[1280,720],"items":[
                {"id":"O_BTN","kind":"ocr_lines","roi":[0,0,10,10]},
                {"id":"O_ROW","kind":"ocr_lines","roi":[0,0,10,10]},
                {"id":"I_TILE","kind":"template","file":"images/a001.png",
                 "roi":[0,0,10,10]}]}"#,
        )
        .unwrap();
        make_task(
            &tasks,
            "T",
            1,
            "[]",
            r#"[{"op":"click_target","args":{"find":[
                  {"kind":"ocr_text","sel":"O_BTN","text":"完成",
                   "region":{"mode":"nearest","asset":"I_TILE"}}]}},
                {"op":"drag","args":{
                  "from":{"kind":"ocr_text","sel":"O_ROW","text":"返回",
                          "region":{"mode":"asset","asset":"I_TILE"}},
                  "direction":"up","distance":[10,20]}},
                {"op":"finish","args":{"exit":"done"}}]"#,
        );
        let outcome = build(&tasks, "T", &td.path().join("out"), false);
        assert_eq!(
            outcome.manifest.engine.flow_ops,
            vec![
                "click_target.region".to_string(),
                "click_target.region_asset".to_string(),
                "drag.region".to_string(),
                "drag.region_asset".to_string(),
            ],
            "the published manifest must advertise region.asset"
        );
    }

    #[test]
    fn operation_region_package_manifest_advertises_the_new_ops() {
        let td = TempDir::new("opregion");
        let tasks = td.path().join("tasks");
        let assets = td.path().join("assets");
        std::fs::create_dir_all(&tasks).unwrap();
        std::fs::create_dir_all(assets.join("images")).unwrap();
        std::fs::write(
            assets.join("assets.json"),
            r#"{"schema_version":1,"design_resolution":[1280,720],"items":[
                {"id":"O_BTN","kind":"ocr_lines","roi":[0,0,10,10]},
                {"id":"O_ROW","kind":"ocr_lines","roi":[0,0,10,10]}]}"#,
        )
        .unwrap();
        make_task(
            &tasks,
            "T",
            1,
            "[]",
            r#"[{"op":"click_target","args":{"find":[
                  {"kind":"ocr_text","sel":"O_BTN","text":"完成",
                   "region":{"mode":"nearest"}}]}},
                {"op":"drag","args":{
                  "from":{"kind":"ocr_text","sel":"O_ROW","text":"返回",
                          "region":{"mode":"offset","offset":[1,2,5,5]}},
                  "direction":"up","distance":[10,20]}},
                {"op":"long_press","args":{"find":[
                  {"kind":"ocr_text","sel":"O_BTN","text":"完成"}]}},
                {"op":"curve_swipe","args":{"points":[[1,1],[10,10]]}},
                {"op":"finish","args":{"exit":"done"}}]"#,
        );
        let outcome = build(&tasks, "T", &td.path().join("out"), false);
        assert_eq!(
            outcome.manifest.engine.flow_ops,
            vec![
                "click_target.region".to_string(),
                "curve_swipe".to_string(),
                "drag.region".to_string(),
                "long_press".to_string(),
            ],
            "the published manifest must advertise region/long_press/curve_swipe"
        );
    }

    #[test]
    fn global_asset_snapshot_is_packaged() {
        let td = TempDir::new("assets");
        let tasks = td.path().join("tasks");
        let assets = td.path().join("assets");
        std::fs::create_dir_all(&tasks).unwrap();
        std::fs::create_dir_all(assets.join("images")).unwrap();
        std::fs::write(assets.join("images").join("a001.png"), b"png-bytes").unwrap();
        std::fs::write(
            assets.join("assets.json"),
            r#"{"schema_version":1,"design_resolution":[1280,720],
                "items":[{"id":"按钮","kind":"template","file":"images/a001.png","rev":1}],
                "collections":[]}"#,
        )
        .unwrap();
        make_task(
            &tasks,
            "A",
            1,
            "[]",
            r#"[{"op":"click","args":{"sel":"按钮"}},{"op":"finish","args":{"exit":"done"}}]"#,
        );
        let outcome = build(&tasks, "A", &td.path().join("out"), false);
        assert_eq!(outcome.manifest.asset_refs.len(), 1);
        assert_eq!(outcome.manifest.asset_refs[0].id, "按钮");
        assert_eq!(
            outcome.manifest.asset_refs[0].sha256,
            sha256_bytes(b"png-bytes")
        );
        assert_eq!(
            outcome.manifest.platform.design_resolution,
            Some([1280, 720])
        );

        let dest = td.path().join("extract");
        extract_to(&outcome.path, &dest).unwrap();
        assert_eq!(
            std::fs::read(dest.join("assets").join("images").join("a001.png")).unwrap(),
            b"png-bytes"
        );
        let snapshot: AssetsFile = serde_json::from_slice(
            &std::fs::read(dest.join("assets").join("assets.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(snapshot.items.len(), 1);
        assert_eq!(snapshot.items[0].id, "按钮");
    }

    #[test]
    fn zip_entry_names_are_relative_forward_slash_paths() {
        let td = TempDir::new("entry-names");
        let tasks = td.path().join("tasks");
        std::fs::create_dir_all(&tasks).unwrap();
        let dir = make_task(&tasks, "A", 1, "[]", FINISH_DONE);
        std::fs::create_dir_all(dir.join("images")).unwrap();
        std::fs::write(dir.join("images").join("a001.png"), b"x").unwrap();
        let outcome = build(&tasks, "A", &td.path().join("out"), false);
        let file = std::fs::File::open(&outcome.path).unwrap();
        let mut zip = zip::ZipArchive::new(file).unwrap();
        for i in 0..zip.len() {
            let entry = zip.by_index(i).unwrap();
            let name = entry.name();
            assert!(!name.contains('\\'), "{name}");
            assert!(!name.starts_with('/'), "{name}");
            validate_archive_path(name).expect("safe relative name");
        }
    }

    #[test]
    fn extract_refuses_a_non_empty_destination_and_tampering_fails_verify() {
        let td = TempDir::new("extract-guard");
        let tasks = td.path().join("tasks");
        std::fs::create_dir_all(&tasks).unwrap();
        make_task(&tasks, "A", 1, "[]", FINISH_DONE);
        let outcome = build(&tasks, "A", &td.path().join("out"), false);
        let dest = td.path().join("dest");
        std::fs::create_dir_all(&dest).unwrap();
        std::fs::write(dest.join("occupied.txt"), b"x").unwrap();
        let err = extract_to(&outcome.path, &dest).expect_err("non-empty dest");
        assert!(err.contains("not empty"), "{err}");

        let clean = td.path().join("clean");
        extract_to(&outcome.path, &clean).unwrap();
        std::fs::write(clean.join("task").join("flow.json"), b"tampered").unwrap();
        let errors = verify_checksums(&clean).expect_err("tamper must fail");
        assert!(
            errors.iter().any(|e| e.contains("checksum mismatch")),
            "{errors:?}"
        );
    }

    #[test]
    fn publish_validation_merges_the_global_store_for_t8() {
        // seq129-fix: a global-asset-mode task (empty local assets.json) whose
        // flow/variable bind a global ocr_var must pass publish validation.
        let td = TempDir::new("publish-global-ocr");
        let tasks = td.path().join("tasks");
        std::fs::create_dir_all(&tasks).unwrap();
        let dir = tasks.join("全局识别任务");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("meta.json"),
            r#"{"schema_version":1,"name":"全局识别任务","variables":[
                 {"key":"p","type":"string","value_source":"ocr_text",
                  "asset_id":"预设识别","default":"甲","runner_editable":true}],"exits":{"done":{"result":"success"},"failed":{"result":"retryable"}}}"#,
        )
        .unwrap();
        std::fs::write(
            dir.join("flow.json"),
            r#"{"schema_version":1,"name":"全局识别任务","steps":[
                 {"op":"drag","args":{
                   "from":{"kind":"asset","sel":"预设识别"},
                   "direction":"up","distance":[10,20]}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        )
        .unwrap();
        let assets = td.path().join("assets");
        std::fs::create_dir_all(&assets).unwrap();
        std::fs::write(
            assets.join("assets.json"),
            r#"{"schema_version":1,"items":[
                 {"id":"预设识别","kind":"ocr_var","runtime_text":"甲",
                  "param_key":"p"}]}"#,
        )
        .unwrap();
        // With the global store: publish validation is clean.
        validate_for_publish(&dir, &tasks, Some(&assets)).expect("global ocr_var must pass publish lint");
        // Without it: the historical T8 error still blocks (no silent relaxation).
        let issues = validate_for_publish(&dir, &tasks, None).expect_err("local-only mode");
        assert!(
            issues.iter().any(|i| i.code == "T8"),
            "without the global store T8 must still block: {issues:?}"
        );
    }

    #[test]
    fn publish_validation_requires_meta_and_flow() {
        let td = TempDir::new("validate");
        let tasks = td.path().join("tasks");
        let dir = tasks.join("A");
        std::fs::create_dir_all(&dir).unwrap();
        let issues = validate_for_publish(&dir, &tasks, None).expect_err("empty bundle");
        assert!(issues.iter().any(|i| i.path == "meta.json"), "{issues:?}");
        assert!(
            issues.iter().any(|i| i.path == "flow.json"),
            "{issues:?}"
        );
        std::fs::write(
            dir.join("meta.json"),
            r#"{"schema_version":1,"name":"A"}"#,
        )
        .unwrap();
        let issues = validate_for_publish(&dir, &tasks, None).expect_err("no flow");
        assert!(issues.iter().any(|i| i.path == "flow.json"), "{issues:?}");
    }

    #[test]
    fn task_assets_reference_missing_image_is_a_publish_error() {
        let td = TempDir::new("asset-missing");
        let tasks = td.path().join("tasks");
        std::fs::create_dir_all(&tasks).unwrap();
        let dir = make_task(
            &tasks,
            "A",
            1,
            "[]",
            r#"[{"op":"click","args":{"sel":"按钮"}},{"op":"finish","args":{"exit":"done"}}]"#,
        );
        std::fs::write(
            dir.join("assets.json"),
            r#"{"schema_version":1,"items":[{"id":"按钮","kind":"template","file":"images/a001.png"}]}"#,
        )
        .unwrap();
        let issues = validate_for_publish(&dir, &tasks, None).expect_err("missing image");
        assert!(
            issues
                .iter()
                .any(|i| i.code == "T5" && i.msg.contains("does not exist")),
            "{issues:?}"
        );
    }

    // ---- P1-2: zip bomb / lying header / encrypted entries ----------------

    /// Offset of the first central-directory file header (`PK\x01\x02`).
    fn first_central_header(bytes: &[u8]) -> usize {
        bytes
            .windows(4)
            .position(|window| window == b"PK\x01\x02")
            .expect("central directory header")
    }

    /// Patch the uncompressed-size field of the first central directory entry.
    /// `ZipArchive` reads sizes from the central directory, so this simulates a
    /// package whose header lies about how much it will decompress to.
    fn patch_declared_size(path: &Path, declared: u32) {
        let mut bytes = std::fs::read(path).expect("read zip");
        let pos = first_central_header(&bytes);
        bytes[pos + 24..pos + 28].copy_from_slice(&declared.to_le_bytes());
        std::fs::write(path, bytes).expect("write patched zip");
    }

    /// Set the "encrypted" general-purpose bit on the first entry.
    fn patch_encrypted_flag(path: &Path) {
        let mut bytes = std::fs::read(path).expect("read zip");
        let pos = first_central_header(&bytes);
        let flags = u16::from_le_bytes([bytes[pos + 8], bytes[pos + 9]]) | 1;
        bytes[pos + 8..pos + 10].copy_from_slice(&flags.to_le_bytes());
        std::fs::write(path, bytes).expect("write patched zip");
    }

    #[test]
    fn high_compression_ratio_is_refused_before_decompression() {
        let td = TempDir::new("zip-ratio");
        let path = td.path().join("bomb.oaspkg");
        // 2 MiB of zeros deflate to a few KiB: ratio far above 100:1.
        let zeros = vec![0u8; 2 * 1024 * 1024];
        write_raw_zip(&path, &[("task/bomb.bin", zeros.as_slice())]);
        let err = read_package(&path).expect_err("ratio must be refused");
        assert!(err.contains("compression ratio"), "{err}");
        let dest = td.path().join("extract");
        let err = extract_to(&path, &dest).expect_err("extract must refuse the ratio");
        assert!(err.contains("compression ratio"), "{err}");
        assert!(!dest.exists(), "nothing may be extracted");
    }

    #[test]
    fn a_lying_declared_size_is_refused_without_reading_the_payload() {
        let td = TempDir::new("zip-lie");
        let path = td.path().join("lie.oaspkg");
        let payload = vec![b'x'; 4096];
        write_raw_zip(&path, &[("task/flow.json", payload.as_slice())]);
        patch_declared_size(&path, 1);
        let err = read_package(&path).expect_err("declared size lie must be refused");
        assert!(err.contains("more than its declared"), "{err}");
        assert!(err.contains("task/flow.json"), "{err}");
    }

    #[test]
    fn package_entry_count_is_capped_at_2000() {
        let td = TempDir::new("zip-entries");
        let path = td.path().join("many.oaspkg");
        let names: Vec<String> = (0..2001).map(|i| format!("task/f{i}.bin")).collect();
        let payload: &[u8] = b"x";
        let entries: Vec<(&str, &[u8])> = names
            .iter()
            .map(|name| (name.as_str(), payload))
            .collect();
        write_raw_zip(&path, &entries);
        let err = read_package(&path).expect_err("2001 entries must be refused");
        assert!(err.contains("2001 entries"), "{err}");
        assert!(err.contains("2000"), "{err}");
    }

    #[test]
    fn encrypted_entries_are_refused() {
        let td = TempDir::new("zip-encrypted");
        let path = td.path().join("encrypted.oaspkg");
        write_raw_zip(&path, &[("task/flow.json", b"x")]);
        patch_encrypted_flag(&path);
        let err = read_package(&path).expect_err("encrypted entry must be refused");
        let lower = err.to_ascii_lowercase();
        assert!(
            lower.contains("encrypt") || lower.contains("password"),
            "unexpected error: {err}"
        );
    }

    // ---- design deviation: empty state/ must not block publishing ---------

    #[test]
    fn empty_state_dir_warns_but_non_empty_state_still_blocks_publish() {
        let td = TempDir::new("empty-state");
        let tasks = td.path().join("tasks");
        std::fs::create_dir_all(&tasks).unwrap();
        let dir = make_task(&tasks, "A", 1, "[]", FINISH_DONE);
        std::fs::create_dir_all(dir.join("state")).unwrap();

        let warnings = validate_for_publish(&dir, &tasks, None)
            .expect("an empty state/ directory must not block the publish");
        assert!(
            warnings.iter().any(|w| w.contains("state")),
            "{warnings:?}"
        );
        let outcome = build(&tasks, "A", &td.path().join("out"), false);
        assert!(
            outcome.warnings.iter().any(|w| w.contains("state")),
            "{:?}",
            outcome.warnings
        );
        let archive = read_package(&outcome.path).unwrap();
        assert!(
            !archive.files.keys().any(|p| p.starts_with("task/state/")),
            "runtime state is never packaged: {:?}",
            archive.files.keys()
        );

        std::fs::write(dir.join("state").join("checkpoint.json"), b"{}").unwrap();
        let issues = validate_for_publish(&dir, &tasks, None)
            .expect_err("a non-empty state/ must still block publishing");
        assert!(
            issues.iter().any(|i| i.msg.contains("runtime state")),
            "{issues:?}"
        );
    }

    #[test]
    fn engine_versions_compare_numerically() {
        assert!(version_at_least("0.1.0", "0.1.0"));
        assert!(version_at_least("0.10.0", "0.9.9"));
        assert!(!version_at_least("0.9.9", "0.10.0"));
        assert!(version_at_least("v1.2.3", "1.2.2"));
        assert!(version_at_least("1.2.3-beta", "1.2.3"));
        assert!(!version_at_least("1.2", "1.2.1"));
        assert!(version_at_least("2", "1.99.99"));
    }
}
