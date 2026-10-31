# KittyShop（猫咪商店 · 活动）

## 来源（只读旧仓）

- `tasks/KittyShop/script_task.py`（135 行）— run / _run / _select_kitty / _main_select_kitty
- `tasks/KittyShop/config.py` — `kitty_attempts`（默认 1）、`kitty_quit_when_finished`（默认 False）
- `tasks/KittyShop/assets.py` — 9 个 RuleClick（选猫/经营位）、17 个模板、1 个 RuleSwipe（列表滑动）
- `tasks/ActivityShikigami/assets.py` — `I_SHI`（活动入口，本任务复用该类资产）
- `tasks/GameUi/default_pages.py` — `page_main`（入口循环从庭院开始）

## 行为摘要（旧仓）

1. `goto_page(page_main)`。
2. 入口循环：`I_START_FARMING` 出现即停；否则点 `I_GO1`（经营入口）或 `I_SHI`（活动入口）。
3. 按 `kitty_attempts` 重复 `_run()`：
   - `_select_kitty()`：点 `I_START_FARMING` → 在选猫页循环（`I_UNSELECTED` 可见 = 还有未选）；
     旧仓用 1.2s 计时器轮换点 `C_SELECT_1..4`，第 5/10 次计时 swipe `S_SELECTION`，第 12 次判失败；
     失败时点黄返回回到 `I_START_FARMING` 重试（最多 3 次）。
   - 点 `I_START_ENSURE`（开始）→ 等 `I_MAIN_FLAG`。
   - 经营循环「哪里亮了点哪里」：`I_MAIN_SHARE` 出现即结算结束；`quit_when_finished` 时处理确认框与 `I_MAIN_FINSH`；
     `I_MAIN_FLAG` 可见时点 `I_MAIN_GIFT`（收菜）/`I_MAIN_ADD`（加菜），`I_MAIN_FLAG1` 时点一个空闲猫咪卡（`I_MAIN_BUSY_*` 未亮）。
   - 结算后点 `I_MAIN_FLAG1` 回到 `I_START_FARMING`。
4. 结束点黄返回回庭院。

## 流程（flow.json）

| # | op | 说明 | 守卫 |
|---|----|------|------|
| 1 | loop×30 | 入口：START_FARMING / GO1 / I_SHI | 模板守卫 |
| 2 | appear START_FARMING | 断言经营页 | |
| 3 | loop×`param('attempts')` | 每次经营： | |
| 3a | click START_FARMING | 进选猫页 | expect `I_START_ENSURE` |
| 3b | loop×5 | `I_UNSELECTED` 消失即停；依次点 C_SELECT_1..4 + 左滑 | 区域点击 |
| 3c | click START_ENSURE | 开始经营 | expect `I_MAIN_FLAG` |
| 3d | loop×240 | SHARE 出现即结算；可选确认/完成；GIFT/ADD/空闲猫咪卡 | |
| 3e | loop×15 | 点 `I_MAIN_FLAG1` 直到回 START_FARMING | |
| 4 | click 黄返回 | 回庭院 | expect `I_CHECK_MAIN` |
| 5 | finish done | | |

## 素材

| 全局 id | 旧 const | 用途 |
|---------|----------|------|
| legacy_KittyShop_I_GO1 / I_START_FARMING / I_START_ENSURE | 入口/经营/开始 | 导航与状态 |
| legacy_KittyShop_I_UNSELECTED | 未选完 | 选猫循环条件 |
| legacy_KittyShop_C_SELECT_1..4 | 选猫列区域（RuleClick） | 选猫 |
| legacy_KittyShop_I_MAIN_FLAG / FLAG1 | 经营页/未选猫标记 | |
| legacy_KittyShop_I_MAIN_ADD / GIFT / SHARE / FINSH / BUSY_1..5 | 收菜/加菜/分享/结算/空闲猫 | |
| legacy_ActivityShikigami_I_SHI | 活动入口 | 入口兜底 |
| legacy_GlobalGame_I_UI_CONFIRM / I_UI_CONFIRM_SAMLL / I_UI_BACK_YELLOW | 通用 | 确认/返回 |

RuleSwipe `S_SELECTION` 未导入（roi_front=(589,239,21,258) → roi_back=(459,234,22,270)），flow 用等价 `swipe`：
(599,368) → (470,369) ms=500。

## 出口

`done`(success) / `failed`(retryable)。

## 与旧仓的差异 / 已知缺口

1. **选猫循环简化**：旧仓用 1.2s 计时器轮换「单点一个 C_SELECT / 每 5 次滑一次」；本 flow 每轮顺序点 C_SELECT_1..4 + 一次左滑，
   最多 5 轮。目标（选满所有猫）一致，但点按节奏/滑动时机不同，需真机确认。
2. **3 次选猫重试（SelectFailed）未迁移**：本 flow 不抛 SelectFailed，选猫循环有界结束后直接开始经营（若仍有未选猫会记录为缺口）。
3. `swipe` 坐标由旧仓 RuleSwipe 的 roi_front/roi_back 推出（不是臆造）；RuleSwipe 现已导入为两个区域 id
   `legacy_KittyShop_S_SELECTION_FROM/_TO`（区域中心不可在 DSL 内计算，故 flow 用字面量坐标）。
4. `quit_when_finished` 用 GlobalGame 通用确认框资产处理；旧仓用 I_UI_CONFIRM / I_UI_CONFIRM_SAMLL（同源）。
5. 未真机验证。

## Lint

```powershell
cd D:\OAS-dev\oas-core
.\target\release\oasd.exe task lint --tasks-dir tasks --task KittyShop
```

<!-- round2:start -->
## Polish round 2（2026-11-01 · Lead 实机反馈）

实机反馈（Lead）：猫咪商店活动当前未开启。原 flow 在 20 轮入口探测后 `appear(I_START_FARMING)` 硬失败，
调度器会记成 retryable 错误。

本次修复：
1. 入口探测有界（20 轮）；探测结束后用 **if/else 分支**判断：
   - 出现 `I_START_FARMING` → 原经营流程（选猫/经营/收菜/结算，`param('attempts')` 次）→ 回庭院 → `done`；
   - 未出现（活动未开启）→ 记录 warn → 回庭院 → `finish(skipped)`，meta 新增 `skipped: success`
     （**no-op 正常日**，调度器不告警、不重试）。
2. 回庭院循环同时尝试黄/红返回，并检查 `庭院_探索灯笼`，避免单按钮缺失时硬失败。

剩余阻塞：活动开启后仍需实机复测选猫/经营循环；本次只验证了「未开启 → skipped」的空跑语义。
<!-- round2:end -->
