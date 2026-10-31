# HeroTest — 行为矩阵（迁移规格）

源：`runhey/OnmyojiAutoScript@dev` · `tasks/HeroTest/`（Liver Emperor Exclusive）

## 层枚举（旧仓 Layer）

| 值 | 旧仓 | 英杰 | 模式页 | 进入按钮 |
|----|------|------|--------|----------|
| 1 | 鬼兵演武 YANWU | 源赖光 | I_CHECK_HERO1_EXP | I_GBB |
| 2 | 兵藏秘境 MIJING | 源赖光 | I_CHECK_HERO1_SKILL | I_BCMJ |
| 3 | 传承试炼 CHUANCHENG | 藤原道长 | I_CHECK_HERO2_EXP | I_ENTER_CCSL |
| 4 | 梦虚秘境 MENGXU | 藤原道长 | I_CHECK_HERO2_SKILL | I_ENTER_MXMJ |

flow 用 `param('layer')` 1..4 对应。

## 源文件

| 文件 | 作用 |
|------|------|
| tasks/HeroTest/script_task.py | run / switch_hero / init_pages / can_run / enter_battle / battle_wait / 技能面板 / buff |
| tasks/HeroTest/config.py | layer / limit_time / limit_count / buff / GeneralBattleConfig / switch_soul |
| tasks/HeroTest/assets.py | 41 模板 + O_FIRE / O_ART_WAR_CARD(_PLUS) / O_REMAIN_AP |
| tasks/Component/GeneralBattle/general_battle.py | run_general_battle / battle_before / battle_wait / green mark |
| tasks/GameUi/default_pages.py | page_hero_test、page_battle* |

## 行为矩阵

| 旧仓行为 | 新 flow 覆盖 | 备注 |
|----------|--------------|------|
| switch_soul（可选） | ❌ | 默认关闭 |
| exp buff（可选） | ❌ | 需锚点定位 |
| switch_hero(layer) | ✅ 4 层分支 | 旧整屏模板点击中心坐标 |
| init_pages + goto mode page | ✅ 4 层进入按钮 | |
| check_and_lock_team | ✅ lock_team 参数 | 层 1/3 与 2/4 两套资产 |
| limit_time 30min | ❌ | DSL 无时间 |
| limit_count 100 | ◑ param('max_battles') 默认 20 | 可由调度参数调整 |
| can_run: O_ART_WAR_CARD(+PLUS) | ✅ ocr_digit | |
| can_run: I_HERO_EXP_MAX | ✅ | |
| enter_battle: O_FIRE/I_START_CHALLENGE/I_BCMJ_RESET_CONFIRM/I_REAL_MONEY | ✅ loop 8 | OCR 文本「挑战」用 click_target |
| battle_before 准备高亮 | ✅ 有界循环 | |
| battle_wait：兵藏/梦虚技能面板 | ✅ 分支选择 + 8 轮确认 | 无 10s 计时器 |
| battle_wait：WIN/DE_WIN/REWARD/FALSE | ✅ 通用结算资产 | 固定区域点击替代随机 |
| random_click_swipt | ❌ | DSL 无随机 |
| 结束后回庭院/模式页 | ✅ 多级返回循环 | |

## 素材

见 README（任务 41 项 + GeneralBattle 7 项 + GameUi core）。

## 出口

`done` / `resource_empty` / `failed`。

## 缺口 / TODO

1. 时间限制（30min）与技能面板 10s 兜底需要 wall-clock → 有界轮次近似。
2. GeneralBuff 动态开关区域需要锚点定位（DSL 缺）。
3. random_click_swipt 需要随机 op（DSL 缺）。
4. PVP 技能列表旧仓为空（未实现），本 flow 只覆盖 PVE。
5. 未真机验证。
