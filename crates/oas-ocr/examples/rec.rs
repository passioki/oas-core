//! Recognise one ROI of a PNG with the PP-OCRv6 rec model.
//!
//! Usage (needs the `onnx` feature):
//!   cargo run -p oas-ocr --features onnx --example rec -- \
//!     <model.onnx> <image.png> <x> <y> <w> <h>

use oas_codec::decode_png;
use oas_ocr::onnx::RecEngine;
use oas_ocr::RgbCrop;
use oas_types::Rect;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    if a.len() != 6 {
        eprintln!("usage: rec <model.onnx> <image.png> <x> <y> <w> <h>");
        std::process::exit(2);
    }
    let model = &a[0];
    let img_path = &a[1];
    let roi = Rect {
        x: a[2].parse()?,
        y: a[3].parse()?,
        w: a[4].parse()?,
        h: a[5].parse()?,
    };

    let bytes = std::fs::read(img_path)?;
    let frame = decode_png(&bytes)?;
    println!("frame: {}x{}", frame.width, frame.height);

    let crop = RgbCrop::from_rgba(frame.width, frame.height, &frame.rgba, roi)?;
    println!("crop: {}x{}", crop.width, crop.height);

    let mut engine = RecEngine::from_file(model)?;
    println!("charset size: {}", engine.charset().len());

    let t0 = std::time::Instant::now();
    let (text, conf) = engine.recognize(&crop)?;
    println!(
        "text = {text:?}  conf = {conf:.4}  elapsed = {:.1} ms",
        t0.elapsed().as_secs_f64() * 1000.0
    );
    Ok(())
}
