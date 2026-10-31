# TalismanPass（花合战）

## 来源（只读旧仓）

- `tasks/TalismanPass/script_task.py`（131 行）— run/get_all/get_flower/harvest_soul
- `tasks/TalismanPass/config.py` — `talisman.level_reward`（默认 TWO=金币/勾玉）、`harvest_soul`（默认 False）
- `tasks/TalismanPass/assets.py` — 14 个模板（红点 / 档位 / 一键收取 / 御魂奖励）
- `tasks/GameUi/default_pages.py` — `page_daily`；`page_main→page_daily` 用 `I_MAIN_GOTO_DAILY`

## 行为摘要（旧仓）

1. `goto_page(page_daily)`。
2. `in_task()`（`I_TP_GOTO` 或 `I_TP_EXP` 可见）→ `get_all()`：出现 `I_TP_GET_ALL` 就 `ui_get_reward` 点掉并等「获得奖励」消失。
3. `get_flower(level)`：出现 `I_RED_POINT_LEVEL` 时点红点 → 循环点所选档位（`I_TP_LEVEL_1/2/3`）、溢出确认帧
   `I_OVERFLOW_CONFIRME`、领取奖励、必要时补点 `I_TP_GET_ALL`，2 秒无动作退出。
4. `harvest_soul`（默认关）：回庭院，循环点 `I_TP_SOUL_1 → I_TP_SOUL_2 → I_TP_SOUL_3`（自选御魂/确认/领取），
   连续 5 秒没有奖励则退出。
5. 结束。

## 流程（flow.json）

| # | op | 说明 | 守卫 |
|---|----|------|------|
| 1 | click `I_MAIN_GOTO_DAILY` | 庭院 → 每日/花合战 | requires 庭院 / expect `I_CHECK_DAILY` |
| 2 | if `appear(I_TP_GET_ALL)` | 一键收取 | |
| 3 | if `appear(I_RED_POINT_LEVEL)` | 等级奖励：点红点 → 循环 12 次（档位/溢出确认/奖励/补点） | 红点点击 expect `I_TP_GET_ALL` |
| 4 | if `param('harvest_soul')` | 回庭院 → 循环收取 1500 签御魂（SOUL_1→2→3） | 每步 requires |
| 5 | 回庭院 | 黄返回直到 `I_CHECK_MAIN` | |
| 6 | finish done | | |

- 档位由 `param('level_reward')`（1/2/3，默认 2 = 旧仓 TWO 金币/勾玉）选择。
- 「获得奖励」用全局 `legacy_GlobalGame_I_UI_REWARD` 识别、点击其操作区 `legacy_GlobalGame_C_UI_REWARD`。

## 素材

| 全局 id | 旧 const | 用途 |
|---------|----------|------|
| legacy_TalismanPass_I_TP_GET_ALL | I_TP_GET_ALL | 一键收取 / 界面守卫 |
| legacy_TalismanPass_I_RED_POINT_LEVEL | I_RED_POINT_LEVEL | 等级奖励红点 |
| legacy_TalismanPass_I_TP_LEVEL_1/2/3 | I_TP_LEVEL_* | 三档奖励 |
| legacy_TalismanPass_I_OVERFLOW_CONFIRME | I_OVERFLOW_CONFIRME | 溢出确认 |
| legacy_TalismanPass_I_TP_SOUL_1/2/3 | I_TP_SOUL_* | 1500 签御魂奖励 |
| legacy_TalismanPass_I_TP_GOTO / I_TP_EXP | I_TP_GOTO / I_TP_EXP | 旧 `in_task()` 判定（flow 以 GET_ALL 红点为主） |
| legacy_GameUi_I_MAIN_GOTO_DAILY / I_CHECK_DAILY | page 图 | 导航 |
| legacy_GlobalGame_I_UI_REWARD / C_UI_REWARD / I_UI_BACK_YELLOW | 通用 | 奖励/返回 |

## 出口

`done`(success) / `failed`(retryable)。

## 与旧仓的差异 / 已知缺口

1. **2 秒无动作 / 5 秒无奖励计时器**：DSL 没有 wall-clock 读秒，改为有界循环（等级奖励 12 轮、御魂 30 轮、SOUL_3 领取 40 轮）。
   语义从「静默 N 秒退出」变为「最多尝试 N 轮」，正常场景等价，异常场景最多多花 ~30s。
2. `ui_get_reward` 内部「等待奖励动画消失」用全局「获得奖励」模板 + 操作区点击实现（旧仓同为 GlobalGame UI）。
3. `appear_multi_scale`（多尺度识别 SOUL_1，兼容缩放差异）DSL 无对应 op；改用普通 `appear`（阈值 0.7），
   若客户端缩放变化可能漏识别 → 记录为缺口。
4. `get_flower` 的 2 秒超时条件下「没有奖励则 break」用固定 12 轮替代；若领取后档位按钮仍在，会多点几次（幂等）。
5. 未真机验证。

## Lint

```powershell
cd D:\OAS-dev\oas-core
.\target\release\oasd.exe task lint --tasks-dir tasks --task TalismanPass
```

<!-- round2:start -->
## Polish round 2（2026-11-01 · Lead 实机反馈）

实机反馈（Lead）：无奖励可领的 no-op 路径已 Done；本轮对照旧仓 `get_flower()` 校对奖励分支。

本次修复/对齐：
1. **顺序对齐**：每轮先点所选档位 `I_TP_LEVEL_1/2/3`，再点 `I_OVERFLOW_CONFIRME`（旧仓顺序；
   原 flow 先 overflow 后档位）。
2. **奖励弹窗优先**：出现 `获得奖励`（GlobalGame `I_UI_REWARD`）时先点其操作区；
   `I_TP_GET_ALL` 只在**奖励弹窗不显示**时补点（旧仓 `ui_reward_appear_click(False)` 后的补点逻辑；
   原 flow 可能在奖励弹窗上误点一键收取）。
3. 红点 `ui_click(I_RED_POINT_LEVEL, stop=I_TP_GET_ALL)` 保持 expect `I_TP_GET_ALL`。
4. 2 秒无动作退出 → 10 轮有界；奖励动画消失等待 8 轮。
5. 未出现红点/一键收取按钮时记录 info 日志并继续（no-op 正常日，保持已实机验证的 Done 语义）。

剩余阻塞：等级奖励分支需在「有红点奖励」的日期实机复测；2s 计时器为有界近似。
<!-- round2:end -->
