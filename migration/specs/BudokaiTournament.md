# BudokaiTournament（斗技大会/活动爬塔）迁移规格

- 源 `runhey/OnmyojiAutoScript@dev` commit `8b4dc66`；只读 `_verify/remote-oas`
- 结论：**daily_training（每日修行）与 cultivation_drills（修行练武）主链路已表达**；御魂切换/随机休整/时间上限未迁移，状态 **🔄**。

## 1. 源文件

| 文件 | 作用 |
|---|---|
| `tasks/BudokaiTournament/script_task.py` | 两种模式 run_sequence=[daily_training,cultivation_drills]；start_battle、battle_wait、领奖、门票判定 |
| `tasks/BudokaiTournament/config.py` | DailyTraining：limit_daily_training(200)/limit_cultivation_drills(30)/active_souls_clean/random_sleep；SwitchSoul/GeneralBattle 配置 |
| `tasks/BudokaiTournament/assets.py` | 5 素材：C_BOSS_DETAIL_CLOSE、I_IMG1(搜寻)/I_IMG2/I_IMG3(开始挑战)/I_IMG4(继续挑战) |
| `tasks/ActivityShikigami/assets.py` + `page.py` | 共用：I_TO_BATTLE_MAIN(_2)、I_CHECK_BATTLE_MAIN、I_TO_BATTLE_BOSS、I_CHECK_BATTLE_BOSS、O_FIRE、O_REMAIN_AP/BOSS、I_LOCK/I_UNLOCK |
| `tasks/Component/GeneralBattle/*`、`RightActivity`、`SwitchSoul` | 战斗/入口/御魂 |

## 2. 入口 / 出口

- 入口：庭院 → 右侧活动（`Component_RightActivity.I_TOGGLE_BUTTON`）→ `ActivityShikigami.I_SHI` → 爬塔主界面 `I_TO_BATTLE_MAIN`。
- 出口：`done`（达到 max_battles）、`no_resource`（门票/次数 0）、`no_target`（预留 blocked）、`failed`。
- 收尾：返回按钮回爬塔主界面 → `GameUi.I_BACK_Y` → 庭院。

## 3. 行为矩阵

| # | 状态 | 判定 | 行为 |
|---|---|---|---|
| S1 | 每日修行入口 | `ActivityShikigami.I_TO_BATTLE_MAIN(_2)` | 点到 `I_CHECK_BATTLE_MAIN` |
| S2 | 每日修行循环 | `O_FIRE`（挑战）、`O_REMAIN_AP`（门票） | 门票=0 → `no_resource`；挑战→通用战斗→计数 |
| S3 | 挑战按钮变体 | `BudokaiTournament.I_IMG3`(开始挑战)/`I_IMG4`(继续挑战) | 出现则点击 |
| S4 | 修行练武入口 | `ActivityShikigami.I_TO_BATTLE_BOSS` | 点到 `I_CHECK_BATTLE_BOSS` |
| S5 | 修行练武循环 | `I_IMG1`(搜寻)、`I_IMG3/I_IMG4`(详情)、`O_REMAIN_BOSS` | 搜寻→详情→挑战→战斗→计数；`C_BOSS_DETAIL_CLOSE` 关闭详情 |
| S6 | 领奖/弹窗 | `I_UI_REWARD`、`I_UI_CONFIRM(_SAMLL)` | 循环点掉 |
| S7 | 退出 | 返回按钮 | 回庭院 |

## 4. 参数

| 旧配置 | 新 params | 说明 |
|---|---|---|
| `run_sequence`（默认 daily_training,cultivation_drills） | `mode`（string，默认 daily_training） | 两模式二选一；旧仓顺序跑两种，可用调度条目拆两次 |
| `limit_daily_training`（200） | `max_battles`（int，默认 200） | 次数上限 |
| `limit_cultivation_drills`（30） | 未映射 | 练武模式次数沿用 max_battles（可不同调度条目传参） |
| `random_sleep` / `active_souls_clean` | 未映射 | 无随机睡眠/跨任务调度原语 |
| `switch_soul_config` / `general_battle`（preset/green/anti-detect） | 未映射 | 御魂/预设/绿标无原语 |

## 5. 差异 / 未覆盖（🔄 TODO）

1. **每日修行与练武不能在同一 run 内顺序跑**：旧仓 `run_sequence` 会先跑 daily_training 再 cultivation_drills；DSL 无「模式切换」原语，改为 `mode` 参数（调度器可配两个条目）。
2. **御魂/预设切换（SwitchSoul + preset team）未迁移**：无 DSL 原语。
3. **battle_wait_daily_training 的 270s 超长战斗处理/随机点击**：新 flow 用通用战斗样板（点继续直到回挑战页），不检测"超长战斗"。
4. **O_FIRE keyword 丢失**：导入后 `appear(O_FIRE)` 仅判断 ROI 有无文字（同 ActivityShikigami）。
5. **limit_time / random_sleep / active_souls_clean（清御魂）** 未迁移。
6. **`I_IMG2`（自己发现）** 未使用（旧仓在练武模式点击关闭弹窗，新 flow 未复刻）。
7. 无实机/replay 证据（模拟器 offline）。
