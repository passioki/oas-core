# SixRealms（六道之门·月海）

远程源：`runhey/OnmyojiAutoScript@dev` `tasks/SixRealms`（commit `8b4dc66`）。

**状态：🔄** — 入口→月海→开始（守族/首技能）→主地图覆盖；岛屿决策/小游戏/商店/首领战未迁移（缺 OCR 包含匹配与按框点击原语）

## 行为（抽象）

1. enable/enable_switch_by_name（配置）
2. `goto_page(page_six_gates)`（探索→玩法→六道之门，入口按活动期从 `I_EXPLORATION_TO_*` 里点）
3. `_start()`：等 `I_MENTER` 点击→等 `I_MSTART`；守族选择（`I_MSHUTEN`/`I_MSHOUZU_SELECT` 直到 `I_MSHOUZU`）；开始（`I_MSTART`/`I_MSTART_CONFIRM(2)`/`I_UI_CONFIRM(_SAMLL)`/`I_MSKIP`/`I_MCONINUE`）；`I_SELECT_SHIKIGAMI_AND_CONFIRM` 时上滑选式神并确认；等 `I_M_STORE`（商店图标）后点 `I_MFIRST_SKILL` 选首个柔风技能
4. 整屏 OCR `map_ocr`：按文本包含 宁息/神秘/回混范/蜜馨屡战/星之 判断岛屿类型；`island106` = `I_BOSS_FIRE`；同时抓「回合/国」行取剩余回合数；未识别 ⇒ 随机从 C_ISLAND_ENTER_1..6 进入
5. `enter_island`：按 OCR box 位置改写 `C_ISLAND_ENTER.roi_front` 后点击（动态区域）
6. 记忆/模仿/战斗等各自子流程（`moon_sea/l101.py` 等），含技能选择、商店买技能、战斗
7. `I_BOSS_FIRE` ⇒ `boss_team_lock`（I_BOSS_TEAM_LOCK/UNLOCK）→ `boss_battle`（胜利后 I_BOSS_SHARE→I_BOSS_SHUTU→I_MSTART；双倍 I_BOSS_USE_DOUBLE；跳过 I_BOSS_SKIP 第二 BOSS）
8. `_run_moon_sea` 限次(`limit_count`=1)/限时(30min) → 退出六道 → 回庭院

## 目录 / 契约

```
tasks/SixRealms/
├─ meta.json       # params/exits/capabilities/schedule_hint
├─ flow.json       # oas-flow DSL（仅既有 op）
├─ migration.json  # 源文件、差异、证据
└─ README.md
```

参数：无

## 运行 / 校验

```powershell
cd D:\OAS-dev\oas-core
.\target\release\oasd.exe task lint --tasks-dir tasks --task SixRealms   # => errors=0
.\target\release\oasd.exe run --task tasks\SixRealms
```

## 已知限制

1. 岛屿决策需 OCR 包含匹配 + 按 OCR 行框点击原语，未迁移
2. l101~l105 小游戏、商店、首领战未迁移
3. 无实机/回放证据（六道活动期限制）

详见 `migration/specs/SixRealms.md`、`tasks/SixRealms/migration.json`、`migration/status/SixRealms.json`。
