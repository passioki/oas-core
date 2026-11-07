//! oas-adapter-pc: PC-side adapters (ADB over MuMu/emulator/phone).
//!
//! This is the ONLY kind of crate allowed to know about a platform. The core
//! crates must never depend on it.
//!
//! Verified against a live MuMu instance in P0-4:
//!   * screencap returns a LANDSCAPE 1280x720 PNG even though `wm size` says
//!     `720x1280` => never trust `wm size`, always use the decoded frame size.
//!   * the same physical device can be reachable under two serials
//!     (`127.0.0.1:16384` and `emulator-5554`); dedupe by `ro.serialno`.

use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use oas_codec::decode_png;
use oas_types::{Frame, InputSink, Point, ScreenSource};

/// Hard deadline for one adb invocation (seq133 #3).
///
/// An emulator/ADB server that wedges used to block the calling thread forever
/// (`Command::output()` has no timeout), which froze Studio's real click/drag
/// tests permanently. Every adb call is now bounded.
pub const ADB_TIMEOUT: Duration = Duration::from_secs(15);
/// `screencap` moves a full PNG over the pipe; keep a slightly larger bound.
pub const ADB_SCREENCAP_TIMEOUT: Duration = Duration::from_secs(20);

/// Deadline for one argv (screencap/exec-out get the longer bound).
fn timeout_for(args: &[&str]) -> Duration {
    if args.iter().any(|a| *a == "screencap" || *a == "exec-out") {
        ADB_SCREENCAP_TIMEOUT
    } else {
        ADB_TIMEOUT
    }
}

fn format_timeout(d: Duration) -> String {
    if d.subsec_millis() == 0 {
        format!("{}s", d.as_secs())
    } else {
        format!("{:.1}s", d.as_secs_f64())
    }
}

/// Compact argv for error messages (never dump an unbounded command line).
fn argv_label(args: &[&str]) -> String {
    let mut text = args.join(" ");
    if text.chars().count() > 160 {
        text = text.chars().take(160).collect::<String>() + "…";
    }
    format!("adb {text}")
}

/// Kill the whole process tree of one timed-out child.
///
/// A plain `child.kill()` on Windows only kills the direct child; grandchildren
/// (e.g. a wrapper `.bat` spawning `ping`) may keep the inherited stdout/stderr
/// pipe handles open, so the reader threads would not see EOF. `taskkill /T`
/// closes that hole. The fake-exe timeout test exercises exactly this.
#[cfg(windows)]
fn kill_process_tree(pid: u32) {
    let _ = Command::new("taskkill")
        .args(["/T", "/F", "/PID", &pid.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

#[cfg(not(windows))]
fn kill_process_tree(_pid: u32) {}

/// Thin wrapper around the adb executable.
#[derive(Clone, Debug)]
pub struct Adb {
    pub exe: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Device {
    /// Serial used by adb (`-s`). Prefer the configured alias when matched.
    pub serial: String,
    /// Physical device identity (android_id/MAC/etc); used to dedupe.
    pub unique: String,
    /// Every adb endpoint for this physical device, e.g.
    /// `["127.0.0.1:16384", "emulator-5554"]`. The editor/config may name
    /// either one; matching must not discard the alias.
    pub aliases: Vec<String>,
}

impl Device {
    pub fn matches(&self, serial: &str) -> bool {
        self.serial == serial || self.aliases.iter().any(|a| a == serial)
    }

    /// Human-readable serial + aliases for errors and the instance picker.
    pub fn label(&self) -> String {
        if self.aliases.len() > 1 {
            format!("{} ({})", self.serial, self.aliases.join(" / "))
        } else {
            self.serial.clone()
        }
    }
}

fn merge_device(out: &mut Vec<Device>, serial: String, unique: String) {
    if let Some(existing) = out.iter_mut().find(|d| d.unique == unique) {
        if !existing.aliases.iter().any(|a| a == &serial) {
            existing.aliases.push(serial);
        }
        return;
    }
    out.push(Device {
        serial: serial.clone(),
        unique,
        aliases: vec![serial],
    });
}

impl Adb {
    pub fn new(exe: impl Into<PathBuf>) -> Self {
        Self { exe: exe.into() }
    }

    /// `ADB_PATH` env var, else `adb` from PATH.
    pub fn from_env() -> Self {
        match std::env::var("ADB_PATH") {
            Ok(v) if !v.trim().is_empty() => Self::new(v),
            _ => Self::new("adb"),
        }
    }

    fn run(&self, args: &[&str]) -> Result<Vec<u8>, String> {
        self.run_with_timeout(args, timeout_for(args))
    }

    /// Run one adb argv with a hard deadline.
    ///
    /// Implementation notes (seq133 #3):
    /// * **spawn** (not `Command::output()`, which waits forever);
    /// * stdout/stderr are drained on two reader threads so a full pipe can
    ///   never deadlock the child;
    /// * `try_wait` polls until the deadline; on timeout the child is killed and
    ///   a loud `adb ... timed out after Ns` error is returned;
    /// * the argv is included (truncated) so the user can tell which call stuck.
    fn run_with_timeout(&self, args: &[&str], timeout: Duration) -> Result<Vec<u8>, String> {
        let mut child = Command::new(&self.exe)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("spawn {:?} {:?}: {e}", self.exe, args))?;
        let mut stdout = child
            .stdout
            .take()
            .ok_or_else(|| format!("{}: stdout pipe unavailable", argv_label(args)))?;
        let mut stderr = child
            .stderr
            .take()
            .ok_or_else(|| format!("{}: stderr pipe unavailable", argv_label(args)))?;
        let stdout_reader = std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = stdout.read_to_end(&mut buf);
            buf
        });
        let stderr_reader = std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = stderr.read_to_end(&mut buf);
            buf
        });
        let deadline = Instant::now() + timeout;
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break Ok(status),
                Ok(None) => {
                    if Instant::now() >= deadline {
                        kill_process_tree(child.id());
                        let _ = child.kill();
                        let _ = child.wait();
                        break Err(format!(
                            "{} timed out after {} (killed)",
                            argv_label(args),
                            format_timeout(timeout)
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(e) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break Err(format!("{}: wait failed: {e}", argv_label(args)));
                }
            }
        };
        // The child is gone (or killed): readers hit EOF and finish promptly.
        let out = stdout_reader.join().unwrap_or_default();
        let err = stderr_reader.join().unwrap_or_default();
        match status {
            Ok(status) if status.success() => Ok(out),
            Ok(status) => Err(format!(
                "{} failed ({status}): {}",
                argv_label(args),
                String::from_utf8_lossy(&err).trim()
            )),
            Err(timeout_error) => Err(timeout_error),
        }
    }

    fn text(&self, args: &[&str]) -> Result<String, String> {
        Ok(String::from_utf8_lossy(&self.run(args)?).trim().to_string())
    }

    /// List attached devices, deduped by **physical identity**.
    ///
    /// Two adb entries for the same device (`127.0.0.1:16384` + `emulator-5554`)
    /// collapse into one `Device`; the first seen serial wins.
    ///
    /// Identity probe order (verified on MuMu in P0-4):
    ///   1. `settings get secure android_id`  (stable per install)
    ///   2. `/sys/class/net/wlan0/address`    (MAC)
    ///   3. `/proc/sys/kernel/random/boot_id` (per boot)
    ///   4. the adb serial itself
    ///
    /// NOTE: `ro.serialno` / `ro.boot.serialno` are EMPTY on MuMu, and
    /// `ro.product.model` is the phone MODEL (e.g. `V2284A`), which is NOT
    /// unique - dedupe must not use it.
    pub fn devices(&self) -> Result<Vec<Device>, String> {
        let raw = self.text(&["devices"])?;
        let mut out: Vec<Device> = Vec::new();
        for serial in parse_device_serials(&raw) {
            let unique = self.unique_id(&serial);
            merge_device(&mut out, serial, unique);
        }
        Ok(out)
    }

    /// Select a physical device while honoring **every alias** of the configured
    /// serial. `127.0.0.1:16384` and `emulator-5554` are the same MuMu instance;
    /// a stale config must not fail just because adb happened to list the other
    /// endpoint first.
    pub fn resolve_device(&self, wanted: Option<&str>) -> Result<Device, String> {
        let mut devices = self.devices()?;
        let wanted = wanted.map(str::trim).filter(|s| !s.is_empty());
        if let Some(serial) = wanted {
            if !devices.iter().any(|d| d.matches(serial)) && serial.contains(':') {
                // MuMu/emulator TCP endpoints can disappear when the adb server
                // restarts. Reconnect the configured endpoint before giving up.
                let _ = self.connect(serial);
                for _ in 0..50 {
                    std::thread::sleep(std::time::Duration::from_millis(200));
                    devices = self.devices()?;
                    if devices.iter().any(|d| d.matches(serial)) {
                        break;
                    }
                }
            }
            if let Some(mut dev) = devices.iter().find(|d| d.matches(serial)).cloned() {
                // Keep the user's configured endpoint as the adb serial; both
                // endpoints are attached and the configured one is explicit.
                if dev.aliases.iter().any(|a| a == serial) {
                    dev.serial = serial.to_string();
                }
                return Ok(dev);
            }
            let seen = devices
                .iter()
                .map(Device::label)
                .collect::<Vec<_>>()
                .join(", ");
            return Err(format!(
                "configured device serial `{serial}` is not attached (seen: [{seen}]; \
                 ADB_PATH={}) — if this is the same emulator, check its TCP port or run \
                 `adb connect {serial}`",
                self.exe.display()
            ));
        }
        devices.into_iter().next().ok_or_else(|| {
            format!(
                "no adb device attached (ADB_PATH={}); attach the emulator or pass --replay",
                self.exe.display()
            )
        })
    }

    fn unique_id(&self, serial: &str) -> String {
        const PROBES: [&[&str]; 3] = [
            &["shell", "settings", "get", "secure", "android_id"],
            &["shell", "cat", "/sys/class/net/wlan0/address"],
            &["shell", "cat", "/proc/sys/kernel/random/boot_id"],
        ];
        for probe in PROBES {
            let mut args: Vec<&str> = vec!["-s", serial];
            args.extend_from_slice(probe);
            if let Ok(t) = self.text(&args) {
                let t = t.trim();
                if !t.is_empty() && t != "null" {
                    return format!("{}:{}", probe[probe.len() - 1], t);
                }
            }
        }
        serial.to_string()
    }

    /// `adb connect <endpoint>` (TCP devices such as MuMu at 127.0.0.1:16384).
    pub fn connect(&self, endpoint: &str) -> Result<(), String> {
        self.run(&["connect", endpoint]).map(|_| ())
    }

    /// Ask adb to reconnect `serial` and wait until it is listed again (§22.16).
    ///
    /// `adb reconnect` itself may fail while the device is briefly offline, so its
    /// result is ignored; the source of truth is the `devices` poll below.
    ///
    /// Restarting the adb server drops TCP devices (MuMu/emulator) until they are
    /// re-`connect`ed. Set `OAS_ADB_RECONNECT_ENDPOINTS` (comma separated, e.g.
    /// `127.0.0.1:16384`) to also try `adb connect` for those endpoints.
    pub fn reconnect(&self, serial: &str) -> Result<(), String> {
        let _ = self.run(&["-s", serial, "reconnect"]);
        let endpoints: Vec<String> = std::env::var("OAS_ADB_RECONNECT_ENDPOINTS")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        for ep in &endpoints {
            let _ = self.run(&["connect", ep]);
        }
        for _ in 0..100 {
            if let Ok(devs) = self.devices() {
                if devs.iter().any(|d| d.serial == serial) {
                    return Ok(());
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        Err(format!(
            "adb reconnect `{serial}`: device not listed again after 20s"
        ))
    }

    pub fn screen(&self, serial: &str) -> AdbScreen {
        AdbScreen {
            adb: self.clone(),
            serial: serial.to_string(),
        }
    }

    pub fn input(&self, serial: &str) -> AdbInput {
        AdbInput {
            adb: self.clone(),
            serial: serial.to_string(),
        }
    }
}

/// Parse `adb devices` output into serials (state == `device` only).
pub fn parse_device_serials(raw: &str) -> Vec<String> {
    raw.lines()
        .skip_while(|l| !l.starts_with("List of devices"))
        .skip(1)
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('*') {
                return None;
            }
            let mut it = line.split_whitespace();
            let serial = it.next()?;
            let state = it.next().unwrap_or("");
            if state == "device" {
                Some(serial.to_string())
            } else {
                None
            }
        })
        .collect()
}

pub struct AdbScreen {
    adb: Adb,
    serial: String,
}

impl ScreenSource for AdbScreen {
    fn grab(&mut self) -> Result<Frame, String> {
        // `exec-out` is binary-safe (unlike `shell`, which may rewrite CRLF).
        let png = self
            .adb
            .run(&["-s", &self.serial, "exec-out", "screencap", "-p"])?;
        decode_png(&png)
    }
}

pub struct AdbInput {
    adb: Adb,
    serial: String,
}

/// The exact `adb` argv of one swipe gesture: `input swipe x1 y1 x2 y2 ms`.
///
/// Shared by [`InputSink::swipe`] and [`InputSink::long_press`] (a long press is
/// the same command with `from == to`). Kept pure so the command contract is
/// unit-testable without a device (v1.3 operation-region P1).
fn swipe_argv(serial: &str, from: Point, to: Point, ms: u32) -> Vec<String> {
    vec![
        "-s".to_string(),
        serial.to_string(),
        "shell".to_string(),
        "input".to_string(),
        "swipe".to_string(),
        from.x.to_string(),
        from.y.to_string(),
        to.x.to_string(),
        to.y.to_string(),
        ms.to_string(),
    ]
}

impl InputSink for AdbInput {
    fn click(&mut self, p: Point) -> Result<(), String> {
        self.adb
            .run(&[
                "-s",
                &self.serial,
                "shell",
                "input",
                "tap",
                &p.x.to_string(),
                &p.y.to_string(),
            ])
            .map(|_| ())
    }

    fn swipe(&mut self, from: Point, to: Point, ms: u32) -> Result<(), String> {
        let args = swipe_argv(&self.serial, from, to, ms);
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        self.adb.run(&refs).map(|_| ())
    }

    /// Long press = a **same-point swipe** (`input swipe x y x y ms`), which is
    /// the portable press-and-hold gesture on Android. ADB has no dedicated
    /// long-press command; this is the recommended approximation.
    fn long_press(&mut self, p: Point, ms: u32) -> Result<(), String> {
        let args = swipe_argv(&self.serial, p, p, ms);
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        self.adb.run(&refs).map(|_| ())
    }

    fn key(&mut self, keycode: u32) -> Result<(), String> {
        self.adb
            .run(&[
                "-s",
                &self.serial,
                "shell",
                "input",
                "keyevent",
                &keycode.to_string(),
            ])
            .map(|_| ())
    }

    fn restart_app(&mut self, package: &str) -> Result<(), String> {
        let pkg = package.trim();
        if pkg.is_empty() {
            return Err("restart_app: package is empty".to_string());
        }
        self.adb
            .run(&["-s", &self.serial, "shell", "am", "force-stop", pkg])?;
        self.adb.run(&[
            "-s",
            &self.serial,
            "shell",
            "monkey",
            "-p",
            pkg,
            "-c",
            "android.intent.category.LAUNCHER",
            "1",
        ])?;
        Ok(())
    }
}

impl AdbInput {
    pub fn key(&mut self, keycode: u32) -> Result<(), String> {
        self.adb
            .run(&[
                "-s",
                &self.serial,
                "shell",
                "input",
                "keyevent",
                &keycode.to_string(),
            ])
            .map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_adb_devices_and_skips_offline() {
        let raw = "List of devices attached\n\
                   127.0.0.1:16384\tdevice\n\
                   emulator-5554\tdevice\n\
                   offline-1\toffline\n\
                   \n";
        assert_eq!(
            parse_device_serials(raw),
            vec!["127.0.0.1:16384".to_string(), "emulator-5554".to_string()]
        );
    }

    #[test]
    fn device_list_without_header_is_empty() {
        assert!(parse_device_serials("").is_empty());
        assert!(parse_device_serials("garbage").is_empty());
    }

    #[test]
    fn dedupe_keeps_every_alias_of_one_physical_device() {
        let mut out = Vec::new();
        merge_device(&mut out, "emulator-5554".into(), "android_id:657eef".into());
        merge_device(
            &mut out,
            "127.0.0.1:16384".into(),
            "android_id:657eef".into(),
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].serial, "emulator-5554");
        assert!(out[0].matches("emulator-5554"));
        assert!(out[0].matches("127.0.0.1:16384"));
        assert!(!out[0].matches("127.0.0.1:16416"));
        assert!(out[0].label().contains("127.0.0.1:16384"));
    }

    #[test]
    fn swipe_argv_is_the_historical_command() {
        let argv = swipe_argv(
            "127.0.0.1:16384",
            Point { x: 10, y: 20 },
            Point { x: 30, y: 40 },
            650,
        );
        assert_eq!(
            argv,
            vec![
                "-s",
                "127.0.0.1:16384",
                "shell",
                "input",
                "swipe",
                "10",
                "20",
                "30",
                "40",
                "650"
            ]
        );
    }

    #[test]
    fn long_press_is_a_same_point_swipe_with_the_hold_duration() {
        // `InputSink::long_press` (ADB) sends `input swipe x y x y ms`.
        let argv = swipe_argv(
            "127.0.0.1:16384",
            Point { x: 640, y: 360 },
            Point { x: 640, y: 360 },
            3000,
        );
        assert_eq!(
            argv,
            vec![
                "-s",
                "127.0.0.1:16384",
                "shell",
                "input",
                "swipe",
                "640",
                "360",
                "640",
                "360",
                "3000"
            ]
        );
    }

    #[test]
    fn timeout_policy_and_labels_are_bounded() {
        assert_eq!(timeout_for(&["devices"]), ADB_TIMEOUT);
        assert_eq!(timeout_for(&["exec-out", "screencap", "-p"]), ADB_SCREENCAP_TIMEOUT);
        assert_eq!(format_timeout(Duration::from_secs(15)), "15s");
        assert_eq!(format_timeout(Duration::from_millis(800)), "0.8s");
        let long = vec!["x"; 200];
        let label = argv_label(&long);
        assert!(label.starts_with("adb "), "{label}");
        assert!(label.contains('…'), "{label}");
        assert!(label.chars().count() < 200, "{label}");
    }

    #[cfg(windows)]
    #[test]
    fn windows_fake_exe_that_hangs_is_killed_at_the_deadline() {
        // seq133 #3: a wedged adb must never block the service thread. A .bat
        // fake "exe" sleeps ~29s; run_with_timeout must kill it at ~800ms and
        // return the loud timeout error instead of waiting for the child.
        let dir = std::env::temp_dir().join(format!(
            "oas-adapter-timeout-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let bat = dir.join("fake-adb-hang.bat");
        std::fs::write(&bat, "@ping -n 30 127.0.0.1 >nul\r\n").expect("write fake adb");
        let adb = Adb::new("cmd");
        let t0 = Instant::now();
        let err = adb
            .run_with_timeout(&["/c", &bat.to_string_lossy()], Duration::from_millis(800))
            .expect_err("a hanging fake adb must time out");
        let elapsed = t0.elapsed();
        assert!(
            err.contains("timed out after"),
            "timeout error must be explicit: {err}"
        );
        assert!(err.contains("fake-adb-hang.bat"), "argv must be in the error: {err}");
        assert!(
            elapsed < Duration::from_secs(5),
            "timeout must return promptly, took {elapsed:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(windows)]
    #[test]
    fn windows_fake_exe_returns_stdout_on_success() {
        let adb = Adb::new("cmd");
        let out = adb
            .run_with_timeout(&["/c", "echo", "adapter-ok"], Duration::from_secs(5))
            .expect("quick fake adb must succeed");
        assert!(
            String::from_utf8_lossy(&out).contains("adapter-ok"),
            "stdout missing: {}",
            String::from_utf8_lossy(&out)
        );
    }
}
