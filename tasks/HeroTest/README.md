# HeroTest（英杰试炼：鬼兵演武 / 兵藏秘境 / 传承试炼 / 梦虚秘境）

## 来源（只读旧仓）

- `tasks/HeroTest/script_task.py`（344 行）— run / enter_battle / battle_wait 重写 / hero1_skill_wait /
  hero2_skill_wait / switch_hero / can_run / check_level_max / check_and_lock_team / init_pages / buff 开关
- `tasks/HeroTest/config.py` — `layer`（默认 鬼兵演武）、`limit_time`（30min）、`limit_count`（100）、
  `exp_50_buff_enable_help` / `exp_100_buff_enable_help`、`GeneralBattleConfig`、`switch_soul_config`
- `tasks/HeroTest/assets.py` — 41 个模板 + 4 个 OCR（O_FIRE=「挑战」Single、O_ART_WAR_CARD/P LUS、O_REMAIN_AP）
- `tasks/Component/GeneralBattle/general_battle.py` — run_general_battle / battle_wait / green mark
- `tasks/GameUi/default_pages.py` — `page_hero_test` 与 `page_exploration→page_hero_test`

## 行为摘要（旧仓）

1. `check_and_switch_soul()`（默认关）→ `open_exp_buff()`（默认关）→ `switch_hero(layer)` →
   `init_pages()`（按层把英杰页连到模式页）→ `goto_page(page_hero_mode)` → `check_and_lock_team()`。
2. `while True`：
   - `limit_time`（30min）/ `limit_count`（100）到 → break；
   - `goto_page(page_hero_mode)`（每轮回模式页）；
   - `can_run(layer)`：兵藏/梦虚读 `O_ART_WAR_CARD`（+PLUS 兜底），传承读 `I_HERO_EXP_MAX`（满级停）；
   - `enter_battle()`：点 `O_FIRE`（OCR「挑战」）/ `I_START_CHALLENGE` / `I_BCMJ_RESET_CONFIRM`，
     `I_REAL_MONEY` 出现表示门票不足；
   - `run_general_battle()`：`battle_before`（准备高亮）+ `battle_wait`（模式特定结算：兵藏/梦虚处理技能面板，
     传承/梦虚处理胜利/失败/奖励）。
3. `close_exp_buff()` → 结束。

## 流程（flow.json）

| # | op | 说明 | 守卫 |
|---|----|------|------|
| 1 | 庭院→探索→英杰试炼 | 两条页面边 | expect `I_CHECK_HERO_TEST` |
| 2 | 切换英杰 | `param('layer')` 1/2→英雄1、3/4→英雄2；开启切换页点固定点、确认页用区域点击 | `I_CHECK_HERO1/2` |
| 3 | 进入模式页 | 层 1 `I_GBB`、层 2 `I_BCMJ`、层 3 `I_ENTER_CCSL`、层 4 `I_ENTER_MXMJ` | 模式页守卫 |
| 4 | 锁定/解锁阵容 | `param('lock_team')`；层 1/3 用 I_LOCK/UNLOCK，层 2/4 用 I_BCMJ_LOCK/UNLOCK | |
| 5 | loop×`param('max_battles')` | 每轮：回模式页 → can_run → 进入战斗 → 自动战斗（技能面板）→ 结算回模式页 | |
| 6 | 回庭院 | 依次尝试层内返回/英杰返回/黄返回 | expect `I_CHECK_MAIN` |
| 7 | if `var('empty')==1` → `resource_empty`；否则 `done` | | |

## 素材

| 全局 id | 来源 | 用途 |
|---------|------|------|
| legacy_HeroTest_I_CHECK_HERO1/2 | 任务 | 英杰是否选中 |
| legacy_HeroTest_I_SWITCH_HERO1/2 | 任务 | 切换英杰界面（点击旧仓 roi_front 中心） |
| legacy_HeroTest_C_SWITCH_HERO_BTN | 任务 RuleClick | 打开切换界面 |
| legacy_HeroTest_I_GBB / I_BCMJ / I_ENTER_CCSL / I_ENTER_MXMJ | 任务 | 进入模式页 |
| legacy_HeroTest_I_CHECK_HERO1_EXP / _SKILL / I_CHECK_HERO2_EXP / _SKILL | 任务 | 模式页守卫 |
| legacy_HeroTest_I_LOCK/UNLOCK / I_BCMJ_LOCK/UNLOCK | 任务 | 阵容锁定 |
| legacy_HeroTest_I_HERO_EXP_MAX / I_REAL_MONEY | 任务 | 满级/门票不足 |
| legacy_HeroTest_O_FIRE | 任务 OCR | click_target OCR 文本「挑战」 |
| legacy_HeroTest_O_ART_WAR_CARD / _PLUS | 任务 OCR | 秘卷数量（ocr_digit） |
| legacy_HeroTest_I_BCMJ_SKILL_ADD1/2 / I_BCMJ_BLESS / I_BCMJ_PROPERTY_ADD_CRITICAL / I_BCMJ__DEFALUT_ATTRIBUTE / I_BCMJ_SKILL_ADD_CONFIRM | 任务 | 兵藏秘境技能面板 |
| legacy_HeroTest_I_HERO2_SKILL1..6 | 任务 | 梦虚秘境技能优先级 |
| legacy_HeroTest_I_BCMJ_WIN / I_BCMJ_RESET_CONFIRM / I_BCMJ_RESET_CLOSE | 任务 | 兵藏结算/重置 |
| legacy_Component_GeneralBattle_I_WIN / I_DE_WIN / I_FALSE / I_REWARD / C_REWARD_2 / C_RANDOM_TOP / C_RANDOM_RIGHT | GeneralBattle | 通用结算 |
| legacy_GameUi_I_EXPLORATION_GOTO_HERO_TEST / I_CHECK_HERO_TEST / I_BACK_YOLLOW / legacy_GameUi_I_CHECK_MAIN | GameUi | 导航 |
| 庭院_探索灯笼 / 世界_章节栏 / 战斗_准备按钮 / 战斗_自动按钮 | core | 导航/战斗 |

## 出口

- `done`(success)：正常结束。
- `resource_empty`(resource)：兵藏/梦虚秘卷不足（`I_REAL_MONEY` 或 O_ART_WAR_CARD 判 0）。
- `failed`(retryable)。

## 与旧仓的差异 / 已知缺口

1. **`limit_time`（30 分钟）不迁移**：DSL 无 wall-clock；用 `param('max_battles')`（默认 20，旧仓 limit_count=100）作为上限。
2. **经验加成（50/100）未迁移**：GeneralBuff 开关区域是「OCR 文本框右侧 + 偏移」的动态区域，DSL 无锚点定位（ENGINE §22.14 未实现）。
3. **换御魂未迁移**（默认关）。
4. **随机点击/滑动防封（random_click_swipt）未迁移**：`C_RANDOM_*` 已导入，但 DSL 无随机分支；结算点击用固定区域近似。
5. **PVP 技能模式（SkillMode.PVP）未迁移**：旧仓 pvp_skill 列表为空（未实现），flow 只覆盖 PVE。
6. 技能面板 10 秒无选择兜底：flow 用「固定优先级 + 区域点击兜底 + 8 轮确认」近似（无计时器）。
7. `switch_hero` 的 I_SWITCH_HERO1/2 是整屏模板，旧仓点击其 roi_front 中心（(301,293)/(634,302)），flow 用同坐标 `click_point`。
8. 未真机验证。

## Lint

```powershell
cd D:\OAS-dev\oas-core
.\target\release\oasd.exe task lint --tasks-dir tasks --task HeroTest
```
