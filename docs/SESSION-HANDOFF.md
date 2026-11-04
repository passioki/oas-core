# 会话交接单（SESSION HANDOFF）★ 一页纸

> ★★★ 新会话先读 `docs/START-HERE.md` → `docs/RESUME.md` → `docs/RESUME-INDEX.md` 最新行（seq 102，`oas-core-runner-studio-split`：Q1~Q7 已批准 + PR1 ✅ 已提交 + PR2 ✅ 已实现（未提交）、PR3~PR6 待做，in_progress）。
> **本轮（2026-11-04 19:5x，seq 102）**：**PR2 ✅ 已实现并验证（代码未提交，父 `da9da9d`）**——`meta.params` 五字段（label≤80/description≤500/enum/required/group≤80）+ `validate_param_values`；`config/values.json`（Runner 独占、原子 RMW、保留其它任务、坏文件带路径）；Studio `GET/POST /task/config`（只替换 params + 保留其它字段 + `.bak`）；Runner `GET/PUT /runner/tasks/<id>/config`（400 errors 含 key）；Studio 配置项 JSON 编辑器 + Runner 配置表单；workspace **562/0**、oasd **255**、task **72**、UI **1338/231/0**；Runner 包 **37,192,710 B**；E2E 临时 `pr2demo`（Studio 8790 / Runner 8791）。**上一轮 seq 101 已提交 `da9da9d`（见下行）。**
> **上一轮（2026-11-04 19:2x，seq 101，已提交 `da9da9d`）**：① 用户批准 §D.2 的 **Q1~Q7（全部按推荐）**；② **PR1 ✅**——`oasd studio` 8788 / `oasd runner` 8789 / `serve` deprecated、`Surface` 路由归属（不匹配纯 404）、Runner 只读 `/runner/health`+`/runner/tasks`、`runner.html/js/css` 5 nav、Studio 去调度入口、Runner-only 包 37,141,295 B；③ 分发纠正（终端用户只下载 Runner-only 包 + 任务包）；④ 跨平台（v1 只 Windows，Android 使用端预留）。再往前 seq 100：设计稿 v1 落盘 + 三项遗留（527/0、UI 1138/227/0，详见 `PENDING-LEDGER §E-78/§E-79`）。
> **上一阶段**：调度器产品化（单活动实例）P0–P5 全部 ✅ 完成（seq 95，done）——P0 文档 / P1 450/0 / P2 474/0 / P3 488/0，UI 453 / P4 501/0，oasd 228，UI 662 / **P5 实机验收 A–F 全部真机通过 + `GET /instance/devices` adb 自动连接修复**。更早节点：0.1.0 Release 已上传、tag `v0.1.0` 未移动、O-1/O-2 收口（`d1a7fcd`），`dev`/`main` 已同步 origin。不要再灌 34/18 旧素材。

## 0. 当前状态（2026-11-04 19:5x）

| 项 | 值 |
|---|---|
| 当前阶段 | **Studio/Runner PR2（seq 102，in_progress，`oas-core-runner-studio-split`）**：Q1~Q7 已批准；**PR1 ✅ 已提交（`da9da9d`）**；**PR2 ✅ 已实现（未提交）**——`meta.params` 五字段 + `validate_param_values`、`config/values.json`、Studio/Runner 配置 API/UI、值校验；**PR3~PR6 待做**（下一步 = PR3 优先级与类型保真）。台账 `§E-79`（PR1 = §F、PR2 = §G）；现行口径 `RUNNER-STUDIO-SPLIT-PLAN §A.9/§A.10` + `ENGINE §22.44/§22.45` + `packaging/CROSS-PLATFORM.md`。上一阶段调度器产品化 P0–P5 ✅（seq 95，done；`§E-75`） |
| 工作区（本轮） | PR2 代码/UI 由并行子进程写入（**不是我改的**）：`crates/oas-task/src/{meta.rs,lib.rs}`、`apps/oasd/src/{runner_config.rs(新),serve.rs,main.rs}`、`.gitignore`；UI `apps/oasd/ui/{index.html,app.js,runner.html,runner.js,runner.css,ui_pure_test.js}`。本子进程只改 **docs 8 份**。用户数据 `assets/assets.json`、`frames/frames.json`、`scheduler.json`、`scheduler.json.bak`、`tasks/切换预设御魂/` **不属于本轮提交，勿动** |
| 分支/HEAD | `dev`；当前 HEAD = **`da9da9d`**（PR1 + seq 101 docs）；**PR1 ✅ 已提交；PR2 代码/UI 未提交，父 = `da9da9d`**，本批 docs 随后（新 HEAD）；推送状态以 `git status -sb` 实测为准 |
| 恢复节点 | tag `v0.1.0`（annotated `cf123b8` → commit `e8edbef`，**未移动**）；**GitHub Release 已上传** |
| 发布 | `https://github.com/passioki/oas-core/releases/tag/v0.1.0`（id `431080249`，draft=false / prerelease=false，即 latest）；附件 zip（asset `700544587`，37,089,992 B，digest `sha256:e6112a33…5f5687`）+ `.sha256`（asset `700545142`，89 B）；本地同物 SHA256 `E6112A33…5F5687` |
| 版本 | `0.1.0`；**包不含 O-1/O-2**（包构建 `42567c1`，O-1/O-2 自 `54c715b` 起，评审修复 `d1a7fcd`） |
| 测试 | 本轮（seq 102）：`cargo test --workspace` **562/0**（PR1 538，**+24** = meta 9 + runner_config 7 + serve 8）；oasd **255/0**（240+15）、task **72/0**（63+9）、runtime **142** 不变；`cargo build -p oasd --features ocr` exit 0（6 warnings，与基线一致）；UI pure test **1338 checks / 231 referenced ids / 0 dead ids**（PR1 1239/224/0，+99；`runner_ui_audit` **41 ids / 0 dead / 5 nav**）；`node --check` 3/3；`git diff --check` clean（本子进程只做 docs，未跑 cargo/serve；以上为 Lead 实测）。seq 101：oasd 240/0、workspace 538/0、UI 1239/224/0；seq 100：527/0、UI 1138/227/0；seq 99：516/0（runtime 136 / task 59 / oasd 228），UI 1112/227/0；seq 98：501/0，UI 907/225/0；上一发布基线 426/0 |
| 数据 | **已提交基线**：全局素材 16 条 / 1 图；帧 28 个 / current_id=20 / next_id=41。**工作区未提交用户数据（2026-11-04 12:5x 实测）**：全局素材 **27 条**（5 模板 + 22 `ocr_line`）、图 **5 张**（a001~a005，a002~a005 未跟踪）；帧 **30 个** / current_id=1 / next_id=43；seq 96 时帧素材映射为 #23=3、#26=1、#41=2、#42=2（后续数据可能再变） |
| 任务 | 只有 `返回庭院`（flow 11 steps / meta 400 B）；`tasks/测试` 已删除（随 `cd0ac34` 提交） |
| 上一轮工作区 | `d1a7fcd` 时 clean；seq 89 docs 订正（8 份，终态，`906bbed`）与同步回填（`11d53a5`）均已提交 |
| 服务 | seq 96 用 serve **8787**（真机 `emulator-5554`，实时预览可用）：`GET /` 200 含 5 个新 DOM id；`/frames`=30、`/task/assets?task=`=26；`match` API 见 `PENDING-LEDGER §E-76 §B`；headless 截图 `_verify/ui-frames-20261103/editor-page.png`；**布局回归另用 CDP 1440×900 实测**（修复前帧列表 0px → 修复后 257px；`frames-fixed.png` / `frames-fixed-detail.png`）。上一节点 P5 用 8787 测 `/runs/scheduler`；P5 的 adb 自动连接需重建 serve 才生效。仓库根 `scheduler.json`/`frames/frames.json` 是用户数据，未提交；**seq 98 未启动 serve/cargo**；**seq 99 用真实 serve 8787 + device 做 CDP 实测**（选中态/共享保存/模板 title/OCR prompt/帧详情/批量按钮；截图 `D:\OAS-dev\_verify\asset-ux-20261104\ux-roi-shared.png` 等）；**seq 101 PR1 E2E：真实 Studio 8788 + Runner 8789 双进程**（两端归属 404 / 共享只读 200 / Runner health 实际 `port:8789`、`scheduler_mode:"loop"`、`lock.held=false`；截图 `_verify/runner-studio-pr1-20261104/`）；**seq 102 PR2 E2E：临时任务 `pr2demo`，Studio 8790 / Runner 8791**（Studio GET/POST config 200、meta 其它字段保留、`.bak`；Runner GET/PUT config 200、中文 `结界` 往返、非法值 400 含 key；两端 config 404；CDP 双端 UI；截图 `_verify/runner-studio-pr2-20261104/studio-config.png` / `runner-config.png`） |
| 文档 | 7 份历史文档已归档 `docs/archive/`；顶层 **14 份**（含 `PUSH-ALL.md`、`RELEASE-NODE-2026-11-02.md`、**`RUNNER-STUDIO-SPLIT-PLAN.md`（新）**） |

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
- 03:0x **本轮 docs 子进程中断**（只完成计划文档 §P4）；12:23 Lead 复跑 `cargo test --workspace` **501/0**、UI **662/219/0**（`recovery-workspace.log` / `recovery-ui.log`），与首轮一致；当时未完成的 docs 由 P4-DOCS-RETRY 补齐（`9134a70`，seq 94）。
- 本轮 **P5 实机验收（Lead，MuMu `127.0.0.1:16384`）**：A 单任务 periodic quota **2/2**（2 次真实运行 Done ~7.2s，OCR「已在庭院」，第 3 pass `scheduler_skip reason=quota_exhausted`）；B temp 连续 2 pass 各一次（每次 `scheduler_temp_reset_before/after`，runs=2/successes=2，运行后 task `state/` 为空）；C 队列 `q1` 2 轮（`rounds_completed=2`/`finished=true`/`queue_finished`，第 3 pass `queue_finished` skip，run 实体 `kind=queue_member`）；D 成员执行中**强杀守护进程** → 重启日志 `scheduler_queue_round_resume` 从 `member_index=0` 续跑、round 只计 1 次 → round 2/3 完成（`rounds_completed=3`）；E `reset-memory`（`reset_memory_entries=1`/`reset_memory_queues=q1`，entries/queues/running 与 task `state/` 清空 + 审计，periodic 同样复位）；F serve 8787 `/runs/scheduler` **9 条真实运行** + `/stats` scheduler `files=9 runs=9 failed=0 fatal=0 by_outcome{Done:9} total_ms≈65185`。证据 `D:\OAS-dev\_verify\p5-realdevice-20261103\`。
- **实机顺手修复（随 P5 提交）**：`serve.rs` 的 `GET /instance/devices` 原来只列 `adb devices`，MuMu 重启后 adb TCP 未 attach 时 UI「检测实例」显示“没有检测到 adb 实例”；现改为**列表为空时自动 `adb connect`**（配置 serial → live config `device.reconnect_endpoints` → 默认 `127.0.0.1:16384`）后重列，响应新增 `auto_connect`；实测 `adb disconnect` 后检测自动连回 16384（`instance-autoconnect.json`）。提交信息 `fix(serve): auto-connect adb endpoint on instance detection`。
- **提交前 Lead 复跑（权威）**：`cargo test --workspace` **501/0**、oasd **228**、serve **68**、UI **662/219/0**、task lint **1/0**、OCR build exit 0。**P5 未新增测试**（501 与 P4 相同）。
- 本轮 **docs 收尾 8 份**（RESUME seq 95 / RESUME-INDEX / START-HERE / README / 本文件 / PENDING §E-75 §C6 / SCHEDULER-RUNNER-PLAN / ENGINE §22.34.7）随本轮 docs 提交入库；docs 提交 = HEAD，P5 代码提交的父 `9134a70`。
- 2026-11-03 23:4x~11-04 00:0x **编辑器帧页需求①②**（`FRAMES-UI-REPORT.md`）：帧搜索 `#frame-search`（纯前端、名称/#id/分类）+ 帧详情素材库 `#frame-assets-list`（全局+任务、按 id 去重、共享 `assetRowElement`）；UI 662→**744** checks。
- 2026-11-04 00:0x **诊断需求③根因**（`MATCH-BUG-DIAGNOSIS.md`）：实时关闭时 `testFrames` 优先 `positions` 忽略当前帧 ⇒ 永远测静态截图，框=存储 ROI；后端 `/task/asset/match` 正确（按 frame_id 重匹配）。实测 `frame_id=23` → 1.0 @ (1208,68)；`frame_id=26` → 0.9933 @ (1082,75)。
- 00:1x **修复需求③**（`MATCH-FIX-REPORT.md`）：`testFramePlan`（live > 显式 frameId > 当前帧 > positions）、五测试函数透传、切帧清框、`matchBoxValid`、单行 OCR 无真实检测框；UI 744→**802** checks。
- 00:2x **只读评审**（`REVIEW-UI.md`）：P0=0，P1×2（P1-1 全局库变更后帧视图不刷新；P1-2 旧文档口径与实现相反 → 本轮 docs 同步）；P2 若干；**00:3x F-UI 修复**（`FIX-P1-REPORT.md`）：`renderFrameAssetViews()` 本地重绘（0 额外请求/0 递归）、`fallbackFullscreenLines` 透传 frameId、删除当前帧清框；UI 802→**828** checks。
- 本轮 **E2E（Lead）**：serve 8787 真机 `emulator-5554`；`GET /` 200 含 5 个新 DOM id；30 帧 / 26 全局素材（4 模板 + 22 `ocr_line`）；帧素材映射 #23=3、#26=1、#41=2、#42=2；headless Edge 截图 `editor-page.png`。**无真实浏览器自动化 E2E**。
- 2026-11-04 00:5x **帧页布局回归修复**（Lead，CDP 1440×900）：用户报告「图片页丢失按分类展示帧的区域」；根因 = 上一轮新增搜索框/状态行/帧详情/该帧素材（约 +296px 固定内容）+ `#panel-images` 固定高 flex（`calc(100vh - 150px)`）+ `frames-list { flex:1; min-height:0 }` ⇒ 1440×900 下列表计算高度 **0px**（12 组 / 30 行仍在，`scrollHeight=1832` 不可见）。修复 4 文件：`min-height:200px` + `overflow-y:auto`；`#frame-detail`/`#frame-assets-status`/`#frame-assets-list` 包进默认折叠 `<details id="frame-details">`（summary「帧详情 / 该帧素材（点击一帧展开）」）；`selectFrame()` 成功即 `details.open=true`；空搜索隐藏 `#frame-search-status`；UI +7 checks。修复后列表 **257px**、12 组可见；点帧 #1 → details 打开、「该帧素材 1 条（全局 1 · 任务 0）」、列表仍 200px；截图 `frames-fixed.png` / `frames-fixed-detail.png`。

- 2026-11-04 12:0x~12:5x **上一轮（seq 98）需求②素材行两按钮**（`ASSET-ROW-REPORT.md` → `REVIEW.md` → `FIX-REPORT.md`）：三处素材行统一「识别 + 设置」；设置 = 行内 9 项（点测/拖测/识别区/操作区/重命名/阈值/位置帧/删除/步骤）；新增 `editAssetRoi`（prompt `x,y,w,h`）/`renameAsset`/`editAssetThreshold`，删旧 `editAsset` + 两处 `onEdit`；禁用：非 template 阈值、template 识别区、region 操作区、`manage:false` 6 管理项、集合仅删除；320px 两按钮同排；UI 835→888→**907** checks。**识别区 prompt / template 识别区 disabled / 帧详情只读等口径已被 seq 99 的 §E-78 取代。**
- 2026-11-04 12:0x~12:5x **上一轮（seq 98）需求①一键提交/推送脚本**：新增 `scripts/push-all.ps1`（PS 5.1、UTF-8 BOM、1004 行）+ `scripts/push-all.bat`（透传参数/退出码）+ `docs/PUSH-ALL.md`；评审 P1-1（模板识别区死动作）、P1-2（任意分支自动同步 main）与 P2 修复；AST **5714/0**；真实仓 `-DryRun` ps1/bat exit 0 且前后 HEAD/index/status 一致；隔离副本 **59/0**。
- 2026-11-04 12:5x **上一轮 docs 收尾 8 份**（`PUSH-ALL` 核对补充 / `EDITOR-UX-PLAN §16.2`+新增 §17 / `PENDING §E-77` / `RESUME` seq 98 / `RESUME-INDEX` / `START-HERE` / `README` / 本文件）；逐处 before→after 见 `D:\OAS-dev\_verify\asset-row-20261104\DOCS-REPORT.md`。**代码/脚本/docs 均未提交**。

- 2026-11-04 14:0x~14:5x **本轮（seq 99）素材库 UX 六项 + OCR 阈值**（`UX-REPORT.md` → `OCR-THRESHOLD-REPORT.md` → `REVIEW.md` → `FIX-REPORT.md` → `THRESH-FIX-REPORT.md`）：行选中态/菜单保持；识别区画布 + 共享 `#canvas-footer`；模板 replace 重截；OCR 每素材最低置信度；帧详情去来源列 + `manage:true` + 按来源 scope；批量测试移素材页 + 可见集合；评审 P0×2/P1×4 + P2 全修；UI 1003→**1112/227/0**（评审时 1003/226/0）、workspace 512→**516/0**；Lead CDP 真实 serve 8787 实测；代码未提交（父 `b4c2b0c`）。
- 2026-11-04 14:5x **本轮 docs 收尾 7 份**（`EDITOR-UX-PLAN §16.2/§17` 订正 + 新增 §18 / `PENDING §E-78` + §E-76/§E-77 取代说明 / `ENGINE §16.5/§22.42` / `RESUME` seq 99 / `RESUME-INDEX` / `START-HERE` / `README` / 本文件）；逐处 before→after 见 `_verify/asset-ux-20261104/DOCS-REPORT.md`。**代码/docs 均未提交**。

- 2026-11-04 17:0x~18:1x **三项遗留修复（seq 100，代码未提交，父 `f16fea1`）**：① `oas-task::update_asset_meta` 新增可选布尔 **`keep_action_shape`**（`true` + `kind=region` 时只更新 `roi`，不动 `action_shape`/`action_source`；缺省仍走旧 B-08 rect 同步），serve `/task/asset/update` 解析该字段（**非布尔 400**），UI region 保存传 `true` 并提示「操作区未改」，补 region ROI 帧内校验；② UI 新增 **`assetRoiPolygonPlan`**：模板 ROI **移动**时按 `dx/dy` 平移 polygon 并校验「平移后 bbox == 新 ROI 且不越界」，模板 `replace` payload 带平移后 polygon（服务端按 polygon bbox 反推 roi）；**仅缩放 / 旧 bbox 不一致 / 越界**回退矩形并明确提示；③ 新增 **`ocr_value_read`**：`ocr()` / `ocr_digit()` / `ocr_to_var` 接入阈值（显式 `t>0` 且低分 → `Null` / 空串，低分文本不进值；未设/0 不变；`wait_until` 轮询保留；多行取**最低行分**；模板/collection 不受影响）。测试 workspace **516→527/0**（runtime 136→142、task 59→63、oasd 228→229）、UI **1112→1138/227/0**。
- 2026-11-04 18:1x **事项 2/3 设计稿 v1**（只读审计产出，659 行 / 235+ `file:line`）：`_verify/arch-runner-studio-20261104/RUNNER-STUDIO-CONFIG-DESIGN.md`；已整理为 **`docs/RUNNER-STUDIO-SPLIT-PLAN.md`**（Studio/Runner 分离 = 同一 `serve::Server` 加 `Surface` + 路由归属表；`oasd studio` 8788 / `oasd runner` 8789；`.oaspkg` = zip+manifest+checksums；`meta.params` 扩 5 字段 + `config/values.json` + 四层优先级；`call_task.params` + `with_params` 作用域 + lint T7；风险 **R1 = call_task 是 load-time 展开**；**7 项决策 Q1~Q7**；**6 PR**；**19 步验收**）。**状态 = 设计完成、未实现、当时待批准**；台账 **§E-79**。（该状态已被同日 19:2x 的 seq 101 取代：Q1~Q7 已批准、PR1 ✅ 已实现。）
- 2026-11-04 18:2x **上一轮 docs 收尾 9 份**（新建 `RUNNER-STUDIO-SPLIT-PLAN.md`；`EDITOR-UX-PLAN §18.3/§18.4/§18.8/§18.9` 订正 + 新增 **§18.10**；`ENGINE §16.5` 取值路径改写 + 新增 **§22.43**；`PENDING §E-78` 订正 + 新增 **§E-79**；`RESUME` seq 100 / `RESUME-INDEX` 100 / `START-HERE` / `README` / 本文件）；逐处 before→after + 残留检查见 `_verify/leftovers-20261104/DOCS-REPORT.md`。**代码/docs 均未提交**。
- 2026-11-04 19:0x~19:2x **本轮（seq 101）PR1 实现 + 验证 + 打包**（代码/脚本/UI 未提交，父 `f84b56e`）：① **Surface 拆分**（`serve.rs` 顶部 `Surface`/`RouteOwner` + `route_owner`/`required_surface`；gate 在 `/ws/live` 特判前，不匹配纯 404；静态面两端隔离）；② **CLI/Config**（`oasd studio` 8788 / `oasd runner` 8789 / `serve` deprecated；`config.runner{port:8789,bind:"127.0.0.1"}`，port≠0/≠serve.port、bind 仅 loopback；端口优先级 CLI > surface config > 默认）；③ **Runner 只读 API**（`/runner/health`、`/runner/tasks`；坏 meta 200 + `meta_error`；Studio 404）；④ **Runner UI 骨架**（5 nav、只 GET + 8s 超时、textContent、5 hash 深链、无 Studio 脚本/创作 API）+ **Studio 去调度入口**（nav/rail 删除、旧页兼容占位、`activatePage('schedule')` 提示并打开 Runner `#runs`）；⑤ **Runner-only 打包**（`scripts/package-runner.ps1` + `package-common.ps1`；`packaging/start-runner|studio|stop-runner|studio.bat`；`README-DEPLOY.md` 两种发行包；`dist/oasd-runner-v0.1.0-win64.zip` **37,141,295 B**，23 entries / 0 反斜杠）；⑥ **跨平台**（`packaging/CROSS-PLATFORM.md`；v1 只 Windows，Android 使用端预留）；测试 **oasd 240/0、workspace 538/0、UI 1239/224/0**；Lead 真实双进程 8788/8789 E2E 通过。证据 `_verify/runner-studio-pr1-20261104/`。
- 2026-11-04 19:2x~19:5x **本轮（seq 102）PR2 实现 + 评审修复 + 测试 + E2E**（代码未提交，父 `da9da9d`）：① **ParamDecl 五字段**（label/description/enum/required/group，全可选 + skip_serializing_if，老 meta 零破坏）+ lint + `validate_param_values`；② **`config/values.json`**（新 `apps/oasd/src/runner_config.rs`，`main.rs:30` 挂载；live config 同目录；缺=空/坏=带路径 Err；RMW + Mutex + tmp/rename 原子；保留其它任务）；③ **Studio API** `GET/POST /task/config`（只替换 params、保留其它字段、`.bak`、400 带 `params[i].field`）+ **Runner API** `GET/PUT /runner/tasks/<id>/config`（400 errors 含 key、不写盘）；④ **UI**：Studio「配置项…」JSON 编辑器 + Runner「配置」动态表单（number/checkbox/select/text）；⑤ 评审无 P0；P1-1 模块迁移验证已解决；P2 修 `meta` 未用变量/`.bak`/`.gitignore`；测试 **workspace 562/0、oasd 255/0、task 72/0、UI 1338/231/0**（+24/+99）；Runner 包重建 **37,192,710 B**（SHA256 `A10DEDC1…B4533F5D`）；Lead E2E 临时 `pr2demo`（Studio 8790 / Runner 8791）。
- 2026-11-04 19:5x **本轮 docs 收尾 8 份**（seq 102）：`RUNNER-STUDIO-SPLIT-PLAN`（状态 + **§A.10 PR2 记录** + §C.1~C.3 实现标注 + §D.3/D.4 + §E）；`PENDING §E-79`（状态/§E 状态/新增 **§G PR2 记录**/台账/footer）；`ENGINE §22.44` 指针 + 新增 **§22.45**；`RESUME` seq 102；`RESUME-INDEX` 102；`START-HERE`；`README`；本文件；逐处 before→after + 残留检查见 `_verify/runner-studio-pr2-20261104/DOCS-REPORT.md`。**代码/UI/docs 均未提交**。

## 2. 下一步

```text
1. 本轮（seq 102，in_progress）：PR2 代码/UI + 本批 docs 8 份已收尾，待 Lead 提交
   （父 `da9da9d`；docs = 新 HEAD）。提交前复核：cargo test --workspace **562/0**
   （oasd **255** / task **72** / runtime 142）、UI **1338/231/0**（runner audit 41/0/5）、
   node --check 3/3、git diff --check clean；**不含** assets/、frames/、tasks/、scheduler.json
   等用户数据；**不要提交 library/**（被 .gitignore 忽略）；dist/ 与 packages/ 不入 git。
2. ★ 下一步 = **PR3 优先级与类型保真**（`RUNNER-STUDIO-SPLIT-PLAN §C.3/§D.3`）：
   `RunOptions.params` 改 `BTreeMap<String, Value>`；`run.rs` 读 `config/values.json` 作第 3 层
   （CLI > entry > values > meta.default）；`entry_run_params` 不再字符串猜类型
   （`{"code":"007"}` 保持字符串）；合并后按 meta.params 防御性复检（错误带任务名+key）。
3. 随后 **PR4 子任务配置**（`call_task.params`/`inherit` + `with_params` + lint T7）
   → **PR5 打包/安装**（`.oaspkg` publish/install/uninstall/素材合并/installed.json）
   → **PR6 内嵌调度/设备租约**（可延后）。每 PR 用 §D.4 对应验收项收口
   （PR1 已收 V1、PR2 已收 V4 的 11~12）。
4. PR2 已知限制（不阻塞，见 `RUNNER-STUDIO-SPLIT-PLAN §A.10.8`）：坏 meta 下
   GET /task/config 400；POST 回写 key 字典序 + min/max 整数序列化 `1.0`；
   values.json 无 .bak/rev、多进程不安全；meta_error 行配置按钮/迟到响应未守卫；
   group 未渲染；run.rs 不读 values（PR3）。**PR1 校正**：旧 `#page-schedule` DOM/对话框
   仍未删（PR2 未覆盖，顺延 PR6）；`.gitignore` 的 values/tmp/bak 已由 PR2 加入；
   `packaging/README-RUNNER.txt` 的「配置随后续版本」措辞待下一轮打包订正
   （本轮 docs 子进程不改 packaging/）。seq 100 的 P2/P3 仍保留（模板 fixed 操作区、
   ROI 缩放回退矩形、OCR 低分为空结果、UI 阈值文案）。
5. ✅ 已完成：PR2（562/0、UI 1338/231/0、E2E V4）；seq 101 PR1（已提交 `da9da9d`）；
   seq 100 三项遗留（527/0、UI 1138/227/0）；seq 99 素材库 UX 六项 + OCR 阈值；
   调度器 P0–P5（P5 = 实机 A–F + `/instance/devices` adb 自动连接）。不 tag/release。
6. 产品 backlog（未被设计稿覆盖的部分）：`SelectionTool` 抽取；collection `real=1` 真机 E2E；
   drag Plan B；P2-10；运行记录收口（点击聚合/分页/run_id 唯一/detail 契约/`--status`）;
   口径统一（事件 attempt/entry、stats vs report）；版本决策（是否升 0.2.0，用户定）。
7. 对外口径：墙钟 24h soak 未跑完（只有 96 轮加速 soak 成立），宣称前需补跑。
```

## 3. 关键事实（防止再记错）

- **双入口（2026-11-04 seq 101 现行）**：`oasd studio` = **8788**（编辑端，`ui/index.html`），`oasd runner` = **8789**（使用端，`ui/runner.html`）；`oasd serve` 保留为 **deprecated 别名**（等价 studio，stderr 提示）。端口优先级 CLI `--port` > 各自 config（`serve.port`/`runner.port`）> 默认 8788/8789；`config.runner{port:8789,bind:"127.0.0.1"}`，bind 仅 loopback。**不要再写「一个 8788 干所有事」或「同一地址」**。
- **Surface 归属（PR1）**：`route_owner`/`required_surface` 在 `handle()` 里 gate（`/ws/live` 特判之前），不匹配 = **纯 404**；Studio 拿不到 `/runner/health|tasks`、`/runs/scheduler*`、`/scheduler/*` 写；Runner 拿不到 `/app.js`、`/frames`、`/task/library`、`/task/save_flow` 等创作路由。Runner 只读 API = `/runner/health`（含实际 port/mode/lock）与 `/runner/tasks`（坏 meta 200 + `meta_error`）；**配置 API/表单已由 PR2 落地**（见下条）；**安装=PR5、调度编辑=PR6 仍未实现**。
- **配置（2026-11-04 seq 102 PR2 现行）**：schema 由 **Studio** 写 `tasks/<id>/meta.json`（`GET/POST /task/config`，POST 只替换 `params`、保留其它/未知字段、写前 `meta.json.bak`）；运行值由 **Runner** 写 `config/values.json`（`GET/PUT /runner/tasks/<id>/config`；`PUT` 先 `validate_param_values`，400 `errors` 含 key 且不写盘；RMW + Mutex + tmp/rename 原子、保留其它任务）。`values.json` 位于 live config 同目录（无 `--config` 回落 `CWD/config/values.json`），结构 `{schema_version:1,updated_ms,tasks:{<task>:{values,updated_ms}}}`；坏文件带路径报错、无 `.bak`/rev。**PR2 只落存储，`run.rs` 仍不读 values（注入在 PR3）**；两端互拿对方 config 路由 404。
- **分发（seq 101）**：终端用户只下载 **Runner-only 包**（`oasd-runner-v<ver>-win64.zip`，8789）+ `.oaspkg` 任务包；**Studio 不发给终端用户**（Studio 包 `oasd-v<ver>-win64.zip`，8788，给开发/编辑/发布者）。Runner 包不含编辑 UI/frames/library；`dist/` 与 `packages/` 不入 git。
- **跨平台（seq 101）**：v1 只 Windows x64；Runner 核心/HTTP API/任务包/配置 schema 保持平台无关（`packaging/CROSS-PLATFORM.md`）；`--port 0` 由 `local_addr()` 回填，供未来 Android/嵌入壳取随机端口；**v1 不实现 Android/iOS/LAN**。
- **测试帧现行口径（2026-11-04，§E-76）**：**单素材测试 = 当前选中帧**（帧详情行 = 该行帧）；**批量帧测试 = 所有位置帧**（`POST /task/assets/batch_test`）；**没有「单素材多帧」入口**；**live 恒优先、真实点击强制 live**。旧「实时关=位置帧 / 单素材多位置逐个测」已被推翻。
- **单行 OCR 没有真实检测框**：`/ocr` 协议无 bbox；单行 OCR 结果 `recognizedBox:null` + `roiBox`，文案「识别区域（存储 ROI，无独立检测框）」；多行/derived 才有 det 框。`frame_source` 线上仍回 `position:#<id>`，UI 标签写「当前帧 #N」（P2 文案待统一）。
- **素材行（2026-11-04 seq 98 + seq 99 + seq 100 现行）**：三处素材行**只有「识别 + 设置」两个按钮**；「设置」是行内展开的 **9 项**（点测/拖测/识别区/操作区/重命名/阈值/位置帧/删除/步骤），不是弹层。选中态按 **asset id** 跨 surface 高亮（`.asset-row.selected`，背景 `rgb(27,42,58)`）；菜单项点击**不收起**，仅切另一个素材关旧菜单，重渲染恢复选中/菜单。**识别区 = 画布拖拽**（旧 prompt 已删），保存按钮在共享 `#canvas-footer` +「识别区编辑中：<id>」；**模板识别区启用**，保存走 `/task/save_asset` + `replace:true`（当前帧重截、rev+1）；OCR/region 走 `/task/asset/update`。**seq 100 起**：region 保存带 `keep_action_shape:true`（**操作多边形不再被降级为矩形**，结果行文案「操作区 action_shape 保持不变」）；模板 ROI **移动**时 polygon 按 `dx/dy` **平移**（`assetRoiPolygonPlan`；仅缩放/旧 bbox 不一致/越界才回退矩形并提示）。**模板 `fixed` 操作区仍不随 ROI 平移**（未改）。帧详情 `manage:true`、去来源 badge，9 项全可用，按行来源分 scope。
- **OCR 阈值（2026-11-04 seq 99 + seq 100 现行）**：OCR 素材（`ocr_line`/`ocr_lines`/derived）`threshold` = **最低识别置信度**；**未设或 0 = 不过滤**（与旧行为一致），`t>0` 时 `score<t` = 未命中。生效 click / 条件 appear / `expect appear/vanish` / `click_target` ocr_text / expect ocr_text / drag ocr_text，**以及取值路径 `ocr()`/`ocr_digit()`/`ocr_to_var`（seq 100 起）**——低分 → `Null`/空串、**低分文本不进值**、**空结果而非硬错误**（保 `wait_until` 轮询）；`ocr_lines` **判定用最高行分、取值用最低行分**，适配层丢 `<0.5`；模板/collection 默认 0.9 不变。详见 `ENGINE §16.5` / `ENGINE §22.43` / `EDITOR-UX-PLAN §18.4/§18.10` / `§E-78`。
- **push-all（2026-11-04 seq 98）**：`scripts/push-all.ps1` / `.bat`；`git add -A` 覆盖 assets/frames/tasks/scheduler.json，但**只剔除未跟踪瞬态**（已跟踪 `*.log` 等保留）；**`library/` 被 .gitignore 忽略、不会提交**；默认**仅 dev 自动同步 main**，其它分支 WARN + exit 0（`-SyncMain` 才同步）；远程无 main WARN + exit 0；push 失败 exit 5 但保留本地 commit，重跑无 staged 时走「只推送」补推。用法见 `docs/PUSH-ALL.md`。
- 当前素材是 **16/1**（已提交基线），不是 34/18；**工作区未提交用户数据（12:5x）为 27 素材（5 模板 + 22 `ocr_line`）/ 5 图（a001~a005，其中 a002~a005 未跟踪）**。34 是 `§E-70` 已推翻的错误恢复，archive 2827/2099 未导入（用户裁定素材可重建）。
- 当前帧：已提交基线 **28 / current_id=20 / next_id=41**；**工作区未提交数据（12:5x）为 30 帧（含 #41/#42）/ current_id=1 / next_id=43**，不是 40。
- `tasks/测试` 已删除并提交；task lint 是 **1/0**，不是 2/0。
- `action_roi`/`roi_back` 的 **JSON/UI 读取已删除**；runtime 内部仍保留 `click_back` 作为 region/collection 的 bbox 快捷路径——不要写成“全仓无 roi”。
- **drag/swipe 不写点击分布**：`record_click()` 只在 `perform_click()` 路径调用。
- 拖测 `margin`：默认 **0.15**，合法 **0~0.45**，作用于起点（配置了 `to` 时也作用于终点）候选矩形内缩；**仅 rect 非反选生效**；有 polygon/invert `shape` 时忽略并记 `margin=ignored`。
- **collection 真实点击已终态收口**（dev `d1a7fcd`）：`real` 严格 `0/1/true/false`；`real=1` 强制 live 帧；匹配通过后在**精确 action shape 内采样**（polygon 内/invert 排除区外）；未过阈值 **409**（不点击）。**默认仍是 dry-run**，响应/日志不变；与“素材行真实点击”共用 `perform_live_tap`；**无真机 E2E**。
- **v0.1.0 发布包不含 O-1/O-2**；混版本 runtime 会忽略 drag `shape`（invert 会点到排除区）⇒ invert 拖动需本版 runtime；**P2-10**（保存 flow 不校验解析错误）本轮不做。
- 当前测试基线 **562/255/1-0**（2026-11-04 seq 102 复核；oasd **255**、task **72**、runtime 142；UI **1338/231/0**，runner audit 41 ids / 0 dead / 5 nav；seq 101 为 538/240 + UI 1239/224/0；seq 100 为 527/229 + UI 1138/227/0；seq 99 为 516/0 + UI 1112/227/0；seq 98 为 907/225/0；seq 97 为 835/225/0）；上一发布基线 **426/259/1-0**。
- **P5 实机环境**：MuMu serial **`127.0.0.1:16384`**（Android 12 / V2284A / 720x1280；Onmyoji 前台庭院），不是 `emulator-5554`；`adb devices` = device。A–F 全部真机通过（quota 2/2、temp 每轮、队列 2 轮、强杀续跑、reset-memory、9 条 run 记录）；证据 `D:\OAS-dev\_verify\p5-realdevice-20261103\`。
- **P5 adb 自动连接**：`GET /instance/devices` 列表为空时自动 `adb connect`（配置 serial → live config `device.reconnect_endpoints` → 默认 `127.0.0.1:16384`）后重列，响应新增 `auto_connect`；`emulator-5554` 这类非 TCP serial 跳过。实测 `adb disconnect` 后检测自动连回 16384。P5 代码未重建 serve 前该行为不生效。
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

- **入口/端口（seq 101）**：用户入口 = Studio **8788** / Runner **8789**；8787 只是**未改的** `scripts\dev-serve.bat` 旧开发手动入口，不是用户版地址。`oasd serve` 仍可用但已 deprecated（等价 studio）。
- **Runner 配置 / 只读（seq 102）**：PR2 起 `/runner/tasks/<id>/config` 的 **GET/PUT 已有 handler**（Studio 拿 → 404）；`/runner/install`、`/runner/uninstall`、`/runner/run`、`/runner/scheduler/*` 仍只登记了归属、没有 handler，请求 404 属预期，不是回归。安装=PR5、调度编辑/临时运行=PR6。**配置写入必须先过 `validate_param_values`**（type/min/max/enum/required；未知 key 拒绝并给允许列表）；`values.json` 无 `.bak`、多进程同时写不安全。
- **`oasd runner --help` 的坑**：`--help` 会被 serve parser 当未知参数；`runner` 子命令本身可用（启动用 `oasd runner --port 8789 ...`）。正式文档不写 help 用法。
- **分发（seq 101）**：终端用户只拿 **Runner-only 包 + `.oaspkg`**；**不要把 Studio 包/编辑端发给终端用户**。Runner 包不含 `index/app/builder/flow_model/asset_ui/dashboard`、`frames/`、`library/`，这是设计而非打包遗漏。
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
- **run 详情契约**：`GET /runs/scheduler/run` 只校验"合法 JSON"，非实体 JSON 也能读出（列表会跳过）；`limit=0` 被 clamp 成 1；list 500 与 detail 500 错误体形状不同（未做，转 backlog）。
- **P4 写失败**：run 文件写失败只影响"可查性"（alert + `report.run_file_errors` + 事件 `run_file_error`），不影响执行/重试/退出码；排查先看 `alert(kind=run_file_write)` 与 `scheduler_run_file_error` 日志。
- **实机检测不到 adb**：MuMu 重启后 adb TCP 设备常掉（`adb devices` 为空）；P5 起 `GET /instance/devices` 会**自动 connect `127.0.0.1:16384`**（配置 serial → `device.reconnect_endpoints` → 默认；响应 `auto_connect` 可核对），仍失败再查 adb server / MuMu 网络设置，不要先怀疑 UI 或代码。
- **帧页测试帧 / 素材行（2026-11-04）**：不要再写/实现「实时关=位置帧」；单素材测试走 `testFramePlan` 的当前帧分支，批量才按 `positions`。单行 OCR 的存储 ROI 不是真实检测框，不要再画成检测框；**§E-78 起**三处素材行统一「识别 + 设置」两按钮（9 项行内菜单），**帧详情也 `manage:true` 全可用**——不要再写「帧详情只放测试类动作」或「帧详情 6 项 disabled」；**批量帧测试按钮在素材页**，对象 = 当前可见素材集合（不是任务 flow 子集）。
- **帧页布局（2026-11-04）**：`#panel-images` 是 `height: calc(100vh - 150px)` 固定高 flex 列；往帧列表上方加固定内容时必须同时保 `#panel-images .frames-list` 的 `min-height`（现 **200px**）或把内容放进默认折叠的 `#frame-details`——否则 1440×900 下列表会被挤成 **0 高**，表现为「分类帧区域消失」。点帧会展开详情；空搜索隐藏状态行。
