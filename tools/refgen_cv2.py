# -*- coding: utf-8 -*-
"""Dev-only reference generator for oas-perceive NCC (ASCII-only).

Generates a DETERMINISTIC synthetic test set and computes reference values with
OpenCV `cv2.matchTemplate(..., cv2.TM_CCOEFF_NORMED)`, so the Rust implementation
can be verified to match OpenCV numerically.

Why synthetic: no game screenshot / account needed for P0-2. Real assets are used
in P1+ (the pipeline is identical: gray bytes in, score out).

Binary format (.gray):  magic b"OASG" + u32le width + u32le height + w*h bytes

Output:
  <out>/frame.gray, tpl_*.gray, ref.json
"""
import json
import pathlib
import sys

import cv2
import numpy as np

MAGIC = b'OASG'


def save_gray(path: pathlib.Path, arr: np.ndarray) -> None:
    assert arr.dtype == np.uint8 and arr.ndim == 2
    h, w = arr.shape
    with open(path, 'wb') as f:
        f.write(MAGIC)
        f.write(int(w).to_bytes(4, 'little'))
        f.write(int(h).to_bytes(4, 'little'))
        f.write(arr.tobytes())


def main() -> int:
    out = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else 'crates/oas-perceive/tests/data')
    out.mkdir(parents=True, exist_ok=True)

    rng = np.random.default_rng(42)
    H, W = 720, 1280
    frame = rng.integers(30, 200, size=(H, W), dtype=np.uint8).copy()

    cases = []

    # --- case 1: template pasted EXACTLY (expect ~1.0 at paste position)
    tpl1 = rng.integers(40, 220, size=(46, 56), dtype=np.uint8)
    frame[300:300 + 46, 700:700 + 56] = tpl1
    save_gray(out / 'tpl_exact.gray', tpl1)
    cases.append(dict(name='exact', tpl='tpl_exact.gray',
                      roi=[650, 250, 160, 140], paste=[700, 300]))

    # --- case 2: template pasted, then the PASTE is noised (score < 1, still high)
    tpl2 = rng.integers(40, 220, size=(40, 40), dtype=np.uint8)
    noisy = np.clip(tpl2.astype(np.int16) +
                    rng.integers(-12, 13, size=tpl2.shape), 0, 255).astype(np.uint8)
    frame[500:540, 200:240] = noisy
    save_gray(out / 'tpl_noisy.gray', noisy)
    cases.append(dict(name='noisy', tpl='tpl_noisy.gray',
                      roi=[150, 450, 160, 140], paste=[200, 500]))

    # --- case 3: template NOT present (expect clearly lower score)
    tpl3 = rng.integers(40, 220, size=(48, 36), dtype=np.uint8)
    save_gray(out / 'tpl_absent.gray', tpl3)
    cases.append(dict(name='absent', tpl='tpl_absent.gray',
                      roi=[900, 100, 200, 200], paste=None))

    # --- case 4: template is a CROP of the frame (most realistic; expect ~1.0)
    crop = frame[120:158, 1000:1044].copy()
    save_gray(out / 'tpl_crop.gray', crop)
    cases.append(dict(name='crop', tpl='tpl_crop.gray',
                      roi=[950, 80, 160, 140], paste=[1000, 120]))

    save_gray(out / 'frame.gray', frame)

    # --- reference values from OpenCV
    for c in cases:
        tpl = cv2.imread(str(out / c['tpl']), cv2.IMREAD_GRAYSCALE)
        # our .gray files are not PNGs -> read manually
        raw = (out / c['tpl']).read_bytes()
        w = int.from_bytes(raw[4:8], 'little')
        h = int.from_bytes(raw[8:12], 'little')
        tpl = np.frombuffer(raw[12:], dtype=np.uint8).reshape(h, w).copy()

        x, y, rw, rh = c['roi']
        patch = frame[y:y + rh, x:x + rw]
        res = cv2.matchTemplate(patch, tpl, cv2.TM_CCOEFF_NORMED)
        _, maxv, _, maxloc = cv2.minMaxLoc(res)
        c['score'] = float(maxv)
        c['x'] = int(x + maxloc[0])
        c['y'] = int(y + maxloc[1])
        c['tpl_w'] = int(w)
        c['tpl_h'] = int(h)

    (out / 'ref.json').write_text(json.dumps({'frame': 'frame.gray', 'cases': cases},
                                             indent=2), encoding='utf-8')
    print('wrote', len(cases), 'cases to', out)
    for c in cases:
        print(f"  {c['name']:8s} score={c['score']:.6f} at ({c['x']},{c['y']})"
              f" paste={c['paste']}")
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
