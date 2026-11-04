# RESUME — 断点续接卡（session-memory 强制 6）

> 规则：本文件每次断点覆盖重写；≤150 行；另一个会话只读本文件 + `RESUME-INDEX.md` 最新行就能继续。
> 细节：`docs/RUNNER-STUDIO-SPLIT-PLAN.md`（§A.9~§A.16）· `PENDING-LEDGER.md §E-79`（§F~§M）· `ENGINE-DESIGN.md §22.44~§22.51`。

```yaml
task_id: oas-core-runner-studio-split
checkpoint_seq: 108
updated_at: 2026-11-05T03:35+08:00
trigger: 最终审计（P0=0；后端 P1×4 + 文档 P1×1）→ 代码 P1 全部修复（68e5768）→ 交接文档收尾（本批 docs，seq 108，紧随其后提交）→ 交接完成，电脑关机（Lead 执行）
status: handoff-ready          # 代码/文档已收口，无技术阻塞；等待产品验收与 Phase 5 决策
code_head: 68e5768            # 本文件记录的代码 HEAD（最终审计 P1 修复提交）
docs_head: 本批 docs 提交后见 `git log -1`（父 = 68e5768；seq 108；不要凭记忆写死）
next_action: 交接完成。新接手者按 §3 做产品验收（§D.4 19 步）/ P2 清理 / Phase 5 选择；不 tag / 不 release
```

## 0. 目标（本阶段）

1. Studio（编辑端，8788）/ Runner（使用端，8789）分离：同一份代码两个 surface。
2. Q1~Q7 已批准；**PR1~PR6 + PR7 全部 ✅ 已提交**（PR1 `da9da9d` / PR2 `dfe8776` / PR3 `c09df25`+`048dff5` / PR4 `442f5cb` / PR5 `617435d`+`830058d` / PR6 `edc037b` / PR7 `5e45c81`）。
3. 最终审计（2026-11-05）发现的后端 P1×4 已全部修复并提交 **`68e5768`**；文档 P1（HEAD/状态陈旧）由本批 docs（seq 108）修复。
4. 本批 docs = 本卡 + `SESSION-HANDOFF` / `START-HERE` / `PENDING-LEDGER` / `RUNNER-STUDIO-SPLIT-PLAN` / `ENGINE-DESIGN` / `README` / `RESUME-INDEX` + `SCHEDULER-RUNNER-PLAN`（历史注）＝ **9 份文件，只改 docs**。另有 Lead 的 `packaging/` 文案订正随本批提交。

## 1. 状态（2026-11-05 03:3x，seq 108）

| 项 | 值 |
|---|---|
| 分支 / 代码 HEAD | `dev`；HEAD = **`68e5768`**（`fix(runtime): final-audit P1 fixes (replay lease, worker panic, scheduler guards, registry degradation)`）；**交接 docs 提交紧随其后（seq 108），最终 HEAD 以 `git log -1` 为准**；未 tag / 未 release |
| 任务状态 | **handoff-ready**（代码/文档已收口）；剩余 = 产品验收 / P2 清理 / Phase 5 决策，均非技术阻塞 |
| 双入口 | **Studio `http://127.0.0.1:8788/`**（`oasd studio`；`oasd serve` = deprecated 别名）· **Runner `http://127.0.0.1:8789/`**（`oasd runner`，nav **6 项**；启动即内嵌调度，`--no-scheduler` 关闭） |
| 测试 | `cargo test --workspace` **721 / 0**（基线 713，最终审计修复 **+8**；oasd **355** / oas-task 111 / oas-flow 62 / oas-runtime 157 lib + 2 integration）；`cargo build -p oasd --features ocr` 与 release build exit 0（**5 warnings**，无新增）；UI pure test **1559 checks / Studio 159 ids / Runner 151 ids / 0 dead / 6 nav**（无 UI 改动，数字延续）；`node --check` 3/3；`git diff --check` clean |
| 分包 | `dist/oasd-runner-v0.1.0-win64.zip` = **37,690,904 B**，SHA256 **`A3BCB4B43EFBF8373A74ADDB44DC4EE5A2AD3847C81D1F61CC039229FE15F960`**（代码 P1 修复后重编；BUILD-INFO `git: 68e5768`，surface=runner，entry `oasd.exe runner --port 8789 --config config/live.json --ui ui`）；**不入 git** |
| 服务 | **两个服务当前已停止**（8788/8789 无监听）；交接后电脑将关机。启动命令见 §3 |
| 数据边界 | 工作区只有用户数据未提交：`assets/assets.json`、`frames/frames.json`、`scheduler.json`(+`.bak`)、未跟踪 `frames/frame-43.png`/`frame-44.png`、`tasks/切换预设御魂/`；**用户数据不入库** |
| 提交边界 | 已提交：PR1~PR7 + seq 107 docs `8344d97`；**待提交 = 本批 docs（9 份文件；父 `68e5768`）+ Lead 同时提交的 packaging 文案订正（`packaging/README-RUNNER.txt` / `README-DEPLOY.md`：绝对路径 + 端口说明）**；用户数据不提交；`dist/` / `packages/` 不入 git |

## 2. 最终审计与 P1 修复（2026-11-05）

两路只读审计（P0=0；session 要求矩阵 R1–R20 = **16 ✅ / 4 ⚠ / 0 ❌**；判定「有条件可交接」）：
`D:\OAS-dev\_verify\runner-studio-final-audit-20261105\AUDIT-BACKEND.md`、`AUDIT-UI-DOCS.md`。

| # | P1 问题 | 修复（`68e5768`） | 回归测试 |
|---|---|---|---|
| P1-1 | replay / dry-run 仍抢设备租约，回放被调度 409 | `run.rs` **先判 mode 再碰租约**（replay→`skipped(replay)`、dry-run→`skipped(dry-run)`、live 才 acquire）；Studio `/task/run` 仅 live 预取；scheduler replay pass 跳过并输出 `scheduler_lease_skipped reason=replay`；`lease=<label>` 观测行 | `run.rs` replay 在外部活租约下 Done；dry-run 无租约；live 仍 409；Studio replay/dry-run 在调度租约下 200；scheduler replay pass 不被阻塞 |
| P1-2 | run worker panic 后 `running=true` 永久 409 | `spawn_run_worker` 用 `catch_unwind` 包 executor；panic → `outcome="Panic"`、`exit=2`、`error/log_tail` 清理、**无条件 `running=false` + 租约 Drop** | Studio+Runner 各注入 panic：running=false、outcome=Panic、租约释放、**再次 run 不是 409** |
| P1-3 | install/activate 可在调度运行中替换任务 | `scheduler_busy_reason`（本进程线程 running 或 `<state>.lock` 被 live pid 持有）→ **409 `scheduler_running`**（`error`/`code`/`errors`/`message`），`force` 不绕过；stop 后恢复 | 嵌入式调度 start 后 install/activate 均 409；外部 CLI 写 lock 也 409；stop 后 install 200 + v2→v1 activate 回滚 |
| P1-4 | `installed.json` 损坏让 `/runner/tasks` 整页 500 | `/runner/tasks` **降级 200 + warning**，任务显示 `unmanaged`；包生命周期 API 保持 500 + `code=installed_registry_corrupt` + 可操作恢复提示（删除/重命名 `installed.json` 后重启）；**无自动隔离**（限制） | 坏 registry：tasks 200 仍列任务、packages/install/activate/uninstall 500 且文案含恢复步骤；install 失败不动 `tasks/` |
| P1-5 | 交接文档 HEAD/状态陈旧 | 本批 docs（seq 108）统一到 **代码 HEAD 68e5768** + 「docs 提交紧随其后」 | 本文件 + `START-HERE`/`SESSION-HANDOFF`/`README`/`PENDING`/`PLAN`/`ENGINE` |

## 3. 下一步交接指引（可直接执行）

```bat
REM 0) 恢复前先核实（磁盘 + 命令，不信摘要）
cd D:\OAS-dev\oas-core
git log -1 --format="%h %ci %s"      REM 期望 68e5768 或其后 docs 提交
git status --porcelain               REM 期望只有用户数据（勿提交 assets/frames/tasks/scheduler.json）
cargo test --workspace               REM 721/0（oasd 355 / task 111 / flow 62 / runtime 157+2）
node apps\oasd\ui\ui_pure_test.js    REM 1559 checks / 159 studio ids / 151 runner ids / 0 dead / 6 nav

REM 1) 启动（当前已停止；构建需 --features ocr）
cargo build -p oasd --features ocr
target\debug\oasd.exe studio --config config\live.json        REM Studio 8788（编辑端）
target\debug\oasd.exe runner --config config\live.json        REM Runner 8789（自动内嵌调度；--no-scheduler 关闭）
REM 打包用户：start-studio.bat（8788）/ start-runner.bat（8789，固定 --port 8789，改 runner.port 不生效）

REM 2) 数据目录（单机单实例，两端口共享）
REM tasks\  assets\  frames\  runs\  scheduler.json  config\live.json  config\values.json
REM Runner 额外：config\device.lease（设备租约）、<state>.lock、packages\、CWD\logs\runner.log、scheduler.events.jsonl
```

1. **产品验收**：按 `RUNNER-STUDIO-SPLIT-PLAN §D.4` 19 步（V1~V7）跑；V1~V4/V6-16/17 已有前序证据，**建议补真实产品任务（click/OCR/image）与真实设备验收**。
2. **P2 清理（不阻塞）**：PR7 的 8 条 UI P2（状态卡旧值、config GET 误报、重复错误行、reset 兜底/409 细节、dialog busy、锁过期、死代码/CSS）+ 本卡 §5 的后端/打包 P2。
3. **Phase 5（用户未定范围）**：上传模式、素材同 id 自动改名 + flow 重写、显式纳管 API、素材 GC、`installed.json` 自动隔离、跨进程 packages 锁、`dependencies` 与 flow 闭包交叉校验。
4. **产品 backlog**：SelectionTool、collection `real=1` 真机 E2E、drag Plan B、P2-10、运行记录/口径收口、是否升 0.2.0。

## 4. 验收进度（§D.4 V1~V7）

| 组 | 状态 | 证据 |
|---|---|---|
| V1 两端不同入口 | ✅ | PR1：`_verify/runner-studio-pr1-20261104/`（8788/8789 双进程、互斥 404） |
| V2 Studio 发布 | ✅ | PR5：`_verify/runner-studio-pr5-20261104/e2e/` |
| V3 Runner 安装/卸载/回滚 | ✅（依赖自动装/被依赖 409 由集成测试覆盖） | PR5 同上 |
| V4 任务配置 | ✅ | PR2：`pr2demo`（8790/8791） |
| V5 调度与执行（13~15） | ✅ | PR6 内嵌调度 + **PR7 用迁移后完整 UI 重验**（`_verify/runner-studio-pr7-20261105/`） |
| V6 子任务取父配置（16~18） | 16/17 ✅ / 18 单测 | PR4 + PR6 + PR7 最小任务链多参数（`runner-child/9/007`、`runner-parent/7/007`、`entry-parent/5/007`、`ui-entry/4/007`） |
| V7 不动用户数据 | ✅ | PR1~PR7 + 最终审计修复均未提交用户数据 |

## 5. 已知限制（P2/已知缺陷，写文档、不阻塞交接）

- **installed.json**：损坏不自动隔离/repair（tasks 200 降级，生命周期 API 500 + 可操作文案）；`/runner/tasks` 不并集「installed 但目录丢失」记录。
- **租约**：pass 级不是 attempt 级；stale 回收 TOCTOU；无 replay 的 `--dry-run` 只读抓屏但不占租约（并发读风险，有意接受）。
- **调度/安装互斥**：install/activate 的 `scheduler_busy_reason` 是 check-then-act 小窗口（非原子）；panic 安全依赖 `panic=unwind`。
- **包**：`manifest.asset_refs`/`dependencies` 未与 flow 闭包逐项交叉校验；`move_dir` copy fallback 可能合并；`snapshot_entry` 失败一致性；`PackageError::Other`→400（应 500）；`GET /task/package` 只扫默认 dist。
- **Runner UI**：暂不渲染 `/runner/tasks` 的 `warning` 字段；PR7 的 8 条 P2（见 §3）。
- **打包/入口**：`start-runner.bat`/`stop-runner.bat` 固定 `--port 8789`（Lead 本批已把 `README-RUNNER.txt`/`README-DEPLOY.md` 订正为「`.oaspkg` 填本机绝对路径」「`start-runner.bat` 固定 8789，换端口需直接命令行」；packaging 改动随本批提交）；Studio `openRunnerPage` 硬编码 8789；`installed.json`/`.values.json.tmp-*` 与 `.gitignore *.tmp` 不匹配。
- **调度器**：`scheduler.json` 缺失/坏时内嵌线程退出（非设计「空转 blocked」）；`?page=schedule` 依赖 `window.open`；墙钟 24h soak 未跑完（只有 96 轮加速 soak）。
- **跨平台**：v1 仅 Windows x64；Android 使用端仅设计预留（`packaging/CROSS-PLATFORM.md`）。

## 6. 连续性最小验证命令（恢复后先跑）

```text
cd D:\OAS-dev\oas-core
git log -1 --format="%h %ci %s"     # 68e5768 或其后 seq 108 docs 提交
git status --porcelain              # 只有用户数据；docs 应已 clean
cargo test --workspace              # 721/0（oasd 355 / task 111 / flow 62 / runtime 157+2）
node apps\oasd\ui\ui_pure_test.js   # 1559 / 159 / 151 / 0 / 6 nav
```

## 7. 证据

- **最终审计**：`_verify\runner-studio-final-audit-20261105\`（`AUDIT-BACKEND.md` / `AUDIT-UI-DOCS.md` / `FIX-REPORT.md` / `fix-test-workspace.log` / `fix-test-oasd.log` / `fix-build.log` / `final-workspace-test.log` / `final-ui-test.log` / `final-package.log`；本批 `DOCS-HANDOFF-REPORT.md`）。
- **PR7**：`_verify\runner-studio-pr7-20261105\`（`PR7-UI-REPORT.md` / `RECON-REPORT.md` / `REVIEW.md` / `e2e/` / 4 张截图）。
- **PR1~PR6**：`_verify\runner-studio-pr1-20261104\` ~ `pr6-20261104\`。
- **现行口径**：`docs/RUNNER-STUDIO-SPLIT-PLAN.md`（§A.9~§A.16）、`docs/ENGINE-DESIGN.md` §22.44~§22.51、`docs/PENDING-LEDGER.md §E-79`（§F~§M）、`packaging/CROSS-PLATFORM.md`、`packaging/README-DEPLOY.md`。
