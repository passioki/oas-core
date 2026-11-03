# 调度器产品化 + 队列轮次（Round）语义与计划（2026-11-02 · 单活动实例阶段）

> 状态：**设计裁决已由用户批准（2026-11-02 22:5x）**；P0 文档已入库（`ENGINE-DESIGN §22.34.7` + `PENDING-LEDGER §E-75`）；P1 ✅ 已提交（450/0，oasd 177）；P2 ✅ 已提交（474/0，oasd 201；调度专项 101）；P3 ✅ 已提交（488/0，oasd 215；UI 453 checks / 205 ids / 0 dead）；**P4 ✅ 本轮提交（全量回归 501/0，oasd 228；UI 662 checks / 219 ids / 0 dead；serve 68、schedule 58）**；**P5 ⬜ 未开始（实机验收，需 MuMu）**。本阶段约束 = **单活动实例、单机、无跨机任务分发**。本轮代码与 docs 提交以 HEAD 相对表述（P4 代码父 `f99c36a`）；hash 以 `git log` 为准。
> 事实依据：`_verify/scheduler-plan-20261102/FACTS.md`（逐项 file:line）· `_verify/scheduler-audit-20261102/SCHEDULER-AUDIT.md`（能力矩阵 42 项 ✅28/🟡7/⬜7）· `_verify/scheduler-audit-20261102/ORCH8-RESEARCH.md`（外接对比参考）。
> 相关文档：`ENGINE-DESIGN.md §22.34/§22.35` · `PENDING-LEDGER.md §E-73/§E-74` · `RESUME.md`。

---

## 一、裁决：四个"轮次/次数"概念必须分层并改名

现状同时存在 4 个叫 round/battle 的东西，语义互不相同（FACTS §11.2-8）：

| 层 | 裁决名称 | 归属 | 计数对象 | 持久化 | 例 |
|---|---|---|---|---|---|
| L1 | **Goal（完成条件）** | 任务定义（编辑器） | 一次运行内的动作量（打几场/循环几次） | 无（单次运行内） | `goal.battles.count=3` → flow `loop n=param('max_battles')` |
| L2 | **Run / Attempt（执行与重试）** | 执行器 `run::execute` | 1 次任务执行；attempt ≤ retry+1 | 事件 `run_finished`（无 run id） | 失败重试 2 次 = 3 attempts |
| L3 | **Pass / Trigger（调度迭代与触发）** | 调度器守护进程 | 每 pass 每个可跑 entry 至多一次；`entry.quota` 管 day/week 次数 | `scheduler.state.json` | "每天最多 2 次" |
| L4 | **Round（队列编排轮次）** | 队列（使用端） | 把队列成员按序走完一遍的编排尝试 | `scheduler.state.json`（新增 queue 字段） | "日常队列跑 2 轮" |

**命名整顿（P1.x 已完成兼容改名）**：
- `config.loop_rounds` / `--max-rounds` / `state.passes` = **pass 上限**，不是业务轮次。文档与 USAGE 统一称 "pass"；**P1.x 已实现兼容别名**：配置接受 `max_passes`（旧 `loop_rounds` 仍读），CLI 主名 **`--max-passes`**（`--max-rounds`/`--max-runs` 保留为 deprecated 别名），**status 输出键仍为 `loop_rounds`**（配置模型字段 `Scheduler::loop_rounds` 与 state 字段名未改，`to_json` 仍写 `loop_rounds`）。
- `flow param('max_rounds')` = L1 的 loop 迭代次数；**只改文档/模板注释，不改现有任务数据**。
- `queue --rounds` = L4 的 Round，保留。
- `goal.battles.count` = L1，保留。

## 二、Round 语义裁决（10 条，单活动实例）

1. **归属**：Round 只存在于**队列**；单任务 entry 没有 Round（只有 Pass/Trigger）。禁止把 "round" 用在工作台/任务层。
2. **唯一触发器**：队列是调度器 entries 的一种（`entry.kind = "task" | "queue"`，queue 定义放 `scheduler.json` 的 `queues[]`）。调度器窗口/quota/cooldown 决定"该队列今天/本周触发几轮"；**队列内部不再有独立触发器**（消除"队列触发 + 任务触发"两级调度反模式，避免双跑）。
3. **一轮定义**：队首→队尾，对每个 enabled 成员**各做一次决策**（执行或跳过）；成员串行（单活动实例）；结果不影响本轮继续。
4. **准入**：成员执行前只做轻量检查：任务存在、设备/实例可用（单活动实例由守护进程锁 + 单执行保证）、成员未禁用。**队列成员不套调度器窗口/quota/冷却**；"每天几次"由队列 entry 的 quota 表达。**同一任务不得同时出现在 entries 与队列成员中**（校验期拒绝，防双跑）。
5. **Round 计数（lazy）**：本轮**至少一个成员实际执行**（attempt≥1）才 `rounds_completed += 1`；全跳过的一轮记 `empty_rounds += 1`、**不消耗** `total_rounds`（防时段不对空烧轮次），同时记 `empty_round` 事件并设连续空轮告警阈值。
6. **生命周期**：`total_rounds = 0` 表示无限（沿用现有 0=unlimited 约定）；`rounds_completed >= total_rounds` → 队列 `finished`（发 `queue_finished` 事件，之后不再被触发）；**不自动重置**，复位靠 reset-memory 或 UI 手动。
7. **失败策略**：默认 `continue`（记录失败，继续下一个成员/下一轮）；可选 `fail_fast`（本轮中止，仍计 1 round，已执行成员不补偿）。**不做自动重跑失败成员**；"重试本轮失败成员"只作为 UI 手动动作（后续可选项），重试语义仍属 L2 attempt。
8. **持久化与恢复**：queue 状态并入 `scheduler.state.json`（schema 3→4，旧 state 自动迁移并备份 `<state>.v3.bak`、新字段 serde default）：`queues{qid:{total_rounds,rounds_completed,empty_rounds,current_round{index,member_index},member_results[],finished}}`（代码字段是 **`finished: bool`**，不是 `status`；`status_json` 也只输出 `finished`）；每次状态变化原子写；重启后**从 current_round/member_index 继续**，本轮不重复计数（round 计数在轮开始时以 lazy 规则决定是否计入，member_results 追加）；reset-memory 清 queue 状态并复位 finished。
9. **与 Goal 的边界**：调度/队列只回答"跑不跑、跑几次"，**不读取、不推导** `meta.goal`；任务内"打几场"由 flow 决定。禁止"队列 N 轮 × 每轮 M 场"自动相乘当配额——需要时由用户在队列 entry params 里显式传参。
10. **单活动实例**：整机同时 1 个调度器守护进程（现有 `<state>.lock`，注意实际文件名是 `scheduler.state.lock`）+ 1 个执行中 run；队列成员串行；跨天时正在跑的一轮**继续跑完**，不中途切换。编辑器/serve 与守护进程的设备互斥目前只是文字规则（`packaging/README-DEPLOY.md:81`）→ 列为 P2 可选补强（共享 run 互斥文件锁）。

**边界自检（反例）**：
- ❌ 队列成员再配 window/quota → 两级触发、可能双跑 → 禁止（规则 2/4）。
- ❌ 全跳过仍扣 total_rounds → 白天窗口外把轮次烧光 → 禁止（规则 5）。
- ❌ 调度器读 `goal.battles` 算"这轮打了几场" → 跨层耦合 → 禁止（规则 9）。
- ❌ 队列结束自动"明天重置继续" → 不设隐式行为；生命周期结束即结束（规则 6）。

---

## 三、实施计划（P0 → P5，单活动实例）

估算：S ≈ 0.5 人日，M ≈ 1–2 人日，L ≈ 2–3 人日；按"实现子进程 + 独立评审 + Lead 回归"的会话节奏推进。

### P0 · 语义冻结 + 文档/CLI 订正（S，先做）
- 本文件提交；`ENGINE-DESIGN §22.34` 补"术语表 + Round 语义"小节；`PENDING-LEDGER` 新增 §E-75（本计划 + 裁决）。
- 修正已知文档/CLI 失真：
  - `PENDING-LEDGER:1069` / `ENGINE-DESIGN:1834`：24h 墙钟 soak **未跑完**（`live-soak` passes=22、stop=0、lock 残留 pid=1268）；只有 96 轮加速 soak 成立。
  - `oasd scheduler --config X` 失败模式（实测）：`--config` 是**全局 live 配置**（被 `main.rs` 在子命令分发前全局摘走并**立即解析**）；误把 `scheduler.json` 传给 `--config` ⇒ **exit 2** + `config error: config JSON: unknown field 'config', expected one of schema_version/serve/paths/ocr/device/scheduler`，**不会跑任何 plan**（旧文档的"静默回退"说法对 scheduler.json 不成立）。调度计划入口是 **`--plan <scheduler.json>`**；仅当 `--config` 传合法 live 配置时，调度 plan 才回退默认 `scheduler.json`（这才是"静默"的那一半）。`schedule.rs` USAGE 与 `start-scheduler.bat` 说明已改为 `--plan`。
  - 事件清单补 `scheduler_start` / `recovery_dropped`（`schedule.rs:52-56` 漂移）。
- 验收：文档与代码一致；无代码行为变更。

### P1 · schema 升级 + 纯函数层（M，✅ 已提交）

- `scheduler.json` schema 3→4：新增 `queues[]`、`entries[].kind`（缺省 `"task"`）、队列 entry 复用既有 windows/quota/lifecycle/params；`validate_scheduler_with_offset` 扩展校验（队列 id/成员存在/成员不得同时出现在 entries/rounds 上限/total_rounds 语义）。
- `state.json` schema 3→4：新增 queue 字段；`load_state` 兼容读 3（迁移 + 备份 `<state>.v3.bak` + 提示）；`write_json_atomic` 不变。
- 纯函数（`scheduler.rs`，天然可单测）：`queue_should_finish` / `queue_finished` / `queue_next_member_index` / `queue_begin_round` / `queue_record_member` / `queue_end_round` / `queue_next_due_ms` / `queue_reset`（以上为代码实际函数名）；`next_wake` 的 queue 候选 P1 未接（P2）。
- 测试：lazy round 计数、全跳过不消耗、fail_fast、total_rounds=0/∞、跨天、重启恢复、reset 复位、非法配置零写（沿用 validate 语义）。
- 不变量：现有单任务 entries 行为**逐字节等价**（回归测试保护）。
- **状态（2026-11-02 23:3x，已提交）**：实现完成；评审修复已并入——`--config` 注释订正、state 迁移备份 `<state>.v3.bak`、reset-memory 覆盖 queue entry/轮次状态、`plan_pass` 把 queue entry 归入 skipped（reason `queue_executor_pending`）、queue 状态机保护（open round）、v3 显式 `kind` 拒绝、`to_json` 注释修正。测试（Lead 2026-11-02 23:35 实跑）：`cargo test --workspace` = **450 passed / 0 failed**（21 个 test binary 全 ok；oasd **177**，P1 172 + 评审修复 5；调度专项 **77** = `scheduler.rs` 45 + `schedule.rs` 32，P1 72）；UI 259 checks / 0 dead ids；task lint 1/0；`cargo build -p oasd --features ocr` exit 0（alt target）。E2E 复核：queue entry 在 `--status` 中 `runnable_now=false` / `blocked_reason=queue_executor_pending`；`max_passes` 配置别名 → `loop_rounds=2`；CLI `--max-passes 1` → stop reason=`max_rounds`；v3 显式 `kind` → exit 2；state v3→v4 生成 `<state>.v3.bak`（SHA256 一致）、schema=4。

### P2 · 执行编排（M，✅ 本轮提交）

- **已实现（`schedule.rs`）**：`execute_pass` 按 `entry.kind` 分派——Task → `run_entry`；Queue → `execute_queue_round`（一轮计 1 个 `stats.ran`）。
- **准入**：queue entry 与 task entry 走**同一条** window/quota/cooldown/lifecycle 过滤链（`plan_pass`）；硬 skip 只剩 `queue_finished`（规则 6）与 `queue_missing`（防御）；新增 `queue_between`（非 open-round 且距上一轮结束不足 `queue.between_ms`）；open round（`current_round.is_some()`）短路过滤，走 `recover`/`recover_temp` + `resume=true`（规则 8）。
- **Round 执行**：有 `current_round` 续跑（不再 `queue_begin_round`），否则 begin；成员按 `member_index` 串行（轻量预检 `task_missing` → 记 skipped → 推进）；每个成员 `mark_running` 写**成员任务名** + save → 执行 → `queue_record_member` + `clear_running` 同一次 save → 审计；每步原子 `save_state`。
- **成员 attempts** = `1 + entry.retry`（只对 `retryable` 重试、退避 `between_ms`、有 checkpoint 默认 resume）；`continue`（默认）/`fail_fast`（记录当前成员后中止剩余成员，轮仍计 1）/`fatal`（一律中止，`execute_pass` 走 fatal 停机，`--keep-going` 可继续）。
- **入口分类**（以本轮**全部** `member_results` 重算，续跑不丢前成员）：Fatal > 有 Success/Resource → Success（耗 quota）> executed 全失败 → Retryable（cooldown）> 空轮 → Blocked（cooldown）；空轮只 `empty_rounds+1`、不耗 `total_rounds`。
- **exactly-once**：`QueueState.pending_entry_record`；`queue_end_round` + pending 置位同一次 save；`record_result` + 清 pending 同一次 save；启动首 pass 前 `recover_pending_entry_records` 按本轮结果重算并补记一次（事件 `queue_entry_record_recovered`）。
- **校验**：**一个 queue 至多被一个 entry 引用**（重复 → 配置错误，错误文本含两个 entry_id）。
- **事件**：`queue_round_started`、`queue_round_resumed`、`queue_member_started`、`queue_member_finished`、`queue_round_finished`、`empty_round`、`queue_finished`、`queue_missing`、`queue_entry_record_recovered`；每轮另有 `run_finished(kind=queue, round, clicks, quota_used)`。
- **report/status**：`SchedulerReport.queues[]`（additive，schema 4）＋ status entry 的 `queue` 进度对象（rounds/empty/finished/current/last_round）；旧键保留。
- **最终测试（Lead 2026-11-03 01:3x 权威）**：`cargo test --workspace` = **474 passed / 0 failed**（21 个 test binary 全 ok）、`cargo test -p oasd` = **201**；调度专项 **101**（`scheduler.rs` 49 + `schedule.rs` 52，P2 实现 91 → 评审修复 +10）；UI 259 checks / 0 dead ids；task lint 1/0；OCR build（alt target）exit 0；warning 基线无新增。
- **E2E 冒烟（子进程 + Lead 独立复核）**：replay 任务 2 成员串行跑 2 轮 → rounds=2、finished=true、`queue_finished` 事件、report/status 形状正确；崩溃恢复（预置 open round `member_index=1`）只跑剩余成员、rounds 只计 1；pending 补记只一次；重复 queue entry exit 2；`queue_between` 与 `next_due` 一致。
- **遗留**（P3 起处理）：① 补记使用恢复时 cycle（跨逻辑日会把该轮记到当天）；② 补记不计本次 session `report.runs`；③ `member_results` 200 环假设本轮成员 ≤200；④ end→record 已由 pending 状态机修复，但**成员执行中外部杀进程**仍按 checkpoint/resume 重跑（成员级 exactly-once 副作用不保证，属轮级承诺之外）；⑤ P3 已处理：serve 按 kind 过滤、`prune_scheduler_task` kind-aware + 队列成员守卫、UI 编辑 entries/队列/状态卡（见 P3 段）；**P4 ✅ 已处理：Run 一等实体（见 P4 段）**；P5：实机验收（需 MuMu）。可选的 serve/scheduler 共享设备互斥文件锁本轮未做（仍为文字规则）。


### P3 · UI 产品化（M–L，前端为主，✅ 本轮提交）

- **后端配置 API（`serve.rs`）**：新增 `GET /scheduler/config`（200 返回 `ok/path/exists/schema_version/config/running/state_path`；文件缺失 `exists:false`/`config:null`；坏 JSON 400；只读、不拿锁）与 `POST /scheduler/config`（body `{config:<完整 scheduler.json>}`；先 `validate_scheduler_with_offset` 后写；运行中 409；写前 `.bak` 备份 + 原子写）。
- **未知字段语义（最终）**：提交 **v4** → 只校验不规范化，磁盘 = 提交对象（pretty + 换行），未知顶层/entry/queue 字段保留、GET 可读回；提交 **v3** → 仍用 `Scheduler::to_json()` 规范化升级 v4（该路径未知字段按迁移语义丢弃）；GET 始终返回磁盘原始对象；GET 顶层 `schema_version` 恒 4。
- **status kind-aware**：task entry 按任务目录存在过滤、queue entry 按 `queues[]` 存在过滤；`running`/`task_choices` 同步（queue 成员即使无 bundle 也保留在 `task_choices`）。
- **`/task/delete` 队列成员守卫**：被删任务是队列成员 → 400 拒绝（错误含 `queue_id`），目录/资产/配置零改动；prune 改 kind-aware（只删 `kind=task` 且 task 命中的 entries，queue entry/queues 不动）+ 原子写 + 失败明确 500（不再静默吞错）。
- **任务库**：每任务「调度设置…」——无条目→加入调度（默认 entry：`kind=task`、enabled、periodic、全天、day×1、继承 `config.defaults`）；多条→WARN 只编辑第一条；已是队列成员→拒绝（防双跑）。
- **调度页可编辑**：状态卡（mode/passes/next_wake/running 锁/quota/config 状态）、entries 列表（启用/禁用、编辑、删除、上移/下移=执行顺序）、队列面板（新建/编辑/删除、任务池多选 + 有序成员、`total_rounds=0` 无限、`on_failure`、`between_ms`）、「+ 添加条目」、折叠**高级 JSON**（本地 parse 校验后保存）。
- **保存路径**：先 GET 最新配置 → 与加载快照比较（外部修改则中止并提示先重新加载）→ POST；409 提示调度器运行中；400 显示服务端 errors；状态读取失败时状态卡显示「调度器状态读取失败：<err>」且各字段显示 `—`（不误报正常）。
- **默认值/兼容**：新建空配置默认 `mode:"loop"`、`loop_interval_s:60`（其余安全默认）；工作台调度面板保持只读 + 「打开调度页 →」；纯函数 **75** 个（`PURE_FUNCTIONS`），UI 检查 **453**。
- **最终测试（Lead 2026-11-03 02:2x 权威）**：`cargo test --workspace` = **488 passed / 0 failed**（21 binaries）、`cargo test -p oasd` = **215**、serve 测试 **64**（P3A 50→61，F3P +3）；调度专项 **101** 不变；UI pure test **453 checks / 205 referenced ids / 0 dead ids**；task lint **1/0**；OCR build（alt target）exit 0、warning 与基线一致；`node --check app.js/ui_pure_test.js` exit 0。
- **E2E 复核（Lead）**：alt 二进制 + 真实 serve 8790 + 绝对路径 live 配置——GET 缺失 → POST v4（1017B）→ GET 读回（queue entry + 中文成员名完好）→ 非法 400 且文件不变 → 锁模拟 409 `scheduler is running` → status `entries=qe:queue`、plan runnable、`task_choices=返回庭院` → `GET /` 含新 DOM id、app.js 含 `/scheduler/config`；仓库根 `scheduler.json` 未被动。
- **遗留**：① GET 顶层 `schema_version` 恒 4、UI 强制 4（未来 >4 文档保存会被降级，P2-1）；② 高级 JSON 保存路径没有 GET-first 快照守卫（P2-6；仅工具栏按钮有）；③ 坏 `scheduler.json` 时删除前的队列成员守卫无法判定（prune 会 500 明确失败）；④ `.bak` 单份（覆盖式）；无真实浏览器 E2E（仅 API E2E + UI pure tests）；状态读取失败路径仅单测覆盖；⑤ 同任务多条 task entry 仍允许（既定语义；一个 pass 可能跑同一任务两次，UI/文档需提示，后续再议）；⑥ `utc_offset_min` 缺失时 POST 校验按 480，可能覆盖 `--tz-offset-min`（P2-12）；⑦ 无 CSS 新 class（沿用现有样式）；`scheduleTasksCache` 可能过期（P2）；⑧ 无浏览器/真机 E2E；**P4 已完成（见 P4 段）**、P5 实机验收仍待做。

### P4 · Run 一等实体 + 可见性（M，✅ 本轮提交）

- **Run 实体（`schedule.rs`）**：`<runs_dir>/scheduler/<run_id>.json`，**每次 attempt 一份**（task entry 含 retry；queue entry 每个成员 attempt 一份）；`run_id=<started_ms>-<seq:04>`（进程内递增 + 磁盘 `exists()` 防碰撞）；tmp+rename 原子写、pretty + 换行；目录按需创建。字段：`schema_version/run_id/source="scheduler"/kind("task"|"queue_member")/entry_id/task/queue_id?/round?/member_index?/attempt/resume/started_ms/finished_ms/duration_ms/class/exit/exit_code/outcome/clicks/error?/clicks_file?`（`?` = 只在该场景出现）。`exit_code` 来自 `RunReport::exit_code()`（0=Done，1=Failed/Exhausted），无 report 的 setup error 记 1。
- **runs_dir 注入**：`SchedulerOptions.runs_dir` + CLI `--runs-dir`（默认 `runs`）；serve 的 `scheduler_options()` 注入 `self.runs_dir`。**CLI 与 serve 各自使用自己的 `--runs-dir`/CWD**；两者不一致时会出现"调度器写、UI 看不到"（运维须保证同目录，见遗留 6）。
- **事件增量（additive）**：`run_finished` 补 `exit_code`、`run_file`（相对 `runs_dir` 的 `scheduler/<id>.json`；语义 = 该 entry/round 的**最后一次 attempt**，空轮/未执行 = `null`）、`run_file_error`（写失败明细，健康 = `null`）；旧字段全保留。**实体是 attempt 级、事件是条目/轮级**（粒度差异见遗留 1）。
- **写失败可观测（非致命）**：写 run 文件失败不中断执行；但不再静默——`alert` 事件 `kind=run_file_write` + ALERT 行 + `SchedulerReport.run_file_errors: u32`（additive；`scheduler_stop`/`scheduler_summary` 输出 `run_file_errors=N`，`--report` 含该字段）。`--status` 未加该计数（会话级、不持久化）；写失败也**不改变退出码**。
- **API（新增路由，旧 `/runs` 未改）**：`GET /runs/scheduler?limit=200&task=&outcome=` → `{ok,dir,count,runs[]}`（`started_ms` 倒序；limit 默认 200/上限 2000；task/outcome 精确匹配；坏文件跳过；缺目录 200 空列表）；`GET /runs/scheduler/run?run_id=` → `{ok,run}`（完整对象；缺参/非法 id 400、未知 404、读失败/坏 JSON 500；`run_id` 白名单 `[A-Za-z0-9._-]`，拒绝 `.`/`..`/斜杠/盘符/NUL）。**旧 `/runs`、`/runs/clicks`、`/runs/aggregate`、`/runs/delete` 行为未变**。
- **契约与计划偏差（必须记）**：计划原写"`GET /runs` 扩展 + 点击聚合关联"；实际为**新增 `/runs/scheduler*` 路由**（旧 `/runs` 不改以保兼容），run 实体记录 `clicks` 计数与 `clicks_file` 路径，但**未接"点击聚合关联" UI/聚合**（留 P5/后续，见遗留 2）。
- **stats（`stats.rs`）**：`Summary.scheduler` additive `{files,runs,failed,fatal,total_clicks,total_ms,by_outcome}`；**attempt 级口径**——`failed=class retryable`、`fatal=class fatal`，`resource/blocked` 不计失败；`files` 含坏文件、`runs` 只计合法实体；坏文件进 `alerts`。与 `SchedulerReport` 的 **entry/round 级** `failures/fatals` 粒度不同：`retryable→success` 的条目会 `stats.failed=1` 而 `report.failures=0`（必须写清）。
- **UI（调度页「运行记录」区）**：刷新/任务+结果+limit 筛选/清除/状态行/列表（14 个新 DOM id，`page-sched-runs-*`）；详情对话框再 `GET /runs/scheduler/run` 取单条，展示全字段 + 完整 JSON + 错误全文；行点击/Enter/Space 打开；状态卡显示最近运行/失败数；任务库/entry/queue 提供快捷筛选；空/失败态明确（「运行记录读取失败：<err>」+ WARN）。旧 `/runs` 点击分布页未改；纯函数 **111**（P3 75 + P4 36）；新代码全部 `textContent`（无 `innerHTML`/`eval`）。
- **最终测试（Lead 2026-11-03 03:0x 权威）**：`cargo test --workspace` = **501 passed / 0 failed**（21 binaries）、`cargo test -p oasd` = **228**；**既有调度专项 101 不变**（`scheduler.rs` 49 + `schedule.rs` P3 52），P4 新增 6 条（P4A +5、F4P +1）→ `schedule.rs` **58**、合计 **107**；serve 测试 **64 → 68**（P4A +4）；stats 测试 **3 → 6**（P4A +2、F4P +1）；UI pure test **662 checks / 219 referenced ids / 0 dead ids**（P3 453/205 → P4 659/219 → F4P 662/219）；task lint **1/0**；OCR build（alt target）exit 0、warning 与基线一致；`node --check app.js/ui_pure_test.js` exit 0。
- **E2E（Lead，真实 serve + alt 二进制）**：`--runs-dir` 指向 e2e 临时目录，queue 2 轮跑出 4 个成员 attempt 实体（task entry 实体由 `schedule.rs` P4 测试覆盖）→ `GET /runs/scheduler` 列表（含中文 task、倒序、limit/task/outcome 筛选）→ `GET /runs/scheduler/run` 完整对象 → 旧 `/runs`、`/runs/clicks`、`/runs/aggregate` 仍 200 → `oasd stats` 汇总含 scheduler 块 → `GET /` 含新 DOM id；证据 `e2e/`（`api-list.json`/`api-detail.json`/`api-stats.json`/`events.jsonl`/`page.html`）。
- **遗留（P5 起，不阻塞本轮提交）**：① 事件 `run_file` 与实体 attempt 级粒度差异；stats attempt 级 vs `SchedulerReport` entry 级；UI 状态卡"失败"把 resource/blocked 也算失败（P2 口径，待统一）。② 未接点击聚合关联；无分页/保留策略（每次扫描目录全量，attempt 无界增长）。③ `run_id` 唯一性依赖单活动实例 + 磁盘检查（无跨进程锁）；`<id>.json.tmp` 写失败残留不清理（列表忽略）。④ detail 只要求合法 JSON，非实体 JSON 也能读（列表会跳过，契约不一致）；`limit=0` 被 clamp 成 1；list 500 与 detail 500 错误体形状不同。⑤ `--status` 无 `run_file_errors`；写失败不改退出码；queue 写失败未单独测试（共用路径）。⑥ 运维：CLI 与 serve 的 `--runs-dir`/CWD 必须一致；`oasd stats` 遇坏 run 文件会因 alerts 非空 exit 1。⑦ 无真实浏览器/真机 E2E（仅 HTTP API E2E + UI pure tests）；REST 无保留/清理 API。⑧ P5 待做：实机验收（需 MuMu）、点击聚合关联、口径统一、P4 收口项。

### P5 · 验收与收口（S，⬜ 未开始）
- 单元/集成：`cargo test --workspace` 全绿（含新增 round 语义测试；P1 实现 445 → 评审修复后 **450/0**、oasd 177；P2 后 **474/0**、oasd **201**；P3 后 **488/0**、oasd **215**、UI **453**；**P4 后 501/0、oasd 228、UI 662、serve 68、调度专项 107**）；replay 集成（队列 2 轮）；UI pure test。
- 实机（需启动 MuMu）：单任务 periodic quota 2/2 + temp 每轮 + **队列 2 轮** + 重启恢复；`reset-memory` 后复位验证。
- 文档：RELEASE-NODE/RESUME/README/PENDING 更新；版本是否升 0.2.0 由用户定。

### 明确不做（本阶段）
- 多实例/多账号/设备池（需用户改判；当前 ENGINE 非目标 + 10-30 裁定"单活动实例"）。
- 跨机任务分发 / TaskPackage / Registry / 签名（仅跨机需要）。
- 外接 Orch8/Temporal（见 ORCH8 调研：执行层仍须自研 worker，部署税高；只借鉴语义）。
- SQLite 迁移（继续 JSON + 原子写；状态复杂化后再评估）。
- serve 托管调度器子进程（用 bat；后续可选）。

---

## 四、风险与对策
| 风险 | 对策 |
|---|---|
| schema 3→4 数据兼容 | 读 3 自动迁移 + 备份 state；测试覆盖；发布说明写明 |
| 队列与 entries 双跑 | 校验期拒绝"同一任务同时出现在两处"（P1） |
| 队列轮次与 quota 双重计数 | 规则 2：队列触发只有调度器一层；round 计数只在 queue 状态 |
| 空轮烧配额 | 规则 5 lazy 计数 + `empty_rounds` 告警阈值 |
| 单活动实例下并发（编辑器 vs 守护进程） | 现有锁 + 文字规则；P2 可选共享互斥锁；P5 实机同机验证 |
| 文档漂移（soak/CLI/事件清单） | P0 先订正，之后每次改行为同步文档 |

## 五、完成定义（DoD）
- P1/P2：`cargo test --workspace` 全绿（含新增 round 语义测试），旧单任务 entries 回归等价，state 3→4 迁移测试通过。
- P3：UI 可编辑 entries/队列并原子落盘；运行中修改被拒绝（409）；UI pure test 全绿（**453 checks / 0 dead ids**）。
- P4：每次运行有 run_id 落盘，UI 可查失败原因（✅ 实测：全量 **501/0**、oasd **228**、UI **662/219/0**；`GET /runs/scheduler*` + 调度页「运行记录」）。
- P5：实机队列 2 轮 + 重启恢复 + reset 验证通过，文档与台账同步。
