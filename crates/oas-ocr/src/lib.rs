//! oas-ocr: PP-OCRv6 text recognition, pure-Rust pre/post processing.
//!
//! # Decision (design doc §16)
//! The OCR *engine* is NOT being replaced: the old repo already migrated from
//! PP-OCRv2 to RapidOCR **PP-OCRv6** after a three-generation comparison
//! (traditional Chinese 戀鳥樹: v2 read 懋岛树 0.70, v6 reads 戀鳥樹 0.99).
//! What changes is only the *runtime*: Python `onnxruntime` -> Rust `ort`,
//! reusing the SAME model files.
//!
//! # Three inherited rules (violating them loses accuracy)
//! 1. Feed **RGB**, never convert to BGR (old measurement: 0.77 vs 0.55).
//! 2. rec input shape is **[3, 48, 320]** (v6 is 48 high; v2 was 32).
//! 3. The charset lives in the ONNX metadata `custom_metadata_map["character"]`
//!    (newline separated) - no external keys file needed.
//!
//! # Scope
//! P1-c1 = recognition only. Our DSL gives every `ocr(sel)` an ROI, so we crop
//! and recognise directly; DBNet detection (for `find_text_in_list`) comes later.

use oas_types::Rect;

pub const REC_HEIGHT: u32 = 48;
pub const REC_MAX_WIDTH: u32 = 320;
pub const CTC_BLANK: usize = 0;

/// Character table decoded from the model metadata (`character` key).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Charset {
    chars: Vec<String>,
}

impl Charset {
    /// Build from the newline separated `character` metadata blob.
    ///
    /// PP-OCRv6's model already includes the CTC blank at index 0 and the space
    /// at the end, so we must NOT add specials ourselves (unlike the v2 path,
    /// which inserted `blank` and ` ` in code).
    pub fn from_metadata(blob: &str) -> Self {
        // Mimic Python's `str.splitlines()`: drop the trailing empty entry that
        // a final "\n" would otherwise produce. Do NOT trim spaces - the space
        // character is a legitimate line in the table.
        let mut chars: Vec<String> = blob
            .split('\n')
            .map(|s| s.trim_end_matches('\r').to_string())
            .collect();
        if chars.last().map(|s| s.is_empty()).unwrap_or(false) {
            chars.pop();
        }
        Self { chars }
    }

    pub fn from_lines(lines: Vec<String>) -> Self {
        Self { chars: lines }
    }

    pub fn len(&self) -> usize {
        self.chars.len()
    }

    pub fn is_empty(&self) -> bool {
        self.chars.is_empty()
    }

    pub fn get(&self, index: usize) -> Option<&str> {
        self.chars.get(index).map(|s| s.as_str())
    }
}

/// An owned RGB crop (tightly packed, 3 bytes per pixel).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbCrop {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl RgbCrop {
    pub fn from_slice(width: u32, height: u32, data: &[u8]) -> Self {
        assert_eq!(
            data.len(),
            width as usize * height as usize * 3,
            "rgb crop size mismatch"
        );
        Self {
            width,
            height,
            data: data.to_vec(),
        }
    }

    /// Crop an ROI out of an RGBA frame, dropping alpha (RGB order preserved).
    pub fn from_rgba(
        frame_w: u32,
        frame_h: u32,
        rgba: &[u8],
        roi: Rect,
    ) -> Result<Self, String> {
        let x = roi.x.max(0) as u32;
        let y = roi.y.max(0) as u32;
        let w = roi.w.max(0) as u32;
        let h = roi.h.max(0) as u32;
        if w == 0 || h == 0 {
            return Err(format!("roi {roi:?} is empty"));
        }
        if x + w > frame_w || y + h > frame_h {
            return Err(format!(
                "roi {roi:?} out of bounds for {frame_w}x{frame_h}"
            ));
        }
        let mut rgb = Vec::with_capacity((w * h * 3) as usize);
        for row in y..(y + h) {
            for col in x..(x + w) {
                let i = ((row * frame_w + col) * 4) as usize;
                rgb.push(rgba[i]);
                rgb.push(rgba[i + 1]);
                rgb.push(rgba[i + 2]);
            }
        }
        Ok(Self {
            width: w,
            height: h,
            data: rgb,
        })
    }

    pub fn preprocess(&self) -> RecTensor {
        preprocess_rec(self.width, self.height, &self.data)
    }
}

/// Normalised NCHW tensor: shape [1, 3, 48, W].
pub struct RecTensor {
    pub w: u32,
    pub data: Vec<f32>,
}

impl RecTensor {
    pub fn shape(&self) -> [i64; 4] {
        [1, 3, REC_HEIGHT as i64, self.w as i64]
    }
}

#[inline]
fn bilinear(src: &[u8], sw: u32, sh: u32, ch: usize, x: f32, y: f32) -> f32 {
    let x0 = x.floor().max(0.0) as u32;
    let y0 = y.floor().max(0.0) as u32;
    let x1 = (x0 + 1).min(sw - 1);
    let y1 = (y0 + 1).min(sh - 1);
    let fx = x - x0 as f32;
    let fy = y - y0 as f32;
    let at = |px: u32, py: u32| -> f32 { src[((py * sw + px) as usize) * 3 + ch] as f32 };
    let top = at(x0, y0) * (1.0 - fx) + at(x1, y0) * fx;
    let bot = at(x0, y1) * (1.0 - fx) + at(x1, y1) * fx;
    top * (1.0 - fy) + bot * fy
}

/// PP-OCRv6 recognition pre-processing.
///
/// * keeps the aspect ratio, resizes height to 48 (bilinear, like `cv2.resize`)
/// * `resized_w = min(320, ceil(48 * w/h))`
/// * normalises each channel with `(v/255 - 0.5)/0.5` (== v/127.5 - 1)
/// * right-pads with zeros to the computed width (batch of one)
///
/// RGB order is preserved on purpose (rule 1 in the module docs).
pub fn preprocess_rec(width: u32, height: u32, rgb: &[u8]) -> RecTensor {
    let h = height.max(1);
    let w = width.max(1);
    let ratio = w as f32 / h as f32;
    let resized_w = ((REC_HEIGHT as f32 * ratio).ceil() as u32)
        .clamp(1, REC_MAX_WIDTH);
    let shape_w = resized_w;
    let mut data = vec![0.0f32; 3 * (REC_HEIGHT as usize) * (shape_w as usize)];

    for ch in 0..3usize {
        for dy in 0..REC_HEIGHT {
            // pixel-centre mapping (matches cv2.resize's half-pixel convention)
            let sy = (dy as f32 + 0.5) * (h as f32) / (REC_HEIGHT as f32) - 0.5;
            for dx in 0..resized_w {
                let sx = (dx as f32 + 0.5) * (w as f32) / (resized_w as f32) - 0.5;
                let v = bilinear(
                    rgb,
                    w,
                    h,
                    ch,
                    sx.clamp(0.0, (w - 1) as f32),
                    sy.clamp(0.0, (h - 1) as f32),
                );
                let norm = (v / 255.0 - 0.5) / 0.5;
                let idx = ch * (REC_HEIGHT as usize) * (shape_w as usize)
                    + (dy as usize) * (shape_w as usize)
                    + (dx as usize);
                data[idx] = norm;
            }
        }
    }

    RecTensor { w: shape_w, data }
}

/// Greedy CTC decode over a `[T, C]` probability (or logit) matrix.
///
/// Returns the text and the mean confidence of the kept timesteps.
/// Blank (index 0) is skipped and consecutive duplicates are collapsed.
pub fn ctc_greedy_decode(
    probs: &[f32],
    steps: usize,
    classes: usize,
    charset: &Charset,
) -> (String, f32) {
    if steps == 0 || classes == 0 || probs.len() < steps * classes {
        return (String::new(), 0.0);
    }
    let mut text = String::new();
    let mut conf_sum = 0.0f32;
    let mut conf_n = 0usize;
    let mut prev: Option<usize> = None;

    for t in 0..steps {
        let row = &probs[t * classes..(t + 1) * classes];
        let mut best = 0usize;
        let mut best_p = row[0];
        for (c, &p) in row.iter().enumerate().skip(1) {
            if p > best_p {
                best_p = p;
                best = c;
            }
        }
        let same_as_prev = prev == Some(best);
        prev = Some(best);
        if best == CTC_BLANK || same_as_prev {
            continue;
        }
        if let Some(ch) = charset.get(best) {
            text.push_str(ch);
            conf_sum += best_p;
            conf_n += 1;
        }
    }
    let conf = if conf_n == 0 {
        0.0
    } else {
        conf_sum / conf_n as f32
    };
    (text, conf)
}

/// Softmax over one row, in place-ish (returns a new row).
pub fn softmax_row(row: &[f32]) -> Vec<f32> {
    let max = row.iter().cloned().fold(f32::MIN, f32::max);
    let exps: Vec<f32> = row.iter().map(|v| (v - max).exp()).collect();
    let sum: f32 = exps.iter().sum();
    exps.into_iter().map(|v| v / sum).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn charset() -> Charset {
        // index 0 = blank, 1..=3 = '1','2','3', 4 = 'text'
        Charset::from_lines(vec![
            "blank".into(),
            "1".into(),
            "2".into(),
            "3".into(),
            "text".into(),
        ])
    }

    #[test]
    fn metadata_blob_is_split_by_newline_and_keeps_embedded_blank() {
        let cs = Charset::from_metadata("blank\n1\n2\n3\r\n");
        assert_eq!(cs.len(), 4, "v6 metadata must not need extra specials");
        assert_eq!(cs.get(0), Some("blank"));
        assert_eq!(cs.get(1), Some("1"));
        assert_eq!(cs.get(3), Some("3"), "trailing \\r must be trimmed");
    }

    #[test]
    fn ctc_collapses_repeats_and_skips_blank() {
        let cs = charset();
        // timesteps: 1,1,0,2,3  => "123"
        let mut probs = vec![0.0f32; 5 * 5];
        let seq = [1usize, 1, 0, 2, 3];
        for (t, c) in seq.iter().enumerate() {
            probs[t * 5 + c] = 0.9;
        }
        let (text, conf) = ctc_greedy_decode(&probs, 5, 5, &cs);
        assert_eq!(text, "123");
        assert!((conf - 0.9).abs() < 1e-6, "conf={conf}");
    }

    #[test]
    fn ctc_same_char_with_blank_between_is_kept_twice() {
        let cs = charset();
        let mut probs = vec![0.0f32; 3 * 5];
        probs[0 * 5 + 2] = 0.9;
        probs[1 * 5 + 0] = 0.9;
        probs[2 * 5 + 2] = 0.9;
        let (text, _) = ctc_greedy_decode(&probs, 3, 5, &cs);
        assert_eq!(text, "22");
    }

    #[test]
    fn ctc_out_of_range_class_is_dropped_not_panicking() {
        let short = Charset::from_lines(vec!["blank".into()]);
        let mut probs = vec![0.0f32; 5];
        probs[4] = 0.9;
        let (text, conf) = ctc_greedy_decode(&probs, 1, 5, &short);
        assert_eq!(text, "");
        assert_eq!(conf, 0.0);
    }

    #[test]
    fn preprocess_matches_expected_shape_and_norm_range() {
        let (w, h) = (100u32, 48u32);
        let data = vec![255u8; (w * h * 3) as usize];
        let crop = RgbCrop::from_slice(w, h, &data);
        let t = crop.preprocess();
        assert_eq!(t.shape(), [1, 3, 48, 100]);
        assert!((t.data[0] - 1.0).abs() < 1e-6, "white => +1, got {}", t.data[0]);
    }

    #[test]
    fn preprocess_caps_wide_crops_at_320() {
        let (w, h) = (1000u32, 40u32);
        let data = vec![0u8; (w * h * 3) as usize];
        let t = RgbCrop::from_slice(w, h, &data).preprocess();
        assert_eq!(t.w, REC_MAX_WIDTH);
        assert_eq!(t.data.len(), 3 * 48 * 320);
        assert!((t.data[0] + 1.0).abs() < 1e-6, "black => -1, got {}", t.data[0]);
    }

    #[test]
    fn preprocess_keeps_aspect_ratio_for_square_crop() {
        let data = vec![128u8; 20 * 20 * 3];
        let t = RgbCrop::from_slice(20, 20, &data).preprocess();
        assert_eq!(t.w, 48, "square crop => resized width == 48");
    }

    #[test]
    fn from_rgba_crop_rejects_out_of_bounds() {
        let rgba = vec![0u8; 10 * 10 * 4];
        let bad = Rect { x: 5, y: 5, w: 10, h: 10 };
        assert!(RgbCrop::from_rgba(10, 10, &rgba, bad).is_err());
        let ok = Rect { x: 0, y: 0, w: 10, h: 10 };
        assert!(RgbCrop::from_rgba(10, 10, &rgba, ok).is_ok());
    }

    #[test]
    fn softmax_rows_sum_to_one() {
        let r = softmax_row(&[1.0, 2.0, 3.0]);
        let s: f32 = r.iter().sum();
        assert!((s - 1.0).abs() < 1e-6, "sum={s}");
    }
}
