# FloatParade（花车巡游）迁移规格

- 源仓库：`runhey/OnmyojiAutoScript@dev` · commit `8b4dc669b47838f42d283f14aa394844c1963c86` · 只读副本 `D:/OAS-dev/_verify/remote-oas`
- 目标：`tasks/FloatParade/`（统一任务模板：meta.json + flow.json + assets.json + images/，素材引用全局 `legacy_*`）
- 迁移结论：**🔄** —— 见 §3 行为矩阵与 §5 缺口

## 1. 读取的源文件

- `tasks/FloatParade/script_task.py`
- `tasks/FloatParade/config.py`
- `tasks/FloatParade/assets.py`
- `tasks/Component/RightActivity/right_activity.py (imported)`
- `tasks/TalismanPass/assets.py (I_OVERFLOW_CONFIRME 复用)`
- `tasks/GameUi/game_ui.py, tasks/GameUi/page.py (imported)`

## 2. 入口 / 出口

- 入口：`meta.entry.requires = appear('庭院_探索灯笼')`；flow 首段用 `ensure_main()` 从任意页面回庭院。
- 出口：
  - `done` → result `success`
  - `failed` → result `retryable`

## 3. 行为矩阵

| # | 行为 | 远程实现（源码位置） | 新 DSL 映射 | 状态 |
|---|---|---|---|---|
| 1 | 打开右侧活动/进入花车 | script_task.py:22 RightActivity.enter(I_FP_ACCESS)（ui_click toggle→access，直到 access 消失） | loop: click I_TOGGLE_BUTTON 直到 I_FP_ACCESS；loop: click I_FP_ACCESS；loop 等 I_FP_UPGRADE + 关红点/礼物弹窗 | 覆盖 |
| 2 | 收取放置里程 | script_task.py:49-57 get_mileage: ui_click(ENTRY→CLAIM); ui_get_reward(CLAIM); 返回 | loop 点 I_ENTRY_MILEAGE 直到出现 I_CLAIM_MILEAGE → loop 领 I_CLAIM_MILEAGE → loop back 直到面板消失 | 覆盖 |
| 3 | 花车等级奖励 | script_task.py:59-114 get_flower：flag1/2 × match_level；批量选择/确认；GETALL0/TASKS/红点；Timer(2) 判停 | loop 20：flag1→level_reward1、flag2→level_reward2、批量选择/确认、GETALL0、TASKS、RED_POINT_0 | 覆盖（判停改固定轮数） |
| 4 | 回主界面 | script_task.py:44 goto_page(page_main) | ensure_main | 覆盖 |
| 5 | 调度间隔 3 天 | config.py:12-14 success_interval/failure_interval | schedule_hint/quota 由 scheduler.json 配置 | 由调度层负责 |

## 4. 参数映射

| key | type | default | 说明 |
|---|---|---|---|
| `level_reward1` | int | `3` | 来自远程同名配置 |
| `level_reward2` | int | `1` | 来自远程同名配置 |

## 5. 缺口 / TODO

- 远程 get_flower 用 Timer(2) 判定「无更多奖励」结束；DSL 无时钟 → 改为固定 20 轮 bounded loop（可能提前退出或多等 20 轮）。
- I_FP_GETALL1 / I_TOGGLE_BUTTON / I_FP_BATCH* 等任务页分支为 best-effort，未逐分支验证。
- 素材识别来自 remote dev 分支，未在当前游戏版本真机验证（等级奖励选择行的 flag 判定依赖动画图标）。

## 6. 通用模式与素材语义（本批任务共用）

- **P-NAV**：`legacy_GameUi_I_CHECK_MAIN` 守卫 + `click` 页面跳转素材（模板点击 = 匹配矩形）+ `expect` 目标页 marker。
- **P-BATTLE**：`战斗_准备按钮`→点击→`战斗_自动按钮`→每 2.5s 点 (640,690) 直到任务页 marker 回来。
- **P-RETURN**：`legacy_GlobalGame_I_UI_BACK_YELLOW` / `legacy_GameUi_I_BACK_YOLLOW` / 坐标兜底 → `I_CHECK_MAIN`。
- 素材契约（导入器 v2）：模板 `roi` = remote `roiBack`，**无 `action_roi`** ⇒ `click` 落在匹配到的模板矩形（= 旧引擎 `RuleImage.coord()` 行为）；`RuleClick` → `kind=region` 精确点击区；`RuleOcr` → `ocr_lines`（`ocr()`/`ocr_digit()`/`text_visible()`/`click_target ocr_text`）。

## 7. 证据

- oasd task lint 绿；覆盖右侧活动入口→花车页→放置里程→等级奖励（档位选择/批量选择/一键领取/红点）；固定 20 轮替代远程 2s Timer 判停。未真机/回放验证。
- 未真机/未回放：迁移会话无可用设备与对应场景回放帧；素材 T4 引用全部解析到全局 `assets/assets.json`。
