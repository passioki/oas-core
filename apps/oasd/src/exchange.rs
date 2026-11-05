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
use std::io::{ErrorKind, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
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
/// Upper bound of a downloaded `.oaspkg` (512 MiB; mirrors `curl --max-filesize`).
pub const MAX_PACKAGE_BYTES: u64 = 512 * 1024 * 1024;
/// Cache subdirectory (under the packages root) for remote package downloads.
pub const REMOTE_CACHE_DIR_NAME: &str = "remote";

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
    /// Minimum engine version declared by the package manifest (`engine.min_engine`).
    /// Optional and additive: an old index without it keeps parsing and is
    /// treated as "no constraint" by the Runner's compatibility pre-check.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_engine: Option<String>,
    /// Engine flow ops required by the package (`manifest.engine.flow_ops`), so
    /// the Runner UI can disable installation before clicking when its engine is
    /// older. Additive: old indexes/Runners simply ignore it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub flow_ops: Vec<String>,
    /// Optional absolute `http(s)://` package URL (P2 remote feeds). The local
    /// exchange index never sets it; a remote feed may use it when it has no
    /// `package_base`. Additive: old readers ignore it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_url: Option<String>,
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

/// Read the exchange index. Missing = empty; unreadable/invalid/unsupported =
/// `Err` with the path in the message (the HTTP layer decides how loud to be).
pub fn read_index(exchange_dir: &Path) -> Result<PublishIndex, String> {
    read_index_file(&index_path(exchange_dir))
}

/// Read/parse one `publish-index.json` at an explicit path.
///
/// Shared by the local exchange dir and by update feeds (P2: the same schema is
/// served from a static web host/GitHub). Missing = empty, a present but
/// unreadable/invalid/unsupported file is a loud `Err`.
pub fn read_index_file(path: &Path) -> Result<PublishIndex, String> {
    let meta = match std::fs::metadata(path) {
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
    let text = std::fs::read_to_string(path).map_err(|e| format!("read `{}`: {e}", path.display()))?;
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
    /// From `manifest.engine`; both optional for older call sites/tests.
    pub min_engine: Option<String>,
    pub flow_ops: Vec<String>,
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
        min_engine: input.min_engine.clone(),
        flow_ops: input.flow_ops.clone(),
        package_url: None,
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
// Studio → Runner best-effort HTTP notify (design §5.2 option B/C)
// ---------------------------------------------------------------------------

/// Self-contained executable version used for the compatibility pre-check.
pub fn engine_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Compatibility pre-check for one event, using only the additive engine fields.
///
/// `None`/empty fields mean "no constraint" (old Studio events) so old exchanges
/// keep working unchanged. `POST /runner/install` still validates the package
/// strictly; this only drives the Runner UI's "incompatible" hint.
pub fn is_compatible(min_engine: Option<&str>, flow_ops: &[String]) -> bool {
    let version_ok = match min_engine.map(str::trim).filter(|s| !s.is_empty()) {
        None => true,
        Some(minimum) => oas_task::package::version_at_least(engine_version(), minimum),
    };
    version_ok
        && flow_ops
            .iter()
            .all(|op| oas_task::package::SUPPORTED_FLOW_OPS.contains(&op.as_str()))
}

/// Minimal payload sent to `POST /runner/notify` (never trusted for installing).
pub fn notify_payload(
    event_id: &str,
    task_id: &str,
    task_version: u32,
    package_id: &str,
    sha256: &str,
    published_at_ms: u64,
) -> serde_json::Value {
    serde_json::json!({
        "event_id": event_id,
        "task_id": task_id,
        "task_version": task_version,
        "package_id": package_id,
        "sha256": sha256,
        "published_at_ms": published_at_ms,
    })
}

/// Best-effort loopback HTTP POST of one publish hint.
///
/// The exchange file is the only truth; this call only refreshes the Runner UI
/// faster when it is running (design §5.2). Failures are reported to the caller
/// and must never fail the publish. Uses `std::net` only (no new dependency).
pub fn notify_runner(
    runner_url: &str,
    payload: &serde_json::Value,
    timeout_ms: u64,
) -> Result<u16, String> {
    let (host, port) = parse_http_base(runner_url)?;
    let body = payload.to_string();
    let timeout = std::time::Duration::from_millis(timeout_ms.max(100));
    let addrs = (host.as_str(), port)
        .to_socket_addrs()
        .map_err(|e| format!("resolve notify host `{host}:{port}`: {e}"))?;
    let mut last_err: Option<String> = None;
    let mut stream = None;
    for addr in addrs {
        match TcpStream::connect_timeout(&addr, timeout) {
            Ok(s) => {
                stream = Some(s);
                break;
            }
            Err(e) => last_err = Some(format!("connect `{addr}`: {e}")),
        }
    }
    let mut stream = stream.ok_or_else(|| {
        last_err.unwrap_or_else(|| format!("cannot connect to `{host}:{port}`"))
    })?;
    let _ = stream.set_read_timeout(Some(timeout));
    let _ = stream.set_write_timeout(Some(timeout));
    let request = format!(
        "POST /runner/notify HTTP/1.1\r\nHost: {host}:{port}\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.as_bytes().len()
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|e| format!("notify write: {e}"))?;
    let mut buf = [0u8; 256];
    let n = stream
        .read(&mut buf)
        .map_err(|e| format!("notify read: {e}"))?;
    let head = String::from_utf8_lossy(&buf[..n]);
    let status = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse::<u16>().ok())
        .ok_or_else(|| format!("notify response has no status line: {:?}", head.trim()))?;
    Ok(status)
}

/// Parse `http://host[:port]` into `(host, port)`; rejects https/other schemes.
fn parse_http_base(url: &str) -> Result<(String, u16), String> {
    let rest = url
        .trim()
        .strip_prefix("http://")
        .ok_or_else(|| format!("notify.runner_url must start with http:// (got `{url}`)"))?;
    let authority = rest.split('/').next().unwrap_or("").trim();
    if authority.is_empty() {
        return Err(format!("notify.runner_url has no host (got `{url}`)"));
    }
    if authority.starts_with('[') {
        return Err("notify.runner_url IPv6 literals are not supported in v1".to_string());
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) if !h.is_empty() => (
            h.to_string(),
            p.parse::<u16>()
                .map_err(|_| format!("notify.runner_url has a bad port (got `{url}`)"))?,
        ),
        Some(_) => return Err(format!("notify.runner_url has no host (got `{url}`)")),
        None => (authority.to_string(), 80),
    };
    if host.is_empty() {
        return Err(format!("notify.runner_url has no host (got `{url}`)"));
    }
    Ok((host, port))
}

// ---------------------------------------------------------------------------
// Runner update feeds (P2/D8)
// ---------------------------------------------------------------------------
//
// A feed is the **same `publish-index.json` schema** as the Studio exchange, but
// it may live on a remote web host / GitHub instead of a shared directory. HTTPS
// is fetched by spawning the **system `curl`** (Windows 10+, macOS and Linux ship
// it) instead of adding a TLS dependency to the engine: `-f` fails on HTTP >=
// 400, `-L` follows redirects (GitHub raw/release URLs), `-sS` keeps stderr
// actionable, `--max-time` comes from `updates.fetch_timeout_ms` and
// `--max-filesize` caps the transfer. The URL is one argv entry (no shell), so a
// feed cannot inject a command.

/// Where one feed's index comes from (resolved against the live config dir).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeedSource {
    /// Local `publish-index.json` path.
    Local(PathBuf),
    /// `http(s)://` URL fetched with curl.
    Url(String),
}

/// One configured update feed resolved to concrete sources.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedFeed {
    /// Feed name (display identity: `source` / `feed` response fields).
    pub name: String,
    /// Where the `publish-index.json` is read from.
    pub source: FeedSource,
    /// Optional package base (local dir or URL base); `None` = each event must
    /// carry its own `package_url`.
    pub package_base: Option<FeedSource>,
    /// `curl` timeout from `updates.fetch_timeout_ms`.
    pub timeout_ms: u32,
}

impl ResolvedFeed {
    /// `"local"` / `"url"` for `GET /runner/updates.source_kind`.
    pub fn source_kind(&self) -> &'static str {
        match self.source {
            FeedSource::Local(_) => "local",
            FeedSource::Url(_) => "url",
        }
    }
}

/// A concrete package location for one event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageRef {
    /// Local `.oaspkg` path (exchange `incoming/` or a local feed `package_base`).
    Local(PathBuf),
    /// `http(s)://` URL that must be downloaded before installing.
    Remote(String),
}

impl PackageRef {
    /// `true` when the Runner must download the package before installing.
    pub fn download_required(&self) -> bool {
        matches!(self, PackageRef::Remote(_))
    }
}

/// Resolve one configured feed (P2).
///
/// `config_path` anchors relative local paths (same rule as `exchange.dir`); a
/// relative path without a loaded `--config` is an error because there is no
/// well-defined anchor directory. `timeout_ms` comes from
/// `updates.fetch_timeout_ms`.
pub fn resolve_feed(
    feed: &crate::config::UpdateFeed,
    config_path: Option<&Path>,
    timeout_ms: u32,
) -> Result<ResolvedFeed, String> {
    let name = feed.name.trim();
    if name.is_empty() {
        return Err("feed name is empty".to_string());
    }
    let source = resolve_feed_source(
        &feed.index_url,
        config_path,
        &format!("feed `{name}` index_url"),
    )?;
    let package_base = if feed.package_base.trim().is_empty() {
        None
    } else {
        Some(resolve_feed_source(
            &feed.package_base,
            config_path,
            &format!("feed `{name}` package_base"),
        )?)
    };
    Ok(ResolvedFeed {
        name: name.to_string(),
        source,
        package_base,
        timeout_ms,
    })
}

fn resolve_feed_source(
    raw: &str,
    config_path: Option<&Path>,
    label: &str,
) -> Result<FeedSource, String> {
    let value = raw.trim();
    if value.is_empty() {
        return Err(format!("{label} must not be empty"));
    }
    if crate::config::is_http_url(value) {
        return Ok(FeedSource::Url(value.to_string()));
    }
    if value.contains("://") {
        return Err(format!(
            "{label} `{value}` must be a local path or an http(s) URL (the Runner fetches \
             with curl)"
        ));
    }
    match crate::config::resolve_against_config_dir(config_path, value) {
        Some(path) => Ok(FeedSource::Local(path)),
        None => Err(format!(
            "{label} `{value}` is relative but no `--config` file was loaded to anchor it"
        )),
    }
}

/// Read/parse one feed index: local file, or `curl`-fetched URL cached as a tmp
/// file in `cache_dir` (removed either way). Size is capped for both kinds.
pub fn load_feed(feed: &ResolvedFeed, cache_dir: &Path) -> Result<PublishIndex, String> {
    match &feed.source {
        FeedSource::Local(path) => {
            if !path.is_file() {
                return Err(format!(
                    "feed `{}`: index not found: `{}` (relative paths resolve against the \
                     live config directory)",
                    feed.name,
                    path.display()
                ));
            }
            read_index_file(path).map_err(|e| format!("feed `{}`: {e}", feed.name))
        }
        FeedSource::Url(url) => {
            std::fs::create_dir_all(cache_dir)
                .map_err(|e| format!("create feed cache `{}`: {e}", cache_dir.display()))?;
            let tmp = unique_tmp_path(cache_dir, "feed");
            let result = fetch_to_file(url, &tmp, feed.timeout_ms, MAX_INDEX_BYTES)
                .and_then(|_| read_index_file(&tmp))
                .map_err(|e| format!("feed `{}` from `{url}`: {e}", feed.name));
            let _ = std::fs::remove_file(&tmp);
            result
        }
    }
}

/// Is the system `curl` executable available?
///
/// The engine deliberately has no TLS dependency: HTTPS update feeds use the
/// `curl` shipped with Windows 10+/macOS/Linux. Only curl-dependent tests need
/// this probe (they skip when it is missing); production reports the spawn
/// failure verbatim through the API instead.
#[cfg(test)]
pub fn curl_available() -> bool {
    std::process::Command::new("curl")
        .arg("--version")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

/// Download one URL to `dest` with the system `curl` (see section docs).
pub fn fetch_to_file(
    url: &str,
    dest: &Path,
    timeout_ms: u32,
    max_bytes: u64,
) -> Result<(), String> {
    let url = url.trim();
    if !crate::config::is_http_url(url) {
        return Err(format!("`{url}` is not an http(s) URL"));
    }
    if let Some(parent) = dest.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("create download dir `{}`: {e}", parent.display()))?;
        }
    }
    let secs = f64::from(timeout_ms.clamp(100, 30_000)) / 1000.0;
    let output = std::process::Command::new("curl")
        .arg("-fsSL")
        .arg("--max-time")
        .arg(format!("{secs:.3}"))
        .arg("--max-filesize")
        .arg(max_bytes.to_string())
        .arg("-o")
        .arg(dest)
        .arg(url)
        .output()
        .map_err(|e| {
            format!("failed to run `curl` (required for http(s) feeds; check PATH): {e}")
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let msg: String = stderr.trim().chars().take(400).collect();
        let msg = if msg.is_empty() {
            "no stderr output".to_string()
        } else {
            msg
        };
        return Err(format!(
            "curl failed for `{url}` ({}): {msg}",
            output.status
        ));
    }
    let len = std::fs::metadata(dest).map(|m| m.len()).unwrap_or(0);
    if len == 0 {
        return Err(format!("curl downloaded an empty file from `{url}`"));
    }
    if len > max_bytes {
        return Err(format!(
            "downloaded {len} bytes from `{url}` (max {max_bytes})"
        ));
    }
    Ok(())
}

static TMP_SEQ: AtomicU64 = AtomicU64::new(0);

fn unique_tmp_path(dir: &Path, stem: &str) -> PathBuf {
    let seq = TMP_SEQ.fetch_add(1, Ordering::Relaxed);
    dir.join(format!(".{stem}-{}-{seq}.tmp", std::process::id()))
}

/// Why a remote package download failed (mapped to HTTP status/code by the API).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageDownloadError {
    /// curl/spawn/filesystem failure (network, timeout, HTTP 404, ...).
    Download(String),
    /// The feed's `size_bytes` does not match the downloaded file.
    SizeMismatch { expected: u64, actual: u64 },
    /// The feed's `sha256` does not match the downloaded file.
    Sha256Mismatch { expected: String, actual: String },
    /// A remote event must declare `sha256`; unverified packages are refused.
    MissingSha256,
}

impl PackageDownloadError {
    /// Stable error `code` for the HTTP body.
    pub fn code(&self) -> &'static str {
        match self {
            PackageDownloadError::Download(_) => "download_failed",
            PackageDownloadError::MissingSha256 => "sha256_missing",
            PackageDownloadError::SizeMismatch { .. }
            | PackageDownloadError::Sha256Mismatch { .. } => "sha256_mismatch",
        }
    }

    /// Failed transfer = 500 (feed/server side); bad content = 400.
    pub fn status(&self) -> u16 {
        match self {
            PackageDownloadError::Download(_) => 500,
            _ => 400,
        }
    }
}

impl std::fmt::Display for PackageDownloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PackageDownloadError::Download(msg) => write!(f, "{msg}"),
            PackageDownloadError::SizeMismatch { expected, actual } => write!(
                f,
                "size mismatch: feed declares {expected} bytes, download has {actual}"
            ),
            PackageDownloadError::Sha256Mismatch { expected, actual } => write!(
                f,
                "sha256 mismatch: feed declares {expected}, download has {actual}"
            ),
            PackageDownloadError::MissingSha256 => write!(
                f,
                "remote event has no sha256; refusing to install an unverified package"
            ),
        }
    }
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Download one remote `.oaspkg` into `<cache_dir>/<event_id>.oaspkg`.
///
/// The download goes to a sibling tmp file first and only becomes the cache file
/// after `size_bytes`/`sha256` verification; any mismatch removes the tmp file
/// so a corrupt/truncated download can never be installed later.
pub fn download_package(
    event: &PublishEvent,
    url: &str,
    cache_dir: &Path,
    timeout_ms: u32,
) -> Result<PathBuf, PackageDownloadError> {
    let expected_sha = event.sha256.trim().to_ascii_lowercase();
    if !is_sha256_hex(&expected_sha) {
        return Err(PackageDownloadError::MissingSha256);
    }
    std::fs::create_dir_all(cache_dir).map_err(|e| {
        PackageDownloadError::Download(format!(
            "create package cache `{}`: {e}",
            cache_dir.display()
        ))
    })?;
    let dest = cached_package_path(cache_dir, &event.event_id);
    let tmp = unique_tmp_path(cache_dir, "pkg");
    if let Err(e) = fetch_to_file(url, &tmp, timeout_ms, MAX_PACKAGE_BYTES) {
        let _ = std::fs::remove_file(&tmp);
        return Err(PackageDownloadError::Download(e));
    }
    let actual_size = std::fs::metadata(&tmp).map(|m| m.len()).unwrap_or(0);
    if event.size_bytes > 0 && actual_size != event.size_bytes {
        let _ = std::fs::remove_file(&tmp);
        return Err(PackageDownloadError::SizeMismatch {
            expected: event.size_bytes,
            actual: actual_size,
        });
    }
    let actual_sha = match oas_task::package::sha256_file(&tmp) {
        Ok(sha) => sha,
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            return Err(PackageDownloadError::Download(format!(
                "hash download: {e}"
            )));
        }
    };
    if !actual_sha.eq_ignore_ascii_case(&expected_sha) {
        let _ = std::fs::remove_file(&tmp);
        return Err(PackageDownloadError::Sha256Mismatch {
            expected: expected_sha,
            actual: actual_sha,
        });
    }
    if dest.exists() {
        let _ = std::fs::remove_file(&dest);
    }
    std::fs::rename(&tmp, &dest).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        PackageDownloadError::Download(format!(
            "move download `{}` -> `{}`: {e}",
            tmp.display(),
            dest.display()
        ))
    })?;
    Ok(dest)
}

/// Path of the cached remote package for one event
/// (`<packages>/remote/<safe-event-id>.oaspkg`).
pub fn cached_package_path(cache_dir: &Path, event_id: &str) -> PathBuf {
    cache_dir.join(cache_file_name(event_id))
}

/// Reuse an already downloaded remote package when it still matches the event.
///
/// Remote feeds are content-addressed (`sha256` travels in the event), so a
/// verified cache entry is exactly as trustworthy as a fresh download — this is
/// what makes a second `POST /runner/updates/install` of the same event work
/// offline / without re-`curl`ing. A mismatching or tampered file is **deleted**
/// (never installed) so the caller re-downloads cleanly; `None` = "download it".
///
/// An event without a valid 64-hex sha256 is never reused: `download_package`
/// reports `sha256_missing` loudly instead of trusting an unverifiable cache.
pub fn reusable_cached_package(event: &PublishEvent, cache_dir: &Path) -> Option<PathBuf> {
    let expected_sha = event.sha256.trim();
    if !is_sha256_hex(expected_sha) {
        return None;
    }
    let path = cached_package_path(cache_dir, &event.event_id);
    if !path.is_file() {
        return None;
    }
    let size_ok = std::fs::metadata(&path)
        .map(|meta| event.size_bytes == 0 || meta.len() == event.size_bytes)
        .unwrap_or(false);
    let sha_ok = oas_task::package::sha256_file(&path)
        .map(|sha| sha.eq_ignore_ascii_case(expected_sha))
        .unwrap_or(false);
    if size_ok && sha_ok {
        return Some(path);
    }
    // Stale/tampered cache: remove it so the next download starts from zero.
    let _ = std::fs::remove_file(&path);
    None
}

/// Filesystem-safe cache name for one event id (the content-addressed suffix is
/// kept; everything else becomes `_`). `.oaspkg` keeps the install API's
/// extension check happy.
pub fn cache_file_name(event_id: &str) -> String {
    let mut safe: String = event_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect();
    if safe.trim_matches('_').is_empty() {
        safe.push_str("event");
    }
    if safe.len() > 120 {
        safe.truncate(120);
    }
    format!("{safe}.oaspkg")
}

/// Resolve where one event's `.oaspkg` lives: local exchange file, local feed
/// `package_base`, or a remote URL (event `package_url` / feed URL base).
///
/// `exchange_dir` wins when set (the event came from the local exchange);
/// otherwise `feed` must be the feed the event came from.
pub fn resolve_package_ref(
    event: &PublishEvent,
    feed: Option<&ResolvedFeed>,
    exchange_dir: Option<&Path>,
) -> Result<PackageRef, String> {
    if let Some(dir) = exchange_dir {
        return Ok(PackageRef::Local(resolve_inside(dir, &event.package_file)?));
    }
    let Some(feed) = feed else {
        return Err("event has no source to resolve its package from".to_string());
    };
    if let Some(raw) = event
        .package_url
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        if !crate::config::is_http_url(raw) {
            return Err(format!(
                "event `{}` package_url `{raw}` is not an http(s) URL",
                event.event_id
            ));
        }
        return Ok(PackageRef::Remote(raw.to_string()));
    }
    match &feed.package_base {
        Some(FeedSource::Url(base)) => {
            let file = event.package_file.trim();
            if file.is_empty() {
                return Err(format!("event `{}` has no package_file", event.event_id));
            }
            if crate::config::is_http_url(file) {
                return Ok(PackageRef::Remote(file.to_string()));
            }
            Ok(PackageRef::Remote(join_url(base, file)))
        }
        Some(FeedSource::Local(base)) => Ok(PackageRef::Local(resolve_inside(
            base,
            &event.package_file,
        )?)),
        None => {
            let file = event.package_file.trim();
            if crate::config::is_http_url(file) {
                Ok(PackageRef::Remote(file.to_string()))
            } else {
                Err(format!(
                    "feed `{}` has no package_base and event `{}` has no package_url",
                    feed.name, event.event_id
                ))
            }
        }
    }
}

fn join_url(base: &str, rel: &str) -> String {
    format!(
        "{}/{}",
        base.trim_end_matches('/'),
        rel.trim_start_matches('/')
    )
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
            min_engine: None,
            flow_ops: Vec::new(),
            package_url: None,
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
            min_engine: Some("0.1.0".into()),
            flow_ops: vec!["call_task".into()],
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

    #[test]
    fn announce_persists_engine_fields_for_the_runner_precheck() {
        let td = TempDir::new("announce-engine");
        let exchange = td.path().join("exchange");
        let input = announce_input(td.path(), b"bytes");
        announce_publish(&exchange, &input).expect("announce");
        let index = read_index(&exchange).unwrap();
        assert_eq!(index.events[0].min_engine.as_deref(), Some("0.1.0"));
        assert_eq!(index.events[0].flow_ops, vec!["call_task".to_string()]);
    }

    #[test]
    fn compatibility_precheck_uses_min_engine_and_flow_ops() {
        assert!(is_compatible(None, &[]));
        assert!(is_compatible(Some("0.1.0"), &["call_task".to_string()]));
        assert!(!is_compatible(Some("99.0.0"), &[]), "newer engine required");
        assert!(
            !is_compatible(None, &["unknown.op".to_string()]),
            "unknown flow op must be reported incompatible"
        );
    }

    #[test]
    fn notify_runner_posts_to_the_loopback_endpoint() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut buf = [0u8; 4096];
            let n = stream.read(&mut buf).expect("read");
            let request = String::from_utf8_lossy(&buf[..n]).to_string();
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 11\r\nConnection: close\r\n\r\n{\"ok\":true}")
                .expect("write");
            request
        });
        let payload = notify_payload("pkg-a", "任务", 1, "pkg-a", &"a".repeat(64), 1);
        let status = notify_runner(&format!("http://{addr}"), &payload, 2000).expect("notify");
        assert_eq!(status, 200);
        let request = handle.join().expect("join");
        assert!(request.starts_with("POST /runner/notify HTTP/1.1"), "{request}");
        assert!(request.contains("\"event_id\":\"pkg-a\""), "{request}");
    }

    #[test]
    fn notify_runner_rejects_non_http_or_missing_host_without_network() {
        assert!(notify_runner("https://127.0.0.1:8789", &serde_json::json!({}), 100).is_err());
        assert!(notify_runner("http://", &serde_json::json!({}), 100).is_err());
        assert!(notify_runner("http://127.0.0.1:notaport", &serde_json::json!({}), 100).is_err());
        assert_eq!(parse_http_base("http://127.0.0.1:8789").unwrap(), ("127.0.0.1".to_string(), 8789));
        assert_eq!(parse_http_base("http://localhost").unwrap(), ("localhost".to_string(), 80));
    }

    // ---- P2: update feeds -------------------------------------------------

    fn test_feed(name: &str, index_url: &str, package_base: &str) -> crate::config::UpdateFeed {
        crate::config::UpdateFeed {
            name: name.to_string(),
            enabled: true,
            index_url: index_url.to_string(),
            package_base: package_base.to_string(),
        }
    }

    #[test]
    fn resolve_feed_accepts_local_paths_and_http_urls() {
        let config = Path::new("pkg").join("config").join("live.json");
        // Remote feed: no loopback rule, https is expected.
        let remote = resolve_feed(
            &test_feed(
                "github",
                "https://example.com/feed/publish-index.json",
                "https://example.com/feed/",
            ),
            None,
            8000,
        )
        .expect("remote feed");
        assert_eq!(
            remote.source,
            FeedSource::Url("https://example.com/feed/publish-index.json".into())
        );
        assert_eq!(remote.source_kind(), "url");
        assert_eq!(
            remote.package_base,
            Some(FeedSource::Url("https://example.com/feed/".into()))
        );

        // Local feed: a relative path is anchored at the live config dir (same
        // rule as exchange.dir); an absolute package_base passes through.
        let local_feed = test_feed("local", "feed/publish-index.json", "D:/shared/exchange");
        let resolved_local = resolve_feed(&local_feed, Some(&config), 500).expect("local feed");
        assert_eq!(
            resolved_local.source,
            FeedSource::Local(
                Path::new("pkg")
                    .join("config")
                    .join("feed")
                    .join("publish-index.json")
            )
        );
        assert_eq!(resolved_local.source_kind(), "local");
        assert_eq!(
            resolved_local.package_base,
            Some(FeedSource::Local(PathBuf::from("D:/shared/exchange")))
        );
        assert_eq!(resolved_local.timeout_ms, 500);

        // A relative local path without --config has no anchor; other schemes are
        // refused up front instead of becoming a confusing curl failure.
        assert!(resolve_feed(&local_feed, None, 8000).is_err());
        assert!(resolve_feed(&test_feed("ftp", "ftp://x/p.json", ""), None, 8000).is_err());
        assert!(resolve_feed(&test_feed("", "x.json", ""), Some(&config), 8000).is_err());
    }

    #[test]
    fn load_feed_reads_a_local_index_and_reports_breaks() {
        let td = TempDir::new("load-feed");
        let feed_dir = td.path().join("feed");
        let mut index = PublishIndex::default();
        index.events = vec![event("pkg-feed-1", 1)];
        write_index(&feed_dir, &index).unwrap();
        let resolved = ResolvedFeed {
            name: "本地".into(),
            source: FeedSource::Local(index_path(&feed_dir)),
            package_base: None,
            timeout_ms: 1000,
        };
        let loaded = load_feed(&resolved, &td.path().join("cache")).expect("local feed");
        assert_eq!(loaded.events.len(), 1);
        assert_eq!(loaded.events[0].event_id, "pkg-feed-1");

        // A configured feed file that does not exist is a feed error (unlike the
        // exchange dir, where a missing index means "nothing published yet").
        let missing = ResolvedFeed {
            name: "missing".into(),
            source: FeedSource::Local(td.path().join("nope.json")),
            package_base: None,
            timeout_ms: 1000,
        };
        let err = load_feed(&missing, &td.path().join("cache")).expect_err("missing feed");
        assert!(err.contains("not found"), "{err}");

        // Invalid JSON and an oversized index are loud errors too.
        std::fs::write(td.path().join("bad.json"), "{ not json").unwrap();
        let bad = ResolvedFeed {
            name: "bad".into(),
            source: FeedSource::Local(td.path().join("bad.json")),
            package_base: None,
            timeout_ms: 1000,
        };
        assert!(load_feed(&bad, &td.path().join("cache")).is_err());
        let big = td.path().join("big.json");
        std::fs::write(&big, vec![b' '; (MAX_INDEX_BYTES + 1) as usize]).unwrap();
        let big_feed = ResolvedFeed {
            name: "big".into(),
            source: FeedSource::Local(big),
            package_base: None,
            timeout_ms: 1000,
        };
        let err = load_feed(&big_feed, &td.path().join("cache")).expect_err("too large");
        assert!(err.contains("max"), "{err}");
    }

    #[test]
    fn fetch_to_file_fails_fast_for_a_silent_endpoint() {
        // Accept the TCP connection but send nothing: curl must fail (reset /
        // empty reply) instead of waiting for the whole --max-time. The thread is
        // detached on purpose so a machine without curl cannot deadlock the test.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        std::thread::spawn(move || {
            let _ = listener.accept();
        });
        let td = TempDir::new("fetch-fail");
        let err = fetch_to_file(
            &format!("http://{addr}/feed.json"),
            &td.path().join("out.json"),
            1000,
            MAX_INDEX_BYTES,
        )
        .expect_err("silent endpoint must fail");
        assert!(err.contains("curl"), "{err}");
        assert!(
            !crate::config::is_http_url("ftp://example.com/feed.json"),
            "fetch only accepts http(s)"
        );
        assert!(fetch_to_file("ftp://example.com/x", &td.path().join("x"), 1000, 1024).is_err());
    }

    #[test]
    fn resolve_package_ref_uses_url_base_and_local_base() {
        let td = TempDir::new("package-ref");
        let mut ev = event("pkg-pkg-1", 1);
        ev.package_file = "x.oaspkg".into();

        // Exchange event: relative to the exchange dir, never a URL.
        let exchange_dir = td.path().join("exchange");
        assert_eq!(
            resolve_package_ref(&ev, None, Some(&exchange_dir)).unwrap(),
            PackageRef::Local(exchange_dir.join("x.oaspkg"))
        );

        // Feed URL base: joined with forward slashes.
        let mut remote = ev.clone();
        remote.package_url = Some("https://host/pkgs/x.oaspkg".into());
        let url_feed = ResolvedFeed {
            name: "web".into(),
            source: FeedSource::Url("https://host/feed.json".into()),
            package_base: Some(FeedSource::Url("https://host/pkgs/".into())),
            timeout_ms: 1000,
        };
        assert_eq!(
            resolve_package_ref(&remote, Some(&url_feed), None).unwrap(),
            PackageRef::Remote("https://host/pkgs/x.oaspkg".into()),
            "package_url wins over package_base"
        );
        let mut no_url = ev.clone();
        no_url.package_file = "pkg.oaspkg".into();
        assert_eq!(
            resolve_package_ref(&no_url, Some(&url_feed), None).unwrap(),
            PackageRef::Remote("https://host/pkgs/pkg.oaspkg".into())
        );
        assert_eq!(
            resolve_package_ref(&no_url, Some(&url_feed), None)
                .unwrap()
                .download_required(),
            true
        );

        // Local feed base: contained inside the base dir.
        let local_feed = ResolvedFeed {
            name: "dir".into(),
            source: FeedSource::Local(td.path().join("feed.json")),
            package_base: Some(FeedSource::Local(td.path().join("pkgs"))),
            timeout_ms: 1000,
        };
        assert_eq!(
            resolve_package_ref(&no_url, Some(&local_feed), None).unwrap(),
            PackageRef::Local(td.path().join("pkgs").join("pkg.oaspkg"))
        );
        // No base + no package_url = not installable, loudly.
        let bare_feed = ResolvedFeed {
            name: "bare".into(),
            source: FeedSource::Url("https://host/feed.json".into()),
            package_base: None,
            timeout_ms: 1000,
        };
        let err = resolve_package_ref(&no_url, Some(&bare_feed), None).expect_err("no base");
        assert!(err.contains("package_base"), "{err}");
        // ...unless the event itself carries an absolute package_file URL.
        let mut abs_file = ev.clone();
        abs_file.package_file = "https://host/pkg.oaspkg".into();
        assert_eq!(
            resolve_package_ref(&abs_file, Some(&bare_feed), None).unwrap(),
            PackageRef::Remote("https://host/pkg.oaspkg".into())
        );
    }

    #[test]
    fn download_refuses_unverified_events_and_cache_names_are_safe() {
        let td = TempDir::new("download-cache");
        let mut ev = event("pkg-任务-1-abc123abc123", 1);
        ev.sha256 = String::new();
        let err = download_package(
            &ev,
            "https://host/x.oaspkg",
            &td.path().join("cache"),
            1000,
        )
        .expect_err("no sha -> refuse before downloading");
        assert_eq!(err.code(), "sha256_missing");
        assert_eq!(err.status(), 400);
        assert_eq!(
            PackageDownloadError::Download("x".into()).code(),
            "download_failed"
        );
        assert_eq!(PackageDownloadError::Download("x".into()).status(), 500);
        assert_eq!(
            PackageDownloadError::Sha256Mismatch {
                expected: "a".into(),
                actual: "b".into()
            }
            .code(),
            "sha256_mismatch"
        );

        assert_eq!(
            cache_file_name("pkg-任务-1-abc123"),
            "pkg-__-1-abc123.oaspkg"
        );
        assert_eq!(cache_file_name(""), "event.oaspkg");
        assert_eq!(cache_file_name("../x"), ".._x.oaspkg");
    }

    #[test]
    fn reusable_cached_package_requires_matching_size_and_sha256() {
        let td = TempDir::new("reuse-cache");
        let cache_dir = td.path().join("remote");
        std::fs::create_dir_all(&cache_dir).unwrap();
        let bytes = b"verified-package-bytes";
        let mut ev = event("pkg-任务-1-abc123abc123", 1);
        ev.size_bytes = bytes.len() as u64;
        ev.sha256 = sha256(bytes);
        let path = cached_package_path(&cache_dir, &ev.event_id);
        assert_eq!(
            path,
            cache_dir.join(cache_file_name(&ev.event_id)),
            "cache path helper is the documented one"
        );

        // Missing file -> download.
        assert_eq!(reusable_cached_package(&ev, &cache_dir), None);

        // Matching size+sha -> reuse, file stays.
        std::fs::write(&path, bytes).unwrap();
        assert_eq!(
            reusable_cached_package(&ev, &cache_dir),
            Some(path.clone())
        );
        assert!(path.is_file());

        // Tampered content (same size) -> deleted, download again.
        std::fs::write(&path, b"verified-package-byteZ").unwrap();
        assert_eq!(reusable_cached_package(&ev, &cache_dir), None);
        assert!(!path.exists(), "a sha-mismatched cache must be deleted");

        // Wrong size -> deleted.
        std::fs::write(&path, b"short").unwrap();
        assert_eq!(reusable_cached_package(&ev, &cache_dir), None);
        assert!(!path.exists(), "a size-mismatched cache must be deleted");

        // An event without a verifiable sha256 is never reused (the download
        // path reports sha256_missing instead); a stray file is left alone so
        // the user can inspect/repair it.
        std::fs::write(&path, bytes).unwrap();
        let mut no_sha = ev.clone();
        no_sha.sha256 = String::new();
        assert_eq!(reusable_cached_package(&no_sha, &cache_dir), None);
        assert!(path.is_file(), "unverifiable events must not delete files");

        // size_bytes == 0 means "feed did not declare it": sha is authoritative.
        let mut no_size = ev.clone();
        no_size.size_bytes = 0;
        assert_eq!(
            reusable_cached_package(&no_size, &cache_dir),
            Some(path.clone())
        );
    }
}
