# BudokaiTournament（斗技大会/活动爬塔）

> 移植自 `runhey/OnmyojiAutoScript@dev`（commit `8b4dc66`）。行为/差异见 [migration/specs/BudokaiTournament.md](../../migration/specs/BudokaiTournament.md)。

## 运行

```powershell
cd D:\OAS-dev\oas-core
# 需要 OCR（门票/挑战按钮区域）；起始画面=庭院
.\target\release\oasd.exe task lint --tasks-dir tasks --task BudokaiTournament
.\target\release\oasd.exe run --task tasks\BudokaiTournament --seed 7 --param mode=daily_training --param max_battles=200 `
  --model <rec.onnx> --det-model <det.onnx>
# mode: daily_training（默认）| cultivation_drills
```

## 流程

| # | 行为 | 守卫 |
|---|---|---|
| 1 | 庭院→右侧活动→爬塔主界面 | `Component_RightActivity.I_TOGGLE_BUTTON` → `ActivityShikigami.I_SHI` → `I_TO_BATTLE_MAIN` |
| 2 | 每日修行 | 点 `I_TO_BATTLE_MAIN(_2)` → `I_CHECK_BATTLE_MAIN`；门票 `O_REMAIN_AP`；`O_FIRE`/`I_IMG3`/`I_IMG4` 挑战；通用战斗 |
| 3 | 修行练武 | 点 `I_TO_BATTLE_BOSS` → `I_CHECK_BATTLE_BOSS`；`I_IMG1` 搜寻 → `I_IMG3/4` 详情 → 挑战 → 通用战斗；`C_BOSS_DETAIL_CLOSE` |
| 4 | 退出 | 返回按钮 → 爬塔主界面 → `GameUi.I_BACK_Y` → 庭院 |

## 退出

| exit | result | 场景 |
|---|---|---|
| done | success | 达到 max_battles |
| no_resource | resource | 门票/挑战次数 0 |
| no_target | blocked | 预留 |
| failed | retryable | 超时/异常 |

## 参数

| key | 默认 | 说明 |
|---|---|---|
| mode | daily_training | daily_training / cultivation_drills |
| max_battles | 200 | 次数上限（旧 limit_daily_training） |

## 素材

`legacy_BudokaiTournament_*`（5）+ 跨任务 `legacy_ActivityShikigami_*`（共用活动页/门票/挑战）+
`legacy_Component_GeneralBattle_*` / `RightActivity_*` + `legacy_GameUi_*` / `legacy_GlobalGame_*`。

## 已知限制

两种模式不能同 run 顺序执行（用 `mode` 参数 + 调度拆条目）；御魂/预设切换未迁移；超长战斗/随机点击/时间上限未迁移；O_FIRE keyword 丢失；无实机证据。
