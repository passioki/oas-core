//! `oasd task` — the unified task-template CLI (ENGINE-DESIGN §22.34.2, M0).
//!
//! Two subcommands:
//!
//! ```text
//! oasd task lint --tasks-dir <dir> [--all | --task <name>]
//! oasd task new  --template daily|loop|utility --id <id> --tasks-dir <dir>
//! ```
//!
//! * `lint` enforces the M0 invariants (T1/T3/T4/T5/T6 + the `meta.json`
//!   contract); every error carries a path. Exit 0 = green, 1 = lint errors,
//!   2 = usage/setup error.
//! * `new` renders one of the built-in skeletons (`<repo>/templates/*.json`,
//!   compiled into this binary so the shipped exe is self-contained), builds the
//!   bundle in a hidden staging directory, **lints it**, and only then renames it
//!   into place. An existing task directory is never overwritten.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicU64, Ordering};

use oas_task::{lint_task, lint_tasks, LintIssue, TaskBundle};

pub const USAGE: &str = "usage:\n  oasd task lint --tasks-dir <dir> [--all | --task <name>]\n  \
                         oasd task new --template daily|loop|utility --id <id> --tasks-dir <dir>";

/// Built-in skeleton templates. The same files live in `<repo>/templates/` and
/// are the single source of truth (this list only pins the allowed names).
pub const TEMPLATES: [&str; 3] = ["daily", "loop", "utility"];

static STAGING_SEQ: AtomicU64 = AtomicU64::new(0);

// ---------------------------------------------------------------------------
// Templates / creation
// ---------------------------------------------------------------------------

struct TemplateSource {
    name: &'static str,
    json: &'static str,
}

fn template_source(template: &str) -> Result<TemplateSource, String> {
    let (name, json) = match template {
        "daily" => ("daily", include_str!("../../../templates/daily.json")),
        "loop" => ("loop", include_str!("../../../templates/loop.json")),
        "utility" => ("utility", include_str!("../../../templates/utility.json")),
        other => {
            return Err(format!(
                "unknown template `{other}` (allowed: {})",
                TEMPLATES.join(", ")
            ))
        }
    };
    Ok(TemplateSource { name, json })
}

/// Result of a successful `task new`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatedTask {
    pub template: String,
    pub path: PathBuf,
}

fn pretty_json(value: &serde_json::Value, what: &str) -> Result<String, String> {
    let mut text = serde_json::to_string_pretty(value)
        .map_err(|e| format!("serialize template {what}: {e}"))?;
    text.push('\n');
    Ok(text)
}

fn format_issues(issues: &[LintIssue]) -> String {
    issues
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("; ")
}

/// Create `<tasks_dir>/<id>/` from a built-in skeleton template.
///
/// Refuses to overwrite an existing task directory. The bundle is first built in
/// a hidden staging directory, linted, then atomically renamed into place, so a
/// failure never leaves a half-written task behind. `meta`/`flow`/`assets` +
/// an empty `images/` directory are always produced.
pub fn create_task(tasks_dir: &Path, template: &str, id: &str) -> Result<CreatedTask, String> {
    let source = template_source(template)?;
    oas_task::validate_task_name(id).map_err(|e| format!("invalid task id `{id}`: {e}"))?;
    if id.starts_with('.') {
        return Err(format!(
            "invalid task id `{id}`: must not start with `.` (hidden/staging names are reserved)"
        ));
    }

    let tasks_dir = if tasks_dir.as_os_str().is_empty() {
        Path::new("tasks")
    } else {
        tasks_dir
    };
    let target = tasks_dir.join(id);
    if target.exists() {
        return Err(format!(
            "task `{id}` already exists at {} - refusing to overwrite",
            target.display()
        ));
    }
    std::fs::create_dir_all(tasks_dir)
        .map_err(|e| format!("create tasks dir {}: {e}", tasks_dir.display()))?;

    // `{{id}}` is substituted on the raw template text. Task ids cannot contain
    // `"`, `\` or control characters (validate_task_name), so the JSON stays valid.
    let rendered = source.json.replace("{{id}}", id);
    let value: serde_json::Value = serde_json::from_str(&rendered)
        .map_err(|e| format!("template `{}` is invalid JSON: {e}", source.name))?;
    let get = |key: &str| {
        value
            .get(key)
            .ok_or_else(|| format!("template `{}`: missing `{key}` object", source.name))
    };
    let meta_text = pretty_json(get("meta")?, "meta")?;
    let flow_text = pretty_json(get("flow")?, "flow")?;
    let assets_text = pretty_json(get("assets")?, "assets")?;

    let staging = tasks_dir.join(format!(
        ".oasd-new-{}-{}",
        std::process::id(),
        STAGING_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&staging);

    let built = (|| -> Result<(), String> {
        std::fs::create_dir_all(staging.join("images")).map_err(|e| {
            format!("create {}: {e}", staging.join("images").display())
        })?;
        for (name, text) in [
            ("meta.json", &meta_text),
            ("flow.json", &flow_text),
            ("assets.json", &assets_text),
        ] {
            let path = staging.join(name);
            std::fs::write(&path, text).map_err(|e| format!("write {}: {e}", path.display()))?;
        }
        // Self-check: a skeleton that cannot pass `task lint` must never be created.
        let bundle = TaskBundle::open(&staging)
            .map_err(|e| format!("open staging bundle {}: {e}", staging.display()))?;
        if let Err(issues) = bundle.lint() {
            return Err(format!(
                "template `{}` skeleton failed the M0 lint: {}",
                source.name,
                format_issues(&issues)
            ));
        }
        Ok(())
    })();
    if let Err(e) = built {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(e);
    }

    match std::fs::rename(&staging, &target) {
        Ok(()) => Ok(CreatedTask {
            template: source.name.to_string(),
            path: target,
        }),
        Err(e) => {
            let _ = std::fs::remove_dir_all(&staging);
            Err(format!(
                "rename {} -> {}: {e}",
                staging.display(),
                target.display()
            ))
        }
    }
}

// ---------------------------------------------------------------------------
// Lint
// ---------------------------------------------------------------------------

/// Result of `task lint` (one task or all of them).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LintSummary {
    pub tasks: usize,
    pub errors: usize,
    /// Greppable lines; the CLI prints them verbatim.
    pub lines: Vec<String>,
}

fn push_issue_lines(lines: &mut Vec<String>, name: &str, issues: &[LintIssue]) {
    for issue in issues {
        lines.push(format!(
            "lint error task={name} code={} path={} msg={}",
            issue.code, issue.path, issue.msg
        ));
    }
}

/// Lint one task (`--task <name>`) or every task (`--all` / `None`).
///
/// Setup errors are `Err`; lint findings are counted in [`LintSummary`].
fn lint_one_with_global(tasks_dir: &Path, name: &str) -> Result<Vec<LintIssue>, String> {
    let assets_root = Path::new(crate::global_assets::DEFAULT_DIR);
    if crate::global_assets::is_initialized(assets_root) {
        let store = crate::global_assets::open_store(assets_root)?;
        let ids: std::collections::BTreeSet<String> = store
            .load_assets_file()?
            .items
            .into_iter()
            .map(|a| a.id)
            .collect();
        let bundle = TaskBundle::open_named(tasks_dir, name)?;
        let mut issues = Vec::new();
        bundle.lint_into_with_assets(&mut issues, Some(&ids));
        Ok(issues)
    } else {
        lint_task(tasks_dir, name)
    }
}

pub fn lint(tasks_dir: &Path, only: Option<&str>) -> Result<LintSummary, String> {
    if let Some(name) = only {
        let issues = lint_one_with_global(tasks_dir, name)?;
        let mut lines = vec![format!("lint task={name} errors={}", issues.len())];
        push_issue_lines(&mut lines, name, &issues);
        return Ok(LintSummary {
            tasks: 1,
            errors: issues.len(),
            lines,
        });
    }

    let names = oas_task::list_tasks(tasks_dir)?;
    let mut results: Vec<(String, Vec<LintIssue>)> = Vec::new();
    for name in names {
        match lint_one_with_global(tasks_dir, &name) {
            Ok(issues) => results.push((name, issues)),
            Err(e) => results.push((name, vec![LintIssue::new("TASK", "task", e)])),
        }
    }
    let mut lines = Vec::new();
    let mut errors = 0usize;
    for (name, issues) in &results {
        if issues.is_empty() {
            lines.push(format!("lint task={name} ok"));
        } else {
            errors += issues.len();
            lines.push(format!("lint task={name} errors={}", issues.len()));
            push_issue_lines(&mut lines, name, issues);
        }
    }
    lines.push(format!("linted={} errors={}", results.len(), errors));
    Ok(LintSummary {
        tasks: results.len(),
        errors,
        lines,
    })
}

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq, Eq)]
struct LintArgs {
    tasks_dir: PathBuf,
    only: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
struct NewArgs {
    tasks_dir: PathBuf,
    template: String,
    id: String,
}

fn take_value(rest: &[String], i: &mut usize, flag: &str) -> Result<String, String> {
    let value = rest
        .get(*i + 1)
        .ok_or_else(|| format!("{flag} needs a value\n{USAGE}"))?;
    *i += 2;
    Ok(value.clone())
}

fn parse_lint_args(rest: &[String]) -> Result<LintArgs, String> {
    let mut tasks_dir: Option<PathBuf> = None;
    let mut all = false;
    let mut task: Option<String> = None;
    let mut i = 0;
    while i < rest.len() {
        match rest[i].as_str() {
            "--tasks-dir" => {
                tasks_dir = Some(PathBuf::from(take_value(rest, &mut i, "--tasks-dir")?));
            }
            "--all" => {
                all = true;
                i += 1;
            }
            "--task" => {
                let name = take_value(rest, &mut i, "--task")?;
                if name.trim().is_empty() {
                    return Err(format!("--task <name> must not be empty\n{USAGE}"));
                }
                task = Some(name);
            }
            other => {
                return Err(format!("unknown task lint argument `{other}`\n{USAGE}"));
            }
        }
    }
    let tasks_dir =
        tasks_dir.ok_or_else(|| format!("--tasks-dir <dir> is required\n{USAGE}"))?;
    if all && task.is_some() {
        return Err(format!("--all and --task are mutually exclusive\n{USAGE}"));
    }
    Ok(LintArgs {
        tasks_dir,
        only: task,
    })
}

fn parse_new_args(rest: &[String]) -> Result<NewArgs, String> {
    let mut tasks_dir: Option<PathBuf> = None;
    let mut template: Option<String> = None;
    let mut id: Option<String> = None;
    let mut i = 0;
    while i < rest.len() {
        match rest[i].as_str() {
            "--tasks-dir" => {
                tasks_dir = Some(PathBuf::from(take_value(rest, &mut i, "--tasks-dir")?));
            }
            "--template" => template = Some(take_value(rest, &mut i, "--template")?),
            "--id" => id = Some(take_value(rest, &mut i, "--id")?),
            other => return Err(format!("unknown task new argument `{other}`\n{USAGE}")),
        }
    }
    let tasks_dir =
        tasks_dir.ok_or_else(|| format!("--tasks-dir <dir> is required\n{USAGE}"))?;
    let template =
        template.ok_or_else(|| format!("--template <daily|loop|utility> is required\n{USAGE}"))?;
    let id = id.ok_or_else(|| format!("--id <id> is required\n{USAGE}"))?;
    Ok(NewArgs {
        tasks_dir,
        template,
        id,
    })
}

/// `oasd task ...` entry point (called from `main.rs`).
pub fn task_cmd(rest: &[String]) -> ExitCode {
    match run(rest) {
        Ok(code) => code,
        Err(msg) => {
            eprintln!("task: {msg}");
            ExitCode::from(2)
        }
    }
}

fn run(rest: &[String]) -> Result<ExitCode, String> {
    match rest.first().map(String::as_str) {
        Some("lint") => {
            let args = parse_lint_args(&rest[1..])?;
            let summary = lint(&args.tasks_dir, args.only.as_deref())?;
            for line in &summary.lines {
                println!("{line}");
            }
            Ok(if summary.errors == 0 {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            })
        }
        Some("new") => {
            let args = parse_new_args(&rest[1..])?;
            let created = create_task(&args.tasks_dir, &args.template, &args.id)?;
            println!(
                "created task={} template={} path={}",
                args.id,
                created.template,
                created.path.display()
            );
            Ok(ExitCode::SUCCESS)
        }
        Some("-h") | Some("--help") | None => {
            println!("{USAGE}");
            Ok(ExitCode::from(2))
        }
        Some(other) => Err(format!("unknown task subcommand `{other}`\n{USAGE}")),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "oasd-task-test-{}-{tag}-{}",
                std::process::id(),
                TEST_SEQ.fetch_add(1, Ordering::Relaxed)
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

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn lint_args_parse_and_reject_bad_shapes() {
        let parsed = parse_lint_args(&args(&["--tasks-dir", "tasks", "--all"])).expect("parse");
        assert_eq!(parsed.tasks_dir, PathBuf::from("tasks"));
        assert_eq!(parsed.only, None);

        let parsed =
            parse_lint_args(&args(&["--task", "任务1", "--tasks-dir", "t"])).expect("parse");
        assert_eq!(parsed.only.as_deref(), Some("任务1"));

        assert!(parse_lint_args(&[]).is_err(), "missing --tasks-dir");
        assert!(
            parse_lint_args(&args(&["--tasks-dir", "t"])).is_ok(),
            "neither --all nor --task defaults to --all"
        );
        assert!(parse_lint_args(&args(&["--tasks-dir", "t", "--all", "--task", "x"])).is_err());
        assert!(parse_lint_args(&args(&["--tasks-dir", "t", "--bogus"])).is_err());
        assert!(parse_lint_args(&args(&["--tasks-dir"])).is_err());
        assert!(parse_lint_args(&args(&["--tasks-dir", "t", "--task", " "])).is_err());
    }

    #[test]
    fn new_args_require_template_id_and_tasks_dir() {
        let parsed = parse_new_args(&args(&[
            "--template",
            "daily",
            "--id",
            "任务1",
            "--tasks-dir",
            "tasks",
        ]))
        .expect("parse");
        assert_eq!(parsed.template, "daily");
        assert_eq!(parsed.id, "任务1");
        assert_eq!(parsed.tasks_dir, PathBuf::from("tasks"));

        assert!(parse_new_args(&args(&["--id", "x", "--tasks-dir", "t"])).is_err());
        assert!(parse_new_args(&args(&["--template", "daily", "--tasks-dir", "t"])).is_err());
        assert!(parse_new_args(&args(&["--template", "daily", "--id", "x"])).is_err());
        assert!(parse_new_args(&args(&["--template", "daily", "--id", "x", "--tasks-dir", "t", "--x"]))
            .is_err());
    }

    #[test]
    fn every_template_creates_a_lint_clean_bundle() {
        for (template, category) in [("daily", "daily"), ("loop", "utility"), ("utility", "utility")] {
            let td = TempDir::new(template);
            let tasks = td.path().join("tasks");
            let created = create_task(&tasks, template, "我的任务").expect("create");
            assert_eq!(created.template, template);
            assert!(created.path.is_dir());
            for file in ["meta.json", "flow.json", "assets.json"] {
                assert!(
                    created.path.join(file).is_file(),
                    "{template}: {file} missing"
                );
            }
            assert!(
                created.path.join("images").is_dir(),
                "{template}: images/ dir missing"
            );

            let bundle = TaskBundle::open(&created.path).expect("open bundle");
            bundle.lint().unwrap_or_else(|issues| {
                panic!("{template} skeleton must lint green: {issues:?}")
            });
            let meta = bundle.load_meta().expect("load meta").expect("meta present");
            assert_eq!(meta.name, "我的任务");
            assert_eq!(meta.task_id(), "我的任务");
            assert_eq!(meta.category(), category, "{template}");
            assert_eq!(meta.template_version, Some(1));
            assert_eq!(meta.exits["done"].result, "success");
            assert!(meta.capabilities.checkpoint);
            if template == "loop" {
                assert_eq!(
                    meta.declared_param_keys(),
                    std::collections::BTreeSet::from(["max_rounds".to_string()])
                );
                assert_eq!(
                    meta.param_defaults().expect("defaults")["max_rounds"],
                    oas_flow::Value::Num(3.0)
                );
            }
            let meta_text = std::fs::read_to_string(created.path.join("meta.json")).expect("meta");
            assert!(!meta_text.contains("{{id}}"), "placeholder not substituted");
            assert!(meta_text.contains("我的任务"));
        }
    }

    #[test]
    fn create_task_refuses_overwrite_bad_template_and_bad_id() {
        let td = TempDir::new("refuse");
        let tasks = td.path().join("tasks");
        create_task(&tasks, "daily", "任务1").expect("first create");

        let err = create_task(&tasks, "daily", "任务1").expect_err("overwrite");
        assert!(err.contains("already exists"), "{err}");

        let err = create_task(&tasks, "magic", "任务2").expect_err("unknown template");
        assert!(err.contains("unknown template"), "{err}");

        let err = create_task(&tasks, "daily", "../evil").expect_err("path traversal id");
        assert!(err.contains("invalid task id"), "{err}");

        let err = create_task(&tasks, "daily", ".hidden").expect_err("hidden id");
        assert!(err.contains("must not start with `.`"), "{err}");

        // No staging directory may leak from any failure path.
        let leaked: Vec<String> = std::fs::read_dir(&tasks)
            .expect("tasks dir")
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.starts_with(".oasd-new"))
            .collect();
        assert!(leaked.is_empty(), "staging dirs leaked: {leaked:?}");
    }

    #[test]
    fn lint_reports_generated_bundles_and_injected_meta_errors() {
        let td = TempDir::new("lintcli");
        let tasks = td.path().join("tasks");
        create_task(&tasks, "daily", "好任务").expect("create good");
        create_task(&tasks, "utility", "坏任务").expect("create bad");
        // INJECTION: `continue` is not one of the five result classes.
        std::fs::write(
            tasks.join("坏任务").join("meta.json"),
            r#"{"schema_version":1,"name":"坏任务","exits":{"done":{"result":"continue"}}}"#,
        )
        .expect("inject bad meta");

        let summary = lint(&tasks, None).expect("lint all");
        assert_eq!(summary.tasks, 2);
        assert_eq!(summary.errors, 1, "{:?}", summary.lines);
        assert!(
            summary.lines.iter().any(|l| l == "lint task=好任务 ok"),
            "{:?}",
            summary.lines
        );
        assert!(
            summary
                .lines
                .iter()
                .any(|l| l.contains("task=坏任务") && l.contains("code=META")),
            "{:?}",
            summary.lines
        );
        assert!(
            summary.lines.iter().any(|l| l == "linted=2 errors=1"),
            "{:?}",
            summary.lines
        );

        let one = lint(&tasks, Some("好任务")).expect("lint one");
        assert_eq!(one.tasks, 1);
        assert_eq!(one.errors, 0, "{:?}", one.lines);
        assert!(lint(&tasks, Some("不存在")).is_err(), "missing task is a setup error");
        assert!(lint_task(&tasks, "../evil").is_err(), "traversal rejected");
    }

    /// M0 acceptance: legacy `{schema_version,name,expire}` meta must pass lint
    /// without being edited. Use a temporary fixture instead of the repo `tasks/`
    /// directory: `tasks/` is user-owned data and may be replaced at any time.
    #[test]
    fn legacy_meta_tasks_pass_lint_without_shipped_data() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let tasks = std::env::temp_dir().join(format!(
            "oasd-legacy-lint-{}-{stamp}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&tasks);
        std::fs::create_dir_all(&tasks).expect("temp tasks dir");
        for name in ["S1切预设", "任务1"] {
            let dir = tasks.join(name);
            std::fs::create_dir_all(&dir).expect("task dir");
            // Deliberately the old meta shape only: schema_version/name/expire.
            std::fs::write(
                dir.join("meta.json"),
                r#"{"schema_version":1,"name":"legacy","expire":"never"}"#,
            )
            .expect("meta");
            std::fs::write(
                dir.join("flow.json"),
                r#"{"schema_version":1,"name":"legacy","steps":[
                    {"op":"finish","args":{"exit":"done"}}
                ]}"#,
            )
            .expect("flow");
        }
        for name in ["S1切预设", "任务1"] {
            let issues = lint_task(&tasks, name).unwrap_or_else(|e| panic!("task `{name}`: {e}"));
            assert!(issues.is_empty(), "legacy task `{name}` must lint green: {issues:?}");
        }
        let summary = lint(&tasks, None).expect("lint fixture");
        assert_eq!(summary.errors, 0, "{:?}", summary.lines);
        let _ = std::fs::remove_dir_all(&tasks);
    }
}
