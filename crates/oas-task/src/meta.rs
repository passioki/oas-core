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
///
/// M0 fields (`key` / `type` / `default` / `min` / `max`) are the runtime
/// contract. The five optional fields added by the Runner Studio split
/// (design §C.1) plus `role` (参数 v2, PARAM-V2-DESIGN §2.1) are
/// **display/form constraints only** — they never change how a value is
/// injected at run time — and old `meta.json` files keep parsing because every
/// one of them is `Option` + `skip_serializing_if`.
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
    /// Human display name for the Runner form (non-blank, <= 80 chars).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// One-line explanation shown under the Runner form field (<= 500 chars).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Allowed values for a `string`/`int`/`float`/`number` parameter
    /// (JSON key `enum`; `bool` is intentionally not supported). Empty and
    /// duplicate lists are rejected; a declared `default` must be a member.
    #[serde(rename = "enum", default, skip_serializing_if = "Option::is_none")]
    pub enum_values: Option<Vec<serde_json::Value>>,
    /// Runner must supply a value when there is no `default`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
    /// Runner form group heading (non-blank, <= 80 chars); UI only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// Optional **semantic hint** for the Studio/Runner UI (参数 v2,
    /// PARAM-V2-DESIGN §2.1). It is deliberately **not** a value type: it never
    /// changes how a value is injected at run time, and unknown values are
    /// ignored (forward compatibility). The only known value is `"ocr_text"`
    /// (= this parameter is meant to be used as the OCR expected text, see
    /// `text_expr`), which the UI may use for hints / default selection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
}

/// Length limit of `meta.params[].role` (PARAM-V2-DESIGN §2.1).
pub const PARAM_ROLE_MAX_CHARS: usize = 32;

/// Length limits of the optional display fields (`meta.params[]`).
pub const PARAM_LABEL_MAX_CHARS: usize = 80;
pub const PARAM_DESCRIPTION_MAX_CHARS: usize = 500;
pub const PARAM_GROUP_MAX_CHARS: usize = 80;

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
    /// v1.1 task-level completion condition. Scheduler quota ("how many times
    /// per day") is separate; this describes "how much work = one run".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goal: Option<Goal>,
}

/// Task-level completion condition (v1.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Goal {
    /// `battles` = repeat the flow N times; `until_resource` / `until_condition`
    /// are reserved for flow-authored checks (UI records them, engine does not
    /// invent game semantics).
    pub mode: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<u32>,
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

/// Type check shared by `meta.params[].default`, `enum` elements and Runner
/// runtime values. `what` only shapes the message (`default` / `value` /
/// `enum value`).
fn check_param_type(kind: &str, value: &serde_json::Value, what: &str) -> Result<(), String> {
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
        Err(format!("`{value}` is not a valid `{kind}` {what}"))
    }
}

fn check_param_default(kind: &str, value: &serde_json::Value) -> Result<(), String> {
    check_param_type(kind, value, "default")
}

/// Numeric enums compare by value (`1` and `1.0` are the same member); all
/// other kinds compare structurally. This keeps `default`/`enum` membership and
/// duplicate detection consistent with how serde_json widens numbers.
fn param_value_eq(kind: &str, a: &serde_json::Value, b: &serde_json::Value) -> bool {
    if matches!(kind, "int" | "float" | "number") {
        match (a.as_f64(), b.as_f64()) {
            (Some(x), Some(y)) => x == y,
            _ => a == b,
        }
    } else {
        a == b
    }
}

fn enum_contains(
    kind: &str,
    enum_values: &[serde_json::Value],
    value: &serde_json::Value,
) -> bool {
    enum_values.iter().any(|e| param_value_eq(kind, e, value))
}

/// Full check of one provided runtime value against a declaration:
/// type + `min`/`max` + `enum`.
fn check_param_value(
    kind: &str,
    value: &serde_json::Value,
    min: Option<f64>,
    max: Option<f64>,
    enum_values: Option<&[serde_json::Value]>,
) -> Result<(), String> {
    check_param_type(kind, value, "value")?;
    if let Some(n) = value.as_f64() {
        if let Some(lo) = min {
            if n < lo {
                return Err(format!("value {n} is below min {lo}"));
            }
        }
        if let Some(hi) = max {
            if n > hi {
                return Err(format!("value {n} is above max {hi}"));
            }
        }
    }
    if let Some(values) = enum_values {
        if !enum_contains(kind, values, value) {
            return Err(format!("`{value}` is not one of the declared enum values"));
        }
    }
    Ok(())
}

/// Validate Runner-supplied runtime values (`config/values.json`, design §C.2)
/// against a task's `meta.params` schema.
///
/// Rules (all errors carry the parameter key or `params[i].key` so the Runner
/// UI / HTTP API can show them verbatim):
/// * an unknown key is an error and the message lists the allowed keys;
/// * a provided value must match its `type`, `min`/`max` and `enum`;
/// * a missing value is only an error when `required == Some(true)` **and**
///   there is no `default` (otherwise omitting it is fine: the runtime falls
///   back to the declared default).
pub fn validate_param_values(
    params: &[ParamDecl],
    values: &serde_json::Map<String, serde_json::Value>,
) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    let mut declared: BTreeSet<&str> = BTreeSet::new();
    for (i, p) in params.iter().enumerate() {
        declared.insert(p.key.as_str());
        let path = format!("params[{i}].key `{}`", p.key);
        match values.get(&p.key) {
            Some(v) => {
                if let Err(e) =
                    check_param_value(&p.kind, v, p.min, p.max, p.enum_values.as_deref())
                {
                    errors.push(format!("{path}: {e}"));
                }
            }
            None => {
                if p.required == Some(true) && p.default.is_none() {
                    errors.push(format!(
                        "{path}: 缺少必填参数 (required parameter has no default)"
                    ));
                }
            }
        }
    }
    let allowed = if params.is_empty() {
        "(none)".to_string()
    } else {
        params
            .iter()
            .map(|p| p.key.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };
    for key in values.keys().filter(|k| !declared.contains(k.as_str())) {
        errors.push(format!(
            "values.{key}: unknown parameter `{key}` (allowed: {allowed})"
        ));
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
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
        if let Some(goal) = &self.goal {
            match goal.mode.as_str() {
                "battles" => {
                    if !matches!(goal.count, Some(n) if (1..=999).contains(&n)) {
                        push(
                            "meta.json:goal.count".to_string(),
                            "mode=battles requires count 1..=999".to_string(),
                        );
                    }
                }
                "until_resource" | "until_condition" => {
                    if goal.count.is_some() {
                        push(
                            "meta.json:goal.count".to_string(),
                            format!("count is not used by mode={}", goal.mode),
                        );
                    }
                }
                other => push(
                    "meta.json:goal.mode".to_string(),
                    format!(
                        "unknown goal mode `{other}` (allowed: battles, until_resource, until_condition)"
                    ),
                ),
            }
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

            // --- Runner Studio split, design §C.1: optional display/form fields.
            for (field, text, limit) in [
                ("label", p.label.as_deref(), PARAM_LABEL_MAX_CHARS),
                (
                    "description",
                    p.description.as_deref(),
                    PARAM_DESCRIPTION_MAX_CHARS,
                ),
                ("group", p.group.as_deref(), PARAM_GROUP_MAX_CHARS),
            ] {
                let Some(text) = text else { continue };
                if text.trim().is_empty() {
                    push(format!("{path}.{field}"), "must not be blank".to_string());
                } else if text.chars().count() > limit {
                    push(
                        format!("{path}.{field}"),
                        format!(
                            "must be <= {limit} characters (got {})",
                            text.chars().count()
                        ),
                    );
                }
            }

            // v2 (PARAM-V2-DESIGN §2.1): `role` is a UI hint, not a contract.
            // Unknown values stay accepted on purpose (forward compatibility);
            // only the length is enforced.
            if let Some(role) = &p.role {
                if role.chars().count() > PARAM_ROLE_MAX_CHARS {
                    push(
                        format!("{path}.role"),
                        format!(
                            "must be <= {PARAM_ROLE_MAX_CHARS} characters (got {})",
                            role.chars().count()
                        ),
                    );
                }
            }

            if let Some(values) = &p.enum_values {
                let enum_path = format!("{path}.enum");
                if !matches!(p.kind.as_str(), "string" | "int" | "float" | "number") {
                    push(
                        enum_path,
                        format!(
                            "enum is only allowed for string/int/float/number params (type is `{}`)",
                            p.kind
                        ),
                    );
                } else {
                    if values.is_empty() {
                        push(enum_path.clone(), "must declare at least one value".to_string());
                    }
                    for (j, value) in values.iter().enumerate() {
                        if let Err(e) = check_param_default(&p.kind, value) {
                            push(
                                format!("{enum_path}[{j}]"),
                                format!("enum value {e}"),
                            );
                        }
                        if values[..j]
                            .iter()
                            .any(|prev| param_value_eq(&p.kind, prev, value))
                        {
                            push(
                                format!("{enum_path}[{j}]"),
                                format!("duplicate enum value `{value}`"),
                            );
                        }
                    }
                    if let Some(d) = &p.default {
                        if !enum_contains(&p.kind, values, d) {
                            push(
                                format!("{path}.default"),
                                format!(
                                    "default `{d}` is not one of the declared enum values"
                                ),
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

    // -----------------------------------------------------------------------
    // Runner Studio split §C.1: optional ParamDecl fields + runtime values.
    // -----------------------------------------------------------------------

    #[test]
    fn param_display_fields_roundtrip_and_are_omitted_when_absent() {
        let m = meta(
            r#"{"schema_version":1,"name":"x","params":[
                 {"key":"mode","type":"string","default":"fast","label":"模式",
                  "description":"选择运行模式","enum":["fast","safe"],
                  "required":true,"group":"基础"}
               ]}"#,
        );
        assert!(m.lint_issues().is_empty(), "{:?}", m.lint_issues());
        let p = &m.params[0];
        assert_eq!(p.label.as_deref(), Some("模式"));
        assert_eq!(p.description.as_deref(), Some("选择运行模式"));
        assert_eq!(
            p.enum_values.as_deref(),
            Some(&[serde_json::json!("fast"), serde_json::json!("safe")][..])
        );
        assert_eq!(p.required, Some(true));
        assert_eq!(p.group.as_deref(), Some("基础"));

        let text = serde_json::to_string(&m).expect("serialize");
        assert!(text.contains(r#""enum":["fast","safe"]"#), "{text}");
        let back = Meta::from_json(&text).expect("roundtrip parses");
        assert_eq!(back, m);

        // An old-style declaration must not gain any of the new keys on write.
        let legacy = meta(
            r#"{"schema_version":1,"name":"x","params":[{"key":"n","type":"int"}]}"#,
        );
        assert!(legacy.lint_issues().is_empty(), "{:?}", legacy.lint_issues());
        let text = serde_json::to_string(&legacy).expect("serialize");
        for key in ["label", "description", "enum", "required", "group"] {
            assert!(
                !text.contains(&format!(r#""{key}""#)),
                "absent `{key}` must not be serialized: {text}"
            );
        }
    }

    #[test]
    fn param_display_fields_are_non_blank_and_length_checked() {
        let m = meta(
            &serde_json::json!({
                "schema_version": 1,
                "name": "x",
                "params": [
                    {"key": "a", "type": "string", "label": "   "},
                    {"key": "b", "type": "string", "label": "x".repeat(81)},
                    {"key": "c", "type": "string", "description": "x".repeat(501)},
                    {"key": "d", "type": "string", "group": "\t"},
                    {"key": "e", "type": "string", "group": "g".repeat(81)}
                ]
            })
            .to_string(),
        );
        let got = paths(&m);
        for want in [
            "meta.json:params[0].label",
            "meta.json:params[1].label",
            "meta.json:params[2].description",
            "meta.json:params[3].group",
            "meta.json:params[4].group",
        ] {
            assert!(got.contains(&want.to_string()), "missing {want} in {got:?}");
        }
        // Exact boundaries are accepted.
        let ok = meta(
            &serde_json::json!({
                "schema_version": 1,
                "name": "x",
                "params": [{
                    "key": "a", "type": "string",
                    "label": "l".repeat(80),
                    "description": "d".repeat(500),
                    "group": "g".repeat(80)
                }]
            })
            .to_string(),
        );
        assert!(ok.lint_issues().is_empty(), "{:?}", ok.lint_issues());
    }

    #[test]
    fn param_role_is_an_optional_ui_hint() {
        let m = meta(
            r#"{"schema_version":1,"name":"x","params":[
                 {"key":"a","type":"string","default":"挑战","role":"ocr_text"},
                 {"key":"b","type":"int","default":1},
                 {"key":"c","type":"string","role":"future_role"}
               ]}"#,
        );
        assert!(
            m.lint_issues().is_empty(),
            "an unknown role must be ignored, not rejected: {:?}",
            m.lint_issues()
        );
        assert_eq!(m.params[0].role.as_deref(), Some("ocr_text"));
        assert_eq!(m.params[1].role, None);
        assert_eq!(m.params[2].role.as_deref(), Some("future_role"));

        // `role` must never change value injection (defaults are unchanged).
        let defaults = m.param_defaults().expect("defaults convert");
        assert_eq!(defaults["a"], oas_flow::Value::Str("挑战".to_string()));
        assert_eq!(defaults["b"], oas_flow::Value::Num(1.0));

        // Roundtrip keeps the hint; absent role is not serialized at all.
        let text = serde_json::to_string(&m).expect("serialize");
        assert!(text.contains(r#""role":"ocr_text""#), "{text}");
        assert!(text.contains(r#""role":"future_role""#), "{text}");
        let back = Meta::from_json(&text).expect("roundtrip parses");
        assert_eq!(back, m);
        let legacy = meta(
            r#"{"schema_version":1,"name":"x","params":[{"key":"n","type":"int"}]}"#,
        );
        let text = serde_json::to_string(&legacy).expect("serialize");
        assert!(
            !text.contains(r#""role""#),
            "absent `role` must not be serialized: {text}"
        );
    }

    #[test]
    fn param_role_length_is_checked() {
        let bad = meta(
            &serde_json::json!({
                "schema_version": 1,
                "name": "x",
                "params": [{"key": "a", "type": "string", "role": "r".repeat(33)}]
            })
            .to_string(),
        );
        assert!(
            paths(&bad).iter().any(|p| p == "meta.json:params[0].role"),
            "{:?}",
            paths(&bad)
        );
        // Exactly 32 characters is accepted.
        let ok = meta(
            &serde_json::json!({
                "schema_version": 1,
                "name": "x",
                "params": [{"key": "a", "type": "string", "role": "r".repeat(32)}]
            })
            .to_string(),
        );
        assert!(ok.lint_issues().is_empty(), "{:?}", ok.lint_issues());
    }

    #[test]
    fn param_enum_rules_are_enforced_with_paths() {
        let m = meta(
            &serde_json::json!({
                "schema_version": 1,
                "name": "x",
                "params": [
                    {"key": "empty", "type": "string", "enum": []},
                    {"key": "dup", "type": "string", "enum": ["a", "a"]},
                    {"key": "bad", "type": "int", "enum": [1, "x"]},
                    {"key": "flag", "type": "bool", "enum": [true]},
                    {"key": "out", "type": "string", "enum": ["a"], "default": "b"},
                    {"key": "one", "type": "int", "enum": [1, 2.5]}
                ]
            })
            .to_string(),
        );
        let got = paths(&m);
        for want in [
            "meta.json:params[0].enum",
            "meta.json:params[1].enum[1]",
            "meta.json:params[2].enum[1]",
            "meta.json:params[3].enum",
            "meta.json:params[4].default",
            "meta.json:params[5].enum[1]", // 2.5 is not an int
        ] {
            assert!(got.contains(&want.to_string()), "missing {want} in {got:?}");
        }
        assert!(
            m.lint_issues()
                .iter()
                .any(|i| i.msg.contains("duplicate enum value")),
            "{:?}",
            m.lint_issues()
        );
        assert!(
            m.lint_issues()
                .iter()
                .any(|i| i.msg.contains("not one of the declared enum")),
            "{:?}",
            m.lint_issues()
        );
    }

    #[test]
    fn param_enum_accepts_string_numeric_and_numeric_default_membership() {
        let m = meta(
            &serde_json::json!({
                "schema_version": 1,
                "name": "x",
                "params": [
                    {"key": "mode", "type": "string", "enum": ["fast", "safe"], "default": "fast"},
                    {"key": "rounds", "type": "int", "enum": [1, 3, 5], "default": 3},
                    {"key": "ratio", "type": "float", "enum": [0.5, 1.0], "default": 0.5},
                    {"key": "n", "type": "number", "enum": [1, 2.0], "default": 1.0}
                ]
            })
            .to_string(),
        );
        assert!(m.lint_issues().is_empty(), "{:?}", m.lint_issues());
    }

    #[test]
    fn validate_param_values_accepts_a_fully_valid_value_set() {
        let m = meta(
            r#"{"schema_version":1,"name":"x","params":[
                 {"key":"rounds","type":"int","default":3,"min":1,"max":99,"enum":[1,3,5]},
                 {"key":"ratio","type":"float","min":0,"max":1},
                 {"key":"count","type":"number"},
                 {"key":"flag","type":"bool"},
                 {"key":"mode","type":"string","enum":["fast","safe"],"default":"fast"}
               ]}"#,
        );
        let values = serde_json::json!({
            "rounds": 5, "ratio": 0.25, "count": 123.5, "flag": true, "mode": "safe"
        });
        let map = values.as_object().expect("object");
        assert_eq!(validate_param_values(&m.params, map), Ok(()));
        // Empty values are fine: every declaration has a default or is optional.
        assert_eq!(
            validate_param_values(&m.params, &serde_json::Map::new()),
            Ok(())
        );
    }

    #[test]
    fn validate_param_values_rejects_unknown_keys_with_the_allowed_list() {
        let m = meta(
            r#"{"schema_version":1,"name":"x","params":[
                 {"key":"rounds","type":"int"},
                 {"key":"mode","type":"string"}
               ]}"#,
        );
        let values = serde_json::json!({"ghost": 1, "rounds": 2});
        let err = validate_param_values(&m.params, values.as_object().unwrap()).unwrap_err();
        assert_eq!(err.len(), 1, "{err:?}");
        assert!(err[0].contains("ghost"), "{err:?}");
        assert!(err[0].contains("unknown parameter"), "{err:?}");
        assert!(err[0].contains("rounds, mode"), "allowed list: {err:?}");
    }

    #[test]
    fn validate_param_values_enforces_each_declared_type() {
        let m = meta(
            r#"{"schema_version":1,"name":"x","params":[
                 {"key":"rounds","type":"int"},
                 {"key":"ratio","type":"float"},
                 {"key":"count","type":"number"},
                 {"key":"flag","type":"bool"},
                 {"key":"mode","type":"string"}
               ]}"#,
        );
        let values = serde_json::json!({
            "rounds": 1.5, "ratio": "x", "count": "y", "flag": "yes", "mode": 3
        });
        let err = validate_param_values(&m.params, values.as_object().unwrap()).unwrap_err();
        assert_eq!(err.len(), 5, "{err:?}");
        for key in ["rounds", "ratio", "count", "flag", "mode"] {
            assert!(
                err.iter().any(|e| e.contains(key) && e.contains("not a valid")),
                "missing type error for {key}: {err:?}"
            );
        }
    }

    #[test]
    fn validate_param_values_enforces_min_max_and_enum() {
        let m = meta(
            r#"{"schema_version":1,"name":"x","params":[
                 {"key":"rounds","type":"int","min":1,"max":9},
                 {"key":"ratio","type":"float","min":0,"max":1},
                 {"key":"mode","type":"string","enum":["fast","safe"]}
               ]}"#,
        );
        let values = serde_json::json!({"rounds": 0, "ratio": 1.5, "mode": "turbo"});
        let err = validate_param_values(&m.params, values.as_object().unwrap()).unwrap_err();
        assert_eq!(err.len(), 3, "{err:?}");
        assert!(err.iter().any(|e| e.contains("rounds") && e.contains("below min")), "{err:?}");
        assert!(err.iter().any(|e| e.contains("ratio") && e.contains("above max")), "{err:?}");
        assert!(err.iter().any(|e| e.contains("mode") && e.contains("enum")), "{err:?}");
    }

    #[test]
    fn validate_param_values_requires_required_without_default_only() {
        let m = meta(
            r#"{"schema_version":1,"name":"x","params":[
                 {"key":"needed","type":"string","required":true},
                 {"key":"with_default","type":"string","required":true,"default":"ok"},
                 {"key":"optional","type":"string","required":false}
               ]}"#,
        );
        let err =
            validate_param_values(&m.params, &serde_json::Map::new()).unwrap_err();
        assert_eq!(err.len(), 1, "{err:?}");
        assert!(err[0].contains("needed"), "{err:?}");
        assert!(err[0].contains("缺少必填参数"), "{err:?}");

        let values = serde_json::json!({"needed": "supplied"});
        assert_eq!(
            validate_param_values(&m.params, values.as_object().unwrap()),
            Ok(())
        );
    }
}
