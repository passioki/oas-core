# FloatParade（花车巡游）

> 来源：`runhey/OnmyojiAutoScript@dev`（只读 checkout `D:/OAS-dev/_verify/remote-oas`，commit `8b4dc669b478`）。
> 迁移方式：行为抽象 + 既有 oas-flow DSL（ENGINE-DESIGN §22），无 Python 代码拷贝。

## 读取的源文件

- `tasks/FloatParade/script_task.py`
- `tasks/FloatParade/config.py`
- `tasks/FloatParade/assets.py`
- `tasks/Component/RightActivity/right_activity.py (imported)`
- `tasks/TalismanPass/assets.py (I_OVERFLOW_CONFIRME 复用)`
- `tasks/GameUi/game_ui.py, tasks/GameUi/page.py (imported)`

## 行为摘要

庭院→右侧活动面板→花车巡游：收取放置里程奖励；按 level_reward1/2 选择花车等级奖励档位，批量选择+一键领取；关闭红点/礼物弹窗；回庭院。

## 参数 / 出口

- `level_reward1` (int) default=3
- `level_reward2` (int) default=1

- exit `done` → `success`
- exit `failed` → `retryable`

## 已实现的行为（行为矩阵见 migration/specs/FloatParade.md）

- 打开右侧活动/进入花车：覆盖
- 收取放置里程：覆盖
- 花车等级奖励：覆盖（判停改固定轮数）
- 回主界面：覆盖
- 调度间隔 3 天：由调度层负责

## 已知缺口 / TODO

- 远程 get_flower 用 Timer(2) 判定「无更多奖励」结束；DSL 无时钟 → 改为固定 20 轮 bounded loop（可能提前退出或多等 20 轮）。
- I_FP_GETALL1 / I_TOGGLE_BUTTON / I_FP_BATCH* 等任务页分支为 best-effort，未逐分支验证。
- 素材识别来自 remote dev 分支，未在当前游戏版本真机验证（等级奖励选择行的 flag 判定依赖动画图标）。

## 复用的通用模式

- **P-NAV**：`legacy_GameUi_I_CHECK_MAIN` 守卫 → `click legacy_GameUi_I_MAIN_GOTO_*` / `legacy_GameUi_I_EXPLORATION_GOTO_*`（导入器已改为：模板 `click` = 匹配到的模板矩形，与旧引擎一致）。
- **P-BATTLE**：`战斗_准备按钮` → 点击 → `战斗_自动按钮` → 循环点安全点 (640,690) 直到任务页 marker 回来。
- **P-RETURN**：`legacy_GlobalGame_I_UI_BACK_YELLOW` / `legacy_GameUi_I_BACK_YOLLOW` / 坐标 (48,43) 兜底，直到 `legacy_GameUi_I_CHECK_MAIN`。
- **OCR**：RuleOcr 已由导入器转为 `ocr_lines`；计数用 `ocr_digit`，条件用 `text_visible`，按行点击用 `click_target` + `{kind:ocr_text, sel|roi, text}`。运行需 `--model`/`--det-model`。

## 证据

- `oasd task lint --tasks-dir tasks --task FloatParade` → 0 errors（见 migration/status/FloatParade.json）。
- 未真机 / 未回放验证（迁移会话无可用设备与对应场景回放）；素材 id 与 ROI 来自 remote dev 分支，素材均已在全局 `assets/assets.json` 解析（T4 绿）。
