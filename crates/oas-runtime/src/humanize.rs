//! Humanised input (anti-ban, design doc §20). **Default ON.**
//!
//! Rules:
//! * HB1 clicks are never a fixed point: sample a **center-biased** point inside
//!   the target box. This is the same shape the old engine used
//!   (`module/base/utils/utils.py::random_normal_distribution_int` = mean of 3
//!   uniform samples, i.e. Irwin–Hall(3)) — a uniform sample is wrong because
//!   enough clicks literally fill (and "raster scan") the whole rectangle.
//! * HB2 raw coordinate clicks get a +/- `jitter_px` offset.
//! * HB3 a random delay between `delay_ms[0]` and `delay_ms[1]`.
//! * HB5 (hard constraint) randomness is **seeded**, so a recording can be
//!   replayed exactly. `seed = 0` means "pick one at run start and persist it".
//! * HB6 the caller logs the actual point that was clicked.
//! * HB7 **[10-22] misses**: with probability `miss_chance` the click lands
//!   **outside the target box** by at most `miss_px` (a human "fat finger").
//!   Perfect hit-rates are themselves a bot tell; the caller logs `miss=true`.
//!
//! HB7 is what the old repo did with `RuleClickExclude` (`module/atom/click.py`):
//! random clicks that deliberately avoid the known click regions. We keep the
//! "miss" attached to the target click instead of adding extra full-screen taps,
//! so the task semantics stay explainable.

use oas_types::{Point, Rect};

#[derive(Debug, Clone, Copy)]
pub struct Humanize {
    pub enabled: bool,
    pub inset_ratio: f32,
    pub jitter_px: i32,
    pub delay_min_ms: u32,
    pub delay_max_ms: u32,
    pub seed: u64,
    /// HB7: probability that a click misses the target box (0.0 = never).
    pub miss_chance: f32,
    /// HB7: maximum distance (px) outside the target box for a miss.
    pub miss_px: i32,
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
            // ~1 in 50 clicks is a visible miss; small enough to keep tasks
            // usable, large enough to break a perfect hit-rate fingerprint.
            miss_chance: 0.02,
            miss_px: 12,
        }
    }
}

/// The result of humanising one click: where it landed **and** whether it missed
/// the target (HB7). The interpreter logs and audits both.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClickPlan {
    pub point: Point,
    pub miss: bool,
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

    /// Uniform in [0, 1).
    pub fn unit_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
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

    /// Irwin–Hall(3): the rounded mean of 3 uniform ints.
    ///
    /// Same distribution family as the old engine's
    /// `random_normal_distribution_int()`, i.e. a bell centered on the interval:
    /// the middle is much more likely than the edges, but every px is possible.
    fn bell_i32(&self, lo: i32, hi: i32, rng: &mut Rng) -> i32 {
        if hi <= lo {
            return lo;
        }
        let sum = rng.range_i32(lo, hi) as i64
            + rng.range_i32(lo, hi) as i64
            + rng.range_i32(lo, hi) as i64;
        ((sum as f64) / 3.0).round() as i32
    }

    /// HB1: a **center-biased** random point inside `rect`, after inset.
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
            x: self.bell_i32(x0, x1.max(x0), rng),
            y: self.bell_i32(y0, y1.max(y0), rng),
        }
    }

    /// HB7 + HB1: click plan for a **box** target (`click` step).
    ///
    /// `miss = true` means the point is outside `rect` (by at most `miss_px`);
    /// the interpreter still performs the click (that is what a human does) but
    /// records the miss.
    pub fn plan_click(&self, rect: Rect, rng: &mut Rng) -> ClickPlan {
        if !self.enabled || self.miss_chance <= 0.0 {
            return ClickPlan {
                point: self.point_in(rect, rng),
                miss: false,
            };
        }
        if rng.unit_f64() < self.miss_chance as f64 {
            ClickPlan {
                point: self.miss_point(rect, rng),
                miss: true,
            }
        } else {
            ClickPlan {
                point: self.point_in(rect, rng),
                miss: false,
            }
        }
    }

    /// HB7 + HB2: click plan for a **coordinate** target (`click_point` step).
    pub fn plan_point_click(&self, p: Point, rng: &mut Rng) -> ClickPlan {
        if !self.enabled {
            return ClickPlan {
                point: p,
                miss: false,
            };
        }
        if self.miss_chance > 0.0 && rng.unit_f64() < self.miss_chance as f64 {
            let dist = rng.range_i32(self.miss_px.max(8), (self.miss_px * 2).max(12));
            let angle = rng.unit_f64() * std::f64::consts::TAU;
            return ClickPlan {
                point: Point {
                    x: p.x + (angle.cos() * dist as f64).round() as i32,
                    y: p.y + (angle.sin() * dist as f64).round() as i32,
                },
                miss: true,
            };
        }
        ClickPlan {
            point: self.jitter_point(p, rng),
            miss: false,
        }
    }

    /// HB7: sample inside the ring `rect` expanded by `miss_px` but **outside**
    /// `rect` itself. Bounded by the caller (the interpreter clamps to the frame).
    fn miss_point(&self, rect: Rect, rng: &mut Rng) -> Point {
        let pad = self.miss_px.max(1);
        let inside = |p: Point| {
            p.x >= rect.x
                && p.x < rect.x + rect.w
                && p.y >= rect.y
                && p.y < rect.y + rect.h
        };
        for _ in 0..16 {
            let p = Point {
                x: rng.range_i32(rect.x - pad, rect.x + rect.w + pad),
                y: rng.range_i32(rect.y - pad, rect.y + rect.h + pad),
            };
            if !inside(p) {
                return p;
            }
        }
        // Extremely small rect: step just outside the right edge instead.
        Point {
            x: rect.x + rect.w + 1,
            y: rect.y + rect.h / 2,
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

    /// HB1 regression: the old uniform sample filled the whole box like a raster
    /// scan; the bell must put most clicks near the centre (old-engine shape).
    #[test]
    fn hb1_is_center_biased_like_the_old_engine() {
        let h = Humanize::default();
        let mut rng = Rng::new(7);
        let rect = r();
        let inset_x = (80.0f32 * 0.15).round() as i32;
        let inset_y = (40.0f32 * 0.15).round() as i32;
        let (x0, x1) = (rect.x + inset_x, rect.x + rect.w - inset_x - 1);
        let (y0, y1) = (rect.y + inset_y, rect.y + rect.h - inset_y - 1);
        let (cx, cy) = ((x0 + x1) as f64 / 2.0, (y0 + y1) as f64 / 2.0);
        let half_w = (x1 - x0 + 1) as f64 / 4.0;
        let half_h = (y1 - y0 + 1) as f64 / 4.0;
        let mut central = 0usize;
        let mut xs = Vec::with_capacity(10_000);
        let mut ys = Vec::with_capacity(10_000);
        for _ in 0..10_000 {
            let p = h.point_in(rect, &mut rng);
            xs.push(p.x as f64);
            ys.push(p.y as f64);
            if (p.x as f64 - cx).abs() <= half_w && (p.y as f64 - cy).abs() <= half_h {
                central += 1;
            }
        }
        // Uniform sampling of the same box would put ~25% in the central quarter.
        assert!(
            central as f64 / 10_000.0 > 0.60,
            "expected a bell (central quarter > 60%), got {central}/10000"
        );
        let std = |v: &[f64]| {
            let m = v.iter().sum::<f64>() / v.len() as f64;
            (v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / v.len() as f64).sqrt()
        };
        // Old engine: sigma ~ interval/6; uniform would be ~/3.46.
        let w = (x1 - x0 + 1) as f64;
        assert!(
            std(&xs) < w / 4.5,
            "x std {} is too close to uniform ({})",
            std(&xs),
            w / 12.0_f64.sqrt()
        );
    }

    #[test]
    fn hb7_misses_are_outside_the_box_and_flagged() {
        let h = Humanize {
            miss_chance: 1.0,
            ..Humanize::default()
        };
        let mut rng = Rng::new(11);
        let rect = r();
        let pad = h.miss_px;
        for _ in 0..1000 {
            let plan = h.plan_click(rect, &mut rng);
            assert!(plan.miss, "miss_chance=1.0 must always miss");
            let p = plan.point;
            let inside = p.x >= rect.x
                && p.x < rect.x + rect.w
                && p.y >= rect.y
                && p.y < rect.y + rect.h;
            assert!(!inside, "miss point {p:?} is inside the target box");
            assert!(
                p.x >= rect.x - pad && p.x <= rect.x + rect.w + pad,
                "{p:?} out of the miss ring"
            );
            assert!(
                p.y >= rect.y - pad && p.y <= rect.y + rect.h + pad,
                "{p:?} out of the miss ring"
            );
        }
    }

    #[test]
    fn hb7_miss_rate_is_roughly_the_configured_chance() {
        let h = Humanize::default();
        let mut rng = Rng::new(13);
        let rect = r();
        let n = 10_000;
        let misses = (0..n)
            .filter(|_| h.plan_click(rect, &mut rng).miss)
            .count();
        let rate = misses as f64 / n as f64;
        assert!(
            (0.012..=0.028).contains(&rate),
            "miss rate {rate} not near the configured 0.02"
        );

        let zero = Humanize {
            miss_chance: 0.0,
            ..Humanize::default()
        };
        let mut rng = Rng::new(13);
        assert!((0..n).all(|_| !zero.plan_click(rect, &mut rng).miss));
    }

    #[test]
    fn hb7_same_seed_replays_the_same_miss_sequence() {
        let h = Humanize::default();
        let mut a = Rng::new(99);
        let mut b = Rng::new(99);
        let sa: Vec<_> = (0..64).map(|_| h.plan_click(r(), &mut a)).collect();
        let sb: Vec<_> = (0..64).map(|_| h.plan_click(r(), &mut b)).collect();
        assert_eq!(sa, sb, "same seed must replay the same misses");
        assert!(
            sa.iter().any(|p| p.miss),
            "64 clicks should contain at least one miss with 2%"
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
        let plan = h.plan_click(r(), &mut rng);
        assert_eq!(plan.point, Point { x: 140, y: 220 });
        assert!(!plan.miss);
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
