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
    pub paths: PathsConfig,
    pub ocr: OcrConfig,
    pub device: DeviceConfig,
    pub scheduler: SchedulerConfig,
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
pub struct PathsConfig {
    pub tasks_dir: String,
    pub runs_dir: String,
    pub library_dir: String,
}

impl Default for PathsConfig {
    fn default() -> Self {
        Self {
            tasks_dir: "tasks".into(),
            runs_dir: "runs".into(),
            library_dir: "library".into(),
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
}

impl Default for DeviceConfig {
    fn default() -> Self {
        Self {
            adb_path: String::new(),
            serial: String::new(),
            reconnect_endpoints: Vec::new(),
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
        if self.paths.tasks_dir.trim().is_empty() {
            return Err("paths.tasks_dir must not be empty".into());
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
        assert!(c.require_serial().is_err(), "no implicit device");
        assert_eq!(c.tasks_dir(), PathBuf::from("tasks"));
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

