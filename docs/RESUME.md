# RESUME — 断点续接卡（session-memory 强制 6）

> 规则：本文件每次断点覆盖重写；≤150 行；另一个会话只读本文件 + `RESUME-INDEX.md` 最新行就能继续。
> 细节：`docs/RUNNER-STUDIO-SPLIT-PLAN.md`（**§A.9 PR1 + §A.10 PR2 + §A.11 PR3 + §A.12 PR4 + §A.13 PR5 + §A.14 PR6 + §A.15 PR7 实现记录**）· `PENDING-LEDGER.md §E-79`（§F PR1 / §G PR2 / §H PR3 / §I PR4 / §J PR5 / §K PR6 / **§L PR7**）· `ENGINE-DESIGN.md §22.44~§22.50`。

```yaml
task_id: oas-core-runner-studio-split
checkpoint_seq: 107
updated_at: 2026-11-05T02:2x+08:00
trigger: PR7（用户纠正：原开发端已有调度，直接移到使用端即可）→ 旧 Studio 完整调度 UI 迁移到 Runner + 最小任务链多参数 E2E 已提交 → docs 收尾（本子进程只改 docs）
status: in_progress
next_action: PR7 代码已提交（5e45c81）；本批 docs 待 Lead 提交；随后做最终用户验收（§D.4 19 步，V5/V6-16/17 已由 PR7 E2E 覆盖）+ PR7 的 8 条 P2 清理 + Phase 5/backlog 选择；不 tag / 不 release
```

## 0. 目标（本阶段）

1. **Q1~Q7 已批准**（2026-11-04，全部按推荐）——见计划文档 §D.2。
2. **PR1~PR6 ✅ 已提交**：Surface 拆分（`da9da9d`）→ 配置模型（`dfe8776`）→ 类型/优先级（`c09df25`+`048dff5`）→ 子任务配置（`442f5cb`）→ TaskPackage 发布/安装（`617435d`+`830058d`）→ 内嵌调度/设备租约/`/runner/run`/Studio 旧调度页删除（`edc037b`）。
3. **PR7 ✅ 已提交（`5e45c81`，2026-11-05）**：用户纠正「原先开发端本来就有调度，直接移到使用端即可；PR1~PR6 删掉后重设计没达到原先完成度」→ 把 **PR1 前（`f84b56e`）的完整 Studio 调度 UI** 迁移到 Runner「队列与调度」，PR6 后端能力保留；最小任务链多参数 E2E 通过。**Studio 调度页保持删除（不回退）。**
4. 本子进程只做 **docs 收尾 8 份**（不改代码/脚本/用户数据，不 commit、不跑 cargo/serve）。

## 1. 状态（2026-11-05 02:2x，seq 107）

| 项 | 值 |
|---|---|
| 分支 / HEAD | `dev`；HEAD = **`5e45c81`**（`feat(runner-ui): migrate the full legacy Studio scheduler UI to the Runner (PR7)`，父 `eca884d`）；PR1~PR6 + PR7 全部已提交；本批 docs 未提交；**未 tag / 未 release** |
| 任务状态 | **in_progress**（seq 107）；PR1~PR7 ✅ 已提交、**最终用户验收 / P2 清理 / Phase 5 待做** |
| 双入口 | **Studio `http://127.0.0.1:8788/`**（`oasd studio`；`oasd serve` = deprecated 别名）· **Runner `http://127.0.0.1:8789/`**（`oasd runner`，nav **6 项**；启动即内嵌调度，`--no-scheduler` 关闭） |
| 测试 | `cargo test --workspace` **713/0**（Rust 零改动；oasd **347**、oas-task **111**、oas-flow **62**、oas-runtime **157 lib + 2 integration**）；UI pure test **1559 checks / 151 ids / 0 dead**（1167 → +392；Studio **159/0** 不变；`runner_ui_audit` 6 nav）；`node --check` 3/3；`git diff --check` clean；OCR/release build exit 0（**5 warnings**） |
| 分包 | `dist/oasd-runner-v0.1.0-win64.zip` **37,684,830 B**（用迁移后 UI 重建）+ SHA256 **`3D5D9E53E7EB0A536D690B3DFF71291074D0F05445FEC0C5E6AFF8E3D2D26653`**；**不入 git** |
| 上一节点 | seq 106 = PR6（713/0、oasd 347、UI 1167/159/0、runner 包 37,653,462 B / SHA `FECC9D24…0910CE00`）→ `edc037b` + docs `eca884d` |
| 文档（本轮） | `RUNNER-STUDIO-SPLIT-PLAN.md`（状态 + **§A.15** + §A.6/§A.8.1/§A.9.9/§A.14.7/§A.14.9/§D.3/§D.4/§E）、`PENDING-LEDGER §E-79`（状态/§D/§E + **§L.1~L.7** + 台账/footer）、`ENGINE-DESIGN`（§22.44/§22.49 指针 + **§22.50**）、`RESUME`（本卡）、`RESUME-INDEX` 107、`START-HERE`、`SESSION-HANDOFF`、`README`（**8 份**） |

## 2. 验收清单（PR7）

- ☑ **旧 UI 整体迁移**：从 `f84b56e` 恢复 **6 DOM 块 / 84 旧 id**（工作台面板 6、完整调度页 29、重置记忆 9、条目 dialog 19、队列 dialog 15、运行详情 6）+ **129 旧函数**（77 纯函数：75 逐字 + 2 文案适配；43 DOM/保存；8 面板/重置；编排重写）；旧事件绑定 + CSS 同步；84/84 id、129/129 函数复核通过
- ☑ **适配**：`/task/library`→`/runner/tasks`；`activatePage('schedule')`→`showPage('queue')`；`log()/resultEl`→`setPageStatus/schedLog`；`.badge`→`runner-badge`；`.sched-runs-table`→`runner-table`；唯一 `runnerFetchJson`（1 处 fetch）；锁判定叠加 health lock/`runnerScheduler.running`；**GET-first 配置快照守卫**；5s 轮询只叠运行态 badge；PR6 启停/临时运行/健康/租约保留
- ☑ **功能对等（Runner）**：entries/queues 全字段、增删改/启用/排序、GET/POST `/scheduler/config` 逐行 400/409、运行记录筛选+详情+快捷筛选、重置记忆 dialog（全选/反选/全不选/`include_checkpoint`）、任务行「调度设置…/运行记录」、高级 JSON 保留
- ☑ **Studio 保持清理**：`app.js`/`index.html` 未回退，仍无调度页；无 Studio-only 路由/全局回流；Runner 6 页不变
- ☑ **打包说明**：`README-RUNNER.txt`/`README-DEPLOY.md` 改为「队列与调度页完整图形化编辑条目/队列/窗口/配额/参数、运行记录、重置记忆；高级 JSON 保留」
- ☑ **评审**：无 P0、无代码级 P1；84/84 id、129/129 函数、77 纯函数、旧 P3/P4 断言 288/288、1559 checks/151 ids/159/6 nav 复核通过
- ☑ **测试/E2E**：Rust 713/0、UI 1559/151/0（+392）、Runner 包 37,684,830 B；真实浏览器 CDP + Runner 8796 + MuMu 在线 E2E（69/69 旧 id、add-entry dialog 保存、两 entry 各跑一次、停止/重置/任务联动）
- ☑ **最小任务链多参数**：父 `p7-parent`（p1/p2）→ 子 `p7-child`（c1/c2/code）；子单独 `Str("runner-child")/Num(9)/Str("007")`；父单独 `runner-parent/7 → 子 runner-parent/7/007`；scheduler `entry-parent/5 → 子 entry-parent/5/007`；UI entry `ui-entry/4 → 子 ui-entry/4/007`（映射/优先级/子默认/类型保真全通过）
- ☐ **本批 docs 提交**（父 `5e45c81`；不含用户数据；`dist/`/`packages/` 不入 git）
- ☐ **最终用户验收 + P2 清理**：§D.4 19 步（V5/V6-16/17 已覆盖）；PR7 的 8 条 P2（状态卡旧值、config GET 误报、重复错误行、reset 兜底/409、dialog busy、锁过期、死代码/CSS）

## 3. 下一步（精确动作）

1. **Lead 提交本批 docs**：只含 `docs/*.md` 8 份；提交前 `git status --porcelain` 应只有 docs + 用户数据（`assets/`/`frames/`/`tasks/`/`scheduler.json`）；**用户数据不提交**。
2. **真实设备/真实任务验收**：跑 §D.4 的 19 步；重点 V5（13~15 已用迁移后 UI 重验）、V6-16/17（多参数/values 覆盖）、V7（无用户数据提交）；真实 MuMu `127.0.0.1:16384` + 真实任务跑一遍，不只用 replay/临时任务。
3. **PR7 P2 清理（不阻塞）**：① `refreshScheduler` 失败清空状态卡；② config GET 失败与「文件不存在」区分；③ 多错误去重；④ reset 恢复 `status.entries` 兜底；⑤ reset 409 保留 `errors[]` + 结果不立即关闭；⑥ dialog 保存前置 busy；⑦ `scheduleLocked` 不用过期 config meta；⑧ 清死代码/死 CSS。
4. **Phase 5 选择（不阻塞）**：上传模式、素材同 id 自动改名 + flow 重写、显式纳管 API、素材 GC、`installed.json` 损坏隔离、跨进程 packages 锁、`dependencies` 与 flow 闭包交叉校验。
5. **产品 backlog（不阻塞）**：SelectionTool、collection `real=1` 真机 E2E、drag Plan B、P2-10、运行记录/口径收口、版本决策（是否升 0.2.0）。
6. 每轮收口：`cargo test --workspace` 全绿（**基线 713**）+ UI pure test（**1559/151/0**）+ `node --check` + `git diff --check`。**不 tag / 不 release**（除非用户明确说发布）。

## 4. 本轮裁定 / 约束（带锚点）

- **调度 UI 正确做法 = 整体迁移，不是重设计**：旧 Studio 调度页（`f84b56e`）是完成度基准；PR7 保留原 id/函数/交互，只换 5 类 surface seam；以后新增调度能力也以「先对齐旧 UI」为前提。
- **PR6 后端能力不丢**：启停（3 s stop 预算）/健康/临时运行/设备租约（pass 级）/锁互斥全部保留；PR7 只改 UI。
- **Studio 仍无调度页**：`app.js`/`index.html` 未回退；`?page=schedule` 仍跳 Runner `#queue`（`window.open` 可能被拦）。
- **数据边界**：用户数据（`assets/`/`frames/`/`tasks/`/`scheduler.json(.bak)`）不提交；`dist/`、`packages/`、`config/device.lease`、`scheduler.state.lock` 不入 git；本子进程只改 docs。
- 若用户改 Q1~Q7 任一项，先回写计划文档 §D.2 与稿源再动代码。

## 5. 阻塞 / 待用户裁定

- **无技术阻塞**。唯一等待项 = 提交本批 docs（父 `5e45c81`）。
- 待用户裁定：最终用户验收范围（是否按 §D.4 全跑）；Phase 5 范围与产品 backlog 优先级；是否升版本 0.2.0；是否发布。

## 6. 证据

- **PR7**：`D:\OAS-dev\_verify\runner-studio-pr7-20261105\`（`PR7-UI-REPORT.md` / `RECON-REPORT.md` / `REVIEW.md` 276 行 / `e2e/` / `ui-final.log` / `runner-queue-full.png` / `runner-entry-dialog.png` / `runner-runs-full.png` / `runner-task-link.png` + 本轮 `DOCS-REPORT.md`）。
- **PR6**：`_verify\runner-studio-pr6-20261104\`；**PR5**：`_verify\runner-studio-pr5-20261104\`；**PR4**：`_verify\runner-studio-pr4-20261104\`；**PR3**：`_verify\runner-studio-pr3-20261104\`；**PR2**：`_verify\runner-studio-pr2-20261104\`；**PR1**：`_verify\runner-studio-pr1-20261104\`。
- **设计稿源**：`_verify\arch-runner-studio-20261104\RUNNER-STUDIO-CONFIG-DESIGN.md`（659 行 / 235+ `file:line`）。
- **台账**：`docs/PENDING-LEDGER.md §E-79`（§F~§K + **§L PR7**）；现行口径：`docs/RUNNER-STUDIO-SPLIT-PLAN.md`（§A.9~§A.15）、`docs/ENGINE-DESIGN.md §22.44~§22.50`、`packaging/CROSS-PLATFORM.md`、`packaging/README-DEPLOY.md`、`packaging/README-RUNNER.txt`。

## 7. 连续性最小验证命令（恢复后先跑）

```text
cd D:\OAS-dev\oas-core
git log -1 --format="%h %ci %s"          # 期望 5e45c81（PR7；本批 docs 提交后为新 HEAD）
git status --porcelain                   # 期望：docs 8 份（若未提交）+ 用户数据（勿提交 assets/frames/tasks/scheduler.json）
node apps\oasd\ui\ui_pure_test.js        # 期望 1559 checks / 151 runner ids / 159 studio ids / 0 dead / 6 nav
cargo test --workspace                   # 期望 713 passed / 0 failed（oasd 347 / task 111 / flow 62 / runtime 157+2）
```
