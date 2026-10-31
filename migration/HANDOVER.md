# OAS MIGRATION HANDOVER (2026-11-01)

## Release

- oas-core `main` `385d611` = v1.2.3 snapshot
- tag `v1.2.3`
- GitHub Release: `https://github.com/passioki/oas-core/releases/tag/v1.2.3`
- zip SHA256: `F135922D762A28AE971377D744B4CB286AB9F21C708CD372CAB30EFE30F9C12E`
- tests: 350 passed
- migration works on `oas-core dev`; not merged to main

## Migration state

- source: `runhey/OnmyojiAutoScript@dev` `8b4dc669...`
- remote dirs: 61 = 53 menu tasks + 8 helpers
- 53 task bundles + 53 specs + 53 status files
- 2825 global assets
- `task lint --all`: 57 tasks / 0 errors
- status: **1 ✅ / 43 🔄 / 9 ⛔ / 8 ⬜**

Only `Pets` is whole-task live-Done. `TalismanPass` has a live no-op Done but reward branch incomplete. `DailyTrifles` friend_love/store_sign have live Done.

## Must know before continuing

1. **Reset to a clean, logged-in courtyard before every live test.** A task leaves arbitrary pages (settings/shop/event/friend); testing from there causes false entry failures.
2. **Main UI may be hidden.** Bottom navigation (图鉴/珍旅居/…/花合战/好友/…) can be absent. Clicking the right-bottom scroll `(1200,680)` sometimes reveals it, but the action is not idempotent.
3. **`ui_main_bar_guide` is not a reliable UI-visible marker.** A hardened 2x toggle failed; don't drive toggles with it.
4. **Remote page templates are stale** for current CN client: navigation icons, `I_CHECK_FRIENDS` (actually main 精灵), `O_LUCK_MSG`, `I_RED_POINT_LEVEL` etc. Prefer editor ROI recapture over threshold lowering.
5. **`I_CHECK_FRIENDS` is wrong**; `ui_friend_page` (好友标题) was added, but friend entry still needs coordinate/state calibration.
6. Task lints passing ≠ behavior equivalence. Do not mark ✅ without live/replay evidence.

## Next actions (priority)

1. Recapture stable UI-visible marker and main navigation icons with editor ROI.
2. Fix DailyTrifles friend entry + luck_msg (吉闻) in clean main.
3. Fix TalismanPass reward path (level reward entry; red-point template unreliable).
4. Continue 43 🔄 using `migration/status/*.json` gaps.
5. 9 ⛔ need DSL/engine primitives: multi-line OCR number/line-box, duration/clock, cross-task scheduling, long press, team room state machine, dynamic scan/geometry, image similarity, external API/ONNX.

## Useful docs

- `migration/README.md`
- `migration/TASK-LEDGER.md`
- `migration/ASSET-DRIFT.md`
- `migration/specs/*.md`
- `migration/status/*.json`
- `docs/MIGRATION-LEDGER.md` (backend)
