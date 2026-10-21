//! PP-OCRv6 text **detection** (DBNet) - pure-Rust pre/post processing.
//!
//! Why this exists: `rec` only understands ONE HORIZONTAL text line. Feeding a
//! multi-line block squashes all lines into one, and a vertical sign is read as
//! a 90-degree-rotated mess. The standard pipeline is det -> crop each line ->
//! rec, which is what P1-c2 wires up.
//!
//! Parameters are taken from RapidOCR's config.yaml (PP-OCRv6 det small):
//!   limit_side_len = 736, limit_type = min, mean/std = 0.5/0.5
//!   thresh = 0.3, box_thresh = 0.5, unclip_ratio = 1.6
//!
//! Simplification vs the Python implementation: we use the connected
//! component's **bounding box** instead of a minimum-area rotated rectangle.
//! For game UI (horizontal and vertical text lines) this is sufficient, and it
//! removes the need for a full contour/min-area-rect implementation.

use oas_types::Rect;

pub const DET_SIDE_LEN: u32 = 736;
pub const DET_THRESH: f32 = 0.3;
pub const DET_BOX_THRESH: f32 = 0.5;
pub const DET_UNCLIP_RATIO: f32 = 1.6;
pub const DET_MAX_CANDIDATES: usize = 1000;

/// Normalised NCHW tensor for the detector.
pub struct DetTensor {
    pub w: u32,
    pub h: u32,
    /// scale applied to the source crop (map coords = source * scale)
    pub scale: f32,
    pub data: Vec<f32>,
}

impl DetTensor {
    pub fn shape(&self) -> [i64; 4] {
        [1, 3, self.h as i64, self.w as i64]
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

fn round_to_32(v: f32) -> u32 {
    (((v / 32.0).round() as i64) * 32).max(32) as u32
}

/// DBNet pre-processing: resize so the SHORT side is >= 736 (limit_type=min),
/// round both sides to multiples of 32, normalise with mean/std 0.5, RGB->NCHW.
pub fn preprocess_det(width: u32, height: u32, rgb: &[u8]) -> DetTensor {
    let (w, h) = (width.max(1), height.max(1));
    let short = w.min(h) as f32;
    let scale = if (short as u32) < DET_SIDE_LEN {
        DET_SIDE_LEN as f32 / short
    } else {
        1.0
    };
    let new_w = round_to_32(w as f32 * scale);
    let new_h = round_to_32(h as f32 * scale);
    let mut data = vec![0.0f32; 3 * (new_w as usize) * (new_h as usize)];
    let plane = (new_w as usize) * (new_h as usize);

    for ch in 0..3usize {
        for dy in 0..new_h {
            let sy = (dy as f32 + 0.5) * (h as f32) / (new_h as f32) - 0.5;
            for dx in 0..new_w {
                let sx = (dx as f32 + 0.5) * (w as f32) / (new_w as f32) - 0.5;
                let v = bilinear(
                    rgb,
                    w,
                    h,
                    ch,
                    sx.clamp(0.0, (w - 1) as f32),
                    sy.clamp(0.0, (h - 1) as f32),
                );
                // (v/255 - 0.5) / 0.5
                data[ch * plane + (dy as usize) * (new_w as usize) + (dx as usize)] =
                    (v / 255.0 - 0.5) / 0.5;
            }
        }
    }

    DetTensor {
        w: new_w,
        h: new_h,
        scale: (new_w as f32 / w as f32).max(new_h as f32 / h as f32),
        data,
    }
}

/// DB post-processing: binarise -> connected components -> box filter ->
/// unclip -> map back to source coordinates.
///
/// `map` is the probability map in row-major `[w * h]` order at map resolution.
pub fn postprocess_det(map: &[f32], w: u32, h: u32, src_w: u32, src_h: u32) -> Vec<Rect> {
    let (w, h) = (w as usize, h as usize);
    if w == 0 || h == 0 || map.len() < w * h {
        return Vec::new();
    }
    let mut seen = vec![false; w * h];
    let mut boxes: Vec<Rect> = Vec::new();
    let mut queue: Vec<usize> = Vec::new();

    let sx = src_w as f32 / w as f32;
    let sy = src_h as f32 / h as f32;

    for start in 0..(w * h) {
        if seen[start] || map[start] <= DET_THRESH {
            continue;
        }
        // flood fill one component (4-connectivity)
        queue.clear();
        queue.push(start);
        seen[start] = true;
        let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0usize, 0usize);
        let mut score_sum = 0.0f32;
        let mut count = 0usize;
        while let Some(idx) = queue.pop() {
            let (x, y) = (idx % w, idx / w);
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
            score_sum += map[idx];
            count += 1;
            if x > 0 {
                let n = idx - 1;
                if !seen[n] && map[n] > DET_THRESH {
                    seen[n] = true;
                    queue.push(n);
                }
            }
            if x + 1 < w {
                let n = idx + 1;
                if !seen[n] && map[n] > DET_THRESH {
                    seen[n] = true;
                    queue.push(n);
                }
            }
            if y > 0 {
                let n = idx - w;
                if !seen[n] && map[n] > DET_THRESH {
                    seen[n] = true;
                    queue.push(n);
                }
            }
            if y + 1 < h {
                let n = idx + w;
                if !seen[n] && map[n] > DET_THRESH {
                    seen[n] = true;
                    queue.push(n);
                }
            }
        }

        if count == 0 {
            continue;
        }
        let score = score_sum / count as f32;
        if score < DET_BOX_THRESH {
            continue;
        }
        let bw = (x1 - x0 + 1) as f32;
        let bh = (y1 - y0 + 1) as f32;
        if bw < 2.0 || bh < 2.0 {
            continue;
        }
        // unclip: DB expands the shrunk text region by area*ratio/perimeter
        let area = bw * bh;
        let perim = 2.0 * (bw + bh);
        let offset = area * DET_UNCLIP_RATIO / perim.max(1.0);

        let fx0 = ((x0 as f32 - offset) * sx).floor().max(0.0);
        let fy0 = ((y0 as f32 - offset) * sy).floor().max(0.0);
        let fx1 = ((x1 as f32 + 1.0 + offset) * sx).ceil().min(src_w as f32);
        let fy1 = ((y1 as f32 + 1.0 + offset) * sy).ceil().min(src_h as f32);
        if fx1 - fx0 < 2.0 || fy1 - fy0 < 2.0 {
            continue;
        }
        boxes.push(Rect {
            x: fx0 as i32,
            y: fy0 as i32,
            w: (fx1 - fx0) as i32,
            h: (fy1 - fy0) as i32,
        });
        if boxes.len() >= DET_MAX_CANDIDATES {
            break;
        }
    }

    // reading order: top-to-bottom, then left-to-right
    boxes.sort_by(|a, b| a.y.cmp(&b.y).then(a.x.cmp(&b.x)));
    boxes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map_with_blocks(w: u32, h: u32, blocks: &[(u32, u32, u32, u32, f32)]) -> Vec<f32> {
        let mut m = vec![0.0f32; (w * h) as usize];
        for &(bx, by, bw, bh, v) in blocks {
            for y in by..(by + bh) {
                for x in bx..(bx + bw) {
                    m[(y * w + x) as usize] = v;
                }
            }
        }
        m
    }

    #[test]
    fn det_preprocess_scales_short_side_and_rounds_to_32() {
        // 100x50 crop: short side 50 < 736 => scale ~14.72, both sides %32 == 0
        let rgb = vec![128u8; 100 * 50 * 3];
        let t = preprocess_det(100, 50, &rgb);
        assert_eq!(t.w % 32, 0);
        assert_eq!(t.h % 32, 0);
        assert!(t.h >= DET_SIDE_LEN, "short side must reach 736, got {}", t.h);
        assert_eq!(t.data.len(), 3 * (t.w as usize) * (t.h as usize));
    }

    #[test]
    fn det_finds_two_stacked_lines_in_reading_order() {
        // map 64x64 with two 20x8 blocks stacked vertically
        let w = 64u32;
        let h = 64u32;
        let m = map_with_blocks(w, h, &[(10, 10, 30, 8, 0.9), (12, 40, 28, 8, 0.9)]);
        let boxes = postprocess_det(&m, w, h, 128, 128);
        assert_eq!(boxes.len(), 2, "got {boxes:?}");
        assert!(boxes[0].y < boxes[1].y, "reading order top-to-bottom");
        // unclip must expand beyond the raw block
        assert!(boxes[0].h > 8, "unclip should expand the box: {:?}", boxes[0]);
    }

    #[test]
    fn det_ignores_low_confidence_components() {
        let w = 32u32;
        let h = 32u32;
        // block above thresh (0.3) but below box_thresh (0.5)
        let m = map_with_blocks(w, h, &[(5, 5, 10, 6, 0.35)]);
        assert!(postprocess_det(&m, w, h, 64, 64).is_empty());
    }

    #[test]
    fn det_ignores_tiny_speckles() {
        let w = 32u32;
        let h = 32u32;
        let m = map_with_blocks(w, h, &[(7, 7, 1, 1, 0.9)]);
        assert!(postprocess_det(&m, w, h, 64, 64).is_empty());
    }

    #[test]
    fn det_scales_boxes_back_to_source_coordinates() {
        // map 64x64 -> source 256x256 => factor 4
        let w = 64u32;
        let h = 64u32;
        let m = map_with_blocks(w, h, &[(16, 16, 8, 8, 0.99)]);
        let b = postprocess_det(&m, w, h, 256, 256);
        assert_eq!(b.len(), 1);
        // raw block starts at 16*4=64, but unclip (offset 3.2 map px = 12.8 src px)
        // moves the left edge out to ~51
        assert!(
            (b[0].x as f32 - 51.0).abs() < 8.0,
            "box x should be ~51 after unclip+scaling, got {}",
            b[0].x
        );
        assert!(
            b[0].w > 32,
            "unclip must widen the box beyond the raw 8*4=32 px, got {}",
            b[0].w
        );
        assert!(b[0].x + b[0].w <= 256 && b[0].y + b[0].h <= 256);
    }

    #[test]
    fn det_handles_empty_and_short_maps() {
        assert!(postprocess_det(&[], 10, 10, 10, 10).is_empty());
        assert!(postprocess_det(&[0.0], 10, 10, 10, 10).is_empty());
        assert!(postprocess_det(&vec![0.0; 100], 10, 10, 10, 10).is_empty());
    }
}
