# ActivityShikigami（活动爬塔）迁移规格

- 源 `runhey/OnmyojiAutoScript@dev` commit `8b4dc66`；只读 `_verify/remote-oas`
- 结论：**pass（门票）模式主链路已表达**；ap / ap100 / boss 模式未迁移（各自有独立入口/奖励/切换逻辑），状态 **🔄**。

## 1. 源文件

| 文件 | 作用 |
|---|---|
| `tasks/ActivityShikigami/script_task.py` | 4 种爬塔模式（pass/ap/ap100/boss）、循环、限次/限时、模式切换、退出 |
| `tasks/ActivityShikigami/page.py` | 页面定义：`page_climb_act`=I_TO_BATTLE_MAIN；入口 `[I_SHI, RightActivity.I_TOGGLE_BUTTON]` |
| `tasks/ActivityShikigami/config.py` | GeneralClimb：limit_time 30min / pass_limit 50 / ap_limit 300 / boss_limit 20 / ap100_limit 20 / run_sequence "pass,ap100,boss,ap" |
| `tasks/ActivityShikigami/assets.py` | 39 素材：PASS_1..9 剧情/奖励、挑战 OCR O_FIRE、门票 O_REMAIN_PASS/AP/BOSS、红退出/跳过 |
| `tasks/Component/GeneralBattle`、`RightActivity`、`SwitchSoul` | 战斗 / 右侧活动入口 / 御魂 |

## 2. 入口 / 出口

- 入口：庭院 → `Component_RightActivity.I_TOGGLE_BUTTON` → `ActivityShikigami.I_SHI` → `I_TO_BATTLE_MAIN`。
- 出口：`done`（达到 max_battles）、`no_target`（非 pass 模式未实现）、`no_resource`（预留）、`failed`。
- 收尾：连点返回（`I_UI_BACK_YELLOW/CIRCLE/RED`）回爬塔主界面 → `GameUi.I_BACK_Y` → 庭院。

## 3. 行为矩阵（pass）

| # | 状态 | 判定 | 行为 |
|---|---|---|---|
| S1 | 爬塔主界面 | `I_TO_BATTLE_MAIN` / `I_CHECK_BATTLE_MAIN` | 已进入则继续 |
| S2 | 跳过剧情/领奖 | `I_UI_CONFIRM(_SAMLL)` / `I_TO_BATTLE_MAIN_2` / `I_PASS_3` / `I_PASS_1/PASS_2` / 点击 (630,360) | 直到 `I_CHECK_BATTLE_MAIN` 出现 |
| S3 | 占领奖励 | `I_PASS_5..7,9`（PASS_8/10/11 素材缺失被导入器跳过） | 循环点击直到挑战界面 |
| S4 | 挑战 | `O_FIRE`（OCR「挑战」区域） | 点击进战斗 |
| S5 | 战斗 | Component_GeneralBattle | `battle_until(I_CHECK_BATTLE_MAIN)` |
| S6 | 限次 | `max_battles` | 达到即结束 |

## 4. 参数

| 旧配置 | 新 params | 说明 |
|---|---|---|
| `run_sequence`（pass,ap100,boss,ap） | `mode`（string，默认 pass） | 仅 pass 实现 |
| `pass_limit`（50） | `max_battles`（int，默认 50） | 次数上限 |
| `limit_time`（30min） | 未映射 | 无时钟 |
| `switch_soul_config` / `general_battle` | 未映射 | 御魂切换无原语；lock/green/anti-detect 未迁移 |

## 5. 差异 / 未覆盖（🔄 TODO）

1. **ap / ap100 / boss 模式未迁移**：ap 用 O_REMAIN_AP 体力判定 + O_ENTER_AP100/I_TO_BATTLE_BOSS 等独立入口与页面跳转；ap100 有 O_REMAIN_AP100；boss 有 boss 挑战次数与 `I_CHECK_BATTLE_BOSS`。需要逐模式补 flow（素材已在全局库）。
2. **O_FIRE 关键词丢失**：旧 `RuleOcr(keyword='挑战')`，导入为 ocr_lines 后 `appear(O_FIRE)` 仅判断 ROI 内有无文字；建议真机标定后改用模板或 ocr 文本比较。
3. **PASS_8/10/11 素材缺失**：旧仓 `fire_pass_8.png` 等图片在导入时被跳过（图不存在），flow 仅用 PASS_5/6/7/9。
4. **剧情跳过/对话点击序列**：旧仓用多尺度/中心点击循环；新 flow 为简化序列，可能需真机调参。
5. **limit_time / active_souls_clean / random_sleep**：无时钟/无跨任务调度/无随机睡眠原语。
6. **switch_soul 御魂切换**未迁移。
7. 无实机/replay 证据（模拟器 offline）。
