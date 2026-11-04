//! Runner task runtime values (`config/values.json`, design §C.2).
//!
//! The Runner surface is the only writer of this file (Studio edits the
//! **schema** in `tasks/<id>/meta.json` instead). It stores the per-task values
//! the Runner operator filled in, so a scheduled/standalone run can resolve
//! them as the layer below the scheduler entry and above `meta.params[].default`
//! (design §C.3).
//!
//! Layout:
//!
//! ```json
//! { "schema_version": 1, "updated_ms": 0,
//!   "tasks": { "每日任务": { "values": {"max_rounds": 5}, "updated_ms": 0 } } }
//! ```
//!
//! * **Location** follows the live config file: `oasd runner --config
//!   config/live.json` -> `config/values.json`; without `--config` it falls
//!   back to `config/values.json` under the current working directory.
//! * Every path is a `Path`/`PathBuf` join; the IO is plain `std::fs` (no
//!   Windows-only API), so the same file contract works for a future Android
//!   client talking to the Runner HTTP API.
//! * Writes are **read-modify-write + atomic** (sibling tmp file + rename) and
//!   preserve the values of every other task. A corrupt file is never
//!   silently replaced: reads return a loud error with the path.

use std::collections::BTreeMap;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// On-disk schema version of `config/values.json`.
pub const VALUES_SCHEMA_VERSION: u32 = 1;

/// File name of the Runner values store.
pub const VALUES_FILE_NAME: &str = "values.json";

/// Directory used when no `--config` was given (relative to the CWD).
pub const DEFAULT_VALUES_DIR: &str = "config";

/// Whole `config/values.json` document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunnerValuesFile {
    pub schema_version: u32,
    #[serde(default)]
    pub updated_ms: u64,
    #[serde(default)]
    pub tasks: BTreeMap<String, RunnerTaskValues>,
}

impl Default for RunnerValuesFile {
    fn default() -> Self {
        Self {
            schema_version: VALUES_SCHEMA_VERSION,
            updated_ms: 0,
            tasks: BTreeMap::new(),
        }
    }
}

/// Values of one task (keyed by the same `meta.params[].key` as the schema).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RunnerTaskValues {
    #[serde(default)]
    pub values: Map<String, Value>,
    #[serde(default)]
    pub updated_ms: u64,
}

/// Absolute-or-relative path of the Runner values file for `config_path`
/// (`None` = serve/runner started without `--config`).
///
/// Pure function so both branches are unit-testable without touching the
/// process-wide `config::global_path()` once-lock.
pub fn runner_values_path_from(config_path: Option<&Path>) -> PathBuf {
    match config_path {
        // `--config config/live.json` => `config/values.json`; a bare
        // `--config live.json` means "next to it" = the CWD.
        Some(path) => path
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .join(VALUES_FILE_NAME),
        None => Path::new(DEFAULT_VALUES_DIR).join(VALUES_FILE_NAME),
    }
}

/// Path of the Runner values file for the current process.
pub fn runner_values_path() -> PathBuf {
    runner_values_path_from(crate::config::global_path())
}

/// Read the store. A missing file is **not** an error (empty defaults); an
/// unreadable/invalid file or an unknown `schema_version` is, with the path in
/// the message so the HTTP layer can surface it verbatim.
pub fn read(path: &Path) -> Result<RunnerValuesFile, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(RunnerValuesFile::default()),
        Err(e) => return Err(format!("read values `{}`: {e}", path.display())),
    };
    let file: RunnerValuesFile = serde_json::from_str(&text)
        .map_err(|e| format!("values file `{}` is invalid JSON: {e}", path.display()))?;
    if file.schema_version != VALUES_SCHEMA_VERSION {
        return Err(format!(
            "values file `{}` has unsupported schema_version {} (expected {})",
            path.display(),
            file.schema_version,
            VALUES_SCHEMA_VERSION
        ));
    }
    Ok(file)
}

/// Write the whole store atomically, creating the parent directory.
pub fn write(path: &Path, file: &RunnerValuesFile) -> Result<(), String> {
    let pretty = serde_json::to_string_pretty(file)
        .map_err(|e| format!("serialize values: {e}"))?;
    write_text_atomic(path, &format!("{pretty}\n"))
}

/// Values currently stored for `task` (`None` when the task has never been
/// configured). A corrupt store is a loud error, never an empty result.
pub fn task_values(path: &Path, task: &str) -> Result<Option<RunnerTaskValues>, String> {
    Ok(read(path)?.tasks.get(task).cloned())
}

/// Serializes the read-modify-write cycle inside the Runner process (design
/// §C.2: "单写者"). The atomic rename alone makes each write crash-safe, but
/// two concurrent HTTP PUTs could otherwise both read the old file and the
/// later rename would drop the other task's update.
static WRITE_LOCK: Mutex<()> = Mutex::new(());

/// Read-modify-write one task's values, preserving every other task.
///
/// This is the only write path the HTTP API uses; `values` is expected to have
/// passed [`oas_task::validate_param_values`] already (the store is schema
/// agnostic). Concurrent callers are serialized by [`WRITE_LOCK`].
pub fn set_task_values(
    path: &Path,
    task: &str,
    values: Map<String, Value>,
    now_ms: u64,
) -> Result<RunnerTaskValues, String> {
    let _guard = WRITE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut file = read(path)?;
    let entry = RunnerTaskValues {
        values,
        updated_ms: now_ms,
    };
    file.tasks.insert(task.to_string(), entry.clone());
    file.updated_ms = now_ms;
    write(path, &file)?;
    Ok(entry)
}

static WRITE_SEQ: AtomicU64 = AtomicU64::new(0);

/// Write `text` atomically (sibling tmp + rename), creating the parent dir.
///
/// Deliberately a module-local copy of `serve::write_text_atomic` so this
/// module stays self-contained; `std::fs::rename` replaces an existing file on
/// both Windows and Unix, and a failed write/rename removes the tmp file.
fn write_text_atomic(path: &Path, text: &str) -> Result<(), String> {
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    if !parent.as_os_str().is_empty() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("create dir `{}`: {e}", parent.display()))?;
    }
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| VALUES_FILE_NAME.to_string());
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
                "oasd-runner-config-{}-{tag}-{}",
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

    fn values(json: &str) -> Map<String, Value> {
        serde_json::from_str::<Value>(json)
            .expect("valid values json")
            .as_object()
            .expect("values object")
            .clone()
    }

    #[test]
    fn missing_file_reads_as_an_empty_default() {
        let td = TempDir::new("missing");
        let path = td.path().join("config").join("values.json");
        let file = read(&path).expect("missing file is not an error");
        assert_eq!(file.schema_version, VALUES_SCHEMA_VERSION);
        assert_eq!(file.updated_ms, 0);
        assert!(file.tasks.is_empty());
        assert!(task_values(&path, "任务1").expect("read").is_none());
        assert!(
            !path.exists(),
            "reading must not create the store as a side effect"
        );
    }

    #[test]
    fn write_then_read_roundtrip_is_atomic_and_creates_the_parent() {
        let td = TempDir::new("roundtrip");
        let path = td.path().join("config").join("values.json");
        let cfg = td.path().join("config").join("live.json");
        assert_eq!(runner_values_path_from(Some(&cfg)), path);

        let file = RunnerValuesFile {
            schema_version: 1,
            updated_ms: 42,
            tasks: BTreeMap::from([(
                "任务1".to_string(),
                RunnerTaskValues {
                    values: values(r#"{"max_rounds":5,"识别关键词":"突破"}"#),
                    updated_ms: 41,
                },
            )]),
        };
        write(&path, &file).expect("write store");
        assert_eq!(read(&path).expect("read back"), file);

        // Only the final file remains: the sibling tmp was renamed away.
        let names: Vec<String> = std::fs::read_dir(td.path().join("config"))
            .expect("list config dir")
            .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["values.json".to_string()], "{names:?}");

        // Overwriting an existing file also succeeds (rename replaces).
        let updated = RunnerValuesFile {
            updated_ms: 43,
            ..file.clone()
        };
        write(&path, &updated).expect("overwrite store");
        assert_eq!(read(&path).expect("read updated").updated_ms, 43);
    }

    #[test]
    fn set_task_values_is_read_modify_write_and_preserves_other_tasks() {
        let td = TempDir::new("rmw");
        let path = td.path().join("config").join("values.json");

        let first = set_task_values(&path, "任务1", values(r#"{"max_rounds":3}"#), 100)
            .expect("set first");
        assert_eq!(first.updated_ms, 100);
        let second = set_task_values(&path, "任务2", values(r#"{"mode":"safe"}"#), 200)
            .expect("set second");
        assert_eq!(second.updated_ms, 200);

        let file = read(&path).expect("read store");
        assert_eq!(file.updated_ms, 200);
        assert_eq!(file.tasks.len(), 2, "both tasks must survive: {file:?}");
        assert_eq!(file.tasks["任务1"].values["max_rounds"], 3);
        assert_eq!(file.tasks["任务1"].updated_ms, 100);
        assert_eq!(file.tasks["任务2"].values["mode"], "safe");

        // Rewriting the same task replaces its values but not its siblings.
        set_task_values(&path, "任务1", values(r#"{"max_rounds":9}"#), 300)
            .expect("overwrite first");
        let file = read(&path).expect("read store");
        assert_eq!(file.tasks["任务1"].values["max_rounds"], 9);
        assert_eq!(file.tasks["任务1"].updated_ms, 300);
        assert_eq!(file.tasks["任务2"].values["mode"], "safe");
    }

    #[test]
    fn concurrent_set_task_values_keeps_every_task() {
        let td = TempDir::new("concurrent");
        let path = td.path().join("config").join("values.json");
        std::thread::scope(|scope| {
            for i in 0..8u32 {
                let path = path.clone();
                scope.spawn(move || {
                    let task = format!("task{i}");
                    let values = values(&format!(r#"{{"n":{i}}}"#));
                    set_task_values(&path, &task, values, 100 + u64::from(i))
                        .expect("concurrent set");
                });
            }
        });
        let file = read(&path).expect("read store");
        assert_eq!(file.tasks.len(), 8, "every task must survive: {file:?}");
        for i in 0..8u32 {
            let task = format!("task{i}");
            assert_eq!(file.tasks[&task].values["n"], serde_json::json!(i));
            assert_eq!(file.tasks[&task].updated_ms, 100 + u64::from(i));
        }
    }

    #[test]
    fn corrupt_json_is_a_loud_error_and_is_never_overwritten() {
        let td = TempDir::new("corrupt");
        let path = td.path().join("config").join("values.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "{ this is not json").unwrap();
        let before = std::fs::read_to_string(&path).unwrap();

        let err = read(&path).expect_err("corrupt store must fail");
        assert!(err.contains(&path.display().to_string()), "error carries the path: {err}");
        assert!(err.contains("invalid JSON"), "{err}");

        let set_err = set_task_values(&path, "任务1", Map::new(), 1)
            .expect_err("must not overwrite a corrupt store");
        assert!(set_err.contains("invalid JSON"), "{set_err}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
    }

    #[test]
    fn unsupported_schema_version_is_an_error() {
        let td = TempDir::new("schema");
        let path = td.path().join("config").join("values.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            r#"{"schema_version":99,"updated_ms":0,"tasks":{}}"#,
        )
        .unwrap();
        let err = read(&path).expect_err("future schema must fail");
        assert!(err.contains("schema_version 99"), "{err}");
        assert!(err.contains("expected 1"), "{err}");
        assert!(
            set_task_values(&path, "任务1", Map::new(), 1).is_err(),
            "write must refuse an unsupported schema"
        );
    }

    #[test]
    fn values_path_follows_the_config_file_or_the_cwd_config_dir() {
        // `--config config/live.json` -> sibling values.json.
        let cfg = Path::new("config").join("live.json");
        assert_eq!(
            runner_values_path_from(Some(&cfg)),
            Path::new("config").join("values.json")
        );
        // Absolute config paths work the same (no drive-letter assumptions).
        let abs = std::env::temp_dir().join("pkg").join("config").join("live.json");
        assert_eq!(
            runner_values_path_from(Some(&abs)),
            std::env::temp_dir()
                .join("pkg")
                .join("config")
                .join("values.json")
        );
        // A bare `--config live.json` means "next to it" = the CWD.
        assert_eq!(
            runner_values_path_from(Some(Path::new("live.json"))),
            Path::new("values.json")
        );
        // No `--config` -> CWD/config/values.json.
        assert_eq!(
            runner_values_path_from(None),
            Path::new("config").join("values.json")
        );
    }
}
