//! OCR provider adapter for `oasd` (ENGINE-DESIGN §17.1 / §22.4).
//!
//! `oas-runtime` only knows the [`OcrProvider`] trait. The ONNX implementation
//! lives here so the default build (`cargo test --workspace`, no features) stays
//! free of `ort`: `main.rs` declares this module only under `--features ocr`, and
//! Cargo makes `oas-ocr` optional behind the same feature.
//!
//! Both OCR paths are supported on purpose — the user kept **rec 与 det 都要**
//! (§17.1 "回滚到rec保留吧 … 只保留rec+det"):
//! * `ocr_line`  → [`OcrProvider::recognize`] (fast single-line rec, 12~20ms);
//! * `ocr_lines` → [`OcrProvider::recognize_lines`] (det + rec, every line).
//!
//! ```text
//! assets.json (kind, roi) -> interpreter crops THAT roi out of the frame
//!   single line: RgbCrop::from_rgba -> RecEngine::recognize
//!   multi line : RgbCrop::from_rgba -> DetEngine::detect -> per-box rec
//! ```

use std::path::Path;

use oas_runtime::{OcrLine, OcrProvider};
use oas_types::{Frame, Rect};

/// PP-OCRv6 recogniser (+ optional text detector) behind the runtime seam.
pub struct OnnxOcr {
    rec: oas_ocr::onnx::RecEngine,
    det: Option<oas_ocr::det_engine::DetEngine>,
}

impl OnnxOcr {
    /// Load a `*_rec_*.onnx` model (charset from its metadata).
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, String> {
        Ok(Self {
            rec: oas_ocr::onnx::RecEngine::from_file(path)?,
            det: None,
        })
    }

    /// Load the text detector (`*_det_*.onnx`) — required by `ocr_lines` assets.
    pub fn with_det(mut self, path: impl AsRef<Path>) -> Result<Self, String> {
        self.det = Some(oas_ocr::det_engine::DetEngine::from_file(path)?);
        Ok(self)
    }
}

impl OcrProvider for OnnxOcr {
    fn recognize(&mut self, frame: &Frame, roi: Rect) -> Result<(String, f32), String> {
        let crop = oas_ocr::RgbCrop::from_rgba(frame.width, frame.height, &frame.rgba, roi)?;
        self.rec.recognize(&crop)
    }

    fn recognize_lines(&mut self, frame: &Frame, roi: Rect) -> Result<Vec<OcrLine>, String> {
        let det = self.det.as_mut().ok_or_else(|| {
            "no text detector loaded: `ocr_lines` needs det+rec - pass \
             --det-model <det.onnx> (or declare the asset as `ocr_line`)"
                .to_string()
        })?;
        recognize_lines_impl(det, &mut self.rec, frame, roi, false)
    }
}

/// Shared det+rec implementation used by:
/// - the runtime [`OcrProvider`] (task execution),
/// - the editor `GET /lines` endpoint,
/// - the frame-based `POST /task/assets/batch_test`.
///
/// This is the single source of truth for orientation retry and noise filtering;
/// otherwise editor tests and runtime can disagree (the 庭院 vertical sign bug
/// was exactly such a drift).
pub fn recognize_lines_impl(
    det: &mut oas_ocr::det_engine::DetEngine,
    rec: &mut oas_ocr::onnx::RecEngine,
    frame: &Frame,
    roi: Rect,
    vertical_only: bool,
) -> Result<Vec<OcrLine>, String> {
    let crop = oas_ocr::RgbCrop::from_rgba(frame.width, frame.height, &frame.rgba, roi)?;
    let boxes = det.detect(&crop)?;
    // det boxes are local to the ROI crop; re-crop each one from an RGBA copy
    // (RgbCrop::from_rgba works on RGBA, the rec engine wants RGB internally).
    let rgba = rgb_to_rgba(&crop);
    let mut out = Vec::with_capacity(boxes.len());
    for b in boxes {
        let sub = oas_ocr::RgbCrop::from_rgba(crop.width, crop.height, &rgba, b)?;
        let (mut text, mut score) = rec.recognize(&sub)?;
        // Vertical/rotated text (e.g. the 庭院 sign) is common in the game.
        // Both editor and runtime must try the same orientations.
        if vertical_only || score < 0.6 {
            for cand in [sub.rotate90_cw(), sub.rotate90_ccw()] {
                if let Ok((t2, s2)) = rec.recognize(&cand) {
                    if s2 > score {
                        text = t2;
                        score = s2;
                    }
                }
            }
        }
        // Drop low-confidence noise boxes consistently (the old editor path did
        // this; runtime used to keep everything).
        if score < 0.5 {
            continue;
        }
        out.push(OcrLine {
            rect: Rect {
                x: roi.x + b.x,
                y: roi.y + b.y,
                w: b.w,
                h: b.h,
            },
            text,
            score,
        });
    }
    Ok(out)
}

fn rgb_to_rgba(c: &oas_ocr::RgbCrop) -> Vec<u8> {
    let mut out = Vec::with_capacity(c.data.len() / 3 * 4);
    for px in c.data.chunks_exact(3) {
        out.extend_from_slice(&[px[0], px[1], px[2], 255]);
    }
    out
}
