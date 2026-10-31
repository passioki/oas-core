# DemonEncounter（逢魔之时）

> 移植自 `runhey/OnmyojiAutoScript@dev`（commit `8b4dc66`）。行为/差异见 [migration/specs/DemonEncounter.md](../../migration/specs/DemonEncounter.md)。
> 当前为**部分实现**：今日次数判定 + 灯笼填充/红达摩奖励 + 逢魔首领搜索/集结/通用战斗；灯笼 6 类型子流程与 480s 等待未迁移。

## 运行

```powershell
cd D:\OAS-dev\oas-core
# 需要 OCR（今日挑战次数）；起始画面=庭院
.\target\release\oasd.exe task lint --tasks-dir tasks --task DemonEncounter
.\target\release\oasd.exe run --task tasks\DemonEncounter --seed 7 `
  --model <rec.onnx> --det-model <det.onnx>
```

## 流程

| # | 行为 | 守卫 |
|---|---|---|
| 1 | 庭院→町中→逢魔→现世逢魔 | `I_MAIN_GOTO_TOWN` → `I_TOWN_GOTO_DEMON_ENCOUNTER` → `I_DEMON_ENCOUNTER_REALWORLD_GOTO` |
| 2 | 今日次数 0 → skipped | `ocr_digit(O_DE_CHALLENGE_COUNT) == 0` |
| 3 | 灯笼填充 + 红达摩 | `I_DE_FIND` 循环直到 `I_DE_AWARD`；奖励点击 |
| 4 | 找首领 | `I_DE_BOSS`/`I_DE_BOSS_BEST` → `C_DM_BOSS_CLICK` → `I_BOSS_FIRE`/`I_BEST_BOSS_FIRE` |
| 5 | 集结+战斗 | `I_BOSS_CONFIRM`/`I_BOSS_GATHER` → `I_PREPARE_HIGHLIGHT` → 通用战斗 → `I_BOSS_DONE_CHECK` |
| 6 | 回庭院 | 返回链 |

## 退出

| exit | result | 场景 |
|---|---|---|
| done | success | 流程完成 |
| skipped | success | 今日挑战次数 0 |
| failed | retryable | 12 次尝试未找到/进入 boss |

## 已知限制

灯笼 6 类型（宝箱/信件/神秘/结界/挑战/boss）未迁移；时间窗/480s 等待/人数满 300 重选/御魂切换未迁移；无实机证据。
