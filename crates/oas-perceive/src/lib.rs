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
}
