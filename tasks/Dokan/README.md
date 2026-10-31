# Dokan（寮道馆突入）

> 移植自 `runhey/OnmyojiAutoScript@dev`（commit `8b4dc66`）。行为/阻塞项见 [migration/specs/Dokan.md](../../migration/specs/Dokan.md)。
> **状态 ⛔**：仅迁移「进入道馆场景 + 场景简单分支（等待/关闭/投票/加入战斗）+ 回庭院」；选馆/入寮境/战斗主链路受 DSL 限制不可表达。

## 运行

```powershell
cd D:\OAS-dev\oas-core
.\target\release\oasd.exe task lint --tasks-dir tasks --task Dokan
.\target\release\oasd.exe run --task tasks\Dokan --seed 7
```

## 已实现流程（部分）

| # | 行为 | 守卫 |
|---|---|---|
| 1 | 庭院→阴阳寮→神社→道馆 | `I_MAIN_GOTO_GUILD` → `I_RYOU_SHENSHE` → `I_RYOU_DOKAN` |
| 2 | 场景轮询（≤60 轮） | `CENTER_TOP`+`TODAY_ATTACK_COUNT`/`REMAIN_ATTACK_COUNT_DONE` → done；`FINDING_DOKAN`/`FOUND_DOKAN`/战斗场景 → `no_target`(⛔ 原因) |
| 2b | 投票/结算分支 | `ABANDON_VOTE` → 放弃；`FAILED_VOTE` → 再战(默认 2 次)/保留赏金；`BATTLE_OVER`/`TOPPA_RANK`/`WIN` → 点关闭区域 |
| 3 | 等待类 | `I_RYOU_DOKAN_GATHERING`/`I_DOKAN_BOSS_WAITING`/`I_RYOU_DOKAN_BATTLE_OVER` → 等待 |
| 4 | 关闭类 | `I_RYOU_DOKAN_TOPPA_RANK`/`C_DOKAN_TOPPA_RANK_CLOSE_AREA` → 点关闭 |
| 5 | 战斗入口 | `I_RYOU_DOKAN_START_CHALLENGE` → 点击（战斗本体 ⛔） |
| 6 | 投票 | `I_RYOU_DOKAN_ABANDONED_TOPPA_ABANDONED`（放弃）/`FAILED_VOTE_KEEP_BOUNTY`（保留赏金）|
| 7 | 回庭院 | `I_RYOU_DOKAN_QUIT_BATTLE_ENSURE`/`I_RYOU_DOKAN_DOKAN_QUIT`/`I_UI_BACK_BLUE`/`I_BACK_Y` |

## 退出

| exit | result | 场景 |
|---|---|---|
| done | success | 道馆已结束 |
| no_target | blocked | 需要动态选馆/入寮境（⛔） |
| failed | retryable | 入口 14 次未进道馆场景 / 超时 |

## 已知限制（⛔，详见 spec §6）

1. `find_dokan` 动态列表扫描 + 赏金/人数 OCR 评分 + 动态 ROI —— 无原语。
2. `enter_dokan` 用 `O_DOKAN_MAP` OCR 关键词框 + 偏移点击 —— 无原语。
3. `dokan_battle` 双馆主阵容/绿标/锁定/安全区随机点击 —— 无原语。
4. 跨日剩余次数持久化与 6 种馆主策略 —— 无配置读写原语。
5. 创建道馆/自动加油 —— 无原语；无实机证据。

## 本轮静态修正（静态润色轮）

- 入口循环 break 误用 GameUi.I_BACK_Y（通用返回键，导致进寮后立即退出）→ 改为道馆场景标记（CENTER_TOP/FINDING_DOKAN/FOUND_DOKAN/GATHERING）
- 神社→道馆导航改用旧 DokanSceneDetector 的 I_SCENE_RYOU/I_SCENE_SHENSHE 场景资产（原流程用 I_RYOU_SHENSHE 兜底、缺场景判定）
- 场景机按旧 DokanSceneDetector 优先级重排：FINISHED 最后判定（CENTER_TOP + TODAY_ATTACK_COUNT/REMAIN_ATTACK_COUNT_DONE），弃用旧仓未使用的 I_RYOU_DOKAN_FINISHED
- 修复对 region 资产 C_DOKAN_TOPPA_RANK_CLOSE_AREA 使用 appear()/requires 的非法用法（工程运行时会对 region 报错）→ 改为模板判定 + click_target(region)
- BATTLE_OVER 不再被 GATHERING 等待分支吞掉（原关闭结算分支不可达）
- 补 ABANDON_VOTE（I_RYOU_DOKAN_ABANDONED_TOPPA_CONTINUE→点放弃）与 FAILED_VOTE（daily_attack_count>=2 点再战，否则保留赏金）场景，以及 WIN/SPOILS_OF_DOKAN 关闭结算
- 入口失败守卫（14 次未进道馆场景→failed），场景机仅在 exit=='done' 时运行；删除死 failed 分发分支

> 注：`migration/specs/Dokan.md` 的 exit/行为表述如与本轮冲突，以本节与 flow.json 为准。
