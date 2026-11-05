//! First-run import of the Runner package's `bundled-data/` snapshot (P0-3).
//!
//! The Runner release zip carries `bundled-data/{manifest.json,tasks/**,assets/**}`:
//! a read-only snapshot generated from `oas-core` at packaging time. On the very
//! first start of a fresh Runner data root the files are verified against the
//! manifest (sha256) and copied into the live `tasks/` + `assets/` directories,
//! so the end-user Runner works out of the box.
//!
//! Safety rules (design `RUNNER-SPLIT-DESIGN.md` §6.5, simplified for P0):
//! * never overwrite an existing data root: if `tasks/` already contains task
//!   directories or `assets/assets.json` already exists, the import is skipped
//!   (the user may still install packages via `.oaspkg`);
//! * every copied file is verified against the sha256 in `manifest.json` before
//!   anything is written; a mismatch aborts with no partial copy;
//! * the import is idempotent: `packages/bundled-import.json` records the source
//!   snapshot, and the same snapshot is never imported twice;
//! * manifest paths are relative and must stay inside the bundled data root
//!   (no absolute paths / `..`).
//!
//! The module is pure std + serde + `oas_task::package::sha256_file`; it is called
//! from `main.rs` for the Runner surface only, before the HTTP server starts.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Directory name of the bundled snapshot inside the Runner package root.
pub const BUNDLED_DIR: &str = "bundled-data";
/// Manifest file inside [`BUNDLED_DIR`].
pub const MANIFEST_FILE: &str = "manifest.json";
/// Marker written under `packages/` after a successful import.
pub const IMPORT_MARKER: &str = "bundled-import.json";
/// Current manifest schema version.
pub const MANIFEST_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct SourceInfo {
    #[serde(default)]
    pub repo: String,
    #[serde(default)]
    pub commit: String,
    #[serde(default)]
    pub version: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct AssetsInfo {
    #[serde(default)]
    pub assets_json_sha256: String,
    #[serde(default)]
    pub image_count: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct FileEntry {
    pub path: String,
    pub sha256: String,
    #[serde(default)]
    pub bytes: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct BundledManifest {
    pub schema_version: u32,
    #[serde(default)]
    pub generated_at_ms: u64,
    #[serde(default)]
    pub source: SourceInfo,
    #[serde(default)]
    pub tasks: Vec<String>,
    #[serde(default)]
    pub assets: AssetsInfo,
    #[serde(default)]
    pub files: Vec<FileEntry>,
    #[serde(default)]
    pub total_bytes: u64,
}

/// Marker written to `packages/bundled-import.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImportMarker {
    pub schema_version: u32,
    pub source: SourceInfo,
    pub imported_at_ms: u64,
    pub tasks: Vec<String>,
    pub files: usize,
    pub total_bytes: u64,
}

/// Result of an import attempt. `imported == false` always carries a `reason`.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportReport {
    pub imported: bool,
    pub reason: String,
    pub tasks: usize,
    pub files: usize,
    pub total_bytes: u64,
}

impl ImportReport {
    fn skipped(reason: &str) -> Self {
        Self {
            imported: false,
            reason: reason.to_string(),
            tasks: 0,
            files: 0,
            total_bytes: 0,
        }
    }
}

/// Path of `bundled-data` for a package root (usually the process CWD).
pub fn bundled_root(package_root: &Path) -> PathBuf {
    package_root.join(BUNDLED_DIR)
}

/// Import the bundled snapshot when this is a fresh data root.
///
/// `package_root` is the Runner package directory (the process CWD when started
/// by `start-runner.bat`); `tasks_dir` / `assets_dir` / `packages_dir` are the
/// configured live data dirs (relative to `package_root` by default).
pub fn import_first_run(
    package_root: &Path,
    tasks_dir: &Path,
    assets_dir: &Path,
    packages_dir: &Path,
) -> Result<ImportReport, String> {
    let root = bundled_root(package_root);
    let manifest_path = root.join(MANIFEST_FILE);
    if !manifest_path.is_file() {
        return Ok(ImportReport::skipped("no_bundled_data"));
    }
    let raw = fs::read_to_string(&manifest_path)
        .map_err(|e| format!("read {}: {e}", manifest_path.display()))?;
    let manifest: BundledManifest =
        serde_json::from_str(&raw).map_err(|e| format!("parse {}: {e}", manifest_path.display()))?;
    if manifest.schema_version != MANIFEST_SCHEMA_VERSION {
        return Err(format!(
            "bundled manifest schema_version {} is unsupported (expected {MANIFEST_SCHEMA_VERSION})",
            manifest.schema_version
        ));
    }
    if manifest.files.is_empty() {
        return Err("bundled manifest has no files".to_string());
    }

    // Idempotency: the same snapshot is never imported twice.
    let marker_path = packages_dir.join(IMPORT_MARKER);
    if let Some(marker) = read_marker(&marker_path)? {
        if !manifest.source.commit.is_empty() && marker.source.commit == manifest.source.commit {
            return Ok(ImportReport::skipped("already_imported"));
        }
    }

    // Never touch a data root that already has user data.
    if tasks_dir_has_data(tasks_dir)? {
        return Ok(ImportReport::skipped("existing_tasks"));
    }
    if assets_dir_has_data(assets_dir)? {
        return Ok(ImportReport::skipped("existing_assets"));
    }

    // Verify every file BEFORE writing anything (fail closed, no partial copy).
    let mut total = 0u64;
    for entry in &manifest.files {
        let rel = safe_relative_path(&entry.path)?;
        let src = root.join(&rel);
        if !src.is_file() {
            return Err(format!("bundled file is missing: {}", src.display()));
        }
        let got = oas_task::package::sha256_file(&src)
            .map_err(|e| format!("hash {}: {e}", src.display()))?;
        if !got.eq_ignore_ascii_case(&entry.sha256) {
            return Err(format!(
                "bundled file sha256 mismatch for {}: manifest {} != actual {}",
                entry.path, entry.sha256, got
            ));
        }
        if entry.bytes != 0 {
            let len = fs::metadata(&src).map(|m| m.len()).unwrap_or(0);
            if len != entry.bytes {
                return Err(format!(
                    "bundled file size mismatch for {}: manifest {} != actual {len}",
                    entry.path, entry.bytes
                ));
            }
        }
        total += entry.bytes.max(fs::metadata(&src).map(|m| m.len()).unwrap_or(0));
    }

    // Copy: tasks/** -> tasks_dir/**, assets/** -> assets_dir/**.
    let mut created_tasks: Vec<PathBuf> = Vec::new();
    let copied = copy_all(&root, tasks_dir, assets_dir, &manifest.files, &mut created_tasks)?;
    if copied != manifest.files.len() {
        rollback(&created_tasks);
        return Err(format!(
            "bundled import copied {copied}/{} files",
            manifest.files.len()
        ));
    }

    let marker = ImportMarker {
        schema_version: 1,
        source: manifest.source.clone(),
        imported_at_ms: now_ms(),
        tasks: manifest.tasks.clone(),
        files: manifest.files.len(),
        total_bytes: total,
    };
    if let Err(e) = write_marker(&marker_path, &marker) {
        rollback(&created_tasks);
        return Err(format!("write {}: {e}", marker_path.display()));
    }

    Ok(ImportReport {
        imported: true,
        reason: "first_run".to_string(),
        tasks: manifest.tasks.len(),
        files: manifest.files.len(),
        total_bytes: total,
    })
}

fn copy_all(
    root: &Path,
    tasks_dir: &Path,
    assets_dir: &Path,
    files: &[FileEntry],
    created_tasks: &mut Vec<PathBuf>,
) -> Result<usize, String> {
    let mut copied = 0usize;
    for entry in files {
        let rel = safe_relative_path(&entry.path)?;
        let src = root.join(&rel);
        let dest = destination_for(&rel, tasks_dir, assets_dir)?;
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
        }
        // Track the top-level task directory so a mid-import failure can roll back.
        if let Ok(rest) = rel.strip_prefix("tasks") {
            if let Some(first) = rest.components().next() {
                let task_root = tasks_dir.join(first.as_os_str());
                if !created_tasks.contains(&task_root) {
                    created_tasks.push(task_root);
                }
            }
        }
        fs::copy(&src, &dest)
            .map_err(|e| format!("copy {} -> {}: {e}", src.display(), dest.display()))?;
        copied += 1;
    }
    Ok(copied)
}

/// Map a manifest-relative path (`tasks/...` / `assets/...`) to the live data dir.
pub fn destination_for(rel: &Path, tasks_dir: &Path, assets_dir: &Path) -> Result<PathBuf, String> {
    if let Ok(rest) = rel.strip_prefix("tasks") {
        return Ok(tasks_dir.join(rest));
    }
    if let Ok(rest) = rel.strip_prefix("assets") {
        return Ok(assets_dir.join(rest));
    }
    Err(format!(
        "bundled manifest path `{}` must live under tasks/ or assets/",
        rel.display()
    ))
}

/// Reject absolute paths and `..` components in manifest entries.
pub fn safe_relative_path(raw: &str) -> Result<PathBuf, String> {
    let raw = raw.trim().replace('\\', "/");
    if raw.is_empty() || raw.starts_with('/') || raw.contains("..") {
        return Err(format!("bundled manifest path is not a safe relative path: `{raw}`"));
    }
    // Windows drive letters / UNC are also absolute; `Path::is_absolute` misses
    // a bare `C:` prefix on non-Windows, so reject an `X:` prefix explicitly.
    if raw.len() >= 2 && raw.as_bytes()[1] == b':' {
        return Err(format!("bundled manifest path is not a safe relative path: `{raw}`"));
    }
    Ok(PathBuf::from(raw))
}

fn tasks_dir_has_data(tasks_dir: &Path) -> Result<bool, String> {
    if !tasks_dir.exists() {
        return Ok(false);
    }
    let entries = fs::read_dir(tasks_dir).map_err(|e| format!("read {}: {e}", tasks_dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("read {}: {e}", tasks_dir.display()))?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name == "README.txt" {
            continue;
        }
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn assets_dir_has_data(assets_dir: &Path) -> Result<bool, String> {
    if assets_dir.join("assets.json").is_file() {
        return Ok(true);
    }
    let images = assets_dir.join("images");
    if images.is_dir() {
        let mut iter = fs::read_dir(&images).map_err(|e| format!("read {}: {e}", images.display()))?;
        return Ok(iter.next().is_some());
    }
    Ok(false)
}

fn read_marker(path: &Path) -> Result<Option<ImportMarker>, String> {
    match fs::read_to_string(path) {
        Ok(raw) => {
            let marker: ImportMarker = serde_json::from_str(&raw)
                .map_err(|e| format!("parse {}: {e}", path.display()))?;
            Ok(Some(marker))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("read {}: {e}", path.display())),
    }
}

fn write_marker(path: &Path, marker: &ImportMarker) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
    }
    let text = serde_json::to_string_pretty(marker).map_err(|e| format!("serialize marker: {e}"))?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, text.as_bytes()).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    fs::rename(&tmp, path).map_err(|e| format!("replace {}: {e}", path.display()))?;
    Ok(())
}

fn rollback(created_tasks: &[PathBuf]) {
    for dir in created_tasks {
        let _ = fs::remove_dir_all(dir);
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            let p = std::env::temp_dir().join(format!(
                "oasd-bundled-{tag}-{}-{nanos}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&p);
            fs::create_dir_all(&p).expect("create temp dir");
            Self(p)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn write(path: &Path, bytes: &[u8]) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("mkdir");
        }
        fs::write(path, bytes).expect("write");
    }

    fn sha(path: &Path) -> String {
        oas_task::package::sha256_file(path).expect("sha256")
    }

    /// Build a package root with `bundled-data/{manifest.json,tasks/T1/meta.json,assets/assets.json}`.
    fn fixture(t: &TempDir) -> (PathBuf, PathBuf, PathBuf, PathBuf) {
        let pkg = t.path().join("runner");
        let bundled = bundled_root(&pkg);
        let meta = bundled.join("tasks").join("T1").join("meta.json");
        write(&meta, br#"{"task_id":"T1"}"#);
        let flow = bundled.join("tasks").join("T1").join("flow.json");
        write(&flow, br#"{"schema_version":1,"steps":[]}"#);
        let assets = bundled.join("assets").join("assets.json");
        write(&assets, br#"{"items":[]}"#);
        let img = bundled.join("assets").join("images").join("a.png");
        write(&img, b"\x89PNG\r\n\x1a\n");
        let mut files = Vec::new();
        for p in [&meta, &flow, &assets, &img] {
            let rel = p
                .strip_prefix(&bundled)
                .expect("relative")
                .to_string_lossy()
                .replace('\\', "/");
            files.push(FileEntry {
                path: rel,
                sha256: sha(p),
                bytes: fs::metadata(p).expect("meta").len(),
            });
        }
        let manifest = BundledManifest {
            schema_version: MANIFEST_SCHEMA_VERSION,
            generated_at_ms: 1,
            source: SourceInfo {
                repo: "https://example.invalid/oas-core.git".to_string(),
                commit: "deadbeef".to_string(),
                version: "0.1.0".to_string(),
            },
            tasks: vec!["T1".to_string()],
            assets: AssetsInfo {
                assets_json_sha256: sha(&assets),
                image_count: 1,
            },
            total_bytes: files.iter().map(|f| f.bytes).sum(),
            files,
        };
        write(
            &bundled.join(MANIFEST_FILE),
            serde_json::to_string_pretty(&manifest).expect("manifest json").as_bytes(),
        );
        let tasks_dir = pkg.join("tasks");
        let assets_dir = pkg.join("assets");
        let packages_dir = pkg.join("packages");
        (pkg, tasks_dir, assets_dir, packages_dir)
    }

    #[test]
    fn first_run_imports_tasks_and_assets() {
        let t = TempDir::new("first");
        let (pkg, tasks, assets, packages) = fixture(&t);
        let report = import_first_run(&pkg, &tasks, &assets, &packages).expect("import");
        assert!(report.imported, "{report:?}");
        assert_eq!(report.tasks, 1);
        assert_eq!(report.files, 4);
        assert!(tasks.join("T1").join("meta.json").is_file());
        assert!(tasks.join("T1").join("flow.json").is_file());
        assert!(assets.join("assets.json").is_file());
        assert!(assets.join("images").join("a.png").is_file());
        assert!(packages.join(IMPORT_MARKER).is_file());
    }

    #[test]
    fn second_run_is_idempotent() {
        let t = TempDir::new("idem");
        let (pkg, tasks, assets, packages) = fixture(&t);
        assert!(import_first_run(&pkg, &tasks, &assets, &packages).expect("first").imported);
        let second = import_first_run(&pkg, &tasks, &assets, &packages).expect("second");
        assert!(!second.imported);
        assert_eq!(second.reason, "already_imported");
    }

    #[test]
    fn existing_tasks_are_never_overwritten() {
        let t = TempDir::new("existing");
        let (pkg, tasks, assets, packages) = fixture(&t);
        write(&tasks.join("Mine").join("meta.json"), br#"{"task_id":"Mine"}"#);
        let report = import_first_run(&pkg, &tasks, &assets, &packages).expect("import");
        assert!(!report.imported);
        assert_eq!(report.reason, "existing_tasks");
        assert!(!packages.join(IMPORT_MARKER).exists());
    }

    #[test]
    fn missing_manifest_is_a_skip() {
        let t = TempDir::new("nomanifest");
        let pkg = t.path().join("runner");
        fs::create_dir_all(&pkg).expect("mkdir");
        let report = import_first_run(
            &pkg,
            &pkg.join("tasks"),
            &pkg.join("assets"),
            &pkg.join("packages"),
        )
        .expect("skip");
        assert!(!report.imported);
        assert_eq!(report.reason, "no_bundled_data");
    }

    #[test]
    fn sha_mismatch_aborts_without_copying() {
        let t = TempDir::new("badsha");
        let (pkg, tasks, assets, packages) = fixture(&t);
        let manifest_path = bundled_root(&pkg).join(MANIFEST_FILE);
        let mut manifest: BundledManifest =
            serde_json::from_str(&fs::read_to_string(&manifest_path).expect("read")).expect("parse");
        manifest.files[0].sha256 = "0".repeat(64);
        write(
            &manifest_path,
            serde_json::to_string_pretty(&manifest).expect("json").as_bytes(),
        );
        let err = import_first_run(&pkg, &tasks, &assets, &packages).expect_err("mismatch");
        assert!(err.contains("sha256 mismatch"), "{err}");
        assert!(!tasks.join("T1").exists(), "nothing may be copied");
        assert!(!packages.join(IMPORT_MARKER).exists());
    }

    #[test]
    fn unsafe_manifest_paths_are_rejected() {
        for bad in ["../evil", "C:/evil", "/abs/path", "tasks/../evil"] {
            assert!(safe_relative_path(bad).is_err(), "{bad} must be rejected");
        }
        assert!(safe_relative_path("tasks/T1/meta.json").is_ok());
    }
}
