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
    /// Serial used by adb (`-s`).
    pub serial: String,
    /// Physical device identity (`ro.serialno`); used to dedupe.
    pub unique: String,
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
            if out.iter().any(|d| d.unique == unique) {
                continue; // duplicate adb endpoint for the same device
            }
            out.push(Device { serial, unique });
        }
        Ok(out)
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
}
