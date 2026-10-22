//! Simulate N humanised clicks and dump them as CD2 JSONL (ENGINE-DESIGN §20).
//!
//! usage:
//!   cargo run -p oas-runtime --example click_sim -- \
//!     <rect|point> <count> <seed> <x> <y> <w> <h> <out.jsonl> [bell] [uniform] [comet]
//!
//! * `rect`  = template click: 30% bell / 20% uniform / 50% comet (hand-side
//!             corner), inset 15%, plus 1~3% misses just outside the box;
//! * `point` = click_point: +/- jitter, plus 1~3% misses;
//! * optional trailing weights override the mixture (e.g. `1 0 0` = bell only)
//!   so the three distributions can be compared side by side;
//! * the run prints the distribution stats (mean/std/inside%/misses).
//!
//! Every row is CD2-compatible (`ts_ms,x,y,target_x,target_y,sel,miss,seed`), so
//! the output can be dropped into `runs/` and viewed in the editor's 点击分布 panel.

use std::collections::HashSet;
use std::io::Write;

use oas_runtime::humanize::{Humanize, Rng};
use oas_types::{Point, Rect};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    if a.len() < 8 {
        eprintln!("usage: click_sim <rect|point> <count> <seed> <x> <y> <w> <h> <out.jsonl>");
        std::process::exit(2);
    }
    let mode = a[0].as_str();
    let n: usize = a[1].parse()?;
    let seed: u64 = a[2].parse()?;
    let rect = Rect {
        x: a[3].parse()?,
        y: a[4].parse()?,
        w: a[5].parse()?,
        h: a[6].parse()?,
    };
    let out = &a[7];
    if n == 0 {
        return Err("<count> must be > 0".into());
    }

    let h = {
        let mut h = Humanize::default();
        if a.len() >= 9 {
            h.bell_weight = a[8].parse()?;
        }
        if a.len() >= 10 {
            h.uniform_weight = a[9].parse()?;
        }
        if a.len() >= 11 {
            h.comet_weight = a[10].parse()?;
        }
        // Optional miss overrides (defaults = 1~3%): `... 0.3 0.2 0.5 0 0`
        // disables misses so a pure distribution shape can be rendered.
        if a.len() >= 12 {
            h.miss_chance_min = a[11].parse()?;
        }
        if a.len() >= 13 {
            h.miss_chance_max = a[12].parse()?;
        }
        h
    };
    let mut rng = Rng::new(seed);
    let mut file = std::fs::File::create(out)?;
    let mut xs = Vec::with_capacity(n);
    let mut ys = Vec::with_capacity(n);
    let mut delays = Vec::with_capacity(n);
    let mut distinct: HashSet<(i32, i32)> = HashSet::new();
    let mut misses = 0usize;
    let mut ts: u128 = 1_792_605_800_000;
    let (cx, cy) = (rect.x + rect.w / 2, rect.y + rect.h / 2);
    for _ in 0..n {
        let plan = match mode {
            "rect" => h.template_click(rect, 1280, &mut rng),
            "point" => h.point_click(Point { x: rect.x, y: rect.y }, &mut rng),
            other => return Err(format!("unknown mode `{other}` (rect|point)").into()),
        };
        let p = plan.point;
        let d = h.delay_ms(&mut rng);
        if plan.miss {
            misses += 1;
        }
        xs.push(p.x as f64);
        ys.push(p.y as f64);
        delays.push(d);
        distinct.insert((p.x, p.y));
        ts += d as u128 + 50;
        writeln!(
            file,
            r#"{{"ts_ms":{ts},"x":{},"y":{},"target_x":{cx},"target_y":{cy},"sel":null,"miss":{},"seed":{seed}}}"#,
            p.x, p.y, plan.miss
        )?;
    }

    // HB1 inset bounds (same formula as Humanize::point_in).
    let ix = (rect.w as f32 * h.inset_ratio).round() as i32;
    let iy = (rect.h as f32 * h.inset_ratio).round() as i32;
    let (x0, x1) = (rect.x + ix, rect.x + rect.w - ix - 1);
    let (y0, y1) = (rect.y + iy, rect.y + rect.h - iy - 1);
    let inside_rect = xs
        .iter()
        .zip(&ys)
        .filter(|(x, y)| {
            **x >= rect.x as f64
                && **y >= rect.y as f64
                && **x < (rect.x + rect.w) as f64
                && **y < (rect.y + rect.h) as f64
        })
        .count();
    let inside_inset = xs
        .iter()
        .zip(&ys)
        .filter(|(x, y)| {
            **x >= x0 as f64 && **y >= y0 as f64 && **x <= x1 as f64 && **y <= y1 as f64
        })
        .count();
    let (mx, sx) = mean_std(&xs);
    let (my, sy) = mean_std(&ys);
    let (md, sd) = mean_std(&delays.iter().map(|d| *d as f64).collect::<Vec<_>>());
    let mut dist: Vec<f64> = xs
        .iter()
        .zip(&ys)
        .map(|(x, y)| ((x - cx as f64).powi(2) + (y - cy as f64).powi(2)).sqrt())
        .collect();
    dist.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p95 = dist[(dist.len() as f64 * 0.95) as usize];

    println!("mode={mode} count={n} seed={seed}");
    println!(
        "target=({},{},{},{}) centre=({cx},{cy}) inset_rect=[{x0},{y0},{}x{}]",
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        x1 - x0 + 1,
        y1 - y0 + 1
    );
    println!(
        "x: mean={mx:.2} std={sx:.2} min={} max={}",
        xs.iter().cloned().fold(f64::INFINITY, f64::min) as i32,
        xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max) as i32
    );
    println!(
        "y: mean={my:.2} std={sy:.2} min={} max={}",
        ys.iter().cloned().fold(f64::INFINITY, f64::min) as i32,
        ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max) as i32
    );
    println!(
        "inside_target={}/{n} ({:.2}%) inside_inset={}/{n} ({:.2}%)",
        inside_rect,
        100.0 * inside_rect as f64 / n as f64,
        inside_inset,
        100.0 * inside_inset as f64 / n as f64
    );
    println!(
        "misses={misses} ({:.2}% of clicks)",
        misses as f64 / n as f64 * 100.0
    );
    println!(
        "mix: bell={:.2} uniform={:.2} comet={:.2}",
        h.bell_weight, h.uniform_weight, h.comet_weight
    );
    println!(
        "distinct_points={} ({:.1}% of {}", distinct.len(),
        100.0 * distinct.len() as f64 / n as f64, n
    );
    println!("distance_from_centre: mean={:.2} p95={p95:.2}", dist.iter().sum::<f64>() / n as f64);
    println!("delay_ms: mean={md:.1} std={sd:.1} min={} max={}",
        delays.iter().min().unwrap(), delays.iter().max().unwrap());
    println!("out={out}");
    Ok(())
}

fn mean_std(v: &[f64]) -> (f64, f64) {
    let m = v.iter().sum::<f64>() / v.len() as f64;
    let var = v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / v.len() as f64;
    (m, var.sqrt())
}
