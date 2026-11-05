# RESUME — 断点续接卡（session-memory 强制 6）

> 规则：本文件每次断点覆盖重写；≤150 行；另一个会话只读本文件 + `RESUME-INDEX.md` 最新行就能继续。
> 细节：`docs/RUNNER-STUDIO-SPLIT-PLAN.md`（§A.9~§A.17）· `PENDING-LEDGER.md §E-79/§E-80/§E-81` · `ENGINE-DESIGN.md §22.44~§22.52`。

```yaml
task_id: oas-core-dual-app-upgrade
checkpoint_seq: 111
updated_at: 2026-11-05T18:2x+08:00
trigger: 用户 2026-11-05 四项新要求（资产/任务随仓发布；参数与 flow 互动 + 动作库 + 使用端 UI；使用端完全独立新仓；Studio 发布→Runner 通知）→ 设计轮（3 路只读设计 + 可点原型 + 决策单）→ **待用户裁定 D1~D12**
status: awaiting-user-decisions   # 设计已交付，未改代码；裁定后进入 P0 实施
code_head: 68e5768            # 本轮/本批 Rust 零改动（代码 HEAD 仍 = 最终审计修复提交）
docs_head: `b47c94e`（工作区起点；本设计轮 docs 提交后以 `git log -1` 为准）
next_action: ① 用户回复 `_verify\dual-app-design-20261105\DECISION-SHEET.md` 的 D1~D12（未回复按推荐）② 实施 P0：数据入仓+两包带数据 / exchange+publish-index+Runner 通知 ③ 再进参数 v2 后端 → Runner UI → Studio → 新仓拆分
```

## 0. 目标（本阶段）

1. Studio（编辑端，8788）/ Runner（使用端，8789）分离：同一份代码两个 surface。
2. Q1~Q7 已批准；**PR1~PR7 全部 ✅ 已提交**（PR1 `da9da9d` / PR2 `dfe8776` / PR3 `c09df25`+`048dff5` / PR4 `442f5cb` / PR5 `617435d`+`830058d` / PR6 `edc037b` / PR7 `5e45c81`）；最终审计 P1 ✅ `68e5768`。
3. **seq 110（上一轮）= 用户三项要求**：① 分离度检查 + 试发布两端 release；② 更新过时的 `scripts/dev-serve.*`；③ 参数配置在编辑端未被表达 ⇒ 只读审计 + 方案。
4. **seq 111（本轮）= 用户四项新要求**：① 资产/任务属于代码仓库、发布随包；② 参数与 flow 互动/像动作库动作（参数作为 OCR 识别文字、分组+预设、使用端暴露可选用、留空=不用）；③ 使用端完全独立新仓库、两 release 可单独/同时运行并交互；④ Studio 发布新任务→Runner 收到通知。**本轮只设计（3 份设计文档 + 可点原型 + 决策单），未改代码。**

## 1. 状态（2026-11-05 18:2x，seq 111）

| 项 | 值 |
|---|---|
| 分支 / HEAD | `dev`；工作区起点 HEAD = `b47c94e`（seq 110 收尾）；**本设计轮 docs 提交后以 `git log -1` 为准**；未 tag / 未 release |
| 任务状态 | **awaiting-user-decisions**：设计已交付（参数 v2 / 动作库 / 使用端 UI / 双仓拆分 / 发布通知 / 数据随包），**待用户裁定 D1~D12** 后进入 P0 实施 |
| 双入口 | **Studio `http://127.0.0.1:8788/`**（`oasd studio`）· **Runner `http://127.0.0.1:8789/`**（`oasd runner`，nav 6，内嵌调度）；`oasd serve` = Studio deprecated 别名 |
| 测试 | `cargo test --workspace` **721 / 0**（21 targets；本轮 Rust 零改动，Lead 复跑 `EXITCODE=0`）；UI pure test **1559 / Studio 159 / Runner 151 / 0 dead / 6 nav**（UI 零改动）；`git diff --check` clean |
| 发行包 | **Studio** `dist/oasd-v0.1.0-win64.zip` = **37,789,721 B**，SHA256 **`737B52B9D80CB4018A88EB7AA30CC939F1706A1A231C1BC4F1032F11317291E6`**（41 entries；surface=studio；真实 tasks/assets 数据）；**Runner** `dist/oasd-runner-v0.1.0-win64.zip` = **37,690,867 B**，SHA256 **`ECB37A783FB92FD64F78BC788DA6DA3A61CDB7FE6897B76E3C0777E78D339ADE`**（23 entries；surface=runner；空 v4 `scheduler.json` 无 BOM）；**不入 git** |
| 服务 | **全部停止**（8787/8788/8789 实测无监听、无 `oasd` 进程；release 评测后已清理） |
| 数据边界 | 工作区用户数据：`assets/assets.json`、`frames/frames.json`、`scheduler.json(.bak)`、未跟踪 `frames/frame-43.png`/`frame-44.png`、`tasks/切换预设御魂/`；**用户数据不入库** |
| 提交边界 | **待提交 = 本批 scripts（`package.ps1` / `package-runner.ps1` / `dev-serve.ps1` / `dev-serve.bat`）+ docs（START-HERE/RESUME/RESUME-INDEX/SESSION-HANDOFF/PENDING/ENGINE/PLAN/README + `packaging/README-DEPLOY.md`）**；用户数据不提交；`dist/` / `packages/` 不入 git |

## 1.5 2026-11-05 18:2x（seq 111）设计轮产出（只读，未改代码）

> 3 路只读设计（Team：param-architect / split-architect / ui-prototyper）+ Lead 决策单；均写在仓外 `D:\OAS-dev\_verify\dual-app-design-20261105\`。

| 交付 | 内容 | 路径 |
|---|---|---|
| 参数 v2 设计 | 1096 行：设计 vs 现状（参数链路 / 动作库 23 op / **OCR 文字硬缺口：`Locator::OcrText.text` 等都是 String 字面量**（`oas-flow:652-656/712-726/787-791`）/ 分组预设 / 4 层优先级不变）；ParamDecl v2（`role` 可选、`group` 首次消费）；动作库 A1~A7 + `text_expr` DSL + T3 扩扫 + flow_ops 门禁；Runner UI 线框/状态机/API v2；Studio 面板/lint；7 个 PR（~13 人日）；14 项待裁定 | `PARAM-V2-DESIGN.md`（仓外） |
| 双端拆分设计 | 926 行：**推荐方案 A 两仓**（oas-core / 新仓 `oas-runner`，共享 crates 用 git 依赖 tag 锁定）；Phase 0 现仓可做（exchange+notify / 两包数据 / 首启不覆盖导入 / serve.rs move-only / 版本 tag）→ Phase 1 开仓搬 Runner；通知推荐 **C：共享 exchange 目录为唯一真相 + loopback POST 即时加速**（`event_id=package_id` 幂等 + 离线矩阵）；发行包数据边界；共 14~19 人日；4 项待裁定 | `RUNNER-SPLIT-DESIGN.md`（仓外） |
| UI 原型 | 可点静态页：`runner-params.html`（独立参数页：预设下拉/管理、按 group 分节、每参使用勾选/留空=不用、5 态）、`studio-params.html`（构建页动作库「任务参数」组 + 参数面板 + 插入参数 + T3 检查/一键声明）；4 张 1440×900 截图 | `prototype\`（仓外） |
| 决策单 | D1~D12 逐条：选项 / 推荐 / 影响（不施行则按推荐） | `DECISION-SHEET.md`（仓外） |

**本轮结论**：① 数据随仓/随包=设计已定（两包带数据 + Runner `bundled-data` 首启不覆盖）；② 参数不是 flow 步骤而是任务级契约 → 动作库加**消费类动作**（参数文字点击/等待/判断/拖动）+ 表达式插入参数 + `text_expr` 支持「参数作为 OCR 识别文字」；③ 预设 = Runner 侧命名值集合（values.json additive，四层优先级不变）；④ 拆仓=方案 A Phase 0 先行；⑤ 发布通知=exchange 文件为真相 + loopback POST 加速。**未改任何代码/UI；裁定后从 P0 开始实施。**

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

1. **用户裁定** `DECISION-SHEET.md` D1~D12（未回复按推荐）。
2. **P0 实施（裁定后先做）**：① 数据入仓 + 两包带数据（`package.ps1`/`package-runner.ps1`，Runner `bundled-data` 首启不覆盖）；② exchange 目录 + `publish-index.json` + Studio 发布写入 + Runner 轮询/徽标/一键安装（= 用户第 4 条，可独立演示）。
3. **P1+**：参数 v2 后端（`text_expr` + T3/flow_ops + values 预设 API）→ Runner 参数页 → Studio 面板/动作库 → `serve.rs` move-only + 新仓 `oas-runner`。
4. 旧待办不变：产品验收 `§D.4` 19 步 / P2 清理 / Phase 5；**不 tag / 不 release**（除非用户明确说发布）。

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

- **本设计轮（seq 111）**：`D:\OAS-dev\_verify\dual-app-design-20261105\`（`PARAM-V2-DESIGN.md` / `RUNNER-SPLIT-DESIGN.md` / `DECISION-SHEET.md` / `prototype\`（2 HTML + 4 PNG + `DESIGN-NOTES.md`））。
- **双端发行包 / 分离度 E2E**：`D:\OAS-dev\_verify\release-both-20261105\`（`REPORT.md` / `DESIGN-VS-NOW.md` / `F1-FIX.md` / `run-e2e.ps1` / `e2e-summary.json`（75/75）/ `e2e-studio.json` / `e2e-runner.json` / `zip-checks.json` / `package-files.json` / `*-entries.txt` / `studio-8788.png` / `runner-8789.png` / `logs\`）。
- **Lead 独立复核**：`D:\OAS-dev\_verify\lead-recheck-20261105\`（`recheck.ps1` + `recheck-log.txt`：zip/BOM/路由/端口/进程/`scheduler --status` 全 PASS；`D:\OAS-dev\_verify\release-both-20261105\logs\lead-cargo-test-workspace.log` 721/0）。
- **参数 UI 审计**：`D:\OAS-dev\_verify\ui-params-audit-20261105\PARAMS-UI-AUDIT.md`（398 行；结论/缺口/方案 A~E/测试清单）。
- **最终审计 / PR7 / PR1~PR6**：`_verify\runner-studio-final-audit-20261105\`、`_verify\runner-studio-pr7-20261105\`、`_verify\runner-studio-pr1~pr6-20261104\`。
- **现行口径**：`docs/RUNNER-STUDIO-SPLIT-PLAN.md`（§A.9~§A.17）、`docs/ENGINE-DESIGN.md` §22.44~§22.52、`docs/PENDING-LEDGER.md §E-79/§E-80`、`packaging/README-DEPLOY.md`、`packaging/CROSS-PLATFORM.md`。
