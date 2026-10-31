# RealmRaid（结界突破·个人 / 个人突破）

> 移植自 `runhey/OnmyojiAutoScript@dev`（commit `8b4dc66`）。行为抽象与差异见 [migration/specs/RealmRaid.md](../../migration/specs/RealmRaid.md)。
> 注：本仓库另有一个早期手写样板 `tasks/个人突破/`（实机验证过的最小版）；本目录是**旧仓完整任务**的迁移版。

## 运行

```powershell
cd D:\OAS-dev\oas-core
# 需要 OCR（票数判定）；起始画面=庭院；模拟器分辨率 1280x720
.\target\release\oasd.exe task lint --tasks-dir tasks --task RealmRaid
.\target\release\oasd.exe run --task tasks\RealmRaid --seed 7 --param max_attacks=9 `
  --model <rec.onnx> --det-model <det.onnx>
```

## 流程

| # | 行为 | 守卫 |
|---|---|---|
| 1 | 庭院→探索 | `GameUi.I_MAIN_GOTO_EXPLORATION` |
| 2 | 探索→个人突破 | `RealmRaid.I_REALM_RAID` → `GameUi.I_CHECK_REALM_RAID` |
| 3 | 呱太活动弹窗 | `I_FROG_RAID` 点到消失；`I_FROG_MEDAL` 仅告警 |
| 4 | 阵容锁定 | `I_UNLOCK`/`I_LOCK`（含 `_2` 变体），按 `lock_team` |
| 5 | 退四打九（可选） | `exit_four=true`：4×(找目标→进攻→`retreat_battle`) |
| 6 | 主循环 | 票数 `O_NUMBER`、弹窗确认、勋章 5>4>3>2>1>0 扫描 |
| 7 | 进攻+战斗 | `I_FIRE` → 通用战斗（`Component_GeneralBattle` 准备/胜利/失败/奖励）→ 回 `I_RR_PERSON` |
| 8 | 三胜奖励 | `I_SOUL_RAID` 出现即点掉（近似旧 O_TEXT 判定） |
| 9 | 无目标 | `I_FRESH` 刷新→`I_UI_CONFIRM`；不可用 → `failed` |
| 10 | 回庭院 | `I_BACK_RED` → 探索 → 庭院 |

## 退出

| exit | result | 场景 |
|---|---|---|
| done | success | 达到 max_attacks / 正常结束 |
| no_resource | resource | 突破券 0 |
| failed | retryable | 无目标且刷新不可用 |
| no_target | blocked | 预留（活动/特殊变体） |

## 参数

| key | 默认 | 说明 |
|---|---|---|
| max_attacks | 9 | 单次最多进攻（旧 number_attack=30 的收敛版） |
| lock_team | false | 锁定阵容（旧 lock_team_enable） |
| refresh_when_empty | true | 无目标时刷新（旧 when_attack_fail=Refresh） |
| exit_four | false | 退四打九（旧 exit_four，默认 true，但旧仓仅第 1 分区触发） |

## 素材

75 个 `legacy_RealmRaid_*` + 新导入 `legacy_Component_GeneralBattle_*`（战斗/退四）+ `legacy_GameUi_*`（导航）+
`legacy_GlobalGame_*`（确认/返回）。全部来自 `assets/assets.json` 全局库，本任务不建 task-local assets。

## 已知限制

见 spec §6：呱太目标判定、partition 索引、three_refresh、票数三元组、三胜奖励精确判定、御魂切换未迁移；无实机证据。
