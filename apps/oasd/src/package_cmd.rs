//! Studio publish CLI + Runner package lifecycle (Runner Studio split §B, PR5).
//!
//! Two responsibilities:
//!
//! 1. **Studio side** — `oasd studio publish` and the shared publish helper used
//!    by `POST /task/publish`. It only builds an `.oaspkg`; it never installs.
//! 2. **Runner side** — install / uninstall / rollback / `installed.json`, used
//!    by the Runner-only `/runner/*` routes. All IO is plain `std::fs` and every
//!    path is a `Path`/`PathBuf` join, so the same layout works on Windows,
//!    Linux and a future Android client that drives the Runner HTTP API.
//!
//! Layout under the packages root (default `<tasks-dir>/../packages`):
//!
//! ```text
//! packages/
//!   incoming/                        # reserved for Phase 5 upload mode
//!   staging/<package_id>/            # verified extraction + commit tree
//!   backup/<task>-<ver>-<ts>/        # {task/, entry.json}; never deleted
//!   installed.json                   # schema_version 1 registry
//! ```
//!
//! Install order is the design §B.4.2 order, and every step after staging is
//! rolled back on failure (task dirs restored from backups, newly applied asset
//! files removed, `installed.json` only replaced after the task commit).

use std::collections::{BTreeMap, BTreeSet};
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use oas_task::package::{
    build_package, extract_to, read_package, sha256_file, unsupported_flow_ops,
    version_at_least, verify_checksums, AssetRef, BuildOptions, BuildOutcome, PackageArchive,
    PackageError, ENGINE_VERSION, PACKAGE_EXTENSION, SUPPORTED_FLOW_OPS,
};
use oas_task::{validate_task_name, AssetDecl, AssetsFile, Meta, TaskBundle, TEMPLATE_VERSION};

use crate::runner_config;

/// Default Studio output directory (`oasd studio publish`).
pub const DEFAULT_OUT_DIR: &str = "dist/packages";
pub const DEFAULT_TASKS_DIR: &str = "tasks";
pub const DEFAULT_ASSETS_DIR: &str = "assets";
pub const INSTALLED_FILE: &str = "installed.json";
pub const INSTALLED_SCHEMA_VERSION: u32 = 1;
const ENTRY_SNAPSHOT: &str = "entry.json";
const BACKUP_DIR_NAME: &str = "backup";

/// Default global asset store for the CLI: the `assets` sibling of the tasks
/// dir, matching [`BuildOptions::new`]. The CLI used to hard-code `assets`
/// relative to the CWD, which silently produced warning-only snapshots when
/// `--tasks-dir` pointed somewhere else.
fn default_assets_dir(tasks_dir: &Path) -> PathBuf {
    tasks_dir
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(|p| p.join("assets"))
        .unwrap_or_else(|| PathBuf::from(DEFAULT_ASSETS_DIR))
}

/// `out` is request/CLI input: keep it inside the repository-local `dist/` tree
/// so a loopback HTTP caller cannot write `.oaspkg` files anywhere on disk.
fn validate_publish_out(out: &Path) -> Result<(), String> {
    if out.as_os_str().is_empty() {
        return Err("`out` must not be empty".to_string());
    }
    if out.is_absolute() {
        return Err(format!(
            "`out` must be relative to the current directory (got `{}`); packages must be \
             written under `dist/`",
            out.display()
        ));
    }
    let mut components = out.components();
    let first = match components.next() {
        Some(Component::CurDir) => components.next(),
        other => other,
    };
    match first {
        Some(Component::Normal(name)) if name.eq_ignore_ascii_case("dist") => {}
        _ => {
            return Err(format!(
                "`out` must start with `dist/` (got `{}`); writing packages outside `dist/` is \
                 not allowed",
                out.display()
            ))
        }
    }
    for component in components {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            _ => {
                return Err(format!(
                    "`out` must stay inside `dist/` (got `{}`; `..` and root prefixes are \
                     refused)",
                    out.display()
                ))
            }
        }
    }
    Ok(())
}

static SEQ: AtomicU64 = AtomicU64::new(0);

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn short_seq() -> u64 {
    SEQ.fetch_add(1, Ordering::Relaxed)
}

// ---------------------------------------------------------------------------
// Studio publish
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ToolContext {
    pub version: String,
    pub git: Option<String>,
    pub dirty: bool,
    pub repo: String,
    pub commit: String,
}

fn git_output(args: &[&str]) -> Option<String> {
    let out = std::process::Command::new("git").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

/// Git/source metadata for the manifest, discovered once per process.
///
/// A non-git checkout (source tarball, Android packaging) simply yields empty
/// `source.*`/`tool.git` fields instead of failing the publish.
pub fn tool_context() -> ToolContext {
    static TOOL: OnceLock<ToolContext> = OnceLock::new();
    TOOL.get_or_init(|| {
        let commit = git_output(&["rev-parse", "--short", "HEAD"]).unwrap_or_default();
        let repo = git_output(&["rev-parse", "--show-toplevel"])
            .map(|p| p.replace('\\', "/"))
            .unwrap_or_default();
        let dirty = git_output(&["status", "--porcelain"])
            .map(|s| !s.is_empty())
            .unwrap_or(false);
        ToolContext {
            version: env!("CARGO_PKG_VERSION").to_string(),
            git: (!commit.is_empty()).then(|| commit.clone()),
            dirty,
            repo,
            commit,
        }
    })
    .clone()
}

#[derive(Debug, Clone, Default)]
pub struct PublishRequest {
    pub task: String,
    pub out: Option<PathBuf>,
    pub force: bool,
    pub include_deps: bool,
    pub reproducible: bool,
}

/// Build one package for `task` under `tasks_dir`.
pub fn publish(
    tasks_dir: &Path,
    assets_dir: &Path,
    default_out: &Path,
    req: &PublishRequest,
) -> Result<BuildOutcome, PackageError> {
    let task = req.task.trim();
    validate_task_name(task).map_err(PackageError::Other)?;
    if let Some(out) = &req.out {
        validate_publish_out(out).map_err(PackageError::Other)?;
    }
    let task_dir = tasks_dir.join(task);
    let out = req.out.clone().unwrap_or_else(|| default_out.to_path_buf());
    let mut opts = BuildOptions::new(&task_dir);
    opts.include_deps = req.include_deps;
    opts.reproducible = req.reproducible;
    opts.force = req.force;
    opts.tasks_dir = tasks_dir.to_path_buf();
    opts.assets_dir = assets_dir.to_path_buf();
    let tool = tool_context();
    opts.tool_version = tool.version;
    opts.git = tool.git;
    opts.dirty = tool.dirty;
    opts.surface = "studio".to_string();
    opts.repo = tool.repo;
    opts.commit = tool.commit;
    // seq136 D1-M2: navigate targets are checked against the ONE global map
    build_package(&task_dir, &out, &opts)
}

fn usage() -> ExitCode {
    eprintln!(
        "usage: oasd studio publish --task <id> [--out <dir|file.oaspkg> (relative, under dist/)] \
         [--tasks-dir <dir>] [--assets-dir <dir>] [--force] [--include-deps] [--reproducible]"
    );
    ExitCode::from(2)
}

/// `oasd studio publish …` CLI.
pub fn publish_cmd(rest: &[String]) -> ExitCode {
    let cfg = crate::config::global();
    let mut task = String::new();
    let mut out: Option<PathBuf> = None;
    let mut tasks_dir = cfg
        .map(|c| c.tasks_dir())
        .unwrap_or_else(|| PathBuf::from(DEFAULT_TASKS_DIR));
    let mut assets_dir: Option<PathBuf> = None;
    let mut force = false;
    let mut include_deps = false;
    let mut reproducible = false;
    let mut i = 0usize;
    while i < rest.len() {
        match rest[i].as_str() {
            "--task" | "-t" if i + 1 < rest.len() => {
                task = rest[i + 1].clone();
                i += 2;
            }
            "--out" | "-o" if i + 1 < rest.len() => {
                out = Some(PathBuf::from(&rest[i + 1]));
                i += 2;
            }
            "--tasks-dir" if i + 1 < rest.len() => {
                tasks_dir = PathBuf::from(&rest[i + 1]);
                i += 2;
            }
            "--assets-dir" if i + 1 < rest.len() => {
                assets_dir = Some(PathBuf::from(&rest[i + 1]));
                i += 2;
            }
            "--force" => {
                force = true;
                i += 1;
            }
            "--include-deps" => {
                include_deps = true;
                i += 1;
            }
            "--reproducible" => {
                reproducible = true;
                i += 1;
            }
            other => {
                eprintln!("unknown publish arg: {other}");
                return usage();
            }
        }
    }
    if task.trim().is_empty() {
        eprintln!("publish requires --task <id>");
        return usage();
    }
    let default_out = PathBuf::from(DEFAULT_OUT_DIR);
    let assets_dir = assets_dir.unwrap_or_else(|| default_assets_dir(&tasks_dir));
    let req = PublishRequest {
        task,
        out,
        force,
        include_deps,
        reproducible,
    };
    match publish(&tasks_dir, &assets_dir, &default_out, &req) {
        Ok(outcome) => {
            println!("published: {}", outcome.path.display());
            println!("sha256: {}", outcome.sha256);
            println!("package_id: {}", outcome.manifest.package_id);
            println!(
                "engine_flow_ops: {}",
                outcome.manifest.engine.flow_ops.join(",")
            );
            println!(
                "dependencies: {} (included: {})",
                outcome.manifest.dependencies.len(),
                outcome
                    .manifest
                    .dependencies
                    .iter()
                    .filter(|d| d.included)
                    .count()
            );
            for warning in &outcome.warnings {
                println!("warning: {warning}");
            }
            ExitCode::SUCCESS
        }
        Err(PackageError::Exists(path)) => {
            eprintln!(
                "publish failed: package already exists: {} (use --force or bump meta.version)",
                path.display()
            );
            ExitCode::FAILURE
        }
        Err(PackageError::Validation(issues)) => {
            eprintln!("publish failed: {} lint/validation error(s)", issues.len());
            for issue in issues {
                eprintln!("  {issue}");
            }
            ExitCode::FAILURE
        }
        Err(PackageError::Other(e)) => {
            eprintln!("publish failed: {e}");
            ExitCode::FAILURE
        }
    }
}

// ---------------------------------------------------------------------------
// HTTP-friendly failure
// ---------------------------------------------------------------------------

/// One install/uninstall/activate failure, already mapped to an HTTP status.
#[derive(Debug, Clone)]
pub struct ApiFailure {
    pub status: u16,
    pub code: Option<String>,
    pub errors: Vec<String>,
}

impl ApiFailure {
    pub fn new(status: u16, message: impl Into<String>) -> Self {
        Self {
            status,
            code: None,
            errors: vec![message.into()],
        }
    }

    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    pub fn errors(status: u16, errors: Vec<String>) -> Self {
        Self {
            status,
            code: None,
            errors,
        }
    }
}

pub fn publish_failure(err: PackageError) -> ApiFailure {
    match err {
        PackageError::Exists(path) => ApiFailure::new(
            409,
            format!(
                "package already exists: {} (use force:true or bump meta.version)",
                path.display()
            ),
        ),
        PackageError::Validation(issues) => {
            ApiFailure::errors(400, issues.iter().map(ToString::to_string).collect())
        }
        PackageError::Other(msg) => ApiFailure::new(400, msg),
    }
}

// ---------------------------------------------------------------------------
// installed.json
// ---------------------------------------------------------------------------

fn default_schema() -> u32 {
    INSTALLED_SCHEMA_VERSION
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct InstalledSource {
    #[serde(default)]
    pub repo: String,
    #[serde(default)]
    pub commit: String,
    #[serde(default)]
    pub signed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstalledPackage {
    pub package_id: String,
    pub task_id: String,
    pub version: u32,
    pub sha256: String,
    pub installed_at_ms: u64,
    #[serde(default)]
    pub source: InstalledSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backup: Option<String>,
    /// Task ids this package declares as `call_task` dependencies (install guard
    /// for uninstall; design §B.4.4). Extra field, older readers ignore it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstalledFile {
    #[serde(default = "default_schema")]
    pub schema_version: u32,
    #[serde(default)]
    pub packages: Vec<InstalledPackage>,
}

impl Default for InstalledFile {
    fn default() -> Self {
        Self {
            schema_version: INSTALLED_SCHEMA_VERSION,
            packages: Vec::new(),
        }
    }
}

impl InstalledFile {
    pub fn find(&self, task_id: &str) -> Option<&InstalledPackage> {
        self.packages.iter().find(|p| p.task_id == task_id)
    }

    pub fn remove_task(&mut self, task_id: &str) -> Option<InstalledPackage> {
        let idx = self.packages.iter().position(|p| p.task_id == task_id)?;
        Some(self.packages.remove(idx))
    }

    pub fn upsert(&mut self, entry: InstalledPackage) {
        self.remove_task(&entry.task_id);
        self.packages.push(entry);
        self.packages.sort_by(|a, b| a.task_id.cmp(&b.task_id));
    }

    pub fn validated(mut self, path: &Path) -> Result<Self, String> {
        if self.schema_version != INSTALLED_SCHEMA_VERSION {
            return Err(format!(
                "installed registry `{}` has unsupported schema_version {} (expected {})",
                path.display(),
                self.schema_version,
                INSTALLED_SCHEMA_VERSION
            ));
        }
        self.packages.sort_by(|a, b| a.task_id.cmp(&b.task_id));
        Ok(self)
    }
}

#[derive(Debug, Clone)]
pub struct PackagesLayout {
    pub root: PathBuf,
}

impl PackagesLayout {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn staging_dir(&self) -> PathBuf {
        self.root.join("staging")
    }

    pub fn backup_dir(&self) -> PathBuf {
        self.root.join("backup")
    }

    /// Reserved for Phase 5 upload mode; present so the on-disk layout is
    /// documented in one place.
    #[allow(dead_code)]
    pub fn incoming_dir(&self) -> PathBuf {
        self.root.join("incoming")
    }

    pub fn installed_path(&self) -> PathBuf {
        self.root.join(INSTALLED_FILE)
    }
}

/// Default packages root: sibling of the tasks dir (`tasks` -> `packages`).
pub fn default_packages_root(tasks_dir: &Path) -> PathBuf {
    match tasks_dir.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.join("packages"),
        _ => PathBuf::from("packages"),
    }
}

pub fn read_installed(root: &Path) -> Result<InstalledFile, String> {
    let path = root.join(INSTALLED_FILE);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(InstalledFile::default()),
        Err(e) => return Err(format!("read {}: {e}", path.display())),
    };
    let file: InstalledFile =
        serde_json::from_str(&text).map_err(|e| format!("{}: invalid JSON: {e}", path.display()))?;
    file.validated(&path)
}

/// Actionable fail-closed message for an unreadable/corrupt `installed.json`
/// (PR7 final audit P1-4).
///
/// Install / uninstall / activate stay fail-closed (500: the registry is their
/// only source of truth and a guess could overwrite the wrong task), but the
/// error must tell the operator how to recover **without** deleting the task
/// bundles. `/runner/tasks` uses the shorter warning variant instead of failing
/// the whole page.
pub fn installed_registry_error(root: &Path, err: &str) -> String {
    format!(
        "installed.json 损坏或不可读（{err}）。请删除或重命名 `{}` 后重启 Runner\
         （`tasks/` 中的任务仍会作为未托管任务显示），或重新安装任务包。",
        root.join(INSTALLED_FILE).display()
    )
}

pub fn write_installed(root: &Path, file: &InstalledFile) -> Result<(), String> {
    let text = serde_json::to_string_pretty(file)
        .map_err(|e| format!("serialize installed registry: {e}"))?;
    write_text_atomic(&root.join(INSTALLED_FILE), &format!("{text}\n"))
}

fn write_text_atomic(path: &Path, text: &str) -> Result<(), String> {
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    if !parent.as_os_str().is_empty() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("create {}: {e}", parent.display()))?;
    }
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("installed.json");
    let tmp = parent.join(format!(".{name}.tmp-{}-{}", std::process::id(), short_seq()));
    if let Err(e) = std::fs::write(&tmp, text) {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("write {}: {e}", tmp.display()));
    }
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!(
            "rename {} -> {}: {e}",
            tmp.display(),
            path.display()
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// FS helpers (cross-platform rename fallback)
// ---------------------------------------------------------------------------

fn copy_dir_all(src: &Path, dst: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dst).map_err(|e| format!("create {}: {e}", dst.display()))?;
    for entry in std::fs::read_dir(src).map_err(|e| format!("read {}: {e}", src.display()))? {
        let entry = entry.map_err(|e| format!("read {}: {e}", src.display()))?;
        let ty = entry
            .file_type()
            .map_err(|e| format!("stat {}: {e}", entry.path().display()))?;
        let target = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &target)?;
        } else if ty.is_file() {
            std::fs::copy(entry.path(), &target)
                .map_err(|e| format!("copy {} -> {}: {e}", entry.path().display(), target.display()))?;
        }
    }
    Ok(())
}

/// Move a directory; falls back to copy+remove across volumes or on Windows
/// when the destination appears concurrently.
fn move_dir(src: &Path, dst: &Path) -> Result<(), String> {
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("create {}: {e}", parent.display()))?;
    }
    match std::fs::rename(src, dst) {
        Ok(()) => Ok(()),
        Err(rename_err) => {
            if let Err(copy_err) = copy_dir_all(src, dst) {
                return Err(format!(
                    "move {} -> {}: rename failed ({rename_err}); copy failed ({copy_err})",
                    src.display(),
                    dst.display()
                ));
            }
            std::fs::remove_dir_all(src)
                .map_err(|e| format!("remove {} after copy: {e}", src.display()))?;
            Ok(())
        }
    }
}

fn unique_child(parent: &Path, base: &str) -> PathBuf {
    let mut candidate = parent.join(base);
    let mut n = 1u32;
    while candidate.exists() {
        candidate = parent.join(format!("{base}-{n}"));
        n += 1;
    }
    candidate
}

fn mtime_ms(path: &Path) -> Option<u64> {
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    modified
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|d| d.as_millis() as u64)
}

fn validate_relative_inside(root: &Path, rel: &str) -> Result<PathBuf, String> {
    if rel.trim().is_empty() {
        return Err("backup path is empty".to_string());
    }
    let path = Path::new(rel);
    if path.is_absolute() {
        return Err(format!("backup path `{rel}` must be relative"));
    }
    for component in path.components() {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            _ => return Err(format!("backup path `{rel}` escapes the packages directory")),
        }
    }
    Ok(root.join(path))
}

/// A backup path must be relative, stay inside the packages root **and** live
/// under the `backup/` prefix. This closes the old "any relative path under the
/// packages root" hole (`staging/`, other control dirs).
fn validate_backup_path(layout: &PackagesLayout, rel: &str) -> Result<PathBuf, String> {
    let abs = validate_relative_inside(&layout.root, rel)?;
    match Path::new(rel).components().next() {
        Some(Component::Normal(first)) if first.eq_ignore_ascii_case(BACKUP_DIR_NAME) => Ok(abs),
        _ => Err(format!(
            "backup path `{rel}` must be inside the packages `backup/` directory"
        )),
    }
}

fn relative_to(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| path.to_string_lossy().replace('\\', "/"))
}

// ---------------------------------------------------------------------------
// Install
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct InstallOptions {
    pub force: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstallOutcome {
    pub package_id: String,
    pub task_id: String,
    pub version: u32,
    pub replaced: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backup: Option<String>,
    pub warnings: Vec<String>,
    pub installed_tasks: Vec<String>,
}

#[derive(Debug)]
struct CommittedTask {
    task_id: String,
    target: PathBuf,
    backup_dir: Option<PathBuf>,
}

struct AssetMerge {
    store_dir: PathBuf,
    store_path: PathBuf,
    old_store: Option<Vec<u8>>,
    new_store: Vec<u8>,
    writes: Vec<(PathBuf, Vec<u8>)>,
}

fn read_meta_at(dir: &Path, at: &str) -> Result<Meta, ApiFailure> {
    let path = dir.join("meta.json");
    let text = std::fs::read_to_string(&path)
        .map_err(|e| ApiFailure::new(400, format!("{at}: read {}: {e}", path.display())))?;
    Meta::from_json(&text)
        .map_err(|e| ApiFailure::new(400, format!("{at}: meta.json is invalid: {e}")))
}

fn check_template_version(meta: &Meta, at: &str) -> Result<(), ApiFailure> {
    if let Some(v) = meta.template_version {
        if v > TEMPLATE_VERSION {
            return Err(ApiFailure::new(
                400,
                format!(
                    "{at}: template_version {v} is newer than this engine supports \
                     ({TEMPLATE_VERSION}); upgrade the Runner"
                ),
            ));
        }
    }
    Ok(())
}

fn list_subdirs(dir: &Path) -> Result<Vec<String>, ApiFailure> {
    let mut out = Vec::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(out),
        Err(e) => {
            return Err(ApiFailure::new(
                500,
                format!("read {}: {e}", dir.display()),
            ))
        }
    };
    for entry in entries {
        let entry = entry.map_err(|e| ApiFailure::new(500, format!("read {}: {e}", dir.display())))?;
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            if let Some(name) = entry.file_name().to_str() {
                out.push(name.to_string());
            }
        }
    }
    out.sort();
    Ok(out)
}

fn copy_meta_flow(src: &Path, dst: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dst).map_err(|e| format!("create {}: {e}", dst.display()))?;
    for name in ["meta.json", "flow.json"] {
        let from = src.join(name);
        if from.is_file() {
            std::fs::copy(&from, dst.join(name))
                .map_err(|e| format!("copy {}: {e}", from.display()))?;
        }
    }
    Ok(())
}

fn collect_lint_issues(
    tree: &Path,
    lint_dir: &Path,
    task_ids: &[String],
    external_assets: Option<&BTreeSet<String>>,
    global_assets: Option<&AssetsFile>,
) -> Result<Vec<String>, ApiFailure> {
    let mut errors = Vec::new();
    for task_id in task_ids {
        let bundle = TaskBundle::open_named(tree, task_id).map_err(|e| {
            ApiFailure::new(400, format!("task `{task_id}`: {e}"))
        })?;
        let mut issues = Vec::new();
        bundle.lint_into_with_calls_and_global_assets(
            &mut issues,
            lint_dir,
            external_assets,
            global_assets,
        );
        if !issues.is_empty() {
            errors.extend(issues.into_iter().map(|mut issue| {
                issue.path = format!("{task_id}/{}", issue.path);
                issue.to_string()
            }));
        }
    }
    Ok(errors)
}

/// Asset universe + merged [`AssetsFile`] for the install-time lint.
///
/// Global-asset-mode tasks reference ids that are **not** in `task/assets.json`;
/// the lint must therefore see (a) the current global store and (b) the ids this
/// package is about to add (the merge happens after the task commit). The parsed
/// global store is required too: T8 binds `meta.variables[].asset_id` to the
/// ocr_var declaration, so an id-only universe would still report a false
/// "not a declared asset" (seq129-fix).
///
/// Priority for the merged view: the **package snapshot wins** on a same-id
/// clash. This mirrors the eventual store merge; note the installer separately
/// rejects a same-id definition/byte conflict (`asset_conflicts`), so the lint
/// view never silently hides a real incompatibility.
///
/// Returns `(None, None)` when neither the store nor the package snapshot exists:
/// that keeps the legacy task-local-only mode unchanged.
fn install_lint_assets(
    staging: &Path,
    assets_dir: &Path,
) -> Result<(Option<BTreeSet<String>>, Option<AssetsFile>), ApiFailure> {
    let global = oas_task::package::global_assets_file(assets_dir)
        .map_err(|e| ApiFailure::new(500, format!("global assets: {e}")))?;
    let package_assets = staging.join("assets").join("assets.json");
    let package: Option<AssetsFile> = if package_assets.is_file() {
        let text = std::fs::read_to_string(&package_assets)
            .map_err(|e| ApiFailure::new(400, format!("read {}: {e}", package_assets.display())))?;
        let file: AssetsFile = serde_json::from_str(&text)
            .map_err(|e| ApiFailure::new(400, format!("package assets.json: {e}")))?;
        Some(file)
    } else {
        None
    };
    let merged = match (package.as_ref(), global.as_ref()) {
        // Package snapshot wins over the current store (same as the install merge).
        (Some(pkg), Some(global)) => Some(oas_task::merge_global_assets_for_lint(Some(pkg), global)),
        (Some(pkg), None) => Some(pkg.clone()),
        (None, Some(global)) => Some(global.clone()),
        (None, None) => None,
    };
    let ids = merged.as_ref().map(|af| {
        let mut ids: BTreeSet<String> = af.items.iter().map(|a| a.id.clone()).collect();
        ids.extend(af.collections.iter().map(|c| c.id.clone()));
        ids
    });
    Ok((ids, merged))
}

fn dependency_resolution(
    manifest: &oas_task::package::Manifest,
    installed: &InstalledFile,
) -> Result<(), ApiFailure> {
    let mut errors = Vec::new();
    for dep in &manifest.dependencies {
        if dep.included {
            continue;
        }
        match installed.find(&dep.task_id) {
            Some(entry) if entry.version >= dep.version => {}
            Some(entry) => errors.push(format!(
                "dependency `{}` requires installed version >= {}, found {}",
                dep.task_id, dep.version, entry.version
            )),
            None => errors.push(format!(
                "dependency `{}` requires installed version >= {}, but it is not installed",
                dep.task_id, dep.version
            )),
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(ApiFailure::new(400, errors.join("; ")).with_code("missing_dependency"))
    }
}

fn same_asset_definition(local: &AssetDecl, packaged: &AssetDecl) -> bool {
    // The allocated image file name and `rev` are implementation details: the
    // installer may rename a new image on the first install, so a re-install of
    // the same package must still count as "same asset". Everything user-visible
    // (kind/ROI/action/OCR/threshold/polygon) must match.
    let mut a = local.clone();
    let mut b = packaged.clone();
    a.file = None;
    b.file = None;
    a.rev = None;
    b.rev = None;
    let definition_same = a == b;
    if !definition_same {
        return false;
    }
    match (local.file.as_deref(), packaged.file.as_deref()) {
        (Some(_), Some(_)) => true, // caller compares file sha256 separately
        (None, None) => true,
        _ => false,
    }
}

#[derive(Debug, Clone)]
struct AssetConflictDetail {
    id: String,
    is_collection: bool,
    local_sha: Option<String>,
    package_sha: Option<String>,
}

fn plan_asset_merge(
    staging: &Path,
    assets_dir: &Path,
    asset_refs: &[AssetRef],
    warnings: &mut Vec<String>,
) -> Result<Option<AssetMerge>, ApiFailure> {
    let package_assets = staging.join("assets").join("assets.json");
    if !package_assets.is_file() {
        // A package that declares asset refs but carries no snapshot cannot run
        // on this Runner (the referenced ids would be undeclared at runtime);
        // that is a corrupt/incomplete package, never a silent skip.
        if !asset_refs.is_empty() {
            let ids: Vec<&str> = asset_refs.iter().map(|a| a.id.as_str()).collect();
            return Err(ApiFailure::new(
                400,
                format!(
                    "package is missing `assets/assets.json` but manifest.asset_refs lists {} \
                     asset(s) ({}); the package is incomplete and the assets cannot be installed \
                     — republish it with the asset snapshot",
                    asset_refs.len(),
                    ids.join(", ")
                ),
            )
            .with_code("missing_asset_snapshot"));
        }
        return Ok(None);
    }
    let package_text = std::fs::read_to_string(&package_assets)
        .map_err(|e| ApiFailure::new(400, format!("read {}: {e}", package_assets.display())))?;
    let package: AssetsFile = serde_json::from_str(&package_text)
        .map_err(|e| ApiFailure::new(400, format!("package assets.json: invalid JSON: {e}")))?;
    if package.schema_version != oas_task::SCHEMA_VERSION {
        return Err(ApiFailure::new(
            400,
            format!(
                "package assets.json schema_version {} is not supported (expected {})",
                package.schema_version,
                oas_task::SCHEMA_VERSION
            ),
        ));
    }

    // A brand-new Runner-only distribution ships `assets/.keep` only: the
    // package snapshot is then the *initial content* of the local store. The
    // normal merge below appends every packaged id, so the store ends up equal
    // to the snapshot and the same rollback path (remove store + images) works.
    let store_path = assets_dir.join("assets.json");
    let initializing = !store_path.is_file();
    let old_store = if initializing {
        None
    } else {
        Some(
            std::fs::read(&store_path)
                .map_err(|e| ApiFailure::new(500, format!("read {}: {e}", store_path.display())))?,
        )
    };
    let mut store: AssetsFile = match &old_store {
        Some(bytes) => serde_json::from_slice(bytes).map_err(|e| {
            ApiFailure::new(500, format!("{}: invalid JSON: {e}", store_path.display()))
        })?,
        None => AssetsFile {
            schema_version: package.schema_version,
            design_resolution: package.design_resolution,
            items: Vec::new(),
            collections: Vec::new(),
        },
    };
    if initializing {
        warnings.push(format!(
            "initialized the global asset store at {} from the package snapshot ({} asset(s), \
             {} collection(s))",
            assets_dir.display(),
            package.items.len(),
            package.collections.len()
        ));
    }

    let mut conflicts: Vec<AssetConflictDetail> = Vec::new();
    for item in &store.items {
        if let Some(file) = item.file.as_deref() {
            if let Err(e) = oas_task::package::validate_archive_path(file) {
                return Err(ApiFailure::new(
                    500,
                    format!("global asset `{}`: {e}", item.id),
                ));
            }
        }
    }
    for item in &package.items {
        if let Some(file) = item.file.as_deref() {
            oas_task::package::validate_archive_path(file).map_err(|e| {
                ApiFailure::new(400, format!("package asset `{}`: {e}", item.id))
            })?;
        }
    }
    for item in &package.items {
        if let Some(local) = store.items.iter().find(|a| a.id == item.id) {
            let definition_same = same_asset_definition(local, item);
            let (bytes_same, local_sha, package_sha) =
                match (local.file.as_deref(), item.file.as_deref()) {
                    (Some(a), Some(b)) => {
                        let local_sha = sha256_file(&assets_dir.join(a)).ok();
                        let package_sha = sha256_file(&staging.join("assets").join(b)).ok();
                        (
                            local_sha.is_some() && local_sha == package_sha,
                            local_sha,
                            package_sha,
                        )
                    }
                    (None, None) => (true, None, None),
                    _ => (false, None, None),
                };
            if !definition_same || !bytes_same {
                conflicts.push(AssetConflictDetail {
                    id: item.id.clone(),
                    is_collection: false,
                    local_sha,
                    package_sha,
                });
            }
        }
    }
    for collection in &package.collections {
        if let Some(local) = store.collections.iter().find(|c| c.id == collection.id) {
            if local != collection {
                conflicts.push(AssetConflictDetail {
                    id: collection.id.clone(),
                    is_collection: true,
                    local_sha: None,
                    package_sha: None,
                });
            }
        }
    }
    if !conflicts.is_empty() {
        // Phase 1 cannot rename a same-id asset and rewrite the flow that points
        // at it, so make the failure actionable instead of just refusing.
        let mut errors = Vec::new();
        for conflict in &conflicts {
            if conflict.is_collection {
                errors.push(format!(
                    "global asset collection `{}` differs from the packaged snapshot",
                    conflict.id
                ));
                continue;
            }
            match (&conflict.local_sha, &conflict.package_sha) {
                (Some(local), Some(packaged)) => errors.push(format!(
                    "global asset `{}`: local sha256 {local}, package sha256 {packaged}",
                    conflict.id
                )),
                _ => errors.push(format!(
                    "global asset `{}`: its definition differs from the packaged snapshot and \
                     the image bytes could not be hashed",
                    conflict.id
                )),
            }
        }
        let ids: Vec<&str> = conflicts.iter().map(|c| c.id.as_str()).collect();
        errors.push(format!(
            "asset_conflicts: {} — rename the conflicting asset(s) in Studio and publish again, \
             or clear this Runner's global asset store (advanced: removes every global asset); \
             automatic rename `<id>_2` + flow rewrite for same-id assets is not available in this \
             version and is planned for a later release",
            ids.join(", ")
        ));
        return Err(ApiFailure {
            status: 400,
            code: Some("asset_conflicts".to_string()),
            errors,
        });
    }

    let mut used_files: BTreeSet<String> = store
        .items
        .iter()
        .filter_map(|a| a.file.clone())
        .collect();
    let mut next_image = store
        .items
        .iter()
        .filter_map(|a| a.file.as_deref())
        .filter_map(|f| f.strip_prefix("images/a"))
        .filter_map(|f| f.strip_suffix(".png"))
        .filter_map(|n| n.parse::<u32>().ok())
        .max()
        .unwrap_or(0)
        + 1;

    let mut writes = Vec::new();
    let mut new_items = Vec::new();
    for item in &package.items {
        if store.items.iter().any(|a| a.id == item.id) {
            continue; // same definition (verified above) => reuse the local file
        }
        let mut next = item.clone();
        if let Some(file) = item.file.as_deref() {
            let src = staging.join("assets").join(file);
            let bytes = std::fs::read(&src)
                .map_err(|e| ApiFailure::new(400, format!("read package asset {}: {e}", src.display())))?;
            let target_file = if used_files.contains(file) || assets_dir.join(file).exists() {
                let allocated = loop {
                    let candidate = format!("images/a{next_image:03}.png");
                    next_image += 1;
                    if !used_files.contains(&candidate) && !assets_dir.join(&candidate).exists() {
                        break candidate;
                    }
                };
                allocated
            } else {
                file.to_string()
            };
            used_files.insert(target_file.clone());
            next.file = Some(target_file.clone());
            writes.push((assets_dir.join(target_file), bytes));
        }
        next.rev = next.rev.or(Some(1));
        new_items.push(next);
    }
    let new_collections: Vec<_> = package
        .collections
        .iter()
        .filter(|c| !store.collections.iter().any(|l| l.id == c.id))
        .cloned()
        .collect();
    if !initializing && new_items.is_empty() && new_collections.is_empty() && writes.is_empty() {
        return Ok(None);
    }
    store.items.extend(new_items);
    store.collections.extend(new_collections);
    let mut new_store = serde_json::to_vec_pretty(&store)
        .map_err(|e| ApiFailure::new(500, format!("serialize global assets: {e}")))?;
    new_store.push(b'\n');
    Ok(Some(AssetMerge {
        store_dir: assets_dir.to_path_buf(),
        store_path,
        old_store,
        new_store,
        writes,
    }))
}

fn apply_asset_merge(merge: &AssetMerge) -> Result<(), String> {
    for (path, bytes) in &merge.writes {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("create {}: {e}", parent.display()))?;
        }
        std::fs::write(path, bytes).map_err(|e| format!("write {}: {e}", path.display()))?;
    }
    write_text_atomic(&merge.store_path, &String::from_utf8_lossy(&merge.new_store))
}

fn rollback_asset_merge(merge: &AssetMerge) {
    for (path, _) in &merge.writes {
        let _ = std::fs::remove_file(path);
    }
    match &merge.old_store {
        Some(bytes) => {
            let _ = std::fs::write(&merge.store_path, bytes);
        }
        None => {
            let _ = std::fs::remove_file(&merge.store_path);
        }
    }
    let _ = merge.store_dir; // keep the field explicit for future cleanup
}

fn rollback_tasks(committed: &[CommittedTask]) {
    for task in committed.iter().rev() {
        let _ = std::fs::remove_dir_all(&task.target);
        if let Some(backup) = &task.backup_dir {
            let source = backup.join("task");
            if source.is_dir() {
                let _ = move_dir(&source, &task.target);
            }
        }
    }
}

fn snapshot_entry(backup_dir: &Path, entry: Option<&InstalledPackage>) -> Result<(), String> {
    if let Some(entry) = entry {
        let text = serde_json::to_string_pretty(entry)
            .map_err(|e| format!("serialize entry snapshot: {e}"))?;
        std::fs::write(backup_dir.join(ENTRY_SNAPSHOT), format!("{text}\n"))
            .map_err(|e| format!("write {ENTRY_SNAPSHOT}: {e}"))?;
    }
    Ok(())
}

/// Install one `.oaspkg` (Phase 1 path mode only).
pub fn install_from_path(
    layout: &PackagesLayout,
    tasks_dir: &Path,
    assets_dir: &Path,
    zip_path: &Path,
    opts: &InstallOptions,
    now: u64,
) -> Result<InstallOutcome, ApiFailure> {
    if zip_path.as_os_str().is_empty() {
        return Err(ApiFailure::new(400, "`path` is required (Phase 1 path mode)"));
    }
    if !zip_path.is_absolute() {
        return Err(ApiFailure::new(
            400,
            format!("`path` must be an absolute path: {}", zip_path.display()),
        ));
    }
    if !zip_path.is_file() {
        return Err(ApiFailure::new(
            400,
            format!("package not found: {}", zip_path.display()),
        ));
    }
    if !zip_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case(PACKAGE_EXTENSION))
        .unwrap_or(false)
    {
        return Err(ApiFailure::new(
            400,
            format!("`path` must end with .{PACKAGE_EXTENSION}"),
        ));
    }

    let archive = read_package(zip_path)
        .map_err(|e| ApiFailure::new(400, format!("invalid package: {e}")))?;
    let archive_sha = sha256_file(zip_path)
        .map_err(|e| ApiFailure::new(400, format!("hash package: {e}")))?;
    let mut warnings = Vec::new();
    let manifest = archive.manifest.clone();
    let unsupported = unsupported_flow_ops(&manifest.engine.flow_ops);
    if !unsupported.is_empty() {
        return Err(ApiFailure::new(
            400,
            format!(
                "unsupported flow_ops: {} (this Runner supports: {}); upgrade the Runner \
                 before installing this package",
                unsupported.join(", "),
                SUPPORTED_FLOW_OPS.join(", ")
            ),
        )
        .with_code("unsupported_flow_ops"));
    }
    let min_engine = manifest.engine.min_engine.trim();
    if !min_engine.is_empty() && !version_at_least(ENGINE_VERSION, min_engine) {
        return Err(ApiFailure::new(
            400,
            format!(
                "package requires engine >= {min_engine}, but this Runner is {ENGINE_VERSION}; \
                 upgrade the Runner before installing this package"
            ),
        )
        .with_code("engine_too_old"));
    }

    let package_id = manifest.package_id.clone();
    let staging_parent = layout.staging_dir();
    std::fs::create_dir_all(&staging_parent).map_err(|e| {
        ApiFailure::new(500, format!("create {}: {e}", staging_parent.display()))
    })?;
    let staging = unique_child(&staging_parent, &sanitize_staging_name(&package_id));
    let cleanup = || {
        let _ = std::fs::remove_dir_all(&staging);
    };
    let result = install_staged(
        layout,
        tasks_dir,
        assets_dir,
        zip_path,
        &archive_sha,
        &archive,
        &staging,
        opts,
        now,
        &mut warnings,
    );
    match result {
        Ok(mut outcome) => {
            outcome.warnings = warnings;
            cleanup();
            Ok(outcome)
        }
        Err(e) => {
            cleanup();
            Err(e)
        }
    }
}

fn sanitize_staging_name(package_id: &str) -> String {
    let mut out = String::new();
    for ch in package_id.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
            out.push(ch);
        } else {
            out.push('_');
        }
        if out.len() >= 120 {
            break;
        }
    }
    if out.is_empty() {
        "package".to_string()
    } else {
        out
    }
}

#[allow(clippy::too_many_arguments)]
fn install_staged(
    layout: &PackagesLayout,
    tasks_dir: &Path,
    assets_dir: &Path,
    zip_path: &Path,
    archive_sha: &str,
    archive: &PackageArchive,
    staging: &Path,
    opts: &InstallOptions,
    now: u64,
    warnings: &mut Vec<String>,
) -> Result<InstallOutcome, ApiFailure> {
    let manifest = &archive.manifest;
    extract_to(zip_path, staging)
        .map_err(|e| ApiFailure::new(400, format!("extract package: {e}")))?;
    verify_checksums(staging)
        .map_err(|errors| ApiFailure::errors(400, errors))?;

    let task_dir = staging.join("task");
    if !task_dir.is_dir() {
        return Err(ApiFailure::new(400, "package is missing the `task/` directory"));
    }
    let root_meta = read_meta_at(&task_dir, &format!("task `{}`", manifest.task_id))?;
    check_template_version(&root_meta, &format!("task `{}`", manifest.task_id))?;
    if root_meta.task_id() != manifest.task_id {
        return Err(ApiFailure::new(
            400,
            format!(
                "manifest.task_id `{}` does not match task/meta.json task_id `{}`",
                manifest.task_id,
                root_meta.task_id()
            ),
        ));
    }
    if let Some(v) = root_meta.version {
        if v != manifest.task_version {
            return Err(ApiFailure::new(
                400,
                format!(
                    "manifest.task_version {} does not match task/meta.json version {v}",
                    manifest.task_version
                ),
            ));
        }
    }

    // ---- dependency layout ------------------------------------------------
    let included: BTreeSet<String> = manifest
        .dependencies
        .iter()
        .filter(|d| d.included)
        .map(|d| d.task_id.clone())
        .collect();
    let dep_dirs = list_subdirs(&staging.join("deps"))?;
    let dep_dir_set: BTreeSet<String> = dep_dirs.iter().cloned().collect();
    if dep_dir_set != included {
        let missing: Vec<&String> = included.difference(&dep_dir_set).collect();
        let extra: Vec<&String> = dep_dir_set.difference(&included).collect();
        return Err(ApiFailure::new(
            400,
            format!(
                "package dependency mismatch: missing deps/ {:?}, undeclared deps/ {:?}",
                missing, extra
            ),
        ));
    }

    // ---- install order: dependencies first, root last --------------------
    let mut install_order: Vec<String> = included.iter().cloned().collect();
    install_order.push(manifest.task_id.clone());

    let installed = read_installed(&layout.root).map_err(|e| {
        ApiFailure::new(500, installed_registry_error(&layout.root, &e))
            .with_code("installed_registry_corrupt")
    })?;
    dependency_resolution(manifest, &installed)?;

    // Verify embedded dependency versions against the manifest.
    let mut dep_metas: BTreeMap<String, Meta> = BTreeMap::new();
    let mut expected_versions: BTreeMap<String, u32> = BTreeMap::new();
    for dep in &manifest.dependencies {
        if !dep.included {
            continue;
        }
        let dir = staging.join("deps").join(&dep.task_id);
        let meta = read_meta_at(&dir, &format!("dependency `{}`", dep.task_id))?;
        check_template_version(&meta, &format!("dependency `{}`", dep.task_id))?;
        if let Some(v) = meta.version {
            if v != dep.version {
                return Err(ApiFailure::new(
                    400,
                    format!(
                        "dependency `{}` version mismatch: manifest {}, meta.json {v}",
                        dep.task_id, dep.version
                    ),
                ));
            }
        }
        expected_versions.insert(dep.task_id.clone(), dep.version);
        dep_metas.insert(dep.task_id.clone(), meta);
    }

    // ---- build the commit tree under one tasks-dir root ------------------
    let tree = staging.join("tree");
    std::fs::create_dir_all(&tree)
        .map_err(|e| ApiFailure::new(500, format!("create {}: {e}", tree.display())))?;
    move_dir(&task_dir, &tree.join(&manifest.task_id))
        .map_err(|e| ApiFailure::new(500, format!("stage task: {e}")))?;
    for dep in &included {
        move_dir(
            &staging.join("deps").join(dep),
            &tree.join(dep),
        )
        .map_err(|e| ApiFailure::new(500, format!("stage dependency `{dep}`: {e}")))?;
    }

    // ---- flow_ops gate (recomputed, not just trusted) --------------------
    let flow_paths: Vec<PathBuf> = install_order
        .iter()
        .map(|id| tree.join(id).join("flow.json"))
        .collect();
    let recomputed = oas_task::package::derive_flow_ops_for_files(&flow_paths)
        .map_err(|e| ApiFailure::new(400, format!("recompute flow_ops: {e}")))?;
    let declared: Vec<String> = manifest.engine.flow_ops.clone();
    if recomputed != declared {
        return Err(ApiFailure::new(
            400,
            format!(
                "manifest.engine.flow_ops does not match the packaged flows: declared {:?}, \
                 rewritten {:?}",
                declared, recomputed
            ),
        ));
    }
    let unsupported = unsupported_flow_ops(&recomputed);
    if !unsupported.is_empty() {
        return Err(ApiFailure::new(
            400,
            format!(
                "unsupported flow_ops: {}; upgrade the Runner (supported: {})",
                unsupported.join(", "),
                SUPPORTED_FLOW_OPS.join(", ")
            ),
        )
        .with_code("unsupported_flow_ops"));
    }

    // ---- lint against a temporary tasks-dir ------------------------------
    // T7 needs every call target readable. Included deps are in `tree`; a
    // non-included dependency is already installed, so a meta/flow-only copy is
    // added to the lint dir (never committed).
    let lint_dir = staging.join("lint_tasks");
    std::fs::create_dir_all(&lint_dir)
        .map_err(|e| ApiFailure::new(500, format!("create {}: {e}", lint_dir.display())))?;
    for id in &install_order {
        copy_meta_flow(&tree.join(id), &lint_dir.join(id))
            .map_err(|e| ApiFailure::new(500, format!("prepare lint `{id}`: {e}")))?;
    }
    for dep in manifest.dependencies.iter().filter(|d| !d.included) {
        let source = tasks_dir.join(&dep.task_id);
        if !source.is_dir() {
            return Err(ApiFailure::new(
                400,
                format!(
                    "dependency `{}` is declared as installed but `{}` is missing",
                    dep.task_id,
                    source.display()
                ),
            ));
        }
        copy_meta_flow(&source, &lint_dir.join(&dep.task_id))
            .map_err(|e| ApiFailure::new(500, format!("prepare lint dependency: {e}")))?;
    }
    let (external_assets, global_assets) = install_lint_assets(staging, assets_dir)?;
    let lint_errors = collect_lint_issues(
        &tree,
        &lint_dir,
        &install_order,
        external_assets.as_ref(),
        global_assets.as_ref(),
    )?;
    if !lint_errors.is_empty() {
        return Err(ApiFailure::errors(400, lint_errors));
    }

    // ---- dirty_task guard -------------------------------------------------
    let mut dirty = Vec::new();
    for id in &install_order {
        let target = tasks_dir.join(id);
        if !target.exists() {
            continue;
        }
        match installed.find(id) {
            Some(entry) => {
                if let Some(mtime) = mtime_ms(&target.join("meta.json")) {
                    if mtime > entry.installed_at_ms {
                        dirty.push(format!(
                            "{id}: meta.json mtime {mtime} is newer than installed_at {}",
                            entry.installed_at_ms
                        ));
                    }
                }
            }
            None => dirty.push(format!(
                "{id}: existing task directory is unmanaged (no installed.json entry)"
            )),
        }
    }
    if !dirty.is_empty() && !opts.force {
        let mut errors = vec!["dirty_task".to_string()];
        errors.extend(dirty);
        return Err(ApiFailure {
            status: 409,
            code: Some("dirty_task".to_string()),
            errors,
        });
    }

    // ---- same-version reinstall guard (design §A.7) -----------------------
    // Replacing an installed task with the *same* version must be explicit; a
    // version bump (upgrade or downgrade) is the normal update path and needs no
    // force (it is still backed up). A missing target directory is a repair, not
    // a replacement, so it is allowed without force.
    let mut same_version = Vec::new();
    if !opts.force {
        for id in &install_order {
            if !tasks_dir.join(id).exists() {
                continue;
            }
            let Some(existing) = installed.find(id) else {
                continue; // unmanaged bare dir: the dirty_task guard owns this case
            };
            let new_version = if id == &manifest.task_id {
                manifest.task_version
            } else {
                expected_versions.get(id).copied().unwrap_or(0)
            };
            if existing.version == new_version {
                same_version.push(format!(
                    "task `{id}` v{new_version} already installed; pass force:true to replace"
                ));
            }
        }
    }
    if !same_version.is_empty() {
        return Err(ApiFailure {
            status: 409,
            code: Some("already_installed".to_string()),
            errors: same_version,
        });
    }

    // ---- asset merge plan -------------------------------------------------
    let asset_merge = plan_asset_merge(staging, assets_dir, &manifest.asset_refs, warnings)?;

    // ---- commit task directories -----------------------------------------
    let backup_root = layout.backup_dir();
    std::fs::create_dir_all(&backup_root).map_err(|e| {
        ApiFailure::new(500, format!("create {}: {e}", backup_root.display()))
    })?;
    let mut committed: Vec<CommittedTask> = Vec::new();
    let mut root_backup: Option<String> = None;
    for id in &install_order {
        let target = tasks_dir.join(id);
        let old_entry = installed.find(id).cloned();
        let old_version = old_entry
            .as_ref()
            .map(|e| e.version)
            .or_else(|| expected_versions.get(id).copied())
            .unwrap_or(0);
        let backup_dir = if target.exists() {
            let name = format!("{id}-{old_version}-{now}");
            let dir = unique_child(&backup_root, &name);
            std::fs::create_dir_all(&dir)
                .map_err(|e| ApiFailure::new(500, format!("create {}: {e}", dir.display())))?;
            move_dir(&target, &dir.join("task"))
                .map_err(|e| ApiFailure::new(500, format!("backup `{id}`: {e}")))?;
            if let Err(e) = snapshot_entry(&dir, old_entry.as_ref()) {
                let _ = move_dir(&dir.join("task"), &target);
                rollback_tasks(&committed);
                return Err(ApiFailure::new(500, format!("write backup entry: {e}")));
            }
            Some(dir)
        } else {
            None
        };
        if let Err(e) = move_dir(&tree.join(id), &target) {
            if let Some(dir) = &backup_dir {
                let _ = move_dir(&dir.join("task"), &target);
            }
            rollback_tasks(&committed);
            return Err(ApiFailure::new(500, format!("commit `{id}`: {e}")));
        }
        if id == &manifest.task_id {
            root_backup = backup_dir
                .as_ref()
                .map(|d| relative_to(&layout.root, d));
        }
        committed.push(CommittedTask {
            task_id: id.clone(),
            target,
            backup_dir,
        });
    }

    // ---- apply asset merge (rollback tasks on failure) -------------------
    if let Some(merge) = &asset_merge {
        if let Err(e) = apply_asset_merge(merge) {
            rollback_asset_merge(merge);
            rollback_tasks(&committed);
            return Err(ApiFailure::new(500, format!("merge assets: {e}")));
        }
    }

    // ---- installed.json ---------------------------------------------------
    // The registry timestamp must be **later** than the mtimes of the files we
    // just moved into tasks/; otherwise the next install would immediately see
    // its own fresh files as a user edit (dirty_task) on coarse-clock systems.
    let installed_at = now_ms().max(now);
    let mut next_installed = installed.clone();
    for (idx, id) in install_order.iter().enumerate() {
        let version = if id == &manifest.task_id {
            manifest.task_version
        } else {
            expected_versions.get(id).copied().unwrap_or(1)
        };
        let backup = committed
            .iter()
            .find(|c| &c.task_id == id)
            .and_then(|c| c.backup_dir.as_ref())
            .map(|d| relative_to(&layout.root, d));
        next_installed.upsert(InstalledPackage {
            package_id: manifest.package_id.clone(),
            task_id: id.clone(),
            version,
            sha256: archive_sha.to_string(),
            installed_at_ms: installed_at,
            source: InstalledSource {
                repo: manifest.source.repo.clone(),
                commit: manifest.source.commit.clone(),
                signed: false,
            },
            backup,
            dependencies: if idx == install_order.len() - 1 {
                manifest.dependencies.iter().map(|d| d.task_id.clone()).collect()
            } else {
                Vec::new()
            },
        });
    }
    if let Err(e) = write_installed(&layout.root, &next_installed) {
        if let Some(merge) = &asset_merge {
            rollback_asset_merge(merge);
        }
        rollback_tasks(&committed);
        return Err(ApiFailure::new(500, format!("write installed.json: {e}")));
    }

    Ok(InstallOutcome {
        package_id: manifest.package_id.clone(),
        task_id: manifest.task_id.clone(),
        version: manifest.task_version,
        replaced: root_backup.is_some(),
        backup: root_backup,
        warnings: Vec::new(),
        installed_tasks: install_order,
    })
}

/// Install order and rollback are documented above; the installed registry
/// records the archive hash passed down from `install_from_path`.
// ---------------------------------------------------------------------------
// Uninstall / activate
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct UninstallGuards {
    /// Scheduler entry/task ids (or queue ids) that reference the task.
    pub scheduler: Vec<String>,
    /// An in-process/scheduled run is currently executing this task.
    pub running: bool,
    /// Installed packages that declare this task as a dependency.
    pub dependents: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UninstallOutcome {
    pub task_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backup: Option<String>,
    pub removed_package: bool,
    pub config_purged: bool,
}

pub fn uninstall(
    layout: &PackagesLayout,
    tasks_dir: &Path,
    task_id: &str,
    purge_config: bool,
    force: bool,
    guards: &UninstallGuards,
    values_path: &Path,
    now: u64,
) -> Result<UninstallOutcome, ApiFailure> {
    validate_task_name(task_id).map_err(|e| ApiFailure::new(400, e))?;
    let mut installed = read_installed(&layout.root).map_err(|e| {
        ApiFailure::new(500, installed_registry_error(&layout.root, &e))
            .with_code("installed_registry_corrupt")
    })?;
    let task_dir = tasks_dir.join(task_id);
    let entry = installed.find(task_id).cloned();
    if entry.is_none() && !task_dir.is_dir() {
        return Err(ApiFailure::new(
            404,
            format!("task `{task_id}` is not installed and has no bare directory"),
        ));
    }
    if !force {
        let mut reasons = Vec::new();
        if !guards.scheduler.is_empty() {
            reasons.push(format!(
                "task `{task_id}` is referenced by scheduler entry/queue `{}`; remove it first",
                guards.scheduler.join("`, `")
            ));
        }
        if guards.running {
            reasons.push(format!("task `{task_id}` is currently running"));
        }
        if !guards.dependents.is_empty() {
            reasons.push(format!(
                "task `{task_id}` is a dependency of installed package(s) `{}`; uninstall them \
                 first or pass force:true",
                guards.dependents.join("`, `")
            ));
        }
        if !reasons.is_empty() {
            return Err(ApiFailure {
                status: 409,
                code: Some("uninstall_refused".to_string()),
                errors: reasons,
            });
        }
    }

    // Purge config first: it has no effect on the task directory, and a corrupt
    // values store must fail before anything is moved.
    let mut config_purged = false;
    if purge_config {
        match runner_config::read(values_path) {
            Ok(mut file) => {
                if file.tasks.remove(task_id).is_some() {
                    file.updated_ms = now;
                    runner_config::write(values_path, &file)
                        .map_err(|e| ApiFailure::new(500, format!("purge config: {e}")))?;
                    config_purged = true;
                }
            }
            Err(e) => {
                return Err(ApiFailure::new(
                    500,
                    format!("purge config: {e} (task was not moved)"),
                ))
            }
        }
    }

    let mut backup_rel = None;
    let mut backup_abs = None;
    if task_dir.is_dir() {
        let dir = unique_child(
            &layout.backup_dir(),
            &format!("uninstall-{task_id}-{now}"),
        );
        std::fs::create_dir_all(&dir)
            .map_err(|e| ApiFailure::new(500, format!("create {}: {e}", dir.display())))?;
        move_dir(&task_dir, &dir.join("task"))
            .map_err(|e| ApiFailure::new(500, format!("backup task `{task_id}`: {e}")))?;
        snapshot_entry(&dir, entry.as_ref())
            .map_err(|e| ApiFailure::new(500, format!("write backup entry: {e}")))?;
        backup_rel = Some(relative_to(&layout.root, &dir));
        backup_abs = Some(dir);
    }

    let removed = installed.remove_task(task_id).is_some();
    if removed {
        if let Err(e) = write_installed(&layout.root, &installed) {
            if let Some(dir) = &backup_abs {
                let _ = move_dir(&dir.join("task"), &task_dir);
            }
            return Err(ApiFailure::new(500, format!("write installed.json: {e}")));
        }
    }
    Ok(UninstallOutcome {
        task_id: task_id.to_string(),
        backup: backup_rel,
        removed_package: removed,
        config_purged,
    })
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ActivateRequest {
    #[serde(default)]
    pub task_id: Option<String>,
    #[serde(default)]
    pub package_id: Option<String>,
    #[serde(default)]
    pub backup: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActivateOutcome {
    pub task_id: String,
    pub version: u32,
    pub package_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restored_from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backup: Option<String>,
}

/// One backup directory opened for rollback/restore.
struct BackupSource {
    /// Task bundle directory (`<dir>/task`, or `<dir>` for the legacy layout).
    source: PathBuf,
    /// Path relative to the packages root (`backup/<name>`), echoed to the API.
    rel: String,
    meta: Meta,
    snapshot: Option<InstalledPackage>,
}

/// Open `<packages>/backup/<name>`: resolve the task dir and read its metadata.
fn open_backup(dir: &Path, rel: &str) -> Result<BackupSource, String> {
    if !dir.is_dir() {
        return Err(format!("backup `{rel}` does not exist"));
    }
    let source = if dir.join("task").is_dir() {
        dir.join("task")
    } else if dir.join("meta.json").is_file() {
        // Legacy/direct backup layout: the backup directory is the task dir.
        dir.to_path_buf()
    } else {
        return Err(format!(
            "backup `{rel}` contains neither `task/` nor `meta.json`"
        ));
    };
    let meta_path = source.join("meta.json");
    let text = std::fs::read_to_string(&meta_path)
        .map_err(|e| format!("read {}: {e}", meta_path.display()))?;
    let meta = Meta::from_json(&text).map_err(|e| format!("backup `{rel}` meta.json: {e}"))?;
    let snapshot = std::fs::read_to_string(dir.join(ENTRY_SNAPSHOT))
        .ok()
        .and_then(|text| serde_json::from_str::<InstalledPackage>(&text).ok());
    Ok(BackupSource {
        source,
        rel: rel.to_string(),
        meta,
        snapshot,
    })
}

/// A backup may only roll back the task it actually contains. Both the
/// `entry.json` snapshot and `task/meta.json` are checked when present.
fn verify_backup_owner(backup: &BackupSource, task_id: &str) -> Result<(), String> {
    let owner = backup.meta.task_id();
    if owner != task_id {
        return Err(format!(
            "backup `{}` belongs to task `{owner}`, not `{task_id}`",
            backup.rel
        ));
    }
    if let Some(snapshot) = &backup.snapshot {
        if !snapshot.task_id.is_empty() && snapshot.task_id != task_id {
            return Err(format!(
                "backup `{}` entry.json belongs to task `{}`, not `{task_id}`",
                backup.rel, snapshot.task_id
            ));
        }
    }
    Ok(())
}

/// Timestamp suffix of a backup directory name: `<task>-<version>-<ts>` for an
/// install-time backup or `uninstall-<task>-<ts>` for an uninstall backup.
/// Unrelated directories (and control dirs) yield `None`.
fn backup_timestamp(name: &str) -> Option<u64> {
    let (rest, ts) = name.rsplit_once('-')?;
    let ts = ts.parse::<u64>().ok()?;
    if let Some(task) = rest.strip_prefix("uninstall-") {
        return (!task.is_empty()).then_some(ts);
    }
    let (task, version) = rest.rsplit_once('-')?;
    if task.is_empty() || version.parse::<u32>().is_err() {
        return None;
    }
    Some(ts)
}

/// Newest backup under `backup/` that verifiably belongs to `task_id`.
fn find_latest_backup(layout: &PackagesLayout, task_id: &str) -> Option<BackupSource> {
    let entries = std::fs::read_dir(layout.backup_dir()).ok()?;
    let mut best: Option<(u64, String, BackupSource)> = None;
    for entry in entries.flatten() {
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(ts) = backup_timestamp(&name) else {
            continue;
        };
        let dir = entry.path();
        let rel = relative_to(&layout.root, &dir);
        let Ok(candidate) = open_backup(&dir, &rel) else {
            continue;
        };
        if verify_backup_owner(&candidate, task_id).is_err() {
            continue;
        }
        let newer = match &best {
            Some((best_ts, best_name, _)) => (ts, &name) > (*best_ts, best_name),
            None => true,
        };
        if newer {
            best = Some((ts, name, candidate));
        }
    }
    best.map(|(_, _, backup)| backup)
}

/// `activate` for a task that has **no** installed.json entry (typically right
/// after `uninstall`, which only moves the task into `backup/`). Restores the
/// requested backup, or the newest backup that belongs to this task, and writes
/// a fresh registry entry from its metadata (`package_id` may be empty for a
/// legacy snapshot, `source` stays local/unsigned).
fn restore_uninstalled(
    layout: &PackagesLayout,
    tasks_dir: &Path,
    task_id: &str,
    requested: Option<&str>,
    installed: &mut InstalledFile,
    now: u64,
) -> Result<ActivateOutcome, ApiFailure> {
    let target = tasks_dir.join(task_id);
    if target.exists() {
        return Err(ApiFailure::new(
            409,
            format!(
                "task `{task_id}` has no installed.json entry, but `{}` already exists; remove \
                 that directory or install a package before restoring a backup",
                target.display()
            ),
        )
        .with_code("unmanaged_dir"));
    }
    let backup = match requested {
        Some(rel) => {
            let abs = validate_backup_path(layout, rel).map_err(|e| ApiFailure::new(400, e))?;
            let opened = open_backup(&abs, rel).map_err(|e| ApiFailure::new(404, e))?;
            verify_backup_owner(&opened, task_id).map_err(|e| ApiFailure::new(400, e))?;
            opened
        }
        None => find_latest_backup(layout, task_id).ok_or_else(|| {
            ApiFailure::new(
                404,
                format!(
                    "task `{task_id}` is not installed and has no backup in {}; install the \
                     package again",
                    layout.backup_dir().display()
                ),
            )
        })?,
    };
    let version = backup
        .meta
        .version
        .or_else(|| backup.snapshot.as_ref().map(|s| s.version))
        .unwrap_or(1);
    if let Err(e) = move_dir(&backup.source, &target) {
        return Err(ApiFailure::new(
            500,
            format!("restore backup `{}`: {e}", backup.rel),
        ));
    }
    // A legacy backup-directory layout can carry control files; they are not
    // part of the task bundle and must not leak into tasks/.
    let _ = std::fs::remove_file(target.join(ENTRY_SNAPSHOT));
    let mut entry = backup.snapshot.clone().unwrap_or_else(|| InstalledPackage {
        package_id: String::new(),
        task_id: task_id.to_string(),
        version,
        sha256: String::new(),
        installed_at_ms: 0,
        source: InstalledSource::default(),
        backup: None,
        dependencies: Vec::new(),
    });
    entry.task_id = task_id.to_string();
    entry.version = version;
    entry.installed_at_ms = now_ms().max(now);
    entry.backup = None;
    let package_id = entry.package_id.clone();
    installed.upsert(entry);
    if let Err(e) = write_installed(&layout.root, installed) {
        let _ = move_dir(&target, &backup.source);
        return Err(ApiFailure::new(
            500,
            format!("write installed.json: {e}"),
        ));
    }
    Ok(ActivateOutcome {
        task_id: task_id.to_string(),
        version,
        package_id,
        restored_from: Some(backup.rel),
        backup: None,
    })
}

pub fn activate(
    layout: &PackagesLayout,
    tasks_dir: &Path,
    req: &ActivateRequest,
    now: u64,
) -> Result<ActivateOutcome, ApiFailure> {
    let mut installed = read_installed(&layout.root).map_err(|e| {
        ApiFailure::new(500, installed_registry_error(&layout.root, &e))
            .with_code("installed_registry_corrupt")
    })?;
    let task_id = match req.task_id.as_deref() {
        Some(id) => {
            validate_task_name(id).map_err(|e| ApiFailure::new(400, e))?;
            id.to_string()
        }
        None => match req.package_id.as_deref() {
            Some(package_id) => {
                let matches: Vec<&InstalledPackage> = installed
                    .packages
                    .iter()
                    .filter(|p| p.package_id == package_id)
                    .collect();
                match matches.as_slice() {
                    [entry] => entry.task_id.clone(),
                    [] => {
                        return Err(ApiFailure::new(
                            404,
                            format!("no installed task with package_id `{package_id}`"),
                        ))
                    }
                    _ => {
                        return Err(ApiFailure::new(
                            400,
                            format!(
                                "package_id `{package_id}` matches {} installed tasks; pass \
                                 task_id explicitly",
                                matches.len()
                            ),
                        ))
                    }
                }
            }
            None => {
                return Err(ApiFailure::new(
                    400,
                    "either `task_id` or `package_id` is required",
                ))
            }
        },
    };
    let Some(entry) = installed.find(&task_id).cloned() else {
        return restore_uninstalled(
            layout,
            tasks_dir,
            &task_id,
            req.backup.as_deref(),
            &mut installed,
            now,
        );
    };
    let rel = match req.backup.clone().or_else(|| entry.backup.clone()) {
        Some(rel) => rel,
        None => {
            return Err(ApiFailure::new(
                404,
                format!("task `{task_id}` has no recorded backup to roll back to"),
            ))
        }
    };
    let backup_abs =
        validate_backup_path(layout, &rel).map_err(|e| ApiFailure::new(400, e))?;
    let restored = open_backup(&backup_abs, &rel).map_err(|e| ApiFailure::new(404, e))?;
    verify_backup_owner(&restored, &task_id).map_err(|e| ApiFailure::new(400, e))?;
    let restored_meta = restored.meta.clone();
    let source = restored.source.clone();
    let snapshot = restored.snapshot.clone();

    let target = tasks_dir.join(&task_id);
    let current_version = entry.version;
    let new_backup = if target.is_dir() {
        let dir = unique_child(
            &layout.backup_dir(),
            &format!("{task_id}-{current_version}-{now}"),
        );
        std::fs::create_dir_all(&dir)
            .map_err(|e| ApiFailure::new(500, format!("create {}: {e}", dir.display())))?;
        move_dir(&target, &dir.join("task"))
            .map_err(|e| ApiFailure::new(500, format!("backup current task: {e}")))?;
        snapshot_entry(&dir, Some(&entry))
            .map_err(|e| ApiFailure::new(500, format!("write backup entry: {e}")))?;
        Some(dir)
    } else {
        None
    };

    if let Err(e) = move_dir(&source, &target) {
        if let Some(dir) = &new_backup {
            let _ = move_dir(&dir.join("task"), &target);
        }
        return Err(ApiFailure::new(500, format!("restore backup `{rel}`: {e}")));
    }
    // A legacy backup-directory layout can carry control files; they are not
    // part of the task bundle and must not leak into tasks/.
    let _ = std::fs::remove_file(target.join(ENTRY_SNAPSHOT));

    let restored_version = snapshot
        .as_ref()
        .map(|s| s.version)
        .or(restored_meta.version)
        .unwrap_or(entry.version);
    let restored_package_id = snapshot
        .as_ref()
        .map(|s| s.package_id.clone())
        .unwrap_or_default();
    let mut next_entry = snapshot.unwrap_or_else(|| entry.clone());
    next_entry.task_id = task_id.clone();
    next_entry.version = restored_version;
    next_entry.package_id = restored_package_id.clone();
    next_entry.sha256 = if next_entry.sha256.is_empty() {
        entry.sha256.clone()
    } else {
        next_entry.sha256
    };
    next_entry.installed_at_ms = now_ms().max(now);
    next_entry.backup = new_backup
        .as_ref()
        .map(|d| relative_to(&layout.root, d));
    let old_entry = installed.remove_task(&task_id);
    installed.upsert(next_entry);
    if let Err(e) = write_installed(&layout.root, &installed) {
        // Restore the previous registry entry if the write failed before rename
        // (atomic write: the old file is still intact), then undo the dir swap.
        if let Some(old) = old_entry {
            installed.upsert(old);
        }
        let _ = move_dir(&target, &source);
        if let Some(dir) = &new_backup {
            let _ = move_dir(&dir.join("task"), &target);
        }
        return Err(ApiFailure::new(500, format!("write installed.json: {e}")));
    }
    Ok(ActivateOutcome {
        task_id,
        version: restored_version,
        package_id: restored_package_id,
        restored_from: Some(rel),
        backup: new_backup
            .as_ref()
            .map(|d| relative_to(&layout.root, d)),
    })
}

/// Convenience for `GET /runner/packages`: installed rows + bare directories.
pub fn list_packages(
    layout: &PackagesLayout,
    tasks_dir: &Path,
) -> Result<(Vec<InstalledPackage>, Vec<serde_json::Value>), String> {
    let installed = read_installed(&layout.root)?;
    let names = oas_task::list_tasks(tasks_dir)?;
    let managed: BTreeSet<&str> = installed
        .packages
        .iter()
        .map(|p| p.task_id.as_str())
        .collect();
    let mut unmanaged = Vec::new();
    for task in names {
        if !managed.contains(task.as_str()) {
            let dir = tasks_dir.join(&task);
            let version = std::fs::read_to_string(dir.join("meta.json"))
                .ok()
                .and_then(|text| Meta::from_json(&text).ok())
                .and_then(|m| m.version);
            unmanaged.push(serde_json::json!({
                "task": task,
                "version": version,
                "unmanaged": true,
            }));
        }
    }
    Ok((installed.packages, unmanaged))
}

// ---------------------------------------------------------------------------
// Shared helpers used by serve.rs
// ---------------------------------------------------------------------------

/// Publish through the Studio HTTP route: parse the request JSON and map every
/// failure to an HTTP status.
pub fn publish_http(
    tasks_dir: &Path,
    assets_dir: &Path,
    default_out: &Path,
    body: &[u8],
) -> Result<BuildOutcome, ApiFailure> {
    let value: serde_json::Value = serde_json::from_slice(body)
        .map_err(|e| ApiFailure::new(400, format!("invalid JSON: {e}")))?;
    let task = value
        .get("task")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if task.is_empty() {
        return Err(ApiFailure::new(400, "`task` is required"));
    }
    let out = value
        .get("out")
        .and_then(|v| v.as_str())
        .map(PathBuf::from);
    if let Some(out) = &out {
        if out.as_os_str().is_empty() {
            return Err(ApiFailure::new(400, "`out` must not be empty"));
        }
    }
    let req = PublishRequest {
        task,
        out,
        force: value.get("force").and_then(|v| v.as_bool()).unwrap_or(false),
        include_deps: value
            .get("include_deps")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        reproducible: value
            .get("reproducible")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
    };
    publish(tasks_dir, assets_dir, default_out, &req).map_err(publish_failure)
}

/// Is this a valid, harmless package file name? Used by the Studio listing.
pub fn is_package_file_name(name: &str) -> bool {
    name.ends_with(&format!(".{PACKAGE_EXTENSION}")) && !name.starts_with('.')
}

/// Honest Phase-1 limits surfaced by the API/UI.
pub fn phase_note() -> &'static str {
    "Phase 1: install accepts a local absolute .oaspkg path; upload/streaming is deferred to \
     Phase 5 (4 MiB JSON body limit still applies to the request)"
}

/// Build an install request from JSON: `{path, force?}` only (Phase 1).
pub fn parse_install_request(body: &[u8]) -> Result<(PathBuf, InstallOptions), ApiFailure> {
    let value: serde_json::Value = serde_json::from_slice(body)
        .map_err(|e| ApiFailure::new(400, format!("invalid JSON: {e}")))?;
    if value.get("upload").and_then(|v| v.as_bool()) == Some(true) {
        return Err(ApiFailure::new(
            400,
            "upload/streaming mode is not implemented in Phase 1; pass an absolute local `path`",
        ));
    }
    let path = value
        .get("path")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .ok_or_else(|| ApiFailure::new(400, "`path` is required (Phase 1 path mode)"))?;
    Ok((
        PathBuf::from(path),
        InstallOptions {
            force: value.get("force").and_then(|v| v.as_bool()).unwrap_or(false),
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publish_navigate_no_longer_requires_a_local_map() {
        // seq137 #3: map is a local runtime resource. Publish must succeed even
        // when the authoring machine has no map.json / no target node.
        let root = test_dir("map-req");
        let tasks = root.join("tasks");
        let assets = root.join("assets");
        let created = crate::task::create_task(&tasks, "utility", "导航任务")
            .expect("create task");
        std::fs::write(
            created.path.join("flow.json"),
            r#"{"schema_version":1,"name":"导航任务","steps":[
                 {"op":"navigate","args":{"target":"庭院"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        )
        .expect("flow");
        std::fs::create_dir_all(&assets).expect("assets");
        std::fs::write(
            assets.join("assets.json"),
            r#"{"schema_version":1,"design_resolution":[1280,720],"items":[],"collections":[]}"#,
        )
        .expect("assets.json");
        // NOTE: no assets/map.json at all.
        let dist = root.join("dist");
        let req = PublishRequest {
            task: "导航任务".into(),
            out: None,
            force: true,
            include_deps: false,
            reproducible: true,
        };
        let outcome = publish(&tasks, &assets, &dist, &req).expect("publish without map");
        assert!(
            outcome
                .manifest
                .engine
                .flow_ops
                .contains(&"navigate".to_string()),
            "{:?}",
            outcome.manifest.engine.flow_ops
        );
        assert!(
            outcome.manifest.map_requirements.is_empty(),
            "map_requirements must not be populated from the local map: {:?}",
            outcome.manifest.map_requirements
        );
        // Install into a runner WITHOUT map.json → success, no map warning.
        let assets2 = root.join("assets2");
        std::fs::create_dir_all(&assets2).expect("assets2");
        std::fs::write(
            assets2.join("assets.json"),
            r#"{"schema_version":1,"design_resolution":[1280,720],"items":[],"collections":[]}"#,
        )
        .expect("assets2 json");
        let layout = PackagesLayout::new(root.join("packages"));
        let installed = install_from_path(
            &layout,
            &tasks,
            &assets2,
            &outcome.path,
            &InstallOptions { force: true },
            1_700_000_000_000,
        )
        .expect("install without map");
        assert!(
            !installed
                .warnings
                .iter()
                .any(|w| w.contains("map node") || w.contains("map.json")),
            "no map hard/warning gate expected: {:?}",
            installed.warnings
        );
    }

    #[test]
    fn packages_root_is_a_sibling_of_tasks_dir() {
        assert_eq!(
            default_packages_root(Path::new("tasks")),
            PathBuf::from("packages")
        );
        assert_eq!(
            default_packages_root(Path::new("D:/pkg/tasks")),
            PathBuf::from("D:/pkg/packages")
        );
        assert_eq!(
            default_packages_root(Path::new("D:/pkg/data/tasks")),
            PathBuf::from("D:/pkg/data/packages"),
        );
    }

    // ---- seq129-fix: install-time lint must see the merged asset universe ----

    const GLOBAL_OCR_TASK: &str = "全局识别任务";
    const GLOBAL_OCR_ASSET: &str = "预设识别";

    fn test_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "oas-package-cmd-{tag}-{}-{}",
            std::process::id(),
            short_seq()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Staged task whose flow drags an **asset endpoint** bound to a global
    /// `ocr_var` and whose `meta.variables` mirrors that binding.
    fn write_global_ocr_task(tree: &Path) {
        let dir = tree.join(GLOBAL_OCR_TASK);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("meta.json"),
            format!(
                r#"{{"schema_version":1,"name":"{GLOBAL_OCR_TASK}","variables":[
                     {{"key":"p","type":"string","value_source":"ocr_text",
                       "asset_id":"{GLOBAL_OCR_ASSET}","default":"甲",
                       "runner_editable":true}}],
                     "exits":{{"done":{{"result":"success"}},"failed":{{"result":"retryable"}}}}}}"#
            ),
        )
        .unwrap();
        std::fs::write(
            dir.join("flow.json"),
            format!(
                r#"{{"schema_version":1,"name":"{GLOBAL_OCR_TASK}","steps":[
                     {{"op":"drag","args":{{
                       "from":{{"kind":"asset","sel":"{GLOBAL_OCR_ASSET}"}},
                       "direction":"up","distance":[10,20]}}}},
                     {{"op":"finish","args":{{"exit":"done"}}}}]}}"#
            ),
        )
        .unwrap();
    }

    /// Write one `assets.json` (package snapshot or global store) with the
    /// `ocr_var` declaration and its runtime text.
    fn write_ocr_var_store(dir: &Path, runtime_text: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(
            dir.join("assets.json"),
            format!(
                r#"{{"schema_version":1,"items":[
                     {{"id":"{GLOBAL_OCR_ASSET}","kind":"ocr_var",
                       "runtime_text":"{runtime_text}","param_key":"p"}}]}}"#
            ),
        )
        .unwrap();
    }

    fn install_lint_errors(
        staging: &Path,
        tree: &Path,
        assets_dir: &Path,
    ) -> Vec<String> {
        let (ids, merged) = install_lint_assets(staging, assets_dir).expect("install lint assets");
        collect_lint_issues(
            tree,
            tree,
            &[GLOBAL_OCR_TASK.to_string()],
            ids.as_ref(),
            merged.as_ref(),
        )
        .expect("install lint")
    }

    #[test]
    fn install_lint_sees_the_package_asset_snapshot_for_t8() {
        // Global-asset-mode package: the task's own assets.json is empty, the
        // referenced ocr_var ships in the package snapshot. Install lint must be
        // green (previously a false T8 "not a declared asset").
        let root = test_dir("install-lint-pkg-snapshot");
        let tree = root.join("tasks");
        let staging = root.join("staging");
        write_global_ocr_task(&tree);
        write_ocr_var_store(&staging.join("assets"), "甲");
        let global = root.join("global-assets"); // not initialized: package only
        let errors = install_lint_errors(&staging, &tree, &global);
        assert!(
            errors.is_empty(),
            "package asset snapshot must satisfy T8: {errors:?}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn install_lint_without_the_snapshot_still_reports_t8() {
        // No global store and no package snapshot: keep the historical strict
        // semantics (a binding to an undeclared asset must still fail).
        let root = test_dir("install-lint-no-assets");
        let tree = root.join("tasks");
        let staging = root.join("staging");
        write_global_ocr_task(&tree);
        let (ids, merged) = install_lint_assets(&staging, &root.join("global-assets"))
            .expect("install lint assets");
        assert!(ids.is_none() && merged.is_none(), "legacy task-local-only mode");
        let errors = install_lint_errors(&staging, &tree, &root.join("global-assets"));
        assert!(
            errors.iter().any(|e| e.contains("T8")),
            "without any asset declarations T8 must still fail: {errors:?}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn install_lint_merged_view_prefers_the_package_snapshot() {
        // Same id in the current store (old text) and in the package (new text):
        // the eventual store merge installs the packaged definition, so the lint
        // view must use it too (package wins), otherwise T8 would flag the task.
        let root = test_dir("install-lint-pkg-wins");
        let tree = root.join("tasks");
        let staging = root.join("staging");
        write_global_ocr_task(&tree);
        write_ocr_var_store(&staging.join("assets"), "甲");
        let global = root.join("global-assets");
        write_ocr_var_store(&global, "乙");
        let errors = install_lint_errors(&staging, &tree, &global);
        assert!(
            errors.is_empty(),
            "the package snapshot must win over the current store: {errors:?}"
        );
        // Prove the choice matters: the store-only view reports T8 drift.
        let global_file = oas_task::package::global_assets_file(&global)
            .expect("read global")
            .expect("global file");
        let global_ids: BTreeSet<String> = global_file.items.iter().map(|a| a.id.clone()).collect();
        let tree_only = collect_lint_issues(
            &tree,
            &tree,
            &[GLOBAL_OCR_TASK.to_string()],
            Some(&global_ids),
            Some(&global_file),
        )
        .expect("store-only lint");
        assert!(
            tree_only.iter().any(|e| e.contains("T8")),
            "store-only view must flag the drift (the package snapshot is what wins): {tree_only:?}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn installed_registry_roundtrip_and_upsert() {
        let dir = std::env::temp_dir().join(format!(
            "oas-package-cmd-registry-{}-{}",
            std::process::id(),
            short_seq()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let mut file = InstalledFile::default();
        file.upsert(InstalledPackage {
            package_id: "pkg-A-1-x".into(),
            task_id: "A".into(),
            version: 1,
            sha256: "abc".into(),
            installed_at_ms: 10,
            source: InstalledSource::default(),
            backup: None,
            dependencies: vec!["B".into()],
        });
        file.upsert(InstalledPackage {
            package_id: "pkg-B-2-y".into(),
            task_id: "B".into(),
            version: 2,
            sha256: "def".into(),
            installed_at_ms: 11,
            source: InstalledSource::default(),
            backup: None,
            dependencies: Vec::new(),
        });
        write_installed(&dir, &file).unwrap();
        let read = read_installed(&dir).unwrap();
        assert_eq!(read.packages.len(), 2);
        assert_eq!(read.find("A").unwrap().dependencies, vec!["B".to_string()]);
        let mut updated = read.clone();
        updated.upsert(InstalledPackage {
            version: 3,
            ..read.find("A").unwrap().clone()
        });
        write_installed(&dir, &updated).unwrap();
        assert_eq!(read_installed(&dir).unwrap().find("A").unwrap().version, 3);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn relative_backup_paths_cannot_escape_the_packages_root() {
        let root = Path::new("packages");
        assert!(validate_relative_inside(root, "backup/A-1-2").is_ok());
        for bad in ["../evil", "/abs", "backup/../../evil", ""] {
            assert!(
                validate_relative_inside(root, bad).is_err(),
                "must reject {bad}"
            );
        }
        // Activate additionally requires the `backup/` prefix: staging and
        // sibling control directories are not rollback sources.
        let layout = PackagesLayout::new(root);
        assert!(validate_backup_path(&layout, "backup/A-1-2").is_ok());
        for bad in ["staging/pkg-1", "A-1-2", "incoming/x", "backup/../staging/x"] {
            assert!(
                validate_backup_path(&layout, bad).is_err(),
                "must reject {bad}"
            );
        }
    }

    #[test]
    fn default_assets_dir_follows_the_tasks_dir() {
        assert_eq!(default_assets_dir(Path::new("tasks")), PathBuf::from("assets"));
        assert_eq!(
            default_assets_dir(Path::new("D:/pkg/tasks")),
            PathBuf::from("D:/pkg/assets")
        );
        assert_eq!(
            default_assets_dir(Path::new("D:/pkg/data/tasks")),
            PathBuf::from("D:/pkg/data/assets")
        );
    }

    #[test]
    fn publish_out_must_stay_inside_dist() {
        for ok in [
            "dist",
            "dist/packages",
            "dist/packages/x.oaspkg",
            "./dist/x.oaspkg",
        ] {
            assert!(validate_publish_out(Path::new(ok)).is_ok(), "{ok} must be allowed");
        }
        for bad in [
            "",
            "out/x.oaspkg",
            "/abs/dist/x.oaspkg",
            "dist/../evil.oaspkg",
            "../dist/x.oaspkg",
            "dist/../../x",
        ] {
            assert!(
                validate_publish_out(Path::new(bad)).is_err(),
                "{bad} must be refused"
            );
        }
    }

    #[test]
    fn backup_directory_timestamps_are_parsed() {
        assert_eq!(backup_timestamp("A-1-1700000000000"), Some(1_700_000_000_000));
        assert_eq!(
            backup_timestamp("uninstall-A-1700000000001"),
            Some(1_700_000_000_001)
        );
        assert_eq!(
            backup_timestamp("my-task-12-1700000000002"),
            Some(1_700_000_000_002)
        );
        assert_eq!(backup_timestamp("staging"), None);
        assert_eq!(backup_timestamp("A-1"), None);
        assert_eq!(backup_timestamp("uninstall--1700000000003"), None);
    }
}
