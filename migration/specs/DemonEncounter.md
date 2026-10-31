# DemonEncounter（逢魔之时）迁移规格

- 源：`runhey/OnmyojiAutoScript@dev` commit `8b4dc66`；只读 `_verify/remote-oas`
- 结论：**逢魔首领（普通/极）战斗简化链路与灯笼填充/红达摩奖励已表达**；
  灯笼 6 类型处理、时间窗、智能伤害/准备配置、挑战次数三元组等未完整迁移，状态 **🔄**。

## 1. 源文件

| 文件 | 作用 |
|---|---|
| `tasks/DemonEncounter/script_task.py`（629 行） | run：时间窗 check_time→换御魂→现世逢魔→今日次数→灯笼（4 处，6 类处理）→逢魔首领（搜索/集结/人数满 300 处理/准备/战斗/480s 等待/回主界面） |
| `tasks/DemonEncounter/config.py` | BossType 枚举（7 种逢魔 boss）、BoxBuyConfig、BestDemonBossSelect、DemonConfig/BestDemonConfig 御魂（enable + group,team）、DemonBattleConfig（preset/lock/green） |
| `tasks/DemonEncounter/assets.py` | 62 素材：O_DE_CHALLENGE_COUNT/O_DE_COUNTER/O_DE_BOSS_PEOPLE/O_DE_BEST_BOSS_PEOPLE；I_DE_FIND/I_DE_LOCATION/I_DE_BOX_CENTER/I_DE_BOSS(_BEST)/C_DM_BOSS_CLICK/I_BOSS_FIRE(_BEST)/I_BOSS_CONFIRM/I_BOSS_GATHER/I_BOSS_DONE_CHECK/I_BOSS_WAIT；灯笼类型 I_DE_BOX/I_DE_LETTER/I_DE_MYSTERY/I_DE_REALM/I_DE_FIND_BOSS/I_DE_DEFEAT_1..4 |
| `tasks/Component/GeneralBattle/*`、`SwitchSoul` | 战斗/御魂 |

## 2. 入口/出口

- 入口：`page_demon_encounter_realworld`（现世逢魔）。新 flow：庭院→町中（`I_MAIN_GOTO_TOWN`）→逢魔（`I_TOWN_GOTO_DEMON_ENCOUNTER`）→现世逢魔（`I_DEMON_ENCOUNTER_REALWORLD_GOTO`）。
- 出口：`done`/`skipped`（今日挑战次数 0）/`failed`。
- 收尾：连点返回（红/黄/确认小按钮）回町中→庭院。

## 3. 行为矩阵

| # | 状态 | 判定 | 行为 |
|---|---|---|---|
| S1 | 时间窗 | `check_time`（旧仓按时间窗口） | **未迁移**（无时钟）→ 调度窗口 |
| S2 | 今日次数 | `O_DE_CHALLENGE_COUNT`（DigitCounter，首数字=剩余） | 0 → `skipped` |
| S3 | 灯笼填充 | `I_DE_FIND`，`O_DE_COUNTER` | 点 I_DE_FIND 直到 `I_DE_AWARD` 出现 |
| S4 | 红达摩奖励 | `I_DE_AWARD` / `GlobalGame.I_UI_REWARD` | 领奖 |
| S5 | 灯笼 6 类型 | `I_DE_BOX/LETTER/MYSTERY/REALM/FIND_BOSS/`空/战斗 | 各自子流程（宝箱/信件/神秘/结界/挑战/boss） **未迁移** |
| S6 | 找首领 | `I_DE_BOSS`/`I_DE_BOSS_BEST` + `C_DM_BOSS_CLICK` | 点搜索→点地图中央→检测 `I_BOSS_FIRE`/`I_BEST_BOSS_FIRE` |
| S7 | 人数满 300 | `O_DE_BOSS_PEOPLE`/`O_DE_BEST_BOSS_PEOPLE` | 退出重选（**未迁移**） |
| S8 | 进入 boss | `I_BOSS_FIRE`→`I_BOSS_CONFIRM`→`I_BOSS_GATHER` | 集结 |
| S9 | 战斗 | `Component_GeneralBattle.I_PREPARE_HIGHLIGHT` → 通用战斗 | 旧仓 480s 等待/准备前配置 |
| S10 | 结束 | `I_BOSS_DONE_CHECK` / `I_DE_LOCATION` | 回逢魔主界面 → 回庭院 |

## 4. 参数映射

| 旧配置 | 新 params | 说明 |
|---|---|---|
| `boss_type`（周一~周日 7 种） | 未映射 | 御魂配置依赖 boss 类型 |
| `demon_soul_config` / `best_demon_soul_config` | 未映射 | 御魂切换无原语 |
| `demon_battle_config`（preset/lock/green） | 未映射 | 无原语 |
| `box_buy_config.box_buy_sushi` | 未映射 | 宝箱子流程未迁移 |
| `best_demon_*_select` | 未映射 | 极逢魔目标选择未迁移 |

## 5. DSL 映射

- 入口导航：`I_MAIN_GOTO_TOWN` → `I_TOWN_GOTO_DEMON_ENCOUNTER` → `I_DEMON_ENCOUNTER_REALWORLD_GOTO`。
- 次数：`ocr_digit(O_DE_CHALLENGE_COUNT) == 0 → skipped`。
- 灯笼：`loop` 点 `I_DE_FIND` 直到 `I_DE_AWARD`；再点奖励。
- 找 boss：`loop` 搜索按钮 → 点 `C_DM_BOSS_CLICK` → 检测 `I_BOSS_FIRE/I_BEST_BOSS_FIRE`。
- 战斗：`I_BOSS_CONFIRM/GATHER` → `I_PREPARE_HIGHLIGHT` → `battle_until(I_BOSS_DONE_CHECK)`。

## 6. 差异 / 未覆盖（🔄 TODO）

1. **时间窗 check_time**：旧仓只在逢魔时段运行；DSL 无时钟 → 由 scheduler windows 表达。
2. **灯笼 6 类型子流程**（宝箱购买 50 勾玉/信件问答/神秘/结界/普通挑战/boss）未迁移；当前只做「填充灯笼 + 红达摩奖励」。
3. **boss 人数满 300 处理**（`O_*_BOSS_PEOPLE` 三元组 → 退出重选）未迁移。
4. **480s 超长战斗等待/准备前御魂配置/准备后重置计数**未迁移（无时钟/无配置写入）。
5. **御魂切换（7 种 boss 对应配置）**无原语。
6. **`I_DE_LOCATION`/`I_DE_BOX_CENTER`/`I_JADE_50` 交互**简化处理。
7. **无实机/replay 证据**（模拟器 offline）。
