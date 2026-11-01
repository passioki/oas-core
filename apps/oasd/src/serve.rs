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
//!   GET  /state           -> JSON {captured, width, height, device, ocr, det}
//!   POST /input/tap       -> live-preview passthrough: tap current device
//!   POST /input/swipe     -> live-preview passthrough: swipe current device
//!   GET  /ocr?x&y&w&h     -> single-line OCR
//!   GET  /lines?x&y&w&h   -> multi-line det+rec
//!   POST /task/save_asset -> crop the LAST captured frame -> task bundle
//!   GET  /task/assets?task=Name -> assets.json summary
//!   POST /task/save_flow  -> validate + save flow.json (asset refs checked)
//!   GET  /task/list       -> task bundle names under --tasks-dir
//!   POST /task/create     -> atomically create a new task bundle skeleton (N38/M1)
//!   POST /task/run        -> run a task bundle in the background via `run::execute`
//!   GET  /task/run/status -> status/log tail of the current or last editor run
//!   GET  /runs            -> click-run JSONL files under --runs-dir (§21 CD1/CD4)
//!   GET  /runs/clicks?file=<name> -> one run's CD2 click rows (§21 CD2/CD3)
//!
//! Validation failures answer `400` with a structured `{"ok":false,"errors":[...]}`.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use oas_adapter_pc::Adb;
use oas_codec::encode_png;
use oas_runtime::RunResult;
use oas_types::{Frame, InputSink, Outcome, Point, ScreenSource};

use crate::run::{RunOptions, RunReport};
use crate::schedule::{ResetOptions, SchedulerOptions};

/// Upper bound for a POST body (flow.json is small; never buffer more).
const MAX_BODY_BYTES: usize = 4 * 1024 * 1024;

/// The **executor** behind `POST /task/run` (N38/M1): the default is the real
/// `run::execute`, the editor never gets a second copy of the interpreter.
///
/// It is a function value on purpose: a unit test can inject a fake executor,
/// so the whole run state machine (single-run mutex, status JSON, outcome,
/// error, log tail) is testable without a device or a replay bundle.
pub type RunExecutor =
    dyn Fn(&RunOptions, &mut dyn FnMut(&str)) -> Result<RunReport, String> + Send + Sync;

/// Maximum number of log lines kept in memory for one editor run.
const RUN_LOG_MAX: usize = 2000;
/// How many of those log lines `GET /task/run/status` returns (the UI polls).
const RUN_LOG_TAIL: usize = 200;

/// Status of the current/last editor run (`POST /task/run` / `GET /task/run/status`).
#[derive(Debug, Default)]
struct RunState {
    running: bool,
    task: Option<String>,
    started_ms: Option<u64>,
    finished_ms: Option<u64>,
    /// `run::RunReport::exit_code()`: 0 = Done, 1 = Failed/Exhausted, 2 = setup error.
    exit: Option<u8>,
    /// `Done` / `Failed` / `Exhausted` / `Error` (setup failure).
    outcome: Option<String>,
    error: Option<String>,
    /// `live` / `dry-run` / `replay`.
    mode: Option<String>,
    dry_run: bool,
    replay: Option<String>,
    seed: Option<u64>,
    /// `live` / `replay` (as reported by the executor); None on setup errors.
    frame_source: Option<String>,
    /// Actual clicks of the run (HB6/CD2), when the executor returned a report.
    clicks: Option<u64>,
    /// Cooperative stop token for the current run.
    cancel: Option<Arc<AtomicBool>>,
    /// Live StepEvent buffer from the runtime; status polls drain it.
    step_log: Option<Arc<Mutex<Vec<String>>>>,
    log: Vec<String>,
}

impl RunState {
    /// Move buffered runtime StepEvents into the visible run log.
    fn drain_step_log(&mut self) {
        let Some(buf) = self.step_log.clone() else { return; };
        let lines: Vec<String> = match buf.lock() {
            Ok(mut b) => b.drain(..).collect(),
            Err(_) => Vec::new(),
        };
        for line in lines {
            self.push_log(&line);
        }
    }

    fn push_log(&mut self, line: &str) {
        self.log.push(line.to_string());
        if self.log.len() > RUN_LOG_MAX {
            let excess = self.log.len() - RUN_LOG_MAX;
            self.log.drain(0..excess);
        }
    }
}

struct State {
    /// Analysis frame: the selected gallery frame or the last explicit capture.
    /// OCR/match/asset crop MUST use this, never the live preview frame.
    frame: Option<Frame>,
    png: Option<Vec<u8>>,
    /// Live preview pixels (WebSocket / `/frame.png?live=1`). Kept separate so
    /// selecting a gallery frame cannot be overwritten by a live update.
    live_frame: Option<Frame>,
    live_png: Option<Vec<u8>>,
    device: String,
    error: Option<String>,
    /// §22.27 会话图片列表（旧 annotator「图片」面板）：上传/截帧都进这里，
    /// 选中某张即成为当前帧（/match、/ocr、素材裁剪都用它）。
    frames: Vec<SessionFrame>,
    next_frame_id: u64,
    current_frame_id: Option<u64>,
    /// Two-phase manual capture: preview is held in memory until the user
    /// confirms name/category; only commit writes a frame into the gallery.
    pending_capture: Option<PendingCapture>,
}

struct PendingCapture {
    png: Vec<u8>,
    width: u32,
    height: u32,
}

struct SessionFrame {
    id: u64,
    /// User-editable display name (rename API; id stays stable).
    name: String,
    /// Free-form category/group used by the frame gallery.
    category: String,
    /// v1.2.2: PNG is persisted under `frames_dir`; the gallery only keeps
    /// metadata, the current frame's pixels live in `State.png`.
    file: PathBuf,
    bytes: u64,
    width: u32,
    height: u32,
    source: String,
    ts_ms: u64,
    /// false until the user views/edits this frame; drives the red new-frame badge.
    seen: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FrameMeta {
    id: u64,
    name: String,
    #[serde(default)]
    category: String,
    file: String,
    width: u32,
    height: u32,
    source: String,
    ts_ms: u64,
    #[serde(default = "default_seen")]
    seen: bool,
}

fn default_seen() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FramesManifest {
    #[serde(default = "default_next_frame_id")]
    next_id: u64,
    #[serde(default)]
    current_id: Option<u64>,
    #[serde(default)]
    frames: Vec<FrameMeta>,
}

fn default_next_frame_id() -> u64 {
    1
}

impl State {
    fn push_frame(
        &mut self,
        frames_dir: &Path,
        png: Vec<u8>,
        width: u32,
        height: u32,
        source: &str,
        name: Option<String>,
        category: String,
        seen: bool,
        select: bool,
    ) -> Result<u64, String> {
        let id = self.next_frame_id;
        self.next_frame_id += 1;
        std::fs::create_dir_all(frames_dir)
            .map_err(|e| format!("create frames dir {}: {e}", frames_dir.display()))?;
        let file_name = format!("frame-{id}.png");
        let file = frames_dir.join(&file_name);
        std::fs::write(&file, &png)
            .map_err(|e| format!("write frame {}: {e}", file.display()))?;
        let bytes = png.len() as u64;
        let base = name
            .map(|n| n.trim().to_string())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| format!("帧 {id}"));
        // Name uniqueness: candidate hints are only hints; the capture path
        // auto-suffixes collisions so a batch can never fail on a duplicate.
        let mut unique = base.clone();
        let mut n = 2;
        while self.frames.iter().any(|f| f.name == unique) {
            unique = format!("{base}-{n}");
            n += 1;
        }
        self.frames.push(SessionFrame {
            id,
            name: unique,
            category: category.trim().to_string(),
            file,
            bytes,
            width,
            height,
            source: source.to_string(),
            ts_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
            seen,
        });
        // v1.2.2: no hard 20-frame cap and frames are persisted under
        // `<frames-dir>/frames.json` + `frame-<id>.png`.
        if select {
            self.current_frame_id = Some(id);
        }
        Ok(id)
    }
}

pub struct Server {
    adb: Adb,
    state: Arc<Mutex<State>>,
    /// Root for task bundles (`--tasks-dir`); one subdirectory per task.
    tasks_dir: PathBuf,
    /// Root for click-run JSONL files (`--runs-dir`, §21): the data plane of the
    /// always-on click-distribution view.
    runs_dir: PathBuf,
    /// Root for the global template asset library (`--library-dir`, §22.22).
    library_dir: PathBuf,
    /// v1.2.1 unified global asset store (`--assets-dir`). When initialized it
    /// replaces per-task assets for the editor, builder and runs.
    assets_dir: PathBuf,
    /// v1.2.2 disk-backed frame gallery (`--frames-dir`): `frames.json` +
    /// `frame-<id>.png`. Metadata is always in memory; pixels are lazy.
    frames_dir: PathBuf,
    /// Editor action log (`logs/editor.log`): OCR/asset/task requests are
    /// persisted so a user's reproduction can be diagnosed after the browser
    /// log window is closed (v1.1).
    editor_log: PathBuf,
    /// Shared state of the editor-triggered background run (N38/M1).
    run_state: Arc<Mutex<RunState>>,
    /// Executor used by `POST /task/run`; replaceable in tests.
    run_executor: Arc<RunExecutor>,
    /// Paths of the OCR models loaded by serve; passed to editor-triggered
    /// runs so `appear(ocr_asset)` / `text_visible()` work there too.
    ocr_model: Option<PathBuf>,
    det_model: Option<PathBuf>,
    /// Open the editor URL in the default browser once the listener is ready.
    open_browser: bool,
    #[cfg(feature = "ocr")]
    ocr: Option<Arc<Mutex<oas_ocr::onnx::RecEngine>>>,
    #[cfg(feature = "ocr")]
    det: Option<Arc<Mutex<oas_ocr::det_engine::DetEngine>>>,
}

/// Grab one device frame (used by the live preview and WebSocket stream).
fn capture_device_frame(adb: &Adb) -> Result<(String, Frame), String> {
    let wanted = crate::config::device_serial();
    let serial = adb.resolve_device(wanted.as_deref())?.serial;
    let mut screen = adb.screen(&serial);
    let frame = screen.grab()?;
    Ok((serial, frame))
}

/// One live-preview passthrough tap. Coordinates are in the current live frame.
#[derive(Debug, Deserialize)]
struct InputTapRequest {
    x: i32,
    y: i32,
}

/// Metadata for one manual capture. The screenshot always comes from adb; the
/// workbench canvas/selection is deliberately not part of this data path.
#[derive(Debug, Default, Deserialize)]
struct CaptureRequest {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    category: String,
    #[serde(default)]
    select: Option<bool>,
}

/// One live-preview passthrough swipe. Coordinates are in the current live frame.
#[derive(Debug, Deserialize)]
struct InputSwipeRequest {
    from: [i32; 2],
    to: [i32; 2],
    #[serde(default)]
    ms: Option<u32>,
}

/// Validate a live-preview input point against the frame just shown to the user.
/// Returning 400 here prevents an accidental desktop click from landing outside
/// the device screen (or on a stale/non-live frame).
fn validate_live_input_point(x: i32, y: i32, width: u32, height: u32) -> Result<(), String> {
    if width == 0 || height == 0 {
        return Err("no live frame size available".to_string());
    }
    if x < 0 || y < 0 || x >= width as i32 || y >= height as i32 {
        return Err(format!(
            "point ({x},{y}) outside live frame {width}x{height}"
        ));
    }
    Ok(())
}

// --- minimal WebSocket live preview (no extra dependency) -------------------
// The old OAS annotator pushes JPEG over WebSocket. We do the same shape with
// PNG frames (the codec already produces PNG) so the editor can display a
// continuous stream instead of polling /frame.png every 1.2s.

fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];
    let ml = (data.len() as u64) * 8;
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&ml.to_be_bytes());
    for chunk in msg.chunks_exact(64) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let (mut a, mut b, mut c, mut d, mut e) = (h[0], h[1], h[2], h[3], h[4]);
        for i in 0..80 {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5A827999),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };
            let t = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(w[i]);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = t;
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }
    let mut out = [0u8; 20];
    for (i, v) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&v.to_be_bytes());
    }
    out
}

fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        out.push(T[(b[0] >> 2) as usize] as char);
        out.push(T[(((b[0] & 0x03) << 4) | (b[1] >> 4)) as usize] as char);
        out.push(if chunk.len() > 1 { T[(((b[1] & 0x0F) << 2) | (b[2] >> 6)) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { T[(b[2] & 0x3F) as usize] as char } else { '=' });
    }
    out
}

fn ws_send_binary(stream: &mut TcpStream, payload: &[u8]) -> std::io::Result<()> {
    let mut head = Vec::with_capacity(10);
    head.push(0x82); // FIN + binary
    if payload.len() < 126 {
        head.push(payload.len() as u8);
    } else if payload.len() <= u16::MAX as usize {
        head.push(126);
        head.extend_from_slice(&(payload.len() as u16).to_be_bytes());
    } else {
        head.push(127);
        head.extend_from_slice(&(payload.len() as u64).to_be_bytes());
    }
    stream.write_all(&head)?;
    stream.write_all(payload)?;
    stream.flush()
}

fn live_ws_loop(mut stream: TcpStream, adb: Adb, state: Arc<Mutex<State>>) {
    loop {
        match capture_device_frame(&adb) {
            Ok((serial, frame)) => match encode_png(&frame) {
                Ok(png) => {
                    {
                        let mut st = state.lock().unwrap();
                        st.live_png = Some(png.clone());
                        st.live_frame = Some(frame.clone());
                        if st.current_frame_id.is_none() {
                            st.png = Some(png.clone());
                            st.frame = Some(frame);
                        }
                        st.device = serial;
                        st.error = None;
                    }
                    if ws_send_binary(&mut stream, &png).is_err() {
                        break;
                    }
                }
                Err(e) => {
                    if let Ok(mut st) = state.lock() {
                        st.error = Some(format!("encode png: {e}"));
                    }
                    std::thread::sleep(Duration::from_millis(500));
                }
            },
            Err(e) => {
                if let Ok(mut st) = state.lock() {
                    st.error = Some(format!("capture failed: {e}"));
                }
                std::thread::sleep(Duration::from_millis(800));
            }
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

impl Server {
    pub fn new(adb: Adb) -> Self {
        let default_executor: Arc<RunExecutor> = Arc::new(default_run_executor);
        Self {
            adb,
            state: Arc::new(Mutex::new(State {
                frame: None,
                png: None,
                live_frame: None,
                live_png: None,
                device: String::new(),
                error: None,
                frames: Vec::new(),
                next_frame_id: 1,
                current_frame_id: None,
                pending_capture: None,
            })),
            tasks_dir: PathBuf::from("tasks"),
            runs_dir: PathBuf::from("runs"),
            library_dir: PathBuf::from(crate::library::DEFAULT_DIR),
            assets_dir: PathBuf::from(crate::global_assets::DEFAULT_DIR),
            frames_dir: PathBuf::from("frames"),
            run_state: Arc::new(Mutex::new(RunState::default())),
            run_executor: default_executor,
            editor_log: PathBuf::from("logs/editor.log"),
            ocr_model: None,
            det_model: None,
            open_browser: true,
            #[cfg(feature = "ocr")]
            ocr: None,
            #[cfg(feature = "ocr")]
            det: None,
        }
    }

    /// Replace the executor behind `POST /task/run` (tests inject a fake one so
    /// the run state machine can be checked without a device).
    pub fn with_run_executor(mut self, executor: Arc<RunExecutor>) -> Self {
        self.run_executor = executor;
        self
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

    /// Set the global template asset library root (default `library`; §22.22).
    pub fn with_library_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.library_dir = dir.into();
        self
    }

    /// Set the unified global asset store root (default `assets`).
    pub fn with_assets_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.assets_dir = dir.into();
        self
    }

    /// Set the disk-backed frame gallery root (default `frames`). If a manifest
    /// already exists it is loaded immediately, so a restarted serve keeps the
    /// gallery without an extra command.
    pub fn with_frames_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.frames_dir = dir.into();
        if let Err(e) = self.load_frames_from_disk() {
            eprintln!("frames manifest load failed: {e}");
        }
        self
    }

    /// `--no-open`: keep the server headless (tests, services, CI).
    pub fn with_open_browser(mut self, open: bool) -> Self {
        self.open_browser = open;
        self
    }

    fn persist_frames(&self, st: &State) -> Result<(), String> {
        std::fs::create_dir_all(&self.frames_dir)
            .map_err(|e| format!("create frames dir {}: {e}", self.frames_dir.display()))?;
        let frames: Vec<FrameMeta> = st
            .frames
            .iter()
            .map(|f| FrameMeta {
                id: f.id,
                name: f.name.clone(),
                category: f.category.clone(),
                file: f
                    .file
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                width: f.width,
                height: f.height,
                source: f.source.clone(),
                ts_ms: f.ts_ms,
                seen: f.seen,
            })
            .collect();
        let manifest = FramesManifest {
            next_id: st.next_frame_id,
            current_id: st.current_frame_id,
            frames,
        };
        let text = serde_json::to_string_pretty(&manifest)
            .map_err(|e| format!("serialize frames manifest: {e}"))?;
        let path = self.frames_dir.join("frames.json");
        let tmp = self.frames_dir.join("frames.json.tmp");
        std::fs::write(&tmp, format!("{text}\n"))
            .map_err(|e| format!("write {}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, &path)
            .map_err(|e| format!("rename {} -> {}: {e}", tmp.display(), path.display()))?;
        Ok(())
    }

    fn load_frames_from_disk(&self) -> Result<(), String> {
        let path = self.frames_dir.join("frames.json");
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(format!("read {}: {e}", path.display())),
        };
        let manifest: FramesManifest = serde_json::from_str(&text)
            .map_err(|e| format!("{}: invalid JSON: {e}", path.display()))?;
        let mut frames = Vec::with_capacity(manifest.frames.len());
        for meta in manifest.frames {
            let file = self.frames_dir.join(&meta.file);
            let bytes = std::fs::metadata(&file).map(|m| m.len()).unwrap_or(0);
            frames.push(SessionFrame {
                id: meta.id,
                name: meta.name,
                category: meta.category,
                file,
                bytes,
                width: meta.width,
                height: meta.height,
                source: meta.source,
                ts_ms: meta.ts_ms,
                seen: meta.seen,
            });
        }
        let mut max_id = frames.iter().map(|f| f.id).max().unwrap_or(0);
        let next_id = manifest.next_id.max(max_id + 1).max(1);
        let mut st = self.state.lock().unwrap();
        st.frames = frames;
        st.next_frame_id = next_id;
        st.current_frame_id = manifest.current_id;
        st.frame = None;
        st.png = None;
        if let Some(id) = manifest.current_id {
            if let Some(f) = st.frames.iter().find(|f| f.id == id) {
                let path = f.file.clone();
                let w = f.width;
                let h = f.height;
                match std::fs::read(&path).and_then(|bytes| {
                    oas_codec::decode_png(&bytes)
                        .map(|frame| (bytes, frame))
                        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
                }) {
                    Ok((png, frame)) => {
                        st.png = Some(png);
                        st.frame = Some(frame);
                    }
                    Err(e) => eprintln!(
                        "frames manifest: current frame {id} ({w}x{h}) load failed: {e}"
                    ),
                }
            }
        }
        Ok(())
    }

    fn using_global_assets(&self) -> bool {
        crate::global_assets::is_initialized(&self.assets_dir)
    }

    /// Asset store used by editor routes: global store when initialized,
    /// otherwise the legacy per-task bundle.
    fn asset_store(&self, task: &str) -> Result<oas_task::TaskBundle, String> {
        if self.using_global_assets() {
            crate::global_assets::open_store(&self.assets_dir)
        } else {
            oas_task::TaskBundle::open_named(&self.tasks_dir, task)
        }
    }

    /// Optional OCR polygon for an asset-backed editor test. Passing `id` makes
    /// `/ocr` and `/lines` apply the same mask runtime `ocr_read` uses.
    fn asset_ocr_polygon(&self, task: &str, id: &str) -> Option<Vec<oas_types::Point>> {
        if id.is_empty() {
            return None;
        }
        let bundle = self.asset_store(task).ok()?;
        let af = bundle.load_assets_file().ok()?;
        let item = af.items.into_iter().find(|a| a.id == id)?;
        item.polygon.map(|pts| {
            pts.into_iter()
                .map(|p| oas_types::Point { x: p[0], y: p[1] })
                .collect()
        })
    }

    fn global_asset_ids(&self) -> Result<std::collections::BTreeSet<String>, String> {
        let store = crate::global_assets::open_store(&self.assets_dir)?;
        let af = store.load_assets_file()?;
        let mut ids: std::collections::BTreeSet<String> =
            af.items.into_iter().map(|a| a.id).collect();
        ids.extend(af.collections.into_iter().map(|c| c.id));
        Ok(ids)
    }

    /// After a collection is saved, switch every task flow from any member asset
    /// id to the collection id (user 10-31: "when an asset is added to a
    /// collection, all tasks referencing it switch to the collection").
    fn rewrite_tasks_to_collection(
        &self,
        collection_id: &str,
        members: &[String],
    ) -> (usize, Vec<String>) {
        let mapping: Vec<(String, String)> = members
            .iter()
            .filter(|m| m.as_str() != collection_id)
            .map(|m| (m.clone(), collection_id.to_string()))
            .collect();
        if mapping.is_empty() {
            return (0, Vec::new());
        }
        let ids = match self.global_asset_ids() {
            Ok(v) => v,
            Err(e) => return (0, vec![e]),
        };
        let names = match oas_task::list_tasks(&self.tasks_dir) {
            Ok(v) => v,
            Err(e) => return (0, vec![e]),
        };
        let mut rewritten = 0usize;
        let mut errors = Vec::new();
        for name in names {
            let bundle = match oas_task::TaskBundle::open_named(&self.tasks_dir, &name) {
                Ok(b) => b,
                Err(_) => continue,
            };
            let path = bundle.flow_path();
            let text = match std::fs::read_to_string(&path) {
                Ok(t) => t,
                Err(_) => continue,
            };
            let mut v: serde_json::Value = match serde_json::from_str(&text) {
                Ok(v) => v,
                Err(e) => {
                    errors.push(format!("{name}: flow.json parse: {e}"));
                    continue;
                }
            };
            if !rewrite_flow_asset_refs(&mut v, &mapping) {
                continue;
            }
            let text = match serde_json::to_string(&v) {
                Ok(t) => t,
                Err(e) => {
                    errors.push(format!("{name}: serialize: {e}"));
                    continue;
                }
            };
            match bundle.save_flow_with_ids(&text, &ids) {
                Ok(()) => rewritten += 1,
                Err(e) => errors.push(format!("{name}: {e}")),
            }
        }
        (rewritten, errors)
    }

    /// Asset ids whose `positions` contain `frame`. Used to block frame delete.
    fn frame_position_referrers(&self, frame: u64) -> Vec<String> {
        let mut out = Vec::new();
        let mut push = |af: oas_task::AssetsFile| {
            for a in af.items {
                if a.positions.contains(&frame) {
                    out.push(a.id);
                }
            }
        };
        if self.using_global_assets() {
            if let Ok(store) = crate::global_assets::open_store(&self.assets_dir) {
                if let Ok(af) = store.load_assets_file() {
                    push(af);
                }
            }
        } else if let Ok(rd) = std::fs::read_dir(&self.tasks_dir) {
            for e in rd.flatten() {
                if !e.path().is_dir() {
                    continue;
                }
                let Some(name) = e.file_name().to_str().map(|s| s.to_string()) else {
                    continue;
                };
                if let Ok(b) = oas_task::TaskBundle::open_named(&self.tasks_dir, &name) {
                    if let Ok(af) = b.load_assets_file() {
                        push(af);
                    }
                }
            }
        }
        out.sort();
        out.dedup();
        out
    }

    /// Task names whose flow.json references `id` (global delete/rename guard).
    fn asset_referrers(&self, id: &str) -> Vec<String> {
        let mut out = Vec::new();
        if let Ok(names) = oas_task::list_tasks(&self.tasks_dir) {
            for name in names {
                if let Ok(b) = oas_task::TaskBundle::open_named(&self.tasks_dir, &name) {
                    if let Ok(Some(flow)) = b.load_flow() {
                        if flow.referenced_assets().contains(id) {
                            out.push(name);
                        }
                    }
                }
            }
        }
        out
    }

    fn library(&self) -> crate::library::Library {
        crate::library::Library::open(&self.library_dir)
    }

    /// Load the PP-OCRv6 rec model; returns the charset size.
    #[cfg(feature = "ocr")]
    pub fn ocr_ready(&self) -> bool {
        self.ocr.is_some()
    }

    #[cfg(not(feature = "ocr"))]
    pub fn ocr_ready(&self) -> bool {
        false
    }

    #[cfg(feature = "ocr")]
    pub fn det_ready(&self) -> bool {
        self.det.is_some()
    }

    #[cfg(not(feature = "ocr"))]
    pub fn det_ready(&self) -> bool {
        false
    }

    #[cfg(feature = "ocr")]
    pub fn enable_ocr(&mut self, model_path: &str) -> Result<usize, String> {
        let engine = oas_ocr::onnx::RecEngine::from_file(model_path)?;
        let n = engine.charset().len();
        self.ocr = Some(Arc::new(Mutex::new(engine)));
        self.ocr_model = Some(PathBuf::from(model_path));
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

        // No eager capture at startup: the editor shows a live frame instead
        // (GET /frame.png?live=1) so starting the app never spams the gallery.
        if self.open_browser {
            open_editor_url(&format!("http://{addr}/"));
        }

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

    /// Live preview capture: update the current frame only. It deliberately
    /// does NOT append to the frame gallery or persist frames.json — only the
    /// explicit 截图/入库 actions do that.
    fn do_live_capture(&self) {
        match capture_device_frame(&self.adb) {
            Ok((serial, frame)) => match encode_png(&frame) {
                Ok(png) => {
                    let mut st = self.state.lock().unwrap();
                    st.live_png = Some(png.clone());
                    st.live_frame = Some(frame.clone());
                    // Live updates the analysis frame only while no gallery
                    // frame is selected; otherwise a selected image would be
                    // silently replaced by the live screen before OCR/match.
                    if st.current_frame_id.is_none() {
                        st.png = Some(png);
                        st.frame = Some(frame);
                    }
                    st.device = serial;
                    st.error = None;
                }
                Err(e) => self.set_error(&format!("encode png: {e}")),
            },
            Err(e) => self.set_error(&format!("capture failed: {e}")),
        }
    }

    fn capture_device_png(&self) -> Result<(Vec<u8>, u32, u32), String> {
        let wanted = crate::config::device_serial();
        let serial = self.adb.resolve_device(wanted.as_deref())?.serial;
        let mut screen = self.adb.screen(&serial);
        let frame = screen.grab()?;
        let png = encode_png(&frame)?;
        Ok((png, frame.width, frame.height))
    }

    fn capture_frame(
        &self,
        req: CaptureRequest,
    ) -> Result<(u64, String, String, u32, u32, bool), String> {
        let wanted = crate::config::device_serial();
        let serial = self.adb.resolve_device(wanted.as_deref())?.serial;
        let mut screen = self.adb.screen(&serial);
        let frame = screen.grab()?;
        let png = encode_png(&frame)?;
        let (w, h) = (frame.width, frame.height);
        let select = req.select.unwrap_or(true);
        let mut st = self.state.lock().map_err(|_| "state lock poisoned".to_string())?;
        let id = st.push_frame(
            &self.frames_dir,
            png.clone(),
            w,
            h,
            "capture",
            req.name,
            req.category,
            false,
            select,
        )?;
        let saved = st
            .frames
            .iter()
            .find(|f| f.id == id)
            .map(|f| (f.name.clone(), f.category.clone(), f.seen))
            .ok_or_else(|| "captured frame disappeared".to_string())?;
        if select {
            st.png = Some(png);
            st.frame = Some(frame);
        }
        st.device = serial;
        st.error = None;
        let _ = self.persist_frames(&st);
        Ok((id, saved.0, saved.1, w, h, saved.2))
    }

    fn set_error(&self, msg: &str) {
        eprintln!("capture error: {msg}");
        let mut st = self.state.lock().unwrap();
        st.error = Some(msg.to_string());
    }

    /// Upgrade `/ws/live` to a WebSocket and stream PNG frames from a dedicated
    /// capture thread. The main HTTP accept loop stays responsive.
    fn handle_live_ws(&self, mut stream: TcpStream, key: &str) -> std::io::Result<()> {
        let accept = base64(&sha1(
            format!("{key}258EAFA5-E914-47DA-95CA-C5AB0DC85B11").as_bytes(),
        ));
        let head = format!(
            "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n"
        );
        stream.write_all(head.as_bytes())?;
        stream.flush()?;
        let adb = self.adb.clone();
        let state = self.state.clone();
        std::thread::Builder::new()
            .name("oasd-live-ws".to_string())
            .spawn(move || live_ws_loop(stream, adb, state))
            .map(|_| ())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
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
        let mut ws_key: Option<String> = None;
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line)? == 0 || line == "\r\n" || line == "\n" {
                break;
            }
            let lower = line.to_ascii_lowercase();
            if let Some(rest) = lower.strip_prefix("content-length:") {
                content_length = rest.trim().parse().unwrap_or(0);
            }
            if lower.starts_with("sec-websocket-key:") {
                if let Some((_, value)) = line.split_once(':') {
                    ws_key = Some(value.trim().to_string());
                }
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
        if path_only == "/ws/live" {
            let Some(key) = ws_key else {
                let mut out = stream;
                return self.send_json(&mut out, 400, r#"{"error":"missing Sec-WebSocket-Key"}"#);
            };
            return self.handle_live_ws(stream, &key);
        }
        // Persist only editor data-plane actions (never the 500ms /state poll).
        const LOGGED_PREFIXES: [&str; 6] = ["/ocr", "/lines", "/match", "/task/", "/library/", "/scheduler/"];
        if LOGGED_PREFIXES
            .iter()
            .any(|p| path_only == p.trim_end_matches('/') || path_only.starts_with(*p) || path_only.starts_with("/task/"))
        {
            self.editor_log(&format!("{method} {path}"));
        }

        let mut out = stream;
        match (method, path_only.as_str()) {
            ("GET", "/") => self.send_file(&mut out, &format!("{ui_dir}/index.html"), "text/html; charset=utf-8"),
            ("GET", "/app.js") => self.send_file(&mut out, &format!("{ui_dir}/app.js"), "application/javascript; charset=utf-8"),
            ("GET", "/asset_ui.js") => self.send_file(&mut out, &format!("{ui_dir}/asset_ui.js"), "application/javascript; charset=utf-8"),
            ("GET", "/builder.js") => self.send_file(&mut out, &format!("{ui_dir}/builder.js"), "application/javascript; charset=utf-8"),
            ("GET", "/style.css") => self.send_file(&mut out, &format!("{ui_dir}/style.css"), "text/css; charset=utf-8"),
            ("GET", "/flow_model.js") => self.send_file(&mut out, &format!("{ui_dir}/flow_model.js"), "application/javascript; charset=utf-8"),
            ("GET", "/dashboard.html") => self.send_file(&mut out, &format!("{ui_dir}/dashboard.html"), "text/html; charset=utf-8"),
            ("GET", "/dashboard.js") => self.send_file(&mut out, &format!("{ui_dir}/dashboard.js"), "application/javascript; charset=utf-8"),
            ("GET", "/frame.png") => {
                let q = parse_query(&path);
                let live = q.iter().any(|(k, v)| k == "live" && v == "1");
                if live {
                    self.do_live_capture();
                }
                let png = {
                    let st = self.state.lock().unwrap();
                    if live {
                        st.live_png.clone().or_else(|| st.png.clone())
                    } else {
                        st.png.clone()
                    }
                };
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
                let current_name = st
                    .current_frame_id
                    .and_then(|id| st.frames.iter().find(|f| f.id == id))
                    .map(|f| f.name.clone())
                    .unwrap_or_default();
                let body = format!(
                    r#"{{"captured":{},"width":{},"height":{},"device":{:?},"error":{:?},"ocr":{},"det":{},"current_frame_id":{},"current_frame_name":{:?},"live":{}}}"#,
                    st.png.is_some(),
                    w,
                    h,
                    st.device,
                    st.error.clone().unwrap_or_default(),
                    self.ocr_ready(),
                    self.det_ready(),
                    serde_json::to_string(&st.current_frame_id).unwrap_or_else(|_| "null".into()),
                    current_name,
                    st.live_png.is_some()
                );
                drop(st);
                self.send_json(&mut out, 200, &body)
            }
            ("POST", "/capture") => self.handle_capture(&mut out, &body),
            // Two-phase manual capture: preview now, commit only after the
            // name/category dialog is confirmed.
            ("POST", "/capture/preview") => self.handle_capture_preview(&mut out),
            ("GET", "/capture/preview.png") => self.handle_capture_preview_png(&mut out),
            ("POST", "/capture/commit") => self.handle_capture_commit(&mut out, &body),
            // Live-preview passthrough: only meaningful while the WebSocket
            // live frame is current. The UI gates this on `livePreview`.
            ("POST", "/input/tap") => self.handle_input_tap(&mut out, &body),
            ("POST", "/input/swipe") => self.handle_input_swipe(&mut out, &body),
            // --- §22.27 会话图片列表（旧 annotator「图片」面板）---
            ("GET", "/frames") => self.handle_frames(&mut out),
            ("GET", "/frames/image") => self.handle_frame_image(&mut out, &path),
            ("POST", "/frames") => self.handle_frame_upload(&mut out, &body),
            ("POST", "/frames/select") => self.handle_frame_select(&mut out, &body),
            ("POST", "/frames/delete") => self.handle_frame_delete(&mut out, &body),
            ("POST", "/frames/rename") => self.handle_frame_rename(&mut out, &body),
            ("POST", "/frames/category") => self.handle_frame_category(&mut out, &body),
            ("POST", "/frames/reorder") => self.handle_frames_reorder(&mut out, &body),
            ("POST", "/frames/clear") => self.handle_frames_clear(&mut out),
            ("GET", "/ocr") => self.handle_ocr(&mut out, &path),
            ("GET", "/lines") => self.handle_lines(&mut out, &path),
            // --- N3: task bundle save routes ---
            ("POST", "/task/save_asset") => self.handle_save_asset(&mut out, &body),
            ("POST", "/task/save_assets") => self.handle_save_assets(&mut out, &body),
            ("GET", "/assets/conflicts") => self.handle_asset_conflicts(&mut out),
            ("POST", "/assets/migrate") => self.handle_assets_migrate(&mut out),
            ("POST", "/task/asset/delete") => self.handle_asset_delete(&mut out, &body),
            ("POST", "/task/asset/collection") => self.handle_collection_save(&mut out, &body),
            ("POST", "/task/asset/collection/delete") => self.handle_collection_delete(&mut out, &body),
            ("POST", "/task/asset/rename") => self.handle_asset_rename(&mut out, &body),
            ("POST", "/task/asset/update") => self.handle_asset_update(&mut out, &body),
            ("POST", "/task/asset/positions") => self.handle_asset_positions(&mut out, &body),
            ("POST", "/task/goal") => self.handle_task_goal(&mut out, &body),
            ("GET", "/task/assets") => self.handle_task_assets(&mut out, &path),
            ("POST", "/task/assets/batch_test") => self.handle_assets_batch_test(&mut out, &body),
            ("GET", "/task/asset/image") => self.handle_asset_image(&mut out, &path),
            ("GET", "/task/asset/collection/match") => self.handle_collection_match(&mut out, &path),
            ("GET", "/task/asset/match") => self.handle_asset_match(&mut out, &path),
            ("GET", "/task/flow") => self.handle_task_flow(&mut out, &path),
            ("POST", "/task/save_flow") => self.handle_save_flow(&mut out, &body),
            ("GET", "/task/list") => self.handle_task_list(&mut out),
            // --- N38/M1: task creation + editor-triggered run ---
            ("POST", "/task/create") => self.handle_task_create(&mut out, &body),
            ("POST", "/task/run") => self.handle_task_run(&mut out, &body),
            ("POST", "/task/run/stop") => self.handle_run_stop(&mut out),
            ("GET", "/task/run/status") => self.handle_run_status(&mut out),
            // --- M2/N37: scheduler status panel + reset-memory dialog ---
            ("GET", "/scheduler/status") => self.handle_scheduler_status(&mut out),
            ("POST", "/scheduler/reset") => self.handle_scheduler_reset(&mut out, &body),
            // --- v1.1 P3: app shell data routes (task library / instance / settings) ---
            ("GET", "/task/library") => self.handle_task_library(&mut out),
            ("POST", "/task/delete") => self.handle_task_delete(&mut out, &body),
            ("GET", "/config") => self.handle_config(&mut out),
            ("GET", "/instance") => self.handle_instance(&mut out),
            ("GET", "/instance/devices") => self.handle_instance_devices(&mut out),
            ("POST", "/instance") => self.handle_instance_update(&mut out, &body),
            ("GET", "/stats") => self.handle_stats(&mut out),
            ("GET", "/match") => self.handle_match(&mut out, &path),
            // --- §22.22 global template asset library ---
            ("GET", "/library") => self.handle_library_list(&mut out),
            ("POST", "/library/save_asset") => self.handle_library_save(&mut out, &body),
            // --- N4: click-distribution data plane (§21 CD1~CD4) ---
            ("GET", "/runs") => self.handle_runs(&mut out),
            ("GET", "/runs/clicks") => self.handle_run_clicks(&mut out, &path),
            ("GET", "/runs/aggregate") => self.handle_runs_aggregate(&mut out, &path),
            ("POST", "/runs/delete") => self.handle_runs_delete(&mut out, &body),
            ("POST", other) => self.send_errors(
                &mut out,
                404,
                vec![format!("no such POST route: {other}")],
            ),
            _ => self.send_json(&mut out, 404, r#"{"error":"not found"}"#),
        }
    }

    /// `GET /frames` — 会话图片列表（不含像素，只有元信息）.
    /// `GET /frames/image?id=N` — raw persisted frame PNG (asset thumbnails and
    /// future batch tests crop from this without changing the current frame).
    fn handle_frame_image(&self, out: &mut TcpStream, raw_path: &str) -> std::io::Result<()> {
        let q = parse_query(raw_path);
        let id = q
            .iter()
            .find(|(k, _)| k == "id")
            .and_then(|(_, v)| v.parse::<u32>().ok());
        let Some(id) = id else {
            return self.send_errors(out, 400, vec!["missing/invalid query param `id`".into()]);
        };
        let path = self.frames_dir.join(format!("frame-{id}.png"));
        match std::fs::read(&path) {
            Ok(bytes) => self.send_bytes(out, 200, "image/png", &bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                self.send_json(out, 404, r#"{"error":"frame image not found"}"#)
            }
            Err(e) => self.send_errors(out, 500, vec![format!("read {}: {e}", path.display())]),
        }
    }

    /// `GET /task/asset/image?task=&id=` — saved template image for the asset
    /// row thumbnail. OCR/region assets have no file and return 404; the UI
    /// crops their ROI from a frame instead.
    fn handle_asset_image(&self, out: &mut TcpStream, raw_path: &str) -> std::io::Result<()> {
        let q = parse_query(raw_path);
        let task = q
            .iter()
            .find(|(k, _)| k == "task")
            .map(|(_, v)| v.clone())
            .unwrap_or_default();
        let id = q
            .iter()
            .find(|(k, _)| k == "id")
            .map(|(_, v)| v.clone())
            .unwrap_or_default();
        if id.is_empty() {
            return self.send_errors(out, 400, vec!["missing query param `id`".into()]);
        }
        let store = match self.asset_store(&task) {
            Ok(b) => b,
            Err(e) => return self.send_errors(out, 400, vec![e]),
        };
        let af = match store.load_assets_file() {
            Ok(a) => a,
            Err(e) => return self.send_errors(out, 400, vec![e]),
        };
        let Some(item) = af.items.into_iter().find(|a| a.id == id) else {
            return self.send_errors(out, 404, vec![format!("asset `{id}` not found")]);
        };
        let Some(file) = item.file else {
            return self.send_json(out, 404, r#"{"error":"asset has no image"}"#);
        };
        let root = match std::fs::canonicalize(store.root()) {
            Ok(p) => p,
            Err(e) => return self.send_errors(out, 500, vec![format!("asset root: {e}")]),
        };
        let path = match std::fs::canonicalize(store.root().join(&file)) {
            Ok(p) => p,
            Err(e) => return self.send_errors(out, 404, vec![format!("asset image `{file}`: {e}")]),
        };
        if !path.starts_with(&root) {
            return self.send_errors(out, 400, vec!["asset image escapes store root".into()]);
        }
        match std::fs::read(&path) {
            Ok(bytes) => self.send_bytes(out, 200, "image/png", &bytes),
            Err(e) => self.send_errors(out, 500, vec![format!("read {}: {e}", path.display())]),
        }
    }

    fn handle_frames(&self, out: &mut TcpStream) -> std::io::Result<()> {
        let st = self.state.lock().unwrap();
        let frames: Vec<serde_json::Value> = st
            .frames
            .iter()
            .map(|f| {
                serde_json::json!({
                    "id": f.id,
                    "name": f.name,
                    "category": f.category,
                    "width": f.width,
                    "height": f.height,
                    "source": f.source,
                    "ts_ms": f.ts_ms,
                    "bytes": f.bytes,
                    "current": st.current_frame_id == Some(f.id),
                    "seen": f.seen,
                })
            })
            .collect();
        drop(st);
        let body = serde_json::json!({"ok": true, "frames": frames});
        self.send_json(out, 200, &body.to_string())
    }

    /// `POST /frames` — body = raw PNG bytes（编辑器把任意图片转成 PNG 后上传）。
    fn handle_frame_upload(&self, out: &mut TcpStream, body: &[u8]) -> std::io::Result<()> {
        if body.is_empty() {
            return self.send_errors(
                out,
                400,
                vec!["empty body: POST the raw PNG bytes (Content-Type: image/png)".to_string()],
            );
        }
        let frame = match oas_codec::decode_png(body) {
            Ok(f) => f,
            Err(e) => {
                return self.send_errors(out, 400, vec![format!("not a valid PNG image: {e}")])
            }
        };
        let (w, h) = (frame.width, frame.height);
        let mut st = self.state.lock().unwrap();
        let id = match st.push_frame(
            &self.frames_dir,
            body.to_vec(),
            w,
            h,
            "upload",
            None,
            String::new(),
            false,
            true,
        ) {
            Ok(id) => id,
            Err(e) => {
                drop(st);
                return self.send_errors(out, 500, vec![format!("save frame: {e}")]);
            }
        };
        st.frame = Some(frame);
        st.png = Some(body.to_vec());
        st.error = None;
        let _ = self.persist_frames(&st);
        drop(st);
        let body = serde_json::json!({"ok": true, "id": id, "width": w, "height": h});
        self.send_json(out, 200, &body.to_string())
    }

    /// `POST /frames/select` — body `{"id":N}`: 切换当前帧（后续 /match、/ocr、裁剪都用它）.
    fn handle_frame_select(&self, out: &mut TcpStream, body: &[u8]) -> std::io::Result<()> {
        let id = match json_u64_field(body, "id") {
            Ok(v) => v,
            Err(e) => return self.send_errors(out, 400, vec![e]),
        };
        let mut st = self.state.lock().unwrap();
        let found = st
            .frames
            .iter_mut()
            .find(|f| f.id == id)
            .map(|f| {
                f.seen = true;
                (f.file.clone(), f.width, f.height)
            });
        let Some((path, w, h)) = found else {
            drop(st);
            return self.send_errors(
                out,
                404,
                vec![format!("frame id {id} not found (GET /frames)")],
            );
        };
        let png = match std::fs::read(&path) {
            Ok(p) => p,
            Err(e) => {
                drop(st);
                return self.send_errors(
                    out,
                    500,
                    vec![format!("read frame {}: {e}", path.display())],
                );
            }
        };
        let frame = match oas_codec::decode_png(&png) {
            Ok(f) => f,
            Err(e) => {
                drop(st);
                return self.send_errors(out, 400, vec![format!("decode frame {id}: {e}")]);
            }
        };
        st.frame = Some(frame);
        st.png = Some(png);
        st.current_frame_id = Some(id);
        let _ = self.persist_frames(&st);
        drop(st);
        let body = serde_json::json!({"ok": true, "id": id, "width": w, "height": h});
        self.send_json(out, 200, &body.to_string())
    }

    /// `POST /frames/delete` — body `{"id":N}`；删当前帧则回退到列表最后一张.
    fn handle_frame_delete(&self, out: &mut TcpStream, body: &[u8]) -> std::io::Result<()> {
        let id = match json_u64_field(body, "id") {
            Ok(v) => v,
            Err(e) => return self.send_errors(out, 400, vec![e]),
        };
        // User rule (10-31): a frame used as an asset position may not be
        // deleted until those positions are removed.
        let referrers = self.frame_position_referrers(id);
        if !referrers.is_empty() {
            return self.send_errors(
                out,
                400,
                vec![format!(
                    "帧 #{id} 被素材位置引用（{}）；请先删除/修改这些素材的位置再删帧",
                    referrers.join("、")
                )],
            );
        }
        let mut st = self.state.lock().unwrap();
        let pos = st.frames.iter().position(|f| f.id == id);
        let Some(pos) = pos else {
            drop(st);
            return self.send_errors(out, 404, vec![format!("frame id {id} not found")]);
        };
        let removed = st.frames.remove(pos);
        let _ = std::fs::remove_file(&removed.file);
        if st.current_frame_id == Some(id) {
            st.current_frame_id = None;
            st.frame = None;
            st.png = None;
            let last = st
                .frames
                .last()
                .map(|f| (f.file.clone(), f.id));
            if let Some((path, last_id)) = last {
                if let Ok(png) = std::fs::read(&path) {
                    if let Ok(frame) = oas_codec::decode_png(&png) {
                        st.png = Some(png);
                        st.frame = Some(frame);
                        st.current_frame_id = Some(last_id);
                    }
                }
            }
        }
        let remaining = st.frames.len();
        let _ = self.persist_frames(&st);
        drop(st);
        let body = serde_json::json!({"ok": true, "remaining": remaining});
        self.send_json(out, 200, &body.to_string())
    }

    /// v1.2.1: rename a session frame (id stays stable).
    fn handle_frame_rename(&self, out: &mut TcpStream, body: &[u8]) -> std::io::Result<()> {
        let id = match json_u64_field(body, "id") {
            Ok(v) => v,
            Err(e) => return self.send_errors(out, 400, vec![e]),
        };
        let v: serde_json::Value = match serde_json::from_slice(body) {
            Ok(v) => v,
            Err(e) => return self.send_errors(out, 400, vec![format!("invalid JSON: {e}")]),
        };
        let name = v.get("name").and_then(|x| x.as_str()).unwrap_or("").trim();
        if name.is_empty() || name.chars().count() > 120 {
            return self.send_errors(out, 400, vec!["frame name must be 1..=120 characters".into()]);
        }
        let mut st = self.state.lock().unwrap();
        let Some(f) = st.frames.iter_mut().find(|f| f.id == id) else {
            drop(st);
            return self.send_errors(out, 404, vec![format!("frame id {id} not found")]);
        };
        f.name = name.to_string();
        f.seen = true;
        let name = f.name.clone();
        let _ = self.persist_frames(&st);
        drop(st);
        self.send_json(out, 200, &serde_json::json!({"ok": true, "id": id, "name": name}).to_string())
    }

    /// `POST /frames/category` — v1.2.1 free-form category/group.
    fn handle_frame_category(&self, out: &mut TcpStream, body: &[u8]) -> std::io::Result<()> {
        let id = match json_u64_field(body, "id") {
            Ok(v) => v,
            Err(e) => return self.send_errors(out, 400, vec![e]),
        };
        let v: serde_json::Value = match serde_json::from_slice(body) {
            Ok(v) => v,
            Err(e) => return self.send_errors(out, 400, vec![format!("invalid JSON: {e}")]),
        };
        let category = v
            .get("category")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .trim();
        if category.chars().count() > 80 {
            return self.send_errors(out, 400, vec!["frame category is longer than 80".into()]);
        }
        let mut st = self.state.lock().unwrap();
        let Some(f) = st.frames.iter_mut().find(|f| f.id == id) else {
            drop(st);
            return self.send_errors(out, 404, vec![format!("frame id {id} not found")]);
        };
        f.category = category.to_string();
        f.seen = true;
        let category = f.category.clone();
        let _ = self.persist_frames(&st);
        drop(st);
        self.send_json(
            out,
            200,
            &serde_json::json!({"ok": true, "id": id, "category": category}).to_string(),
        )
    }

    /// v1.2.1: reorder frames. `ids` may be a partial permutation; listed ids
    /// come first in the given order, remaining frames keep their relative order.
    fn handle_frames_reorder(&self, out: &mut TcpStream, body: &[u8]) -> std::io::Result<()> {
        let v: serde_json::Value = match serde_json::from_slice(body) {
            Ok(v) => v,
            Err(e) => return self.send_errors(out, 400, vec![format!("invalid JSON: {e}")]),
        };
        let Some(arr) = v.get("ids").and_then(|x| x.as_array()) else {
            return self.send_errors(out, 400, vec!["`ids` array is required".into()]);
        };
        let mut want: Vec<u64> = Vec::new();
        for item in arr {
            let Some(n) = item.as_u64() else {
                return self.send_errors(out, 400, vec!["`ids` values must be integers".into()]);
            };
            want.push(n);
        }
        let mut st = self.state.lock().unwrap();
        let mut ordered: Vec<SessionFrame> = Vec::with_capacity(st.frames.len());
        for id in &want {
            if let Some(pos) = st.frames.iter().position(|f| f.id == *id) {
                ordered.push(st.frames.remove(pos));
            }
        }
        ordered.append(&mut st.frames);
        st.frames = ordered;
        for f in &mut st.frames {
            f.seen = true;
        }
        let ids: Vec<u64> = st.frames.iter().map(|f| f.id).collect();
        let _ = self.persist_frames(&st);
        drop(st);
        self.send_json(out, 200, &serde_json::json!({"ok": true, "ids": ids}).to_string())
    }

    /// `POST /frames/clear` — 清空会话图片列表（当前帧像素保留在内存里，避免画面突然消失）.
    fn handle_frames_clear(&self, out: &mut TcpStream) -> std::io::Result<()> {
        let mut st = self.state.lock().unwrap();
        for frame in &st.frames {
            let _ = std::fs::remove_file(&frame.file);
        }
        st.frames.clear();
        st.current_frame_id = None;
        let _ = self.persist_frames(&st);
        drop(st);
        self.send_json(out, 200, r#"{"ok":true}"#)
    }

    #[allow(unused_variables)]
    fn handle_ocr(&self, out: &mut TcpStream, raw_path: &str) -> std::io::Result<()> {
        let q = parse_query(raw_path);
        let mut x = 0i32;
        let mut y = 0i32;
        let mut w = 0i32;
        let mut h = 0i32;
        let mut task = String::new();
        let mut id = String::new();
        for (k, v) in &q {
            let val: i32 = v.parse().unwrap_or(0);
            match k.as_str() {
                "x" => x = val,
                "y" => y = val,
                "w" => w = val,
                "h" => h = val,
                "task" => task = v.clone(),
                "id" => id = v.clone(),
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
            let poly = self.asset_ocr_polygon(&task, &id);
            let masked;
            let frame = if let Some(poly) = poly.as_ref() {
                masked = oas_runtime::mask_frame_for_ocr(&frame, roi, poly);
                &masked
            } else {
                &frame
            };
            let crop = match oas_ocr::RgbCrop::from_rgba(frame.width, frame.height, &frame.rgba, roi) {
                Ok(c) => c,
                Err(e) => {
                    self.editor_log(&format!("ocr FAIL roi={roi:?} task={task} id={id} error={e:?}"));
                    return self.send_json(out, 400, &format!(r#"{{"error":{:?}}}"#, e));
                }
            };
            let t0 = std::time::Instant::now();
            let mut eng = engine.lock().unwrap();
            match eng.recognize(&crop) {
                Ok((text, score)) => {
                    let ms = t0.elapsed().as_secs_f64() * 1000.0;
                    self.editor_log(&format!(
                        "ocr OK roi={roi:?} task={task} id={id} text={text:?} score={score:.4} elapsed_ms={ms:.1} polygon={}",
                        poly.is_some()
                    ));
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
                Err(e) => {
                    self.editor_log(&format!(
                        "ocr FAIL roi={roi:?} task={task} id={id} error={e:?}"
                    ));
                    self.send_json(out, 500, &format!(r#"{{"error":{:?}}}"#, e))
                }
            }
        }
    }

    fn editor_log(&self, msg: &str) {
        let Some(parent) = self.editor_log.parent() else {
            return;
        };
        if std::fs::create_dir_all(parent).is_err() {
            return;
        }
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.editor_log)
        {
            let _ = writeln!(f, "[{ts}] {msg}");
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

    /// Phase 1 of the manual capture dialog: grab the emulator screen into
    /// memory only. No gallery frame/current frame is touched until commit.
    fn handle_capture_preview(&self, out: &mut TcpStream) -> std::io::Result<()> {
        match self.capture_device_png() {
            Ok((png, width, height)) => {
                let mut st = self.state.lock().unwrap();
                st.pending_capture = Some(PendingCapture { png, width, height });
                drop(st);
                self.send_json(
                    out,
                    200,
                    &serde_json::json!({
                        "ok": true,
                        "width": width,
                        "height": height,
                        "preview_url": "/capture/preview.png",
                    })
                    .to_string(),
                )
            }
            Err(e) => {
                let _ = self.set_error(&format!("capture preview failed: {e}"));
                self.send_errors(out, 500, vec![format!("capture preview failed: {e}")])
            }
        }
    }

    /// `GET /capture/preview.png` — pixels of the pending capture, shown inside
    /// the metadata dialog. Not persisted to the gallery yet.
    fn handle_capture_preview_png(&self, out: &mut TcpStream) -> std::io::Result<()> {
        let png = {
            let st = self.state.lock().unwrap();
            st.pending_capture.as_ref().map(|p| p.png.clone())
        };
        match png {
            Some(bytes) => self.send_bytes(out, 200, "image/png", &bytes),
            None => self.send_json(out, 404, r#"{"error":"no pending capture"}"#),
        }
    }

    /// Phase 2 of manual capture: commit the pending pixels with user metadata.
    /// Always `select:false`: the gallery/current analysis frame is untouched.
    fn handle_capture_commit(&self, out: &mut TcpStream, body: &[u8]) -> std::io::Result<()> {
        let req: CaptureRequest = if body.is_empty() {
            CaptureRequest::default()
        } else {
            match serde_json::from_slice(body) {
                Ok(v) => v,
                Err(e) => return self.send_errors(out, 400, vec![format!("invalid JSON: {e}")]),
            }
        };
        let pending = {
            let mut st = self.state.lock().unwrap();
            st.pending_capture.take()
        };
        let Some(pending) = pending else {
            return self.send_errors(out, 400, vec!["no pending capture; click 截取当前帧入库 first".into()]);
        };
        let mut st = self.state.lock().unwrap();
        let id = match st.push_frame(
            &self.frames_dir,
            pending.png,
            pending.width,
            pending.height,
            "capture",
            req.name,
            req.category,
            false,
            false,
        ) {
            Ok(id) => id,
            Err(e) => {
                drop(st);
                return self.send_errors(out, 500, vec![format!("save frame: {e}")]);
            }
        };
        let saved = st
            .frames
            .iter()
            .find(|f| f.id == id)
            .map(|f| (f.name.clone(), f.category.clone(), f.width, f.height))
            .ok_or_else(|| "captured frame disappeared".to_string());
        let (name, category, width, height) = match saved {
            Ok(v) => v,
            Err(e) => {
                drop(st);
                return self.send_errors(out, 500, vec![e]);
            }
        };
        let _ = self.persist_frames(&st);
        drop(st);
        self.send_json(
            out,
            200,
            &serde_json::json!({
                "ok": true,
                "id": id,
                "name": name,
                "category": category,
                "width": width,
                "height": height,
                "seen": false,
                "selected": false,
            })
            .to_string(),
        )
    }

    /// `POST /capture` — always grab the current emulator screen. Optional body
    /// `{name,category,select}`. UI sends `select:false` so capture never jumps
    /// the gallery/current analysis frame and never pauses live preview.
    fn handle_capture(&self, out: &mut TcpStream, body: &[u8]) -> std::io::Result<()> {
        let req: CaptureRequest = if body.is_empty() {
            CaptureRequest::default()
        } else {
            match serde_json::from_slice(body) {
                Ok(v) => v,
                Err(e) => return self.send_errors(out, 400, vec![format!("invalid JSON: {e}")]),
            }
        };
        let selected = req.select.unwrap_or(true);
        match self.capture_frame(req) {
            Ok((id, name, category, width, height, seen)) => self.send_json(
                out,
                200,
                &serde_json::json!({
                    "ok": true,
                    "id": id,
                    "name": name,
                    "category": category,
                    "width": width,
                    "height": height,
                    "seen": seen,
                    "selected": selected,
                })
                .to_string(),
            ),
            Err(e) => {
                let _ = self.set_error(&format!("capture failed: {e}"));
                self.send_errors(out, 500, vec![format!("capture failed: {e}")])
            }
        }
    }

    /// Input passthrough is legal only while a live frame exists. The UI gates
    /// this on the 实时 toggle; a selected gallery frame pauses that toggle.
    fn live_input_context(&self) -> Result<(String, u32, u32), String> {
        let (serial, width, height) = {
            let st = self
                .state
                .lock()
                .map_err(|_| "state lock poisoned".to_string())?;
            let Some(frame) = st.live_frame.as_ref() else {
                return Err("no live frame yet; wait for live preview or press 实时：开".to_string());
            };
            (st.device.clone(), frame.width, frame.height)
        };
        if width == 0 || height == 0 {
            return Err("live frame has zero size".to_string());
        }
        let serial = if serial.trim().is_empty() {
            let wanted = crate::config::device_serial();
            self.adb
                .resolve_device(wanted.as_deref())
                .map_err(|e| format!("resolve device: {e}"))?
                .serial
        } else {
            serial
        };
        Ok((serial, width, height))
    }

    /// `POST /input/tap` — send one tap to the live device.
    fn handle_input_tap(&self, out: &mut TcpStream, body: &[u8]) -> std::io::Result<()> {
        let req: InputTapRequest = match serde_json::from_slice(body) {
            Ok(v) => v,
            Err(e) => return self.send_errors(out, 400, vec![format!("invalid JSON: {e}")]),
        };
        let (serial, width, height) = match self.live_input_context() {
            Ok(v) => v,
            Err(e) => return self.send_errors(out, 400, vec![e]),
        };
        if let Err(e) = validate_live_input_point(req.x, req.y, width, height) {
            return self.send_errors(out, 400, vec![e]);
        }
        let mut input = self.adb.input(&serial);
        match input.click(Point {
            x: req.x,
            y: req.y,
        }) {
            Ok(()) => {
                let body = serde_json::json!({
                    "ok": true,
                    "device": serial,
                    "gesture": "tap",
                    "x": req.x,
                    "y": req.y,
                });
                self.editor_log(&format!("input tap device={serial} x={} y={}", req.x, req.y));
                self.send_json(out, 200, &body.to_string())
            }
            Err(e) => {
                self.editor_log(&format!("input tap FAIL device={serial} error={e}"));
                self.send_errors(out, 500, vec![format!("adb input tap failed: {e}")])
            }
        }
    }

    /// `POST /input/swipe` — send one swipe to the live device.
    fn handle_input_swipe(&self, out: &mut TcpStream, body: &[u8]) -> std::io::Result<()> {
        let req: InputSwipeRequest = match serde_json::from_slice(body) {
            Ok(v) => v,
            Err(e) => return self.send_errors(out, 400, vec![format!("invalid JSON: {e}")]),
        };
        let (serial, width, height) = match self.live_input_context() {
            Ok(v) => v,
            Err(e) => return self.send_errors(out, 400, vec![e]),
        };
        if let Err(e) = validate_live_input_point(req.from[0], req.from[1], width, height) {
            return self.send_errors(out, 400, vec![format!("from: {e}")]);
        }
        if let Err(e) = validate_live_input_point(req.to[0], req.to[1], width, height) {
            return self.send_errors(out, 400, vec![format!("to: {e}")]);
        }
        let ms = req.ms.unwrap_or(300).clamp(50, 2000);
        let from = Point {
            x: req.from[0],
            y: req.from[1],
        };
        let to = Point {
            x: req.to[0],
            y: req.to[1],
        };
        let mut input = self.adb.input(&serial);
        match input.swipe(from, to, ms) {
            Ok(()) => {
                let body = serde_json::json!({
                    "ok": true,
                    "device": serial,
                    "gesture": "swipe",
                    "from": req.from,
                    "to": req.to,
                    "ms": ms,
                });
                self.editor_log(&format!(
                    "input swipe device={serial} from={:?} to={:?} ms={ms}",
                    req.from, req.to
                ));
                self.send_json(out, 200, &body.to_string())
            }
            Err(e) => {
                self.editor_log(&format!("input swipe FAIL device={serial} error={e}"));
                self.send_errors(out, 500, vec![format!("adb input swipe failed: {e}")])
            }
        }
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

/// Best-effort "open editor in the default browser". Never fails the server:
/// a missing desktop/session just means the URL is printed for manual use.
fn open_editor_url(url: &str) {
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(url).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
}

/// HTTP reason phrase (the old code always sent "OK", even on 400/500).
fn reason_phrase(code: u16) -> &'static str {
    match code {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        409 => "Conflict",
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

/// Replace references to `old_ids` with `new_id` in a flow JSON document.
/// Only asset-reference positions are touched (`sel`, `require_template`) and
/// the asset-name literals inside appear/ocr functions; log text is left alone.
fn rewrite_flow_asset_refs(v: &mut serde_json::Value, mapping: &[(String, String)]) -> bool {
    fn repl_str(s: &str, mapping: &[(String, String)]) -> Option<String> {
        let mut out = s.to_string();
        let mut changed = false;
        for (old, new) in mapping {
            let q1 = format!("'{old}'");
            let q2 = format!("\"{old}\"");
            if out.contains(&q1) || out.contains(&q2) {
                out = out.replace(&q1, &format!("'{new}'")).replace(&q2, &format!("\"{new}\""));
                changed = true;
            }
        }
        changed.then_some(out)
    }
    fn walk(v: &mut serde_json::Value, key: Option<&str>, mapping: &[(String, String)]) -> bool {
        match v {
            serde_json::Value::String(s) => {
                let is_asset_key = matches!(key, Some("sel") | Some("require_template"));
                let is_expr = matches!(key, Some("cond") | Some("text"));
                if is_asset_key {
                    if let Some((_, new)) = mapping.iter().find(|(old, _)| old == s) {
                        *s = new.clone();
                        return true;
                    }
                }
                if is_expr {
                    if let Some(next) = repl_str(s, mapping) {
                        *s = next;
                        return true;
                    }
                }
                false
            }
            serde_json::Value::Array(a) => {
                let mut changed = false;
                for item in a {
                    changed |= walk(item, key, mapping);
                }
                changed
            }
            serde_json::Value::Object(o) => {
                let mut changed = false;
                for (k, item) in o.iter_mut() {
                    changed |= walk(item, Some(k), mapping);
                }
                changed
            }
            _ => false,
        }
    }
    walk(v, None, mapping)
}

fn percent_decode(s: &str) -> String {    let bytes = s.as_bytes();
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
        self.det_model = Some(PathBuf::from(model_path));
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
            let mut task = String::new();
            let mut id = String::new();
            for (k, v) in &q {
                match k.as_str() {
                    "x" => x = v.parse().unwrap_or(0),
                    "y" => y = v.parse().unwrap_or(0),
                    "w" => w = v.parse().unwrap_or(0),
                    "h" => h = v.parse().unwrap_or(0),
                    "vertical" => vertical_only = v == "1",
                    "task" => task = v.clone(),
                    "id" => id = v.clone(),
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
            let masked;
            let frame = if let Some(poly) = self.asset_ocr_polygon(&task, &id) {
                masked = oas_runtime::mask_frame_for_ocr(&frame, roi, &poly);
                &masked
            } else {
                &frame
            };
            let t0 = std::time::Instant::now();
            let lines = {
                let mut det_guard = det.lock().unwrap();
                let mut rec_guard = rec.lock().unwrap();
                match crate::ocr::recognize_lines_impl(&mut det_guard, &mut rec_guard, frame, roi, vertical_only) {
                    Ok(l) => l,
                    Err(e) => {
                        self.editor_log(&format!(
                            "lines FAIL roi={roi:?} task={task} id={id} vertical={vertical_only} error={e:?}"
                        ));
                        return self.send_json(out, 400, &format!(r#"{{"error":{:?}}}"#, e));
                    }
                }
            };
            let text_summary = lines
                .iter()
                .map(|l| format!("{:?}({:.3})", l.text, l.score))
                .collect::<Vec<_>>()
                .join(", ");
            self.editor_log(&format!(
                "lines OK roi={roi:?} task={task} id={id} vertical={vertical_only} count={} elapsed_ms={:.1} lines=[{text_summary}]",
                lines.len(),
                t0.elapsed().as_secs_f64() * 1000.0
            ));
            let items: Vec<String> = lines
                .iter()
                .map(|l| {
                    format!(
                        r#"{{"box":[{},{},{},{}],"text":{},"score":{:.4}}}"#,
                        l.rect.x - roi.x,
                        l.rect.y - roi.y,
                        l.rect.w,
                        l.rect.h,
                        json_escape(&l.text),
                        l.score
                    )
                })
                .collect();
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
        let global_mode = self.using_global_assets();
        let task = v
            .get("task")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if !global_mode {
            if task.is_empty() {
                return self.send_errors(out, 400, vec!["`task` is required".into()]);
            }
            if let Err(e) = oas_task::validate_task_name(&task) {
                return self.send_errors(out, 400, vec![e]);
            }
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
        // §22.25: optional explicit operation region (`action_roi`, legacy `roi_back`).
        let roi_back = match json_opt_action_roi(&v) {
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
                    let parsed = pair.and_then(|a| Some((a[0].as_i64()?, a[1].as_i64()?)));
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
                                vec![format!("`polygon[{i}]` must be an [x,y] pair of integers")],
                            )
                        }
                    }
                }
                Some(pts)
            }
        };
        let positions = match v.get("positions") {
            None => Vec::new(),
            Some(x) => match x.as_array() {
                Some(arr) => {
                    let mut out_ids = Vec::with_capacity(arr.len());
                    for (i, item) in arr.iter().enumerate() {
                        let Some(n) = item.as_u64() else {
                            return self.send_errors(
                                out,
                                400,
                                vec![format!("`positions[{i}]` must be a frame id (u64)")],
                            );
                        };
                        out_ids.push(n);
                    }
                    out_ids
                }
                None => {
                    return self.send_errors(
                        out,
                        400,
                        vec!["`positions` must be an array of frame ids".to_string()],
                    )
                }
            },
        };
        if positions.is_empty() {
            return self.send_errors(
                out,
                400,
                vec!["添加素材必须选择至少一个位置帧（positions）".to_string()],
            );
        }
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
        let bundle = if global_mode {
            match self.asset_store("") {
                Ok(b) => b,
                Err(e) => return self.send_errors(out, 400, vec![e]),
            }
        } else {
            match oas_task::TaskBundle::open_named(&self.tasks_dir, &task) {
                Ok(b) => b,
                Err(e) => return self.send_errors(out, 400, vec![e]),
            }
        };
        // v1.2 global-unique naming: creating a new object may not reuse an id
        // from ANY task or the global library. Overwrite (replace=true) is the
        // explicit "same object, rev+1" path and is allowed. In unified global
        // mode the store itself is the single namespace; no legacy scan needed.
        if !replace && !global_mode {
            match self.global_asset_entries() {
                Ok(entries) => {
                    if let Some((_, source)) = entries.iter().find(|(gid, _)| gid == &id) {
                        return self.send_errors(
                            out,
                            400,
                            vec![format!(
                                "asset id `{id}` already exists ({source}); names must be globally unique"
                            )],
                        );
                    }
                }
                Err(e) => return self.send_errors(out, 500, vec![e]),
            }
        }
        let decl = oas_task::AssetDecl {
            id,
            kind,
            file: None,
            roi: Some(roi),
            action_roi: roi_back,
            ocr_mode: None,
            ocr_source_roi: None,
            ocr_text: None,
            threshold,
            polygon,
            rev: None,
            positions,
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
    /// `GET /match?x&y&w&h` — image-recognition smoke test for the editor
    /// (§22.21): crop the ROI as a template, search the WHOLE current frame,
    /// report the best score/position. This is the image counterpart of /ocr.
    fn handle_match(&self, out: &mut TcpStream, raw_path: &str) -> std::io::Result<()> {
        let q = parse_query(raw_path);
        // §22.22: `?lib=<id>` matches a GLOBAL library template over the whole frame.
        if let Some((_, lib_id)) = q.iter().find(|(k, _)| k == "lib") {
            let frame = {
                let st = self.state.lock().unwrap();
                match st.frame.as_ref() {
                    Some(f) => oas_types::Frame::new(f.width, f.height, f.rgba.clone()),
                    None => return self.send_json(out, 400, r#"{"error":"no frame yet"}"#),
                }
            };
            let gray = oas_perceive::GrayImage::from_frame(&frame);
            let (_key, tpl) = match self.library().load_one(lib_id) {
                Ok(Some(t)) => t,
                Ok(None) => {
                    return self.send_errors(
                        out,
                        404,
                        vec![format!(
                            "library asset `{lib_id}` not found (see GET /library)"
                        )],
                    )
                }
                Err(e) => return self.send_errors(out, 400, vec![e]),
            };
            let t0 = std::time::Instant::now();
            let full = oas_perceive::match_template_full(&gray, &tpl.gray);
            let ms = t0.elapsed().as_secs_f64() * 1000.0;
            let body = match full {
                Some(b) => format!(
                    r#"{{"ok":true,"lib":{},"score":{},"x":{},"y":{},"w":{},"h":{},"threshold":{},"elapsed_ms":{}}}"#,
                    json_escape(lib_id),
                    b.score,
                    b.x,
                    b.y,
                    tpl.gray.width,
                    tpl.gray.height,
                    tpl.threshold,
                    ms
                ),
                None => format!(
                    r#"{{"ok":true,"lib":{},"score":-2,"x":-1,"y":-1,"elapsed_ms":{}}}"#,
                    json_escape(lib_id),
                    ms
                ),
            };
            return self.send_json(out, 200, &body);
        }
        let (mut x, mut y, mut w, mut h) = (0i32, 0i32, 0i32, 0i32);
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
        let frame = {
            let st = self.state.lock().unwrap();
            match st.frame.as_ref() {
                Some(f) => oas_types::Frame::new(f.width, f.height, f.rgba.clone()),
                None => return self.send_json(out, 400, r#"{"error":"no frame yet"}"#),
            }
        };
        let gray = oas_perceive::GrayImage::from_frame(&frame);
        let roi = oas_types::Rect { x, y, w, h };
        let tpl = match gray.crop(roi) {
            Some(t) => t,
            None => return self.send_json(out, 400, r#"{"error":"roi outside frame"}"#),
        };
        let t0 = std::time::Instant::now();
        let full = oas_perceive::match_template_full(&gray, &tpl);
        let roi_hit = oas_perceive::match_template(&gray, &tpl, Some(roi));
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        let body = match full {
            Some(b) => format!(
                r#"{{"ok":true,"score":{},"x":{},"y":{},"w":{},"h":{},"elapsed_ms":{},"roi_score":{}}}"#,
                b.score,
                b.x,
                b.y,
                w,
                h,
                ms,
                roi_hit.map(|r| r.score).unwrap_or(-2.0)
            ),
            None => format!(
                r#"{{"ok":true,"score":-2,"x":-1,"y":-1,"roi_score":{},"elapsed_ms":{}}}"#,
                roi_hit.map(|r| r.score).unwrap_or(-2.0),
                ms
            ),
        };
        self.send_json(out, 200, &body)
    }

    /// `GET /library` — list global template library assets (§22.22).
    fn handle_library_list(&self, out: &mut TcpStream) -> std::io::Result<()> {
        match self.library().list() {
            Ok(items) => {
                let body = serde_json::json!({"ok": true, "items": items});
                self.send_json(out, 200, &body.to_string())
            }
            Err(e) => self.send_errors(out, 500, vec![e]),
        }
    }

    /// `POST /library/save_asset` — crop the last captured frame into the global
    /// library, body: `{id, roi:[x,y,w,h], threshold?, replace?}`.
    fn handle_library_save(&self, out: &mut TcpStream, body: &[u8]) -> std::io::Result<()> {
        let v: serde_json::Value = match serde_json::from_slice(body) {
            Ok(v) => v,
            Err(e) => return self.send_errors(out, 400, vec![format!("invalid JSON body: {e}")]),
        };
        let id = match json_str_field(&v, "id") {
            Ok(s) => s,
            Err(e) => return self.send_errors(out, 400, vec![e]),
        };
        let roi = match v.get("roi").and_then(|r| r.as_array()) {
            Some(a) if a.len() == 4 => {
                let mut r = [0i32; 4];
                for (i, val) in a.iter().enumerate() {
                    match val.as_i64() {
                        Some(n) if n >= i32::MIN as i64 && n <= i32::MAX as i64 => {
                            r[i] = n as i32
                        }
                        _ => {
                            return self.send_errors(
                                out,
                                400,
                                vec!["roi must be 4 integers [x,y,w,h]".to_string()],
                            )
                        }
                    }
                }
                r
            }
            _ => {
                return self.send_errors(
                    out,
                    400,
                    vec!["missing integer array field `roi` [x,y,w,h]".to_string()],
                )
            }
        };
        let threshold = v.get("threshold").and_then(|t| t.as_f64()).map(|t| t as f32);
        let replace = v
            .get("replace")
            .and_then(|r| r.as_bool())
            .unwrap_or(false);
        let frame = {
            let st = self.state.lock().unwrap();
            match st.frame.as_ref() {
                Some(f) => oas_types::Frame::new(f.width, f.height, f.rgba.clone()),
                None => {
                    return self.send_errors(
                        out,
                        400,
                        vec!["no frame yet (POST /capture first)".to_string()],
                    )
                }
            }
        };
        match self
            .library()
            .save_template(&frame, roi, &id, threshold, replace)
        {
            Ok(asset) => {
                let body = serde_json::json!({"ok": true, "asset": asset});
                self.send_json(out, 200, &body.to_string())
            }
            Err(e) => self.send_errors(out, 400, vec![e]),
        }
    }

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

    /// POST /task/assets/batch_test
    /// body `{task?, assets?:[ids], frames?:[ids]}` — dry-run every asset on
    /// each of its position frames (or the explicit frame list). Template:
    /// full-frame match; OCR: ROI rec; region: bounds check.
    fn handle_assets_batch_test(&self, out: &mut TcpStream, body: &[u8]) -> std::io::Result<()> {
        let v: serde_json::Value = match serde_json::from_slice(body) {
            Ok(v) => v,
            Err(e) => return self.send_errors(out, 400, vec![format!("invalid JSON: {e}")]),
        };
        let global_mode = self.using_global_assets();
        let task = v.get("task").and_then(|x| x.as_str()).unwrap_or("").trim();
        let only_assets: Option<std::collections::BTreeSet<String>> =
            v.get("assets").and_then(|x| x.as_array()).map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect()
            });
        let only_frames: Option<std::collections::BTreeSet<u64>> =
            v.get("frames").and_then(|x| x.as_array()).map(|a| {
                a.iter()
                    .filter_map(|x| x.as_u64())
                    .collect()
            });
        let bundle = if global_mode {
            match self.asset_store("") {
                Ok(b) => b,
                Err(e) => return self.send_errors(out, 400, vec![e]),
            }
        } else {
            match oas_task::TaskBundle::open_named(&self.tasks_dir, task) {
                Ok(b) => b,
                Err(e) => return self.send_errors(out, 400, vec![e]),
            }
        };
        let af = match bundle.load_assets_file() {
            Ok(v) => v,
            Err(e) => return self.send_errors(out, 400, vec![e]),
        };
        let assets = match bundle.load_runtime_assets() {
            Ok(v) => v,
            Err(e) => return self.send_errors(out, 400, vec![e]),
        };
        let frame_meta: std::collections::BTreeMap<u64, (std::path::PathBuf, u32, u32)> = {
            let st = self.state.lock().unwrap();
            st.frames
                .iter()
                .map(|f| (f.id, (f.file.clone(), f.width, f.height)))
                .collect()
        };
        let mut decoded: std::collections::BTreeMap<u64, oas_types::Frame> =
            std::collections::BTreeMap::new();
        let mut results = Vec::new();
        for item in &af.items {
            if let Some(only) = &only_assets {
                if !only.contains(&item.id) {
                    continue;
                }
            }
            let mut frames = item.positions.clone();
            if let Some(only) = &only_frames {
                frames.retain(|f| only.contains(f));
            }
            if frames.is_empty() {
                results.push(serde_json::json!({
                    "id": item.id, "kind": item.kind, "frame_id": null,
                    "pass": false, "error": "asset has no position frame"
                }));
                continue;
            }
            for fid in frames {
                let Some((path, fw, fh)) = frame_meta.get(&fid) else {
                    results.push(serde_json::json!({
                        "id": item.id, "kind": item.kind, "frame_id": fid,
                        "pass": false, "error": "frame id not found"
                    }));
                    continue;
                };
                if !decoded.contains_key(&fid) {
                    match std::fs::read(path).map_err(|e| e.to_string()).and_then(|b| {
                        oas_codec::decode_png(&b)
                    }) {
                        Ok(frame) => {
                            decoded.insert(fid, frame);
                        }
                        Err(e) => {
                            results.push(serde_json::json!({
                                "id": item.id, "kind": item.kind, "frame_id": fid,
                                "pass": false, "error": format!("decode frame: {e}")
                            }));
                            continue;
                        }
                    }
                }
                let frame = decoded.get(&fid).expect("decoded");
                let mut result = serde_json::json!({
                    "id": item.id, "kind": item.kind, "frame_id": fid,
                    "frame_width": fw, "frame_height": fh
                });
                match item.kind.as_str() {
                    "template" => {
                        if let Some(tpl) = assets.get(&item.id) {
                            let matcher = oas_perceive::Matcher::new(oas_perceive::GrayImage::from_frame(frame));
                            let t0 = std::time::Instant::now();
                            let (score, point) = oas_runtime::locate_template(&matcher, tpl);
                            let (x, y) = point.map(|p| (p.x, p.y)).unwrap_or((-1, -1));
                            result["score"] = serde_json::json!(score);
                            result["x"] = serde_json::json!(x);
                            result["y"] = serde_json::json!(y);
                            result["threshold"] = serde_json::json!(tpl.threshold);
                            result["pass"] = serde_json::json!(score >= tpl.threshold);
                            result["elapsed_ms"] =
                                serde_json::json!(t0.elapsed().as_secs_f64() * 1000.0);
                        } else {
                            result["pass"] = serde_json::json!(false);
                            result["error"] = serde_json::json!("template not loaded");
                        }
                    }
                    "region" => {
                        let ok = item.roi.map_or(false, |r| {
                            r[0] >= 0
                                && r[1] >= 0
                                && r[2] > 0
                                && r[3] > 0
                                && (r[0] + r[2]) as u32 <= *fw
                                && (r[1] + r[3]) as u32 <= *fh
                        });
                        result["pass"] = serde_json::json!(ok);
                        if !ok {
                            result["error"] = serde_json::json!("roi outside frame");
                        }
                    }
                    _ => {
                        #[cfg(feature = "ocr")]
                        {
                            // Multi-line-derived OCR repeats det+rec over the
                            // single saved recognition ROI (v1.2.3 contract).
                            if item.kind == "ocr_line"
                                && item.ocr_mode.as_deref() == Some("lines")
                            {
                                let (Some(det), Some(rec)) = (self.det.as_ref(), self.ocr.as_ref())
                                else {
                                    result["pass"] = serde_json::json!(false);
                                    result["error"] = serde_json::json!("det/rec model not loaded");
                                    results.push(result);
                                    continue;
                                };
                                let Some(src) = item.roi else {
                                    result["pass"] = serde_json::json!(false);
                                    result["error"] = serde_json::json!("derived OCR has no recognition ROI");
                                    results.push(result);
                                    continue;
                                };
                                let rect = oas_types::Rect { x: src[0], y: src[1], w: src[2], h: src[3] };
                                let poly = item.polygon.as_ref().map(|pts| {
                                    pts.iter()
                                        .map(|p| oas_types::Point { x: p[0], y: p[1] })
                                        .collect::<Vec<_>>()
                                });
                                let masked;
                                let oframe = if let Some(poly) = poly.as_ref() {
                                    masked = oas_runtime::mask_frame_for_ocr(frame, rect, poly);
                                    &masked
                                } else {
                                    frame
                                };
                                let mut det_guard = det.lock().unwrap();
                                let mut rec_guard = rec.lock().unwrap();
                                let lines = match crate::ocr::recognize_lines_impl(
                                    &mut det_guard,
                                    &mut rec_guard,
                                    oframe,
                                    rect,
                                    false,
                                ) {
                                    Ok(l) => l,
                                    Err(e) => {
                                        result["pass"] = serde_json::json!(false);
                                        result["error"] = serde_json::json!(e);
                                        results.push(result);
                                        continue;
                                    }
                                };
                                let expected = item.ocr_text.as_deref().unwrap_or("").trim();
                                let got: Vec<(String, f32)> = lines
                                    .iter()
                                    .map(|l| (l.text.clone(), l.score))
                                    .collect();
                                let hit = got
                                    .iter()
                                    .find(|(t, _)| oas_runtime::ocr_text_matches(expected, t));
                                result["text"] = serde_json::json!(
                                    hit.map(|(t, _)| t.clone()).or_else(|| got.first().map(|(t, _)| t.clone()))
                                );
                                result["score"] = serde_json::json!(
                                    hit.map(|(_, s)| *s).or_else(|| got.first().map(|(_, s)| *s))
                                );
                                result["pass"] = serde_json::json!(hit.is_some());
                                results.push(result);
                                continue;
                            }
                            let Some(engine) = self.ocr.as_ref() else {
                                result["pass"] = serde_json::json!(false);
                                result["error"] = serde_json::json!("no OCR model loaded");
                                results.push(result);
                                continue;
                            };
                            let Some(roi) = item.roi else {
                                result["pass"] = serde_json::json!(false);
                                result["error"] = serde_json::json!("OCR asset has no roi");
                                results.push(result);
                                continue;
                            };
                            let rect = oas_types::Rect {
                                x: roi[0],
                                y: roi[1],
                                w: roi[2],
                                h: roi[3],
                            };
                            let poly = item.polygon.as_ref().map(|pts| {
                                pts.iter()
                                    .map(|p| oas_types::Point { x: p[0], y: p[1] })
                                    .collect::<Vec<_>>()
                            });
                            let masked;
                            let oframe = if let Some(poly) = poly.as_ref() {
                                masked = oas_runtime::mask_frame_for_ocr(frame, rect, poly);
                                &masked
                            } else {
                                frame
                            };
                            let crop = match oas_ocr::RgbCrop::from_rgba(
                                oframe.width,
                                oframe.height,
                                &oframe.rgba,
                                rect,
                            ) {
                                Ok(c) => c,
                                Err(e) => {
                                    result["pass"] = serde_json::json!(false);
                                    result["error"] = serde_json::json!(format!("crop: {e}"));
                                    results.push(result);
                                    continue;
                                }
                            };
                            let mut eng = engine.lock().unwrap();
                            match eng.recognize(&crop) {
                                Ok((text, score)) => {
                                    let expected = item.ocr_text.as_deref().unwrap_or("");
                                    let pass = if expected.is_empty() {
                                        !text.trim().is_empty()
                                    } else {
                                        oas_runtime::ocr_text_matches(expected.trim(), &text)
                                    };
                                    result["text"] = serde_json::json!(text);
                                    result["score"] = serde_json::json!(score);
                                    result["pass"] = serde_json::json!(pass);
                                }
                                Err(e) => {
                                    result["pass"] = serde_json::json!(false);
                                    result["error"] = serde_json::json!(e);
                                }
                            }
                        }
                        #[cfg(not(feature = "ocr"))]
                        {
                            result["pass"] = serde_json::json!(false);
                            result["error"] = serde_json::json!("batch OCR needs --features ocr");
                        }
                    }
                }
                results.push(result);
            }
        }
        let passed = results.iter().filter(|r| r["pass"] == true).count();
        let body = serde_json::json!({
            "ok": true,
            "task": if global_mode { "（全局素材库）" } else { task },
            "total": results.len(),
            "passed": passed,
            "results": results,
        });
        self.send_json(out, 200, &body.to_string())
    }

    /// `GET /task/asset/collection/match?task=&id=` — collection test using the
    /// same best-member selection as runtime `locate` (helpers shared).
    fn handle_collection_match(&self, out: &mut TcpStream, raw_path: &str) -> std::io::Result<()> {
        let q = parse_query(raw_path);
        let mut task = String::new();
        let mut id = String::new();
        for (k, v) in &q {
            match k.as_str() {
                "task" => task = v.clone(),
                "id" => id = v.clone(),
                _ => {}
            }
        }
        let global_mode = self.using_global_assets();
        if id.is_empty() || (!global_mode && task.is_empty()) {
            return self.send_errors(
                out,
                400,
                vec!["`task` and `id` query params are required".to_string()],
            );
        }
        let bundle = match self.asset_store(&task) {
            Ok(b) => b,
            Err(e) => return self.send_errors(out, 400, vec![e]),
        };
        let assets = match bundle.load_runtime_assets() {
            Ok(a) => a,
            Err(e) => return self.send_errors(out, 400, vec![e]),
        };
        let Some(members) = assets.collection_members(&id).map(|m| m.to_vec()) else {
            return self.send_errors(out, 404, vec![format!("collection `{id}` not found")]);
        };
        let frame = {
            let st = self.state.lock().unwrap();
            match st.frame.as_ref() {
                Some(f) => oas_types::Frame::new(f.width, f.height, f.rgba.clone()),
                None => return self.send_json(out, 400, r#"{"error":"no frame yet"}"#),
            }
        };
        let matcher = oas_perceive::Matcher::new(oas_perceive::GrayImage::from_frame(&frame));
        let t0 = std::time::Instant::now();
        let mut best: Option<(String, f32, Option<oas_types::Point>)> = None;
        for m in members {
            if let Some(tpl) = assets.get(&m) {
                let (score, point) = oas_runtime::locate_template(&matcher, tpl);
                if best.as_ref().map(|b| score > b.1).unwrap_or(true) {
                    best = Some((m, score, point));
                }
            }
        }
        let threshold = assets.threshold_for(&id).unwrap_or(0.9);
        let best = best.ok_or_else(|| "collection has no resolvable members".to_string());
        let body = match best {
            Ok((member, score, point)) => {
                let action = assets.click_back(&member);
                serde_json::json!({
                    "ok": true,
                    "task": task,
                    "id": id,
                    "member": member,
                    "score": score,
                    "x": point.map(|p| p.x).unwrap_or(-1),
                    "y": point.map(|p| p.y).unwrap_or(-1),
                    "threshold": threshold,
                    "pass": score >= threshold,
                    "action_roi": action.map(|r| [r.x, r.y, r.w, r.h]),
                    "elapsed_ms": t0.elapsed().as_secs_f64() * 1000.0,
                })
            }
            Err(e) => serde_json::json!({"ok": false, "errors": [e]}),
        };
        if body["ok"] == true {
            self.editor_log(&format!(
                "collection_match OK task={task} id={id} member={} score={} threshold={} pass={} elapsed_ms={}",
                body["member"], body["score"], body["threshold"], body["pass"], body["elapsed_ms"]
            ));
        } else {
            self.editor_log(&format!("collection_match FAIL task={task} id={id} body={body}"));
        }
        self.send_json(out, if body["ok"] == true { 200 } else { 404 }, &body.to_string())
    }

    /// `GET /task/asset/match?task=<name>&id=<asset>` — §22.28 行内「测试」：
    /// 用任务里**已保存的模板图**在当前帧全屏匹配（不依赖 ROI 选得准不准）.
    fn handle_asset_match(&self, out: &mut TcpStream, raw_path: &str) -> std::io::Result<()> {
        let q = parse_query(raw_path);
        let mut task = String::new();
        let mut id = String::new();
        for (k, v) in &q {
            match k.as_str() {
                "task" => task = v.clone(),
                "id" => id = v.clone(),
                _ => {}
            }
        }
        let global_mode = self.using_global_assets();
        if id.is_empty() || (!global_mode && task.is_empty()) {
            return self.send_errors(
                out,
                400,
                vec!["`task` and `id` query params are required".to_string()],
            );
        }
        if !global_mode {
            if let Err(e) = oas_task::validate_task_name(&task) {
                return self.send_errors(out, 400, vec![e]);
            }
        }
        let bundle = if global_mode {
            match self.asset_store("") {
                Ok(b) => b,
                Err(e) => return self.send_errors(out, 400, vec![e]),
            }
        } else {
            match oas_task::TaskBundle::open_named(&self.tasks_dir, &task) {
                Ok(b) => b,
                Err(e) => return self.send_errors(out, 400, vec![e]),
            }
        };
        let assets = match bundle.load_runtime_assets() {
            Ok(a) => a,
            Err(e) => return self.send_errors(out, 400, vec![e]),
        };
        let Some(tpl) = assets.get(&id) else {
            return self.send_errors(
                out,
                404,
                vec![format!(
                    "asset `{id}` is not a template (or does not exist) in task `{task}`"
                )],
            );
        };
        let frame = {
            let st = self.state.lock().unwrap();
            match st.frame.as_ref() {
                Some(f) => oas_types::Frame::new(f.width, f.height, f.rgba.clone()),
                None => return self.send_json(out, 400, r#"{"error":"no frame yet"}"#),
            }
        };
        let matcher = oas_perceive::Matcher::new(oas_perceive::GrayImage::from_frame(&frame));
        let t0 = std::time::Instant::now();
        let (score, point) = oas_runtime::locate_template(&matcher, tpl);
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        let (x, y) = point.map(|p| (p.x, p.y)).unwrap_or((-1, -1));
        let pass = score >= tpl.threshold;
        self.editor_log(&format!(
            "asset_match {} task={task} id={id} score={score:.4} threshold={:.4} point=({x},{y}) elapsed_ms={ms:.1}",
            if pass { "OK" } else { "MISS" },
            tpl.threshold
        ));
        let body = serde_json::json!({
            "ok": true,
            "task": task,
            "id": id,
            "score": score,
            "x": x,
            "y": y,
            "w": tpl.gray.width,
            "h": tpl.gray.height,
            "threshold": tpl.threshold,
            "pass": pass,
            "elapsed_ms": ms,
        });
        self.send_json(out, 200, &body.to_string())
    }

    /// POST /task/asset/collection body:
    /// `{task?, id, members:[template ids], threshold?, action_roi?}`
    fn handle_collection_save(&self, out: &mut TcpStream, body: &[u8]) -> std::io::Result<()> {
        let v: serde_json::Value = match serde_json::from_slice(body) {
            Ok(v) => v,
            Err(e) => return self.send_errors(out, 400, vec![format!("invalid JSON: {e}")]),
        };
        let global_mode = self.using_global_assets();
        let task = v.get("task").and_then(|x| x.as_str()).unwrap_or("").trim();
        let id = v.get("id").and_then(|x| x.as_str()).unwrap_or("").trim();
        if id.is_empty() || (!global_mode && task.is_empty()) {
            return self.send_errors(out, 400, vec!["`id` and `task` are required".into()]);
        }
        let members: Vec<String> = match v.get("members").and_then(|x| x.as_array()) {
            Some(arr) => arr
                .iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect(),
            None => return self.send_errors(out, 400, vec!["`members` array is required".into()]),
        };
        let threshold = v.get("threshold").and_then(|x| x.as_f64()).map(|x| x as f32);
        let action_roi = match json_opt_action_roi(&v) {
            Ok(x) => x,
            Err(e) => return self.send_errors(out, 400, vec![e]),
        };
        let bundle = if global_mode {
            match self.asset_store("") {
                Ok(b) => b,
                Err(e) => return self.send_errors(out, 400, vec![e]),
            }
        } else {
            match oas_task::TaskBundle::open_named(&self.tasks_dir, task) {
                Ok(b) => b,
                Err(e) => return self.send_errors(out, 400, vec![e]),
            }
        };
        let coll = oas_task::AssetCollection {
            id: id.to_string(),
            members,
            threshold,
            action_roi,
        };
        match bundle.save_collection(coll) {
            Ok(saved) => {
                let (rewritten, errors) =
                    self.rewrite_tasks_to_collection(&saved.id, &saved.members);
                self.send_json(
                    out,
                    200,
                    &serde_json::json!({
                        "ok": true,
                        "collection": saved,
                        "tasks_rewritten": rewritten,
                        "rewrite_errors": errors,
                    })
                    .to_string(),
                )
            }
            Err(e) => self.send_errors(out, 400, vec![e]),
        }
    }

    /// POST /task/asset/collection/delete body `{task?, id}`.
    fn handle_collection_delete(&self, out: &mut TcpStream, body: &[u8]) -> std::io::Result<()> {
        let v: serde_json::Value = match serde_json::from_slice(body) {
            Ok(v) => v,
            Err(e) => return self.send_errors(out, 400, vec![format!("invalid JSON: {e}")]),
        };
        let global_mode = self.using_global_assets();
        let task = v.get("task").and_then(|x| x.as_str()).unwrap_or("").trim();
        let id = v.get("id").and_then(|x| x.as_str()).unwrap_or("").trim();
        if id.is_empty() || (!global_mode && task.is_empty()) {
            return self.send_errors(out, 400, vec!["`id` and `task` are required".into()]);
        }
        let bundle = if global_mode {
            match self.asset_store("") {
                Ok(b) => b,
                Err(e) => return self.send_errors(out, 400, vec![e]),
            }
        } else {
            match oas_task::TaskBundle::open_named(&self.tasks_dir, task) {
                Ok(b) => b,
                Err(e) => return self.send_errors(out, 400, vec![e]),
            }
        };
        match bundle.delete_collection(id) {
            Ok(item) => self.send_json(
                out,
                200,
                &serde_json::json!({"ok": true, "collection": item}).to_string(),
            ),
            Err(e) => self.send_errors(out, 400, vec![e]),
        }
    }

    /// v1.1 asset CRUD: `POST /task/asset/delete` body `{task,id}`.
    /// v1.2: all asset/object ids visible to this editor (every task + global
    /// library), used for the global-unique naming contract.
    fn global_asset_entries(&self) -> Result<Vec<(String, String)>, String> {
        let mut out = Vec::new();
        let names = oas_task::list_tasks(&self.tasks_dir)?;
        for name in names {
            let Ok(bundle) = oas_task::TaskBundle::open_named(&self.tasks_dir, &name) else {
                continue;
            };
            let Ok(af) = bundle.load_assets_file() else {
                continue;
            };
            for item in af.items {
                out.push((item.id, format!("task:{name}")));
            }
            for coll in af.collections {
                out.push((coll.id, format!("task:{name}")));
            }
        }
        if self.using_global_assets() {
            if let Ok(store) = crate::global_assets::open_store(&self.assets_dir) {
                if let Ok(af) = store.load_assets_file() {
                    for item in af.items {
                        out.push((item.id, "global".to_string()));
                    }
                    for coll in af.collections {
                        out.push((coll.id, "global".to_string()));
                    }
                }
            }
        }
        let lib = crate::library::Library::open(&self.library_dir);
        if let Ok(items) = lib.list() {
            for item in items {
                out.push((item.id, "library".to_string()));
            }
        }
        Ok(out)
    }

    /// v1.2.1: one-time migration of all task assets + old library into the
    /// unified global store. Auto-suffixes duplicate ids and rewrites each task
    /// flow with its own old→new mapping.
    fn handle_assets_migrate(&self, out: &mut TcpStream) -> std::io::Result<()> {
        match crate::global_assets::migrate(&self.tasks_dir, &self.library_dir, &self.assets_dir) {
            Ok(report) => {
                self.editor_log(&format!(
                    "assets_migrate assets={} tasks={} renamed={}",
                    report.assets,
                    report.tasks,
                    report.renamed.len()
                ));
                self.send_json(out, 200, &serde_json::json!({"ok": true, "report": report}).to_string())
            }
            Err(e) => self.send_errors(out, 400, vec![e]),
        }
    }

    /// v1.2: report duplicate ids across tasks/library (migration scan).
    fn handle_asset_conflicts(&self, out: &mut TcpStream) -> std::io::Result<()> {
        if self.using_global_assets() {
            // Unified store: ids are unique by construction; legacy task asset
            // files are no longer the active namespace.
            return self.send_json(
                out,
                200,
                &serde_json::json!({"ok": true, "conflicts": []}).to_string(),
            );
        }
        let entries = match self.global_asset_entries() {
            Ok(v) => v,
            Err(e) => return self.send_errors(out, 500, vec![e]),
        };
        let mut by_id: std::collections::BTreeMap<String, Vec<String>> =
            std::collections::BTreeMap::new();
        for (id, source) in entries {
            by_id.entry(id).or_default().push(source);
        }
        let conflicts: Vec<serde_json::Value> = by_id
            .into_iter()
            .filter(|(_, sources)| sources.len() > 1)
            .map(|(id, sources)| serde_json::json!({"id": id, "sources": sources}))
            .collect();
        self.send_json(
            out,
            200,
            &serde_json::json!({"ok": true, "conflicts": conflicts}).to_string(),
        )
    }

    /// v1.2: batch save recognized candidates. Body:
    /// `{task, assets:[{id,kind,roi,action_roi?,threshold?}]}`
    ///
    /// All ids are checked against the **global** namespace (all tasks + library)
    /// before anything is written; the batch is all-or-nothing.
    fn handle_save_assets(&self, out: &mut TcpStream, body: &[u8]) -> std::io::Result<()> {
        let v: serde_json::Value = match serde_json::from_slice(body) {
            Ok(v) => v,
            Err(e) => return self.send_errors(out, 400, vec![format!("invalid JSON: {e}")]),
        };
        let global_mode = self.using_global_assets();
        let task = v.get("task").and_then(|x| x.as_str()).unwrap_or("").trim();
        let arr = match v.get("assets").and_then(|x| x.as_array()) {
            Some(a) => a.clone(),
            None => return self.send_errors(out, 400, vec!["`assets` array is required".into()]),
        };
        if arr.is_empty() {
            return self.send_errors(out, 400, vec!["`assets` cannot be empty".into()]);
        }
        if !global_mode && task.is_empty() {
            return self.send_errors(out, 400, vec!["`task` is required".into()]);
        }
        let bundle = if global_mode {
            match self.asset_store("") {
                Ok(b) => b,
                Err(e) => return self.send_errors(out, 400, vec![e]),
            }
        } else {
            match oas_task::TaskBundle::open_named(&self.tasks_dir, task) {
                Ok(b) => b,
                Err(e) => return self.send_errors(out, 400, vec![e]),
            }
        };
        let global = if global_mode {
            Vec::new()
        } else {
            match self.global_asset_entries() {
                Ok(v) => v,
                Err(e) => return self.send_errors(out, 500, vec![e]),
            }
        };
        let mut decls = Vec::with_capacity(arr.len());
        let mut errors: Vec<String> = Vec::new();
        let mut batch_ids = std::collections::BTreeSet::new();
        for (i, item) in arr.iter().enumerate() {
            let p = format!("assets[{i}]");
            let id = item.get("id").and_then(|x| x.as_str()).unwrap_or("").trim();
            if let Err(e) = oas_task::validate_id(id) {
                errors.push(format!("{p}.id: {e}"));
                continue;
            }
            if !batch_ids.insert(id.to_string()) {
                errors.push(format!("{p}.id: duplicate `{id}` in this batch"));
            }
            if let Some((_, source)) = global.iter().find(|(gid, _)| gid == id) {
                errors.push(format!(
                    "{p}.id: `{id}` already exists ({source}); names must be globally unique"
                ));
            }
            let kind = item.get("kind").and_then(|x| x.as_str()).unwrap_or("");
            if !oas_task::KINDS.contains(&kind) {
                errors.push(format!(
                    "{p}.kind: unknown `{kind}` (allowed: {})",
                    oas_task::KINDS.join(", ")
                ));
                continue;
            }
            let roi = if item.get("roi").is_some() {
                match json_roi_field(item) {
                    Ok(r) => Some(r),
                    Err(e) => {
                        errors.push(format!("{p}.roi: {e}"));
                        continue;
                    }
                }
            } else {
                None
            };
            let action_roi = match json_opt_action_roi(item) {
                Ok(r) => r,
                Err(e) => {
                    errors.push(format!("{p}.action_roi: {e}"));
                    continue;
                }
            };
            let threshold = item.get("threshold").and_then(|x| x.as_f64()).map(|t| t as f32);
            let ocr_mode = item.get("ocr_mode").and_then(|x| x.as_str()).map(|s| s.to_string());
            let ocr_source_roi = match item.get("ocr_source_roi") {
                None => None,
                Some(x) => {
                    let arr = x.as_array().filter(|a| a.len() == 4);
                    let Some(a) = arr else {
                        errors.push(format!("{p}.ocr_source_roi: must be [x,y,w,h]"));
                        continue;
                    };
                    let mut r = [0i32; 4];
                    let mut ok = true;
                    for (j, v) in a.iter().enumerate() {
                        match v.as_i64() {
                            Some(n) => r[j] = n as i32,
                            None => ok = false,
                        }
                    }
                    if !ok {
                        errors.push(format!("{p}.ocr_source_roi: values must be integers"));
                        continue;
                    }
                    Some(r)
                }
            };
            let ocr_text = item.get("ocr_text").and_then(|x| x.as_str()).map(|s| s.to_string());
            let positions = match item.get("positions") {
                None => Vec::new(),
                Some(x) => match x.as_array() {
                    Some(a) => {
                        let mut ids = Vec::with_capacity(a.len());
                        let mut ok = true;
                        for (j, v) in a.iter().enumerate() {
                            match v.as_u64() {
                                Some(n) => ids.push(n),
                                None => {
                                    errors.push(format!("{p}.positions[{j}]: must be a frame id (u64)"));
                                    ok = false;
                                }
                            }
                        }
                        if !ok { continue; }
                        ids
                    }
                    None => {
                        errors.push(format!("{p}.positions: must be an array of frame ids"));
                        continue;
                    }
                },
            };
            if positions.is_empty() {
                errors.push(format!("{p}.positions: 添加素材必须选择至少一个位置帧"));
                continue;
            }
            decls.push(oas_task::AssetDecl {
                id: id.to_string(),
                kind: kind.to_string(),
                file: None,
                roi,
                action_roi,
                ocr_mode,
                ocr_source_roi,
                ocr_text,
                threshold,
                polygon: None,
                rev: None,
                positions,
            });
        }
        if !errors.is_empty() {
            return self.send_errors(out, 400, errors);
        }
        let frame = self.state.lock().unwrap().frame.clone();
        match bundle.save_assets_batch(decls, frame.as_ref()) {
            Ok(saved) => {
                self.editor_log(&format!("assets_batch_save task={task} count={}", saved.len()));
                self.send_json(
                    out,
                    200,
                    &serde_json::json!({"ok": true, "saved": saved}).to_string(),
                )
            }
            Err(e) => self.send_errors(out, 400, vec![e]),
        }
    }

    fn handle_asset_delete(&self, out: &mut TcpStream, body: &[u8]) -> std::io::Result<()> {
        let v: serde_json::Value = match serde_json::from_slice(body) {
            Ok(v) => v,
            Err(e) => return self.send_errors(out, 400, vec![format!("invalid JSON: {e}")]),
        };
        let global_mode = self.using_global_assets();
        let task = v.get("task").and_then(|x| x.as_str()).unwrap_or("").trim();
        let id = v.get("id").and_then(|x| x.as_str()).unwrap_or("").trim();
        if id.is_empty() || (!global_mode && task.is_empty()) {
            return self.send_errors(out, 400, vec!["`task` and `id` are required".into()]);
        }
        if global_mode {
            let refs = self.asset_referrers(id);
            if !refs.is_empty() {
                return self.send_errors(
                    out,
                    400,
                    vec![format!(
                        "素材「{id}」仍被以下任务引用：{}；请先在任务构建中删除/替换这些引用，再删除素材",
                        refs.join("、")
                    )],
                );
            }
        }
        let b = if global_mode {
            match self.asset_store("") {
                Ok(b) => b,
                Err(e) => return self.send_errors(out, 400, vec![e]),
            }
        } else {
            match oas_task::TaskBundle::open_named(&self.tasks_dir, task) {
                Ok(b) => b,
                Err(e) => return self.send_errors(out, 400, vec![e]),
            }
        };
        match b.delete_asset(id) {
            Ok(item) => {
                self.editor_log(&format!("asset_delete task={task} id={id}"));
                self.send_json(out, 200, &serde_json::json!({"ok": true, "item": item}).to_string())
            }
            Err(e) => self.send_errors(out, 400, vec![e]),
        }
    }

    /// v1.1 asset CRUD: `POST /task/asset/rename` body `{task,from,to}`.
    fn handle_asset_rename(&self, out: &mut TcpStream, body: &[u8]) -> std::io::Result<()> {
        let v: serde_json::Value = match serde_json::from_slice(body) {
            Ok(v) => v,
            Err(e) => return self.send_errors(out, 400, vec![format!("invalid JSON: {e}")]),
        };
        let global_mode = self.using_global_assets();
        let task = v.get("task").and_then(|x| x.as_str()).unwrap_or("").trim();
        let from = v.get("from").and_then(|x| x.as_str()).unwrap_or("").trim();
        let to = v.get("to").and_then(|x| x.as_str()).unwrap_or("").trim();
        if from.is_empty() || to.is_empty() || (!global_mode && task.is_empty()) {
            return self.send_errors(out, 400, vec!["`task`, `from`, `to` are required".into()]);
        }
        if global_mode {
            let refs = self.asset_referrers(from);
            if !refs.is_empty() {
                return self.send_errors(
                    out,
                    400,
                    vec![format!(
                        "asset `{from}` is referenced by task(s): {}; update/rewrite those flows first",
                        refs.join(", ")
                    )],
                );
            }
        } else if let Ok(entries) = self.global_asset_entries() {
            if let Some((_, source)) = entries.iter().find(|(gid, _)| gid == to) {
                return self.send_errors(
                    out,
                    400,
                    vec![format!(
                        "asset id `{to}` already exists ({source}); names must be globally unique"
                    )],
                );
            }
        }
        let b = if global_mode {
            match self.asset_store("") {
                Ok(b) => b,
                Err(e) => return self.send_errors(out, 400, vec![e]),
            }
        } else {
            match oas_task::TaskBundle::open_named(&self.tasks_dir, task) {
                Ok(b) => b,
                Err(e) => return self.send_errors(out, 400, vec![e]),
            }
        };
        match b.rename_asset(from, to) {
            Ok(item) => {
                self.editor_log(&format!("asset_rename task={task} from={from} to={to}"));
                self.send_json(out, 200, &serde_json::json!({"ok": true, "item": item}).to_string())
            }
            Err(e) => self.send_errors(out, 400, vec![e]),
        }
    }

    /// v1.1 asset CRUD: `POST /task/asset/update` body `{task,id,roi?,threshold?}`.
    fn handle_asset_update(&self, out: &mut TcpStream, body: &[u8]) -> std::io::Result<()> {
        let v: serde_json::Value = match serde_json::from_slice(body) {
            Ok(v) => v,
            Err(e) => return self.send_errors(out, 400, vec![format!("invalid JSON: {e}")]),
        };
        let global_mode = self.using_global_assets();
        let task = v.get("task").and_then(|x| x.as_str()).unwrap_or("").trim();
        let id = v.get("id").and_then(|x| x.as_str()).unwrap_or("").trim();
        if id.is_empty() || (!global_mode && task.is_empty()) {
            return self.send_errors(out, 400, vec!["`task` and `id` are required".into()]);
        }
        let roi = match v.get("roi") {
            None => None,
            Some(x) => {
                let arr = x.as_array().filter(|a| a.len() == 4);
                let Some(a) = arr else {
                    return self.send_errors(out, 400, vec!["`roi` must be [x,y,w,h]".into()]);
                };
                let mut r = [0i32; 4];
                for (i, item) in a.iter().enumerate() {
                    let Some(n) = item.as_i64() else {
                        return self.send_errors(out, 400, vec!["`roi` values must be integers".into()]);
                    };
                    if !(i32::MIN as i64..=i32::MAX as i64).contains(&n) {
                        return self.send_errors(out, 400, vec!["`roi` value out of i32 range".into()]);
                    }
                    r[i] = n as i32;
                }
                Some(r)
            }
        };
        let threshold = match v.get("threshold") {
            None => None,
            Some(x) => match x.as_f64() {
                Some(t) => Some(t as f32),
                None => return self.send_errors(out, 400, vec!["`threshold` must be a number".into()]),
            },
        };
        let b = if global_mode {
            match self.asset_store("") {
                Ok(b) => b,
                Err(e) => return self.send_errors(out, 400, vec![e]),
            }
        } else {
            match oas_task::TaskBundle::open_named(&self.tasks_dir, task) {
                Ok(b) => b,
                Err(e) => return self.send_errors(out, 400, vec![e]),
            }
        };
        let action_roi = match json_opt_action_roi(&v) {
            Ok(r) => r,
            Err(e) => return self.send_errors(out, 400, vec![e]),
        };
        match b.update_asset_meta(id, roi, action_roi, threshold) {
            Ok(item) => {
                self.editor_log(&format!("asset_update task={task} id={id}"));
                self.send_json(out, 200, &serde_json::json!({"ok": true, "item": item}).to_string())
            }
            Err(e) => self.send_errors(out, 400, vec![e]),
        }
    }

    /// v1.2.3: replace an asset's frame positions.
    /// body `{task?, id, positions:[frame_id,...]}`.
    fn handle_asset_positions(
        &self,
        out: &mut TcpStream,
        body: &[u8],
    ) -> std::io::Result<()> {
        let v: serde_json::Value = match serde_json::from_slice(body) {
            Ok(v) => v,
            Err(e) => return self.send_errors(out, 400, vec![format!("invalid JSON: {e}")]),
        };
        let global_mode = self.using_global_assets();
        let task = v.get("task").and_then(|x| x.as_str()).unwrap_or("").trim();
        let id = v.get("id").and_then(|x| x.as_str()).unwrap_or("").trim();
        if id.is_empty() || (!global_mode && task.is_empty()) {
            return self.send_errors(out, 400, vec!["`task` and `id` are required".into()]);
        }
        let positions = match v.get("positions").and_then(|x| x.as_array()) {
            Some(arr) => {
                let mut ids = Vec::with_capacity(arr.len());
                for (i, item) in arr.iter().enumerate() {
                    let Some(n) = item.as_u64() else {
                        return self.send_errors(
                            out,
                            400,
                            vec![format!("`positions[{i}]` must be a frame id (u64)")],
                        );
                    };
                    ids.push(n);
                }
                ids
            }
            None => {
                return self.send_errors(
                    out,
                    400,
                    vec!["`positions` must be an array of frame ids".to_string()],
                )
            }
        };
        let bundle = if global_mode {
            match self.asset_store("") {
                Ok(b) => b,
                Err(e) => return self.send_errors(out, 400, vec![e]),
            }
        } else {
            match oas_task::TaskBundle::open_named(&self.tasks_dir, task) {
                Ok(b) => b,
                Err(e) => return self.send_errors(out, 400, vec![e]),
            }
        };
        match bundle.update_asset_positions(id, &positions) {
            Ok(item) => self.send_json(
                out,
                200,
                &serde_json::json!({"ok": true, "item": item}).to_string(),
            ),
            Err(e) => self.send_errors(out, 400, vec![e]),
        }
    }

    /// POST /task/goal body `{task, goal:{mode,count?}}`.
    ///
    /// Stores the task-level completion condition in `meta.json`; for
    /// `mode=battles` it also ensures `params.max_battles` exists so the flow
    /// can use `loop n=param('max_battles')`.
    fn handle_task_goal(&self, out: &mut TcpStream, body: &[u8]) -> std::io::Result<()> {
        let v: serde_json::Value = match serde_json::from_slice(body) {
            Ok(v) => v,
            Err(e) => return self.send_errors(out, 400, vec![format!("invalid JSON: {e}")]),
        };
        let task = v.get("task").and_then(|x| x.as_str()).unwrap_or("").trim();
        if task.is_empty() {
            return self.send_errors(out, 400, vec!["`task` is required".into()]);
        }
        if let Err(e) = oas_task::validate_task_name(task) {
            return self.send_errors(out, 400, vec![e]);
        }
        let goal = match v.get("goal") {
            Some(g) if g.is_object() => g.clone(),
            _ => return self.send_errors(out, 400, vec!["`goal` object is required".into()]),
        };
        let mode = goal.get("mode").and_then(|x| x.as_str()).unwrap_or("");
        let count = goal.get("count").and_then(|x| x.as_u64());
        match mode {
            "battles" => {
                let Some(n) = count else {
                    return self.send_errors(out, 400, vec!["goal.mode=battles needs count".into()]);
                };
                if !(1..=999).contains(&n) {
                    return self.send_errors(out, 400, vec!["goal.count must be 1..=999".into()]);
                }
            }
            "until_resource" | "until_condition" => {}
            other => {
                return self.send_errors(
                    out,
                    400,
                    vec![format!(
                        "unknown goal mode `{other}` (allowed: battles, until_resource, until_condition)"
                    )],
                )
            }
        }
        let path = self.tasks_dir.join(task).join("meta.json");
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => return self.send_errors(out, 400, vec![format!("read meta.json: {e}")]),
        };
        let mut meta: serde_json::Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(e) => return self.send_errors(out, 400, vec![format!("meta.json invalid: {e}")]),
        };
        meta["goal"] = goal.clone();
        if mode == "battles" {
            let count = count.unwrap_or(1);
            let params = meta
                .as_object_mut()
                .unwrap()
                .entry("params")
                .or_insert_with(|| serde_json::json!([]));
            let Some(arr) = params.as_array_mut() else {
                return self.send_errors(out, 400, vec!["meta.params must be an array".into()]);
            };
            let existing = arr
                .iter()
                .position(|p| p.get("key").and_then(|k| k.as_str()) == Some("max_battles"));
            let decl = serde_json::json!({
                "key": "max_battles",
                "type": "int",
                "default": count,
                "min": 1,
                "max": 999
            });
            if let Some(i) = existing {
                arr[i] = decl;
            } else {
                arr.push(decl);
            }
        }
        let pretty = match serde_json::to_string_pretty(&meta) {
            Ok(s) => s,
            Err(e) => return self.send_errors(out, 500, vec![format!("serialize meta: {e}")]),
        };
        // validate through the typed contract before writing
        if let Err(e) = oas_task::Meta::from_json(&pretty) {
            return self.send_errors(out, 400, vec![format!("meta validation: {e}")]);
        }
        if let Err(e) = std::fs::write(&path, format!("{pretty}\n")) {
            return self.send_errors(out, 500, vec![format!("write meta.json: {e}")]);
        }
        self.editor_log(&format!("task_goal task={task} mode={mode}"));
        self.send_json(out, 200, &serde_json::json!({"ok": true, "task": task, "goal": goal}).to_string())
    }

    /// v1.1 P3: task library summary for the 任务库 page.
    fn handle_task_library(&self, out: &mut TcpStream) -> std::io::Result<()> {
        let names = match oas_task::list_tasks(&self.tasks_dir) {
            Ok(n) => n,
            Err(e) => return self.send_errors(out, 400, vec![e]),
        };
        let mut tasks = Vec::new();
        let global_count = if self.using_global_assets() {
            crate::global_assets::open_store(&self.assets_dir)
                .and_then(|s| s.load_assets_file())
                .map(|af| af.items.len())
                .ok()
        } else {
            None
        };
        for name in names {
            let bundle = match oas_task::TaskBundle::open_named(&self.tasks_dir, &name) {
                Ok(b) => b,
                Err(e) => {
                    tasks.push(serde_json::json!({"name": name, "error": e}));
                    continue;
                }
            };
            let meta = bundle.load_meta().ok().flatten();
            let assets = global_count.unwrap_or_else(|| {
                bundle
                    .load_assets_file()
                    .map(|a| a.items.len())
                    .unwrap_or(0)
            });
            let mtime_ms = std::fs::metadata(self.tasks_dir.join(&name).join("meta.json"))
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            let display = meta.as_ref().map(|m| m.name.clone()).unwrap_or(name.clone());
            let category = meta
                .as_ref()
                .and_then(|m| m.category.clone())
                .or_else(|| meta.as_ref().and_then(|m| m.expire.clone()))
                .unwrap_or_default();
            let description = meta
                .as_ref()
                .and_then(|m| m.description.clone())
                .unwrap_or_default();
            let goal = meta
                .as_ref()
                .and_then(|m| m.goal.clone())
                .map(|g| serde_json::json!({"mode": g.mode, "count": g.count}))
                .unwrap_or(serde_json::Value::Null);
            tasks.push(serde_json::json!({
                "name": name,
                "display_name": display,
                "category": category,
                "description": description,
                "assets": assets,
                "goal": goal,
                "mtime_ms": mtime_ms,
            }));
        }
        self.send_json(out, 200, &serde_json::json!({"ok": true, "tasks": tasks}).to_string())
    }

    /// v1.1 P3: delete a task bundle from the library (UI asks for confirmation).
    fn handle_task_delete(&self, out: &mut TcpStream, body: &[u8]) -> std::io::Result<()> {
        let v: serde_json::Value = match serde_json::from_slice(body) {
            Ok(v) => v,
            Err(e) => return self.send_errors(out, 400, vec![format!("invalid JSON: {e}")]),
        };
        let task = v.get("task").and_then(|x| x.as_str()).unwrap_or("").trim();
        if let Err(e) = oas_task::validate_task_name(task) {
            return self.send_errors(out, 400, vec![e]);
        }
        let root = self.tasks_dir.join(task);
        let root = match std::fs::canonicalize(&root) {
            Ok(p) => p,
            Err(e) => return self.send_errors(out, 400, vec![format!("task `{task}`: {e}")]),
        };
        let tasks_root = match std::fs::canonicalize(&self.tasks_dir) {
            Ok(p) => p,
            Err(e) => return self.send_errors(out, 500, vec![format!("tasks dir: {e}")]),
        };
        if root.parent() != Some(tasks_root.as_path()) {
            return self.send_errors(out, 400, vec!["refusing to delete outside tasks dir".into()]);
        }
        if let Err(e) = std::fs::remove_dir_all(&root) {
            return self.send_errors(out, 500, vec![format!("delete task `{task}`: {e}")]);
        }
        let pruned = self.prune_scheduler_task(task).unwrap_or(0);
        self.editor_log(&format!("task_delete task={task} scheduler_entries_removed={pruned}"));
        self.send_json(
            out,
            200,
            &serde_json::json!({"ok": true, "task": task, "scheduler_entries_removed": pruned}).to_string(),
        )
    }

    /// Remove scheduler entries that point at a deleted task. Keeping stale
    /// entries is confusing (the schedule page still lists deleted tasks) and
    /// dangerous (the daemon would try to open a missing bundle).
    fn prune_scheduler_task(&self, task: &str) -> Result<usize, String> {
        let path = self.scheduler_options().config;
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
            Err(e) => return Err(format!("read {}: {e}", path.display())),
        };
        let mut v: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        let Some(entries) = v.get_mut("entries").and_then(|x| x.as_array_mut()) else {
            return Ok(0);
        };
        let before = entries.len();
        entries.retain(|e| e.get("task").and_then(|x| x.as_str()) != Some(task));
        let removed = before - entries.len();
        if removed > 0 {
            let text = serde_json::to_string_pretty(&v)
                .map_err(|e| format!("serialize {}: {e}", path.display()))?;
            std::fs::write(&path, format!("{text}\n"))
                .map_err(|e| format!("write {}: {e}", path.display()))?;
        }
        Ok(removed)
    }

    /// v1.1 P3: current deployment config (settings page).
    fn handle_config(&self, out: &mut TcpStream) -> std::io::Result<()> {
        match crate::config::global() {
            Some(cfg) => {
                let body = serde_json::json!({"ok": true, "config": cfg});
                self.send_json(out, 200, &body.to_string())
            }
            None => self.send_errors(
                out,
                400,
                vec!["no --config loaded (start serve with --config config/live.json)".into()],
            ),
        }
    }

    /// v1.1 P3: active instance view (single active device in this process).
    fn handle_instance(&self, out: &mut TcpStream) -> std::io::Result<()> {
        let cfg = crate::config::global();
        let st = self.state.lock().unwrap();
        let body = serde_json::json!({
            "ok": true,
            "active": true,
            "name": "本机实例",
            "device_serial": cfg.map(|c| c.device.serial.clone()).unwrap_or_default(),
            "adb_path": cfg.map(|c| c.device.adb_path.clone()).unwrap_or_default(),
            "serve_port": cfg.map(|c| c.serve.port).unwrap_or(0),
            "bind": cfg.map(|c| c.serve.bind.clone()).unwrap_or_default(),
            "state": {
                "captured": st.png.is_some(),
                "device": st.device,
                "error": st.error,
            },
        });
        self.send_json(out, 200, &body.to_string())
    }

    /// GET /instance/devices — attached physical devices with every adb alias.
    /// The settings UI uses this dropdown so the user picks one instance by
    /// human-readable label instead of typing a serial they cannot see.
    fn handle_instance_devices(&self, out: &mut TcpStream) -> std::io::Result<()> {
        match self.adb.devices() {
            Ok(devices) => {
                let rows: Vec<serde_json::Value> = devices
                    .iter()
                    .map(|d| {
                        serde_json::json!({
                            "serial": d.serial,
                            "aliases": d.aliases,
                            "unique": d.unique,
                            "label": d.label(),
                        })
                    })
                    .collect();
                let body = serde_json::json!({
                    "ok": true,
                    "configured": crate::config::device_serial().unwrap_or_default(),
                    "devices": rows,
                });
                self.send_json(out, 200, &body.to_string())
            }
            Err(e) => self.send_errors(out, 400, vec![format!("adb devices: {e}")]),
        }
    }

    /// v1.1 P3: update the active instance's serial/adb path in `config/live.json`.
    ///
    /// The running process keeps its already-applied env; the response says
    /// restart is required (the UI shows that clearly).
    fn handle_instance_update(&self, out: &mut TcpStream, body: &[u8]) -> std::io::Result<()> {
        let v: serde_json::Value = match serde_json::from_slice(body) {
            Ok(v) => v,
            Err(e) => return self.send_errors(out, 400, vec![format!("invalid JSON: {e}")]),
        };
        let Some(path) = crate::config::global_path() else {
            return self.send_errors(
                out,
                400,
                vec!["no --config file loaded; start serve with --config config/live.json".into()],
            );
        };
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) => return self.send_errors(out, 500, vec![format!("read config: {e}")]),
        };
        let mut cfg: crate::config::OasConfig = match serde_json::from_str(&text) {
            Ok(c) => c,
            Err(e) => return self.send_errors(out, 500, vec![format!("parse config: {e}")]),
        };
        if let Some(serial) = v.get("device_serial").and_then(|x| x.as_str()) {
            let serial = serial.trim();
            if serial.is_empty() || serial.contains(char::is_whitespace) {
                return self.send_errors(out, 400, vec!["device_serial must be non-empty without spaces".into()]);
            }
            cfg.device.serial = serial.to_string();
        }
        if let Some(adb) = v.get("adb_path").and_then(|x| x.as_str()) {
            let adb = adb.trim();
            if !adb.is_empty() && !std::path::Path::new(adb).is_file() {
                return self.send_errors(out, 400, vec![format!("adb_path does not exist: {adb}")]);
            }
            cfg.device.adb_path = adb.to_string();
        }
        let pretty = match serde_json::to_string_pretty(&cfg) {
            Ok(s) => s,
            Err(e) => return self.send_errors(out, 500, vec![format!("serialize config: {e}")]),
        };
        if let Err(e) = std::fs::write(path, format!("{pretty}\n")) {
            return self.send_errors(out, 500, vec![format!("write config: {e}")]);
        }
        self.editor_log(&format!("instance_update serial={}", cfg.device.serial));
        self.send_json(
            out,
            200,
            &serde_json::json!({
                "ok": true,
                "device_serial": cfg.device.serial,
                "adb_path": cfg.device.adb_path,
                "restart_required": true
            })
            .to_string(),
        )
    }

    fn handle_task_assets(&self, out: &mut TcpStream, raw_path: &str) -> std::io::Result<()> {
        let global_mode = self.using_global_assets();
        let q = parse_query(raw_path);
        let task = q
            .iter()
            .find(|(k, _)| k == "task")
            .map(|(_, v)| v.clone())
            .unwrap_or_default();
        if !global_mode {
            if task.is_empty() {
                return self.send_errors(out, 400, vec!["missing query param `task`".to_string()]);
            }
            if let Err(e) = oas_task::validate_task_name(&task) {
                return self.send_errors(out, 400, vec![e]);
            }
        }
        let bundle = if global_mode {
            match self.asset_store("") {
                Ok(b) => b,
                Err(e) => return self.send_errors(out, 400, vec![e]),
            }
        } else {
            match oas_task::TaskBundle::open_named(&self.tasks_dir, &task) {
                Ok(b) => b,
                Err(e) => return self.send_errors(out, 400, vec![e]),
            }
        };
        match bundle.load_assets_file() {
            Ok(af) => {
                let count = af.items.len() + af.collections.len();
                // v1.2.3: assets inherit the category of their first position
                // frame, so the asset library groups exactly like 帧管理.
                let frame_cats: std::collections::BTreeMap<u64, String> = {
                    let st = self.state.lock().unwrap();
                    st.frames
                        .iter()
                        .map(|f| (f.id, f.category.clone()))
                        .collect()
                };
                let mut items_json = Vec::with_capacity(af.items.len());
                for item in af.items.into_iter() {
                    let mut v = serde_json::to_value(&item).unwrap_or_else(|_| serde_json::json!({}));
                    let cats: Vec<String> = item
                        .positions
                        .iter()
                        .filter_map(|id| frame_cats.get(id))
                        .map(|c| c.trim().to_string())
                        .filter(|c| !c.is_empty())
                        .collect();
                    let category = cats.first().cloned().unwrap_or_else(|| "未分类".to_string());
                    v["category"] = serde_json::json!(category);
                    if !cats.is_empty() {
                        v["categories"] = serde_json::json!(cats);
                    }
                    items_json.push(v);
                }
                let mut collections_json = Vec::with_capacity(af.collections.len());
                for c in af.collections.into_iter() {
                    let mut v = serde_json::to_value(&c).unwrap_or_else(|_| serde_json::json!({}));
                    v["category"] = serde_json::json!("素材集合");
                    collections_json.push(v);
                }
                let body = serde_json::json!({
                    "ok": true,
                    "task": if global_mode { "（全局素材库）".to_string() } else { task },
                    "design_resolution": af.design_resolution,
                    "count": count,
                    "items": items_json,
                    "collections": collections_json,
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
        // §22.22: `lib:<id>` refs must exist in the global library.
        if let Ok(parsed) = oas_flow::Flow::from_json(&text) {
            let known = match self.library().selector_ids() {
                Ok(s) => s,
                Err(e) => return self.send_errors(out, 400, vec![e]),
            };
            let missing: Vec<String> = parsed
                .referenced_assets()
                .into_iter()
                .filter(|a| a.starts_with("lib:") && !known.contains(a))
                .collect();
            if !missing.is_empty() {
                return self.send_errors(
                    out,
                    400,
                    vec![format!(
                        "flow.json: library asset(s) not found: {} (see GET /library)",
                        missing.join(", ")
                    )],
                );
            }
        }
        let bundle = match oas_task::TaskBundle::open_named(&self.tasks_dir, &task) {
            Ok(b) => b,
            Err(e) => return self.send_errors(out, 400, vec![e]),
        };
        let result = if self.using_global_assets() {
            match self.global_asset_ids() {
                Ok(ids) => bundle.save_flow_with_ids(&text, &ids),
                Err(e) => return self.send_errors(out, 500, vec![e]),
            }
        } else {
            bundle.save_flow(&text)
        };
        match result {
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

    /// GET /runs/aggregate?scope=all|recent|latest&task=<filter> ->
    /// `{"ok":true,"scope","runs":N,"clicks":N,"distinct":N,"files":[...],"points":[...]}`
    ///
    /// Aggregation is computed on demand from the per-run JSONL files (raw data
    /// stays untouched); the UI never needs one file per observation view.
    fn handle_runs_aggregate(&self, out: &mut TcpStream, raw_path: &str) -> std::io::Result<()> {
        let q = parse_query(raw_path);
        let scope = q
            .iter()
            .find(|(k, _)| k == "scope")
            .map(|(_, v)| v.as_str())
            .unwrap_or("all");
        let task_filter = q
            .iter()
            .find(|(k, _)| k == "task")
            .map(|(_, v)| v.clone())
            .unwrap_or_default();
        let entries = match std::fs::read_dir(&self.runs_dir) {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return self.send_json(
                    out,
                    200,
                    &serde_json::json!({"ok":true,"scope":scope,"runs":0,"clicks":0,"distinct":0,"files":[],"points":[]}).to_string(),
                )
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
            if !task_filter.is_empty() && !name.contains(&task_filter) {
                continue;
            }
            found.push((name, mtime_ms(&entry)));
        }
        found.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        let take = match scope {
            "latest" => 1,
            "recent" => 10,
            _ => found.len(),
        };
        found.truncate(take.min(found.len()));
        let mut points: Vec<serde_json::Value> = Vec::new();
        let mut files: Vec<String> = Vec::new();
        let mut total_rows = 0usize;
        let mut distinct = 0usize;
        for (name, _) in &found {
            let path = self.runs_dir.join(name);
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let Ok(rows) = parse_clicks_jsonl(&text) else {
                continue;
            };
            total_rows += rows.len();
            distinct += distinct_clicks(&rows);
            files.push(name.clone());
            for r in rows {
                if points.len() >= 50_000 {
                    break;
                }
                points.push(serde_json::json!({
                    "ts_ms": r.ts_ms as u64,
                    "x": r.x,
                    "y": r.y,
                    "target_x": r.target_x,
                    "target_y": r.target_y,
                    "sel": r.sel,
                    "seed": r.seed,
                    "file": name,
                }));
            }
        }
        let body = serde_json::json!({
            "ok": true,
            "scope": scope,
            "task_filter": task_filter,
            "runs": found.len(),
            "clicks": total_rows,
            "distinct": distinct,
            "files": files,
            "points": points,
        });
        self.send_json(out, 200, &body.to_string())
    }

    /// POST /runs/delete body `{files:[...]}`: move run JSONL files to
    /// `runs/_trash/` (raw files are preserved, no permanent delete).
    fn handle_runs_delete(&self, out: &mut TcpStream, body: &[u8]) -> std::io::Result<()> {
        let v: serde_json::Value = match serde_json::from_slice(body) {
            Ok(v) => v,
            Err(e) => return self.send_errors(out, 400, vec![format!("invalid JSON: {e}")]),
        };
        let Some(arr) = v.get("files").and_then(|x| x.as_array()) else {
            return self.send_errors(out, 400, vec!["`files` must be an array".into()]);
        };
        if arr.is_empty() {
            return self.send_errors(out, 400, vec!["`files` cannot be empty".into()]);
        }
        let trash = self.runs_dir.join("_trash");
        if let Err(e) = std::fs::create_dir_all(&trash) {
            return self.send_errors(out, 500, vec![format!("create _trash: {e}")]);
        }
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let mut moved: Vec<String> = Vec::new();
        let mut errors: Vec<String> = Vec::new();
        for item in arr {
            let raw = item.as_str().unwrap_or("");
            let name = match sanitize_run_file(raw) {
                Ok(n) => n,
                Err(e) => {
                    errors.push(format!("`{raw}`: {e}"));
                    continue;
                }
            };
            let src = self.runs_dir.join(name);
            if !src.is_file() {
                errors.push(format!("`{name}`: not a run file"));
                continue;
            }
            let dst = trash.join(format!("{ts}_{name}"));
            match std::fs::rename(&src, &dst) {
                Ok(()) => moved.push(name.to_string()),
                Err(e) => errors.push(format!("`{name}`: {e}")),
            }
        }
        if !errors.is_empty() {
            return self.send_errors(
                out,
                400,
                vec![format!(
                    "moved {} file(s); errors: {}",
                    moved.len(),
                    errors.join("; ")
                )],
            );
        }
        self.editor_log(&format!("runs_delete count={} files={moved:?}", moved.len()));
        self.send_json(
            out,
            200,
            &serde_json::json!({"ok": true, "moved": moved, "trash": "_trash"}).to_string(),
        )
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

/// Parse a required `u64` field from a raw JSON POST body.
fn json_u64_field(body: &[u8], key: &str) -> Result<u64, String> {
    let v: serde_json::Value =
        serde_json::from_slice(body).map_err(|e| format!("invalid JSON body: {e}"))?;
    v.get(key)
        .and_then(|x| x.as_u64())
        .ok_or_else(|| format!("missing integer field `{key}`"))
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

/// §22.25: optional `action_roi` (operation region; legacy `roi_back`).
fn json_opt_action_roi(v: &serde_json::Value) -> Result<Option<[i32; 4]>, String> {
    let Some(x) = v.get("action_roi").or_else(|| v.get("roi_back")) else {
        return Ok(None);
    };
    let arr = x
        .as_array()
        .ok_or_else(|| "`action_roi` must be an array of 4 integers [x,y,w,h]".to_string())?;
    if arr.len() != 4 {
        return Err(format!(
            "`action_roi` must have exactly 4 integers [x,y,w,h], got {}",
            arr.len()
        ));
    }
    let mut out = [0i32; 4];
    for (i, item) in arr.iter().enumerate() {
        let n = item
            .as_i64()
            .ok_or_else(|| format!("`action_roi[{i}]` must be an integer"))?;
        out[i] = i32::try_from(n)
            .map_err(|_| format!("`action_roi[{i}]` = {n} does not fit in i32"))?;
    }
    Ok(Some(out))
}

// ---------------------------------------------------------------------------
// N38/M1: task creation + editor-triggered runs
// ---------------------------------------------------------------------------

/// Templates accepted by `POST /task/create` (the same names as M0
/// `oasd task new --template daily|loop|utility`).
const TASK_TEMPLATES: [&str; 3] = ["daily", "loop", "utility"];

/// Default executor: the one and only `run::execute` (no second interpreter).
fn default_run_executor(
    opts: &RunOptions,
    out: &mut dyn FnMut(&str),
) -> Result<RunReport, String> {
    crate::run::execute(opts, out)
}

/// Per-template defaults for a freshly created bundle, aligned with M0's
/// `templates/` (ENGINE §22.34.2 contract fields):
/// `(category, params, capabilities.duration_hint_s)`.
fn template_defaults(template: &str) -> (&'static str, serde_json::Value, u32) {
    match template {
        "loop" => (
            // ENGINE's category enum has no `loop`; M0 maps the loop template to
            // a repeatable utility task (the lifecycle lives in scheduler.json).
            "utility",
            serde_json::json!([
                {"key": "max_rounds", "type": "int", "default": 3, "min": 1, "max": 99}
            ]),
            600,
        ),
        "utility" => ("utility", serde_json::json!([]), 60),
        _ => ("daily", serde_json::json!([]), 300),
    }
}

/// A bundle that is about to be created.
struct CreateSpec<'a> {
    name: &'a str,
    template: &'a str,
    description: &'a str,
}

/// Where [`create_task_bundle_with_hook`] currently is. Test failpoints hook
/// here so every abort path can be proven to clean up after itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CreateStage {
    /// Staging directory created (visible bundle dir does not exist yet).
    Staged,
    /// `flow.json` / `assets.json` / `images/` written inside the staging dir.
    FilesWritten,
    /// `meta.json` written; `TaskBundle::validate()` + M0 `lint()` passed
    /// (still the staging dir, so a lint failure rolls back nothing visible).
    Validated,
    /// Last step before the staging dir is published as `tasks/<name>`.
    BeforeRename,
}

impl Server {
    /// `POST /task/create` — body `{name, template?, description?}`.
    ///
    /// Creates `tasks/<name>/{meta.json,flow.json,assets.json,images/}` in one
    /// shot. Nothing is visible (and nothing is overwritten) until every file is
    /// written and the generated skeleton passed `TaskBundle::validate()` plus
    /// the M0 unified-template lint: the staging directory is then renamed into
    /// place. Any failure removes the staging directory again (rollback).
    /// Duplicate/existing names answer 409.
    fn handle_task_create(&self, out: &mut TcpStream, body: &[u8]) -> std::io::Result<()> {
        let v: serde_json::Value = match serde_json::from_slice(body) {
            Ok(v) => v,
            Err(e) => {
                return self.send_errors(out, 400, vec![format!("请求体不是合法 JSON：{e}")])
            }
        };
        let name = match json_str_field(&v, "name") {
            Ok(n) => n,
            Err(e) => return self.send_errors(out, 400, vec![format!("任务名称缺失：{e}")]),
        };
        if name.is_empty() {
            return self.send_errors(out, 400, vec!["任务名称不能为空".to_string()]);
        }
        if let Err(e) = oas_task::validate_task_name(&name) {
            return self.send_errors(out, 400, vec![format!("任务名称不合法：{e}")]);
        }
        let template = match v.get("template") {
            None | Some(serde_json::Value::Null) => "daily".to_string(),
            Some(serde_json::Value::String(s)) => {
                let t = s.trim().to_string();
                if !TASK_TEMPLATES.contains(&t.as_str()) {
                    return self.send_errors(
                        out,
                        400,
                        vec![format!(
                            "未知模板「{t}」（可选：{}）",
                            TASK_TEMPLATES.join(" / ")
                        )],
                    );
                }
                t
            }
            Some(other) => {
                return self.send_errors(
                    out,
                    400,
                    vec![format!(
                        "`template` 必须是字符串（daily/loop/utility），收到 {other}"
                    )],
                )
            }
        };
        let description = match v.get("description") {
            None | Some(serde_json::Value::Null) => String::new(),
            Some(serde_json::Value::String(s)) => s.trim().to_string(),
            Some(other) => {
                return self.send_errors(
                    out,
                    400,
                    vec![format!("`description` 必须是字符串，收到 {other}")],
                )
            }
        };
        if description.chars().count() > 500 {
            return self.send_errors(out, 400, vec!["任务描述最多 500 个字符".to_string()]);
        }
        let target = self.tasks_dir.join(&name);
        if target.exists() {
            return self.send_errors(
                out,
                409,
                vec![format!("任务「{name}」已存在（不会覆盖已有任务）")],
            );
        }
        let spec = CreateSpec {
            name: &name,
            template: &template,
            description: &description,
        };
        match create_task_bundle(&self.tasks_dir, &spec) {
            Ok(task) => {
                let body = serde_json::json!({"ok": true, "task": task});
                self.send_json(out, 200, &body.to_string())
            }
            Err(e) => self.send_errors(out, 400, vec![e]),
        }
    }

    /// `POST /task/run` — body `{task, dry_run?, seed?, replay?}`.
    ///
    /// Starts the run in a background thread through [`Server::run_executor`]
    /// (the real one calls `run::execute`); the state lives in `run_state` and is
    /// polled via `GET /task/run/status`. Only one run at a time: a second start
    /// while running answers 409.
    fn handle_task_run(&self, out: &mut TcpStream, body: &[u8]) -> std::io::Result<()> {
        let v: serde_json::Value = match serde_json::from_slice(body) {
            Ok(v) => v,
            Err(e) => {
                return self.send_errors(out, 400, vec![format!("请求体不是合法 JSON：{e}")])
            }
        };
        let task = match json_str_field(&v, "task") {
            Ok(t) => t,
            Err(e) => return self.send_errors(out, 400, vec![format!("缺少任务名：{e}")]),
        };
        if let Err(e) = oas_task::validate_task_name(&task) {
            return self.send_errors(out, 400, vec![format!("任务名称不合法：{e}")]);
        }
        let dry_run = match json_opt_bool(&v, "dry_run") {
            Ok(b) => b.unwrap_or(false),
            Err(e) => return self.send_errors(out, 400, vec![e]),
        };
        let seed = match json_opt_u64(&v, "seed") {
            Ok(s) => s,
            Err(e) => return self.send_errors(out, 400, vec![e]),
        };
        let replay = match json_opt_str(&v, "replay") {
            Ok(s) => s.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
            Err(e) => return self.send_errors(out, 400, vec![e]),
        };
        if let Some(dir) = &replay {
            if !Path::new(dir).is_dir() {
                return self.send_errors(
                    out,
                    400,
                    vec![format!(
                        "回放目录不存在：{dir}（先用 `oasd run --record` 录一个回放包，或留空）"
                    )],
                );
            }
        }
        let task_dir = self.tasks_dir.join(&task);
        if !task_dir.join("meta.json").is_file() {
            return self.send_errors(
                out,
                404,
                vec![format!(
                    "任务「{task}」不存在（先在任务面板点「新建任务」或刷新任务列表）"
                )],
            );
        }
        let mode = if replay.is_some() {
            "replay"
        } else if dry_run {
            "dry-run"
        } else {
            "live"
        };
        let started_ms = now_ms();
        let cancel = Arc::new(AtomicBool::new(false));
        let step_log = Arc::new(Mutex::new(Vec::<String>::new()));
        {
            let mut st = self.run_state.lock().unwrap();
            if st.running {
                let running = st.task.clone().unwrap_or_else(|| "（未知任务）".to_string());
                drop(st);
                return self.send_errors(
                    out,
                    409,
                    vec![format!(
                        "已有任务正在运行：{running}；等它结束再试（GET /task/run/status 查看进度）"
                    )],
                );
            }
            *st = RunState {
                running: true,
                task: Some(task.clone()),
                started_ms: Some(started_ms),
                mode: Some(mode.to_string()),
                dry_run,
                replay: replay.clone(),
                seed: seed.filter(|s| *s != 0),
                cancel: Some(cancel.clone()),
                step_log: Some(step_log.clone()),
                ..RunState::default()
            };
        }

        // The exact `oasd run` equivalent: same RunOptions, same execute().
        let mut opts = RunOptions::default();
        opts.task = Some(task_dir);
        opts.replay = replay.as_ref().map(PathBuf::from);
        opts.seed = seed;
        opts.dry_run = dry_run;
        opts.library = self.library_dir.clone();
        opts.assets = self.assets_dir.clone();
        opts.model = self.ocr_model.clone();
        opts.det_model = self.det_model.clone();
        opts.cancel = Some(cancel.clone());
        opts.step_log = Some(step_log.clone());
        opts.trace_appear = true;
        // Click audit (CD2) is written even for dry/replay runs, so the editor's
        // 点击分布 panel sees editor-triggered runs exactly like CLI runs.
        opts.clicks_out = Some(
            self.runs_dir
                .join(format!("editor-{task}-{started_ms}.clicks.jsonl")),
        );

        let state = self.run_state.clone();
        let executor = self.run_executor.clone();
        let spawned = std::thread::Builder::new()
            .name("oasd-editor-run".to_string())
            .spawn(move || {
                let log_state = state.clone();
                let mut emit = move |line: &str| {
                    // Step lines are streamed via RunState.step_log; skip the
                    // duplicate batch emitted by execute() at finish.
                    if line.starts_with("step seq=") {
                        return;
                    }
                    if let Ok(mut st) = log_state.lock() {
                        st.push_log(line);
                    }
                };
                let emit_ref: &mut dyn FnMut(&str) = &mut emit;
                let result = executor(&opts, emit_ref);
                let mut st = state.lock().unwrap_or_else(|e| e.into_inner());
                st.drain_step_log();
                st.running = false;
                st.finished_ms = Some(now_ms());
                match result {
                    Ok(report) => {
                        st.exit = Some(report.exit_code());
                        st.outcome = Some(outcome_label(report.outcome()).to_string());
                        st.frame_source = Some(report.frame_source.as_str().to_string());
                        st.clicks = Some(report.clicks.len() as u64);
                        if let RunResult::Failed(m) | RunResult::Exhausted(m) = &report.result {
                            st.error = Some(m.clone());
                        }
                    }
                    Err(e) => {
                        // Setup errors are `oasd run` exit code 2, never "Failed".
                        st.exit = Some(2);
                        st.outcome = Some("Error".to_string());
                        st.error = Some(e.clone());
                        st.push_log(&format!("error={e}"));
                    }
                }
            });
        if let Err(e) = spawned {
            let mut st = self.run_state.lock().unwrap();
            st.running = false;
            st.error = Some(format!("启动后台运行线程失败：{e}"));
            drop(st);
            return self.send_errors(out, 500, vec![format!("启动后台运行线程失败：{e}")]);
        }
        let body = serde_json::json!({
            "ok": true,
            "running": true,
            "task": task,
            "started_ms": started_ms,
            "mode": mode,
            "dry_run": dry_run,
            "seed": seed,
            "replay": replay,
        });
        self.send_json(out, 200, &body.to_string())
    }

    /// `POST /task/run/stop` — request cooperative cancellation of the current
    /// editor run. The worker checks the token before each step.
    fn handle_run_stop(&self, out: &mut TcpStream) -> std::io::Result<()> {
        let mut st = self.run_state.lock().unwrap();
        if !st.running {
            return self.send_json(out, 409, r#"{"ok":false,"error":"no run is running"}"#);
        }
        let Some(cancel) = st.cancel.clone() else {
            return self.send_json(out, 409, r#"{"ok":false,"error":"run has no cancel token"}"#);
        };
        cancel.store(true, Ordering::Relaxed);
        st.push_log("[stop] cancellation requested by editor");
        let task = st.task.clone().unwrap_or_default();
        drop(st);
        self.editor_log(&format!("run stop requested task={task}"));
        self.send_json(
            out,
            200,
            r#"{"ok":true,"stop_requested":true,"message":"已请求停止当前任务"}"#,
        )
    }

    /// `GET /task/run/status` — `{running, task, started_ms, finished_ms, exit,
    /// outcome, error, log_tail[], ...}`. Always 200 (never run == empty state).
    fn handle_run_status(&self, out: &mut TcpStream) -> std::io::Result<()> {
        let mut st = self.run_state.lock().unwrap();
        st.drain_step_log();
        let start = st.log.len().saturating_sub(RUN_LOG_TAIL);
        let body = serde_json::json!({
            "ok": true,
            "running": st.running,
            "task": st.task,
            "started_ms": st.started_ms,
            "finished_ms": st.finished_ms,
            "exit": st.exit,
            "outcome": st.outcome,
            "error": st.error,
            "mode": st.mode,
            "dry_run": st.dry_run,
            "replay": st.replay,
            "seed": st.seed,
            "frame_source": st.frame_source,
            "clicks": st.clicks,
            "log_count": st.log.len(),
            "log_tail": &st.log[start..],
        });
        drop(st);
        self.send_json(out, 200, &body.to_string())
    }

    /// Scheduler options for the read-only panel: paths come from the global
    /// live config (`scheduler.plan/state`), never shell out to another process.
    fn scheduler_options(&self) -> SchedulerOptions {
        let mut o = SchedulerOptions {
            tasks_dir: self.tasks_dir.clone(),
            library: self.library_dir.clone(),
            ..SchedulerOptions::default()
        };
        if let Some(cfg) = crate::config::global() {
            if !cfg.scheduler.plan.trim().is_empty() {
                o.config = PathBuf::from(cfg.scheduler.plan.trim());
            }
            if !cfg.scheduler.state.trim().is_empty() {
                o.state = Some(PathBuf::from(cfg.scheduler.state.trim()));
            }
        }
        o
    }

    fn scheduler_reset_options(&self, body: &serde_json::Value) -> ResetOptions {
        let mut o = ResetOptions {
            tasks_dir: self.tasks_dir.clone(),
            include_checkpoint: true,
            ..ResetOptions::default()
        };
        let base = self.scheduler_options();
        o.config = base.config;
        o.state = base.state;
        o.tasks = body
            .get("tasks")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str())
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        o.all = body.get("all").and_then(|v| v.as_bool()).unwrap_or(false);
        o.include_checkpoint = body
            .get("include_checkpoint")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        o.keep_checkpoint = !o.include_checkpoint;
        o
    }

    /// `GET /scheduler/status` — read-only scheduler panel data (M2/N37).
    /// Never takes the scheduler lock; safe while the daemon runs.
    fn handle_scheduler_status(&self, out: &mut TcpStream) -> std::io::Result<()> {
        match crate::schedule::status_json(&self.scheduler_options()) {
            Ok(mut v) => {
                // A task deleted outside /task/delete (or before this cleanup
                // existed) must not keep appearing as a schedulable entry.
                let tasks: std::collections::BTreeSet<String> =
                    oas_task::list_tasks(&self.tasks_dir).unwrap_or_default().into_iter().collect();
                let task_exists = |e: &serde_json::Value| {
                    e.get("task")
                        .and_then(|x| x.as_str())
                        .map(|t| tasks.contains(t))
                        .unwrap_or(false)
                };
                if let Some(entries) = v.get_mut("entries").and_then(|x| x.as_array_mut()) {
                    entries.retain(task_exists);
                }
                if let Some(choices) = v.get_mut("task_choices").and_then(|x| x.as_array_mut()) {
                    choices.retain(task_exists);
                }
                if let Some(plan) = v.get_mut("plan").and_then(|x| x.as_object_mut()) {
                    for key in ["runnable", "skipped"] {
                        if let Some(arr) = plan.get_mut(key).and_then(|x| x.as_array_mut()) {
                            arr.retain(task_exists);
                        }
                    }
                }
                let running_missing = v
                    .get("running")
                    .map(|r| !r.is_null() && !task_exists(r))
                    .unwrap_or(false);
                if running_missing {
                    v["running"] = serde_json::Value::Null;
                }
                self.send_json(out, 200, &v.to_string())
            }
            Err(e) => self.send_errors(out, 400, vec![format!("scheduler status: {e}")]),
        }
    }

    /// `POST /scheduler/reset` — reset memory for selected tasks (all/invert
    /// selection is computed by the UI). 409 when the scheduler is running.
    fn handle_scheduler_reset(
        &self,
        out: &mut TcpStream,
        body: &[u8],
    ) -> std::io::Result<()> {
        let v: serde_json::Value = match serde_json::from_slice(body) {
            Ok(v) => v,
            Err(e) => return self.send_errors(out, 400, vec![format!("invalid JSON: {e}")]),
        };
        let opts = self.scheduler_reset_options(&v);
        if !opts.all && opts.tasks.is_empty() {
            return self.send_errors(
                out,
                400,
                vec!["请选择要重置记忆的任务（tasks）或 all=true".into()],
            );
        }
        let mut lines: Vec<String> = Vec::new();
        match crate::schedule::reset_memory(&opts, &mut |s| lines.push(s.to_string())) {
            Ok(rep) => {
                let body = serde_json::json!({
                    "ok": true,
                    "report": rep,
                    "log": lines,
                });
                self.send_json(out, 200, &body.to_string())
            }
            Err(e) => {
                let busy = e.contains("running")
                    || e.contains("运行")
                    || e.contains("占用")
                    || e.contains("lock");
                let code = if busy { 409 } else { 500 };
                self.send_errors(out, code, vec![format!("reset-memory: {e}")])
            }
        }
    }
}

/// `Outcome` -> the 3-state label `oasd run` prints (`outcome=Done|Failed|Exhausted`).
fn outcome_label(outcome: Outcome) -> &'static str {
    match outcome {
        Outcome::Done => "Done",
        Outcome::Failed => "Failed",
        Outcome::Exhausted => "Exhausted",
    }
}

/// Epoch milliseconds (same clock `oasd run` reports in `ts_ms`/seed).
fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

static CREATE_SEQ: AtomicU64 = AtomicU64::new(0);

/// Unique staging directory **inside** `tasks_dir` (same volume => the final
/// rename is atomic). The name keeps the task name so a crash leaves a clue.
fn unique_staging_dir(tasks_dir: &Path, name: &str) -> PathBuf {
    loop {
        let seq = CREATE_SEQ.fetch_add(1, Ordering::Relaxed);
        let candidate = tasks_dir.join(format!(
            ".oasd-create-{}-{seq}-{name}",
            std::process::id()
        ));
        if !candidate.exists() {
            return candidate;
        }
    }
}

/// Write `text` atomically (sibling tmp file + rename), creating the parent dir.
fn write_text_atomic(path: &Path, text: &str) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("`{}` 没有父目录", path.display()))?;
    std::fs::create_dir_all(parent)
        .map_err(|e| format!("创建目录 `{}` 失败：{e}", parent.display()))?;
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("oas-task");
    let seq = CREATE_SEQ.fetch_add(1, Ordering::Relaxed);
    let tmp = parent.join(format!(".{name}.tmp-{}-{seq}", std::process::id()));
    if let Err(e) = std::fs::write(&tmp, text) {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("写入 `{}` 失败：{e}", tmp.display()));
    }
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!(
            "重命名 `{}` -> `{}` 失败：{e}",
            tmp.display(),
            path.display()
        ));
    }
    Ok(())
}

fn json_pretty(v: &serde_json::Value) -> Result<String, String> {
    let mut text =
        serde_json::to_string_pretty(v).map_err(|e| format!("序列化 JSON 失败：{e}"))?;
    text.push('\n');
    Ok(text)
}

/// M0 contract (`task_id` syntax == task-directory-name syntax): derive a stable
/// id from the (human, Chinese-allowed) task name.
fn slugify_task_id(name: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            last_dash = false;
        } else if c.is_whitespace() || matches!(c, '-' | '_' | '.') {
            if !out.is_empty() && !last_dash {
                out.push('-');
                last_dash = true;
            }
        } else if ('\u{4e00}'..='\u{9fa5}').contains(&c) {
            out.push(c);
            last_dash = false;
        }
        if out.chars().count() >= 120 {
            break;
        }
    }
    let mut slug = out.trim_matches('-').to_string();
    if slug.is_empty() {
        return "task".to_string();
    }
    // The slug itself must be a legal task-directory name (e.g. "con" is not).
    if oas_task::validate_task_name(&slug).is_ok() {
        return slug;
    }
    slug = format!("task-{slug}");
    if slug.chars().count() > 120 {
        slug = slug.chars().take(120).collect();
    }
    if oas_task::validate_task_name(&slug).is_ok() {
        slug
    } else {
        "task".to_string()
    }
}

/// `meta.json` of a freshly created bundle: ENGINE §22.34.2 contract fields with
/// the M0 defaults (old bundles without them behave exactly like this).
fn new_task_meta(spec: &CreateSpec) -> serde_json::Value {
    let (category, params, duration_hint_s) = template_defaults(spec.template);
    let task_id = slugify_task_id(spec.name);
    let mut meta = serde_json::json!({
        "schema_version": 1,
        "template_version": 1,
        "task_id": task_id,
        "name": spec.name,
        "version": 1,
        // Old (pre-contract) field: still accepted, `category` is authoritative.
        "expire": "daily",
        "category": category,
        "params": params,
        "exits": {
            "done": {"result": "success"},
            "failed": {"result": "retryable"},
        },
        "capabilities": {
            "checkpoint": true,
            "interruptible": false,
            "duration_hint_s": duration_hint_s,
        },
        "resources": [],
    });
    if spec.template == "daily" {
        // schedule_hint only feeds UI defaults; it is never the scheduling authority.
        meta["schedule_hint"] = serde_json::json!({
            "windows": [],
            "quota": {"per": "day", "times": 1},
        });
    }
    if !spec.description.is_empty() {
        meta["description"] = serde_json::json!(spec.description);
    }
    meta
}

/// Skeleton `flow.json`: `log` + `finish("done")`.
///
/// The `loop` template adds M0's bounded `loop(param('max_rounds'))` block
/// (ENGINE §22.34.2 T1: the last top-level step is still a declared `finish`;
/// `param('max_rounds')` is declared in that template's `meta.params`).
/// `name` and `schema_version` are mandatory for the DSL loader.
fn new_task_flow(spec: &CreateSpec) -> Result<String, String> {
    // The expression language has string literals but NO escapes, so the log
    // text is built from the validated name (no `"` / control chars are legal in
    // a task name, by oas_task::validate_task_name).
    let message = format!(
        "任务「{}」骨架已创建（{} 模板）：在编辑器里添加素材/步骤后点「运行任务」",
        spec.name, spec.template
    );
    let mut steps = vec![serde_json::json!({
        "op": "log",
        "args": {"level": "info", "text": format!("\"{message}\"")},
    })];
    if spec.template == "loop" {
        steps.push(serde_json::json!({
            "op": "loop",
            // Bounded by the declared param (default 3, range 1..=99 from meta).
            "args": {"n": "param('max_rounds')"},
            "body": [{
                "op": "log",
                "args": {
                    "level": "info",
                    "text": "\"loop round（占位：请在编辑器里替换为真实操作）\"",
                },
            }],
        }));
    }
    steps.push(serde_json::json!({"op": "finish", "args": {"exit": "done"}}));
    let flow = serde_json::json!({
        "schema_version": 1,
        "name": spec.name,
        "steps": steps,
    });
    json_pretty(&flow)
}

/// `assets.json` skeleton: the editor's canonical design resolution, no items.
fn new_task_assets() -> Result<String, String> {
    json_pretty(&serde_json::json!({
        "schema_version": 1,
        "design_resolution": [1280, 720],
        "items": [],
    }))
}

/// Create one task bundle (no failpoints; production path).
fn create_task_bundle(tasks_dir: &Path, spec: &CreateSpec) -> Result<serde_json::Value, String> {
    create_task_bundle_with_hook(tasks_dir, spec, |_| Ok(()))
}

/// Create one task bundle with an injectable failpoint (tests prove rollback).
///
/// Steps: staging dir -> flow/assets/images -> meta -> `TaskBundle::validate()`
/// -> rename into `tasks/<name>`. A failure at **any** point removes the staging
/// directory again, so a failed create leaves neither a half bundle nor an
/// existing task touched. The visible directory only appears via the final
/// atomic rename of an already valid bundle.
fn create_task_bundle_with_hook<F>(
    tasks_dir: &Path,
    spec: &CreateSpec,
    mut fail: F,
) -> Result<serde_json::Value, String>
where
    F: FnMut(CreateStage) -> Result<(), String>,
{
    if spec.name.is_empty() {
        return Err("任务名称不能为空".to_string());
    }
    oas_task::validate_task_name(spec.name).map_err(|e| format!("任务名称不合法：{e}"))?;
    let final_dir = tasks_dir.join(spec.name);
    if final_dir.exists() {
        return Err(format!("任务「{}」已存在（不会覆盖已有任务）", spec.name));
    }
    let meta = new_task_meta(spec);
    let meta_text = json_pretty(&meta)?;
    let flow_text = new_task_flow(spec)?;
    let assets_text = new_task_assets()?;
    // Fail BEFORE touching the disk if our own skeleton is not loadable. The same
    // rules the runtime uses (not a second, weaker parser).
    let flow = oas_flow::Flow::from_json(&flow_text)
        .map_err(|e| format!("生成的 flow.json 不合法（内部错误）：{e}"))?;
    let no_assets = std::collections::BTreeSet::new();
    flow.validate_assets(&no_assets).map_err(|errs| {
        format!(
            "生成的 flow.json 引用校验失败（内部错误）：{}",
            errs.iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join("; ")
        )
    })?;

    std::fs::create_dir_all(tasks_dir)
        .map_err(|e| format!("创建任务根目录 `{}` 失败：{e}", tasks_dir.display()))?;
    let staging = unique_staging_dir(tasks_dir, spec.name);
    let published = std::cell::Cell::new(false);
    let result = (|| -> Result<(), String> {
        std::fs::create_dir(&staging)
            .map_err(|e| format!("创建临时目录 `{}` 失败：{e}", staging.display()))?;
        fail(CreateStage::Staged)?;
        std::fs::write(staging.join("flow.json"), &flow_text)
            .map_err(|e| format!("写入 flow.json 失败：{e}"))?;
        std::fs::write(staging.join("assets.json"), &assets_text)
            .map_err(|e| format!("写入 assets.json 失败：{e}"))?;
        std::fs::create_dir_all(staging.join("images"))
            .map_err(|e| format!("创建 images/ 失败：{e}"))?;
        fail(CreateStage::FilesWritten)?;
        write_text_atomic(&staging.join("meta.json"), &meta_text)?;
        // Pre-M0 `bundle.validate()` (old bundles: schema/name/expire + asset refs)
        // AND M0's unified-template lint (ENGINE §22.34.2 META/T1/T3/T4/T5/T6).
        // A freshly generated skeleton must satisfy both; otherwise roll back.
        let bundle = oas_task::TaskBundle::open(&staging)?;
        bundle.validate().map_err(|errs| {
            format!("生成的骨架没有通过 bundle 校验：{}", errs.join("; "))
        })?;
        bundle.lint().map_err(|issues| {
            format!(
                "生成的骨架没有通过任务 lint：{}",
                issues
                    .iter()
                    .map(|i| i.to_string())
                    .collect::<Vec<_>>()
                    .join("; ")
            )
        })?;
        fail(CreateStage::Validated)?;
        fail(CreateStage::BeforeRename)?;
        std::fs::rename(&staging, &final_dir).map_err(|e| {
            format!(
                "发布任务目录 `{}` -> `{}` 失败（是否已被占用？）：{e}",
                staging.display(),
                final_dir.display()
            )
        })?;
        published.set(true);
        Ok(())
    })();

    // Rollback: remove the staging dir on every abort path. If the rename already
    // happened the staging path no longer exists (the run is a success then).
    if result.is_err() && !published.get() {
        let _ = std::fs::remove_dir_all(&staging);
    }
    result?;
    let (category, _, _) = template_defaults(spec.template);
    Ok(serde_json::json!({
        "name": spec.name,
        "template": spec.template,
        "category": category,
        "task_id": meta["task_id"],
        "path": final_dir.display().to_string(),
        "files": ["meta.json", "flow.json", "assets.json", "images/"],
        "meta": meta,
    }))
}

/// Optional boolean JSON field (`missing`/`null` => `None`, wrong type => error).
fn json_opt_bool(v: &serde_json::Value, key: &str) -> Result<Option<bool>, String> {
    match v.get(key) {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::Bool(b)) => Ok(Some(*b)),
        Some(other) => Err(format!("`{key}` 必须是布尔值，收到 {other}")),
    }
}

/// Optional unsigned JSON field.
fn json_opt_u64(v: &serde_json::Value, key: &str) -> Result<Option<u64>, String> {
    match v.get(key) {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(x) => match x.as_u64() {
            Some(n) => Ok(Some(n)),
            None => Err(format!("`{key}` 必须是无符号整数（u64），收到 {x}")),
        },
    }
}

/// Optional string JSON field.
fn json_opt_str(v: &serde_json::Value, key: &str) -> Result<Option<String>, String> {
    match v.get(key) {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::String(s)) => Ok(Some(s.clone())),
        Some(other) => Err(format!("`{key}` 必须是字符串，收到 {other}")),
    }
}

// ---------------------------------------------------------------------------
// N4 tests: pure helpers + the real routes over a loopback socket// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::atomic::{AtomicU64, Ordering};

    #[test]
    fn collection_rewrite_maps_sel_and_expression_asset_refs() {
        let mut v = serde_json::json!({
            "steps": [
                {"op":"click","args":{"sel":"甲"}},
                {"op":"if","args":{"cond":"appear('甲') or ocr('乙') == 'x'"}},
                {"op":"find","args":{"find":[{"kind":"template","sel":"乙"}]}}
            ]
        });
        let mapping = vec![
            ("甲".to_string(), "集合".to_string()),
            ("乙".to_string(), "集合".to_string()),
        ];
        assert!(rewrite_flow_asset_refs(&mut v, &mapping));
        let text = v.to_string();
        assert!(text.contains("\"sel\":\"集合\""), "{text}");
        assert!(text.contains("appear('集合')"), "{text}");
        assert!(text.contains("ocr('集合')"), "{text}");
        assert!(!text.contains("甲"), "{text}");
        assert!(!text.contains("乙"), "{text}");
    }

    #[test]
    fn live_input_point_validation_rejects_bad_coordinates() {
        assert!(validate_live_input_point(0, 0, 1280, 720).is_ok());
        assert!(validate_live_input_point(1279, 719, 1280, 720).is_ok());
        let err = validate_live_input_point(1280, 10, 1280, 720).unwrap_err();
        assert!(err.contains("outside"), "{err}");
        let err = validate_live_input_point(-1, 10, 1280, 720).unwrap_err();
        assert!(err.contains("outside"), "{err}");
        let err = validate_live_input_point(1, 1, 0, 720).unwrap_err();
        assert!(err.contains("frame size"), "{err}");
    }

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

    /// Drive one POST through the REAL [`Server::handle`] over a loopback socket
    /// pair and return `(status line, body)`.
    fn post_route(
        server: &Server,
        target: &str,
        content_type: &str,
        body: &[u8],
    ) -> (String, String) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test listener");
        let addr = listener.local_addr().expect("local addr");
        let mut client = TcpStream::connect(addr).expect("connect");
        let (srv, _) = listener.accept().expect("accept");
        let head = format!(
            "POST {target} HTTP/1.1\r\nHost: test\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n\r\n",
            body.len()
        );
        client.write_all(head.as_bytes()).expect("write head");
        client.write_all(body).expect("write body");
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

    #[test]
    fn session_frames_upload_select_delete_roundtrip() {
        let td = TempDir::new("frames-roundtrip");
        let server = Server::new(Adb::from_env())
            .with_frames_dir(td.path().join("frames"));
        // frame A: 2x2; frame B: 3x2 (different size proves select really swaps).
        let a = Frame::new(
            2,
            2,
            vec![
                255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 200, 200, 200, 255,
            ],
        );
        let b = Frame::new(3, 2, vec![10u8; 3 * 2 * 4]);
        let png_a = oas_codec::encode_png(&a).expect("png a");
        let png_b = oas_codec::encode_png(&b).expect("png b");

        let (status, out) = post_route(&server, "/frames", "image/png", &png_a);
        assert!(status.contains("200"), "{status} {out}");
        let v: serde_json::Value = serde_json::from_str(&out).expect("upload json");
        let id_a = v.get("id").and_then(|x| x.as_u64()).expect("id a");
        assert_eq!(v.get("width").and_then(|x| x.as_u64()), Some(2));

        let (_, out) = post_route(&server, "/frames", "image/png", &png_b);
        let v: serde_json::Value = serde_json::from_str(&out).expect("upload json b");
        let id_b = v.get("id").and_then(|x| x.as_u64()).expect("id b");
        assert_eq!(v.get("height").and_then(|x| x.as_u64()), Some(2));

        let (status, out) = get_route(&server, "/frames");
        assert!(status.contains("200"), "{status} {out}");
        let v: serde_json::Value = serde_json::from_str(&out).expect("frames json");
        let frames = v.get("frames").and_then(|f| f.as_array()).expect("frames");
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[1].get("current").and_then(|c| c.as_bool()), Some(true));

        let (status, out) = post_route(
            &server,
            "/frames/select",
            "application/json",
            format!(r#"{{"id":{id_a}}}"#).as_bytes(),
        );
        assert!(status.contains("200"), "{status} {out}");
        let (_, out) = get_route(&server, "/state");
        let st: serde_json::Value = serde_json::from_str(&out).expect("state json");
        assert_eq!(st.get("captured").and_then(|c| c.as_bool()), Some(true));
        assert_eq!(st.get("width").and_then(|w| w.as_u64()), Some(2));
        assert_eq!(st.get("height").and_then(|h| h.as_u64()), Some(2));

        // deleting the current frame falls back to the remaining last frame.
        let (status, out) = post_route(
            &server,
            "/frames/delete",
            "application/json",
            format!(r#"{{"id":{id_a}}}"#).as_bytes(),
        );
        assert!(status.contains("200"), "{status} {out}");
        let (_, out) = get_route(&server, "/state");
        let st: serde_json::Value = serde_json::from_str(&out).expect("state json");
        assert_eq!(st.get("width").and_then(|w| w.as_u64()), Some(3));
        assert!(id_b > id_a);

        let (status, out) = post_route(&server, "/frames/clear", "application/json", b"{}");
        assert!(status.contains("200"), "{status} {out}");
        let (_, out) = get_route(&server, "/frames");
        let v: serde_json::Value = serde_json::from_str(&out).expect("frames json");
        assert_eq!(
            v.get("frames").and_then(|f| f.as_array()).map(|a| a.len()),
            Some(0)
        );
    }

    // -----------------------------------------------------------------------
    // N38/M1: task creation + editor-triggered run
    // -----------------------------------------------------------------------

    #[test]
    fn frames_manifest_survives_restart_and_keeps_metadata() {
        let td = TempDir::new("frames-restart");
        let frames_dir = td.path().join("frames");
        let server = Server::new(Adb::from_env()).with_frames_dir(frames_dir.clone());
        let frame = Frame::new(2, 2, vec![128u8; 2 * 2 * 4]);
        let png = oas_codec::encode_png(&frame).expect("png");
        let (status, out) = post_route(&server, "/frames", "image/png", &png);
        assert!(status.contains("200"), "{status} {out}");
        let v: serde_json::Value = serde_json::from_str(&out).expect("upload json");
        let id = v["id"].as_u64().expect("id");

        let (status, out) = post_route(
            &server,
            "/frames/rename",
            "application/json",
            format!(r#"{{"id":{id},"name":"庭院"}}"#).as_bytes(),
        );
        assert!(status.contains("200"), "{status} {out}");
        let (status, out) = post_route(
            &server,
            "/frames/category",
            "application/json",
            format!(r#"{{"id":{id},"category":"探索"}}"#).as_bytes(),
        );
        assert!(status.contains("200"), "{status} {out}");
        assert!(frames_dir.join("frames.json").is_file(), "manifest written");
        assert!(frames_dir.join(format!("frame-{id}.png")).is_file(), "png written");

        // Fresh server with the same dir: metadata and current frame load back.
        let restarted = Server::new(Adb::from_env()).with_frames_dir(frames_dir);
        let (status, out) = get_route(&restarted, "/frames");
        assert!(status.contains("200"), "{status} {out}");
        let v: serde_json::Value = serde_json::from_str(&out).expect("frames json");
        let frames = v["frames"].as_array().expect("frames");
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0]["name"], "庭院");
        assert_eq!(frames[0]["category"], "探索");
        assert_eq!(frames[0]["current"], true);
        let (_, out) = get_route(&restarted, "/state");
        let st: serde_json::Value = serde_json::from_str(&out).expect("state json");
        assert_eq!(st["captured"], true);
        assert_eq!(st["width"], 2);
    }

    fn server_with_tasks(td: &Path) -> Server {
        Server::new(Adb::from_env())
            .with_tasks_dir(td.join("tasks"))
            .with_runs_dir(td.join("runs"))
            .with_library_dir(td.join("library"))
    }

    /// Create one valid bundle in the test tasks dir (through the real helper).
    fn make_task(tasks_dir: &Path, name: &str) -> serde_json::Value {
        let spec = CreateSpec {
            name,
            template: "daily",
            description: "",
        };
        create_task_bundle(tasks_dir, &spec).expect("create a test task bundle")
    }

    fn read_json(path: &Path) -> serde_json::Value {
        serde_json::from_str(&std::fs::read_to_string(path).expect("read json file"))
            .expect("valid json")
    }

    fn source_entries(dir: &Path) -> Vec<String> {
        match std::fs::read_dir(dir) {
            Ok(rd) => rd
                .filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    fn poll_run_status(server: &Server) -> serde_json::Value {
        let (status, body) = get_route(server, "/task/run/status");
        assert!(status.contains("200"), "status route: {status} {body}");
        serde_json::from_str(&body).expect("status JSON")
    }

    fn wait_run_finished(server: &Server, timeout_ms: u64) -> serde_json::Value {
        let t0 = std::time::Instant::now();
        loop {
            let v = poll_run_status(server);
            if v["running"] == serde_json::Value::Bool(false) {
                return v;
            }
            assert!(
                t0.elapsed() < std::time::Duration::from_millis(timeout_ms),
                "run did not finish in {timeout_ms}ms: {v}"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    fn done_report() -> RunReport {
        RunReport {
            seed: 42,
            frame_source: crate::run::FrameSource::Live,
            events: Vec::new(),
            result: RunResult::Finished(oas_types::Exit::Done),
            clicks: vec![oas_types::Point { x: 10, y: 20 }],
        }
    }

    /// Blocking fake executor: proves the single-run mutex (409) and that the
    /// status route reports `running` before the result is available.
    struct Gate {
        released: Mutex<bool>,
        cv: std::sync::Condvar,
    }

    impl Gate {
        fn new() -> Self {
            Self {
                released: Mutex::new(false),
                cv: std::sync::Condvar::new(),
            }
        }

        fn wait(&self) {
            let mut g = self.released.lock().unwrap();
            while !*g {
                g = self.cv.wait(g).unwrap();
            }
        }

        fn open(&self) {
            *self.released.lock().unwrap() = true;
            self.cv.notify_all();
        }
    }

    #[test]
    fn task_create_writes_a_valid_skeleton_and_lists_it() {
        let td = TempDir::new("task-create");
        let tasks_dir = td.path().join("tasks");
        let server = server_with_tasks(td.path());
        let (status, body) = post_route(
            &server,
            "/task/create",
            "application/json",
            r#"{"name":"新建任务A","template":"daily","description":"冒烟任务"}"#
                .as_bytes(),
        );
        assert!(status.contains("200"), "{status} {body}");
        let v: serde_json::Value = serde_json::from_str(&body).expect("create JSON");
        assert_eq!(v["ok"], serde_json::Value::Bool(true));
        let task = &v["task"];
        assert_eq!(task["name"], "新建任务A");
        assert_eq!(task["template"], "daily");
        assert_eq!(task["category"], "daily");
        assert!(task["task_id"].as_str().is_some_and(|s| !s.is_empty()));

        let dir = tasks_dir.join("新建任务A");
        for f in ["meta.json", "flow.json", "assets.json"] {
            assert!(dir.join(f).is_file(), "missing {f}");
        }
        assert!(dir.join("images").is_dir(), "images/ must exist");

        let meta = read_json(&dir.join("meta.json"));
        assert_eq!(meta["schema_version"], 1);
        assert_eq!(meta["template_version"], 1);
        assert_eq!(meta["name"], "新建任务A");
        assert_eq!(meta["version"], 1);
        assert_eq!(meta["expire"], "daily");
        assert_eq!(meta["category"], "daily");
        assert_eq!(meta["params"], serde_json::json!([]));
        assert_eq!(meta["exits"]["done"]["result"], "success");
        assert_eq!(meta["exits"]["failed"]["result"], "retryable");
        assert_eq!(meta["capabilities"]["checkpoint"], true);
        assert_eq!(meta["resources"], serde_json::json!([]));
        assert_eq!(meta["description"], "冒烟任务");

        let assets = read_json(&dir.join("assets.json"));
        assert_eq!(assets["design_resolution"], serde_json::json!([1280, 720]));
        assert_eq!(assets["items"], serde_json::json!([]));

        // The skeleton is loadable by the DSL and ends in a declared exit.
        let flow_text = std::fs::read_to_string(dir.join("flow.json")).expect("flow text");
        let flow = oas_flow::Flow::from_json(&flow_text).expect("skeleton flow parses");
        assert_eq!(flow.name, "新建任务A");
        assert!(flow_text.contains("\"log\""));
        assert!(flow_text.contains("\"finish\""));

        // The same bundle validation /task/create runs after writing.
        oas_task::TaskBundle::open_named(&tasks_dir, "新建任务A")
            .expect("open bundle")
            .validate()
            .expect("skeleton bundle validates");
        oas_task::TaskBundle::open_named(&tasks_dir, "新建任务A")
            .expect("open bundle")
            .lint()
            .expect("created skeleton must pass M0 task lint");

        // GET /task/list now lists it.
        let (status, body) = get_route(&server, "/task/list");
        assert!(status.contains("200"), "{status} {body}");
        let list: serde_json::Value = serde_json::from_str(&body).expect("list JSON");
        let names: Vec<&str> = list["tasks"]
            .as_array()
            .expect("tasks array")
            .iter()
            .filter_map(|t| t.as_str())
            .collect();
        assert!(names.contains(&"新建任务A"), "{names:?}");
    }

    #[test]
    fn task_create_defaults_follow_the_m0_contract() {
        let td = TempDir::new("task-templates");
        let tasks_dir = td.path().join("tasks");
        let server = server_with_tasks(td.path());
        // (name, template, category, duration_hint_s, param key or None)
        let cases = [
            ("模板日常", "daily", "daily", 300u64, None),
            ("模板循环", "loop", "utility", 600u64, Some("max_rounds")),
            ("模板工具", "utility", "utility", 60u64, None),
        ];
        for (name, template, category, duration, param_key) in cases {
            let body = format!(r#"{{"name":"{name}","template":"{template}"}}"#);
            let (status, out) =
                post_route(&server, "/task/create", "application/json", body.as_bytes());
            assert!(status.contains("200"), "{template}: {status} {out}");
            let dir = tasks_dir.join(name);
            let meta = read_json(&dir.join("meta.json"));
            assert_eq!(meta["category"], category, "{template} category");
            assert_eq!(
                meta["capabilities"]["duration_hint_s"], duration,
                "{template} duration_hint_s"
            );
            assert_eq!(meta["exits"]["done"]["result"], "success");
            assert_eq!(meta["exits"]["failed"]["result"], "retryable");
            match param_key {
                Some(key) => {
                    let params = meta["params"].as_array().expect("params array");
                    assert_eq!(params.len(), 1, "{template} must declare {key}");
                    assert_eq!(params[0]["key"], key);
                    assert_eq!(params[0]["default"], 3);
                }
                None => assert_eq!(meta["params"], serde_json::json!([])),
            }
            let flow = oas_flow::Flow::from_json(
                &std::fs::read_to_string(dir.join("flow.json")).expect("flow"),
            )
            .expect("flow parses");
            assert_eq!(flow.name, name);
            if template == "loop" {
                assert!(
                    flow.param_uses().contains("max_rounds"),
                    "loop skeleton must use its declared max_rounds param"
                );
            }
            oas_task::TaskBundle::open_named(&tasks_dir, name)
                .expect("open")
                .validate()
                .expect("validates");
            oas_task::TaskBundle::open_named(&tasks_dir, name)
                .expect("open")
                .lint()
                .unwrap_or_else(|issues| {
                    panic!(
                        "{template} skeleton must pass M0 lint: {:?}",
                        issues.iter().map(|i| i.to_string()).collect::<Vec<_>>()
                    )
                });
        }
        // daily also carries the UI-default schedule_hint (never authoritative).
        let daily = read_json(&tasks_dir.join("模板日常").join("meta.json"));
        assert_eq!(daily["schedule_hint"]["quota"]["per"], "day");
    }

    #[test]
    fn task_create_rejects_bad_names_and_traversal() {
        let td = TempDir::new("task-bad-names");
        let tasks_dir = td.path().join("tasks");
        let server = server_with_tasks(td.path());
        for (name, why) in [
            ("", "empty"),
            ("..", "dotdot"),
            ("../evil", "traversal"),
            ("..\\evil", "traversal-win"),
            ("a/b", "slash"),
            ("a\\b", "backslash"),
            ("a:b", "colon"),
            ("a*b", "star"),
            ("任务.", "trailing dot"),
            (" con ", "padding"),
            ("con", "reserved"),
            ("   ", "spaces"),
        ] {
            let body = serde_json::json!({"name": name}).to_string();
            let (status, out) =
                post_route(&server, "/task/create", "application/json", body.as_bytes());
            assert!(status.contains("400"), "{why} ({name:?}) => {status} {out}");
            let v: serde_json::Value = serde_json::from_str(&out).expect("error JSON");
            assert_eq!(v["ok"], serde_json::Value::Bool(false), "{why}: {out}");
            assert!(
                !v["errors"].as_array().map(|a| a.is_empty()).unwrap_or(true),
                "{why}: rejection must explain why ({out})"
            );
        }
        // Nothing escaped the tasks dir and no half bundle was left behind.
        assert!(!td.path().join("evil").exists(), "path traversal must not write");
        assert!(!tasks_dir.join("evil").exists());
        for entry in source_entries(&tasks_dir) {
            assert!(
                !entry.starts_with(".oasd-create-"),
                "staging leftovers: {entry}"
            );
        }
    }

    #[test]
    fn task_create_rejects_duplicate_without_touching_the_bundle() {
        let td = TempDir::new("task-duplicate");
        let tasks_dir = td.path().join("tasks");
        let server = server_with_tasks(td.path());
        let payload = r#"{"name":"重复任务","template":"daily"}"#.as_bytes();
        let (status, body) = post_route(&server, "/task/create", "application/json", payload);
        assert!(status.contains("200"), "{status} {body}");
        let dir = tasks_dir.join("重复任务");
        let meta_before = std::fs::read_to_string(dir.join("meta.json")).expect("meta");
        let flow_before = std::fs::read_to_string(dir.join("flow.json")).expect("flow");
        let (status, body) = post_route(&server, "/task/create", "application/json", payload);
        assert!(status.contains("409"), "{status} {body}");
        assert!(body.contains("已存在"), "duplicate must be explained: {body}");
        assert_eq!(
            meta_before,
            std::fs::read_to_string(dir.join("meta.json")).expect("meta after"),
            "a rejected duplicate must not overwrite meta.json"
        );
        assert_eq!(
            flow_before,
            std::fs::read_to_string(dir.join("flow.json")).expect("flow after"),
            "a rejected duplicate must not overwrite flow.json"
        );
    }

    #[test]
    fn task_create_rolls_back_on_every_failure_stage() {
        for stage in [
            CreateStage::Staged,
            CreateStage::FilesWritten,
            CreateStage::Validated,
            CreateStage::BeforeRename,
        ] {
            let td = TempDir::new("task-rollback");
            let tasks_dir = td.path().join("tasks");
            let name = format!("回滚任务-{stage:?}");
            let spec = CreateSpec {
                name: &name,
                template: "daily",
                description: "",
            };
            let err = create_task_bundle_with_hook(&tasks_dir, &spec, |s| {
                if s == stage {
                    Err(format!("注入失败（{s:?}）"))
                } else {
                    Ok(())
                }
            })
            .expect_err("the injected failure must abort creation");
            assert_eq!(err, format!("注入失败（{stage:?}）"));
            assert!(
                !tasks_dir.join(&name).exists(),
                "{stage:?}: the visible bundle dir must not exist"
            );
            let leftovers = source_entries(&tasks_dir);
            assert!(
                leftovers.is_empty(),
                "{stage:?}: rollback must leave no staging dir, found {leftovers:?}"
            );
        }
    }

    #[test]
    fn run_status_route_starts_empty() {
        let td = TempDir::new("run-empty");
        let server = server_with_tasks(td.path());
        let v = poll_run_status(&server);
        assert_eq!(v["ok"], serde_json::Value::Bool(true));
        assert_eq!(v["running"], serde_json::Value::Bool(false));
        assert_eq!(v["task"], serde_json::Value::Null);
        assert_eq!(v["started_ms"], serde_json::Value::Null);
        assert_eq!(v["finished_ms"], serde_json::Value::Null);
        assert_eq!(v["exit"], serde_json::Value::Null);
        assert_eq!(v["outcome"], serde_json::Value::Null);
        assert_eq!(v["error"], serde_json::Value::Null);
        assert_eq!(v["log_tail"].as_array().map(|a| a.len()), Some(0));
    }

    #[test]
    fn run_route_validates_input_and_missing_task() {
        let td = TempDir::new("run-bad-request");
        let server = server_with_tasks(td.path());
        for (body, why) in [
            ("{}", "missing task"),
            (r#"{"task":""}"#, "empty task"),
            (r#"{"task":"../evil"}"#, "traversal"),
            (r#"{"task":"ok","dry_run":"yes"}"#, "bad dry_run"),
            (r#"{"task":"ok","seed":-1}"#, "negative seed"),
            (r#"{"task":"ok","replay":42}"#, "bad replay"),
        ] {
            let (status, out) =
                post_route(&server, "/task/run", "application/json", body.as_bytes());
            assert!(status.contains("400"), "{why}: {status} {out}");
        }
        // A valid name that has no bundle is a 404, not a crash.
        let (status, body) = post_route(
            &server,
            "/task/run",
            "application/json",
            r#"{"task":"不存在的任务"}"#.as_bytes(),
        );
        assert!(status.contains("404"), "{status} {body}");
        assert!(body.contains("不存在"), "{body}");
        // A missing replay dir is rejected before the background thread starts.
        let tasks_dir = td.path().join("tasks");
        make_task(&tasks_dir, "有任务");
        let (status, body) = post_route(
            &server,
            "/task/run",
            "application/json",
            r#"{"task":"有任务","replay":"D:\\no-such-replay-dir"}"#.as_bytes(),
        );
        assert!(status.contains("400"), "{status} {body}");
        assert!(body.contains("回放目录不存在"), "{body}");
        // Nothing must be running after these rejections.
        assert_eq!(poll_run_status(&server)["running"], false);
    }

    #[test]
    fn run_route_runs_in_background_and_reports_done() {
        let td = TempDir::new("run-done");
        let tasks_dir = td.path().join("tasks");
        make_task(&tasks_dir, "可运行任务");
        let exec: Arc<RunExecutor> = Arc::new(
            |_opts: &RunOptions, out: &mut dyn FnMut(&str)| {
                out("seed=42");
                out("frame_source=live");
                out("outcome=Done reason=Done");
                Ok(done_report())
            },
        );
        let server = server_with_tasks(td.path()).with_run_executor(exec);
        let (status, body) = post_route(
            &server,
            "/task/run",
            "application/json",
            r#"{"task":"可运行任务","dry_run":true}"#.as_bytes(),
        );
        assert!(status.contains("200"), "{status} {body}");
        let started: serde_json::Value = serde_json::from_str(&body).expect("start JSON");
        assert_eq!(started["ok"], serde_json::Value::Bool(true));
        assert_eq!(started["task"], "可运行任务");
        assert_eq!(started["mode"], "dry-run");
        let st = wait_run_finished(&server, 5000);
        assert_eq!(st["running"], serde_json::Value::Bool(false));
        assert_eq!(st["task"], "可运行任务");
        assert_eq!(st["outcome"], "Done");
        assert_eq!(st["exit"], 0);
        assert_eq!(st["mode"], "dry-run");
        assert_eq!(st["dry_run"], serde_json::Value::Bool(true));
        assert_eq!(st["frame_source"], "live");
        assert_eq!(st["clicks"], 1);
        assert_eq!(st["error"], serde_json::Value::Null);
        assert!(st["started_ms"].as_u64().is_some());
        assert!(
            st["finished_ms"].as_u64().expect("finished_ms")
                >= st["started_ms"].as_u64().expect("started_ms")
        );
        let tail: Vec<&str> = st["log_tail"]
            .as_array()
            .expect("log_tail")
            .iter()
            .filter_map(|l| l.as_str())
            .collect();
        assert!(tail.contains(&"outcome=Done reason=Done"), "{tail:?}");
        assert_eq!(st["log_count"], 3);
    }

    #[test]
    fn run_route_allows_only_one_run_at_a_time() {
        let td = TempDir::new("run-mutex");
        let tasks_dir = td.path().join("tasks");
        make_task(&tasks_dir, "互斥任务");
        let gate = Arc::new(Gate::new());
        let gate_in_thread = gate.clone();
        let exec: Arc<RunExecutor> = Arc::new(
            move |_opts: &RunOptions, out: &mut dyn FnMut(&str)| {
                out("seed=7");
                gate_in_thread.wait();
                out("outcome=Done reason=Done");
                Ok(done_report())
            },
        );
        let server = server_with_tasks(td.path()).with_run_executor(exec);
        let payload = r#"{"task":"互斥任务"}"#.as_bytes();
        let (status, body) = post_route(&server, "/task/run", "application/json", payload);
        assert!(status.contains("200"), "{status} {body}");
        // `running` is set synchronously BEFORE the response, so the second
        // start is a deterministic 409 (the fake executor is still blocked).
        assert_eq!(poll_run_status(&server)["running"], true);
        let (status, body) = post_route(&server, "/task/run", "application/json", payload);
        assert!(status.contains("409"), "{status} {body}");
        assert!(body.contains("已有任务正在运行"), "{body}");
        gate.open();
        let st = wait_run_finished(&server, 5000);
        assert_eq!(st["outcome"], "Done");
        assert_eq!(st["exit"], 0);
        // Once finished, the mutex is released and status keeps the last result.
        assert_eq!(poll_run_status(&server)["task"], "互斥任务");
    }

    #[test]
    fn run_route_reports_setup_errors_with_exit_2() {
        let td = TempDir::new("run-setup-error");
        let tasks_dir = td.path().join("tasks");
        make_task(&tasks_dir, "无设备任务");
        let exec: Arc<RunExecutor> = Arc::new(
            |_opts: &RunOptions, out: &mut dyn FnMut(&str)| {
                out("seed=1");
                Err("no adb device attached (fake executor)".to_string())
            },
        );
        let server = server_with_tasks(td.path()).with_run_executor(exec);
        let (status, body) = post_route(
            &server,
            "/task/run",
            "application/json",
            r#"{"task":"无设备任务"}"#.as_bytes(),
        );
        assert!(status.contains("200"), "{status} {body}");
        let st = wait_run_finished(&server, 5000);
        assert_eq!(st["running"], serde_json::Value::Bool(false));
        assert_eq!(st["outcome"], "Error");
        assert_eq!(st["exit"], 2);
        let err = st["error"].as_str().expect("error string");
        assert!(err.contains("no adb device"), "{err}");
        let tail: Vec<&str> = st["log_tail"]
            .as_array()
            .expect("log_tail")
            .iter()
            .filter_map(|l| l.as_str())
            .collect();
        assert!(tail.iter().any(|l| l.contains("error=no adb device")), "{tail:?}");
    }
}
