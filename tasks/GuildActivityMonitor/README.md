# GuildActivityMonitor（阴阳寮活动监控）— ⛔ blocked

远程源：`runhey/OnmyojiAutoScript@dev` `tasks/GuildActivityMonitor`（commit `8b4dc66`）。

**状态：⛔ 未实现（架构缺原语）**。上游任务不是普通游戏 flow，而是「系统通知监听器 + 调度触发器」：

1. 每个 `detection_interval`（默认 30s）用 `adb shell dumpsys notification --noredact` 解析 Android 系统通知（`when=<ts>` 块 + 关键字：道馆/狭间/宴会/退治）；或 OCR 屏幕通知区域（`O_GUILD_ACTIVITY_NOTIFY`）。
2. 命中关键字 ⇒ 把对应任务（Dokan/AbyssShadows/GuildBanquet/DemonRetreat）设为「立刻运行」并对自身延迟 5 分钟重查，然后结束监控。
3. 监控 `monitor_duration`（默认 15 分钟）到点结束；`run_days` 控制周几运行。

oas-core 当前 DSL 缺：

| 缺口 | 说明 |
|---|---|
| 读系统通知 | 无 `adb_shell`/notification 原语（引擎只有截图/OCR/点击/滑动） |
| 拉起其他任务 | flow 不能 `set_next_run(task, now)` / 入队其他任务（调度器域操作） |
| 时间/日期 | 表达式无 `now()/weekday()`（`run_days`、监控时长依赖 wall-clock） |
| OCR 包含匹配 | `ocr()` 返回整段文本但表达式无 `contains`；`click_target.ocr_text` 只支持整行精确 |

因此本 bundle 只保留最小骨架：入口守卫（庭院）+ 明确 `window_closed` 退出，**不做任何点击**，避免假装实现。补上上述原语后可重做。

## 目录

```
tasks/GuildActivityMonitor/
├─ meta.json       # 无 params；exits: window_closed(retryable)/failed(fatal)
├─ flow.json       # log + 入口守卫 + finish(window_closed)（骨架）
├─ migration.json
└─ README.md
```

## 校验

```powershell
.\target\release\oasd.exe task lint --tasks-dir tasks --task GuildActivityMonitor   # => errors=0
```

## 建议的引擎原语（一条即可部分解锁）

- `notifications()` / `adb_shell(cmd)` 文本原语（Android）；
- `enqueue_task(task, at:"now")` / 调度器写回接口；
- 表达式 `now()/weekday()`；表达式 `contains(ocr(sel), text)`。

详见 `migration/specs/GuildActivityMonitor.md` §3。
