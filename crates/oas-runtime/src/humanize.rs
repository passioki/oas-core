//! Humanised input (anti-ban, design doc §20). **Default ON.**
//!
//! Rules:
//! * HB1 clicks are never a fixed point: pick uniformly inside the target box
//!   inset by `inset_ratio` (the outer ring is usually decoration, not clickable).
//! * HB2 raw coordinate clicks get a +/- `jitter_px` offset.
//! * HB3 a random delay between `delay_ms[0]` and `delay_ms[1]`.
//! * HB5 (hard constraint) randomness is **seeded**, so a recording can be
//!   replayed exactly. `seed = 0` means "pick one at run start and persist it".
//! * HB6 the caller logs the actual point that was clicked.

use oas_types::{Point, Rect};

#[derive(Debug, Clone, Copy)]
pub struct Humanize {
    pub enabled: bool,
    pub inset_ratio: f32,
    pub jitter_px: i32,
    pub delay_min_ms: u32,
    pub delay_max_ms: u32,
    pub seed: u64,
}

impl Default for Humanize {
    fn default() -> Self {
        // HB4: default ON.
        Self {
            enabled: true,
            inset_ratio: 0.15,
            jitter_px: 6,
            delay_min_ms: 30,
            delay_max_ms: 120,
            seed: 0,
        }
    }
}

/// xorshift64* - tiny, deterministic, good enough for jitter.
#[derive(Debug, Clone, Copy)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        // never allow state 0
        Self(if seed == 0 { 0x9E37_79B9_7F4A_7C15 } else { seed })
    }

    pub fn peek(&self) -> u64 {
        self.0
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform in [lo, hi] (inclusive), `lo <= hi`.
    pub fn range_i32(&mut self, lo: i32, hi: i32) -> i32 {
        if hi <= lo {
            return lo;
        }
        let span = (hi - lo) as u64 + 1;
        lo + (self.next_u64() % span) as i32
    }
}

impl Humanize {
    /// Resolve `seed == 0` into a concrete seed (call once per run, persist it).
    pub fn resolved_seed(&self, now_ms: u128) -> u64 {
        if self.seed != 0 {
            self.seed
        } else {
            (now_ms as u64) ^ 0xA5A5_5A5A_1234_5678
        }
    }

    /// HB1: a random point inside `rect`, after inset. Deterministic per rng state.
    pub fn point_in(&self, rect: Rect, rng: &mut Rng) -> Point {
        let (x, y, w, h) = (rect.x, rect.y, rect.w, rect.h);
        if !self.enabled {
            return Point {
                x: x + w / 2,
                y: y + h / 2,
            };
        }
        // inset both axes; keep at least 1px of range
        let inset_x = ((w as f32) * self.inset_ratio).round() as i32;
        let inset_y = ((h as f32) * self.inset_ratio).round() as i32;
        let x0 = x + inset_x.min(w / 2);
        let x1 = x + w - inset_x.min(w / 2) - 1;
        let y0 = y + inset_y.min(h / 2);
        let y1 = y + h - inset_y.min(h / 2) - 1;
        Point {
            x: rng.range_i32(x0, x1.max(x0)),
            y: rng.range_i32(y0, y1.max(y0)),
        }
    }

    /// HB2: raw coordinate click with jitter.
    pub fn jitter_point(&self, p: Point, rng: &mut Rng) -> Point {
        if !self.enabled || self.jitter_px <= 0 {
            return p;
        }
        Point {
            x: p.x + rng.range_i32(-self.jitter_px, self.jitter_px),
            y: p.y + rng.range_i32(-self.jitter_px, self.jitter_px),
        }
    }

    /// HB3: delay before an action, in milliseconds.
    pub fn delay_ms(&self, rng: &mut Rng) -> u32 {
        if !self.enabled || self.delay_max_ms <= self.delay_min_ms {
            return 0;
        }
        rng.range_i32(self.delay_min_ms as i32, self.delay_max_ms as i32) as u32
    }

    /// Clamp a humanised point back into the frame; returns None if impossible.
    pub fn clamp(p: Point, width: u32, height: u32) -> Option<Point> {
        if p.x < 0 || p.y < 0 || p.x >= width as i32 || p.y >= height as i32 {
            None
        } else {
            Some(p)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r() -> Rect {
        Rect {
            x: 100,
            y: 200,
            w: 80,
            h: 40,
        }
    }

    #[test]
    fn hb1_points_never_a_fixed_point_and_stay_inside_inset() {
        let h = Humanize::default();
        let mut rng = Rng::new(42);
        let rect = r();
        let inset_x = (80.0f32 * 0.15).round() as i32; // 12
        let inset_y = (40.0f32 * 0.15).round() as i32; // 6
        let mut seen = std::collections::HashSet::new();
        for _ in 0..10_000 {
            let p = h.point_in(rect, &mut rng);
            assert!(p.x >= rect.x + inset_x && p.x <= rect.x + rect.w - inset_x - 1, "{p:?}");
            assert!(p.y >= rect.y + inset_y && p.y <= rect.y + rect.h - inset_y - 1, "{p:?}");
            seen.insert((p.x, p.y));
        }
        let area = (rect.w - 2 * inset_x) * (rect.h - 2 * inset_y);
        assert!(
            seen.len() > area as usize / 2,
            "must not collapse to a fixed point: {} distinct",
            seen.len()
        );
    }

    #[test]
    fn hb5_same_seed_same_sequence_and_different_seed_differs() {
        let h = Humanize::default();
        let mut a = Rng::new(7);
        let mut b = Rng::new(7);
        let mut c = Rng::new(8);
        let sa: Vec<_> = (0..32).map(|_| h.point_in(r(), &mut a)).collect();
        let sb: Vec<_> = (0..32).map(|_| h.point_in(r(), &mut b)).collect();
        let sc: Vec<_> = (0..32).map(|_| h.point_in(r(), &mut c)).collect();
        assert_eq!(sa, sb, "same seed must replay exactly");
        assert_ne!(sa, sc, "different seed must differ");
    }

    #[test]
    fn disabled_returns_center_and_zero_delay() {
        let h = Humanize {
            enabled: false,
            ..Humanize::default()
        };
        let mut rng = Rng::new(1);
        assert_eq!(h.point_in(r(), &mut rng), Point { x: 140, y: 220 });
        assert_eq!(h.delay_ms(&mut rng), 0);
        assert_eq!(h.jitter_point(Point { x: 5, y: 5 }, &mut rng), Point { x: 5, y: 5 });
    }

    #[test]
    fn hb2_jitter_is_bounded_and_varies() {
        let h = Humanize::default();
        let mut rng = Rng::new(3);
        let mut seen = std::collections::HashSet::new();
        for _ in 0..1000 {
            let p = h.jitter_point(Point { x: 500, y: 500 }, &mut rng);
            assert!((p.x - 500).abs() <= 6 && (p.y - 500).abs() <= 6);
            seen.insert((p.x, p.y));
        }
        assert!(seen.len() > 10, "jitter must vary");
    }

    #[test]
    fn hb3_delay_within_range() {
        let h = Humanize::default();
        let mut rng = Rng::new(9);
        for _ in 0..1000 {
            let d = h.delay_ms(&mut rng);
            assert!((30..=120).contains(&d), "delay {d} out of range");
        }
    }

    #[test]
    fn clamp_rejects_offscreen() {
        assert!(Humanize::clamp(Point { x: -1, y: 5 }, 100, 100).is_none());
        assert!(Humanize::clamp(Point { x: 100, y: 5 }, 100, 100).is_none());
        assert_eq!(Humanize::clamp(Point { x: 99, y: 99 }, 100, 100), Some(Point { x: 99, y: 99 }));
    }
}
