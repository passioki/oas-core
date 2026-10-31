# Duel（斗技）

> 来源：`runhey/OnmyojiAutoScript@dev`（只读 checkout `D:/OAS-dev/_verify/remote-oas`，commit `8b4dc669b478`）。
> 迁移方式：行为抽象 + 既有 oas-flow DSL（ENGINE-DESIGN §22），无 Python 代码拷贝。

## 读取的源文件

- `tasks/Duel/script_task.py`
- `tasks/Duel/config.py`
- `tasks/Duel/assets.py`
- `tasks/Component/GeneralBattle/general_battle.py (imported)`
- `tasks/Component/SwitchSoul/switch_soul.py (imported)`
- `tasks/Component/SwitchOnmyoji/switch_onmyoji.py (imported)`
- `tasks/GameUi/game_ui.py, tasks/GameUi/page.py (imported)`

## 行为摘要

庭院→町中→斗技：进入斗技主界面，loop max_battles 次「点战斗→等待准备页→自动上阵/准备→P-BATTLE→回斗技页」；分数/名仕/ban/荣誉/时间窗未迁移。

## 参数 / 出口

- `max_battles` (int) default=6

- exit `done` → `success`
- exit `failed` → `retryable`

## 行为矩阵（详见 migration/specs/Duel.md）

- 进入斗技：覆盖
- 开始战斗：覆盖
- 准备阶段：部分
- 战斗等待：部分
- 分数/名仕/时间：缺口：无时钟/分数目标逻辑
- 回庭院：覆盖

## 已知缺口 / TODO

- 目标分数/名仕(celeb)判定（O_D_SCORE OCR、I_BAN、O_D_BAN_NAME、C_DUEL_CLICK_5 自动上阵）未迁移：flow 只按 max_battles 次数循环，不打到分数目标不退出。
- 12:00-23:00 时间窗无时钟原语 → scheduler windows。
- 荣誉满退出（O_D_HONOR）、练习模式检测（I_BATTLE_WITH_TRAIN）、结算分享（I_D_VICTORY_SHARE）未迁移。
- 御魂/式神/阴阳师切换（switch_soul/switch_onmyoji/switch_all_soul）未迁移。
- P-BATTLE v2 不含斗技专有的准备阶段处理（名仕 ban、I_D_CHECK_BAN 等）。

## 复用的通用模式

- **P-NAV**：`legacy_GameUi_I_CHECK_MAIN` → `click legacy_GameUi_I_*_GOTO_*`（模板点击 = 匹配到的模板矩形）→ 目标页 marker。
- **P-BATTLE v2**：`Component_GeneralBattle_I_PREPARE_HIGHLIGHT` → 点准备 → 等 `I_WIN/I_FALSE/I_REWARD/I_REWARD_GOLD` → 逐一点掉胜负横幅与奖励 → 等任务页 marker。
- **P-RETURN**：`GlobalGame_I_UI_BACK_*` / `GameUi_I_BACK_YOLLOW` / 坐标兜底 → `I_CHECK_MAIN`。
- **OCR**：`RuleOcr` 已导入为 `ocr_lines`；计数 `ocr_digit`，条件 `text_visible`，按行点击 `click_target {kind:ocr_text, sel|roi, text}`。运行需 `--model/--det-model`。

## 证据

- `oasd task lint --tasks-dir tasks --task Duel` → 0 errors；状态见 `migration/status/Duel.json`。
- 未真机 / 未回放验证（迁移会话无可用设备与对应场景回放）。
