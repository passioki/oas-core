# 会话交接单（SESSION HANDOFF）★ 一页纸

> ★★★ 新会话先读 `docs/START-HERE.md` → `docs/RESUME.md` → `docs/RESUME-INDEX.md` 最新行（seq 94）。
> 当前是 **调度器产品化阶段（单活动实例）**：P0 ✅ 文档（`docs/SCHEDULER-RUNNER-PLAN.md` + ENGINE §22.34.7 + PENDING §E-75）/ P1 ✅ 已提交（450/0，oasd 177）/ P2 ✅ 已提交（474/0，oasd 201；调度专项 101）/ P3 ✅ 已提交（488/0，oasd 215，serve 64，UI 453）/ **P4 ✅ 本轮提交（全量回归 501/0，oasd 228，serve 68，UI 662）** / P5 ⬜（实机验收，需 MuMu）。更早节点：0.1.0 Release 已上传、tag `v0.1.0` 未移动、O-1/O-2 收口（`d1a7fcd`），`dev`/`main` 已同步 origin。不要再灌 34/18 旧素材。

## 0. 当前状态（2026-11-03 12:3x）

| 项 | 值 |
|---|---|
| 当前阶段 | 调度器产品化 + 队列 Round 语义：**P0 ✅ / P1 ✅ / P2 ✅ / P3 ✅ / P4 ✅ 本轮提交（501/0，oasd 228，serve 68，UI 662）/ P5 ⬜**；计划 `docs/SCHEDULER-RUNNER-PLAN.md`，台账 `PENDING-LEDGER §E-75`（§C3 = P2、§C4 = P3、§C5 = P4 最终实测） |
| 工作区（本轮） | P4 代码 + docs 由 Lead 分两次提交；`frames/frames.json` 是**用户数据改动，不属于本轮**，勿动 |
| 分支/HEAD | `dev` = 本轮 docs 提交（HEAD，父 = 本轮 P4 代码提交；P4 代码提交的父 `f99c36a`）；推送状态以 `git status -sb` 实测为准；包构建 commit `42567c1` |
| 恢复节点 | tag `v0.1.0`（annotated `cf123b8` → commit `e8edbef`，**未移动**）；**GitHub Release 已上传** |
| 发布 | `https://github.com/passioki/oas-core/releases/tag/v0.1.0`（id `431080249`，draft=false / prerelease=false，即 latest）；附件 zip（asset `700544587`，37,089,992 B，digest `sha256:e6112a33…5f5687`）+ `.sha256`（asset `700545142`，89 B）；本地同物 SHA256 `E6112A33…5F5687` |
| 版本 | `0.1.0`；**包不含 O-1/O-2**（包构建 `42567c1`，O-1/O-2 自 `54c715b` 起，评审修复 `d1a7fcd`） |
| 测试 | `cargo test --workspace` = **501 passed / 0 failed**（21 个 test binary 全 ok；oasd **228**，P4A 499 + F4P 2；serve **68**；调度专项 **107** = `scheduler.rs` 49 + `schedule.rs` 58；stats 6）；UI pure test **662 checks / 219 referenced ids / 0 dead ids**；task lint 1/0；`node --check` exit 0；`cargo build -p oasd --features ocr` exit 0（alt target）；warning 与基线一致。上一发布基线 426/0（oasd 153 / flow 57 / runtime 125 + 2 integration，13:35 实测） |
| 数据 | 全局素材 **16 条 / 1 图**；帧 **28 个 / current_id=20 / next_id=41** |
| 任务 | 只有 `返回庭院`（flow 11 steps / meta 400 B）；`tasks/测试` 已删除（随 `cd0ac34` 提交） |
| 上一轮工作区 | `d1a7fcd` 时 clean；seq 89 docs 订正（8 份，终态，`906bbed`）与同步回填（`11d53a5`）均已提交 |
| 服务 | serve **8787**：未用主 target 重建/重启（P4 E2E 用 alt 二进制 + e2e 临时 `--runs-dir`，仓库数据未动）；需要 UI 验证时按 START-HERE §1.3 重建并重启 |
| 文档 | 7 份历史文档已归档 `docs/archive/`；顶层 11 份 + `RELEASE-NODE-2026-11-02.md` |

## 1. 最近发生了什么

- `7a3869f`：记录 ResizeObserver 循环修复（`a2b4b12`）；用户 00:37 验收“没问题”。
- `cd0ac34`：**最终恢复节点数据快照**（00:58）——16 素材（新增 `管理分组`）、frames current_id=20、删除 `tasks/测试`。
- `42567c1`：0.1.0 文档归档 / scripts package assets 修复；本地包 01:08 构建（E611…5687）。
- `e8edbef`：docs 定稿为 tag `v0.1.0` target；**GitHub Release 上传**（13:0x，id `431080249`，即 latest）。
- `54c715b`：**O-1 collection 真实点击 + O-2 drag action shape** 初版（6 files +1413/−103）；146/54/119 tests（中间态）。
- `d1a7fcd`：**O-1/O-2 评审修复**（P1-1..P1-4 + P2，6 files +1436/−225）——exact shape 采样 / 409 / plain rect / 溢出校验；最终 **426/259** 回归。
- 本轮 docs 订正 8 份（RELEASE-NODE / PENDING-LEDGER §E-74 / RESUME / RESUME-INDEX / START-HERE / EDITOR-UX-PLAN / README / 本文件）到最终事实——已提交 `906bbed`（其后 docs 同步回填 `11d53a5`；最新 docs 提交 = HEAD），dev 推送 + main 快进同步已完成（13:55）。
- `131d6a7`（22:21）：**调度器产品化 + 队列 Round 语义计划**入库（用户已批准 4 条裁决；`docs/SCHEDULER-RUNNER-PLAN.md`）。
- 22:5x 本轮 **P0 文档**（P0 只改 `docs/`；已随本轮 docs 提交入库）：NEW `ENGINE-DESIGN §22.34.7`（术语表 / Round 10 条 / 四名消歧 / `--config` 陷阱）+ 订正 §22.35 的 24h soak 行 + `PENDING-LEDGER §E-75`（含 N40 行订正）+ `RESUME` seq 90 + `RESUME-INDEX` + `START-HERE §0.2` + `README` + 本文件 + `SCHEDULER-RUNNER-PLAN.md` 状态行；**P1 由另一子进程进行**（schema 3→4 + 纯函数层）。
- 23:0x **P1 代码**：`scheduler.json`/`scheduler.state.json` schema 4（`queues[]`、`entries[].kind`、v3 兼容读 + state 迁移）、8 个 queue 纯函数、`plan_pass`/`status`/`reset-memory` 队列视图；P1 报告见 `_verify/scheduler-plan-20261102/P1-REPORT.md`。评审（`REVIEW-P1.md`）后本轮并入：`--config` 注释订正、state 迁移备份 `<state>.v3.bak`、reset-memory 覆盖 queue entry/轮次、`plan_pass` queue entry→skipped `queue_executor_pending`、queue 状态机保护、v3 显式 kind 拒绝、`to_json` 注释修正（已由本轮 P1 代码提交入库）。
- 23:3x **P1 收尾（F3）**：全量回归 `cargo test --workspace` = **450 passed / 0 failed**（21 binary；oasd **177**；调度专项 **77**）；UI 259/0；lint 1/0；OCR 构建 exit 0（alt target）；E2E 复核通过（queue skipped 语义、`max_passes` 别名、`--max-passes` stop reason、v3 显式 kind 拒绝、state v3→v4 备份 + schema 4）；P1 代码与 docs 分两次提交（不含 `frames/frames.json`）。
- 23:5x–00:5x **P2 实现**（`P2-REPORT.md`）：`plan_pass` 恢复 queue entry 可跑（硬 skip 仅 `queue_finished`/`queue_missing` + 新增 `queue_between`）、`execute_pass` 按 `kind` 分派、`execute_queue_round` 成员串行 + 每步原子 save + 续跑不重复计轮、`record_result` 给 entry 记一次 quota/cooldown；report/status 队列汇总；`queue.rs` 零改动。中间态 464/0、oasd 191、调度 91。
- 01:0x **P2 评审**（`REVIEW-P2.md`）：P0=0；P1-1（续跑分类只看本进程）/ P1-2（end→record 崩溃丢记账）/ P1-3（同一 queue 多 entry 未拒绝）/ P2-1（between 门与 plan/status 不一致）；B5 测试缺口（mark_running / task_missing）。
- 01:3x **F2P 修复并入**（`F2P-REPORT.md`）：`queue_round_summary` 全量重算、`pending_entry_record` 状态机 + 启动补记、one-queue-one-entry 校验、`queue_between` skip；`cargo test --workspace` 464→**474/0**、oasd 191→**201**、调度专项 91→**101**；冒烟（补记只一次 / 续跑分类 / dup exit 2 / between 对齐）通过。
- 01:3x **本轮 docs 收尾 8 份**（本文件 / RESUME seq 92 / RESUME-INDEX / START-HERE / README / PENDING §E-75 §C3 / SCHEDULER-RUNNER-PLAN / ENGINE §22.34.7）随本轮 docs 提交入库（docs 提交 = HEAD，父 = 本轮 P2 代码提交，父 `865724d`）。
- 01:5x–02:0x **P3A 后端**（`P3A-REPORT.md`）：`serve.rs` 新增 `GET/POST /scheduler/config`（validate + 运行中 409 + `.bak` + 原子写）、status kind-aware 过滤（task/queue entry + running/task_choices）、`/task/delete` 队列成员守卫（400 含 `queue_id`）+ kind-aware prune + 失败 500；serve 测试 50→61。
- 01:5x **P3B 前端**（`P3B-REPORT.md`）：任务库「调度设置…」、调度页 entries/队列编辑 + 状态卡 + 折叠高级 JSON、保存 GET-first + 快照比较、状态读取失败 `—` 错误态、空配置默认 `mode:"loop"`/`loop_interval_s:60`；75 个纯函数，UI 428 checks。
- 02:1x **REVIEW-P3**（`REVIEW-P3.md`）：P0=0；P1-1（v4 提交被 `to_json` 规范化丢未知字段）/ P1-4（保存前无 GET-first 快照守卫）/ P2 若干（空配置 mode=stop、过期 `#[allow(dead_code)]`、`.bak` 单份、`utc_offset_min` 等）。
- 02:2x **F3P 修复并入**（`F3P-REPORT.md`）：v4 逐字节保真（仅 v3 走 `to_json` 迁移）、保存 GET-first 快照、空配置 `mode:"loop"`、去掉过期 allow；`cargo test --workspace` 485→**488/0**、oasd 212→**215**、UI 428→**453**、serve 61→**64**；调度专项 101 不变。
- 02:2x **Lead E2E 复核**：alt 二进制 + 真实 serve 8790 + 绝对路径 live 配置——GET 缺失→POST v4（1017B）→GET 读回（queue entry+中文成员名完好）→非法 400 文件不变→锁模拟 409→status kind 正确→`GET /` 含新 DOM id、app.js 含 `/scheduler/config`；仓库根 `scheduler.json` 未被动。
- 02:4x **本轮 docs 收尾 8 份**（本文件 / RESUME seq 93 / RESUME-INDEX / START-HERE / README / PENDING §E-75 §C4 / SCHEDULER-RUNNER-PLAN / ENGINE §22.34.7）随本轮 docs 提交入库（docs 提交 = HEAD，父 = 本轮 P3 代码提交，父 `9f28b02`）。
- 02:3x–02:4x **P4A/P4B**（`P4A-REPORT.md` / `P4B-REPORT.md`）：Run 一等实体落盘（每 attempt 一份、tmp+rename 原子写）、`--runs-dir`、`run_finished` additive `exit_code`/`run_file`/`run_file_error`、新增 `GET /runs/scheduler` + `/runs/scheduler/run`、stats `Summary.scheduler`（attempt 级）、调度页「运行记录」+ 详情框；中间态 499/0、oasd 226、UI 659。
- 02:5x **REVIEW-P4**（`REVIEW-P4.md`）：P0=0（可提交）；P1-1 文档/契约漂移（计划仍写扩 `/runs` + 点击聚合）；P1-2 stats attempt 级 vs report entry 级；P1-3 写失败静默；P2 detail 非实体 JSON/`limit=0`/500 形状等。
- 03:0x **F4P 修复并入**（`F4P-REPORT.md`）：P1-3 写失败可观测（`alert(kind=run_file_write)` + `report.run_file_errors` + 事件 `run_file_error`）、UI 测试 fixture 改真实 `outcome=Done` + 静态安全断言（+3 checks）、stats 口径测试缺口（+2）；`cargo test --workspace` 499→**501/0**、oasd 226→**228**、UI 659→**662**。
- 03:0x **Lead P4 E2E**：alt 二进制 + 真实 serve + e2e `--runs-dir`——queue 2 轮跑出 4 个成员 attempt 实体（task entry 实体由 `schedule.rs` P4 测试覆盖）→ `GET /runs/scheduler`（中文/倒序/筛选）→ `GET /runs/scheduler/run` 完整对象 → 旧 `/runs`、`/runs/clicks`、`/runs/aggregate` 仍 200 → `oasd stats` 含 scheduler 块 → `GET /` 含新 DOM id；证据 `D:\OAS-dev\_verify\scheduler-plan-20261103-p4\e2e\`。
- 03:0x **本轮 docs 子进程中断**（只完成计划文档 §P4）；12:23 Lead 复跑 `cargo test --workspace` **501/0**、UI **662/219/0**（`recovery-workspace.log` / `recovery-ui.log`），与首轮一致；03:0x 前 docs 未完成（PENDING/RESUME/INDEX/START-HERE/README/本文件/ENGINE §22.34.7），由 P4-DOCS-RETRY 补齐。

## 2. 下一步

```text
1. ✅ 本轮已收口：全量回归 501/0（oasd 228、serve 68、UI 662）→ 审 diff（P4 Run 实体 / 事件增量 /
   /runs/scheduler* / stats / 写失败可观测 / UI 运行记录面）→ 由 Lead 分两次提交（P4 代码 + docs；
   不 tag/release；不含 frames/frames.json）→ RESUME seq 94 收尾（本文件与 RESUME/RESUME-INDEX/PENDING/
   SCHEDULER-PLAN/ENGINE 已同步）。
2. P5 实机验收（需 MuMu emulator-5554）：单任务 periodic quota 2/2 + temp 每轮 + replay/实机队列 2 轮
   + 重启恢复 + reset-memory 复位；先启动模拟器。
3. P5 起遗留：点击聚合关联（run.clicks/clicks_file 已有但未接 UI/聚合）、口径统一（事件 attempt/entry、
   stats attempt 级 vs report entry 级、UI 状态卡失败含 resource/blocked）、P4 收口（分页/保留策略、
   run_id 跨进程唯一性、detail 实体校验、list/detail 500 统一、--status 的 run_file_errors 取舍）。
4. 旧开口（不阻塞）：SelectionTool 结构债、真机 collection real=1 E2E、P2-10、drag Plan B。
   P3 遗留：GET 顶层 schema_version 恒 4/UI 强制 4、高级 JSON 无 GET-first 守卫、坏 config 时
   删除守卫无法判定、.bak 单份、同任务多条 task entry、utc_offset_min 缺失按 480。
   P2 遗留：补记恢复时 cycle、不计 session report.runs、member_results 200 环、成员级 exactly-once 副作用。
```

## 3. 关键事实（防止再记错）

- 当前素材是 **16/1**，不是 34/18；34 是 `§E-70` 已推翻的错误恢复，archive 2827/2099 未导入（用户裁定素材可重建）。
- 当前帧 **28 / current_id=20 / next_id=41**，不是 40。
- `tasks/测试` 已删除并提交；task lint 是 **1/0**，不是 2/0。
- `action_roi`/`roi_back` 的 **JSON/UI 读取已删除**；runtime 内部仍保留 `click_back` 作为 region/collection 的 bbox 快捷路径——不要写成“全仓无 roi”。
- **drag/swipe 不写点击分布**：`record_click()` 只在 `perform_click()` 路径调用。
- 拖测 `margin`：默认 **0.15**，合法 **0~0.45**，作用于起点（配置了 `to` 时也作用于终点）候选矩形内缩；**仅 rect 非反选生效**；有 polygon/invert `shape` 时忽略并记 `margin=ignored`。
- **collection 真实点击已终态收口**（dev `d1a7fcd`）：`real` 严格 `0/1/true/false`；`real=1` 强制 live 帧；匹配通过后在**精确 action shape 内采样**（polygon 内/invert 排除区外）；未过阈值 **409**（不点击）。**默认仍是 dry-run**，响应/日志不变；与“素材行真实点击”共用 `perform_live_tap`；**无真机 E2E**。
- **v0.1.0 发布包不含 O-1/O-2**；混版本 runtime 会忽略 drag `shape`（invert 会点到排除区）⇒ invert 拖动需本版 runtime；**P2-10**（保存 flow 不校验解析错误）本轮不做。
- 当前测试基线 **501/228/1-0**（03:0x + 12:23；21 binary；serve 68；调度专项 107 = scheduler 49 + schedule 58；stats 6；UI 662/219/0）；上一发布基线 **426/259/1-0**（oasd 153 / flow 57 / runtime 125 + 2 integration；DSL E2E `linted=8 errors=5`）。
- **P2 起 queue entry 真正执行**：`execute_pass` 按 `entry.kind` 分派；`plan.runnable` 会包含 queue entry，勿再假设 runnable 只有 task；queue entry 的 `task` 是 **queue_id**（不是任务 bundle）。
- **queue 轮次真相在 `state.queues[qid]`**：`current_round.member_index` = 下一个待跑成员；`member_results` 200 环；`pending_entry_record` 是 end→record 的 exactly-once 暂存（正常路径不落盘）。
- **一个 queue 至多一个 entry**（重复 → 配置错误含两个 entry_id）；`queue_between` 是非 open-round 的轮间门，open round 不受门限制（规则 8）。
- 队列 `run_finished` 事件带 `kind:"queue"` + `round` + `clicks` + `quota_used`；report `queues[]` additive（schema 4 未再升）；status entry 有 `queue` 进度对象，旧键保留。
- **P3 config API**：`GET /scheduler/config` 返回 `ok/path/exists/schema_version/config/running/state_path`（只读不拿锁；坏 JSON 400）；`POST /scheduler/config` = body `{config:<完整 scheduler.json>}`，先 `validate_scheduler_with_offset` 后写，运行中 **409**，写前 `scheduler.json.bak`（单份覆盖）+ 原子写。
- **P3 未知字段语义**：提交 **v4** → 只校验不规范化，磁盘 = 提交对象（pretty + 换行），未知顶层/entry/queue 字段保留、GET 可读回；提交 **v3** → 走 `Scheduler::to_json()` 规范化升级 v4（该路径未知字段按迁移语义丢弃）；GET 始终返回磁盘原始对象；GET 顶层 `schema_version` 恒 4（UI 也强制 4，见遗留 P2-1）。
- **P3 status/delete**：status 是 **kind-aware**（queue entry 按 `queues[]` 存在，不看任务目录；running/task_choices 同步）；`/task/delete` 被删任务是队列成员 → **400**（错误含 `queue_id`，目录/资产/配置零改动）；prune 只删 `kind=task` 且命中的 entries + 原子写，失败 500；坏 config 时守卫无法判定（prune 会 500）。
- **P3 UI 编辑面**：任务库「调度设置…」+ 调度页 entries/队列编辑 + 状态卡 + 折叠高级 JSON；保存先 GET 最新 → 快照比较（外部修改中止）→ POST；空配置默认 `mode:"loop"` / `loop_interval_s:60`；纯函数 **75**、UI **453** checks。
- **P4 run 实体**：`<runs_dir>/scheduler/<run_id>.json`，**每次 attempt 一份**（task retry / queue 成员 attempt 各一份）；`run_id=<started_ms>-<seq:04>`；tmp+rename 原子写；`kind=task|queue_member`，`task` 是任务名（queue 成员的任务名，queue 本身在 `queue_id`）；`exit_code` = `RunReport::exit_code()`（0=Done，1=Failed/Exhausted，setup error=1）；`clicks_file` 仅当 `--clicks-dir` 产生点击审计时出现。**实体是 attempt 级、事件是 entry/round 级**。
- **P4 事件增量**：`run_finished` 旧字段全保留，新增 `exit_code`、`run_file`（相对 `runs_dir` 的 `scheduler/<id>.json`，= 最后 attempt；空轮/未执行 `null`）、`run_file_error`（写失败明细；健康 `null`）。
- **P4 API**：新增 `GET /runs/scheduler?limit=200&task=&outcome=`（`{ok,dir,count,runs[]}`，`started_ms` 倒序，limit 默认 200/上限 2000，task/outcome 精确匹配，坏文件跳过，缺目录 200 空）与 `GET /runs/scheduler/run?run_id=`（`{ok,run}`，缺参/非法 400、未知 404、坏 JSON 500）；`run_id` 白名单 `[A-Za-z0-9._-]`；**旧 `/runs`、`/runs/clicks`、`/runs/aggregate`、`/runs/delete` 未改**。
- **P4 stats**：`Summary.scheduler` additive `{files,runs,failed,fatal,total_clicks,total_ms,by_outcome}`，**attempt 级**（failed=retryable、fatal=fatal；resource/blocked 不计失败；files 含坏文件；runs 只计合法实体；坏文件进 alerts）。与 `SchedulerReport` 的 entry/round 级 `failures/fatals` 粒度不同（`retryable→success` 时 stats.failed=1 而 report.failures=0）。`oasd stats` 遇坏 run 文件会因 alerts 非空 exit 1。
- **P4 写失败可观测**：写 run 文件失败**不中断、不改退出码**；`alert(kind=run_file_write)` + `report.run_file_errors`（`scheduler_stop`/`scheduler_summary` 输出 `run_file_errors=N`，`--report` 含）；`--status` 不显示（会话级）。
- **P4 UI**：调度页「运行记录」区（刷新/任务+结果+limit 筛选/清除/状态行/列表；14 个 `page-sched-runs-*` DOM id）+ 详情对话框（再 GET 单条，全字段 + 完整 JSON + 错误全文）；行点击/Enter/Space；状态卡最近运行/失败数；旧 `/runs` 点击分布页未改；纯函数 **111**；新代码全 `textContent`（无 `innerHTML`/`eval`）。
- **runs-dir 一致性**：CLI 与 serve 各用自己的 `--runs-dir`/CWD（默认 `runs`）；两者不一致 ⇒ "调度器写、UI 看不到"。E2E/运维必须同目录。
- 真实 UI 手势：用户 00:37 说已测试；助手独立证据只有真机 `/input/tap`，两者要区分。
- E2E 36/36 的证据边界是 `4d5759f`（23:0x），不含 23:39 点击分布/反选/真实开关与 00:02 RO 修复。

## 4. 活跃的坑

- 不要在未确认前覆盖 `assets/`、`frames/`、`tasks/`；`assets/_deleted-tasks/` 是唯一副本安全网，绝不删。
- 不要用 task 历史再拼“全局库”；不要再跑 `scripts/recover_historical_assets.py`。
- 中文文档/代码用 `read`/`edit`/`write` 工具；PowerShell 直接重写 UTF-8 会 mojibake。
- `_verify/n44/...` 是仓外路径（`D:\OAS-dev\_verify\n44\...`），不是 `oas-core/_verify`。
- `ENGINE §22.39 R11`：唯一 `assets/`，任务只引用全局 id；`ASSET-DATA-MODEL.md` 是数据模型入口。
- 调度 CLI：`--config` 是**全局 live 配置**（被 `main.rs` 在子命令分发前全局摘走并立即解析），**不是**调度配置；调度计划入口是 `oasd scheduler --plan <scheduler.json>`。**误把 `scheduler.json` 传给 `--config` ⇒ exit 2 + `config error: unknown field 'config'`，不会跑任何 plan**；只有 `--config` 传合法 live 配置时，调度 plan 才静默回退默认 `scheduler.json`。
- 调度队列：`scheduler.rs` 的 `queue_*` 纯函数与 `schedule.rs` 的 `execute_queue_round` 分工明确；**不要绕过 `queue_record_member`/`queue_end_round` 直接改 `member_index`/`rounds_completed`**（exactly-once 与 lazy 计数都依赖它们）。**P3 已实现 serve 写入口**：写 `scheduler.json` 前必须 `validate_scheduler_with_offset`（否则 one-queue-one-entry / 成员重复等校验被绕过）；`POST /scheduler/config` 运行中 409，UI 保存前先 GET 最新并比较快照。
- **不要用 `to_json` 重写 v4 配置**：`POST /scheduler/config` 对 v4 只校验不规范化（未知字段保真）；只有 v3 才走 `to_json` 迁移。GET 顶层 `schema_version` 恒 4 是已知遗留（P2-1），未来 >4 文档经 UI 保存会被降级。
- 长期口径：**墙钟 24h soak 未跑完**（`_verify/n42/live-soak` passes=22、`last_stop=""`、events 无 `stop`、lock 残留 pid=1268）；对外只宣称 **96 轮加速 soak** 成立。
- **P4 run 目录**：调度器写 `<runs-dir>/scheduler/`（默认 `runs/scheduler`）；serve 也按**自己的** `--runs-dir` 读。CLI/serve 的 `--runs-dir`/CWD 不一致时 UI「运行记录」为空，不要先怀疑代码。
- **`oasd stats` 与坏 run 文件**：坏 `scheduler/*.json` 只跳过 + 进 alerts，但 alerts 非空会让 `oasd stats` **exit 1**；这不是回归。
- **run 详情契约**：`GET /runs/scheduler/run` 只校验"合法 JSON"，非实体 JSON 也能读出（列表会跳过）；`limit=0` 被 clamp 成 1；list 500 与 detail 500 错误体形状不同（P5 收口）。
- **P4 写失败**：run 文件写失败只影响"可查性"（alert + `report.run_file_errors` + 事件 `run_file_error`），不影响执行/重试/退出码；排查先看 `alert(kind=run_file_write)` 与 `scheduler_run_file_error` 日志。
