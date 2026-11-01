#!/usr/bin/env python3
"""Recover historical task assets (all task assets.json snapshots in git) into
the current unified global store `assets/`, without touching `tasks/`.

Safety:
  * backs up the current store to assets/.backup/recover-<timestamp>/
  * never overwrites an existing global asset id
  * copies template image blobs from the exact historical commit
  * converts legacy action_roi -> action_shape/action_source
  * writes assets/history-recovery-report.json
"""
import json
import shutil
import subprocess
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / "assets"


def git(*args: str) -> bytes:
    return subprocess.check_output(
        ["git", "-c", "core.quotepath=false", *args], cwd=ROOT
    )


def text_git(*args: str) -> str:
    return git(*args).decode("utf-8")


def historical_items() -> dict[str, dict]:
    """id -> {item, task, commit, assets_json}; newest snapshot wins."""
    by_id: dict[str, dict] = {}
    for commit in text_git("rev-list", "--all", "--date-order", "--", "tasks").split():
        for rel in text_git("ls-tree", "-r", "--name-only", commit, "--", "tasks").splitlines():
            if not rel.endswith("/assets.json"):
                continue
            task = rel.split("/")[1]
            try:
                af = json.loads(text_git("show", f"{commit}:{rel}"))
            except Exception:
                continue
            for item in af.get("items", []):
                aid = str(item.get("id", "")).strip()
                if not aid or aid in by_id:
                    continue
                by_id[aid] = {
                    "item": item,
                    "task": task,
                    "commit": commit,
                    "assets_json": rel,
                }
    return by_id


def main() -> int:
    ASSETS.mkdir(parents=True, exist_ok=True)
    current_path = ASSETS / "assets.json"
    current = (
        json.loads(current_path.read_text(encoding="utf-8"))
        if current_path.is_file()
        else {"schema_version": 1, "design_resolution": [1280, 720], "items": [], "collections": []}
    )
    current_ids = {str(it["id"]) for it in current.get("items", [])}
    hist = historical_items()
    missing = [v for k, v in hist.items() if k not in current_ids]

    stamp = time.strftime("%Y%m%d-%H%M%S")
    backup = ASSETS / ".backup" / f"recover-{stamp}"
    backup.mkdir(parents=True, exist_ok=True)
    if current_path.is_file():
        shutil.copy2(current_path, backup / "assets.json")
    if (ASSETS / "images").is_dir():
        shutil.copytree(ASSETS / "images", backup / "images", dirs_exist_ok=True)

    items = list(current.get("items", []))
    recovered = []
    failed = []
    # Continue the canonical aNNN.png sequence; never use a non-sequential name.
    next_img = 1
    for p in (ASSETS / "images").glob("a*.png"):
        try:
            n = int(p.stem[1:])
            next_img = max(next_img, n + 1)
        except ValueError:
            pass
    for entry in missing:
        old = dict(entry["item"])
        aid = str(old["id"])
        item = dict(old)
        # Clean legacy-only fields and convert operation region.
        action_roi = item.pop("action_roi", None)
        item.pop("roi_back", None)
        item.pop("ocr_source_roi", None)
        if action_roi and item.get("action_shape") is None:
            item["action_shape"] = {
                "kind": "rect",
                "roi": action_roi,
                "invert": False,
            }
            item["action_source"] = "fixed"
        elif item.get("action_shape") is None:
            item["action_source"] = "recognition"
        # Copy the exact template image blob from the historical commit.
        rel_file = old.get("file")
        if rel_file:
            dot = Path(rel_file).suffix or ".png"
            new_rel = f"images/a{next_img:03d}{dot}"
            next_img += 1
            src = f"{entry['commit']}:tasks/{entry['task']}/{rel_file}"
            try:
                data = git("show", src)
            except subprocess.CalledProcessError:
                failed.append({"id": aid, "reason": f"missing image blob {src}"})
                continue
            dest = ASSETS / new_rel
            dest.parent.mkdir(parents=True, exist_ok=True)
            dest.write_bytes(data)
            item["file"] = new_rel
        items.append(item)
        recovered.append(
            {
                "id": aid,
                "kind": item.get("kind"),
                "from_task": entry["task"],
                "commit": entry["commit"],
                "file": item.get("file"),
            }
        )

    out = {
        "schema_version": 1,
        "design_resolution": current.get("design_resolution") or [1280, 720],
        "items": items,
        "collections": current.get("collections", []),
    }
    current_path.write_text(
        json.dumps(out, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    report = {
        "backup": str(backup),
        "before": len(current_ids),
        "historical_unique": len(hist),
        "recovered": len(recovered),
        "after": len(items),
        "failed": failed,
        "recovered_items": recovered,
    }
    (ASSETS / "history-recovery-report.json").write_text(
        json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    print(json.dumps({k: report[k] for k in ("before", "historical_unique", "recovered", "after", "failed")}, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
