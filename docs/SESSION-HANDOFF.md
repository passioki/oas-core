# 会话交接单（SESSION HANDOFF）★ 一页纸

> ★ 新会话先读 `docs/START-HERE.md` → `docs/RESUME.md` → `docs/RESUME-INDEX.md` 最新行（**seq 108**，`oas-core-runner-studio-split`）。
> **状态：handoff-ready**——PR1~PR7 全部提交，最终审计 P1 已修（代码 `68e5768`），交接 docs（seq 108）紧随其后提交；**两个服务已停止，交接后电脑关机（Lead 执行）**。

## 0. 当前状态（2026-11-05 03:3x）

| 项 | 值 |
|---|---|
| 分支 / HEAD | `dev`；**代码 HEAD = `68e5768`**（`fix(runtime): final-audit P1 fixes`）；交接 docs 提交为其后一个提交（seq 108），**最终 HEAD 以 `git log -1` 为准**；`dev`/`main`/`origin` 是否同步以 `git status -sb` 为准；未 tag / 未 release |
| 任务 | **Studio/Runner 分离 + TaskPackage + 任务配置/子任务配置**：Q1~Q7 已批准；PR1 `da9da9d` / PR2 `dfe8776` / PR3 `c09df25`+`048dff5` / PR4 `442f5cb` / PR5 `617435d`+`830058d` / PR6 `edc037b` / PR7 `5e45c81`；**最终审计修复 `68e5768`** |
| 测试 | `cargo test --workspace` **721/0**（基线 713，修复 +8）；oasd **355**、oas-task 111、oas-flow 62、oas-runtime 157 lib + 2 integration；OCR/release build exit 0（**5 warnings**）；UI **1559 checks / Studio 159 ids / Runner 151 ids / 0 dead / 6 nav**（无 UI 改动）；`node --check` 3/3；`git diff --check` clean |
| 分包 | `dist/oasd-runner-v0.1.0-win64.zip` = **37,690,904 B**，SHA256 **`A3BCB4B43EFBF8373A74ADDB44DC4EE5A2AD3847C81D1F61CC039229FE15F960`**；BUILD-INFO `git: 68e5768`、surface=runner、entry `oasd.exe runner --port 8789 --config config/live.json --ui ui`；Runner-only，不入 git |
| 服务 | **8788 / 8789 当前均未监听**；交接后关机。启动见 §2 |
| 工作区 | 只有用户数据未提交：`assets/assets.json`、`frames/frames.json`、`scheduler.json`(+`.bak`)、未跟踪 `frames/frame-43.png`/`frame-44.png`、`tasks/切换预设御魂/`；另有 **Lead 的 packaging 文案订正**（`packaging/README-RUNNER.txt`/`README-DEPLOY.md`，随本批提交）；**用户数据不入库** |

## 1. 最近 3 轮关键变更（历史细节见 PLAN/ENGINE/PENDING）

1. **PR6（`edc037b`，2026-11-05）**：可停止调度（`SchedulerOptions.stop`，attempt 不打断，stop ≤3 s 完成/超时两态）+ Runner 内嵌调度线程（`--no-scheduler` 关闭；`/runner/scheduler/start|stop|status`、health.scheduler；锁冲突 409 `scheduler_locked`）+ 设备租约 `config/device.lease`（pass 级，Studio `/task/run`、Runner `/runner/run`、CLI 冲突 409 `device_busy`）+ `POST /runner/run` 临时运行（values 铺底、body 覆盖、类型保真）+ **Studio 旧调度页彻底删除**（0 残留）。
2. **PR7（`5e45c81`）**：用户纠正「原开发端本来就有调度，直接移到使用端即可」→ 把 PR1 前（`f84b56e`）的**完整 Studio 调度 UI 整体迁移**到 Runner「队列与调度」：6 DOM 块 / **84 旧 id** / **129 旧函数**（77 纯函数 75 逐字+2 文案）；entries/queues 全字段、CRUD/启用/排序、400/409 逐行、运行记录筛选+详情、重置记忆 dialog、任务行「调度设置…/运行记录」、高级 JSON 保留；GET-first 快照守卫；Studio 保持 0 调度。**PR7 的 8 条 UI P2 未修**（见 §4）。
3. **最终审计修复（`68e5768`）**：两路只读审计 P0=0、R1–R20 = 16✅/4⚠/0❌、判定「有条件可交接」；4 个代码 P1 全修——① replay/dry-run 不再抢租约（scheduler replay pass 跳过 + 可观测）；② run worker panic 安全（catch_unwind、`outcome=Panic`、running/租约无条件清理，panic 后可再 run）；③ install/activate 在调度运行/外部 live lock 下 **409 `scheduler_running`**（force 不绕过，stop 后恢复）；④ `installed.json` 损坏时 `/runner/tasks` **降级 200 + warning**、生命周期 API 保持 500 + `installed_registry_corrupt` 可操作文案（无自动隔离）。

## 2. 启动与验证（当前服务已停，交接后关机）

```bat
cd D:\OAS-dev\oas-core
cargo test --workspace                 REM 721/0（oasd 355 / task 111 / flow 62 / runtime 157+2）
node apps\oasd\ui\ui_pure_test.js      REM 1559 / 159 studio / 151 runner / 0 dead / 6 nav
cargo build -p oasd --features ocr     REM 不带 feature 会报 no OCR

target\debug\oasd.exe studio --config config\live.json   REM Studio 8788（编辑/发布）
target\debug\oasd.exe runner --config config\live.json   REM Runner 8789（自动内嵌调度；--no-scheduler 关）
```

- 打包用户：`start-studio.bat`（8788）/ `start-runner.bat`（8789；**固定 `--port 8789`，改 `runner.port` 不生效**）；`stop-runner.bat` 先 POST 停止再强杀。
- 本机实机：MuMu `127.0.0.1:16384`（不是 `emulator-5554`）；`GET /instance/devices` 列表为空时自动 `adb connect`。
- Studio 无调度/重置记忆；调度/重置只在 Runner「队列与调度」页（或 CLI `oasd scheduler reset-memory`）。`oasd serve` = Studio 的 deprecated 别名。

## 3. 数据目录与用户数据边界

| 路径 | 说明 |
|---|---|
| `tasks/<id>/{meta,flow,assets}.json` | 任务（用户数据）；`state/`、`meta.json.bak` 已 ignore |
| `assets/`、`frames/` | 全局素材/帧（用户数据）；`frames/.backup/` 是清空安全网 |
| `scheduler.json`(+`.bak`)、`runs/` | 调度配置/运行记录（用户数据） |
| `config/live.json`、`config/values.json` | 本机配置 / Runner 运行值（values 已 ignore） |
| `config/device.lease`、`<state>.lock`、`packages/`、`CWD/logs/` | 运行态（已 ignore / 不入 git）；`packages/` 是安装包与备份 |
| `dist/` | 构建产物，不入 git |

★ **提交红线**：`assets/`、`frames/`、`tasks/`、`scheduler.json(.bak)` 不提交；`library/` 已 ignore；`packaging/` 旧 `start-scheduler.bat` 文案未清（P2）。

## 4. 已知限制（不阻塞交接）

- **PR7 的 8 条 UI P2**：① `refreshScheduler` 失败时状态卡保留旧值；② config GET 失败误报「文件不存在」；③ 多错误重复显示；④ reset 列表丢 `status.entries` 兜底；⑤ reset 409 丢细节、成功后关框过早；⑥ entry/queue dialog 保存无早期 busy（双击重复 POST）；⑦ `scheduleLocked` OR 过期 config meta；⑧ 死代码/死 CSS。
- **包/registry**：`installed.json` 损坏不自动隔离/repair；`/runner/tasks` 不并集 installed-but-missing；`manifest.asset_refs`/`dependencies` 未与 flow 闭包逐项交叉校验；`move_dir` copy fallback 合并风险；`snapshot_entry` 失败一致性；`PackageError::Other`→400；自定义 `out` 历史不可见；无跨进程 packages 锁；Phase 1 仅本机绝对路径安装。
- **租约/调度**：pass 级不是 attempt 级；stale 回收 TOCTOU；install/activate 的调度检查是 check-then-act 小窗口；panic 安全依赖 unwind；`scheduler.json` 缺失/坏时内嵌线程退出（非设计「空转 blocked」）；`?page=schedule` 依赖 `window.open`；日志环形缓冲仅内存；墙钟 24h soak 未跑完。
- **打包/入口**：`start-runner.bat`/`stop-runner.bat` 固定 8789（Lead 本批已把 `README-RUNNER.txt`/`README-DEPLOY.md` 订正为「`.oaspkg` 填本机绝对路径」「换端口需直接命令行」，packaging 改动随本批提交）；Studio `openRunnerPage` 硬编码 8789；`.gitignore *.tmp` 不匹配 runtime `.values.json.tmp-*`/`.installed.json.tmp-*`。
- **跨平台**：v1 仅 Windows x64；Android 使用端仅设计预留（`packaging/CROSS-PLATFORM.md`）。

## 5. 下一位接手第一步

1. 读 `START-HERE` → 本文件 → `RESUME` → `RESUME-INDEX` seq 108；用 `git log -1` / `git status --porcelain` 核实 HEAD 与工作区，**不要凭摘要判断 docs 提交状态**。
2. 跑 §2 两条验证命令（721/0、1559/159/151/6）与 `cargo build -p oasd --features ocr`。
3. 需要 UI 时启动 Studio 8788（编辑/发布）；需要跑任务时启动 Runner 8789（调度/安装/临时运行）；真机先确认 MuMu `127.0.0.1:16384`。
4. 按 `RUNNER-STUDIO-SPLIT-PLAN §D.4` 19 步做**产品验收**（V1~V7；V1~V4/V6-16/17 已有证据，建议补真实产品任务 + 真机）；再决定 P2 清理 / Phase 5 / 产品 backlog 优先级。
5. 不 tag / 不 release（除非用户明确说发布）；改调度/队列语义先改 `RUNNER-STUDIO-SPLIT-PLAN` + `ENGINE-DESIGN §22.34.7`，再改代码。

## 6. 证据路径

- 最终审计/修复：`D:\OAS-dev\_verify\runner-studio-final-audit-20261105\`（`AUDIT-BACKEND.md` / `AUDIT-UI-DOCS.md` / `FIX-REPORT.md` / `fix-test-workspace.log` / `final-workspace-test.log` / `final-ui-test.log` / `final-package.log` / 本批 `DOCS-HANDOFF-REPORT.md`）。
- PR7：`_verify\runner-studio-pr7-20261105\`（`PR7-UI-REPORT.md` / `RECON-REPORT.md` / `REVIEW.md` / `e2e/` / 4 张截图）。
- PR1~PR6：`_verify\runner-studio-pr1-20261104\` ~ `pr6-20261104\`（各自 `REVIEW` / `FIX-REPORT` / `e2e/`）。
- 现行文档：`docs/RUNNER-STUDIO-SPLIT-PLAN.md` §A.9~§A.16、`docs/ENGINE-DESIGN.md` §22.44~§22.51、`docs/PENDING-LEDGER.md §E-79`（§F~§M）。
