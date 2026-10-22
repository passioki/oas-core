//! OCR provider adapter for `oasd` (P1-c, ENGINE-DESIGN §22.4).
//!
//! `oas-runtime` only knows the [`OcrProvider`] trait. The ONNX implementation
//! lives here so the default build (`cargo test --workspace`, no features) stays
//! free of `ort`: `main.rs` declares this module only under `--features ocr`, and
//! Cargo makes `oas-ocr` optional behind the same feature.
//!
//! Layout of one recognition (design §16.4: rec only, the ROI comes from the DSL):
//! ```text
//! assets.json item (kind=ocr_line, roi=[x,y,w,h])
//!   -> interpreter crops THAT roi out of the current frame
//!   -> RgbCrop::from_rgba (RGB, never BGR - §16.3 rule 1)
//!   -> RecEngine::recognize -> (text, score)
//! ```

use std::path::Path;

use oas_runtime::OcrProvider;
use oas_types::{Frame, Rect};

/// PP-OCRv6 recogniser behind the runtime's provider seam.
pub struct OnnxOcr {
    engine: oas_ocr::onnx::RecEngine,
}

impl OnnxOcr {
    /// Load a `*_rec_*.onnx` model (the charset is read from its metadata).
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, String> {
        Ok(Self {
            engine: oas_ocr::onnx::RecEngine::from_file(path)?,
        })
    }
}

impl OcrProvider for OnnxOcr {
    fn recognize(&mut self, frame: &Frame, roi: Rect) -> Result<(String, f32), String> {
        let crop = oas_ocr::RgbCrop::from_rgba(frame.width, frame.height, &frame.rgba, roi)?;
        self.engine.recognize(&crop)
    }
}
