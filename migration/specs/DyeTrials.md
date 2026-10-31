# DyeTrials — 行为矩阵（迁移规格）

源：`runhey/OnmyojiAutoScript@dev` · `tasks/DyeTrials/`（旧仓标注「灵染 试炼」）

## 源文件

| 文件 | 作用 |
|------|------|
| tasks/DyeTrials/script_task.py | run / get_all（入口循环 + 挑战循环） |
| tasks/DyeTrials/config.py | switch_soul_config（默认关闭） |
| tasks/DyeTrials/assets.py | 5 模板 + O_BATTLE_NUM |
| tasks/Component/GeneralBattle/general_battle.py | 通用战斗（本任务实际用自动战斗 + 轮询） |
| tasks/Component/SwitchSoul/switch_soul.py | 换御魂（可选，未迁移） |
| tasks/Restart/assets.py | I_HARVEST_CHAT_CLOSE |

## 行为矩阵

| 旧仓行为 | 新 flow 覆盖 | 备注 |
|----------|--------------|------|
| switch_soul（可选） | ❌ | 默认关闭；组件资产未导入 |
| goto_page(page_main) | ✅ meta.entry=庭院 | |
| 入口循环（CHALLENGE / ACCESS / ACCESS_1 / TOGGLE） | ✅ loop 30 | |
| boss_timer 60s → 通知退出 | ❌ 有界循环替代 | DSL 无时间 |
| ui_reward_appear_click | ✅ appear(UI_REWARD) → 点操作区 | GlobalGame 通用 |
| I_HARVEST_CHAT_CLOSE 点击 | ✅ | |
| I_FP_CHALLENGE → O_BATTLE_NUM 50/50 break | ◑ 只保留 max_battles=50 上限 | DigitCounter 未实现 |
| battle_num>=50 break | ✅ loop 上限 | |
| ui_click_until_disappear(CHALLENGE) | ◑ 每轮点一次，未进入则下轮再点 | 有界重试 |
| I_BATTLE_SUCCESS 点击 | ✅ | |
| 退出 | ✅ 黄/红返回直到庭院 | |

## 出口

`done` / `failed`。

## 缺口 / TODO

1. DSL 缺 wall-clock：boss_timer 60s 与 notifier 通知无法表达（TODO：`now()`/`elapsed()` 或 meta 时间预算）。
2. O_BATTLE_NUM 为 DigitCounter（current/remain/total 三值）；`ocr_digit` 只读第一个数字 → 50/50 提前退出未实现。
3. SwitchSoul 组件素材未进全局库（`Component/SwitchSoul/assets.py`）。
4. 未真机验证。
