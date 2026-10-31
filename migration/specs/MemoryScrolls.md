# MemoryScrolls（追忆绘卷·捐献碎片）迁移规格

- 远程源：`runhey/OnmyojiAutoScript@dev` · commit `8b4dc669b47838f42d283f14aa394844c1963c86`
- 已读文件：
  - `tasks/MemoryScrolls/script_task.py`（178 行）
  - `tasks/MemoryScrolls/config.py`（40 行）
  - `tasks/MemoryScrolls/assets.py`（`I_*` + `C_*`；`tasks/MemoryScrolls/ms/{image,click}.json` 原始清单）
  - 依赖：`tasks/GameUi/page.py`（`page_summon` 导航）
- 本地目标：`tasks/MemoryScrolls/`

## 1. 上游行为矩阵

| # | 条件 | 行为 | 出口/副作用 |
|---|---|---|---|
| 1 | 启动 | `goto_page(page_summon)`（庭院 → 召唤） | |
| 2 | 绘卷入口 | `wait_until_appear(I_MS_ENTER, 30s)`；循环：`appear(I_MS_FRAGMENT_S)` ⇒ 已进绘卷主页；否则处理入口（见 3） | 30s 内没入口 ⇒ `set_next_run(success=False)` 失败退出 |
| 3 | 双绘卷活动 | `appear(I_MS_DOUBLE_SCROLLS_ENTER)`：默认绘卷一直接用；选二则 `click(C_MS_DOUBLE_SCROLLS_2)` 后再点双绘卷入口 | 配置 `double_scrolls` |
| 4 | 普通入口 | `appear_then_click(I_MS_ENTER)`（右上角绘卷铃铛） | |
| 5 | 小绘卷 50 检查（可选） | `auto_finish_exploration=true` 时：循环点 `I_MS_FRAGMENT_S` 直到 `I_MS_FRAGMENT_S_VERIFICATION` 出现；`I_MS_FRAGMENT_S_50` 存在 ⇒ 安排次日探索；否则本次失败退出 | 调度写回 |
| 6 | 进指定分卷 | 循环：`appear(I_UI_BACK_RED)` ⇒ 已进捐献页；否则按 `scroll_number` 点 `C_MS_SCROLL_1..6` | 参数 `scroll_number` 1..6 |
| 7 | 95% 通知（可选） | `notification_95=true` 且 `appear(I_MS_COMPLETE_95)` 为假 ⇒ 推送通知 | 配置/通知层 |
| 8 | 捐献判定 | `appear(I_MS_CONTRIBUTE) or not appear(I_MS_COMPLETE)` ⇒ 需要捐献；否则「已满」 | |
| 9 | 捐献循环（最多 10 轮） | 每轮：三行各点两次 `C_MS_MAX_S/M/L` 把数量拉满 → 点 `I_MS_CONTRIBUTE` → 等 `I_MS_CONTRIBUTED` 出现 → 点 `C_MS_CONTRIBUTED`；若三种碎片都显示 0（`I_MS_ZERO_S/M/L`）则提前结束；超过 10 轮放弃本轮 | 防死循环 |
| 10 | 已满 / 结束 | `set_next_run(success=True/False)`；可选 `auto_close_exploration` 改探索配置 | 配置写回 |
| 11 | 返回 | `ui_click_until_disappear(I_UI_BACK_RED)` 关闭捐献页，再 `ui_click_until_disappear(I_UI_BACK_YELLOW)` 回召唤页 | |

无战斗、无时间窗判断（绘卷活动期由调度器决定）。

## 2. oas-core DSL 映射

| 上游 | 本迁移 |
|---|---|
| `goto_page(page_summon)` | `click(legacy_GameUi_I_MAIN_GOTO_SUMMON)` + `expect appear(legacy_GameUi_I_CHECK_SUMMON)` |
| `wait_until_appear(I_MS_ENTER,30)` + 入口 while | `wait_until appear(I_MS_ENTER)` 30s；`loop n=30`：`break` on `I_MS_FRAGMENT_S`；双绘卷分支按 `double_scroll_two` 参数先点 `C_MS_DOUBLE_SCROLLS_2` 再点入口；否则点 `I_MS_ENTER` |
| 小绘卷 50 分支 | `check_small_fragments` 参数；`I_MS_FRAGMENT_S_50` 出现则 log（“次日探索”调度写回未迁移）；否则 `finish(failed)`（= 上游 success=False） |
| 进分卷 | `loop n=20`：`break` on `I_UI_BACK_RED`；按 `scroll_number` 分支 `click_target` region `C_MS_SCROLL_n` |
| 95% 通知 / auto_close_exploration | **不迁移**（通知与玩家配置层职责；记入 differences） |
| 捐献 | `loop n=param('contribute_rounds')`（默认 10）：先判三种 0 → break；`C_MS_MAX_S/M/L` 各点 2 次（`click_target` region）；`appear(I_MS_CONTRIBUTE)` ⇒ `click` + `loop n=5` 等 `I_MS_CONTRIBUTED` 并 `click_target` region `C_MS_CONTRIBUTED`；无捐献按钮 ⇒ log + break（等价「已满无需捐献」） |
| 返回 | `loop n=8`：`I_CHECK_SUMMON` 出现 break；否则依次点 `I_UI_BACK_RED` / `I_UI_BACK_YELLOW` |
| 出口 | `done`(success) / `failed`(retryable) |

## 3. 覆盖判定

- ✅ 覆盖（默认配置 + 参数化）：召唤页导航、绘卷入口（普通/双绘卷二选一）、进指定分卷（1~6）、捐献循环（拉满→捐献→领取贡献动画→重复，含「全 0 提前结束」与「10 轮上限」）、已满不捐献、返回召唤页。
- 🔄 未迁移（记录差异，非核心默认路径）：
  1. `notification_95` 推送（通知层，引擎无 notifier 原语）；
  2. `auto_close_exploration` 改 `exploration.scheduler.enable`（调度器/配置写回）；
  3. `auto_finish_exploration` 的「安排次日探索运行时间」（`custom_next_run`，调度器职责）；本 flow 只做小绘卷 50 检测 + 未达标时 `failed` 出口；
  4. 每次运行结束的 `set_next_run(success=True/False)`（调度器职责，`schedule_hint` 仅给 quota 默认）。
- 差异：捐献循环「三行各点两次」为上游原样复刻；上游 `ui_click_until_disappear`（点到自己消失）用「轮询 + 条件点击」近似。

## 4. 素材（全局引用，不新增）

`legacy_MemoryScrolls_I_MS_*`（模板：ENTER/FRAGMENT_S/FRAGMENT_S_50/FRAGMENT_S_VERIFICATION/CONTRIBUTE/CONTRIBUTED/COMPLETE/COMPLETE_95/ZERO_S/ZERO_M/ZERO_L/DOUBLE_SCROLLS_ENTER）·
`legacy_MemoryScrolls_C_MS_*`（region：SCROLL_1..6 / MAX_S/M/L / CONTRIBUTED / DOUBLE_SCROLLS_2）·
导航：`庭院_探索灯笼`、`legacy_GameUi_I_MAIN_GOTO_SUMMON`、`legacy_GameUi_I_CHECK_SUMMON`、`legacy_GlobalGame_I_UI_BACK_RED`、`legacy_GlobalGame_I_UI_BACK_YELLOW`。

## 5. 待办

1. 无实机/回放证据（绘卷活动期开放；`I_MS_*` 阈值来自旧仓）。
2. OCR 未参与本任务；纯模板/区域链路。
3. `I_MS_ENTER` 阈值仅 0.5（旧仓原值），建议后续用当前客户端真机帧重新标定。
