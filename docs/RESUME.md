# RESUME — 断点续接卡（session-memory 强制 6）

> 规则：本文件每次断点覆盖重写；≤150 行；另一个会话只读本文件 + `RESUME-INDEX.md` 最新行
> 就能继续。细节放 `ENGINE-DESIGN.md` / `SCHEDULER-RUNNER-PLAN.md` / `PENDING-LEDGER.md`。

```yaml
task_id: oas-core-scheduler-productization
checkpoint_seq: 94
updated_at: 2026-11-03T12:3x+08:00
trigger: 调度器产品化 P4：Run 一等实体 + 运行可见性（run 文件/事件/API/stats/写失败可观测/UI 运行记录），REVIEW-P4 → F4P 收口；全量回归 501/0（oasd 228，serve 68，UI 662）
status: in_progress
```

## 0. 目标（一句话）

把**调度器产品化 + 队列 Round 语义**（单活动实例阶段）按 P0→P5 落地：P0 语义冻结、P1 schema 4 + 纯函数、P2 队列 Round 执行编排、P3 UI 产品化、P4 Run 一等实体均已完成；余 **P5 验收收口**（实机验收需 MuMu）。

计划与裁决全文：**`docs/SCHEDULER-RUNNER-PLAN.md`**（术语表 + Round 10 条 + P0–P5 + 风险/DoD）。

## 1. 当前状态（2026-11-03 12:3x）

| 项 | 值 |
|---|---|
| 分支/HEAD | `dev` = 本轮 docs 提交（HEAD，父 = 本轮 P4 代码提交；P4 代码提交的父 `f99c36a`）；hash 以 `git log` 为准（不写死）；未 tag/release |
| 当前阶段 | **P0 ✅ / P1 ✅ / P2 ✅ / P3 ✅ / P4 ✅ 本轮提交**（全量回归 **501/0**，oasd **228**，serve **68**，UI **662**）；**P5 ⬜ 未开始**（实机验收，需 MuMu；任务 status 仍 in_progress） |
| 任务 ID | `oas-core-scheduler-productization`；checkpoint **94** |
| 测试基线 | `cargo test --workspace` = **501 passed / 0 failed**（21 个 test binary 全 ok；oasd **228**；serve **68**；调度专项 **107** = `scheduler.rs` 49 + `schedule.rs` 58；`stats.rs` 6）；UI pure test **662 checks / 219 referenced ids / 0 dead ids**；task lint **1/0**；`node --check app.js/ui_pure_test.js` exit 0；`cargo build -p oasd --features ocr` exit 0（alt target；warning 与基线一致）。上一发布基线 426/153（seq 89，见 `RELEASE-NODE-2026-11-02.md`） |
| 上一节点 | seq 93：P3 UI 产品化 + 评审修复（F3P），488/0、oasd 215、serve 64、UI 453 |
| 本阶段约束 | **单活动实例、单机、无跨机任务分发**（多实例/多账号/设备池明确不做） |
| 工作区 | P4 代码 + docs 由 Lead 分两次提交；`frames/frames.json` 是**用户数据改动**，不在本轮提交内、不要动 |

## 2. 阶段看板（P0–P5）

| 阶段 | 内容 | 状态 |
|---|---|---|
| **P0** | 语义冻结 + 文档/CLI 订正（ENGINE §22.34.7、§E-75 等） | ✅ 完成（seq 90） |
| **P1** | `scheduler.json`/`scheduler.state.json` schema 3→4 + queue 纯函数 | ✅ 已提交（seq 91；450/0，oasd 177） |
| **P2** | 执行编排：`execute_queue_round` 成员串行 + 每步原子 save；`pending_entry_record` exactly-once；report/status 队列汇总 | ✅ 已提交（seq 92；474/0，oasd 201；调度专项 101） |
| **P3** | UI 产品化：`GET/POST /scheduler/config`、任务库「调度设置…」、调度页 entries/队列编辑、状态卡、status kind-aware、删除守卫 | ✅ 已提交（seq 93；488/0，oasd 215，UI 453） |
| **P4** | Run 一等实体：`runs/scheduler/<run_id>.json`（每 attempt 一份）+ 事件增量 + `/runs/scheduler*` + stats + 写失败可观测 + 调度页运行记录 | ✅ 本轮提交（501/0，oasd 228，serve 68，UI 662；F4P 收口） |
| **P5** | 验收收口（全量回归、replay 队列 2 轮、实机 2 轮 + 重启恢复 + reset、点击聚合关联、口径统一） | ⬜ 未开始（需 MuMu） |

## 3. 已批准的四条裁决（摘要）

1. **队列 = entries 的一种 kind**（`entry.kind="task"|"queue"`，队列定义在 `scheduler.json.queues[]`）；单任务条目没有 Round。
2. **全跳过的一轮不消耗 `total_rounds`**（lazy：至少一个成员 attempt≥1 才 `rounds_completed+1`；全跳过只记 `empty_rounds` + `empty_round` 事件）。
3. **失败默认 `continue`**，可选 **`fail_fast`**（本轮中止仍计 1 round）；不自动重跑失败成员（手动重试属 L2 attempt）。
4. **`config.loop_rounds` → `max_passes` 兼容改名（P1.x 已实现）**：配置接受 `max_passes` 别名（旧 `loop_rounds` 仍读）；CLI 主名 `--max-passes`，`--max-rounds`/`--max-runs` 为 deprecated 别名；**status 输出键仍为 `loop_rounds`**。

Round 语义 10 条全文与边界反例见 `docs/SCHEDULER-RUNNER-PLAN.md §二`；ENGINE 摘要见 `§22.34.7`。

## 4. P4 完成摘要（本轮，2026-11-03 03:0x）

- **Run 实体**（`schedule.rs`）：`<runs_dir>/scheduler/<run_id>.json`，**每次 attempt 一份**（task entry 含 retry；queue entry 每个成员 attempt 一份）；`run_id=<started_ms>-<seq:04>`（进程内递增 + 磁盘 `exists()` 防碰撞）；tmp+rename 原子写、pretty + 换行。字段：`schema_version(=1)/run_id/source="scheduler"/kind("task"|"queue_member")/entry_id/task/queue_id?/round?/member_index?/attempt/resume/started_ms/finished_ms/duration_ms/class/exit/exit_code/outcome/clicks/error?/clicks_file?`；`exit_code` 来自 `RunReport::exit_code()`（0=Done，1=Failed/Exhausted），无 report 的 setup error 记 1。
- **runs_dir 注入**：`SchedulerOptions.runs_dir` + CLI `--runs-dir`（默认 `runs`）；serve `scheduler_options()` 注入 `self.runs_dir`。**CLI 与 serve 各自使用自己的 `--runs-dir`/CWD**，不一致会出现"调度器写、UI 看不到"。
- **事件增量（additive）**：`run_finished` 补 `exit_code`、`run_file`（相对 `runs_dir` 的 `scheduler/<id>.json`；语义 = 该 entry/round 的**最后一次 attempt**；空轮/未执行 = `null`）、`run_file_error`（写失败明细；健康 = `null`）；旧字段全保留。
- **API（新增路由，旧 `/runs*` 未改）**：`GET /runs/scheduler?limit=200&task=&outcome=` → `{ok,dir,count,runs[]}`（`started_ms` 倒序；limit 默认 200/上限 2000；task/outcome 精确匹配；坏文件跳过；缺目录 200 空）；`GET /runs/scheduler/run?run_id=` → `{ok,run}`（缺参/非法 id 400、未知 404、读失败/坏 JSON 500；`run_id` 白名单 `[A-Za-z0-9._-]`，拒绝 `.`/`..`/斜杠/盘符/NUL）。
- **stats**：`Summary.scheduler` additive `{files,runs,failed,fatal,total_clicks,total_ms,by_outcome}`；**attempt 级口径**（`failed=class retryable`、`fatal=class fatal`；resource/blocked 不计失败；`files` 含坏文件、`runs` 只计合法实体；坏文件进 `alerts`）；与 `SchedulerReport` 的 entry/round 级 `failures/fatals` 粒度不同（`retryable→success` 会 `stats.failed=1` 而 `report.failures=0`）。
- **写失败可观测（F4P P1-3，非致命）**：写 run 文件失败不中断执行、不改退出码；`alert(kind=run_file_write)` + ALERT 行 + `SchedulerReport.run_file_errors: u32`（`scheduler_stop`/`scheduler_summary` 输出 `run_file_errors=N`，`--report` 含）；`--status` 未加（会话级、不持久化）。
- **UI（调度页「运行记录」区）**：刷新/任务+结果+limit 筛选/清除/状态行/列表（14 个新 DOM id，`page-sched-runs-*`）；详情对话框再 `GET /runs/scheduler/run` 取单条，展示全字段 + 完整 JSON + 错误全文；行点击/Enter/Space 打开；状态卡显示最近运行/失败数；任务库/entry/queue 快捷筛选；空/失败态明确（「运行记录读取失败：<err>」+ WARN）。旧 `/runs` 点击分布页未改；纯函数 **111**；新代码全 `textContent`（无 `innerHTML`/`eval`）。
- **计划偏差**：计划原写"`GET /runs` 扩展 + 点击聚合关联"；实际为**新增 `/runs/scheduler*` 路由**（旧 `/runs` 保兼容），run 实体记录 `clicks`/`clicks_file`，但**未接点击聚合关联**（留 P5）。
- **E2E（Lead）**：真实 serve + alt 二进制 + e2e `--runs-dir` → queue 2 轮跑出 4 个成员 attempt 实体（task entry 实体由 `schedule.rs` P4 测试覆盖）→ 列表/详情/筛选 → 旧 `/runs`、`/runs/clicks`、`/runs/aggregate` 仍 200 → `oasd stats` 含 scheduler 块 → `GET /` 含新 DOM id；详见 `_verify\scheduler-plan-20261103-p4\e2e\`。
- **中断/恢复**：2026-11-03 03:0x 首轮 docs 子进程中断（仅计划文档 §P4 完成）；12:23 Lead 复跑 `cargo test --workspace` **501/0**、UI pure test **662/219/0**（`recovery-workspace.log` / `recovery-ui.log`），与首轮一致；其余 docs 由 P4-DOCS-RETRY 补齐。

## 5. 遗留（P5 起，不阻塞）

1. **口径不统一**：事件 `run_file` 是 entry/round 最后一次 attempt 而实体是 attempt 级；stats attempt 级 vs `SchedulerReport` entry 级；UI 状态卡"失败"把 resource/blocked 也算失败（P2 口径）。
2. **点击聚合关联未接**：run 实体有 `clicks`/`clicks_file`，但未与旧 `/runs/clicks`/点击分布页关联；无分页/保留策略（每次扫描目录全量，attempt 无界增长）。
3. **run_id 唯一性**依赖单活动实例 + 磁盘检查（无跨进程锁）；`<id>.json.tmp` 写失败残留不清理（列表忽略）。
4. detail 只要求合法 JSON，**非实体 JSON 也能读**（列表会跳过，契约不一致）；`limit=0` 被 clamp 成 1；list 500 与 detail 500 错误体形状不同。
5. `--status` 无 `run_file_errors`；写失败不改退出码；queue 写失败未单独测试（共用路径）。
6. 运维：CLI 与 serve 的 `--runs-dir`/CWD 必须一致；`oasd stats` 遇坏 run 文件会因 alerts 非空 exit 1。
7. 无真实浏览器/真机 E2E（仅 HTTP API E2E + UI pure tests）；REST 无保留/清理 API。
8. P5 待做：实机验收（需 MuMu）、点击聚合关联、口径统一、P4 收口项。

（P3 遗留仍在：GET 顶层 `schema_version` 恒 4/UI 强制 4；高级 JSON 无 GET-first 快照守卫；坏 config 时删除守卫无法判定；`.bak` 单份；同任务多条 task entry；`utc_offset_min` 缺失按 480。P2 遗留：补记使用恢复时 cycle；补记不计本次 session `report.runs`；`member_results` 200 环假设；成员执行中杀进程不保证成员级 exactly-once。）

## 6. 下一步精确动作

1. **P5 实机验收（先启动 MuMu `emulator-5554`）**：单任务 periodic quota 2/2 + temp 每轮 + **队列 2 轮** + 重启恢复；`reset-memory` 后复位验证。
2. **点击聚合关联**：把 run 实体的 `clicks`/`clicks_file` 与点击分布/聚合页接起来（或明确不做并写结论）。
3. **口径统一**：事件 attempt/entry 粒度、stats attempt 级 vs report entry 级、UI 状态卡失败含 resource/blocked。
4. **P4 收口**：分页/保留策略、run_id 唯一性（多实例/锁）、detail 契约（校验实体字段）、list/detail 500 统一、`--status` 的 run_file_errors 取舍。
5. 旧开口（不阻塞）：`SelectionTool` 结构债；真机 collection `real=1` E2E；P2-10；drag Plan B。

## 7. 证据

- 计划/裁决：`docs/SCHEDULER-RUNNER-PLAN.md`；台账 `docs/PENDING-LEDGER.md §E-75`（§C5 = P4 最终实测；§C3 = P2、§C4 = P3）。
- P4 实现 / 评审 / 修复：`D:\OAS-dev\_verify\scheduler-plan-20261103-p4\P4A-REPORT.md` · `P4B-REPORT.md` · `REVIEW-P4.md` · `F4P-REPORT.md`。
- P4 最终测试：`lead2-workspace.log`（501/0，21 binary）· `recovery-workspace.log`（12:23 复跑 501/0）· `lead2-ui.log` / `recovery-ui.log`（662 checks / 219 ids / 0 dead）· `f4p-cargo-test-oasd.log`（228/0）· `lead2-build.log`；E2E `e2e\`。
- 本轮 docs 收尾（含中断核对）：`D:\OAS-dev\_verify\scheduler-plan-20261103-p4\P4-DOCS-RETRY-REPORT.md`。
- 历史：P3 `P3A/P3B/REVIEW-P3/F3P`；P2 `P2/REVIEW-P2/F2P`；P1 `P1/REVIEW-P1/F1-FIX`；P0/F2/F3 同目录 `P0-DOCS-REPORT.md` · `F2-DOCS-FIX-REPORT.md` · `F3-DOCS-NUMBERS-REPORT.md`。
- 调度审计：`D:\OAS-dev\_verify\scheduler-audit-20261102\SCHEDULER-AUDIT.md`（42 项能力 ✅28/🟡7/⬜7）。
- 历史长跑：`D:\OAS-dev\_verify\n42\accel-soak\`（96/96 成立）· `live-soak\`（21 轮即断，未完成）。

## 8. 环境（E11~E14，详见 START-HERE §3）

- PowerShell **5.1** + 执行策略 Undefined ⇒ `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\dev-serve.ps1 -NoOpen`。
- `node` 不在 PATH；bundled `C:\Users\passioki\.dsh\dsh-runtimes\dsh-primary-runtime\dependencies\node\bin\node.exe`（v24.21.0）。
- PS 5.1 重定向日志是 **UTF-16LE**（`-Encoding Unicode` 或转 UTF-8）。
- `adb devices` 为空（MuMu `emulator-5554` 未运行）。
- 中文文件用 `read`/`edit`/`write` 工具，不要用 PowerShell 重写 UTF-8。

## 9. 本节点（seq 94）与上一节点（seq 93）

本节点 seq 94：**P4 Run 一等实体 + 运行可见性**（run 文件/事件增量/`/runs/scheduler*`/stats/写失败可观测/UI 运行记录）+ 评审修复（REVIEW-P4 → F4P：P1-3 写失败可观测、UI 测试真实性、stats 测试缺口），全量回归 **501/0**、oasd **228**、serve **68**、UI **662/219/0**；03:0x 首轮 docs 中断，12:23 恢复并复核数字一致。P5 未开始。

上一节点 seq 93：P3 UI 产品化（config API + 任务库/调度页编辑 + 状态卡 + 高级 JSON + status kind-aware + 删除守卫）+ F3P，488/0、oasd 215、serve 64、UI 453；更早 seq 92 = P2 队列 Round（474/0），seq 91 = P1 schema 4 + 纯函数（450/0），seq 90 = P0 文档，seq 89 = 0.1.0 Release + O-1/O-2 收口（`d1a7fcd`）。
