# AutoCheckinBigGod（⛔ 迁移阻塞）

## 结论

本任务**无法**用当前 oas-core 引擎（oas-flow DSL = 屏幕识别/点击）表达，迁移状态 ⛔。

`script_task.py`（1147 行）流程：
1. `_check_adb_connection()` / `_ensure_frida_server_running()`：向模拟器推送并启动 `frida-server`；
2. `_launch_app()` 启动 `com.netease.gl`（大神 APP）；
3. `_auto_login()`：读 `auth` 数据库 + HTTP API 登录拿 `GL_TOKEN`；失败回退 frida 内存提取；
4. 用 token 调用 `god-welfare.gameyw.netease.com` / `god.gameyw.netease.com` 的签到接口；
5. 依赖第三方包 `oas-checkin-biggod`（`FRIDA_SERVER_XZ`/`ADB_PATH`），缺失直接 ImportError。
`assets.py` 为空类；`config.py` 只有 usage 多行文本。

## 来源（只读旧仓）

- `tasks/AutoCheckinBigGod/script_task.py`
- `tasks/AutoCheckinBigGod/config.py`
- `tasks/AutoCheckinBigGod/assets.py`

## 阻塞原因（具体）

1. 这是**外部服务自动化**任务，不属于 oas-core「屏幕自动化」范围：需要 ADB 子进程、frida、HTTP 客户端、SQLite/加密存储读取。
2. 旧仓依赖 `oas-checkin-biggod` 第三方包（非本仓库代码）。
3. 按 ENGINE-DESIGN「非目标」：引擎不做任意代码执行/网络代理。

## 精确 TODO（解锁条件）

1. 不要用 DSL 迁移；应在 oasd 之外的独立工具（Python 脚本/服务）中实现，或作为 external job 由 scheduler 调用。
2. 若必须在 oasd 内完成，需要新增受控的 `http_request` / `adb_shell` 原生 op（安全评审）与第三方依赖打包方案。
3. 登记为「引擎范围外」，由用户手动/外部脚本执行。

## 现有 flow

`log(原因) × N` + `finish(failed)`，`meta.exits.failed.result = "blocked"`：
调度器会把它记为 blocked（人工介入），**不会伪装成功**。

## 素材

无新增素材需求（旧仓该任务没有可用的 UI 素材，或素材不属于屏幕自动化）。

## Lint

```powershell
cd D:\OAS-dev\oas-core
.\target\release\oasd.exe task lint --tasks-dir tasks --task AutoCheckinBigGod
```
