# Hunt 迁移规格

- 远程源：`runhey/OnmyojiAutoScript@dev` · commit `8b4dc669b47838f42d283f14aa394844c1963c86`（只读 `D:\OAS-dev\_verify\remote-oas`）
- 状态：**✅**（两条分支（麒麟 Mon-Thu / 阴界之门 Fri-Sun）均覆盖；时间窗口/御魂切换属调度器/配置职责）
- 已读文件：
  - `tasks/Hunt/script_task.py`
  - `tasks/Hunt/config.py`
  - `tasks/Hunt/assets.py`
  - `tasks/Hunt/hunt/{image,click}.json`
  - `tasks/Hunt/kirin/image.json`
  - `tasks/Hunt/netherworld/image.json`
  - `tasks/GameUi/page.py`
- 共享阅读：`tasks/Component/GeneralBattle/{general_battle.py,battle.py,battle_wait.py}`、`tasks/Component/GeneralRoom/general_room.py`、`tasks/GameUi/default_pages.py`（导航图）

## 1. 上游行为矩阵

| # | 条件 / 输入 | 行为 |
|---|---|---|
| 1 | UTC+8 周一~周四（麒麟日）6:00 起；周五~周日（阴界日）17:00 起 | `check_datetime()` 判定麒麟/阴界；不在时段则 `custom_next_run` 到当日/次日 19:00 |
| 2 | 启动 | 若配置了切换御魂组（`kirin_group_team`/`netherworld_group_team` ≠ -1,-1）→ 式神录切御魂 |
| 3 | 麒麟日 | `goto_page(page_hunt_kirin)` → `kirin()` |
| 4 | 麒麟 `kirin()` | 循环：`I_PREPARE_HIGHLIGHT` 出现 ⇒ 开战；点 `I_UI_CONFIRM` / `I_KIRIN_CHALLAGE` / `I_KIRIN_START_CHALLAGE`；`I_KIRIN_END` ⇒ 今日已挑战，点返回退出 |
| 5 | 麒麟战斗 | 通用战斗 `run_general_battle`（准备→自动战斗→胜负/奖励结算→回御魂页） |
| 6 | 阴界日 | `goto_page(page_hunt)` → `netherworld()` |
| 7 | 阴界 `netherworld()` | 循环：在房间则等 `I_FIRE` 并点（开战）；否则点 `I_NW` / `I_UI_CONFIRM` / `I_NW_CHALLAGE`；`I_NW_DONE` ⇒ 今日已挑战，点 `I_UI_BACK_RED` 退出 |
| 8 | 阴界战斗 | 重写 `battle_wait`：`I_WIN` 处理胜利；`I_FALSE` 失败；战斗中若出现 `I_PREPARE_HIGHLIGHT` 继续点准备（失败后可续战） |
| 9 | 结束 | `plan_tomorrow_hunt()` 安排次日 19:00（麒麟/阴界按明日星期）；`TaskEnd` |

## 2. oas-core 映射与覆盖

| 上游行为 | 本迁移实现 | 状态 |
|---|---|---|
| 麒麟/阴界分支 | 进入狩猎战页后 `if appear(I_CHECK_HUNT_KIRIN)` 走麒麟分支，否则阴界分支 | ✅ |
| goto_page(page_hunt*) | 庭院→町中（`I_MAIN_GOTO_TOWN`）→狩猎战（`I_TOWN_GOTO_HUNT`）→ `wait_until(I_CHECK_HUNT or I_CHECK_HUNT_KIRIN)` | ✅ |
| 麒麟挑战链 | `I_KIRIN_END` → break；点 `I_KIRIN_CHALLAGE` → `I_UI_CONFIRM` → `I_KIRIN_START_CHALLAGE` | ✅ |
| 阴界房间开战 | `I_NW` → `I_UI_CONFIRM` → `I_NW_CHALLAGE` → 循环等 `GeneralInvite_I_FIRE` 点击（素材已导入） | ✅ |
| 通用战斗 | `battle_cycle`：`I_PREPARE_HIGHLIGHT`→`I_BATTLE_INFO`→结算循环（`I_WIN`/`I_FALSE`/轮间准备/贪吃鬼/御魂溢出/奖励/中心点击）→回到 `I_CHECK_HUNT*` | ✅ |
| 返回 | `I_UI_BACK_YELLOW` → 町中 → `I_TOWN_GOTO_MAIN` → 庭院 | ✅ |
| 时间窗/次日排期/御魂切换 | 不迁移（scheduler.json 职责；switch_soul 为配置项） | 不迁移 |
| battle_wait 的随机点击/滑动防封 | 引擎内置 humanize/点击分布（§22.6~22.9），不经 flow | 引擎承担 |

## 3. Flow 结构要点

- `max_battles`（默认 1）控制本次最多挑战次数；次数上限/时间上限由调度器配置承担。
- 战斗结算用 GeneralBattle 资产（prepare/win/false/greed/overflow/reward/battle_info），与上游 `battle_wait` 的判定集合一致。

## 4. 素材（全部为全局 `assets/assets.json`）

所有引用素材都是全局 `assets/assets.json` 的 id（本任务不新增素材、无 images/）。导入语义：`template` 的 `roi` 是旧仓搜索窗（roiBack），点击落在**匹配到的模板外接框**；`region` 是旧仓 RuleClick 的精确操作区域；OCR 资产为 `ocr_lines`（旧 RuleOcr）。

- `庭院_探索灯笼`
- `legacy_GameUi_I_MAIN_GOTO_TOWN`
- `legacy_GameUi_I_TOWN_GOTO_HUNT`
- `legacy_GameUi_I_CHECK_TOWN`
- `legacy_GameUi_I_CHECK_HUNT`
- `legacy_GameUi_I_CHECK_HUNT_KIRIN`
- `legacy_Hunt_I_KIRIN_CHALLAGE`
- `legacy_Hunt_I_KIRIN_START_CHALLAGE`
- `legacy_Hunt_I_KIRIN_END`
- `legacy_Hunt_I_NW`
- `legacy_Hunt_I_NW_CHALLAGE`
- `legacy_Hunt_I_NW_DONE`
- `legacy_Component_GeneralInvite_I_FIRE`
- `legacy_Component_GeneralRoom_I_CREATE_ROOM`
- `legacy_Component_GeneralBattle_{I_PREPARE_HIGHLIGHT,I_BATTLE_INFO,I_WIN,I_FALSE,I_GREED_GHOST,I_OVER_GHOST,I_REWARD}`
- `legacy_GlobalGame_I_UI_CONFIRM`
- `legacy_GlobalGame_I_UI_BACK_YELLOW`

## 5. 差异 / 待办

1. 无实机/回放证据（狩猎战时段限制）
2. 时间窗/次日排期/御魂切换属调度器与玩家配置职责，未在 flow 内实现
3. 阴界之门房间时序（匹配/自动匹配）未实机复核

## 8. Polish round (2026-11-01)

- 入口守卫：`庭院_探索灯笼`（探索灯笼模板，跨庭院皮肤不稳定）→ `legacy_GameUi_I_CHECK_MAIN`；町中→狩猎战的 `requires` 从按钮自身改为 `legacy_GameUi_I_CHECK_TOWN`。
- 麒麟/阴界之门两条分支的战斗段改为「先检测准备页（Component_GeneralBattle_I_PREPARE_HIGHLIGHT 或 战斗_准备按钮）再进入准备/战斗/结算」，避免按钮没点到时无条件 `wait_until(BATTLE_INFO)` 超时导致整任务失败。
- 回庭院/结束标记统一用 `legacy_GameUi_I_CHECK_MAIN`（与远程 page_main 一致）。



