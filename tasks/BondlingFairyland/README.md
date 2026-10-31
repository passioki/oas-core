# BondlingFairyland（契灵之境）

> 来源：`runhey/OnmyojiAutoScript@dev`（只读 checkout `D:/OAS-dev/_verify/remote-oas`，commit `8b4dc669b478`）。
> 迁移方式：行为抽象 + 既有 oas-flow DSL（ENGINE-DESIGN §22），无 Python 代码拷贝。

## 读取的源文件

- `tasks/BondlingFairyland/script_task.py`
- `tasks/BondlingFairyland/battle.py`
- `tasks/BondlingFairyland/config.py`
- `tasks/BondlingFairyland/config_battle.py`
- `tasks/BondlingFairyland/assets.py`
- `tasks/BondlingFairyland/general_invite.py (imported)`
- `tasks/Component/GeneralBattle, GeneralRoom, SwitchSoul (imported)`

## 行为摘要

庭院→探索→契灵之境→结契区：循环「挑战→确认→战斗」；结契捕捉小游戏、契灵石头购买、搜索/邀友模式、结契设置等本轮未迁移（flow 以 blocked 退出）。

## 参数 / 出口

- `limit_count` (int) default=5

- exit `done` → `success`
- exit `no_target` → `blocked`
- exit `failed` → `retryable`

## 行为矩阵（详见 migration/specs/BondlingFairyland.md）

- 进入契灵之境：覆盖（商店设置未迁移）
- 进入结契区：部分
- 挑战：部分
- 结契捕捉：缺口（核心）
- 契灵石头/商店：缺口
- 搜索/组队/邀请：缺口

## 已知缺口 / TODO

- 结契捕捉流程（run_catch + capture_setting：自动结契/极简模式/低中高品质/首次/连续开关、I_CAP_SUCCESS/I_CAP_FAILURE/I_CAP_AGAIN、回合数/球数 OCR）未迁移 —— 这是该任务的核心之一。
- 契灵种类识别（get_bondling_class: I_TOMB_GUARD/I_SNOWBALL/I_LITTLE_KURO/I_AZURE_BASAN）与球数/石板数 OCR 未迁移。
- 契灵石头购买 run_stone（I_BUY_PLUS/I_STONE_SURE）未迁移。
- 搜索模式 run_search（I_BF_SEARSH/I_BF_STORE）与队长/成员/邀请（GeneralRoom+GeneralInvite 全套）未迁移；仅单刷挑战骨架。
- BondlingBattle（战斗内策略）未迁移；用 P-BATTLE v2 通用骨架。
- limit_time/次数上限部分保留（limit_count）。

## 复用的通用模式

- **P-NAV**：`legacy_GameUi_I_CHECK_MAIN` → `click legacy_GameUi_I_*_GOTO_*`（模板点击 = 匹配到的模板矩形）→ 目标页 marker。
- **P-BATTLE v2**：`Component_GeneralBattle_I_PREPARE_HIGHLIGHT` → 点准备 → 等 `I_WIN/I_FALSE/I_REWARD/I_REWARD_GOLD` → 逐一点掉胜负横幅与奖励 → 等任务页 marker。
- **P-RETURN**：`GlobalGame_I_UI_BACK_*` / `GameUi_I_BACK_YOLLOW` / 坐标兜底 → `I_CHECK_MAIN`。
- **OCR**：`RuleOcr` 已导入为 `ocr_lines`；计数 `ocr_digit`，条件 `text_visible`，按行点击 `click_target {kind:ocr_text, sel|roi, text}`。运行需 `--model/--det-model`。

## 证据

- `oasd task lint --tasks-dir tasks --task BondlingFairyland` → 0 errors；状态见 `migration/status/BondlingFairyland.json`。
- 未真机 / 未回放验证（迁移会话无可用设备与对应场景回放）。
