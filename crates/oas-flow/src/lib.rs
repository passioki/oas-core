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

#[derive(Debug, Clone, PartialEq)]
pub struct Flow {
    pub schema_version: u32,
    pub name: String,
    pub steps: Vec<Step>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    /// Structured branch (nested blocks; there is no goto).
    If {
        cond: Expr,
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
    },
    Ocr {
        sel: String,
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

fn parse_steps(v: &serde_json::Value, path: &str) -> Result<Vec<Step>, FlowError> {
    let arr = v
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
        }),
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
                 wait/wait_until/appear/click/ocr/log/finish)"
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
        Ok(Self {
            schema_version,
            name,
            steps,
        })
    }

    /// Collect every asset id referenced by any expression or selector.
    pub fn referenced_assets(&self) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        collect_assets_steps(&self.steps, &mut out);
        out
    }

    /// Reject references to assets that the task does not declare.
    ///
    /// This is the "save-time error" rule: a typo in an asset id must never
    /// survive until the run that needs it.
    pub fn validate_assets(&self, known: &BTreeSet<String>) -> Result<(), Vec<FlowError>> {
        let mut errs = Vec::new();
        for a in self.referenced_assets() {
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
            Step::Appear { sel } | Step::Click { sel } | Step::Ocr { sel } => {
                out.insert(sel.clone());
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
}
