# -*- coding: utf-8 -*-
"""Click-distribution heatmap (permanent tool, design doc 21).

Reads a click log (JSONL, one object per line) and renders a PNG so you can SEE
whether clicks land on varied points inside each target (anti-ban sanity) or all
stack on one pixel (a bot tell).

Log line format (written by the engine's step log):
    {"ts_ms": 1234, "x": 1265, "y": 122, "sel": "分组2", "seed": 7}

Usage:
    python tools/click_heatmap.py <clicks.jsonl> <out.png> [--base frame.png]
"""
import json
import sys
import numpy as np
from PIL import Image, ImageDraw

W, H = 1280, 720


def load(path):
    pts = []
    with open(path, "r", encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                o = json.loads(line)
            except json.JSONDecodeError:
                continue
            if "x" in o and "y" in o:
                pts.append((int(o["x"]), int(o["y"]), o.get("sel", "")))
    return pts


def render(pts, out, base=None):
    acc = np.zeros((H, W), dtype=np.float32)
    for x, y, _ in pts:
        if 0 <= x < W and 0 <= y < H:
            acc[y, x] += 1.0
    # cheap gaussian-ish spread so single pixels stay visible when scaled down
    k = np.array([1, 4, 6, 4, 1], dtype=np.float32)
    for axis in (0, 1):
        for _ in range(2):
            acc = np.apply_along_axis(
                lambda m: np.convolve(m, k / k.sum(), mode="same"), axis, acc)
    peak = acc.max() or 1.0
    heat = (acc / peak * 255.0).clip(0, 255).astype(np.uint8)
    rgb = np.zeros((H, W, 3), dtype=np.uint8)
    rgb[:, :, 0] = heat                      # red
    rgb[:, :, 1] = (heat * 0.35).astype(np.uint8)
    if base:
        bg = Image.open(base).convert("RGB").resize((W, H))
        canvas = np.asarray(bg).astype(np.float32) * 0.35
        canvas[:, :, 0] = np.clip(canvas[:, :, 0] + heat * 0.9, 0, 255)
        canvas[:, :, 1] = np.clip(canvas[:, :, 1] + heat * 0.3, 0, 255)
        img = Image.fromarray(canvas.astype(np.uint8))
    else:
        img = Image.fromarray(rgb)
    d = ImageDraw.Draw(img)
    for x, y, sel in pts:
        d.ellipse([x - 2, y - 2, x + 2, y + 2], outline=(0, 255, 200))
    d.text((8, 8), f"clicks={len(pts)}  distinct={len({(p[0], p[1]) for p in pts})}",
           fill=(255, 255, 0))
    img.save(out)
    return len(pts), len({(p[0], p[1]) for p in pts})


def main():
    if len(sys.argv) < 3:
        print("usage: click_heatmap.py <clicks.jsonl> <out.png> [--base frame.png]")
        return 2
    base = None
    if "--base" in sys.argv:
        base = sys.argv[sys.argv.index("--base") + 1]
    pts = load(sys.argv[1])
    n, d = render(pts, sys.argv[2], base)
    print(f"clicks={n} distinct_points={d} -> {sys.argv[2]}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
