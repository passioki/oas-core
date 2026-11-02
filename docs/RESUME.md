# RESUME — 断点续接卡（session-memory 强制 6）

> 规则：本文件每次断点覆盖重写；≤150 行；另一个会话只读本文件 + `RESUME-INDEX.md` 最新行
> 就能继续。细节放 `ENGINE-DESIGN.md` / `SCHEDULER-RUNNER-PLAN.md` / `PENDING-LEDGER.md`。

```yaml
task_id: oas-core-scheduler-productization
checkpoint_seq: 91
updated_at: 2026-11-02T23:3x+08:00
trigger: 调度器产品化 + 队列 Round 语义：P0 文档 / P1 实现 + 评审修复并入并提交；全量回归 450/0（oasd 177）
status: in_progress
```

## 0. 目标（一句话）

把**调度器产品化 + 队列 Round 语义**（单活动实例阶段）按 P0→P5 落地：P0 语义冻结与文档订正、P1 schema 3→4 + 纯函数层（均已完成并提交，P1 评审修复已并入），随后 P2 执行编排、P3 UI 产品化、P4 Run 一等实体、P5 验收收口。

计划与裁决全文：**`docs/SCHEDULER-RUNNER-PLAN.md`**（术语表 + Round 10 条 + P0–P5 + 风险/DoD）。

## 1. 当前状态（2026-11-02 23:3x）

| 项 | 值 |
|---|---|
| 分支/HEAD | `dev` = 本轮 docs 提交（HEAD，父 = 本轮 P1 代码提交）+ 本轮 P1 代码提交（父 `131d6a7`）；hash 以 `git log` 为准（不写死）；未 tag/release |
| 当前阶段 | **P0 ✅ 完成**（语义冻结 + 文档/CLI 订正）；**P1 ✅ 已提交**（P1 代码 + docs 两次提交；全量回归 450/0，oasd 177）；**P2–P5 ⬜ 未开始**（任务 status 仍 in_progress） |
| 任务 ID | `oas-core-scheduler-productization`；checkpoint **91** |
| 测试基线 | `cargo test --workspace` = **450 passed / 0 failed**（21 个 test binary 全 ok；oasd **177**；调度专项 **77** = `scheduler.rs` 45 + `schedule.rs` 32；P1 实现 445/172 → 评审修复 +5）；UI pure test **259 checks / 0 dead ids**（未变）；task lint **1 task / 0 errors**；`cargo build -p oasd --features ocr` exit 0（alt target，默认 target 被外部句柄占用）。上一发布基线 426/153（seq 89，见 `RELEASE-NODE-2026-11-02.md`） |
| 上一节点 | seq 90：P0 文档（ENGINE §22.34.7 / PENDING §E-75 / RESUME 等）；seq 89：0.1.0 Release 上传 + O-1/O-2 两轮评审收口（`d1a7fcd`） |
| 本阶段约束 | **单活动实例、单机、无跨机任务分发**（多实例/多账号/设备池明确不做） |
| 工作区 | 已提交（本轮两次提交：P1 代码 + docs）；`frames/frames.json` 是**用户数据改动**，不在本轮提交内、不要动 |

## 2. 阶段看板（P0–P5）

| 阶段 | 内容 | 状态 |
|---|---|---|
| **P0** | 语义冻结 + 文档/CLI 订正（ENGINE §22.34.7、§E-75、RESUME 等 7 份 + 计划文档状态行） | ✅ 完成（seq 90） |
| **P1** | `scheduler.json`/`scheduler.state.json` schema 3→4（`queues[]`、`entries[].kind`、迁移 + `<state>.v3.bak` 备份）+ queue 纯函数 `queue_should_finish`/`queue_finished`/`queue_next_member_index`/`queue_begin_round`/`queue_record_member`/`queue_end_round`/`queue_next_due_ms`/`queue_reset` | ✅ 已提交（本轮 P1 代码提交；全量回归 450/0，oasd 177） |
| **P2** | 执行编排 `execute_queue_round` + `queue_*`/`empty_round` 事件 + report 队列汇总（`next_wake` queue 候选一并在 P2 接入） | ⬜ 未开始 |
| **P3** | UI 产品化（entries CRUD、队列面板、状态卡） | ⬜ 未开始 |
| **P4** | Run 一等实体（`runs/scheduler/<run_id>.json` + `/runs` + UI 历史） | ⬜ 未开始 |
| **P5** | 验收收口（全量回归、replay 队列 2 轮、实机 2 轮 + 重启恢复 + reset） | ⬜ 未开始（需 MuMu） |

## 3. 已批准的四条裁决（摘要）

1. **队列 = entries 的一种 kind**（`entry.kind="task"|"queue"`，队列定义在 `scheduler.json.queues[]`）；单任务条目没有 Round。
2. **全跳过的一轮不消耗 `total_rounds`**（lazy：至少一个成员 attempt≥1 才 `rounds_completed+1`；全跳过只记 `empty_rounds` + `empty_round` 事件）。
3. **失败默认 `continue`**，可选 **`fail_fast`**（本轮中止仍计 1 round）；不自动重跑失败成员（手动重试属 L2 attempt）。
4. **`config.loop_rounds` → `max_passes` 兼容改名（P1.x 已实现）**：配置接受 `max_passes` 别名（旧 `loop_rounds` 仍读）；CLI 主名 `--max-passes`，`--max-rounds`/`--max-runs` 为 deprecated 别名；**status 输出键仍为 `loop_rounds`**（配置模型字段 `Scheduler::loop_rounds` 与 state 字段名未改）。

Round 语义 10 条全文与边界反例见 `docs/SCHEDULER-RUNNER-PLAN.md §二`；ENGINE 摘要见 `§22.34.7`。

## 4. 本轮已落文档（P0 + F2 订正 + F3 数字收尾）

- `ENGINE-DESIGN.md`：新增 **§22.34.7 术语与 Round 语义（2026-11-02 裁决）**（四层术语表 / 10 条摘要 / 四个 round 名字消歧 / `max_passes` 兼容改名 / `--config` 陷阱）；订正 §22.35 的 24h soak 行（加速 soak 成立、**墙钟 24h 未跑完**）。
- `PENDING-LEDGER.md`：N40 行 + N38/N39/N40 汇总行订正；新增 **§E-75**（4 裁决 / P0–P5 / 台账）；P1 状态与评审修复项已同步。
- `RESUME.md`（本文件，seq 91 覆盖重写）· `RESUME-INDEX.md`（追加 seq 91）· `START-HERE.md §0.2` · `README.md` · `SESSION-HANDOFF.md` · `SCHEDULER-RUNNER-PLAN.md`（状态行 / P1 段 / 契约命名）。

两处事实订正：
- **墙钟 24h soak 未跑完**：`D:\OAS-dev\_verify\n42\live-soak\scheduler.state.json` = passes=22、`last_stop=""`、`running=null`；events 无 `stop`；lock 残留 pid=1268。只有 96 轮加速 soak（96/96、temp reset 192）成立。
- **`oasd scheduler --config` 失败模式**：`--config` 是全局 live 配置（被 `main.rs` 在分发前摘走并立即解析）；误把 `scheduler.json` 传给 `--config` ⇒ **exit 2 + `config error: config JSON: unknown field 'config'`**，不会跑任何 plan；调度计划入口是 **`--plan <scheduler.json>`**；只有传合法 live 配置时 plan 才回退默认 `scheduler.json`。

F2 文档订正（2026-11-02 23:1x，只改 `docs/*.md`）：`--config` 失败模式（exit 2 · unknown field）、queue 状态字段 `finished: bool`（无 `status` 字段）、纯函数名对齐代码、`max_passes` 兼容改名状态、P1 评审修复项、测试数字（P1 实现 445；评审修复后实测 450/0，oasd 177）。

F3 数字收尾（2026-11-02 23:3x，只改 `docs/*.md`）：把"待实测"类占位全部收敛为 Lead 最终实测（2026-11-02 23:35）。全量 `cargo test --workspace` = **450 passed / 0 failed**（21 个 test binary 全 ok；oasd **177**，P1 172 + 评审修复 5；调度专项 **77** = `scheduler.rs` 45 + `schedule.rs` 32，P1 72）；UI pure test **259 checks / 0 dead ids**（未变）；task lint **1 task / 0 errors**；`cargo build -p oasd --features ocr` **exit 0**（alt target，默认 target 被外部句柄占用）。E2E 复核：queue entry 在 `--status` 中 `runnable_now=false` / `blocked_reason=queue_executor_pending`；`max_passes` 配置别名 → status `loop_rounds=2`；CLI `--max-passes 1` → stop reason=`max_rounds`；v3 显式 `kind` → exit 2（`schema_version 3 must not declare entries[0].kind`）；state v3→v4 迁移生成 `<state>.v3.bak`（SHA256 与原文一致）、state schema=4。P1 状态行收尾为 ✅（评审修复已并入并提交）。

## 5. 下一步精确动作

1. ✅ **本轮收口已完成（2026-11-02 23:35）**：`cargo test --workspace` = **450 passed / 0 failed**（oasd 177、调度专项 77；允许数增加，不允许 existing 变红）；审 diff 完成；已分两次提交（P1 代码 + docs；**不 tag、不 release**；排除 `frames/frames.json`）。
2. **P2 执行编排**：`schedule.rs` 把 `kind="queue"` 接到 `execute_queue_round`；每轮写 `queue_round_started`/`queue_member_finished`/`queue_round_finished`/`queue_finished`/`empty_round`；`scheduler-report.json` 加队列汇总；`next_wake` 接 `queue_next_due_ms` 候选；单成员执行抽小 helper（2 处调用，注释写 DRY 判据）。
3. **P3 → P5**：按计划文档 §三 推进；P5 实机需先启动 MuMu `emulator-5554`。

## 6. 已知开口 / 阻塞

- **阻塞：无**。
- 已知开口（不阻塞）：P2–P5 未开始；墙钟 24h soak 未完成（历史项）；实机验收需 MuMu（当前 `adb devices` 为空）；P1 的 `next_wake` queue 候选与队列执行编排留 P2。

## 7. 证据

- 计划/裁决：`docs/SCHEDULER-RUNNER-PLAN.md`；台账 `docs/PENDING-LEDGER.md §E-75`。
- 事实提取：`D:\OAS-dev\_verify\scheduler-plan-20261102\FACTS.md`（逐项 file:line）。
- P0 文档报告：`D:\OAS-dev\_verify\scheduler-plan-20261102\P0-DOCS-REPORT.md`。
- P1 报告 / 复核：`D:\OAS-dev\_verify\scheduler-plan-20261102\P1-REPORT.md` · `REVIEW-P1.md`。
- F2 文档订正报告：`D:\OAS-dev\_verify\scheduler-plan-20261102\F2-DOCS-FIX-REPORT.md`（逐处 before→after + 残留检查）。
- F3 数字收尾报告：`D:\OAS-dev\_verify\scheduler-plan-20261102\F3-DOCS-NUMBERS-REPORT.md`（逐处 before→after + 残留检查）；Lead 最终实测日志 `lead-cargo-workspace.log`（450/0，21 binary）· `lead-ui-pure.log` · `lead-task-lint.log` · `lead-build-alt2.log`；E2E 复核证据 `e2e/`。
- 调度审计：`D:\OAS-dev\_verify\scheduler-audit-20261102\SCHEDULER-AUDIT.md`（42 项能力 ✅28/🟡7/⬜7）。
- 历史长跑：`D:\OAS-dev\_verify\n42\accel-soak\`（96/96 成立）· `live-soak\`（21 轮即断，未完成）。

## 8. 环境（E11~E14，详见 START-HERE §3）

- PowerShell **5.1** + 执行策略 Undefined ⇒ `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\dev-serve.ps1 -NoOpen`。
- `node` 不在 PATH；bundled `C:\Users\passioki\.dsh\dsh-runtimes\dsh-primary-runtime\dependencies\node\bin\node.exe`（v24.21.0）。
- PS 5.1 重定向日志是 **UTF-16LE**（`-Encoding Unicode` 或转 UTF-8）。
- `adb devices` 为空（MuMu `emulator-5554` 未运行）。
- 中文文件用 `read`/`edit`/`write` 工具，不要用 PowerShell 重写 UTF-8。

## 9. 本节点（seq 91）与上一节点（seq 90，P0 文档）

本节点 seq 91：P1 schema 4 + queue 纯函数 + 评审修复（F1）+ 文档订正（F2）+ 数字收尾（F3），已提交；最终实测见 §1「测试基线」，评审修复项见 `REVIEW-P1.md` 与 `PENDING-LEDGER §E-75`。

上一节点 P0 语义冻结 + 文档/CLI 订正已落：`ENGINE-DESIGN §22.34.7`（四层术语 / Round 10 条 / 四名消歧 / `--config` 陷阱）、`PENDING-LEDGER §E-75`（4 裁决 / P0–P5 / 台账）、`RESUME` seq 90、`RESUME-INDEX` 追加 seq 90、`START-HERE §0.2`、`README`、`SESSION-HANDOFF`、`SCHEDULER-RUNNER-PLAN` 状态行；订正「墙钟 24h soak 未跑完」。更早节点 seq 89 = 0.1.0 Release 上传 + O-1/O-2 收口（`d1a7fcd`），细节见 `docs/RELEASE-NODE-2026-11-02.md` 与 `PENDING-LEDGER.md §E-74`。
