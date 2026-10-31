# FrogBoss（呱太竞猜）

> 移植自 `runhey/OnmyojiAutoScript@dev`（commit `8b4dc66`）。行为/差异见 [migration/specs/FrogBoss.md](../../migration/specs/FrogBoss.md)。

## 运行

```powershell
cd D:\OAS-dev\oas-core
# 需要 OCR（左右票数）；起始画面=庭院
.\target\release\oasd.exe task lint --tasks-dir tasks --task FrogBoss
.\target\release\oasd.exe run --task tasks\FrogBoss --seed 7 --param strategy=frog_majority `
  --model <rec.onnx> --det-model <det.onnx>
# strategy: frog_majority (默认) | frog_minority | frog_bilibili | always_red | always_blue
#           frog_oas / frog_dashen 需要在线预测数据（不支持，自动回退 majority 并告警）
```

## 流程

| # | 行为 | 守卫 |
|---|---|---|
| 1 | 庭院 → 右侧活动 → 呱太 | `Component_RightActivity.I_TOGGLE_BUTTON` → `I_FROG_BOSS_ENTER` → `I_FROG_CHECK` |
| 2 | 状态循环（最多 40 轮） | 已下注 `I_BETTED` / 休息 `I_FROG_BOSS_REST` / 成功 `I_BET_SUCCESS` / 失败 `I_BET_FAILURE` / 可下注 `I_BET_LEFT`+`I_BET_RIGHT` |
| 3 | 下注 | 按 `strategy` 用 OCR 票数比较选左右，点 `I_BET_LEFT`/`I_BET_RIGHT` |
| 4 | 确认 | 金袋说明 `I_GOLD_30_CHECK`→`C_REWARD_2`；选 30 勾玉（`I_GOLD_30` 上部 region）；提交 `I_BET_SURE` 最多 3 次 |
| 5 | 竞猜成功 | 点 `I_BET_SUCCESS_BOX`/`I_REWARD`/`I_NEXT_COMPETITION` 直到下一局 |
| 6 | 竞猜失败 | 点 `I_NEXT_COMPETITION` 到消失 |
| 7 | 退出 | 连点返回直到庭院 |

## 退出

| exit | result | 场景 |
|---|---|---|
| done | success | 已下注/正常结束 |
| skipped | success | 呱太休息中 |
| failed | retryable | 超时/异常 |

## 参数

| key | 默认 | 说明 |
|---|---|---|
| strategy | frog_majority | 下注策略；oas/dashen 回退 majority |

## 素材

`legacy_FrogBoss_*`（28）+ 新导入 `legacy_Component_RightActivity_*`（活动入口）+ `legacy_Component_GeneralBattle_*`（领奖/奖励区）+
`legacy_GlobalGame_*`（确认/返回）+ `legacy_GameUi_*`（庭院守卫）。全部全局素材。

## 已知限制

Oas/Dashen 在线预测策略 ⛔（无网络/指纹/jsonl 原语，回退 majority）；时间窗调度交给 scheduler；无实机证据。
