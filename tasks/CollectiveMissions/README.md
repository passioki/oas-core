# CollectiveMissions（阴阳寮集体任务）

> 移植自 `runhey/OnmyojiAutoScript@dev`（commit `8b4dc66`）的旧仓任务。行为抽象/差异见 [migration/specs/CollectiveMissions.md](../../migration/specs/CollectiveMissions.md)。

## 目录

```
tasks/CollectiveMissions/
├─ meta.json     # 统一模板契约
├─ flow.json     # oas-flow DSL（全局 legacy_* 素材引用）
└─ README.md
```

## 运行

```powershell
cd D:\OAS-dev\oas-core
# 需要 OCR 模型：任务位分类/完成度判定都依赖 OCR；起始画面=庭院
.\target\release\oasd.exe task lint --tasks-dir tasks --task CollectiveMissions
.\target\release\oasd.exe run --task tasks\CollectiveMissions --seed 7 --param mission_select=觉醒一 `
  --model <rec.onnx> --det-model <det.onnx>
```

## 流程（flow.json）

| # | 行为 | 守卫/判定 |
|---|---|---|
| 1 | 庭院 → 阴阳寮 | `GameUi.I_MAIN_GOTO_GUILD` → 等 `I_CHECK_GUILD` |
| 2 | 阴阳寮 → 神社 → 集体任务 | `I_CM_SHRINE` → `I_CM_CM` → 等 `I_CM_RECORDS` |
| 3 | 30/30 完成判定 | `ocr_digit(O_CM_NUMBER) >= 30` → `skipped` |
| 4 | 切换到 `mission_select` | 循环最多 20 次：不匹配则点 `I_CM_SWITCH` 刷新 |
| 5 | 三任务位 OCR 分类 | `O_CM_1/3/5` 名称（契灵探查/远远不够/结伴同行）+ `O_CM_2/4/6` 等级（觉醒/御灵/御魂 ×一/二/三） |
| 6 | 选优先级最高 | 默认规则：契灵 > 觉醒三 > 觉醒二 > 觉醒一 > 御灵三 > 御灵二 > 御灵一 > 御魂二 > 御魂一 > 远远不够 |
| 7 | 分支执行 | 喂N卡（`I_FEED_HEAP`/`I_FEED_SUBMIT`）/ 御魂捐献（`I_SL_SUBMIT`+`L_SL_LONG`）/ 材料捐献（`I_CM_PRESENT`）/ 契灵领奖（`I_CM_REWARDS`） |
| 8 | 领奖 | 循环点 `GlobalGame.I_UI_REWARD/I_UI_AWARD/I_UI_CONFIRM` + 提交按钮 |
| 9 | 退出 | 连点 `I_UI_BACK_RED`/`I_UI_BACK_YELLOW` 直到神社/庭院，再回庭院 → `finish(exit)` |

## 退出（meta.exits）

| exit | result | 场景 |
|---|---|---|
| done | success | 已执行/无需执行 |
| skipped | success | 今日 30/30 已完成 |
| no_target | blocked | 最优为「契灵探查」但未完成（需先跑 BondlingFairyland；跨任务调度未迁移） |
| failed | retryable | 流程超时/识别失败 |

## 参数

| key | type | 默认 | 说明 |
|---|---|---|---|
| mission_select | string | 觉醒一 | 刷新到指定任务类型（旧 `missions_select`） |

## 素材

35 个全局素材全部来自 `assets/assets.json`：`legacy_CollectiveMissions_*`（C_CM_1..3 / I_CM_* / O_CM_* / O_SL_* / L_*）+
`legacy_GameUi_*`（导航/守卫）+ `legacy_GlobalGame_*`（返回、奖励弹窗）。

## 已知限制（与 spec §6 一致）

1. 材料捐献**未实现「取存量最大材料」**：DSL 只有 `ocr_digit`（首个数字），旧仓用第 3 个数字 total。→ TODO 引擎 `ocr_nth_number`。
2. 御魂捐献**无长按**：`L_SL_LONG` 用 10 次短点近似。→ TODO 引擎 `long_press`。
3. 契灵分支需要跨任务调度（旧仓 `set_next_run(BondlingFairyland)`）→ 以 `no_target`(blocked) 结束，交由调度器配置顺序。
4. 刷新按钮可用性（旧仓 brightness/mean_color 检查）退化为模板 `appear` 判定。
5. 双倍奖励领取未精确计数（4 轮近似）。
6. 无实机/replay 证据（迁移时模拟器 offline）；OCR 分类与捐献页布局需真机复验。
