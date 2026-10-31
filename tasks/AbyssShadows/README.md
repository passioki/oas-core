# AbyssShadows（狭间暗影）

> 移植自 `runhey/OnmyojiAutoScript@dev`（commit `8b4dc66`）。行为/差异见 [migration/specs/AbyssShadows.md](../../migration/specs/AbyssShadows.md)。

## 运行

```powershell
cd D:\OAS-dev\oas-core
# 起始画面=庭院；需要 OCR（智能伤害时长/前往按钮无需 OCR，但通用战斗需要）
.\target\release\oasd.exe task lint --tasks-dir tasks --task AbyssShadows
.\target\release\oasd.exe run --task tasks\AbyssShadows --seed 7 --param combat_time=60
# combat_time=0（默认）=通用战斗；>0=智能伤害模式，等待 N 秒后主动退出战斗
```

## 流程

| # | 行为 | 守卫 |
|---|---|---|
| 1 | 庭院→阴阳寮→神社→狭间 | `I_MAIN_GOTO_GUILD` → `I_RYOU_SHENSHE` → `I_ABYSS_SHADOWS` → `I_ABYSS_NAVIGATION` |
| 2 | 选择神龙暗域 | `C_ABYSS_DRAGON` / `I_DRAGON_AREA` |
| 3 | 等待集结 | `I_WAIT_TO_START` 消失 |
| 4 | 6 个目标 | 首领 `C_BOSS_CLICK_AREA`×1、副将 `C_GENERAL_*`×2、精英 `C_ELITE_*`×3：点区域→前往→火焰→准备→战斗 |
| 5 | 切换孔雀暗域 | `I_CHANGE_AREA` / `C_ABYSS_PEACOCK` |
| 6 | 重复 6 目标 | 同上 |
| 7 | 回庭院 | `I_ABYSS_MAP_EXIT` / 返回链 |

## 退出

| exit | result | 场景 |
|---|---|---|
| done | success | 两个暗域打完 |
| no_target | blocked | 预留（暗域不可用） |
| failed | retryable | 超时/异常 |

## 参数

| key | 默认 | 说明 |
|---|---|---|
| combat_time | 0 | 智能伤害等待秒数；0=通用战斗直到结束 |

## 已知限制

周几判定交给调度窗口；固定只打神龙+孔雀两个暗域（旧仓可能打 4 个）；战斗退出细节简化；御魂切换未迁移；无实机证据。
