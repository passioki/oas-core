//! oas-perceive: perception layer (template matching / OCR / page detection).
//!
//! OS-agnostic by design: depends only on `oas-types`, never on platform APIs.
//!
//! # NCC template matching (P0-2)
//!
//! Equivalent to OpenCV `TM_CCOEFF_NORMED`, computed in **exact integer form**
//! (no float rounding inside the sums):
//!
//! ```text
//! n   = tw*th
//! num  = n*SUM(I*T)  - SUM(T)*SUM(I)
//! denT = n*SUM(T^2)  - SUM(T)^2
//! denI = n*SUM(I^2)  - SUM(I)^2
//! R    = num / sqrt(denT * denI)
//! ```
//!
//! Window sums `SUM(I)` / `SUM(I^2)` come from integral images (O(1) per window);
//! only the dot product `SUM(I*T)` is O(n) per position.

use oas_types::{Frame, Rect};

#[derive(Debug, Clone)]
pub struct GrayImage {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl GrayImage {
    pub fn new(width: u32, height: u32, data: Vec<u8>) -> Self {
        assert_eq!(
            data.len(),
            width as usize * height as usize,
            "gray buffer size mismatch"
        );
        Self { width, height, data }
    }

    #[inline]
    pub fn pixel(&self, x: u32, y: u32) -> u8 {
        self.data[(y as usize) * (self.width as usize) + (x as usize)]
    }

    /// Box-filter downsample (odd leftovers are dropped). Used by the full-frame
    /// template search pyramid.
    pub fn downsample(&self, factor: u32) -> GrayImage {
        let f = factor.max(1) as usize;
        let w = ((self.width as usize) / f).max(1);
        let h = ((self.height as usize) / f).max(1);
        let mut data = vec![0u8; w * h];
        let src_w = self.width as usize;
        for y in 0..h {
            for x in 0..w {
                let mut sum = 0u32;
                let mut n = 0u32;
                for dy in 0..f {
                    for dx in 0..f {
                        let sx = x * f + dx;
                        let sy = y * f + dy;
                        if sx < src_w && sy < self.height as usize {
                            sum += self.data[sy * src_w + sx] as u32;
                            n += 1;
                        }
                    }
                }
                data[y * w + x] = (sum / n.max(1)) as u8;
            }
        }
        GrayImage {
            width: w as u32,
            height: h as u32,
            data,
        }
    }

    /// Crop a rectangle out of this gray image (bounds-checked).
    pub fn crop(&self, rect: Rect) -> Option<GrayImage> {
        if rect.w <= 0 || rect.h <= 0 {
            return None;
        }
        let x0 = rect.x.max(0) as usize;
        let y0 = rect.y.max(0) as usize;
        let x1 = (rect.x + rect.w) as usize;
        let y1 = (rect.y + rect.h) as usize;
        if x1 > self.width as usize || y1 > self.height as usize {
            return None;
        }
        let w = x1 - x0;
        let h = y1 - y0;
        let mut data = vec![0u8; w * h];
        for y in 0..h {
            let src = (y0 + y) * self.width as usize + x0;
            let dst = y * w;
            data[dst..dst + w].copy_from_slice(&self.data[src..src + w]);
        }
        Some(GrayImage {
            width: w as u32,
            height: h as u32,
            data,
        })
    }

    /// Approximate luma (77R + 150G + 29B) >> 8.
    /// NOTE: intentionally NOT OpenCV's exact formula; equivalence tests feed
    /// grayscale buffers directly so no conversion ambiguity is involved.
    pub fn from_frame(frame: &Frame) -> Self {
        let n = (frame.width as usize) * (frame.height as usize);
        let mut g = vec![0u8; n];
        for i in 0..n {
            let r = frame.rgba[i * 4] as u32;
            let gr = frame.rgba[i * 4 + 1] as u32;
            let b = frame.rgba[i * 4 + 2] as u32;
            g[i] = ((77 * r + 150 * gr + 29 * b) >> 8) as u8;
        }
        Self {
            width: frame.width,
            height: frame.height,
            data: g,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MatchBest {
    pub score: f32,
    pub x: i32,
    pub y: i32,
}

struct Integral {
    stride: usize,
    sum: Vec<i64>,
    sum2: Vec<i64>,
}

impl Integral {
    fn build(img: &GrayImage) -> Self {
        let w = img.width as usize;
        let h = img.height as usize;
        let stride = w + 1;
        let mut sum = vec![0i64; stride * (h + 1)];
        let mut sum2 = vec![0i64; stride * (h + 1)];
        for y in 0..h {
            let mut row_s = 0i64;
            let mut row_s2 = 0i64;
            for x in 0..w {
                let v = img.data[y * w + x] as i64;
                row_s += v;
                row_s2 += v * v;
                sum[(y + 1) * stride + (x + 1)] = sum[y * stride + (x + 1)] + row_s;
                sum2[(y + 1) * stride + (x + 1)] = sum2[y * stride + (x + 1)] + row_s2;
            }
        }
        Self { stride, sum, sum2 }
    }

    #[inline]
    fn at(buf: &[i64], stride: usize, x: usize, y: usize) -> i64 {
        buf[y * stride + x]
    }

    /// Inclusive-exclusive rectangle sum via the inclusion-exclusion rule.
    #[inline]
    fn rect(&self, x: usize, y: usize, w: usize, h: usize) -> (i64, i64) {
        let r = x + w;
        let b = y + h;
        let a0 = Self::at(&self.sum, self.stride, x, y);
        let a1 = Self::at(&self.sum, self.stride, r, y);
        let a2 = Self::at(&self.sum, self.stride, x, b);
        let a3 = Self::at(&self.sum, self.stride, r, b);
        let b0 = Self::at(&self.sum2, self.stride, x, y);
        let b1 = Self::at(&self.sum2, self.stride, r, y);
        let b2 = Self::at(&self.sum2, self.stride, x, b);
        let b3 = Self::at(&self.sum2, self.stride, r, b);
        (a3 - a1 - a2 + a0, b3 - b1 - b2 + b0)
    }
}

#[inline]
fn dot(img: &GrayImage, tpl: &GrayImage, px: usize, py: usize) -> i64 {
    let iw = img.width as usize;
    let tw = tpl.width as usize;
    let th = tpl.height as usize;
    let mut total: i64 = 0;
    for ty in 0..th {
        let irow = (py + ty) * iw + px;
        let trow = ty * tw;
        // row slices let LLVM auto-vectorize this loop.
        let is = &img.data[irow..irow + tw];
        let ts = &tpl.data[trow..trow + tw];
        // per-row i32 accumulator (safe: tw*255*255 <= i32 for any sane template
        // width), widened once per row => no overflow for big templates either.
        let mut row: i32 = 0;
        for k in 0..tw {
            row += (is[k] as i32) * (ts[k] as i32);
        }
        total += row as i64;
    }
    total
}

/// Reusable matcher for one frame: **owns** the gray frame and builds the
/// integral images **once**, so every template reuses them (building them per
/// template was a real 40x slowdown measured in P0-2).
///
/// Owning the frame (instead of borrowing) lets an interpreter cache one
/// matcher per captured frame in a struct field without lifetime gymnastics.
pub struct Matcher {
    img: GrayImage,
    integral: Integral,
}

impl Matcher {
    /// Take ownership of the frame (no copy).
    pub fn new(img: GrayImage) -> Self {
        let integral = Integral::build(&img);
        Self { img, integral }
    }

    /// Keep the caller's frame by reference (copies the gray buffer once).
    pub fn from_ref(img: &GrayImage) -> Self {
        Self::new(img.clone())
    }

    pub fn image(&self) -> &GrayImage {
        &self.img
    }

    /// Best NCC match of `tpl` inside the frame (optionally restricted to `roi`).
    pub fn match_template(&self, tpl: &GrayImage, roi: Option<Rect>) -> Option<MatchBest> {
        let img = &self.img;
        let (ox, oy, sw, sh) = match roi {
            Some(r) => (r.x, r.y, r.w, r.h),
            None => (0, 0, img.width as i32, img.height as i32),
        };
        let tw = tpl.width as i32;
        let th = tpl.height as i32;
        if tw <= 0 || th <= 0 {
            return None;
        }

        // clamp the search window into the image
        let x0 = ox.max(0);
        let y0 = oy.max(0);
        let x1 = (ox + sw).min(img.width as i32);
        let y1 = (oy + sh).min(img.height as i32);
        if x1 - x0 < tw || y1 - y0 < th {
            return None;
        }

        let n = (tw * th) as i64;

        // template statistics
        let mut sum_t: i64 = 0;
        let mut sum_t2: i64 = 0;
        for &v in tpl.data.iter() {
            let v = v as i64;
            sum_t += v;
            sum_t2 += v * v;
        }
        let den_t = n * sum_t2 - sum_t * sum_t;
        if den_t <= 0 {
            return None; // flat template
        }
        let den_t_f = den_t as f64;

        let mut best = MatchBest {
            score: -2.0,
            x: -1,
            y: -1,
        };

        let px_end = x1 - tw;
        let py_end = y1 - th;
        for py in y0..=py_end {
            for px in x0..=px_end {
                let (sum_i, sum_i2) =
                    self.integral
                        .rect(px as usize, py as usize, tw as usize, th as usize);
                let den_i = n * sum_i2 - sum_i * sum_i;
                if den_i <= 0 {
                    continue; // flat patch: undefined correlation
                }
                let d = dot(img, tpl, px as usize, py as usize);
                let num = n * d - sum_t * sum_i;
                let score = (num as f64) / ((den_t_f * den_i as f64).sqrt());
                let score = score.clamp(-1.0, 1.0) as f32;
                if score > best.score {
                    best = MatchBest {
                        score,
                        x: px,
                        y: py,
                    };
                }
            }
        }

        if best.score <= -1.5 {
            None
        } else {
            Some(best)
        }
    }
}

/// One-shot convenience (copies the frame and builds the integral image).
/// Prefer [`Matcher`] when matching several templates against the same frame.
pub fn match_template(img: &GrayImage, tpl: &GrayImage, roi: Option<Rect>) -> Option<MatchBest> {
    Matcher::from_ref(img).match_template(tpl, roi)
}

impl Matcher {
    /// Masked NCC (§22.10 N10b-1): only pixels where `mask[i]` is true take part
    /// in the correlation. Out-of-polygon template pixels can change freely
    /// (background/animation/neighbour content) without hurting the score.
    ///
    /// `mask` must have `tpl.width * tpl.height` entries (row-major, true = use).
    pub fn match_template_masked(
        &self,
        tpl: &GrayImage,
        mask: &[bool],
        roi: Option<Rect>,
    ) -> Option<MatchBest> {
        let img = &self.img;
        let tw = tpl.width as i32;
        let th = tpl.height as i32;
        if tw <= 0 || th <= 0 || mask.len() != (tw * th) as usize {
            return None;
        }
        let (ox, oy, sw, sh) = match roi {
            Some(r) => (r.x, r.y, r.w, r.h),
            None => (0, 0, img.width as i32, img.height as i32),
        };
        let x0 = ox.max(0);
        let y0 = oy.max(0);
        let x1 = (ox + sw).min(img.width as i32);
        let y1 = (oy + sh).min(img.height as i32);
        if x1 - x0 < tw || y1 - y0 < th {
            return None;
        }
        // Masked template statistics + the list of offsets that participate.
        let mut offsets: Vec<(usize, i64)> = Vec::new();
        let mut sum_t: i64 = 0;
        let mut sum_t2: i64 = 0;
        for j in 0..th as usize {
            for i in 0..tw as usize {
                let idx = j * tw as usize + i;
                if mask[idx] {
                    let v = tpl.data[idx] as i64;
                    offsets.push((idx, v));
                    sum_t += v;
                    sum_t2 += v * v;
                }
            }
        }
        let n = offsets.len() as i64;
        if n == 0 {
            return None;
        }
        let den_t = n * sum_t2 - sum_t * sum_t;
        if den_t <= 0 {
            return None; // flat masked template
        }
        let den_t_f = den_t as f64;
        let mut best = MatchBest {
            score: -2.0,
            x: -1,
            y: -1,
        };
        let iw = img.width as usize;
        let px_end = x1 - tw;
        let py_end = y1 - th;
        for py in y0..=py_end {
            for px in x0..=px_end {
                let mut sum_i: i64 = 0;
                let mut sum_i2: i64 = 0;
                let mut dot: i64 = 0;
                for &(idx, tv) in &offsets {
                    let dx = (idx % tw as usize) as i32;
                    let dy = (idx / tw as usize) as i32;
                    let v = img.data[(py + dy) as usize * iw + (px + dx) as usize] as i64;
                    sum_i += v;
                    sum_i2 += v * v;
                    dot += v * tv;
                }
                let den_i = n * sum_i2 - sum_i * sum_i;
                if den_i <= 0 {
                    continue;
                }
                let num = n * dot - sum_t * sum_i;
                let score = (num as f64) / ((den_t_f * den_i as f64).sqrt());
                let score = score.clamp(-1.0, 1.0) as f32;
                if score > best.score {
                    best = MatchBest {
                        score,
                        x: px,
                        y: py,
                    };
                }
            }
        }
        if best.score <= -1.5 {
            None
        } else {
            Some(best)
        }
    }
}

/// One NCC match candidate: template top-left in **frame** coordinates.
///
/// Produced by [`Matcher::match_template_all`] /
/// [`Matcher::match_template_masked_all`] for the v1.4 "whole-library operation
/// asset + multi-candidate nearest" feature (`region.asset` with
/// `mode=nearest`). The existing single-best APIs are untouched and remain the
/// only path used by ordinary clicks/appear checks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MatchCandidate {
    pub score: f32,
    pub x: i32,
    pub y: i32,
}

/// Upper bound on the candidate list returned by the `*_all` matchers.
///
/// `nearest` only ever needs the few closest candidates; the cap keeps a
/// pathological frame (hundreds of above-threshold local maxima) bounded.
pub const MAX_MATCH_CANDIDATES: usize = 64;

/// Upper bound on intermediate local maxima collected before NMS.
const MAX_SCAN_CANDIDATES: usize = 4096;

/// Score of a position that is absent / undefined (flat patch).
const NO_SCORE: f32 = f32::NEG_INFINITY;

/// NMS suppression radius in pixels: `max(2, min(tw, th)/2)`.
///
/// Candidates whose centres are closer than this are treated as the same
/// physical match (an NCC plateau or a one-pixel-shifted duplicate).
fn nms_radius(tpl: &GrayImage) -> f32 {
    (tpl.width.min(tpl.height) as f32 / 2.0).max(2.0)
}

/// Push one candidate if it is a 3x3 local maximum of the NCC score map.
///
/// Missing neighbours (image border) count as [`NO_SCORE`]. `>=` (not `>`) makes
/// a flat-topped peak survive once; NMS then removes the duplicate plateau
/// points.
fn push_if_local_max(
    out: &mut Vec<MatchCandidate>,
    above: &[f32],
    row: &[f32],
    below: &[f32],
    j: usize,
    py: i32,
    x0: i32,
    threshold: f32,
) {
    let v = row[j];
    if !(v >= threshold) {
        return;
    }
    let mut neighbours = [NO_SCORE; 8];
    let mut k = 0usize;
    for dy in [-1i32, 0, 1] {
        let line: &[f32] = match dy {
            -1 => above,
            0 => row,
            _ => below,
        };
        for dx in [-1i32, 0, 1] {
            if dx == 0 && dy == 0 {
                continue;
            }
            let x = j as i32 + dx;
            neighbours[k] = if x < 0 || x >= line.len() as i32 {
                NO_SCORE
            } else {
                line[x as usize]
            };
            k += 1;
        }
    }
    if neighbours.iter().all(|n| v >= *n) {
        out.push(MatchCandidate {
            score: v,
            x: x0 + j as i32,
            y: py,
        });
    }
}

/// Scan the whole score map for 3x3 local maxima above `threshold`.
///
/// `score_at(x, y)` is the NCC at that template top-left position (`None` =
/// undefined/flat patch). Keeps at most [`MAX_SCAN_CANDIDATES`] intermediate
/// candidates; the caller sorts + applies NMS + caps at
/// [`MAX_MATCH_CANDIDATES`].
fn scan_all_candidates(
    x0: i32,
    y0: i32,
    px_end: i32,
    py_end: i32,
    threshold: f32,
    mut score_at: impl FnMut(i32, i32) -> Option<f32>,
) -> Vec<MatchCandidate> {
    let cols = (px_end - x0 + 1).max(0) as usize;
    let rows = (py_end - y0 + 1).max(0) as usize;
    if cols == 0 || rows == 0 {
        return Vec::new();
    }
    let empty_row: Vec<f32> = vec![NO_SCORE; cols];
    let mut out: Vec<MatchCandidate> = Vec::new();
    let mut prev_rows: Vec<Vec<f32>> = Vec::with_capacity(3);
    for r in 0..rows {
        if out.len() >= MAX_SCAN_CANDIDATES {
            break;
        }
        let py = y0 + r as i32;
        let cur: Vec<f32> = (0..cols)
            .map(|j| score_at(x0 + j as i32, py).unwrap_or(NO_SCORE))
            .collect();
        prev_rows.push(cur);
        if prev_rows.len() == 3 {
            let mid_py = py - 1;
            for j in 0..cols {
                push_if_local_max(
                    &mut out,
                    &prev_rows[0],
                    &prev_rows[1],
                    &prev_rows[2],
                    j,
                    mid_py,
                    x0,
                    threshold,
                );
            }
            prev_rows.remove(0);
        }
    }
    // Flush the trailing rows (the loop above only finalizes rows that have a
    // row below them).
    match prev_rows.len() {
        2 => {
            let py = y0 + rows as i32 - 1;
            for j in 0..cols {
                push_if_local_max(
                    &mut out,
                    &prev_rows[0],
                    &prev_rows[1],
                    &empty_row,
                    j,
                    py,
                    x0,
                    threshold,
                );
            }
        }
        1 => {
            let py = y0 + rows as i32 - 1;
            for j in 0..cols {
                push_if_local_max(
                    &mut out,
                    &empty_row,
                    &prev_rows[0],
                    &empty_row,
                    j,
                    py,
                    x0,
                    threshold,
                );
            }
        }
        _ => {}
    }
    out
}

/// Sort candidates (score desc, then y/x asc for determinism), suppress
/// overlapping duplicates and cap at [`MAX_MATCH_CANDIDATES`].
fn finalize_candidates(mut candidates: Vec<MatchCandidate>, tpl: &GrayImage) -> Vec<MatchCandidate> {
    candidates.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.y.cmp(&b.y))
            .then(a.x.cmp(&b.x))
    });
    let radius = nms_radius(tpl);
    let radius2 = radius * radius;
    let mut kept: Vec<MatchCandidate> = Vec::new();
    for c in candidates {
        let overlaps = kept.iter().any(|k| {
            let dx = (c.x - k.x) as f32;
            let dy = (c.y - k.y) as f32;
            dx * dx + dy * dy < radius2
        });
        if overlaps {
            continue;
        }
        kept.push(c);
        if kept.len() >= MAX_MATCH_CANDIDATES {
            break;
        }
    }
    kept
}

impl Matcher {
    /// Search window clamp shared by the `*_all` matchers: the inclusive range of
    /// valid template top-left positions, or `None` when the template cannot fit.
    fn all_search_window(&self, tpl: &GrayImage, roi: Option<Rect>) -> Option<(i32, i32, i32, i32)> {
        let img = &self.img;
        let (ox, oy, sw, sh) = match roi {
            Some(r) => (r.x, r.y, r.w, r.h),
            None => (0, 0, img.width as i32, img.height as i32),
        };
        let tw = tpl.width as i32;
        let th = tpl.height as i32;
        if tw <= 0 || th <= 0 {
            return None;
        }
        let x0 = ox.max(0);
        let y0 = oy.max(0);
        let x1 = (ox + sw).min(img.width as i32);
        let y1 = (oy + sh).min(img.height as i32);
        if x1 - x0 < tw || y1 - y0 < th {
            return None;
        }
        Some((x0, y0, x1 - tw, y1 - th))
    }

    /// Plain (unmasked) NCC at one top-left position, using the integral images.
    fn score_plain_at(
        &self,
        tpl: &GrayImage,
        n: i64,
        sum_t: i64,
        den_t_f: f64,
        px: i32,
        py: i32,
    ) -> Option<f32> {
        let tw = tpl.width as usize;
        let th = tpl.height as usize;
        let (sum_i, sum_i2) = self
            .integral
            .rect(px as usize, py as usize, tw, th);
        let den_i = n * sum_i2 - sum_i * sum_i;
        if den_i <= 0 {
            return None; // flat patch: undefined correlation
        }
        let d = dot(&self.img, tpl, px as usize, py as usize);
        let num = n * d - sum_t * sum_i;
        let score = (num as f64) / ((den_t_f * den_i as f64).sqrt());
        Some(score.clamp(-1.0, 1.0) as f32)
    }

    /// **All** NCC candidates of `tpl` at or above `threshold`, after NMS.
    ///
    /// This is the multi-candidate companion of [`Self::match_template`] used by
    /// `region.asset` (`mode=nearest`); the single-best path is unchanged.
    /// Candidates are sorted by score descending, then `y`/`x` ascending, and
    /// capped at [`MAX_MATCH_CANDIDATES`].
    pub fn match_template_all(
        &self,
        tpl: &GrayImage,
        roi: Option<Rect>,
        threshold: f32,
    ) -> Vec<MatchCandidate> {
        let Some((x0, y0, px_end, py_end)) = self.all_search_window(tpl, roi) else {
            return Vec::new();
        };
        let tw = tpl.width as i64;
        let th = tpl.height as i64;
        let n = tw * th;
        let mut sum_t: i64 = 0;
        let mut sum_t2: i64 = 0;
        for &v in tpl.data.iter() {
            let v = v as i64;
            sum_t += v;
            sum_t2 += v * v;
        }
        let den_t = n * sum_t2 - sum_t * sum_t;
        if den_t <= 0 {
            return Vec::new(); // flat template
        }
        let den_t_f = den_t as f64;
        let candidates = scan_all_candidates(x0, y0, px_end, py_end, threshold, |px, py| {
            self.score_plain_at(tpl, n, sum_t, den_t_f, px, py)
        });
        finalize_candidates(candidates, tpl)
    }

    /// Masked (polygon-mask) companion of [`Self::match_template_all`].
    ///
    /// Same NMS/ordering/cap contract; the mask contract is identical to
    /// [`Self::match_template_masked`] (row-major, true = pixel participates).
    pub fn match_template_masked_all(
        &self,
        tpl: &GrayImage,
        mask: &[bool],
        roi: Option<Rect>,
        threshold: f32,
    ) -> Vec<MatchCandidate> {
        let tw_i = tpl.width as i32;
        let th_i = tpl.height as i32;
        if tw_i <= 0 || th_i <= 0 || mask.len() != (tw_i * th_i) as usize {
            return Vec::new();
        }
        let Some((x0, y0, px_end, py_end)) = self.all_search_window(tpl, roi) else {
            return Vec::new();
        };
        // Precompute the participating offsets + statistics once.
        let mut offsets: Vec<(usize, i64)> = Vec::new();
        let mut sum_t: i64 = 0;
        let mut sum_t2: i64 = 0;
        for j in 0..th_i as usize {
            for i in 0..tw_i as usize {
                let idx = j * tw_i as usize + i;
                if mask[idx] {
                    let v = tpl.data[idx] as i64;
                    offsets.push((idx, v));
                    sum_t += v;
                    sum_t2 += v * v;
                }
            }
        }
        let n = offsets.len() as i64;
        if n == 0 {
            return Vec::new();
        }
        let den_t = n * sum_t2 - sum_t * sum_t;
        if den_t <= 0 {
            return Vec::new();
        }
        let den_t_f = den_t as f64;
        let iw = self.img.width as usize;
        let tw = tw_i as usize;
        let candidates = scan_all_candidates(x0, y0, px_end, py_end, threshold, |px, py| {
            let mut sum_i: i64 = 0;
            let mut sum_i2: i64 = 0;
            let mut d: i64 = 0;
            for &(idx, tv) in &offsets {
                let dx = (idx % tw) as i32;
                let dy = (idx / tw) as i32;
                let v =
                    self.img.data[(py + dy) as usize * iw + (px + dx) as usize] as i64;
                sum_i += v;
                sum_i2 += v * v;
                d += v * tv;
            }
            let den_i = n * sum_i2 - sum_i * sum_i;
            if den_i <= 0 {
                return None;
            }
            let num = n * d - sum_t * sum_i;
            let score = (num as f64) / ((den_t_f * den_i as f64).sqrt());
            Some(score.clamp(-1.0, 1.0) as f32)
        });
        finalize_candidates(candidates, tpl)
    }
}

impl Matcher {
    /// Full-frame template search with a 4x pyramid: coarse scan on a downscaled
    /// frame, then a full-resolution refine around the coarse best. This is what
    /// gives layout-shift tolerance without a user-tuned search ROI.
    pub fn match_template_full(&self, tpl: &GrayImage) -> Option<MatchBest> {
        const F: u32 = 4;
        let tw = tpl.width as i32;
        let th = tpl.height as i32;
        let iw = self.img.width as i32;
        let ih = self.img.height as i32;
        if tw <= 0 || th <= 0 || tw > iw || th > ih {
            return None;
        }
        // Tiny templates: a pyramid would shrink them below usefulness.
        if tpl.width < F * 4 || tpl.height < F * 4 {
            return self.match_template(tpl, None);
        }
        let img_c = self.img.downsample(F);
        let tpl_c = tpl.downsample(F);
        if tpl_c.width == 0 || tpl_c.height == 0 {
            return self.match_template(tpl, None);
        }
        let coarse = Matcher::from_ref(&img_c).match_template(&tpl_c, None)?;
        let cx = coarse.x * F as i32;
        let cy = coarse.y * F as i32;
        let pad = F as i32 * 2;
        let x0 = (cx - pad).max(0);
        let y0 = (cy - pad).max(0);
        let w = (tw + pad * 2).min(iw - x0);
        let h = (th + pad * 2).min(ih - y0);
        if w < tw || h < th {
            return Some(coarse);
        }
        self.match_template(
            tpl,
            Some(Rect {
                x: x0,
                y: y0,
                w,
                h,
            }),
        )
    }
}

/// Convenience wrapper for the full-frame pyramid search.
pub fn match_template_full(img: &GrayImage, tpl: &GrayImage) -> Option<MatchBest> {
    Matcher::from_ref(img).match_template_full(tpl)
}

/// Convenience: does the template appear in `roi` at or above `threshold`?
pub fn appear(img: &GrayImage, tpl: &GrayImage, roi: Option<Rect>, threshold: f32) -> bool {
    match match_template(img, tpl, roi) {
        Some(b) => b.score >= threshold,
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: u32, h: u32, v: u8) -> GrayImage {
        GrayImage::new(w, h, vec![v; (w * h) as usize])
    }

    #[test]
    fn flat_template_returns_none() {
        let img = solid(32, 32, 128);
        let tpl = solid(8, 8, 128);
        assert!(match_template(&img, &tpl, None).is_none());
    }

    #[test]
    fn roi_too_small_returns_none() {
        let mut img = solid(32, 32, 10);
        img.data[0] = 200;
        let tpl = solid(10, 10, 10);
        let roi = Rect { x: 0, y: 0, w: 5, h: 5 };
        assert!(match_template(&img, &tpl, Some(roi)).is_none());
    }

    // ---- full-frame pyramid search (search window = whole frame, §22.20) ----

    fn noise_img(w: u32, h: u32, seed: u32) -> GrayImage {
        let mut s = seed.wrapping_mul(2654435761).max(1);
        let data = (0..(w * h) as usize)
            .map(|_| {
                s ^= s << 13;
                s ^= s >> 17;
                s ^= s << 5;
                (s & 0xff) as u8
            })
            .collect();
        GrayImage::new(w, h, data)
    }

    fn stamp(img: &mut GrayImage, tpl: &GrayImage, x: u32, y: u32) {
        for j in 0..tpl.height {
            for i in 0..tpl.width {
                let v = tpl.pixel(i, j);
                img.data[((y + j) * img.width + (x + i)) as usize] = v;
            }
        }
    }

    #[test]
    fn full_frame_finds_template_and_tolerates_translation() {
        let mut frame = noise_img(1280, 720, 7);
        let tpl = noise_img(48, 32, 99);
        stamp(&mut frame, &tpl, 700, 400);
        let b = match_template_full(&frame, &tpl).expect("match");
        assert!(b.score > 0.99, "score {}", b.score);
        assert_eq!((b.x, b.y), (700, 400));

        // Same template moved by (+60,+30): full-frame search must still find it.
        let mut shifted = noise_img(1280, 720, 7);
        stamp(&mut shifted, &tpl, 760, 430);
        let b2 = match_template_full(&shifted, &tpl).expect("match shifted");
        assert!(b2.score > 0.99, "shifted score {}", b2.score);
        assert_eq!((b2.x, b2.y), (760, 430));
    }

    #[test]
    fn full_frame_search_does_not_invent_a_match() {
        let frame = noise_img(1280, 720, 11);
        let tpl = noise_img(40, 30, 12);
        let b = match_template_full(&frame, &tpl).expect("best candidate");
        assert!(b.score < 0.5, "unrelated template must score low: {}", b.score);
    }

    #[test]
    fn full_frame_search_is_fast_enough_for_live_use() {
        let mut frame = noise_img(1280, 720, 21);
        let tpl = noise_img(95, 55, 22);
        stamp(&mut frame, &tpl, 330, 69);
        let t0 = std::time::Instant::now();
        let b = match_template_full(&frame, &tpl).expect("match");
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        println!(
            "full-frame 1280x720 tpl 95x55: {ms:.1} ms score={:.3}",
            b.score
        );
        assert!(b.score > 0.99);
        assert!(ms < 10_000.0, "too slow for live use: {ms:.1} ms");
    }

    #[test]
    fn masked_ncc_ignores_pixels_outside_the_polygon_mask() {
        // Template area: LEFT half is "inside the polygon" (must match), RIGHT half
        // is background that is allowed to change.
        let tpl = noise_img(20, 20, 31);
        let mut frame_a = noise_img(64, 48, 32);
        stamp(&mut frame_a, &tpl, 10, 10);
        let mut frame_b = frame_a.clone();
        // Change ONLY the right (mask=false) half of the template area.
        for j in 0..20u32 {
            for i in 10..20u32 {
                frame_b.data[((10 + j) * 64 + (10 + i)) as usize] =
                    ((i * 7 + j * 11 + 33) % 251) as u8;
            }
        }
        let mut mask = vec![false; 20 * 20];
        for j in 0..20usize {
            for i in 0..10usize {
                mask[j * 20 + i] = true;
            }
        }
        let roi = Rect {
            x: 10,
            y: 10,
            w: 20,
            h: 20,
        };
        let m_a = Matcher::from_ref(&frame_a);
        let m_b = Matcher::from_ref(&frame_b);
        let plain_ok = m_a.match_template(&tpl, Some(roi)).expect("plain").score;
        let plain_changed = m_b.match_template(&tpl, Some(roi)).expect("plain b").score;
        let masked_changed = m_b
            .match_template_masked(&tpl, &mask, Some(roi))
            .expect("masked")
            .score;
        assert!(plain_ok > 0.99, "sanity: plain match on identical frame {plain_ok}");
        assert!(
            plain_changed < 0.9,
            "unmasked score must drop when background changes: {plain_changed}"
        );
        assert!(
            masked_changed > 0.997,
            "masked score must ignore the changed background: {masked_changed}"
        );
    }

    // ---- v1.4 multi-candidate NCC (`region.asset` mode=nearest) ----

    #[test]
    fn match_template_all_finds_every_separate_stamp() {
        let mut frame = noise_img(128, 96, 7);
        let tpl = noise_img(16, 12, 99);
        stamp(&mut frame, &tpl, 10, 20);
        stamp(&mut frame, &tpl, 70, 50);
        let m = Matcher::from_ref(&frame);
        let all = m.match_template_all(&tpl, None, 0.9);
        assert_eq!(all.len(), 2, "two separate stamps => two candidates: {all:?}");
        let pos: Vec<(i32, i32)> = all.iter().map(|c| (c.x, c.y)).collect();
        assert_eq!(pos, vec![(10, 20), (70, 50)], "score-desc / y-x order");
        assert!(all.iter().all(|c| c.score > 0.99), "{all:?}");

        // Threshold filters: a high threshold that the noise background never
        // reaches still keeps both stamps; an unrelated template yields none.
        assert_eq!(m.match_template_all(&tpl, None, 1.01).len(), 0);
        let unrelated = noise_img(16, 12, 1234);
        assert!(m.match_template_all(&unrelated, None, 0.9).is_empty());
    }

    #[test]
    fn match_template_all_respects_the_search_roi() {
        let mut frame = noise_img(128, 96, 11);
        let tpl = noise_img(16, 12, 55);
        stamp(&mut frame, &tpl, 10, 20);
        stamp(&mut frame, &tpl, 90, 60);
        let m = Matcher::from_ref(&frame);
        let roi = Rect {
            x: 80,
            y: 50,
            w: 48,
            h: 46,
        };
        let all = m.match_template_all(&tpl, Some(roi), 0.9);
        assert_eq!(all.len(), 1, "{all:?}");
        assert_eq!((all[0].x, all[0].y), (90, 60));
    }

    #[test]
    fn match_template_all_suppresses_near_duplicate_candidates() {
        // Two stamps only 4px apart (NMS radius is max(2, 12/2)=6) must collapse
        // to a single physical match instead of flooding `nearest` with aliases.
        let mut frame = noise_img(128, 96, 13);
        let tpl = noise_img(12, 12, 77);
        stamp(&mut frame, &tpl, 20, 20);
        stamp(&mut frame, &tpl, 24, 20);
        let m = Matcher::from_ref(&frame);
        let all = m.match_template_all(&tpl, None, 0.8);
        assert_eq!(all.len(), 1, "close stamps are one match after NMS: {all:?}");
    }

    #[test]
    fn match_template_masked_all_is_the_masked_companion() {
        let tpl = noise_img(20, 20, 31);
        let mut frame = noise_img(96, 96, 32);
        stamp(&mut frame, &tpl, 10, 10);
        // Change the right (mask=false) half of that template area only.
        for j in 0..20u32 {
            for i in 10..20u32 {
                frame.data[((10 + j) * 96 + (10 + i)) as usize] =
                    ((i * 7 + j * 11 + 33) % 251) as u8;
            }
        }
        let mut mask = vec![false; 20 * 20];
        for j in 0..20usize {
            for i in 0..10usize {
                mask[j * 20 + i] = true;
            }
        }
        let m = Matcher::from_ref(&frame);
        assert!(
            m.match_template_all(&tpl, None, 0.95).is_empty(),
            "unmasked candidates must not reach the mask-protected threshold"
        );
        let all = m.match_template_masked_all(&tpl, &mask, None, 0.95);
        assert_eq!(all.len(), 1, "{all:?}");
        assert_eq!((all[0].x, all[0].y), (10, 10));
        assert!(all[0].score > 0.99, "{all:?}");
    }

    #[test]
    fn match_template_all_caps_the_candidate_list() {
        let mut frame = noise_img(220, 220, 5);
        let tpl = noise_img(6, 6, 6);
        // 9 x 9 = 81 stamps, 20px apart (far beyond the NMS radius of 3).
        let mut stamped = 0;
        for row in 0..9u32 {
            for col in 0..9u32 {
                stamp(&mut frame, &tpl, 5 + col * 20, 5 + row * 20);
                stamped += 1;
            }
        }
        assert_eq!(stamped, 81);
        let m = Matcher::from_ref(&frame);
        let all = m.match_template_all(&tpl, None, 0.95);
        assert_eq!(
            all.len(),
            MAX_MATCH_CANDIDATES,
            "the candidate list is capped at {MAX_MATCH_CANDIDATES}"
        );
        assert!(all.iter().all(|c| c.score > 0.99));
        // Sorted by score desc then y/x asc: all scores tie at ~1.0 here.
        assert_eq!((all[0].x, all[0].y), (5, 5));
    }
}
