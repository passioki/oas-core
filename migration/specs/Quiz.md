# Quiz（对弈社答题）迁移规格

- 源仓库：`runhey/OnmyojiAutoScript@dev` · commit `8b4dc669b47838f42d283f14aa394844c1963c86` · 只读副本 `D:/OAS-dev/_verify/remote-oas`
- 目标：`tasks/Quiz/`（统一任务模板 + 全局 `legacy_*` 素材）
- 迁移结论：**⛔** —— 行为矩阵见 §3，缺口见 §5

## 1. 读取的源文件

- `tasks/Quiz/script_task.py`
- `tasks/Quiz/config.py`
- `tasks/Quiz/assets.py`
- `tasks/DemonEncounter/data/answer.py (answer DB, not migrated)`
- `tasks/Quiz/debug.py (remove_symbols/append_one)`

## 2. 入口 / 出口

- 入口：`appear('庭院_探索灯笼')`；flow 首段 `ensure_main()` 回庭院。
- 出口：
  - `done` → result `success`
  - `no_target` → result `blocked`
  - `failed` → result `retryable`

## 3. 行为矩阵

| # | 行为 | 远程实现（源码位置） | 新 DSL 映射 | 状态 |
|---|---|---|---|---|
| 1 | 进入答题活动 | script_task.py:52-80 goto_page(page_main); enter(): I_ENTRY/I_SHI→I_START | ensure_main→world→REALM_RAID→loop click I_ENTRY 直到 I_START | 覆盖 |
| 2 | 答题主循环 | script_task.py:82-121 once(): I_MESSAGE→计时循环，I_FAIL_QUIT/I_SHARE 结算 | — | ⛔ 依赖题库/文本处理/时钟 |
| 3 | 题目+选项 OCR | script_task.py:199-216 detect_question_and_answers | （素材 O_QUESTION/O_ANSWER1..4 已导入，但无文本查询/清洗原语） | ⛔ |
| 4 | 答案索引与点击 | script_task.py:173-195 answer_one + C_ANSWER_ENSURE_n | — | ⛔ |
| 5 | 无票/退出到庭院 | script_task.py:64-67 NoTicket → 回庭院 | ensure_main | 覆盖（入口段） |

## 4. 参数映射

- 无参数。

## 5. 缺口 / TODO

- BLOCKER：远程用 tasks/DemonEncounter/data/answer.py 的本地题库按题目文本查答案索引（含模糊匹配/追加新题）。DSL/引擎没有「读取外部数据文件+文本查询」原语，也没有变量字符串比较/拼接；quiz 的核心价值在于答对，不能降级为随机选项。
- 倒计时判断 O_COUNTDOWN (Digit) 可 ocr_digit，但「最后两秒再选」需要时钟/秒级等待原语（wait_until 只有条件+超时，无绝对时间点）。
- 题目/选项 OCR 的拼接（det 多行按 y 过滤）、remove_symbols 文本清洗、新题检测（4 选项变化≥3）无可用的字符串集合原语。
- I_ALONE_ENSURE/分享/失败退出等分支未迁移。

## 6. 素材契约与通用模式

- 导入器 v3：模板 `roi` = remote `roiBack`，无 `action_roi` ⇒ `click` 落在匹配到的模板矩形（= 旧引擎行为）；`RuleClick` → `kind=region`；`RuleOcr` → `ocr_lines`；`RuleSwipe` → `_FROM`/`_TO` 两个 region；Component 素材 `legacy_Component_<Module>_<CONST>`。
- P-NAV / P-BATTLE v2 / P-RETURN / OCR 语义同批一 README。

## 7. 证据

- oasd task lint 绿；已迁移：进入答题活动到 I_START 界面 + 返回庭院；核心答题（本地题库查询/选项匹配/点击确认/倒计时）被架构阻塞。
- 未真机/未回放：无设备、无对应场景回放帧；T4 素材引用全部解析到全局 `assets/assets.json`。
