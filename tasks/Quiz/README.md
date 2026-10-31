# Quiz（对弈社答题）

> 来源：`runhey/OnmyojiAutoScript@dev`（只读 checkout `D:/OAS-dev/_verify/remote-oas`，commit `8b4dc669b478`）。
> 迁移方式：行为抽象 + 既有 oas-flow DSL（ENGINE-DESIGN §22），无 Python 代码拷贝。

## 读取的源文件

- `tasks/Quiz/script_task.py`
- `tasks/Quiz/config.py`
- `tasks/Quiz/assets.py`
- `tasks/DemonEncounter/data/answer.py (answer DB, not migrated)`
- `tasks/Quiz/debug.py (remove_symbols/append_one)`

## 行为摘要

庭院→探索→结界突破→对弈社：进入答题活动并到达「开始」界面；每题答案需要本地题库（题目/选项 OCR 文本 → 答案索引），当前 DSL 无外部数据查询能力 → blocked 退出。

## 参数 / 出口

- 无参数

- exit `done` → `success`
- exit `no_target` → `blocked`
- exit `failed` → `retryable`

## 行为矩阵（详见 migration/specs/Quiz.md）

- 进入答题活动：覆盖
- 答题主循环：⛔ 依赖题库/文本处理/时钟
- 题目+选项 OCR：⛔
- 答案索引与点击：⛔
- 无票/退出到庭院：覆盖（入口段）

## 已知缺口 / TODO

- BLOCKER：远程用 tasks/DemonEncounter/data/answer.py 的本地题库按题目文本查答案索引（含模糊匹配/追加新题）。DSL/引擎没有「读取外部数据文件+文本查询」原语，也没有变量字符串比较/拼接；quiz 的核心价值在于答对，不能降级为随机选项。
- 倒计时判断 O_COUNTDOWN (Digit) 可 ocr_digit，但「最后两秒再选」需要时钟/秒级等待原语（wait_until 只有条件+超时，无绝对时间点）。
- 题目/选项 OCR 的拼接（det 多行按 y 过滤）、remove_symbols 文本清洗、新题检测（4 选项变化≥3）无可用的字符串集合原语。
- I_ALONE_ENSURE/分享/失败退出等分支未迁移。

## 复用的通用模式

- **P-NAV**：`legacy_GameUi_I_CHECK_MAIN` → `click legacy_GameUi_I_*_GOTO_*`（模板点击 = 匹配到的模板矩形）→ 目标页 marker。
- **P-BATTLE v2**：`Component_GeneralBattle_I_PREPARE_HIGHLIGHT` → 点准备 → 等 `I_WIN/I_FALSE/I_REWARD/I_REWARD_GOLD` → 逐一点掉胜负横幅与奖励 → 等任务页 marker。
- **P-RETURN**：`GlobalGame_I_UI_BACK_*` / `GameUi_I_BACK_YOLLOW` / 坐标兜底 → `I_CHECK_MAIN`。
- **OCR**：`RuleOcr` 已导入为 `ocr_lines`；计数 `ocr_digit`，条件 `text_visible`，按行点击 `click_target {kind:ocr_text, sel|roi, text}`。运行需 `--model/--det-model`。

## 证据

- `oasd task lint --tasks-dir tasks --task Quiz` → 0 errors；状态见 `migration/status/Quiz.json`。
- 未真机 / 未回放验证（迁移会话无可用设备与对应场景回放）。
