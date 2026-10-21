//! Detect + recognise text lines in one ROI of a PNG.
//!
//! cargo run -p oas-ocr --features onnx --example lines -- \
//!   <det.onnx> <rec.onnx> <image.png> <x> <y> <w> <h> [vertical]

use oas_codec::decode_png;
use oas_ocr::det_engine::DetEngine;
use oas_ocr::onnx::RecEngine;
use oas_ocr::RgbCrop;
use oas_types::Rect;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    if a.len() < 7 {
        eprintln!("usage: lines <det.onnx> <rec.onnx> <image.png> <x> <y> <w> <h> [vertical]");
        std::process::exit(2);
    }
    let (det_path, rec_path, img_path) = (&a[0], &a[1], &a[2]);
    let roi = Rect {
        x: a[3].parse()?,
        y: a[4].parse()?,
        w: a[5].parse()?,
        h: a[6].parse()?,
    };
    let force_vertical = a.get(7).map(|s| s == "vertical").unwrap_or(false);

    let frame = decode_png(&std::fs::read(img_path)?)?;
    let crop = RgbCrop::from_rgba(frame.width, frame.height, &frame.rgba, roi)?;
    println!("crop: {}x{}", crop.width, crop.height);

    let mut det = DetEngine::from_file(det_path)?;
    let boxes = det.detect(&crop)?;
    println!("detected {} line(s)", boxes.len());

    let mut rec = RecEngine::from_file(rec_path)?;
    for (i, b) in boxes.iter().enumerate() {
        let sub = RgbCrop::from_rgba(crop.width, crop.height, &crop_to_rgba(&crop), *b)?;
        let (mut text, mut score) = rec.recognize(&sub)?;
        // vertical / rotated fallback: take the best of the 4 orientations
        if force_vertical || score < 0.6 {
            for cand in [sub.rotate90_cw(), sub.rotate90_ccw()] {
                let (t2, s2) = rec.recognize(&cand)?;
                if s2 > score {
                    text = t2;
                    score = s2;
                }
            }
        }
        println!(
            "  [{i}] box=({},{},{},{}) text={text:?} conf={score:.4}",
            b.x, b.y, b.w, b.h
        );
    }
    Ok(())
}

fn crop_to_rgba(c: &RgbCrop) -> Vec<u8> {
    let mut out = Vec::with_capacity(c.data.len() / 3 * 4);
    for px in c.data.chunks_exact(3) {
        out.extend_from_slice(&[px[0], px[1], px[2], 255]);
    }
    out
}
