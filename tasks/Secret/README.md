# Secret（秘闻副本）

> 来源：`runhey/OnmyojiAutoScript@dev`（只读 checkout `D:/OAS-dev/_verify/remote-oas`，commit `8b4dc669b478`）。
> 迁移方式：行为抽象 + 既有 oas-flow DSL（ENGINE-DESIGN §22），无 Python 代码拷贝。

## 读取的源文件

- `tasks/Secret/script_task.py`
- `tasks/Secret/config.py`
- `tasks/Secret/assets.py`
- `tasks/Component/GeneralBattle/general_battle.py (imported)`
- `tasks/Component/SwitchSoul/switch_soul.py (imported)`
- `tasks/Component/GeneralBuff/general_buff.py (imported)`
- `tasks/WeeklyTrifles/assets.py (I_WT_SE_SHARE)`

## 行为摘要

庭院→探索→秘闻：进入秘闻副本（需已在副本内），loop max_battles 次「挑战→准备→战斗→回页」，检测 I_SE_FINISHED_1 完成；回庭院。

## 参数 / 出口

- `max_battles` (int) default=20

- exit `done` → `success`
- exit `failed` → `retryable`

## 行为矩阵（详见 migration/specs/Secret.md）

- 进入秘闻页：覆盖
- 进入副本：覆盖
- 按层选择：缺口
- 战斗循环：覆盖（层选择缺口）
- 加成/御魂：缺口
- 回庭院：覆盖

## 已知缺口 / TODO

- 层选择 find_battle（O_SE_LAYER_*/O_SE_NO_PASS 的 OCR 定位、向下滑动、'尚未' 文本判定）未迁移 —— flow 直接点挑战(即默认当前层)，可能重复打已通关层。
- 秘闻已完成/本周已分享判定：远程用 I_WT_SE_SHARE 与 O_SE_TOTAL_TIME 文本 contains '尚未'；flow 只用 I_SE_FINISHED_1（缺口）。
- 加成开关（secret_gold_50/100 open_buff）未迁移；御魂预设切换未迁移。
- layer_9/layer_10 的开关（con.layer_9/layer_10）未映射为参数，flow 统一按 max_battles。
- P-BATTLE v2 不含 GeneralBattle 的 DE_WIN/特殊奖励分支。

## 复用的通用模式

- **P-NAV**：`legacy_GameUi_I_CHECK_MAIN` → `click legacy_GameUi_I_*_GOTO_*`（模板点击 = 匹配到的模板矩形）→ 目标页 marker。
- **P-BATTLE v2**：`Component_GeneralBattle_I_PREPARE_HIGHLIGHT` → 点准备 → 等 `I_WIN/I_FALSE/I_REWARD/I_REWARD_GOLD` → 逐一点掉胜负横幅与奖励 → 等任务页 marker。
- **P-RETURN**：`GlobalGame_I_UI_BACK_*` / `GameUi_I_BACK_YOLLOW` / 坐标兜底 → `I_CHECK_MAIN`。
- **OCR**：`RuleOcr` 已导入为 `ocr_lines`；计数 `ocr_digit`，条件 `text_visible`，按行点击 `click_target {kind:ocr_text, sel|roi, text}`。运行需 `--model/--det-model`。

## 证据

- `oasd task lint --tasks-dir tasks --task Secret` → 0 errors；状态见 `migration/status/Secret.json`。
- 未真机 / 未回放验证（迁移会话无可用设备与对应场景回放）。
