//! seq136 D1: neutral map model + deterministic route planning.
//!
//! The JSON schema/validation lives in `oas-task::map`; the runtime only needs
//! this neutral view (no serde/JSON dependency here). `identify`/navigation
//! execution are implemented as `Interpreter` helpers in `lib.rs`.
//!
//! Planning is a pure function of the map + from/to, so it is unit tested
//! without any screen/OCR machinery (design §3, §8).

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap, HashMap};

use oas_types::{Point, Rect};

/// Upper bounds shared with validation / execution (design §4.5).
pub const MAX_NAV_ROUNDS: u32 = 3;
pub const MAX_NAV_HOPS: usize = 16;
pub const MAX_IDENTIFY_NODES: usize = 500;
pub const MAX_NODE_FEATURES: usize = 6;

/// One recognition feature of a map node.
#[derive(Clone, Debug, PartialEq)]
pub enum MapFeature {
    OcrText { text: String, roi: Option<Rect> },
    Asset { asset: String },
}

/// One node = one game page.
#[derive(Clone, Debug, PartialEq)]
pub struct MapNode {
    pub id: String,
    pub name: Option<String>,
    pub parent: Option<String>,
    pub return_asset: Option<String>,
    pub min_hits: u32,
    pub features: Vec<MapFeature>,
    pub priority: i32,
}

impl MapNode {
    pub fn asset_feature_count(&self) -> usize {
        self.features
            .iter()
            .filter(|f| matches!(f, MapFeature::Asset { .. }))
            .count()
    }
}

/// One directed transition (`from --action--> to`). `derived=true` means the
/// edge came from `parent`+`return_asset` (the author did not write it).
#[derive(Clone, Debug, PartialEq)]
pub struct MapEdge {
    pub from: String,
    pub to: String,
    pub action: MapAction,
    pub wait: MapWait,
    pub derived: bool,
    /// Declaration order (explicit edge array index, or node index for derived).
    pub order: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MapAction {
    ClickAsset { asset: String, hold_ms: u32 },
    Key { keycode: u32 },
    Swipe { from: Point, to: Point, ms: u32 },
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

/// The one global map (neutral runtime view).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct MapConfig {
    pub hub: String,
    pub default_return_asset: Option<String>,
    pub nodes: Vec<MapNode>,
    pub edges: Vec<MapEdge>,
}

impl MapConfig {
    /// Node ids in declaration order (UI/plan preview).
    pub fn node_ids(&self) -> Vec<&str> {
        self.nodes.iter().map(|n| n.id.as_str()).collect()
    }

    pub fn node(&self, id: &str) -> Option<&MapNode> {
        self.nodes.iter().find(|n| n.id == id)
    }
}

/// Indexed directed graph used by the planner.
#[derive(Clone, Debug, Default)]
pub struct MapGraph {
    pub nodes: Vec<String>,
    pub hub: String,
    /// adjacency: from node index -> edge indices (deterministic order).
    pub adj: Vec<Vec<usize>>,
    pub edges: Vec<MapEdge>,
}

/// Build the indexed graph: explicit edges first (declaration order), then
/// derived `node->parent` return edges unless an explicit edge already covers
/// that exact pair (explicit wins, design §3.1).
pub fn build_graph(map: &MapConfig) -> MapGraph {
    let nodes: Vec<String> = map.nodes.iter().map(|n| n.id.clone()).collect();
    let index: HashMap<&str, usize> = nodes.iter().enumerate().map(|(i, s)| (s.as_str(), i)).collect();
    let mut edges: Vec<MapEdge> = Vec::new();
    for (i, e) in map.edges.iter().enumerate() {
        if index.contains_key(e.from.as_str()) && index.contains_key(e.to.as_str()) {
            let mut e = e.clone();
            e.derived = false;
            e.order = i;
            edges.push(e);
        }
    }
    let explicit: BTreeSet<(String, String)> = edges
        .iter()
        .map(|e| (e.from.clone(), e.to.clone()))
        .collect();
    for (i, node) in map.nodes.iter().enumerate() {
        let Some(parent) = node.parent.as_ref() else {
            continue;
        };
        if explicit.contains(&(node.id.clone(), parent.clone())) {
            continue;
        }
        let Some(asset) = node
            .return_asset
            .clone()
            .or_else(|| map.default_return_asset.clone())
        else {
            continue;
        };
        if !index.contains_key(parent.as_str()) {
            continue;
        }
        edges.push(MapEdge {
            from: node.id.clone(),
            to: parent.clone(),
            action: MapAction::ClickAsset {
                asset,
                hold_ms: 0,
            },
            wait: MapWait::default(),
            derived: true,
            order: i,
        });
    }
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
    for (ei, edge) in edges.iter().enumerate() {
        let fi = index[edge.from.as_str()];
        adj[fi].push(ei);
    }
    MapGraph {
        nodes,
        hub: map.hub.clone(),
        adj,
        edges,
    }
}

impl MapGraph {
    fn idx(&self, id: &str) -> Option<usize> {
        self.nodes.iter().position(|n| n == id)
    }

    /// Deterministic shortest path (hop count), tie-broken by:
    /// fewer derived edges → lexicographically smaller declaration-order list.
    /// `forbid_hub_pass_through` never expands the hub as an intermediate node
    /// (the hub may still be the start or the target; design §3.2 step 1).
    fn dijkstra(
        &self,
        from: &str,
        to: &str,
        forbid_hub_pass_through: bool,
    ) -> Option<Vec<usize>> {
        let start = self.idx(from)?;
        let goal = self.idx(to)?;
        let hub_idx = self.idx(&self.hub);
        let mut best: HashMap<usize, PathCost> = HashMap::new();
        let mut heap: BinaryHeap<HeapEntry> = BinaryHeap::new();
        let zero = PathCost {
            hops: 0,
            derived: 0,
            seq: Vec::new(),
        };
        best.insert(start, zero.clone());
        heap.push(HeapEntry {
            cost: zero,
            node: start,
        });
        while let Some(HeapEntry { cost, node }) = heap.pop() {
            if best.get(&node) != Some(&cost) {
                continue;
            }
            if node == goal {
                return Some(cost.seq);
            }
            if forbid_hub_pass_through && Some(node) == hub_idx && node != start {
                continue;
            }
            for &ei in &self.adj[node] {
                let edge = &self.edges[ei];
                let next = self.idx(&edge.to)?;
                let mut seq = cost.seq.clone();
                seq.push(ei);
                let candidate = PathCost {
                    hops: cost.hops + 1,
                    derived: cost.derived + u32::from(edge.derived),
                    seq,
                };
                let better = match best.get(&next) {
                    None => true,
                    Some(old) => candidate.cmp(old) == Ordering::Less,
                };
                if better {
                    best.insert(next, candidate.clone());
                    heap.push(HeapEntry {
                        cost: candidate,
                        node: next,
                    });
                }
            }
        }
        None
    }

    fn edge_for(&self, from: &str, to: &str, explicit_only: bool) -> Option<MapEdge> {
        let fi = self.idx(from)?;
        self.adj[fi]
            .iter()
            .map(|&ei| self.edges[ei].clone())
            .find(|e| e.to == to && (!explicit_only || !e.derived))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PathCost {
    hops: u32,
    derived: u32,
    seq: Vec<usize>,
}

impl Ord for PathCost {
    fn cmp(&self, other: &Self) -> Ordering {
        self.hops
            .cmp(&other.hops)
            .then_with(|| self.derived.cmp(&other.derived))
            .then_with(|| self.seq.cmp(&other.seq))
    }
}

impl PartialOrd for PathCost {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Min-heap entry (reverse ordering so `BinaryHeap` pops the smallest cost).
#[derive(Clone, Debug)]
struct HeapEntry {
    cost: PathCost,
    node: usize,
}

impl PartialEq for HeapEntry {
    fn eq(&self, other: &Self) -> bool {
        self.cost == other.cost && self.node == other.node
    }
}
impl Eq for HeapEntry {}
impl Ord for HeapEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .cost
            .cmp(&self.cost)
            .then_with(|| other.node.cmp(&self.node))
    }
}
impl PartialOrd for HeapEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Plan a route from `from` to `to` (design §3.2, deterministic):
/// 1. direct shortest path that does **not** pass through the hub;
/// 2. same-tree LCA route (up via return edges, down via explicit edges only);
/// 3. global shortest path (hub/any detour fallback).
///
/// Returns the ordered edges; `from == to` returns an empty route.
pub fn plan_route(map: &MapConfig, from: &str, to: &str) -> Result<Vec<MapEdge>, String> {
    if map.node(from).is_none() {
        return Err(format!("navigate: 起点 `{from}` 不在地图中"));
    }
    if map.node(to).is_none() {
        return Err(format!("navigate: 目标 `{to}` 不在地图中"));
    }
    if from == to {
        return Ok(Vec::new());
    }
    let graph = build_graph(map);
    // 1) direct (hub excluded as an intermediate node).
    if let Some(seq) = graph.dijkstra(from, to, true) {
        return Ok(seq.into_iter().map(|ei| graph.edges[ei as usize].clone()).collect());
    }
    // 2) LCA / tree route.
    if let Some(route) = lca_route(map, &graph, from, to) {
        return Ok(route);
    }
    // 3) hub/global fallback.
    if let Some(seq) = graph.dijkstra(from, to, false) {
        return Ok(seq.into_iter().map(|ei| graph.edges[ei as usize].clone()).collect());
    }
    let reachable = reachable_nodes(&graph, from);
    Err(format!(
        "navigate: 无法从「{from}」到达「{to}」（hub={}；从起点可达：{}）；\
         提示：目标有 parent 但缺少返回素材，或缺少显式 from→to 边",
        map.hub,
        if reachable.is_empty() {
            "无".to_string()
        } else {
            reachable.join("、")
        }
    ))
}

fn reachable_nodes(graph: &MapGraph, from: &str) -> Vec<String> {
    let Some(start) = graph.idx(from) else {
        return Vec::new();
    };
    let mut seen = vec![false; graph.nodes.len()];
    let mut stack = vec![start];
    seen[start] = true;
    let mut out = Vec::new();
    while let Some(u) = stack.pop() {
        for &ei in &graph.adj[u] {
            let v = graph.edges[ei].to.clone();
            if let Some(vi) = graph.idx(&v) {
                if !seen[vi] {
                    seen[vi] = true;
                    out.push(v);
                    stack.push(vi);
                }
            }
        }
    }
    out
}

/// LCA route: `from -> ... -> lca` via explicit-or-derived return edges, then
/// `lca -> ... -> to` via explicit parent→child edges only.
fn lca_route(
    map: &MapConfig,
    graph: &MapGraph,
    from: &str,
    to: &str,
) -> Option<Vec<MapEdge>> {
    let chain = |start: &str| -> Option<Vec<String>> {
        let mut out = vec![start.to_string()];
        let mut cur = start.to_string();
        for _ in 0..=16 {
            let node = map.node(&cur)?;
            match node.parent.as_ref() {
                None => return Some(out),
                Some(parent) => {
                    out.push(parent.clone());
                    cur = parent.clone();
                }
            }
        }
        None
    };
    let a = chain(from)?;
    let b = chain(to)?;
    let bset: BTreeSet<&str> = b.iter().map(String::as_str).collect();
    // Deepest common node (a is [self, parent, ...]; first match = deepest).
    let lca = a
        .iter()
        .find(|id| bset.contains(id.as_str()))
        .cloned()?;
    let mut route = Vec::new();
    let apos = a.iter().position(|id| id == &lca)?;
    for pair in a[..=apos].windows(2) {
        let edge = graph.edge_for(&pair[0], &pair[1], false)?;
        route.push(edge);
    }
    let bpos = b.iter().position(|id| id == &lca)?;
    // b is [to, parent, ..., lca]: descend lca -> ... -> to via explicit edges.
    let mut down_nodes: Vec<String> = vec![lca.clone()];
    let mut descend = b[..bpos].to_vec();
    descend.reverse();
    down_nodes.extend(descend);
    for pair in down_nodes.windows(2) {
        let edge = graph.edge_for(&pair[0], &pair[1], true)?;
        route.push(edge);
    }
    Some(route)
}

/// Display one route as `from -> to` pairs (logs/UI/tests).
pub fn route_summary(route: &[MapEdge]) -> String {
    if route.is_empty() {
        return "(already there)".to_string();
    }
    let mut parts: Vec<String> = Vec::new();
    parts.push(route[0].from.clone());
    for e in route {
        parts.push(format!("{}[{}]", e.to, e.action.label()));
    }
    parts.join(" -> ")
}

impl MapAction {
    pub fn label(&self) -> String {
        match self {
            MapAction::ClickAsset { asset, .. } => format!("click_asset:{asset}"),
            MapAction::Key { keycode } => format!("key:{keycode}"),
            MapAction::Swipe { ms, .. } => format!("swipe:{ms}ms"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str, parent: Option<&str>, return_asset: Option<&str>) -> MapNode {
        MapNode {
            id: id.into(),
            name: None,
            parent: parent.map(str::to_string),
            return_asset: return_asset.map(str::to_string),
            min_hits: 1,
            features: vec![MapFeature::OcrText {
                text: id.into(),
                roi: None,
            }],
            priority: 0,
        }
    }

    fn edge(from: &str, to: &str, asset: &str, order: usize) -> MapEdge {
        MapEdge {
            from: from.into(),
            to: to.into(),
            action: MapAction::ClickAsset {
                asset: asset.into(),
                hold_ms: 0,
            },
            wait: MapWait::default(),
            derived: false,
            order,
        }
    }

    /// 庭院 (hub) <- 御魂 <- 御魂十层 ; plus 探索 under 庭院. Directed edges only
    /// downhill (庭院→御魂→十层) + explicit 十层→御魂 key edge.
    fn sample_map() -> MapConfig {
        MapConfig {
            hub: "庭院".into(),
            default_return_asset: Some("I_BACK".into()),
            nodes: vec![
                node("庭院", None, None),
                node("御魂", Some("庭院"), Some("I_BACK")),
                node("御魂十层", Some("御魂"), Some("I_BACK")),
                node("探索", Some("庭院"), Some("I_BACK")),
            ],
            edges: vec![
                edge("庭院", "御魂", "I_御魂入口", 0),
                edge("御魂", "御魂十层", "I_十层入口", 1),
                edge("庭院", "探索", "I_探索入口", 2),
                MapEdge {
                    from: "御魂十层".into(),
                    to: "御魂".into(),
                    action: MapAction::Key { keycode: 4 },
                    wait: MapWait::default(),
                    derived: false,
                    order: 3,
                },
            ],
        }
    }

    #[test]
    fn direct_route_never_passes_through_hub() {
        let map = sample_map();
        // 御魂 -> 御魂十层 is direct (no hub crossing).
        let route = plan_route(&map, "御魂", "御魂十层").expect("direct");
        assert_eq!(route.len(), 1);
        assert_eq!(route[0].from, "御魂");
        assert_eq!(route[0].to, "御魂十层");
        // 探索 -> 御魂十层: only path is up (探索->庭院->御魂->十层) with 庭院
        // as an intermediate, but phase 1 forbids it; LCA route handles it.
        let route = plan_route(&map, "探索", "御魂十层").expect("lca");
        assert_eq!(route.len(), 3, "{route:?}");
        assert_eq!(route[0].to, "庭院");
    }

    #[test]
    fn lca_route_uses_derived_return_edges_and_explicit_downhill_edges() {
        let map = sample_map();
        let route = plan_route(&map, "御魂十层", "探索").expect("lca");
        // up: 十层->御魂 explicit key, 御魂->庭院 derived; down: 庭院->探索 explicit.
        assert_eq!(route.len(), 3, "{route:?}");
        assert_eq!(route[0].from, "御魂十层");
        assert_eq!(route[0].to, "御魂");
        assert!(route[0].derived == false);
        assert_eq!(route[1].to, "庭院");
        assert!(route[1].derived);
        assert_eq!(route[2].to, "探索");
    }

    #[test]
    fn explicit_return_edge_overrides_the_derived_one() {
        let mut map = sample_map();
        // Author overrides 御魂->庭院 with a long-press style explicit edge.
        map.edges.insert(2, edge("御魂", "庭院", "I_显式返回", 99));
        let route = plan_route(&map, "御魂", "庭院").expect("return");
        assert_eq!(route.len(), 1);
        assert!(route[0].derived == false);
        match &route[0].action {
            MapAction::ClickAsset { asset, .. } => assert_eq!(asset, "I_显式返回"),
            other => panic!("unexpected action {other:?}"),
        }
    }

    #[test]
    fn hub_fallback_and_unreachable_are_loud() {
        let mut map = sample_map();
        // Isolate 探索 by removing its only edge: unreachable from 御魂 without
        // any path (no parent return asset for 庭院 root, no explicit edge).
        map.edges.retain(|e| !(e.from == "庭院" && e.to == "探索"));
        let err = plan_route(&map, "御魂", "探索").expect_err("unreachable");
        assert!(err.contains("无法从「御魂」到达「探索」"), "{err}");
        assert!(err.contains("可达"), "{err}");
        // Hub fallback: 探索 -> 御魂 requires going through the hub; phase 1 is
        // forbidden, phase 2 LCA works (探索->庭院->御魂). Remove explicit
        // 庭院->御魂 too and add only 探索->庭院 explicit; then the hub path still
        // exists via LCA up, but no downhill edge -> unreachable.
        let mut map2 = sample_map();
        map2.edges.retain(|e| !(e.from == "庭院" && e.to == "御魂"));
        let err = plan_route(&map2, "探索", "御魂").expect_err("no downhill");
        assert!(err.contains("无法从「探索」到达「御魂」"), "{err}");
    }

    #[test]
    fn cyclic_graph_terminates_and_is_deterministic() {
        let mut map = sample_map();
        // Add cycles both ways; planner must terminate and pick a stable route.
        map.edges.push(edge("御魂", "庭院", "I_返回", 4));
        map.edges.push(edge("御魂十层", "庭院", "I_回庭院", 5));
        let a = plan_route(&map, "御魂十层", "探索").expect("route");
        let b = plan_route(&map, "御魂十层", "探索").expect("route");
        assert_eq!(a, b, "same input must be deterministic");
    }

    #[test]
    fn from_equals_to_is_an_empty_route() {
        let map = sample_map();
        assert!(plan_route(&map, "御魂", "御魂").unwrap().is_empty());
        assert!(plan_route(&map, "不存在", "御魂")
            .unwrap_err()
            .contains("不在地图中"));
    }
}
