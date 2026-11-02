# RESUME — 断点续接卡（session-memory 强制 6）

> 规则：本文件每次断点覆盖重写；≤150 行；另一个会话只读本文件 + `RESUME-INDEX.md` 最新行
> 就能继续。细节放 `ENGINE-DESIGN.md` / `SCHEDULER-RUNNER-PLAN.md` / `PENDING-LEDGER.md`。

```yaml
task_id: oas-core-scheduler-productization
checkpoint_seq: 92
updated_at: 2026-11-03T01:3x+08:00
trigger: 调度器产品化 + 队列 Round 语义：P2 队列 Round 执行编排 + 评审修复（P1-1/2/3 + P2-1）并入并提交；全量回归 474/0（oasd 201，调度专项 101）
status: in_progress
```

## 0. 目标（一句话）

把**调度器产品化 + 队列 Round 语义**（单活动实例阶段）按 P0→P5 落地：P0 语义冻结、P1 schema 4 + 纯函数、P2 队列 Round 执行编排均已完成；随后 P3 UI 产品化、P4 Run 一等实体、P5 验收收口。

计划与裁决全文：**`docs/SCHEDULER-RUNNER-PLAN.md`**（术语表 + Round 10 条 + P0–P5 + 风险/DoD）。

## 1. 当前状态（2026-11-03 01:3x）

| 项 | 值 |
|---|---|
| 分支/HEAD | `dev` = 本轮 docs 提交（HEAD，父 = 本轮 P2 代码提交）+ 本轮 P2 代码提交（父 `865724d`）；hash 以 `git log` 为准（不写死）；未 tag/release |
| 当前阶段 | **P0 ✅ / P1 ✅ / P2 ✅**（P2 本轮提交；全量回归 474/0，oasd 201）；**P3–P5 ⬜ 未开始**（任务 status 仍 in_progress） |
| 任务 ID | `oas-core-scheduler-productization`；checkpoint **92** |
| 测试基线 | `cargo test --workspace` = **474 passed / 0 failed**（21 个 test binary 全 ok；oasd **201**；P2 实现 464 + 评审修复 +10；调度专项 **101** = `scheduler.rs` 49 + `schedule.rs` 52，P2 91 → 修复 +10）；UI pure test **259 checks / 0 dead ids**；task lint **1 task / 0 errors**；`cargo build -p oasd --features ocr` exit 0（alt target）；warning 基线无新增。上一发布基线 426/153（seq 89，见 `RELEASE-NODE-2026-11-02.md`） |
| 上一节点 | seq 91：P1 schema 4 + queue 纯函数 + 评审修复（F1）+ 文档订正（F2）+ 数字收尾（F3） |
| 本阶段约束 | **单活动实例、单机、无跨机任务分发**（多实例/多账号/设备池明确不做） |
| 工作区 | 本轮 P2 代码 + docs 由 Lead 分两次提交；`frames/frames.json` 是**用户数据改动**，不在本轮提交内、不要动 |

## 2. 阶段看板（P0–P5）

| 阶段 | 内容 | 状态 |
|---|---|---|
| **P0** | 语义冻结 + 文档/CLI 订正（ENGINE §22.34.7、§E-75、RESUME 等 7 份 + 计划文档状态行） | ✅ 完成（seq 90） |
| **P1** | `scheduler.json`/`scheduler.state.json` schema 3→4（`queues[]`、`entries[].kind`、迁移 + `<state>.v3.bak` 备份）+ queue 纯函数 | ✅ 已提交（seq 91；450/0，oasd 177） |
| **P2** | 执行编排：`execute_pass` 按 `kind` 分派 → `execute_queue_round`；成员串行 + 每步原子 save；`pending_entry_record` exactly-once；`queue_between`；report/status 队列汇总 | ✅ 本轮提交（474/0，oasd 201；调度专项 101） |
| **P3** | UI 产品化（entries CRUD、队列面板、状态卡、`POST /scheduler/config` 写入口 + 运行中拒绝） | ⬜ 未开始（下一步） |
| **P4** | Run 一等实体（`runs/scheduler/<run_id>.json` + `/runs` + UI 历史） | ⬜ 未开始 |
| **P5** | 验收收口（全量回归、replay 队列 2 轮、实机 2 轮 + 重启恢复 + reset） | ⬜ 未开始（需 MuMu） |

## 3. 已批准的四条裁决（摘要）

1. **队列 = entries 的一种 kind**（`entry.kind="task"|"queue"`，队列定义在 `scheduler.json.queues[]`）；单任务条目没有 Round。
2. **全跳过的一轮不消耗 `total_rounds`**（lazy：至少一个成员 attempt≥1 才 `rounds_completed+1`；全跳过只记 `empty_rounds` + `empty_round` 事件）。
3. **失败默认 `continue`**，可选 **`fail_fast`**（本轮中止仍计 1 round）；不自动重跑失败成员（手动重试属 L2 attempt）。
4. **`config.loop_rounds` → `max_passes` 兼容改名（P1.x 已实现）**：配置接受 `max_passes` 别名（旧 `loop_rounds` 仍读）；CLI 主名 `--max-passes`，`--max-rounds`/`--max-runs` 为 deprecated 别名；**status 输出键仍为 `loop_rounds`**。

Round 语义 10 条全文与边界反例见 `docs/SCHEDULER-RUNNER-PLAN.md §二`；ENGINE 摘要见 `§22.34.7`。

## 4. P2 完成摘要（本轮，2026-11-03 01:3x）

- **执行分派**：`execute_pass` 按 `entry.kind` 分派：Task → `run_entry`；Queue → `execute_queue_round`（一轮计 1 个 `stats.ran`）。
- **准入**：queue entry 与 task 同 window/quota/cooldown/lifecycle 链；硬 skip 仅 `queue_finished`/`queue_missing`；新增 `queue_between`（非 open-round 且距上一轮结束不足 `between_ms`）；open round 走 `recover`/`recover_temp` + resume（规则 8）。
- **Round 执行**：有 `current_round` 续跑否则 begin；成员串行（`mark_running` 写成员任务名 + save → 执行 → `queue_record_member` + `clear_running` 同一次 save → audit）；每步原子 `save_state`。
- **attempts / 失败策略**：成员 attempts = `1+entry.retry`（仅 `retryable` 重试、`between_ms` 退避、有 checkpoint 默认 resume）；`continue`/`fail_fast`/`fatal` 语义。
- **入口分类**（本轮全部 `member_results` 重算）：Fatal > 有 Success/Resource → Success（耗 quota）> executed 全失败 → Retryable（cooldown）> 空轮 → Blocked（cooldown）；空轮只 `empty_rounds+1`，不耗 `total_rounds`。
- **exactly-once**：`QueueState.pending_entry_record`；`queue_end_round`+pending 同一次 save；`record_result`+清 pending 同一次 save；启动首 pass 前 `recover_pending_entry_records` 补记（`queue_entry_record_recovered`）。
- **校验**：一个 queue 至多被一个 entry 引用（重复 → 配置错误，错误含两个 entry_id）。
- **事件**：`queue_round_started`、`queue_round_resumed`、`queue_member_started`、`queue_member_finished`、`queue_round_finished`、`empty_round`、`queue_finished`、`queue_missing`、`queue_entry_record_recovered`；每轮另有 `run_finished(kind=queue, round, clicks, quota_used)`。
- **report/status**：`SchedulerReport.queues[]`（additive，schema 4）；status entry 增 `queue` 进度对象；旧键保留。
- **测试 / E2E**：见 §1；E2E（子进程 + Lead 独立复核）= replay 2 成员 2 轮（rounds=2 / finished / `queue_finished`）、崩溃续跑只跑剩余成员且 rounds 只计 1、pending 补记只一次、重复 queue entry exit 2、`queue_between` 与 `next_due` 一致。

## 5. 遗留（P3 起，不阻塞）

1. 补记使用**恢复时** cycle（跨逻辑日会把该轮 quota 记到当天）。
2. 补记不计本次 session `report.runs`（`report.entries/queues` 从 state 派生）。
3. `member_results` 200 环假设本轮成员 ≤200（校验上限）。
4. end→record 已由 pending 状态机修复；**成员执行中外部杀进程**仍按 checkpoint/resume 重跑（不保证成员级 exactly-once 副作用，属轮级承诺之外）。
5. P3 起：serve 按 kind 过滤（`serve.rs:6449-6469`）、`prune_scheduler_task` 不清队列 members、UI 编辑 entries；P4：Run 一等实体；P5：实机验收（需 MuMu）。可选共享设备互斥文件锁未做（仍为文字规则）。

## 6. 下一步精确动作

1. **P3 UI 产品化**：任务库「启用/加入调度」→ entries CRUD（`GET/POST /scheduler/config`，写前 `validate_scheduler_with_offset` + 运行中拒绝 `lock_held_by_live_process` + 原子写）；调度页 entries 编辑（启用/窗口/quota/params/排序）+ 队列面板（新建/成员/排序/`total_rounds`/`on_failure`/`between_ms`）+ 状态卡（passes、next_wake、running、本轮 `queue` 进度/成员结果/失败原因）；UI pure test 扩展。
2. **P4**：`runs/scheduler/<run_id>.json` 一等实体 + `run_finished` 补 exit_code + `/runs` 扩展 + UI 历史。
3. **P5**：全量回归 + replay 队列 2 轮 + 实机 2 轮/重启恢复/`reset-memory`（先启动 MuMu `emulator-5554`）。
4. 旧开口（不阻塞）：`SelectionTool` 结构债；真机 collection `real=1` E2E；P2-10；drag Plan B。

## 7. 证据

- 计划/裁决：`docs/SCHEDULER-RUNNER-PLAN.md`；台账 `docs/PENDING-LEDGER.md §E-75`（§C3 = P2 最终实测）。
- P2 实现报告 / 评审 / 修复：`D:\OAS-dev\_verify\scheduler-plan-20261102\P2-REPORT.md` · `REVIEW-P2.md` · `F2P-REPORT.md`。
- P2 最终测试：`_f2p-test-workspace-final.txt`（474/0，21 binary）· `_f2p-test-oasd-final.txt`（201/0）· `_f2p-test-workspace.txt`；构建 `lead-p2f-build.log`。
- E2E：`p2-smoke-f2p\`（pending/续跑/dup/between）· `e2e\`（Lead 独立复核）· `p2-smoke\`（2 轮/续跑/错误路径）。
- 历史：P1 报告 `P1-REPORT.md` · `REVIEW-P1.md` · `F1-FIX-REPORT.md`；P0/F2/F3 报告 `P0-DOCS-REPORT.md` · `F2-DOCS-FIX-REPORT.md` · `F3-DOCS-NUMBERS-REPORT.md`。
- 调度审计：`D:\OAS-dev\_verify\scheduler-audit-20261102\SCHEDULER-AUDIT.md`（42 项能力 ✅28/🟡7/⬜7）。
- 历史长跑：`D:\OAS-dev\_verify\n42\accel-soak\`（96/96 成立）· `live-soak\`（21 轮即断，未完成）。

## 8. 环境（E11~E14，详见 START-HERE §3）

- PowerShell **5.1** + 执行策略 Undefined ⇒ `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\dev-serve.ps1 -NoOpen`。
- `node` 不在 PATH；bundled `C:\Users\passioki\.dsh\dsh-runtimes\dsh-primary-runtime\dependencies\node\bin\node.exe`（v24.21.0）。
- PS 5.1 重定向日志是 **UTF-16LE**（`-Encoding Unicode` 或转 UTF-8）。
- `adb devices` 为空（MuMu `emulator-5554` 未运行）。
- 中文文件用 `read`/`edit`/`write` 工具，不要用 PowerShell 重写 UTF-8。

## 9. 本节点（seq 92）与上一节点（seq 91）

本节点 seq 92：P2 队列 Round 执行编排（`execute_queue_round` / 成员串行 / `pending_entry_record` / `queue_between` / 事件与 report/status）+ 评审修复 P1-1/2/3 + P2-1（F2P 并入），全量回归 **474/0**、oasd **201**、调度专项 **101**；P3–P5 未开始。

上一节点 seq 91：P1 schema 4 + queue 纯函数 + 评审修复（F1）+ 文档订正（F2）+ 数字收尾（F3），450/0、oasd 177；更早节点 seq 90 = P0 文档，seq 89 = 0.1.0 Release 上传 + O-1/O-2 收口（`d1a7fcd`）。
