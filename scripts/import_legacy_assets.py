# -*- coding: utf-8 -*-
"""Import legacy OnmyojiAutoScript task assets into oas-core's global asset store.

Source (read-only): <remote>/tasks/<Task>/res/{image,click,ocr,long_click}.json
Target: <oas-core>/assets/assets.json + assets/images/legacy/<Task>/...

Naming:
  new id = legacy_<Task>_<itemName>
  image file = images/legacy/<Task>/<imageName>

This importer is intentionally mechanical: it does NOT translate task logic.
It produces a mapping file (migration/asset-map.json) so generated flows can
refer to the same asset ids.
"""
from __future__ import annotations

import json
import re
import shutil
import sys
import ast
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REMOTE = ROOT.parent / "_verify" / "remote-oas"
ASSETS = ROOT / "assets"
IMAGES = ASSETS / "images" / "legacy"
MAP_FILE = ROOT / "migration" / "asset-map.json"


def roi(s: str):
    parts = [int(x.strip()) for x in str(s or "").split(",")]
    if len(parts) != 4:
        return None
    x, y, w, h = parts
    if w <= 0 or h <= 0 or x < 0 or y < 0:
        return None
    return [x, y, w, h]


def clean_id(s: str) -> str:
    s = re.sub(r"[^0-9A-Za-z_\u4e00-\u9fff]+", "_", s or "").strip("_")
    return s or "item"


def load_items(path: Path):
    if not path.is_file():
        return []
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except Exception:
        return []
    return data if isinstance(data, list) else []


def kw(call, name, default=None):
    for k in call.keywords:
        if k.arg == name:
            try:
                return ast.literal_eval(k.value)
            except Exception:
                return default
    return default


def import_assets_py(scope: str, assets_py: Path, by_id, mapping, copy_image):
    """Parse generated assets.py; returns (imported, skipped)."""
    imported = skipped = 0
    if not assets_py.is_file():
        return 0, 0
    try:
        tree = ast.parse(assets_py.read_text(encoding="utf-8"))
    except Exception:
        return 0, 0
    for node in ast.walk(tree):
        if not isinstance(node, ast.Assign) or len(node.targets) != 1:
            continue
        if not isinstance(node.targets[0], ast.Name):
            continue
        const = node.targets[0].id
        call = node.value
        if not isinstance(call, ast.Call):
            continue
        if isinstance(call.func, ast.Name):
            kind = call.func.id
        elif isinstance(call.func, ast.Attribute):
            kind = call.func.attr
        else:
            continue
        if kind not in ("RuleImage", "RuleClick", "RuleLongClick", "RuleOcr", "RuleSwipe"):
            continue
        new_id = f"legacy_{scope}_{const}"
        if new_id in by_id:
            continue
        try:
            rf = kw(call, "roi_front")
            rb = kw(call, "roi_back")
            rr = kw(call, "roi") or kw(call, "area")
            r = None
            for candidate in (rb, rf, rr):
                if candidate:
                    r = roi(",".join(str(x) for x in candidate))
                    if r:
                        break
            if r is None:
                skipped += 1
                continue
            if kind == "RuleImage":
                file_s = kw(call, "file")
                if not file_s:
                    skipped += 1
                    continue
                rel = str(file_s).replace("\\", "/")
                rel = rel[2:] if rel.startswith("./") else rel
                src = REMOTE / rel
                if not src.is_file():
                    skipped += 1
                    continue
                dst_rel = copy_image(src)
                by_id[new_id] = {
                    "id": new_id,
                    "kind": "template",
                    "file": dst_rel,
                    "roi": r,
                    "threshold": float(kw(call, "threshold", 0.8) or 0.8),
                    "rev": 1,
                }
            elif kind == "RuleOcr":
                by_id[new_id] = {
                    "id": new_id,
                    "kind": "ocr_lines",
                    "roi": r,
                    "rev": 1,
                }
            elif kind == "RuleSwipe":
                # one legacy swipe = two operation regions; flows use drag/ swipe
                for suffix, reg in (("FROM", rf), ("TO", rb)):
                    if not reg:
                        continue
                    r2 = roi(",".join(str(x) for x in reg))
                    if not r2:
                        continue
                    sid = f"{new_id}_{suffix}"
                    by_id[sid] = {"id": sid, "kind": "region", "roi": r2, "rev": 1}
                    mapping[f"{scope}.{const}.{suffix}"] = sid
                    imported += 1
                continue
            else:
                by_id[new_id] = {
                    "id": new_id,
                    "kind": "region",
                    "roi": r,
                    "rev": 1,
                }
            mapping[f"{scope}.{const}"] = new_id
            imported += 1
        except Exception:
            skipped += 1
    return imported, skipped


def next_image_factory():
    used = []
    for p in (ASSETS / "images").glob("a*.png"):
        m = re.fullmatch(r"a(\d+)\.png", p.name)
        if m:
            used.append(int(m.group(1)))
    n = max(used, default=0) + 1

    def copy_image(src: Path) -> str:
        nonlocal n
        rel = f"images/a{n:03d}.png"
        n += 1
        dst = ASSETS / rel
        dst.parent.mkdir(parents=True, exist_ok=True)
        head = src.read_bytes()[:8]
        if head == b"\x89PNG\r\n\x1a\n":
            shutil.copy2(src, dst)
        else:
            # Legacy `res/*.png` files are sometimes JPEG data with a .png name;
            # the oas-codec asset loader requires a real PNG signature.
            from PIL import Image
            with Image.open(src) as im:
                im.convert("RGB").save(dst, "PNG")
        return rel

    return copy_image


def main() -> int:
    if not REMOTE.is_dir():
        print(f"missing remote checkout: {REMOTE}", file=sys.stderr)
        return 1
    af_path = ASSETS / "assets.json"
    if not af_path.is_file():
        print(f"missing {af_path}; seed it from a migrated stage first", file=sys.stderr)
        return 1
    af = json.loads(af_path.read_text(encoding="utf-8"))
    by_id = {it.get("id"): it for it in af.get("items", [])}
    mapping = {}
    imported = 0
    skipped = 0
    # New global assets require sequential image names aNNN.png.
    copy_image = next_image_factory()

    for task_dir in sorted((REMOTE / "tasks").iterdir()):
        if not task_dir.is_dir():
            continue
        task = task_dir.name
        res = task_dir / "res"

        # image assets
        for item in load_items(res / "image.json"):
            name = clean_id(item.get("itemName", ""))
            image_name = item.get("imageName", "")
            src = res / image_name
            if not src.is_file():
                skipped += 1
                continue
            r = roi(item.get("roiBack")) or roi(item.get("roiFront"))
            if r is None:
                skipped += 1
                continue
            new_id = f"legacy_{task}_{name}"
            dst_rel = copy_image(src)
            by_id[new_id] = {
                "id": new_id,
                "kind": "template",
                "file": dst_rel,
                "roi": r,
                "threshold": float(item.get("threshold") or 0.8),
                "rev": 1,
            }
            mapping[f"{task}.{item.get('itemName')}"] = new_id
            imported += 1

        # click regions
        for item in load_items(res / "click.json"):
            name = clean_id(item.get("itemName", ""))
            r = roi(item.get("roiBack")) or roi(item.get("roiFront"))
            if r is None:
                skipped += 1
                continue
            new_id = f"legacy_{task}_{name}"
            by_id[new_id] = {
                "id": new_id,
                "kind": "region",
                "roi": r,
                "rev": 1,
            }
            mapping[f"{task}.{item.get('itemName')}"] = new_id
            imported += 1

        # long-click regions (runtime has no long-click op; keep region)
        for item in load_items(res / "long_click.json"):
            name = clean_id(item.get("itemName", ""))
            r = roi(item.get("roiBack")) or roi(item.get("roiFront"))
            if r is None:
                skipped += 1
                continue
            new_id = f"legacy_{task}_{name}"
            by_id[new_id] = {
                "id": new_id,
                "kind": "region",
                "roi": r,
                "rev": 1,
            }
            mapping[f"{task}.{item.get('itemName')}"] = new_id
            imported += 1

        # OCR regions (no expected text -> ocr_lines det+rec)
        for item in load_items(res / "ocr.json"):
            name = clean_id(item.get("itemName", ""))
            r = roi(item.get("roiBack")) or roi(item.get("roiFront"))
            if r is None:
                skipped += 1
                continue
            new_id = f"legacy_{task}_{name}"
            by_id[new_id] = {
                "id": new_id,
                "kind": "ocr_lines",
                "roi": r,
                "rev": 1,
            }
            mapping[f"{task}.{item.get('itemName')}"] = new_id
            imported += 1

        # Also parse generated assets.py (highlight/ images, tasks without res/).
        imp, sk = import_assets_py(task, task_dir / "assets.py", by_id, mapping, copy_image)
        imported += imp
        skipped += sk

    # Shared component assets (GeneralBattle/GeneralRoom/Invite/Buff/SwitchSoul/Buy...)
    comp_root = REMOTE / "tasks" / "Component"
    if comp_root.is_dir():
        for comp_dir in sorted(comp_root.iterdir()):
            if comp_dir.is_dir():
                imp, sk = import_assets_py(
                    f"Component_{comp_dir.name}",
                    comp_dir / "assets.py", by_id, mapping, copy_image,
                )
                imported += imp
                skipped += sk

    af["items"] = sorted(by_id.values(), key=lambda x: str(x.get("id")))
    # Current CN client main-screen template scores ~0.83; the remote .95
    # threshold is stale for this client. Keep the guard usable instead of
    # failing every migrated task before its first action.
    main_guard = by_id.get("legacy_GameUi_I_CHECK_MAIN")
    if isinstance(main_guard, dict):
        main_guard["threshold"] = 0.8
    af.setdefault("schema_version", 1)
    af.setdefault("design_resolution", [1280, 720])
    af_path.write_text(json.dumps(af, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    MAP_FILE.parent.mkdir(parents=True, exist_ok=True)
    MAP_FILE.write_text(
        json.dumps({"imported": imported, "skipped": skipped, "map": mapping},
                   ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    print(f"imported={imported} skipped={skipped} total_assets={len(af['items'])}")
    print(f"map -> {MAP_FILE}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
