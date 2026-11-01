//! Global UI relationship map (`ui-map/ui-map.json`).
//!
//! This is deliberately a plain, reviewable JSON graph: screen nodes carry OCR
//! markers and reusable actions; transitions connect screens. Frames point at
//! an interface (`Frame.interface_id`) and assets point at frames
//! (`Asset.positions`), so an asset's interfaces are derived instead of being
//! stored twice.
//!
//! Commands:
//!   oasd ui-map lint [--file <ui-map.json>] [--frames <frames.json>]
//!   oasd ui-map render [--file <ui-map.json>] [--out <ui-map.generated.md>]
//!   oasd ui-map migrate-frames [--file <ui-map.json>] [--frames <frames.json>]

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::Path;
use std::process::ExitCode;

use serde::{Deserialize, Serialize};

pub const DEFAULT_PATH: &str = "ui-map/ui-map.json";
pub const DEFAULT_FRAMES: &str = "frames/frames.json";
pub const DESIGN_W: i32 = 1280;
pub const DESIGN_H: i32 = 720;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiMap {
    #[serde(default = "default_schema")]
    pub schema_version: u32,
    pub goal: String,
    #[serde(default)]
    pub restart: Option<Restart>,
    #[serde(default)]
    pub interfaces: BTreeMap<String, Interface>,
    #[serde(default)]
    pub transitions: Vec<Transition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Restart {
    #[serde(default = "default_restart_kind")]
    pub kind: String,
    #[serde(default)]
    pub package: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Interface {
    #[serde(default)]
    pub title: String,
    #[serde(default = "default_type", rename = "type")]
    pub kind: String,
    #[serde(default = "default_match", rename = "match")]
    pub match_: String,
    #[serde(default)]
    pub markers: Vec<Marker>,
    #[serde(default)]
    pub actions: BTreeMap<String, Action>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Marker {
    #[serde(default = "default_marker_kind")]
    pub kind: String,
    #[serde(default)]
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub roi: Option<[i32; 4]>,
    #[serde(default)]
    pub keycode: Option<u32>,
    #[serde(default)]
    pub package: Option<String>,
    #[serde(default)]
    pub seconds: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transition {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub action: String,
    #[serde(default)]
    pub priority: i32,
}

fn default_schema() -> u32 {
    1
}
fn default_restart_kind() -> String {
    "restart_app".to_string()
}
fn default_type() -> String {
    "screen".to_string()
}
fn default_match() -> String {
    "any".to_string()
}
fn default_marker_kind() -> String {
    "ocr_text".to_string()
}

/// Category fallback used while migrating old frame manifests. Kept here so the
/// graph is the single place that names screens.
pub fn interface_for_category(category: &str) -> String {
    match category.trim() {
        "庭院" => "courtyard",
        "设置" => "settings",
        "式神录" => "shikigami",
        "町中" => "town",
        "探索总界面" | "探索" => "exploration",
        "御魂" => "soul",
        "斗技" => "duel",
        "结界突破" => "breakthrough",
        "逢魔之时" | "弹窗" => "boss",
        "悬赏封印" => "bounty",
        "启动" => "startup",
        "加载" => "loading",
        "宠物小屋" => "pet_house",
        _ => "unknown",
    }
    .to_string()
}

/// Category is the primary signal; a few legacy frames only had a descriptive
/// name (e.g. category empty + name "宠物小屋"), so use the name as a fallback.
pub fn interface_for_frame(category: &str, name: &str) -> String {
    let by_cat = interface_for_category(category);
    if by_cat != "unknown" {
        return by_cat;
    }
    let n = name.trim();
    match n {
        "宠物小屋" => "pet_house",
        _ => "unknown",
    }
    .to_string()
}

impl UiMap {
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("read {}: {e}", path.display()))?;
        serde_json::from_str(&text).map_err(|e| format!("{}: invalid JSON: {e}", path.display()))
    }

    /// Structural lint. Empty result means the graph is usable.
    pub fn lint(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if self.schema_version != 1 {
            errors.push(format!(
                "schema_version must be 1, got {}",
                self.schema_version
            ));
        }
        if self.goal.trim().is_empty() {
            errors.push("goal must not be empty".to_string());
        } else if !self.interfaces.contains_key(&self.goal) {
            errors.push(format!("goal `{}` is not an interface", self.goal));
        }
        if self.interfaces.is_empty() {
            errors.push("interfaces must not be empty".to_string());
        }

        for (id, itf) in &self.interfaces {
            if id.trim().is_empty() {
                errors.push("interface id must not be empty".to_string());
            }
            if itf.title.trim().is_empty() {
                errors.push(format!("[{id}] title must not be empty"));
            }
            let kind = itf.kind.as_str();
            if !matches!(
                kind,
                "screen" | "popup" | "loading" | "startup" | "unknown"
            ) {
                errors.push(format!("[{id}] unknown interface type `{kind}`"));
            }
            if kind != "unknown" && itf.markers.is_empty() {
                errors.push(format!("[{id}] needs at least one OCR marker"));
            }
            for (i, m) in itf.markers.iter().enumerate() {
                if m.text.trim().is_empty() {
                    errors.push(format!("[{id}].markers[{i}].text is empty"));
                }
                if !matches!(m.kind.as_str(), "ocr_text" | "text_visible") {
                    errors.push(format!(
                        "[{id}].markers[{i}].kind `{}` is not supported",
                        m.kind
                    ));
                }
            }
            for (aid, a) in &itf.actions {
                if aid.trim().is_empty() {
                    errors.push(format!("[{id}] action id is empty"));
                }
                validate_action(&format!("[{id}].actions[{aid}]"), a, &mut errors);
            }
        }

        let mut froms = BTreeSet::new();
        for (i, tr) in self.transitions.iter().enumerate() {
            let p = format!("transitions[{i}]");
            if !self.interfaces.contains_key(&tr.from) {
                errors.push(format!("{p}.from `{}` is not an interface", tr.from));
            }
            if !self.interfaces.contains_key(&tr.to) {
                errors.push(format!("{p}.to `{}` is not an interface", tr.to));
            }
            if tr.action.trim().is_empty() {
                errors.push(format!("{p}.action is empty"));
            } else if let Some(from) = self.interfaces.get(&tr.from) {
                if !from.actions.contains_key(&tr.action) {
                    errors.push(format!(
                        "{p}.action `{}` is not declared on `{}`",
                        tr.action, tr.from
                    ));
                }
            }
            froms.insert(tr.from.clone());
        }

        for id in self.interfaces.keys() {
            if id != &self.goal && !froms.contains(id) {
                errors.push(format!("[{id}] has no outgoing transition"));
            }
        }

        // Reachability: every interface must have a path to goal (the goal
        // itself is trivially reachable).
        let mut reverse: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for tr in &self.transitions {
            reverse
                .entry(tr.to.as_str())
                .or_default()
                .push(tr.from.as_str());
        }
        let mut seen = BTreeSet::new();
        let mut q = VecDeque::new();
        if self.interfaces.contains_key(&self.goal) {
            seen.insert(self.goal.as_str());
            q.push_back(self.goal.as_str());
        }
        while let Some(cur) = q.pop_front() {
            if let Some(prev) = reverse.get(cur) {
                for p in prev {
                    if seen.insert(p) {
                        q.push_back(p);
                    }
                }
            }
        }
        for id in self.interfaces.keys() {
            if !seen.contains(id.as_str()) {
                errors.push(format!("[{id}] cannot reach goal `{}`", self.goal));
            }
        }

        if let Some(r) = &self.restart {
            if r.kind != "restart_app" {
                errors.push(format!("restart.kind must be restart_app, got `{}`", r.kind));
            }
            if r.package.trim().is_empty() {
                errors.push("restart.package must not be empty".to_string());
            }
        } else {
            errors.push("restart fallback is required".to_string());
        }
        errors
    }

    /// Markdown + Mermaid generated view. Human-maintained design text lives in
    /// docs/UI-MAP.md; this file is disposable generated output.
    pub fn render_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str("# UI-MAP generated view\n\n");
        out.push_str("> Generated by `oasd ui-map render`; do not hand-edit.\n\n");
        out.push_str(&format!("Goal: `{}`\n\n", self.goal));
        out.push_str("```mermaid\nflowchart TD\n");
        for (id, itf) in &self.interfaces {
            let mark = if itf.markers.is_empty() {
                String::new()
            } else {
                format!(
                    "<br/>OCR: {}",
                    itf.markers
                        .iter()
                        .map(|m| m.text.as_str())
                        .collect::<Vec<_>>()
                        .join(" / ")
                )
            };
            out.push_str(&format!(
                "  {id}[\"{}{}\"]\n",
                itf.title.replace('"', "'"),
                mark.replace('"', "'")
            ));
        }
        for tr in &self.transitions {
            out.push_str(&format!(
                "  {} -->|{}| {}\n",
                tr.from,
                if tr.label.is_empty() {
                    tr.action.as_str()
                } else {
                    tr.label.as_str()
                },
                tr.to
            ));
        }
        out.push_str("```\n\n");
        out.push_str("## Interfaces\n\n| ID | title | type | OCR markers | actions |\n|---|---|---|---|---|\n");
        for (id, itf) in &self.interfaces {
            let markers = itf
                .markers
                .iter()
                .map(|m| m.text.as_str())
                .collect::<Vec<_>>()
                .join(" / ");
            let actions = itf.actions.keys().cloned().collect::<Vec<_>>().join(", ");
            out.push_str(&format!(
                "| `{id}` | {} | {} | {} | {} |\n",
                itf.title, itf.kind, markers, actions
            ));
        }
        out.push_str("\n## Transitions\n\n| from | to | action | label | priority |\n|---|---|---|---|---|\n");
        for tr in &self.transitions {
            out.push_str(&format!(
                "| `{}` | `{}` | `{}` | {} | {} |\n",
                tr.from, tr.to, tr.action, tr.label, tr.priority
            ));
        }
        out
    }

    /// Generate a reviewable recovery flow from the graph. This is intentionally
    /// a *static* expansion (one condition per interface + one edge action), not
    /// a runtime graph interpreter; the generated file is meant to be inspected
    /// and tested before replacing a hand-maintained task flow.
    pub fn generate_flow(&self, name: &str) -> serde_json::Value {
        let mut body = Vec::new();
        body.push(serde_json::json!({
            "op": "log",
            "args": {"level": "info", "text": format!("'{}：开始（由 ui-map 生成）'", escape_single(name))}
        }));
        body.push(serde_json::json!({
            "op": "if",
            "args": {"cond": self.goal_condition()},
            "then": [self.finish_step()],
            "else": []
        }));

        let mut main_body = Vec::new();
        for (id, itf) in self.ordered_interfaces() {
            if id == self.goal {
                continue;
            }
            let Some(cond) = self.interface_condition(&id) else {
                continue;
            };
            let Some(tr) = self.first_edge_toward_goal(&id) else {
                continue;
            };
            let Some(action) = self
                .interfaces
                .get(&tr.from)
                .and_then(|x| x.actions.get(&tr.action))
            else {
                continue;
            };
            let mut then = Vec::new();
            if let Some(step) = action_step(action) {
                then.push(step);
                then.push(serde_json::json!({"op":"wait","args":{"seconds":0.6}}));
                then.push(serde_json::json!({"op":"continue"}));
                main_body.push(serde_json::json!({
                    "op":"if",
                    "args":{"cond":cond},
                    "then":then,
                    "else":[]
                }));
            }
        }
        if let Some(back) = self
            .interfaces
            .get("unknown")
            .and_then(|x| x.actions.get("default_back"))
        {
            if let Some(step) = action_step(back) {
                main_body.push(step);
                main_body.push(serde_json::json!({"op":"wait","args":{"seconds":0.9}}));
            }
        }
        body.push(serde_json::json!({
            "op":"loop",
            "args":{"n":"8"},
            "body":main_body
        }));
        body.push(serde_json::json!({
            "op":"if",
            "args":{"cond":self.goal_condition()},
            "then":[self.finish_step()],
            "else":[]
        }));

        if let Some(restart) = &self.restart {
            body.push(serde_json::json!({
                "op":"restart_app",
                "args":{"package":restart.package}
            }));
            body.push(serde_json::json!({"op":"wait","args":{"seconds":18}}));
            let mut recovery = Vec::new();
            recovery.push(serde_json::json!({
                "op":"if",
                "args":{"cond":self.goal_condition()},
                "then":[self.finish_step()],
                "else":[]
            }));
            for (id, itf) in self.ordered_interfaces() {
                if id == self.goal || itf.markers.is_empty() {
                    continue;
                }
                let Some(tr) = self.first_edge_toward_goal(&id) else {
                    continue;
                };
                let Some(action) = self
                    .interfaces
                    .get(&tr.from)
                    .and_then(|x| x.actions.get(&tr.action))
                else {
                    continue;
                };
                if let Some(step) = action_step(action) {
                    recovery.push(serde_json::json!({
                        "op":"if",
                        "args":{"cond":self.interface_condition(&id).unwrap_or_else(|| "false".into())},
                        "then":[step],
                        "else":[]
                    }));
                }
            }
            recovery.push(serde_json::json!({"op":"wait","args":{"seconds":1}}));
            body.push(serde_json::json!({
                "op":"loop",
                "args":{"n":"20"},
                "body":recovery
            }));
        }
        body.push(serde_json::json!({
            "op":"if",
            "args":{"cond":self.goal_condition()},
            "then":[self.finish_step()],
            "else":[]
        }));
        body.push(serde_json::json!({
            "op":"finish",
            "args":{"exit":"failed"}
        }));
        serde_json::json!({
            "schema_version": 1,
            "name": name,
            "steps": body
        })
    }

    fn goal_condition(&self) -> String {
        self.interface_condition(&self.goal)
            .unwrap_or_else(|| "false".to_string())
    }

    fn interface_condition(&self, id: &str) -> Option<String> {
        let itf = self.interfaces.get(id)?;
        if itf.markers.is_empty() {
            return None;
        }
        Some(
            itf.markers
                .iter()
                .map(|m| format!("text_visible('{}')", escape_single(&m.text)))
                .collect::<Vec<_>>()
                .join(" or "),
        )
    }

    fn ordered_interfaces(&self) -> Vec<(String, &Interface)> {
        let mut items: Vec<(String, &Interface)> = self
            .interfaces
            .iter()
            .map(|(id, itf)| (id.clone(), itf))
            .collect();
        // Popups first, then startup/loading, then regular screens. Stable
        // alphabetically inside each bucket.
        items.sort_by_key(|(id, itf)| {
            let rank = match itf.kind.as_str() {
                "popup" => 0,
                "startup" => 1,
                "loading" => 2,
                _ => 3,
            };
            (rank, id.clone())
        });
        items
    }

    fn first_edge_toward_goal(&self, start: &str) -> Option<&Transition> {
        if start == self.goal {
            return None;
        }
        let mut q = VecDeque::new();
        let mut seen = BTreeSet::new();
        q.push_back((start.to_string(), None::<usize>));
        seen.insert(start.to_string());
        while let Some((cur, first_idx)) = q.pop_front() {
            for (i, tr) in self.transitions.iter().enumerate() {
                if tr.from != cur {
                    continue;
                }
                let first = match first_idx {
                    Some(x) => x,
                    None => i,
                };
                if tr.to == self.goal {
                    return self.transitions.get(first);
                }
                if seen.insert(tr.to.clone()) {
                    q.push_back((tr.to.clone(), Some(first)));
                }
            }
        }
        None
    }

    fn finish_step(&self) -> serde_json::Value {
        serde_json::json!({"op":"finish","args":{"exit":"done"}})
    }

    /// Migrate old frame manifests: empty `interface_id` -> category mapping.
    pub fn migrate_frames_file(&self, path: &Path) -> Result<usize, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("read {}: {e}", path.display()))?;
        let mut root: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| format!("{}: invalid JSON: {e}", path.display()))?;
        let Some(frames) = root.get_mut("frames").and_then(|v| v.as_array_mut()) else {
            return Err(format!("{}: missing frames array", path.display()));
        };
        let mut changed = 0usize;
        for f in frames.iter_mut() {
            let current = f
                .get("interface_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            let category = f
                .get("category")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let name = f
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let mapped = interface_for_frame(&category, &name);
            if !current.is_empty() && !(current == "unknown" && mapped != "unknown") {
                continue;
            }
            f["interface_id"] = serde_json::Value::String(mapped);
            changed += 1;
        }
        let out = serde_json::to_string_pretty(&root)
            .map_err(|e| format!("serialize frames: {e}"))?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, format!("{out}\n"))
            .map_err(|e| format!("write {}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, path)
            .map_err(|e| format!("rename {} -> {}: {e}", tmp.display(), path.display()))?;
        Ok(changed)
    }

    /// Lint frame manifests against this map (used by `ui-map lint --frames`).
    pub fn lint_frames_file(&self, path: &Path) -> Vec<String> {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) => return vec![format!("read {}: {e}", path.display())],
        };
        let root: serde_json::Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(e) => return vec![format!("{}: invalid JSON: {e}", path.display())],
        };
        let mut errors = Vec::new();
        let Some(frames) = root.get("frames").and_then(|v| v.as_array()) else {
            return vec![format!("{}: missing frames array", path.display())];
        };
        for f in frames {
            let id = f.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
            let interface_id = f
                .get("interface_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            if interface_id.is_empty() {
                errors.push(format!("frame #{id} has no interface_id"));
            } else if !self.interfaces.contains_key(interface_id) {
                errors.push(format!(
                    "frame #{id} references unknown interface `{interface_id}`"
                ));
            }
        }
        errors
    }
}

fn validate_action(path: &str, a: &Action, errors: &mut Vec<String>) {
    match a.kind.as_str() {
        "region" => match a.roi {
            None => errors.push(format!("{path}: region action needs roi")),
            Some([x, y, w, h]) => {
                if w <= 0 || h <= 0 {
                    errors.push(format!("{path}: roi width/height must be > 0"));
                }
                if x < 0 || y < 0 || x + w > DESIGN_W || y + h > DESIGN_H {
                    errors.push(format!(
                        "{path}: roi [{x},{y},{w},{h}] outside {DESIGN_W}x{DESIGN_H}"
                    ));
                }
            }
        },
        "ocr_click" => {
            if a.text.as_deref().unwrap_or("").trim().is_empty() {
                errors.push(format!("{path}: ocr_click needs text"));
            }
        }
        "key" => {
            if a.keycode.unwrap_or(0) == 0 {
                errors.push(format!("{path}: key action needs a nonzero keycode"));
            }
        }
        "restart_app" => {
            if a.package.as_deref().unwrap_or("").trim().is_empty() {
                errors.push(format!("{path}: restart_app needs package"));
            }
        }
        "wait" => {
            if a.seconds.unwrap_or(-1.0) < 0.0 {
                errors.push(format!("{path}: wait needs seconds >= 0"));
            }
        }
        other => errors.push(format!("{path}: unsupported action kind `{other}`")),
    }
}

fn escape_single(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\'', "\\'")
}

/// Convert one graph action into the equivalent recovery-flow step. `None`
/// means the action is not executable yet and the generator should skip it.
fn action_step(a: &Action) -> Option<serde_json::Value> {
    match a.kind.as_str() {
        "region" => {
            let roi = a.roi?;
            Some(serde_json::json!({
                "op":"click_target",
                "args":{"find":[{"kind":"region","roi":roi}]}
            }))
        }
        "ocr_click" => {
            let text = a.text.as_deref()?.trim();
            if text.is_empty() {
                return None;
            }
            Some(serde_json::json!({
                "op":"click_target",
                "args":{"find":[{"kind":"ocr_text","roi":[0,0,DESIGN_W,DESIGN_H],"text":text}]}
            }))
        }
        "key" => {
            let keycode = a.keycode?;
            Some(serde_json::json!({"op":"key","args":{"keycode":keycode}}))
        }
        "restart_app" => {
            let package = a.package.as_deref()?.trim();
            if package.is_empty() {
                return None;
            }
            Some(serde_json::json!({"op":"restart_app","args":{"package":package}}))
        }
        "wait" => {
            let seconds = a.seconds.unwrap_or(1.0).max(0.0);
            Some(serde_json::json!({"op":"wait","args":{"seconds":seconds}}))
        }
        _ => None,
    }
}

fn parse_flag<'a>(args: &'a [String], name: &str, default: &'a str) -> Result<String, String> {
    let mut i = 0;
    while i < args.len() {
        if args[i] == name {
            return args
                .get(i + 1)
                .cloned()
                .ok_or_else(|| format!("{name} requires a value"));
        }
        if let Some(v) = args[i].strip_prefix(&format!("{name}=")) {
            return Ok(v.to_string());
        }
        i += 1;
    }
    Ok(default.to_string())
}

pub fn cmd(args: &[String]) -> ExitCode {
    let sub = args.first().map(|s| s.as_str()).unwrap_or("");
    match sub {
        "lint" => {
            let file = match parse_flag(&args[1..], "--file", DEFAULT_PATH) {
                Ok(v) => v,
                Err(e) => return usage_err(&e),
            };
            let frames = parse_flag(&args[1..], "--frames", "").unwrap_or_default();
            let map = match UiMap::load(Path::new(&file)) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("ui-map lint: {e}");
                    return ExitCode::from(1);
                }
            };
            let mut errors = map.lint();
            if !frames.is_empty() {
                errors.extend(map.lint_frames_file(Path::new(&frames)));
            }
            if errors.is_empty() {
                println!(
                    "ui-map lint: ok ({} interfaces, {} transitions)",
                    map.interfaces.len(),
                    map.transitions.len()
                );
                ExitCode::SUCCESS
            } else {
                for e in &errors {
                    eprintln!("- {e}");
                }
                eprintln!("ui-map lint: {} error(s)", errors.len());
                ExitCode::from(1)
            }
        }
        "render" => {
            let file = match parse_flag(&args[1..], "--file", DEFAULT_PATH) {
                Ok(v) => v,
                Err(e) => return usage_err(&e),
            };
            let out = match parse_flag(
                &args[1..],
                "--out",
                "docs/UI-MAP.generated.md",
            ) {
                Ok(v) => v,
                Err(e) => return usage_err(&e),
            };
            let map = match UiMap::load(Path::new(&file)) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("ui-map render: {e}");
                    return ExitCode::from(1);
                }
            };
            let errors = map.lint();
            if !errors.is_empty() {
                for e in &errors {
                    eprintln!("- {e}");
                }
                eprintln!("ui-map render: fix lint errors first");
                return ExitCode::from(1);
            }
            if let Some(parent) = Path::new(&out).parent() {
                if !parent.as_os_str().is_empty() {
                    if let Err(e) = std::fs::create_dir_all(parent) {
                        eprintln!("ui-map render: create {}: {e}", parent.display());
                        return ExitCode::from(1);
                    }
                }
            }
            if let Err(e) = std::fs::write(&out, map.render_markdown()) {
                eprintln!("ui-map render: write {out}: {e}");
                return ExitCode::from(1);
            }
            println!("ui-map render: {out}");
            ExitCode::SUCCESS
        }
        "migrate-frames" => {
            let file = match parse_flag(&args[1..], "--file", DEFAULT_PATH) {
                Ok(v) => v,
                Err(e) => return usage_err(&e),
            };
            let frames = match parse_flag(&args[1..], "--frames", DEFAULT_FRAMES) {
                Ok(v) => v,
                Err(e) => return usage_err(&e),
            };
            let map = match UiMap::load(Path::new(&file)) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("ui-map migrate-frames: {e}");
                    return ExitCode::from(1);
                }
            };
            match map.migrate_frames_file(Path::new(&frames)) {
                Ok(n) => {
                    println!("ui-map migrate-frames: updated {n} frame(s) in {frames}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("ui-map migrate-frames: {e}");
                    ExitCode::from(1)
                }
            }
        }
        "flow" => {
            let file = match parse_flag(&args[1..], "--file", DEFAULT_PATH) {
                Ok(v) => v,
                Err(e) => return usage_err(&e),
            };
            let out = match parse_flag(&args[1..], "--out", "tasks/返回庭院/flow.generated.json") {
                Ok(v) => v,
                Err(e) => return usage_err(&e),
            };
            let name = match parse_flag(&args[1..], "--name", "返回庭院（生成）") {
                Ok(v) => v,
                Err(e) => return usage_err(&e),
            };
            let map = match UiMap::load(Path::new(&file)) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("ui-map flow: {e}");
                    return ExitCode::from(1);
                }
            };
            let errors = map.lint();
            if !errors.is_empty() {
                for e in &errors {
                    eprintln!("- {e}");
                }
                eprintln!("ui-map flow: fix lint errors first");
                return ExitCode::from(1);
            }
            let flow = map.generate_flow(&name);
            let text = match serde_json::to_string_pretty(&flow) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("ui-map flow: serialize: {e}");
                    return ExitCode::from(1);
                }
            };
            if let Some(parent) = Path::new(&out).parent() {
                if !parent.as_os_str().is_empty() {
                    if let Err(e) = std::fs::create_dir_all(parent) {
                        eprintln!("ui-map flow: create {}: {e}", parent.display());
                        return ExitCode::from(1);
                    }
                }
            }
            match std::fs::write(&out, format!("{text}\n")) {
                Ok(()) => {
                    println!("ui-map flow: {out}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("ui-map flow: write {out}: {e}");
                    ExitCode::from(1)
                }
            }
        }
        _ => usage_err(
            "usage: oasd ui-map lint|render|flow|migrate-frames [--file ...] [--frames ...] [--out ...]",
        ),
    }
}

fn usage_err(msg: &str) -> ExitCode {
    eprintln!("{msg}");
    ExitCode::from(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> UiMap {
        let mut interfaces = BTreeMap::new();
        interfaces.insert(
            "courtyard".to_string(),
            Interface {
                title: "庭院".to_string(),
                kind: "screen".to_string(),
                match_: "any".to_string(),
                markers: vec![Marker {
                    kind: "ocr_text".into(),
                    text: "町中".into(),
                }],
                actions: BTreeMap::new(),
            },
        );
        interfaces.insert(
            "settings".to_string(),
            Interface {
                title: "设置".to_string(),
                kind: "screen".to_string(),
                match_: "any".to_string(),
                markers: vec![Marker {
                    kind: "ocr_text".into(),
                    text: "设置".into(),
                }],
                actions: BTreeMap::from([(
                    "close".to_string(),
                    Action {
                        kind: "region".into(),
                        text: None,
                        roi: Some([1045, 88, 78, 72]),
                        keycode: None,
                        package: None,
                        seconds: None,
                    },
                )]),
            },
        );
        UiMap {
            schema_version: 1,
            goal: "courtyard".into(),
            restart: Some(Restart {
                kind: "restart_app".into(),
                package: "com.netease.onmyoji".into(),
            }),
            interfaces,
            transitions: vec![Transition {
                from: "settings".into(),
                to: "courtyard".into(),
                label: "关闭".into(),
                action: "close".into(),
                priority: 10,
            }],
        }
    }

    #[test]
    fn valid_graph_lints_and_renders() {
        let map = sample();
        assert!(map.lint().is_empty(), "{:?}", map.lint());
        let md = map.render_markdown();
        assert!(md.contains("flowchart TD"));
        assert!(md.contains("settings -->|关闭| courtyard"));
    }

    #[test]
    fn unreachable_and_missing_action_are_reported() {
        let mut map = sample();
        map.transitions.clear();
        let errors = map.lint();
        assert!(errors.iter().any(|e| e.contains("no outgoing transition")));
        assert!(errors.iter().any(|e| e.contains("cannot reach goal")));
    }

    #[test]
    fn generated_flow_contains_goal_check_and_restart_fallback() {
        let map = sample();
        let flow = map.generate_flow("返回庭院（生成）");
        let text = serde_json::to_string(&flow).expect("serialize generated flow");
        assert!(text.contains("text_visible"));
        assert!(text.contains("restart_app"));
        assert!(text.contains("finish"));
        assert!(text.contains("loop"));
    }

    #[test]
    fn category_mapping_covers_existing_frames() {
        assert_eq!(interface_for_category("庭院"), "courtyard");
        assert_eq!(interface_for_category("式神录"), "shikigami");
        assert_eq!(interface_for_category(""), "unknown");
    }
}
