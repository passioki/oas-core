//! oasd: minimal CLI for live checks (P0). No UI, no scheduler - just enough to
//! prove the capture/input ports work against a real device.
//!
//! Usage:
//!   oasd probe [--out <file.png>]   list devices (deduped) + capture one frame
//!   oasd tap <x> <y>                tap once (input port smoke test)

use std::process::ExitCode;

use oas_adapter_pc::Adb;
use oas_types::ScreenSource;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().map(|s| s.as_str()).unwrap_or("");
    match cmd {
        "probe" => probe(&args[1..]),
        "tap" => tap(&args[1..]),
        "" | "-h" | "--help" => {
            eprintln!("usage: oasd probe [--out <file.png>] | oasd tap <x> <y>");
            ExitCode::from(2)
        }
        other => {
            eprintln!("unknown command: {other}");
            ExitCode::from(2)
        }
    }
}

fn pick_device(adb: &Adb) -> Result<String, String> {
    let devices = adb.devices()?;
    if devices.is_empty() {
        return Err("no adb device attached".into());
    }
    for d in &devices {
        println!("device: serial={} unique={}", d.serial, d.unique);
    }
    println!("picked: {} (of {} deduped endpoint(s))", devices[0].serial, devices.len());
    Ok(devices[0].serial.clone())
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
        Err(e) => {
            eprintln!("probe failed: {e}");
            return ExitCode::FAILURE;
        }
    };

    let mut screen = adb.screen(&serial);
    let frame = match screen.grab() {
        Ok(f) => f,
        Err(e) => {
            eprintln!("capture failed: {e}");
            return ExitCode::FAILURE;
        }
    };
    println!(
        "captured frame: {}x{} rgba_bytes={}",
        frame.width,
        frame.height,
        frame.rgba.len()
    );

    if let Some(path) = out_path {
        match save_png(&frame, &path) {
            Ok(()) => println!("saved decoded frame -> {path}"),
            Err(e) => {
                eprintln!("save failed: {e}");
                return ExitCode::FAILURE;
            }
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
        Err(e) => {
            eprintln!("tap failed: {e}");
            return ExitCode::FAILURE;
        }
    };
    let mut input = adb.input(&serial);
    use oas_types::{InputSink, Point};
    match input.click(Point { x, y }) {
        Ok(()) => {
            println!("tapped ({x},{y}) on {serial}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("tap failed: {e}");
            ExitCode::FAILURE
        }
    }
}

fn save_png(frame: &oas_types::Frame, path: &str) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| format!("create {path}: {e}"))?;
    let mut enc = png::Encoder::new(file, frame.width, frame.height);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header().map_err(|e| format!("png header: {e}"))?;
    writer
        .write_image_data(&frame.rgba)
        .map_err(|e| format!("png data: {e}"))?;
    Ok(())
}
