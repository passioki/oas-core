# FallenSun（日轮之陨）迁移规格

- 源：`runhey/OnmyojiAutoScript@dev` commit `8b4dc66`；只读 `_verify/remote-oas`
- 结论：**单人模式（ALONE）主链路已表达**；队长（LEADER）/队员（MEMBER）组队模式需要「组队事件机」（创建房间/私密/邀请/接受邀请/等待队友），线性 DSL 无法表达 → **🔄**。

## 1. 读取的源文件

| 文件 | 作用 |
|---|---|
| `tasks/FallenSun/script_task.py` | 4 种身份（leader/member/alone/wild）；进入日轮、层数选择、阵容锁定、战斗循环、退出 |
| `tasks/FallenSun/config.py` | `user_status`(默认 LEADER)、`layer`(默认 叁层)、`limit_time`(30min)、`limit_count`(50)、invite/general_battle/switch_soul |
| `tasks/FallenSun/assets.py` | 8 素材：I_FALLEN_SUN（入口）、I_FORM_TEAM、I_FALLEN_SUN_FIRE、LOCK/UNLOCK、I_PET_PRESENT、L_LAYER_LIST(RuleList)、O_O_TEST_OCR |
| `tasks/Component/GeneralBattle/*` | 通用战斗 |
| `tasks/Component/GeneralRoom/*`、`GeneralInvite/*` | 创建/加入房间、邀请、接受邀请、等待战斗（队长/队员模式） |
| `tasks/Component/SwitchSoul/*` | 御魂切换（未迁移） |
| `tasks/GameUi/page.py` | page_soul_zones（探索→御魂） |

## 2. 入口/出口

- 入口：`page_soul_zones` → `I_FALLEN_SUN`（日轮图标）→ `I_FORM_TEAM`（组队页）。
  新 flow：庭院→探索（`I_MAIN_GOTO_EXPLORATION`）→ 御魂（`I_EXPLORATION_GOTO_SOUL_ZONE`）→ 循环点 `I_FALLEN_SUN` → 等 `I_FORM_TEAM`。
- 出口：`done`（达到 max_battles/正常结束）、`no_resource`（预留）、`failed`（超时）。
- 收尾：日轮页 → 御魂（`I_UI_BACK_BLUE`）→ 探索（`I_UI_BACK_YELLOW`）→ 庭院。

## 3. 行为矩阵（ALONE 模式，已迁移）

| # | 状态 | 判定 | 行为 |
|---|---|---|---|
| S1 | 进入日轮 | `I_FALLEN_SUN` | 点到 `I_FORM_TEAM` 出现 |
| S2 | 层数选择 | `L_LAYER_LIST`（RuleList OCR 列表） | 旧仓 list_find 后点击对应行；**未迁移**（见 §6） |
| S3 | 阵容锁定 | `I_FALLEN_SUN_LOCK` / `_UNLOCK` | 按 `lock_team` 对齐 |
| S4 | 猫咪奖励 | `I_PET_PRESENT` | 点 `C_WIN_3` 奖励区（最多 5 次） |
| S5 | 挑战 | `I_FALLEN_SUN_FIRE` | 点到消失进战斗 |
| S6 | 战斗 | Component_GeneralBattle | `battle_until(I_FALLEN_SUN_FIRE)` |
| S7 | 计数/退出 | `max_battles` | 循环直到上限，回庭院 |

## 4. 参数映射

| 旧配置 | 新 params | 说明 |
|---|---|---|
| `user_status`（LEADER/MEMBER/ALONE/WILD） | `mode`（string，默认 alone） | 仅 alone 实现；leader/member 见 §6；wild 旧仓也未实现 |
| `layer`（壹/贰/叁/日蚀） | `layer`（string，默认 ""） | 未参与选择逻辑（RuleList 未导入 + 源文件层名乱码） |
| `limit_count`（50） | `max_battles`（int，默认 30，1..50） | 单次 run 战斗次数上限 |
| `limit_time`（30min） | 未映射 | 无时钟；由 run budget/调度兜底 |
| `lock_team_enable` | `lock_team`（bool，默认 false） | 阵容锁定 |
| `invite_config` / `general_battle_config` / `switch_soul` | 部分 | 邀请/房间配置属 leader/member；御魂未迁移 |

## 5. DSL 映射

- 进入：`click(I_FALLEN_SUN)` 循环 + `wait_until I_FORM_TEAM`。
- 战斗：`click(I_FALLEN_SUN_FIRE)` → `battle_until(I_FALLEN_SUN_FIRE)`。
- 猫咪奖励：`if appear(I_PET_PRESENT) → click_target(region C_WIN_3)`。
- 锁定：`if appear(UNLOCK) → click(UNLOCK, expect appear(LOCK))` 或反向。

## 6. 差异 / 未覆盖（🔄 TODO）

1. **⛔ 架构：LEADER/MEMBER 组队模式**：需要 `create_room` / `ensure_private` / `create_ensure` / `run_invite` / `check_and_invite` / `check_then_accept` / `wait_battle` / `exit_room` / `exit_team` 等组队事件机；DSL 无「等待其它玩家/接受邀请弹窗/房间生命周期」原语。当前只实现 ALONE。
2. **层数选择未迁移**：`L_LAYER_LIST` 是 RuleList（未导入），且远端 checkout 中 `Layer` 枚举字符串为乱码（GBK→UTF-8 转码问题），无法确认真实层名文本；flow 保持当前层。
3. **limit_time（30min）**：无时钟表达式；由 `--budget-wall`/调度兜底。
4. **switch_soul 御魂切换**未迁移（无 DSL 原语）。
5. **WILD 模式**：旧仓 `run_wild` 本身 `logger.error('Wild mode is not implemented')`，不迁移。
6. **无实机/replay 证据**（模拟器 offline）。
