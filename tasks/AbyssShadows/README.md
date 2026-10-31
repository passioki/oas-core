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
| failed | retryable | 超时/异常 |

## 参数

| key | 默认 | 说明 |
|---|---|---|
| combat_time | 0 | 智能伤害等待秒数；0=通用战斗直到结束 |

## 已知限制

周几判定交给调度窗口；固定只打神龙+孔雀两个暗域（旧仓可能打 4 个）；战斗退出细节简化；御魂切换未迁移；无实机证据。

## 本轮静态修正（静态润色轮）

- 集结等待修正：原流程用 `appear(I_ABYSS_NAVIGATION)` 出现立即 break（等于没有等待）→ 改为 I_WAIT_TO_START 消失（对齐旧 wait_until_disappear）
- 目标战斗块：I_ABYSS_FIRE 未出现（目标已死亡/被占用）时不再无条件点击→改为跳过，避免硬失败（12 个目标块）
- 战斗语义对齐旧 run_general_battle_back：总是主动退出战斗（I_EXIT→确认→I_WIN→回 NAV），combat_time>0 时先动态等待 N 秒（原默认走通用战斗到结束，与旧仓不一致）
- 查找狭间入口的上滑改用 RuleSwipe FROM/TO 的 ROI（drag 的 region 目标只支持内联 roi，不支持 asset sel）
- 进入狭间/神龙暗域/孔雀暗域增加显式失败守卫，并 gate 后续 32 步工作区（失败直接进收尾）；删除死出口 no_target

> 注：`migration/specs/AbyssShadows.md` 的 exit/行为表述如与本轮冲突，以本节与 flow.json 为准。
