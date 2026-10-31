# MemoryScrolls（追忆绘卷·捐献碎片）

远程源：`runhey/OnmyojiAutoScript@dev` `tasks/MemoryScrolls`（commit `8b4dc66`）。

**状态：🔄** — 静态迁移完成（`task lint` 0 errors），无实机/回放证据（绘卷活动期限制）；见 `migration/status/MemoryScrolls.json`。

## 行为（抽象）

1. 庭院 → 召唤（`legacy_GameUi_I_MAIN_GOTO_SUMMON`，守卫 `庭院_探索灯笼` / `legacy_GameUi_I_CHECK_SUMMON`）。
2. 等召唤页右上角绘卷铃铛 `legacy_MemoryScrolls_I_MS_ENTER`（30s）；循环进入绘卷：双绘卷活动时按 `param('double_scroll_two')` 选「默认绘卷一/双绘卷之二」，普通入口直接点铃铛，进到 `I_MS_FRAGMENT_S` 出现即成功。
3. 可选小绘卷 50 检查（`param('check_small_fragments')=1`）：点碎片图标直到验证弹窗，检查 `I_MS_FRAGMENT_S_50`；未达 50 走 `finish(failed)`（上游 success=False）。
4. 进指定分卷（`param('scroll_number')` 1~6，`C_MS_SCROLL_n` 区域点击）直到 `legacy_GlobalGame_I_UI_BACK_RED` 出现。
5. 捐献循环（`param('contribute_rounds')`，默认 10）：三种碎片全 0 → 结束；否则小/中/大各点两次 `C_MS_MAX_S/M/L`（数量条拉满）→ 出现 `I_MS_CONTRIBUTE` 就点 → 等 `I_MS_CONTRIBUTED` 并点 `C_MS_CONTRIBUTED`；没有捐献按钮即视为已满/次数用完。
6. 返回召唤页：循环点 `I_UI_BACK_RED` / `I_UI_BACK_YELLOW` 直到 `legacy_GameUi_I_CHECK_SUMMON`，`finish(done)`。

## 目录

```
tasks/MemoryScrolls/
├─ meta.json       # params: scroll_number / double_scroll_two / contribute_rounds / check_small_fragments
├─ flow.json
├─ migration.json
└─ README.md
```

全部素材引用自全局 `assets/assets.json`（本任务不新增素材、无 images/）。

参数：`scroll_number=1`（1~6）; `double_scroll_two=0`（0~1）; `contribute_rounds=10`（1~20）; `check_small_fragments=0`（0~1）

## 运行 / 校验

```powershell
cd D:\OAS-dev\oas-core
.\target\release\oasd.exe task lint --tasks-dir tasks --task MemoryScrolls   # => errors=0
.\target\release\oasd.exe run --task tasks\MemoryScrolls --param scroll_number=1
```

## 已知限制

1. **无实机/回放证据**：绘卷活动期开放，迁移时没有活动画面。
2. 未迁移：`notification_95` 推送、`auto_close_exploration` 改探索配置、`auto_finish_exploration` 的次日探索排期、每次运行的 `set_next_run` —— 均为通知/调度器职责（见 `migration.json.differences`）。
3. `legacy_MemoryScrolls_I_MS_ENTER` 阈值仅 0.5（旧仓原值），建议用当前客户端帧重标定。
4. 捐献循环用 `click_target` 区域点击（`C_MS_MAX_*`/`C_MS_CONTRIBUTED`），区域坐标来自旧仓 `click.json`，未在当前客户端复核。

## 2026-11-01 静态修整（polish round）

- 修复：捐献循环：旧 flow 在 I_MS_CONTRIBUTE 不可点时 `break` 提前结束；上游 `if appear_then_click(...)` 失败是**继续下一轮**（重新拉满数量条）→ 改为记录 warn 并继续，由 contribute_rounds 上限收敛
- 修复：补上游捐献门槛 `appear(I_MS_CONTRIBUTE) or not appear(I_MS_COMPLETE)`：分卷已完成（I_MS_COMPLETE 且无捐献按钮）时跳过捐献并记录，不再无意义点数量条
- 修复：（状态口径）migration.json status 由 ✅ 调整为 🔄，与 migration/status/MemoryScrolls.json 一致（无设备证据不判 ✅）
- 仍存在（blocker/未迁移）：无实机/回放证据 → 状态由 ✅ 降为 🔄（2026-11-01，按「no ✅ without device」口径）
- 校验：`oasd task lint --tasks-dir tasks --task MemoryScrolls` => errors=0
