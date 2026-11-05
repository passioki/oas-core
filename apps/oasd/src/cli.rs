//! The oasd command-line layer (Phase 1a of the Runner/Studio split).
//!
//! This is the **whole CLI** that used to live in `main.rs`: global `--config`
//! parsing, command dispatch, the probe/tap/record/replay/assets commands and
//! the two long-running surfaces (`studio` / `runner`). It lives in the library
//! so a separate, thin application (the future `oas-runner` repo) can reuse the
//! exact same entry point instead of copying it.
//!
//! Usage:
//!   oasd probe  [--out <file.png>]   list devices (deduped) + capture one frame
//!   oasd tap <x> <y>                tap once (input port smoke test)
//!   oasd record <dir> <n>           record n live frames into a replay bundle
//!   oasd replay <dir>               re-drive a bundle offline, print frame hashes
//!   oasd run --task <dir> [--replay <dir>] [--seed <n>] [--model <rec.onnx>] ...
//!   oasd studio [--port N] [--ui <dir>] ...   editor surface (default 8788)
//!   oasd runner [--port N] [--ui <dir>] ...   usage surface (default 8789)
//!   oasd serve ...                            deprecated alias for `studio`

use std::process::ExitCode;

use oas_adapter_pc::Adb;
use oas_codec::{encode_png, frame_hash};
use oas_obs::{read_events, read_meta, Recorder, ReplaySource};
use oas_types::{InputSink, Point, ScreenSource};

// Phase 1a: these modules now live at the crate root (`lib.rs`), not in this
// file; importing them keeps the dispatch table as readable as the old main().
use crate::{
    config, global_assets, library, package_cmd, queue, run, schedule, serve, stats, task,
};

/// The `oasd` binary entry point: dispatch `std::env::args()` verbatim.
pub fn run() -> ExitCode {
    run_with_args(std::env::args().skip(1))
}

/// Surface-locked entry point for a **separate** application crate.
///
/// It is exactly [`run`] with `surface` inserted as argv[1], i.e.
/// `run_as("runner")` behaves like the user typed `oasd runner <args...>`:
/// `--config` is still parsed globally (it is stripped before dispatch), and
/// every surface flag of `serve_cmd` keeps working. Used by the thin
/// `oasd-runner` app that lives outside this repository (Phase 1 of the
/// Runner/Studio split, `RUNNER-SPLIT-DESIGN.md §3.5`).
pub fn run_as(surface: &str) -> ExitCode {
    run_as_with_args(surface, Vec::new())
}

/// [`run_as`] with extra **default** arguments inserted before the process args.
///
/// Used by the independent `oasd-runner` app to inject defaults that only make
/// sense in its own repository (e.g. `--ui ui` instead of oasd's
/// `apps/oasd/ui`). Process arguments come last, so users can always override
/// the defaults on the command line.
pub fn run_as_with_args(
    surface: &str,
    extra: impl IntoIterator<Item = String>,
) -> ExitCode {
    let args = std::iter::once(surface.to_string())
        .chain(extra)
        .chain(std::env::args().skip(1));
    run_with_args(args)
}

/// Shared implementation of [`run`] / [`run_as`].
fn run_with_args(args: impl IntoIterator<Item = String>) -> ExitCode {
    let mut args: Vec<String> = args.into_iter().collect();
    // Global deployment config (DELIVERY-PLAN M3 / ENGINE §22.34):
    //   oasd [--config config/live.json] <command> ...
    // The flag is consumed here so every subcommand can rely on the env/global
    // values without growing a `--config` parser of its own.
    //
    // NOTE (docs/SCHEDULER-RUNNER-PLAN.md §三 P0): `--config` is the GLOBAL live
    // deployment config (`config/live.json`) and is stripped *before* dispatch,
    // so it never reaches the scheduler parser. The failure modes are asymmetric:
    // * `oasd scheduler --config scheduler.json` fails loudly here — the plan
    //   file is validated as an `OasConfig` (`deny_unknown_fields`) and the
    //   process exits 2 with `config error: config JSON: unknown field
    //   'config'`; no plan runs at all;
    // * `oasd scheduler --config config/live.json` loads a valid live config,
    //   the flag is consumed here, and the scheduler parser silently falls back
    //   to the default `scheduler.json` (this is the only "silent" half).
    // The schedule plan is selected with `--plan <scheduler.json>`.
    let mut config_path: Option<String> = None;
    let mut i = 0usize;
    while i < args.len() {
        if args[i] == "--config" {
            if i + 1 >= args.len() {
                eprintln!("--config requires a file path");
                return ExitCode::from(2);
            }
            config_path = Some(args[i + 1].clone());
            args.drain(i..=i + 1);
        } else if let Some(p) = args[i].strip_prefix("--config=") {
            config_path = Some(p.to_string());
            args.remove(i);
        } else if args[i] == "--version" || args[i] == "-V" {
            println!("oasd {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        } else {
            i += 1;
        }
    }
    if let Some(path) = config_path {
        match config::OasConfig::load(std::path::Path::new(&path)) {
            Ok(cfg) => {
                cfg.apply_env();
                config::set_global(cfg);
                config::set_global_path(std::path::PathBuf::from(&path));
                println!("config={path}");
            }
            Err(e) => {
                eprintln!("config error: {e}");
                return ExitCode::from(2);
            }
        }
    }

    let cmd = args.first().map(|s| s.as_str()).unwrap_or("");
    match cmd {
        "probe" => probe(&args[1..]),
        "tap" => tap(&args[1..]),
        "record" => record(&args[1..]),
        "replay" => replay(&args[1..]),
        "run" => run::run_cmd(&args[1..]),
        "queue" => queue::queue_cmd(&args[1..]),
        "schedule" => schedule::schedule_cmd(&args[1..]),
        "scheduler" => schedule::scheduler_cmd(&args[1..]),
        "stats" => stats::stats_cmd(&args[1..]),
        "task" => task::task_cmd(&args[1..]),
        "assets" => assets_cmd(&args[1..]),
        // PR1: one binary, two surfaces. `serve` stays as a deprecated alias so
        // existing `start-editor.bat`/docs/scripts keep working (design Q2).
        "studio" => {
            // PR5: `oasd studio publish` is a CLI action on the Studio surface,
            // not a long-running server (`oasd studio` with no subcommand still
            // starts the server on 8788).
            if args.get(1).map(String::as_str) == Some("publish") {
                package_cmd::publish_cmd(&args[2..])
            } else {
                serve_cmd(&args[1..], serve::Surface::Studio)
            }
        }
        "runner" => serve_cmd(&args[1..], serve::Surface::Runner),
        "serve" => {
            eprintln!(
                "note: 'oasd serve' is deprecated, use 'oasd studio' (same Studio surface)"
            );
            serve_cmd(&args[1..], serve::Surface::Studio)
        }
        "" | "-h" | "--help" => {
            eprintln!(
                "usage:\n  oasd [--config <live.json>] [--version] <command> ...\n  \
                 oasd probe [--out <file.png>]\n  oasd tap <x> <y>\n  \
                 oasd record <dir> <n>\n  oasd replay <dir>\n  \
                 oasd run --task <bundle-dir> [--replay <bundle-dir>] [--seed <u64>] \
                 [--dry-run] [--record <bundle-dir>] [--events-out <file.jsonl>] \
                 [--clicks-out <file.jsonl>] [--model <rec.onnx>] [--det-model <det.onnx>] \
                 [--budget-steps N] [--budget-wall S] [--param k=v]...\n  \
                 oasd task lint --tasks-dir <dir> [--all | --task <name>]\n  \
                 oasd task new --template daily|loop|utility --id <id> --tasks-dir <dir>\n  \
                 oasd assets migrate [--tasks-dir <dir>] [--library-dir <dir>] [--assets-dir <dir>]\n  \
                 oasd assets cleanup-legacy [--tasks-dir <dir>] [--assets-dir <dir>] [--apply]\n  \
                 oasd studio [--port N] [--ui <dir>] [--tasks-dir <dir>] [--runs-dir <dir>] [--assets-dir <dir>] [--frames-dir <dir>] [--model <path>] [--det-model <path>] [--no-open]   (default port 8788)\n  \
                 oasd studio publish --task <id> [--out <dir|file.oaspkg>] [--tasks-dir <dir>] [--assets-dir <dir>] [--force] [--include-deps] [--reproducible]\n  \
                 oasd runner [--port N] [--ui <dir>] [--tasks-dir <dir>] [--runs-dir <dir>] [--assets-dir <dir>] [--frames-dir <dir>] [--model <path>] [--det-model <path>] [--no-scheduler] [--no-open]   (default port 8789; --no-scheduler disables the embedded scheduler thread)\n  \
                 oasd serve  ... (deprecated alias of `oasd studio`, default port 8788)"
            );
            ExitCode::from(2)
        }
        other => {
            eprintln!("unknown command: {other}");
            ExitCode::from(2)
        }
    }
}

fn pick_device(adb: &Adb) -> Result<String, String> {
    let wanted = config::device_serial();
    let dev = adb.resolve_device(wanted.as_deref())?;
    println!(
        "device: serial={} aliases={:?} unique={}",
        dev.serial, dev.aliases, dev.unique
    );
    println!(
        "picked: {} (of {} deduped endpoint(s))",
        dev.serial,
        adb.devices().map(|d| d.len()).unwrap_or(0)
    );
    Ok(dev.serial)
}

fn probe(rest: &[String]) -> ExitCode {
    let mut out_path: Option<String> = None;
    let mut i = 0;
    while i < rest.len() {
        if rest[i] == "--out" && i + 1 < rest.len() {
            out_path = Some(rest[i + 1].clone());
            i += 2;
        } else {
            eprintln!("unknown probe arg: {}", rest[i]);
            return ExitCode::from(2);
        }
    }

    let adb = Adb::from_env();
    let serial = match pick_device(&adb) {
        Ok(s) => s,
        Err(e) => return fail("probe", e),
    };

    let mut screen = adb.screen(&serial);
    let frame = match screen.grab() {
        Ok(f) => f,
        Err(e) => return fail("capture", e),
    };
    println!(
        "captured frame: {}x{} rgba_bytes={} hash={:016x}",
        frame.width,
        frame.height,
        frame.rgba.len(),
        frame_hash(&frame)
    );

    if let Some(path) = out_path {
        match encode_png(&frame).and_then(|png| {
            std::fs::write(&path, png).map_err(|e| format!("write {path}: {e}"))
        }) {
            Ok(()) => println!("saved decoded frame -> {path}"),
            Err(e) => return fail("save", e),
        }
    }
    ExitCode::SUCCESS
}

fn tap(rest: &[String]) -> ExitCode {
    if rest.len() != 2 {
        eprintln!("usage: oasd tap <x> <y>");
        return ExitCode::from(2);
    }
    let (x, y) = match (rest[0].parse::<i32>(), rest[1].parse::<i32>()) {
        (Ok(x), Ok(y)) => (x, y),
        _ => {
            eprintln!("x/y must be integers");
            return ExitCode::from(2);
        }
    };
    let adb = Adb::from_env();
    let serial = match pick_device(&adb) {
        Ok(s) => s,
        Err(e) => return fail("tap", e),
    };
    let mut input = adb.input(&serial);
    match input.click(Point { x, y }) {
        Ok(()) => {
            println!("tapped ({x},{y}) on {serial}");
            ExitCode::SUCCESS
        }
        Err(e) => fail("tap", e),
    }
}

fn record(rest: &[String]) -> ExitCode {
    if rest.len() != 2 {
        eprintln!("usage: oasd record <dir> <n>");
        return ExitCode::from(2);
    }
    let dir = &rest[0];
    let n: usize = match rest[1].parse() {
        Ok(v) if v > 0 => v,
        _ => {
            eprintln!("<n> must be a positive integer");
            return ExitCode::from(2);
        }
    };

    let adb = Adb::from_env();
    let serial = match pick_device(&adb) {
        Ok(s) => s,
        Err(e) => return fail("record", e),
    };
    let mut screen = adb.screen(&serial);
    let mut rec = match Recorder::create(dir, format!("adb:{serial}")) {
        Ok(r) => r,
        Err(e) => return fail("recorder", e),
    };
    for k in 0..n {
        match rec.capture(&mut screen) {
            Ok(hash) => println!("frame {k}: hash={hash:016x}"),
            Err(e) => return fail("capture", e),
        }
    }
    match rec.finish() {
        Ok(meta) => {
            println!(
                "recorded {} frame(s) {}x{} -> {dir} (schema v{})",
                meta.frames, meta.width, meta.height, meta.schema_version
            );
            ExitCode::SUCCESS
        }
        Err(e) => fail("finish", e),
    }
}

fn replay(rest: &[String]) -> ExitCode {
    if rest.len() != 1 {
        eprintln!("usage: oasd replay <dir>");
        return ExitCode::from(2);
    }
    let dir = &rest[0];
    match read_meta(dir) {
        Ok(m) => println!(
            "bundle: {}x{} frames={} source={} schema=v{}",
            m.width, m.height, m.frames, m.source, m.schema_version
        ),
        Err(e) => return fail("meta", e),
    }
    let events = match read_events(dir) {
        Ok(e) => e,
        Err(e) => return fail("events", e),
    };
    println!("events: {}", events.len());

    let mut src = match ReplaySource::open(dir) {
        Ok(s) => s,
        Err(e) => return fail("open", e),
    };
    let total = src.len();
    let mut k = 0usize;
    loop {
        match src.grab() {
            Ok(f) => {
                println!("frame {k}: {}x{} hash={:016x}", f.width, f.height, frame_hash(&f));
                k += 1;
            }
            Err(e) => {
                println!("replay stopped after {k}/{total}: {e}");
                break;
            }
        }
    }
    if k == total {
        println!("replay OK: all {total} frame(s) decoded");
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn assets_cmd(rest: &[String]) -> ExitCode {
    let sub = rest.first().map(|s| s.as_str()).unwrap_or("");
    if sub != "migrate" && sub != "cleanup-legacy" {
        eprintln!(
            "usage:\n  oasd assets migrate [--tasks-dir <dir>] [--library-dir <dir>] [--assets-dir <dir>]\n  oasd assets cleanup-legacy [--tasks-dir <dir>] [--assets-dir <dir>] [--apply]"
        );
        return ExitCode::from(2);
    }
    let mut tasks_dir = "tasks".to_string();
    let mut library_dir = library::DEFAULT_DIR.to_string();
    let mut assets_dir = global_assets::DEFAULT_DIR.to_string();
    let mut apply = false;
    let mut i = 1usize;
    while i < rest.len() {
        match rest[i].as_str() {
            "--tasks-dir" if i + 1 < rest.len() => {
                tasks_dir = rest[i + 1].clone();
                i += 2;
            }
            "--library-dir" if i + 1 < rest.len() => {
                library_dir = rest[i + 1].clone();
                i += 2;
            }
            "--assets-dir" if i + 1 < rest.len() => {
                assets_dir = rest[i + 1].clone();
                i += 2;
            }
            "--apply" => {
                apply = true;
                i += 1;
            }
            other => {
                eprintln!("unknown assets arg: {other}");
                return ExitCode::from(2);
            }
        }
    }
    if sub == "cleanup-legacy" {
        return match global_assets::cleanup_legacy(
            std::path::Path::new(&tasks_dir),
            std::path::Path::new(&assets_dir),
            apply,
        ) {
            Ok(report) => {
                println!(
                    "assets cleanup-legacy {}: tasks={} candidates={} backup={}",
                    if report.dry_run { "DRY-RUN" } else { "OK" },
                    report.tasks_scanned,
                    report.items.len(),
                    report
                        .backup_dir
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "(not applied)".to_string())
                );
                for item in report.items {
                    println!(
                        "  {} assets.json={} images={}",
                        item.task, item.assets_json, item.images_dir
                    );
                }
                ExitCode::SUCCESS
            }
            Err(e) => fail("assets cleanup-legacy", e),
        };
    }
    match global_assets::migrate(
        std::path::Path::new(&tasks_dir),
        std::path::Path::new(&library_dir),
        std::path::Path::new(&assets_dir),
    ) {
        Ok(report) => {
            println!(
                "assets migrate OK: assets={} tasks={} flows_rewritten={} renamed={}",
                report.assets,
                report.tasks,
                report.flows_rewritten,
                report.renamed.len()
            );
            for r in report.renamed {
                println!("  renamed {} -> {} ({})", r.old, r.new, r.source);
            }
            ExitCode::SUCCESS
        }
        Err(e) => fail("assets migrate", e),
    }
}

fn serve_cmd(rest: &[String], surface: serve::Surface) -> ExitCode {
    // `None` = not on the command line; resolve from the global live config
    // (when loaded), then from the surface default. CLI still wins.
    let mut port: Option<u16> = None;
    // Both surfaces ship from the same `ui/` directory: Studio uses
    // index/app/builder..., Runner uses runner.html/runner.js/runner.css plus
    // the shared style.css. Packaging can still override it with `--ui`.
    let mut ui = "apps/oasd/ui".to_string();
    let mut tasks_dir = "tasks".to_string();
    let mut runs_dir = "runs".to_string();
    let mut model: Option<String> = None;
    let mut det_model: Option<String> = None;
    let mut library_dir = library::DEFAULT_DIR.to_string();
    let mut assets_dir = global_assets::DEFAULT_DIR.to_string();
    let mut frames_dir = "frames".to_string();
    let mut open_browser = true;
    // PR6 (design §A.6.2): `oasd runner` starts the embedded scheduler thread by
    // default; `--no-scheduler` keeps the old CLI-only behaviour. Accepted on
    // both surfaces for symmetry, only the Runner acts on it.
    let mut auto_scheduler = true;
    let mut i = 0;
    while i < rest.len() {
        match rest[i].as_str() {
            "--port" if i + 1 < rest.len() => {
                port = match rest[i + 1].parse() {
                    Ok(p) => Some(p),
                    Err(_) => {
                        eprintln!("bad --port value");
                        return ExitCode::from(2);
                    }
                };
                i += 2;
            }
            "--ui" if i + 1 < rest.len() => {
                ui = rest[i + 1].clone();
                i += 2;
            }
            "--tasks-dir" if i + 1 < rest.len() => {
                tasks_dir = rest[i + 1].clone();
                i += 2;
            }
            "--runs-dir" if i + 1 < rest.len() => {
                runs_dir = rest[i + 1].clone();
                i += 2;
            }
            "--model" if i + 1 < rest.len() => {
                model = Some(rest[i + 1].clone());
                i += 2;
            }
            "--det-model" if i + 1 < rest.len() => {
                det_model = Some(rest[i + 1].clone());
                i += 2;
            }
            "--library-dir" if i + 1 < rest.len() => {
                library_dir = rest[i + 1].clone();
                i += 2;
            }
            "--assets-dir" if i + 1 < rest.len() => {
                assets_dir = rest[i + 1].clone();
                i += 2;
            }
            "--frames-dir" if i + 1 < rest.len() => {
                frames_dir = rest[i + 1].clone();
                i += 2;
            }
            "--no-open" => {
                open_browser = false;
                i += 1;
            }
            "--no-scheduler" => {
                auto_scheduler = false;
                i += 1;
            }
            "--open" => {
                open_browser = true;
                i += 1;
            }
            other => {
                eprintln!("unknown serve arg: {other}");
                return ExitCode::from(2);
            }
        }
    }
    // Port precedence: CLI `--port` > global live config for THIS surface >
    // built-in default. Studio and Runner never share a default (8788 / 8789).
    let port = resolve_surface_port(port, surface, crate::config::global());

    // P0-3 (`RUNNER-SPLIT-DESIGN.md` §6.5): the Runner release zip carries a
    // read-only `bundled-data/` snapshot. Import it exactly once into a fresh
    // data root (never overwrites an existing tasks/assets store). The Studio
    // surface never imports. Must run before the server touches the dirs.
    if surface == serve::Surface::Runner {
        let packages_dir = crate::config::global()
            .and_then(|cfg| cfg.packages_dir())
            .unwrap_or_else(|| {
                std::path::Path::new(&tasks_dir)
                    .parent()
                    .map(|p| p.join("packages"))
                    .unwrap_or_else(|| std::path::PathBuf::from("packages"))
            });
        match crate::bundled_data::import_first_run(
            std::path::Path::new("."),
            std::path::Path::new(&tasks_dir),
            std::path::Path::new(&assets_dir),
            &packages_dir,
        ) {
            Ok(report) if report.imported => eprintln!(
                "bundled data: imported {} task(s), {} file(s) into tasks/ + assets/",
                report.tasks, report.files
            ),
            Ok(report) => {
                if report.reason != "no_bundled_data" {
                    eprintln!("bundled data: skipped ({})", report.reason);
                }
            }
            Err(e) => eprintln!("warning: bundled data import failed: {e}"),
        }
    }

    let adb = Adb::from_env();
    let mut server = serve::Server::new(adb)
        .with_surface(surface)
        .with_tasks_dir(&tasks_dir)
        .with_runs_dir(&runs_dir)
        .with_library_dir(&library_dir)
        .with_assets_dir(&assets_dir)
        .with_frames_dir(&frames_dir)
        .with_open_browser(open_browser)
        .with_auto_scheduler(auto_scheduler);
    enable_models(&mut server, model.as_deref(), det_model.as_deref());
    match server.run(port, &ui) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => fail(surface.as_str(), e),
    }
}

/// Load OCR models when the `ocr` feature is on.
///
/// The whole point of this helper is the `#[cfg(not(feature = "ocr"))]` twin
/// below: calling `server.enable_ocr()` unconditionally is what made the
/// DEFAULT build (`cargo test --workspace` without features) fail to compile.
#[cfg(feature = "ocr")]
fn enable_models(server: &mut serve::Server, model: Option<&str>, det_model: Option<&str>) {
    if let Some(path) = model {
        match server.enable_ocr(path) {
            Ok(n) => println!("ocr model loaded: {path} (charset {n})"),
            Err(e) => eprintln!("ocr disabled: {e}"),
        }
    }
    if let Some(path) = det_model {
        match server.enable_det(path) {
            Ok(()) => println!("det model loaded: {path}"),
            Err(e) => eprintln!("det disabled: {e}"),
        }
    }
}

/// Default (feature-less) build: OCR flags are accepted but ignored, loudly.
#[cfg(not(feature = "ocr"))]
fn enable_models(_server: &mut serve::Server, model: Option<&str>, det_model: Option<&str>) {
    if model.is_some() || det_model.is_some() {
        eprintln!(
            "warning: --model/--det-model ignored: this build has no OCR \
             (rebuild with `--features ocr`)"
        );
    }
}

fn fail(what: &str, e: String) -> ExitCode {
    eprintln!("{what} failed: {e}");
    ExitCode::FAILURE
}

/// Resolve the HTTP port for one surface without touching the network.
///
/// Precedence (PR1): explicit `--port` > the global live config's port for
/// **this** surface (`serve.port` / `runner.port`) > the surface default.
/// Kept pure so the "studio 8788 / runner 8789, never mixed" rule is testable.
fn resolve_surface_port(
    cli_port: Option<u16>,
    surface: serve::Surface,
    cfg: Option<&config::OasConfig>,
) -> u16 {
    cli_port.unwrap_or_else(|| match (cfg, surface) {
        (Some(cfg), serve::Surface::Studio) => cfg.serve.port,
        (Some(cfg), serve::Surface::Runner) => cfg.runner.port,
        (None, s) => s.default_port(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::OasConfig;

    #[test]
    fn surface_ports_default_to_8788_and_8789_and_cli_wins() {
        assert_eq!(
            resolve_surface_port(None, serve::Surface::Studio, None),
            8788
        );
        assert_eq!(
            resolve_surface_port(None, serve::Surface::Runner, None),
            8789
        );

        let cfg = OasConfig::from_json(
            r#"{"serve":{"port":9008},"runner":{"port":9009}}"#,
        )
        .expect("valid config");
        assert_eq!(
            resolve_surface_port(None, serve::Surface::Studio, Some(&cfg)),
            cfg.serve.port,
            "Studio must read serve.port, not runner.port"
        );
        assert_eq!(
            resolve_surface_port(None, serve::Surface::Runner, Some(&cfg)),
            cfg.runner.port,
            "Runner must read runner.port, not serve.port"
        );
        assert_eq!(
            resolve_surface_port(Some(7777), serve::Surface::Runner, Some(&cfg)),
            7777,
            "CLI --port wins over config"
        );
    }
}
