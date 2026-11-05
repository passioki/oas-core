//! Studio → Runner **publish exchange** (P0-1a; design
//! `RUNNER-SPLIT-DESIGN.md §5.3/§5.4/§5.7`, kept as a minimal vertical slice).
//!
//! The exchange directory is the **file truth** that lets the Runner discover
//! what the Studio published, even when the two apps are never running at the
//! same time (`GET /runner/updates` polls it). A loopback HTTP hint is only an
//! accelerator and must never be required for correctness:
//!
//! ```text
//! <exchange_dir>/publish-index.json            # atomic; Studio is the only writer (schema 1)
//! <exchange_dir>/incoming/<file>.oaspkg        # published package copy
//! <exchange_dir>/incoming/<file>.oaspkg.sha256 # sha256sum-compatible sidecar
//! ```
//!
//! * [`announce_publish`] is the **Studio** side: copy the freshly built
//!   `.oaspkg` into `incoming/` (skip when the same sha256 is already there),
//!   upsert one event by `event_id = package_id` (content addressed, therefore
//!   idempotent), bump `seq` and keep the newest [`MAX_EVENTS`] events.
//! * [`read_index`] is the **Runner** side: a missing file is an empty index,
//!   anything unreadable/invalid is a loud `Err` that the HTTP layer degrades to
//!   `200` + `error` (the Runner page must stay usable — fail-soft, like
//!   `/runner/tasks`).
//! * `config/notifications.json` ([`read_notifications`] / [`ack_events`]) is
//!   the Runner-local "seen" state behind `POST /runner/updates/ack`; it is
//!   read-modify-written atomically and never touched by the Studio.
//!
//! Everything here is plain `std::fs` + `Path` joins (no platform branch) and
//! uses forward slashes in the JSON contract (`package_file`), so the same
//! directory layout works for a future Android usage client.

use std::collections::BTreeMap;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

/// `publish-index.json` name inside the exchange directory.
pub const INDEX_FILE_NAME: &str = "publish-index.json";
/// Subdirectory holding the published `.oaspkg` copies.
pub const INCOMING_DIR_NAME: &str = "incoming";
/// On-disk schema version of `publish-index.json`.
pub const INDEX_SCHEMA_VERSION: u32 = 1;
/// Upper bound of events kept in the index (newest win; design §5.3).
pub const MAX_EVENTS: usize = 200;
/// Refuse absurd index files instead of loading them (design §5.6 cap).
pub const MAX_INDEX_BYTES: u64 = 2 * 1024 * 1024;
/// Runner-local seen store, next to the live `--config` file.
pub const NOTIFICATIONS_FILE_NAME: &str = "notifications.json";
/// On-disk schema version of `notifications.json`.
pub const NOTIFICATIONS_SCHEMA_VERSION: u32 = 1;

// ---------------------------------------------------------------------------
// publish-index.json (schema 1)
// ---------------------------------------------------------------------------

/// Where a package came from (copied from the package manifest `source`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PublishSource {
    #[serde(default)]
    pub repo: String,
    #[serde(default)]
    pub commit: String,
}

/// Who wrote the index (Studio tool context; informational only).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Publisher {
    #[serde(default)]
    pub surface: String,
    #[serde(default)]
    pub repo: String,
    #[serde(default)]
    pub commit: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub dirty: bool,
}

/// One published task package.
///
/// `event_id` is the content-addressed package id (`pkg-<task>-<ver>-<sha12>`),
/// so publishing the same bytes twice is the same event and a content change is
/// a new one (design §5.5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublishEvent {
    pub event_id: String,
    pub task_id: String,
    pub task_version: u32,
    pub package_id: String,
    /// Path **relative to the exchange dir** (`incoming/<file>.oaspkg`), always
    /// forward slashes (cross-platform contract).
    pub package_file: String,
    pub sha256: String,
    #[serde(default)]
    pub size_bytes: u64,
    #[serde(default)]
    pub published_at_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<PublishSource>,
}

/// Whole `publish-index.json` document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublishIndex {
    pub schema_version: u32,
    #[serde(default)]
    pub updated_at_ms: u64,
    #[serde(default)]
    pub seq: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publisher: Option<Publisher>,
    #[serde(default)]
    pub events: Vec<PublishEvent>,
}

impl Default for PublishIndex {
    fn default() -> Self {
        Self {
            schema_version: INDEX_SCHEMA_VERSION,
            updated_at_ms: 0,
            seq: 0,
            publisher: None,
            events: Vec::new(),
        }
    }
}

/// `<exchange_dir>/publish-index.json`.
pub fn index_path(exchange_dir: &Path) -> PathBuf {
    exchange_dir.join(INDEX_FILE_NAME)
}

/// Read the index. Missing = empty; unreadable/invalid/unsupported = `Err`
/// with the path in the message (the HTTP layer decides how loud to be).
pub fn read_index(exchange_dir: &Path) -> Result<PublishIndex, String> {
    let path = index_path(exchange_dir);
    let meta = match std::fs::metadata(&path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(PublishIndex::default()),
        Err(e) => return Err(format!("read `{}`: {e}", path.display())),
    };
    if meta.len() > MAX_INDEX_BYTES {
        return Err(format!(
            "publish index `{}` is {} bytes (max {MAX_INDEX_BYTES})",
            path.display(),
            meta.len()
        ));
    }
    let text =
        std::fs::read_to_string(&path).map_err(|e| format!("read `{}`: {e}", path.display()))?;
    let index: PublishIndex = serde_json::from_str(&text)
        .map_err(|e| format!("publish index `{}` is invalid JSON: {e}", path.display()))?;
    if index.schema_version != INDEX_SCHEMA_VERSION {
        return Err(format!(
            "publish index `{}` has unsupported schema_version {} (expected {})",
            path.display(),
            index.schema_version,
            INDEX_SCHEMA_VERSION
        ));
    }
    Ok(index)
}

/// Upsert one event by `event_id`, bump `seq` and trim to [`MAX_EVENTS`].
///
/// Returning the new `seq` keeps the caller from having to re-read the index;
/// `now_ms` is passed in so this stays a pure function and unit tests are
/// deterministic. Re-announcing the same content keeps the event count stable
/// (idempotent) while `seq` still advances — it is a write counter, not an
/// event counter.
pub fn upsert_event(index: &mut PublishIndex, event: PublishEvent, now_ms: u64) -> u64 {
    index.events.retain(|e| e.event_id != event.event_id);
    index.events.push(event);
    if index.events.len() > MAX_EVENTS {
        let excess = index.events.len() - MAX_EVENTS;
        index.events.drain(..excess);
    }
    index.updated_at_ms = now_ms;
    index.seq = index.seq.saturating_add(1);
    index.seq
}

/// Write the index atomically (sibling tmp + rename), creating the dir.
pub fn write_index(exchange_dir: &Path, index: &PublishIndex) -> Result<(), String> {
    let pretty =
        serde_json::to_string_pretty(index).map_err(|e| format!("serialize publish index: {e}"))?;
    write_text_atomic(&index_path(exchange_dir), &format!("{pretty}\n"))
}

// ---------------------------------------------------------------------------
// Studio publish → exchange
// ---------------------------------------------------------------------------

/// Inputs of one [`announce_publish`] call (the Studio handler maps a
/// `BuildOutcome` onto this; the module stays independent of `package_cmd`).
#[derive(Debug, Clone)]
pub struct AnnounceInput {
    /// The freshly built `.oaspkg` (Studio `dist/packages/...`).
    pub package_path: PathBuf,
    pub package_id: String,
    pub task_id: String,
    pub task_version: u32,
    pub sha256: String,
    pub published_at_ms: u64,
    pub source: Option<PublishSource>,
    pub publisher: Option<Publisher>,
}

/// Result of one announce (also what `POST /task/publish` reports).
#[derive(Debug, Clone)]
pub struct AnnounceResult {
    pub event_id: String,
    pub index_path: PathBuf,
    /// Relative path inside the exchange dir (`incoming/<file>.oaspkg`).
    pub package_file: String,
    /// Absolute-ish path of the copy (exchange dir joined, as configured).
    pub package_path: PathBuf,
    pub size_bytes: u64,
    pub copied: bool,
    pub seq: u64,
}

/// Copy `package_path` into `<exchange_dir>/incoming/` and upsert the index.
///
/// The exchange is an additive channel: the caller (`serve::handle_task_publish`)
/// turns an `Err` into a warning instead of failing the publish. The copy is
/// skipped when the target already has the same sha256, which makes a
/// force-republish of identical bytes cheap and keeps `incoming/` stable.
pub fn announce_publish(
    exchange_dir: &Path,
    input: &AnnounceInput,
) -> Result<AnnounceResult, String> {
    if exchange_dir.as_os_str().is_empty() {
        return Err("exchange dir is empty".to_string());
    }
    let file_name = input
        .package_path
        .file_name()
        .and_then(|n| n.to_str())
        .filter(|n| !n.is_empty())
        .ok_or_else(|| {
            format!(
                "package path `{}` has no usable file name",
                input.package_path.display()
            )
        })?
        .to_string();
    let package_rel = format!("{INCOMING_DIR_NAME}/{file_name}");
    // Validate before touching the filesystem shared with the Runner.
    let target = resolve_inside(exchange_dir, &package_rel)?;
    let incoming = exchange_dir.join(INCOMING_DIR_NAME);
    std::fs::create_dir_all(&incoming)
        .map_err(|e| format!("create incoming dir `{}`: {e}", incoming.display()))?;

    let same_content = match oas_task::package::sha256_file(&target) {
        Ok(existing) => existing.eq_ignore_ascii_case(input.sha256.trim()),
        Err(_) => false,
    };
    let copied = if same_content {
        false
    } else {
        std::fs::copy(&input.package_path, &target).map_err(|e| {
            format!(
                "copy package `{}` -> `{}`: {e}",
                input.package_path.display(),
                target.display()
            )
        })?;
        true
    };
    let size_bytes = std::fs::metadata(&input.package_path)
        .map(|m| m.len())
        .unwrap_or_else(|_| std::fs::metadata(&target).map(|m| m.len()).unwrap_or(0));

    // `sha256sum -c` compatible sidecar (same format as the local publish).
    // Best effort: a sidecar failure must not invalidate an otherwise good
    // exchange event (same rule as `package.rs`).
    let sidecar = target.with_extension("oaspkg.sha256");
    let _ = write_text_atomic(
        &sidecar,
        &format!("{}  {file_name}\n", input.sha256.trim().to_lowercase()),
    );

    let mut index = read_index(exchange_dir)?;
    if let Some(publisher) = &input.publisher {
        index.publisher = Some(publisher.clone());
    }
    let event = PublishEvent {
        event_id: input.package_id.clone(),
        task_id: input.task_id.clone(),
        task_version: input.task_version,
        package_id: input.package_id.clone(),
        package_file: package_rel.clone(),
        sha256: input.sha256.clone(),
        size_bytes,
        published_at_ms: input.published_at_ms,
        source: input.source.clone(),
    };
    let seq = upsert_event(&mut index, event, input.published_at_ms);
    write_index(exchange_dir, &index)?;
    Ok(AnnounceResult {
        event_id: input.package_id.clone(),
        index_path: index_path(exchange_dir),
        package_file: package_rel,
        package_path: target,
        size_bytes,
        copied,
        seq,
    })
}

/// Which action the Runner UI shows for one event, given the installed version
/// of the same task (`None` = not installed).
pub fn action_for(installed_version: Option<u32>, published_version: u32) -> &'static str {
    match installed_version {
        None => "install",
        Some(v) if published_version > v => "upgrade",
        Some(v) if published_version < v => "downgrade",
        Some(_) => "reinstall",
    }
}

// ---------------------------------------------------------------------------
// Runner-local notifications.json (schema 1)
// ---------------------------------------------------------------------------

/// Seen state of one event (`dismissed` / `installed`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeenEvent {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub updated_ms: u64,
}

/// Whole `config/notifications.json` document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationsFile {
    pub schema_version: u32,
    #[serde(default)]
    pub updated_ms: u64,
    #[serde(default)]
    pub seen: BTreeMap<String, SeenEvent>,
}

impl Default for NotificationsFile {
    fn default() -> Self {
        Self {
            schema_version: NOTIFICATIONS_SCHEMA_VERSION,
            updated_ms: 0,
            seen: BTreeMap::new(),
        }
    }
}

/// Read the seen store. Missing = empty; unreadable/invalid = `Err`.
pub fn read_notifications(path: &Path) -> Result<NotificationsFile, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(NotificationsFile::default()),
        Err(e) => return Err(format!("read notifications `{}`: {e}", path.display())),
    };
    let file: NotificationsFile = serde_json::from_str(&text).map_err(|e| {
        format!(
            "notifications file `{}` is invalid JSON: {e}",
            path.display()
        )
    })?;
    if file.schema_version != NOTIFICATIONS_SCHEMA_VERSION {
        return Err(format!(
            "notifications file `{}` has unsupported schema_version {} (expected {})",
            path.display(),
            file.schema_version,
            NOTIFICATIONS_SCHEMA_VERSION
        ));
    }
    Ok(file)
}

/// Write the seen store atomically (read-modify-write helper for [`ack_events`]).
pub fn write_notifications(path: &Path, file: &NotificationsFile) -> Result<(), String> {
    let pretty =
        serde_json::to_string_pretty(file).map_err(|e| format!("serialize notifications: {e}"))?;
    write_text_atomic(path, &format!("{pretty}\n"))
}

/// Serializes the read-modify-write cycle inside one Runner process; the atomic
/// rename makes each write crash-safe, the mutex keeps two concurrent ACKs from
/// dropping each other (same single-writer rule as [`crate::runner_config`]).
static ACK_LOCK: Mutex<()> = Mutex::new(());

/// Mark `event_ids` as `status` (`dismissed` / `installed`) and return the new
/// `updated_ms`. Idempotent: re-acking the same ids just refreshes the clock.
pub fn ack_events(
    path: &Path,
    event_ids: &[String],
    status: &str,
    now_ms: u64,
) -> Result<u64, String> {
    if status != "dismissed" && status != "installed" {
        return Err(format!(
            "ack status `{status}` must be \"dismissed\" or \"installed\""
        ));
    }
    let _guard = ACK_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut file = read_notifications(path)?;
    for id in event_ids {
        file.seen.insert(
            id.clone(),
            SeenEvent {
                status: status.to_string(),
                updated_ms: now_ms,
            },
        );
    }
    file.updated_ms = now_ms;
    write_notifications(path, &file)?;
    Ok(now_ms)
}

// ---------------------------------------------------------------------------
// Path contract
// ---------------------------------------------------------------------------

/// Resolve a **relative, forward-slash** path (`incoming/<file>.oaspkg`) inside
/// `root`, rejecting absolute paths, drive letters, backslashes, `..` and `.`
/// components (design §5.6: a shared directory must not be able to point the
/// Runner at an arbitrary file). Returns the joined path, not canonicalized.
pub fn resolve_inside(root: &Path, rel: &str) -> Result<PathBuf, String> {
    if rel.trim().is_empty() {
        return Err("relative path is empty".to_string());
    }
    if rel.contains('\\') {
        return Err(format!(
            "path `{rel}` must use forward slashes (cross-platform contract)"
        ));
    }
    let path = Path::new(rel);
    if path.is_absolute() {
        return Err(format!("path `{rel}` must be relative"));
    }
    for component in path.components() {
        match component {
            Component::Normal(_) => {}
            _ => {
                return Err(format!(
                    "path `{rel}` must stay inside the exchange directory (no `.`/`..`/prefix components)"
                ))
            }
        }
    }
    Ok(root.join(path))
}

// ---------------------------------------------------------------------------
// Atomic write helper
// ---------------------------------------------------------------------------
//
// Deliberately a module-local copy (like `runner_config`/`package_cmd` each keep
// their own) so this module can later move to the separate Runner repository
// without dragging half of `serve.rs` along. `std::fs::rename` replaces an
// existing file on both Windows and Unix; a failed write/rename removes the tmp
// file and reports the path.

static WRITE_SEQ: AtomicU64 = AtomicU64::new(0);

fn write_text_atomic(path: &Path, text: &str) -> Result<(), String> {
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    if !parent.as_os_str().is_empty() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("create dir `{}`: {e}", parent.display()))?;
    }
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| INDEX_FILE_NAME.to_string());
    let seq = WRITE_SEQ.fetch_add(1, Ordering::Relaxed);
    let tmp = parent.join(format!(".{name}.tmp-{}-{seq}", std::process::id()));
    if let Err(e) = std::fs::write(&tmp, text) {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("write `{}`: {e}", tmp.display()));
    }
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!(
            "rename `{}` -> `{}`: {e}",
            tmp.display(),
            path.display()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            static SEQ: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "oasd-exchange-{}-{tag}-{}",
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

    fn event(id: &str, version: u32) -> PublishEvent {
        PublishEvent {
            event_id: id.to_string(),
            task_id: "任务".to_string(),
            task_version: version,
            package_id: id.to_string(),
            package_file: format!("incoming/{id}.oaspkg"),
            sha256: "0".repeat(64),
            size_bytes: 1,
            published_at_ms: 1,
            source: None,
        }
    }

    fn announce_input(td: &Path, bytes: &[u8]) -> AnnounceInput {
        let package = td.join("built").join("任务-v1.oaspkg");
        std::fs::create_dir_all(package.parent().unwrap()).unwrap();
        std::fs::write(&package, bytes).unwrap();
        AnnounceInput {
            package_path: package,
            package_id: "pkg-任务-1-abc123abc123".to_string(),
            task_id: "任务".to_string(),
            task_version: 1,
            sha256: sha256(bytes),
            published_at_ms: 111,
            source: Some(PublishSource {
                repo: "D:/repo".into(),
                commit: "abc1234".into(),
            }),
            publisher: Some(Publisher {
                surface: "studio".into(),
                repo: "D:/repo".into(),
                commit: "abc1234".into(),
                version: "0.1.0".into(),
                dirty: false,
            }),
        }
    }

    fn sha256(bytes: &[u8]) -> String {
        oas_task::package::sha256_bytes(bytes)
    }

    #[test]
    fn upsert_is_idempotent_by_event_id() {
        let mut index = PublishIndex::default();
        assert_eq!(upsert_event(&mut index, event("pkg-a", 1), 10), 1);
        assert_eq!(index.events.len(), 1);
        // Same content/event -> still one event, but the write counter advances.
        assert_eq!(upsert_event(&mut index, event("pkg-a", 1), 20), 2);
        assert_eq!(index.events.len(), 1);
        assert_eq!(index.seq, 2);
        assert_eq!(index.updated_at_ms, 20);
        // A different event is appended.
        assert_eq!(upsert_event(&mut index, event("pkg-b", 2), 30), 3);
        assert_eq!(
            index
                .events
                .iter()
                .map(|e| e.event_id.as_str())
                .collect::<Vec<_>>(),
            vec!["pkg-a", "pkg-b"]
        );
    }

    #[test]
    fn upsert_keeps_only_the_newest_200_events() {
        let mut index = PublishIndex::default();
        for i in 0..205u32 {
            upsert_event(&mut index, event(&format!("pkg-{i}"), i), u64::from(i));
        }
        assert_eq!(index.events.len(), MAX_EVENTS);
        assert_eq!(index.events[0].event_id, "pkg-5", "oldest five are dropped");
        assert_eq!(index.events[MAX_EVENTS - 1].event_id, "pkg-204");
        assert_eq!(index.seq, 205);
    }

    #[test]
    fn read_index_missing_is_empty_and_bad_json_is_loud() {
        let td = TempDir::new("read-index");
        let empty = read_index(td.path()).expect("missing index = empty");
        assert_eq!(empty, PublishIndex::default());

        std::fs::write(index_path(td.path()), "{ not json").unwrap();
        let err = read_index(td.path()).expect_err("bad JSON must be loud");
        assert!(err.contains("publish-index.json"), "{err}");
        assert!(err.contains("invalid JSON"), "{err}");

        std::fs::write(index_path(td.path()), r#"{"schema_version":2,"events":[]}"#).unwrap();
        let err = read_index(td.path()).expect_err("unsupported schema must be loud");
        assert!(err.contains("schema_version 2"), "{err}");
    }

    #[test]
    fn relative_package_paths_cannot_escape_the_exchange_dir() {
        let root = Path::new("exchange");
        assert_eq!(
            resolve_inside(root, "incoming/任务-v1.oaspkg").unwrap(),
            root.join("incoming").join("任务-v1.oaspkg")
        );
        for bad in [
            "",
            "../secret",
            "incoming/../../secret",
            "/abs/pkg.oaspkg",
            "C:/abs/pkg.oaspkg",
            "incoming\\任务.oaspkg",
            "./incoming/oaspkg",
        ] {
            assert!(
                resolve_inside(root, bad).is_err(),
                "`{bad}` must be refused"
            );
        }
    }

    #[test]
    fn announce_copies_package_writes_index_and_is_idempotent() {
        let td = TempDir::new("announce");
        let exchange = td.path().join("exchange");
        let input = announce_input(td.path(), b"package-bytes");

        let first = announce_publish(&exchange, &input).expect("first announce");
        assert!(first.copied);
        assert_eq!(first.event_id, "pkg-任务-1-abc123abc123");
        assert_eq!(first.package_file, "incoming/任务-v1.oaspkg");
        assert!(first.index_path.is_file());
        assert!(first.package_path.is_file(), "copy must exist");
        assert_eq!(
            std::fs::read(&first.package_path).unwrap(),
            b"package-bytes"
        );
        assert_eq!(first.seq, 1);
        assert_eq!(first.size_bytes, b"package-bytes".len() as u64);
        assert!(
            first.package_path.with_extension("oaspkg.sha256").is_file(),
            "sidecar next to the copy"
        );

        let index = read_index(&exchange).expect("index readable");
        assert_eq!(index.schema_version, 1);
        assert_eq!(index.seq, 1);
        assert_eq!(index.events.len(), 1);
        assert_eq!(index.events[0].event_id, first.event_id);
        assert_eq!(index.events[0].task_version, 1);
        assert_eq!(index.events[0].sha256, input.sha256);
        assert_eq!(
            index.events[0].source.as_ref().map(|s| s.commit.as_str()),
            Some("abc1234")
        );
        assert_eq!(
            index.publisher.as_ref().map(|p| p.surface.as_str()),
            Some("studio")
        );

        // Same bytes again: no duplicate event, copy skipped, seq advances.
        let second = announce_publish(&exchange, &input).expect("second announce");
        assert!(!second.copied, "same sha256 is not copied twice");
        assert_eq!(second.seq, 2);
        let index = read_index(&exchange).unwrap();
        assert_eq!(index.events.len(), 1);
        assert_eq!(index.updated_at_ms, 111);
    }

    #[test]
    fn announce_overwrites_a_stale_copy_with_the_same_file_name() {
        let td = TempDir::new("announce-overwrite");
        let exchange = td.path().join("exchange");
        let mut input = announce_input(td.path(), b"old-bytes");
        announce_publish(&exchange, &input).expect("first announce");

        // Force republish: same file name, different content/sha -> new event.
        std::fs::write(&input.package_path, b"new-bytes").unwrap();
        input.sha256 = sha256(b"new-bytes");
        input.package_id = "pkg-任务-1-def456def456".to_string();
        input.task_version = 2;
        let second = announce_publish(&exchange, &input).expect("second announce");
        assert!(second.copied, "different sha256 must replace the copy");
        assert_eq!(
            std::fs::read(&second.package_path).unwrap(),
            b"new-bytes",
            "incoming/ holds the newest bytes"
        );
        let index = read_index(&exchange).unwrap();
        assert_eq!(index.events.len(), 2, "new content = new event");
    }

    #[test]
    fn notifications_roundtrip_and_ack_is_idempotent() {
        let td = TempDir::new("notifications");
        let path = td.path().join("config").join(NOTIFICATIONS_FILE_NAME);
        assert_eq!(
            read_notifications(&path).unwrap(),
            NotificationsFile::default(),
            "missing file = empty"
        );

        let acked = ack_events(
            &path,
            &["pkg-a".to_string(), "pkg-b".to_string()],
            "dismissed",
            123,
        )
        .expect("ack");
        assert_eq!(acked, 123);
        let file = read_notifications(&path).expect("notifications readable");
        assert_eq!(file.schema_version, 1);
        assert_eq!(file.updated_ms, 123);
        assert_eq!(file.seen["pkg-a"].status, "dismissed");
        assert_eq!(file.seen["pkg-b"].updated_ms, 123);

        // Re-ack only pkg-a as installed: pkg-b keeps its old status (RMW).
        ack_events(&path, &["pkg-a".to_string()], "installed", 456).expect("second ack");
        let file = read_notifications(&path).unwrap();
        assert_eq!(file.seen["pkg-a"].status, "installed");
        assert_eq!(file.seen["pkg-b"].status, "dismissed");
        assert_eq!(file.updated_ms, 456);

        assert!(
            ack_events(&path, &["pkg-a".to_string()], "bogus", 1).is_err(),
            "unknown statuses are refused"
        );
        std::fs::write(&path, "{ broken").unwrap();
        let err = read_notifications(&path).expect_err("corrupt store must be loud");
        assert!(err.contains("notifications.json"), "{err}");
    }

    #[test]
    fn action_reflects_the_installed_version() {
        assert_eq!(action_for(None, 3), "install");
        assert_eq!(action_for(Some(2), 3), "upgrade");
        assert_eq!(action_for(Some(3), 3), "reinstall");
        assert_eq!(action_for(Some(4), 3), "downgrade");
    }
}
