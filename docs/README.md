# OAS-CORE 文档入口

> 本目录是 **oas-core 的独立项目文档**，从旧 backend 文档仓迁移而来。
> 旧仓库 `D:\OAS-dev\backend` 已冻结，仅作为资产/历史资源；oas-core 的构建、运行、测试、发布不依赖它。

## 当前有效文档（顶层 11 份 + 发布节点）

| 文件 | 用途 |
|---|---|
| `START-HERE.md` | ★ 新会话唯一入口 |
| `RESUME.md` | 断点续接卡（每次覆盖重写） |
| `RESUME-INDEX.md` | 断点索引（只追加） |
| `SESSION-HANDOFF.md` | 一页交接：当前状态/下一步/坑 |
| `RELEASE-NODE-2026-11-02.md` | ★ 当前发布/恢复节点索引：状态、关键 commit、测试、已知开口、release 产物（已回填 GitHub Release） |
| `ENGINE-DESIGN.md` | 引擎规格（§22 为引擎交付/设计；§22.34 调度 v3 已交付；§22.34.7 = 2026-11-02 术语与 Round 语义裁决，含 P4 Run 实体/API） |
| `SCHEDULER-RUNNER-PLAN.md` | ★ 调度器产品化 + 队列 Round 语义（单活动实例）：四层术语、Round 10 条、P0–P5 计划（当前主线） |
| `PENDING-LEDGER.md` | 未完成台账（当前看 §E-66~§E-75；§E-75 = 调度器产品化 + Round 裁决） |
| `EDITOR-UX-PLAN.md` | 编辑器 UX/选择模型/实时帧/区域有效性改版（含实现后状态 §15） |
| `ASSET-DATA-MODEL.md` | 全局 `assets/`、任务素材=flow 引用子集、旧 `library/` 兼容边界 |
| `AUDIT-SUBPROCESS-2026-11-01.md` | 子进程独立复核（范围 21:50 / 361 tests；当前状态见顶部横幅与 §10） |

> **历史文档已归档**到 `docs/archive/`（7 份，`git mv`，未删除）：
> `DESIGN-SPEC.md`、`DESIGN-SPEC-PROGRESS.md`、`SESSION-INDEX.md`、`DELIVERY-PLAN.md`、
> `RECOVERY-AUDIT.md`、`CODE-AUDIT-2026-11-01.md`、`MIGRATION-NOTES.md`。
> 归档原因/替代文档见 `archive/README.md`；`session-memory.config.json` 的
> `spec`/`progress`/`index` 已改为 `archive/...`（只读历史输入，不是现行设计权威）。

## 当前基线（2026-11-02 发布节点；调度器状态更新至 2026-11-03 12:3x）

- 版本：`0.1.0`（`oasd --version` = 0.1.0）；**发布节点时** `dev` = `main` = `origin/dev` = `origin/main`（已同步 origin，2026-11-02 13:55 实测；实质代码提交 `54c715b` + `d1a7fcd`，docs 终态订正提交 `906bbed`，其后为 docs 同步回填提交 `11d53a5`）；tag `v0.1.0` → `e8edbef`（**未移动**）；调度器当前 P4 已实现（本轮提交，见下行测试与 §维护规则 6）。
- 发布：**GitHub Release 已上传** [`v0.1.0`](https://github.com/passioki/oas-core/releases/tag/v0.1.0)（release id `431080249`，draft=false / prerelease=false，即 latest）；附件 `oasd-v0.1.0-win64.zip`（37,089,992 B，digest `sha256:e6112a33…5f5687`）+ `.sha256`（89 B）；本地同物 `dist/oasd-v0.1.0-win64.zip`，SHA256 `E6112A33…5F5687`；**包构建 commit `42567c1`（不含 O-1/O-2）**。
- 当前任务：只有 `返回庭院`（flow 11 steps / meta 400 B）；`tasks/测试` 已删除。帧 **28 个**，`current_id=20`，`next_id=41`；**唯一关系 = `Asset.positions`（帧-素材）**。
- 全局素材：**16 条 / 1 图**（15 `ocr_line` + 1 `template`；全部 `action_source=fixed`，有 `action_shape` 与 `positions`）。34/18 是 11-01 错误恢复（§E-70 已推翻）；`archive/migration-20261101` 的 2827 素材 / 2099 图 / 57 个任务目录是权威迁移归档，但未导入当前 dev（用户裁定素材可重建）。
- 测试：上一发布基线 `cargo test --workspace` = **426 passed / 0 failed**（oasd 153 / flow 57 / runtime 125 + 2 integration）；UI pure test = **259 checks / 0 dead ids**；task lint = **1/0**；DSL E2E `linted=8 errors=5`（2026-11-02 13:35 实测；**v0.1.0 发布包是 393/203 时构建**）；E2E 36/36（证据边界：`4d5759f`，不含 23:39 点击分布/反选/真实开关与 00:02 RO 修复）。调度器 P4 已实现（本轮提交）：`cargo test --workspace` = **501 passed / 0 failed**（21 个 test binary 全 ok；oasd **228**；serve **68**；调度专项 **107** = `scheduler.rs` 49 + `schedule.rs` 58；stats 6）；UI pure test **662 checks / 219 referenced ids / 0 dead ids**；task lint 1/0；OCR 构建 exit 0（alt target，warning 与基线一致）；`node --check` exit 0。P4 前 P3 为 488/0（oasd 215，serve 64，UI 453）；P2 中间态为 474/0（oasd 201）。
- 编辑器首轮 + 验收修复：16:9 stage、帧详情侧栏、结果/日志收起、随机点测/拖测、`action_shape`+`action_source`、操作区多边形/全屏反选、runtime exact shape、素材拖动排序、帧分类重命名、live/position 批量测试、点击分布（只画点位/热力）、真实/演练开关、ResizeObserver 循环修复。
- 已回退：interface 抽象、`ui-map/`、`Frame.interface_id`、界面图视图（用户裁定不需要）。
- 帧安全：清空列表必须确认；清空前自动备份到 `frames/.backup/clear-<时间>/`（含 frames.json + PNG）。
- **调度器产品化（进行中，当前主线；P5 验收收口）**：计划入口 `SCHEDULER-RUNNER-PLAN.md`（P0 ✅ 文档 / P1 ✅ 已提交（450/0，oasd 177）/ P2 ✅ 已提交（474/0，oasd 201）/ P3 ✅ 已提交（488/0，oasd 215，UI 453）/ P4 ✅ 本轮提交（501/0，oasd 228，serve 68，UI 662）/ P5 ⬜（实机验收，需 MuMu））；Round 语义见 `ENGINE-DESIGN §22.34.7` / `PENDING-LEDGER §E-75`（§C3 = P2、§C4 = P3、§C5 = P4 最终实测）；**墙钟 24h soak 未跑完**（只有 96 轮加速 soak 成立；`_verify/n42/live-soak` passes=22、无 `stop`、lock 残留 pid=1268）。
- 已知开口（11-02 终态收窄 + P3/P4 遗留）：O-1/O-2 经两轮评审在 `d1a7fcd` 收口；**结构债只剩** `SelectionTool` 未抽出；collection `real=1` **真机 E2E 未做**（MuMu 未运行、`adb devices` 为空）；**混版本风险**=旧 runtime 忽略 drag `shape`（invert 会点到排除区，需本版 runtime）；**P2-10**（保存 flow 不校验解析错误）明确不做；模板移动/多位置仍是生成时 shape 快照（Plan B 未做）；活动页参考帧未补齐；第二轮评审残留 P2：P2-12 显式闭合环 polygon 误判自交、P2-11 极端 `distance` 溢出（既有）、P2-13~16 措辞/截断，详见 `D:\OAS-dev\_verify\final-review-20261102\REVIEW2.md`。**P3 已收口**：serve 按 kind 过滤（task/queue entry + `running`/`task_choices`）、`prune_scheduler_task` kind-aware + 队列成员守卫（400 含 `queue_id`）、UI entries/队列编辑 + 状态卡 + 高级 JSON；**P3 遗留**：GET 顶层 `schema_version` 恒 4/UI 强制 4（>4 文档保存降级）、高级 JSON 无 GET-first 快照守卫、坏 config 时删除守卫无法判定（prune 500 明确失败）、`.bak` 单份、同任务多条 task entry 仍允许、`utc_offset_min` 缺失按 480、无 CSS 新 class/`scheduleTasksCache` 可能过期。**P4 已收口**：Run 一等实体（每 attempt 一份、tmp+rename 原子写）、`run_finished` additive `exit_code`/`run_file`/`run_file_error`、新增 `GET /runs/scheduler*`（旧 `/runs*` 未变）、stats `Summary.scheduler`（attempt 级）、写失败可观测（`alert(kind=run_file_write)` + `report.run_file_errors`）、调度页「运行记录」+ 详情；**P4 遗留（P5 起）**：事件 attempt/entry 粒度差异、stats attempt 级 vs report entry 级、UI 状态卡失败含 resource/blocked、点击聚合关联未接、无分页/保留策略、run_id 无跨进程锁、detail/list 契约差异、`--status` 无 `run_file_errors`、runs-dir/CWD 须一致、无真机 E2E。**P2 遗留**：补记使用恢复时 cycle、补记不计本次 session `report.runs`、`member_results` 200 环假设成员 ≤200、成员执行中杀进程不保证成员级 exactly-once 副作用。
- 本地备份：`D:\OAS-dev\_backups\oas-core-20261102-005826\`；历史 docs 归档 `docs/archive/`（7 份，`git mv`，未删）。

## 维护规则

1. 所有文档更新都在 **oas-core 仓库**完成，不再回写 backend 文档仓。
2. 文档随 `oas-core dev` 提交；0.1.0 恢复节点已打 tag `v0.1.0` 并上传 GitHub Release；**发布节点时** `dev`/`main` 已同步 origin（2026-11-02 13:55 实测）；此后 P4 代码 + docs 提交以 `git log` 为准。
3. 新会话固定：
   - 工作目录 `D:\OAS-dev\oas-core`
   - 先加载 skill `session-memory`
   - 再读 `docs/START-HERE.md`
4. skill 配置：仓库根目录 `session-memory.config.json`（`spec`/`progress`/`index` 已指向 `docs/archive/`）。
5. 旧 backend 文档仍位于 `D:\OAS-dev\backend\docs`，仅供历史查证；其中的 `DEV-WORKFLOW.md`、`LAYOUT.md`、旧架构/验收文档已不迁入。
6. 调度器产品化按 `docs/SCHEDULER-RUNNER-PLAN.md` 推进（P0 ✅ / P1 ✅ 已提交（450/0，oasd 177）/ P2 ✅ 已提交（474/0，oasd 201）/ P3 ✅ 已提交（488/0，oasd 215，UI 453）/ P4 ✅ 本轮提交（501/0，oasd 228，UI 662）/ P5 ⬜ 实机验收）；改调度/队列语义先改计划文档与 `ENGINE-DESIGN §22.34.7`，再改代码。

## 迁移来源

- 源仓库：`https://github.com/passioki/OnmyojiAutoScript.git`（旧 OAS backend）
- 源提交：`2861d8ba`（v1.2.3 发布文档节点）
- 迁移日期：2026-11-01
- 详细对照：`archive/MIGRATION-NOTES.md`（历史归档）
