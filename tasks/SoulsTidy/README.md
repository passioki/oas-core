# SoulsTidy（御魂整理）

> 来源：`runhey/OnmyojiAutoScript@dev`（只读 checkout `D:/OAS-dev/_verify/remote-oas`，commit `8b4dc669b478`）。
> 迁移方式：行为抽象 + 既有 oas-flow DSL（ENGINE-DESIGN §22），无 Python 代码拷贝。

## 读取的源文件

- `tasks/SoulsTidy/script_task.py`
- `tasks/SoulsTidy/config.py`
- `tasks/SoulsTidy/assets.py`
- `tasks/GameUi/game_ui.py, tasks/GameUi/page.py (imported)`
- `tasks/GlobalGame/assets.py (UI_CONFIRM 等)`

## 行为摘要

庭院→式神录→御魂：进入御魂界面，喂贪吃鬼（最多 3 次），切到奉纳界面，按御魂堆叠逐次奉纳并处理神赐/吉运/确认弹窗；回庭院。

## 参数 / 出口

- `enable_greed` (bool) default=True
- `enable_maneki` (bool) default=True
- `max_donations` (int) default=5

- exit `done` → `success`
- exit `failed` → `retryable`

## 行为矩阵（详见 migration/specs/SoulsTidy.md）

- 进入御魂界面：覆盖
- 喂贪吃鬼：覆盖
- 切奉纳界面：覆盖
- 确保弃置筛选：覆盖
- 按等级排序：缺口：OCR keyword 未保留
- 奉纳循环：部分（长按/等级双保险/奖励等待缺口）

## 已知缺口 / TODO

- 奉纳前置「按等级排序」在远程用 O_ST_SORT_LEVEL_1/2 的 keyword 命中（ocr_appear_click）实现；导入的 OCR 素材没有 expected text → 新 flow 假设列表已按等级排序（缺口）。
- 奉纳选中御魂远程用 RuleLongClick L_ONE（长按）；DSL 无 long_click 原语 → 用 click_target(region L_ONE) 单击近似。
- O_ST_GOLD 金额：远程要求 int 且 != 0；新 flow 用 ocr_digit==0 判断（OCR 失败 → Null ≠ 0，可能继续）。
- 远程「+0 等级双保险」(O_ST_FIRSET_LEVEL 文本 == 古/+0) 与 stack/level-0 两模式分支未覆盖。
- ui_reward_appear_click / I_ST_GOLD 等待、金币不足保护未覆盖。

## 复用的通用模式

- **P-NAV**：`legacy_GameUi_I_CHECK_MAIN` → `click legacy_GameUi_I_*_GOTO_*`（模板点击 = 匹配到的模板矩形）→ 目标页 marker。
- **P-BATTLE v2**：`Component_GeneralBattle_I_PREPARE_HIGHLIGHT` → 点准备 → 等 `I_WIN/I_FALSE/I_REWARD/I_REWARD_GOLD` → 逐一点掉胜负横幅与奖励 → 等任务页 marker。
- **P-RETURN**：`GlobalGame_I_UI_BACK_*` / `GameUi_I_BACK_YOLLOW` / 坐标兜底 → `I_CHECK_MAIN`。
- **OCR**：`RuleOcr` 已导入为 `ocr_lines`；计数 `ocr_digit`，条件 `text_visible`，按行点击 `click_target {kind:ocr_text, sel|roi, text}`。运行需 `--model/--det-model`。

## 证据

- `oasd task lint --tasks-dir tasks --task SoulsTidy` → 0 errors；状态见 `migration/status/SoulsTidy.json`。
- 未真机 / 未回放验证（迁移会话无可用设备与对应场景回放）。
