//! seq138 V2: global map = **locations / landmarks / paths**.
//!
//! A location is identified by full-screen features; a landmark is a directed
//! reference to another location plus the click path that goes there. There is
//! no parent/LCA/hub — return and outbound landmarks are explicit directed exits.
//! Planning is a pure BFS over that directed graph (unit tested here).

use std::collections::{BTreeSet, HashMap, VecDeque};

use oas_types::Rect;

pub const MAX_NAV_ROUNDS: u32 = 3;
pub const MAX_NAV_HOPS: usize = 16;
pub const MAX_IDENTIFY_LOCATIONS: usize = 500;
pub const MAX_NODE_FEATURES: usize = 6;
pub const MAX_FALLBACK_CYCLES: u32 = 1;

#[derive(Clone, Debug, PartialEq)]
pub enum MapFeature {
    OcrText { text: String, roi: Option<Rect> },
    Asset { asset: String },
}

/// One click path to another location (V2: `action="click"` only).
#[derive(Clone, Debug, PartialEq)]
pub struct MapPath {
    pub asset: String,
    pub wait: MapWait,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapWait {
    pub settle_ms: u32,
    pub stable_ms: u32,
    pub timeout_ms: u32,
    pub poll_ms: u32,
}

impl Default for MapWait {
    fn default() -> Self {
        Self {
            settle_ms: 350,
            stable_ms: 0,
            timeout_ms: 3000,
            poll_ms: 150,
        }
    }
}

/// One landmark: a reference to another location + its path.
#[derive(Clone, Debug, PartialEq)]
pub struct Landmark {
    pub name: String,
    pub path: MapPath,
    /// `true` for `return_landmarks`, `false` for `outbound_landmarks`.
    pub is_return: bool,
    /// Declaration index inside its group (deterministic tie-break).
    pub order: usize,
}

/// One location (= game page).
#[derive(Clone, Debug, PartialEq)]
pub struct Location {
    pub name: String,
    pub label: Option<String>,
    pub min_hits: u32,
    pub features: Vec<MapFeature>,
    pub return_landmarks: Vec<Landmark>,
    pub outbound_landmarks: Vec<Landmark>,
}

impl Location {
    pub fn asset_feature_count(&self) -> usize {
        self.features
            .iter()
            .filter(|f| matches!(f, MapFeature::Asset { .. }))
            .count()
    }
    /// All landmarks in deterministic order: return group then outbound group.
    pub fn landmarks(&self) -> Vec<&Landmark> {
        self.return_landmarks
            .iter()
            .chain(self.outbound_landmarks.iter())
            .collect()
    }
}

/// The ONE global map (V2 neutral runtime view).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct MapConfig {
    pub fallback_task: String,
    pub locations: Vec<Location>,
}

impl MapConfig {
    pub fn location(&self, name: &str) -> Option<&Location> {
        self.locations.iter().find(|l| l.name == name)
    }
    pub fn location_names(&self) -> Vec<&str> {
        self.locations.iter().map(|l| l.name.as_str()).collect()
    }
}

/// One directed graph edge derived from a landmark.
#[derive(Clone, Debug, PartialEq)]
pub struct MapEdge {
    pub from: String,
    pub to: String,
    pub path: MapPath,
    pub is_return: bool,
    pub order: usize,
}

/// Indexed directed graph.
#[derive(Clone, Debug, Default)]
pub struct MapGraph {
    pub names: Vec<String>,
    pub adj: Vec<Vec<usize>>,
    pub edges: Vec<MapEdge>,
}

/// Build the directed graph: every return/outbound landmark is one edge. No
/// reverse edge is added automatically.
pub fn build_graph(map: &MapConfig) -> MapGraph {
    let names: Vec<String> = map.locations.iter().map(|l| l.name.clone()).collect();
    let index: HashMap<&str, usize> = names
        .iter()
        .enumerate()
        .map(|(i, n)| (n.as_str(), i))
        .collect();
    let mut edges: Vec<MapEdge> = Vec::new();
    for loc in &map.locations {
        let Some(&fi) = index.get(loc.name.as_str()) else {
            continue;
        };
        for (group_idx, lm) in loc.landmarks().into_iter().enumerate() {
            if !index.contains_key(lm.name.as_str()) {
                continue; // validation reports it; planner ignores dangling
            }
            edges.push(MapEdge {
                from: names[fi].clone(),
                to: lm.name.clone(),
                path: lm.path.clone(),
                is_return: lm.is_return,
                order: group_idx,
            });
        }
    }
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); names.len()];
    for (ei, e) in edges.iter().enumerate() {
        let Some(&fi) = index.get(e.from.as_str()) else {
            continue;
        };
        adj[fi].push(ei);
    }
    MapGraph { names, adj, edges }
}

impl MapGraph {
    fn idx(&self, name: &str) -> Option<usize> {
        self.names.iter().position(|n| n == name)
    }
}

/// Plan a route between two locations with BFS (shortest hop count; ties keep
/// declaration order of return-then-outbound landmarks). No LCA/hub.
pub fn plan_route(map: &MapConfig, from: &str, to: &str) -> Result<Vec<MapEdge>, String> {
    if map.location(from).is_none() {
        return Err(format!("navigate: 当前所在地 `{from}` 不在地图中"));
    }
    if map.location(to).is_none() {
        return Err(format!("navigate: 目标所在地 `{to}` 不在地图中"));
    }
    if from == to {
        return Ok(Vec::new());
    }
    let graph = build_graph(map);
    let start = graph.idx(from).expect("checked");
    let goal = graph.idx(to).expect("checked");
    let mut prev: Vec<Option<usize>> = vec![None; graph.names.len()]; // previous edge
    let mut seen = vec![false; graph.names.len()];
    seen[start] = true;
    let mut q = VecDeque::new();
    q.push_back(start);
    while let Some(u) = q.pop_front() {
        if u == goal {
            break;
        }
        for &ei in &graph.adj[u] {
            let v = graph.idx(&graph.edges[ei].to).expect("edge target exists");
            if !seen[v] {
                seen[v] = true;
                prev[v] = Some(ei);
                q.push_back(v);
            }
        }
    }
    if !seen[goal] {
        let reachable: Vec<&str> = graph
            .names
            .iter()
            .enumerate()
            .filter(|(i, _)| seen[*i])
            .map(|(_, n)| n.as_str())
            .collect();
        return Err(format!(
            "navigate: 无法从「{from}」到达「{to}」（可达：{}）；请检查路标是否有向配置",
            if reachable.is_empty() {
                "无".to_string()
            } else {
                reachable.join("、")
            }
        ));
    }
    let mut route = Vec::new();
    let mut cur = goal;
    while cur != start {
        let ei = prev[cur].expect("BFS parent");
        route.push(graph.edges[ei].clone());
        cur = graph.idx(&graph.edges[ei].from).expect("from exists");
    }
    route.reverse();
    Ok(route)
}

/// Display one route (logs/tests/UI): `A -> B[click:asset] -> C`.
pub fn route_summary(route: &[MapEdge]) -> String {
    if route.is_empty() {
        return "(already there)".to_string();
    }
    let mut out = route[0].from.clone();
    for e in route {
        out.push_str(&format!(" -> {}[click:{}]", e.to, e.path.asset));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loc(name: &str, returns: &[(&str, &str)], outbound: &[(&str, &str)]) -> Location {
        let mk = |group: &[(&str, &str)], is_return: bool| {
            group
                .iter()
                .enumerate()
                .map(|(i, (n, a))| Landmark {
                    name: n.to_string(),
                    path: MapPath {
                        asset: a.to_string(),
                        wait: MapWait::default(),
                    },
                    is_return,
                    order: i,
                })
                .collect()
        };
        Location {
            name: name.into(),
            label: None,
            min_hits: 1,
            features: vec![MapFeature::OcrText {
                text: name.into(),
                roi: None,
            }],
            return_landmarks: mk(returns, true),
            outbound_landmarks: mk(outbound, false),
        }
    }

    fn map() -> MapConfig {
        MapConfig {
            fallback_task: "返回庭院".into(),
            locations: vec![
                loc(
                    "庭院",
                    &[],
                    &[("御魂", "I_御魂入口"), ("探索", "I_探索入口")],
                ),
                loc("御魂", &[("庭院", "I_BACK")], &[("御魂十层", "I_十层入口")]),
                loc("御魂十层", &[("御魂", "I_BACK")], &[]),
                loc("探索", &[("庭院", "I_BACK")], &[]),
            ],
        }
    }

    #[test]
    fn bfs_finds_shortest_directed_route() {
        let m = map();
        let r = plan_route(&m, "庭院", "御魂十层").unwrap();
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].to, "御魂");
        assert_eq!(r[1].to, "御魂十层");
        // No automatic reverse edge: removing the explicit return landmark makes
        // 御魂→庭院 unreachable.
        let mut no_ret = map();
        no_ret.locations[1].return_landmarks.clear();
        let err = plan_route(&no_ret, "御魂", "庭院").unwrap_err();
        assert!(err.contains("无法从"), "{err}");
    }

    #[test]
    fn return_landmarks_are_left_to_right_explicit_edges() {
        let m = map();
        let r = plan_route(&m, "御魂十层", "庭院").unwrap();
        assert_eq!(r.len(), 2);
        assert!(r.iter().all(|e| e.is_return), "{r:?}");
        assert_eq!(r[0].to, "御魂");
        assert_eq!(r[1].to, "庭院");
    }

    #[test]
    fn cycles_terminate_and_ties_are_deterministic() {
        let mut m = map();
        // Explicit return edge from 庭院 to 御魂 creates a cycle 庭院<->御魂.
        m.locations[0]
            .return_landmarks
            .push(Landmark {
                name: "御魂".into(),
                path: MapPath {
                    asset: "I_BACK2".into(),
                    wait: MapWait::default(),
                },
                is_return: true,
                order: 0,
            });
        let a = plan_route(&m, "庭院", "御魂十层").unwrap();
        let b = plan_route(&m, "庭院", "御魂十层").unwrap();
        assert_eq!(a, b);
        assert_eq!(a.len(), 2, "shortest path wins over cycles: {a:?}");
    }

    #[test]
    fn unknown_location_and_unreachable_are_loud() {
        let m = map();
        assert!(plan_route(&m, "不存在", "庭院").unwrap_err().contains("不在地图中"));
        let mut m2 = map();
        m2.locations.retain(|l| l.name != "御魂");
        let err = plan_route(&m2, "庭院", "御魂十层").unwrap_err();
        assert!(err.contains("无法从「庭院」到达「御魂十层」"), "{err}");
    }
}
