//! Humanised input (anti-ban, design doc §20). **Default ON.**
//!
//! Distribution of a template click inside the (inset) target box — user spec
//! `[10-22 19:0x]`, replacing the previous "bell only":
//!
//! * **30% bell** — Irwin–Hall(3), center biased (the old-engine shape);
//! * **20% uniform** — fully random inside the inset box (edge/coverage hygiene);
//! * **50% comet** — density grows toward the **hand-side bottom corner**:
//!   targets on the right half are denser toward the bottom-right, targets on the
//!   left half toward the bottom-left ("彗星尾巴方向与双手操作方向有联系").
//!   Biomechanically this is the thumb-reach bias: the thumb pivots from its
//!   bottom hand anchor, so points nearer that anchor are easier/more frequent.
//! * **1~3% miss** — per click a miss rate uniform in `[miss_chance_min,
//!   miss_chance_max]`; the click then lands up to `miss_px` outside the box
//!   (still a real click, audited as `miss=true`).
//!
//! Other rules kept: HB2 +/- jitter for raw coordinate clicks, HB3 random delay,
//! HB5 seeded randomness (replayable), HB6 actual point logged.

use oas_types::{Point, Rect};

#[derive(Debug, Clone, Copy)]
pub struct Humanize {
    pub enabled: bool,
    pub inset_ratio: f32,
    pub jitter_px: i32,
    pub delay_min_ms: u32,
    pub delay_max_ms: u32,
    pub seed: u64,
    // --- distribution mixture (weights are normalised at sampling time) ---
    pub bell_weight: f32,
    pub uniform_weight: f32,
    pub comet_weight: f32,
    /// Comet contrast: tail-length ratio toward the hand side (default 1.8).
    /// `λ_toward = 0.45 * comet_contrast`, `λ_away = 0.45`, cross = 0.55.
    pub comet_contrast: f32,
    // --- miss (1~3 per 100 clicks, user spec) ---
    pub miss_chance_min: f32,
    pub miss_chance_max: f32,
    /// Max distance (px) outside the target box for a miss.
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
            bell_weight: 0.30,
            uniform_weight: 0.20,
            comet_weight: 0.50,
            comet_contrast: 1.8,
            miss_chance_min: 0.01,
            miss_chance_max: 0.03,
            miss_px: 12,
        }
    }
}

/// The result of humanising one click: where it landed **and** whether it missed
/// the target (audited by the interpreter).
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

    fn inset_bounds(&self, rect: Rect) -> (i32, i32, i32, i32) {
        let (x, y, w, h) = (rect.x, rect.y, rect.w, rect.h);
        let ix = ((w as f32) * self.inset_ratio).round() as i32;
        let iy = ((h as f32) * self.inset_ratio).round() as i32;
        let x0 = x + ix.min(w / 2);
        let x1 = x + w - ix.min(w / 2) - 1;
        let y0 = y + iy.min(h / 2);
        let y1 = y + h - iy.min(h / 2) - 1;
        (x0, x1.max(x0), y0, y1.max(y0))
    }

    /// Irwin–Hall(3): the rounded mean of 3 uniform ints.
    fn bell_i32(&self, lo: i32, hi: i32, rng: &mut Rng) -> i32 {
        if hi <= lo {
            return lo;
        }
        let sum = rng.range_i32(lo, hi) as i64
            + rng.range_i32(lo, hi) as i64
            + rng.range_i32(lo, hi) as i64;
        ((sum as f64) / 3.0).round() as i32
    }

    /// HB1 (bell-only view): center-biased point inside `rect`, after inset.
    /// Kept for tests / callers that want the pure bell.
    pub fn point_in(&self, rect: Rect, rng: &mut Rng) -> Point {
        if !self.enabled {
            return Point {
                x: rect.x + rect.w / 2,
                y: rect.y + rect.h / 2,
            };
        }
        let (x0, x1, y0, y1) = self.inset_bounds(rect);
        Point {
            x: self.bell_i32(x0, x1, rng),
            y: self.bell_i32(y0, y1, rng),
        }
    }

    /// Per-click miss rate: uniform in `[miss_chance_min, miss_chance_max]`.
    fn miss_rate(&self, rng: &mut Rng) -> f32 {
        if self.miss_chance_max <= self.miss_chance_min {
            self.miss_chance_min.max(0.0)
        } else {
            let span = (self.miss_chance_max - self.miss_chance_min) as f64;
            (self.miss_chance_min as f64 + rng.unit_f64() * span) as f32
        }
    }

    fn pick_branch(&self, rng: &mut Rng) -> u8 {
        let (b, u, c) = (
            self.bell_weight.max(0.0) as f64,
            self.uniform_weight.max(0.0) as f64,
            self.comet_weight.max(0.0) as f64,
        );
        let total = b + u + c;
        if total <= 0.0 {
            return 0;
        }
        let r = rng.unit_f64() * total;
        if r < b {
            0
        } else if r < b + u {
            1
        } else {
            2
        }
    }

    /// Comet sampling: **head at the box centre, tail toward the hand side**.
    ///
    /// Density falls off **exponentially with distance from the centre** (user
    /// spec: "越靠中心点远，概率应该指数级下降"), but the decay length is longer
    /// along the hand direction (`λ_toward = 0.45*contrast`) than away from it
    /// (`λ_away = 0.45`) — the directional stretch is what produces the comet
    /// tail while probability still **monotonically decreases** with distance.
    ///
    /// Why exponential and not a steep linear ramp: a linear density hits zero at
    /// a fixed distance, which creates a visible "wall"/plateau edge inside the
    /// button (an unmistakable synthetic tell). An exponential has a smooth,
    /// unbounded tail that still decays everywhere.
    ///
    /// `screen_w` decides the hand: right half -> bottom-right, left half ->
    /// bottom-left; near the middle the x component fades and the tail points
    /// straight down (both thumbs are anchored at the bottom).
    fn comet_in(&self, rect: Rect, screen_w: u32, rng: &mut Rng) -> Point {
        let (x0, x1, y0, y1) = self.inset_bounds(rect);
        if x1 <= x0 || y1 <= y0 {
            return Point { x: x0, y: y0 };
        }
        let half_w = ((x1 - x0) as f64 / 2.0).max(0.5);
        let half_h = ((y1 - y0) as f64 / 2.0).max(0.5);
        let cx = (x0 as f64 + x1 as f64) / 2.0;
        let cy = (y0 as f64 + y1 as f64) / 2.0;
        let half_screen = (screen_w.max(1) as f64) / 2.0;
        let hand = ((cx - half_screen) / half_screen).clamp(-1.0, 1.0); // -1..1

        // Direction of the comet tail = toward the hand side and downward.
        let (dx, dy) = (hand, 1.0);
        let norm = (dx * dx + dy * dy).sqrt();
        let (ax, ay) = (dx / norm, dy / norm); // unit "along" axis

        let lambda_away = 0.45f64;
        let lambda_toward = 0.45 * self.comet_contrast.max(0.2) as f64;
        let lambda_cross = 0.55f64;

        for _ in 0..96 {
            let px = rng.range_i32(x0, x1);
            let py = rng.range_i32(y0, y1);
            let u = (px as f64 - cx) / half_w; // -1..1 in box
            let v = (py as f64 - cy) / half_h;
            let along = ax * u + ay * v;
            let cross = -ay * u + ax * v;
            let lambda_along = if along >= 0.0 { lambda_toward } else { lambda_away };
            // Anisotropic exponential distance; weight is 1 at the centre.
            let d = ((along / lambda_along).powi(2) + (cross / lambda_cross).powi(2)).sqrt();
            if rng.unit_f64() < (-d).exp() {
                return Point { x: px, y: py };
            }
        }
        // Degenerate / extremely unlucky: fall back to the bell, never a fixed point.
        self.point_in(rect, rng)
    }

    /// HB1 + HB7: click plan for a **box** target (`click` step).
    ///
    /// Branch order: miss (1~3%) → distribution mixture (30% bell / 20% uniform /
    /// 50% comet). `screen_w` is only used by the comet (hand-side) term.
    pub fn template_click(&self, rect: Rect, screen_w: u32, rng: &mut Rng) -> ClickPlan {
        if !self.enabled {
            return ClickPlan {
                point: Point {
                    x: rect.x + rect.w / 2,
                    y: rect.y + rect.h / 2,
                },
                miss: false,
            };
        }
        let rate = self.miss_rate(rng);
        if rate > 0.0 && rng.unit_f64() < rate as f64 {
            return ClickPlan {
                point: self.miss_point(rect, rng),
                miss: true,
            };
        }
        let (x0, x1, y0, y1) = self.inset_bounds(rect);
        let point = match self.pick_branch(rng) {
            0 => Point {
                x: self.bell_i32(x0, x1, rng),
                y: self.bell_i32(y0, y1, rng),
            },
            1 => Point {
                x: rng.range_i32(x0, x1),
                y: rng.range_i32(y0, y1),
            },
            _ => self.comet_in(rect, screen_w, rng),
        };
        ClickPlan { point, miss: false }
    }

    /// HB2 + HB7: click plan for a **coordinate** target (`click_point` step).
    pub fn point_click(&self, p: Point, rng: &mut Rng) -> ClickPlan {
        if !self.enabled {
            return ClickPlan { point: p, miss: false };
        }
        let rate = self.miss_rate(rng);
        if rate > 0.0 && rng.unit_f64() < rate as f64 {
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

    /// Miss sampling: inside the ring `rect` expanded by `miss_px`, outside `rect`.
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

    fn no_miss() -> Humanize {
        Humanize {
            miss_chance_min: 0.0,
            miss_chance_max: 0.0,
            ..Humanize::default()
        }
    }

    fn bell_only() -> Humanize {
        Humanize {
            bell_weight: 1.0,
            uniform_weight: 0.0,
            comet_weight: 0.0,
            ..no_miss()
        }
    }

    fn uniform_only() -> Humanize {
        Humanize {
            bell_weight: 0.0,
            uniform_weight: 1.0,
            comet_weight: 0.0,
            ..no_miss()
        }
    }

    fn comet_only() -> Humanize {
        Humanize {
            bell_weight: 0.0,
            uniform_weight: 0.0,
            comet_weight: 1.0,
            ..no_miss()
        }
    }

    /// Bell branch: most points near the centre (old-engine shape).
    #[test]
    fn mixture_bell_branch_is_center_biased() {
        let h = bell_only();
        let mut rng = Rng::new(7);
        let (x0, x1, y0, y1) = h.inset_bounds(r());
        let (cx, cy) = ((x0 + x1) as f64 / 2.0, (y0 + y1) as f64 / 2.0);
        let hw = (x1 - x0 + 1) as f64 / 4.0;
        let hh = (y1 - y0 + 1) as f64 / 4.0;
        let mut central = 0;
        for _ in 0..10_000 {
            let p = h.template_click(r(), 1280, &mut rng);
            assert!(!p.miss);
            let q = p.point;
            assert!(q.x >= x0 && q.x <= x1 && q.y >= y0 && q.y <= y1, "{q:?}");
            if (q.x as f64 - cx).abs() <= hw && (q.y as f64 - cy).abs() <= hh {
                central += 1;
            }
        }
        assert!(central as f64 / 10_000.0 > 0.60, "bell central {central}/10000");
    }

    /// Uniform branch: ~25% in the central quarter (the "filling" shape; only 20% of clicks).
    #[test]
    fn mixture_uniform_branch_fills_the_box_evenly() {
        let h = uniform_only();
        let mut rng = Rng::new(7);
        let (x0, x1, y0, y1) = h.inset_bounds(r());
        let (cx, cy) = ((x0 + x1) as f64 / 2.0, (y0 + y1) as f64 / 2.0);
        let hw = (x1 - x0 + 1) as f64 / 4.0;
        let hh = (y1 - y0 + 1) as f64 / 4.0;
        let mut central = 0;
        for _ in 0..10_000 {
            let q = h.template_click(r(), 1280, &mut rng).point;
            if (q.x as f64 - cx).abs() <= hw && (q.y as f64 - cy).abs() <= hh {
                central += 1;
            }
        }
        let share = central as f64 / 10_000.0;
        assert!((0.18..=0.32).contains(&share), "uniform central {share}");
    }

    /// Comet branch: right-side target => denser bottom-right than top-left.
    #[test]
    fn comet_right_side_is_denser_toward_bottom_right() {
        let h = comet_only();
        let mut rng = Rng::new(5);
        let rect = Rect {
            x: 900,
            y: 300,
            w: 120,
            h: 60,
        };
        let (x0, x1, y0, y1) = h.inset_bounds(rect);
        let (mx, my) = ((x0 + x1) / 2, (y0 + y1) / 2);
        let mut br = 0;
        let mut tl = 0;
        for _ in 0..20_000 {
            let q = h.template_click(rect, 1280, &mut rng).point;
            if q.x >= mx && q.y >= my {
                br += 1;
            }
            if q.x <= mx && q.y <= my {
                tl += 1;
            }
        }
        assert!(
            br as f64 > tl as f64 * 1.3,
            "right-side comet should favour bottom-right: br={br} tl={tl}"
        );
    }

    /// Comet branch: left-side target => denser bottom-left than top-right.
    #[test]
    fn comet_left_side_is_denser_toward_bottom_left() {
        let h = comet_only();
        let mut rng = Rng::new(5);
        let rect = Rect {
            x: 100,
            y: 300,
            w: 120,
            h: 60,
        };
        let (x0, x1, y0, y1) = h.inset_bounds(rect);
        let (mx, my) = ((x0 + x1) / 2, (y0 + y1) / 2);
        let mut bl = 0;
        let mut tr = 0;
        for _ in 0..20_000 {
            let q = h.template_click(rect, 1280, &mut rng).point;
            if q.x <= mx && q.y >= my {
                bl += 1;
            }
            if q.x >= mx && q.y <= my {
                tr += 1;
            }
        }
        assert!(
            bl as f64 > tr as f64 * 1.3,
            "left-side comet should favour bottom-left: bl={bl} tr={tr}"
        );
    }

    /// Comet branch: probability **decreases with distance from the box centre**
    /// (exponential falloff) — inner region must out-count the far corners even
    /// though it is a much smaller area.
    #[test]
    fn comet_density_falls_with_distance_from_centre() {
        let h = comet_only();
        let mut rng = Rng::new(17);
        let rect = Rect {
            x: 900,
            y: 300,
            w: 120,
            h: 60,
        };
        let (x0, x1, y0, y1) = h.inset_bounds(rect);
        let (cx, cy) = ((x0 + x1) as f64 / 2.0, (y0 + y1) as f64 / 2.0);
        let hw = ((x1 - x0) as f64 / 2.0).max(1.0);
        let hh = ((y1 - y0) as f64 / 2.0).max(1.0);
        let mut inner = 0usize;
        let mut corners = 0usize;
        for _ in 0..20_000 {
            let p = h.template_click(rect, 1280, &mut rng).point;
            let u = (p.x as f64 - cx) / hw;
            let v = (p.y as f64 - cy) / hh;
            if (u * u + v * v).sqrt() <= 0.5 {
                inner += 1;
            }
            if u.abs() > 0.7 && v.abs() > 0.7 {
                corners += 1;
            }
        }
        assert!(
            inner > corners,
            "exponential falloff expected: inner={inner} corners={corners}"
        );
    }


    #[test]
    fn miss_rate_is_1_to_3_percent() {
        let h = Humanize::default();
        let mut rng = Rng::new(13);
        let n = 20_000;
        let misses = (0..n).filter(|_| h.template_click(r(), 1280, &mut rng).miss).count();
        let rate = misses as f64 / n as f64;
        assert!((0.014..=0.026).contains(&rate), "miss rate {rate}");
    }

    #[test]
    fn matches_land_in_the_inset_box_unless_missed() {
        let h = Humanize::default();
        let mut rng = Rng::new(21);
        let (x0, x1, y0, y1) = h.inset_bounds(r());
        for _ in 0..10_000 {
            let plan = h.template_click(r(), 1280, &mut rng);
            if plan.miss {
                continue;
            }
            let p = plan.point;
            assert!(p.x >= x0 && p.x <= x1 && p.y >= y0 && p.y <= y1, "{p:?}");
        }
    }

    #[test]
    fn miss_points_are_outside_the_box() {
        let h = Humanize {
            miss_chance_min: 1.0,
            miss_chance_max: 1.0,
            ..Humanize::default()
        };
        let mut rng = Rng::new(11);
        let rect = r();
        let pad = h.miss_px;
        for _ in 0..1000 {
            let plan = h.template_click(rect, 1280, &mut rng);
            assert!(plan.miss);
            let p = plan.point;
            let inside = p.x >= rect.x
                && p.x < rect.x + rect.w
                && p.y >= rect.y
                && p.y < rect.y + rect.h;
            assert!(!inside, "{p:?}");
            assert!(p.x >= rect.x - pad && p.x <= rect.x + rect.w + pad);
            assert!(p.y >= rect.y - pad && p.y <= rect.y + rect.h + pad);
        }
    }

    #[test]
    fn hb5_same_seed_same_sequence() {
        let h = Humanize::default();
        let mut a = Rng::new(99);
        let mut b = Rng::new(99);
        let sa: Vec<_> = (0..64).map(|_| h.template_click(r(), 1280, &mut a)).collect();
        let sb: Vec<_> = (0..64).map(|_| h.template_click(r(), 1280, &mut b)).collect();
        assert_eq!(sa, sb, "same seed must replay exactly (including misses)");
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
        let plan = h.template_click(r(), 1280, &mut rng);
        assert_eq!(plan.point, Point { x: 140, y: 220 });
        assert!(!plan.miss);
    }

    #[test]
    fn hb2_jitter_and_point_miss_are_bounded() {
        let h = Humanize::default();
        let mut rng = Rng::new(3);
        let mut seen = std::collections::HashSet::new();
        for _ in 0..1000 {
            let plan = h.point_click(Point { x: 500, y: 500 }, &mut rng);
            assert!(plan.point.x >= 0 && plan.point.y >= 0);
            seen.insert((plan.point.x, plan.point.y));
        }
        assert!(seen.len() > 10, "jitter must vary");
        let no = no_miss();
        let mut rng = Rng::new(3);
        for _ in 0..1000 {
            let p = no.point_click(Point { x: 500, y: 500 }, &mut rng).point;
            assert!((p.x - 500).abs() <= 6 && (p.y - 500).abs() <= 6);
        }
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
