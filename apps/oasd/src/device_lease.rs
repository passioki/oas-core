//! Cross-process **device lease** (design `docs/RUNNER-STUDIO-SPLIT-PLAN.md`
//! §A.6.4, PR6).
//!
//! The scheduler lock (`<state>.lock`, [`crate::schedule`]) serialises
//! *schedulers*; it does not stop a Studio trial run and the Runner from
//! clicking the same phone at the same time. This module adds the missing
//! single-resource lock for the **physical device**:
//!
//! ```text
//! config/device.lease = {"pid":1234,"surface":"studio","owner":"studio",
//!                        "mode":"editor-run","started_ms":1760000000000}
//! ```
//!
//! * `POST /task/run` (Studio) acquires `owner="studio" / mode="editor-run"`;
//! * `POST /runner/run` acquires `owner="runner" / mode="temp-run"`;
//! * the Runner's embedded scheduler acquires
//!   `owner="runner-scheduler" / mode="scheduler"` **once per pass** and drops
//!   it when the pass ends, so an idle/sleeping scheduler does not block a
//!   Runner temp run or a Studio trial (PR6 P1-5); a conflict skips the pass
//!   and is retried, it never aborts the scheduler;
//! * `oasd run` (CLI) acquires `owner="cli" / mode="run"` around one execution
//!   (PR6 P1-6). Scheduler passes / pre-acquired HTTP runs set
//!   `RunOptions::skip_lease` so the same process never double-acquires.
//!
//! Semantics mirror [`crate::schedule`]'s `SchedulerLock` on purpose:
//! `create_new` (atomic claim), a pid-based stale reclaim, and a `Drop` that
//! removes only the file this process created. A live holder — **including the
//! same pid**, because a second thread must not steal the device — is a loud
//! error carrying the holder's pid/surface/owner/mode, so the HTTP layer can
//! answer `409` with an actionable message (design §A.6.4).
//!
//! The lease is **optional**: without a live `--config` file there is no
//! well-defined `config/` directory, so [`lease_path_from_config`] returns
//! `None` and callers simply do not lock (unit tests and pure-CLI use). A
//! configured `device.lease` path is resolved relative to the **live config
//! file's directory**, so changing the process CWD can never move the lease.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// File name used when `config.device.lease` is not set (next to `live.json`).
pub const LEASE_FILE_NAME: &str = "device.lease";

/// JSON payload of a held lease (also what a conflicting caller reports).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeaseInfo {
    /// Owning process. A live pid blocks every other acquire (see module docs).
    pub pid: u32,
    /// `studio` / `runner` (derived from the owner prefix by [`DeviceLease::acquire`]).
    #[serde(default)]
    pub surface: String,
    /// Who holds the lease: `studio`, `runner`, `runner-scheduler`.
    #[serde(default)]
    pub owner: String,
    /// What is running: `editor-run`, `temp-run`, `scheduler`.
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub started_ms: u64,
}

/// An acquired lease. Dropping it removes the lease file **only if it is still
/// the file this process created** (a replaced/removed lease is left alone).
#[derive(Debug)]
pub struct DeviceLease {
    path: PathBuf,
    info: LeaseInfo,
}

impl DeviceLease {
    /// Atomically claim `path` for this process.
    ///
    /// * `owner` is the logical holder (`studio`, `runner`, `runner-scheduler`);
    ///   its first `-`-separated segment becomes [`LeaseInfo::surface`]
    ///   (`studio` / `runner`), so callers cannot disagree with themselves.
    /// * `mode` names the activity (`editor-run`, `temp-run`, `scheduler`).
    ///
    /// A live holder (even our own pid) is an `Err` whose message names the
    /// holder; a dead pid (or an unreadable/partial file left by a crashed
    /// process) is reclaimed exactly once. A second `Err` means the stale lease
    /// could not be reclaimed (permissions/lock), never a silent success.
    pub fn acquire(path: &Path, owner: &str, mode: &str) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    format!("create lease dir `{}`: {e}", parent.display())
                })?;
            }
        }
        let pid = std::process::id();
        let info = LeaseInfo {
            pid,
            surface: owner
                .split('-')
                .next()
                .filter(|s| !s.is_empty())
                .unwrap_or("runner")
                .to_string(),
            owner: owner.to_string(),
            mode: mode.to_string(),
            started_ms: now_ms(),
        };
        let payload =
            serde_json::to_string(&info).map_err(|e| format!("serialize device lease: {e}"))?;

        for attempt in 0..2 {
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
            {
                Ok(mut file) => {
                    if let Err(e) = write!(file, "{payload}") {
                        let _ = std::fs::remove_file(path);
                        return Err(format!("write lease `{}`: {e}", path.display()));
                    }
                    return Ok(DeviceLease {
                        path: path.to_path_buf(),
                        info,
                    });
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    let holder = read_lease(path);
                    if let Some(holder) = holder {
                        // Any live holder blocks us — including our own pid: a
                        // second thread in this process must not steal the
                        // device (the same rule as `SchedulerLock`).
                        if holder.pid == pid || crate::schedule::process_alive(holder.pid) {
                            return Err(conflict_message(path, &holder));
                        }
                    }
                    // Stale (dead pid / unreadable / partial): reclaim once.
                    match std::fs::remove_file(path) {
                        Ok(()) => {}
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                        Err(e) => {
                            return Err(format!(
                                "stale device lease `{}` could not be removed: {e}",
                                path.display()
                            ))
                        }
                    }
                    if attempt == 1 {
                        return Err(format!(
                            "device lease `{}`: could not reclaim a stale lease",
                            path.display()
                        ));
                    }
                }
                Err(e) => return Err(format!("device lease `{}`: {e}", path.display())),
            }
        }
        Err(format!("device lease `{}`: unreachable", path.display()))
    }

    #[cfg(test)]
    pub fn info(&self) -> &LeaseInfo {
        &self.info
    }

    #[cfg(test)]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for DeviceLease {
    fn drop(&mut self) {
        // Only delete the file this process created with this exact heartbeat:
        // if another owner replaced it, its lease must survive our Drop.
        match read_lease(&self.path) {
            Some(current)
                if current.pid == self.info.pid
                    && current.started_ms == self.info.started_ms =>
            {
                let _ = std::fs::remove_file(&self.path);
            }
            _ => {}
        }
    }
}

/// Read a lease file. Missing/unreadable/invalid => `None` (the caller treats
/// that as stale; it never guesses a pid).
pub fn read_lease(path: &Path) -> Option<LeaseInfo> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

/// Human message for a conflicting holder (shared by every acquire path and by
/// the Runner's advisory start pre-check).
pub(crate) fn conflict_message(path: &Path, holder: &LeaseInfo) -> String {
    let what = match holder.mode.as_str() {
        "scheduler" => "Runner 正在调度",
        "temp-run" => "Runner 正在临时运行",
        "editor-run" => "Studio 正在试运行",
        _ => "设备已被占用",
    };
    format!(
        "{what}（pid {}，surface={}，owner={}，mode={}）——请等它结束后再试（lease `{}`）",
        holder.pid,
        holder.surface,
        holder.owner,
        holder.mode,
        path.display()
    )
}

/// Lease path for an explicit `(config file, device.lease setting)` pair.
///
/// `None` when there is no config path at all (no `--config` => no well-defined
/// `config/` directory => the lease is disabled). A relative `configured` path
/// is resolved against the **live config file's directory**, never the CWD.
pub fn lease_path_from(config_path: Option<&Path>, configured: Option<&str>) -> Option<PathBuf> {
    let config_path = config_path?;
    let dir = config_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    match configured.map(str::trim).filter(|s| !s.is_empty()) {
        Some(raw) => {
            let raw = PathBuf::from(raw);
            Some(if raw.is_absolute() { raw } else { dir.join(raw) })
        }
        None => Some(dir.join(LEASE_FILE_NAME)),
    }
}

/// Production path: `config/device.lease` (or `device.lease` override) next to
/// the live `--config` file. `None` when no config was loaded.
pub fn lease_path_from_config() -> Option<PathBuf> {
    let configured = crate::config::global().and_then(|cfg| cfg.device.lease.as_deref());
    lease_path_from(crate::config::global_path(), configured)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "oasd-device-lease-{}-{tag}",
                std::process::id()
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

    #[test]
    fn acquire_writes_the_contract_and_drop_releases_it() {
        let td = TempDir::new("roundtrip");
        let path = td.path().join("config").join(LEASE_FILE_NAME);
        let lease = DeviceLease::acquire(&path, "runner", "temp-run").expect("acquire");
        assert_eq!(lease.path(), path.as_path());
        assert_eq!(lease.info().pid, std::process::id());
        assert_eq!(lease.info().owner, "runner");
        assert_eq!(lease.info().mode, "temp-run");
        assert_eq!(lease.info().surface, "runner");
        assert!(lease.info().started_ms > 0);

        let on_disk = read_lease(&path).expect("lease file must be valid JSON");
        assert_eq!(&on_disk, lease.info(), "the JSON contract is the in-memory info");

        drop(lease);
        assert!(!path.exists(), "Drop must release the lease");
    }

    #[test]
    fn a_second_acquire_in_the_same_process_is_a_loud_conflict() {
        let td = TempDir::new("conflict");
        let path = td.path().join(LEASE_FILE_NAME);
        let first = DeviceLease::acquire(&path, "runner-scheduler", "scheduler").expect("first");
        let err = DeviceLease::acquire(&path, "studio", "editor-run")
            .expect_err("a live holder — even our own pid — must block");
        assert!(err.contains("Runner 正在调度"), "{err}");
        assert!(err.contains(&format!("pid {}", std::process::id())), "{err}");
        assert!(err.contains("mode=scheduler"), "{err}");
        assert!(path.exists(), "the failed acquire must not steal the lease");
        drop(first);
        // ... and the device is usable again afterwards.
        let again = DeviceLease::acquire(&path, "studio", "editor-run").expect("after release");
        assert_eq!(again.info().surface, "studio");
    }

    #[test]
    fn a_stale_pid_is_reclaimed() {
        let td = TempDir::new("stale");
        let path = td.path().join(LEASE_FILE_NAME);
        // A child that is certainly dead by the time we write its pid.
        let mut command = if cfg!(windows) {
            let mut c = std::process::Command::new("cmd");
            c.args(["/C", "exit 0"]);
            c
        } else {
            std::process::Command::new("true")
        };
        let mut child = command.spawn().expect("spawn a short-lived child");
        let dead_pid = child.id();
        child.wait().expect("reap the child");

        let stale = LeaseInfo {
            pid: dead_pid,
            surface: "runner".into(),
            owner: "runner-scheduler".into(),
            mode: "scheduler".into(),
            started_ms: 1,
        };
        std::fs::write(&path, serde_json::to_string(&stale).unwrap()).expect("write stale lease");

        let lease = DeviceLease::acquire(&path, "studio", "editor-run").expect("stale reclaim");
        assert_eq!(lease.info().pid, std::process::id());
        assert_eq!(read_lease(&path).map(|i| i.mode), Some("editor-run".into()));
    }

    #[test]
    fn a_garbage_lease_file_is_reclaimed_instead_of_blocking_forever() {
        let td = TempDir::new("garbage");
        let path = td.path().join(LEASE_FILE_NAME);
        std::fs::write(&path, "{ this is not a lease").expect("write garbage");
        let lease = DeviceLease::acquire(&path, "runner", "temp-run").expect("reclaim garbage");
        assert_eq!(lease.info().owner, "runner");
    }

    #[test]
    fn drop_does_not_delete_a_lease_replaced_by_another_holder() {
        let td = TempDir::new("replaced");
        let path = td.path().join(LEASE_FILE_NAME);
        let lease = DeviceLease::acquire(&path, "runner", "temp-run").expect("acquire");
        let other = LeaseInfo {
            pid: std::process::id(),
            surface: "studio".into(),
            owner: "studio".into(),
            mode: "editor-run".into(),
            started_ms: lease.info().started_ms + 1,
        };
        std::fs::write(&path, serde_json::to_string(&other).unwrap()).expect("replace lease");
        drop(lease);
        assert_eq!(read_lease(&path), Some(other), "a replaced lease must survive Drop");
    }

    #[test]
    fn lease_path_follows_the_live_config_dir_or_the_explicit_setting() {
        let config = Path::new("pkg").join("config").join("live.json");
        // Default: a sibling of the live config file.
        assert_eq!(
            lease_path_from(Some(&config), None),
            Some(Path::new("pkg").join("config").join(LEASE_FILE_NAME))
        );
        // Empty/blank setting behaves like "not configured".
        assert_eq!(
            lease_path_from(Some(&config), Some("  ")),
            Some(Path::new("pkg").join("config").join(LEASE_FILE_NAME))
        );
        // Relative setting: relative to the config directory, not the CWD.
        assert_eq!(
            lease_path_from(Some(&config), Some("state/device.lease")),
            Some(
                Path::new("pkg")
                    .join("config")
                    .join("state/device.lease")
            )
        );
        // Absolute setting: used verbatim.
        let abs = std::env::temp_dir().join("custom-device.lease");
        assert_eq!(
            lease_path_from(Some(&config), Some(&abs.display().to_string())),
            Some(abs)
        );
        // No live config => lease disabled (tests / pure CLI).
        assert_eq!(lease_path_from(None, None), None);
        assert_eq!(lease_path_from(None, Some("device.lease")), None);
    }
}
