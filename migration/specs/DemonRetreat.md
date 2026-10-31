# DemonRetreat 迁移规格

- 远程源：`runhey/OnmyojiAutoScript@dev` · commit `8b4dc669b47838f42d283f14aa394844c1963c86`（只读 `D:\OAS-dev\_verify\remote-oas`）
- 状态：**✅**（周六活动主链路（进神社→退治→集结/迟到→三轮战斗→领奖→回庭院）覆盖；周六窗口由调度器负责）
- 已读文件：
  - `tasks/DemonRetreat/script_task.py`
  - `tasks/DemonRetreat/config.py`
  - `tasks/DemonRetreat/assets.py`
  - `tasks/DemonRetreat/DemonRetreat/image.json`
  - `tasks/DemonRetreat/DemonRetreat/ocr.json`
  - `tasks/AbyssShadows/assets.py`
  - `tasks/GameUi/page.py`
- 共享阅读：`tasks/Component/GeneralBattle/{general_battle.py,battle.py,battle_wait.py}`、`tasks/Component/GeneralRoom/general_room.py`、`tasks/GameUi/default_pages.py`（导航图）

## 1. 上游行为矩阵

| # | 条件 / 输入 | 行为 |
|---|---|---|
| 1 | 非周六 | 按 `weekday()` 计算到周六的天数 → `custom_next_run` → `TaskEnd`（跳过） |
| 2 | 周六 | 可选御魂切换（enable / enable_switch_by_name） |
| 3 | 进退治 | `goto_page(page_guild)`→循环：点 `I_SHRINE`（神社）/`I_HUNT`（首领退治）/`I_QUIT_BACK`；`I_HUNT_CHECK` 或 `I_DEMON_GATHER` 或准备页 ⇒ 进入成功；点 `I_REWARD_ALL` ⇒ 已打完只领奖；`I_RANK_LSIT` 3s/20s 重试；超过 5 次 `I_HUNT` 未进入 ⇒ 失败 |
| 4 | 集结/迟到判定 | `demon_retreat()`：OCR `O_LATER_ENTER_CHECK` 与「集结」的交集为空 ⇒ 迟到，点 `I_ENTER_FIRE` 直接开战；否则等 `I_DEMON_GATHER` 消失后开战 |
| 5 | 战斗（三轮） | `run_demon_battle`：点准备 `I_PREPARE_HIGHLIGHT`（轮间会重现）→ `battle_wait`：`I_WIN` 胜、`I_FALSE` 败、轮间准备续点、3min 超时延长 |
| 6 | 领奖 | `I_PRAY` 祈福 / `I_HUNT` / `I_REWARD_ALL` 一键领取 → `ui_reward_appear_click` 点奖励；`I_RANK_LSIT` ⇒ 无奖励可领 |
| 7 | 结束 | 回庭院；`custom_next_run` 下周六 / 失败标记 |

## 2. oas-core 映射与覆盖

| 上游行为 | 本迁移实现 | 状态 |
|---|---|---|
| 庭院→阴阳寮 | `click(I_MAIN_GOTO_GUILD)` + 守卫 `I_CHECK_GUILD` | ✅ |
| 进退治循环 | `loop 10`：`I_SHRINE`/`I_HUNT`/`I_QUIT_BACK` 点击；`I_HUNT_CHECK or I_DEMON_GATHER` ⇒ break | ✅ |
| 集结/迟到 | `if appear(I_DEMON_GATHER)` ⇒ `wait_until(not appear(...))` 等集结结束；否则循环等 `I_ENTER_FIRE` 点击 | ✅ |
| 三轮战斗 | `battle_cycle(rounds=100)`：内部每次循环都点轮间 `I_PREPARE_HIGHLIGHT`，`I_WIN`/`I_FALSE` 处理，回到退治页即结束 | ✅ |
| 领奖 | `loop 10`：`I_REWARD_ALL`⇒点击+点 `I_UI_REWARD`+break；`I_PRAY`/`I_HUNT` 可点时点；`I_RANK_LSIT` ⇒ 无奖励 break | ✅ |
| 周六窗口/时间判断 | 不迁移（scheduler.json windows） | 不迁移 |
| 御魂切换/绿标/预设 | 未迁移（配置项） | 不迁移 |

## 3. Flow 结构要点

- `appear(I_HUNT_CHECK)` 作为退治页断言；若当前在 `I_DEMON_GATHER` 集会态也放行。
- 领奖循环里 `I_REWARD_ALL` 的点击带 `I_UI_REWARD` 收尾，覆盖 `ui_reward_appear_click`。

## 4. 素材（全部为全局 `assets/assets.json`）

所有引用素材都是全局 `assets/assets.json` 的 id（本任务不新增素材、无 images/）。导入语义：`template` 的 `roi` 是旧仓搜索窗（roiBack），点击落在**匹配到的模板外接框**；`region` 是旧仓 RuleClick 的精确操作区域；OCR 资产为 `ocr_lines`（旧 RuleOcr）。

- `庭院_探索灯笼`
- `legacy_GameUi_I_MAIN_GOTO_GUILD`
- `legacy_GameUi_I_CHECK_GUILD`
- `legacy_DemonRetreat_I_SHRINE`
- `legacy_DemonRetreat_I_HUNT`
- `legacy_DemonRetreat_I_HUNT_CHECK`
- `legacy_DemonRetreat_I_DEMON_GATHER`
- `legacy_DemonRetreat_I_QUIT_BACK`
- `legacy_DemonRetreat_I_ENTER_FIRE`
- `legacy_DemonRetreat_I_PRAY`
- `legacy_DemonRetreat_I_REWARD_ALL`
- `legacy_DemonRetreat_I_RANK_LSIT`
- `legacy_GlobalGame_I_UI_REWARD`
- `legacy_GlobalGame_I_UI_BACK_{RED,YELLOW}`
- `legacy_Component_GeneralBattle_{I_PREPARE_HIGHLIGHT,I_BATTLE_INFO,I_WIN,I_FALSE,I_GREED_GHOST,I_OVER_GHOST,I_REWARD}`

## 5. 差异 / 待办

1. 无实机/回放证据（周六活动期限制）
2. 周六日期判断与下次排期属调度器职责，未在 flow 内实现
3. 御魂切换/绿标/预设配置未迁移
