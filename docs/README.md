# OAS-CORE 文档入口

> 本目录是 **oas-core 的独立项目文档**，从旧 backend 文档仓迁移而来。
> 旧仓库 `D:\OAS-dev\backend` 已冻结，仅作为资产/历史资源；oas-core 的构建、运行、测试、发布不依赖它。

## 当前有效文档（顶层 14 份，含发布节点）

| 文件 | 用途 |
|---|---|
| `START-HERE.md` | ★ 新会话唯一入口 |
| `RESUME.md` | 断点续接卡（每次覆盖重写） |
| `RESUME-INDEX.md` | 断点索引（只追加） |
| `SESSION-HANDOFF.md` | 一页交接：当前状态/下一步/坑 |
| `PUSH-ALL.md` | ★ 一键提交/推送脚本 `scripts/push-all.ps1` / `.bat` 使用说明：参数、`git add -A` 范围、瞬态排除（已跟踪不剔除）、只推送补推、main 同步规则、退出码 |
| `RELEASE-NODE-2026-11-02.md` | ★ 当前发布/恢复节点索引：状态、关键 commit、测试、已知开口、release 产物（已回填 GitHub Release） |
| `RUNNER-STUDIO-SPLIT-PLAN.md` | ★ **设计稿 v1（待用户拍板，未实现）**：Studio / Runner 分离（同一 `serve` 加 `Surface` + 路由白名单；`oasd studio` 8788 / `oasd runner` 8789）、TaskPackage `.oaspkg` 发布/安装、任务配置（`meta.params` + `config/values.json` + 四层优先级）、子任务配置（`call_task.params` + `with_params`）、风险 R1、**7 项决策 Q1~Q7**、6 PR、19 步验收 |
| `ENGINE-DESIGN.md` | 引擎规格（§22 为引擎交付/设计；§22.34 调度 v3 已交付；§22.34.7 = 2026-11-02 术语与 Round 语义裁决 + P4 Run 实体/API + P5 实机验收/`auto_connect`；**§22.42 = 素材库 UX + OCR 阈值；§22.43 = 2026-11-04 晚三项遗留修复**） |
| `SCHEDULER-RUNNER-PLAN.md` | ★ 调度器产品化 + 队列 Round 语义（单活动实例）：四层术语、Round 10 条、P0–P5 计划（**P0–P5 已完成**） |
| `PENDING-LEDGER.md` | 未完成台账（当前看 **§E-66~§E-79**；§E-75 = 调度器产品化 + Round 裁决，§C6 = P5 实机验收最终实测；§E-76 = 编辑器帧页；§E-77 = 素材行两按钮 + 一键提交/推送脚本；**§E-78 = 素材库 UX 六项 + OCR 阈值 + 11-04 晚三项遗留修复**；**§E-79 = Studio/Runner 分离 + TaskPackage + 配置/子任务配置 设计稿 v1（待拍板）**） |
| `EDITOR-UX-PLAN.md` | 编辑器 UX/选择模型/实时帧/区域有效性改版（实现后状态 §15；**2026-11-04 帧页现行口径 §16；§16.6 = 布局回归修复；§17 = 素材行两按钮 + 9 项设置菜单；§18 = 素材库 UX 六项 + OCR 阈值（现行）；§18.10 = 三项遗留修复**） |
| `ASSET-DATA-MODEL.md` | 全局 `assets/`、任务素材=flow 引用子集、旧 `library/` 兼容边界 |
| `AUDIT-SUBPROCESS-2026-11-01.md` | 子进程独立复核（范围 21:50 / 361 tests；当前状态见顶部横幅与 §10） |

> **历史文档已归档**到 `docs/archive/`（7 份，`git mv`，未删除）：
> `DESIGN-SPEC.md`、`DESIGN-SPEC-PROGRESS.md`、`SESSION-INDEX.md`、`DELIVERY-PLAN.md`、
> `RECOVERY-AUDIT.md`、`CODE-AUDIT-2026-11-01.md`、`MIGRATION-NOTES.md`。
> 归档原因/替代文档见 `archive/README.md`；`session-memory.config.json` 的
> `spec`/`progress`/`index` 已改为 `archive/...`（只读历史输入，不是现行设计权威）。

## 当前基线（2026-11-02 发布节点；调度器更新至 2026-11-03 19:0x；编辑器/脚本/遗留修复更新至 2026-11-04 18:2x）

- ★ **2026-11-04 18:2x（seq 100）**：① **三项遗留已修**（代码未提交，父 `f16fea1`）——region 识别区保存**不再**把操作多边形降级为矩形（`keep_action_shape`）、模板 ROI **移动**保留 polygon（`assetRoiPolygonPlan` 平移）、OCR **取值路径**应用阈值（`ocr_value_read`；低分 → `Null`/空串、`wait_until` 轮询保留、多行最低行分）；② **设计稿 v1 待拍板** —— `RUNNER-STUDIO-SPLIT-PLAN.md`（Studio/Runner 分离 + TaskPackage + 任务配置/子任务配置；7 决策 Q1~Q7、6 PR、19 步验收；**未实现**），台账 `§E-79`。测试 **527/0**（runtime 142 / task 63 / oasd 229）、UI **1138/227/0**。

- 版本：`0.1.0`（`oasd --version` = 0.1.0）；**发布节点时** `dev` = `main` = `origin/dev` = `origin/main`（已同步 origin，2026-11-02 13:55 实测；实质代码提交 `54c715b` + `d1a7fcd`，docs 终态订正提交 `906bbed`，其后为 docs 同步回填提交 `11d53a5`）；tag `v0.1.0` → `e8edbef`（**未移动**）；**调度器产品化 P0–P5 已完成**（P5 本轮提交，见下行测试与 §维护规则 6）。
- 发布：**GitHub Release 已上传** [`v0.1.0`](https://github.com/passioki/oas-core/releases/tag/v0.1.0)（release id `431080249`，draft=false / prerelease=false，即 latest）；附件 `oasd-v0.1.0-win64.zip`（37,089,992 B，digest `sha256:e6112a33…5f5687`）+ `.sha256`（89 B）；本地同物 `dist/oasd-v0.1.0-win64.zip`，SHA256 `E6112A33…5F5687`；**包构建 commit `42567c1`（不含 O-1/O-2）**。
- 当前任务：只有 `返回庭院`（flow 11 steps / meta 400 B）；`tasks/测试` 已删除。帧 **28 个**，`current_id=20`，`next_id=41`；**唯一关系 = `Asset.positions`（帧-素材）**。
- 全局素材：**16 条 / 1 图**（15 `ocr_line` + 1 `template`；全部 `action_source=fixed`，有 `action_shape` 与 `positions`）。34/18 是 11-01 错误恢复（§E-70 已推翻）；`archive/migration-20261101` 的 2827 素材 / 2099 图 / 57 个任务目录是权威迁移归档，但未导入当前 dev（用户裁定素材可重建）。
- 测试：上一发布基线 `cargo test --workspace` = **426 passed / 0 failed**（oasd 153 / flow 57 / runtime 125 + 2 integration）；UI pure test = **259 checks / 0 dead ids**；task lint = **1/0**；DSL E2E `linted=8 errors=5`（2026-11-02 13:35 实测；**v0.1.0 发布包是 393/203 时构建**）；E2E 36/36（证据边界：`4d5759f`，不含 23:39 点击分布/反选/真实开关与 00:02 RO 修复）。调度器 P4/P5 已完成（P5 本轮提交）：`cargo test --workspace` = **501 passed / 0 failed**（21 个 test binary 全 ok；oasd **228**；serve **68**；调度专项 **107** = `scheduler.rs` 49 + `schedule.rs` 58；stats 6；P5 未新增测试）；UI pure test **662 checks / 219 referenced ids / 0 dead ids**；task lint 1/0；OCR 构建 exit 0（alt target，warning 与基线一致）；`node --check` exit 0。**2026-11-04 编辑器帧页 + 布局回归（只改 UI，无 Rust 改动）**：`cargo test --workspace` 仍 **501/0**（oasd **228**；serve 68）；UI pure test **835 checks / 225 referenced ids / 0 dead ids**（布局回归 828→835，+7）；`node --check` exit 0；task lint **1/0**。**2026-11-04 seq 98（素材行两按钮 + push-all，无 Rust 改动）**：workspace 仍 **501/0**（oasd 228；serve 68）；UI pure test **907/225/0**（835→888→907，+72）；脚本 AST **5714/0**、隔离集成 **59/0**。**2026-11-04 下午 seq 99（素材库 UX 六项 + OCR 阈值）**：`cargo test --workspace` **516 passed / 0 failed**（runtime **136** / task **59** / oasd **228**；serve 68；501→512→516，新增 15 条）；UI pure test **1112 checks / 227 referenced ids / 0 dead ids**（907→1003→1112）；`cargo build -p oasd --features ocr` exit 0（6 个既有 warnings）；task lint **1/0**；`node --check` 3/3；`git diff --check` clean。**2026-11-04 18:2x seq 100（三项遗留修复）**：`cargo test --workspace` **516 → 527 passed / 0 failed**（runtime **136→142**、task **59→63**、oasd **228→229**；+11 = OCR 取值路径 6（runtime）+ region keep_action_shape 2 与模板 polygon 平移 2（task）+ serve 布尔解析 1（oasd））；UI pure test **1138 checks / 227 referenced ids / 0 dead ids**（1112→1138，+26）；`node --check` 3/3 = 0；`git diff --check` = 0。P4 前 P3 为 488/0（oasd 215，serve 64，UI 453）；P2 中间态为 474/0（oasd 201）。
- 编辑器首轮 + 验收修复：16:9 stage、帧详情侧栏、结果/日志收起、随机点测/拖测、`action_shape`+`action_source`、操作区多边形/全屏反选、runtime exact shape、素材拖动排序、帧分类重命名、live/position 批量测试、点击分布（只画点位/热力）、真实/演练开关、ResizeObserver 循环修复。**2026-11-04 帧页**：帧搜索（名称/`#id`/分类，纯前端不改 `/frames`）、当前帧素材列表（全局+任务 `positions` 命中、按 id 去重、共享 `assetRowElement` 三处复用）、素材测试帧错位修复（单素材测试=当前选中帧；批量=所有位置帧；live 恒优先；单行 OCR 无真实检测框）、**布局回归修复**（帧列表被固定内容挤成 0 高 → `min-height:200px` + `#frame-details` 默认折叠 + 点帧展开 + 空搜索隐藏状态行；CDP 1440×900 实测 0→257px）。**2026-11-04 seq 98 素材行**：三处素材行统一「识别 + 设置」两按钮 + 行内 9 项菜单（点测/拖测/识别区/操作区/重命名/阈值/位置帧/删除/步骤）；新增 `editAssetRoi`/`renameAsset`/`editAssetThreshold`，删旧 `editAsset`/`onEdit`；template 识别区、非 template 阈值、region 操作区、帧详情 `manage:false` 6 管理项、集合仅删除均禁用；320px 两按钮同排（**该段禁用口径已被 seq 99 取代**）。**2026-11-04 下午 seq 99**：行选中态按 asset id 跨 surface 高亮 + 菜单项点击不收起/重渲染恢复；识别区改**画布拖拽**（旧 prompt 删除），保存按钮移到共享 `#canvas-footer` +「识别区编辑中：<id>」；模板识别区启用，保存走 `/task/save_asset` + `replace:true`（当前帧重截、rev+1），OCR/region 走 ROI PATCH；**OCR 每素材最低置信度**（未设/0=不过滤）；帧详情去来源列/badge、`manage:true`、按来源分 scope；批量帧测试移到素材页、对象=当前可见集合；评审 P0×2/P1×4 全修 + P2。
- 已回退：interface 抽象、`ui-map/`、`Frame.interface_id`、界面图视图（用户裁定不需要）。
- 帧安全：清空列表必须确认；清空前自动备份到 `frames/.backup/clear-<时间>/`（含 frames.json + PNG）。
- **调度器产品化（已完成：P0–P5，2026-11-03）**：计划入口 `SCHEDULER-RUNNER-PLAN.md`（P0 ✅ 文档 / P1 ✅ 450/0，oasd 177 / P2 ✅ 474/0，oasd 201 / P3 ✅ 488/0，oasd 215，UI 453 / P4 ✅ 501/0，oasd 228，serve 68，UI 662 / **P5 ✅ 实机验收 A–F 全部真机通过 + `GET /instance/devices` adb 自动连接修复**）；Round 语义见 `ENGINE-DESIGN §22.34.7` / `PENDING-LEDGER §E-75`（§C3 = P2、§C4 = P3、§C5 = P4、**§C6 = P5 最终实测**）；**墙钟 24h soak 未跑完**（只有 96 轮加速 soak 成立；`_verify/n42/live-soak` passes=22、无 `stop`、lock 残留 pid=1268）。
- 已知开口（调度器产品化 **P0–P5 已完成**；以下 = 产品 backlog）：O-1/O-2 经两轮评审在 `d1a7fcd` 收口；**结构债只剩** `SelectionTool` 未抽出；collection `real=1` **真机 E2E 未做**（P5 后 MuMu `127.0.0.1:16384` 已可用，可补）；**混版本风险**=旧 runtime 忽略 drag `shape`（invert 会点到排除区，需本版 runtime）；**P2-10**（保存 flow 不校验解析错误）明确不做；模板移动/多位置仍是生成时 shape 快照（Plan B 未做）；活动页参考帧未补齐；第二轮评审残留 P2：P2-12 显式闭合环 polygon 误判自交、P2-11 极端 `distance` 溢出（既有）、P2-13~16 措辞/截断，详见 `D:\OAS-dev\_verify\final-review-20261102\REVIEW2.md`。**P3 已收口**：serve 按 kind 过滤（task/queue entry + `running`/`task_choices`）、`prune_scheduler_task` kind-aware + 队列成员守卫（400 含 `queue_id`）、UI entries/队列编辑 + 状态卡 + 高级 JSON；**P3 遗留**：GET 顶层 `schema_version` 恒 4/UI 强制 4（>4 文档保存降级）、高级 JSON 无 GET-first 快照守卫、坏 config 时删除守卫无法判定（prune 500 明确失败）、`.bak` 单份、同任务多条 task entry 仍允许、`utc_offset_min` 缺失按 480、无 CSS 新 class/`scheduleTasksCache` 可能过期。**P4 已收口**：Run 一等实体（每 attempt 一份、tmp+rename 原子写）、`run_finished` additive `exit_code`/`run_file`/`run_file_error`、新增 `GET /runs/scheduler*`（旧 `/runs*` 未变）、stats `Summary.scheduler`（attempt 级）、写失败可观测（`alert(kind=run_file_write)` + `report.run_file_errors`）、调度页「运行记录」+ 详情；**P4 遗留（转 backlog）**：事件 attempt/entry 粒度差异、stats attempt 级 vs report entry 级、UI 状态卡失败含 resource/blocked、点击聚合关联未接、无分页/保留策略、run_id 无跨进程锁、detail/list 契约差异、`--status` 无 `run_file_errors`、runs-dir/CWD 须一致、无独立浏览器 E2E。**P2 遗留**：补记使用恢复时 cycle、补记不计本次 session `report.runs`、`member_results` 200 环假设成员 ≤200、成员执行中杀进程不保证成员级 exactly-once 副作用。**P5 已完成**（实机 A–F + `GET /instance/devices` 自动 `adb connect`/`auto_connect`；详见 §E-75 §C6）；**新主线 = 产品 backlog**：`SelectionTool` 抽取、collection `real=1` 真机 E2E、drag Plan B、P2-10、点击聚合关联、分页/保留、run_id 跨进程唯一、detail 契约、口径统一、版本决策（0.2.0 未升）。**2026-11-04 帧页遗留（P2/P3）**：帧搜索无 Enter/防抖、分类+搜索状态计数跨分类、纯数字查询既精确又子串、全局模式下来源可能重复计数、`frame_source` 文案（`position:#N` vs「当前帧 #N」）、切帧只清测试框不清 ROI、单行 OCR 测点画布无框（P2 沟通项）、`renderAssetRows` DRY 复制、仅 headless 截图 + CDP 布局实测（无真实浏览器自动化 E2E）；**seq 99 新增**：~~`region` 识别区保存仍把 polygon/invert 降级为矩形（P2-5）、模板 ROI 移动后 polygon 有意重同步为矩形、取值路径不加阈值~~ ⇒ **seq 100 已修**（见下行）、legacy 非全局批量测试仍需任务名、帧详情 task-only 无任务报错（P2-10）、菜单恢复极端多 surface 可能开在最后可见 surface、无真实浏览器自动化 E2E；**seq 100 保留遗留**：模板 `fixed` 操作区不随 ROI 平移、ROI **缩放**时 polygon 回退矩形、OCR 低分为空结果（非硬错误）、UI 阈值文案未加「同时作用于取值」；**seq 100 待拍板**：`RUNNER-STUDIO-SPLIT-PLAN.md` 的 7 项决策（Q1~Q7），未拍板前不动代码；**seq 98 遗留**：脚本控制台中文编码依赖本机（PS 5.1）、`library/` 未纳入版本控制、脚本未在真实仓库实测 push 失败后的分支恢复（隔离 59/0 覆盖逻辑）。
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
6. 调度器产品化已完成：P0 ✅ / P1 ✅（450/0，oasd 177）/ P2 ✅（474/0，oasd 201）/ P3 ✅（488/0，oasd 215，UI 453）/ P4 ✅（501/0，oasd 228，UI 662）/ **P5 ✅（实机 A–F；含 adb 自动连接；501/0，oasd 228，UI 662/219/0）**；**seq 99 = 素材库 UX 六项 + OCR 阈值（516/0，UI 1112/227/0）；seq 100 = 三项遗留修复（527/0，UI 1138/227/0）**；下一步 = **用户拍板 `RUNNER-STUDIO-SPLIT-PLAN.md §D.2` 的 7 项决策**（Q1~Q7）→ 按 PR1→PR6 实施；未拍板前不动代码。**改分离/包格式/配置优先级/子任务语义 → 先改 `RUNNER-STUDIO-SPLIT-PLAN.md`（及稿源）再改代码**；改调度/队列语义先改计划文档与 `ENGINE-DESIGN §22.34.7`，再改代码。
7. 一键提交/推送：`scripts\push-all.ps1`（`scripts\push-all.bat` 包装，透传参数/退出码）——`-DryRun` 预览、`-Yes` 免确认、`-NoPush` 只提交；默认**仅 `dev` 自动同步 `main`**（其它分支用 `-SyncMain`），远程无 main 则 WARN 跳过；**已跟踪文件不会被当作瞬态剔除，`library/` 被 `.gitignore` 忽略不会提交**；详见 `PUSH-ALL.md`。

## 迁移来源

- 源仓库：`https://github.com/passioki/OnmyojiAutoScript.git`（旧 OAS backend）
- 源提交：`2861d8ba`（v1.2.3 发布文档节点）
- 迁移日期：2026-11-01
- 详细对照：`archive/MIGRATION-NOTES.md`（历史归档）
