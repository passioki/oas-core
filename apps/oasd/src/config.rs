//! 统一部署配置（`config/live.json`，DELIVERY-PLAN M3）。
//!
//! 设计原则（ENGINE §22.34 / DELIVERY-PLAN §4）：
//! * **配置文件是实机部署的唯一入口**：端口/目录/模型/adb/serial 都从这里来；
//! * 命令行 flags 仍然优先（`--port` 等覆盖 config）；
//! * 所有相对路径按**配置文件所在目录/包根**解析（标准布局 `<包>/config/live.json` → `<包>/adb/adb.exe`）；
//! * 设备 serial 必须显式：生产环境不允许"第一个设备"隐式选择；但同一物理设备的
//!   adb serial 对（`127.0.0.1:port` / `emulator-xxxx`）都视为同一实例；
//! * 本模块只做**解析/校验**，不碰网络/adb（便于单测）。
//!
//! `main.rs` 在分发子命令前处理全局 `--config <file>`：
//! 1. `Config::load` 校验；
//! 2. `apply_env` 把 adb/serial 写进环境变量（适配器/运行器沿用现有 env 入口）；
//! 3. `set_global` 供 serve/scheduler 读取默认值；
//! 4. 从 argv 移除 `--config`，子命令解析器不会看到未知参数。

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct OasConfig {
    pub schema_version: u32,
    pub serve: ServeConfig,
    /// Runner (usage surface) settings. Separate from `serve` on purpose: the
    /// two surfaces must never share a port (PR1 of the Studio/Runner split).
    pub runner: RunnerConfig,
    pub paths: PathsConfig,
    pub ocr: OcrConfig,
    pub device: DeviceConfig,
    pub scheduler: SchedulerConfig,
    /// Studio ⇄ Runner publish exchange (P0-1a, design
    /// `RUNNER-SPLIT-DESIGN.md §5.3`). Additive: an old `live.json` without the
    /// section keeps parsing, and an unset section does not change the file's
    /// serialised shape (`skip_serializing_if`), so `POST /instance` rewrites
    /// stay byte-compatible for existing deployments.
    #[serde(default, skip_serializing_if = "ExchangeConfig::is_default")]
    pub exchange: ExchangeConfig,
}

/// Studio → Runner publish exchange settings.
///
/// Both surfaces share this schema (one `OasConfig`): the Studio writes the
/// shared directory (`announce_publish` + `POST /task/publish`), the Runner
/// reads it (`GET /runner/updates`). `notify` is only meaningful on the Studio
/// side and `poll_secs` only on the Runner side — keeping one schema means the
/// same `live.json` section is valid on both ends.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct ExchangeConfig {
    /// Exchange directory (`publish-index.json` + `incoming/`). A relative path
    /// is resolved against the **live config file's directory** (same rule as
    /// `device.lease`, see [`exchange_dir_from`]); absolute paths are used
    /// verbatim. Ignored when `enabled` is false.
    pub dir: String,
    /// Master switch. Default **off** so an old `live.json` never starts writing
    /// (or polling) an exchange directory it never opted into.
    pub enabled: bool,
    /// Runner-side polling hint in seconds (`0..=3600`; `0` = no hint/disable).
    /// The current Runner UI polls `GET /runner/updates` on its own timer; this
    /// value is the contract for a future poller.
    pub poll_secs: u32,
    /// Best-effort loopback HTTP hint (Studio side; reserved for the follow-up
    /// `POST /runner/notify` client — the exchange file is the only truth).
    pub notify: NotifyConfig,
}

impl Default for ExchangeConfig {
    fn default() -> Self {
        Self {
            dir: "exchange".into(),
            enabled: false,
            poll_secs: 3,
            notify: NotifyConfig::default(),
        }
    }
}

impl ExchangeConfig {
    /// `true` when the whole section equals its default (used for
    /// `skip_serializing_if` so old `live.json` files keep their shape).
    pub fn is_default(&self) -> bool {
        self == &Self::default()
    }
}

/// Loopback accelerator settings (`POST /runner/notify`).
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct NotifyConfig {
    /// Opt-in switch; disabled by default because the exchange file alone is
    /// sufficient (and the P0-1a slice only ships the Runner-side receiver).
    pub enabled: bool,
    /// Base URL of the local Runner (`http://127.0.0.1:8789`). Only loopback
    /// http URLs are accepted: both surfaces bind loopback (PR1) and the hint
    /// must never be sent to a remote host.
    pub runner_url: String,
    /// Connect/read timeout in ms (`100..=5000`).
    pub timeout_ms: u32,
}

impl Default for NotifyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            runner_url: "http://127.0.0.1:8789".into(),
            timeout_ms: 800,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct ServeConfig {
    pub port: u16,
    pub bind: String,
    pub ui: String,
}

impl Default for ServeConfig {
    fn default() -> Self {
        Self {
            port: 8788,
            bind: "127.0.0.1".into(),
            ui: "ui".into(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct RunnerConfig {
    /// `oasd runner` HTTP port. Defaults to 8789, one above the Studio 8788.
    pub port: u16,
    /// Reserved for the LAN/token era (design Q1). PR1 still binds loopback
    /// only; values other than a loopback address are rejected by `validate`
    /// so this never becomes a "looks configurable but is ignored" trap.
    pub bind: String,
}

impl Default for RunnerConfig {
    fn default() -> Self {
        Self {
            port: 8789,
            bind: "127.0.0.1".into(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct PathsConfig {
    pub tasks_dir: String,
    pub runs_dir: String,
    pub library_dir: String,
    /// Runner package root (`installed.json` + staging/backup). Empty = derive
    /// `<tasks_dir>/../packages` (design §B; the field is optional so an old
    /// `live.json` keeps parsing and no existing deployment changes layout).
    pub packages_dir: String,
}

impl Default for PathsConfig {
    fn default() -> Self {
        Self {
            tasks_dir: "tasks".into(),
            runs_dir: "runs".into(),
            library_dir: "library".into(),
            packages_dir: String::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct OcrConfig {
    pub model: String,
    pub det_model: String,
}

impl Default for OcrConfig {
    fn default() -> Self {
        Self {
            model: "models/PP-OCRv6_rec_small.onnx".into(),
            det_model: "models/PP-OCRv6_det_small.onnx".into(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct DeviceConfig {
    /// 显式 adb 路径；空 = 走 `ADB_PATH` / PATH。
    pub adb_path: String,
    /// 显式设备 serial（如 `127.0.0.1:16384`）。生产必填。
    pub serial: String,
    /// 冷启动时 adb server 丢设备后尝试 connect 的端点。
    pub reconnect_endpoints: Vec<String>,
    /// Device lease file (design §A.6.4, PR6). Empty = default
    /// `<live config dir>/device.lease`; a relative path is resolved against
    /// the live config file's directory. Only written by the app when a
    /// `--config` was loaded (see `device_lease::lease_path_from_config`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lease: Option<String>,
}

impl Default for DeviceConfig {
    fn default() -> Self {
        Self {
            adb_path: String::new(),
            serial: String::new(),
            reconnect_endpoints: Vec::new(),
            lease: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct SchedulerConfig {
    pub plan: String,
    pub state: String,
    pub report: String,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            plan: "scheduler.json".into(),
            state: "scheduler.state.json".into(),
            report: "runs/scheduler-report.json".into(),
        }
    }
}

impl OasConfig {
    pub fn from_json(text: &str) -> Result<Self, String> {
        let cfg: OasConfig = serde_json::from_str(text).map_err(|e| format!("config JSON: {e}"))?;
        cfg.validate()?;
        Ok(cfg)
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("read config `{}`: {e}", path.display()))?;
        let mut cfg = Self::from_json(&text)?;
        // The deployed `live.json` uses paths relative to the package root
        // (`adb/adb.exe`, `models/...`). Resolve adb_path against the config file
        // directory so changing the process CWD cannot make adb point elsewhere.
        let adb = cfg.device.adb_path.trim().to_string();
        if !adb.is_empty() {
            let raw = PathBuf::from(&adb);
            if raw.is_relative() {
                let cfg_dir = path
                    .parent()
                    .filter(|p| !p.as_os_str().is_empty())
                    .map(PathBuf::from)
                    .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
                // Standard layout: <package>/config/live.json + <package>/adb/adb.exe.
                // Also support a config file next to the executable.
                let package_root = cfg_dir
                    .file_name()
                    .and_then(|n| n.to_str())
                    .filter(|n| n.eq_ignore_ascii_case("config"))
                    .and_then(|_| cfg_dir.parent())
                    .map(PathBuf::from)
                    .unwrap_or_else(|| cfg_dir.clone());
                let a = package_root.join(&raw);
                let b = cfg_dir.join(&raw);
                cfg.device.adb_path = if a.is_file() {
                    a.to_string_lossy().to_string()
                } else {
                    b.to_string_lossy().to_string()
                };
            }
        }
        Ok(cfg)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 0 && self.schema_version != 1 {
            return Err(format!(
                "config schema_version must be 1, got {}",
                self.schema_version
            ));
        }
        if self.serve.port == 0 {
            return Err("serve.port must be 1..=65535".into());
        }
        if self.serve.bind.trim().is_empty() {
            return Err("serve.bind must not be empty".into());
        }
        if self.runner.port == 0 {
            return Err("runner.port must be 1..=65535".into());
        }
        if self.runner.port == self.serve.port {
            return Err(format!(
                "runner.port ({}) must differ from serve.port ({}): Studio and Runner are \
                 separate surfaces",
                self.runner.port, self.serve.port
            ));
        }
        let runner_bind = self.runner.bind.trim();
        if runner_bind.is_empty() {
            return Err("runner.bind must not be empty".into());
        }
        // PR1 has no token auth yet, so only loopback is accepted (design Q1).
        // This keeps the field honest instead of silently binding 127.0.0.1.
        let loopback = runner_bind.eq_ignore_ascii_case("localhost")
            || runner_bind
                .parse::<std::net::IpAddr>()
                .map(|ip| ip.is_loopback())
                .unwrap_or(false);
        if !loopback {
            return Err(format!(
                "runner.bind `{runner_bind}` must be a loopback address (PR1; LAN needs the \
                 token auth planned in design Q1)"
            ));
        }
        if self.paths.tasks_dir.trim().is_empty() {
            return Err("paths.tasks_dir must not be empty".into());
        }
        // P0-1a exchange (design §5.3). Checked before the optional-serial early
        // return below so a valid-but-deviceless config still cannot hide a
        // broken exchange/notify setting.
        if self.exchange.enabled && self.exchange.dir.trim().is_empty() {
            return Err("exchange.dir must not be empty when exchange.enabled".into());
        }
        if self.exchange.poll_secs > 3600 {
            return Err(format!(
                "exchange.poll_secs must be 0..=3600, got {}",
                self.exchange.poll_secs
            ));
        }
        if !(100..=5000).contains(&self.exchange.notify.timeout_ms) {
            return Err(format!(
                "exchange.notify.timeout_ms must be 100..=5000, got {}",
                self.exchange.notify.timeout_ms
            ));
        }
        let notify_url = self.exchange.notify.runner_url.trim();
        if notify_url.is_empty() {
            if self.exchange.notify.enabled {
                return Err(
                    "exchange.notify.runner_url must not be empty when notify.enabled".into(),
                );
            }
        } else if !is_loopback_http_url(notify_url) {
            return Err(format!(
                "exchange.notify.runner_url `{notify_url}` must be an http loopback URL \
                 (http://127.0.0.1:<port> / http://localhost:<port> / http://[::1]:<port>)"
            ));
        }
        if self.device.serial.trim().is_empty() {
            // Not fatal at load time (dev can omit), but callers that need a
            // device must call `require_serial()`.
            return Ok(());
        }
        if self.device.serial.contains(char::is_whitespace) {
            return Err("device.serial must not contain whitespace".into());
        }
        Ok(())
    }

    /// 生产路径/运行器用：没有显式 serial 就响亮报错，绝不隐式选第一个设备。
    pub fn require_serial(&self) -> Result<&str, String> {
        let s = self.device.serial.trim();
        if s.is_empty() {
            return Err(
                "device.serial is required in config (explicit device selection; no first-device fallback)"
                    .into(),
            );
        }
        Ok(s)
    }

    pub fn tasks_dir(&self) -> PathBuf {
        PathBuf::from(&self.paths.tasks_dir)
    }
    pub fn runs_dir(&self) -> PathBuf {
        PathBuf::from(&self.paths.runs_dir)
    }
    pub fn library_dir(&self) -> PathBuf {
        PathBuf::from(&self.paths.library_dir)
    }
    /// Configured Runner package root, or `None` to derive it next to the tasks
    /// dir (see [`Self::packages_dir`] call sites in `serve.rs`).
    pub fn packages_dir(&self) -> Option<PathBuf> {
        let raw = self.paths.packages_dir.trim();
        (!raw.is_empty()).then(|| PathBuf::from(raw))
    }

    /// Resolved exchange directory for this process (P0-1a). `None` when the
    /// setting is blank, or when it is relative and no `--config` was loaded
    /// (there is no well-defined anchor directory then).
    pub fn exchange_dir(&self) -> Option<PathBuf> {
        exchange_dir_from(global_path(), &self.exchange.dir)
    }

    /// 把设备相关配置写进现有适配器使用的环境变量。
    pub fn apply_env(&self) {
        if !self.device.adb_path.trim().is_empty() {
            std::env::set_var("ADB_PATH", self.device.adb_path.trim());
        }
        if !self.device.reconnect_endpoints.is_empty() {
            std::env::set_var(
                "OAS_ADB_RECONNECT_ENDPOINTS",
                self.device.reconnect_endpoints.join(","),
            );
        }
        if !self.device.serial.trim().is_empty() {
            std::env::set_var("OAS_DEVICE_SERIAL", self.device.serial.trim());
        }
    }
}

/// Resolve an exchange `dir` against an explicit live-config path (pure, so the
/// two branches are unit-testable without the process-wide `global_path()`
/// once-lock — same pattern as `device_lease::lease_path_from`).
///
/// * absolute `configured` -> used verbatim (works even without `--config`);
/// * relative `configured` -> joined to the live config file's directory;
/// * blank `configured` / no config path for a relative setting -> `None`.
pub fn exchange_dir_from(config_path: Option<&Path>, configured: &str) -> Option<PathBuf> {
    let raw = configured.trim();
    if raw.is_empty() {
        return None;
    }
    let raw = PathBuf::from(raw);
    if raw.is_absolute() {
        return Some(raw);
    }
    let config_path = config_path?;
    let dir = config_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    Some(dir.join(raw))
}

/// Is `url` an `http://` URL whose host is a loopback address? Only
/// `127.0.0.0/8`, `localhost` and `::1` qualify: both surfaces bind loopback
/// and the publish hint must never leave the machine (design §5.6).
fn is_loopback_http_url(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("http://") else {
        return false;
    };
    // Authority ends at the first path/query/fragment separator.
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.is_empty() || authority.contains('@') {
        // `@` = userinfo; a local service base URL has no credentials.
        return false;
    }
    let host = if let Some(v6) = authority.strip_prefix('[') {
        match v6.split_once(']') {
            Some((addr, rest)) if rest.is_empty() || rest.starts_with(':') => addr,
            _ => return false,
        }
    } else {
        authority.split(':').next().unwrap_or("")
    };
    if host.is_empty() {
        return false;
    }
    host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<std::net::IpAddr>()
            .map(|ip| ip.is_loopback())
            .unwrap_or(false)
}

static GLOBAL: OnceLock<OasConfig> = OnceLock::new();

pub fn set_global(cfg: OasConfig) {
    let _ = GLOBAL.set(cfg);
}

/// Path of the loaded `--config` file (v1.1 P3 settings/instance editing).
static GLOBAL_PATH: OnceLock<PathBuf> = OnceLock::new();

pub fn set_global_path(path: PathBuf) {
    let _ = GLOBAL_PATH.set(path);
}

pub fn global_path() -> Option<&'static Path> {
    GLOBAL_PATH.get().map(|p| p.as_path())
}

/// 已加载的全局配置；未加载（如纯 CLI/dev）时为 None。
pub fn global() -> Option<&'static OasConfig> {
    GLOBAL.get()
}

/// 显式设备 serial：环境变量（由 config.apply_env 或用户设置）优先，其次全局 config。
pub fn device_serial() -> Option<String> {
    if let Ok(s) = std::env::var("OAS_DEVICE_SERIAL") {
        let s = s.trim().to_string();
        if !s.is_empty() {
            return Some(s);
        }
    }
    global().and_then(|c| {
        let s = c.device.serial.trim();
        (!s.is_empty()).then(|| s.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_production_sane() {
        let c = OasConfig::default();
        assert_eq!(c.serve.port, 8788);
        assert_eq!(c.serve.bind, "127.0.0.1");
        // PR1: Runner is a separate surface with its own default port.
        assert_eq!(c.runner.port, 8789);
        assert_eq!(c.runner.bind, "127.0.0.1");
        assert_ne!(c.runner.port, c.serve.port);
        assert!(c.require_serial().is_err(), "no implicit device");
        assert_eq!(c.tasks_dir(), PathBuf::from("tasks"));
        assert_eq!(c.packages_dir(), None, "packages dir is optional");
        assert_eq!(
            OasConfig::from_json(r#"{"paths":{"packages_dir":"D:/pkg/packages"}}"#)
                .unwrap()
                .packages_dir(),
            Some(PathBuf::from("D:/pkg/packages"))
        );
    }

    #[test]
    fn parses_the_optional_device_lease() {
        // PR6: the field is additive and optional — an old live.json without it
        // keeps parsing and serialising exactly as before (skip_serializing_if).
        assert_eq!(OasConfig::default().device.lease, None);
        let c = OasConfig::from_json(r#"{"device":{"lease":"state/device.lease"}}"#)
            .expect("lease setting parses");
        assert_eq!(c.device.lease.as_deref(), Some("state/device.lease"));
        let json = serde_json::to_value(&OasConfig::default()).expect("serialize default");
        assert!(
            json["device"].get("lease").is_none(),
            "an unset lease must not change the live.json shape: {json}"
        );
    }

    #[test]
    fn parses_and_validates_the_runner_section() {
        let c = OasConfig::from_json(r#"{"runner":{"port":9001,"bind":"127.0.0.1"}}"#).unwrap();
        assert_eq!(c.runner.port, 9001);
        // Unknown keys stay rejected, and a colliding port or a non-loopback
        // bind is refused instead of being silently ignored (design Q1/A.7).
        assert!(OasConfig::from_json(r#"{"runner":{"port":0}}"#).is_err());
        assert!(OasConfig::from_json(r#"{"runner":{"unknown":1}}"#).is_err());
        assert!(OasConfig::from_json(
            r#"{"serve":{"port":9001},"runner":{"port":9001}}"#
        )
        .is_err());
        assert!(OasConfig::from_json(r#"{"runner":{"bind":"0.0.0.0"}}"#).is_err());
        assert!(OasConfig::from_json(r#"{"runner":{"bind":"localhost"}}"#).is_ok());
    }

    #[test]
    fn parses_old_live_json_without_the_exchange_section() {
        // Backwards compatibility (P0-1a): a deployed live.json predates the
        // exchange/notify sections and must parse unchanged and *stay* off.
        let old = r#"{
          "schema_version": 1,
          "serve": {"port": 8788, "bind": "127.0.0.1", "ui": "ui"},
          "runner": {"port": 8789, "bind": "127.0.0.1"},
          "paths": {"tasks_dir": "tasks", "runs_dir": "runs", "library_dir": "library"},
          "ocr": {"model": "models/rec.onnx", "det_model": "models/det.onnx"},
          "device": {"adb_path": "adb/adb.exe", "serial": "127.0.0.1:16384"},
          "scheduler": {"plan": "scheduler.json", "state": "s.state.json", "report": "r.json"}
        }"#;
        let c = OasConfig::from_json(old).expect("old live.json parses");
        assert_eq!(c.exchange, ExchangeConfig::default());
        assert!(!c.exchange.enabled, "exchange must stay opt-in");
        assert_eq!(c.exchange.dir, "exchange");
        assert_eq!(c.exchange.poll_secs, 3);
        assert_eq!(c.exchange.notify.runner_url, "http://127.0.0.1:8789");
        assert_eq!(c.exchange.notify.timeout_ms, 800);
        assert_eq!(c.exchange.notify.enabled, false);
        // ...and re-serialising it must not grow an `exchange` section.
        let json = serde_json::to_value(&c).expect("serialize");
        assert!(
            json.get("exchange").is_none(),
            "an unset exchange must not change the live.json shape: {json}"
        );
    }

    #[test]
    fn parses_and_validates_the_exchange_section() {
        let c = OasConfig::from_json(
            r#"{"exchange":{"dir":"../shared/exchange","enabled":true,"poll_secs":5,
                "notify":{"enabled":true,"runner_url":"http://127.0.0.1:8789","timeout_ms":1200}}}"#,
        )
        .expect("exchange section parses");
        assert_eq!(c.exchange.dir, "../shared/exchange");
        assert!(c.exchange.enabled);
        assert_eq!(c.exchange.poll_secs, 5);
        assert_eq!(c.exchange.notify.timeout_ms, 1200);
        assert_eq!(
            serde_json::to_value(&c.exchange).unwrap()["notify"]["runner_url"],
            "http://127.0.0.1:8789"
        );

        // Defaults are valid, so `enabled:true` alone is enough to opt in.
        assert!(OasConfig::from_json(r#"{"exchange":{"enabled":true}}"#).is_ok());
        // Unknown keys are rejected (deny_unknown_fields) at both levels.
        assert!(OasConfig::from_json(r#"{"exchange":{"unknown":1}}"#).is_err());
        assert!(OasConfig::from_json(r#"{"exchange":{"notify":{"unknown":1}}}"#).is_err());
        // Enabled but blank dir is refused.
        assert!(OasConfig::from_json(r#"{"exchange":{"enabled":true,"dir":"  "}}"#).is_err());
        // Notify URL must be loopback + http (mirror of the runner.bind rule).
        for bad in [
            r#"{"exchange":{"notify":{"runner_url":"http://0.0.0.0:8789"}}}"#,
            r#"{"exchange":{"notify":{"runner_url":"http://example.com:8789"}}}"#,
            r#"{"exchange":{"notify":{"runner_url":"https://127.0.0.1:8789"}}}"#,
            r#"{"exchange":{"notify":{"runner_url":"127.0.0.1:8789"}}}"#,
            r#"{"exchange":{"notify":{"runner_url":"http://user@127.0.0.1:8789"}}}"#,
        ] {
            assert!(OasConfig::from_json(bad).is_err(), "{bad} must be refused");
        }
        for ok in [
            "http://127.0.0.1:8789",
            "http://localhost:8789",
            "http://[::1]:8789",
        ] {
            let json = format!(r#"{{"exchange":{{"notify":{{"runner_url":"{ok}"}}}}}}"#);
            assert!(OasConfig::from_json(&json).is_ok(), "{ok} must be accepted");
        }
        // Range checks.
        assert!(OasConfig::from_json(r#"{"exchange":{"notify":{"timeout_ms":99}}}"#).is_err());
        assert!(OasConfig::from_json(r#"{"exchange":{"notify":{"timeout_ms":5001}}}"#).is_err());
        assert!(OasConfig::from_json(r#"{"exchange":{"poll_secs":3601}}"#).is_err());
        // Empty URL is tolerated only while notify is off (not configured).
        assert!(OasConfig::from_json(r#"{"exchange":{"notify":{"runner_url":""}}}"#).is_ok());
        assert!(OasConfig::from_json(
            r#"{"exchange":{"notify":{"enabled":true,"runner_url":""}}}"#
        )
        .is_err());
    }

    #[test]
    fn exchange_dir_follows_the_live_config_dir_or_an_absolute_setting() {
        let config = Path::new("pkg").join("config").join("live.json");
        // Relative: against the live config file's directory, never the CWD.
        assert_eq!(
            exchange_dir_from(Some(&config), "exchange"),
            Some(Path::new("pkg").join("config").join("exchange"))
        );
        assert_eq!(
            exchange_dir_from(Some(&config), "../shared/exchange"),
            Some(Path::new("pkg").join("config").join("../shared/exchange"))
        );
        // Absolute: used verbatim, even without a loaded config.
        let abs = std::env::temp_dir().join("oas-shared-exchange");
        assert_eq!(
            exchange_dir_from(None, &abs.display().to_string()),
            Some(abs)
        );
        // Blank / relative-without-config -> unusable (not a guess).
        assert_eq!(exchange_dir_from(Some(&config), "  "), None);
        assert_eq!(exchange_dir_from(None, "exchange"), None);
    }

    #[test]
    fn parses_example_and_serial() {
        let text = r#"{
          "schema_version": 1,
          "serve": {"port": 8788, "bind": "127.0.0.1", "ui": "ui"},
          "paths": {"tasks_dir": "tasks", "runs_dir": "runs", "library_dir": "library"},
          "ocr": {"model": "models/rec.onnx", "det_model": "models/det.onnx"},
          "device": {"adb_path": "adb/adb.exe", "serial": "127.0.0.1:16384",
                     "reconnect_endpoints": ["127.0.0.1:16384"]},
          "scheduler": {"plan": "scheduler.json", "state": "s.state.json", "report": "r.json"}
        }"#;
        let c = OasConfig::from_json(text).unwrap();
        assert_eq!(c.require_serial().unwrap(), "127.0.0.1:16384");
        assert_eq!(c.ocr.det_model, "models/det.onnx");
        assert_eq!(c.scheduler.state, "s.state.json");
    }

    #[test]
    fn relative_adb_path_is_resolved_against_config_dir() {
        let dir = std::env::temp_dir().join(format!(
            "oas-config-resolve-{}-{:?}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(dir.join("config")).unwrap();
        std::fs::create_dir_all(dir.join("adb")).unwrap();
        std::fs::write(dir.join("adb").join("adb.exe"), b"fake").unwrap();
        let cfg_path = dir.join("config").join("live.json");
        std::fs::write(
            &cfg_path,
            r#"{"schema_version":1,"device":{"adb_path":"adb/adb.exe","serial":"127.0.0.1:16384"}}"#,
        )
        .unwrap();
        let c = OasConfig::load(&cfg_path).expect("load");
        assert_eq!(
            PathBuf::from(&c.device.adb_path),
            dir.join("adb").join("adb.exe")
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn rejects_bad_values() {
        assert!(OasConfig::from_json(r#"{"serve":{"port":0}}"#).is_err());
        assert!(OasConfig::from_json(r#"{"schema_version":9}"#).is_err());
        assert!(OasConfig::from_json(r#"{"device":{"serial":"two words"}}"#).is_err());
        assert!(OasConfig::from_json(r#"{"unknown_key":1}"#).is_err());
    }

    #[test]
    fn load_missing_file_is_error() {
        let e = OasConfig::load(Path::new("no-such-dir/no-such.json")).unwrap_err();
        assert!(e.contains("read config"), "{e}");
    }
}

