//! seq136 D1: the ONE global map file (`<assets-dir>/map.json`).
//!
//! Schema/validation/serde live here (`oas-task` owns user JSON); the runtime
//! consumes the neutral view via [`MapFile::to_runtime`]. The map is a global
//! user file: atomic write + `.bak`, never auto-migrated or rewritten.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{validate_id, AssetsFile};

pub const MAP_SCHEMA_VERSION: u32 = 1;
pub const MAX_MAP_NODES: usize = 500;
pub const MAX_MAP_EDGES: usize = 2000;
pub const MAX_NODE_FEATURES: usize = 6;
pub const MAX_PARENT_DEPTH: usize = 16;

fn default_schema() -> u32 {
    MAP_SCHEMA_VERSION
}
fn default_one() -> u32 {
    1
}
fn default_settle() -> u32 {
    350
}
fn default_timeout() -> u32 {
    3000
}
fn default_poll() -> u32 {
    150
}

fn default_stable() -> u32 {
    0
}

/// One recognition feature of a map node.
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

/// `recognize` block of a node.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MapRecognize {
    #[serde(default = "default_one")]
    pub min_hits: u32,
    pub features: Vec<MapFeature>,
}

/// One map node = one game page.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MapNode {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub return_asset: Option<String>,
    pub recognize: MapRecognize,
    #[serde(default)]
    pub priority: i32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub frames: Vec<u64>,
}

/// One edge action (V1: click_asset / key / swipe).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MapAction {
    ClickAsset {
        asset: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
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

/// Per-edge wait/verification parameters.
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

/// One directed transition. No id: array order is the identity/order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MapEdge {
    pub from: String,
    pub action: MapAction,
    pub to: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wait: Option<MapWait>,
}

/// The global map file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MapFile {
    #[serde(default = "default_schema")]
    pub schema_version: u32,
    pub hub: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_return_asset: Option<String>,
    #[serde(default)]
    pub nodes: Vec<MapNode>,
    #[serde(default)]
    pub edges: Vec<MapEdge>,
}

/// One validation finding. `warning == false` = error (blocks save/publish).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapIssue {
    pub path: String,
    pub message: String,
    pub warning: bool,
}

/// Validation report (errors + warnings).
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

/// `<assets-dir>/map.json` path.
pub fn map_path(assets_dir: &Path) -> PathBuf {
    assets_dir.join("map.json")
}

/// Load `map.json`; missing file = `Ok(None)` (flow without navigate is fine).
/// Bad JSON / unknown schema = loud error with the file path.
pub fn load(assets_dir: &Path) -> Result<Option<MapFile>, String> {
    let path = map_path(assets_dir);
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("read {}: {e}", path.display())),
    };
    let map: MapFile = serde_json::from_str(&text)
        .map_err(|e| format!("{}: invalid map JSON: {e}", path.display()))?;
    if map.schema_version != MAP_SCHEMA_VERSION {
        return Err(format!(
            "{}: unsupported map schema_version {} (expected {MAP_SCHEMA_VERSION})",
            path.display(),
            map.schema_version
        ));
    }
    Ok(Some(map))
}

/// Save atomically (`map.json.tmp` → rename) with a `.bak` of the previous file.
/// Validation is the caller's job (`validate()`), since it needs assets/frames.
pub fn save_atomic(assets_dir: &Path, map: &MapFile) -> Result<PathBuf, String> {
    std::fs::create_dir_all(assets_dir)
        .map_err(|e| format!("create {}: {e}", assets_dir.display()))?;
    let path = map_path(assets_dir);
    if path.is_file() {
        let bak = path.with_extension("json.bak");
        std::fs::copy(&path, &bak)
            .map_err(|e| format!("backup {}: {e}", path.display()))?;
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

impl MapFile {
    pub fn node_ids(&self) -> Vec<&str> {
        self.nodes.iter().map(|n| n.id.as_str()).collect()
    }

    /// Convert to the runtime's neutral view.
    pub fn to_runtime(&self) -> oas_runtime::map::MapConfig {
        use oas_runtime::map as rt;
        let nodes = self
            .nodes
            .iter()
            .map(|n| rt::MapNode {
                id: n.id.clone(),
                name: n.name.clone(),
                parent: n.parent.clone(),
                return_asset: n.return_asset.clone(),
                min_hits: n.recognize.min_hits.max(1),
                features: n
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
                priority: n.priority,
            })
            .collect();
        let edges = self
            .edges
            .iter()
            .enumerate()
            .map(|(i, e)| {
                let wait = e.wait.clone().unwrap_or_default();
                rt::MapEdge {
                    from: e.from.clone(),
                    to: e.to.clone(),
                    action: match &e.action {
                        MapAction::ClickAsset { asset, hold_ms } => rt::MapAction::ClickAsset {
                            asset: asset.clone(),
                            hold_ms: hold_ms.unwrap_or(0),
                        },
                        MapAction::Key { keycode } => rt::MapAction::Key {
                            keycode: *keycode,
                        },
                        MapAction::Swipe { from, to, ms } => rt::MapAction::Swipe {
                            from: oas_types::Point {
                                x: from[0],
                                y: from[1],
                            },
                            to: oas_types::Point { x: to[0], y: to[1] },
                            ms: *ms,
                        },
                    },
                    wait: rt::MapWait {
                        settle_ms: wait.settle_ms,
                        stable_ms: wait.stable_ms,
                        timeout_ms: wait.timeout_ms,
                        poll_ms: wait.poll_ms,
                    },
                    derived: false,
                    order: i,
                }
            })
            .collect();
        rt::MapConfig {
            hub: self.hub.clone(),
            default_return_asset: self.default_return_asset.clone(),
            nodes,
            edges,
        }
    }

    /// Validate the whole map against assets and existing frame ids.
    ///
    /// Errors block save/publish; warnings are display-only (missing sample
    /// frames, duplicate `(from,to)` edges, parent without a return asset).
    pub fn validate(&self, assets: &AssetsFile, frame_ids: &BTreeSet<u64>) -> MapValidation {
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
        if self.nodes.len() > MAX_MAP_NODES {
            issues.error(
                "$.nodes",
                format!("节点数 {} 超过上限 {MAX_MAP_NODES}", self.nodes.len()),
            );
        }
        if self.edges.len() > MAX_MAP_EDGES {
            issues.error(
                "$.edges",
                format!("边数 {} 超过上限 {MAX_MAP_EDGES}", self.edges.len()),
            );
        }
        let known_assets: HashMap<&str, &str> = assets
            .items
            .iter()
            .map(|a| (a.id.as_str(), a.kind.as_str()))
            .collect();
        let collections: BTreeSet<&str> =
            assets.collections.iter().map(|c| c.id.as_str()).collect();
        let asset_exists = |id: &str| known_assets.contains_key(id) || collections.contains(id);
        let asset_recognizable = |id: &str| {
            if collections.contains(id) {
                return true;
            }
            match known_assets.get(id) {
                Some(kind) => *kind != "region",
                None => false,
            }
        };
        // ---- nodes ------------------------------------------------------
        let mut ids: BTreeSet<&str> = BTreeSet::new();
        for (i, node) in self.nodes.iter().enumerate() {
            let p = format!("$.nodes[{i}]");
            if validate_id(&node.id).is_err() {
                issues.error(format!("{p}.id"), format!("非法节点 id `{}`", node.id));
            }
            if !ids.insert(node.id.as_str()) {
                issues.error(format!("{p}.id"), format!("重复节点 id `{}`", node.id));
            }
            if node.recognize.features.is_empty() {
                issues.error(format!("{p}.recognize.features"), "至少需要 1 个识别特征");
            }
            if node.recognize.features.len() > MAX_NODE_FEATURES {
                issues.error(
                    format!("{p}.recognize.features"),
                    format!(
                        "识别特征数 {} 超过上限 {MAX_NODE_FEATURES}",
                        node.recognize.features.len()
                    ),
                );
            }
            let n = node.recognize.features.len() as u32;
            if node.recognize.min_hits < 1 || node.recognize.min_hits > n.max(1) {
                issues.error(
                    format!("{p}.recognize.min_hits"),
                    format!("min_hits {} 必须在 1..={n}", node.recognize.min_hits),
                );
            }
            for (fi, f) in node.recognize.features.iter().enumerate() {
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
                                    format!("ROI 宽高必须为正：[{}]", r.iter().map(i32::to_string).collect::<Vec<_>>().join(",")),
                                );
                            }
                        }
                    }
                    MapFeature::Asset { asset } => {
                        if collections.contains(asset.as_str()) {
                            // collections are recognizable via candidate enumeration
                        } else if !known_assets.contains_key(asset.as_str()) {
                            issues.error(
                                format!("{p}.recognize.features[{fi}].asset"),
                                format!("识别特征素材 `{asset}` 不存在"),
                            );
                        } else if !asset_recognizable(asset) {
                            issues.error(
                                format!("{p}.recognize.features[{fi}].asset"),
                                format!("素材 `{asset}` 是 region（无识别语义），不能作为识别特征"),
                            );
                        }
                    }
                }
            }
            if let Some(parent) = &node.parent {
                if !self.nodes.iter().any(|x| &x.id == parent) {
                    issues.error(
                        format!("{p}.parent"),
                        format!("parent `{parent}` 不存在"),
                    );
                }
            }
            if let Some(ret) = &node.return_asset {
                if !asset_exists(ret) {
                    issues.error(
                        format!("{p}.return_asset"),
                        format!("返回素材 `{ret}` 不存在"),
                    );
                }
            }
            if node.parent.is_some()
                && node.return_asset.is_none()
                && self.default_return_asset.is_none()
            {
                issues.warn(
                    format!("{p}.return_asset"),
                    "节点有 parent 但没有 return_asset/default_return_asset，无法派生返回边",
                );
            }
            if !frame_ids.is_empty() {
                for f in &node.frames {
                    if !frame_ids.contains(f) {
                        issues.warn(
                            format!("{p}.frames"),
                            format!("帧 #{f} 不存在（编辑器引用，忽略）"),
                        );
                    }
                }
            }
        }
        if self.hub.trim().is_empty() {
            issues.error("$.hub", "hub 不能为空");
        } else if !ids.contains(self.hub.as_str()) {
            issues.error("$.hub", format!("hub `{}` 不是已声明节点", self.hub));
        }
        if let Some(ret) = &self.default_return_asset {
            if !asset_exists(ret) {
                issues.error(
                    "$.default_return_asset",
                    format!("默认返回素材 `{ret}` 不存在"),
                );
            }
        }
        // parent cycles / depth.
        for (i, node) in self.nodes.iter().enumerate() {
            let mut seen: BTreeSet<&str> = BTreeSet::new();
            let mut cur = Some(node.id.as_str());
            let mut depth = 0usize;
            while let Some(id) = cur {
                if depth > MAX_PARENT_DEPTH {
                    issues.error(
                        format!("$.nodes[{i}].parent"),
                        format!("parent 深度超过 {MAX_PARENT_DEPTH}"),
                    );
                    break;
                }
                if !seen.insert(id) {
                    issues.error(
                        format!("$.nodes[{i}].parent"),
                        format!("parent 成环（节点 `{id}` 重复）"),
                    );
                    break;
                }
                cur = self
                    .nodes
                    .iter()
                    .find(|n| n.id == id)
                    .and_then(|n| n.parent.as_deref());
                depth += 1;
            }
        }
        // ---- edges ------------------------------------------------------
        let mut pairs: BTreeSet<(&str, &str)> = BTreeSet::new();
        for (i, edge) in self.edges.iter().enumerate() {
            let p = format!("$.edges[{i}]");
            if !ids.contains(edge.from.as_str()) {
                issues.error(format!("{p}.from"), format!("节点 `{}` 不存在", edge.from));
            }
            if !ids.contains(edge.to.as_str()) {
                issues.error(format!("{p}.to"), format!("节点 `{}` 不存在", edge.to));
            }
            if edge.from == edge.to {
                issues.error(format!("{p}.to"), "自环边不允许（from == to）");
            }
            if !pairs.insert((edge.from.as_str(), edge.to.as_str())) {
                issues.warn(
                    format!("{p}"),
                    format!("重复边 {}→{}（允许，按声明序选择）", edge.from, edge.to),
                );
            }
            match &edge.action {
                MapAction::ClickAsset { asset, hold_ms } => {
                    if collections.contains(asset.as_str()) {
                        // collections are clickable through the single-source best match
                    } else if !known_assets.contains_key(asset.as_str()) {
                        issues.error(
                            format!("{p}.action.asset"),
                            format!("边动作素材 `{asset}` 不存在"),
                        );
                    } else if !asset_recognizable(asset) {
                        issues.error(
                            format!("{p}.action.asset"),
                            format!("素材 `{asset}` 是 region（不可识别点击 target）"),
                        );
                    }
                    if let Some(ms) = hold_ms {
                        if *ms > 10_000 {
                            issues.error(
                                format!("{p}.action.hold_ms"),
                                "hold_ms 必须在 0..=10000",
                            );
                        }
                    }
                }
                MapAction::Key { keycode } => {
                    if *keycode < 1 || *keycode > 300 {
                        issues.error(
                            format!("{p}.action.keycode"),
                            "keycode 必须在 1..=300",
                        );
                    }
                }
                MapAction::Swipe { ms, .. } => {
                    if *ms == 0 {
                        issues.error(format!("{p}.action.ms"), "swipe ms 必须 >= 1");
                    }
                }
            }
            if let Some(w) = &edge.wait {
                if w.settle_ms > 5000 {
                    issues.error(format!("{p}.wait.settle_ms"), "settle_ms 必须在 0..=5000");
                }
                if w.stable_ms > 2000 {
                    issues.error(format!("{p}.wait.stable_ms"), "stable_ms 必须在 0..=2000");
                }
                if !(100..=15_000).contains(&w.timeout_ms) {
                    issues.error(
                        format!("{p}.wait.timeout_ms"),
                        "timeout_ms 必须在 100..=15000",
                    );
                }
                if !(50..=1000).contains(&w.poll_ms) {
                    issues.error(format!("{p}.wait.poll_ms"), "poll_ms 必须在 50..=1000");
                }
            }
        }
        issues
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal temp dir (the crate's main test helper is private to its module).
    struct TempDir(std::path::PathBuf);
    impl TempDir {
        fn new(tag: &str) -> Self {
            let p = std::env::temp_dir().join(format!(
                "oas-task-map-{}-{tag}-{}",
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
                {"id":"I_BACK","kind":"template","file":"images/a003.png","roi":[0,0,10,10]},
                {"id":"I_REGION","kind":"region","roi":[0,0,10,10]}
            ],
            "collections": []
        }))
        .expect("assets")
    }

    fn sample() -> MapFile {
        serde_json::from_value(serde_json::json!({
            "schema_version": 1, "hub": "庭院", "default_return_asset": "I_BACK",
            "nodes": [
                {"id":"庭院","recognize":{"min_hits":1,"features":[{"kind":"ocr_text","text":"庭院"}]}},
                {"id":"御魂","parent":"庭院","recognize":{"min_hits":1,"features":[{"kind":"ocr_text","text":"御魂"}]}},
                {"id":"御魂十层","parent":"御魂","recognize":{"min_hits":2,"features":[
                    {"kind":"ocr_text","text":"十层"},{"kind":"asset","asset":"I_十层入口"}]},
                 "priority":5,"frames":[12]},
                {"id":"探索","parent":"庭院","recognize":{"min_hits":1,"features":[{"kind":"ocr_text","text":"探索"}]}}
            ],
            "edges": [
                {"from":"庭院","action":{"kind":"click_asset","asset":"I_御魂入口"},"to":"御魂",
                 "wait":{"settle_ms":600,"timeout_ms":3000}},
                {"from":"御魂","action":{"kind":"click_asset","asset":"I_十层入口"},"to":"御魂十层"},
                {"from":"御魂十层","action":{"kind":"key","keycode":4},"to":"御魂"}
            ]
        }))
        .expect("map")
    }

    #[test]
    fn valid_map_roundtrips_through_runtime_and_save() {
        let td = TempDir::new("map-valid");
        let dir = td.path().join("assets");
        let map = sample();
        let v = map.validate(&assets(), &BTreeSet::new());
        assert!(v.is_ok(), "{:?}", v.errors);
        let path = save_atomic(&dir, &map).expect("save");
        assert!(path.is_file());
        let loaded = load(&dir).expect("load").expect("present");
        assert_eq!(loaded, map);
        // Save again creates map.json.bak.
        save_atomic(&dir, &map).expect("save2");
        assert!(dir.join("map.json.bak").is_file());
        let rt = map.to_runtime();
        assert_eq!(rt.node_ids(), vec!["庭院", "御魂", "御魂十层", "探索"]);
        let route = oas_runtime::map::plan_route(&rt, "御魂", "御魂十层").expect("route");
        assert_eq!(route.len(), 1);
        assert_eq!(route[0].action.label(), "click_asset:I_十层入口");
    }

    #[test]
    fn validation_matrix_reports_errors_and_warnings() {
        let af = assets();
        let mut map = sample();
        // duplicate id + missing hub + self loop + bad refs/ranges.
        map.nodes.push(MapNode {
            id: "御魂".into(),
            name: None,
            parent: None,
            return_asset: None,
            recognize: MapRecognize {
                min_hits: 2,
                features: vec![MapFeature::Asset {
                    asset: "I_REGION".into(),
                }],
            },
            priority: 0,
            frames: vec![],
        });
        map.hub = "不存在".into();
        map.edges.push(MapEdge {
            from: "御魂".into(),
            action: MapAction::ClickAsset {
                asset: "没这个".into(),
                hold_ms: Some(20_000),
            },
            to: "御魂".into(),
            wait: Some(MapWait {
                settle_ms: 9000,
                stable_ms: 3000,
                timeout_ms: 50,
                poll_ms: 10,
            }),
        });
        let v = map.validate(&af, &BTreeSet::from([12]));
        assert!(!v.is_ok());
        let msgs = v.error_text();
        for needle in [
            "重复节点 id",
            "hub",
            "自环",
            "不存在",
            "region",
            "min_hits",
            "settle_ms",
            "timeout_ms",
            "poll_ms",
            "hold_ms",
        ] {
            assert!(msgs.contains(needle), "missing `{needle}` in {msgs}");
        }
    }

    #[test]
    fn parent_cycles_and_depth_are_rejected() {
        let af = assets();
        let mut map = sample();
        // cycle 御魂 <-> 御魂十层
        map.nodes[1].parent = Some("御魂十层".into());
        let v = map.validate(&af, &BTreeSet::new());
        assert!(v.error_text().contains("成环"), "{:?}", v.errors);
        // depth: chain deeper than 16
        let mut deep = sample();
        deep.nodes.clear();
        for i in 0..18 {
            deep.nodes.push(MapNode {
                id: format!("N{i}"),
                name: None,
                parent: if i == 0 {
                    None
                } else {
                    Some(format!("N{}", i - 1))
                },
                return_asset: Some("I_BACK".into()),
                recognize: MapRecognize {
                    min_hits: 1,
                    features: vec![MapFeature::OcrText {
                        text: format!("N{i}"),
                        roi: None,
                    }],
                },
                priority: 0,
                frames: vec![],
            });
        }
        deep.hub = "N0".into();
        let v = deep.validate(&af, &BTreeSet::new());
        assert!(v.error_text().contains("深度"), "{:?}", v.errors);
    }

    #[test]
    fn missing_map_file_is_ok_but_bad_json_is_loud() {
        let td = TempDir::new("map-load");
        assert!(load(td.path()).expect("missing ok").is_none());
        std::fs::write(td.path().join("map.json"), "{not json").unwrap();
        assert!(load(td.path()).unwrap_err().contains("invalid map JSON"));
        std::fs::write(td.path().join("map.json"), r#"{"schema_version":2,"hub":"A"}"#).unwrap();
        assert!(load(td.path()).unwrap_err().contains("schema_version"));
    }
}
