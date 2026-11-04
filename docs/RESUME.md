# RESUME — 断点续接卡（session-memory 强制 6）

> 规则：本文件每次断点覆盖重写；≤150 行；另一个会话只读本文件 + `RESUME-INDEX.md` 最新行就能继续。
> 细节放 `docs/RUNNER-STUDIO-SPLIT-PLAN.md`（**§A.8 分发/跨平台 + §A.9 PR1 实现记录**）· `PENDING-LEDGER.md §E-79` · `ENGINE-DESIGN.md §22.44`。

```yaml
task_id: oas-core-runner-studio-split
checkpoint_seq: 101
updated_at: 2026-11-04T19:25+08:00
trigger: 用户批准 Q1~Q7（按推荐）+ 分发纠正（终端用户只下载 Runner+任务包）+ 跨平台要求（Android 使用端预留）→ PR1 实现/验证/打包 + docs 收尾
status: in_progress
next_action: Lead 提交 PR1 代码/脚本/UI + docs（父 f84b56e）；随后开 PR2 配置模型（meta.params 扩 5 字段 + config/values.json + Runner 配置 API/表单 + 值校验）
```

## 0. 目标（本阶段）

1. **设计批准**：用户 2026-11-04 批准 `RUNNER-STUDIO-SPLIT-PLAN.md §D.2` 的 **Q1~Q7（全部按推荐）**。
2. **分发纠正**：终端使用者**只需要下载「使用端（Runner）」+ 任务包**；Studio 是创作/构建工具，**不发给终端用户**。
3. **跨平台**：后续可能有 **Android 使用端**；v1 仍只 Windows 打包，但 Runner 核心 / HTTP API / 任务包 / 配置 schema 必须平台无关。
4. **PR1 落地**：`Surface` 拆分 + `oasd studio|runner`（8788/8789）+ Runner 只读 API/UI 骨架 + Studio 去调度入口 + Runner-only 发行包 + 跨平台文档；本子进程只做 **docs 收尾**（不改代码/脚本/用户数据，不 commit）。

## 1. 状态（2026-11-04 19:2x）

| 项 | 值 |
|---|---|
| 分支 / HEAD | `dev`；HEAD = **`f84b56e`**（`docs: leftover fixes + Studio/Runner split design draft v1 (seq 100)`）；**PR1 代码/脚本/UI 未提交，父 = `f84b56e`**；docs = 本批（新 HEAD）；hash 以 `git log` 为准；**未 tag / 未 release** |
| 任务状态 | **in_progress**（seq 101）；PR1 ✅ 已实现并验证，等 Lead 提交；PR2~PR6 待做 |
| 双入口 | **Studio `http://127.0.0.1:8788/`**（`oasd studio`；`oasd serve` = deprecated 别名）· **Runner `http://127.0.0.1:8789/`**（`oasd runner`）；均只绑 `127.0.0.1` |
| 测试 | `cargo test -p oasd` **240/0**（基线 229，+11）；`cargo test --workspace` **538/0**（基线 527，+11）；UI pure test **1239 checks / 224 referenced ids / 0 dead ids**（基线 1138/227/0，+101；`runner_ui_audit` 33 ids / 0 dead / 5 nav）；`node --check` 3/3；`git diff --check` 0 |
| 分包 | `dist/oasd-runner-v0.1.0-win64.zip` **37,141,295 B**（Runner-only；23 entries / 0 反斜杠）+ `.sha256` + `dist/BUILD-INFO.txt`；**不入 git**（`dist/` 已 ignore） |
| 上一节点 | seq 100 = 三项遗留修复（region `keep_action_shape` / 模板 polygon 平移 / OCR 取值路径阈值）+ 设计稿 v1（**当时待拍板，现已被 seq 101 的批准 + PR1 取代**；527/0、UI 1138/227/0；docs `f84b56e`） |
| 工作区（非我改动，勿碰） | 代码/脚本/UI：`apps/oasd/src/{serve,main,config}.rs`、`apps/oasd/ui/{runner.*,index.html,app.js,ui_pure_test.js}`、`packaging/*`、`scripts/package*.ps1`；用户数据：`assets/`、`frames/`、`scheduler.json(.bak)`、`tasks/切换预设御魂/`；**均不提交用户数据** |
| 文档（本轮） | `RUNNER-STUDIO-SPLIT-PLAN.md`（状态 + §A.8 + §A.9 + §D.2 批准）、`PENDING-LEDGER §E-79`（PR1 记录/台账）、`ENGINE-DESIGN §22.44`、`RESUME`（本卡）、`RESUME-INDEX` 101、`START-HERE`、`SESSION-HANDOFF`、`README`（**8 份**） |

## 2. 验收清单（本轮）

- ☑ **Q1~Q7 批准**并落 docs：Runner 8789 / `serve` deprecated 别名 / `.oaspkg` zip / 优先级 CLI>entry>values>meta.default / 同名继承+子 default 兜底+`inherit:"none"` / Runner 内嵌线程 / 发布产物不入 git
- ☑ **后端 PR1**：`Surface{Studio,Runner}` + `route_owner`/`required_surface`（严格 §A.3）；gate 在 `/ws/live` 特判前，不匹配纯 404；静态面两端隔离（Runner `/app.js` 404、Studio `/runner.js` 404）；`Server.surface/port`
- ☑ **CLI/Config**：`oasd studio` 默认 8788、`oasd runner` 默认 8789、`serve` deprecated（stderr 提示）；端口优先级 CLI > surface config > 默认；banner 双入口；`/instance.serve_port` 回显实际端口；`config.runner{port:8789,bind:"127.0.0.1"}`（serde default；port≠0/≠serve.port、bind 仅 loopback）
- ☑ **Runner 只读 API**：`GET /runner/health`（ok/surface/version/实际 port/scheduler_mode/tasks_dir/runs_dir/state_path/lock{held,pid?}）、`GET /runner/tasks`（task/has_meta/has_flow/has_assets/params/version?/unmanaged；坏 meta 200 + `meta_error`）；Studio 两端 404
- ☑ **Runner UI**：`runner.html` 5 nav（概览/已安装任务/队列与调度/运行记录/设置）、`runner.css`（≤720px 单列）、`runner.js`（唯一 GET + 8s 超时；textContent；5 hash 深链；无创作/构建 API 与 Studio 脚本）
- ☑ **Studio 去调度入口**：删 nav「调度」+ rail `panel-schedule` 入口；旧页 DOM 仅兼容占位；`activatePage('schedule')` 提示并打开 Runner `#runs`；任务库删「调度设置…」、运行记录改在 Runner 打开；`?reset=1` 提示去 Runner
- ☑ **测试**：oasd **240/0** / workspace **538/0** / UI **1239/224/0** / runner audit 33/0/5 / `node --check` 3/3 / `git diff --check` 0
- ☑ **E2E（Lead）**：真实 Studio 8788 + Runner 8789 双进程；两端归属 404 / 共享只读 200 / Runner health 实际 `port:8789`、`scheduler_mode:"loop"`、`lock.held=false`；`/runner/tasks` 3 个本机任务
- ☑ **Runner-only 打包**：`scripts/package-runner.ps1` + `package-common.ps1`；`packaging/start-runner|studio|stop-runner|studio.bat` + `start-editor.bat` 转调；`live.example.json` 增 `runner` 段；`README-DEPLOY.md` 两种发行包；产物 37,141,295 B（23 entries / 0 反斜杠）
- ☑ **跨平台**：`packaging/CROSS-PLATFORM.md`（四层平台无关契约 + Android 候选 ① on-device Runner，v1 不实现）+ `README-RUNNER.txt` 非 Windows 用法；PR1 新代码无盘符/`cmd` 硬编码，`--port 0` 回填实际端口
- ☐ **Lead 提交** PR1 代码/脚本/UI + docs（不含用户数据；`dist/` 与 `packages/` 不入 git）

## 3. 下一步（精确动作）

1. **Lead 提交**：代码/脚本/UI/打包（父 `f84b56e`）→ docs（新 HEAD）→ 复核 `git status` **不含** `assets/`/`frames/`/`tasks/`/`scheduler.json`；`dist/` 已 ignore。
2. **PR2 配置模型**（下一步开发）：`meta.params` 扩 `label/description/enum/required/group` 五个可选字段；新增 `config/values.json`（`schema_version` + 原子写，Runner 独占）；Runner 配置 API（`GET/PUT /runner/tasks/<id>/config`）+ `ui/runner.js` 表单；运行值校验（type/min/max/enum/required，非法 400 文案含 key）。
3. **PR3**：`RunOptions.params` 改 `BTreeMap<String,Value>` + 四入口合并（CLI > entry > values > meta.default）+ 类型保真（`"007"` 保持字符串）。
4. **PR4**：`call_task.params`/`inherit` + loader 生成 `with_params` + runtime params 栈 + `Flow::call_task_uses` + lint T7；**PR5**：`package.rs` + `publish` + `/runner/install|uninstall` + 素材合并 + `installed.json` + 裸目录纳管；**PR6**：内嵌调度线程 + 调度启停 + 设备租约（可延后）。
5. 每 PR 收口：`cargo test --workspace` 全绿（PR1 后基线 **538**）+ UI pure test（**1239/224/0**）+ `node --check` + `git diff --check`；每 PR 用计划文档 §D.4 对应验收项收口。

## 4. 本轮裁定 / 约束（带锚点）

- **分发**：Studio（`oasd-v*-win64.zip`，8788）只给开发/编辑/发布者；**终端用户只下载 Runner-only 包（`oasd-runner-v*-win64.zip`，8789）+ `.oaspkg`**；Studio 不随使用端分发（`RUNNER-STUDIO-SPLIT-PLAN §A.8.1`、`packaging/README-DEPLOY.md`）。
- **跨平台**：v1 只 Windows x64；macOS/Linux 仅命令行；**Android 使用端 v1 不实现**，但 Runner 核心/HTTP API/任务包/配置 schema 保持平台无关（`CROSS-PLATFORM.md`、计划文档 §A.8.2）。
- **Q1~Q7 全部按推荐**：Runner 8789 / `serve` 降级别名 / `.oaspkg` zip / 配置优先级 CLI>entry>values>meta.default / 未映射同名继承+子 default 兜底+`inherit:"none"` / Runner 内嵌调度线程 / 发布产物不入 git（计划文档 §D.2）。
- **分离不拆二进制**：同一 `serve::Server` 两个 surface；**Studio 无调度写/运维（无调度页），Runner 无创作/构建 API**；数据目录共享（单机单活动实例）。
- **单机单实例、无 LAN**：两 surface 均只绑 `127.0.0.1`；无 token 前不开放 LAN；设备租约 PR6 前，Studio 试运行与 Runner 调度不要同时控制同一台设备。

## 5. 阻塞 / 待用户裁定

- **无技术阻塞**。唯一等待项 = Lead 提交 PR1（提交边界已在计划文档 §A.9 / ENGINE §22.44 写清）。
- PR2 起按计划推进；若用户改 Q1~Q7 任一项，先回写计划文档 §D.2 与稿源再动代码。

## 6. 证据

- **PR1**：`D:\OAS-dev\_verify\runner-studio-pr1-20261104\`（`PR1-BACKEND-REPORT.md` / `PR1-UI-REPORT.md` / `PR1-PACKAGING-REPORT.md` / `oasd-test.log` / `workspace-test.log` / `runner-*.png` / `studio-nav.png` / `runner-health.json` / `runner-tasks.json`）。
- **docs 收尾（本子进程）**：`D:\OAS-dev\_verify\runner-studio-pr1-20261104\DOCS-REPORT.md`（逐处 before→after + 残留检查 + git 状态）。
- **设计稿源**：`_verify\arch-runner-studio-20261104\RUNNER-STUDIO-CONFIG-DESIGN.md`（659 行 / 235+ `file:line`）。
- **上一节点 seq 100**：`_verify\leftovers-20261104\DOCS-REPORT.md`（三项遗留 + 设计稿 v1）。
- **台账**：`docs/PENDING-LEDGER.md §E-79`（PR1 明细/台账）；**现行口径**：`docs/RUNNER-STUDIO-SPLIT-PLAN.md`（§A.8/§A.9/§D.2）、`docs/ENGINE-DESIGN.md §22.44`、`packaging/CROSS-PLATFORM.md`、`packaging/README-DEPLOY.md`、`packaging/README-RUNNER.txt`。

## 7. 连续性最小验证命令（恢复后先跑）

```text
cd D:\OAS-dev\oas-core
git log -1 --format="%h %ci %s"          # 期望 f84b56e（PR1 提交前）
git status --porcelain                   # 期望：PR1 代码/脚本/UI + docs 8 份 + 用户数据（勿提交 assets/frames/tasks/scheduler.json）
node apps\oasd\ui\ui_pure_test.js        # 期望 1239 checks / 224 ids / 0 dead（runner audit 33/0/5）
cargo test --workspace                   # 期望 538 passed / 0 failed（oasd 240）
```
