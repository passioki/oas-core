# FallenSun（日轮之陨）

> 移植自 `runhey/OnmyojiAutoScript@dev`（commit `8b4dc66`）。行为/差异见 [migration/specs/FallenSun.md](../../migration/specs/FallenSun.md)。
> 当前实现 **单人模式（alone）**；队长/队员组队模式受 DSL 架构限制未迁移。

## 运行

```powershell
cd D:\OAS-dev\oas-core
.\target\release\oasd.exe task lint --tasks-dir tasks --task FallenSun
.\target\release\oasd.exe run --task tasks\FallenSun --seed 7 --param max_battles=30
# --param lock_team=true 锁定阵容；--param mode=alone（默认）
```

## 流程

| # | 行为 | 守卫 |
|---|---|---|
| 1 | 庭院→探索→御魂 | `I_MAIN_GOTO_EXPLORATION` → `I_EXPLORATION_GOTO_SOUL_ZONE` |
| 2 | 御魂→日轮之陨 | 循环点 `FallenSun.I_FALLEN_SUN` → 等 `I_FORM_TEAM` |
| 3 | 阵容锁定 | `I_FALLEN_SUN_LOCK`/`_UNLOCK`，按 `lock_team` |
| 4 | 主循环 | 猫咪奖励 `I_PET_PRESENT`→`C_WIN_3`；`I_FALLEN_SUN_FIRE` 挑战；通用战斗；计数 |
| 5 | 退出 | `I_UI_BACK_BLUE`→御魂→`I_UI_BACK_YELLOW`→探索→庭院 |

## 退出

| exit | result | 场景 |
|---|---|---|
| done | success | 正常打完/达到 max_battles |
| no_resource | resource | 预留 |
| failed | retryable | 超时/异常 |

## 参数

| key | 默认 | 说明 |
|---|---|---|
| max_battles | 30 | 战斗次数上限（旧 limit_count=50 收敛） |
| lock_team | false | 阵容锁定 |
| mode | alone | 仅 alone 实现 |
| layer | "" | 未参与（RuleList 未导入，见 spec） |

## 已知限制

⛔ 队长/队员组队模式（创建房间/邀请/接受邀请/等待队友）DSL 无原语；层数选择 RuleList 未导入且源枚举乱码；limit_time 无时钟；御魂切换未迁移；无实机证据。
