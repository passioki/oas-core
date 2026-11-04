# RESUME — 断点续接卡（session-memory 强制 6）

> 规则：本文件每次断点覆盖重写；≤150 行；另一个会话只读本文件 + `RESUME-INDEX.md` 最新行就能继续。
> 细节：`docs/RUNNER-STUDIO-SPLIT-PLAN.md`（**§A.9 PR1 + §A.10 PR2 + §A.11 PR3 + §A.12 PR4 + §A.13 PR5 + §A.14 PR6 实现记录**）· `PENDING-LEDGER.md §E-79`（§F PR1 / §G PR2 / §H PR3 / §I PR4 / §J PR5 / **§K PR6**）· `ENGINE-DESIGN.md §22.44~§22.49`。

```yaml
task_id: oas-core-runner-studio-split
checkpoint_seq: 106
updated_at: 2026-11-05T00:5x+08:00
trigger: PR6（可停止调度 + Runner 内嵌调度 + 设备租约 + /runner/run + Studio 旧调度页清理 + 评审修复）已提交 → docs 收尾（本子进程只改 docs）
status: in_progress
next_action: PR6 已提交（代码 edc037b）；本批 docs 待 Lead 提交；随后做 PR6 后收尾（§D.4 19 步最终验收 + Phase 5/backlog 选择 + 用户验收），不 tag / 不 release
```

## 0. 目标（本阶段）

1. **Q1~Q7 已批准**（2026-11-04，全部按推荐）——见计划文档 §D.2。
2. **PR1~PR5 ✅ 已提交**：Surface 拆分（`da9da9d`）→ 配置模型（`dfe8776`）→ 类型/优先级（`c09df25`+`048dff5`）→ 子任务配置（`442f5cb`）→ TaskPackage 发布/安装（`617435d`+`830058d`）。
3. **PR6 ✅ 已实现并提交（代码 `edc037b`，父 `c0fc7a6`，2026-11-05）**：可停止调度 + Runner 内嵌调度线程/启停健康 API + 设备租约（pass 级）+ `POST /runner/run` 临时运行 + Runner 队列页控制/临时运行/高级 JSON + **Studio 旧调度页彻底删除** + 打包脚本/说明；评审 7 个 P1 全部已修/处理。
4. 本子进程只做 **docs 收尾 8 份**（不改代码/脚本/用户数据，不 commit、不跑 cargo/serve）。

## 1. 状态（2026-11-05 00:5x，seq 106）

| 项 | 值 |
|---|---|
| 分支 / HEAD | `dev`；HEAD = **`edc037b`**（`feat(runner): embedded scheduler + /runner/run + device lease + Studio schedule-page removal (PR6)`）；**6 个 PR 全部已提交**；本批 docs 未提交，父 `edc037b`；**未 tag / 未 release** |
| 任务状态 | **in_progress**（seq 106）；PR1~PR6 ✅ 已提交、**PR6 后收尾/验收待做**、Phase 5 未做 |
| 双入口 | **Studio `http://127.0.0.1:8788/`**（`oasd studio`；`oasd serve` = deprecated 别名）· **Runner `http://127.0.0.1:8789/`**（`oasd runner`，nav **6 项**；启动即内嵌调度，`--no-scheduler` 关闭） |
| 测试 | `cargo test --workspace` **713/0**（701 → +12 修复测试）；oasd **347**、oas-task **111**、oas-flow **62**、oas-runtime **157 lib + 2 integration**；OCR build + release build exit 0（**5 warnings**，PR6 新增 warning 清零）；UI pure test **1167 checks / 159 ids / 0 dead**（`runner_ui_audit` **75 ids / 0 dead / 6 nav**）；`node --check` 3/3；`git diff --check` clean |
| 分包 | `dist/oasd-runner-v0.1.0-win64.zip` **37,653,462 B**（PR6 release 重建；Runner-only）+ SHA256 **FECC9D241FAD13D2516CB155675824975D4579FB1EC22613AFA6BE7A0910CE00** + `.sha256`；**不入 git** |
| 上一节点 | seq 105 = PR5（677/0、oasd 311、UI 1442/237/0、runner 包 37,591,694 B / SHA `823F93CD…14031E`）→ 已提交 `617435d` + docs `830058d` |
| 文档（本轮） | `RUNNER-STUDIO-SPLIT-PLAN.md`（状态 + **§A.14** + §A.3/§A.6/§A.9.9/§A.11/§A.13/§D.3/§D.4/§E）、`PENDING-LEDGER §E-79`（状态/§D/§E + **§K.1~K.8** + 台账/footer）、`ENGINE-DESIGN`（§22.44/§22.45 指针 + **§22.49**）、`RESUME`（本卡）、`RESUME-INDEX` 106、`START-HERE`、`SESSION-HANDOFF`、`README`（**8 份**） |

## 2. 验收清单（PR6）

- ☑ **可停止调度**：`SchedulerOptions.stop`（默认 None）；pass 前/后检查；轮间 sleep ≤200 ms 分片（无 token 旧单次 sleep 不变）；`stopped_reason="api_stop"` + `state.last_stop`/audit/report/日志/锁释放正常；attempt 不打断；stop 优先级低于 fatal/max_rounds/loop_rounds/once/stop-mode/no_enabled_entries
- ☑ **内嵌调度**：`oasd runner` HTTP 启动后自动起线程（`--no-scheduler` 关闭）；`POST /runner/scheduler/start|stop`、`GET /runner/scheduler/status`、`/runner/health.scheduler`；stdout 环形缓冲 500 行 + `CWD/logs/runner.log`、`CWD/logs/scheduler.events.jsonl`；锁冲突 **409 `scheduler_locked`**；线程 Err/panic 被捕获、HTTP 不崩；stop ≤**3 s**（完成 / `stopping+timeout`），重复 stop `already_stopped`
- ☑ **设备租约**（`device_lease.rs` 新）：`config/device.lease`（`config.device.lease` 可配）；活 pid（含同进程）冲突、stale/坏文件回收、Drop 只删自己；**调度每 pass acquire/Drop**（冲突跳过 pass + `scheduler_lease_busy` + 500 ms 重试）；Studio `/task/run`、Runner `/runner/run` 预 acquire 409 `device_busy`；`RunOptions.skip_lease`；`run::execute` 默认 acquire（CLI `oasd run` 受保护）；CLI scheduler/queue 已接
- ☑ **`POST /runner/run`**：`{task,params?}`；**Runner values 铺底、body params 覆盖（类型保真）**、不读其它任务 values；异步 + `GET /runner/run/status`；409 `run_busy`/`device_busy`、404 未知任务
- ☑ **UI**：Runner 队列页调度控制条/临时运行卡/高级 JSON 编辑；概览调度卡；Runner 6 页不变；**Studio 旧调度页彻底删除**（0 残留，`?page=schedule` 跳 Runner `#queue`、`?reset=1` 提示保留）
- ☑ **打包**：`README-RUNNER.txt`/`README-DEPLOY.md` 更新（内嵌调度/队列页控制/临时运行/高级 JSON/重置记忆 CLI）；`stop-runner.bat` 先 POST 停止再强杀
- ☑ **测试/E2E**：workspace **713/0**、oasd **347**、UI **1167/159/0**（audit **75/0/6**）、runner 包 **37,653,462 B**；Lead E2E（见 §6）全部通过
- ☐ **本批 docs 提交**（父 `edc037b`；不含用户数据；`dist/`/`packages/` 不入 git）
- ☐ **PR6 后收尾**：§D.4 19 步最终用户验收（V1~V7；V5/V6-17 已由 PR6 E2E 覆盖）、`packaging/` 旧 `start-scheduler.bat` 文案/docs 旧截图清理

## 3. 下一步（精确动作）

1. **Lead 提交本批 docs**：只含 `docs/*.md` 8 份；提交前 `git status --porcelain` 应只有 docs + 用户数据（`assets/`/`frames/`/`tasks/`/`scheduler.json`）；**用户数据不提交**。
2. **PR6 后收尾**：跑 §D.4 的 19 步清单做最终用户验收；重点核对 V5（13~15，13 用高级 JSON 加 entry）、V6-17（`/runner/run` values/body）、V7（无用户数据提交）。
3. **Phase 5 选择（不阻塞）**：上传模式（`upload`/incoming/流式）、素材同 id 自动改名 + flow 重写、显式纳管 API、素材 GC、`installed.json` 损坏隔离、跨进程 packages 锁、`dependencies` 与 flow 闭包交叉校验。
4. **产品 backlog（不阻塞）**：SelectionTool、collection `real=1` 真机 E2E、drag Plan B、P2-10、运行记录/口径收口、版本决策（是否升 0.2.0）。
5. **PR6 遗留（P2，写文档不阻塞）**：调度计划仍高级 JSON（无完整 entry/queue 表单）、stop 不打断 attempt（长任务显示停止中）、租约 pass 级不是 attempt 级、无调度「立即执行 entry」、`?page=schedule` 依赖 `window.open`、Runner 无重置入口（用 CLI）、日志环形缓冲仅内存、stale 回收 TOCTOU。
6. 每轮收口：`cargo test --workspace` 全绿（**基线 713**）+ UI pure test（**1167/159/0**）+ `node --check` + `git diff --check`。**不 tag / 不 release**（除非用户明确说发布）。

## 4. 本轮裁定 / 约束（带锚点）

- **stop 语义**：只「跑完当前 attempt」后退出，**不打断**正在执行的任务；单活动实例约束不变。
- **租约语义**：pass 级、跨进程、活 pid（含同进程）一律冲突；无 live config 的裸运行不启用；第三方直接调 runtime 不受保护。
- **`/runner/run` 合并**：Runner values 为底层、body params 覆盖、类型保真、不读其它任务 values；`run::execute` 再叠 meta defaults。
- **Studio 无调度页**：旧 DOM/对话框/JS/CSS 全删；`?page=schedule` 兼容跳转 Runner；`?reset=1` 只提示（重置记忆 = CLI）。
- **数据边界**：用户数据（`assets/`/`frames/`/`tasks/`/`scheduler.json(.bak)`）不提交；`dist/`、`packages/`、`config/device.lease`、`scheduler.state.lock` 不入 git；本子进程只改 docs。
- 若用户改 Q1~Q7 任一项，先回写计划文档 §D.2 与稿源再动代码。

## 5. 阻塞 / 待用户裁定

- **无技术阻塞**。唯一等待项 = 提交本批 docs（父 `edc037b`）。
- 待用户裁定：Phase 5 范围（上传/素材改名/纳管/GC）与产品 backlog 优先级；是否升版本 0.2.0；是否发布。

## 6. 证据

- **PR6**：`D:\OAS-dev\_verify\runner-studio-pr6-20261104\`（`PR6-BACKEND-REPORT.md` / `PR6-UI-REPORT.md` / `REVIEW.md` / `FIX-REPORT.md` / `e2e/` / `runner-queue.png` / `runner-run.png` / `studio-clean.png` + 本轮 `DOCS-REPORT.md`）。
- **PR5**：`_verify\runner-studio-pr5-20261104\`；**PR4**：`_verify\runner-studio-pr4-20261104\`；**PR3**：`_verify\runner-studio-pr3-20261104\`；**PR2**：`_verify\runner-studio-pr2-20261104\`；**PR1**：`_verify\runner-studio-pr1-20261104\`。
- **设计稿源**：`_verify\arch-runner-studio-20261104\RUNNER-STUDIO-CONFIG-DESIGN.md`（659 行 / 235+ `file:line`）。
- **台账**：`docs/PENDING-LEDGER.md §E-79`（§F~§J + **§K PR6**）；现行口径：`docs/RUNNER-STUDIO-SPLIT-PLAN.md`（§A.9~§A.14）、`docs/ENGINE-DESIGN.md §22.44~§22.49`、`packaging/CROSS-PLATFORM.md`、`packaging/README-DEPLOY.md`、`packaging/README-RUNNER.txt`。

## 7. 连续性最小验证命令（恢复后先跑）

```text
cd D:\OAS-dev\oas-core
git log -1 --format="%h %ci %s"          # 期望 edc037b（PR6；本批 docs 提交后为新 HEAD）
git status --porcelain                   # 期望：docs 8 份（若未提交）+ 用户数据（勿提交 assets/frames/tasks/scheduler.json）
node apps\oasd\ui\ui_pure_test.js        # 期望 1167 checks / 159 ids / 0 dead（runner audit 75/0/6）
cargo test --workspace                   # 期望 713 passed / 0 failed（oasd 347 / task 111 / flow 62 / runtime 157+2）
```
