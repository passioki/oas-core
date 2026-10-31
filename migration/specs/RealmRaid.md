# RealmRaid（结界突破·个人）迁移规格

- 源：`runhey/OnmyojiAutoScript@dev` commit `8b4dc66`（只读 `_verify/remote-oas`）
- 目标：`oas-core` DSL + 全局素材（`legacy_RealmRaid_*` / `legacy_Component_GeneralBattle_*` / `legacy_GameUi_*`）
- 结论：**个人突破主循环已表达**（进页 → 呱太弹窗 → 锁阵容 → 按勋章优先级扫描 → 进攻 → 通用战斗 → 三胜奖励 → 票数/刷新出口 → 回庭院）；
  呱太目标特殊判定与「退四打九」的最初分区触发条件未完全迁移，状态 **🔄**。

## 1. 读取的源文件

| 文件 | 作用 |
|---|---|
| `tasks/RealmRaid/script_task.py` | 主循环 run_2：票数、找目标 find_one、进攻 fire、通用战斗、奖励/刷新/失败策略、呱太判定 |
| `tasks/RealmRaid/config.py` | `RaidConfig`：number_attack(30)/number_base/exit_four/order_attack(5>4>3>2>1>0)/three_refresh/when_attack_fail |
| `tasks/RealmRaid/assets.py` | 75 素材（9 分区点击框、勋章 I_MEDAL_0..5、票数 OCR O_NUMBER、刷新/锁定/呱太/三胜奖励） |
| `tasks/Component/GeneralBattle/general_battle.py`、`battle_wait.py` | `run_general_battle` / `run_general_battle_back`（退四） |
| `tasks/Component/SwitchSoul/switch_soul.py` | 御魂/预设切换（DSL 无原语） |
| `tasks/GameUi/page.py`、`default_pages.py` | page_realm_raid 导航（探索→个人突破） |

## 2. 入口 / 出口

- 入口：`page_realm_raid`（个人突破页）。新 flow：庭院→探索（`I_MAIN_GOTO_EXPLORATION`）→ `RealmRaid.I_REALM_RAID`（探索底部图标）→ expect `GameUi.I_CHECK_REALM_RAID`。
- 出口：
  - `done`(success)：达到 `max_attacks` 或正常收尾；
  - `no_resource`(resource)：`O_NUMBER` 当前票数 = 0（旧 `cu==0 && cu+res==total`，新只用第一个数字）；
  - `failed`(retryable)：无目标可打且刷新不可用（旧 `check_refresh` 返回 False / `when_attack_fail=EXIT`）；
  - `no_target`(blocked)：预留（呱太/活动变体等需要人工时）。
- 收尾：`I_BACK_RED`（个人页→探索）→ `GlobalGame.I_UI_BACK_YELLOW`（探索→庭院）。

## 3. 行为矩阵

| # | 状态 | 判定 | 行为 |
|---|---|---|---|
| S1 | 呱太活动弹窗 | `I_FROG_RAID` | 循环点到消失 |
| S2 | 呱太活动标记 | `I_FROG_MEDAL` | 记录 warn（特殊目标判定 TODO，见 §6） |
| S3 | 阵容锁定 | `I_UNLOCK/I_UNLOCK_2` vs `I_LOCK/I_LOCK_2` | 按 `lock_team` 点一次解锁/锁定（旧 `ensure_lock` 近似） |
| S4 | 票数 | `ocr_digit(O_NUMBER)` | 0 → `no_resource` |
| S5 | 找目标 | 勋章模板 `I_MEDAL_5..0`（优先级 5>4>3>2>1>0，旧 `order_attack`） | 命中即点击该勋章 |
| S6 | 进攻 | `I_FIRE`（挑战按钮） | 点击后进入通用战斗 |
| S7 | 战斗 | `GeneralBattle.I_PREPARE_HIGHLIGHT` / `I_WIN` / `I_DE_WIN` / `I_FALSE` / `I_REWARD(_GOLD)` | 点准备 → 自动战斗 → 点胜利/奖励区 → 直到 `I_RR_PERSON` 回个人页 |
| S8 | 三胜奖励 | `I_SOUL_RAID` | 战斗后循环点掉聊天气泡/奖励直到回个人页 |
| S9 | 无目标 | `I_FRESH` 刷新按钮 | 有则点刷新→确认；无/关闭 → `failed` |
| S10 | 退四打九 | `param('exit_four')` | 4 次：找目标→进攻→`retreat_battle`（`I_EXIT`→确认→`I_FALSE`）|
| S11 | 收尾 | `I_BACK_RED` | 返回探索→庭院 |

## 4. 参数映射

| 旧配置 | 新 params | 说明 |
|---|---|---|
| `number_attack`（默认 30） | `max_attacks`（int，默认 9，1..30） | 单次 run 最多进攻次数（默认值按一页 9 个目标收敛） |
| `lock_team_enable`（默认 false） | `lock_team`（bool，默认 false） | 阵容锁定 |
| `when_attack_fail`（默认 Refresh） | `refresh_when_empty`（bool，默认 true） | 无目标时是否刷新；false/false-CD → failed |
| `exit_four`（默认 true） | `exit_four`（bool，默认 false） | 退四打九（显式 opt-in；旧仓仅目标落在第 1 分区时触发） |
| `order_attack`（5>4>3>2>1>0） | 固定顺序 | flow 内展开 6 档勋章优先级；配置串未映射 |
| `three_refresh` | 未映射 | 三胜后刷新未迁移（TODO） |
| `number_base`（票数基准） | 未映射 | 需要 res/total 两个数字（DSL 只有第一个） |
| `switch_soul_config` | 未映射 | 无御魂切换原语 |

## 5. DSL 映射

- 找目标：`for medal in 5..0` 展开 `if var('attacked')==0 and appear(I_MEDAL_n)` → `click(I_MEDAL_n)`；旧 `find_one` 用 `ImageGrid.find_anyone` + 分区中心判定 index，新 flow 直接点命中的勋章（等价于「点这个目标」），放弃 partition 索引。
- 进攻：`if appear(I_FIRE) → click(I_FIRE)`（旧 `fire()` 循环点 进攻/分裂/分区，新 flow 以 `I_FIRE` 为唯一进攻入口，失败则响亮 warn 并继续下一档）。
- 战斗：`battle_until(I_RR_PERSON)`（见 §3 S7，素材来自新导入的 `Component_GeneralBattle`）。
- 退四：`retreat_battle(I_RR_PERSON)`（`I_EXIT` → `GlobalGame.I_UI_CONFIRM` → `I_FALSE`）。
- 票数：`ocr_digit('legacy_RealmRaid_O_NUMBER')`。
- 刷新：`I_FRESH` → `I_UI_CONFIRM`，`await` 用 expect。

## 6. 差异 / 未覆盖（🔄 TODO）

1. **呱太目标特殊判定**：旧 `is_frog`/`check_medal_is_frog` 用 `I_FROG_MEDAL` + `O_FROG_1..9` OCR==20 判定呱太并临时解锁阵容；新 flow 仅提示「检测到呱太活动」，未实现逐目标判定（需要 OCR 数值==20 + 分区索引）。
2. **partition 索引语义丢失**：旧仓用勋章中心坐标反推 1..9 分区（并用于 exit_four 的第 1 分区触发、`false_roi` 涂抹已失败目标）；新 flow 直接点勋章，`exit_four` 变显式参数。
3. **`three_refresh`/`when_attack_fail=EXIT|CONTINUE` 未映射**：固定为「无目标→刷新/失败」。
4. **`number_base` 票数基准未映射**：需要 `(cu,res,total)` 三元组；DSL 只有 `ocr_digit`=首数字。
5. **三胜奖励判定简化**：旧仓用 `O_TEXT` 是否含中文 + `\d+/\d+` 判断聊天框遮挡；新 flow 用 `I_SOUL_RAID` 出现即点掉。
6. **御魂/预设切换未迁移**：`SwitchSoul` 组件无 DSL 原语（默认配置关闭）。
7. **无实机/replay 证据**（迁移时模拟器 offline）；勋章模板阈值（0.8）、`I_RR_PERSON` 页守卫、通用战斗素材需真机复验。
