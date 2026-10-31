# WeeklyTrifles（每周琐事）

> 来源：`runhey/OnmyojiAutoScript@dev`（只读 checkout `D:/OAS-dev/_verify/remote-oas`，commit `8b4dc669b478`）。
> 迁移方式：行为抽象 + 既有 oas-flow DSL（ENGINE-DESIGN §22），无 Python 代码拷贝。

## 读取的源文件

- `tasks/WeeklyTrifles/script_task.py`
- `tasks/WeeklyTrifles/config.py`
- `tasks/WeeklyTrifles/assets.py`
- `tasks/GameUi/game_ui.py, tasks/GameUi/page.py (imported)`
- `tasks/GlobalGame/assets.py (I_UI_BACK_RED/BLUE/YELLOW)`

## 行为摘要

每周琐事五项：图鉴分享、地鬼分享、惠比寿摸鱼存储、破碎符咒召唤；秘闻分享当前未迁移（显式 TODO）。入口/关闭分享弹窗/奖励点均用远程素材；回庭院。

## 参数 / 出口

- `share_collect` (bool) default=True
- `share_area_boss` (bool) default=True
- `share_secret` (bool) default=True
- `save_touch_fish` (bool) default=False
- `broken_amulet` (int) default=100

- exit `done` → `success`
- exit `failed` → `retryable`

## 行为矩阵（详见 migration/specs/WeeklyTrifles.md）

- 图鉴分享：覆盖（近似）
- 地鬼分享：覆盖（近似）
- 秘闻分享：缺口（TODO 参数占位）
- 摸鱼存储：部分
- 破碎符咒：部分（固定 10 次/轮）
- 初始分享奖励点：覆盖（近似）

## 已知缺口 / TODO

- 秘闻分享（_share_secret, script_task.py:227-267）未迁移：需要 I_WT_ENTER_SE/I_WT_SE_SHARE/I_WT_SE_RANK/I_WT_SE_JADE 多级判定 + I_UI_BACK_BLUE 返回链；本轮只保留参数与 TODO。
- 分享流程远程有 Timer(3/7s) 判停与 ui_reward_appear_click；新 flow 用固定轮数 bounded loop 近似。
- 地鬼分享「O? 本周已领取」判定远程用 I_WT_AB_JADE 出现与否（本 flow 部分覆盖），且 C_WT_AB_CLICK 为 RuleClick 区域。
- 摸鱼存储：远程 O_WT_LUCKY_TICKETS 是 DigitCounter 返回 (cu,res,total) 且 O_WT_SAVE_COST 为花费；新 flow 用 ocr_digit 比较两个数值，OCR 失败(Null)时比较为 false → 走「不足」分支。
- 破碎符咒：远程按 10/50 次按钮位置切换数量(I_BMT_CHECK + O_BA_TIMES 的 '10次'/'50次' OCR 定位)，新 flow 固定按 10 次累计；O_BA_AMOUNT OCR 失败保护未覆盖。
- 分享到微信的 C_WT_WECHAT / QR 码流程为无人值守 best-effort（远程同样等待超时后放弃）。

## 复用的通用模式

- **P-NAV**：`legacy_GameUi_I_CHECK_MAIN` → `click legacy_GameUi_I_*_GOTO_*`（模板点击 = 匹配到的模板矩形）→ 目标页 marker。
- **P-BATTLE v2**：`Component_GeneralBattle_I_PREPARE_HIGHLIGHT` → 点准备 → 等 `I_WIN/I_FALSE/I_REWARD/I_REWARD_GOLD` → 逐一点掉胜负横幅与奖励 → 等任务页 marker。
- **P-RETURN**：`GlobalGame_I_UI_BACK_*` / `GameUi_I_BACK_YOLLOW` / 坐标兜底 → `I_CHECK_MAIN`。
- **OCR**：`RuleOcr` 已导入为 `ocr_lines`；计数 `ocr_digit`，条件 `text_visible`，按行点击 `click_target {kind:ocr_text, sel|roi, text}`。运行需 `--model/--det-model`。

## 证据

- `oasd task lint --tasks-dir tasks --task WeeklyTrifles` → 0 errors；状态见 `migration/status/WeeklyTrifles.json`。
- 未真机 / 未回放验证（迁移会话无可用设备与对应场景回放）。
