# RESUME — 断点续接卡（session-memory 强制 6）

> 规则：本文件每次断点覆盖重写；≤150 行；另一个会话只读本文件 + `RESUME-INDEX.md` 最新行就能继续。
> 细节：`docs/RUNNER-STUDIO-SPLIT-PLAN.md`（§A.9~§A.17）· `PENDING-LEDGER.md §E-79/§E-80` · `ENGINE-DESIGN.md §22.44~§22.52`。

```yaml
task_id: oas-core-runner-studio-split
checkpoint_seq: 110
updated_at: 2026-11-05T13:3x+08:00
trigger: 双端发行包（package.ps1 重写为 Studio-only + 两 zip 产出）→ 分离度 E2E 75/75 → F1（PS5.1 BOM 使 Runner 包 scheduler.json 首启 invalid_config）修复 → dev-serve 双入口化 → 参数 UI 审计（待用户裁定）→ 本批脚本+docs 收尾
status: handoff-ready          # 双端 release 已产出并被 Lead 独立复核；待用户裁定参数 UI 方案 + 提交/推送
code_head: 68e5768            # 本轮 Rust 零改动（代码 HEAD 仍 = 最终审计修复提交）
docs_head: 本批 docs 提交后见 `git log -1`（父 `e9404a4`；dev = main = origin）
next_action: ① 用户裁定参数 UI 方案（推荐 A+C：构建页入口 + Studio-only GET /task/lint）② 提交/推送本批 scripts+docs ③ 产品验收（§D.4 19 步）；不 tag / 不 release
```

## 0. 目标（本阶段）

1. Studio（编辑端，8788）/ Runner（使用端，8789）分离：同一份代码两个 surface。
2. Q1~Q7 已批准；**PR1~PR7 全部 ✅ 已提交**（PR1 `da9da9d` / PR2 `dfe8776` / PR3 `c09df25`+`048dff5` / PR4 `442f5cb` / PR5 `617435d`+`830058d` / PR6 `edc037b` / PR7 `5e45c81`）；最终审计 P1 ✅ `68e5768`。
3. **本批（seq 110）= 用户三项要求**：① 分离度检查 + 试发布两端 release；② 更新过时的 `scripts/dev-serve.*`；③ 参数配置在编辑端未被表达（是「设置变量」吗）⇒ 只读审计 + 方案待裁定。

## 1. 状态（2026-11-05 13:3x，seq 110）

| 项 | 值 |
|---|---|
| 分支 / HEAD | `dev`；本批提交前 HEAD = `e9404a4`（seq 108 docs，父 `68e5768`）；**本批提交后以 `git log -1` 为准**；未 tag / 未 release |
| 任务状态 | **handoff-ready**：双端 release 已产出并独立复核；参数 UI 审计完成，**待用户裁定方案** |
| 双入口 | **Studio `http://127.0.0.1:8788/`**（`oasd studio`）· **Runner `http://127.0.0.1:8789/`**（`oasd runner`，nav 6，内嵌调度）；`oasd serve` = Studio deprecated 别名 |
| 测试 | `cargo test --workspace` **721 / 0**（21 targets；本轮 Rust 零改动，Lead 复跑 `EXITCODE=0`）；UI pure test **1559 / Studio 159 / Runner 151 / 0 dead / 6 nav**（UI 零改动）；`git diff --check` clean |
| 发行包 | **Studio** `dist/oasd-v0.1.0-win64.zip` = **37,789,671 B**，SHA256 **`920AF6686314E2C28881C59C442C0AABFC45FB0646B284BA53DEFD0D080F07C3`**（41 entries；surface=studio；真实 tasks/assets 数据）；**Runner** `dist/oasd-runner-v0.1.0-win64.zip` = **37,690,867 B**，SHA256 **`ECB37A783FB92FD64F78BC788DA6DA3A61CDB7FE6897B76E3C0777E78D339ADE`**（23 entries；surface=runner；空 v4 `scheduler.json` 无 BOM）；**不入 git** |
| 服务 | **全部停止**（8787/8788/8789 实测无监听、无 `oasd` 进程；release 评测后已清理） |
| 数据边界 | 工作区用户数据：`assets/assets.json`、`frames/frames.json`、`scheduler.json(.bak)`、未跟踪 `frames/frame-43.png`/`frame-44.png`、`tasks/切换预设御魂/`；**用户数据不入库** |
| 提交边界 | **待提交 = 本批 scripts（`package.ps1` / `package-runner.ps1` / `dev-serve.ps1` / `dev-serve.bat`）+ docs（START-HERE/RESUME/RESUME-INDEX/SESSION-HANDOFF/PENDING/ENGINE/PLAN/README + `packaging/README-DEPLOY.md`）**；用户数据不提交；`dist/` / `packages/` 不入 git |

## 2. 本批交付（seq 110）

| # | 交付 | 结果 / 证据 |
|---|---|---|
| 1 | **Studio-only 发行包** | `scripts/package.ps1` 由旧「全量包」重写为 Studio-only（旧对照 D1-D19：9 项不符全消 + D7/D8 补齐）：Studio UI 白名单 8 文件 + 真实 tasks/assets + 空 frames/runs/logs + 4 启停脚本 + config/models/adb + `surface: studio` BUILD-INFO + stage 审计 + `/` 分隔 zip。证据：`D:\OAS-dev\_verify\release-both-20261105\DESIGN-VS-NOW.md`、`package-files.json` |
| 2 | **Runner 包重建 + F1 修复** | `package-runner.ps1` 新增 `Write-Utf8NoBom`（`[System.IO.File]::WriteAllText` + `UTF8Encoding($false)`）覆盖 `scheduler.json`/`VERSION`/`BUILD-INFO`/`tasks\README.txt` + stage 首字节守卫；`package.ps1` 同类修复。证据：`F1-FIX.md`（修复前 `scheduler_mode=invalid_config` / `scheduler --status` exit 2；修复后 exit 0、`scheduler_mode=stop`、无 `invalid JSON`） |
| 3 | **分离度 E2E 75/75** | 两 zip 解压到仓外、CWD=解压目录、`--no-open`：Studio 8788 单进程、8789/8787 不监听、`/app.js` `/task/library` 200、`/runner.js` `/runner/tasks` `/runner/health` 404；Runner 8789 单进程、`/runner.js` `/runner/tasks` `/runner/health` 200、`/task/library` `/app.js` 404；两包 entry 互斥、0 反斜杠、自包含无仓库路径。证据：`e2e-summary.json`、`REPORT.md`、2 张截图 |
| 4 | **dev-serve 双入口化** | `scripts/dev-serve.ps1` 默认 `studio` 8788；`-Surface runner` → 8789（`-NoScheduler` 可选）；保留 `-Release/-SkipBuild/-NoOpen/-DryRun` + OCR 模型；8787 退役；`dev-serve.bat` 注释/传参说明更新；START-HERE 旧 8787 描述已同步 |
| 5 | **参数 UI 审计（待裁定）** | `_verify/ui-params-audit-20261105/PARAMS-UI-AUDIT.md`（只读，398 行）：**「设置变量」≠ 任务参数**；meta.params 唯一入口 = 任务库「配置项…」JSON 对话框；任务构建页 0 入口/0 提示；`save_flow` 不 lint、Studio 无 lint 路由 ⇒ 发布 400 T3 才首次反馈；**推荐 A+C**（构建页入口/参数面板 + Studio-only `GET /task/lint`） |

## 3. 下一步交接指引（可直接执行）

```bat
REM 0) 恢复前先核实（磁盘 + 命令，不信摘要）
cd D:\OAS-dev\oas-core
git log -1 --format="%h %ci %s"      REM 本批提交后以实际为准（父 e9404a4）
git status --porcelain               REM 期望只有用户数据（勿提交 assets/frames/tasks/scheduler.json）
cargo test --workspace               REM 721/0（oasd 355 / task 111 / flow 62 / runtime 157+2）
node apps\oasd\ui\ui_pure_test.js    REM 1559 / 159 / 151 / 0 / 6 nav

REM 1) 开发启动（当前已停止；构建需 --features ocr）
cargo build -p oasd --features ocr
scripts\dev-serve.bat                          REM Studio 8788（默认）
scripts\dev-serve.bat -Surface runner          REM Runner 8789（内嵌调度；-NoScheduler 关闭）
scripts\dev-serve.bat -DryRun                  REM 只打印命令

REM 2) 打包（release exe 已存在；两包已产出，-DryRun 可预览）
scripts\package.ps1 -SkipBuild                 REM Studio -> dist\oasd-v0.1.0-win64.zip
scripts\package-runner.ps1                     REM Runner -> dist\oasd-runner-v0.1.0-win64.zip

REM 3) release 分离度 E2E 复跑（仓外脚本，自动解压+起停+断言）
powershell -NoProfile -ExecutionPolicy Bypass -File D:\OAS-dev\_verify\release-both-20261105\run-e2e.ps1
```

1. **用户裁定参数 UI 方案**（A+C 推荐；报告 §5 有 B/D/E 与工作量）→ 再实施；不要先动 builder/app.js。
2. **提交/推送本批**：`git add scripts/package.ps1 scripts/package-runner.ps1 scripts/dev-serve.ps1 scripts/dev-serve.bat docs packaging/README-DEPLOY.md`（**显式路径，勿 `git add -A`**）；push `dev`，ff 同步 `main`；不 tag / 不 release。
3. **产品验收**：`RUNNER-STUDIO-SPLIT-PLAN §D.4` 19 步（V1~V7）；本批 release E2E 已强化 V1（两端独立 release），建议补真实产品任务 + 真机。
4. **P2 清理 / Phase 5 / 产品 backlog**：见 §5 与 `PENDING-LEDGER §E-79 §M`；不阻塞。

## 4. 验收进度（§D.4 V1~V7）

| 组 | 状态 | 证据 |
|---|---|---|
| V1 两端不同入口 | ✅ **本批补强**：两端 release 包各自独立运行、互斥 404、单进程/单端口 | `release-both-20261105/e2e-summary.json` 75/75 + `lead-recheck-20261105/recheck-log.txt` ALL PASS |
| V2 Studio 发布 | ✅ | PR5：`_verify/runner-studio-pr5-20261104/e2e/` |
| V3 Runner 安装/卸载/回滚 | ✅（依赖自动装/被依赖 409 由集成测试覆盖） | PR5 同上 |
| V4 任务配置 | ✅ | PR2：`pr2demo`（8790/8791） |
| V5 调度与执行（13~15） | ✅ | PR6 内嵌调度 + PR7 迁移后完整 UI 重验（`_verify/runner-studio-pr7-20261105/`） |
| V6 子任务取父配置（16~18） | 16/17 ✅ / 18 单测 | PR4 + PR6 + PR7 最小任务链多参数 |
| V7 不动用户数据 | ✅ | 本批只提交 scripts/docs（显式路径），用户数据不入库 |

## 5. 已知限制（P2/已知缺陷，写文档、不阻塞）

- **F1 已修（2026-11-05）**：PS 5.1 `Set-Content -Encoding UTF8` 写 BOM ⇒ `serde_json` 拒绝 ⇒ Runner 包 `scheduler.json` 首启 `invalid_config`；打包脚本已改 `WriteAllText` 无 BOM + stage 首字节审计（START-HERE §3 E17）；历史包须用新脚本重建。
- **参数 UI 缺口（待裁定）**：任务构建页无参数入口/提示；唯一入口 = 任务库「配置项…」JSON 对话框；`POST /task/save_flow` 不跑 lint、Studio 无 `/task/lint`；`param()` 未声明只在发布 400 T3 或运行期报错；Studio 试运行 UI 无 params 输入；Runner `group` 未渲染、数字 enum 非下拉（P2）。
- **Studio 包数据边界**：按 `§A.8.1` 内嵌工作区真实数据快照（3 任务 / assets 31 条 + 5 图）；对外分发前需确认数据边界（未来可加 `-NoData`）。
- **installed.json**：损坏不自动隔离/repair；`/runner/tasks` 不并集 installed-but-missing。
- **租约/调度**：pass 级不是 attempt 级；stale 回收 TOCTOU；install/activate 调度检查是 check-then-act 小窗口；panic 安全依赖 unwind；`scheduler.json` 缺失/坏时内嵌线程退出。
- **包**：`manifest.asset_refs`/`dependencies` 未与 flow 闭包交叉校验；`move_dir` copy fallback 合并风险；`PackageError::Other`→400；无跨进程 packages 锁。
- **Runner UI / 打包**：PR7 的 8 条 UI P2；`start-runner.bat` 固定 8789；`README-DEPLOY.md` 示例路径 `D:\OAS-dev-live`（F3，仅文档）；`.gitignore *.tmp` 不匹配 runtime tmp。
- **跨平台**：v1 仅 Windows x64；Android 使用端仅设计预留（`packaging/CROSS-PLATFORM.md`）。

## 6. 连续性最小验证命令（恢复后先跑）

```text
cd D:\OAS-dev\oas-core
git log -1 --format="%h %ci %s"     # 本批提交后以实际为准
git status --porcelain              # 只有用户数据；scripts/docs 应已 clean
cargo test --workspace              # 721/0（oasd 355 / task 111 / flow 62 / runtime 157+2）
node apps\oasd\ui\ui_pure_test.js   # 1559 / 159 / 151 / 0 / 6 nav
# 双端 release（已产出，可复跑仓外 E2E）：
#   powershell -NoProfile -ExecutionPolicy Bypass -File D:\OAS-dev\_verify\release-both-20261105\run-e2e.ps1
```

## 7. 证据

- **双端发行包 / 分离度 E2E**：`D:\OAS-dev\_verify\release-both-20261105\`（`REPORT.md` / `DESIGN-VS-NOW.md` / `F1-FIX.md` / `run-e2e.ps1` / `e2e-summary.json`（75/75）/ `e2e-studio.json` / `e2e-runner.json` / `zip-checks.json` / `package-files.json` / `*-entries.txt` / `studio-8788.png` / `runner-8789.png` / `logs\`）。
- **Lead 独立复核**：`D:\OAS-dev\_verify\lead-recheck-20261105\`（`recheck.ps1` + `recheck-log.txt`：zip/BOM/路由/端口/进程/`scheduler --status` 全 PASS；`D:\OAS-dev\_verify\release-both-20261105\logs\lead-cargo-test-workspace.log` 721/0）。
- **参数 UI 审计**：`D:\OAS-dev\_verify\ui-params-audit-20261105\PARAMS-UI-AUDIT.md`（398 行；结论/缺口/方案 A~E/测试清单）。
- **最终审计 / PR7 / PR1~PR6**：`_verify\runner-studio-final-audit-20261105\`、`_verify\runner-studio-pr7-20261105\`、`_verify\runner-studio-pr1~pr6-20261104\`。
- **现行口径**：`docs/RUNNER-STUDIO-SPLIT-PLAN.md`（§A.9~§A.17）、`docs/ENGINE-DESIGN.md` §22.44~§22.52、`docs/PENDING-LEDGER.md §E-79/§E-80`、`packaging/README-DEPLOY.md`、`packaging/CROSS-PLATFORM.md`。
