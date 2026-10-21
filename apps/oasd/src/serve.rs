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
            "oasd serve: http://{addr}/  (ui dir: {ui_dir}, tasks dir: {})",
            self.tasks_dir.display()
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
            ("POST", "/task/save_flow") => self.handle_save_flow(&mut out, &body),
            ("GET", "/task/list") => self.handle_task_list(&mut out),
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
            rev: None,
        };
        match bundle.save_asset(decl, Some(&frame)) {
            Ok(item) => {
                let body = serde_json::json!({ "ok": true, "task": task, "item": item });
                self.send_json(out, 200, &body.to_string())
            }
            Err(e) => self.send_errors(out, 400, vec![e]),
        }
    }

    /// GET /task/assets?task=任务1 -> assets.json summary.
    fn handle_task_assets(&self, out: &mut TcpStream, raw_path: &str) -> std::io::Result<()> {
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
