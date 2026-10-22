//! Minimal HTTP surface for the editor MVP (P1-d step 1) + task-bundle save
//! routes (N3).
//!
//! Deliberately std::net only: the UI is plain static HTML/JS, so the exact same
//! files can later be wrapped by Tauri (desktop) or a WebView shell (Android)
//! without changes. JSON is handled with `serde_json` (the only new dependency).
//!
//! Routes:
//!   GET  /                -> ui/index.html
//!   GET  /app.js          -> ui/app.js
//!   GET  /frame.png       -> last captured frame (404 before the first capture)
//!   POST /capture         -> capture a fresh frame, returns JSON {width,height}
//!   GET  /state           -> JSON {captured: bool, width, height, device}
//!   GET  /ocr?x&y&w&h     -> single-line OCR
//!   GET  /lines?x&y&w&h   -> multi-line det+rec
//!   POST /task/save_asset -> crop the LAST captured frame -> task bundle
//!   GET  /task/assets?task=Name -> assets.json summary
//!   POST /task/save_flow  -> validate + save flow.json (asset refs checked)
//!   GET  /task/list       -> task bundle names under --tasks-dir
//!   GET  /runs            -> click-run JSONL files under --runs-dir (§21 CD1/CD4)
//!   GET  /runs/clicks?file=<name> -> one run's CD2 click rows (§21 CD2/CD3)
//!
//! Validation failures answer `400` with a structured `{"ok":false,"errors":[...]}`.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use oas_adapter_pc::Adb;
use oas_codec::encode_png;
use oas_types::{Frame, ScreenSource};

/// Upper bound for a POST body (flow.json is small; never buffer more).
const MAX_BODY_BYTES: usize = 4 * 1024 * 1024;

struct State {
    frame: Option<Frame>,
    png: Option<Vec<u8>>,
    device: String,
    error: Option<String>,
}

pub struct Server {
    adb: Adb,
    state: Arc<Mutex<State>>,
    /// Root for task bundles (`--tasks-dir`); one subdirectory per task.
    tasks_dir: PathBuf,
    /// Root for click-run JSONL files (`--runs-dir`, §21): the data plane of the
    /// always-on click-distribution view.
    runs_dir: PathBuf,
    #[cfg(feature = "ocr")]
    ocr: Option<Arc<Mutex<oas_ocr::onnx::RecEngine>>>,
    #[cfg(feature = "ocr")]
    det: Option<Arc<Mutex<oas_ocr::det_engine::DetEngine>>>,
}

impl Server {
    pub fn new(adb: Adb) -> Self {
        Self {
            adb,
            state: Arc::new(Mutex::new(State {
                frame: None,
                png: None,
                device: String::new(),
                error: None,
            })),
            tasks_dir: PathBuf::from("tasks"),
            runs_dir: PathBuf::from("runs"),
            #[cfg(feature = "ocr")]
            ocr: None,
            #[cfg(feature = "ocr")]
            det: None,
        }
    }

    /// Set the task-bundle root (default `tasks`).
    pub fn with_tasks_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.tasks_dir = dir.into();
        self
    }

    /// Set the click-run JSONL root (default `runs`; produced by
    /// `oasd run --clicks-out runs/<name>.jsonl`).
    pub fn with_runs_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.runs_dir = dir.into();
        self
    }

    /// Load the PP-OCRv6 rec model; returns the charset size.
    #[cfg(feature = "ocr")]
    pub fn enable_ocr(&mut self, model_path: &str) -> Result<usize, String> {
        let engine = oas_ocr::onnx::RecEngine::from_file(model_path)?;
        let n = engine.charset().len();
        self.ocr = Some(Arc::new(Mutex::new(engine)));
        Ok(n)
    }

    /// Serve forever on 127.0.0.1:`port`. Returns only on bind failure.
    pub fn run(self, port: u16, ui_dir: &str) -> Result<(), String> {
        let addr = format!("127.0.0.1:{port}");
        let listener = TcpListener::bind(&addr).map_err(|e| format!("bind {addr}: {e}"))?;
        println!(
            "oasd serve: http://{addr}/  (ui dir: {ui_dir}, tasks dir: {}, runs dir: {})",
            self.tasks_dir.display(),
            self.runs_dir.display()
        );

        // Capture the first frame eagerly so the UI has something to show.
        self.do_capture();

        for stream in listener.incoming() {
            match stream {
                Ok(s) => {
                    if let Err(e) = self.handle(s, ui_dir) {
                        eprintln!("request error: {e}");
                    }
                }
                Err(e) => eprintln!("accept error: {e}"),
            }
        }
        Ok(())
    }

    fn do_capture(&self) {
        let devices = match self.adb.devices() {
            Ok(d) if !d.is_empty() => d,
            Ok(_) => {
                self.set_error("no adb device attached");
                return;
            }
            Err(e) => {
                self.set_error(&format!("adb devices failed: {e}"));
                return;
            }
        };
        let serial = devices[0].serial.clone();
        let mut screen = self.adb.screen(&serial);
        match screen.grab() {
            Ok(frame) => match encode_png(&frame) {
                Ok(png) => {
                    let mut st = self.state.lock().unwrap();
                    st.png = Some(png);
                    st.device = serial;
                    st.error = None;
                    st.frame = Some(frame);
                }
                Err(e) => self.set_error(&format!("encode png: {e}")),
            },
            Err(e) => self.set_error(&format!("capture failed: {e}")),
        }
    }

    fn set_error(&self, msg: &str) {
        eprintln!("capture error: {msg}");
        let mut st = self.state.lock().unwrap();
        st.error = Some(msg.to_string());
    }

    fn handle(&self, stream: TcpStream, ui_dir: &str) -> std::io::Result<()> {
        let mut reader = BufReader::new(stream.try_clone()?);
        let mut request_line = String::new();
        reader.read_line(&mut request_line)?;
        if request_line.trim().is_empty() {
            return Ok(());
        }
        // Drain headers, remembering Content-Length so the request body can be
        // consumed. Not draining it makes the OS send an RST on close, which
        // shows up in clients as a reset connection.
        let mut content_length = 0usize;
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line)? == 0 || line == "\r\n" || line == "\n" {
                break;
            }
            let lower = line.to_ascii_lowercase();
            if let Some(rest) = lower.strip_prefix("content-length:") {
                content_length = rest.trim().parse().unwrap_or(0);
            }
        }
        // Reject oversized bodies before buffering them.
        if content_length > MAX_BODY_BYTES {
            let mut out = stream;
            return self.send_errors(
                &mut out,
                413,
                vec![format!(
                    "request body too large: {content_length} bytes (max {MAX_BODY_BYTES})"
                )],
            );
        }
        let mut body = Vec::new();
        if content_length > 0 {
            body.resize(content_length, 0);
            let _ = reader.read_exact(&mut body);
        }

        let mut parts = request_line.split_whitespace();
        let method = parts.next().unwrap_or("");
        let path = parts.next().unwrap_or("/").to_string();
        let path_only = path.split('?').next().unwrap_or("/").to_string();

        let mut out = stream;
        match (method, path_only.as_str()) {
            ("GET", "/") => self.send_file(&mut out, &format!("{ui_dir}/index.html"), "text/html; charset=utf-8"),
            ("GET", "/app.js") => self.send_file(&mut out, &format!("{ui_dir}/app.js"), "application/javascript; charset=utf-8"),
            ("GET", "/style.css") => self.send_file(&mut out, &format!("{ui_dir}/style.css"), "text/css; charset=utf-8"),
            ("GET", "/flow_model.js") => self.send_file(&mut out, &format!("{ui_dir}/flow_model.js"), "application/javascript; charset=utf-8"),
            ("GET", "/frame.png") => {
                let png = self.state.lock().unwrap().png.clone();
                match png {
                    Some(bytes) => self.send_bytes(&mut out, 200, "image/png", &bytes),
                    None => self.send_json(&mut out, 404, r#"{"error":"no frame captured yet"}"#),
                }
            }
            ("GET", "/state") => {
                let st = self.state.lock().unwrap();
                let (w, h) = st
                    .frame
                    .as_ref()
                    .map(|f| (f.width, f.height))
                    .unwrap_or((0, 0));
                let body = format!(
                    r#"{{"captured":{},"width":{},"height":{},"device":{:?},"error":{:?}}}"#,
                    st.png.is_some(),
                    w,
                    h,
                    st.device,
                    st.error.clone().unwrap_or_default()
                );
                drop(st);
                self.send_json(&mut out, 200, &body)
            }
            ("POST", "/capture") => {
                self.do_capture();
                let st = self.state.lock().unwrap();
                let (w, h) = st
                    .frame
                    .as_ref()
                    .map(|f| (f.width, f.height))
                    .unwrap_or((0, 0));
                let ok = st.png.is_some();
                let err = st.error.clone().unwrap_or_default();
                drop(st);
                let body = format!(
                    r#"{{"ok":{},"width":{},"height":{},"error":{:?}}}"#,
                    ok, w, h, err
                );
                self.send_json(&mut out, 200, &body)
            }
            ("GET", "/ocr") => self.handle_ocr(&mut out, &path),
            ("GET", "/lines") => self.handle_lines(&mut out, &path),
            // --- N3: task bundle save routes ---
            ("POST", "/task/save_asset") => self.handle_save_asset(&mut out, &body),
            ("GET", "/task/assets") => self.handle_task_assets(&mut out, &path),
            ("GET", "/task/flow") => self.handle_task_flow(&mut out, &path),
            ("POST", "/task/save_flow") => self.handle_save_flow(&mut out, &body),
            ("GET", "/task/list") => self.handle_task_list(&mut out),
            ("GET", "/stats") => self.handle_stats(&mut out),
            // --- N4: click-distribution data plane (§21 CD1~CD4) ---
            ("GET", "/runs") => self.handle_runs(&mut out),
            ("GET", "/runs/clicks") => self.handle_run_clicks(&mut out, &path),
            ("POST", other) => self.send_errors(
                &mut out,
                404,
                vec![format!("no such POST route: {other}")],
            ),
            _ => self.send_json(&mut out, 404, r#"{"error":"not found"}"#),
        }
    }

    #[allow(unused_variables)]
    fn handle_ocr(&self, out: &mut TcpStream, raw_path: &str) -> std::io::Result<()> {
        let q = parse_query(raw_path);
        let mut x = 0i32;
        let mut y = 0i32;
        let mut w = 0i32;
        let mut h = 0i32;
        for (k, v) in &q {
            let val: i32 = v.parse().unwrap_or(0);
            match k.as_str() {
                "x" => x = val,
                "y" => y = val,
                "w" => w = val,
                "h" => h = val,
                _ => {}
            }
        }
        if w <= 0 || h <= 0 {
            return self.send_json(out, 400, r#"{"error":"invalid roi"}"#);
        }
        // `x`/`y` are only consumed by the OCR branch; the feature-less build
        // answers 501 so keep the assignments from warning there.
        let _ = (x, y);

        #[cfg(not(feature = "ocr"))]
        {
            self.send_json(
                out,
                501,
                r#"{"error":"this build has no OCR (rebuild with --features ocr)"}"#,
            )
        }

        #[cfg(feature = "ocr")]
        {
            let engine = match self.ocr.as_ref() {
                Some(e) => e.clone(),
                None => {
                    return self.send_json(
                        out,
                        501,
                        r#"{"error":"no OCR model loaded (start with --model <path>)"}"#,
                    )
                }
            };
            let frame = {
                let st = self.state.lock().unwrap();
                match st.frame.as_ref() {
                    Some(f) => oas_types::Frame::new(f.width, f.height, f.rgba.clone()),
                    None => return self.send_json(out, 400, r#"{"error":"no frame yet"}"#),
                }
            };
            let roi = oas_types::Rect { x, y, w, h };
            let crop = match oas_ocr::RgbCrop::from_rgba(frame.width, frame.height, &frame.rgba, roi) {
                Ok(c) => c,
                Err(e) => return self.send_json(out, 400, &format!(r#"{{"error":{:?}}}"#, e)),
            };
            let t0 = std::time::Instant::now();
            let mut eng = engine.lock().unwrap();
            match eng.recognize(&crop) {
                Ok((text, score)) => {
                    let ms = t0.elapsed().as_secs_f64() * 1000.0;
                    let body = format!(
                        r#"{{"text":{},"score":{},"elapsed_ms":{},"charset":{}}}"#,
                        json_escape(&text),
                        score,
                        ms,
                        eng.charset().len()
                    );
                    drop(eng);
                    self.send_json(out, 200, &body)
                }
                Err(e) => self.send_json(out, 500, &format!(r#"{{"error":{:?}}}"#, e)),
            }
        }
    }

    fn send_json(&self, out: &mut TcpStream, code: u16, body: &str) -> std::io::Result<()> {
        let head = format!(
            "HTTP/1.1 {code} {}\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
            reason_phrase(code),
            body.as_bytes().len()
        );
        out.write_all(head.as_bytes())?;
        out.write_all(body.as_bytes())?;
        out.flush()
    }

    /// Structured validation failure: `{"ok":false,"errors":[...]}` + 4xx/5xx.
    fn send_errors(
        &self,
        out: &mut TcpStream,
        code: u16,
        errors: Vec<String>,
    ) -> std::io::Result<()> {
        let body = serde_json::json!({ "ok": false, "errors": errors });
        self.send_json(out, code, &body.to_string())
    }

    fn send_file(&self, out: &mut TcpStream, path: &str, ctype: &str) -> std::io::Result<()> {
        match std::fs::read(path) {
            Ok(bytes) => self.send_bytes(out, 200, ctype, &bytes),
            Err(e) => {
                let body = format!(
                    r#"{{"error":"cannot read {}: {}"}}"#,
                    path.replace('\\', "/"),
                    e
                );
                self.send_json(out, 500, &body)
            }
        }
    }

    fn send_bytes(
        &self,
        out: &mut TcpStream,
        code: u16,
        ctype: &str,
        bytes: &[u8],
    ) -> std::io::Result<()> {
        let head = format!(
            "HTTP/1.1 {code} {}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
            reason_phrase(code),
            bytes.len()
        );
        out.write_all(head.as_bytes())?;
        out.write_all(bytes)?;
        out.flush()
    }
}

/// HTTP reason phrase (the old code always sent "OK", even on 400/500).
fn reason_phrase(code: u16) -> &'static str {
    match code {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        413 => "Payload Too Large",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        _ => "OK",
    }
}

/// Parse a raw request target's query string, percent-decoding both keys and
/// values (task names may be Chinese; `URLSearchParams` sends `%E4%BB%BB…`).
fn parse_query(raw_path: &str) -> Vec<(String, String)> {
    let q = raw_path.split_once('?').map(|(_, q)| q).unwrap_or("");
    q.split('&')
        .filter(|kv| !kv.is_empty())
        .filter_map(|kv| kv.split_once('='))
        .map(|(k, v)| (percent_decode(k), percent_decode(v)))
        .collect()
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                let hex = |b: u8| -> Option<u8> {
                    match b {
                        b'0'..=b'9' => Some(b - b'0'),
                        b'a'..=b'f' => Some(b - b'a' + 10),
                        b'A'..=b'F' => Some(b - b'A' + 10),
                        _ => None,
                    }
                };
                match (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                    (Some(hi), Some(lo)) => {
                        out.push((hi << 4) | lo);
                        i += 3;
                    }
                    _ => {
                        out.push(bytes[i]);
                        i += 1;
                    }
                }
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Minimal JSON string escaping for the legacy hand-built response bodies
/// (only the OCR routes use it, so it is dead code in a feature-less build).
#[allow(dead_code)]
pub fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
// --- P1-c2: multi-line detection route (additive; keeps /ocr single-line) ---

impl Server {
    /// Load the detector; returns Ok(()) when ready.
    #[cfg(feature = "ocr")]
    pub fn enable_det(&mut self, model_path: &str) -> Result<(), String> {
        let det = oas_ocr::det_engine::DetEngine::from_file(model_path)?;
        self.det = Some(Arc::new(Mutex::new(det)));
        Ok(())
    }

    /// GET /lines?x=&y=&w=&h=[&vertical=1]  ->  [{box:[x,y,w,h], text, score}, ...]
    #[allow(unused_variables)]
    pub fn handle_lines(&self, out: &mut TcpStream, raw_path: &str) -> std::io::Result<()> {
        #[cfg(not(feature = "ocr"))]
        {
            return self.send_json(
                out,
                501,
                r#"{"error":"this build has no OCR (rebuild with --features ocr)"}"#,
            );
        }
        #[cfg(feature = "ocr")]
        {
            let q = parse_query(raw_path);
            let (mut x, mut y, mut w, mut h) = (0i32, 0i32, 0i32, 0i32);
            let mut vertical_only = false;
            for (k, v) in &q {
                match k.as_str() {
                    "x" => x = v.parse().unwrap_or(0),
                    "y" => y = v.parse().unwrap_or(0),
                    "w" => w = v.parse().unwrap_or(0),
                    "h" => h = v.parse().unwrap_or(0),
                    "vertical" => vertical_only = v == "1",
                    _ => {}
                }
            }
            if w <= 0 || h <= 0 {
                return self.send_json(out, 400, r#"{"error":"invalid roi"}"#);
            }
            let (det, rec) = match (self.det.as_ref(), self.ocr.as_ref()) {
                (Some(d), Some(r)) => (d.clone(), r.clone()),
                _ => {
                    return self.send_json(
                        out,
                        501,
                        r#"{"error":"det/rec model not loaded (start with --model and --det-model)"}"#,
                    )
                }
            };
            let frame = {
                let st = self.state.lock().unwrap();
                match st.frame.as_ref() {
                    Some(f) => oas_types::Frame::new(f.width, f.height, f.rgba.clone()),
                    None => return self.send_json(out, 400, r#"{"error":"no frame yet"}"#),
                }
            };
            let roi = oas_types::Rect { x, y, w, h };
            let crop = match oas_ocr::RgbCrop::from_rgba(frame.width, frame.height, &frame.rgba, roi) {
                Ok(c) => c,
                Err(e) => return self.send_json(out, 400, &format!(r#"{{"error":{:?}}}"#, e)),
            };
            // RGB crop -> RGBA so sub-boxes can be cut with the same helper
            let mut rgba = Vec::with_capacity(crop.data.len() / 3 * 4);
            for px in crop.data.chunks_exact(3) {
                rgba.extend_from_slice(&[px[0], px[1], px[2], 255]);
            }

            let t0 = std::time::Instant::now();
            let boxes = match det.lock().unwrap().detect(&crop) {
                Ok(b) => b,
                Err(e) => return self.send_json(out, 500, &format!(r#"{{"error":{:?}}}"#, e)),
            };
            let mut items: Vec<String> = Vec::new();
            let mut rec_guard = rec.lock().unwrap();
            for b in boxes.iter() {
                let sub = match oas_ocr::RgbCrop::from_rgba(crop.width, crop.height, &rgba, *b) {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                let (mut text, mut score) = match rec_guard.recognize(&sub) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                // vertical / rotated fallback: keep the best orientation
                if vertical_only || score < 0.6 {
                    for cand in [sub.rotate90_cw(), sub.rotate90_ccw()] {
                        if let Ok((t2, s2)) = rec_guard.recognize(&cand) {
                            if s2 > score {
                                text = t2;
                                score = s2;
                            }
                        }
                    }
                }
                if score < 0.5 {
                    continue; // drop noise boxes (e.g. a lone "o" at 0.46)
                }
                items.push(format!(
                    r#"{{"box":[{},{},{},{}],"text":{},"score":{:.4}}}"#,
                    b.x,
                    b.y,
                    b.w,
                    b.h,
                    json_escape(&text),
                    score
                ));
            }
            drop(rec_guard);
            let ms = t0.elapsed().as_secs_f64() * 1000.0;
            let body = format!(
                r#"{{"lines":[{}],"count":{},"elapsed_ms":{:.1}}}"#,
                items.join(","),
                items.len(),
                ms
            );
            self.send_json(out, 200, &body)
        }
    }
}

// ---------------------------------------------------------------------------
// N3: task bundle routes
// ---------------------------------------------------------------------------

impl Server {
    /// POST /task/save_asset
    /// body: {"task":"任务1","id":"分组2","kind":"template","roi":[x,y,w,h],"threshold":0.85}
    ///
    /// Crops the **last captured frame** and persists it into the task bundle.
    /// All validation (id grammar, duplicate, design resolution, flow refs)
    /// happens inside `oas_task::TaskBundle::save_asset`, which writes nothing
    /// when it rejects.
    fn handle_save_asset(&self, out: &mut TcpStream, body: &[u8]) -> std::io::Result<()> {
        let v: serde_json::Value = match serde_json::from_slice(body) {
            Ok(v) => v,
            Err(e) => return self.send_errors(out, 400, vec![format!("invalid JSON body: {e}")]),
        };
        let task = match json_str_field(&v, "task") {
            Ok(s) => s,
            Err(e) => return self.send_errors(out, 400, vec![e]),
        };
        if let Err(e) = oas_task::validate_task_name(&task) {
            return self.send_errors(out, 400, vec![e]);
        }
        let id = match json_str_field(&v, "id") {
            Ok(s) => s,
            Err(e) => return self.send_errors(out, 400, vec![e]),
        };
        let kind = match json_str_field(&v, "kind") {
            Ok(s) => s,
            Err(e) => return self.send_errors(out, 400, vec![e]),
        };
        let roi = match json_roi_field(&v) {
            Ok(r) => r,
            Err(e) => return self.send_errors(out, 400, vec![e]),
        };
        let threshold = match v.get("threshold") {
            None => None,
            Some(x) => match x.as_f64() {
                Some(t) => Some(t as f32),
                None => {
                    return self.send_errors(
                        out,
                        400,
                        vec!["`threshold` must be a number".to_string()],
                    )
                }
            },
        };
        // `replace: true` = overwrite an existing asset id (rev+1) instead of
        // failing with "already exists" (ENGINE-DESIGN §4 rev semantics).
        let replace = match v.get("replace") {
            None => false,
            Some(x) => match x.as_bool() {
                Some(b) => b,
                None => {
                    return self.send_errors(
                        out,
                        400,
                        vec!["`replace` must be a boolean".to_string()],
                    )
                }
            },
        };
        // §22.10: optional polygon (frame coords, ≥3 points) — validated by
        // oas-task (which also derives roi = its bounding box).
        let polygon = match v.get("polygon") {
            None => None,
            Some(x) => {
                let arr = match x.as_array() {
                    Some(a) => a,
                    None => {
                        return self.send_errors(
                            out,
                            400,
                            vec!["`polygon` must be an array of [x,y] integer pairs".to_string()],
                        )
                    }
                };
                let mut pts: Vec<[i32; 2]> = Vec::with_capacity(arr.len());
                for (i, item) in arr.iter().enumerate() {
                    let pair = item.as_array().filter(|a| a.len() == 2);
                    let parsed = pair.and_then(|a| {
                        Some((a[0].as_i64()?, a[1].as_i64()?))
                    });
                    match parsed {
                        Some((x, y))
                            if (i32::MIN as i64..=i32::MAX as i64).contains(&x)
                                && (i32::MIN as i64..=i32::MAX as i64).contains(&y) =>
                        {
                            pts.push([x as i32, y as i32])
                        }
                        _ => {
                            return self.send_errors(
                                out,
                                400,
                                vec![format!(
                                    "`polygon[{i}]` must be an [x,y] pair of integers"
                                )],
                            )
                        }
                    }
                }
                Some(pts)
            }
        };
        let frame = match self.state.lock().unwrap().frame.clone() {
            Some(f) => f,
            None => {
                return self.send_errors(
                    out,
                    400,
                    vec!["no frame captured yet (POST /capture first)".to_string()],
                )
            }
        };
        let bundle = match oas_task::TaskBundle::open_named(&self.tasks_dir, &task) {
            Ok(b) => b,
            Err(e) => return self.send_errors(out, 400, vec![e]),
        };
        let decl = oas_task::AssetDecl {
            id,
            kind,
            file: None,
            roi: Some(roi),
            threshold,
            polygon,
            rev: None,
        };
        match if replace {
            bundle.update_asset(decl, Some(&frame))
        } else {
            bundle.save_asset(decl, Some(&frame))
        } {
            Ok(item) => {
                let body = serde_json::json!({ "ok": true, "task": task, "item": item });
                self.send_json(out, 200, &body.to_string())
            }
            Err(e) => self.send_errors(out, 400, vec![e]),
        }
    }

    /// GET /task/assets?task=任务1 -> assets.json summary.
    /// `GET /stats` — long-run summary (ENGINE §22.19); the editor renders it.
    fn handle_stats(&self, out: &mut TcpStream) -> std::io::Result<()> {
        match crate::stats::collect(&self.runs_dir) {
            Ok(s) => {
                let s = crate::stats::alerts_for(s, 0.10, 0);
                let body = serde_json::json!({"ok": true, "summary": s});
                self.send_json(out, 200, &body.to_string())
            }
            Err(e) => self.send_errors(out, 400, vec![e]),
        }
    }

    /// `GET /task/flow?task=<name>` — the task's flow.json (visual editor input).
    fn handle_task_flow(&self, out: &mut TcpStream, raw_path: &str) -> std::io::Result<()> {
        let q = parse_query(raw_path);
        let task = match q.iter().find(|(k, _)| k == "task").map(|(_, v)| v.clone()) {
            Some(t) => t,
            None => {
                return self.send_errors(out, 400, vec!["missing query param `task`".to_string()])
            }
        };
        if let Err(e) = oas_task::validate_task_name(&task) {
            return self.send_errors(out, 400, vec![e]);
        }
        let path = self.tasks_dir.join(&task).join("flow.json");
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                let flow: serde_json::Value = match serde_json::from_str(&text) {
                    Ok(v) => v,
                    Err(e) => {
                        return self.send_errors(
                            out,
                            400,
                            vec![format!("flow.json is not valid JSON: {e}")],
                        )
                    }
                };
                let body = serde_json::json!({"ok": true, "task": task, "flow": flow});
                self.send_json(out, 200, &body.to_string())
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let body = serde_json::json!({"ok": true, "task": task, "flow": serde_json::Value::Null});
                self.send_json(out, 200, &body.to_string())
            }
            Err(e) => self.send_errors(
                out,
                400,
                vec![format!("read `{}`: {e}", path.display())],
            ),
        }
    }

    fn handle_task_assets(&self, out: &mut TcpStream, raw_path: &str) -> std::io::Result<()> {        let q = parse_query(raw_path);
        let task = match q.iter().find(|(k, _)| k == "task").map(|(_, v)| v.clone()) {
            Some(t) => t,
            None => {
                return self.send_errors(out, 400, vec!["missing query param `task`".to_string()])
            }
        };
        if let Err(e) = oas_task::validate_task_name(&task) {
            return self.send_errors(out, 400, vec![e]);
        }
        let bundle = match oas_task::TaskBundle::open_named(&self.tasks_dir, &task) {
            Ok(b) => b,
            Err(e) => return self.send_errors(out, 400, vec![e]),
        };
        match bundle.load_assets_file() {
            Ok(af) => {
                let count = af.items.len();
                let body = serde_json::json!({
                    "ok": true,
                    "task": task,
                    "design_resolution": af.design_resolution,
                    "count": count,
                    "items": af.items,
                });
                self.send_json(out, 200, &body.to_string())
            }
            Err(e) => self.send_errors(out, 400, vec![e]),
        }
    }

    /// POST /task/save_flow body: {"task":"任务1","flow":{...}}
    ///
    /// The flow is parsed with the DSL's load-time rules AND cross-checked
    /// against `assets.json`; an undeclared asset reference is a 400 and the old
    /// flow.json stays untouched.
    fn handle_save_flow(&self, out: &mut TcpStream, body: &[u8]) -> std::io::Result<()> {
        let v: serde_json::Value = match serde_json::from_slice(body) {
            Ok(v) => v,
            Err(e) => return self.send_errors(out, 400, vec![format!("invalid JSON body: {e}")]),
        };
        let task = match json_str_field(&v, "task") {
            Ok(s) => s,
            Err(e) => return self.send_errors(out, 400, vec![e]),
        };
        if let Err(e) = oas_task::validate_task_name(&task) {
            return self.send_errors(out, 400, vec![e]);
        }
        let flow = match v.get("flow") {
            Some(flow) if flow.is_object() => flow,
            Some(_) => {
                return self.send_errors(
                    out,
                    400,
                    vec!["`flow` must be a JSON object (not a string)".to_string()],
                )
            }
            None => {
                return self.send_errors(out, 400, vec!["missing object field `flow`".to_string()])
            }
        };
        let text = match serde_json::to_string_pretty(flow) {
            Ok(t) => t,
            Err(e) => return self.send_errors(out, 400, vec![format!("serialize flow: {e}")]),
        };
        let bundle = match oas_task::TaskBundle::open_named(&self.tasks_dir, &task) {
            Ok(b) => b,
            Err(e) => return self.send_errors(out, 400, vec![e]),
        };
        match bundle.save_flow(&text) {
            Ok(()) => {
                let body = serde_json::json!({ "ok": true, "task": task });
                self.send_json(out, 200, &body.to_string())
            }
            Err(e) => self.send_errors(out, 400, vec![e]),
        }
    }

    /// GET /task/list -> task bundle names under `--tasks-dir`.
    fn handle_task_list(&self, out: &mut TcpStream) -> std::io::Result<()> {
        match oas_task::list_tasks(&self.tasks_dir) {
            Ok(tasks) => {
                let body = serde_json::json!({
                    "ok": true,
                    "tasks_dir": self.tasks_dir.display().to_string(),
                    "count": tasks.len(),
                    "tasks": tasks,
                });
                self.send_json(out, 200, &body.to_string())
            }
            Err(e) => self.send_errors(out, 500, vec![e]),
        }
    }
}

// ---------------------------------------------------------------------------
// N4: click-distribution data plane (§21 CD1~CD4)
// ---------------------------------------------------------------------------

/// One CD2 click-audit row (`oasd run --clicks-out`): `x`/`y` are the ACTUAL
/// clicked point, `target_*` the intended target, `sel` the asset id (or null
/// for `click_point`) and `seed` the run seed (§21 CD2, §19 CP4, §20 HB6).
#[derive(Debug, Clone, PartialEq)]
struct ClickRow {
    ts_ms: u128,
    x: i32,
    y: i32,
    target_x: i32,
    target_y: i32,
    sel: Option<String>,
    seed: u64,
}

/// Reject a `file=` value that could escape `--runs-dir` (the name comes from a
/// URL query string, i.e. from untrusted input).
fn sanitize_run_file(raw: &str) -> Result<&str, String> {
    if raw.is_empty() {
        return Err(
            "missing query param `file` (example: /runs/clicks?file=s1.clicks.jsonl)".to_string(),
        );
    }
    if raw.contains('/') || raw.contains('\\') {
        return Err(format!(
            "invalid run file name `{raw}`: path separators are not allowed"
        ));
    }
    if raw.contains("..") {
        return Err(format!(
            "invalid run file name `{raw}`: `..` is not allowed"
        ));
    }
    Ok(raw)
}

/// Parse one JSONL line as a CD2 row. `ts_ms`/`x`/`y`/`sel`/`seed` are required;
/// `target_x`/`target_y` are optional because §21's CD2 row does not name them
/// (the runtime adds them for CP4/HB6). A bad value is an error, never a silent 0.
fn parse_click_row(line: &str) -> Result<ClickRow, String> {
    let line = line.trim_end_matches('\r');
    let v: serde_json::Value =
        serde_json::from_str(line).map_err(|e| format!("invalid JSON: {e}"))?;
    let obj = v
        .as_object()
        .ok_or_else(|| "row must be a JSON object".to_string())?;
    let int = |key: &str| -> Result<i64, String> {
        obj.get(key)
            .and_then(|x| x.as_i64())
            .ok_or_else(|| format!("missing integer field `{key}`"))
    };
    let i32_of = |key: &str| -> Result<i32, String> {
        let n = int(key)?;
        i32::try_from(n).map_err(|_| format!("`{key}` = {n} does not fit in i32"))
    };
    let optional_i32 = |key: &str| -> Result<i32, String> {
        match obj.get(key) {
            None | Some(serde_json::Value::Null) => Ok(0),
            Some(_) => i32_of(key),
        }
    };
    let ts_ms = obj
        .get("ts_ms")
        .and_then(|x| x.as_u64())
        .ok_or_else(|| "missing unsigned field `ts_ms`".to_string())?
        as u128;
    let seed = obj
        .get("seed")
        .and_then(|x| x.as_u64())
        .ok_or_else(|| "missing unsigned field `seed`".to_string())?;
    let sel = match obj.get("sel") {
        None | Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::String(s)) => Some(s.clone()),
        Some(other) => return Err(format!("`sel` must be a string or null, got {other}")),
    };
    Ok(ClickRow {
        ts_ms,
        x: i32_of("x")?,
        y: i32_of("y")?,
        target_x: optional_i32("target_x")?,
        target_y: optional_i32("target_y")?,
        sel,
        seed,
    })
}

/// Parse a whole clicks JSONL file: blank lines (incl. CRLF-only) are skipped,
/// everything else must be a CD2 row; failures name the 1-based line.
fn parse_clicks_jsonl(text: &str) -> Result<Vec<ClickRow>, String> {
    let mut rows = Vec::new();
    for (i, line) in text.split('\n').enumerate() {
        let line = line.trim_end_matches('\r');
        if line.trim().is_empty() {
            continue;
        }
        rows.push(parse_click_row(line).map_err(|e| format!("line {}: {e}", i + 1))?);
    }
    Ok(rows)
}

/// CD4: `distinct` = unique ACTUAL click points (small distinct = every run taps
/// the same spot = ban risk).
fn distinct_clicks(rows: &[ClickRow]) -> usize {
    rows.iter()
        .map(|r| (r.x, r.y))
        .collect::<std::collections::HashSet<_>>()
        .len()
}

/// File modification time in epoch ms (0 when unavailable).
fn mtime_ms(entry: &std::fs::DirEntry) -> i64 {
    entry
        .metadata()
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

impl Server {
    /// Structured error for the `/runs*` routes.
    fn send_run_error(&self, out: &mut TcpStream, code: u16, msg: &str) -> std::io::Result<()> {
        let body = serde_json::json!({ "ok": false, "error": msg });
        self.send_json(out, code, &body.to_string())
    }

    /// GET /runs -> `{"ok":true,"runs":[{"file","clicks","distinct","mtime_ms"}]}`
    ///
    /// Only regular `*.jsonl` files directly under `--runs-dir` (no recursion).
    /// A malformed file does not fail the whole listing: it gets an `error` field
    /// instead of stats (the UI can still show the other runs).
    fn handle_runs(&self, out: &mut TcpStream) -> std::io::Result<()> {
        let entries = match std::fs::read_dir(&self.runs_dir) {
            Ok(e) => e,
            // No runs yet is a normal state, not an error.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let body = serde_json::json!({ "ok": true, "runs": [] });
                return self.send_json(out, 200, &body.to_string());
            }
            Err(e) => {
                return self.send_errors(
                    out,
                    500,
                    vec![format!("scan runs dir `{}`: {e}", self.runs_dir.display())],
                )
            }
        };
        let mut found: Vec<(String, i64)> = Vec::new();
        for entry in entries {
            let Ok(entry) = entry else { continue };
            let is_file = entry.file_type().map(|t| t.is_file()).unwrap_or(false);
            if !is_file {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name.ends_with(".jsonl") {
                continue;
            }
            found.push((name, mtime_ms(&entry)));
        }
        // Newest first; name is the deterministic tie-break.
        found.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

        let mut runs: Vec<serde_json::Value> = Vec::new();
        for (name, mtime) in found {
            let path = self.runs_dir.join(&name);
            let item = match std::fs::read_to_string(&path) {
                Ok(text) => match parse_clicks_jsonl(&text) {
                    Ok(rows) => serde_json::json!({
                        "file": name,
                        "clicks": rows.len(),
                        "distinct": distinct_clicks(&rows),
                        "mtime_ms": mtime,
                    }),
                    Err(e) => serde_json::json!({
                        "file": name,
                        "mtime_ms": mtime,
                        "error": e,
                    }),
                },
                Err(e) => serde_json::json!({
                    "file": name,
                    "mtime_ms": mtime,
                    "error": format!("read: {e}"),
                }),
            };
            runs.push(item);
        }
        let body = serde_json::json!({ "ok": true, "runs": runs });
        self.send_json(out, 200, &body.to_string())
    }

    /// GET /runs/clicks?file=<name> ->
    /// `{"ok":true,"file","clicks","distinct","points":[ClickRow...]}`
    ///
    /// 400 for a missing/unsafe name or a malformed row (with the line number),
    /// 404 for a name that is not a file in `--runs-dir`.
    fn handle_run_clicks(&self, out: &mut TcpStream, raw_path: &str) -> std::io::Result<()> {
        let q = parse_query(raw_path);
        let raw = match q.iter().find(|(k, _)| k == "file").map(|(_, v)| v.as_str()) {
            Some(v) => v,
            None => {
                return self.send_run_error(
                    out,
                    400,
                    "missing query param `file` (example: /runs/clicks?file=s1.clicks.jsonl)",
                )
            }
        };
        let name = match sanitize_run_file(raw) {
            Ok(n) => n,
            Err(e) => return self.send_run_error(out, 400, &e),
        };
        let path = self.runs_dir.join(name);
        if !path.is_file() {
            return self.send_run_error(
                out,
                404,
                &format!(
                    "no such run file `{name}` in `{}`",
                    self.runs_dir.display()
                ),
            );
        }
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => {
                return self.send_run_error(out, 500, &format!("read `{}`: {e}", path.display()))
            }
        };
        let rows = match parse_clicks_jsonl(&text) {
            Ok(r) => r,
            Err(e) => return self.send_run_error(out, 400, &format!("{name}: {e}")),
        };
        let points: Vec<serde_json::Value> = rows
            .iter()
            .map(|r| {
                serde_json::json!({
                    "ts_ms": r.ts_ms as u64,
                    "x": r.x,
                    "y": r.y,
                    "target_x": r.target_x,
                    "target_y": r.target_y,
                    "sel": r.sel,
                    "seed": r.seed,
                })
            })
            .collect();
        let body = serde_json::json!({
            "ok": true,
            "file": name,
            "clicks": rows.len(),
            "distinct": distinct_clicks(&rows),
            "points": points,
        });
        self.send_json(out, 200, &body.to_string())
    }
}

fn json_str_field(v: &serde_json::Value, key: &str) -> Result<String, String> {
    v.get(key)
        .and_then(|x| x.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| format!("missing string field `{key}`"))
}

fn json_roi_field(v: &serde_json::Value) -> Result<[i32; 4], String> {
    let arr = v
        .get("roi")
        .and_then(|x| x.as_array())
        .ok_or_else(|| "missing array field `roi` ([x,y,w,h])".to_string())?;
    if arr.len() != 4 {
        return Err(format!(
            "`roi` must have exactly 4 integers [x,y,w,h], got {}",
            arr.len()
        ));
    }
    let mut out = [0i32; 4];
    for (i, item) in arr.iter().enumerate() {
        let n = item
            .as_i64()
            .ok_or_else(|| format!("`roi[{i}]` must be an integer"))?;
        out[i] = i32::try_from(n).map_err(|_| format!("`roi[{i}]` = {n} does not fit in i32"))?;
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// N4 tests: pure helpers + the real routes over a loopback socket
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            static SEQ: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "oasd-serve-test-{}-{tag}-{}",
                std::process::id(),
                SEQ.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).expect("create temp dir");
            Self(path)
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

    fn server_with_runs(dir: &Path) -> Server {
        Server::new(Adb::from_env()).with_runs_dir(dir)
    }

    /// Pin a file's mtime so the "newest first" assertion is deterministic.
    fn set_mtime(path: &Path, ms_since_epoch: u64) {
        let t = std::time::UNIX_EPOCH + std::time::Duration::from_millis(ms_since_epoch);
        let f = std::fs::OpenOptions::new()
            .write(true)
            .open(path)
            .expect("open for mtime");
        f.set_times(std::fs::FileTimes::new().set_modified(t))
            .expect("set mtime");
    }

    /// Drive one GET through the REAL [`Server::handle`] over a loopback socket
    /// pair and return `(status line, body)`.
    fn get_route(server: &Server, target: &str) -> (String, String) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test listener");
        let addr = listener.local_addr().expect("local addr");
        let mut client = TcpStream::connect(addr).expect("connect");
        let (srv, _) = listener.accept().expect("accept");
        client
            .write_all(format!("GET {target} HTTP/1.1\r\nHost: test\r\n\r\n").as_bytes())
            .expect("write request");
        server.handle(srv, "ui").expect("handle request");
        let mut raw = Vec::new();
        client.read_to_end(&mut raw).expect("read response");
        let text = String::from_utf8_lossy(&raw).into_owned();
        let (head, body) = text.split_once("\r\n\r\n").expect("header/body split");
        (
            head.lines().next().unwrap_or_default().to_string(),
            body.to_string(),
        )
    }

    const ROW_A: &str =
        r#"{"ts_ms":1700000000000,"x":10,"y":20,"target_x":11,"target_y":21,"sel":null,"seed":3}"#;
    const ROW_B: &str =
        r#"{"ts_ms":1700000000001,"x":30,"y":40,"target_x":31,"target_y":41,"sel":"I_BRIGHT","seed":3}"#;

    #[test]
    fn run_file_names_are_sanitised() {
        for ok in ["s1.clicks.jsonl", "运行1.clicks.jsonl", "a-b_c.jsonl"] {
            assert_eq!(sanitize_run_file(ok).expect("valid name"), ok);
        }
        for bad in [
            "",
            "../secret.jsonl",
            "a/b.jsonl",
            "a\\b.jsonl",
            "..",
            "..\\x.jsonl",
            "x/../y.jsonl",
        ] {
            let err = sanitize_run_file(bad).expect_err("must reject a traversal name");
            assert!(!err.is_empty(), "rejection must explain why: {bad}");
        }
    }

    #[test]
    fn clicks_jsonl_parses_valid_blank_and_crlf_rows() {
        // CRLF (Windows-written) + blank lines must not break the parser, and the
        // optional §19 target columns are carried through.
        let text = format!("{ROW_A}\r\n\r\n   \r\n{ROW_B}\n");
        let rows = parse_clicks_jsonl(&text).expect("valid JSONL");
        assert_eq!(rows.len(), 2, "blank/CRLF-only lines are skipped: {rows:?}");
        assert_eq!((rows[0].x, rows[0].y), (10, 20));
        assert_eq!(
            (rows[0].target_x, rows[0].target_y),
            (11, 21),
            "target columns must survive parsing"
        );
        assert_eq!(rows[0].sel, None);
        assert_eq!(rows[0].seed, 3);
        assert_eq!(rows[0].ts_ms, 1_700_000_000_000);
        assert_eq!(rows[1].sel.as_deref(), Some("I_BRIGHT"));

        // A §21-minimal row (only ts_ms/x/y/sel/seed) is still a valid CD2 row.
        let minimal = parse_clicks_jsonl(
            r#"{"ts_ms":5,"x":3,"y":4,"sel":null,"seed":9}"#,
        )
        .expect("minimal CD2 row");
        assert_eq!((minimal[0].target_x, minimal[0].target_y), (0, 0));
        assert!(parse_clicks_jsonl("").expect("empty file").is_empty());
    }

    #[test]
    fn malformed_clicks_jsonl_reports_the_line_number() {
        let text = format!("{ROW_A}\nnot json\n");
        let err = parse_clicks_jsonl(&text).expect_err("malformed row must fail");
        assert!(err.contains("line 2"), "{err}");

        let err = parse_clicks_jsonl(r#"{"ts_ms":1,"x":"oops","y":2,"sel":null,"seed":1}"#)
            .expect_err("non-numeric x must fail, not silently become 0");
        assert!(err.contains("line 1"), "{err}");
        assert!(err.contains("x"), "{err}");

        let err = parse_clicks_jsonl(r#"{"ts_ms":1,"x":1,"y":2,"sel":null}"#)
            .expect_err("missing seed must fail");
        assert!(err.contains("seed"), "{err}");
    }

    #[test]
    fn distinct_counts_unique_actual_points() {
        // `target_*` is IDENTICAL on every row on purpose: `distinct` must count
        // the ACTUAL clicked points (CD4), not the intended targets.
        let row = |x, y| ClickRow {
            ts_ms: 0,
            x,
            y,
            target_x: 99,
            target_y: 99,
            sel: None,
            seed: 1,
        };
        assert_eq!(distinct_clicks(&[]), 0);
        assert_eq!(distinct_clicks(&[row(10, 20)]), 1);
        assert_eq!(
            distinct_clicks(&[row(10, 20), row(10, 20), row(11, 20), row(10, 21)]),
            3,
            "distinct is on the ACTUAL (x,y) point"
        );
    }

    #[test]
    fn runs_route_lists_regular_jsonl_files_newest_first_with_errors() {
        let td = TempDir::new("runs-list");
        std::fs::write(td.path().join("good.jsonl"), format!("{ROW_A}\n{ROW_B}\n"))
            .expect("write good");
        std::fs::write(td.path().join("bad.jsonl"), "not json\n").expect("write bad");
        std::fs::write(td.path().join("notes.txt"), ROW_A).expect("write ignored");
        std::fs::create_dir(td.path().join("dir.jsonl")).expect("create dir");
        // Deterministic mtimes: good is NEWER than bad, so mtime-desc must put it
        // first (a plain alphabetical/readdir order would be bad first).
        set_mtime(&td.path().join("good.jsonl"), 1_700_000_100_000);
        set_mtime(&td.path().join("bad.jsonl"), 1_700_000_000_000);
        let server = server_with_runs(td.path());

        let (status, body) = get_route(&server, "/runs");
        assert!(status.contains("200"), "{status} {body}");
        let v: serde_json::Value = serde_json::from_str(&body).expect("JSON body");
        assert_eq!(v["ok"], serde_json::Value::Bool(true));
        let runs = v["runs"].as_array().expect("runs array");
        let files: Vec<&str> = runs
            .iter()
            .map(|r| r["file"].as_str().expect("file name"))
            .collect();
        assert_eq!(
            files,
            ["good.jsonl", "bad.jsonl"],
            "only regular *.jsonl files, newest (mtime desc) first: {files:?}"
        );

        let good = runs
            .iter()
            .find(|r| r["file"] == "good.jsonl")
            .expect("good entry");
        assert_eq!(good["clicks"], 2);
        assert_eq!(good["distinct"], 2);
        assert_eq!(good["mtime_ms"].as_i64(), Some(1_700_000_100_000));

        // A malformed file carries an error INSTEAD of breaking the listing.
        let bad = runs
            .iter()
            .find(|r| r["file"] == "bad.jsonl")
            .expect("bad entry");
        let err = bad["error"].as_str().expect("error field");
        assert!(err.contains("line 1"), "{err}");

        // Newest first (equal mtimes are allowed; the order must not be ascending).
        let mtimes: Vec<i64> = runs
            .iter()
            .map(|r| r["mtime_ms"].as_i64().expect("mtime"))
            .collect();
        assert!(
            mtimes.windows(2).all(|w| w[0] >= w[1]),
            "runs must be mtime-desc: {mtimes:?}"
        );
    }

    #[test]
    fn runs_route_on_a_missing_dir_is_an_empty_list() {
        let td = TempDir::new("runs-none");
        let server = server_with_runs(&td.path().join("does-not-exist"));
        let (status, body) = get_route(&server, "/runs");
        assert!(status.contains("200"), "{status} {body}");
        let v: serde_json::Value = serde_json::from_str(&body).expect("JSON body");
        assert_eq!(v["ok"], serde_json::Value::Bool(true));
        assert_eq!(v["runs"].as_array().map(|a| a.len()), Some(0));
    }

    #[test]
    fn runs_clicks_route_returns_points_and_rejects_bad_names() {
        let td = TempDir::new("runs-clicks");
        std::fs::write(
            td.path().join("s1.clicks.jsonl"),
            format!("{ROW_A}\n{ROW_B}\n"),
        )
        .expect("write run");
        std::fs::write(td.path().join("broken.clicks.jsonl"), format!("{ROW_A}\nnot json\n"))
            .expect("write broken run");
        let server = server_with_runs(td.path());

        let (status, body) = get_route(&server, "/runs/clicks?file=s1.clicks.jsonl");
        assert!(status.contains("200"), "{status} {body}");
        let v: serde_json::Value = serde_json::from_str(&body).expect("JSON body");
        assert_eq!(v["ok"], serde_json::Value::Bool(true));
        assert_eq!(v["file"], "s1.clicks.jsonl");
        assert_eq!(v["clicks"], 2);
        assert_eq!(v["distinct"], 2);
        let points = v["points"].as_array().expect("points array");
        assert_eq!(points.len(), 2);
        for (p, raw) in points.iter().zip([(10, 20, 11, 21), (30, 40, 31, 41)]) {
            assert_eq!(p["x"].as_i64(), Some(raw.0));
            assert_eq!(p["y"].as_i64(), Some(raw.1));
            assert_eq!(p["target_x"].as_i64(), Some(raw.2));
            assert_eq!(p["target_y"].as_i64(), Some(raw.3));
            assert!(p["ts_ms"].as_u64().is_some());
            assert_eq!(p["seed"].as_u64(), Some(3));
        }
        assert_eq!(points[0]["sel"], serde_json::Value::Null);
        assert_eq!(points[1]["sel"], "I_BRIGHT");

        // Malformed line => 400 naming the line.
        let (status, body) = get_route(&server, "/runs/clicks?file=broken.clicks.jsonl");
        assert!(status.contains("400"), "{status} {body}");
        assert!(body.contains("line 2"), "{body}");

        // Empty / traversal names => 400 (rejected, never joined to the dir).
        for bad in [
            "/runs/clicks",
            "/runs/clicks?file=",
            "/runs/clicks?file=..%2Fsecret.jsonl",
            "/runs/clicks?file=a%5Cb.jsonl",
            "/runs/clicks?file=..",
        ] {
            let (status, body) = get_route(&server, bad);
            assert!(status.contains("400"), "{bad} => {status} {body}");
        }
        // Missing file (safe name) => 404.
        let (status, body) = get_route(&server, "/runs/clicks?file=nope.jsonl");
        assert!(status.contains("404"), "{status} {body}");
    }
}
