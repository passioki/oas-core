//! Demonstrate ENGINE §22.10: polygon -> largest inscribed rectangle -> clicks.
//!
//! usage:
//!   cargo run -p oas-runtime --example polygon_demo -- \
//!     <x,y,x,y,...> <count> <seed> <out.jsonl>
//!
//! Prints the LAAR (largest axis-aligned inscribed rectangle) and writes one
//! CD2-ish point per click (same Humanize mixture/miss as production).

use std::io::Write;

use oas_runtime::humanize::{Humanize, Rng};
use oas_runtime::largest_inscribed_rect;
use oas_types::{Point, Rect};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    if a.len() < 4 {
        eprintln!("usage: polygon_demo <x,y,x,y,...> <count> <seed> <out.jsonl>");
        std::process::exit(2);
    }
    let nums: Vec<i32> = a[0]
        .split(',')
        .map(|s| s.trim().parse::<i32>())
        .collect::<Result<_, _>>()?;
    if nums.len() < 6 || nums.len() % 2 != 0 {
        return Err("polygon needs >= 3 [x,y] pairs".into());
    }
    let poly: Vec<Point> = nums
        .chunks_exact(2)
        .map(|c| Point { x: c[0], y: c[1] })
        .collect();
    let count: usize = a[1].parse()?;
    let seed: u64 = a[2].parse()?;
    let out = &a[3];

    let laar = largest_inscribed_rect(&poly)
        .ok_or("polygon has no inscribed axis-aligned rectangle")?;
    println!(
        "polygon_points={} laar=({},{},{}x{}) area={}",
        poly.len(),
        laar.x,
        laar.y,
        laar.w,
        laar.h,
        laar.w * laar.h
    );

    let h = Humanize::default();
    let mut rng = Rng::new(seed);
    let mut f = std::fs::File::create(out)?;
    let (mut in_laar, mut misses) = (0usize, 0usize);
    for _ in 0..count {
        let plan = h.template_click(laar, 1280, &mut rng);
        if plan.miss {
            misses += 1;
        } else if in_rect(plan.point, laar) {
            in_laar += 1;
        }
        writeln!(
            f,
            "{{\"x\":{},\"y\":{},\"miss\":{}}}",
            plan.point.x, plan.point.y, plan.miss
        )?;
    }
    println!("count={count} in_laar={in_laar} misses={misses}");
    Ok(())
}

fn in_rect(p: Point, r: Rect) -> bool {
    p.x >= r.x && p.x < r.x + r.w && p.y >= r.y && p.y < r.y + r.h
}
