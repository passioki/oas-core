//! 使用端OCR文字配置项 (`kind:"ocr_var"`) ↔ `meta.params` derived-row sync and
//! the explicit old-param migration (2026-11-06 参数→素材 redesign, C1′).
//!
//! # Why a single sync function
//!
//! The design (D1 / C1′) keeps `meta.params` as the **only** `ParamDecl` source
//! of truth for the runtime (lint / four-layer injection / `values.json` /
//! Runner), while the **asset** is the single UI truth of an ocr_var. A derived
//! row is a `meta.params[]` entry with `asset_id = Some(<ocr_var id>)`, shaped
//! `type=string`, `role=ocr_text`, `default=runtime_text`,
//! `label=<asset label|id>`; `group`/`description` stay user-editable and are
//! preserved. [`sync_ocr_var_params`] is the only writer of those rows — the
//! Studio/Runner show them read-only, and the T8 lint
//! ([`derived_param_issues`]) flags any drift instead of fixing it silently.
//!
//! # Which assets are "used"
//!
//! An ocr_var keeps a derived row when **either**
//! * the flow references its asset id (`Flow::referenced_assets()`), or
//! * the flow uses its parameter key (`Flow::param_uses()`), which is the
//!   migration case: an old flow still says `text_expr: "param('group')"` and
//!   does not reference the new ocr_var asset yet.
//!
//! Without the second condition a migration could never keep the old
//! `param('key')` call sites working (design §8.3 keeps flow/values untouched).

use std::collections::{BTreeMap, BTreeSet};

use oas_flow::Flow;

use crate::meta::{is_param_key, Meta, ParamDecl};
use crate::{validate_ocr_var_fields, AssetDecl, AssetsFile, LintIssue};

/// What one [`sync_ocr_var_params`] call changed (in `meta.params` row keys).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SyncReport {
    /// Keys of derived rows that did not exist before.
    pub added: Vec<String>,
    /// Keys of derived rows whose content was refreshed from the asset/flow.
    pub updated: Vec<String>,
    /// Keys of derived rows removed because their asset is no longer used.
    pub removed: Vec<String>,
}

/// One old `role=ocr_text` parameter that a migration would turn into an
/// `ocr_var` asset.
#[derive(Debug, Clone, PartialEq)]
pub struct MigrateCandidate {
    /// Old `meta.params[].key` (also the new asset's `param_key`).
    pub param_key: String,
    /// New asset id (`O_<sanitized key>`, unique in the store).
    pub asset_id: String,
    /// Old parameter default → `runtime_text`.
    pub runtime_text: String,
    /// Old `label`, if any (carried to the asset / derived row).
    pub label: Option<String>,
}

/// One old parameter that cannot be migrated automatically and needs the user.
#[derive(Debug, Clone, PartialEq)]
pub struct MigrateSkip {
    pub param_key: String,
    pub reason: String,
}

/// Result of [`migrate_params_to_ocr_vars`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MigrateReport {
    pub dry_run: bool,
    /// Assets that would be created / were created.
    pub candidates: Vec<MigrateCandidate>,
    /// Old parameters left untouched (non-string default etc.).
    pub skipped: Vec<MigrateSkip>,
    /// Set for a real run: the derived-row sync performed after the migration.
    pub sync: Option<SyncReport>,
}

/// Every `kind:"ocr_var"` asset, keyed by id (declaration order preserved by the
/// caller where it matters).
fn ocr_var_assets(assets: &AssetsFile) -> BTreeMap<String, &AssetDecl> {
    assets
        .items
        .iter()
        .filter(|a| a.kind == "ocr_var")
        .map(|a| (a.id.clone(), a))
        .collect()
}

/// The ocr_var ids whose derived rows must exist: referenced by the flow as an
/// asset **or** by its `param_key` (`param('key')` call sites).
fn used_ocr_var_ids(flow: &Flow, ocr_vars: &BTreeMap<String, &AssetDecl>) -> BTreeSet<String> {
    let mut used: BTreeSet<String> = flow
        .referenced_assets()
        .into_iter()
        .filter(|id| ocr_vars.contains_key(id))
        .collect();
    let param_uses = flow.param_uses();
    for (id, asset) in ocr_vars {
        if let Some(key) = asset.param_key.as_deref() {
            if param_uses.contains(key) {
                used.insert(id.clone());
            }
        }
    }
    used
}

/// Build the `ParamDecl` one ocr_var asset must have in `meta.params`.
///
/// `previous` is the existing derived row for this asset (if any); only its
/// user-editable `group`/`description` are preserved.
fn derived_param(asset: &AssetDecl, previous: Option<&ParamDecl>) -> Result<ParamDecl, String> {
    validate_ocr_var_fields(asset, &format!("ocr_var `{}`", asset.id))?;
    let runtime_text = asset
        .runtime_text
        .clone()
        .ok_or_else(|| format!("ocr_var `{}` has no runtime_text", asset.id))?;
    let param_key = asset
        .param_key
        .clone()
        .ok_or_else(|| format!("ocr_var `{}` has no param_key", asset.id))?;
    let label = asset
        .label
        .clone()
        .filter(|l| !l.trim().is_empty())
        .unwrap_or_else(|| asset.id.clone());
    // A long asset id must not produce an invalid (over-limit) parameter label.
    let label: String = label
        .chars()
        .take(crate::meta::PARAM_LABEL_MAX_CHARS)
        .collect();
    Ok(ParamDecl {
        key: param_key,
        kind: "string".to_string(),
        default: Some(serde_json::Value::String(runtime_text)),
        min: None,
        max: None,
        label: Some(label),
        description: previous.and_then(|p| p.description.clone()),
        enum_values: None,
        required: None,
        group: previous.and_then(|p| p.group.clone()),
        role: Some("ocr_text".to_string()),
        asset_id: Some(asset.id.clone()),
    })
}

/// Derive the expected `asset_id` → [`ParamDecl`] map for every **used** ocr_var.
///
/// Also rejects the two hard conflicts: two ocr_vars sharing one `param_key`,
/// and a derived `param_key` colliding with a user-authored parameter row.
fn expected_derived_params(
    meta: &Meta,
    assets: &AssetsFile,
    flow: &Flow,
) -> Result<BTreeMap<String, ParamDecl>, String> {
    let ocr_vars = ocr_var_assets(assets);
    let used = used_ocr_var_ids(flow, &ocr_vars);
    let mut expected: BTreeMap<String, ParamDecl> = BTreeMap::new(); // asset id -> row
    let mut keys: BTreeSet<String> = BTreeSet::new();
    for id in &used {
        let asset = ocr_vars
            .get(id)
            .ok_or_else(|| format!("ocr_var `{id}` is missing from assets.json"))?;
        let previous = meta
            .params
            .iter()
            .find(|p| p.asset_id.as_deref() == Some(id.as_str()));
        let row = derived_param(asset, previous)?;
        if !is_param_key(&row.key) {
            return Err(format!(
                "ocr_var `{id}` param_key `{}` is not a valid parameter key",
                row.key
            ));
        }
        if !keys.insert(row.key.clone()) {
            return Err(format!(
                "ocr_var parameter key `{}` is used by more than one ocr_var asset",
                row.key
            ));
        }
        if let Some(user) = meta
            .params
            .iter()
            .find(|p| p.asset_id.is_none() && p.key == row.key)
        {
            let _ = user;
            return Err(format!(
                "derived parameter key `{}` conflicts with a user-authored meta.params row; \
                 rename the parameter or the ocr_var",
                row.key
            ));
        }
        expected.insert(id.clone(), row);
    }
    Ok(expected)
}

/// Synchronise the `asset_id`-marked derived rows of `meta.params` with the
/// ocr_var assets and the flow (2026-11-06 参数→素材 C1′).
///
/// * only derived rows (`asset_id.is_some()`) are added / updated / removed —
///   user-authored rows are never touched;
/// * an ocr_var is "used" when the flow references it as an asset or uses its
///   `param_key` in a `param()` expression;
/// * `type`/`role`/`default`/`label`/`key` come from the asset; `group` and
///   `description` are preserved from the previous row (Lead ruling 7);
/// * key conflicts (two derived rows, or derived vs user row) are loud errors;
/// * the caller persists the result with [`crate::TaskBundle::write_meta`]
///   (which keeps a `.bak` backup).
pub fn sync_ocr_var_params(
    meta: &mut Meta,
    assets: &AssetsFile,
    flow: &Flow,
) -> Result<SyncReport, String> {
    let expected = expected_derived_params(meta, assets, flow)?;
    let mut report = SyncReport::default();
    let mut out: Vec<ParamDecl> = Vec::with_capacity(meta.params.len() + expected.len());
    let mut present: BTreeSet<String> = BTreeSet::new(); // asset ids kept
    for row in meta.params.drain(..) {
        let Some(asset_id) = row.asset_id.clone() else {
            out.push(row); // user-authored row: never touched
            continue;
        };
        let Some(next) = expected.get(&asset_id) else {
            // The ocr_var is no longer used by this flow: drop the derived row.
            report.removed.push(row.key);
            continue;
        };
        present.insert(asset_id);
        if row != *next {
            report.updated.push(next.key.clone());
        }
        out.push(next.clone());
    }
    for (asset_id, next) in &expected {
        if present.contains(asset_id) {
            continue;
        }
        report.added.push(next.key.clone());
        out.push(next.clone());
    }
    meta.params = out;
    Ok(report)
}

/// Sanitize an old parameter key into an asset id stem (`O_<key>`): keys are
/// already identifiers, so only the length is bounded.
fn migrate_asset_id(key: &str, used: &mut BTreeSet<String>) -> String {
    let mut candidate = format!("O_{key}");
    let mut n = 2u32;
    while used.contains(&candidate) {
        candidate = format!("O_{key}_{n}");
        n += 1;
    }
    let _ = used.insert(candidate.clone());
    candidate
}

/// Migrate old `meta.params` `role=ocr_text` parameters into real `ocr_var`
/// assets (design §8.3), **explicitly** (never on open/lint/publish).
///
/// Dry run (`dry_run=true`) only computes the plan (assets that would be
/// created, parameters that cannot be migrated). A real run mutates the passed
/// `assets`/`meta` in memory:
/// 1. create one `roi:None` ocr_var asset per migratable parameter
///    (`runtime_text` = the old string default, `param_key` = the old key,
///    `label` = the old label);
/// 2. remove those old parameter rows (their `text_expr` call sites stay
///    untouched — design §8.3);
/// 3. re-run [`sync_ocr_var_params`] to recreate the derived rows with the same
///    keys (requires the task flow).
///
/// The caller (serve) owns the `meta.json.bak` / `assets.json.bak` backups, the
/// disk writes and the rollback; nothing here touches the filesystem.
pub fn migrate_params_to_ocr_vars(
    assets: &mut AssetsFile,
    meta: &mut Meta,
    flow: Option<&Flow>,
    dry_run: bool,
) -> Result<MigrateReport, String> {
    let mut used: BTreeSet<String> = assets.items.iter().map(|a| a.id.clone()).collect();
    used.extend(assets.collections.iter().map(|c| c.id.clone()));
    let mut report = MigrateReport {
        dry_run,
        ..MigrateReport::default()
    };
    for p in &meta.params {
        if p.asset_id.is_some() || p.role.as_deref() != Some("ocr_text") {
            continue; // derived rows and other params are not migration input
        }
        if !is_param_key(&p.key) {
            report.skipped.push(MigrateSkip {
                param_key: p.key.clone(),
                reason: "key is not a valid parameter key".to_string(),
            });
            continue;
        }
        let Some(text) = p.default.as_ref().and_then(|v| v.as_str()) else {
            report.skipped.push(MigrateSkip {
                param_key: p.key.clone(),
                reason: "role=ocr_text parameter has no string default".to_string(),
            });
            continue;
        };
        if text.trim().is_empty() {
            report.skipped.push(MigrateSkip {
                param_key: p.key.clone(),
                reason: "role=ocr_text parameter has a blank default".to_string(),
            });
            continue;
        }
        let asset_id = migrate_asset_id(&p.key, &mut used);
        report.candidates.push(MigrateCandidate {
            param_key: p.key.clone(),
            asset_id,
            runtime_text: text.to_string(),
            label: p.label.clone(),
        });
    }
    if dry_run {
        return Ok(report);
    }
    if report.candidates.is_empty() {
        return Ok(report);
    }
    let Some(flow) = flow else {
        return Err(
            "migrate ocr params: a real run needs the task flow to rebuild the derived \
             meta.params rows"
                .to_string(),
        );
    };
    let mut migrated_keys: BTreeSet<String> = BTreeSet::new();
    for cand in &report.candidates {
        let mut decl = AssetDecl::ocr_var(
            cand.asset_id.clone(),
            cand.param_key.clone(),
            cand.runtime_text.clone(),
            None,
        );
        decl.label = cand.label.clone();
        validate_ocr_var_fields(&decl, &format!("migrated asset `{}`", decl.id))?;
        assets.items.push(decl);
        migrated_keys.insert(cand.param_key.clone());
    }
    meta.params
        .retain(|p| !(p.asset_id.is_none() && migrated_keys.contains(&p.key)));
    // Recreate the derived rows (same keys) so every old `param('key')` call site
    // keeps resolving; the flow itself is never rewritten (§8.3).
    report.sync = Some(sync_ocr_var_params(meta, assets, flow)?);
    Ok(report)
}

/// **T8 lint** (Lead ruling 4: error, blocks publish): `meta.params` derived rows
/// must match the ocr_var assets and the flow.
///
/// Every issue carries a `meta.json:params[i]` path (or `meta.json:params` for a
/// missing row). Drift is reported, never fixed: the user must re-save the asset
/// (which triggers the sync) or run the migration.
pub fn derived_param_issues(meta: &Meta, assets: &AssetsFile, flow: &Flow) -> Vec<LintIssue> {
    let mut issues = Vec::new();
    let ocr_vars = ocr_var_assets(assets);
    let used = used_ocr_var_ids(flow, &ocr_vars);
    let expected = match expected_derived_params(meta, assets, flow) {
        Ok(expected) => expected,
        Err(e) => {
            issues.push(LintIssue::new(
                "T8",
                "meta.json:params",
                format!("derived ocr_var parameter rows are inconsistent: {e}"),
            ));
            BTreeMap::new()
        }
    };
    let mut present: BTreeSet<String> = BTreeSet::new();
    for (i, p) in meta.params.iter().enumerate() {
        let path = format!("meta.json:params[{i}]");
        let Some(asset_id) = p.asset_id.as_deref() else {
            continue; // user-authored row
        };
        present.insert(asset_id.to_string());
        let Some(asset) = ocr_vars.get(asset_id) else {
            issues.push(LintIssue::new(
                "T8",
                path.clone(),
                format!(
                    "derived row `{}` references `{asset_id}`, which is not an ocr_var asset \
                     (re-save the asset or run the ocr-param migration)",
                    p.key
                ),
            ));
            continue;
        };
        if !used.contains(asset_id) {
            issues.push(LintIssue::new(
                "T8",
                path.clone(),
                format!(
                    "derived row `{}` is stale: ocr_var `{asset_id}` is not referenced by \
                     flow.json (re-save the flow or delete the asset)",
                    p.key
                ),
            ));
            continue;
        }
        let Some(want) = expected.get(asset_id) else {
            continue; // already reported above
        };
        if p != want {
            issues.push(LintIssue::new(
                "T8",
                path.clone(),
                format!(
                    "derived row `{}` does not match ocr_var `{asset_id}` \
                     (expected key=`{}` type=string role=ocr_text default={:?}); \
                     re-save the asset to sync",
                    p.key, want.key, want.default
                ),
            ));
        }
        if asset.param_key.as_deref() != Some(p.key.as_str()) {
            issues.push(LintIssue::new(
                "T8",
                path,
                format!(
                    "derived row key `{}` differs from ocr_var `{asset_id}` param_key `{}`",
                    p.key,
                    asset.param_key.as_deref().unwrap_or("")
                ),
            ));
        }
    }
    for (asset_id, want) in &expected {
        if !present.contains(asset_id) {
            issues.push(LintIssue::new(
                "T8",
                "meta.json:params",
                format!(
                    "ocr_var `{asset_id}` is used but its derived parameter `{}` is missing \
                     (re-save the asset to sync)",
                    want.key
                ),
            ));
        }
    }
    issues
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn meta(params: serde_json::Value) -> Meta {
        Meta::from_json(
            &json!({"schema_version":1,"name":"任务1","params":params}).to_string(),
        )
        .expect("meta json")
    }

    fn assets(items: serde_json::Value) -> AssetsFile {
        serde_json::from_value(json!({
            "schema_version":1,
            "design_resolution":[1280,720],
            "items":items
        }))
        .expect("assets json")
    }

    fn flow(steps: serde_json::Value) -> Flow {
        Flow::from_json(
            &json!({"schema_version":1,"name":"任务1","steps":steps}).to_string(),
        )
        .expect("flow json")
    }

    /// Flow that references the ocr_var asset AND uses its parameter key.
    fn flow_using(asset_id: &str, key: &str) -> Flow {
        flow(json!([
            {"op":"click_target","args":{"find":[
                {"kind":"ocr_text","sel":asset_id,"text_expr":format!("param('{key}')")}
            ]}},
            {"op":"finish","args":{"exit":"done"}}
        ]))
    }

    fn ocr_var(id: &str, key: &str, text: &str) -> serde_json::Value {
        json!({"id":id,"kind":"ocr_var","runtime_text":text,"param_key":key})
    }

    #[test]
    fn sync_adds_a_derived_row_for_a_used_ocr_var() {
        let mut m = meta(json!([]));
        let a = assets(json!([ocr_var("O_group", "group", "御魂")]));
        let f = flow_using("O_group", "group");
        let report = sync_ocr_var_params(&mut m, &a, &f).expect("sync");
        assert_eq!(report.added, vec!["group".to_string()]);
        assert!(report.updated.is_empty() && report.removed.is_empty());
        assert_eq!(m.params.len(), 1);
        let row = &m.params[0];
        assert_eq!(row.key, "group");
        assert_eq!(row.kind, "string");
        assert_eq!(row.role.as_deref(), Some("ocr_text"));
        assert_eq!(row.default, Some(json!("御魂")));
        assert_eq!(row.label.as_deref(), Some("O_group"));
        assert_eq!(row.asset_id.as_deref(), Some("O_group"));
        // A second sync is a no-op.
        let again = sync_ocr_var_params(&mut m, &a, &f).expect("resync");
        assert_eq!(again, SyncReport::default());
    }

    #[test]
    fn sync_keeps_the_row_for_an_old_flow_that_only_uses_the_param_key() {
        // Migration case: the old flow still recognizes an old OCR asset and
        // only uses `param('group')`; the new ocr_var asset is not referenced
        // yet, but its derived row must exist so the call site keeps resolving.
        let mut m = meta(json!([]));
        let a = assets(json!([ocr_var("O_group", "group", "御魂")]));
        let f = flow(json!([
            {"op":"click_target","args":{"find":[
                {"kind":"ocr_text","sel":"旧OCR行","text_expr":"param('group')"}
            ]}},
            {"op":"finish","args":{"exit":"done"}}
        ]));
        let report = sync_ocr_var_params(&mut m, &a, &f).expect("sync");
        assert_eq!(report.added, vec!["group".to_string()]);
        assert_eq!(m.params[0].asset_id.as_deref(), Some("O_group"));
    }

    #[test]
    fn sync_updates_the_default_and_preserves_group_and_description() {
        let mut m = meta(json!([
            {"key":"group","type":"string","default":"旧文字","role":"ocr_text",
             "asset_id":"O_group","group":"识别文字","description":"游戏内分组名",
             "label":"旧标签","required":true}
        ]));
        let a = assets(json!([ocr_var("O_group", "group", "结界")]));
        let f = flow_using("O_group", "group");
        let report = sync_ocr_var_params(&mut m, &a, &f).expect("sync");
        assert_eq!(report.updated, vec!["group".to_string()]);
        let row = &m.params[0];
        assert_eq!(row.default, Some(json!("结界")), "default follows runtime_text");
        assert_eq!(row.group.as_deref(), Some("识别文字"), "group is user-owned");
        assert_eq!(row.description.as_deref(), Some("游戏内分组名"));
        assert_eq!(row.label.as_deref(), Some("O_group"), "label follows the asset/id");
        assert_eq!(row.required, None, "derived rows always have a default");
    }

    #[test]
    fn sync_removes_a_stale_derived_row_and_never_touches_user_rows() {
        let mut m = meta(json!([
            {"key":"loop_n","type":"int","default":3},
            {"key":"old_text","type":"string","default":"x","role":"ocr_text",
             "asset_id":"O_old"}
        ]));
        // O_old still exists but the flow no longer uses it (finish only).
        let a = assets(json!([ocr_var("O_old", "old_text", "x")]));
        let f = flow(json!([{"op":"finish","args":{"exit":"done"}}]));
        let report = sync_ocr_var_params(&mut m, &a, &f).expect("sync");
        assert_eq!(report.removed, vec!["old_text".to_string()]);
        // The user row survives untouched; order is preserved.
        assert_eq!(m.params.len(), 1);
        assert_eq!(m.params[0].key, "loop_n");
        assert!(m.params[0].asset_id.is_none());
    }

    #[test]
    fn sync_rejects_parameter_key_conflicts() {
        // (a) A user-authored row already owns the derived key.
        let mut m = meta(json!([{"key":"group","type":"string","default":"用户自己的"}]));
        let a = assets(json!([ocr_var("O_group", "group", "御魂")]));
        let f = flow_using("O_group", "group");
        let err = sync_ocr_var_params(&mut m, &a, &f).expect_err("key conflict");
        assert!(err.contains("conflicts with a user-authored"), "{err}");

        // (b) Two used ocr_var assets share one param_key.
        let mut m = meta(json!([]));
        let a = assets(json!([
            ocr_var("O_a", "group", "甲"),
            ocr_var("O_b", "group", "乙")
        ]));
        let f = flow(json!([
            {"op":"click_target","args":{"find":[
                {"kind":"ocr_text","sel":"O_a","text_expr":"param('group')"},
                {"kind":"ocr_text","sel":"O_b","text_expr":"param('group')"}
            ]}},
            {"op":"finish","args":{"exit":"done"}}
        ]));
        let err = sync_ocr_var_params(&mut m, &a, &f).expect_err("duplicate key");
        assert!(err.contains("more than one ocr_var"), "{err}");
    }

    #[test]
    fn migrate_dry_run_plans_assets_and_skips_non_string_params() {
        let mut m = meta(json!([
            {"key":"group","type":"string","default":"御魂","role":"ocr_text","label":"预设分组"},
            {"key":"loop_n","type":"int","default":3},
            {"key":"bad","type":"string","default":7,"role":"ocr_text"}
        ]));
        let mut a = assets(json!([]));
        let report = migrate_params_to_ocr_vars(&mut a, &mut m, None, true).expect("dry");
        assert!(report.dry_run);
        assert_eq!(report.candidates.len(), 1);
        assert_eq!(report.candidates[0].param_key, "group");
        assert_eq!(report.candidates[0].asset_id, "O_group");
        assert_eq!(report.candidates[0].runtime_text, "御魂");
        assert_eq!(report.skipped.len(), 1);
        assert_eq!(report.skipped[0].param_key, "bad");
        // Nothing was mutated.
        assert!(a.items.is_empty());
        assert_eq!(m.params.len(), 3);
    }

    #[test]
    fn migrate_real_creates_assets_and_rebuilds_the_derived_rows() {
        let mut m = meta(json!([
            {"key":"group","type":"string","default":"御魂","role":"ocr_text","label":"预设分组"},
            {"key":"loop_n","type":"int","default":3}
        ]));
        let mut a = assets(json!([]));
        // Old-style flow: the OCR selector is an ordinary asset; the parameter
        // key is only used through `text_expr`.
        let f = flow(json!([
            {"op":"click_target","args":{"find":[
                {"kind":"ocr_text","sel":"旧OCR行","text_expr":"param('group')"}
            ]}},
            {"op":"finish","args":{"exit":"done"}}
        ]));
        let report =
            migrate_params_to_ocr_vars(&mut a, &mut m, Some(&f), false).expect("migrate");
        assert_eq!(report.candidates.len(), 1);
        assert!(report.sync.is_some());
        // The ocr_var asset exists with the old key/text, whole-screen ROI.
        assert_eq!(a.items.len(), 1);
        let item = &a.items[0];
        assert_eq!(item.kind, "ocr_var");
        assert_eq!(item.id, "O_group");
        assert_eq!(item.param_key.as_deref(), Some("group"));
        assert_eq!(item.runtime_text.as_deref(), Some("御魂"));
        assert_eq!(item.label.as_deref(), Some("预设分组"));
        assert_eq!(item.roi, None, "migrated ocr_var has no ROI (whole-screen OCR)");
        // Old hand-written row is gone, the derived row with the SAME key is
        // there, the unrelated user row is preserved.
        let keys: Vec<&str> = m.params.iter().map(|p| p.key.as_str()).collect();
        assert_eq!(keys, vec!["loop_n", "group"]);
        let derived = &m.params[1];
        assert_eq!(derived.asset_id.as_deref(), Some("O_group"));
        assert_eq!(derived.default, Some(json!("御魂")));
        // T8 is clean after a successful migration.
        assert!(
            derived_param_issues(&m, &a, &f).is_empty(),
            "T8: {:?}",
            derived_param_issues(&m, &a, &f)
        );
    }

    #[test]
    fn migrate_real_needs_a_flow_and_allocates_a_unique_asset_id() {
        let mut m = meta(json!([
            {"key":"group","type":"string","default":"御魂","role":"ocr_text"}
        ]));
        let mut a = assets(json!([
            {"id":"O_group","kind":"ocr_line","roi":[0,0,10,10]}
        ]));
        let err = migrate_params_to_ocr_vars(&mut a, &mut m, None, false)
            .expect_err("real migration needs the flow");
        assert!(err.contains("needs the task flow"), "{err}");
        // Dry run still previews the collision-free id.
        let report = migrate_params_to_ocr_vars(&mut a, &mut m, None, true).expect("dry");
        assert_eq!(report.candidates[0].asset_id, "O_group_2");
    }

    #[test]
    fn t8_flags_derived_row_drift_missing_and_stale_rows() {
        let a = assets(json!([ocr_var("O_group", "group", "御魂")]));
        let f = flow_using("O_group", "group");
        // Matching row: clean.
        let mut m = meta(json!([
            {"key":"group","type":"string","default":"御魂","role":"ocr_text",
             "label":"O_group","asset_id":"O_group"}
        ]));
        assert!(derived_param_issues(&m, &a, &f).is_empty());
        // Drifted default: flagged with a T8 issue pointing at the row.
        m.params[0].default = Some(json!("过期文字"));
        let issues = derived_param_issues(&m, &a, &f);
        assert_eq!(issues.len(), 1, "{issues:?}");
        assert_eq!(issues[0].code, "T8");
        assert_eq!(issues[0].path, "meta.json:params[0]");
        // Missing derived row for a used ocr_var.
        let m = meta(json!([]));
        let issues = derived_param_issues(&m, &a, &f);
        assert_eq!(issues.len(), 1, "{issues:?}");
        assert!(issues[0].msg.contains("missing"), "{issues:?}");
        // Stale row whose asset is no longer used by the flow.
        let m = meta(json!([
            {"key":"group","type":"string","default":"御魂","role":"ocr_text",
             "asset_id":"O_group"}
        ]));
        let unused = flow(json!([{"op":"finish","args":{"exit":"done"}}]));
        let issues = derived_param_issues(&m, &a, &unused);
        assert!(issues.iter().any(|i| i.msg.contains("stale")), "{issues:?}");
    }
}

