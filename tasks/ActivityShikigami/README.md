# ActivityShikigami（活动爬塔）

> 移植自 `runhey/OnmyojiAutoScript@dev`（commit `8b4dc66`）。行为/差异见 [migration/specs/ActivityShikigami.md](../../migration/specs/ActivityShikigami.md)。
> 当前实现 **pass（门票）模式**；ap / ap100 / boss 模式未迁移。

## 运行

```powershell
cd D:\OAS-dev\oas-core
.\target\release\oasd.exe task lint --tasks-dir tasks --task ActivityShikigami
.\target\release\oasd.exe run --task tasks\ActivityShikigami --seed 7 --param mode=pass --param max_battles=50
```

## 流程

| # | 行为 | 守卫 |
|---|---|---|
| 1 | 庭院→右侧活动→爬塔主界面 | `Component_RightActivity.I_TOGGLE_BUTTON` → `ActivityShikigami.I_SHI` → `I_TO_BATTLE_MAIN` |
| 2 | PASS 进入战斗主界面 | 循环处理 `I_UI_CONFIRM(_SAMLL)`/`I_TO_BATTLE_MAIN_2`/`I_PASS_3,1,2`/中心点击 → `I_CHECK_BATTLE_MAIN` |
| 3 | 占领奖励 | 点 `I_PASS_5..7,9` 直到 `I_CHECK_BATTLE_MAIN` |
| 4 | 战斗循环 | `O_FIRE` 挑战 → 通用战斗 → 计数，直到 `max_battles` |
| 5 | 退出 | 返回按钮 → `I_TO_BATTLE_MAIN` → `GameUi.I_BACK_Y` → 庭院 |

## 退出

| exit | result | 场景 |
|---|---|---|
| done | success | 达到 max_battles |
| no_target | blocked | 非 pass 模式未实现 |
| no_resource | resource | 预留 |
| failed | retryable | 超时/异常 |

## 参数

| key | 默认 | 说明 |
|---|---|---|
| mode | pass | 仅 pass 实现 |
| max_battles | 50 | 次数上限（旧 pass_limit） |

## 已知限制

ap/ap100/boss 模式未迁移；O_FIRE keyword 丢失（导入为 ocr_lines，仅判断有无文字）；PASS_8/10/11 素材缺失；剧情点击序列需真机调参；limit_time/御魂切换/跨任务清理未迁移；无实机证据。
