//! Minimal HTTP surface for the editor MVP (P1-d step 1).
//!
//! Deliberately dependency-free (std::net only): the UI is plain static HTML/JS,
//! so the exact same files can later be wrapped by Tauri (desktop) or a WebView
//! shell (Android) without changes.
//!
//! Routes:
//!   GET  /                -> ui/index.html
//!   GET  /app.js          -> ui/app.js
//!   GET  /frame.png       -> last captured frame (404 before the first capture)
//!   POST /capture         -> capture a fresh frame, returns JSON {width,height}
//!   GET  /state           -> JSON {captured: bool, width, height, device}

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

use oas_adapter_pc::Adb;
use oas_codec::encode_png;
use oas_types::{Frame, ScreenSource};

struct State {
    frame: Option<Frame>,
    png: Option<Vec<u8>>,
    device: String,
    error: Option<String>,
}

pub struct Server {
    adb: Adb,
    state: Arc<Mutex<State>>,
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
        }
    }

    /// Serve forever on 127.0.0.1:`port`. Returns only on bind failure.
    pub fn run(self, port: u16, ui_dir: &str) -> Result<(), String> {
        let addr = format!("127.0.0.1:{port}");
        let listener = TcpListener::bind(&addr).map_err(|e| format!("bind {addr}: {e}"))?;
        println!("oasd serve: http://{addr}/  (ui dir: {ui_dir})");

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
        let mut body = Vec::new();
        if content_length > 0 {
            body.resize(content_length, 0);
            let _ = reader.read_exact(&mut body);
        }
        let _ = body;

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
            ("POST", "/ocr") => self.send_json(
                &mut out,
                501,
                r#"{"error":"OCR route is wired in the next step (P1-d2)"}"#,
            ),
            _ => self.send_json(&mut out, 404, r#"{\"error\":\"not found\"}"#), 
        }
    }

    fn send_json(&self, out: &mut TcpStream, code: u16, body: &str) -> std::io::Result<()> {
        let head = format!(
            "HTTP/1.1 {code} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
            body.as_bytes().len()
        );
        out.write_all(head.as_bytes())?;
        out.write_all(body.as_bytes())?;
        out.flush()
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
            "HTTP/1.1 {code} OK\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
            bytes.len()
        );
        out.write_all(head.as_bytes())?;
        out.write_all(bytes)?;
        out.flush()
    }
}

/// Read the whole body for POST requests with Content-Length.
#[allow(dead_code)]
fn read_body(reader: &mut impl Read, len: usize) -> Vec<u8> {
    let mut buf = vec![0u8; len];
    let _ = reader.read_exact(&mut buf);
    buf
}
