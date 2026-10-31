# CollectiveMissions（阴阳寮集体任务）迁移规格

- 源：`runhey/OnmyojiAutoScript@dev` commit `8b4dc669b47838f42d283f14aa394844c1963c86`（只读副本 `_verify/remote-oas`）
- 目标：`oas-core` DSL（全局素材 `legacy_*`）
- 结论：**主链路已表达**（进页 → 30/30 完成判定 → 目标类型刷新 → 3 任务位 OCR 分类 → 按优先级选最优 → 分支执行 → 退出）；
  但「按材料存量选最大材料」「长按追加御魂」「契灵分支的跨任务调度」在现有 DSL 下只能近似/留 TODO，故状态 **🔄**。

## 1. 读取的源文件

| 文件 | 作用 |
|---|---|
| `tasks/CollectiveMissions/script_task.py` | 主流程、OCR 分类、四类任务分支（契灵/喂N卡/材料捐献/御魂捐献）、退出 |
| `tasks/CollectiveMissions/config.py` | `missions_rule`（任务优先级多行串）、`setup_when_bondling`、`missions_select`（默认「觉醒一」） |
| `tasks/CollectiveMissions/assets.py` | 35 个素材：点击框 C_CM_1..3、OCR O_CM_1..6 / O_CM_*_MATTER / O_CM_NUMBER / O_SL_*、长按框 L_SL_LONG、四材料区 |
| `tasks/GameUi/page.py` / `default_pages.py` | `page_guild` 导航与守卫 |
| `tasks/Component/GeneralBattle` | 本任务无战斗 |
| `tasks/BondlingFairyland`（间接） | 契灵任务本体；旧流程会调度它 |

## 2. 入口/出口

- 入口：`page_guild` → `I_CM_SHRINE`（神社）→ `I_CM_CM`（集体任务）→ 等待 `I_CM_RECORDS`。
  新 flow 从庭院出发：庭院→阴阳寮（`GameUi.I_MAIN_GOTO_GUILD`）→ 神社 → 集体任务。
- 出口：
  - `skipped`（success）：今日 30/30 已完成（旧仓 `current==total==30`，直接结束）；
  - `done`（success）：执行了任务/无任务可做；
  - `no_target`（blocked）：最优任务是「契灵探查」但未完成（旧仓转跑 BondlingFairyland 并 2h 后重跑 → 跨任务调度未迁移）；
  - `failed`（retryable）：未识别/流程超时等；
  - 收尾：循环点返回红/黄按钮直到回到神社或庭院，再回庭院。

## 3. 行为矩阵

| # | 状态 | 判定 | 行为 |
|---|---|---|---|
| S1 | 集体任务页 | `I_CM_RECORDS` | 读 `O_CM_NUMBER`（current/remain/total） |
| S2 | 已完成 | `ocr_digit(O_CM_NUMBER) >= 30` | `skipped` |
| S3 | 切换目标 | `ocr(O_CM_1/O_CM_2) == param(mission_select)` | 循环：不匹配则点 `I_CM_SWITCH`（刷新），最多 20 次 |
| S4 | 任务位分类 | 三个位的 `O_CM_1/3/5`(名称) + `O_CM_2/4/6`(等级) | 映射为 BL/AW1..3/GR1..3/SO1..2/FEED/FRIEND |
| S5 | 选最优 | 默认 `missions_rule`：契灵>觉醒三>觉醒二>觉醒一>御灵三>御灵二>御灵一>御魂二>御魂一>远远不够 | OCR 后按 rank 数值比较，最小者胜出 |
| S6 | 契灵探查 BL | `I_CM_REWARDS` 可领 | 领取奖励；否则 `no_target`（旧仓调度 BondlingFairyland） |
| S7 | 喂 N 卡 FEED | `I_FEED_HEAP` → `I_FEED_SUBMIT` | 点两个随机的 `L_FEED_CLICK_*` 区域直到提交按钮出现 → 提交 → 领奖（双倍，两次） |
| S8 | 材料捐献 AW/GR | `I_CM_PRESENT` | 旧仓 OCR 4 种材料取 total 最大者再滑过去；新 flow 只用首个材料近似（TODO） |
| S9 | 御魂捐献 SO | `I_SL_SUBMIT`、`L_SL_LONG` 长按、`O_SL_NUMBER`、`O_SL_LEVEL` | 旧仓长按批量追加直到可提交数 >0；新 flow 用 10 次 region 点击近似（DSL 无长按原语） |
| S10 | 退出 | 神社/庭院 | 连点返回，带守卫 |

## 4. 参数映射

| 旧配置 | 新 params | 说明 |
|---|---|---|
| `missions_select`（默认 觉醒一） | `mission_select`（string，默认 觉醒一） | 目标刷新类型 |
| `missions_rule`（多行优先级串） | 未映射 | 新 flow 固定代码内默认顺序（见 S5）；TODO：字符串参数 + 排序表达式不可行，需引擎支持列表参数 |
| `setup_when_bondling` | 未映射 | 属于 BondlingFairyland 的联动，跨任务调度未迁移 |

## 5. DSL 映射要点

- 分类：`ocr(sel) == '觉醒一'` 等字符串比较 + `set_var/var` 记录 mission/rank；3 个任务位 × 10 类型全部展开。
- 选最优：`if var('r1') <= var('r2') and ...` 选 best 位，`set_var('mission', var('m1'))` 传递类型。
- 区域点击：`click_target([{"kind":"region","sel":"legacy_CollectiveMissions_C_CM_n"}])`。
- 领奖：`if appear(GlobalGame.I_UI_REWARD/I_UI_AWARD/I_UI_CONFIRM) → click` 循环 4 次（旧仓 `ui_reward_appear_click` 的近似）。

## 6. 差异 / 未覆盖（🔄 的 TODO 明细）

1. **材料捐献未选「存量最大」**：旧仓 `O_CM_*_MATTER.ocr` 取 `(curr,remain,total)` 第 3 个数字比较；DSL 表达式只有 `ocr_digit`（首个数字），无「第 N 个数字」。TODO：引擎新增 `ocr_digits(sel)` 返回数组或 `ocr_nth_number(sel, n)`。
2. **长按追加御魂缺失**：`L_SL_LONG` 是 RuleLongClick，导入成 region；DSL 无 `long_press` op。新 flow 用 10 次短点近似（每次 1 个），提交条件 `ocr_digit(O_SL_NUMBER) > 0` 本可作为循环条件但无法保证效率。TODO：引擎 `long_press(sel, ms)`。
3. **契灵分支跨任务调度**：旧仓 `set_next_run(BondlingFairyland, ...)` + 自身 2h 后重跑；新引擎没有「流内调度其它任务」op（调度归 scheduler.json）。当前以 `no_target`(blocked) 结束，由调度层配置先后顺序。TODO：调度器依赖关系/前置任务声明。
4. **刷新可用性检查**：旧仓 `I_CM_SWITCH.match_brightness/mean_color` 判断按钮是否可点；DSL 只有模板匹配。新 flow 用 `appear(I_CM_SWITCH)` 近似 → 若按钮灰显仍可能点（点了无效，循环有 20 次上限）。
5. **双倍奖励两次领取**：旧仓显式 `reward_number>=2`；新 flow 用 4 轮「点奖励/提交按钮」近似，不精确计数。
6. **任务位分类容错**：旧仓去掉 OCR 文本中的 `·` 再比较；新 flow 直接 `ocr()==文字`，遇到噪点会落入 UNKNOWN/FRIEND（旧仓同样会 UNKNOWN）。
7. **无实机/replay 证据**（迁移时模拟器 offline）；OCR 分类与捐献页布局需真机复验。
