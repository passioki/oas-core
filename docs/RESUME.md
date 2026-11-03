# RESUME — 断点续接卡（session-memory 强制 6）

> 规则：本文件每次断点覆盖重写；≤150 行；另一个会话只读本文件 + `RESUME-INDEX.md` 最新行
> 就能继续。细节放 `ENGINE-DESIGN.md` / `SCHEDULER-RUNNER-PLAN.md` / `PENDING-LEDGER.md`。

```yaml
task_id: oas-core-scheduler-productization
checkpoint_seq: 95
updated_at: 2026-11-03T19:0x+08:00
trigger: 调度器产品化 P5 实机验收收口（A periodic quota / B temp / C 队列 2 轮 / D 重启崩溃续跑 / E reset-memory / F UI/API 可见性）+ `/instance/devices` adb 自动连接修复；501/0（oasd 228，serve 68，UI 662）
status: done
```

## 0. 目标（已完成）

把**调度器产品化 + 队列 Round 语义**（单活动实例阶段）按 P0→P5 落地：P0 语义冻结、P1 schema 4 + 纯函数、P2 队列 Round 执行编排、P3 UI 产品化、P4 Run 一等实体、P5 实机验收与收口——**P0–P5 全部完成（P5 本轮提交）**。

计划与裁决全文：**`docs/SCHEDULER-RUNNER-PLAN.md`**（术语表 + Round 10 条 + P0–P5 + 风险/DoD）。

## 1. 当前状态（2026-11-03 19:0x）

| 项 | 值 |
|---|---|
| 分支/HEAD | `dev`：HEAD = 本轮 docs 提交（父 = 本轮 P5 代码提交；**P5 代码提交的父 `9134a70`**）；hash 以 `git log` 为准（不写死）；未 tag/release |
| 任务状态 | **done**（P0–P5 全部完成；checkpoint **95**） |
| 测试基线 | `cargo test --workspace` = **501 passed / 0 failed**（21 个 test binary 全 ok；oasd **228**；serve **68**；调度专项 **107** = `scheduler.rs` 49 + `schedule.rs` 58；`stats.rs` 6）；UI pure test **662 checks / 219 referenced ids / 0 dead ids**；task lint **1/0**；`node --check` exit 0；`cargo build -p oasd --features ocr` exit 0（alt target；warning 与基线一致）。上一发布基线 426/153（seq 89，见 `RELEASE-NODE-2026-11-02.md`） |
| 实机 | MuMu **`127.0.0.1:16384`**（Android 12 / V2284A / 720x1280，Onmyoji `com.netease.onmyoji` 前台庭院）；`adb devices` = device；**P5 A–F 全部真机通过** |
| 上一节点 | seq 94：P4 Run 一等实体 + 运行可见性（501/0、oasd 228、serve 68、UI 662） |
| 本阶段约束 | **单活动实例、单机、无跨机任务分发**（多实例/多账号/设备池明确不做） |
| 工作区 | P5 代码（`apps/oasd/src/serve.rs` adb 自动连接）+ docs 由 Lead 分两次提交；`frames/frames.json` / `scheduler.json`（P3 UI 编辑产物）/ `scheduler.json.bak` 是**用户数据，不提交、不要动** |

## 2. 阶段看板（P0–P5）

| 阶段 | 内容 | 状态 |
|---|---|---|
| **P0** | 语义冻结 + 文档/CLI 订正（ENGINE §22.34.7、PENDING §E-75） | ✅ 完成（seq 90） |
| **P1** | `scheduler.json`/`scheduler.state.json` schema 3→4 + queue 纯函数 | ✅ 已提交（seq 91；450/0，oasd 177） |
| **P2** | 执行编排：`execute_queue_round` 成员串行 + 每步原子 save；`pending_entry_record` exactly-once；report/status 队列汇总 | ✅ 已提交（seq 92；474/0，oasd 201） |
| **P3** | UI 产品化：`GET/POST /scheduler/config`、任务库「调度设置…」、调度页 entries/队列编辑、状态卡、status kind-aware、删除守卫 | ✅ 已提交（seq 93；488/0，oasd 215，UI 453） |
| **P4** | Run 一等实体：`runs/scheduler/<run_id>.json`（每 attempt 一份）+ 事件增量 + `/runs/scheduler*` + stats + 写失败可观测 + 调度页运行记录 | ✅ 已提交（seq 94；501/0，oasd 228，serve 68，UI 662） |
| **P5** | 验收收口：实机 A–F + adb 自动连接修复 + 文档收口 | ✅ 本轮提交（seq 95；501/0，oasd 228，serve 68，UI 662） |

## 3. P5 实机验收摘要（2026-11-03，Lead 实机权威）

- **A periodic quota**：单任务 `返回庭院` 每日 quota **2/2** → 第 1、2 次真实运行均 `Done`（~**7.2s**，OCR 识别「已在庭院」）；第 3 pass `scheduler_skip reason=quota_exhausted`。
- **B temp**：连续 **2 pass 各跑一次**（每次 `scheduler_temp_reset_before/after`），runs=2 / successes=2；运行后 `tasks/返回庭院/state/` 为空（重置生效）。
- **C 队列 2 轮**：`q1`（1 成员）2 轮成功 → `rounds_completed=2`、`finished=true`、`queue_finished`；第 3 pass `queue_finished` skip；run 实体 `kind=queue_member`（queue_id=q1/round/member_index/exit_code=0）落盘。
- **D 重启恢复（崩溃续跑）**：成员执行中**强杀守护进程**（state 保留 `running` + `current_round{1,0}`）→ 重启日志 `scheduler_queue_round_resume` 从 `member_index=0` 续跑、round **只计 1**；随后 round 2/3 完成（`rounds_completed=3 finished=true`）。
- **E reset-memory**：`reset_memory_entries=1`、`reset_memory_queues=q1`；entries/queues/running 与 `tasks/返回庭院/state/` 清空 + 审计事件；periodic 条目同样复位。
- **F UI/API 可见性**：serve 8787 `GET /runs/scheduler` = **9 条真实运行**（全部 success/`Done`、exit_code=0、约 7.1–7.3s）；`/stats` scheduler 块 `files=9 runs=9 failed=0 fatal=0 by_outcome{Done:9} total_ms≈65185`；调度页「运行记录」可查。
- **实机顺手修复**：`GET /instance/devices` 列表为空时**自动 `adb connect`**（配置 serial → live config `device.reconnect_endpoints` → 默认 `127.0.0.1:16384`）后重列，响应新增 `auto_connect`；实测 `adb disconnect` 后检测自动连回 16384。

## 4. 下一步 = 产品 backlog（P5 之后，按需排序）

1. **`SelectionTool` 抽取**（结构债，唯一剩下的 P1 级重构）。
2. **collection `real=1` 真机 E2E**（O-1 遗留；MuMu 现已可用，可补）。
3. **drag Plan B**（asset 引用/运行时跟随识别，替代生成时 shape 快照）。
4. **P2-10**：保存 flow 时校验解析错误（此前明确不做）。
5. **口径统一**：事件 attempt/entry 粒度、stats attempt 级 vs `SchedulerReport` entry 级、UI 状态卡“失败”含 resource/blocked。
6. **运行记录收口**：点击聚合关联（run `clicks`/`clicks_file` ↔ 点击分布/聚合）、分页/保留策略（attempt 无界增长）、`run_id` 跨进程唯一、detail 实体校验、list/detail 500 统一、`--status` 的 `run_file_errors` 取舍。
7. **P3 遗留**：GET 顶层 schema_version 恒 4/UI 强制 4、高级 JSON 无 GET-first 快照守卫、坏 config 时删除守卫无法判定（prune 500）、`.bak` 单份、同任务多 task entry、`utc_offset_min` 缺失按 480。
8. **P2 遗留**：补记使用恢复时 cycle、补记不计本次 session `report.runs`、`member_results` 200 环假设、成员执行中杀进程不保证成员级 exactly-once。
9. **版本决策**：是否升 0.2.0（由用户定；本轮未升、未 tag/release）。另：**墙钟 24h soak 仍未跑完**（只有 96 轮加速 soak 成立），对外宣称前需补跑。

## 5. 证据

- **P5 实机**：`D:\OAS-dev\_verify\p5-realdevice-20261103\`（`periodic.*` · `temp.*` · `queue.*` · `resume.*` · `reset-*.txt` · `ui-runs.json` · `ui-stats.json` · `instance-autoconnect.json` · `workspace-test.log` · `test-osd.log` · `ui-test.log` · `build.log` · `lint.log` · `device-final.png`）。
- **P5 文档收尾**：`D:\OAS-dev\_verify\p5-realdevice-20261103\P5-DOCS-REPORT.md`（逐处 before→after + 残留检查）。
- **台账**：`docs/PENDING-LEDGER.md §E-75`（**§C6 = P5 最终实测**；§C5 = P4、§C4 = P3、§C3 = P2、§C2 = P1）。
- **计划/裁决**：`docs/SCHEDULER-RUNNER-PLAN.md`；`docs/ENGINE-DESIGN.md §22.34.7`。
- **历史**：P4 `D:\OAS-dev\_verify\scheduler-plan-20261103-p4\`（P4A/P4B/REVIEW-P4/F4P/e2e）；P3 `scheduler-plan-20261103-p3\`；P2/P1 `scheduler-plan-20261102\`；调度审计 `_verify\scheduler-audit-20261102\SCHEDULER-AUDIT.md`；长跑 `_verify\n42\accel-soak\`（96/96 成立）· `live-soak\`（21 轮即断，未完成）。

## 6. 环境（E11~E14，详见 START-HERE §3）

- PowerShell **5.1** + 执行策略 Undefined ⇒ `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\dev-serve.ps1 -NoOpen`。
- `node` 不在 PATH；bundled `C:\Users\passioki\.dsh\dsh-runtimes\dsh-primary-runtime\dependencies\node\bin\node.exe`（v24.21.0）。
- PS 5.1 重定向日志是 **UTF-16LE**（`-Encoding Unicode` 或转 UTF-8）。
- 实机：MuMu 运行中，serial **`127.0.0.1:16384`**（不是 `emulator-5554`）；`adb devices` 为空时 UI「检测实例」会**自动 connect 16384**（P5 新增，响应带 `auto_connect`），仍失败再查 adb server / MuMu 设置。
- 中文文件用 `read`/`edit`/`write` 工具，不要用 PowerShell 重写 UTF-8。

## 7. 本节点（seq 95）与上一节点（seq 94）

本节点 seq 95：**P5 实机验收收口**——A–F 全部真机通过（periodic quota 2/2、temp 每轮、队列 2 轮、崩溃续跑、reset-memory、UI/API 可见性）+ `/instance/devices` adb 自动连接修复；测试维持 **501/0**、oasd **228**、serve **68**、UI **662/219/0**；**任务标 done（P0–P5 完成）**，遗留转产品 backlog。

上一节点 seq 94：P4 Run 一等实体 + 运行可见性（run 文件/事件增量/`/runs/scheduler*`/stats/写失败可观测/UI 运行记录）+ F4P，501/0、oasd 228、serve 68、UI 662/219/0；更早 seq 93 = P3 UI 产品化（488/0）、seq 92 = P2 队列 Round（474/0）、seq 91 = P1 schema 4 + 纯函数（450/0）、seq 90 = P0 文档、seq 89 = 0.1.0 Release + O-1/O-2 收口（`d1a7fcd`）。
