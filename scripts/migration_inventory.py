# -*- coding: utf-8 -*-
"""Build a migration inventory for remote OnmyojiAutoScript tasks."""
import ast
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REMOTE = ROOT.parent / "_verify" / "remote-oas"
OUT = ROOT / "migration"


def menu_groups():
    p = REMOTE / "module/config/config_menu.py"
    groups = {}
    tree = ast.parse(p.read_text(encoding="utf-8"))
    for node in ast.walk(tree):
        if not isinstance(node, ast.Assign) or len(node.targets) != 1:
            continue
        t = node.targets[0]
        if not (isinstance(t, ast.Subscript) and isinstance(t.value, ast.Attribute)
                and isinstance(t.value.value, ast.Name) and t.value.value.id == "self"
                and t.value.attr == "menu"):
            continue
        try:
            key = ast.literal_eval(t.slice)
            val = ast.literal_eval(node.value)
        except Exception:
            continue
        if isinstance(key, str) and isinstance(val, list):
            groups[key] = [str(x) for x in val]
    return groups


def main():
    groups = menu_groups()
    menu = {}
    for g, names in groups.items():
        for n in names:
            menu[n] = g
    helpers = {"Component", "General", "GameUi", "GlobalGame", "GotoMain", "Restart", "Script", "Utils"}
    asset_map = json.loads((OUT / "asset-map.json").read_text(encoding="utf-8"))["map"]
    rows = []
    for d in sorted((REMOTE / "tasks").iterdir()):
        if not d.is_dir():
            continue
        name = d.name
        script = d / "script_task.py"
        lines = 0
        imports = 0
        if script.is_file():
            text = script.read_text(encoding="utf-8", errors="replace")
            lines = len(text.splitlines())
            imports = len(re.findall(r"^(?:from|import)\s+", text, re.M))
        assets = sum(1 for k in asset_map if k.startswith(name + "."))
        res = sum(1 for _ in (d / "res").glob("*.png")) if (d / "res").is_dir() else 0
        rows.append({
            "task": name,
            "group": menu.get(name, "helper"),
            "is_helper": name in helpers,
            "script_lines": lines,
            "imports": imports,
            "assets": assets,
            "res_png": res,
            "has_config": (d / "config.py").is_file(),
            "has_script": script.is_file(),
            "has_res": (d / "res").is_dir(),
        })
    rows.sort(key=lambda r: (r["is_helper"], r["script_lines"], r["task"]))
    OUT.mkdir(exist_ok=True)
    (OUT / "inventory.json").write_text(json.dumps(rows, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    # ledger skeleton
    lines = ["# TASK MIGRATION LEDGER", "",
             "> Source: runhey/OnmyojiAutoScript@dev (read-only). Status: ⬜ pending / 🔄 in progress / ✅ implemented / ⛔ blocked.",
             "",
             "| # | Task | Group | Script lines | Assets | Res PNG | Status | Notes |",
             "|---|---|---|---:|---:|---:|---|---|"]
    for i, r in enumerate(rows, 1):
        status = "⬜" if not r["is_helper"] else "⛔ helper"
        lines.append(f"| {i} | {r['task']} | {r['group']} | {r['script_lines']} | {r['assets']} | {r['res_png']} | {status} | |")
    (OUT / "TASK-LEDGER.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"tasks={len(rows)} menu={sum(1 for r in rows if not r['is_helper'])} helpers={sum(1 for r in rows if r['is_helper'])}")
    print("wrote migration/inventory.json and migration/TASK-LEDGER.md")


if __name__ == "__main__":
    main()
