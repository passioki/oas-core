# Sougenbi（业原火）

> 来源：`runhey/OnmyojiAutoScript@dev`（只读 checkout `D:/OAS-dev/_verify/remote-oas`，commit `8b4dc669b478`）。
> 迁移方式：行为抽象 + 既有 oas-flow DSL（ENGINE-DESIGN §22），无 Python 代码拷贝。

## 读取的源文件

- `tasks/Sougenbi/script_task.py`
- `tasks/Sougenbi/config.py`
- `tasks/Sougenbi/assets.py`
- `tasks/Component/GeneralBattle/general_battle.py (imported)`
- `tasks/Component/SwitchSoul/switch_soul.py (imported)`
- `tasks/GameUi/game_ui.py, tasks/GameUi/page.py (imported)`

## 行为摘要

庭院→探索→御魂→业原火：选贪/嗔/痴，按所选类型读顶部突破券数量（=0 → resource_empty），锁/解锁阵容，loop limit_count 次「挑战→准备→自动战斗→结算回到业原火页」，最后回庭院。

## 参数 / 出口

- `sougenbi_class` (string) default='痴'
- `limit_count` (int) default=30
- `lock_team` (bool) default=False

- exit `done` → `success`
- exit `resource_empty` → `resource`
- exit `failed` → `retryable`

## 已实现的行为（行为矩阵见 migration/specs/Sougenbi.md）

- 进入业原火：覆盖
- 选择贪/嗔/痴：覆盖
- 券数=0 退出：覆盖（出口从 success 改为 resource）
- 阵容锁：覆盖
- 挑战循环/次数上限：覆盖
- 战斗：部分：通用骨架
- 回庭院：覆盖
- 加成开关：缺口：GeneralBuff 未迁移
- 御魂预设切换：缺口：SwitchSoul 未迁移
- 时间上限：缺口：无时钟原语

## 已知缺口 / TODO

- limit_time（30 分钟时间上限）无 DSL 时钟原语 → 只保留 limit_count 次数上限（调度层可用 --budget-wall 兜底）。
- 加成开关（GeneralBuff: gold_50/gold_100/exp_50/exp_100）未迁移：Component/GeneralBuff 素材未导入全局。
- 御魂预设切换（SwitchSoul: run_switch_soul / run_switch_soul_by_name）未迁移：DSL 无预设/御魂切换原语。
- 战斗用通用 P-BATTLE 骨架（准备→自动→安全点直到回页），不含 GeneralBattle 的绿标/预设阵容/失败分支/奖励统计页处理。
- class 页检测与券 OCR ROI 来自 remote dev 分支，未在当前游戏版本真机验证。

## 复用的通用模式

- **P-NAV**：`legacy_GameUi_I_CHECK_MAIN` 守卫 → `click legacy_GameUi_I_MAIN_GOTO_*` / `legacy_GameUi_I_EXPLORATION_GOTO_*`（导入器已改为：模板 `click` = 匹配到的模板矩形，与旧引擎一致）。
- **P-BATTLE**：`战斗_准备按钮` → 点击 → `战斗_自动按钮` → 循环点安全点 (640,690) 直到任务页 marker 回来。
- **P-RETURN**：`legacy_GlobalGame_I_UI_BACK_YELLOW` / `legacy_GameUi_I_BACK_YOLLOW` / 坐标 (48,43) 兜底，直到 `legacy_GameUi_I_CHECK_MAIN`。
- **OCR**：RuleOcr 已由导入器转为 `ocr_lines`；计数用 `ocr_digit`，条件用 `text_visible`，按行点击用 `click_target` + `{kind:ocr_text, sel|roi, text}`。运行需 `--model`/`--det-model`。

## 证据

- `oasd task lint --tasks-dir tasks --task Sougenbi` → 0 errors（见 migration/status/Sougenbi.json）。
- 未真机 / 未回放验证（迁移会话无可用设备与对应场景回放）；素材 id 与 ROI 来自 remote dev 分支，素材均已在全局 `assets/assets.json` 解析（T4 绿）。

## Polish round (2026-11-01)

- 突破券=0 的退出路径修正：原 flow 在挑战循环内直接 `finish(resource_empty)`（停在业原火页）；现改为置位 + break，循环结束后照常回庭院，再 `finish(resource_empty)` —— 与远程「break 循环 → 返回探索/庭院」一致。
- `meta.entry.requires` 从 `庭院_探索灯笼` 改为 `legacy_GameUi_I_CHECK_MAIN`（page_main 标记）。
- lint: 0 errors; no real-device evidence.



