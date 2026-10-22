//! `meta.json` contract of the **unified task template** (ENGINE-DESIGN §22.34.2, M0).
//!
//! This module is the typed, backward-compatible view of `meta.json`:
//!
//! * Every new field (`template_version` / `task_id` / `category` / `entry` /
//!   `params` / `exits` / `capabilities` / `resources` / `schedule_hint`) is
//!   **optional**; old bundles keep working with M0 defaults
//!   (`exits = {done:success, failed:retryable}`, `params = []`,
//!   `category = expire`, `capabilities.checkpoint = true`).
//! * `exits[*].result` is restricted to the five result classes
//!   `success / resource / blocked / retryable / fatal` — `continue` / `stop`
//!   are gone (§22.34.2).
//! * Unknown extra fields are ignored (forward compatibility: a newer bundle
//!   read by an older engine must still parse).
//!
//! The shape is checked by serde; the **contract** is checked by
//! [`Meta::lint_issues`], which returns every violation with a `meta.json:…`
//! path (the lint calls it, the run path only needs [`Meta::param_defaults`]).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::LintIssue;

/// `template_version` this engine understands (R7: a mismatch must be refused).
pub const TEMPLATE_VERSION: u32 = 1;

/// The only five result classes an exit may map to (§22.34.2; no `continue`/`stop`).
pub const EXIT_RESULTS: [&str; 5] = ["success", "resource", "blocked", "retryable", "fatal"];

/// Category used when `meta.category` is absent (M0 default, old tasks).
pub const CATEGORY_DEFAULT: &str = "expire";

/// Default exit table of a task that does not declare `meta.exits` (old tasks).
pub const DEFAULT_EXIT_NAMES: [(&str, &str); 2] = [("done", "success"), ("failed", "retryable")];

/// Allowed `meta.params[].type` values.
pub const PARAM_TYPES: [&str; 5] = ["int", "float", "number", "bool", "string"];

// ---------------------------------------------------------------------------
// Data model (serde: this is the on-disk contract)
// ---------------------------------------------------------------------------

/// One declared task parameter (`meta.params[]`), injectable as `param('key')`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParamDecl {
    pub key: String,
    #[serde(rename = "type")]
    pub kind: String,
    /// Default value used when neither `--param` nor the scheduler passes one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
}

/// One declared exit (`meta.exits.<name>`): maps a flow `finish` name to a result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExitDecl {
    /// `success | resource | blocked | retryable | fatal`.
    pub result: String,
}

/// `entry.requires` — the cheap preflight guard the scheduler may evaluate
/// before starting a periodic task. Purely declarative here (M2 consumes it).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntryRequires {
    /// `appear | vanish | ocr_text` (same kinds as a flow `Expect`).
    pub kind: String,
    pub sel: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u32>,
}

/// `meta.entry` — how the task expects to be entered.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MetaEntry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires: Option<EntryRequires>,
}

fn default_true() -> bool {
    true
}

/// `meta.capabilities` — what the runtime may do with this task.
///
/// `checkpoint` defaults to `true` (old tasks effectively had checkpoint/resume
/// support); `interruptible` defaults to `false` (never guess a safe pause point).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Capabilities {
    #[serde(default = "default_true")]
    pub checkpoint: bool,
    #[serde(default)]
    pub interruptible: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_hint_s: Option<u32>,
}

impl Default for Capabilities {
    fn default() -> Self {
        Self {
            checkpoint: true,
            interruptible: false,
            duration_hint_s: None,
        }
    }
}

/// `schedule_hint.quota` — a UI default only; **never** the scheduling authority
/// (the authoritative values live in the player's `scheduler.json`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuotaHint {
    /// `day | week`.
    pub per: String,
    pub times: u32,
}

/// `meta.schedule_hint` — M2 scheduler/UI default hints (not authoritative).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ScheduleHint {
    #[serde(default)]
    pub windows: Vec<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quota: Option<QuotaHint>,
}

fn default_exits() -> BTreeMap<String, ExitDecl> {
    DEFAULT_EXIT_NAMES
        .iter()
        .map(|(name, result)| {
            (
                (*name).to_string(),
                ExitDecl {
                    result: (*result).to_string(),
                },
            )
        })
        .collect()
}

/// Typed `meta.json` of the unified task template (§22.34.2).
///
/// Required fields keep the old contract (`schema_version`, `name`); everything
/// added in M0 is optional with a defined default, so **old tasks are untouched**.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Meta {
    pub schema_version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template_version: Option<u32>,
    /// Stable task id (defaults to `name` when absent; validated like a task name).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<u32>,
    /// Legacy §22.33 field, still accepted; the new contract uses `category`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expire: Option<String>,
    /// Optional human description (M1 create dialog); non-blank, <= 500 chars.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// `daily | weekly | limited | utility | expire` (default `expire`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry: Option<MetaEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<ParamDecl>,
    /// Missing `exits` => [`DEFAULT_EXIT_NAMES`] (old-task compatibility).
    #[serde(default = "default_exits")]
    pub exits: BTreeMap<String, ExitDecl>,
    #[serde(default)]
    pub capabilities: Capabilities,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resources: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schedule_hint: Option<ScheduleHint>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn is_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    s.len() <= 64 && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn is_category_token(s: &str) -> bool {
    if s.is_empty() || s.len() > 32 {
        return false;
    }
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// Convert a `meta.params[].default` JSON scalar into a runtime expression value.
pub(crate) fn json_to_param_value(v: &serde_json::Value) -> Result<oas_flow::Value, String> {
    match v {
        serde_json::Value::Bool(b) => Ok(oas_flow::Value::Bool(*b)),
        serde_json::Value::Number(n) => n
            .as_f64()
            .map(oas_flow::Value::Num)
            .ok_or_else(|| format!("number `{n}` cannot be represented as f64")),
        serde_json::Value::String(s) => Ok(oas_flow::Value::Str(s.clone())),
        other => Err(format!(
            "must be a bool/number/string scalar, got `{other}`"
        )),
    }
}

fn check_param_default(kind: &str, value: &serde_json::Value) -> Result<(), String> {
    let ok = match kind {
        "int" => {
            value.is_i64()
                || value.is_u64()
                || value.as_f64().map(|f| f.fract() == 0.0).unwrap_or(false)
        }
        "float" | "number" => value.is_number(),
        "bool" => value.is_boolean(),
        "string" => value.is_string(),
        // Unknown `type` is reported separately; do not pile on a second error.
        _ => true,
    };
    if ok {
        Ok(())
    } else {
        Err(format!(
            "`{}` is not a valid `{kind}` default",
            value
        ))
    }
}

// ---------------------------------------------------------------------------
// Meta
// ---------------------------------------------------------------------------

impl Meta {
    /// Parse `meta.json` text (shape only; contract rules live in
    /// [`Self::lint_issues`]).
    pub fn from_json(text: &str) -> Result<Self, String> {
        serde_json::from_str::<Meta>(text).map_err(|e| format!("invalid JSON: {e}"))
    }

    /// Effective category: the declared one, or [`CATEGORY_DEFAULT`].
    pub fn category(&self) -> &str {
        self.category.as_deref().unwrap_or(CATEGORY_DEFAULT)
    }

    /// Effective task id: the declared one, or the task `name`.
    pub fn task_id(&self) -> &str {
        self.task_id.as_deref().unwrap_or(&self.name)
    }

    /// Every `meta.params[].key`.
    pub fn declared_param_keys(&self) -> BTreeSet<String> {
        self.params.iter().map(|p| p.key.clone()).collect()
    }

    /// Every `meta.exits` name.
    pub fn declared_exit_names(&self) -> BTreeSet<String> {
        self.exits.keys().cloned().collect()
    }

    /// Runtime values injected for `param('key')`: the declared defaults.
    ///
    /// The driver merges these with the `--param k=v` overrides (CLI wins).
    pub fn param_defaults(&self) -> Result<BTreeMap<String, oas_flow::Value>, String> {
        let mut out = BTreeMap::new();
        for (i, p) in self.params.iter().enumerate() {
            let Some(v) = &p.default else { continue };
            let value = json_to_param_value(v)
                .map_err(|e| format!("meta.json: params[{i}].default: {e}"))?;
            out.insert(p.key.clone(), value);
        }
        Ok(out)
    }

    /// Contract validation (M0). Every issue carries a `meta.json:…` path.
    pub fn lint_issues(&self) -> Vec<LintIssue> {
        let mut issues = Vec::new();
        let mut push = |path: String, msg: String| {
            issues.push(LintIssue::new("META", path, msg));
        };

        if self.schema_version != crate::SCHEMA_VERSION {
            push(
                "meta.json:schema_version".to_string(),
                format!(
                    "must be {} (got {})",
                    crate::SCHEMA_VERSION,
                    self.schema_version
                ),
            );
        }
        if self.name.trim().is_empty() {
            push(
                "meta.json:name".to_string(),
                "must be a non-empty task name".to_string(),
            );
        }
        if let Some(v) = self.template_version {
            if v == 0 {
                push(
                    "meta.json:template_version".to_string(),
                    "must be >= 1".to_string(),
                );
            } else if v > TEMPLATE_VERSION {
                // Forward compatibility: a newer template is not silently run.
                push(
                    "meta.json:template_version".to_string(),
                    format!(
                        "template_version {v} is newer than this engine supports \
                         ({TEMPLATE_VERSION}); upgrade the engine before running this task"
                    ),
                );
            }
        }
        if let Some(v) = self.version {
            if v == 0 {
                push("meta.json:version".to_string(), "must be >= 1".to_string());
            }
        }
        if let Some(id) = &self.task_id {
            if let Err(e) = crate::validate_task_name(id) {
                push("meta.json:task_id".to_string(), e);
            }
        }
        if let Some(d) = &self.description {
            if d.trim().is_empty() {
                push(
                    "meta.json:description".to_string(),
                    "must not be blank when present".to_string(),
                );
            } else if d.chars().count() > 500 {
                push(
                    "meta.json:description".to_string(),
                    "is longer than 500 characters".to_string(),
                );
            }
        }
        if let Some(c) = &self.category {
            if !is_category_token(c) {
                push(
                    "meta.json:category".to_string(),
                    format!(
                        "`{c}` must be a lowercase token (daily/weekly/limited/utility/expire)"
                    ),
                );
            }
        }

        if let Some(entry) = &self.entry {
            if let Some(req) = &entry.requires {
                let p = "meta.json:entry.requires";
                if !matches!(req.kind.as_str(), "appear" | "vanish" | "ocr_text") {
                    push(
                        format!("{p}.kind"),
                        format!(
                            "unknown kind `{}` (allowed: appear, vanish, ocr_text)",
                            req.kind
                        ),
                    );
                }
                if req.sel.trim().is_empty() {
                    push(format!("{p}.sel"), "must not be empty".to_string());
                }
                if let Some(t) = req.timeout_ms {
                    if t == 0 || t > 60_000 {
                        push(
                            format!("{p}.timeout_ms"),
                            format!("must be in 1..=60000 (got {t})"),
                        );
                    }
                }
            }
        }

        let mut seen_params: BTreeSet<&str> = BTreeSet::new();
        for (i, p) in self.params.iter().enumerate() {
            let path = format!("meta.json:params[{i}]");
            if !is_identifier(&p.key) {
                push(
                    format!("{path}.key"),
                    format!(
                        "`{}` must match [A-Za-z_][A-Za-z0-9_]* (<= 64 chars)",
                        p.key
                    ),
                );
            }
            if !seen_params.insert(p.key.as_str()) {
                push(
                    format!("{path}.key"),
                    format!("duplicate parameter key `{}`", p.key),
                );
            }
            if !PARAM_TYPES.contains(&p.kind.as_str()) {
                push(
                    format!("{path}.type"),
                    format!(
                        "unknown type `{}` (allowed: {})",
                        p.kind,
                        PARAM_TYPES.join(", ")
                    ),
                );
            }
            if let Some(d) = &p.default {
                if let Err(e) = check_param_default(&p.kind, d) {
                    push(format!("{path}.default"), e);
                }
            }
            let numeric = matches!(p.kind.as_str(), "int" | "float" | "number");
            if (p.min.is_some() || p.max.is_some()) && !numeric {
                push(
                    format!("{path}.min"),
                    "min/max are only valid for numeric params".to_string(),
                );
            }
            match (p.min, p.max) {
                (Some(a), Some(b)) if !a.is_finite() || !b.is_finite() => push(
                    format!("{path}.max"),
                    "min/max must be finite numbers".to_string(),
                ),
                (Some(a), Some(b)) if a > b => push(
                    format!("{path}.max"),
                    format!("max {b} must be >= min {a}"),
                ),
                _ => {}
            }
            if let Some(d) = &p.default {
                if let Some(n) = d.as_f64() {
                    if let Some(min) = p.min {
                        if n < min {
                            push(
                                format!("{path}.default"),
                                format!("default {n} is below min {min}"),
                            );
                        }
                    }
                    if let Some(max) = p.max {
                        if n > max {
                            push(
                                format!("{path}.default"),
                                format!("default {n} is above max {max}"),
                            );
                        }
                    }
                }
            }
        }

        if self.exits.is_empty() {
            push(
                "meta.json:exits".to_string(),
                "must declare at least one exit (omit the field to use the defaults \
                 done:success, failed:retryable)"
                    .to_string(),
            );
        }
        for (name, decl) in &self.exits {
            let path = format!("meta.json:exits.{name}");
            if !is_identifier(name) {
                push(
                    path.clone(),
                    format!(
                        "exit name `{name}` must match [A-Za-z_][A-Za-z0-9_]* (<= 64 chars)"
                    ),
                );
            }
            if !EXIT_RESULTS.contains(&decl.result.as_str()) {
                push(
                    format!("{path}.result"),
                    format!(
                        "result `{}` is not allowed; use one of {} (continue/stop were removed)",
                        decl.result,
                        EXIT_RESULTS.join(" / ")
                    ),
                );
            }
        }

        if let Some(secs) = self.capabilities.duration_hint_s {
            if secs == 0 || secs > 604_800 {
                push(
                    "meta.json:capabilities.duration_hint_s".to_string(),
                    format!("must be in 1..=604800 seconds (got {secs})"),
                );
            }
        }

        let mut seen_resources: BTreeSet<&str> = BTreeSet::new();
        for (i, r) in self.resources.iter().enumerate() {
            let path = format!("meta.json:resources[{i}]");
            if r.trim().is_empty() || r.chars().any(|c| c.is_control() || c.is_whitespace()) {
                push(
                    path,
                    "must be a non-empty resource id without whitespace".to_string(),
                );
            } else if !seen_resources.insert(r.as_str()) {
                push(path, format!("duplicate resource `{r}`"));
            }
        }

        if let Some(hint) = &self.schedule_hint {
            if let Some(quota) = &hint.quota {
                let p = "meta.json:schedule_hint.quota";
                if !matches!(quota.per.as_str(), "day" | "week") {
                    push(format!("{p}.per"), "must be `day` or `week`".to_string());
                }
                if quota.times == 0 || quota.times > 100 {
                    push(
                        format!("{p}.times"),
                        format!("must be in 1..=100 (got {})", quota.times),
                    );
                }
            }
        }

        issues
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(json: &str) -> Meta {
        Meta::from_json(json).expect("meta parses")
    }

    fn paths(m: &Meta) -> Vec<String> {
        m.lint_issues().into_iter().map(|i| i.path).collect()
    }

    #[test]
    fn legacy_meta_gets_the_m0_defaults() {
        let m = meta(r#"{"schema_version":1,"name":"任务1","expire":"daily"}"#);
        assert_eq!(m.exits.len(), 2, "missing exits => default table");
        assert_eq!(m.exits["done"].result, "success");
        assert_eq!(m.exits["failed"].result, "retryable");
        assert!(m.params.is_empty(), "missing params => []");
        assert_eq!(m.category(), "expire", "missing category => expire");
        assert!(m.capabilities.checkpoint, "checkpoint defaults to true");
        assert!(!m.capabilities.interruptible);
        assert_eq!(m.capabilities.duration_hint_s, None);
        assert_eq!(m.task_id(), "任务1", "task_id defaults to the name");
        assert!(m.lint_issues().is_empty(), "{:?}", m.lint_issues());
    }

    #[test]
    fn full_contract_example_parses_and_lints_green() {
        let m = meta(
            r#"{
              "schema_version": 1,
              "template_version": 1,
              "task_id": "s1-daily",
              "name": "每日结界",
              "version": 1,
              "category": "daily",
              "entry": {"requires": {"kind": "appear", "sel": "主界面", "timeout_ms": 3000}},
              "params": [
                {"key": "max_rounds", "type": "int", "default": 3, "min": 1, "max": 99},
                {"key": "mode", "type": "string", "default": "fast"}
              ],
              "exits": {
                "done":       {"result": "success"},
                "no_tickets": {"result": "resource"},
                "blocked":    {"result": "blocked"},
                "error":      {"result": "retryable"},
                "fatal":      {"result": "fatal"}
              },
              "capabilities": {"checkpoint": true, "interruptible": false, "duration_hint_s": 180},
              "resources": ["stamina"],
              "schedule_hint": {"windows": [], "quota": {"per": "day", "times": 1}}
            }"#,
        );
        assert_eq!(m.category(), "daily");
        assert_eq!(m.task_id(), "s1-daily");
        assert!(m.lint_issues().is_empty(), "{:?}", m.lint_issues());

        let defaults = m.param_defaults().expect("defaults convert");
        assert_eq!(defaults["max_rounds"], oas_flow::Value::Num(3.0));
        assert_eq!(
            defaults["mode"],
            oas_flow::Value::Str("fast".to_string())
        );
    }

    #[test]
    fn exit_result_must_be_one_of_five_classes() {
        for bad in ["continue", "stop", "retry", "", "Success"] {
            let m = meta(&format!(
                r#"{{"schema_version":1,"name":"x","exits":{{"done":{{"result":"{bad}"}}}}}}"#
            ));
            let issues = m.lint_issues();
            assert_eq!(issues.len(), 1, "{bad}: {issues:?}");
            assert_eq!(issues[0].path, "meta.json:exits.done.result");
            assert!(
                issues[0].msg.contains("not allowed"),
                "{bad}: {}",
                issues[0].msg
            );
        }
        // `continue`/`stop` are explicitly named in the message (removed by design).
        let m = meta(r#"{"schema_version":1,"name":"x","exits":{"done":{"result":"continue"}}}"#);
        assert!(
            m.lint_issues()[0].msg.contains("continue/stop"),
            "{}",
            m.lint_issues()[0].msg
        );
    }

    #[test]
    fn empty_exits_is_rejected_but_missing_exits_defaults() {
        let m = meta(r#"{"schema_version":1,"name":"x","exits":{}}"#);
        let issues = m.lint_issues();
        assert!(
            issues.iter().any(|i| i.path == "meta.json:exits"),
            "{issues:?}"
        );
        let m = meta(r#"{"schema_version":1,"name":"x"}"#);
        assert!(m.lint_issues().is_empty(), "{:?}", m.lint_issues());
    }

    #[test]
    fn params_are_validated_with_paths() {
        let m = meta(
            r#"{"schema_version":1,"name":"x","params":[
                 {"key":"max_rounds","type":"int","default":3,"min":1,"max":99},
                 {"key":"max_rounds","type":"int"},
                 {"key":"2bad","type":"weird"},
                 {"key":"ratio","type":"int","default":1.5},
                 {"key":"range","type":"int","min":9,"max":1},
                 {"key":"flag","type":"bool","min":0},
                 {"key":"text","type":"string","default":3}
               ]}"#,
        );
        let issues = m.lint_issues();
        let got: Vec<&str> = issues.iter().map(|i| i.path.as_str()).collect();
        for want in [
            "meta.json:params[1].key",    // duplicate
            "meta.json:params[2].key",    // bad key
            "meta.json:params[2].type",   // bad type
            "meta.json:params[3].default",// 1.5 is not an int
            "meta.json:params[4].max",    // max < min
            "meta.json:params[5].min",    // min on non-numeric
            "meta.json:params[6].default",// 3 is not a string
        ] {
            assert!(got.contains(&want), "missing {want} in {got:?}");
        }
        assert!(
            issues.iter().any(|i| i.msg.contains("duplicate parameter key")),
            "{issues:?}"
        );
    }

    #[test]
    fn task_id_and_description_grammar() {
        let m = meta(r#"{"schema_version":1,"name":"x","task_id":"../evil"}"#);
        assert!(paths(&m).iter().any(|p| p == "meta.json:task_id"), "{:?}", paths(&m));
        let m = meta(r#"{"schema_version":1,"name":"x","task_id":"good-slug_2"}"#);
        assert!(m.lint_issues().is_empty(), "{:?}", m.lint_issues());
        let m = meta(r#"{"schema_version":1,"name":"x","description":"   "}"#);
        assert!(
            paths(&m).iter().any(|p| p == "meta.json:description"),
            "{:?}",
            paths(&m)
        );
        let m = meta(r#"{"schema_version":1,"name":"x","description":"每日任务"}"#);
        assert!(m.lint_issues().is_empty(), "{:?}", m.lint_issues());
    }

    #[test]
    fn unknown_extra_fields_are_ignored_for_forward_compatibility() {
        let m = meta(
            r#"{"schema_version":1,"name":"x","future_field":42,
                 "capabilities":{"checkpoint":false,"future_capability":true}}"#,
        );
        assert!(
            m.lint_issues().is_empty(),
            "future fields must not break old engines: {:?}",
            m.lint_issues()
        );
        assert!(!m.capabilities.checkpoint);
    }

    #[test]
    fn param_defaults_include_only_scalar_defaults() {
        let m = meta(
            r#"{"schema_version":1,"name":"x","params":[
                 {"key":"a","type":"int"},
                 {"key":"b","type":"bool","default":true},
                 {"key":"c","type":"float","default":1.25}
               ]}"#,
        );
        let defaults = m.param_defaults().expect("convert");
        assert_eq!(defaults.len(), 2, "a has no default => not injected");
        assert_eq!(defaults["b"], oas_flow::Value::Bool(true));
        assert_eq!(defaults["c"], oas_flow::Value::Num(1.25));
    }
}
