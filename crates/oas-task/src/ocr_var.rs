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

use crate::meta::{is_param_key, Meta, ParamDecl, Variable};
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

/// Build the [`Variable`] one ocr_var asset must have in `meta.variables`
/// (V2 design §2.2: `value_source=ocr_text`).
///
/// `previous` is the existing derived variable for this asset (if any); only its
/// user-editable `group`/`description` are preserved (Lead ruling 7).
fn derived_variable(asset: &AssetDecl, previous: Option<&Variable>) -> Result<Variable, String> {
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
    // A long asset id must not produce an invalid (over-limit) variable label.
    let label: String = label
        .chars()
        .take(crate::meta::PARAM_LABEL_MAX_CHARS)
        .collect();
    let mut out = Variable::ocr_text(param_key, asset.id.clone(), runtime_text);
    out.label = Some(label);
    out.description = previous.and_then(|v| v.description.clone());
    out.group = previous.and_then(|v| v.group.clone());
    Ok(out)
}

/// Derive the expected `asset_id` → [`Variable`] map for every **used** ocr_var.
///
/// Also rejects the two hard conflicts: two ocr_vars sharing one `param_key`,
/// and a derived `param_key` colliding with a user-authored literal variable.
fn expected_derived_variables(
    meta: &Meta,
    assets: &AssetsFile,
    flow: &Flow,
) -> Result<BTreeMap<String, Variable>, String> {
    let effective = meta.effective_variables();
    let ocr_vars = ocr_var_assets(assets);
    let used = used_ocr_var_ids(flow, &ocr_vars);
    let mut expected: BTreeMap<String, Variable> = BTreeMap::new(); // asset id -> row
    let mut keys: BTreeSet<String> = BTreeSet::new();
    for id in &used {
        let asset = ocr_vars
            .get(id)
            .ok_or_else(|| format!("ocr_var `{id}` is missing from assets.json"))?;
        let previous = effective
            .iter()
            .find(|v| v.asset_id.as_deref() == Some(id.as_str()));
        let row = derived_variable(asset, previous)?;
        if !is_param_key(&row.key) {
            return Err(format!(
                "ocr_var `{id}` param_key `{}` is not a valid variable key",
                row.key
            ));
        }
        if !keys.insert(row.key.clone()) {
            return Err(format!(
                "ocr_var variable key `{}` is used by more than one ocr_var asset",
                row.key
            ));
        }
        if effective
            .iter()
            .any(|v| v.asset_id.is_none() && v.key == row.key)
        {
            return Err(format!(
                "derived variable key `{}` conflicts with a user-authored variable; \
                 rename the variable or the ocr_var",
                row.key
            ));
        }
        expected.insert(id.clone(), row);
    }
    Ok(expected)
}

/// Synchronise the `value_source=ocr_text` variables of `meta.variables` with the
/// ocr_var assets and the flow (V2.0; replaces the ParamDecl-based C1′ sync).
///
/// * only `ocr_text` variables bound to an asset (`asset_id.is_some()`) are
///   added / updated / removed — user-authored literal variables are never
///   touched;
/// * an ocr_var is "used" when the flow references it as an asset or uses its
///   `param_key` in a `param()` expression;
/// * `type`/`key`/`default`/`label` come from the asset; `group` and
///   `description` are preserved from the previous variable (Lead ruling 7);
/// * key conflicts (two derived variables, or derived vs user literal) are loud
///   errors;
/// * when `meta.variables` is empty but legacy `meta.params` exists, the bridge
///   ([`Meta::variables_from_params`]) promotes the legacy rows **in memory**
///   first, so a save-time sync migrates the task and preserves user rows;
/// * the caller persists the result with [`crate::TaskBundle::write_meta`]
///   (which keeps a `.bak` backup).
pub fn sync_ocr_var_variables(
    meta: &mut Meta,
    assets: &AssetsFile,
    flow: &Flow,
) -> Result<SyncReport, String> {
    if meta.variables.is_empty() && meta.has_legacy_params() {
        meta.variables = meta.variables_from_params();
    }
    let expected = expected_derived_variables(meta, assets, flow)?;
    let mut report = SyncReport::default();
    let mut out: Vec<Variable> = Vec::with_capacity(meta.variables.len() + expected.len());
    let mut present: BTreeSet<String> = BTreeSet::new(); // asset ids kept
    for var in meta.variables.drain(..) {
        let Some(asset_id) = var.asset_id.clone() else {
            out.push(var); // user-authored literal variable: never touched
            continue;
        };
        let Some(next) = expected.get(&asset_id) else {
            // The ocr_var is no longer used by this flow: drop the derived row.
            report.removed.push(var.key);
            continue;
        };
        present.insert(asset_id);
        if var != *next {
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
    meta.variables = out;
    Ok(report)
}

/// Mirror the effective variables back to the **legacy** `meta.params` array.
///
/// Temporary V2.0a bridge: the old serve endpoints (`/task/config`, derived-row
/// protection tests) still read `meta.params` until V2.0b switches them over.
/// V2.0b removes this function together with the rest of the params bridge.
fn mirror_params_from_variables(meta: &Meta) -> Vec<ParamDecl> {
    meta.variables
        .iter()
        .map(|v| ParamDecl {
            key: v.key.clone(),
            kind: v.kind.clone(),
            default: v.default.clone(),
            min: v.min,
            max: v.max,
            label: v.label.clone(),
            description: v.description.clone(),
            enum_values: v.enum_values.clone(),
            required: v.required,
            group: v.group.clone(),
            role: v.is_ocr_text().then(|| "ocr_text".to_string()),
            asset_id: v.asset_id.clone(),
        })
        .collect()
}

/// **Deprecated** (V2.0a compat; V2.0b deletes it): ParamDecl-based sync wrapper.
///
/// Internally it runs [`sync_ocr_var_variables`] and then mirrors the result
/// back into `meta.params` so the intermediate serve build keeps working.
#[deprecated(note = "V2.0: use sync_ocr_var_variables (meta.variables is the storage)")]
pub fn sync_ocr_var_params(
    meta: &mut Meta,
    assets: &AssetsFile,
    flow: &Flow,
) -> Result<SyncReport, String> {
    if meta.variables.is_empty() && meta.has_legacy_params() {
        meta.variables = meta.variables_from_params();
    }
    let report = sync_ocr_var_variables(meta, assets, flow)?;
    meta.params = mirror_params_from_variables(meta);
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
    #[allow(deprecated)] // temporary V2.0a bridge: migration is a legacy path
    let synced = sync_ocr_var_params(meta, assets, flow)?;
    report.sync = Some(synced);
    Ok(report)
}

/// Do the **semantic** fields of a derived variable match what the asset says?
///
/// Display/UX fields (`label`/`group`/`description`, effective
/// `runner_editable`) are not drift: they are user-owned or defaulted.
fn variable_semantically_matches(v: &Variable, want: &Variable) -> bool {
    v.key == want.key
        && v.kind == want.kind
        && v.value_source() == want.value_source()
        && v.asset_id == want.asset_id
        && v.default == want.default
}

/// **T8 lint** (Lead ruling 4: error, blocks publish): every
/// `value_source=ocr_text` variable must match its bound ocr_var asset and the
/// flow (V2.0; replaces the old ParamDecl-row check).
///
/// Every issue carries a `meta.json:variables[i]` path (or `meta.json:params[i]`
/// for a legacy-only task / `meta.json:variables` for a missing row). Drift is
/// reported, never fixed: the user must re-save the asset (which triggers the
/// sync) or run the migration.
pub fn derived_variable_issues(meta: &Meta, assets: &AssetsFile, flow: &Flow) -> Vec<LintIssue> {
    let mut issues = Vec::new();
    let ocr_vars = ocr_var_assets(assets);
    let used = used_ocr_var_ids(flow, &ocr_vars);
    let effective = meta.effective_variables();
    let legacy_only = meta.variables.is_empty() && meta.has_legacy_params();
    let base = if legacy_only {
        "meta.json:params"
    } else {
        "meta.json:variables"
    };
    let expected = match expected_derived_variables(meta, assets, flow) {
        Ok(expected) => expected,
        Err(e) => {
            issues.push(LintIssue::new(
                "T8",
                "meta.json:variables",
                format!("derived ocr_var variables are inconsistent: {e}"),
            ));
            BTreeMap::new()
        }
    };
    let mut present: BTreeSet<String> = BTreeSet::new();
    let mut seen_assets: BTreeSet<&str> = BTreeSet::new();
    for (i, v) in effective.iter().enumerate() {
        let path = format!("{base}[{i}]");
        let Some(asset_id) = v.asset_id.as_deref() else {
            continue; // literal variable
        };
        if !seen_assets.insert(asset_id) {
            issues.push(LintIssue::new(
                "T8",
                path.clone(),
                format!(
                    "ocr_var `{asset_id}` is bound by more than one variable \
                     (only one ocr_text variable may reference an asset)"
                ),
            ));
        }
        let Some(asset) = ocr_vars.get(asset_id) else {
            issues.push(LintIssue::new(
                "T8",
                path.clone(),
                if assets.items.iter().any(|a| a.id == asset_id) {
                    format!(
                        "variable `{}` binds `{asset_id}`, which is not a kind=ocr_var asset",
                        v.key
                    )
                } else {
                    format!(
                        "variable `{}` binds `{asset_id}`, which is not a declared asset \
                         (re-create the variable or the ocr_var asset)",
                        v.key
                    )
                },
            ));
            continue;
        };
        if !used.contains(asset_id) {
            issues.push(LintIssue::new(
                "T8",
                path.clone(),
                format!(
                    "variable `{}` is stale: ocr_var `{asset_id}` is not referenced by \
                     flow.json (re-save the flow or delete the asset)",
                    v.key
                ),
            ));
            continue;
        }
        present.insert(asset_id.to_string());
        let Some(want) = expected.get(asset_id) else {
            continue; // already reported above
        };
        // V2.0a bridge: a legacy imported row has `runner_editable: None` while
        // the sync-generated V2 row spells `Some(true)` (same effective value for
        // an input variable), so drift is decided on the semantic fields only.
        if !variable_semantically_matches(v, want) {
            issues.push(LintIssue::new(
                "T8",
                path.clone(),
                format!(
                    "variable `{}` does not match ocr_var `{asset_id}` \
                     (expected key=`{}` type=string value_source=ocr_text default={:?}); \
                     re-save the asset to sync",
                    v.key, want.key, want.default
                ),
            ));
        }
        if asset.param_key.as_deref() != Some(v.key.as_str()) {
            issues.push(LintIssue::new(
                "T8",
                path,
                format!(
                    "variable key `{}` differs from ocr_var `{asset_id}` param_key `{}`",
                    v.key,
                    asset.param_key.as_deref().unwrap_or("")
                ),
            ));
        }
    }
    for (asset_id, want) in &expected {
        if !present.contains(asset_id) {
            issues.push(LintIssue::new(
                "T8",
                "meta.json:variables",
                format!(
                    "ocr_var `{asset_id}` is used but its ocr_text variable `{}` is missing \
                     (re-save the asset to sync)",
                    want.key
                ),
            ));
        }
    }
    issues
}

/// **Deprecated** (V2.0a compat; V2.0b deletes it): ParamDecl-row T8 wrapper.
#[deprecated(note = "V2.0: use derived_variable_issues (meta.variables is the storage)")]
pub fn derived_param_issues(meta: &Meta, assets: &AssetsFile, flow: &Flow) -> Vec<LintIssue> {
    let mut tmp = meta.clone();
    if tmp.variables.is_empty() && tmp.has_legacy_params() {
        tmp.variables = tmp.variables_from_params();
    }
    derived_variable_issues(&tmp, assets, flow)
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

    /// V2-style meta with `variables` (the new storage).
    fn meta_vars(variables: serde_json::Value) -> Meta {
        Meta::from_json(
            &json!({"schema_version":1,"name":"任务1","variables":variables}).to_string(),
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
    fn sync_adds_a_derived_variable_for_a_used_ocr_var() {
        let mut m = meta(json!([]));
        let a = assets(json!([ocr_var("O_group", "group", "御魂")]));
        let f = flow_using("O_group", "group");
        let report = sync_ocr_var_variables(&mut m, &a, &f).expect("sync");
        assert_eq!(report.added, vec!["group".to_string()]);
        assert!(report.updated.is_empty() && report.removed.is_empty());
        assert_eq!(m.variables.len(), 1);
        let var = &m.variables[0];
        assert_eq!(var.key, "group");
        assert_eq!(var.kind, "string");
        assert_eq!(var.value_source(), "ocr_text");
        assert_eq!(var.default, Some(json!("御魂")));
        assert_eq!(var.label.as_deref(), Some("O_group"));
        assert_eq!(var.asset_id.as_deref(), Some("O_group"));
        assert!(var.runner_editable(), "ocr_text rows are Runner-editable");
        // A second sync is a no-op.
        let again = sync_ocr_var_variables(&mut m, &a, &f).expect("resync");
        assert_eq!(again, SyncReport::default());
        // The deprecated compat wrapper mirrors the same rows into meta.params.
        #[allow(deprecated)]
        {
            let mut legacy = meta(json!([]));
            let report = sync_ocr_var_params(&mut legacy, &a, &f).expect("wrapper sync");
            assert_eq!(report.added, vec!["group".to_string()]);
            assert_eq!(legacy.params.len(), 1);
            assert_eq!(legacy.params[0].role.as_deref(), Some("ocr_text"));
            assert_eq!(legacy.params[0].asset_id.as_deref(), Some("O_group"));
        }
    }

    #[test]
    fn sync_keeps_the_variable_for_an_old_flow_that_only_uses_the_param_key() {
        // Migration case: the old flow still recognizes an old OCR asset and
        // only uses `param('group')`; the new ocr_var asset is not referenced
        // yet, but its derived variable must exist so the call site keeps
        // resolving.
        let mut m = meta(json!([]));
        let a = assets(json!([ocr_var("O_group", "group", "御魂")]));
        let f = flow(json!([
            {"op":"click_target","args":{"find":[
                {"kind":"ocr_text","sel":"旧OCR行","text_expr":"param('group')"}
            ]}},
            {"op":"finish","args":{"exit":"done"}}
        ]));
        let report = sync_ocr_var_variables(&mut m, &a, &f).expect("sync");
        assert_eq!(report.added, vec!["group".to_string()]);
        assert_eq!(m.variables[0].asset_id.as_deref(), Some("O_group"));
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
        let report = sync_ocr_var_variables(&mut m, &a, &f).expect("sync");
        assert_eq!(report.updated, vec!["group".to_string()]);
        let var = &m.variables[0];
        assert_eq!(var.default, Some(json!("结界")), "default follows runtime_text");
        assert_eq!(var.group.as_deref(), Some("识别文字"), "group is user-owned");
        assert_eq!(var.description.as_deref(), Some("游戏内分组名"));
        assert_eq!(var.label.as_deref(), Some("O_group"), "label follows the asset/id");
        assert_eq!(var.required, None, "derived rows always have a default");
    }

    #[test]
    fn sync_removes_a_stale_derived_variable_and_never_touches_user_rows() {
        let mut m = meta(json!([
            {"key":"loop_n","type":"int","default":3},
            {"key":"old_text","type":"string","default":"x","role":"ocr_text",
             "asset_id":"O_old"}
        ]));
        // O_old still exists but the flow no longer uses it (finish only).
        let a = assets(json!([ocr_var("O_old", "old_text", "x")]));
        let f = flow(json!([{"op":"finish","args":{"exit":"done"}}]));
        let report = sync_ocr_var_variables(&mut m, &a, &f).expect("sync");
        assert_eq!(report.removed, vec!["old_text".to_string()]);
        // The user literal variable survives untouched; order is preserved.
        assert_eq!(m.variables.len(), 1);
        assert_eq!(m.variables[0].key, "loop_n");
        assert!(m.variables[0].asset_id.is_none());
    }

    #[test]
    fn sync_rejects_variable_key_conflicts() {
        // (a) A user-authored literal variable already owns the derived key.
        let mut m = meta(json!([{"key":"group","type":"string","default":"用户自己的"}]));
        let a = assets(json!([ocr_var("O_group", "group", "御魂")]));
        let f = flow_using("O_group", "group");
        let err = sync_ocr_var_variables(&mut m, &a, &f).expect_err("key conflict");
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
        let err = sync_ocr_var_variables(&mut m, &a, &f).expect_err("duplicate key");
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
            derived_variable_issues(&m, &a, &f).is_empty(),
            "T8: {:?}",
            derived_variable_issues(&m, &a, &f)
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
    fn t8_flags_derived_variable_drift_missing_and_stale_rows() {
        let a = assets(json!([ocr_var("O_group", "group", "御魂")]));
        let f = flow_using("O_group", "group");
        // Matching V2 variable: clean.
        let mut m = meta_vars(json!([
            {"key":"group","type":"string","value_source":"ocr_text","default":"御魂",
             "label":"O_group","asset_id":"O_group","runner_editable":true}
        ]));
        assert!(derived_variable_issues(&m, &a, &f).is_empty(), "{:?}", derived_variable_issues(&m, &a, &f));
        // Drifted default: flagged with a T8 issue pointing at the variable.
        m.variables[0].default = Some(json!("过期文字"));
        let issues = derived_variable_issues(&m, &a, &f);
        assert_eq!(issues.len(), 1, "{issues:?}");
        assert_eq!(issues[0].code, "T8");
        assert_eq!(issues[0].path, "meta.json:variables[0]");
        // Missing derived variable for a used ocr_var.
        let m = meta_vars(json!([]));
        let issues = derived_variable_issues(&m, &a, &f);
        assert_eq!(issues.len(), 1, "{issues:?}");
        assert!(issues[0].msg.contains("missing"), "{issues:?}");
        // Stale variable whose asset is no longer used by the flow.
        let m = meta_vars(json!([
            {"key":"group","type":"string","value_source":"ocr_text","default":"御魂",
             "asset_id":"O_group"}
        ]));
        let unused = flow(json!([{"op":"finish","args":{"exit":"done"}}]));
        let issues = derived_variable_issues(&m, &a, &unused);
        assert!(issues.iter().any(|i| i.msg.contains("stale")), "{issues:?}");
        // Legacy params-only meta is bridged in memory and is clean (paths are
        // reported under meta.json:params while the file is still legacy).
        let legacy = meta(json!([
            {"key":"group","type":"string","default":"御魂","role":"ocr_text",
             "label":"O_group","asset_id":"O_group"}
        ]));
        assert!(derived_variable_issues(&legacy, &a, &f).is_empty());
        // A variable binding a missing/non-ocr_var asset is flagged.
        let bad = meta_vars(json!([
            {"key":"group","type":"string","value_source":"ocr_text","default":"御魂",
             "asset_id":"NOT_THERE"}
        ]));
        let issues = derived_variable_issues(&bad, &a, &f);
        assert!(
            issues.iter().any(|i| i.msg.contains("not a declared asset")),
            "{issues:?}"
        );
    }
}

