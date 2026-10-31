# GuildActivityMonitor（阴阳寮活动监控）迁移规格

- 远程源：`runhey/OnmyojiAutoScript@dev` · commit `8b4dc669b47838f42d283f14aa394844c1963c86`
- 已读文件：
  - `tasks/GuildActivityMonitor/script_task.py`（158 行）
  - `tasks/GuildActivityMonitor/config.py`（37 行）
  - `tasks/GuildActivityMonitor/assets.py`（`O_GUILD_ACTIVITY_NOTIFY` OCR 区域）
  - 依赖：`tasks/GameUi/page.py`（`page_main`）
- 本地目标：`tasks/GuildActivityMonitor/`
- **结论：⛔ blocked（引擎架构缺原语）**，详见 §3。

## 1. 上游行为矩阵

| # | 条件 | 行为 |
|---|---|---|
| 1 | `run_days`（1~7，默认 1,2,3,4,5,6,7） | 今天不在运行日 ⇒ 记日志 + `set_next_run` 到下一个运行日（服务器 09:00 或玩家时间）→ 直接 `TaskEnd`（跳过） |
| 2 | 运行日 | `goto_page(page_main)` 保证在庭院 |
| 3 | `guild_activity` 开关 | 构建关键字→任务映射：`道馆→Dokan`、`狭间→AbyssShadows`、`宴会→GuildBanquet`、`退治→DemonRetreat`（各自开关） |
| 4 | `use_ocr=false`（默认） | 每 `detection_interval`(30s) 用 **`adb shell dumpsys notification --noredact`** 读系统通知，正则解析 `when=<ts>` 块；最新时间戳 > 初始时间且块内含关键字 ⇒ 触发 |
| 5 | `use_ocr=true` | 每 interval 对 `O_GUILD_ACTIVITY_NOTIFY` ROI 做 OCR（旧 `RuleOcr.ocr(keyword=...)` = 包含匹配），命中关键字 ⇒ 触发 |
| 6 | 触发 | `set_next_run(task=<对应任务>, target=datetime.now())`（立刻拉起该任务）+ `set_next_run(self, now+recheck_interval 5min)` → `TaskEnd` |
| 7 | 监控时长 `monitor_duration`（15min） | 到点 ⇒ 结束（`TaskEnd`，success=None；由调度器安排下次） |
| 8 | 空转保护 | 每 280s `stuck_record_clear/add('PAUSE')`；每分钟日志剩余时间 |

本任务**不点游戏内按钮**、不战斗、不消耗资源；它本质是「系统通知监听器 + 调度触发器」。

## 2. oas-core DSL 现状

- `flow` 可用：`appear/ocr/ocr_digit/text_visible`、`wait/wait_until/loop/while`、`click*`、`finish`。
- 不可用（本任务所需）：
  1. **无 Android 系统通知读取**：引擎只有截图/OCR/点击/滑动/adb 重连；DSL 没有 `adb_shell`/`dumpsys notification` 原语（`oas-adapter-pc` 内部有 adb 命令但未暴露为 flow op）；
  2. **无调度写回/任务拉起**：flow 不能对其他任务 `set_next_run(target=now)`，也不能调用 scheduler 条目；`finish` 只有 8 个具名出口，没有「触发另一个任务」语义；
  3. **无时间/日期函数**：`run_days` 判定、`datetime.now()` 比较、监控时长都依赖 wall-clock；表达式函数只有 `appear/ocr/ocr_digit/text_visible/page/param/var`（无 `now()/weekday()`）；
  4. **无「包含匹配」字符串运算**：OCR 关键字匹配 `keyword in text`；`ocr()` 返回整段文本，表达式无 `contains`/`in` 运算，`click_target.ocr_text` 只支持整行精确（≤1 编辑距离）。

⇒ 只靠 flow 无法实现「监听通知 → 立即拉起目标任务」这一核心闭环；OCR 变体也只能做到「检测到关键字」但无法触发任务。

## 3. 结论与所需原语（建议）

**⛔ blocked**。要让本任务可迁移，至少需要（任一组合）：

| 缺口 | 建议原语 |
|---|---|
| 读系统通知 | 新 flow op：`adb_shell(cmd) -> text`，或专用 `notifications()` 事件源（仅 Android）；或用 OCR 变体替代 |
| 触发/排期其他任务 | 新 flow op：`enqueue_task(task, at="now")` / `set_next_run(task, ...)`（调度器域操作，需引擎确认可写 state 的接口） |
| 时间判断 | 表达式函数 `now()/weekday()/minutes_since(...)`，或由调度器 `windows` 承担 run_days 判定 |
| 字符串包含 | 表达式 `contains(ocr(sel), '道馆')` 或 `ocr_contains(sel, text)` |

在缺这些原语前，本 bundle 保持最小骨架：`flow.json` 只做入口守卫 + 明确 `window_closed` 退出（**不做任何点击**），避免「假装实现了监控」。

## 4. 素材

`legacy_GuildActivityMonitor_O_GUILD_ACTIVITY_NOTIFY`（ocr_lines，roi `[13,213,457,86]`，通知区域，OCR 变体用；当前 blocked 状态未引用）。

## 5. 待办

1. 等引擎补上 §3 原语后重做：`notifications()`/`adb_shell` + `enqueue_task`/`set_next_run`。
2. `run_days` 日期判定建议放调度器 `windows`（ENGINE §22.34 三层分离），flow 内不再重复实现。
3. 无实机验证（任务本身不点游戏内 UI）。
