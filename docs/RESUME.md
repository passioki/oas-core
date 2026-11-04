# RESUME — 断点续接卡（session-memory 强制 6）

> 规则：本文件每次断点覆盖重写；≤150 行；另一个会话只读本文件 + `RESUME-INDEX.md` 最新行就能继续。
> 细节：`docs/RUNNER-STUDIO-SPLIT-PLAN.md`（**§A.9 PR1 + §A.10 PR2 + §A.11 PR3 + §A.12 PR4 + §A.13 PR5 实现记录**）· `PENDING-LEDGER.md §E-79`（§F PR1 / §G PR2 / §H PR3 / §I PR4 / **§J PR5**）· `ENGINE-DESIGN.md §22.44~§22.48`。

```yaml
task_id: oas-core-runner-studio-split
checkpoint_seq: 105
updated_at: 2026-11-04T22:5x+08:00
trigger: PR5（TaskPackage .oaspkg 发布/安装 + flow_ops 闸门 + 素材初始化 + 3 P1 评审修复）实现/验证/打包完成 → docs 收尾（本子进程只改 docs）
status: in_progress
next_action: Lead 已提交 PR5 代码 617435d + seq105 docs 830058d；本子进程后修的 docs（HEAD 830058d，待提交）；随后开 PR6 Runner 内嵌调度/队列执行 + /runner/run + 设备租约 + 调度健康/启停
```

## 0. 目标（本阶段）

1. **Q1~Q7 已批准**（2026-11-04，全部按推荐）——见计划文档 §D.2。
2. **PR1 ✅ 已提交**（`da9da9d`）：`Surface` 拆分 + `oasd studio|runner` + Runner 只读 API/UI 骨架 + Runner-only 发行包 + 跨平台约束。
3. **PR2 ✅ 已提交**（`dfe8776`）：`meta.params` 扩 5 字段 + `config/values.json` + Studio/Runner 配置 API/UI + 运行值校验。
4. **PR3 ✅ 已提交**（`c09df25` + `048dff5`）：类型保真 + 四层优先级 + 合并后 schema 复检 + CLI 引号逃生口。
5. **PR4 ✅ 已提交**（HEAD `442f5cb`）：`call_task.params`/`inherit` + 不落盘 `with_params` + loader 严格展开 + runtime 作用域 + lint T7 + T6/checkpoint 修复。
6. **PR5 ✅ 已实现并验证（代码已提交 `617435d`；seq105 docs 已提交 `830058d`）**：`.oaspkg` 包核心 + Studio 发布 + Runner 安装/卸载/回滚 + 素材初始化/冲突 + flow_ops 闸门 + UI；评审 P1-1/P1-2 已修、P1-3 明确 defer Phase 5。
7. 本子进程只做 **docs 收尾 8 份**（不改代码/脚本/用户数据，不 commit、不跑 cargo/serve）。

## 1. 状态（2026-11-04 22:5x，seq 105）

| 项 | 值 |
|---|---|
| 分支 / HEAD | `dev`；HEAD = **`830058d`**（`docs: TaskPackage publish/install + flow_ops gate final state (seq 105)`）；**PR5 代码已提交 `617435d`、seq105 docs 已提交 `830058d`；本子进程后修的 docs 未提交，父 `830058d`**；本批后修 docs 提交后为新 HEAD（不写死 hash）；**未 tag / 未 release** |
| 任务状态 | **in_progress**（seq 105）；PR1~PR5 ✅ 已提交、**后修 docs 待提交**、PR6 待做 |
| 双入口 | **Studio `http://127.0.0.1:8788/`**（`oasd studio`；`oasd serve` = deprecated 别名）· **Runner `http://127.0.0.1:8789/`**（`oasd runner`，nav **6 项**）；均只绑 `127.0.0.1` |
| 测试 | `cargo test --workspace` **677/0**（21 targets；PR4 624 → +53 = 原 36 + 修复 17）；oas-task **111**（89→111）、oasd **311**（280→311）、oas-flow **62**、oas-runtime **157 lib + 2 integration**；OCR build + release build exit 0（**6 warnings**）；UI pure test **1442 checks / 237 ids / 0 dead**（`runner_ui_audit` **55 ids / 0 dead / 6 nav**，原 41/0/5）；`node --check` 3/3；`git diff --check` clean |
| 分包 | `dist/oasd-runner-v0.1.0-win64.zip` **37,591,694 B**（PR5 release 重建；Runner-only）+ SHA256 **823F93CDB634DA56C91421AAEF39255FBB0522ED9F275116DC2891E59D14031E** + `.sha256`；**不入 git** |
| 上一节点 | seq 104 = PR4（624/0、oasd 280、UI 1338/231/0、runner 包 37,246,079 B / SHA `07B6D930…852E506B`）→ 已提交 `442f5cb` |
| 工作区（非我改动，勿碰） | PR5 代码已提交 `617435d`；**本子进程后修的 docs（ENGINE/PENDING/RESUME/PLAN 及可能的 8 份状态行）未提交，父 `830058d`**；用户数据：`assets/assets.json`、`frames/frames.json`、`scheduler.json(.bak)`、`tasks/切换预设御魂/`（未提交，**均不提交用户数据**） |
| 文档（本轮） | `RUNNER-STUDIO-SPLIT-PLAN.md`（状态 + **§A.13** + §A.3/§B/§C.4.6/§D/§E）、`PENDING-LEDGER §E-79`（状态/§B~§E + **§J.1~J.9** + 台账/footer）、`ENGINE-DESIGN`（§22.44/§22.47 指针 + **§22.48**）、`RESUME`（本卡）、`RESUME-INDEX` 105、`START-HERE`、`SESSION-HANDOFF`、`README`（**8 份**） |

## 2. 验收清单（本轮 PR5）

- ☑ **包核心**：`package.rs`（zip 0.6.6 deflate + sha2 0.10.9）；`.oaspkg` = `manifest.json` + `checksums.sha256` + `task/` + `deps/` + `assets/` + 可选 README；`.oaspkg.sha256` sidecar；`package_id` = 内容摘要
- ☑ **加固**：zip-slip 双向；≤2000 条目、≤512 MiB 总/≤256 MiB 单条、压缩比 >100:1 拒绝、encrypted 拒绝、`take(declared+1)`；checksums 集合严格相等；运行态排除；空 `state/` 仅 warning；reproducible
- ☑ **flow_ops 闸门**：从**原始 flow** 的 `call_task.args.{params,inherit}` 推导（不扫内存 `with_params`）；manifest 写入；安装时从实际打包 flow **重算严格相等** + ⊆ `SUPPORTED_FLOW_OPS`；`min_engine` 逐段数字比较；篡改 → 400 `flow_ops does not match`
- ☑ **Studio 发布**：CLI `oasd studio publish --task … [--out] [--force] [--include-deps] [--reproducible]`（out 只允许相对 `dist/`）；HTTP `POST /task/publish`（200/400 lint/409 exists）+ `GET /task/package`（版本降序）；lint 失败不产半成品；发布校验接 T7
- ☑ **Runner 安装**：`POST /runner/install {path,force?}`（Phase 1 仅本机绝对路径）；严格顺序（读包→flow_ops→staging/checksums→meta/template→deps→tree→flow_ops 重算→lint→dirty→同版本 force→素材计划→备份→提交→素材→installed.json→清理）；任一步回滚；同版本无 force 409 `already_installed`；裸目录覆盖需 force
- ☑ **卸载/回滚**：`POST /runner/uninstall` 先备份 `backup/uninstall-<id>-<ts>/`，守卫 scheduler/运行中/被依赖 409；`POST /runner/packages/activate` 恢复最近/显式 backup，卸载后无记录自动扫 backup 恢复（`restored_from`），归属校验
- ☑ **素材（P1-1）**：全新 Runner 用包快照初始化 store + warning；无快照有引用 400 `missing_asset_snapshot`；同 id 变化 400 `asset_conflicts`（不自动改名，P1-3 defer Phase 5）；失败回滚
- ☑ **UI**：Studio 发布对话框（历史/成功 path/package_id/sha/warnings、409/400 逐行）；Runner 第 6 页「安装包」（路径+force 安装、列表/卸载/回滚、unmanaged、任务行徽标）；全 textContent、无跨 surface
- ☑ **测试**：workspace **677/0**（oas-task 111 / oasd 311 / flow 62 / runtime 157+2）；UI **1442/237/0**（audit 55/0/6）；OCR+release build exit 0（6 warnings）；`git diff --check` clean
- ☑ **Lead E2E（临时数据，不碰用户数据）**：`asset_task` 发布 → 全新 Runner 安装 200 + 初始化素材 warning → 同版本 409 → force 200 → 卸载 backup → activate `restored_from`；篡改 flow_ops/zip-slip 400；两端路由 404；CDP 双端 UI
- ☐ **提交后修 docs**（父 `830058d`；PR5 代码 `617435d` 与 seq105 docs `830058d` 已提交；不含用户数据；`dist/`/`packages/` 不入 git）

## 3. 下一步（精确动作）

1. **Lead 已提交 PR5**：代码/UI/打包 = `617435d`（16 文件，父 `442f5cb`）；seq105 docs = `830058d`（8 份）。本子进程后修的 docs（当前仅 4 份 diff）待提交到 `dev`；提交前复核 `git status` **不含** `assets/`、`frames/`、`tasks/`、`scheduler.json(.bak)`；`dist/`、`packages/` 不入 git。
2. **PR6（下一步开发，计划文档 §D.3）**：Runner 内嵌调度线程/队列执行 + `POST /runner/run` 临时运行 + 调度健康/启停 + 设备租约。
3. **PR6 的 `/runner/run` 必须自行合并 values 层**（body params 覆盖 values）；`run.rs` 不自动读 values（PR3 约束）。
4. **Phase 5/后续（不阻塞）**：安装上传/流式、素材同 id 自动改名 + flow 重写、显式纳管 API、素材 GC、`installed.json` 损坏隔离、跨进程 packages 锁、`dependencies` 与 flow 闭包交叉校验。
5. 每 PR 收口：`cargo test --workspace` 全绿（**PR5 后基线 677**）+ UI pure test（**1442/237/0**）+ `node --check` + `git diff --check`；按 §D.4 对应验收项收口。
6. **不 tag / 不 release**（除非用户明确说发布）。

## 4. 本轮裁定 / 约束（带锚点）

- **Phase 1 只做 path 安装**：`upload/incoming/streaming` defer Phase 5；4 MiB JSON body 限制仍适用于请求（请求只传路径）。
- **flow_ops 按语义粒度**：`call_task` / `call_task.params` / `call_task.inherit.none` / `with_params`；安装重算**严格相等**（不是“包含即可”），老 Runner 一律拒绝。
- **同版本重装必须 force**（409 `already_installed`）；版本变化正常升/降级；裸目录/unmanaged 覆盖需 force（409 `dirty_task`）。
- **素材冲突 fail-closed**：同 id 不同定义/字节 → 400；**不自动改名/不改写 flow**（设计 Phase 5）；全新 Runner 用包快照初始化 store。
- **卸载永不直接删**：先 `packages/backup/uninstall-<id>-<ts>/`；守卫（scheduler/运行中/被依赖）无 force 409；activate 只回滚任务目录/registry，不回收素材。
- **数据边界**：终端用户只下载 Runner-only 包 + `.oaspkg`；用户数据（`assets/`/`frames/`/`tasks/`/`scheduler.json`）不提交；`dist/`、`packages/` 不入 git；本子进程只改 docs。

## 5. 阻塞 / 待用户裁定

- **无技术阻塞**。唯一等待项 = 提交本批后修 docs（父 `830058d`；PR5 代码 `617435d` / seq105 docs `830058d` 已提交；边界见 §3.1 / 计划文档 §A.13 / ENGINE §22.48）。
- 用户已接受 **P1-3 限制**（素材同 id 变化不能升级；无自动改名 + flow 重写，Phase 5 再议）。
- 若用户改 Q1~Q7 任一项，先回写计划文档 §D.2 与稿源再动代码。

## 6. 证据

- **PR5**：`D:\OAS-dev\_verify\runner-studio-pr5-20261104\`（`PR5-BACKEND-REPORT.md` / `PR5-UI-REPORT.md` / `REVIEW.md` / `FIX-REPORT.md` / `e2e/*.out.txt` / `studio-publish*.png` / `runner-packages.png` + 本轮 `DOCS-REPORT.md`）。
- **PR4**：`D:\OAS-dev\_verify\runner-studio-pr4-20261104\`（`PR4-BACKEND-REPORT.md` / `REVIEW.md` / `FIX-REPORT.md` / `workspace-fix.log` / `e2e/` + `DOCS-REPORT.md`）。
- **PR3**：`D:\OAS-dev\_verify\runner-studio-pr3-20261104\`（`PR3-BACKEND-REPORT.md` / `REVIEW.md` / `FIX-REPORT.md` / `workspace-final.log` / `e2e/` + `DOCS-REPORT.md`）。
- **PR2**：`D:\OAS-dev\_verify\runner-studio-pr2-20261104\`（实现/评审/测试报告 + `studio-config.png` / `runner-config.png` + `DOCS-REPORT.md`）。
- **PR1**：`D:\OAS-dev\_verify\runner-studio-pr1-20261104\`（`PR1-*-REPORT.md` / 测试日志 / `runner-*.png` / `studio-nav.png`）。
- **设计稿源**：`_verify\arch-runner-studio-20261104\RUNNER-STUDIO-CONFIG-DESIGN.md`（659 行 / 235+ `file:line`）。
- **台账**：`docs/PENDING-LEDGER.md §E-79`（§F PR1 / §G PR2 / §H PR3 / §I PR4 / **§J PR5**）；**现行口径**：`docs/RUNNER-STUDIO-SPLIT-PLAN.md`（§A.9~§A.13）、`docs/ENGINE-DESIGN.md §22.47/§22.48`、`packaging/CROSS-PLATFORM.md`、`packaging/README-DEPLOY.md`、`packaging/README-RUNNER.txt`。

## 7. 连续性最小验证命令（恢复后先跑）

```text
cd D:\OAS-dev\oas-core
git log -1 --format="%h %ci %s"          # 期望 830058d（PR5 代码 617435d + seq105 docs 已提交）
git status --porcelain                   # 期望：后修 docs（4 份左右）+ 用户数据（勿提交 assets/frames/tasks/scheduler.json）
node apps\oasd\ui\ui_pure_test.js        # 期望 1442 checks / 237 ids / 0 dead（runner audit 55/0/6）
cargo test --workspace                   # 期望 677 passed / 0 failed（oas-task 111 / oasd 311 / flow 62 / runtime 157+2）
```
