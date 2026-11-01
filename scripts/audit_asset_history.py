#!/usr/bin/env python3
"""Audit every historical task assets.json in git and compare with the current
global store. Read-only: prints a JSON report, writes nothing to the repo."""
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def git(*args: str) -> bytes:
    return subprocess.check_output(
        ["git", "-c", "core.quotepath=false", *args], cwd=ROOT
    )


def decode(data: bytes) -> str:
    return data.decode("utf-8")


def main() -> int:
    commits = decode(
        git("rev-list", "--all", "--date-order", "--", "tasks")
    ).split()
    by_id: dict[str, dict] = {}
    per_task: dict[str, dict[str, int]] = {}
    scanned = 0
    for commit in commits:
        tree = decode(git("ls-tree", "-r", "--name-only", commit, "--", "tasks"))
        for rel in tree.splitlines():
            if not rel.endswith("/assets.json"):
                continue
            task = rel.split("/")[1]
            try:
                af = json.loads(decode(git("show", f"{commit}:{rel}")))
            except Exception as e:  # corrupted historical snapshot
                print(f"skip {commit}:{rel}: {e}", file=sys.stderr)
                continue
            scanned += 1
            for item in af.get("items", []):
                aid = item.get("id", "")
                if not aid:
                    continue
                # Newest commit wins (rev-list is newest-first).
                if aid in by_id:
                    continue
                by_id[aid] = {
                    "id": aid,
                    "kind": item.get("kind"),
                    "task": task,
                    "commit": commit,
                    "item": item,
                    "assets_json": rel,
                }
                per_task.setdefault(task, {})[aid] = 1
    current_path = ROOT / "assets" / "assets.json"
    current: dict[str, dict] = {}
    if current_path.is_file():
        cur = json.loads(current_path.read_text(encoding="utf-8"))
        current = {it["id"]: it for it in cur.get("items", [])}
    missing = [a for a in by_id.values() if a["id"] not in current]
    report = {
        "commits_scanned": len(commits),
        "assets_json_snapshots_scanned": scanned,
        "historical_unique_ids": len(by_id),
        "current_global_ids": len(current),
        "missing_from_current_global": len(missing),
        "tasks": {k: len(v) for k, v in sorted(per_task.items())},
        "missing": sorted(missing, key=lambda a: (a["task"], a["id"])),
    }
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
