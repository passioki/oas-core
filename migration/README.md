# OAS Task Migration (remote OnmyojiAutoScript → oas-core)

## Source of truth

- Source: `https://github.com/runhey/OnmyojiAutoScript` **dev**
- Local read-only checkout: `D:\OAS-dev\_verify\remote-oas` (shallow clone; never edit)
- This is **logic abstraction**, not a Python-to-JSON code dump.

## Status vocabulary

- `⬜ pending` — not started
- `🔄 in progress` — behavior mapped, implementation incomplete
- `✅ implemented` — flow + assets + lint pass; real-device/ replay evidence recorded
- `⛔ blocked` — current oas-core DSL/architecture cannot express it; record concrete reason

## Task bundle contract

`tasks/<RemoteTaskName>/`

- `meta.json` — `task_id`, `name`, `category`, `entry`, `params`, `exits`, `capabilities`, `schedule_hint`
- `flow.json` — oas-flow DSL; must end every path with a declared `finish(exit)`
- `README.md` — source files read, behavior summary, known gaps
- `migration.json` — source commit, source files, status, evidence, differences

## Asset contract

- Imported templates: `roi` = remote `roi_back` (search window), **no `action_roi`**; clicking resolves to the matched template rectangle (old behavior). Old `RuleClick` becomes `kind=region` with the exact operation region.
- Old `RuleOcr` (`roi`/`area` in assets.py) becomes `ocr_lines` without `ocr_text`.
- Shared `tasks/Component/*/assets.py` are imported as `legacy_Component_<Module>_<const>`.
- `RuleSwipe` becomes two region ids `<id>_FROM` / `<id>_TO`; callers use `drag` with their centers.
- Mapping: `migration/asset-map.json` (`Task.const → global asset id`).
- Images are copied into the global `assets/images/aNNN.png` sequence by `scripts/import_legacy_assets.py`.
- Do not reference the original repo at runtime.

## Flow contract

- Use existing DSL only (`docs/ENGINE-DESIGN.md §22`); do not invent ops.
- Reusable behavior should become a helper flow or a documented pattern, never 50 copies.
- A task is **not** complete just because `task lint` passes; the behavior checklist must be verified.
- For game actions needing unknown frame layout, record `⛔ blocked` with the missing primitive/asset rather than guessing.

## Migration ledger

- `migration/TASK-LEDGER.md` — one row per remote task.
- `migration/specs/<Task>.md` — behavior matrix and open questions.
- `migration/asset-map.json` — asset mapping.
- `migration/inventory.json` — raw scan.

## Current facts (2026-11-01)

- Remote task dirs: 61; menu tasks 53; helper dirs 8.
- Imported global assets: 2825 total (23 core + task/component legacy).
- Baseline `cargo test --workspace`: 350 passed.
