//! v1.2.1 global asset store + one-time migration.
//!
//! One namespace for every task: `<assets-dir>/assets.json` + `images/`.
//! Usage from the editor/serve: `is_initialized()` decides whether global mode
//! is active; old task-local assets remain readable as a compatibility fallback
//! until migration runs.
//!
//! Migration:
//! * every task asset (and old `library/` template) is copied into the global
//!   store;
//! * duplicate ids get deterministic `_2`, `_3`, … suffixes;
//! * every task's flow.json is rewritten with THAT task's old→new mapping (so
//!   two tasks that both had `世界_章节栏` keep their own asset);
//! * the old task assets/library files are left untouched (harmless), flows are
//!   backed up before writing, and any failure rolls all flows back.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::Serialize;

use oas_task::{list_tasks, validate_id, AssetDecl, AssetsFile, TaskBundle, SCHEMA_VERSION};

pub const DEFAULT_DIR: &str = "assets";

pub fn is_initialized(root: &Path) -> bool {
    root.join("assets.json").is_file()
}

pub fn open_store(root: &Path) -> Result<TaskBundle, String> {
    TaskBundle::open_asset_store(root)
}

#[derive(Debug, Clone, Serialize)]
pub struct Renamed {
    pub old: String,
    pub new: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MigrateReport {
    pub assets: usize,
    pub tasks: usize,
    pub flows_rewritten: usize,
    pub renamed: Vec<Renamed>,
}

fn allocate(base: &str, used: &mut BTreeSet<String>) -> Result<String, String> {
    validate_id(base)?;
    if used.insert(base.to_string()) {
        return Ok(base.to_string());
    }
    let mut n = 2u32;
    loop {
        let candidate = format!("{base}_{n}");
        if used.insert(candidate.clone()) {
            return Ok(candidate);
        }
        n += 1;
    }
}

/// Rewrite quoted asset references in a flow JSON text.
///
/// Asset ids live in JSON strings and in expression strings (`appear('X')`), so
/// we replace exact quoted forms. This deliberately does not replace arbitrary
/// substrings; the migration report lists every rename for audit.
fn rewrite_flow_text(text: &str, pairs: &[(String, String)]) -> String {
    let mut out = text.to_string();
    // Longest first so `X_2` is not rewritten by the `X` rule first.
    let mut sorted = pairs.to_vec();
    sorted.sort_by(|a, b| b.0.len().cmp(&a.0.len()));
    for (old, new) in sorted {
        // JSON string + expression single-quoted forms; `lib:` namespace alias.
        for (pattern, replacement) in [
            (format!("\"{old}\""), format!("\"{new}\"")),
            (format!("'{old}'"), format!("'{new}'")),
            (format!("\"lib:{old}\""), format!("\"{new}\"")),
            (format!("'lib:{old}'"), format!("'{new}'")),
        ] {
            out = out.replace(&pattern, &replacement);
        }
    }
    out
}

/// Copy one asset image into the global store and return its new relative path.
fn copy_image(
    src: &Path,
    images_dir: &Path,
    seq: &mut u32,
) -> Result<String, String> {
    let bytes = std::fs::read(src).map_err(|e| format!("read image {}: {e}", src.display()))?;
    let name = format!("a{:03}.png", *seq);
    *seq += 1;
    std::fs::create_dir_all(images_dir)
        .map_err(|e| format!("create {}: {e}", images_dir.display()))?;
    std::fs::write(images_dir.join(&name), bytes)
        .map_err(|e| format!("write global image {}: {e}", name))?;
    Ok(format!("images/{name}"))
}

pub fn migrate(
    tasks_dir: &Path,
    library_dir: &Path,
    assets_dir: &Path,
) -> Result<MigrateReport, String> {
    if is_initialized(assets_dir) {
        return Err(format!(
            "global asset store already initialized at {}",
            assets_dir.display()
        ));
    }
    let task_names = list_tasks(tasks_dir)?;
    let mut used_ids: BTreeSet<String> = BTreeSet::new();
    let mut items: Vec<AssetDecl> = Vec::new();
    let mut renamed: Vec<Renamed> = Vec::new();
    let mut image_seq = 1u32;
    let mut design: Option<[u32; 2]> = None;
    let images_dir = assets_dir.join("images");
    std::fs::create_dir_all(&images_dir)
        .map_err(|e| format!("create asset images dir: {e}"))?;

    let mut task_maps: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    let mut task_flows: BTreeMap<String, (PathBuf, String)> = BTreeMap::new();

    // ---- task assets -----------------------------------------------------
    for task in &task_names {
        let bundle = TaskBundle::open_named(tasks_dir, task)?;
        let flow_path = bundle.flow_path();
        if let Ok(text) = std::fs::read_to_string(&flow_path) {
            task_flows.insert(task.clone(), (flow_path.clone(), text));
        }
        let af = bundle.load_assets_file()?;
        if af.design_resolution.is_some() && design.is_none() {
            design = af.design_resolution;
        }
        let mut map: BTreeMap<String, String> = BTreeMap::new();
        for old in &af.items {
            let new_id = allocate(&old.id, &mut used_ids)?;
            if map.insert(old.id.clone(), new_id.clone()).is_some() {
                return Err(format!(
                    "task `{task}` has duplicate asset id `{}`; fix it before migration",
                    old.id
                ));
            }
            let mut item = old.clone();
            item.id = new_id.clone();
            if let Some(file) = old.file.as_deref() {
                let src = bundle.root().join(file);
                item.file = Some(copy_image(&src, &images_dir, &mut image_seq)?);
            }
            if item.id != old.id {
                renamed.push(Renamed {
                    old: old.id.clone(),
                    new: new_id.clone(),
                    source: format!("task:{task}"),
                });
            }
            items.push(item);
        }
        task_maps.insert(task.clone(), map);
    }

    // ---- legacy global library (templates only) --------------------------
    let lib = crate::library::Library::open(library_dir);
    let lib_file = lib.load_file()?;
    let mut lib_map: BTreeMap<String, String> = BTreeMap::new();
    for old in &lib_file.items {
        let new_id = allocate(&old.id, &mut used_ids)?;
        let src = lib.root().join(&old.file);
        let new_file = copy_image(&src, &images_dir, &mut image_seq)?;
        lib_map.insert(old.id.clone(), new_id.clone());
        if new_id != old.id {
            renamed.push(Renamed {
                old: old.id.clone(),
                new: new_id.clone(),
                source: "library".to_string(),
            });
        }
        items.push(AssetDecl {
            id: new_id,
            kind: "template".to_string(),
            file: Some(new_file),
            // Old library templates had no authored ROI; full-frame search.
            roi: None,
            action_roi: None,
            ocr_mode: None,
            ocr_source_roi: None,
            ocr_text: None,
            threshold: old.threshold,
            polygon: None,
            rev: old.rev,
            positions: Vec::new(),
        });
    }

    // ---- write global assets.json --------------------------------------
    let assets_file = AssetsFile {
        schema_version: SCHEMA_VERSION,
        design_resolution: design,
        items,
        collections: Vec::new(),
    };
    let text = serde_json::to_string_pretty(&assets_file)
        .map_err(|e| format!("serialize global assets: {e}"))?;
    std::fs::write(assets_dir.join("assets.json"), format!("{text}\n"))
        .map_err(|e| format!("write global assets.json: {e}"))?;

    // ---- rewrite flows ---------------------------------------------------
    let global_ids: BTreeSet<String> = assets_file.items.iter().map(|a| a.id.clone()).collect();
    let mut rewritten: Vec<(PathBuf, String)> = Vec::new();
    let mut flows_rewritten = 0usize;
    for (task, (path, original)) in &task_flows {
        let mut pairs: Vec<(String, String)> = task_maps
            .get(task)
            .map(|m| m.iter().map(|(o, n)| (o.clone(), n.clone())).collect())
            .unwrap_or_default();
        pairs.extend(lib_map.iter().map(|(o, n)| (o.clone(), n.clone())));
        let next = rewrite_flow_text(original, &pairs);
        let flow = match oas_flow::Flow::from_json(&next) {
            Ok(f) => f,
            Err(e) => {
                // Roll back any flows written so far.
                for (p, old) in &rewritten {
                    let _ = std::fs::write(p, old);
                }
                let _ = std::fs::remove_dir_all(assets_dir);
                return Err(format!("task `{task}` flow.json invalid after migration: {e}"));
            }
        };
        if let Err(errs) = flow.validate_assets(&global_ids) {
            for (p, old) in &rewritten {
                let _ = std::fs::write(p, old);
            }
            let _ = std::fs::remove_dir_all(assets_dir);
            let joined = errs.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("; ");
            return Err(format!("task `{task}` flow references missing assets: {joined}"));
        }
        rewritten.push((path.clone(), original.clone()));
        std::fs::write(path, next).map_err(|e| format!("write {}: {e}", path.display()))?;
        flows_rewritten += 1;
    }

    let report = MigrateReport {
        assets: assets_file.items.len(),
        tasks: task_names.len(),
        flows_rewritten,
        renamed,
    };
    let report_path = assets_dir.join("migration-report.json");
    let _ = std::fs::write(
        &report_path,
        serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".to_string()) + "\n",
    );
    Ok(report)
}

#[derive(Debug, Clone, Serialize)]
pub struct LegacyCleanupItem {
    pub task: String,
    pub assets_json: bool,
    pub images_dir: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct LegacyCleanupReport {
    pub dry_run: bool,
    pub tasks_scanned: usize,
    pub items: Vec<LegacyCleanupItem>,
    pub backup_dir: Option<PathBuf>,
}

fn move_legacy_path(src: &Path, dst: &Path) -> Result<(), String> {
    if !src.exists() {
        return Ok(());
    }
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("create {}: {e}", parent.display()))?;
    }
    std::fs::rename(src, dst)
        .map_err(|e| format!("move {} -> {}: {e}", src.display(), dst.display()))
}

/// v1.2.2 (N46): after `assets migrate`, old per-task `assets.json` / `images/`
/// are unused. Move them into `<assets-dir>/_legacy-backup/<timestamp>/<task>/`
/// (never delete, never touch meta/flow). `apply=false` only reports.
pub fn cleanup_legacy(
    tasks_dir: &Path,
    assets_dir: &Path,
    apply: bool,
) -> Result<LegacyCleanupReport, String> {
    if !is_initialized(assets_dir) {
        return Err(format!(
            "global asset store not initialized at {}; run `assets migrate` first",
            assets_dir.display()
        ));
    }
    let names = list_tasks(tasks_dir)?;
    let mut items = Vec::new();
    for task in &names {
        let bundle = TaskBundle::open_named(tasks_dir, task)?;
        let has_assets = bundle.assets_path().is_file();
        let has_images = bundle.images_dir().is_dir();
        if has_assets || has_images {
            items.push(LegacyCleanupItem {
                task: task.clone(),
                assets_json: has_assets,
                images_dir: has_images,
            });
        }
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let backup_dir = assets_dir.join("_legacy-backup").join(stamp.to_string());
    if apply {
        for item in &items {
            let bundle = TaskBundle::open_named(tasks_dir, &item.task)?;
            let dst = backup_dir.join(&item.task);
            if item.assets_json {
                move_legacy_path(&bundle.assets_path(), &dst.join("assets.json"))?;
            }
            if item.images_dir {
                move_legacy_path(&bundle.images_dir(), &dst.join("images"))?;
            }
        }
        let report = LegacyCleanupReport {
            dry_run: false,
            tasks_scanned: names.len(),
            items,
            backup_dir: Some(backup_dir),
        };
        let report_path = assets_dir.join("legacy-cleanup-report.json");
        let _ = std::fs::write(
            &report_path,
            serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".into()) + "\n",
        );
        Ok(report)
    } else {
        Ok(LegacyCleanupReport {
            dry_run: true,
            tasks_scanned: names.len(),
            items,
            backup_dir: None,
        })
    }
}
