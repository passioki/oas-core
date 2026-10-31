# Delegation（式神委派）

> 来源：`runhey/OnmyojiAutoScript@dev`（只读 checkout `D:/OAS-dev/_verify/remote-oas`，commit `8b4dc669b478`）。
> 迁移方式：行为抽象 + 既有 oas-flow DSL（ENGINE-DESIGN §22），无 Python 代码拷贝。

## 读取的源文件

- `tasks/Delegation/script_task.py`
- `tasks/Delegation/config.py`
- `tasks/Delegation/assets.py`
- `tasks/GameUi/game_ui.py, tasks/GameUi/page.py (imported)`
- `tasks/GlobalGame/assets.py (I_UI_CANCEL 等)`

## 行为摘要

庭院→探索→式神委派：循环领取已完成委派的奖励（领取/聊天气泡/完成/失败弹窗）；按 6 个开关参数用 OCR 精确行匹配选中委派单，填 4 个素材槽并开始委派；已在委派中的单跳过；回庭院。

## 参数 / 出口

- `miyoshino_painting` (bool) default=False
- `bird_feather` (bool) default=False
- `find_earring` (bool) default=False
- `cat_boss` (bool) default=False
- `miyoshino` (bool) default=False
- `strange_trace` (bool) default=False

- exit `done` → `success`
- exit `failed` → `retryable`

## 已实现的行为（行为矩阵见 migration/specs/Delegation.md）

- 进入委派页：覆盖
- 领取委派奖励：部分
- 按名字选委派单：部分（精确匹配近似）
- 已在委派中跳过：部分（无 I_REWARDS_MIN 等待）
- 进入并填 4 素材槽：覆盖
- 开始委派：覆盖
- 回庭院：覆盖

## 已知缺口 / TODO

- check_reward 中 O_D_DONE「完成」缎带的「点其正下方同样大小的可领取区」（script_task.py:150-164）无对应原语（inline OCR 点击只会点识别行中心）→ 目前只覆盖模板类 I_REWARDS_* 领奖，OCR 完成卡片可能漏领。
- 远程 delegate_one 用 keyword 子串匹配（'画'/'鸟羽'/'接送'/'痕迹'），新引擎 OCR 行匹配是规范化精确相等（容 1 字编辑距离）→ 本 flow 用完整单名（弥助的画/鸟之羽/接送弥助/奇怪的痕迹），若卡片行还带其它文案会不匹配并跳过。
- 远程「已在委派中」分支会等 I_REWARDS_MIN 后返回 False；本 flow 只回退并跳过（无 I_REWARDS_MIN 等待语义）。
- 开始委派后不校验最终是否成功（远程同样只点开始循环），靠下一步页面状态体现。

## 复用的通用模式

- **P-NAV**：`legacy_GameUi_I_CHECK_MAIN` 守卫 → `click legacy_GameUi_I_MAIN_GOTO_*` / `legacy_GameUi_I_EXPLORATION_GOTO_*`（导入器已改为：模板 `click` = 匹配到的模板矩形，与旧引擎一致）。
- **P-BATTLE**：`战斗_准备按钮` → 点击 → `战斗_自动按钮` → 循环点安全点 (640,690) 直到任务页 marker 回来。
- **P-RETURN**：`legacy_GlobalGame_I_UI_BACK_YELLOW` / `legacy_GameUi_I_BACK_YOLLOW` / 坐标 (48,43) 兜底，直到 `legacy_GameUi_I_CHECK_MAIN`。
- **OCR**：RuleOcr 已由导入器转为 `ocr_lines`；计数用 `ocr_digit`，条件用 `text_visible`，按行点击用 `click_target` + `{kind:ocr_text, sel|roi, text}`。运行需 `--model`/`--det-model`。

## 证据

- `oasd task lint --tasks-dir tasks --task Delegation` → 0 errors（见 migration/status/Delegation.json）。
- 未真机 / 未回放验证（迁移会话无可用设备与对应场景回放）；素材 id 与 ROI 来自 remote dev 分支，素材均已在全局 `assets/assets.json` 解析（T4 绿）。
