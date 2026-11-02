# RESUME — 断点续接卡（session-memory 强制 6）

> 规则：本文件每次断点覆盖重写；≤150 行；另一个会话只读本文件 + `RESUME-INDEX.md` 最新行
> 就能继续。细节放 `ENGINE-DESIGN.md` / `SCHEDULER-RUNNER-PLAN.md` / `PENDING-LEDGER.md`。

```yaml
task_id: oas-core-scheduler-productization
checkpoint_seq: 93
updated_at: 2026-11-03T02:4x+08:00
trigger: 调度器产品化 + 队列 Round 语义：P3 UI 产品化完成（config API + 任务库/调度页编辑）并评审修复（REVIEW-P3 → F3P）；全量回归 488/0（oasd 215，serve 64，UI 453）
status: in_progress
```

## 0. 目标（一句话）

把**调度器产品化 + 队列 Round 语义**（单活动实例阶段）按 P0→P5 落地：P0 语义冻结、P1 schema 4 + 纯函数、P2 队列 Round 执行编排、P3 UI 产品化均已完成；余 P4 Run 一等实体、P5 验收收口。

计划与裁决全文：**`docs/SCHEDULER-RUNNER-PLAN.md`**（术语表 + Round 10 条 + P0–P5 + 风险/DoD）。

## 1. 当前状态（2026-11-03 02:4x）

| 项 | 值 |
|---|---|
| 分支/HEAD | `dev` = 本轮 docs 提交（HEAD，父 = 本轮 P3 代码提交）+ 本轮 P3 代码提交（父 `9f28b02`）；hash 以 `git log` 为准（不写死）；未 tag/release |
| 当前阶段 | **P0 ✅ / P1 ✅ / P2 ✅ / P3 ✅**（P3 本轮提交；全量回归 488/0，oasd 215，UI 453）；**P4–P5 ⬜ 未开始**（任务 status 仍 in_progress） |
| 任务 ID | `oas-core-scheduler-productization`；checkpoint **93** |
| 测试基线 | `cargo test --workspace` = **488 passed / 0 failed**（21 个 test binary 全 ok；oasd **215**；serve 64；调度专项 **101** = `scheduler.rs` 49 + `schedule.rs` 52）；UI pure test **453 checks / 205 referenced ids / 0 dead ids**；task lint **1/0**；`node --check app.js/ui_pure_test.js` exit 0；`cargo build -p oasd --features ocr` exit 0（alt target；warning 与基线一致）。上一发布基线 426/153（seq 89，见 `RELEASE-NODE-2026-11-02.md`） |
| 上一节点 | seq 92：P2 队列 Round 执行编排 + 评审修复（F2P），474/0、oasd 201 |
| 本阶段约束 | **单活动实例、单机、无跨机任务分发**（多实例/多账号/设备池明确不做） |
| 工作区 | P3 代码 + docs 由 Lead 分两次提交；`frames/frames.json` 是**用户数据改动**，不在本轮提交内、不要动 |

## 2. 阶段看板（P0–P5）

| 阶段 | 内容 | 状态 |
|---|---|---|
| **P0** | 语义冻结 + 文档/CLI 订正（ENGINE §22.34.7、§E-75、RESUME 等 7 份 + 计划文档状态行） | ✅ 完成（seq 90） |
| **P1** | `scheduler.json`/`scheduler.state.json` schema 3→4 + queue 纯函数 | ✅ 已提交（seq 91；450/0，oasd 177） |
| **P2** | 执行编排：`execute_pass` 按 kind 分派 → `execute_queue_round`；成员串行 + 每步原子 save；`pending_entry_record` exactly-once；`queue_between`；report/status 队列汇总 | ✅ 已提交（seq 92；474/0，oasd 201；调度专项 101） |
| **P3** | UI 产品化：`GET/POST /scheduler/config`、任务库「调度设置…」、调度页 entries/队列编辑、状态卡、高级 JSON、status kind-aware、删除守卫 | ✅ 本轮提交（488/0，oasd 215，UI 453） |
| **P4** | Run 一等实体（`runs/scheduler/<run_id>.json` + `/runs` + UI 历史/失败原因/点击关联） | ⬜ 未开始（下一步） |
| **P5** | 验收收口（全量回归、replay 队列 2 轮、实机 2 轮 + 重启恢复 + reset） | ⬜ 未开始（需 MuMu） |

## 3. 已批准的四条裁决（摘要）

1. **队列 = entries 的一种 kind**（`entry.kind="task"|"queue"`，队列定义在 `scheduler.json.queues[]`）；单任务条目没有 Round。
2. **全跳过的一轮不消耗 `total_rounds`**（lazy：至少一个成员 attempt≥1 才 `rounds_completed+1`；全跳过只记 `empty_rounds` + `empty_round` 事件）。
3. **失败默认 `continue`**，可选 **`fail_fast`**（本轮中止仍计 1 round）；不自动重跑失败成员（手动重试属 L2 attempt）。
4. **`config.loop_rounds` → `max_passes` 兼容改名（P1.x 已实现）**：配置接受 `max_passes` 别名（旧 `loop_rounds` 仍读）；CLI 主名 `--max-passes`，`--max-rounds`/`--max-runs` 为 deprecated 别名；**status 输出键仍为 `loop_rounds`**。

Round 语义 10 条全文与边界反例见 `docs/SCHEDULER-RUNNER-PLAN.md §二`；ENGINE 摘要见 `§22.34.7`。

## 4. P3 完成摘要（本轮，2026-11-03 02:2x）

- **后端配置 API**：`GET /scheduler/config`（200：`ok/path/exists/schema_version/config/running/state_path`；文件缺失 `exists:false`/`config:null`；坏 JSON 400；只读、不拿锁）；`POST /scheduler/config`（body `{config:<完整 scheduler.json>}`；先 `validate_scheduler_with_offset` 后写；运行中 409；写前 `.bak` 备份 + 原子写）。
- **未知字段语义（最终）**：提交 v4 → 只校验不规范化，磁盘 = 提交对象（pretty + 换行），未知顶层/entry/queue 字段保留、GET 可读回；提交 v3 → 仍用 `Scheduler::to_json()` 规范化升级 v4（该路径未知字段按迁移语义丢弃）；GET 始终返回磁盘原始对象；GET 顶层 `schema_version` 恒为 4。
- **status kind-aware**：task entry 按任务目录存在过滤、queue entry 按 `queues[]` 存在过滤；`running`/`task_choices` 同步（queue 成员无 bundle 也保留在 `task_choices`）。
- **`/task/delete`**：被删任务是队列成员 → 400 拒绝（错误含 `queue_id`），目录/资产/配置零改动；prune 改 kind-aware（只删 `kind=task` 且 task 命中的 entries，queue entries/queues 不动）+ 原子写 + 失败明确 500，不再静默吞错。
- **UI**：任务库每任务「调度设置…」（无条目→加入调度（默认 entry：kind=task/enabled/periodic/全天/day×1/继承 defaults）；多条→WARN 只编辑第一条；已是队列成员→拒绝防双跑）；调度页状态卡 + entries 列表（启用禁用/编辑/删除/上移下移=执行顺序）+ 队列面板（新建/编辑/删除、任务池多选+有序成员、`total_rounds=0` 无限、`on_failure`、`between_ms`）+「+ 添加条目」+ 折叠高级 JSON（本地 parse 校验后保存）；保存先 GET 最新 → 快照比较（外部修改中止并提示先重新加载）→ POST，409/400 分别提示；状态读取失败时状态卡显示「调度器状态读取失败：<err>」+ 各字段 `—`。
- **默认/兼容**：新建空配置默认 `mode:"loop"`、`loop_interval_s:60`；工作台调度面板保持只读 + 「打开调度页 →」；纯函数 75（PURE_FUNCTIONS）。
- **E2E（Lead）**：alt 二进制 + 真实 serve 8790 + 绝对路径 live 配置——GET 缺失→POST v4（1017B）→GET 读回（queue entry+中文成员名完好）→非法 400 且文件不变→锁模拟 409→status entries=qe:queue/plan runnable/task_choices=返回庭院→`GET /` 含新 DOM id；仓库根 `scheduler.json` 未被动。

## 5. 遗留（P4 起，不阻塞）

1. **P2-1**：GET 顶层 `schema_version` 恒 4、UI 强制 4；未来 >4 文档保存会被降级（需后续修）。
2. 高级 JSON 保存路径没有 GET-first 快照守卫（P2-6；仅工具栏按钮有）。
3. 坏 `scheduler.json` 时删除前的队列成员守卫无法判定（prune 会 500 明确失败）。
4. `.bak` 单份（覆盖式）；无真实浏览器 E2E（仅 API E2E + UI pure tests）；状态读取失败路径仅单测覆盖。
5. 同任务多条 task entry 仍允许（既定语义；一个 pass 可能跑同一任务两次，文档/UI 需提示，后续再议）。
6. `utc_offset_min` 缺失时 POST 校验按 480，可能覆盖 `--tz-offset-min`（P2-12）。
7. 无 CSS 新 class（沿用现有样式）；`scheduleTasksCache` 可能过期（P2）。
8. 无浏览器/真机 E2E；P4 Run 一等实体、P5 实机验收仍待做。

（P2 遗留仍在：补记使用恢复时 cycle；补记不计本次 session `report.runs`；`member_results` 200 环假设；成员执行中杀进程不保证成员级 exactly-once。）

## 6. 下一步精确动作

1. **P4 Run 一等实体**：`runs/scheduler/<run_id>.json`（run_id/task/queue/round/member/attempt/started/ended/outcome/exit/clicks）+ `run_finished` 补 exit_code + `GET /runs` 扩展（兼容 `*.clicks.jsonl`）+ UI 运行历史/失败原因/点击聚合关联；`stats.rs` 纳入（其只认 `results[]`，需扩展）。
2. **P5**：全量回归 + replay 队列 2 轮 + 实机 2 轮/重启恢复/`reset-memory`（先启动 MuMu `emulator-5554`）。
3. 旧开口（不阻塞）：`SelectionTool` 结构债；真机 collection `real=1` E2E；P2-10；drag Plan B。

## 7. 证据

- 计划/裁决：`docs/SCHEDULER-RUNNER-PLAN.md`；台账 `docs/PENDING-LEDGER.md §E-75`（§C3 = P2 最终实测，§C4 = P3 最终实测）。
- P3 实现 / 评审 / 修复：`D:\OAS-dev\_verify\scheduler-plan-20261103-p3\P3A-REPORT.md` · `P3B-REPORT.md` · `REVIEW-P3.md` · `F3P-REPORT.md`。
- P3 最终测试：`lead2-workspace.log`（488/0，21 binary）· `f3p-oasd-test.log`（215/0）· `f3p-ui.log`（453 checks / 205 ids / 0 dead）· `f3p-build.log`；E2E `e2e\` · `e2e2\` · `P3A-CONTRACT-SAMPLES.txt`。
- 历史：P2 `P2-REPORT.md` · `REVIEW-P2.md` · `F2P-REPORT.md`；P1 `P1-REPORT.md` · `REVIEW-P1.md` · `F1-FIX-REPORT.md`；P0/F2/F3 `P0-DOCS-REPORT.md` · `F2-DOCS-FIX-REPORT.md` · `F3-DOCS-NUMBERS-REPORT.md`。
- 调度审计：`D:\OAS-dev\_verify\scheduler-audit-20261102\SCHEDULER-AUDIT.md`（42 项能力 ✅28/🟡7/⬜7）。
- 历史长跑：`D:\OAS-dev\_verify\n42\accel-soak\`（96/96 成立）· `live-soak\`（21 轮即断，未完成）。

## 8. 环境（E11~E14，详见 START-HERE §3）

- PowerShell **5.1** + 执行策略 Undefined ⇒ `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\dev-serve.ps1 -NoOpen`。
- `node` 不在 PATH；bundled `C:\Users\passioki\.dsh\dsh-runtimes\dsh-primary-runtime\dependencies\node\bin\node.exe`（v24.21.0）。
- PS 5.1 重定向日志是 **UTF-16LE**（`-Encoding Unicode` 或转 UTF-8）。
- `adb devices` 为空（MuMu `emulator-5554` 未运行）。
- 中文文件用 `read`/`edit`/`write` 工具，不要用 PowerShell 重写 UTF-8。

## 9. 本节点（seq 93）与上一节点（seq 92）

本节点 seq 93：P3 UI 产品化（config API + 任务库/调度页编辑 + 状态卡 + 高级 JSON + status kind-aware + 删除守卫）+ 评审修复（REVIEW-P3 → F3P：P1-1/P1-4/P2-1/P2-2/P2-10 等），全量回归 **488/0**、oasd **215**、serve 64、UI **453**；P4–P5 未开始。

上一节点 seq 92：P2 队列 Round 执行编排（`execute_queue_round` / 成员串行 / `pending_entry_record` / `queue_between` / 事件与 report/status）+ 评审修复 P1-1/2/3 + P2-1（F2P），474/0、oasd 201、调度专项 101；更早 seq 91 = P1 schema 4 + 纯函数（450/0），seq 90 = P0 文档，seq 89 = 0.1.0 Release 上传 + O-1/O-2 收口（`d1a7fcd`）。
