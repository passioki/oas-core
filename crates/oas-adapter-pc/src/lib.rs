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

use std::path::PathBuf;
use std::process::Command;

use oas_codec::decode_png;
use oas_types::{Frame, InputSink, Point, ScreenSource};

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
        let out = Command::new(&self.exe)
            .args(args)
            .output()
            .map_err(|e| format!("spawn {:?} {:?}: {e}", self.exe, args))?;
        if !out.status.success() {
            return Err(format!(
                "adb {:?} failed ({}): {}",
                args,
                out.status,
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        Ok(out.stdout)
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
        self.adb
            .run(&[
                "-s",
                &self.serial,
                "shell",
                "input",
                "swipe",
                &from.x.to_string(),
                &from.y.to_string(),
                &to.x.to_string(),
                &to.y.to_string(),
                &ms.to_string(),
            ])
            .map(|_| ())
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
}
