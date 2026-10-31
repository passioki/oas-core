# AreaBoss（地域鬼王）

> 移植自 `runhey/OnmyojiAutoScript@dev`（commit `8b4dc66`）。行为/差异见 [migration/specs/AreaBoss.md](../../migration/specs/AreaBoss.md)。

## 运行

```powershell
cd D:\OAS-dev\oas-core
# 需要 OCR（声望→可挑战次数）；起始画面=庭院
.\target\release\oasd.exe task lint --tasks-dir tasks --task AreaBoss
.\target\release\oasd.exe run --task tasks\AreaBoss --seed 7 --param floor_mode=normal_1 `
  --model <rec.onnx> --det-model <det.onnx>
# floor_mode: default | ji_1 | ji_10 | normal_1 | normal_60
# use_collect=true 用收藏列表；boss_reward=true 悬赏鬼王（当前为 TODO 告警）
```

## 流程

| # | 行为 | 守卫 |
|---|---|---|
| 1 | 庭院→探索→地域鬼王 | `GameUi.I_MAIN_GOTO_EXPLORATION` → `I_EXPLORATION_GOTO_AREA_BOSS` → `I_CHECK_AREA_BOSS` |
| 2 | 打开筛选 + 热门/收藏 | `I_FILTER` → `I_AB_FILTER_OPENED`；`C_AB_FAMOUS_BTN`/`C_AB_COLLECTION_BTN` |
| 3 | 无鬼王检测 | `I_BATTLE_1/2/3` → `no_target` |
| 4 | 声望→次数 | `ocr_digit(O_AB_REPUTATION)` ≥10000→3 / ≥2000→2 / else 1 |
| 5 | 逐个鬼王 | 点 `I_BATTLE_n` → `I_AB_CLOSE_RED` 详情 |
| 6 | 难度/层数 | `floor_mode`：极·壹/拾（`I_AB_DIFFICULTY_*`、`I_AB_JI_FLOOR_*`、下滑）或普通·1/60（等级滑条右拖） |
| 7 | 挑战+战斗 | `I_FIRE` → 通用战斗（准备/胜负/奖励）→ `I_AB_CLOSE_RED` 回详情 |
| 8 | 关详情→下一个 | `I_AB_CLOSE_RED` 点到消失 |
| 9 | 回庭院 | `I_UI_BACK_YELLOW` 循环直到 `I_CHECK_MAIN` |

## 退出

| exit | result | 场景 |
|---|---|---|
| done | success | 正常完成 N 次挑战 |
| no_target | blocked | 列表无鬼王 |
| no_resource | resource | 预留 |
| failed | retryable | 流程异常 |

## 素材

`legacy_AreaBoss_*`（74）+ `legacy_GameUi_*` + `legacy_GlobalGame_*` + `legacy_Component_GeneralBattle_*`；全部全局，无 task-local。

## 已知限制

见 spec §6：悬赏鬼王 OCR 名称匹配未迁移、等级滑条手柄动态坐标用固定 region 近似、部分筛选（挑战人数/已排行）未使用、御魂切换未迁移、无实机证据。

## Polish round (2026-11-01)

- 补齐 bundle 契约：新增 `assets.json`（空 items，素材全部走全局 legacy_*）+ `images/` 目录。
- 结束标记统一 `legacy_GameUi_I_CHECK_MAIN`；`meta.entry.requires` 对齐 page_main。
- 复核入口链（庭院→探索→地域鬼王）与过滤/选鬼王/开详情/开战/关详情链，未发现新的错误 marker；`boss_reward`/`use_collect`/`floor_mode` 分支保持原迁移（未新增功能）。
- lint: 0 errors; no real-device evidence.



