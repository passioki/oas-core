# -*- coding: utf-8 -*-
"""Add the idempotent main-UI bootstrap to every migrated task flow."""
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TASKS = ROOT / "tasks"
# Only tasks that came from the remote migration (baseline Chinese tasks are untouched).
MIGRATED = {
    p.stem for p in (ROOT / "migration" / "status").glob("*.json")
}

BOOTSTRAP = {
    "op": "if",
    "args": {
        "cond": "not appear('ui_main_bar_guide')",
    },
    "then": [
        {"op": "click_point", "args": {"x": 1200, "y": 680}},
        {"op": "wait", "args": {"seconds": 1.5}},
    ],
    "else": [],
}


def main():
    changed = 0
    for name in sorted(MIGRATED):
        p = TASKS / name / "flow.json"
        if not p.is_file():
            continue
        flow = json.loads(p.read_text(encoding="utf-8"))
        text = json.dumps(flow, ensure_ascii=False)
        if "ui_main_bar_guide" in text:
            continue
        steps = flow.setdefault("steps", [])
        steps.insert(0, json.loads(json.dumps(BOOTSTRAP)))
        p.write_text(json.dumps(flow, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        changed += 1
    print(f"bootstrap inserted into {changed} task flows")


if __name__ == "__main__":
    main()
