# 会话交接单（SESSION HANDOFF）★ 一页纸

> ★★★ 新会话先读 `docs/START-HERE.md` → `docs/RESUME.md` → `docs/RESUME-INDEX.md` 最新行（seq 91）。
> 当前是 **调度器产品化阶段（单活动实例）**：P0 ✅ 文档（`docs/SCHEDULER-RUNNER-PLAN.md` + ENGINE §22.34.7 + PENDING §E-75）/ **P1 ✅ 已提交（全量回归 450/0，oasd 177；调度专项 77）** / P2–P5 ⬜。更早节点：0.1.0 Release 已上传、tag `v0.1.0` 未移动、O-1/O-2 收口（`d1a7fcd`），`dev`/`main` 已同步 origin。不要再灌 34/18 旧素材。

## 0. 当前状态（2026-11-02 23:3x）

| 项 | 值 |
|---|---|
| 当前阶段 | 调度器产品化 + 队列 Round 语义：**P0 ✅ / P1 ✅ 已提交（450/0，oasd 177）/ P2–P5 ⬜**；计划 `docs/SCHEDULER-RUNNER-PLAN.md`，台账 `PENDING-LEDGER §E-75` |
| 工作区（本轮） | 已提交（本轮两次提交：P1 代码 + docs；hash 以 `git log` 为准）；`frames/frames.json` 是**用户数据改动，不属于本轮**，勿动 |
| 分支/HEAD | `dev` = 本轮 docs 提交（HEAD，父 = 本轮 P1 代码提交）+ 本轮 P1 代码提交（父 `131d6a7`）；推送状态以 `git status -sb` 实测为准；包构建 commit `42567c1` |
| 恢复节点 | tag `v0.1.0`（annotated `cf123b8` → commit `e8edbef`，**未移动**）；**GitHub Release 已上传** |
| 发布 | `https://github.com/passioki/oas-core/releases/tag/v0.1.0`（id `431080249`，draft=false / prerelease=false，即 latest）；附件 zip（asset `700544587`，37,089,992 B，digest `sha256:e6112a33…5f5687`）+ `.sha256`（asset `700545142`，89 B）；本地同物 SHA256 `E6112A33…5F5687` |
| 版本 | `0.1.0`；**包不含 O-1/O-2**（包构建 `42567c1`，O-1/O-2 自 `54c715b` 起，评审修复 `d1a7fcd`） |
| 测试 | `cargo test --workspace` = **450 passed / 0 failed**（21 个 test binary 全 ok；oasd **177**，P1 实现 172 + 评审修复 5；调度专项 **77** = `scheduler.rs` 45 + `schedule.rs` 32）；UI pure test 259 checks / 0 dead ids；task lint 1/0；`cargo build -p oasd --features ocr` exit 0（alt target）。上一发布基线 426/0（oasd 153 / flow 57 / runtime 125 + 2 integration，13:35 实测） |
| 数据 | 全局素材 **16 条 / 1 图**；帧 **28 个 / current_id=20 / next_id=41** |
| 任务 | 只有 `返回庭院`（flow 11 steps / meta 400 B）；`tasks/测试` 已删除（随 `cd0ac34` 提交） |
| 上一轮工作区 | `d1a7fcd` 时 clean；seq 89 docs 订正（8 份，终态，`906bbed`）与同步回填（`11d53a5`）均已提交 |
| 服务 | serve **8787**：最终构建已重建，Lead 以 `-SkipBuild` 重启（使用本版二进制） |
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
- 23:3x **本轮收尾（F3）**：全量回归 `cargo test --workspace` = **450 passed / 0 failed**（21 binary；oasd **177**；调度专项 **77**）；UI 259/0；lint 1/0；OCR 构建 exit 0（alt target）；E2E 复核通过（queue skipped 语义、`max_passes` 别名、`--max-passes` stop reason、v3 显式 kind 拒绝、state v3→v4 备份 + schema 4）；P1 代码与 docs 分两次提交（不含 `frames/frames.json`）。

## 2. 下一步

```text
1. ✅ 本轮已收口：全量回归 450/0（oasd 177、调度专项 77）→ 审 diff（schema 3→4 / state 迁移 / 纯函数 /
   旧单任务 entries 回归等价）→ 分两次提交（P1 代码 + docs；不 tag/release；不含 frames/frames.json）
   → RESUME seq 91 收尾（本文件与 RESUME/RESUME-INDEX/PENDING/SCHEDULER-PLAN 已同步）
2. P2 执行编排：schedule.rs 识别 kind="queue" → execute_queue_round；queue_round_started/queue_member_finished/
   queue_round_finished/queue_finished/empty_round 事件 + report 队列汇总（单成员执行抽小 helper，2 处调用）
3. P3 UI（entries CRUD / 队列面板 / 状态卡）→ P4 Run 一等实体 → P5 验收（实机需先启动 MuMu emulator-5554）
4. 旧开口（不阻塞）：SelectionTool 结构债；真机 collection real=1 E2E；P2-10；drag Plan B
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
- 最终测试基线 **426/259/1-0**（13:35）；oasd 153 / flow 57 / runtime 125（+2 integration）；DSL E2E `linted=8 errors=5`。
- 真实 UI 手势：用户 00:37 说已测试；助手独立证据只有真机 `/input/tap`，两者要区分。
- E2E 36/36 的证据边界是 `4d5759f`（23:0x），不含 23:39 点击分布/反选/真实开关与 00:02 RO 修复。

## 4. 活跃的坑

- 不要在未确认前覆盖 `assets/`、`frames/`、`tasks/`；`assets/_deleted-tasks/` 是唯一副本安全网，绝不删。
- 不要用 task 历史再拼“全局库”；不要再跑 `scripts/recover_historical_assets.py`。
- 中文文档/代码用 `read`/`edit`/`write` 工具；PowerShell 直接重写 UTF-8 会 mojibake。
- `_verify/n44/...` 是仓外路径（`D:\OAS-dev\_verify\n44\...`），不是 `oas-core/_verify`。
- `ENGINE §22.39 R11`：唯一 `assets/`，任务只引用全局 id；`ASSET-DATA-MODEL.md` 是数据模型入口。
- 调度 CLI：`--config` 是**全局 live 配置**（被 `main.rs` 在子命令分发前全局摘走并立即解析），**不是**调度配置；调度计划入口是 `oasd scheduler --plan <scheduler.json>`。**误把 `scheduler.json` 传给 `--config` ⇒ exit 2 + `config error: unknown field 'config'`，不会跑任何 plan**；只有 `--config` 传合法 live 配置时，调度 plan 才静默回退默认 `scheduler.json`。
- 长期口径：**墙钟 24h soak 未跑完**（`_verify/n42/live-soak` passes=22、`last_stop=""`、events 无 `stop`、lock 残留 pid=1268）；对外只宣称 **96 轮加速 soak** 成立。
