//! seq138 V2: global map `<assets-dir>/map.json` — **locations / landmarks /
//! paths**.
//!
//! * `locations` is keyed by the location name (globally unique; a landmark
//!   references another location by that name);
//! * a landmark is only `{name, path}` — no target/id/priority/frames/hub;
//! * `path` is `{action:"click", asset, wait?}` (V2: click only);
//! * `fallback_task` (default `返回庭院`) is invoked by the runtime when the
//!   current location is unknown / no route exists / execution fails.
//!
//! V1 (`nodes/edges/parent/return_asset`) migration exists as **code + tests
//! only**: loading a v1 file converts it in memory for read compatibility, but
//! nothing is auto-rewritten on disk (seq138 §8).

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::AssetsFile;

pub const MAP_SCHEMA_VERSION: u32 = 2;
pub const MAX_MAP_LOCATIONS: usize = 500;
pub const MAX_MAP_LANDMARKS: usize = 2000;
pub const MAX_LOCATION_FEATURES: usize = 6;

fn default_schema() -> u32 {
    MAP_SCHEMA_VERSION
}
fn default_one() -> u32 {
    1
}
fn default_click() -> String {
    "click".to_string()
}
fn default_fallback_task() -> String {
    "返回庭院".to_string()
}
fn default_settle() -> u32 {
    350
}
fn default_stable() -> u32 {
    0
}
fn default_timeout() -> u32 {
    3000
}
fn default_poll() -> u32 {
    150
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MapFeature {
    OcrText {
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        roi: Option<[i32; 4]>,
    },
    Asset {
        asset: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MapRecognize {
    #[serde(default = "default_one")]
    pub min_hits: u32,
    pub features: Vec<MapFeature>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MapWait {
    #[serde(default = "default_settle")]
    pub settle_ms: u32,
    #[serde(default = "default_stable")]
    pub stable_ms: u32,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u32,
    #[serde(default = "default_poll")]
    pub poll_ms: u32,
}

impl Default for MapWait {
    fn default() -> Self {
        Self {
            settle_ms: default_settle(),
            stable_ms: default_stable(),
            timeout_ms: default_timeout(),
            poll_ms: default_poll(),
        }
    }
}

/// One click path (V2 only supports `action="click"`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MapPath {
    #[serde(default = "default_click")]
    pub action: String,
    pub asset: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wait: Option<MapWait>,
}

/// A landmark: reference to another location + path.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Landmark {
    pub name: String,
    pub path: MapPath,
}

/// One location (= game page).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MapLocation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub recognize: MapRecognize,
    #[serde(default)]
    pub return_landmarks: Vec<Landmark>,
    #[serde(default)]
    pub outbound_landmarks: Vec<Landmark>,
}

/// The global V2 map file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MapFile {
    #[serde(default = "default_schema")]
    pub schema_version: u32,
    #[serde(default = "default_fallback_task")]
    pub fallback_task: String,
    #[serde(default)]
    pub locations: BTreeMap<String, MapLocation>,
}

/// One validation finding (`warning == false` = blocks save/publish).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapIssue {
    pub path: String,
    pub message: String,
    pub warning: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MapValidation {
    pub errors: Vec<MapIssue>,
    pub warnings: Vec<MapIssue>,
}

impl MapValidation {
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }
    pub fn error(&mut self, path: impl Into<String>, message: impl Into<String>) {
        self.errors.push(MapIssue {
            path: path.into(),
            message: message.into(),
            warning: false,
        });
    }
    pub fn warn(&mut self, path: impl Into<String>, message: impl Into<String>) {
        self.warnings.push(MapIssue {
            path: path.into(),
            message: message.into(),
            warning: true,
        });
    }
    pub fn error_text(&self) -> String {
        self.errors
            .iter()
            .map(|e| format!("{}: {}", e.path, e.message))
            .collect::<Vec<_>>()
            .join("; ")
    }
}

pub fn map_path(assets_dir: &Path) -> PathBuf {
    assets_dir.join("map.json")
}

impl MapFile {
    pub fn location_names(&self) -> Vec<&str> {
        self.locations.keys().map(String::as_str).collect()
    }
    /// Backward-compatible alias used by older call sites/tests.
    pub fn node_ids(&self) -> Vec<&str> {
        self.location_names()
    }

    /// Convert to the runtime neutral view (sorted by location name, matching
    /// the JSON object key order).
    pub fn to_runtime(&self) -> oas_runtime::map::MapConfig {
        use oas_runtime::map as rt;
        let to_wait = |w: &MapWait| rt::MapWait {
            settle_ms: w.settle_ms,
            stable_ms: w.stable_ms,
            timeout_ms: w.timeout_ms,
            poll_ms: w.poll_ms,
        };
        let to_lm = |lm: &Landmark, is_return: bool, order: usize| rt::Landmark {
            name: lm.name.clone(),
            path: rt::MapPath {
                asset: lm.path.asset.clone(),
                wait: lm
                    .path
                    .wait
                    .as_ref()
                    .map(to_wait)
                    .unwrap_or_default(),
            },
            is_return,
            order,
        };
        let locations = self
            .locations
            .iter()
            .map(|(name, loc)| rt::Location {
                name: name.clone(),
                label: loc.label.clone(),
                min_hits: loc.recognize.min_hits.max(1),
                features: loc
                    .recognize
                    .features
                    .iter()
                    .map(|f| match f {
                        MapFeature::OcrText { text, roi } => rt::MapFeature::OcrText {
                            text: text.clone(),
                            roi: roi.map(|r| oas_types::Rect {
                                x: r[0],
                                y: r[1],
                                w: r[2],
                                h: r[3],
                            }),
                        },
                        MapFeature::Asset { asset } => rt::MapFeature::Asset {
                            asset: asset.clone(),
                        },
                    })
                    .collect(),
                return_landmarks: loc
                    .return_landmarks
                    .iter()
                    .enumerate()
                    .map(|(i, lm)| to_lm(lm, true, i))
                    .collect(),
                outbound_landmarks: loc
                    .outbound_landmarks
                    .iter()
                    .enumerate()
                    .map(|(i, lm)| to_lm(lm, false, i))
                    .collect(),
            })
            .collect();
        rt::MapConfig {
            fallback_task: self.fallback_task.clone(),
            locations,
        }
    }

    /// Validate the whole V2 map against the global asset store.
    ///
    /// Errors block save; warnings are display-only. The second argument is kept
    /// for source compatibility (V1 used it for sample `frames`, which V2 removed).
    pub fn validate(&self, assets: &AssetsFile, _frame_ids: &BTreeSet<u64>) -> MapValidation {
        let mut issues = MapValidation::default();
        if self.schema_version != MAP_SCHEMA_VERSION {
            issues.error(
                "$.schema_version",
                format!(
                    "unsupported schema_version {} (expected {MAP_SCHEMA_VERSION})",
                    self.schema_version
                ),
            );
        }
        if self.fallback_task.trim().is_empty() {
            issues.error("$.fallback_task", "fallback_task 不能为空字符串");
        }
        if self.locations.len() > MAX_MAP_LOCATIONS {
            issues.error(
                "$.locations",
                format!("所在地数 {} 超过上限 {MAX_MAP_LOCATIONS}", self.locations.len()),
            );
        }
        let known_assets: HashSet<&str> = assets
            .items
            .iter()
            .map(|a| a.id.as_str())
            .collect();
        let collections: HashSet<&str> =
            assets.collections.iter().map(|c| c.id.as_str()).collect();
        let asset_exists = |id: &str| known_assets.contains(id) || collections.contains(id);
        let names: BTreeSet<&str> = self.locations.keys().map(String::as_str).collect();
        let mut total_landmarks = 0usize;
        for (name, loc) in &self.locations {
            let p = format!("$.locations[{name:?}]");
            if name.trim().is_empty()
                || name.len() > 64
                || name.chars().any(char::is_control)
            {
                issues.error(
                    format!("{p}"),
                    format!(
                        "非法所在地名称 `{name}`（不能为空/控制字符，长度需 <=64）"
                    ),
                );
            }
            if loc.recognize.features.is_empty() {
                issues.error(
                    format!("{p}.recognize.features"),
                    "至少需要 1 个识别特征",
                );
            }
            if loc.recognize.features.len() > MAX_LOCATION_FEATURES {
                issues.error(
                    format!("{p}.recognize.features"),
                    format!(
                        "识别特征数 {} 超过上限 {MAX_LOCATION_FEATURES}",
                        loc.recognize.features.len()
                    ),
                );
            }
            let n = loc.recognize.features.len() as u32;
            if loc.recognize.min_hits < 1 || loc.recognize.min_hits > n.max(1) {
                issues.error(
                    format!("{p}.recognize.min_hits"),
                    format!("min_hits {} 必须在 1..={n}", loc.recognize.min_hits),
                );
            }
            for (fi, f) in loc.recognize.features.iter().enumerate() {
                match f {
                    MapFeature::OcrText { text, roi } => {
                        if text.trim().is_empty() {
                            issues.error(
                                format!("{p}.recognize.features[{fi}].text"),
                                "OCR 特征文字不能为空",
                            );
                        }
                        if let Some(r) = roi {
                            if r[2] <= 0 || r[3] <= 0 {
                                issues.error(
                                    format!("{p}.recognize.features[{fi}].roi"),
                                    "ROI 宽高必须为正",
                                );
                            }
                        }
                    }
                    MapFeature::Asset { asset } => {
                        if !asset_exists(asset) {
                            issues.error(
                                format!("{p}.recognize.features[{fi}].asset"),
                                format!("识别特征素材 `{asset}` 不存在"),
                            );
                        }
                    }
                }
            }
            for (group, list) in [
                ("return_landmarks", &loc.return_landmarks),
                ("outbound_landmarks", &loc.outbound_landmarks),
            ] {
                total_landmarks += list.len();
                let mut seen_targets: BTreeSet<&str> = BTreeSet::new();
                for (li, lm) in list.iter().enumerate() {
                    let lp = format!("{p}.{group}[{li}]");
                    if lm.name.trim().is_empty() {
                        issues.error(format!("{lp}.name"), "路标名称不能为空");
                    } else if lm.name == *name {
                        issues.error(format!("{lp}.name"), "路标不能指向自身所在地");
                    } else if !names.contains(lm.name.as_str()) {
                        issues.error(
                            format!("{lp}.name"),
                            format!("路标目标所在地 `{}` 不存在", lm.name),
                        );
                    }
                    if !seen_targets.insert(lm.name.as_str()) {
                        issues.warn(
                            format!("{lp}.name"),
                            format!("同一组内重复路标目标 `{}`", lm.name),
                        );
                    }
                    if lm.path.action != "click" {
                        issues.error(
                            format!("{lp}.path.action"),
                            format!(
                                "V2 只支持 action=click（得到 `{}`）",
                                lm.path.action
                            ),
                        );
                    }
                    if !asset_exists(&lm.path.asset) {
                        issues.error(
                            format!("{lp}.path.asset"),
                            format!("路径素材 `{}` 不存在", lm.path.asset),
                        );
                    }
                    if let Some(w) = &lm.path.wait {
                        if w.settle_ms > 5000 {
                            issues.error(
                                format!("{lp}.path.wait.settle_ms"),
                                "settle_ms 必须在 0..=5000",
                            );
                        }
                        if w.stable_ms > 2000 {
                            issues.error(
                                format!("{lp}.path.wait.stable_ms"),
                                "stable_ms 必须在 0..=2000",
                            );
                        }
                        if !(100..=15_000).contains(&w.timeout_ms) {
                            issues.error(
                                format!("{lp}.path.wait.timeout_ms"),
                                "timeout_ms 必须在 100..=15000",
                            );
                        }
                        if !(50..=1000).contains(&w.poll_ms) {
                            issues.error(
                                format!("{lp}.path.wait.poll_ms"),
                                "poll_ms 必须在 50..=1000",
                            );
                        }
                    }
                }
            }
        }
        if total_landmarks > MAX_MAP_LANDMARKS {
            issues.error(
                "$.locations",
                format!("路标总数 {total_landmarks} 超过上限 {MAX_MAP_LANDMARKS}"),
            );
        }
        if self.locations.is_empty() {
            issues.warn("$.locations", "地图还没有任何所在地");
        }
        issues
    }
}

/// Load `map.json`: V2 native; V1 is migrated **in memory only** (never
/// rewritten on disk) for read compatibility. Missing file = `Ok(None)`.
pub fn load(assets_dir: &Path) -> Result<Option<MapFile>, String> {
    let path = map_path(assets_dir);
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("read {}: {e}", path.display())),
    };
    let map = parse_compat_json(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(Some(map))
}

/// Parse V2 JSON, or a V1 object (detected by schema_version=1 / `nodes`) into
/// the V2 in-memory shape via [`migrate_v1`].
pub fn parse_compat_json(text: &str) -> Result<MapFile, String> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|e| format!("invalid map JSON: {e}"))?;
    parse_compat_value(&value)
}

pub fn parse_compat_value(value: &serde_json::Value) -> Result<MapFile, String> {
    let version = value
        .get("schema_version")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let is_v1 = version == 1 || value.get("nodes").is_some();
    if is_v1 {
        let v1: MapFileV1 = serde_json::from_value(value.clone())
            .map_err(|e| format!("invalid v1 map JSON: {e}"))?;
        let (map, _warnings) = migrate_v1_owned(&v1);
        return Ok(map);
    }
    let map: MapFile =
        serde_json::from_value(value.clone()).map_err(|e| format!("invalid v2 map JSON: {e}"))?;
    if map.schema_version != MAP_SCHEMA_VERSION {
        return Err(format!(
            "unsupported map schema_version {} (expected {MAP_SCHEMA_VERSION})",
            map.schema_version
        ));
    }
    Ok(map)
}

/// Save V2 atomically (`map.json.tmp` → rename) with `.bak` of the previous file.
pub fn save_atomic(assets_dir: &Path, map: &MapFile) -> Result<PathBuf, String> {
    std::fs::create_dir_all(assets_dir)
        .map_err(|e| format!("create {}: {e}", assets_dir.display()))?;
    let path = map_path(assets_dir);
    if path.is_file() {
        let bak = path.with_extension("json.bak");
        std::fs::copy(&path, &bak).map_err(|e| format!("backup {}: {e}", path.display()))?;
    }
    let text = serde_json::to_string_pretty(map)
        .map_err(|e| format!("serialize map: {e}"))?
        + "\n";
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, &path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("rename {}: {e}", path.display())
    })?;
    Ok(path)
}

// ---------------------------------------------------------------------------
// V1 types + in-memory migration (code/tests only; user data is never touched)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct MapFileV1 {
    #[serde(default)]
    schema_version: u32,
    #[serde(default)]
    hub: String,
    #[serde(default)]
    default_return_asset: Option<String>,
    #[serde(default)]
    nodes: Vec<V1Node>,
    #[serde(default)]
    edges: Vec<V1Edge>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct V1Node {
    id: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    parent: Option<String>,
    #[serde(default)]
    return_asset: Option<String>,
    #[serde(default = "v1_recognize_default")]
    recognize: MapRecognize,
    #[serde(default)]
    priority: i32,
    #[serde(default)]
    frames: Vec<u64>,
}

fn v1_recognize_default() -> MapRecognize {
    MapRecognize {
        min_hits: 1,
        features: vec![],
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct V1Edge {
    from: String,
    #[serde(default)]
    action: V1Action,
    to: String,
    #[serde(default)]
    wait: Option<MapWait>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum V1Action {
    ClickAsset {
        asset: String,
        #[serde(default)]
        hold_ms: Option<u32>,
    },
    Key {
        keycode: u32,
    },
    Swipe {
        from: [i32; 2],
        to: [i32; 2],
        ms: u32,
    },
}

impl Default for V1Action {
    fn default() -> Self {
        Self::ClickAsset {
            asset: String::new(),
            hold_ms: None,
        }
    }
}

fn v2_path(asset: &str, wait: Option<MapWait>) -> MapPath {
    MapPath {
        action: "click".to_string(),
        asset: asset.to_string(),
        wait,
    }
}

fn migrate_v1_owned(v1: &MapFileV1) -> (MapFile, Vec<String>) {
    let mut warnings = Vec::new();
    let mut locations: BTreeMap<String, MapLocation> = BTreeMap::new();
    let ids: BTreeSet<&str> = v1.nodes.iter().map(|n| n.id.as_str()).collect();
    // Only a supported click_asset explicit edge overrides the parent-derived
    // return landmark; a key/swipe edge is dropped and must not suppress it.
    let mut explicit: BTreeSet<(&str, &str)> = BTreeSet::new();
    for edge in &v1.edges {
        if matches!(edge.action, V1Action::ClickAsset { .. }) {
            explicit.insert((edge.from.as_str(), edge.to.as_str()));
        }
    }
    for node in &v1.nodes {
        let mut return_landmarks = Vec::new();
        let mut outbound_landmarks = Vec::new();
        let mut seen: BTreeSet<String> = BTreeSet::new();
        if let Some(parent) = node.parent.as_deref() {
            let derived_asset = node
                .return_asset
                .as_deref()
                .or(v1.default_return_asset.as_deref());
            if explicit.contains(&(node.id.as_str(), parent)) {
                // explicit edge wins
            } else if let Some(asset) = derived_asset {
                return_landmarks.push(Landmark {
                    name: parent.to_string(),
                    path: v2_path(asset, None),
                });
                seen.insert(parent.to_string());
            } else {
                warnings.push(format!(
                    "v1 node `{}`: parent `{parent}` has no return_asset/default_return_asset; \
                     no return landmark was generated",
                    node.id
                ));
            }
        }
        for edge in v1.edges.iter().filter(|e| e.from == node.id) {
            if !ids.contains(edge.to.as_str()) {
                warnings.push(format!(
                    "v1 edge {}→{}: target node is missing; dropped",
                    edge.from, edge.to
                ));
                continue;
            }
            let (asset, wait) = match &edge.action {
                V1Action::ClickAsset { asset, .. } => (asset.clone(), edge.wait.clone()),
                V1Action::Key { .. } => {
                    warnings.push(format!(
                        "v1 edge {}→{} uses key; V2 path only supports click; dropped",
                        edge.from, edge.to
                    ));
                    continue;
                }
                V1Action::Swipe { .. } => {
                    warnings.push(format!(
                        "v1 edge {}→{} uses swipe; V2 path only supports click; dropped",
                        edge.from, edge.to
                    ));
                    continue;
                }
            };
            let target = Landmark {
                name: edge.to.clone(),
                path: v2_path(&asset, wait),
            };
            let is_return = node.parent.as_deref() == Some(edge.to.as_str());
            if is_return {
                return_landmarks.retain(|lm| lm.name != edge.to);
                return_landmarks.insert(0, target);
            } else if seen.insert(edge.to.clone()) {
                outbound_landmarks.push(target);
            }
        }
        let label = node
            .name
            .clone()
            .filter(|n| !n.is_empty() && *n != node.id);
        locations.insert(
            node.id.clone(),
            MapLocation {
                label,
                recognize: node.recognize.clone(),
                return_landmarks,
                outbound_landmarks,
            },
        );
    }
    if !v1.hub.trim().is_empty() {
        warnings.push(format!(
            "v1 hub `{}` dropped: V2 uses the explicit `fallback_task` instead",
            v1.hub
        ));
    }
    if v1.nodes.iter().any(|n| n.priority != 0) {
        warnings.push("v1 priority dropped: V2 identification uses features/min_hits".to_string());
    }
    if v1.nodes.iter().any(|n| !n.frames.is_empty()) {
        warnings.push("v1 frames dropped: V2 uses the workbench current frame".to_string());
    }
    (
        MapFile {
            schema_version: MAP_SCHEMA_VERSION,
            fallback_task: default_fallback_task(),
            locations,
        },
        warnings,
    )
}

/// Public V1→V2 migration used by tests/tools (never writes to disk).
pub fn migrate_v1_json(value: &serde_json::Value) -> Result<(MapFile, Vec<String>), String> {
    let v1: MapFileV1 =
        serde_json::from_value(value.clone()).map_err(|e| format!("invalid v1 map JSON: {e}"))?;
    Ok(migrate_v1_owned(&v1))
}

/// Alias of [`migrate_v1_json`].
pub fn migrate_v1(value: &serde_json::Value) -> Result<(MapFile, Vec<String>), String> {
    migrate_v1_json(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDir(std::path::PathBuf);
    impl TempDir {
        fn new(tag: &str) -> Self {
            let p = std::env::temp_dir().join(format!(
                "oas-task-v2map-{}-{tag}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir_all(&p).unwrap();
            Self(p)
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

    fn assets() -> AssetsFile {
        serde_json::from_value(serde_json::json!({
            "schema_version": 1,
            "design_resolution": [1280, 720],
            "items": [
                {"id":"I_御魂入口","kind":"template","file":"images/a001.png","roi":[0,0,10,10]},
                {"id":"I_十层入口","kind":"template","file":"images/a002.png","roi":[0,0,10,10]},
                {"id":"I_BACK","kind":"template","file":"images/a003.png","roi":[0,0,10,10]}
            ],
            "collections": []
        }))
        .expect("assets")
    }

    fn sample() -> MapFile {
        serde_json::from_value(serde_json::json!({
            "schema_version": 2,
            "fallback_task": "返回庭院",
            "locations": {
                "庭院": {
                    "recognize": {"min_hits":1, "features":[{"kind":"ocr_text","text":"庭院"}]},
                    "return_landmarks": [],
                    "outbound_landmarks": [
                        {"name":"御魂","path":{"action":"click","asset":"I_御魂入口",
                          "wait":{"settle_ms":600,"timeout_ms":3000}}}
                    ]
                },
                "御魂": {
                    "recognize": {"min_hits":2, "features":[
                        {"kind":"ocr_text","text":"御魂"},{"kind":"asset","asset":"I_十层入口"}]},
                    "return_landmarks": [{"name":"庭院","path":{"action":"click","asset":"I_BACK"}}],
                    "outbound_landmarks": [{"name":"御魂十层","path":{"action":"click","asset":"I_十层入口"}}]
                },
                "御魂十层": {
                    "recognize": {"min_hits":1, "features":[{"kind":"ocr_text","text":"十层"}]},
                    "return_landmarks": [{"name":"御魂","path":{"action":"click","asset":"I_BACK"}}]
                }
            }
        }))
        .expect("v2 map")
    }

    #[test]
    fn v2_map_roundtrips_validates_and_plans() {
        let td = TempDir::new("map-v2");
        let dir = td.path().join("assets");
        let map = sample();
        let v = map.validate(&assets(), &BTreeSet::new());
        assert!(v.is_ok(), "{:?}", v.errors);
        let path = save_atomic(&dir, &map).expect("save");
        assert!(path.is_file());
        let loaded = load(&dir).expect("load").expect("present");
        assert_eq!(loaded, map);
        save_atomic(&dir, &map).expect("save2");
        assert!(dir.join("map.json.bak").is_file());
        let rt = map.to_runtime();
        assert_eq!(rt.fallback_task, "返回庭院");
        assert_eq!(rt.location_names(), vec!["庭院", "御魂", "御魂十层"]);
        let route = oas_runtime::map::plan_route(&rt, "庭院", "御魂十层").expect("route");
        assert_eq!(route.len(), 2);
        assert_eq!(route[0].to, "御魂");
    }

    #[test]
    fn fallback_task_defaults_and_must_be_non_empty() {
        let map: MapFile = serde_json::from_value(serde_json::json!({
            "schema_version": 2,
            "locations": {
                "庭院": {"recognize":{"min_hits":1,"features":[
                    {"kind":"ocr_text","text":"庭院"}]}}
            }
        }))
        .expect("v2 without fallback_task");
        assert_eq!(map.fallback_task, "返回庭院");
        let v = map.validate(&assets(), &BTreeSet::new());
        assert!(v.is_ok(), "{:?}", v.errors);
        let mut empty = map;
        empty.fallback_task = "   ".into();
        let err = empty.validate(&assets(), &BTreeSet::new()).error_text();
        assert!(err.contains("fallback_task"), "{err}");
    }

    #[test]
    fn v2_validation_matrix() {
        let mut map = sample();
        map.fallback_task = "  ".into();
        map.locations.get_mut("御魂").unwrap().return_landmarks[0].name = "御魂".into();
        map.locations.get_mut("庭院").unwrap().outbound_landmarks[0].path.asset = "NOPE".into();
        map.locations.get_mut("御魂十层").unwrap().outbound_landmarks.push(Landmark {
            name: "不存在".into(),
            path: MapPath {
                action: "key".into(),
                asset: "I_BACK".into(),
                wait: None,
            },
        });
        let v = map.validate(&assets(), &BTreeSet::new());
        assert!(!v.is_ok());
        let text = v.error_text();
        for needle in ["fallback_task", "自身", "NOPE", "不存在", "action=click"] {
            assert!(text.contains(needle), "missing `{needle}` in {text}");
        }
    }

    #[test]
    fn v1_migration_is_in_memory_only_and_classifies_landmarks() {
        let td = TempDir::new("map-v1");
        let dir = td.path().join("assets");
        std::fs::create_dir_all(&dir).unwrap();
        let v1 = serde_json::json!({
            "schema_version": 1, "hub": "庭院", "default_return_asset": "I_BACK",
            "nodes": [
                {"id":"庭院","recognize":{"min_hits":1,"features":[{"kind":"ocr_text","text":"庭院"}]}},
                {"id":"御魂","parent":"庭院","recognize":{"min_hits":1,
                  "features":[{"kind":"ocr_text","text":"御魂"}]},"priority":3,"frames":[9]},
                {"id":"御魂十层","parent":"御魂","return_asset":"I_BACK",
                 "recognize":{"min_hits":1,"features":[{"kind":"ocr_text","text":"十层"}]}}
            ],
            "edges": [
                {"from":"庭院","action":{"kind":"click_asset","asset":"I_御魂入口"},"to":"御魂"},
                {"from":"御魂","action":{"kind":"key","keycode":4},"to":"庭院"}
            ]
        });
        let text = serde_json::to_string(&v1).unwrap();
        std::fs::write(dir.join("map.json"), &text).unwrap();
        let loaded = load(&dir).expect("load").expect("v1 present");
        assert_eq!(loaded.schema_version, 2);
        assert_eq!(loaded.fallback_task, "返回庭院");
        // v1 parent-derived return + explicit outbound.
        let a = loaded.locations.get("庭院").unwrap();
        assert_eq!(a.outbound_landmarks[0].name, "御魂");
        let b = loaded.locations.get("御魂").unwrap();
        assert_eq!(b.return_landmarks[0].name, "庭院");
        // key edge dropped; hub/priority/frames warnings are code-level only.
        let (migrated, warnings) =
            migrate_v1_json(&v1).expect("migrate");
        assert_eq!(migrated, loaded);
        assert!(warnings.iter().any(|w| w.contains("hub")), "{warnings:?}");
        assert!(warnings.iter().any(|w| w.contains("priority")), "{warnings:?}");
        // No auto-rewrite: the file still contains schema_version 1.
        let disk = std::fs::read_to_string(dir.join("map.json")).unwrap();
        assert!(disk.contains("\"schema_version\":1"), "v1 file must not be rewritten");
    }
}
