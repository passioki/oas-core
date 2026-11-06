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

use std::collections::{BTreeMap, BTreeSet};
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
            "&&" => { out.push(Tok::And); i += 2; continue; }
            "||" => { out.push(Tok::Or); i += 2; continue; }
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
            '!' => Some(Tok::Not),
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

/// Convert a JSON scalar (`null`/bool/number/string) into an expression [`Value`].
///
/// `with_params.defaults` uses this: a default is a literal, never an expression
/// (expressions belong in `params`, which are evaluated in the caller scope).
/// Anything structured (array/object) is a loud error — defaults must be scalars.
pub fn json_to_value(v: &serde_json::Value) -> Result<Value, String> {
    match v {
        serde_json::Value::Null => Ok(Value::Null),
        serde_json::Value::Bool(b) => Ok(Value::Bool(*b)),
        serde_json::Value::Number(n) => n
            .as_f64()
            .map(Value::Num)
            .ok_or_else(|| format!("number `{n}` cannot be represented as f64")),
        serde_json::Value::String(s) => Ok(Value::Str(s.clone())),
        other => Err(format!(
            "must be a null/bool/number/string literal, got `{other}`"
        )),
    }
}

/// Everything an expression may observe. Implemented by the interpreter (P1-b).
pub trait EvalContext {
    fn appear(&mut self, sel: &str) -> Result<bool, String>;
    fn ocr(&mut self, sel: &str) -> Result<Option<String>, String>;
    fn ocr_digit(&mut self, sel: &str) -> Result<Option<f64>, String>;
    fn page(&mut self) -> Result<Option<String>, String>;

    /// `text_visible('文字')` — full-screen multi-line OCR text search.
    /// This is deliberately independent of any asset (user 10-31): text equals
    /// means "this text is visible anywhere on the screen", not "read this ROI".
    fn text_visible(&mut self, _text: &str) -> Result<bool, String> {
        Err("text_visible() is only available during a flow run".to_string())
    }

    /// `param('key')` — one declared task parameter (ENGINE §22.34.2, M0).
    ///
    /// The default implementation is a **loud error**, not a silent null: an
    /// `EvalContext` that was not wired to a parameter map (e.g. an old/offline
    /// context) must never make `param('x')` look like "no value provided".
    /// The interpreter overrides this with the bundle defaults + `--param`
    /// values injected by the driver.
    fn param(&mut self, key: &str) -> Result<Value, String> {
        Err(format!(
            "param('{key}') is not available in this evaluation context \
             (declare it in meta.params and run through the interpreter)"
        ))
    }

    /// `var('name')` (v1.2): a runtime variable set by `set_var`/`inc_var`/
    /// `ocr_to_var`. Unset variables are a loud error, not null.
    fn var(&mut self, name: &str) -> Result<Value, String> {
        Err(format!(
            "var('{name}') is not available in this evaluation context \
             (variables are only set during a flow run)"
        ))
    }
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
    // `param('key')` (M0): a declared task parameter, resolved by the context.
    if name == "param" {
        if args.len() != 1 {
            return Err("param() takes exactly one key argument".into());
        }
        let key = match eval(&args[0], ctx)? {
            Value::Str(s) => s,
            other => return Err(format!("param() expects a string key, got {other:?}")),
        };
        return ctx.param(&key);
    }
    // `var('name')` (v1.2): runtime variable set by set_var/inc_var/ocr_to_var.
    if name == "var" {
        if args.len() != 1 {
            return Err("var() takes exactly one name argument".into());
        }
        let key = match eval(&args[0], ctx)? {
            Value::Str(s) => s,
            other => return Err(format!("var() expects a string name, got {other:?}")),
        };
        return ctx.var(&key);
    }
    if name == "text_visible" {
        if args.len() != 1 {
            return Err("text_visible() takes exactly one text argument".into());
        }
        let text = match eval(&args[0], ctx)? {
            Value::Str(s) => s,
            other => return Err(format!("text_visible() expects a string, got {other:?}")),
        };
        return ctx.text_visible(&text).map(Value::Bool);
    }
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
        other => {
            return Err(format!(
                "unknown function `{other}` (allowed: appear, ocr, ocr_digit, text_visible, page, param, var)"
            ))
        }
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

/// Where the expected OCR text of an `ocr_text` locator / expect / drag target
/// comes from (参数 v2, PARAM-V2-DESIGN §3.2).
///
/// * [`TextSpec::Literal`] is the historical `text` field: a byte-for-byte
///   literal (no trim, no quotes, no expression syntax). Old flows are
///   unchanged.
/// * [`TextSpec::Expr`] is the new `text_expr` field: a full expression (the
///   same syntax as every other `*_expr` field, [`parse_expr`]) that the runtime
///   evaluates **once when the step starts** — never per poll — and that
///   **must produce a string**. This is how a task parameter can drive OCR
///   recognition, e.g. `text_expr: "param('战斗按钮文字')"`.
///
/// The two spellings are mutually exclusive in JSON and exactly one of them is
/// required; the parser enforces this, so the runtime never has to guess.
#[derive(Debug, Clone, PartialEq)]
pub enum TextSpec {
    /// Old `text`: the expected line is a literal.
    Literal(String),
    /// New `text_expr`: an expression evaluated once at step entry; the value
    /// must be a non-empty string.
    Expr(Expr),
}

impl TextSpec {
    /// `Some(key)` when the expression is exactly `param('literal-key')`.
    ///
    /// Only `param()` with a single string-literal argument is statically
    /// decidable (T3 lint, logging, UI); a computed key returns `None`.
    pub fn param_key(&self) -> Option<&str> {
        match self {
            TextSpec::Expr(Expr::Call(name, args)) if name == "param" => match args.as_slice() {
                [Expr::Lit(Lit::Str(s))] => Some(s.as_str()),
                _ => None,
            },
            _ => None,
        }
    }
}

/// Hard bound for one `region.offset` value (`dx`/`dy`/`w`/`h`).
///
/// Frame coordinates are far below this; the cap keeps the runtime offset math
/// (i64 add + clamp) well inside its safe range, mirroring
/// [`MAX_DRAG_SHAPE_COORD`].
const MAX_REGION_COORD: i64 = 1_000_000;

/// Optional **operation region** override on an OCR locator / drag endpoint
/// (v1.3, `docs/OPERATION-REGION-DESIGN.md §6.1`).
///
/// A flow without the `region` field yields `None` and keeps the historical
/// behaviour byte-for-byte. Only OCR locators / OCR drag endpoints accept it:
/// template / region / coord_memory / point kinds are a parse error, because
/// their operation geometry is already defined by their own match.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpRegion {
    /// `{"mode":"asset"}`: the asset's own operation area (`action_shape` /
    /// `click_back`), falling back to the matched OCR line bbox when the asset
    /// declares none. (For `click_target` this is the default OCR behaviour,
    /// made explicit; for `drag` it is new.)
    Asset,
    /// `{"mode":"ocr"}`: force the matched OCR line bbox and ignore the asset's
    /// `action_shape` / `click_back`.
    Ocr,
    /// `{"mode":"nearest"}`: with several matching OCR lines, use the one whose
    /// bbox centre is nearest to the OCR **recognition ROI** centre (`sel`'s
    /// `ocr_roi` centre, or the inline `roi` centre). A single matching line
    /// behaves exactly like [`OpRegion::Ocr`].
    Nearest,
    /// `{"mode":"offset","offset":[dx,dy]|[dx,dy,w,h]}`: shift the base bbox
    /// (the matched OCR line bbox) top-left by `(dx,dy)`; a 4-value offset also
    /// resizes it to `(w,h)`, a 2-value offset keeps the matched size.
    Offset {
        dx: i32,
        dy: i32,
        w: Option<i32>,
        h: Option<i32>,
    },
}

impl OpRegion {
    /// `asset` / `ocr` / `nearest` / `offset` (used by logs + tests).
    pub fn mode_str(self) -> &'static str {
        match self {
            OpRegion::Asset => "asset",
            OpRegion::Ocr => "ocr",
            OpRegion::Nearest => "nearest",
            OpRegion::Offset { .. } => "offset",
        }
    }
}

/// v1.1: target of a `drag` endpoint (region/OCR text/point).
///
/// `region` is the "独立区域操作" path: the user marks any rectangle (no
/// recognition) and the runtime picks a random point inside it.
///
/// v1.2.5 (O-2): a `region` may additionally carry an **inline** `shape`
/// (rect/polygon + `invert`), field-for-field the same contract as the asset
/// `action_shape`. `roi` stays mandatory: it is the bbox fallback for runtimes
/// that predate `shape` and the human-readable bounds. An old flow without
/// `shape` keeps the pure-bbox behaviour (no behaviour change at all).
#[derive(Debug, Clone, PartialEq)]
pub enum DragTarget {
    Region {
        roi: [i32; 4],
        /// Optional exact operation shape (frame-absolute). `None` => bbox only.
        shape: Option<DragShape>,
    },
    OcrText {
        sel: Option<String>,
        roi: Option<[i32; 4]>,
        /// `text` (literal) or `text_expr` (expression, evaluated once at step
        /// entry and required to be a string).
        text: TextSpec,
        /// v1.3 operation-area override (OCR endpoints only).
        region: Option<OpRegion>,
    },
    Point { x: i32, y: i32 },
}

impl DragTarget {
    /// Expected OCR text source (`text` / `text_expr`) when this is an
    /// `ocr_text` endpoint; `None` for `region` / `point` (T3/T4 collectors and
    /// the runtime share this accessor).
    pub fn text_spec(&self) -> Option<&TextSpec> {
        match self {
            DragTarget::OcrText { text, .. } => Some(text),
            DragTarget::Region { .. } | DragTarget::Point { .. } => None,
        }
    }

    /// v1.3 operation-region override (`ocr_text` endpoints only).
    pub fn region(&self) -> Option<OpRegion> {
        match self {
            DragTarget::OcrText { region, .. } => *region,
            DragTarget::Region { .. } | DragTarget::Point { .. } => None,
        }
    }
}

/// v1.2.5 (O-2): inline operation shape of a `region` drag endpoint.
///
/// Same JSON contract as `oas_task::ActionShape` (`kind`/`roi`/`points`/
/// `invert`) so the editor can pass its `action_shape` unchanged:
/// `kind:"rect"` uses `roi`, `kind:"polygon"` uses `points`, and
/// `invert:true` means "everything **except** the shape". It is inline (no
/// asset id), so it adds no asset reference and needs no translation at run
/// time: coordinates are frame-absolute, exactly like the sibling `roi`.
#[derive(Debug, Clone, PartialEq)]
pub struct DragShape {
    pub kind: String,
    pub roi: Option<[i32; 4]>,
    pub points: Option<Vec<[i32; 2]>>,
    pub invert: bool,
}

/// Drag direction is mandatory (v1.1 拟人拖动): the end point must lie in this
/// direction from the start point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragAxis {
    Up,
    Down,
    Left,
    Right,
}

impl DragAxis {
    pub fn as_str(self) -> &'static str {
        match self {
            DragAxis::Up => "up",
            DragAxis::Down => "down",
            DragAxis::Left => "left",
            DragAxis::Right => "right",
        }
    }
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
        /// `text` (literal) or `text_expr` (expression) — see [`TextSpec`].
        text: TextSpec,
        require_template: Option<String>,
        /// v1.3 operation-area override (OCR locators only).
        region: Option<OpRegion>,
    },
    /// Text recognition with an **inline ROI** (v1.1): no asset needed.
    ///
    /// This is the "整屏 OCR → 直接点某一行文字" path: the flow carries the
    /// region it was recognized in plus the expected text, so per-line assets do
    /// not have to be maintained. `require_template` keeps the H5 cross-check.
    OcrTextInline {
        roi: [i32; 4],
        /// `text` (literal) or `text_expr` (expression) — see [`TextSpec`].
        text: TextSpec,
        require_template: Option<String>,
        /// v1.3 operation-area override (OCR locators only).
        region: Option<OpRegion>,
    },
    /// v1.1 independent region operation: click a random point inside an
    /// arbitrary frame rectangle (no recognition / no asset needed).
    Region {
        roi: [i32; 4],
    },
    /// v1.2 saved pure operation region (`kind: region`): click a random point
    /// inside the named region asset.
    RegionRef {
        sel: String,
    },
    /// Remembered coordinate (fallback), keyed in the task's `state/coords.json`.
    CoordMemory { key: String },
}

impl Locator {
    pub fn kind(&self) -> &'static str {
        match self {
            Locator::Template { .. } => "template",
            Locator::OcrText { .. } | Locator::OcrTextInline { .. } => "ocr_text",
            Locator::Region { .. } | Locator::RegionRef { .. } => "region",
            Locator::CoordMemory { .. } => "coord_memory",
        }
    }

    /// 0 = highest priority (image), 2 = lowest (memory).
    pub fn priority(&self) -> u8 {
        match self {
            Locator::Template { .. } => 0,
            Locator::OcrText { .. }
            | Locator::OcrTextInline { .. }
            | Locator::Region { .. }
            | Locator::RegionRef { .. } => 1,
            Locator::CoordMemory { .. } => 2,
        }
    }

    /// Asset id referenced by this locator (inline OCR / inline region / memory none).
    pub fn asset(&self) -> Option<&str> {
        match self {
            Locator::Template { sel, .. }
            | Locator::OcrText { sel, .. }
            | Locator::RegionRef { sel } => Some(sel),
            Locator::OcrTextInline { .. } | Locator::Region { .. } | Locator::CoordMemory { .. } => {
                None
            }
        }
    }

    /// Expected OCR text source (`text` / `text_expr`) when this is an OCR
    /// locator; `None` for every other locator kind (T3/T4 collectors and the
    /// runtime share this accessor).
    pub fn text_spec(&self) -> Option<&TextSpec> {
        match self {
            Locator::OcrText { text, .. } | Locator::OcrTextInline { text, .. } => Some(text),
            Locator::Template { .. }
            | Locator::Region { .. }
            | Locator::RegionRef { .. }
            | Locator::CoordMemory { .. } => None,
        }
    }

    /// v1.3 operation-region override (`ocr_text` locators only).
    pub fn region(&self) -> Option<OpRegion> {
        match self {
            Locator::OcrText { region, .. } | Locator::OcrTextInline { region, .. } => *region,
            Locator::Template { .. }
            | Locator::Region { .. }
            | Locator::RegionRef { .. }
            | Locator::CoordMemory { .. } => None,
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
    ///
    /// `text` may also be a `text_expr` expression ([`TextSpec::Expr`]): it is
    /// evaluated once when the verification starts (not per poll) and must be a
    /// string (§3.2.4 Q8=A).
    OcrText {
        sel: String,
        text: TextSpec,
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

    /// Expected OCR text source (`text` / `text_expr`) for `ocr_text` guards /
    /// expectations; `None` for the template kinds (T3/T4 collectors and the
    /// runtime share this accessor).
    pub fn text_spec(&self) -> Option<&TextSpec> {
        match self {
            Expect::OcrText { text, .. } => Some(text),
            Expect::Appear { .. } | Expect::Vanish { .. } => None,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CallInherit {
    /// Same-name parameters from the caller stay visible inside the subtask.
    ///
    /// This is the default because it preserves today's flat shared namespace
    /// (Runner Studio split design §C.4.4): every existing task keeps working.
    #[default]
    All,
    /// Strict isolation: the subtask sees only explicit `params` mappings plus
    /// its own `meta.params[].default` values.
    None,
}

impl CallInherit {
    pub fn as_str(self) -> &'static str {
        match self {
            CallInherit::All => "all",
            CallInherit::None => "none",
        }
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
    /// `while`: loop while `cond` is truthy. There is deliberately no numeric
    /// iteration field here — that belongs to `loop`. `false`/`0` means zero
    /// iterations; `true`/nonzero loops until `break`/budget, like a normal
    /// programming language.
    While {
        cond: Expr,
        body: Vec<Step>,
    },
    Break,
    Continue,
    Wait {
        seconds: f64,
    },
    /// Send one Android key event (4 = BACK, 3 = HOME). Used by recovery flows.
    Key {
        keycode: u32,
    },
    /// Force-stop and relaunch an installed Android app (game recovery fallback).
    RestartApp {
        package: String,
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
    /// v1.1 拟人拖动：起点/终点在区域/OCR 行内**随机**，时长/速度随机；
    /// 方向必填（上/下/左/右），终点必须落在该方向（方向点积 > 0）。
    ///
    /// Engine limitation: the PC adapter's `input swipe` is a straight line, so
    /// intermediate curve jitter is represented in the audit but not sent as
    /// extra move events yet (minitouch/scrcpy can add that later).
    Drag {
        from: DragTarget,
        to: Option<DragTarget>,
        direction: DragAxis,
        /// `[min,max]` random distance when `to` is omitted.
        distance: Option<[i32; 2]>,
        /// `[min,max]` random duration (ms).
        duration_ms: [u32; 2],
        /// Fraction of the region inset for random points (0..=0.45).
        margin: f32,
    },
    /// v1.3 长按（`docs/OPERATION-REGION-DESIGN.md §6.1`）：locator 链解析目标后
    /// 在目标上按住 `hold_ms`（或 `hold_range` 内随机）毫秒，用于「长安/按住」类操作。
    ///
    /// `find` 与 `click_target` 完全同构（模板 / OCR / 区域 / 坐标记忆，OCR 文字可写
    /// `text_expr`，OCR locator 可带 `region` 操作区域覆盖）。`requires`/`expect`
    /// 与 click 家族同义：guard 不满足则**不按下**，expect 在按下后校验。
    /// 输入层是 `InputSink::long_press`（ADB = 同坐标 `input swipe x y x y ms`）。
    LongPress {
        find: Vec<Locator>,
        /// 固定按住时长（ms），100..=10000；`hold_range` 存在时被其覆盖。
        hold_ms: u32,
        /// 可选拟人随机时长区间 `[min,max]`（ms，100..=10000，min<=max），优先于 `hold_ms`。
        hold_range: Option<[u32; 2]>,
        /// 可选按下前页面 guard（同 click_target）。
        requires: Option<Expect>,
        /// 可选按下后校验（同 click_target）。
        expect: Option<Expect>,
    },
    /// v1.3 多段近似曲线滑动（`docs/OPERATION-REGION-DESIGN.md §6.1`）。
    ///
    /// 把给定屏内点序列按 **N-1 段直线** 依次发送 `input swipe`（总时长随机后按段
    /// 均分，每段 >= 1ms）。**每段都是独立手势**：ADB 没有可移植的连续路径注入，
    /// 因此 App 可能把一次曲线识别成多次滑动（设计已确认接受该风险）。
    CurveSwipe {
        /// 2..=32 个屏内点 `[[x,y], ...]`。
        points: Vec<[i32; 2]>,
        /// `[min,max]` 总时长（ms），每段 = total/(n-1)，至少 1ms。
        duration_ms: [u32; 2],
    },
    Ocr {
        sel: String,
    },
    /// v1.2 变量：`set_var('name', value)`（值可以是表达式）。
    SetVar {
        name: String,
        value: Expr,
    },
    /// v1.2 变量：`inc_var('name', by)`；变量不存在按 0 起算。
    IncVar {
        name: String,
        by: Expr,
    },
    /// v1.2 变量：把 OCR 识别结果写入变量（多行用换行连接）。
    OcrToVar {
        sel: String,
        var: String,
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
    /// v1.2.3 U-01(B): call another task's flow as a subflow. The oas-task
    /// loader expands this step into a [`Step::WithParams`] scope before
    /// execution, so the runtime never sees it; it is kept in flow.json so the
    /// reference stays live (edit the target once, all callers pick it up on
    /// next load).
    ///
    /// Runner Studio split (§C.4): `params` maps a **child** parameter key to an
    /// expression evaluated in the **caller** scope; `inherit` controls whether
    /// same-name caller parameters stay visible (design §C.4.4:
    /// mapping > same-name inheritance > subtask default > loud failure).
    ///
    /// Compatibility (P1-4): only `call_task` + `args.params`/`args.inherit` is
    /// written to `flow.json`. A pre-PR4 loader reads just `task` and **silently
    /// ignores** the new args, so an old Runner runs the call with the old
    /// semantics and no error. Task packages must therefore be gated on
    /// `manifest.engine.flow_ops` (design §C.4.6, PR5) before an old Runner is
    /// allowed to install them; `with_params` never appears on disk, so an
    /// `unknown instruction` failure is *not* the protection mechanism.
    CallTask {
        task: String,
        /// child key -> expression evaluated in the caller scope.
        params: BTreeMap<String, Expr>,
        inherit: CallInherit,
    },
    /// A parameter scope around an inline step block (Runner Studio split §C.4.3).
    ///
    /// **Internal implementation detail of the task loader.** It is normally
    /// generated when `oas-task` expands a `call_task` and is never written to
    /// `flow.json`; the parser accepts a hand-authored one for tests/tooling,
    /// but authoring it directly is not supported: T3 still requires the parent
    /// `meta.params` to declare every `param()` key in the body, and it does not
    /// subtract keys supplied by this scope's `params`/`defaults` (P2-2).
    ///
    /// Semantics:
    /// 1. evaluate every `params` expression in the **caller** scope,
    /// 2. `block` keys are temporarily hidden (used by `inherit:"none"`; it is a
    ///    targeted hide list, not a full isolate),
    /// 3. `defaults` fill keys that still have no value,
    /// 4. explicit mappings win over all of the above,
    /// 5. the body runs with that temporary parameter table, restored afterwards.
    ///
    /// `label` is the subtask name used in loud `param()` errors; when absent the
    /// historical (unscoped) error text is kept.
    WithParams {
        label: Option<String>,
        params: Vec<(String, Expr)>,
        defaults: BTreeMap<String, Value>,
        block: Vec<String>,
        body: Vec<Step>,
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

/// `call_task.params` / `with_params.params`: a JSON object mapping a parameter
/// key to an expression **string**. Order is preserved (evaluation order).
fn parse_params_object(
    args: &serde_json::Value,
    path: &str,
) -> Result<Vec<(String, Expr)>, FlowError> {
    let Some(v) = args.get("params") else {
        return Ok(Vec::new());
    };
    let obj = v.as_object().ok_or_else(|| {
        FlowError::new(
            format!("{path}.params"),
            "`params` must be an object mapping parameter key -> expression string",
        )
    })?;
    let mut out = Vec::with_capacity(obj.len());
    for (key, value) in obj {
        let src = value.as_str().ok_or_else(|| {
            FlowError::new(
                format!("{path}.params.{key}"),
                format!("parameter mapping `{key}` must be an expression string, got {value}"),
            )
        })?;
        let expr =
            parse_expr(src).map_err(|e| FlowError::new(format!("{path}.params.{key}"), e))?;
        out.push((key.clone(), expr));
    }
    Ok(out)
}

/// `with_params.defaults`: a JSON object of **literal** scalars.
fn parse_params_defaults(
    args: &serde_json::Value,
    path: &str,
) -> Result<BTreeMap<String, Value>, FlowError> {
    let Some(v) = args.get("defaults") else {
        return Ok(BTreeMap::new());
    };
    let obj = v.as_object().ok_or_else(|| {
        FlowError::new(
            format!("{path}.defaults"),
            "`defaults` must be an object mapping parameter key -> literal value",
        )
    })?;
    let mut out = BTreeMap::new();
    for (key, value) in obj {
        let value = json_to_value(value)
            .map_err(|e| FlowError::new(format!("{path}.defaults.{key}"), e))?;
        out.insert(key.clone(), value);
    }
    Ok(out)
}

/// A required-or-optional JSON array of strings (e.g. `with_params.block`).
fn parse_string_array(
    args: &serde_json::Value,
    key: &str,
    path: &str,
) -> Result<Vec<String>, FlowError> {
    let Some(v) = args.get(key) else {
        return Ok(Vec::new());
    };
    let arr = v.as_array().ok_or_else(|| {
        FlowError::new(
            format!("{path}.{key}"),
            format!("`{key}` must be an array of parameter keys"),
        )
    })?;
    arr.iter()
        .enumerate()
        .map(|(i, item)| {
            item.as_str().map(str::to_string).ok_or_else(|| {
                FlowError::new(
                    format!("{path}.{key}[{i}]"),
                    "every entry must be a string parameter key",
                )
            })
        })
        .collect()
}

/// `call_task.args.inherit`: `"all"` (default) or `"none"` (§C.4.4).
fn parse_call_inherit(args: &serde_json::Value, path: &str) -> Result<CallInherit, FlowError> {
    match args.get("inherit") {
        None => Ok(CallInherit::All),
        Some(serde_json::Value::String(s)) => match s.as_str() {
            "all" => Ok(CallInherit::All),
            "none" => Ok(CallInherit::None),
            other => Err(FlowError::new(
                format!("{path}.inherit"),
                format!("`inherit` must be \"all\" or \"none\" (got `{other}`)"),
            )),
        },
        Some(other) => Err(FlowError::new(
            format!("{path}.inherit"),
            format!("`inherit` must be the string \"all\" or \"none\", got {other}"),
        )),
    }
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

/// Parse `[x,y,w,h]` (i32) used by v1.1 inline OCR / region targets.
fn parse_roi4(v: &serde_json::Value, path: &str) -> Result<[i32; 4], FlowError> {
    let arr = v
        .as_array()
        .ok_or_else(|| FlowError::new(path, "roi must be [x,y,w,h]"))?;
    if arr.len() != 4 {
        return Err(FlowError::new(path, "roi must be exactly 4 integers [x,y,w,h]"));
    }
    let mut out = [0i32; 4];
    for (i, item) in arr.iter().enumerate() {
        out[i] = item
            .as_i64()
            .ok_or_else(|| FlowError::new(format!("{path}[{i}]"), "roi values must be integers"))?
            as i32;
    }
    if out[2] <= 0 || out[3] <= 0 {
        return Err(FlowError::new(path, "roi width/height must be > 0"));
    }
    Ok(out)
}

fn parse_i32_pair(v: &serde_json::Value, path: &str) -> Result<[i32; 2], FlowError> {
    let arr = v
        .as_array()
        .ok_or_else(|| FlowError::new(path, "expected [min,max] integers"))?;
    if arr.len() != 2 {
        return Err(FlowError::new(path, "expected exactly [min,max]"));
    }
    let a = arr[0]
        .as_i64()
        .ok_or_else(|| FlowError::new(format!("{path}[0]"), "must be an integer"))? as i32;
    let b = arr[1]
        .as_i64()
        .ok_or_else(|| FlowError::new(format!("{path}[1]"), "must be an integer"))? as i32;
    if a > b || a < 0 {
        return Err(FlowError::new(path, "expected 0 <= min <= max"));
    }
    Ok([a, b])
}

fn parse_u32_pair(v: &serde_json::Value, path: &str) -> Result<[u32; 2], FlowError> {
    let arr = v
        .as_array()
        .ok_or_else(|| FlowError::new(path, "expected [min,max] integers"))?;
    if arr.len() != 2 {
        return Err(FlowError::new(path, "expected exactly [min,max]"));
    }
    let a = arr[0]
        .as_u64()
        .ok_or_else(|| FlowError::new(format!("{path}[0]"), "must be a positive integer"))?
        as u32;
    let b = arr[1]
        .as_u64()
        .ok_or_else(|| FlowError::new(format!("{path}[1]"), "must be a positive integer"))?
        as u32;
    if a > b {
        return Err(FlowError::new(path, "expected min <= max"));
    }
    Ok([a, b])
}

/// Bounds of one `long_press` hold duration (ms): `hold_ms` and each side of
/// `hold_range` (docs/OPERATION-REGION-DESIGN.md §6.1).
const MIN_HOLD_MS: u32 = 100;
const MAX_HOLD_MS: u32 = 10_000;

/// Bounds of one `curve_swipe` total duration side (ms) and point count.
const MIN_CURVE_MS: u32 = 100;
const MAX_CURVE_MS: u32 = 10_000;
const MIN_CURVE_POINTS: usize = 2;
const MAX_CURVE_POINTS: usize = 32;

/// Parse the optional `region` object of an OCR locator / drag endpoint
/// (v1.3, `docs/OPERATION-REGION-DESIGN.md §6.1`).
///
/// JSON contract:
/// ```json
/// "region": {"mode": "asset" | "ocr" | "nearest" | "offset", "offset": [dx, dy(, w, h)]}
/// ```
/// * missing / `null` → `None` (old flows unchanged);
/// * `offset` is only allowed for `mode:"offset"` (and is required there);
/// * a 4-value offset needs `w`/`h` > 0; a 2-value one keeps the base size.
fn parse_region(v: &serde_json::Value, path: &str) -> Result<Option<OpRegion>, FlowError> {
    let p = format!("{path}.region");
    let Some(raw) = v.get("region") else {
        return Ok(None);
    };
    if raw.is_null() {
        return Ok(None);
    }
    let obj = raw
        .as_object()
        .ok_or_else(|| FlowError::new(&p, "`region` must be an object {mode, offset?}"))?;
    let mode = obj
        .get("mode")
        .and_then(|m| m.as_str())
        .ok_or_else(|| {
            FlowError::new(
                format!("{p}.mode"),
                "`region.mode` is required (allowed: asset, ocr, nearest, offset)",
            )
        })?;
    let offset = obj.get("offset");
    match mode {
        "asset" | "ocr" | "nearest" => {
            if offset.is_some() {
                return Err(FlowError::new(
                    format!("{p}.offset"),
                    format!(
                        "`region.offset` is only allowed with mode=\"offset\" \
                         (mode is `{mode}`)"
                    ),
                ));
            }
            Ok(Some(match mode {
                "asset" => OpRegion::Asset,
                "ocr" => OpRegion::Ocr,
                _ => OpRegion::Nearest,
            }))
        }
        "offset" => {
            let off = offset.ok_or_else(|| {
                FlowError::new(
                    &p,
                    "mode=\"offset\" needs `offset:[dx,dy]` or `[dx,dy,w,h]` \
                     (relative to the OCR line bbox top-left)",
                )
            })?;
            let arr = off.as_array().ok_or_else(|| {
                FlowError::new(
                    format!("{p}.offset"),
                    "`region.offset` must be an array of 2 or 4 integers",
                )
            })?;
            if arr.len() != 2 && arr.len() != 4 {
                return Err(FlowError::new(
                    format!("{p}.offset"),
                    format!(
                        "`region.offset` must have 2 or 4 integers, got {}",
                        arr.len()
                    ),
                ));
            }
            let mut coords = [0i32; 4];
            for (i, item) in arr.iter().enumerate() {
                let at = format!("{p}.offset[{i}]");
                let raw = item
                    .as_i64()
                    .ok_or_else(|| FlowError::new(&at, "offset values must be integers"))?;
                if !(-MAX_REGION_COORD..=MAX_REGION_COORD).contains(&raw) {
                    return Err(FlowError::new(
                        &at,
                        format!("offset value must be within +-{MAX_REGION_COORD}, got {raw}"),
                    ));
                }
                coords[i] = raw as i32;
            }
            let (w, h) = if arr.len() == 4 {
                if coords[2] <= 0 || coords[3] <= 0 {
                    return Err(FlowError::new(
                        format!("{p}.offset"),
                        "a 4-value `region.offset` needs w/h > 0",
                    ));
                }
                (Some(coords[2]), Some(coords[3]))
            } else {
                (None, None)
            };
            Ok(Some(OpRegion::Offset {
                dx: coords[0],
                dy: coords[1],
                w,
                h,
            }))
        }
        other => Err(FlowError::new(
            format!("{p}.mode"),
            format!("unknown region mode `{other}` (allowed: asset, ocr, nearest, offset)"),
        )),
    }
}

/// Reject a `region` override on a non-OCR locator / drag endpoint kind.
///
/// The model deliberately only covers `kind:"ocr_text"`: a template match has
/// its own `action_shape`/bbox geometry and a `point`/`region` target IS the
/// geometry, so silently ignoring a `region` there would be surprising.
fn reject_region_on_non_ocr(v: &serde_json::Value, path: &str) -> Result<(), FlowError> {
    match v.get("region") {
        None | Some(serde_json::Value::Null) => Ok(()),
        Some(_) => Err(FlowError::new(
            format!("{path}.region"),
            "`region` is only supported on `kind:\"ocr_text\"` locators / drag endpoints",
        )),
    }
}

/// Parse one `long_press.hold_ms` value: integer in `100..=10000`.
fn parse_hold_ms(v: &serde_json::Value, path: &str) -> Result<u32, FlowError> {
    let raw = v
        .as_u64()
        .ok_or_else(|| FlowError::new(path, "`hold_ms` must be an integer"))?;
    let n = u32::try_from(raw)
        .map_err(|_| FlowError::new(path, format!("`hold_ms` {raw} does not fit u32")))?;
    if !(MIN_HOLD_MS..=MAX_HOLD_MS).contains(&n) {
        return Err(FlowError::new(
            path,
            format!("`hold_ms` must be within {MIN_HOLD_MS}..={MAX_HOLD_MS}, got {n}"),
        ));
    }
    Ok(n)
}

/// Parse one `long_press.hold_range` value: `[min,max]` in `100..=10000`.
fn parse_hold_range(v: &serde_json::Value, path: &str) -> Result<[u32; 2], FlowError> {
    let pair = parse_u32_pair(v, path)?;
    for (i, n) in pair.iter().enumerate() {
        if !(MIN_HOLD_MS..=MAX_HOLD_MS).contains(n) {
            return Err(FlowError::new(
                format!("{path}[{i}]"),
                format!("hold_range side must be within {MIN_HOLD_MS}..={MAX_HOLD_MS}, got {n}"),
            ));
        }
    }
    Ok(pair)
}

/// Parse `curve_swipe.points`: 2..=32 integer screen points `[[x,y], ...]`.
fn parse_curve_points(v: &serde_json::Value, path: &str) -> Result<Vec<[i32; 2]>, FlowError> {
    let arr = v
        .as_array()
        .ok_or_else(|| FlowError::new(path, "`points` must be an array of [x,y] points"))?;
    if !(MIN_CURVE_POINTS..=MAX_CURVE_POINTS).contains(&arr.len()) {
        return Err(FlowError::new(
            path,
            format!(
                "`points` must have {MIN_CURVE_POINTS}..={MAX_CURVE_POINTS} points, got {}",
                arr.len()
            ),
        ));
    }
    let mut out = Vec::with_capacity(arr.len());
    for (i, item) in arr.iter().enumerate() {
        let p = format!("{path}[{i}]");
        let pair = item
            .as_array()
            .ok_or_else(|| FlowError::new(&p, "each point must be [x,y]"))?;
        if pair.len() != 2 {
            return Err(FlowError::new(
                &p,
                format!("each point must have exactly 2 integers, got {}", pair.len()),
            ));
        }
        out.push([
            parse_drag_shape_coord(&pair[0], &format!("{p}[0]"))?,
            parse_drag_shape_coord(&pair[1], &format!("{p}[1]"))?,
        ]);
    }
    Ok(out)
}

/// Parse `curve_swipe.duration_ms`: `[min,max]` (ms) or one fixed integer.
fn parse_curve_duration(v: &serde_json::Value, path: &str) -> Result<[u32; 2], FlowError> {
    let pair = match v.as_u64() {
        Some(n) => [u32::try_from(n).unwrap_or(u32::MAX), u32::try_from(n).unwrap_or(u32::MAX)],
        None => parse_u32_pair(v, path)?,
    };
    for (i, n) in pair.iter().enumerate() {
        if !(MIN_CURVE_MS..=MAX_CURVE_MS).contains(n) {
            return Err(FlowError::new(
                format!("{path}[{i}]"),
                format!("duration_ms side must be within {MIN_CURVE_MS}..={MAX_CURVE_MS}, got {n}"),
            ));
        }
    }
    Ok(pair)
}

fn parse_drag_axis(s: &str, path: &str) -> Result<DragAxis, FlowError> {
    match s {
        "up" => Ok(DragAxis::Up),
        "down" => Ok(DragAxis::Down),
        "left" => Ok(DragAxis::Left),
        "right" => Ok(DragAxis::Right),
        other => Err(FlowError::new(
            path,
            format!("unknown direction `{other}` (allowed: up, down, left, right)"),
        )),
    }
}

/// Hard upper bound for one inline drag-shape coordinate.
///
/// Frame coordinates are far below this; the cap keeps every geometric
/// predicate (shoelace area, self-intersection) inside `i64` and stops absurd
/// values from overflowing the runtime's `i32` rectangle math (P1-4).
const MAX_DRAG_SHAPE_COORD: i64 = 1_000_000;

/// Cap on inline drag polygon vertices: the self-intersection test is O(n²).
const MAX_DRAG_SHAPE_POINTS: usize = 256;

/// Parse one inline drag-shape coordinate: integer, `0..=MAX_DRAG_SHAPE_COORD`.
///
/// Unlike [`parse_roi4`] (shared with older region/OCR targets) this never
/// truncates an `i64` with `as i32`: every out-of-range value is a loud parse
/// error with the exact JSON path.
fn parse_drag_shape_coord(v: &serde_json::Value, path: &str) -> Result<i32, FlowError> {
    let raw = v
        .as_i64()
        .ok_or_else(|| FlowError::new(path, "drag shape coordinates must be integers"))?;
    if !(0..=MAX_DRAG_SHAPE_COORD).contains(&raw) {
        return Err(FlowError::new(
            path,
            format!("drag shape coordinate must be within 0..={MAX_DRAG_SHAPE_COORD}, got {raw}"),
        ));
    }
    i32::try_from(raw)
        .map_err(|_| FlowError::new(path, format!("drag shape coordinate {raw} does not fit i32")))
}

/// Parse `[x,y,w,h]` for a `rect` drag shape.
///
/// Stricter than the legacy [`parse_roi4`] on purpose: x/y must be `>= 0`,
/// w/h must be `> 0`, every value and the `x+w` / `y+h` edges must stay within
/// [`MAX_DRAG_SHAPE_COORD`]. The checked edges are what keep the runtime bbox
/// math from overflowing.
fn parse_drag_shape_roi(v: &serde_json::Value, path: &str) -> Result<[i32; 4], FlowError> {
    let arr = v
        .as_array()
        .ok_or_else(|| FlowError::new(path, "rect drag shape `roi` must be [x,y,w,h]"))?;
    if arr.len() != 4 {
        return Err(FlowError::new(
            path,
            "rect drag shape `roi` must be exactly 4 integers [x,y,w,h]",
        ));
    }
    let mut out = [0i32; 4];
    for (i, item) in arr.iter().enumerate() {
        out[i] = parse_drag_shape_coord(item, &format!("{path}[{i}]"))?;
    }
    if out[2] <= 0 || out[3] <= 0 {
        return Err(FlowError::new(
            path,
            "rect drag shape `roi` width/height must be > 0",
        ));
    }
    let right = out[0] as i64 + out[2] as i64;
    let bottom = out[1] as i64 + out[3] as i64;
    if right > MAX_DRAG_SHAPE_COORD || bottom > MAX_DRAG_SHAPE_COORD {
        return Err(FlowError::new(
            path,
            format!(
                "rect drag shape `roi` x+w/y+h must be <= {MAX_DRAG_SHAPE_COORD} \
                 (got x+w={right}, y+h={bottom})"
            ),
        ));
    }
    Ok(out)
}

/// Shoelace area × 2 (i128) for an inline drag polygon (`0` = collinear).
///
/// `i128` keeps the accumulation exact even at the maximum accepted coordinate
/// (1e6): an `i64` accumulator could still wrap on adversarial vertex counts.
fn drag_polygon_area2(points: &[[i32; 2]]) -> i128 {
    let mut area2: i128 = 0;
    for i in 0..points.len() {
        let a = points[i];
        let b = points[(i + 1) % points.len()];
        area2 += a[0] as i128 * b[1] as i128 - b[0] as i128 * a[1] as i128;
    }
    area2
}

/// O(n²) self-intersection test for an inline drag polygon.
///
/// Mirrors `oas_task`'s `polygon_self_intersects` (the dependency direction
/// forbids importing it from `oas-task`). Coordinates are bounded by
/// [`MAX_DRAG_SHAPE_COORD`], so each cross product fits in `i64`.
fn drag_polygon_self_intersects(poly: &[[i32; 2]]) -> bool {
    let n = poly.len();
    if n < 4 {
        return false;
    }
    let cross = |o: [i32; 2], a: [i32; 2], b: [i32; 2]| -> i64 {
        (a[0] as i64 - o[0] as i64) * (b[1] as i64 - o[1] as i64)
            - (a[1] as i64 - o[1] as i64) * (b[0] as i64 - o[0] as i64)
    };
    let on_seg = |a: [i32; 2], b: [i32; 2], p: [i32; 2]| -> bool {
        p[0] >= a[0].min(b[0])
            && p[0] <= a[0].max(b[0])
            && p[1] >= a[1].min(b[1])
            && p[1] <= a[1].max(b[1])
    };
    for i in 0..n {
        let a1 = poly[i];
        let a2 = poly[(i + 1) % n];
        for j in 0..n {
            if j == i {
                continue;
            }
            let b1 = poly[j];
            let b2 = poly[(j + 1) % n];
            // Adjacent edges share a vertex by construction: skip them.
            if (i + 1) % n == j || (j + 1) % n == i {
                continue;
            }
            let d1 = cross(b1, b2, a1);
            let d2 = cross(b1, b2, a2);
            let d3 = cross(a1, a2, b1);
            let d4 = cross(a1, a2, b2);
            let proper = ((d1 > 0 && d2 < 0) || (d1 < 0 && d2 > 0))
                && ((d3 > 0 && d4 < 0) || (d3 < 0 && d4 > 0));
            if proper {
                return true;
            }
            // Collinear touching/overlap.
            if (d1 == 0 && on_seg(b1, b2, a1))
                || (d2 == 0 && on_seg(b1, b2, a2))
                || (d3 == 0 && on_seg(a1, a2, b1))
                || (d4 == 0 && on_seg(a1, a2, b2))
            {
                return true;
            }
        }
    }
    false
}

/// v1.2.5 (O-2): parse the optional inline `shape` of a `region` drag target.
///
/// The contract mirrors the asset `action_shape` (`kind`/`roi`|`points`/
/// `invert`) but is validated here so a broken shape fails loudly with a JSON
/// path instead of silently degrading to the bbox at run time. `path` points at
/// `...from.shape` / `...to.shape`; nested errors add `.roi` / `.points[i]` /
/// `.invert`.
fn parse_drag_shape(v: &serde_json::Value, path: &str) -> Result<DragShape, FlowError> {
    let kind = v
        .get("kind")
        .and_then(|x| x.as_str())
        .ok_or_else(|| FlowError::new(path, "drag shape needs `kind` (allowed: rect, polygon)"))?;
    let invert = match v.get("invert") {
        None => false,
        Some(b) => b.as_bool().ok_or_else(|| {
            FlowError::new(format!("{path}.invert"), "`invert` must be a boolean")
        })?,
    };
    match kind {
        "rect" => {
            if v.get("points").is_some() {
                return Err(FlowError::new(
                    format!("{path}.points"),
                    "rect drag shape must use `roi`, not `points`",
                ));
            }
            let roi = parse_drag_shape_roi(
                v.get("roi")
                    .ok_or_else(|| FlowError::new(format!("{path}.roi"), "rect drag shape needs `roi`"))?,
                &format!("{path}.roi"),
            )?;
            Ok(DragShape {
                kind: "rect".to_string(),
                roi: Some(roi),
                points: None,
                invert,
            })
        }
        "polygon" => {
            if v.get("roi").is_some() {
                return Err(FlowError::new(
                    format!("{path}.roi"),
                    "polygon drag shape must use `points`, not `roi`",
                ));
            }
            let pts_path = format!("{path}.points");
            let arr = v.get("points").and_then(|x| x.as_array()).ok_or_else(|| {
                FlowError::new(pts_path.clone(), "polygon drag shape needs `points`: [[x,y],...]")
            })?;
            if arr.len() < 3 {
                return Err(FlowError::new(
                    pts_path.clone(),
                    format!(
                        "polygon drag shape needs at least 3 points (got {})",
                        arr.len()
                    ),
                ));
            }
            if arr.len() > MAX_DRAG_SHAPE_POINTS {
                return Err(FlowError::new(
                    pts_path.clone(),
                    format!(
                        "polygon drag shape allows at most {MAX_DRAG_SHAPE_POINTS} points (got {})",
                        arr.len()
                    ),
                ));
            }
            let mut points = Vec::with_capacity(arr.len());
            for (i, p) in arr.iter().enumerate() {
                let pair = p.as_array().ok_or_else(|| {
                    FlowError::new(format!("{pts_path}[{i}]"), "each point must be [x,y]")
                })?;
                if pair.len() != 2 {
                    return Err(FlowError::new(
                        format!("{pts_path}[{i}]"),
                        "each point must be exactly [x,y]",
                    ));
                }
                let x = parse_drag_shape_coord(&pair[0], &format!("{pts_path}[{i}][0]"))?;
                let y = parse_drag_shape_coord(&pair[1], &format!("{pts_path}[{i}][1]"))?;
                points.push([x, y]);
            }
            if drag_polygon_self_intersects(&points) {
                return Err(FlowError::new(
                    pts_path,
                    "polygon drag shape must not self-intersect",
                ));
            }
            if drag_polygon_area2(&points) == 0 {
                return Err(FlowError::new(
                    pts_path,
                    "polygon drag shape has zero area (collinear points)",
                ));
            }
            Ok(DragShape {
                kind: "polygon".to_string(),
                roi: None,
                points: Some(points),
                invert,
            })
        }
        other => Err(FlowError::new(
            path,
            format!("unknown drag shape `{other}` (allowed: rect, polygon)"),
        )),
    }
}

/// P2-4: `shape` is only meaningful on a `region` drag target.
///
/// Silently ignoring it on `point`/`ocr_text` would execute a different gesture
/// than the author wrote, so the misplaced key is a loud parse error. A JSON
/// `null` still counts as "absent" (same as on `region`).
fn reject_drag_shape_on_non_region(
    v: &serde_json::Value,
    path: &str,
) -> Result<(), FlowError> {
    match v.get("shape") {
        None | Some(serde_json::Value::Null) => Ok(()),
        Some(_) => Err(FlowError::new(
            format!("{path}.shape"),
            "shape is only supported on `region` drag targets",
        )),
    }
}

/// Parse the expected OCR text of an `ocr_text` locator / expect / drag target
/// (参数 v2, PARAM-V2-DESIGN §3.2.3).
///
/// Rules (R1/R2/R3):
/// * `text` (JSON string, historical literal) and `text_expr` (JSON string, an
///   expression) are **mutually exclusive** — both present is a parse error on
///   the `text_expr` path;
/// * exactly one of them is required — neither present is a parse error on the
///   owning path;
/// * `text` keeps its byte-for-byte literal semantics (no trim, no expression
///   parsing), so every old flow behaves exactly as before;
/// * `text_expr` is parsed with [`parse_expr`]; the runtime evaluates it **once
///   when the step starts** and it must produce a string.
fn parse_text_spec(v: &serde_json::Value, path: &str) -> Result<TextSpec, FlowError> {
    match (v.get("text"), v.get("text_expr")) {
        (Some(_), Some(_)) => Err(FlowError::new(
            format!("{path}.text_expr"),
            "ocr_text needs exactly one of `text` / `text_expr` \
             (`text` and `text_expr` are mutually exclusive)",
        )),
        (None, None) => Err(FlowError::new(
            path,
            "ocr_text needs exactly one of `text` / `text_expr`",
        )),
        (Some(literal), None) => {
            let s = literal.as_str().ok_or_else(|| {
                FlowError::new(format!("{path}.text"), "`text` must be a string")
            })?;
            Ok(TextSpec::Literal(s.to_string()))
        }
        (None, Some(expr)) => {
            let src = expr.as_str().ok_or_else(|| {
                FlowError::new(
                    format!("{path}.text_expr"),
                    "`text_expr` must be an expression string",
                )
            })?;
            let expr = parse_expr(src)
                .map_err(|e| FlowError::new(format!("{path}.text_expr"), e))?;
            Ok(TextSpec::Expr(expr))
        }
    }
}

fn parse_drag_target(v: &serde_json::Value, path: &str) -> Result<DragTarget, FlowError> {
    let kind = v
        .get("kind")
        .and_then(|x| x.as_str())
        .ok_or_else(|| FlowError::new(path, "drag target needs `kind`"))?;
    match kind {
        "region" => {
            reject_region_on_non_ocr(v, path)?;
            Ok(DragTarget::Region {
                roi: parse_roi4(
                    v.get("roi")
                        .ok_or_else(|| FlowError::new(path, "region needs `roi`"))?,
                    &format!("{path}.roi"),
                )?,
                // O-2: optional inline shape; a JSON `null` means "no shape" too.
                shape: match v.get("shape") {
                    None | Some(serde_json::Value::Null) => None,
                    Some(s) => Some(parse_drag_shape(s, &format!("{path}.shape"))?),
                },
            })
        }
        "point" => {
            reject_drag_shape_on_non_region(v, path)?;
            reject_region_on_non_ocr(v, path)?;
            let x = v
                .get("x")
                .and_then(|x| x.as_i64())
                .ok_or_else(|| FlowError::new(path, "point needs integer `x`"))? as i32;
            let y = v
                .get("y")
                .and_then(|x| x.as_i64())
                .ok_or_else(|| FlowError::new(path, "point needs integer `y`"))? as i32;
            Ok(DragTarget::Point { x, y })
        }
        "ocr_text" => {
            reject_drag_shape_on_non_region(v, path)?;
            let text = parse_text_spec(v, path)?;
            let region = parse_region(v, path)?;
            let sel = v.get("sel").and_then(|x| x.as_str()).map(|s| s.to_string());
            let roi = match v.get("roi") {
                None => None,
                Some(x) => Some(parse_roi4(x, &format!("{path}.roi"))?),
            };
            if sel.is_none() && roi.is_none() {
                return Err(FlowError::new(
                    path,
                    "ocr_text drag target needs `sel` or `roi`",
                ));
            }
            Ok(DragTarget::OcrText {
                sel,
                roi,
                text,
                region,
            })
        }
        other => Err(FlowError::new(
            path,
            format!("unknown drag target `{other}` (allowed: region, point, ocr_text)"),
        )),
    }
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
            text: parse_text_spec(obj, &p)?,
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
            reject_region_on_non_ocr(v, path)?;
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
        "ocr_text" => {
            let text = parse_text_spec(v, path)?;
            let region = parse_region(v, path)?;
            let require_template = match v.get("require_template") {
                None => None,
                Some(x) => Some(
                    x.as_str()
                        .ok_or_else(|| {
                            FlowError::new(
                                format!("{path}.require_template"),
                                "`require_template` must be a string asset id",
                            )
                        })?
                        .to_string(),
                ),
            };
            // v1.1: inline ROI form — no OCR asset needed, so whole-screen OCR
            // lines can be clicked without per-line assets (the user's request).
            if let Some(roi_v) = v.get("roi") {
                let arr = roi_v.as_array().ok_or_else(|| {
                    FlowError::new(format!("{path}.roi"), "`roi` must be [x,y,w,h]")
                })?;
                if arr.len() != 4 {
                    return Err(FlowError::new(
                        format!("{path}.roi"),
                        "`roi` must be exactly 4 integers [x,y,w,h]",
                    ));
                }
                let mut roi = [0i32; 4];
                for (i, item) in arr.iter().enumerate() {
                    roi[i] = item.as_i64().ok_or_else(|| {
                        FlowError::new(format!("{path}.roi[{i}]"), "roi values must be integers")
                    })? as i32;
                }
                if roi[2] <= 0 || roi[3] <= 0 {
                    return Err(FlowError::new(
                        format!("{path}.roi"),
                        "inline OCR roi width/height must be > 0",
                    ));
                }
                return Ok(Locator::OcrTextInline {
                    roi,
                    text,
                    require_template,
                    region,
                });
            }
            Ok(Locator::OcrText {
                sel: req_str(v, "sel", path)?,
                text,
                require_template,
                region,
            })
        }
        "region" => {
            reject_region_on_non_ocr(v, path)?;
            if let Some(sel) = v.get("sel").and_then(|x| x.as_str()) {
                if sel.trim().is_empty() {
                    return Err(FlowError::new(format!("{path}.sel"), "region `sel` is empty"));
                }
                return Ok(Locator::RegionRef {
                    sel: sel.to_string(),
                });
            }
            let roi = v
                .get("roi")
                .ok_or_else(|| FlowError::new(path, "region needs `roi` or `sel`"))?;
            Ok(Locator::Region {
                roi: parse_roi4(roi, &format!("{path}.roi"))?,
            })
        }
        "coord_memory" => {
            reject_region_on_non_ocr(v, path)?;
            Ok(Locator::CoordMemory {
                key: req_str(v, "key", path)?,
            })
        }
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
        "while" => Ok(Step::While {
            cond: parse_expr_field(&args, "cond", &a_path)?,
            body: parse_steps(
                v.get("body")
                    .ok_or_else(|| FlowError::new(path, "`while` needs `body`"))?,
                &format!("{path}.body"),
            )?,
        }),
        "break" => Ok(Step::Break),
        "continue" => Ok(Step::Continue),
        "wait" => Ok(Step::Wait {
            seconds: req_f64(&args, "seconds", &a_path)?,
        }),
        "key" => {
            let keycode = args
                .get("keycode")
                .and_then(|x| x.as_u64())
                .ok_or_else(|| {
                    FlowError::new(format!("{a_path}.keycode"), "`key` needs integer keycode")
                })?;
            if keycode == 0 || keycode > u32::MAX as u64 {
                return Err(FlowError::new(
                    format!("{a_path}.keycode"),
                    "keycode must be a positive u32",
                ));
            }
            Ok(Step::Key {
                keycode: keycode as u32,
            })
        }
        "restart_app" => Ok(Step::RestartApp {
            package: req_str(&args, "package", &a_path)?,
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
        "drag" => {
            let from = parse_drag_target(
                args.get("from")
                    .ok_or_else(|| FlowError::new(format!("{a_path}.from"), "`drag` needs `from`"))?,
                &format!("{a_path}.from"),
            )?;
            let to = match args.get("to") {
                None => None,
                Some(v) => Some(parse_drag_target(v, &format!("{a_path}.to"))?),
            };
            let direction = parse_drag_axis(
                args.get("direction")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        FlowError::new(
                            format!("{a_path}.direction"),
                            "`drag` needs direction up|down|left|right",
                        )
                    })?,
                &format!("{a_path}.direction"),
            )?;
            let distance = match args.get("distance") {
                None => None,
                Some(v) => Some(parse_i32_pair(v, &format!("{a_path}.distance"))?),
            };
            if to.is_none() && distance.is_none() {
                return Err(FlowError::new(
                    format!("{a_path}"),
                    "`drag` needs `to` or `distance:[min,max]`",
                ));
            }
            let duration_ms = match args.get("duration_ms") {
                None => [300, 650],
                Some(v) => {
                    if let Some(n) = v.as_u64() {
                        let n = n as u32;
                        [n, n]
                    } else {
                        parse_u32_pair(v, &format!("{a_path}.duration_ms"))?
                    }
                }
            };
            if duration_ms[0] == 0 || duration_ms[1] == 0 {
                return Err(FlowError::new(
                    format!("{a_path}.duration_ms"),
                    "duration_ms must be > 0",
                ));
            }
            let margin = match args.get("margin") {
                None => 0.15f32,
                Some(v) => {
                    let x = v.as_f64().ok_or_else(|| {
                        FlowError::new(format!("{a_path}.margin"), "`margin` must be a number")
                    })?;
                    if !(0.0..=0.45).contains(&x) {
                        return Err(FlowError::new(
                            format!("{a_path}.margin"),
                            "`margin` must be 0..=0.45",
                        ));
                    }
                    x as f32
                }
            };
            Ok(Step::Drag {
                from,
                to,
                direction,
                distance,
                duration_ms,
                margin,
            })
        }
        "click_target" => Ok(Step::ClickTarget {
            find: parse_locators(&args, &a_path)?,
            requires: parse_expect(&args, "requires", &a_path)?,
            expect: parse_expect(&args, "expect", &a_path)?,
            retry: parse_retry(&args, &a_path)?,
        }),
        // v1.3 long press (docs/OPERATION-REGION-DESIGN.md §6.1): same locator
        // chain / guard / verification as `click_target`, but the action is a
        // hold. `hold_range` (humanized random duration) wins over `hold_ms`.
        "long_press" => {
            let hold_ms = match args.get("hold_ms") {
                None => 3000u32,
                Some(v) => parse_hold_ms(v, &format!("{a_path}.hold_ms"))?,
            };
            let hold_range = match args.get("hold_range") {
                None => None,
                Some(v) => Some(parse_hold_range(v, &format!("{a_path}.hold_range"))?),
            };
            Ok(Step::LongPress {
                find: parse_locators(&args, &a_path)?,
                hold_ms,
                hold_range,
                requires: parse_expect(&args, "requires", &a_path)?,
                expect: parse_expect(&args, "expect", &a_path)?,
            })
        }
        // v1.3 multi-segment curve (docs/OPERATION-REGION-DESIGN.md §6.1):
        // N points → N-1 independent `input swipe` calls; NOT one continuous
        // gesture (documented limitation).
        "curve_swipe" => {
            let points = match args.get("points") {
                Some(v) => parse_curve_points(v, &format!("{a_path}.points"))?,
                None => {
                    return Err(FlowError::new(
                        format!("{a_path}.points"),
                        "`curve_swipe` needs `points`: 2..=32 screen points [[x,y], ...]",
                    ))
                }
            };
            let duration_ms = match args.get("duration_ms") {
                None => [600, 1000],
                Some(v) => parse_curve_duration(v, &format!("{a_path}.duration_ms"))?,
            };
            Ok(Step::CurveSwipe {
                points,
                duration_ms,
            })
        }
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
        "set_var" => Ok(Step::SetVar {
            name: req_str(&args, "name", &a_path)?,
            value: parse_expr_field(&args, "value", &a_path)?,
        }),
        "inc_var" => Ok(Step::IncVar {
            name: req_str(&args, "name", &a_path)?,
            by: match args.get("by") {
                Some(v) => parse_coord_value(v, format!("{a_path}.by"))?,
                None => Expr::Lit(Lit::Num(1.0)),
            },
        }),
        "ocr_to_var" => Ok(Step::OcrToVar {
            sel: req_str(&args, "sel", &a_path)?,
            var: req_str(&args, "var", &a_path)?,
        }),
        "log" => Ok(Step::Log {
            level: args
                .get("level")
                .and_then(|x| x.as_str())
                .unwrap_or("info")
                .to_string(),
            text: parse_expr_field(&args, "text", &a_path)?,
        }),
        "call_task" => Ok(Step::CallTask {
            task: req_str(&args, "task", &a_path)?,
            params: parse_params_object(&args, &a_path)?.into_iter().collect(),
            inherit: parse_call_inherit(&args, &a_path)?,
        }),
        // Runner Studio split §C.4.3: this is the in-memory wrapper the loader
        // generates when it expands `call_task`; it is an internal implementation
        // detail (never part of flow.json), not an authoring surface.
        "with_params" => {
            let label = match args.get("task") {
                None => None,
                Some(serde_json::Value::String(s)) => Some(s.clone()),
                Some(other) => {
                    return Err(FlowError::new(
                        format!("{a_path}.task"),
                        format!(
                            "`task` (the subtask label for error messages) must be a string, \
                             got {other}"
                        ),
                    ))
                }
            };
            let body = parse_steps(
                args.get("body")
                    .ok_or_else(|| FlowError::new(format!("{path}.args.body"), "`with_params` needs `body`"))?,
                &format!("{path}.args.body"),
            )?;
            Ok(Step::WithParams {
                label,
                params: parse_params_object(&args, &a_path)?,
                defaults: parse_params_defaults(&args, &a_path)?,
                block: parse_string_array(&args, "block", &a_path)?,
                body,
            })
        }
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
                 wait/wait_until/appear/click/click_point/click_target/long_press/\
                 curve_swipe/dismiss_popup/swipe/\
                 drag/key/restart_app/ocr/set_var/inc_var/ocr_to_var/log/call_task/\
                 with_params/finish)"
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

    /// Every exit name that any `finish` step in the flow can produce (nested
    /// blocks included). This is the T1 input for the lint: each name must be
    /// declared in `meta.exits` (ENGINE §22.34.2).
    pub fn reachable_exits(&self) -> BTreeSet<String> {
        self.finish_uses().into_iter().map(|u| u.exit).collect()
    }

    /// Like [`Self::reachable_exits`] but keeps the path of every finish step.
    pub fn finish_uses(&self) -> Vec<ExitUse> {
        let mut out = Vec::new();
        collect_finish_steps(&self.steps, "steps", &mut out);
        out
    }

    /// Every literal key passed to `param('key')` anywhere in the flow (T3).
    ///
    /// Non-literal keys (e.g. `param('a' + 'b')`) cannot be checked statically
    /// and are intentionally absent here; use [`Self::param_uses_at`] to report
    /// them as lint errors.
    pub fn param_uses(&self) -> BTreeSet<String> {
        self.param_uses_at()
            .into_iter()
            .filter_map(|u| u.key)
            .collect()
    }

    /// Every `param(...)` call site with its JSON-ish path. `key == None` means
    /// the argument is not exactly one string literal => lint must flag it.
    pub fn param_uses_at(&self) -> Vec<ParamUse> {
        let mut out = Vec::new();
        collect_param_steps(&self.steps, "steps", &mut out);
        out
    }

    /// Every `call_task` step with its parameter mapping (Runner Studio split
    /// §C.4.5 T7; also shared with the Studio UI).
    ///
    /// `call_task` is deliberately a leaf of [`Self::param_uses_at`]: mappings
    /// belong to the *caller* scope, but the T7 lint reports them with a precise
    /// per-call message instead of the generic T3 one.
    pub fn call_task_uses(&self) -> Vec<CallTaskUse> {
        let mut out = Vec::new();
        collect_call_task_steps(&self.steps, "steps", &mut out);
        out
    }

    /// T1: does every path through the flow end with `finish(<exit>)`?
    ///
    /// Conservative: the last step must be a `finish`, or an `if` whose BOTH
    /// branches terminate (a loop is not terminal: it may run zero times).
    pub fn ends_with_finish(&self) -> bool {
        steps_end_with_finish(&self.steps)
    }
}

/// Name of an [`Exit`] (the inverse of [`parse_exit`]).
pub fn exit_name(exit: Exit) -> &'static str {
    match exit {
        Exit::Done => "done",
        Exit::NoResource => "no_resource",
        Exit::WindowClosed => "window_closed",
        Exit::NoTarget => "no_target",
        Exit::ResourceEmpty => "resource_empty",
        Exit::Exhausted => "exhausted",
        Exit::Failed => "failed",
        Exit::Skipped => "skipped",
    }
}

/// One `finish` site: the exit name + its JSON-ish path (`steps[2].args.exit`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExitUse {
    pub exit: String,
    pub path: String,
}

/// One `param('key')` call site; `key == None` = not a single string literal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParamUse {
    pub key: Option<String>,
    pub path: String,
}

/// One `call_task` step and its parameter mapping (T7 / Studio shared view).
///
/// `params` maps the **child** parameter key to the parent key when the mapping
/// value is exactly `param('parentKey')`; a constant expression or a computed
/// `param(...)` call is `None` (see `computed_params`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallTaskUse {
    pub task: String,
    pub inherit: CallInherit,
    pub params: BTreeMap<String, Option<String>>,
    /// Child keys whose mapping calls `param(...)` but is not exactly
    /// `param('literalKey')` (e.g. `param('a' + 'b')`). T7 allows constant
    /// mappings but cannot statically verify a computed parent key, so it
    /// rejects these.
    pub computed_params: BTreeSet<String>,
    /// JSON-ish path of the step's `args` object (`steps[2].args`).
    pub path: String,
}

/// The parent key of a mapping value, but only for exactly `param('key')`.
fn call_param_parent(e: &Expr) -> Option<String> {
    match e {
        Expr::Call(name, args) if name == "param" => match args.as_slice() {
            [Expr::Lit(Lit::Str(s))] => Some(s.clone()),
            _ => None,
        },
        _ => None,
    }
}

/// Does the expression call `param(...)` anywhere? Used to keep computed
/// mappings out of the constant-mapping allowance (T7).
fn expr_uses_param(e: &Expr) -> bool {
    match e {
        Expr::Call(name, args) => name == "param" || args.iter().any(expr_uses_param),
        Expr::Not(inner) | Expr::Neg(inner) => expr_uses_param(inner),
        Expr::Binary(_, lhs, rhs) => expr_uses_param(lhs) || expr_uses_param(rhs),
        Expr::Lit(_) => false,
    }
}

fn collect_call_task_steps(steps: &[Step], prefix: &str, out: &mut Vec<CallTaskUse>) {
    for (i, s) in steps.iter().enumerate() {
        let p = format!("{prefix}[{i}]");
        match s {
            Step::CallTask {
                task,
                params,
                inherit,
            } => out.push(CallTaskUse {
                task: task.clone(),
                inherit: *inherit,
                params: params
                    .iter()
                    .map(|(child, expr)| (child.clone(), call_param_parent(expr)))
                    .collect(),
                computed_params: params
                    .iter()
                    .filter(|(_, expr)| {
                        call_param_parent(expr).is_none() && expr_uses_param(expr)
                    })
                    .map(|(child, _)| child.clone())
                    .collect(),
                path: format!("{p}.args"),
            }),
            Step::If {
                then, otherwise, ..
            } => {
                collect_call_task_steps(then, &format!("{p}.then"), out);
                collect_call_task_steps(otherwise, &format!("{p}.else"), out);
            }
            Step::Loop { body, .. } | Step::While { body, .. } => {
                collect_call_task_steps(body, &format!("{p}.body"), out)
            }
            Step::WithParams { body, .. } => {
                collect_call_task_steps(body, &format!("{p}.args.body"), out)
            }
            _ => {}
        }
    }
}

fn collect_finish_steps(steps: &[Step], prefix: &str, out: &mut Vec<ExitUse>) {
    for (i, s) in steps.iter().enumerate() {
        let p = format!("{prefix}[{i}]");
        match s {
            Step::Finish { exit } => out.push(ExitUse {
                exit: exit_name(*exit).to_string(),
                path: format!("{p}.args.exit"),
            }),
            Step::If {
                then, otherwise, ..
            } => {
                collect_finish_steps(then, &format!("{p}.then"), out);
                collect_finish_steps(otherwise, &format!("{p}.else"), out);
            }
            Step::Loop { body, .. } | Step::While { body, .. } => {
                collect_finish_steps(body, &format!("{p}.body"), out)
            }
            Step::WithParams { body, .. } => {
                collect_finish_steps(body, &format!("{p}.args.body"), out)
            }
            _ => {}
        }
    }
}

fn steps_end_with_finish(steps: &[Step]) -> bool {
    match steps.last() {
        Some(Step::Finish { .. }) => true,
        Some(Step::If {
            then, otherwise, ..
        }) => {
            !then.is_empty()
                && !otherwise.is_empty()
                && steps_end_with_finish(then)
                && steps_end_with_finish(otherwise)
        }
        // `with_params` is an inline scope: if its body ends in a finish, the
        // caller's path terminates there too. Loader-generated scopes never
        // contain a finish (the target's finishes are stripped), so old flows
        // keep their T1 behaviour.
        Some(Step::WithParams { body, .. }) => steps_end_with_finish(body),
        _ => false,
    }
}

fn collect_param_steps(steps: &[Step], prefix: &str, out: &mut Vec<ParamUse>) {
    for (i, s) in steps.iter().enumerate() {
        let p = format!("{prefix}[{i}]");
        match s {
            Step::If {
                cond,
                then,
                otherwise,
            } => {
                collect_param_expr(cond, &format!("{p}.args.cond"), out);
                collect_param_steps(then, &format!("{p}.then"), out);
                collect_param_steps(otherwise, &format!("{p}.else"), out);
            }
            Step::Loop { n, body } => {
                collect_param_expr(n, &format!("{p}.args.n"), out);
                collect_param_steps(body, &format!("{p}.body"), out);
            }
            Step::While { cond, body, .. } => {
                collect_param_expr(cond, &format!("{p}.args.cond"), out);
                collect_param_steps(body, &format!("{p}.body"), out);
            }
            Step::WithParams { params, body, .. } => {
                // These expressions are evaluated in the CALLER scope, so lint
                // (T3) must require the caller to declare these keys.
                for (key, expr) in params {
                    collect_param_expr(expr, &format!("{p}.args.params.{key}"), out);
                }
                collect_param_steps(body, &format!("{p}.args.body"), out);
            }
            Step::WaitUntil { cond, .. } => {
                collect_param_expr(cond, &format!("{p}.args.cond"), out)
            }
            Step::ClickPoint {
                x,
                y,
                requires,
                expect,
                ..
            } => {
                collect_param_expr(x, &format!("{p}.args.x"), out);
                collect_param_expr(y, &format!("{p}.args.y"), out);
                collect_click_expect_params(requires, expect, &p, out);
            }
            // 参数 v2 (PARAM-V2-DESIGN §3.2.6): `text_expr` on an OCR locator /
            // guard / expectation / drag endpoint is a `param()` call site too.
            Step::Click {
                requires, expect, ..
            } => collect_click_expect_params(requires, expect, &p, out),
            Step::ClickTarget {
                find,
                requires,
                expect,
                ..
            } => {
                collect_locator_params(find, &p, out);
                collect_click_expect_params(requires, expect, &p, out);
            }
            // v1.3 long press: same `find[]` chain as click_target, so T3 must
            // see every `text_expr` param use (and the optional guards).
            Step::LongPress {
                find,
                requires,
                expect,
                ..
            } => {
                collect_locator_params(find, &p, out);
                collect_click_expect_params(requires, expect, &p, out);
            }
            Step::Drag { from, to, .. } => {
                if let Some(text) = from.text_spec() {
                    collect_text_spec_params(text, &format!("{p}.args.from"), out);
                }
                if let Some(t) = to {
                    if let Some(text) = t.text_spec() {
                        collect_text_spec_params(text, &format!("{p}.args.to"), out);
                    }
                }
            }
            Step::Swipe {
                from_x,
                from_y,
                to_x,
                to_y,
                ..
            } => {
                collect_param_expr(from_x, &format!("{p}.args.from[0]"), out);
                collect_param_expr(from_y, &format!("{p}.args.from[1]"), out);
                collect_param_expr(to_x, &format!("{p}.args.to[0]"), out);
                collect_param_expr(to_y, &format!("{p}.args.to[1]"), out);
            }
            Step::Log { text, .. } => collect_param_expr(text, &format!("{p}.args.text"), out),
            Step::SetVar { value, .. } => {
                collect_param_expr(value, &format!("{p}.args.value"), out)
            }
            Step::IncVar { by, .. } => collect_param_expr(by, &format!("{p}.args.by"), out),
            Step::Break
            | Step::Continue
            | Step::Wait { .. }
            | Step::Key { .. }
            | Step::RestartApp { .. }
            | Step::Appear { .. }
            | Step::Ocr { .. }
            | Step::OcrToVar { .. }
            | Step::DismissPopup { .. }
            | Step::CurveSwipe { .. }
            | Step::CallTask { .. }
            | Step::Finish { .. } => {}
        }
    }
}

/// `find[]` chain of a locator-driven step → `ParamUse` (T3).
/// Shared by `click_target` / `long_press` (same locator contract).
fn collect_locator_params(find: &[Locator], step_path: &str, out: &mut Vec<ParamUse>) {
    for (j, loc) in find.iter().enumerate() {
        if let Some(text) = loc.text_spec() {
            collect_text_spec_params(text, &format!("{step_path}.args.find[{j}]"), out);
        }
    }
}

/// `text_expr` expression of one OCR text site → `ParamUse` (T3).
fn collect_text_spec_params(t: &TextSpec, path: &str, out: &mut Vec<ParamUse>) {
    if let TextSpec::Expr(expr) = t {
        collect_param_expr(expr, &format!("{path}.text_expr"), out);
    }
}

/// `requires` / `expect` guard of a click-family step → `ParamUse` (T3).
///
/// Both are `Expect` values; only the `ocr_text` kind can carry a `text_expr`.
fn collect_click_expect_params(
    requires: &Option<Expect>,
    expect: &Option<Expect>,
    step_path: &str,
    out: &mut Vec<ParamUse>,
) {
    for (key, e) in [("requires", requires.as_ref()), ("expect", expect.as_ref())] {
        if let Some(e) = e {
            if let Some(text) = e.text_spec() {
                collect_text_spec_params(text, &format!("{step_path}.args.{key}"), out);
            }
        }
    }
}

fn collect_param_expr(e: &Expr, path: &str, out: &mut Vec<ParamUse>) {
    match e {
        Expr::Lit(_) => {}
        Expr::Call(name, args) => {
            if name == "param" {
                let key = match args.as_slice() {
                    [Expr::Lit(Lit::Str(s))] => Some(s.clone()),
                    _ => None,
                };
                out.push(ParamUse {
                    key,
                    path: path.to_string(),
                });
            }
            for a in args {
                collect_param_expr(a, path, out);
            }
        }
        Expr::Not(x) | Expr::Neg(x) => collect_param_expr(x, path, out),
        Expr::Binary(_, a, b) => {
            collect_param_expr(a, path, out);
            collect_param_expr(b, path, out);
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

fn collect_drag_target_assets(t: &DragTarget, out: &mut BTreeSet<String>) {
    if let DragTarget::OcrText { sel, text, .. } = t {
        if let Some(sel) = sel {
            out.insert(sel.clone());
        }
        collect_text_spec_assets(text, out);
    }
}

/// Assets referenced by a `text_expr` (`ocr('O_x')` etc.); a literal references
/// none. Shared by locators, `requires`/`expect` guards and drag endpoints so
/// T4 sees every `text_expr` asset (§3.2.6).
fn collect_text_spec_assets(t: &TextSpec, out: &mut BTreeSet<String>) {
    if let TextSpec::Expr(expr) = t {
        collect_assets_expr(expr, out);
    }
}

/// Assets referenced by a `requires` / `expect` guard: its selector plus any
/// `text_expr` asset. Shared by `click` / `click_point` / `click_target`.
fn collect_expect_assets(e: &Expect, out: &mut BTreeSet<String>) {
    out.insert(e.sel().to_string());
    if let Some(text) = e.text_spec() {
        collect_text_spec_assets(text, out);
    }
}

fn collect_click_expect_assets(
    requires: &Option<Expect>,
    expect: &Option<Expect>,
    out: &mut BTreeSet<String>,
) {
    for e in requires.iter().chain(expect.iter()) {
        collect_expect_assets(e, out);
    }
}

/// Assets referenced by one locator: its selector, any `text_expr` asset and
/// the H5 `require_template` cross-check. Shared by `click_target` /
/// `long_press` (same `find[]` contract).
fn collect_locator_assets(loc: &Locator, out: &mut BTreeSet<String>) {
    if let Some(a) = loc.asset() {
        out.insert(a.to_string());
    }
    // 参数 v2: a `text_expr` may call `ocr('O_x')` — that asset is referenced
    // just like one in any other expression (§3.2.6).
    if let Some(text) = loc.text_spec() {
        collect_text_spec_assets(text, out);
    }
    // §22.31 H5: the cross-check template is also a referenced asset.
    if let Locator::OcrText {
        require_template: Some(rt),
        ..
    }
    | Locator::OcrTextInline {
        require_template: Some(rt),
        ..
    } = loc
    {
        out.insert(rt.clone());
    }
}

fn collect_assets_steps(steps: &[Step], out: &mut BTreeSet<String>) {    for s in steps {
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
            Step::WithParams { params, body, .. } => {
                for (_, expr) in params {
                    collect_assets_expr(expr, out);
                }
                collect_assets_steps(body, out);
            }
            Step::Wait { .. }
            | Step::Key { .. }
            | Step::RestartApp { .. }
            | Step::Break
            | Step::Continue
            | Step::CurveSwipe { .. }
            | Step::CallTask { .. } => {}
            Step::WaitUntil { cond, .. } => collect_assets_expr(cond, out),
            Step::Appear { sel } | Step::Ocr { sel } => {
                out.insert(sel.clone());
            }
            Step::OcrToVar { sel, .. } => {
                out.insert(sel.clone());
            }
            Step::SetVar { value, .. } => collect_assets_expr(value, out),
            Step::IncVar { by, .. } => collect_assets_expr(by, out),
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
                collect_click_expect_assets(requires, expect, out);
            }
            Step::ClickTarget {
                find,
                requires,
                expect,
                ..
            } => {
                for loc in find {
                    collect_locator_assets(loc, out);
                }
                collect_click_expect_assets(requires, expect, out);
            }
            // v1.3 long press: same locator chain as click_target (T4 assets).
            Step::LongPress {
                find,
                requires,
                expect,
                ..
            } => {
                for loc in find {
                    collect_locator_assets(loc, out);
                }
                collect_click_expect_assets(requires, expect, out);
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
                collect_click_expect_assets(requires, expect, out);
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
            Step::Drag { from, to, .. } => {
                collect_drag_target_assets(from, out);
                if let Some(t) = to {
                    collect_drag_target_assets(t, out);
                }
            }
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
    fn parses_key_and_restart_app_recovery_steps() {
        let key = serde_json::json!({"op":"key","args":{"keycode":4}});
        match parse_step(&key, "steps[0]").expect("key step") {
            Step::Key { keycode } => assert_eq!(keycode, 4),
            other => panic!("unexpected key parse: {other:?}"),
        }
        let app = serde_json::json!({
            "op":"restart_app","args":{"package":"com.netease.onmyoji"}
        });
        match parse_step(&app, "steps[1]").expect("restart step") {
            Step::RestartApp { package } => assert_eq!(package, "com.netease.onmyoji"),
            other => panic!("unexpected restart parse: {other:?}"),
        }
    }

    #[test]
    fn while_has_no_iteration_limit_arg() {
        // max_iters belongs to `loop`, not `while`; unknown args are ignored, and
        // the condition alone controls entry/false/true (0 is falsy at runtime).
        let v = serde_json::json!({
            "op":"while","args":{"cond":"0","max_iters":3},"body":[]
        });
        assert!(parse_step(&v, "steps[0]").is_ok());
    }

    #[test]
    fn parses_symbolic_logic_operators() {
        assert!(parse_expr("appear('a') && appear('b')").is_ok());
        assert!(parse_expr("appear('a') || appear('b')").is_ok());
        assert!(parse_expr("!appear('a')").is_ok());
        assert!(parse_expr("text_visible('a') or text_visible('b')").is_ok());
        // mixed words/symbols still follow and-before-or precedence
        let e = parse_expr("appear('a') || appear('b') && !appear('c')").expect("parse");
        match e {
            Expr::Binary(BinOp::Or, _, rhs) => {
                assert!(matches!(*rhs, Expr::Binary(BinOp::And, _, _)));
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
    fn inline_ocr_roi_is_parsed_without_asset_reference() {
        let flow = Flow::from_json(
            r#"{"schema_version":1,"name":"t","steps":[{"op":"click_target","args":{
                 "find":[{"kind":"ocr_text","roi":[5,1,1272,717],"text":"个人突破"}]}}]}"#,
        )
        .expect("flow");
        assert!(flow.referenced_assets().is_empty(), "inline OCR needs no asset");
        match &flow.steps[0] {
            Step::ClickTarget { find, .. } => match &find[0] {
                Locator::OcrTextInline { roi, text, .. } => {
                    assert_eq!(*roi, [5, 1, 1272, 717]);
                    assert_eq!(text, &TextSpec::Literal("个人突破".to_string()));
                    assert!(find[0].asset().is_none());
                }
                other => panic!("unexpected locator: {other:?}"),
            },
            other => panic!("unexpected step: {other:?}"),
        }
    }

    // ---- v1.2.5 (O-2): drag `region` inline shape ----

    /// Parse one `drag` step whose `from` is a region with `shape_json`
    /// (an already-serialized shape object), returning the parse error on failure.
    fn drag_with_from_shape(shape_json: &str) -> Result<Flow, FlowError> {
        let text = format!(
            r#"{{"schema_version":1,"name":"t","steps":[
                 {{"op":"drag","args":{{
                   "from":{{"kind":"region","roi":[0,0,80,60],"shape":{shape_json}}},
                   "direction":"up","distance":[10,20]}}}}]}}"#
        );
        Flow::from_json(&text)
    }

    #[test]
    fn parses_drag_region_with_polygon_shape() {
        let flow =
            drag_with_from_shape(r#"{"kind":"polygon","points":[[0,0],[80,0],[0,60]],"invert":false}"#)
                .expect("polygon drag shape must parse");
        match &flow.steps[0] {
            Step::Drag { from, .. } => match from {
                DragTarget::Region { roi, shape } => {
                    assert_eq!(*roi, [0, 0, 80, 60], "roi stays mandatory");
                    let shape = shape.as_ref().expect("shape");
                    assert_eq!(shape.kind, "polygon");
                    assert_eq!(shape.points, Some(vec![[0, 0], [80, 0], [0, 60]]));
                    assert!(shape.roi.is_none());
                    assert!(!shape.invert);
                }
                other => panic!("unexpected drag target: {other:?}"),
            },
            other => panic!("unexpected step: {other:?}"),
        }
        assert!(
            flow.referenced_assets().is_empty(),
            "inline drag shape must not add an asset reference"
        );
    }

    #[test]
    fn parses_drag_region_shape_invert_true() {
        let flow = drag_with_from_shape(r#"{"kind":"rect","roi":[10,10,30,20],"invert":true}"#)
            .expect("invert drag shape must parse");
        match &flow.steps[0] {
            Step::Drag { from, .. } => match from {
                DragTarget::Region { shape, .. } => {
                    let shape = shape.as_ref().expect("shape");
                    assert_eq!(shape.kind, "rect");
                    assert_eq!(shape.roi, Some([10, 10, 30, 20]));
                    assert!(shape.points.is_none());
                    assert!(shape.invert, "invert must be preserved, not defaulted");
                }
                other => panic!("unexpected drag target: {other:?}"),
            },
            other => panic!("unexpected step: {other:?}"),
        }
    }

    #[test]
    fn drag_region_without_shape_keeps_bbox_only_semantics() {
        let flow = Flow::from_json(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"drag","args":{
                   "from":{"kind":"region","roi":[5,6,10,20]},
                   "direction":"down","distance":[5,9]}}]}"#,
        )
        .expect("legacy drag flow must parse unchanged");
        match &flow.steps[0] {
            Step::Drag { from, to, .. } => {
                assert!(to.is_none());
                match from {
                    DragTarget::Region { roi, shape } => {
                        assert_eq!(*roi, [5, 6, 10, 20]);
                        assert!(shape.is_none(), "absent `shape` must stay None (bbox path)");
                    }
                    other => panic!("unexpected drag target: {other:?}"),
                }
            }
            other => panic!("unexpected step: {other:?}"),
        }
    }

    #[test]
    fn rejects_invalid_drag_shape_geometry_with_exact_path() {
        let cases: &[(&str, &str, &str)] = &[
            (
                r#"{"kind":"circle"}"#,
                "steps[0].args.from.shape",
                "unknown drag shape",
            ),
            (
                r#"{"kind":"polygon","points":[[0,0],[10,10]]}"#,
                "steps[0].args.from.shape.points",
                "at least 3",
            ),
            (
                r#"{"kind":"polygon","points":[[-1,0],[10,0],[0,10]]}"#,
                "steps[0].args.from.shape.points[0][0]",
                "0..=1000000",
            ),
            (
                r#"{"kind":"polygon","points":[[0,0],[2147483647,0],[0,2147483647]]}"#,
                "steps[0].args.from.shape.points[1][0]",
                "0..=1000000",
            ),
            (
                // Review repro: 2e9-long square with every vertex repeated.
                // The old i64 shoelace accumulated to ~1.6e19 and panicked in
                // debug; the coordinate bound rejects it before any arithmetic.
                r#"{"kind":"polygon","points":[[0,0],[2000000000,0],[2000000000,2000000000],[0,2000000000],[0,0],[2000000000,0],[2000000000,2000000000],[0,2000000000]]}"#,
                "steps[0].args.from.shape.points[1][0]",
                "0..=1000000",
            ),
            (
                r#"{"kind":"polygon","points":[[0,0],[10,10],[0,10],[10,0]]}"#,
                "steps[0].args.from.shape.points",
                "self-intersect",
            ),
            (
                r#"{"kind":"rect","roi":[-5,0,10,10]}"#,
                "steps[0].args.from.shape.roi[0]",
                "0..=1000000",
            ),
            (
                r#"{"kind":"rect","roi":[2147483647,0,1,1],"invert":true}"#,
                "steps[0].args.from.shape.roi[0]",
                "0..=1000000",
            ),
            (
                r#"{"kind":"rect","roi":[600000,0,600000,10]}"#,
                "steps[0].args.from.shape.roi",
                "x+w",
            ),
            (
                r#"{"kind":"polygon","points":[[0,0],[10,10],[20,20]]}"#,
                "steps[0].args.from.shape.points",
                "zero area",
            ),
            (
                r#"{"kind":"polygon","roi":[0,0,10,10],"points":[[0,0],[10,0],[0,10]]}"#,
                "steps[0].args.from.shape.roi",
                "not `roi`",
            ),
            (
                r#"{"kind":"rect","roi":[0,0,0,10]}"#,
                "steps[0].args.from.shape.roi",
                "> 0",
            ),
            (
                r#"{"kind":"rect"}"#,
                "steps[0].args.from.shape.roi",
                "needs `roi`",
            ),
            (
                r#"{"kind":"rect","roi":[0,0,10,10],"points":[[0,0],[10,0],[0,10]]}"#,
                "steps[0].args.from.shape.points",
                "not `points`",
            ),
            (
                r#"{"kind":"rect","roi":[0,0,10,10],"invert":"yes"}"#,
                "steps[0].args.from.shape.invert",
                "boolean",
            ),
        ];
        for (json, path, needle) in cases {
            let err = drag_with_from_shape(json).expect_err(&format!("{json} must be rejected"));
            assert_eq!(err.path, *path, "wrong path for {json}: {err:?}");
            assert!(
                err.msg.contains(needle),
                "message for {json} must contain {needle:?}: {}",
                err.msg
            );
        }
    }

    #[test]
    fn rejects_drag_polygon_with_too_many_points() {
        // 257 vertices is over MAX_DRAG_SHAPE_POINTS: rejected before parsing
        // coordinates or running the O(n²) self-intersection test.
        let points: Vec<String> = (0..257).map(|i| format!("[{i},0]")).collect();
        let json = format!(r#"{{"kind":"polygon","points":[{}]}}"#, points.join(","));
        let err = drag_with_from_shape(&json).expect_err("257 points must be rejected");
        assert_eq!(err.path, "steps[0].args.from.shape.points");
        assert!(err.msg.contains("at most 256"), "{}", err.msg);
    }

    #[test]
    fn drag_region_shape_null_is_treated_as_absent() {
        let flow = drag_with_from_shape("null").expect("shape:null must parse as no shape");
        match &flow.steps[0] {
            Step::Drag { from, .. } => match from {
                DragTarget::Region { roi, shape } => {
                    assert_eq!(*roi, [0, 0, 80, 60]);
                    assert!(shape.is_none(), "shape:null must stay None (bbox path)");
                }
                other => panic!("unexpected drag target: {other:?}"),
            },
            other => panic!("unexpected step: {other:?}"),
        }
    }

    #[test]
    fn rejects_drag_shape_on_non_region_targets() {
        // P2-4: a `shape` on `point`/`ocr_text` used to be silently ignored,
        // which runs a different gesture than the author wrote. It is now a
        // loud parse error with the misplaced field's exact path.
        let point = r#"{"schema_version":1,"name":"t","steps":[
             {"op":"drag","args":{
               "from":{"kind":"point","x":10,"y":10,
                       "shape":{"kind":"rect","roi":[0,0,10,10]}},
               "direction":"up","distance":[5,9]}}]}"#;
        let err = Flow::from_json(point).expect_err("shape on a point target must be rejected");
        assert_eq!(err.path, "steps[0].args.from.shape");
        assert!(
            err.msg.contains("only supported on `region`"),
            "{}",
            err.msg
        );

        let ocr = r#"{"schema_version":1,"name":"t","steps":[
             {"op":"drag","args":{
               "from":{"kind":"ocr_text","sel":"O_TEXT","text":"x",
                       "shape":{"kind":"polygon","points":[[0,0],[1,0],[0,1]]}},
               "direction":"up","distance":[5,9]}}]}"#;
        let err = Flow::from_json(ocr).expect_err("shape on an ocr_text target must be rejected");
        assert_eq!(err.path, "steps[0].args.from.shape");
        assert!(
            err.msg.contains("only supported on `region`"),
            "{}",
            err.msg
        );

        // `shape:null` is still just "absent" on a non-region target.
        let null_point = r#"{"schema_version":1,"name":"t","steps":[
             {"op":"drag","args":{
               "from":{"kind":"point","x":10,"y":10,"shape":null},
               "direction":"up","distance":[5,9]}}]}"#;
        let flow = Flow::from_json(null_point).expect("shape:null on a point target is absent");
        match &flow.steps[0] {
            Step::Drag { from, .. } => assert!(matches!(from, DragTarget::Point { x: 10, y: 10 })),
            other => panic!("unexpected step: {other:?}"),
        }
    }

    #[test]
    fn parses_drag_to_shape_with_invert() {
        // P2-9: the `to` endpoint accepts the same inline shape contract,
        // including `invert:true` (not just `from`).
        let flow = Flow::from_json(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"drag","args":{
                   "from":{"kind":"region","roi":[0,0,10,10]},
                   "to":{"kind":"region","roi":[20,0,10,48],
                         "shape":{"kind":"rect","roi":[20,0,10,48],"invert":true}},
                   "direction":"right"}}]}"#,
        )
        .expect("to shape with invert must parse");
        match &flow.steps[0] {
            Step::Drag { to, .. } => match to {
                Some(DragTarget::Region { shape, .. }) => {
                    let shape = shape.as_ref().expect("to shape");
                    assert_eq!(shape.kind, "rect");
                    assert!(shape.invert, "to shape invert must be preserved");
                }
                other => panic!("unexpected drag to target: {other:?}"),
            },
            other => panic!("unexpected step: {other:?}"),
        }
    }

    #[test]
    fn variable_steps_parse_and_collect_ocr_asset() {
        let flow = Flow::from_json(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"set_var","args":{"name":"n","value":"0"}},
                 {"op":"inc_var","args":{"name":"n","by":"1"}},
                 {"op":"ocr_to_var","args":{"sel":"O_LIST","var":"txt"}},
                 {"op":"if","args":{"cond":"var('n') > 0"},"then":[{"op":"finish","args":{"exit":"done"}}],"else":[]}
               ]}"#,
        )
        .expect("flow");
        let refs = flow.referenced_assets();
        assert!(refs.contains("O_LIST"), "{refs:?}");
        assert_eq!(flow.steps.len(), 4);
    }

    #[test]
    fn saved_region_locator_is_parsed_and_collected_as_asset() {
        let flow = Flow::from_json(
            r#"{"schema_version":1,"name":"t","steps":[{"op":"click_target","args":{
                 "find":[{"kind":"region","sel":"右侧列表"}]}}]}"#,
        )
        .expect("flow");
        let refs = flow.referenced_assets();
        assert!(refs.contains("右侧列表"), "{refs:?}");
        match &flow.steps[0] {
            Step::ClickTarget { find, .. } => match &find[0] {
                Locator::RegionRef { sel } => assert_eq!(sel, "右侧列表"),
                other => panic!("unexpected locator: {other:?}"),
            },
            other => panic!("unexpected step: {other:?}"),
        }
    }

    #[test]
    fn inline_ocr_requires_a_positive_roi() {
        for bad in [
            r#"{"kind":"ocr_text","roi":[1,2,3],"text":"x"}"#,
            r#"{"kind":"ocr_text","roi":[1,2,0,9],"text":"x"}"#,
            r#"{"kind":"ocr_text","roi":[1,2,9,0],"text":"x"}"#,
        ] {
            let text = format!(
                r#"{{"schema_version":1,"name":"t","steps":[{{"op":"click_target","args":{{"find":[{bad}]}}}}]}}"#
            );
            assert!(Flow::from_json(&text).is_err(), "should reject {bad}");
        }
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
                        text: TextSpec::Literal("返回".to_string()),
                        require_template: None,
                        region: None,
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

    // ---- 参数 v2 (PARAM-V2-DESIGN §3.2): `text_expr` OCR text source ----

    /// One flow that uses `text_expr` in every supported position.
    const TEXT_EXPR_FLOW: &str = r#"{"schema_version":1,"name":"t","steps":[
        {"op":"click_target","args":{
          "find":[{"kind":"ocr_text","sel":"O_BTN","text_expr":"param('click_text')"},
                  {"kind":"ocr_text","roi":[5,1,60,40],"text_expr":"param('inline_text')"}],
          "requires":{"kind":"ocr_text","sel":"O_TOP","text_expr":"param('guard_text')"},
          "expect":{"kind":"ocr_text","sel":"O_TOP","text_expr":"param('expect_text')","timeout_ms":500}}},
        {"op":"drag","args":{
          "from":{"kind":"ocr_text","sel":"O_ROW","text_expr":"param('from_text')"},
          "to":{"kind":"ocr_text","roi":[0,0,64,48],"text_expr":"param('to_text')"},
          "direction":"up","distance":[10,20]}},
        {"op":"finish","args":{"exit":"done"}}]}"#;

    #[test]
    fn text_expr_is_parsed_in_all_four_ocr_text_positions() {
        let flow = Flow::from_json(TEXT_EXPR_FLOW).expect("text_expr flow must parse");
        let Step::ClickTarget {
            find,
            requires,
            expect,
            ..
        } = &flow.steps[0]
        else {
            panic!("expected click_target, got {:?}", flow.steps[0]);
        };
        assert_eq!(
            find[0].text_spec().and_then(|t| t.param_key()),
            Some("click_text")
        );
        assert_eq!(
            find[1].text_spec(),
            Some(&TextSpec::Expr(
                parse_expr("param('inline_text')").expect("expr")
            ))
        );
        assert_eq!(
            requires
                .as_ref()
                .and_then(|e| e.text_spec())
                .and_then(|t| t.param_key()),
            Some("guard_text")
        );
        assert_eq!(
            expect
                .as_ref()
                .and_then(|e| e.text_spec())
                .and_then(|t| t.param_key()),
            Some("expect_text")
        );

        let Step::Drag { from, to, .. } = &flow.steps[1] else {
            panic!("expected drag, got {:?}", flow.steps[1]);
        };
        assert_eq!(from.text_spec().and_then(|t| t.param_key()), Some("from_text"));
        assert_eq!(
            to.as_ref()
                .and_then(|t| t.text_spec())
                .and_then(|t| t.param_key()),
            Some("to_text")
        );
    }

    #[test]
    fn text_and_text_expr_are_mutually_exclusive_with_exact_paths() {
        // Locator: the error path must name the offending locator field.
        let locator = r#"{"schema_version":1,"name":"t","steps":[{"op":"click_target","args":{"find":[
            {"kind":"ocr_text","sel":"O","text":"x","text_expr":"param('k')"}]}}]}"#;
        let e = errs(locator);
        assert_eq!(e.len(), 1);
        assert!(
            e[0].path.ends_with("args.find[0].text_expr"),
            "{}",
            e[0]
        );
        assert!(e[0].msg.contains("mutually exclusive"), "{}", e[0]);

        // requires / expect (shared parser).
        let expect = r#"{"schema_version":1,"name":"t","steps":[{"op":"click","args":{
            "sel":"S","requires":{"kind":"ocr_text","sel":"O","text":"x","text_expr":"param('k')"}}}]}"#;
        let e = errs(expect);
        assert!(e[0].path.ends_with("args.requires.text_expr"), "{}", e[0]);

        // Drag endpoint.
        let drag = r#"{"schema_version":1,"name":"t","steps":[{"op":"drag","args":{
            "from":{"kind":"ocr_text","sel":"O","text":"x","text_expr":"param('k')"},
            "direction":"up","distance":[1,2]}}]}"#;
        let e = errs(drag);
        assert!(e[0].path.ends_with("args.from.text_expr"), "{}", e[0]);
    }

    #[test]
    fn ocr_text_needs_exactly_one_text_source() {
        // Neither `text` nor `text_expr` => a loud parse error that names both.
        let cases = [
            r#"{"schema_version":1,"name":"t","steps":[{"op":"click_target","args":{"find":[
                {"kind":"ocr_text","sel":"O"}]}}]}"#,
            r#"{"schema_version":1,"name":"t","steps":[{"op":"click","args":{
                "sel":"S","expect":{"kind":"ocr_text","sel":"O"}}}]}"#,
            r#"{"schema_version":1,"name":"t","steps":[{"op":"drag","args":{
                "from":{"kind":"ocr_text","sel":"O"},"direction":"up","distance":[1,2]}}]}"#,
        ];
        for json in cases {
            let e = errs(json);
            assert_eq!(e.len(), 1, "must reject {json}");
            assert!(
                e[0].msg.contains("`text` / `text_expr`"),
                "message must name both spellings: {}",
                e[0]
            );
        }
    }

    #[test]
    fn text_expr_must_be_a_valid_expression_string() {
        let bad_expr = r#"{"schema_version":1,"name":"t","steps":[{"op":"click_target","args":{"find":[
            {"kind":"ocr_text","sel":"O","text_expr":"1 +"}]}}]}"#;
        let e = errs(bad_expr);
        assert!(e[0].path.ends_with("args.find[0].text_expr"), "{}", e[0]);

        let not_a_string = r#"{"schema_version":1,"name":"t","steps":[{"op":"click","args":{
            "sel":"S","expect":{"kind":"ocr_text","sel":"O","text_expr":42}}}]}"#;
        let e = errs(not_a_string);
        assert!(e[0].path.ends_with("args.expect.text_expr"), "{}", e[0]);
        assert!(e[0].msg.contains("expression string"), "{}", e[0]);
    }

    #[test]
    fn literal_text_is_still_a_literal_and_not_an_expression() {
        // R3: an old `text` that happens to look like an expression stays a
        // byte-for-byte literal (no evaluation, no T3 use).
        let flow = Flow::from_json(
            r#"{"schema_version":1,"name":"t","steps":[{"op":"click_target","args":{"find":[
                 {"kind":"ocr_text","sel":"O","text":"param('not_evaluated')"}]}}]}"#,
        )
        .expect("flow");
        let Step::ClickTarget { find, .. } = &flow.steps[0] else {
            panic!("expected click_target");
        };
        assert_eq!(
            find[0].text_spec(),
            Some(&TextSpec::Literal("param('not_evaluated')".to_string()))
        );
        assert!(
            flow.param_uses().is_empty(),
            "a literal must never be treated as a param() use"
        );
    }

    #[test]
    fn param_uses_at_collects_every_text_expr_site() {
        let flow = Flow::from_json(TEXT_EXPR_FLOW).expect("flow");
        let uses = flow.param_uses_at();
        let got: Vec<(Option<&str>, &str)> = uses
            .iter()
            .map(|u| (u.key.as_deref(), u.path.as_str()))
            .collect();
        for (key, path) in [
            ("click_text", "steps[0].args.find[0].text_expr"),
            ("inline_text", "steps[0].args.find[1].text_expr"),
            ("guard_text", "steps[0].args.requires.text_expr"),
            ("expect_text", "steps[0].args.expect.text_expr"),
            ("from_text", "steps[1].args.from.text_expr"),
            ("to_text", "steps[1].args.to.text_expr"),
        ] {
            assert!(
                got.contains(&(Some(key), path)),
                "missing ({key}, {path}) in {got:?}"
            );
        }
        assert_eq!(uses.len(), 6, "{got:?}");
        assert!(flow.param_uses().contains("expect_text"));
    }

    #[test]
    fn text_expr_with_a_computed_key_is_undecidable_for_t3() {
        let flow = Flow::from_json(
            r#"{"schema_version":1,"name":"t","steps":[{"op":"click_target","args":{"find":[
                 {"kind":"ocr_text","sel":"O","text_expr":"param('a' + 'b')"}]}}]}"#,
        )
        .expect("flow");
        assert!(
            flow.param_uses().is_empty(),
            "a computed key is not a literal key"
        );
        let uses = flow.param_uses_at();
        assert_eq!(uses.len(), 1, "{uses:?}");
        assert_eq!(uses[0].key, None);
        assert!(
            uses[0].path.ends_with("steps[0].args.find[0].text_expr"),
            "{}",
            uses[0].path
        );
    }

    #[test]
    fn text_expr_assets_are_collected_for_t4() {
        // A `text_expr` may call ocr()/appear()/ocr_digit(); those asset ids
        // must be collected exactly like a `cond` expression's are.
        let flow = Flow::from_json(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click_target","args":{
                   "find":[{"kind":"ocr_text","sel":"O_BTN","text_expr":"ocr('O_HINT')"}],
                   "requires":{"kind":"ocr_text","sel":"O_TOP","text_expr":"ocr('O_GUARD')"},
                   "expect":{"kind":"ocr_text","sel":"O_TOP","text_expr":"ocr('O_EXPECT')"}}},
                 {"op":"drag","args":{
                   "from":{"kind":"ocr_text","sel":"O_ROW","text_expr":"ocr('O_FROM')"},
                   "to":{"kind":"ocr_text","roi":[0,0,64,48],"text_expr":"ocr('O_TO')"},
                   "direction":"up","distance":[10,20]}}]}"#,
        )
        .expect("flow");
        let got: Vec<String> = flow.referenced_assets().into_iter().collect();
        assert_eq!(
            got,
            vec![
                "O_BTN", "O_EXPECT", "O_FROM", "O_GUARD", "O_HINT", "O_ROW", "O_TO", "O_TOP"
            ]
        );
    }

    // ---- v1.3 operation region + long_press + curve_swipe ----

    #[test]
    fn region_parses_all_modes_on_ocr_locators_and_drag_endpoints() {
        let flow = Flow::from_json(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"click_target","args":{"find":[
                    {"kind":"ocr_text","sel":"O_A","text":"甲","region":{"mode":"asset"}},
                    {"kind":"ocr_text","roi":[0,0,64,48],"text":"乙","region":{"mode":"ocr"}},
                    {"kind":"ocr_text","sel":"O_B","text":"丙","region":{"mode":"nearest"}},
                    {"kind":"ocr_text","sel":"O_C","text":"丁",
                     "region":{"mode":"offset","offset":[10,-5]}},
                    {"kind":"ocr_text","sel":"O_D","text":"戊",
                     "region":{"mode":"offset","offset":[1,2,30,40]}}]}},
                 {"op":"drag","args":{
                    "from":{"kind":"ocr_text","sel":"O_R1","text":"行1",
                            "region":{"mode":"nearest"}},
                    "to":{"kind":"ocr_text","roi":[0,0,64,48],"text":"行2",
                          "region":{"mode":"offset","offset":[0,0,10,10]}},
                    "direction":"up","distance":[10,20]}},
                 {"op":"long_press","args":{
                    "find":[{"kind":"ocr_text","sel":"O_L","text":"长按",
                             "region":{"mode":"ocr"}}],
                    "hold_ms":800}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        )
        .expect("v1.3 region flow must parse");
        let Step::ClickTarget { find, .. } = &flow.steps[0] else {
            panic!("expected click_target");
        };
        assert_eq!(find[0].region(), Some(OpRegion::Asset));
        assert_eq!(find[1].region(), Some(OpRegion::Ocr));
        assert_eq!(find[2].region(), Some(OpRegion::Nearest));
        assert_eq!(
            find[3].region(),
            Some(OpRegion::Offset {
                dx: 10,
                dy: -5,
                w: None,
                h: None
            })
        );
        assert_eq!(
            find[4].region(),
            Some(OpRegion::Offset {
                dx: 1,
                dy: 2,
                w: Some(30),
                h: Some(40)
            })
        );

        let Step::Drag { from, to, .. } = &flow.steps[1] else {
            panic!("expected drag");
        };
        assert_eq!(from.region(), Some(OpRegion::Nearest));
        assert_eq!(
            to.as_ref().and_then(|t| t.region()),
            Some(OpRegion::Offset {
                dx: 0,
                dy: 0,
                w: Some(10),
                h: Some(10)
            })
        );

        let Step::LongPress { find, hold_ms, hold_range, .. } = &flow.steps[2] else {
            panic!("expected long_press");
        };
        assert_eq!(*hold_ms, 800);
        assert_eq!(*hold_range, None);
        assert_eq!(find[0].region(), Some(OpRegion::Ocr));

        // No `region` field = historical behaviour (None), not an implicit mode.
        let plain = Flow::from_json(
            r#"{"schema_version":1,"name":"t","steps":[{"op":"click_target","args":{"find":[
                 {"kind":"ocr_text","sel":"O","text":"x"}]}}]}"#,
        )
        .expect("plain flow");
        let Step::ClickTarget { find, .. } = &plain.steps[0] else {
            panic!("expected click_target");
        };
        assert_eq!(find[0].region(), None);
    }

    #[test]
    fn region_offset_validation_is_loud_and_path_precise() {
        let cases: [(&str, &str); 9] = [
            // (region JSON, expected path suffix)
            (r#"{"mode":"asset","offset":[1,2]}"#, ".region.offset"),
            (r#"{"mode":"offset"}"#, ".region"),
            (r#"{"mode":"offset","offset":[1]}"#, ".region.offset"),
            (r#"{"mode":"offset","offset":[1,2,3]}"#, ".region.offset"),
            (r#"{"mode":"offset","offset":[1,2,0,30]}"#, ".region.offset"),
            (r#"{"mode":"offset","offset":[1,2,30,-1]}"#, ".region.offset"),
            (r#"{"mode":"offset","offset":[1,"x"]}"#, ".region.offset[1]"),
            (r#"{"mode":"magic"}"#, ".region.mode"),
            (r#"[1,2]"#, ".region"),
        ];
        for (region, want_path) in cases {
            let json = format!(
                r#"{{"schema_version":1,"name":"t","steps":[{{"op":"click_target","args":{{"find":[
                     {{"kind":"ocr_text","sel":"O","text":"x","region":{region}}}]}}}}]}}"#
            );
            let e = errs(&json);
            assert_eq!(e.len(), 1, "must reject region {region}");
            assert!(
                e[0].path.ends_with(want_path),
                "region {region}: expected path ending {want_path}, got {}",
                e[0]
            );
        }
        // Out-of-range offset values are rejected (i32-safe cap).
        let json = r#"{"schema_version":1,"name":"t","steps":[{"op":"click_target","args":{"find":[
             {"kind":"ocr_text","sel":"O","text":"x",
              "region":{"mode":"offset","offset":[1000001,0]}}]}}]}"#;
        let e = errs(json);
        assert!(e[0].path.ends_with(".region.offset[0]"), "{}", e[0]);
    }

    #[test]
    fn region_is_rejected_on_non_ocr_locator_and_drag_kinds() {
        let find_cases = [
            r#"{"kind":"template","sel":"I","region":{"mode":"ocr"}}"#,
            r#"{"kind":"region","roi":[0,0,10,10],"region":{"mode":"ocr"}}"#,
            r#"{"kind":"coord_memory","key":"k","region":{"mode":"ocr"}}"#,
        ];
        for find in find_cases {
            let json = format!(
                r#"{{"schema_version":1,"name":"t","steps":[{{"op":"click_target","args":{{"find":[{find}]}}}}]}}"#
            );
            let e = errs(&json);
            assert_eq!(e.len(), 1, "must reject {find}");
            assert!(
                e[0].path.ends_with(".find[0].region"),
                "{find}: {}",
                e[0]
            );
        }
        for target in [
            r#"{"kind":"point","x":1,"y":2,"region":{"mode":"ocr"}}"#,
            r#"{"kind":"region","roi":[0,0,10,10],"region":{"mode":"ocr"}}"#,
        ] {
            let json = format!(
                r#"{{"schema_version":1,"name":"t","steps":[{{"op":"drag","args":{{
                     "from":{target},"direction":"up","distance":[1,2]}}}}]}}"#
            );
            let e = errs(&json);
            assert_eq!(e.len(), 1, "must reject drag target {target}");
            assert!(
                e[0].path.ends_with(".from.region"),
                "{target}: {}",
                e[0]
            );
        }
    }

    #[test]
    fn long_press_parses_defaults_ranges_and_rejects_bad_values() {
        let flow = Flow::from_json(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"long_press","args":{"find":[{"kind":"template","sel":"I_BTN"}]}}]}"#,
        )
        .expect("default long_press");
        match &flow.steps[0] {
            Step::LongPress {
                hold_ms,
                hold_range,
                requires,
                expect,
                ..
            } => {
                assert_eq!(*hold_ms, 3000, "default hold_ms");
                assert_eq!(*hold_range, None);
                assert!(requires.is_none() && expect.is_none());
            }
            other => panic!("unexpected step {other:?}"),
        }

        let with_range = Flow::from_json(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"long_press","args":{
                   "find":[{"kind":"template","sel":"I_BTN"}],
                   "hold_ms":800,"hold_range":[2500,3500],
                   "requires":{"kind":"appear","sel":"I_GUARD"},
                   "expect":{"kind":"appear","sel":"I_AFTER","timeout_ms":500}}}]}"#,
        )
        .expect("long_press with range");
        match &with_range.steps[0] {
            Step::LongPress {
                hold_ms: 800,
                hold_range: Some([2500, 3500]),
                requires: Some(Expect::Appear { sel, .. }),
                expect: Some(Expect::Appear { sel: esel, .. }),
                ..
            } => {
                assert_eq!(sel, "I_GUARD");
                assert_eq!(esel, "I_AFTER");
            }
            other => panic!("unexpected step {other:?}"),
        }

        let bad = [
            (r#"{"find":[{"kind":"template","sel":"I"}],"hold_ms":99}"#, ".hold_ms"),
            (r#"{"find":[{"kind":"template","sel":"I"}],"hold_ms":10001}"#, ".hold_ms"),
            (r#"{"find":[{"kind":"template","sel":"I"}],"hold_range":[50,500]}"#, ".hold_range[0]"),
            (r#"{"find":[{"kind":"template","sel":"I"}],"hold_range":[500,200]}"#, ".hold_range"),
            (r#"{"find":[{"kind":"template","sel":"I"}],"hold_ms":"800"}"#, ".hold_ms"),
            (r#"{"find":[]}"#, ".find"),
            (r#"{}"#, ".args.find"),
        ];
        for (args, want) in bad {
            let json = format!(
                r#"{{"schema_version":1,"name":"t","steps":[{{"op":"long_press","args":{args}}}]}}"#
            );
            let e = errs(&json);
            assert_eq!(e.len(), 1, "must reject long_press {args}");
            assert!(
                e[0].path.contains(want),
                "long_press {args}: expected path containing {want}, got {}",
                e[0]
            );
        }
    }

    #[test]
    fn long_press_feeds_t3_params_and_t4_assets() {
        let flow = Flow::from_json(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"long_press","args":{
                   "find":[{"kind":"ocr_text","sel":"O_BTN","text_expr":"param('hold_text')",
                            "require_template":"I_ICON"}],
                   "requires":{"kind":"ocr_text","sel":"O_GUARD","text_expr":"ocr('O_G')"},
                   "expect":{"kind":"ocr_text","sel":"O_AFTER","text_expr":"param('after')"}}}]}"#,
        )
        .expect("long_press flow");
        let uses = flow.param_uses_at();
        let got: Vec<(Option<&str>, &str)> = uses
            .iter()
            .map(|u| (u.key.as_deref(), u.path.as_str()))
            .collect();
        for (key, path) in [
            ("hold_text", "steps[0].args.find[0].text_expr"),
            ("after", "steps[0].args.expect.text_expr"),
        ] {
            assert!(
                got.contains(&(Some(key), path)),
                "missing ({key}, {path}) in {got:?}"
            );
        }
        let assets: Vec<String> = flow.referenced_assets().into_iter().collect();
        for want in ["O_BTN", "I_ICON", "O_GUARD", "O_G", "O_AFTER"] {
            assert!(
                assets.contains(&want.to_string()),
                "missing asset {want} in {assets:?}"
            );
        }
    }

    #[test]
    fn curve_swipe_parses_points_and_durations() {
        let flow = Flow::from_json(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"curve_swipe","args":{"points":[[10,20],[30,40],[50,60]]}}]}"#,
        )
        .expect("default curve_swipe");
        match &flow.steps[0] {
            Step::CurveSwipe {
                points,
                duration_ms,
            } => {
                assert_eq!(points, &vec![[10, 20], [30, 40], [50, 60]]);
                assert_eq!(*duration_ms, [600, 1000], "default duration");
            }
            other => panic!("unexpected step {other:?}"),
        }

        // A single integer means one fixed total duration.
        let fixed = Flow::from_json(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"curve_swipe","args":{"points":[[1,1],[2,2]],"duration_ms":900}}]}"#,
        )
        .expect("fixed duration");
        match &fixed.steps[0] {
            Step::CurveSwipe { duration_ms, .. } => assert_eq!(*duration_ms, [900, 900]),
            other => panic!("unexpected step {other:?}"),
        }

        let bad = [
            (r#"{"points":[[1,1]]}"#, ".points"),
            (
                r#"{"points":[[1,1],[2,2],[3,3],[4,4],[5,5],[6,6],[7,7],[8,8],[9,9],[10,10],[11,11],[12,12],[13,13],[14,14],[15,15],[16,16],[17,17],[18,18],[19,19],[20,20],[21,21],[22,22],[23,23],[24,24],[25,25],[26,26],[27,27],[28,28],[29,29],[30,30],[31,31],[32,32],[33,33]]}"#,
                ".points",
            ),
            (r#"{"points":[[1,1],[2,2,3]]}"#, ".points[1]"),
            (r#"{"points":[[1,1],[-2,2]]}"#, ".points[1][0]"),
            (r#"{"points":[{"x":1,"y":2},[3,4]],"duration_ms":900}"#, ".points[0]"),
            (r#"{"points":[[1,1],[2,2]],"duration_ms":[50,900]}"#, ".duration_ms[0]"),
            (r#"{"points":[[1,1],[2,2]],"duration_ms":[900,10050]}"#, ".duration_ms[1]"),
            (r#"{"points":[[1,1],[2,2]],"duration_ms":[900,600]}"#, ".duration_ms"),
            (r#"{}"#, ".args.points"),
        ];
        for (args, want) in bad {
            let json = format!(
                r#"{{"schema_version":1,"name":"t","steps":[{{"op":"curve_swipe","args":{args}}}]}}"#
            );
            let e = errs(&json);
            assert_eq!(e.len(), 1, "must reject curve_swipe {args}");
            assert!(
                e[0].path.contains(want),
                "curve_swipe {args}: expected path containing {want}, got {}",
                e[0]
            );
        }
    }

    #[test]
    fn unknown_instruction_hint_lists_the_v13_ops() {
        let e = errs(r#"{"schema_version":1,"name":"t","steps":[{"op":"teleport"}]}"#);
        assert!(e[0].msg.contains("long_press"), "{}", e[0]);
        assert!(e[0].msg.contains("curve_swipe"), "{}", e[0]);
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
    fn while_condition_only_is_valid() {
        // `while` has only cond+body; no numeric iteration field.
        let v = serde_json::json!({
            "schema_version":1,"name":"t","steps":[
                {"op":"while","args":{"cond":"true"},"body":[]}
            ]
        });
        assert!(parse_step(&v["steps"][0], "steps[0]").is_ok());
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

    // ---- param() + exits collection (M0 unified task template) ----

    /// One param() call site: `key` is `None` when the argument is not a single
    /// string literal (undecidable at lint time => must be an error).
    #[test]
    fn flow_collects_param_uses_with_paths() {
        let flow = Flow::from_json(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"if","args":{"cond":"param('max_rounds') > 0 and param('mode') == 'fast'"},
                  "then":[{"op":"log","args":{"text":"param('deep_key')"}}]},
                 {"op":"wait_until","args":{"cond":"param('ready')","timeout_s":2}},
                 {"op":"loop","args":{"n":"param('rounds')"},"body":[]},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        )
        .expect("flow");

        let uses: std::collections::BTreeSet<String> = flow.param_uses();
        assert_eq!(
            uses,
            std::collections::BTreeSet::from([
                "max_rounds".to_string(),
                "mode".to_string(),
                "deep_key".to_string(),
                "ready".to_string(),
                "rounds".to_string(),
            ])
        );

        let detailed = flow.param_uses_at();
        assert_eq!(detailed.len(), 5, "{detailed:?}");
        let paths: Vec<&str> = detailed.iter().map(|u| u.path.as_str()).collect();
        assert!(
            paths.iter().any(|p| p.contains("steps[0].args.cond")),
            "{paths:?}"
        );
        assert!(
            paths.iter().any(|p| p.contains("steps[0].then[0].args.text")),
            "{paths:?}"
        );
        assert!(
            paths.iter().any(|p| p.contains("steps[1].args.cond")),
            "{paths:?}"
        );
        assert!(
            paths.iter().any(|p| p.contains("steps[2].args.n")),
            "{paths:?}"
        );
        assert!(detailed.iter().all(|u| u.key.is_some()), "{detailed:?}");
    }

    #[test]
    fn flow_param_uses_flags_a_dynamic_key_as_undecidable() {
        let flow = Flow::from_json(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"log","args":{"text":"param('a' + 'b')"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        )
        .expect("flow");
        assert!(
            flow.param_uses().is_empty(),
            "a computed key is not a literal key"
        );
        let detailed = flow.param_uses_at();
        assert_eq!(detailed.len(), 1, "{detailed:?}");
        assert_eq!(detailed[0].key, None);
        assert!(detailed[0].path.contains("steps[0].args.text"));
    }

    #[test]
    fn reachable_exits_collects_nested_finish_exits_with_paths() {
        let flow = Flow::from_json(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"if","args":{"cond":"true"},
                  "then":[{"op":"finish","args":{"exit":"done"}}],
                  "else":[{"op":"finish","args":{"exit":"failed"}}]},
                 {"op":"loop","args":{"n":"2"},"body":[
                    {"op":"finish","args":{"exit":"exhausted"}}]},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        )
        .expect("flow");
        let exits: std::collections::BTreeSet<String> = flow.reachable_exits();
        assert_eq!(
            exits,
            std::collections::BTreeSet::from([
                "done".to_string(),
                "failed".to_string(),
                "exhausted".to_string(),
            ])
        );
        let uses = flow.finish_uses();
        assert_eq!(uses.len(), 4, "{uses:?}");
        let paths: Vec<&str> = uses.iter().map(|u| u.path.as_str()).collect();
        assert!(
            paths.iter().any(|p| p.contains("steps[0].then[0].args.exit")),
            "{paths:?}"
        );
        assert!(
            paths.iter().any(|p| p.contains("steps[0].else[0].args.exit")),
            "{paths:?}"
        );
        assert!(
            paths.iter().any(|p| p.contains("steps[1].body[0].args.exit")),
            "{paths:?}"
        );
    }

    #[test]
    fn ends_with_finish_requires_every_path_to_terminate() {
        let fin = |json: &str| Flow::from_json(json).expect("flow").ends_with_finish();
        // Last step is finish => terminal.
        assert!(fin(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"log","args":{"text":"'x'"}},
                 {"op":"finish","args":{"exit":"done"}}]}"#
        ));
        // Last step is not finish => some path falls off the end.
        assert!(!fin(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"finish","args":{"exit":"done"}},
                 {"op":"log","args":{"text":"'x'"}}]}"#
        ));
        // A terminal if-then-else where BOTH branches finish is terminal.
        assert!(fin(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"if","args":{"cond":"true"},
                  "then":[{"op":"finish","args":{"exit":"done"}}],
                  "else":[{"op":"finish","args":{"exit":"failed"}}]}]}"#
        ));
        // ... but only when the else-branch exists too.
        assert!(!fin(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"if","args":{"cond":"true"},
                  "then":[{"op":"finish","args":{"exit":"done"}}]}]}"#
        ));
        // An empty flow cannot be terminal.
        assert!(!fin(r#"{"schema_version":1,"name":"t","steps":[]}"#));
    }

    #[test]
    fn exit_name_is_the_inverse_of_parse_exit() {
        for name in [
            "done",
            "no_resource",
            "window_closed",
            "no_target",
            "resource_empty",
            "exhausted",
            "failed",
            "skipped",
        ] {
            let exit = parse_exit(name).expect("known exit");
            assert_eq!(exit_name(exit), name);
        }
    }

    // ---- param() evaluation through EvalContext ----

    /// Context that answers `param()` from a fixed map; other accessors fail
    /// loudly (a test never wants a silent default).
    struct ParamCtx(std::collections::BTreeMap<String, Value>);

    impl EvalContext for ParamCtx {
        fn appear(&mut self, _sel: &str) -> Result<bool, String> {
            Err("appear() unavailable in this test".into())
        }
        fn ocr(&mut self, _sel: &str) -> Result<Option<String>, String> {
            Err("ocr() unavailable in this test".into())
        }
        fn ocr_digit(&mut self, _sel: &str) -> Result<Option<f64>, String> {
            Err("ocr_digit() unavailable in this test".into())
        }
        fn page(&mut self) -> Result<Option<String>, String> {
            Err("page() unavailable in this test".into())
        }
        fn param(&mut self, key: &str) -> Result<Value, String> {
            self.0
                .get(key)
                .cloned()
                .ok_or_else(|| format!("no test param `{key}`"))
        }
    }

    /// Context that does NOT override `param()`: the trait default must be a
    /// loud error (old EvalContext implementors keep compiling, but a flow that
    /// needs `param()` cannot silently see a null).
    struct NoParamCtx;

    impl EvalContext for NoParamCtx {
        fn appear(&mut self, _sel: &str) -> Result<bool, String> {
            Err("appear() unavailable".into())
        }
        fn ocr(&mut self, _sel: &str) -> Result<Option<String>, String> {
            Err("ocr() unavailable".into())
        }
        fn ocr_digit(&mut self, _sel: &str) -> Result<Option<f64>, String> {
            Err("ocr_digit() unavailable".into())
        }
        fn page(&mut self) -> Result<Option<String>, String> {
            Err("page() unavailable".into())
        }
    }

    #[test]
    fn param_call_evaluates_through_the_context() {
        let e = parse_expr("param('max_rounds') * 2 + 1").expect("parse");
        let mut ctx = ParamCtx(std::collections::BTreeMap::from([(
            "max_rounds".to_string(),
            Value::Num(3.0),
        )]));
        assert_eq!(eval(&e, &mut ctx).expect("eval"), Value::Num(7.0));

        let e = parse_expr("param('mode') == 'fast'").expect("parse");
        let mut ctx = ParamCtx(std::collections::BTreeMap::from([(
            "mode".to_string(),
            Value::Str("fast".to_string()),
        )]));
        assert_eq!(eval(&e, &mut ctx).expect("eval"), Value::Bool(true));
    }

    #[test]
    fn param_without_context_support_or_value_is_a_loud_error() {
        let e = parse_expr("param('missing')").expect("parse");
        let err = eval(&e, &mut ParamCtx(Default::default())).expect_err("no value");
        assert!(err.contains("missing"), "must name the key: {err}");

        let e = parse_expr("param('missing')").expect("parse");
        let err = eval(&e, &mut NoParamCtx).expect_err("no support");
        assert!(
            err.contains("param") && err.contains("missing"),
            "trait default must name param+key: {err}"
        );
    }

    #[test]
    fn param_requires_exactly_one_string_argument() {
        let mut ctx = ParamCtx(Default::default());
        let e = parse_expr("param('a', 'b')").expect("parse");
        let err = eval(&e, &mut ctx).expect_err("arity");
        assert!(err.contains("exactly one"), "{err}");

        let e = parse_expr("param(1)").expect("parse");
        let err = eval(&e, &mut ctx).expect_err("type");
        assert!(err.contains("string"), "{err}");
    }

    // ---- call_task params/inherit + with_params scope (Runner Studio §C.4) ----

    #[test]
    fn call_task_parses_params_inherit_and_keeps_the_legacy_shape() {
        let step = parse_step(
            &serde_json::json!({
                "op":"call_task","args":{
                    "task":"任务1",
                    "params":{"触发文字":"param('识别关键词')"},
                    "inherit":"none"
                }
            }),
            "steps[0]",
        )
        .expect("parse");
        match step {
            Step::CallTask {
                task,
                params,
                inherit,
            } => {
                assert_eq!(task, "任务1");
                assert_eq!(params.len(), 1);
                assert_eq!(
                    params["触发文字"],
                    parse_expr("param('识别关键词')").expect("expr")
                );
                assert_eq!(inherit, CallInherit::None);
            }
            other => panic!("expected CallTask, got {other:?}"),
        }

        // Old shape `{"op":"call_task","args":{"task":"X"}}` is unchanged:
        // no mapping, inherit defaults to "all", unknown args stay ignored.
        let legacy = parse_step(
            &serde_json::json!({"op":"call_task","args":{"task":"任务1","mode":"run"}}),
            "steps[0]",
        )
        .expect("legacy parse");
        match legacy {
            Step::CallTask {
                params, inherit, ..
            } => {
                assert!(params.is_empty(), "no params in the legacy shape");
                assert_eq!(inherit, CallInherit::All);
            }
            other => panic!("expected CallTask, got {other:?}"),
        }

        // Explicit "all" is the documented default and parses the same.
        let explicit = parse_step(
            &serde_json::json!({
                "op":"call_task","args":{"task":"X","params":{},"inherit":"all"}
            }),
            "steps[0]",
        )
        .expect("explicit all");
        assert!(matches!(
            explicit,
            Step::CallTask {
                inherit: CallInherit::All,
                ..
            }
        ));
    }

    #[test]
    fn call_task_rejects_bad_params_and_inherit_with_exact_paths() {
        let bad_params = parse_step(
            &serde_json::json!({"op":"call_task","args":{"task":"X","params":["a"]}}),
            "steps[0]",
        )
        .expect_err("params must be an object");
        assert_eq!(bad_params.path, "steps[0].args.params");
        assert!(bad_params.msg.contains("object"), "{bad_params}");

        let bad_value = parse_step(
            &serde_json::json!({
                "op":"call_task","args":{"task":"X","params":{"child":3}}
            }),
            "steps[0]",
        )
        .expect_err("values must be expression strings");
        assert_eq!(bad_value.path, "steps[0].args.params.child");
        assert!(bad_value.msg.contains("expression string"), "{bad_value}");

        let bad_expr = parse_step(
            &serde_json::json!({
                "op":"call_task","args":{"task":"X","params":{"child":"param("}}
            }),
            "steps[0]",
        )
        .expect_err("expression must parse");
        assert_eq!(bad_expr.path, "steps[0].args.params.child");

        for bad in ["sometimes", ""] {
            let err = parse_step(
                &serde_json::json!({
                    "op":"call_task","args":{"task":"X","inherit":bad}
                }),
                "steps[0]",
            )
            .expect_err("invalid inherit");
            assert_eq!(err.path, "steps[0].args.inherit");
            assert!(err.msg.contains("\"all\" or \"none\""), "{err}");
        }

        let err = parse_step(
            &serde_json::json!({"op":"call_task","args":{"task":"X","inherit":1}}),
            "steps[0]",
        )
        .expect_err("inherit must be a string");
        assert_eq!(err.path, "steps[0].args.inherit");
    }

    #[test]
    fn with_params_parses_the_scope_and_traverses_its_body() {
        let flow = Flow::from_json(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"with_params","args":{
                   "task":"子任务",
                   "params":{"child":"param('parent') + appear('弹窗按钮')"},
                   "defaults":{"rounds":3,"flag":true,"label":"x","empty":null},
                   "block":["parent_only","child"],
                   "body":[
                     {"op":"log","args":{"text":"param('child')"}},
                     {"op":"if","args":{"cond":"true"},
                      "then":[{"op":"finish","args":{"exit":"done"}}]}
                   ]
                 }},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        )
        .expect("flow");

        let Step::WithParams {
            label,
            params,
            defaults,
            block,
            body,
        } = &flow.steps[0]
        else {
            panic!("expected with_params, got {:?}", flow.steps[0]);
        };
        assert_eq!(label.as_deref(), Some("子任务"));
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].0, "child");
        assert_eq!(defaults["rounds"], Value::Num(3.0));
        assert_eq!(defaults["flag"], Value::Bool(true));
        assert_eq!(defaults["label"], Value::Str("x".to_string()));
        assert_eq!(defaults["empty"], Value::Null);
        assert_eq!(block, &vec!["parent_only".to_string(), "child".to_string()]);
        assert_eq!(body.len(), 2);

        // `param()` uses: the mapping expression + the body, in caller-scope order.
        let uses = flow.param_uses_at();
        let paths: Vec<&str> = uses.iter().map(|u| u.path.as_str()).collect();
        assert!(
            paths.contains(&"steps[0].args.params.child"),
            "{paths:?}"
        );
        assert!(
            paths.contains(&"steps[0].args.body[0].args.text"),
            "{paths:?}"
        );
        let keys: Vec<Option<&str>> = uses.iter().map(|u| u.key.as_deref()).collect();
        assert!(keys.contains(&Some("parent")), "{keys:?}");
        assert!(keys.contains(&Some("child")), "{keys:?}");

        // Asset + finish traversals must descend into args.body.
        assert!(flow.referenced_assets().contains("弹窗按钮"));
        let finishes = flow.finish_uses();
        assert_eq!(finishes.len(), 2, "{finishes:?}");
        assert!(
            finishes.iter().any(|f| f.path
                == "steps[0].args.body[1].then[0].args.exit"),
            "{finishes:?}"
        );
        assert!(
            flow.ends_with_finish(),
            "the trailing top-level finish still terminates the flow"
        );

        // A scope whose body itself ends in finish is terminal.
        let terminal = Flow::from_json(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"with_params","args":{"body":[
                   {"op":"finish","args":{"exit":"done"}}]}}]}"#,
        )
        .expect("flow");
        assert!(terminal.ends_with_finish());
        let Step::WithParams {
            label, params, block, ..
        } = &terminal.steps[0]
        else {
            panic!("expected with_params");
        };
        assert!(label.is_none(), "label is optional");
        assert!(params.is_empty(), "params are optional");
        assert!(block.is_empty(), "block is optional");
    }

    #[test]
    fn with_params_is_in_the_unknown_instruction_hint() {
        let e = errs(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"nope","args":{}}]}"#,
        );
        assert!(e[0].msg.contains("with_params"), "{}", e[0]);
    }

    #[test]
    fn call_task_uses_reports_nested_calls_with_parent_keys() {
        let flow = Flow::from_json(
            r#"{"schema_version":1,"name":"t","steps":[
                 {"op":"if","args":{"cond":"true"},
                  "then":[{"op":"call_task","args":{
                    "task":"任务1",
                    "params":{"a":"param('pa')"},
                    "inherit":"none"}}]},
                 {"op":"with_params","args":{"body":[
                   {"op":"call_task","args":{
                     "task":"任务2",
                     "params":{"b":"'constant'","c":"param('pc')","d":"param('p' + 'c')"}}}
                 ]}},
                 {"op":"loop","args":{"n":"1"},"body":[
                   {"op":"call_task","args":{"task":"任务3"}}]},
                 {"op":"finish","args":{"exit":"done"}}]}"#,
        )
        .expect("flow");

        let uses = flow.call_task_uses();
        assert_eq!(uses.len(), 3, "{uses:?}");

        let by_task = |task: &str| {
            uses.iter()
                .find(|u| u.task == task)
                .unwrap_or_else(|| panic!("no use for {task}"))
        };
        let a = by_task("任务1");
        assert_eq!(a.inherit, CallInherit::None);
        assert_eq!(a.params["a"].as_deref(), Some("pa"));
        assert_eq!(a.path, "steps[0].then[0].args");
        let b = by_task("任务2");
        assert_eq!(b.inherit, CallInherit::All);
        assert_eq!(b.params["b"], None, "constant mapping has no parent key");
        assert_eq!(b.params["c"].as_deref(), Some("pc"));
        assert_eq!(b.params["d"], None, "computed param key is not literal");
        assert!(
            b.computed_params.contains("d"),
            "computed param() mapping must be flagged for T7: {b:?}"
        );
        assert!(!b.computed_params.contains("b"), "constant is allowed");
        assert!(!b.computed_params.contains("c"));
        assert_eq!(b.path, "steps[1].args.body[0].args");
        let c = by_task("任务3");
        assert!(c.params.is_empty());
        assert_eq!(c.path, "steps[2].body[0].args");

        // T3 is unchanged: call_task mappings are T7's job, not param_uses'.
        assert!(
            flow.param_uses().is_empty(),
            "call_task must stay a leaf for T3"
        );
    }
}
