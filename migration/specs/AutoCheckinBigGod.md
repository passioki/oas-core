# AutoCheckinBigGod — 行为矩阵（迁移规格 · 阻塞）

源：`runhey/OnmyojiAutoScript@dev` · `tasks/AutoCheckinBigGod/`

## 源文件

- `tasks/AutoCheckinBigGod/script_task.py`
- `tasks/AutoCheckinBigGod/config.py`
- `tasks/AutoCheckinBigGod/assets.py`

## 行为矩阵

| 旧仓行为 | 新 flow 覆盖 | 说明 |
|----------|--------------|------|
| 见 README「阻塞原因」 | ❌ | 这是**外部服务自动化**任务，不属于 oas-core「屏幕自动化」范围 |
| 见 README「阻塞原因」 | ❌ | 旧仓依赖 `oas-checkin-biggod` 第三方包（非本仓库代码）。 |
| 见 README「阻塞原因」 | ❌ | 按 ENGINE-DESIGN「非目标」 |

## 阻塞原因

1. 这是**外部服务自动化**任务，不属于 oas-core「屏幕自动化」范围：需要 ADB 子进程、frida、HTTP 客户端、SQLite/加密存储读取。
2. 旧仓依赖 `oas-checkin-biggod` 第三方包（非本仓库代码）。
3. 按 ENGINE-DESIGN「非目标」：引擎不做任意代码执行/网络代理。

## TODO

1. 不要用 DSL 迁移；应在 oasd 之外的独立工具（Python 脚本/服务）中实现，或作为 external job 由 scheduler 调用。
2. 若必须在 oasd 内完成，需要新增受控的 `http_request` / `adb_shell` 原生 op（安全评审）与第三方依赖打包方案。
3. 登记为「引擎范围外」，由用户手动/外部脚本执行。
