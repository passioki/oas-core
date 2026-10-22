//! oas-flow: the task DSL (P1-a) - expression parser + flow schema + validation.
//!
//! # Why a DSL and not Python
//! The engine runs user-authored logic. Editing Python in a web UI = arbitrary
//! code execution. Here the instruction set is fixed and expressions are parsed
//! by us (Pratt parser), so **`eval` is impossible by construction**.
//!
//! # Parallel design notes
//! * Steps are **nested** (`then`/`else`/`body` hold child steps), no goto.
//! * Every waiting instruction takes a timeout; a missing timeout is a **load
//!   error**, not a runtime hang.
//! * Unknown instruction names, missing args and references to assets that are
//!   not declared are rejected at **load time** (fail early, not mid-run).

use std::collections::BTreeSet;
use std::fmt;

use oas_types::Exit;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowError {
    /// JSON-ish path of the offending node, e.g. `steps[0].then[2]`.
    pub path: String,
    pub msg: String,
}

impl FlowError {
    pub fn new(path: impl Into<String>, msg: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            msg: msg.into(),
        }
    }
}

impl fmt::Display for FlowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path, self.msg)
    }
}

// ---------------------------------------------------------------------------
// Expressions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum Lit {
    Bool(bool),
    Num(f64),
    Str(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Or,
    And,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Lit(Lit),
    Call(String, Vec<Expr>),
    Not(Box<Expr>),
    Neg(Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Str(String),
    Ident(String),
    True,
    False,
    LParen,
    RParen,
    Comma,
    Op(&'static str),
    Not,
    And,
    Or,
    End,
}

fn tokenize(src: &str) -> Result<Vec<Tok>, String> {
    let b: Vec<char> = src.chars().collect();
    let mut i = 0usize;
    let mut out = Vec::new();
    while i < b.len() {
        let c = b[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        // numbers
        if c.is_ascii_digit() || (c == '.' && i + 1 < b.len() && b[i + 1].is_ascii_digit()) {
            let start = i;
            while i < b.len() && (b[i].is_ascii_digit() || b[i] == '.') {
                i += 1;
            }
            let s: String = b[start..i].iter().collect();
            let v: f64 = s.parse().map_err(|_| format!("bad number literal `{s}`"))?;
            out.push(Tok::Num(v));
            continue;
        }
        // strings: single or double quoted, no escapes (asset names are ASCII ids)
        if c == '\'' || c == '"' {
            let quote = c;
            i += 1;
            let start = i;
            while i < b.len() && b[i] != quote {
                i += 1;
            }
            if i >= b.len() {
                return Err(format!("unterminated string literal starting at char {start}"));
            }
            let s: String = b[start..i].iter().collect();
            i += 1;
            out.push(Tok::Str(s));
            continue;
        }
        // identifiers / keywords
        if c.is_ascii_alphabetic() || c == '_' {
            let start = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == '_') {
                i += 1;
            }
            let s: String = b[start..i].iter().collect();
            match s.as_str() {
                "true" => out.push(Tok::True),
                "false" => out.push(Tok::False),
                "not" => out.push(Tok::Not),
                "and" => out.push(Tok::And),
                "or" => out.push(Tok::Or),
                _ => out.push(Tok::Ident(s)),
            }
            continue;
        }
        // operators & punctuation
        let two: String = b[i..(i + 2).min(b.len())].iter().collect();
        let two_op = match two.as_str() {
            "==" => Some("=="),
            "!=" => Some("!="),
            "<=" => Some("<="),
            ">=" => Some(">="),
            _ => None,
        };
        if let Some(op) = two_op {
            out.push(Tok::Op(op));
            i += 2;
            continue;
        }
        let one = match c {
            '(' => Some(Tok::LParen),
            ')' => Some(Tok::RParen),
            ',' => Some(Tok::Comma),
            _ => None,
        };
        if let Some(t) = one {
            out.push(t);
            i += 1;
            continue;
        }
        let op = match c {
            '<' => "<",
            '>' => ">",
            '+' => "+",
            '-' => "-",
            '*' => "*",
            '/' => "/",
            '%' => "%",
            _ => return Err(format!("unexpected character `{c}` at char {i}")),
        };
        out.push(Tok::Op(op));
        i += 1;
    }
    out.push(Tok::End);
    Ok(out)
}

fn op_name(t: &Tok) -> Option<&'static str> {
    if let Tok::Op(s) = t {
        Some(*s)
    } else {
        None
    }
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> &Tok {
        &self.toks[self.pos]
    }
    fn next(&mut self) -> Tok {
        let t = self.toks[self.pos].clone();
        self.pos += 1;
        t
    }

    fn parse_expr(&mut self) -> Result<Expr, String> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Expr, String> {
        let mut lhs = self.parse_and()?;
        while matches!(self.peek(), Tok::Or) {
            self.next();
            let rhs = self.parse_and()?;
            lhs = Expr::Binary(BinOp::Or, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn parse_and(&mut self) -> Result<Expr, String> {
        let mut lhs = self.parse_cmp()?;
        while matches!(self.peek(), Tok::And) {
            self.next();
            let rhs = self.parse_cmp()?;
            lhs = Expr::Binary(BinOp::And, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn parse_cmp(&mut self) -> Result<Expr, String> {
        let mut lhs = self.parse_add()?;
        loop {
            let op = match op_name(self.peek()) {
                Some("==") => BinOp::Eq,
                Some("!=") => BinOp::Ne,
                Some("<") => BinOp::Lt,
                Some("<=") => BinOp::Le,
                Some(">") => BinOp::Gt,
                Some(">=") => BinOp::Ge,
                _ => break,
            };
            self.next();
            let rhs = self.parse_add()?;
            lhs = Expr::Binary(op, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn parse_add(&mut self) -> Result<Expr, String> {
        let mut lhs = self.parse_mul()?;
        loop {
            let op = match op_name(self.peek()) {
                Some("+") => BinOp::Add,
                Some("-") => BinOp::Sub,
                _ => break,
            };
            self.next();
            let rhs = self.parse_mul()?;
            lhs = Expr::Binary(op, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn parse_mul(&mut self) -> Result<Expr, String> {
        let mut lhs = self.parse_unary()?;
        loop {
            let op = match op_name(self.peek()) {
                Some("*") => BinOp::Mul,
                Some("/") => BinOp::Div,
                Some("%") => BinOp::Mod,
                _ => break,
            };
            self.next();
            let rhs = self.parse_unary()?;
            lhs = Expr::Binary(op, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn parse_unary(&mut self) -> Result<Expr, String> {
        match self.peek() {
            Tok::Not => {
                self.next();
                Ok(Expr::Not(Box::new(self.parse_unary()?)))
            }
            t if op_name(t) == Some("-") => {
                self.next();
                Ok(Expr::Neg(Box::new(self.parse_unary()?)))
            }
            _ => self.parse_primary(),
        }
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        match self.next() {
            Tok::Num(v) => Ok(Expr::Lit(Lit::Num(v))),
            Tok::Str(s) => Ok(Expr::Lit(Lit::Str(s))),
            Tok::True => Ok(Expr::Lit(Lit::Bool(true))),
            Tok::False => Ok(Expr::Lit(Lit::Bool(false))),
            Tok::LParen => {
                let e = self.parse_expr()?;
                match self.next() {
                    Tok::RParen => Ok(e),
                    other => Err(format!("expected `)`, got {other:?}")),
                }
            }
            Tok::Ident(name) => {
                // builtin calls: appear('X'), ocr('X'), ocr_digit('X'), page()
                match self.next() {
                    Tok::LParen => {
                        let mut args = Vec::new();
                        if !matches!(self.peek(), Tok::RParen) {
                            loop {
                                args.push(self.parse_expr()?);
                                match self.peek() {
                                    Tok::Comma => {
                                        self.next();
                                    }
                                    Tok::RParen => break,
                                    other => {
                                        return Err(format!("expected `,` or `)`, got {other:?}"))
                                    }
                                }
                            }
                        }
                        match self.next() {
                            Tok::RParen => Ok(Expr::Call(name, args)),
                            other => Err(format!("expected `)`, got {other:?}")),
                        }
                    }
                    // bare identifier is not allowed (no variables yet)
                    other => Err(format!(
                        "bare identifier `{name}` is not allowed; only builtin calls are \
                         supported (got {other:?})"
                    )),
                }
            }
            other => Err(format!("unexpected token {other:?}")),
        }
    }
}

pub fn parse_expr(src: &str) -> Result<Expr, String> {
    let toks = tokenize(src)?;
    let mut p = Parser { toks, pos: 0 };
    let e = p.parse_expr()?;
    match p.peek() {
        Tok::End => Ok(e),
        other => Err(format!("trailing tokens after expression: {other:?}")),
    }
}

// ---------------------------------------------------------------------------
// Evaluation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Bool(bool),
    Num(f64),
    Str(String),
    Null,
}

impl Value {
    fn truthy(&self) -> Result<bool, String> {
        match self {
            Value::Bool(b) => Ok(*b),
            other => Err(format!("expected bool, got {other:?}")),
        }
    }
    fn num(&self) -> Result<f64, String> {
        match self {
            Value::Num(n) => Ok(*n),
            other => Err(format!("expected number, got {other:?}")),
        }
    }
}

/// Everything an expression may observe. Implemented by the interpreter (P1-b).
pub trait EvalContext {
    fn appear(&mut self, sel: &str) -> Result<bool, String>;
    fn ocr(&mut self, sel: &str) -> Result<Option<String>, String>;
    fn ocr_digit(&mut self, sel: &str) -> Result<Option<f64>, String>;
    fn page(&mut self) -> Result<Option<String>, String>;
}

pub fn eval(e: &Expr, ctx: &mut dyn EvalContext) -> Result<Value, String> {
    match e {
        Expr::Lit(l) => Ok(match l {
            Lit::Bool(b) => Value::Bool(*b),
            Lit::Num(n) => Value::Num(*n),
            Lit::Str(s) => Value::Str(s.clone()),
        }),
        Expr::Not(inner) => Ok(Value::Bool(!eval(inner, ctx)?.truthy()?)),
        Expr::Neg(inner) => Ok(Value::Num(-eval(inner, ctx)?.num()?)),
        Expr::Call(name, args) => eval_call(name, args, ctx),
        Expr::Binary(op, lhs, rhs) => {
            // short-circuit for and/or
            match op {
                BinOp::And => {
                    if !eval(lhs, ctx)?.truthy()? {
                        return Ok(Value::Bool(false));
                    }
                    return Ok(Value::Bool(eval(rhs, ctx)?.truthy()?));
                }
                BinOp::Or => {
                    if eval(lhs, ctx)?.truthy()? {
                        return Ok(Value::Bool(true));
                    }
                    return Ok(Value::Bool(eval(rhs, ctx)?.truthy()?));
                }
                _ => {}
            }
            let a = eval(lhs, ctx)?;
            let b = eval(rhs, ctx)?;
            binary(*op, a, b)
        }
    }
}

fn eval_call(name: &str, args: &[Expr], ctx: &mut dyn EvalContext) -> Result<Value, String> {
    let sel = match name {
        "page" => {
            if !args.is_empty() {
                return Err("page() takes no arguments".into());
            }
            return Ok(match ctx.page()? {
                Some(p) => Value::Str(p),
                None => Value::Null,
            });
        }
        "appear" | "ocr" | "ocr_digit" => {
            if args.len() != 1 {
                return Err(format!("{name}() takes exactly one asset argument"));
            }
            match eval(&args[0], ctx)? {
                Value::Str(s) => s,
                other => return Err(format!("{name}() expects a string asset id, got {other:?}")),
            }
        }
        other => return Err(format!("unknown function `{other}` (allowed: appear, ocr, ocr_digit, page)")),
    };
    match name {
        "appear" => Ok(Value::Bool(ctx.appear(&sel)?)),
        "ocr" => Ok(match ctx.ocr(&sel)? {
            Some(s) => Value::Str(s),
            None => Value::Null,
        }),
        "ocr_digit" => Ok(match ctx.ocr_digit(&sel)? {
            Some(n) => Value::Num(n),
            None => Value::Null,
        }),
        _ => unreachable!(),
    }
}

fn binary(op: BinOp, a: Value, b: Value) -> Result<Value, String> {
    use BinOp::*;
    match op {
        Eq => Ok(Value::Bool(values_eq(&a, &b))),
        Ne => Ok(Value::Bool(!values_eq(&a, &b))),
        Lt | Le | Gt | Ge => {
            let (x, y) = (a.num()?, b.num()?);
            Ok(Value::Bool(match op {
                Lt => x < y,
                Le => x <= y,
                Gt => x > y,
                _ => x >= y,
            }))
        }
        Add | Sub | Mul | Div | Mod => {
            let (x, y) = (a.num()?, b.num()?);
            let v = match op {
                Add => x + y,
                Sub => x - y,
                Mul => x * y,
                Div => {
                    if y == 0.0 {
                        return Err("division by zero in expression".into());
                    }
                    x / y
                }
                _ => {
                    if y == 0.0 {
                        return Err("modulo by zero in expression".into());
                    }
                    x % y
                }
            };
            Ok(Value::Num(v))
        }
        And | Or => unreachable!("handled with short-circuit"),
    }
}

fn values_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Null, Value::Null) => true,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Num(x), Value::Num(y)) => (x - y).abs() < f64::EPSILON,
        (Value::Str(x), Value::Str(y)) => x == y,
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Flow schema
// ---------------------------------------------------------------------------

/// A known popup/interruption rule (ENGINE §22.15).
///
/// The runtime checks `detect` **before every step**; when it matches, `action`
/// is clicked (defaults to `detect` itself — e.g. the close button). Use
/// `action` for popups that must be ACCEPTED (e.g. 悬赏封印邀请 → 「接受」).
#[derive(Debug, Clone, PartialEq)]
pub struct InterruptRule {
    /// Template asset that indicates the popup is present.
    pub detect: String,
    /// Template asset to click to resolve it; `None` => click `detect`.
    pub action: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Flow {
    pub schema_version: u32,
    pub name: String,
    pub steps: Vec<Step>,
    /// Auto-dismiss rules, checked before each step (§22.15).
    pub interrupts: Vec<InterruptRule>,
}

/// One locator in a `click_target` redundancy chain (ENGINE §22.11).
///
/// Priority is **fixed** (image > text > memory), regardless of the array order
/// in `flow.json`; the runtime sorts before trying.
#[derive(Debug, Clone, PartialEq)]
pub enum Locator {
    /// Image recognition: a declared template asset.
    Template {
        sel: String,
        /// Per-locator override; `None` = use the asset's own threshold.
        threshold: Option<f32>,
    },
    /// Text recognition: an OCR asset + the expected line (exact match after trim).
    /// §22.31 H5: optional `require_template` cross-checks that the line's region
    /// also contains a declared template (same text but different button => skip).
    OcrText {
        sel: String,
        text: String,
        require_template: Option<String>,
    },
    /// Remembered coordinate (fallback), keyed in the task's `state/coords.json`.
    CoordMemory { key: String },
}

impl Locator {
    pub fn kind(&self) -> &'static str {
        match self {
            Locator::Template { .. } => "template",
            Locator::OcrText { .. } => "ocr_text",
            Locator::CoordMemory { .. } => "coord_memory",
        }
    }

    /// 0 = highest priority (image), 2 = lowest (memory).
    pub fn priority(&self) -> u8 {
        match self {
            Locator::Template { .. } => 0,
            Locator::OcrText { .. } => 1,
            Locator::CoordMemory { .. } => 2,
        }
    }

    /// Asset id referenced by this locator (memory locators reference no asset).
    pub fn asset(&self) -> Option<&str> {
        match self {
            Locator::Template { sel, .. } | Locator::OcrText { sel, .. } => Some(sel),
            Locator::CoordMemory { .. } => None,
        }
    }
}

/// Page/verification expectation (ENGINE §22.11 补充层).
///
/// Used twice: `requires` = pre-click page guard (the click must NOT happen when
/// unmet), `expect` = post-click verification (the step fails when unmet).
#[derive(Debug, Clone, PartialEq)]
pub enum Expect {
    /// A template must become findable (≥ its threshold) within `timeout_ms`.
    Appear { sel: String, timeout_ms: u32 },
    /// A template must become NOT findable within `timeout_ms` (e.g. popup gone).
    Vanish { sel: String, timeout_ms: u32 },
    /// An OCR line must equal `text` after trim within `timeout_ms`.
    OcrText {
        sel: String,
        text: String,
        timeout_ms: u32,
    },
}

impl Expect {
    pub fn sel(&self) -> &str {
        match self {
            Expect::Appear { sel, .. }
            | Expect::Vanish { sel, .. }
            | Expect::OcrText { sel, .. } => sel,
        }
    }

    pub fn timeout_ms(&self) -> u32 {
        match self {
            Expect::Appear { timeout_ms, .. }
            | Expect::Vanish { timeout_ms, .. }
            | Expect::OcrText { timeout_ms, .. } => *timeout_ms,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Expect::Appear { .. } => "appear",
            Expect::Vanish { .. } => "vanish",
            Expect::OcrText { .. } => "ocr_text",
        }
    }
}

/// Step-level retry policy (ENGINE §22.16 纵向②).
///
/// Attached to action steps that can fail transiently (a template that appears a
/// moment late, a tap lost during an animation). Control-flow steps never take a
/// retry policy: re-running a whole block would replay its side effects.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RetryPolicy {
    /// Extra attempts after the first failure (1..=10).
    pub times: u32,
    /// Base delay before the first retry (ms).
    pub delay_ms: u32,
    /// `true` = exponential backoff (base × 2^(attempt-1), capped at 30s).
    pub exponential: bool,
}

impl RetryPolicy {
    pub fn delay_for_attempt(&self, attempt: u32) -> u32 {
        let base = self.delay_ms as u64;
        let d = if self.exponential {
            base.saturating_mul(1u64 << (attempt.saturating_sub(1)).min(6))
        } else {
            base
        };
        d.min(30_000) as u32
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    /// Structured branch (nested blocks; there is no goto).
    If {        cond: Expr,
        then: Vec<Step>,
        otherwise: Vec<Step>,
    },
    Loop {
        n: Expr,
        body: Vec<Step>,
    },
    While {
        cond: Expr,
        max_iters: u32,
        body: Vec<Step>,
    },
    Break,
    Continue,
    Wait {
        seconds: f64,
    },
    /// `timeout_s` is REQUIRED: a wait without a bound is a load error.
    WaitUntil {
        cond: Expr,
        timeout_s: f64,
    },
    Appear {
        sel: String,
    },
    Click {
        sel: String,
        /// Optional extra template assets for the SAME target, tried in order
        /// after `sel` (ENGINE §22.11 补充层 "多模板变体": skins/animation).
        alts: Vec<String>,
        /// Optional pre-click page guard (ENGINE §22.11 补充层): if the page
        /// precondition is not satisfied, the click never happens.
        requires: Option<Expect>,
        /// Optional post-click verification (ENGINE §22.11 补充层).
        expect: Option<Expect>,
        /// Optional retry with backoff (§22.16).
        retry: Option<RetryPolicy>,
        /// §22.29 多帧投票（H4）: require the SAME target to match on N
        /// consecutive frames (same position ± tolerance) before clicking.
        /// `None` / `Some(1)` = current behaviour (single frame).
        stable_frames: Option<u8>,
    },
    /// §19 CP1: click an absolute frame coordinate computed from expressions.
    ///
    /// This exists because a block produced by multi-line OCR is *not* a
    /// declared asset - it only has a box centre. Both coordinates are
    /// expressions, so "click the N-th row" style logic stays possible.
    ClickPoint {
        x: Expr,
        y: Expr,
        requires: Option<Expect>,
        expect: Option<Expect>,
        retry: Option<RetryPolicy>,
    },
    /// Action-class swipe/scroll (§3.2): drag from one frame point to another.
    ///
    /// Kept deterministic (no jitter/delay): a scroll is a movement, not a
    /// click, and the recorded `ms` is part of the action's identity.
    Swipe {
        from_x: Expr,
        from_y: Expr,
        to_x: Expr,
        to_y: Expr,
        ms: u32,
    },
    Ocr {
        sel: String,
    },
    /// Explicitly resolve a known popup (ENGINE §22.15). When `required` is false
    /// an absent popup is OK; when true it is a loud failure. `action` selects the
    /// button to click (e.g. 接受); `None` => click the detected `sel` itself.
    DismissPopup {
        sel: String,
        action: Option<String>,
        timeout_ms: u32,
        required: bool,
        retry: Option<RetryPolicy>,
    },
    /// ENGINE §22.11: click a target resolved by an ordered redundancy chain
    /// (image > text > remembered coordinate). All locators failing = loud error.
    ClickTarget {
        find: Vec<Locator>,
        /// Optional pre-click page guard; unsatisfied => the click never happens.
        requires: Option<Expect>,
        /// Optional post-click verification (ENGINE §22.11 补充层).
        expect: Option<Expect>,
        retry: Option<RetryPolicy>,
    },
    Log {
        level: String,
        text: Expr,
    },
    Finish {
        exit: Exit,
    },
}

pub const SUPPORTED_SCHEMA: u32 = 1;

fn req_str(v: &serde_json::Value, key: &str, path: &str) -> Result<String, FlowError> {
    v.get(key)
        .and_then(|x| x.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| FlowError::new(path, format!("missing string arg `{key}`")))
}

fn req_f64(v: &serde_json::Value, key: &str, path: &str) -> Result<f64, FlowError> {
    v.get(key)
        .and_then(|x| x.as_f64())
        .ok_or_else(|| FlowError::new(path, format!("missing number arg `{key}`")))
}

fn opt_f64(v: &serde_json::Value, key: &str) -> Option<f64> {
    v.get(key).and_then(|x| x.as_f64())
}

fn parse_expr_field(
    v: &serde_json::Value,
    key: &str,
    path: &str,
) -> Result<Expr, FlowError> {
    let src = req_str(v, key, path)?;
    parse_expr(&src).map_err(|e| FlowError::new(format!("{path}.{key}"), e))
}

/// CP1/CP2: a `click_point` coordinate accepts a JSON number (a literal) or a
/// JSON string (an expression). The distinction is a load-time concern only:
/// both become an [`Expr`].
fn parse_coord_value(v: &serde_json::Value, coord_path: String) -> Result<Expr, FlowError> {
    match v {
        serde_json::Value::Number(n) => n
            .as_f64()
            .map(|f| Expr::Lit(Lit::Num(f)))
            .ok_or_else(|| FlowError::new(coord_path, "coordinate is not a number")),
        serde_json::Value::String(src) => parse_expr(src).map_err(|e| FlowError::new(coord_path, e)),
        other => Err(FlowError::new(
            coord_path,
            format!("coordinate must be a number or an expression string, got {other}"),
        )),
    }
}

fn parse_point_coord(
    v: &serde_json::Value,
    key: &str,
    path: &str,
) -> Result<Expr, FlowError> {
    let coord_path = format!("{path}.{key}");
    match v.get(key) {
        Some(inner) => parse_coord_value(inner, coord_path),
        None => Err(FlowError::new(
            coord_path,
            format!("missing arg `{key}` (a number or an expression string)"),
        )),
    }
}

/// §3.2 swipe endpoints: `[<expr>, <expr>]` (a JSON 2-element array).
fn parse_coord_pair(
    v: &serde_json::Value,
    key: &str,
    path: &str,
) -> Result<(Expr, Expr), FlowError> {
    let pair_path = format!("{path}.{key}");
    let arr = v.get(key).and_then(|x| x.as_array()).ok_or_else(|| {
        FlowError::new(
            pair_path.clone(),
            format!("`{key}` must be a 2-element array of numbers/expressions"),
        )
    })?;
    if arr.len() != 2 {
        return Err(FlowError::new(
            pair_path,
            format!("`{key}` must have exactly 2 elements, got {}", arr.len()),
        ));
    }
    let first = parse_coord_value(&arr[0], format!("{path}.{key}[0]"))?;
    let second = parse_coord_value(&arr[1], format!("{path}.{key}[1]"))?;
    Ok((first, second))
}

/// Parse an optional `alts` array of template asset ids (multi-variant click).
fn parse_alts(args: &serde_json::Value, a_path: &str) -> Result<Vec<String>, FlowError> {
    let p = format!("{a_path}.alts");
    let Some(v) = args.get("alts") else {
        return Ok(Vec::new());
    };
    let arr = v.as_array().ok_or_else(|| {
        FlowError::new(&p, "`alts` must be an array of template asset ids")
    })?;
    let mut out = Vec::with_capacity(arr.len());
    for (i, item) in arr.iter().enumerate() {
        let s = item.as_str().ok_or_else(|| {
            FlowError::new(format!("{p}[{i}]"), "`alts` entries must be strings")
        })?;
        if s.trim().is_empty() {
            return Err(FlowError::new(
                format!("{p}[{i}]"),
                "`alts` entries must be non-empty",
            ));
        }
        out.push(s.to_string());
    }
    Ok(out)
}

/// Parse an optional `retry` object (`{times, delay_ms?, backoff?}`).
/// §22.29 H4: optional `stable_frames` (consecutive matching frames before click).
fn parse_stable_frames(
    args: &serde_json::Value,
    a_path: &str,
) -> Result<Option<u8>, FlowError> {
    let Some(v) = args.get("stable_frames") else {
        return Ok(None);
    };
    let n = v.as_u64().ok_or_else(|| {
        FlowError::new(
            format!("{a_path}.stable_frames"),
            "`stable_frames` must be an integer",
        )
    })?;
    if !(1..=10).contains(&n) {
        return Err(FlowError::new(
            format!("{a_path}.stable_frames"),
            format!("`stable_frames` must be within 1..=10, got {n}"),
        ));
    }
    Ok(Some(n as u8))
}

fn parse_retry(
    args: &serde_json::Value,
    a_path: &str,
) -> Result<Option<RetryPolicy>, FlowError> {
    let p = format!("{a_path}.retry");
    let Some(obj) = args.get("retry") else {
        return Ok(None);
    };
    if !obj.is_object() {
        return Err(FlowError::new(
            &p,
            "`retry` must be an object {times, delay_ms?, backoff?}",
        ));
    }
    let times = obj
        .get("times")
        .and_then(|x| x.as_u64())
        .ok_or_else(|| FlowError::new(format!("{p}.times"), "`retry.times` must be an integer"))?;
    if times == 0 || times > 10 {
        return Err(FlowError::new(
            format!("{p}.times"),
            format!("`retry.times` must be 1..=10, got {times}"),
        ));
    }
    let delay_ms = match obj.get("delay_ms") {
        None => 0u32,
        Some(x) => {
            let n = x.as_u64().ok_or_else(|| {
                FlowError::new(format!("{p}.delay_ms"), "`retry.delay_ms` must be an integer")
            })?;
            if n > 60_000 {
                return Err(FlowError::new(
                    format!("{p}.delay_ms"),
                    format!("`retry.delay_ms` must be <= 60000, got {n}"),
                ));
            }
            n as u32
        }
    };
    let exponential = match obj.get("backoff") {
        None => false,
        Some(x) => match x.as_str() {
            Some("constant") => false,
            Some("exponential") => true,
            _ => {
                return Err(FlowError::new(
                    format!("{p}.backoff"),
                    "`retry.backoff` must be \"constant\" or \"exponential\"",
                ))
            }
        },
    };
    Ok(Some(RetryPolicy {
        times: times as u32,
        delay_ms,
        exponential,
    }))
}

/// Parse an optional `requires` / `expect` object on a click step.
fn parse_expect(args: &serde_json::Value, key: &str, a_path: &str) -> Result<Option<Expect>, FlowError> {
    let Some(obj) = args.get(key) else {
        return Ok(None);
    };
    let p = format!("{a_path}.{key}");
    let kind = req_str(obj, "kind", &p)?;
    let timeout_ms = match obj.get("timeout_ms") {
        None => 1000u32,
        Some(x) => {
            let n = x.as_u64().ok_or_else(|| {
                FlowError::new(format!("{p}.timeout_ms"), "`timeout_ms` must be an integer")
            })?;
            if n == 0 || n > 60_000 {
                return Err(FlowError::new(
                    format!("{p}.timeout_ms"),
                    format!("`timeout_ms` must be 1..=60000, got {n}"),
                ));
            }
            n as u32
        }
    };
    match kind.as_str() {
        "appear" | "vanish" => {
            let sel = req_str(obj, "sel", &p)?;
            Ok(Some(if kind == "appear" {
                Expect::Appear { sel, timeout_ms }
            } else {
                Expect::Vanish { sel, timeout_ms }
            }))
        }
        "ocr_text" => Ok(Some(Expect::OcrText {
            sel: req_str(obj, "sel", &p)?,
            text: req_str(obj, "text", &p)?,
            timeout_ms,
        })),
        other => Err(FlowError::new(
            &p,
            format!("unknown {key} kind `{other}` (allowed: appear, vanish, ocr_text)"),
        )),
    }
}

fn parse_locator(v: &serde_json::Value, path: &str) -> Result<Locator, FlowError> {
    let kind = req_str(v, "kind", path)?;
    match kind.as_str() {
        "template" => {
            let sel = req_str(v, "sel", path)?;
            let threshold = match v.get("threshold") {
                None => None,
                Some(x) => Some(
                    x.as_f64()
                        .ok_or_else(|| FlowError::new(format!("{path}.threshold"), "`threshold` must be a number"))?
                        as f32,
                ),
            };
            Ok(Locator::Template { sel, threshold })
        }
        "ocr_text" => Ok(Locator::OcrText {
            sel: req_str(v, "sel", path)?,
            text: req_str(v, "text", path)?,
            require_template: match v.get("require_template") {
                None => None,
                Some(x) => Some(
                    x.as_str()
                        .ok_or_else(|| {
                            FlowError::new(
                                format!("{path}.require_template"),
                                "`require_template` must be an asset id string",
                            )
                        })?
                        .to_string(),
                ),
            },
        }),
        "coord_memory" => Ok(Locator::CoordMemory {
            key: req_str(v, "key", path)?,
        }),
        other => Err(FlowError::new(
            path,
            format!(
                "unknown locator kind `{other}` (allowed: template, ocr_text, coord_memory)"
            ),
        )),
    }
}

fn parse_locators(args: &serde_json::Value, a_path: &str) -> Result<Vec<Locator>, FlowError> {
    let find_path = format!("{a_path}.find");
    let arr = args
        .get("find")
        .and_then(|x| x.as_array())
        .ok_or_else(|| {
            FlowError::new(&find_path, "`find` must be a non-empty array of locators")
        })?;
    if arr.is_empty() {
        return Err(FlowError::new(
            &find_path,
            "`find` must have at least one locator",
        ));
    }
    let mut out = Vec::with_capacity(arr.len());
    for (i, item) in arr.iter().enumerate() {
        out.push(parse_locator(item, &format!("{find_path}[{i}]"))?);
    }
    Ok(out)
}

fn parse_steps(v: &serde_json::Value, path: &str) -> Result<Vec<Step>, FlowError> {    let arr = v
        .as_array()
        .ok_or_else(|| FlowError::new(path, "expected an array of steps"))?;
    let mut out = Vec::with_capacity(arr.len());
    for (i, item) in arr.iter().enumerate() {
        out.push(parse_step(item, &format!("{path}[{i}]"))?);
    }
    Ok(out)
}

pub fn parse_step(v: &serde_json::Value, path: &str) -> Result<Step, FlowError> {
    let op = req_str(v, "op", path)?;
    let args = v.get("args").cloned().unwrap_or(serde_json::json!({}));
    let a_path = format!("{path}.args");
    match op.as_str() {
        "if" => {
            let cond = parse_expr_field(&args, "cond", &a_path)?;
            let then = parse_steps(
                v.get("then")
                    .ok_or_else(|| FlowError::new(path, "`if` needs `then`"))?,
                &format!("{path}.then"),
            )?;
            let otherwise = match v.get("else") {
                Some(e) => parse_steps(e, &format!("{path}.else"))?,
                None => Vec::new(),
            };
            Ok(Step::If {
                cond,
                then,
                otherwise,
            })
        }
        "loop" => Ok(Step::Loop {
            n: parse_expr_field(&args, "n", &a_path)?,
            body: parse_steps(
                v.get("body")
                    .ok_or_else(|| FlowError::new(path, "`loop` needs `body`"))?,
                &format!("{path}.body"),
            )?,
        }),
        "while" => {
            let max_iters = match args.get("max_iters").and_then(|x| x.as_u64()) {
                Some(n) if n > 0 => n as u32,
                _ => {
                    return Err(FlowError::new(
                        a_path,
                        "`while` requires `max_iters` > 0 (no unbounded loops)",
                    ))
                }
            };
            Ok(Step::While {
                cond: parse_expr_field(&args, "cond", &a_path)?,
                max_iters,
                body: parse_steps(
                    v.get("body")
                        .ok_or_else(|| FlowError::new(path, "`while` needs `body`"))?,
                    &format!("{path}.body"),
                )?,
            })
        }
        "break" => Ok(Step::Break),
        "continue" => Ok(Step::Continue),
        "wait" => Ok(Step::Wait {
            seconds: req_f64(&args, "seconds", &a_path)?,
        }),
        "wait_until" => {
            // timeout is mandatory; a default of "infinite" is exactly the bug
            // class this rewrite exists to kill.
            let timeout_s = match opt_f64(&args, "timeout_s") {
                Some(t) if t > 0.0 => t,
                _ => {
                    return Err(FlowError::new(
                        a_path,
                        "`wait_until` requires `timeout_s` > 0 (no unbounded waits)",
                    ))
                }
            };
            Ok(Step::WaitUntil {
                cond: parse_expr_field(&args, "cond", &a_path)?,
                timeout_s,
            })
        }
        "appear" => Ok(Step::Appear {
            sel: req_str(&args, "sel", &a_path)?,
        }),
        "click" => Ok(Step::Click {
            sel: req_str(&args, "sel", &a_path)?,
            alts: parse_alts(&args, &a_path)?,
            requires: parse_expect(&args, "requires", &a_path)?,
            expect: parse_expect(&args, "expect", &a_path)?,
            retry: parse_retry(&args, &a_path)?,
            stable_frames: parse_stable_frames(&args, &a_path)?,
        }),
        "click_point" => Ok(Step::ClickPoint {
            x: parse_point_coord(&args, "x", &a_path)?,
            y: parse_point_coord(&args, "y", &a_path)?,
            requires: parse_expect(&args, "requires", &a_path)?,
            expect: parse_expect(&args, "expect", &a_path)?,
            retry: parse_retry(&args, &a_path)?,
        }),
        "swipe" => {
            let (from_x, from_y) = parse_coord_pair(&args, "from", &a_path)?;
            let (to_x, to_y) = parse_coord_pair(&args, "to", &a_path)?;
            // `ms` is optional, defaults to 300; 0 would be an instantaneous
            // teleport, not a swipe.
            let ms = match args.get("ms") {
                None => 300u32,
                Some(v) => {
                    let n = v.as_f64().ok_or_else(|| {
                        FlowError::new(format!("{a_path}.ms"), "`ms` must be a number")
                    })?;
                    if !n.is_finite() || n <= 0.0 {
                        return Err(FlowError::new(
                            format!("{a_path}.ms"),
                            format!("`ms` must be > 0, got {n}"),
                        ));
                    }
                    n.round() as u32
                }
            };
            Ok(Step::Swipe {
                from_x,
                from_y,
                to_x,
                to_y,
                ms,
            })
        }
        "click_target" => Ok(Step::ClickTarget {
            find: parse_locators(&args, &a_path)?,
            requires: parse_expect(&args, "requires", &a_path)?,
            expect: parse_expect(&args, "expect", &a_path)?,
            retry: parse_retry(&args, &a_path)?,
        }),
        "dismiss_popup" => {
            let sel = req_str(&args, "sel", &a_path)?;
            let action = match args.get("action") {
                None => None,
                Some(x) => Some(
                    x.as_str()
                        .ok_or_else(|| {
                            FlowError::new(format!("{a_path}.action"), "`action` must be a string")
                        })?
                        .to_string(),
                ),
            };
            let timeout_ms = match args.get("timeout_ms") {
                None => 0u32,
                Some(x) => {
                    let n = x.as_u64().ok_or_else(|| {
                        FlowError::new(format!("{a_path}.timeout_ms"), "`timeout_ms` must be an integer")
                    })?;
                    if n > 60_000 {
                        return Err(FlowError::new(
                            format!("{a_path}.timeout_ms"),
                            format!("`timeout_ms` must be <= 60000, got {n}"),
                        ));
                    }
                    n as u32
                }
            };
            let required = match args.get("required") {
                None => false,
                Some(x) => x.as_bool().ok_or_else(|| {
                    FlowError::new(format!("{a_path}.required"), "`required` must be a boolean")
                })?,
            };
            Ok(Step::DismissPopup {
                sel,
                action,
                timeout_ms,
                required,
                retry: parse_retry(&args, &a_path)?,
            })
        }
        "ocr" => Ok(Step::Ocr {
            sel: req_str(&args, "sel", &a_path)?,
        }),
        "log" => Ok(Step::Log {
            level: args
                .get("level")
                .and_then(|x| x.as_str())
                .unwrap_or("info")
                .to_string(),
            text: parse_expr_field(&args, "text", &a_path)?,
        }),
        "finish" => {
            let name = req_str(&args, "exit", &a_path)?;
            Ok(Step::Finish {
                exit: parse_exit(&name)
                    .map_err(|e| FlowError::new(format!("{a_path}.exit"), e))?,
            })
        }
        other => Err(FlowError::new(
            path,
            format!(
                "unknown instruction `{other}` (supported: if/loop/while/break/continue/\
                 wait/wait_until/appear/click/click_point/click_target/dismiss_popup/swipe/ocr/log/finish)"
            ),
        )),
    }
}

pub fn parse_exit(name: &str) -> Result<Exit, String> {
    Ok(match name {
        "done" => Exit::Done,
        "no_resource" => Exit::NoResource,
        "window_closed" => Exit::WindowClosed,
        "no_target" => Exit::NoTarget,
        "resource_empty" => Exit::ResourceEmpty,
        "exhausted" => Exit::Exhausted,
        "failed" => Exit::Failed,
        "skipped" => Exit::Skipped,
        other => {
            return Err(format!(
                "unknown exit `{other}` (8 allowed: done/no_resource/window_closed/\
                 no_target/resource_empty/exhausted/failed/skipped)"
            ))
        }
    })
}

impl Flow {
    pub fn from_json(text: &str) -> Result<Self, FlowError> {
        let v: serde_json::Value =
            serde_json::from_str(text).map_err(|e| FlowError::new("$", format!("invalid JSON: {e}")))?;
        let schema_version = v
            .get("schema_version")
            .and_then(|x| x.as_u64())
            .unwrap_or(0) as u32;
        if schema_version != SUPPORTED_SCHEMA {
            return Err(FlowError::new(
                "$.schema_version",
                format!("unsupported schema_version {schema_version} (expected {SUPPORTED_SCHEMA})"),
            ));
        }
        let name = req_str(&v, "name", "$")?;
        let steps = parse_steps(
            v.get("steps")
                .ok_or_else(|| FlowError::new("$", "missing `steps`"))?,
            "steps",
        )?;
        // ENGINE §22.15: optional list of known popups dismissed before each step.
        let interrupts = match v.get("interrupts") {
            None => Vec::new(),
            Some(x) => {
                let arr = x.as_array().ok_or_else(|| {
                    FlowError::new(
                        "$.interrupts",
                        "`interrupts` must be an array of {detect, action?} rules",
                    )
                })?;
                arr.iter()
                    .enumerate()
                    .map(|(i, item)| {
                        let p = format!("$.interrupts[{i}]");
                        // `detect` is canonical; `sel` is the legacy alias.
                        let detect = if item.get("detect").is_some() {
                            req_str(item, "detect", &p)?
                        } else {
                            req_str(item, "sel", &p)?
                        };
                        let action = match item.get("action") {
                            None => None,
                            Some(x) => Some(
                                x.as_str()
                                    .ok_or_else(|| {
                                        FlowError::new(
                                            format!("{p}.action"),
                                            "`action` must be a string",
                                        )
                                    })?
                                    .to_string(),
                            ),
                        };
                        Ok(InterruptRule { detect, action })
                    })
                    .collect::<Result<Vec<_>, FlowError>>()?
            }
        };
        Ok(Self {
            schema_version,
            name,
            steps,
            interrupts,
        })
    }

    /// Collect every asset id referenced by any expression or selector.
    pub fn referenced_assets(&self) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        collect_assets_steps(&self.steps, &mut out);
        for rule in &self.interrupts {
            out.insert(rule.detect.clone());
            if let Some(a) = &rule.action {
                out.insert(a.clone());
            }
        }
        out
    }

    /// Reject references to assets that the task does not declare.
    ///
    /// This is the "save-time error" rule: a typo in an asset id must never
    /// survive until the run that needs it.
    pub fn validate_assets(&self, known: &BTreeSet<String>) -> Result<(), Vec<FlowError>> {
        let mut errs = Vec::new();
        for a in self.referenced_assets() {
            // §22.22: `lib:<id>` selectors come from the GLOBAL asset library, not
            // the task bundle; the runtime validates them when it loads the library.
            if a.starts_with("lib:") {
                continue;
            }
            if !known.contains(&a) {
                errs.push(FlowError::new(
                    "assets",
                    format!("asset `{a}` is referenced but not declared"),
                ));
            }
        }
        if errs.is_empty() {
            Ok(())
        } else {
            Err(errs)
        }
    }
}

fn collect_assets_expr(e: &Expr, out: &mut BTreeSet<String>) {
    match e {
        Expr::Lit(_) => {}
        Expr::Call(name, args) => {
            if matches!(name.as_str(), "appear" | "ocr" | "ocr_digit") {
                if let Some(Expr::Lit(Lit::Str(s))) = args.first() {
                    out.insert(s.clone());
                }
            }
            for a in args {
                collect_assets_expr(a, out);
            }
        }
        Expr::Not(x) | Expr::Neg(x) => collect_assets_expr(x, out),
        Expr::Binary(_, a, b) => {
            collect_assets_expr(a, out);
            collect_assets_expr(b, out);
        }
    }
}

fn collect_assets_steps(steps: &[Step], out: &mut BTreeSet<String>) {
    for s in steps {
        match s {
            Step::If {
                cond,
                then,
                otherwise,
            } => {
                collect_assets_expr(cond, out);
                collect_assets_steps(then, out);
                collect_assets_steps(otherwise, out);
            }
            Step::Loop { n, body } => {
                collect_assets_expr(n, out);
                collect_assets_steps(body, out);
            }
            Step::While {
                cond, body, ..
            } => {
                collect_assets_expr(cond, out);
                collect_assets_steps(body, out);
            }
            Step::Wait { .. } | Step::Break | Step::Continue => {}
            Step::WaitUntil { cond, .. } => collect_assets_expr(cond, out),
            Step::Appear { sel } | Step::Ocr { sel } => {
                out.insert(sel.clone());
            }
            Step::DismissPopup { sel, action, .. } => {
                out.insert(sel.clone());
                if let Some(a) = action {
                    out.insert(a.clone());
                }
            }
            Step::Click {
                sel,
                alts,
                requires,
                expect,
                ..
            } => {
                out.insert(sel.clone());
                for a in alts {
                    out.insert(a.clone());
                }
                for e in requires.iter().chain(expect.iter()) {
                    out.insert(e.sel().to_string());
                }
            }
            Step::ClickTarget {
                find,
                requires,
                expect,
                ..
            } => {
                for loc in find {
                    if let Some(a) = loc.asset() {
                        out.insert(a.to_string());
                    }
                    // §22.31 H5: the cross-check template is also a referenced asset.
                    if let Locator::OcrText {
                        require_template: Some(rt),
                        ..
                    } = loc
                    {
                        out.insert(rt.clone());
                    }
                }
                for e in requires.iter().chain(expect.iter()) {
                    out.insert(e.sel().to_string());
                }
            }
            Step::ClickPoint {
                x,
                y,
                requires,
                expect,
                ..
            } => {
                collect_assets_expr(x, out);
                collect_assets_expr(y, out);
                for e in requires.iter().chain(expect.iter()) {
                    out.insert(e.sel().to_string());
                }
            }
            Step::Swipe {
                from_x,
                from_y,
                to_x,
                to_y,
                ..
            } => {
                collect_assets_expr(from_x, out);
                collect_assets_expr(from_y, out);
                collect_assets_expr(to_x, out);
                collect_assets_expr(to_y, out);
            }
            Step::Log { text, .. } => collect_assets_expr(text, out),
            Step::Finish { .. } => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- expression parser ----

    #[test]
    fn parses_precedence_and_parens() {
        let e = parse_expr("1 + 2 * 3 == 7 and not false").expect("parse");
        // shape check: Binary(And, cmp, Not)
        match e {
            Expr::Binary(BinOp::And, _, rhs) => {
                assert_eq!(*rhs, Expr::Not(Box::new(Expr::Lit(Lit::Bool(false)))))
            }
            other => panic!("unexpected parse: {other:?}"),
        }
    }

    #[test]
    fn rejects_bare_identifiers_and_unknown_chars() {
        assert!(parse_expr("foo").is_err());
        assert!(parse_expr("a & b").is_err());
    }

    #[test]
    fn rejects_string_call_with_non_string_asset() {
        // appear() with a numeric arg must fail at runtime-check level (eval),
        // but parsing is fine.
        assert!(parse_expr("appear(1)").is_ok());
    }

    #[test]
    fn collects_referenced_assets() {
        let flow = Flow::from_json(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"if","args":{"cond":"appear('I_A') and ocr('O_B') == 'x'"},
                  "then":[{"op":"click","args":{"sel":"I_C"}}],
                  "else":[{"op":"log","args":{"text":"ocr('O_B')"}}]}
               ]}"#,
        )
        .expect("flow");
        let got: Vec<String> = flow.referenced_assets().into_iter().collect();
        assert_eq!(got, vec!["I_A", "I_C", "O_B"]);
    }

    #[test]
    fn ocr_text_require_template_is_parsed_and_collected() {
        let flow = Flow::from_json(
            r#"{"schema_version":1,"name":"t","steps":[{"op":"click_target","args":{
                 "find":[{"kind":"ocr_text","sel":"O_LIST","text":"确认","require_template":"I_ICON"}]}}]}"#,
        )
        .expect("flow");
        let refs = flow.referenced_assets();
        assert!(refs.contains("O_LIST"), "{refs:?}");
        assert!(refs.contains("I_ICON"), "require_template asset must be collected: {refs:?}");
        match &flow.steps[0] {
            Step::ClickTarget { find, .. } => match &find[0] {
                Locator::OcrText {
                    require_template: Some(rt),
                    ..
                } => assert_eq!(rt, "I_ICON"),
                other => panic!("unexpected locator: {other:?}"),
            },
            other => panic!("unexpected step: {other:?}"),
        }
        // wrong type is rejected at parse time
        let bad = r#"{"schema_version":1,"name":"t","steps":[{"op":"click_target","args":{
            "find":[{"kind":"ocr_text","sel":"O_LIST","text":"确认","require_template":1}]}}]}"#;
        assert!(Flow::from_json(bad).is_err());
    }

    #[test]
    fn parses_click_target_and_collects_both_asset_locators() {
        let flow = Flow::from_json(
            r#"{"schema_version":1,"name":"t","steps":[{"op":"click_target","args":{"find":[
                 {"kind":"coord_memory","key":"返回"},
                 {"kind":"ocr_text","sel":"O_LIST","text":"返回"},
                 {"kind":"template","sel":"I_BACK","threshold":0.9}]}}]}"#,
        )
        .expect("flow");
        match &flow.steps[0] {
            Step::ClickTarget { find, .. } => {
                assert_eq!(find.len(), 3);
                assert_eq!(
                    find[1],
                    Locator::OcrText {
                        sel: "O_LIST".to_string(),
                        text: "返回".to_string(),
                        require_template: None,
                    }
                );
                // Priority is fixed by kind, not by the declared array order.
                assert_eq!(find[2].priority(), 0);
                assert_eq!(find[0].priority(), 2);
                assert_eq!(find[1].asset(), Some("O_LIST"));
                assert_eq!(find[0].asset(), None);
            }
            other => panic!("unexpected step: {other:?}"),
        }
        let got: Vec<String> = flow.referenced_assets().into_iter().collect();
        assert_eq!(got, vec!["I_BACK", "O_LIST"]);
    }

    #[test]
    fn rejects_click_target_with_empty_or_bad_locators() {
        let bad_finds = [
            "[]",
            r#"[{"kind":"magic"}]"#,
            r#"[{"kind":"template"}]"#,
            r#"[{"kind":"ocr_text","sel":"O"}]"#,
            r#"[{"kind":"coord_memory"}]"#,
        ];
        for find in bad_finds {
            let json = format!(
                r#"{{"schema_version":1,"name":"t","steps":[{{"op":"click_target","args":{{"find":{find}}}}}]}}"#
            );
            assert!(!errs(&json).is_empty(), "must reject find={find}");
        }
    }

    #[test]
    fn parses_requires_and_expect_and_collects_their_assets() {
        let flow = Flow::from_json(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click","args":{"sel":"预设",
                   "requires":{"kind":"appear","sel":"式神录","timeout_ms":2000},
                   "expect":{"kind":"vanish","sel":"关闭"}}},
                 {"op":"click_point","args":{"x":1,"y":2,
                   "requires":{"kind":"ocr_text","sel":"顶栏","text":"式神录"}}},
                 {"op":"click_target","args":{
                   "find":[{"kind":"template","sel":"返回"}],
                   "requires":{"kind":"appear","sel":"分组列表"},
                   "expect":{"kind":"ocr_text","sel":"分组列表","text":"分组2"}}}]}"#,
        )
        .expect("flow");
        match &flow.steps[0] {
            Step::Click {
                requires: Some(Expect::Appear { sel, timeout_ms }),
                expect: Some(Expect::Vanish { sel: vsel, .. }),
                ..
            } => {
                assert_eq!(sel, "式神录");
                assert_eq!(*timeout_ms, 2000);
                assert_eq!(vsel, "关闭");
            }
            other => panic!("unexpected {other:?}"),
        }
        match &flow.steps[2] {
            Step::ClickTarget {
                requires: Some(Expect::Appear { .. }),
                expect: Some(Expect::OcrText { .. }),
                ..
            } => {}
            other => panic!("unexpected {other:?}"),
        }
        let got: Vec<String> = flow.referenced_assets().into_iter().collect();
        assert_eq!(
            got,
            vec!["关闭", "分组列表", "式神录", "返回", "顶栏", "预设"]
        );
    }

    #[test]
    fn rejects_bad_requires_and_expect() {
        let bad = [
            r#"{"kind":"magic","sel":"X"}"#,
            r#"{"kind":"appear"}"#,
            r#"{"kind":"ocr_text","sel":"O"}"#,
            r#"{"kind":"appear","sel":"X","timeout_ms":0}"#,
            r#"{"kind":"appear","sel":"X","timeout_ms":999999}"#,
        ];
        for key in ["requires", "expect"] {
            for spec in bad {
                let json = format!(
                    r#"{{"schema_version":1,"name":"t","steps":[{{"op":"click","args":{{"sel":"S","{key}":{spec}}}}}]}}"#
                );
                assert!(!errs(&json).is_empty(), "must reject {key}={spec}");
            }
        }
    }

    #[test]
    fn parses_click_alts_and_collects_them() {
        let flow = Flow::from_json(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click","args":{"sel":"式神录","alts":["式神录_夜","式神录_昼"]}}]}"#,
        )
        .expect("flow");
        match &flow.steps[0] {
            Step::Click { alts, .. } => {
                assert_eq!(alts, &vec!["式神录_夜".to_string(), "式神录_昼".to_string()]);
            }
            other => panic!("unexpected {other:?}"),
        }
        let got: Vec<String> = flow.referenced_assets().into_iter().collect();
        assert_eq!(got, vec!["式神录", "式神录_夜", "式神录_昼"]);
    }

    #[test]
    fn rejects_bad_alts() {
        for spec in [r#""x""#, "[1]", r#"[""]"#] {
            let json = format!(
                r#"{{"schema_version":1,"name":"t","steps":[{{"op":"click","args":{{"sel":"S","alts":{spec}}}}}]}}"#
            );
            assert!(!errs(&json).is_empty(), "must reject alts={spec}");
        }
    }

    #[test]
    fn parses_interrupts_and_dismiss_popup() {
        let flow = Flow::from_json(
            r#"{"schema_version":1,"name":"t",
                 "interrupts":[
                   {"sel":"公告关闭"},
                   {"detect":"悬赏封印邀请","action":"接受"},
                   {"detect":"网络异常"}],
                 "steps":[
                   {"op":"dismiss_popup","args":{"sel":"公告关闭","action":"接受","timeout_ms":2000,"required":true}},
                   {"op":"finish","args":{"exit":"done"}}]}"#,
        )
        .expect("flow");
        assert_eq!(
            flow.interrupts,
            vec![
                InterruptRule {
                    detect: "公告关闭".to_string(),
                    action: None
                },
                InterruptRule {
                    detect: "悬赏封印邀请".to_string(),
                    action: Some("接受".to_string())
                },
                InterruptRule {
                    detect: "网络异常".to_string(),
                    action: None
                }
            ]
        );
        match &flow.steps[0] {
            Step::DismissPopup {
                sel,
                action,
                timeout_ms,
                required,
                ..
            } => {
                assert_eq!(sel, "公告关闭");
                assert_eq!(action.as_deref(), Some("接受"));
                assert_eq!(*timeout_ms, 2000);
                assert!(*required);
            }
            other => panic!("unexpected {other:?}"),
        }
        let got: Vec<String> = flow.referenced_assets().into_iter().collect();
        assert_eq!(got, vec!["公告关闭", "悬赏封印邀请", "接受", "网络异常"]);
    }

    #[test]
    fn rejects_bad_interrupts() {
        for interrupts in [
            r#"{}}"#,
            r#"[{}]"#,
            r#"[{"sel":1}]"#,
            r#"[{"detect":"D","action":1}]"#,
        ] {
            let json = format!(
                r#"{{"schema_version":1,"name":"t","interrupts":{interrupts},
                    "steps":[{{"op":"finish","args":{{"exit":"done"}}}}]}}"#
            );
            assert!(!errs(&json).is_empty(), "must reject interrupts={interrupts}");
        }
    }

    #[test]
    fn parses_step_retry_and_backoff() {
        let flow = Flow::from_json(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click","args":{"sel":"I","retry":{"times":2,"delay_ms":100,"backoff":"exponential"}}},
                 {"op":"click_point","args":{"x":1,"y":2,"retry":{"times":1}}}]}"#,
        )
        .expect("flow");
        match &flow.steps[0] {
            Step::Click {
                retry: Some(policy),
                ..
            } => {
                assert_eq!(*policy, RetryPolicy {
                    times: 2,
                    delay_ms: 100,
                    exponential: true
                });
                assert_eq!(policy.delay_for_attempt(1), 100);
                assert_eq!(policy.delay_for_attempt(2), 200);
                assert_eq!(policy.delay_for_attempt(3), 400);
            }
            other => panic!("unexpected {other:?}"),
        }
        match &flow.steps[1] {
            Step::ClickPoint {
                retry: Some(policy),
                ..
            } => assert_eq!(policy.times, 1),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn rejects_bad_retry() {
        for retry in [
            r#""x""#,
            r#"{"times":0}"#,
            r#"{"times":11}"#,
            r#"{"times":1,"delay_ms":60001}"#,
            r#"{"times":1,"backoff":"linear"}"#,
        ] {
            let json = format!(
                r#"{{"schema_version":1,"name":"t","steps":[{{"op":"click","args":{{"sel":"S","retry":{retry}}}}}]}}"#
            );
            assert!(!errs(&json).is_empty(), "must reject retry={retry}");
        }
    }

    // ---- schema validation ----

    fn errs(json: &str) -> Vec<FlowError> {
        match Flow::from_json(json) {
            Ok(_) => Vec::new(),
            Err(e) => vec![e],
        }
    }

    #[test]
    fn rejects_unknown_instruction() {
        let e = errs(r#"{"schema_version":1,"name":"t","steps":[{"op":"teleport"}]}"#);
        assert_eq!(e.len(), 1);
        assert!(e[0].msg.contains("unknown instruction"), "{}", e[0]);
    }

    #[test]
    fn rejects_wait_until_without_timeout() {
        let e = errs(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"wait_until","args":{"cond":"appear('X')"}}]}"#,
        );
        assert!(e[0].msg.contains("timeout_s"), "{}", e[0]);
    }

    #[test]
    fn rejects_while_without_max_iters() {
        let e = errs(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"while","args":{"cond":"true"},"body":[]}]}"#,
        );
        assert!(e[0].msg.contains("max_iters"), "{}", e[0]);
    }

    #[test]
    fn rejects_bad_schema_version() {
        let e = errs(r#"{"schema_version":9,"name":"t","steps":[]}"#);
        assert!(e[0].msg.contains("schema_version"), "{}", e[0]);
    }

    #[test]
    fn rejects_unknown_exit_name() {
        let e = errs(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"finish","args":{"exit":"exploded"}}]}"#,
        );
        assert!(e[0].msg.contains("unknown exit"), "{}", e[0]);
    }

    #[test]
    fn reports_json_path_for_nested_error() {
        let e = errs(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"if","args":{"cond":"true"},
                  "then":[{"op":"look_ma_no_hands"}]}]}"#,
        );
        assert_eq!(e[0].path, "steps[0].then[0]");
    }

    #[test]
    fn validate_assets_accepts_library_selectors_without_declaring_them() {
        let flow = Flow::from_json(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click","args":{"sel":"lib:预设入口"}},
                 {"op":"log","args":{"text":"appear('lib:关闭')"}}]}"#,
        )
        .expect("flow");
        flow.validate_assets(&BTreeSet::new())
            .expect("lib: refs are external, not task assets");
        assert_eq!(flow.referenced_assets().len(), 2);
    }

    #[test]
    fn validate_assets_catches_typos() {
        let flow = Flow::from_json(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click","args":{"sel":"I_FIRE"}},
                 {"op":"log","args":{"text":"appear('I_TYPO')"}}]}"#,
        )
        .expect("flow");
        let known: BTreeSet<String> = ["I_FIRE".to_string()].into_iter().collect();
        let e = flow.validate_assets(&known).expect_err("must reject typo");
        assert_eq!(e.len(), 1);
        assert!(e[0].msg.contains("I_TYPO"), "{}", e[0]);
    }

    // ---- click_point (design doc §19 CP1/CP2) ----

    #[test]
    fn click_stable_frames_is_parsed_and_validated() {
        let flow = Flow::from_json(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click","args":{"sel":"I_A","stable_frames":2}}]}"#,
        )
        .expect("flow");
        match &flow.steps[0] {
            Step::Click {
                stable_frames: Some(2),
                ..
            } => {}
            other => panic!("stable_frames not parsed: {other:?}"),
        }
        for bad in ["0", "11", "1.5"] {
            let json = format!(
                r#"{{"schema_version":1,"name":"t","steps":[
                     {{"op":"click","args":{{"sel":"I_A","stable_frames":{bad}}}}}]}}"#
            );
            assert!(Flow::from_json(&json).is_err(), "must reject {bad}");
        }
    }

    #[test]
    fn click_point_accepts_numeric_literals() {
        let flow = Flow::from_json(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click_point","args":{"x":20,"y":15}}]}"#,
        )
        .expect("flow");
        match &flow.steps[0] {
            Step::ClickPoint { x, y, .. } => {
                assert_eq!(*x, Expr::Lit(Lit::Num(20.0)));
                assert_eq!(*y, Expr::Lit(Lit::Num(15.0)));
            }
            other => panic!("expected ClickPoint, got {other:?}"),
        }
        assert!(flow.referenced_assets().is_empty(), "literals reference no assets");
    }

    #[test]
    fn click_point_accepts_expression_strings_and_collects_assets() {
        let flow = Flow::from_json(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click_point","args":{"x":"ocr_digit('体力') * 10 + 5","y":"120"}}]}"#,
        )
        .expect("flow");
        match &flow.steps[0] {
            Step::ClickPoint { x, y, .. } => {
                // first arg is a parsed call, not a literal string
                assert!(
                    matches!(x, Expr::Binary(BinOp::Add, _, _)),
                    "expected parsed expression, got {x:?}"
                );
                assert_eq!(*y, Expr::Lit(Lit::Num(120.0)));
            }
            other => panic!("expected ClickPoint, got {other:?}"),
        }
        let got: Vec<String> = flow.referenced_assets().into_iter().collect();
        assert_eq!(got, vec!["体力"]);
    }

    #[test]
    fn click_point_missing_coordinate_is_a_load_error() {
        let e = errs(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click_point","args":{"y":10}}]}"#,
        );
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].path, "steps[0].args.x");
        assert!(e[0].msg.contains("missing arg `x`"), "{}", e[0]);
    }

    #[test]
    fn click_point_bad_expression_string_is_a_load_error_with_exact_path() {
        let e = errs(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click_point","args":{"x":"15 +","y":10}}]}"#,
        );
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].path, "steps[0].args.x");
        assert!(e[0].msg.contains("unexpected token"), "{}", e[0]);
    }

    #[test]
    fn click_point_coordinate_of_bad_type_is_a_load_error() {
        let e = errs(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click_point","args":{"x":true,"y":10}}]}"#,
        );
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].path, "steps[0].args.x");
    }

    #[test]
    fn unknown_instruction_message_lists_click_point() {
        let e = errs(r#"{"schema_version":1,"name":"t","steps":[{"op":"teleport"}]}"#);
        assert!(e[0].msg.contains("click_point"), "{}", e[0]);
        assert!(e[0].msg.contains("swipe"), "{}", e[0]);
    }

    // ---- swipe (§3.2 action class) ----

    #[test]
    fn swipe_parses_numbers_expression_strings_and_default_ms() {
        let flow = Flow::from_json(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"swipe","args":{"from":[10,20],"to":["5 * 2","480 / 2"],"ms":450}},
                 {"op":"swipe","args":{"from":[0,0],"to":[63,47]}}]}"#,
        )
        .expect("flow");
        match &flow.steps[0] {
            Step::Swipe {
                from_x,
                from_y,
                to_x,
                to_y,
                ms,
            } => {
                assert_eq!(*from_x, Expr::Lit(Lit::Num(10.0)));
                assert_eq!(*from_y, Expr::Lit(Lit::Num(20.0)));
                assert!(matches!(to_x, Expr::Binary(BinOp::Mul, _, _)), "{to_x:?}");
                assert!(matches!(to_y, Expr::Binary(BinOp::Div, _, _)), "{to_y:?}");
                assert_eq!(*ms, 450);
            }
            other => panic!("expected Swipe, got {other:?}"),
        }
        match &flow.steps[1] {
            Step::Swipe { ms, .. } => assert_eq!(*ms, 300, "ms defaults to 300"),
            other => panic!("expected Swipe, got {other:?}"),
        }
        assert!(flow.referenced_assets().is_empty(), "literal swipe references no assets");
    }

    #[test]
    fn swipe_collects_assets_from_expression_endpoints() {
        let flow = Flow::from_json(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"swipe","args":{"from":["appear('I_A')","ocr_digit('体力')"],
                                       "to":[63,47]}}]}"#,
        )
        .expect("flow");
        let got: Vec<String> = flow.referenced_assets().into_iter().collect();
        assert_eq!(got, vec!["I_A", "体力"]);
    }

    #[test]
    fn swipe_rejects_bad_endpoint_arrays() {
        let e = errs(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"swipe","args":{"from":[10],"to":[1,2]}}]}"#,
        );
        assert_eq!(e[0].path, "steps[0].args.from");
        assert!(e[0].msg.contains("2 elements"), "{}", e[0]);

        let e = errs(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"swipe","args":{"from":"10,20","to":[1,2]}}]}"#,
        );
        assert_eq!(e[0].path, "steps[0].args.from");
        assert!(e[0].msg.contains("2-element array"), "{}", e[0]);

        let e = errs(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"swipe","args":{"from":[10,20]}}]}"#,
        );
        assert_eq!(e[0].path, "steps[0].args.to");
    }

    #[test]
    fn swipe_rejects_non_positive_or_non_numeric_ms() {
        let e = errs(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"swipe","args":{"from":[0,0],"to":[10,10],"ms":0}}]}"#,
        );
        assert_eq!(e[0].path, "steps[0].args.ms");
        assert!(e[0].msg.contains("> 0"), "{}", e[0]);

        let e = errs(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"swipe","args":{"from":[0,0],"to":[10,10],"ms":"fast"}}]}"#,
        );
        assert_eq!(e[0].path, "steps[0].args.ms");
    }
}
