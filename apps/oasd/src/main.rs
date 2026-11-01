//! oasd: minimal CLI for live checks (P0). No UI, no scheduler - just enough to
//! prove the capture/input/record/replay ports work.
//!
//! Usage:
//!   oasd probe  [--out <file.png>]   list devices (deduped) + capture one frame
//!   oasd tap <x> <y>                tap once (input port smoke test)
//!   oasd record <dir> <n>           record n live frames into a replay bundle
//!   oasd replay <dir>               re-drive a bundle offline, print frame hashes
//!   oasd run --task <dir> [--replay <dir>] [--seed <n>] [--model <rec.onnx>] ...
//!   oasd serve [--port N] [--ui <dir>] [--tasks-dir <dir>] [--runs-dir <dir>]

use std::process::ExitCode;

mod run;
mod serve;
mod coords;
mod checkpoint;
mod reconnect;
mod queue;
mod stats;
mod library;
mod schedule;
mod scheduler;
mod config;
mod task;
mod global_assets;
#[cfg(feature = "ocr")]
mod ocr;

use oas_adapter_pc::Adb;
use oas_codec::{encode_png, frame_hash};
use oas_obs::{read_events, read_meta, Recorder, ReplaySource};
use oas_types::{InputSink, Point, ScreenSource};

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    // Global deployment config (DELIVERY-PLAN M3 / ENGINE §22.34):
    //   oasd [--config config/live.json] <command> ...
    // The flag is consumed here so every subcommand can rely on the env/global
    // values without growing a `--config` parser of its own.
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
        "serve" => serve_cmd(&args[1..]),
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
                 oasd serve [--port N] [--ui <dir>] [--tasks-dir <dir>] [--runs-dir <dir>] [--assets-dir <dir>] [--frames-dir <dir>] [--model <path>] [--det-model <path>] [--no-open]"
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

fn serve_cmd(rest: &[String]) -> ExitCode {
    let mut port: u16 = 8787;
    let mut ui = "apps/oasd/ui".to_string();
    let mut tasks_dir = "tasks".to_string();
    let mut runs_dir = "runs".to_string();
    let mut model: Option<String> = None;
    let mut det_model: Option<String> = None;
    let mut library_dir = library::DEFAULT_DIR.to_string();
    let mut assets_dir = global_assets::DEFAULT_DIR.to_string();
    let mut frames_dir = "frames".to_string();
    let mut open_browser = true;
    let mut i = 0;
    while i < rest.len() {
        match rest[i].as_str() {
            "--port" if i + 1 < rest.len() => {
                port = match rest[i + 1].parse() {
                    Ok(p) => p,
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
    let adb = Adb::from_env();
    let mut server = serve::Server::new(adb)
        .with_tasks_dir(&tasks_dir)
        .with_runs_dir(&runs_dir)
        .with_library_dir(&library_dir)
        .with_assets_dir(&assets_dir)
        .with_frames_dir(&frames_dir)
        .with_open_browser(open_browser);
    enable_models(&mut server, model.as_deref(), det_model.as_deref());
    match server.run(port, &ui) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => fail("serve", e),
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
