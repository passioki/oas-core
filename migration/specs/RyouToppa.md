# RyouToppa（阴阳寮结界突破 / 寮突）迁移规格

- 源仓库：`runhey/OnmyojiAutoScript` **dev** · commit `8b4dc669b47838f42d283f14aa394844c1963c86` · 只读副本 `D:\OAS-dev\_verify\remote-oas`
- 目标：`oas-core` 统一任务模板（`meta.json` + `flow.json` DSL，全局素材 id `legacy_*`）
- 迁移结论：**默认路径行为已覆盖**（进页 → 寮突开关状态判定 → 管理员开局 → 逐区域进攻 → 资源耗尽/完成出口 → 回庭院）；
  可选「御魂/阵容切换」与「30 分钟时间上限」不在现有 DSL 能力内（见 §10）。

## 1. 读取的源文件

| 文件 | 作用 |
|---|---|
| `tasks/RyouToppa/script_task.py` | 主流程：进入寮突页、寮突状态判定、管理员开局、区域循环、战斗、退出 |
| `tasks/RyouToppa/config.py` | `RyouToppa.raid_config`（limit_time/limit_count/next_ryoutoppa_time/skip_difficult/ryou_access/random_delay）+ general_battle_config + switch_soul_config |
| `tasks/RyouToppa/assets.py` | 65 个素材（8 区域点击框、8×失败/已攻破标记、寮突 tab、开局按钮、锁定阵型等） |
| `tasks/RealmRaid/assets.py` | 共享：`I_FIRE`（进攻按钮）、`I_MEDAL_*`（勋章）等 |
| `tasks/GameUi/page.py` + `default_pages.py` | 页面导航：page_main → page_exploration → page_realm_raid → page_kekkai_toppa |
| `tasks/Component/GeneralBattle/general_battle.py`、`battle_wait.py` | 通用战斗：准备页 → 自动战斗 → 胜利/失败/奖励判定（旧仓用 Component 素材，未导入全局） |
| `tasks/Component/SwitchSoul/switch_soul.py` | 御魂/预设切换（DSL 无对应原语） |
| `tasks/GameUi/navigator.py` | `goto_page` 语义（BFS 路径 + 页面 check 资产） |

## 2. 入口 / 出口

- 入口页：`page_kekkai_toppa`（寮突页签；旧仓 `goto_page(page_kekkai_toppa)`）。
  新 flow 从**庭院**（`legacy_GameUi_I_CHECK_MAIN`）开始：
  庭院 →`I_MAIN_GOTO_EXPLORATION`→ 探索 →`I_EXPLORATION_GOTO_REALM_RAID`→ 个人突破页 →`RyouToppa.I_RYOU_TOPPA`(寮突 tab)→ 寮突页。
- 出口（`meta.exits`）：
  - `done` / success：正常打完（达到 `max_attacks`，或检测到「已攻破」标记）
  - `no_resource` / resource：`O_NUMBER` OCR 显示进攻机会 = 0（旧仓 1 小时后重试）
  - `no_target` / blocked：寮突未开启且无管理员权限（需人工/寮管理开启）
  - `failed` / retryable：连续 3 轮刷新列表仍无任何可攻区域（旧仓会无限 flush，这里收敛为 bounded 退出）
- 结束后统一回庭院（`GlobalGame.I_UI_BACK_RED` → 探索 → `GlobalGame.I_UI_BACK_YELLOW` → 庭院；带 `appear` 容错）。

## 3. 状态机（行为矩阵）

| # | 状态/检测 | 判定素材 | 行为 |
|---|---|---|---|
| S0 | 个人突破页 | `GameUi.I_CHECK_REALM_RAID` | 点 `RyouToppa.I_RYOU_TOPPA` 进入寮突页 |
| S1 | 已 100% 攻破 | `RyouToppa.I_SUCCESS_PENETRATION`（「攻破阴阳寮」） | 直接 `done`（旧仓 `plan_tomorrow_ryoutoppa` 排明天，新引擎由 schedule 负责） |
| S2 | 寮突未开启·可开局 | `I_SELECT_RYOU_BUTTON`（选择阴阳寮） | `open_ryou_raid=true` 时：点选择寮 → `C_SELECT_FIRST_RYOU` 选第一个寮 → `I_START_TOPPA_BUTTON` 开局直到 `I_RYOU_REWARD`；否则 `no_target` |
| S3 | 寮突未开启·无权限 | `I_NO_SELECT_RYOU`（晴明占位） | `no_target`(blocked) |
| S4 | 寮突已开启 | `I_RYOU_REWARD` / `I_RYOU_REWARD_90`（寮击破奖励） | 进入主循环 |
| S5 | 主列表 | `I_TOPPA_RECORD`（底部突破记录） | 逐区域检查/进攻 |
| S6 | 区域已攻破 | `I_AREA_n_FINISHED` / `..._FINISHED_NEW` | 今日目标完成 → `done` |
| S7 | 区域上轮失败 | `I_AREA_n_IS_FAILURE` / `..._IS_FAILURE_NEW` | 跳过该区域（`skip_difficult` 语义） |
| S8 | 区域可打 | 点击 `C_AREA_n` 后出现 `RealmRaid.I_FIRE`（「进攻」） | 点进攻 → 准备页 → 自动战斗 |
| S9 | 无进攻机会 | `O_NUMBER` = 机会数（DigitCounter） | `no_resource` |
| S10 | 全区域不可打 | 8 个区域都无 I_FIRE | 上滑刷新列表（2 次 swipe），最多 3 轮 → `failed` |

## 4. 参数映射

| 旧配置 | 新 `meta.params` | 说明 |
|---|---|---|
| `raid_config.limit_count`（默认 50） | `max_attacks`（int，默认 6，1..50） | 旧仓为「本次最多进攻次数」；新引擎一次 run 默认按 6 次（≈票数）收敛，可 `--param max_attacks=N` |
| `general_battle_config.lock_team_enable` | `lock_team`（bool，默认 false） | 决定点 `I_TOPPA_UNLOCK_TEAM`→`I_TOPPA_LOCK_TEAM` 还是反向 |
| `raid_config.ryou_access`（默认 false） | `open_ryou_raid`（bool，默认 false） | 寮管理开局权限 |
| `raid_config.skip_difficult`（默认 true） | 固定行为 | flow 固定「失败标记区域跳过」 |
| `raid_config.random_delay` | 取消 | 新引擎 `click` 自带人类化（钟形/彗尾 + 1~3% 手误），无需任务级随机延迟 |
| `raid_config.limit_time`（30 分钟） | **未映射** | DSL 无时钟/耗时表达式 → 由调度层/预算（`--budget-wall`）兜底（见 §10） |
| `switch_soul_config.*` | **未映射** | 旧仓 `SwitchSoul` 组件（按组/队/OCR 切御魂）；DSL 无 presets/御魂切换原语 |

## 5. 战斗 / 阵容 / 寮逻辑

- **战斗**：旧仓 `run_general_battle` = 准备页点「准备」→ `battle_wait`（检测 `I_WIN`/`I_DE_WIN`/`I_FALSE`/`I_REWARD`/`I_REWARD_GOLD`）→ 点结算继续点 → 回列表。
  新 flow 用通用样板：点 `I_FIRE` → `click(战斗_准备按钮)`（expect vanish）→ `wait_until(战斗_自动按钮/战斗_自动按钮_2)` → loop 点 (640,690) 直到 `I_TOPPA_RECORD` 重新出现 → 计数 +1。
  胜负不判定（旧仓 win/lose 只影响统计，不改流程）；`battle_wait` 的随机滑动/绿标/御魂预设均未迁移。
- **阵容**：仅迁移「锁定/解锁阵型」开关；不替玩家改阵容。
- **寮逻辑**：单寮突场景；`I_SUCCESS_PENETRATION` 表示本寮已 100% 攻破 → 当日完成。
- **区域顺序**：旧仓从 area_index 0 起顺序尝试，失败/不可用则 +1，越界后 flush cache 回 0；新 flow 每轮按 1..8 展开（DSL 无数组/动态素材 id），逻辑等价。

## 6. 资源与限制

- 突破机会来自 `O_NUMBER`（旧 OCR `DigitCounter`，ROI 271,560,48,31）。旧判定：`cu==0 && cu+res==total` 才算无票；
  新 flow 用 `ocr(...,digit) == 0`（`ocr_digit` 取识别文本第一个数字，即 cu），**丢弃 res/total 部分**（见 §10）。
- 旧仓 21:00–05:00 为「无限进攻机会」，`has_ticket` 直接返回 True；新 flow 无时钟 → 该窗口未实现。
- `meta.resources = ["realm_raid_ticket"]`（调度配额/记忆用）。

## 7. 错误语义

- 旧仓大量 `while 1 + screenshot` 重试（旧引擎靠 stuck 检测兜底）；新 flow 每个动作带 `requires`/`expect`：
  - 未进目标页 → 该步响亮失败（不会静默误点）；
  - `I_FIRE` 5 次点不进去 → 旧仓放弃换区域；新 flow `click(I_FIRE)` 失败直接报错（by design：宁可失败不猜）——已记为差距；
  - OCR 失败（无 OCR provider）→ `ocr_digit` 报错（需 `--model/--det-model`）；
  - 战斗准备页 12s 未出现 → 报错。
- 回庭院用 `appear` 容错，避免收尾把已成功的结果变成失败。

## 8. DSL 映射

| 旧行为 | 新 op |
|---|---|
| `goto_page(...)` | `click` + `requires/expect` 页面守卫链 |
| `appear/ appear_then_click` | `if appear(...)` / `click(sel, requires, expect)` |
| `ui_click(a, stop=b)` | `if appear(a) → click(a, expect appear(b))` |
| `O_NUMBER.ocr` | `ocr_digit('legacy_RyouToppa_number')` |
| `self.click(C_AREA_n)` | `click_target([{"kind":"region","sel":"legacy_RyouToppa_C_AREA_n"}])` |
| `swipe_adb(p1,p2,duration)` | `swipe` from/to/ms |
| `while 1` + break | `while`/`loop` + `break` |
| `set_next_run` | `meta.schedule_hint` + `finish(exit)` result 类 |

## 9. 资产（flow 实际引用 66 个：62 个 legacy + 4 个核心战斗素材，全部来自 assets/assets.json 全局库）

| 全局 id | 旧名 | 类型 | ROI | 用途 |
|---|---|---|---|---|
| legacy_GameUi_I_CHECK_MAIN | I_CHECK_MAIN | template | [49,98,1033,74] | 庭院守卫（入口/收尾） |
| legacy_GameUi_I_MAIN_GOTO_EXPLORATION | I_MAIN_GOTO_EXPLORATION | template | [243,100,933,211] | 庭院→探索 |
| legacy_GameUi_I_CHECK_EXPLORATION | I_CHECK_EXPLORATION | template | [1152,148,21,22] | 探索守卫 |
| legacy_GameUi_I_EXPLORATION_GOTO_REALM_RAID | ..._REALM_RAID | template | [33,619,1150,99] | 探索→结界突破 |
| legacy_GameUi_I_CHECK_REALM_RAID | I_CHECK_REALM_RAID | template | [66,526,233,188] | 个人突破页守卫 |
| legacy_GameUi_I_KEKKAI_TOPPA | I_KEKKAI_TOPPA | template | [1065,3,203,61] | 寮突页守卫 |
| legacy_GlobalGame_I_UI_BACK_RED | I_UI_BACK_RED | template | [877,15,383,254] | 寮突→探索返回 |
| legacy_GlobalGame_I_UI_BACK_YELLOW | I_UI_BACK_YELLOW | template | [0,0,100,100] | 探索→庭院返回 |
| legacy_RyouToppa_I_RYOU_TOPPA | ryou_toppa | template | [1161,323,118,193] | 寮突页签 |
| legacy_RyouToppa_I_SUCCESS_PENETRATION | success_penetration | template | [141,374,234,37] | 已 100% |
| legacy_RyouToppa_I_SELECT_RYOU_BUTTON | select_ryou_button | template | [560,577,156,46] | 寮突未开·可开局 |
| legacy_RyouToppa_I_NO_SELECT_RYOU | no_select_ryou | template | [554,180,100,167] | 寮突未开·无权限 |
| legacy_RyouToppa_I_RYOU_REWARD / _90 | ryou_reward / _90 | template | 340x84 / 232x38 | 寮突已开启 |
| legacy_RyouToppa_I_GUILD_ORDERS_REWARDS | guild_orders_rewards | template | [1123,31,115,56] | 开局选择寮页守卫 |
| legacy_RyouToppa_C_SELECT_FIRST_RYOU | select_first_ryou | region | [1148,138,21,22] | 选第一个寮 |
| legacy_RyouToppa_I_START_TOPPA_BUTTON | start_toppa_button | template | [1,1,1055,718] | 开局按钮 |
| legacy_RyouToppa_I_TOPPA_RECORD | I_TOPPA_RECORD | template | [40,606,119,89] | 列表页/战斗结束判定 |
| legacy_RyouToppa_I_TOPPA_LOCK_TEAM / UNLOCK_TEAM | ... | template | 203,602 / 202,603 | 阵型锁定开关 |
| legacy_RealmRaid_I_FIRE | I_FIRE | template | [140,129,1024,584] | 「进攻」按钮 |
| legacy_RyouToppa_C_AREA_1..8 | C_AREA_n | region | 8 个格子 | 选区域 |
| legacy_RyouToppa_I_AREA_n_IS_FAILURE(_NEW) / FINISHED(_NEW) | ×32 | template | 区域标记 | 失败/已攻破状态 |
| legacy_RyouToppa_number | O_NUMBER | ocr_lines | [271,560,48,31] | 进攻机会 OCR |
| 战斗_准备按钮 / 战斗_自动按钮 / _2 | 引擎核心素材 | template | — | 通用战斗样板（跨任务复用，来自 P1 实机标定） |

## 10. 差异 / 未覆盖 / 风险

1. **御魂/阵容切换未迁移**：旧 `SwitchSoul`（`enable_switch`/`enable_switch_by_name`）无 DSL 原语；默认配置为关闭，故默认行为不受影响。TODO：引擎新增「预设/御魂切换」helper（建议以 `call_task` 复用独立 helper 任务）。
2. **时间上限未迁移**：`limit_time=30min` 需要时钟/耗时表达式；由 `--budget-wall` 或调度层窗口兜底。TODO：引擎增加 `elapsed()` 或 run budget 贯通。
3. **无票判定简化**：旧 `cu==0 && cu+res==total` → 新 `ocr_digit==0`；`res`（恢复中）部分丢失，可能提前一轮退出（下一调度轮补打）。
4. **夜间无限机会窗口**（21:00–05:00）未实现：无时钟。表现为：夜间仍按 OCR=0 判定 `no_resource` 退出（行为更保守）。
5. **战斗胜利/失败判定**：旧仓检测 `I_WIN/I_FALSE/I_REWARD`；新 flow 不判定胜负，只等回列表。RyouToppa 本身不使用战斗结果（只计数），故不影响流程；结算界面点 (640,690) 是复用 P1 实机样板。
6. **区域进攻重试**：旧仓「FIRE/区域点击失败 5 次 → 放弃该区域」；新 flow 点 `I_FIRE` 失败即响亮报错。TODO：可改为 `if appear` 后置重试策略（DSL 有 step `retry`，但不能跨多次点击状态机）。
7. **无实机/replay 证据**：当前模拟器 `emulator-5554 offline`，本任务只有 lint 与行为矩阵证据（`_verify` 中无寮突页录制帧）。真机首跑需重点验证：页面导航守卫、`I_TOPPA_RECORD` 判定、`战斗_准备按钮` 在寮突战斗是否同版式。
