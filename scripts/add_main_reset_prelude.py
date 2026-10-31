# -*- coding: utf-8 -*-
"""Insert a bounded back-to-main prelude before the UI bootstrap in migrated flows."""
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TASKS = ROOT / "tasks"
MIGRATED = {p.stem for p in (ROOT / "migration" / "status").glob("*.json")}

PRELUDE = [
    {
        "op": "loop",
        "args": {"n": "5"},
        "body": [
            {
                "op": "if",
                "args": {"cond": "appear('legacy_GameUi_I_CHECK_MAIN')"},
                "then": [{"op": "break"}],
                "else": [],
            },
            {"op": "click_point", "args": {"x": 44, "y": 31}},
            {"op": "wait", "args": {"seconds": 1.2}},
        ],
    }
]


def main():
    changed = 0
    for name in sorted(MIGRATED):
        p = TASKS / name / "flow.json"
        if not p.is_file():
            continue
        flow = json.loads(p.read_text(encoding="utf-8"))
        text = json.dumps(flow, ensure_ascii=False)
        if "__main_reset_prelude" in text:
            continue
        steps = flow.setdefault("steps", [])
        # Marker only in a log so static tooling can detect the prelude.
        marker = {
            "op": "log",
            "args": {"level": "debug", "text": "main reset prelude"},
        }
        steps[0:0] = [marker] + json.loads(json.dumps(PRELUDE))
        p.write_text(json.dumps(flow, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        changed += 1
    print(f"prelude inserted into {changed} task flows")


if __name__ == "__main__":
    main()
