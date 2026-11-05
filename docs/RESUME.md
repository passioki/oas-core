# RESUME — 断点续接卡（session-memory 强制 6）

> 规则：本文件每次断点覆盖重写；≤150 行；另一个会话只读本文件 + `RESUME-INDEX.md` 最新行就能继续。
> 细节：`docs/RUNNER-STUDIO-SPLIT-PLAN.md`（§A.9~§A.17）· `PENDING-LEDGER.md §E-79/§E-80/§E-81` · `ENGINE-DESIGN.md §22.44~§22.52`。

```yaml
task_id: oas-core-dual-app-upgrade
checkpoint_seq: 113
updated_at: 2026-11-05T19:1x+08:00
trigger: seq 112 P0 首批落地 → **seq 113 参数 v2 后端 + 构建端参数 UI/动作库 + P0-1b（notify 客户端/compatible 预检）**→ 用户澄清 D2（预设只是举例，不做预设值集合）与 D8（通知通道=发布后可被检测，远程网页/GitHub 或本地安装）→ 测试/E2E 全绿
status: in_progress               # P1 参数后端 ✅ / P3 构建端 UI ✅ / P0-1b ✅；Runner 参数 UI、远程更新 feed、新仓拆分待继续
code_head: 本批提交后以 `git log -1` 为准（父 `a123182`）
docs_head: 本批 docs 随代码提交（以 `git log -1` 为准；dev = main = origin）
next_action: ① P2 Runner 参数 UI（group 分组/使用开关/留空=不注入；**不做预设管理**）；② D8 远程更新 feed（本地路径/URL 拉取，GitHub/HTTP 下载策略待定）；③ Studio 权威 lint `GET /task/lint` + expects/drag 结构化编辑；④ 新仓 `oas-runner`（D7，需用户建 GitHub 空仓）；⑤ 产品验收/P2/Phase 5
```

## 0. 目标（本阶段）

1. Studio（编辑端，8788）/ Runner（使用端，8789）分离：同一份代码两个 surface。
2. Q1~Q7 已批准；**PR1~PR7 全部 ✅ 已提交**（PR1 `da9da9d` / PR2 `dfe8776` / PR3 `c09df25`+`048dff5` / PR4 `442f5cb` / PR5 `617435d`+`830058d` / PR6 `edc037b` / PR7 `5e45c81`）；最终审计 P1 ✅ `68e5768`。
3. **seq 110（上一轮）= 用户三项要求**：① 分离度检查 + 试发布两端 release；② 更新过时的 `scripts/dev-serve.*`；③ 参数配置在编辑端未被表达 ⇒ 只读审计 + 方案。
4. **seq 111（设计轮）= 用户四项新要求**：① 资产/任务属于代码仓库、发布随包；② 参数与 flow 互动/像动作库动作（参数作为 OCR 识别文字、分组+预设、使用端暴露可选用、留空=不用）；③ 使用端完全独立新仓库、两 release 可单独/同时运行并交互；④ Studio 发布新任务→Runner 收到通知。
5. **seq 112（已实现）**：数据随包（Studio 全数据 / Runner `bundled-data` + 首启导入）+ exchange 发布通知 + UI。
6. **seq 113（本轮）**：① 参数 v2 后端（`text_expr` 参数作为 OCR 文字 + T3/T4/flow_ops 门禁 + `ParamDecl.role`）；② 构建端参数 UI/动作库（参数文字点击/等待、定位器文字来源、构建页参数面板、插入参数）；③ P0-1b（event engine 字段 + `compatible` 预检 + Studio→Runner HTTP notify 客户端）。用户澄清：**D2 预设只是举例（不做预设值集合）、D8 通知=发布后可被检测（远程网页/GitHub 或本地）**。

## 1. 状态（2026-11-05 19:1x，seq 113）

| 项 | 值 |
|---|---|
| 分支 / HEAD | `dev`；本批提交后以 `git log -1` 为准（父 `a123182`）；未 tag / 未 release |
| 任务状态 | **in_progress**：P1 参数后端 ✅、P3 构建端参数 UI/动作库 ✅、P0-1b ✅；Runner 参数 UI（P2）、远程更新 feed（D8）、新仓拆分待继续 |
| 双入口 | **Studio `http://127.0.0.1:8788/`**（`oasd studio`）· **Runner `http://127.0.0.1:8789/`**（`oasd runner`，nav 6，内嵌调度）；`oasd serve` = Studio deprecated 别名 |
| 测试 | `cargo test --workspace` **774 / 0**（21 suites；oas-flow **70** / oas-runtime **167 lib**+2 / oas-task **116** / oasd **385**）；UI pure test **1727 checks / Studio 159 / Runner 157 / 0 dead / 6 nav**；debug+release OCR build exit 0（5 warnings 无新增）；`git diff --check` clean |
| 发行包 | **Studio** `dist/oasd-v0.1.0-win64.zip` = **76,611,083 B**，SHA256 **`62B0692EC8D2794C1BDD39F1C8C14533F0A90102C634F333EC1ADE2D786FCD33`**；**Runner** `dist/oasd-runner-v0.1.0-win64.zip` = **37,812,123 B**，SHA256 **`8321818C47EF2FDE12C95DA8126BE0353413D48CBA7AF1BB0AE3BAB7A1863CF5`**（含参数新 UI/后端；不入 git） |
| 服务 | **全部停止**（8787/8788/8789 实测无监听、无 `oasd` 进程；两轮 E2E 后已清理） |
| 数据边界 | 数据已入库（seq 112）；`scheduler.json` 仍为本机运行配置**未提交**；`library/`/`dist/`/`packages/` 不入库 |
| 提交边界 | 本批 = `crates/*` 5 文件 + `apps/oasd/src/{exchange,serve}.rs` + `apps/oasd/ui` 5 文件 + docs；`dist/`/`packages/` 不入 git；不 tag / 不 release |

## 1.5 2026-11-05 18:2x（seq 111）设计轮产出（只读，未改代码）

> 3 路只读设计（Team：param-architect / split-architect / ui-prototyper）+ Lead 决策单；均写在仓外 `D:\OAS-dev\_verify\dual-app-design-20261105\`。

| 交付 | 内容 | 路径 |
|---|---|---|
| 参数 v2 设计 | 1096 行：设计 vs 现状（参数链路 / 动作库 23 op / **OCR 文字硬缺口：`Locator::OcrText.text` 等都是 String 字面量**（`oas-flow:652-656/712-726/787-791`）/ 分组预设 / 4 层优先级不变）；ParamDecl v2（`role` 可选、`group` 首次消费）；动作库 A1~A7 + `text_expr` DSL + T3 扩扫 + flow_ops 门禁；Runner UI 线框/状态机/API v2；Studio 面板/lint；7 个 PR（~13 人日）；14 项待裁定 | `PARAM-V2-DESIGN.md`（仓外） |
| 双端拆分设计 | 926 行：**推荐方案 A 两仓**（oas-core / 新仓 `oas-runner`，共享 crates 用 git 依赖 tag 锁定）；Phase 0 现仓可做（exchange+notify / 两包数据 / 首启不覆盖导入 / serve.rs move-only / 版本 tag）→ Phase 1 开仓搬 Runner；通知推荐 **C：共享 exchange 目录为唯一真相 + loopback POST 即时加速**（`event_id=package_id` 幂等 + 离线矩阵）；发行包数据边界；共 14~19 人日；4 项待裁定 | `RUNNER-SPLIT-DESIGN.md`（仓外） |
| UI 原型 | 可点静态页：`runner-params.html`（独立参数页：预设下拉/管理、按 group 分节、每参使用勾选/留空=不用、5 态）、`studio-params.html`（构建页动作库「任务参数」组 + 参数面板 + 插入参数 + T3 检查/一键声明）；4 张 1440×900 截图 | `prototype\`（仓外） |
| 决策单 | D1~D12 逐条：选项 / 推荐 / 影响（不施行则按推荐） | `DECISION-SHEET.md`（仓外） |

**本轮结论**：① 数据随仓/随包=设计已定（两包带数据 + Runner `bundled-data` 首启不覆盖）；② 参数不是 flow 步骤而是任务级契约 → 动作库加**消费类动作**（参数文字点击/等待/判断/拖动）+ 表达式插入参数 + `text_expr` 支持「参数作为 OCR 识别文字」；③ 预设 = Runner 侧命名值集合（values.json additive，四层优先级不变）；④ 拆仓=方案 A Phase 0 先行；⑤ 发布通知=exchange 文件为真相 + loopback POST 加速。**未改任何代码/UI；裁定后从 P0 开始实施。**

## 1.6 2026-11-05 18:1x→18:4x（seq 112）P0 首批落地（已实现）

> 用户尚未回复 D1~D12；按决策单「未回复按推荐」先落地 P0 中**不依赖参数设计裁定**的两项（数据随包/发布通知）+ 首启导入。设计依据 `RUNNER-SPLIT-DESIGN §5/§6`；实现记录 `ENGINE §22.53`；台账 `PENDING §E-82`。

| # | 交付 | 结果 |
|---|---|---|
| P0-2a | Studio 包带全数据 | `package.ps1` 新增 frames（frames.json+PNG，剔 `.backup/`）/relations/templates + `data:` BUILD-INFO + stage 审计；Studio 包 76,582,684 B（78 entries） |
| P0-2b | Runner 包带 `bundled-data/` | `package-runner.ps1` 生成 `{manifest.json,tasks,assets}`（BOM-free；逐文件 sha；`-NoData` 可精简）；Runner 包 37,793,723 B |
| P0-3 | Runner 首启导入 | 新 `bundled_data.rs`：manifest sha 校验 → 空数据根复制 → 写 `packages/bundled-import.json`；**永不覆盖**；幂等；失败回滚；`main.rs` Runner 启动前调用 |
| P0-1a | exchange 发布通知（后端+UI） | 新 `exchange.rs` + `config.rs ExchangeConfig`（默认关闭）+ `serve.rs` 3 路由 + `/task/publish` exchange 响应；Runner nav 徽标/「新发布（来自 Studio）」列表（安装→ack/忽略）；Studio 发布状态提示 |
| 验证 | 测试 + E2E | `cargo test --workspace` **746/0**、UI **1633/159/157/0/6**；两 release 仓外 E2E：Studio publish → exchange → Studio 停 → Runner 启动导入 → updates new → install → ack installed **ALL PASS** |

**本批未做**：HTTP notify 客户端/`.studio-writer.lock`/`compatible` 预检（最小集外）；参数 v2；新仓 `oas-runner`；`scheduler.json` 运行态化。

## 1.7 2026-11-05 18:5x→19:1x（seq 113）参数 v2 后端 + 构建端参数 UI + P0-1b（已实现）

> 用户澄清（19:12）：**D2「预设」只是举例**（不做预设值集合）；**D8 通知 = 发布后可被检测（远程网页/GitHub 或本地安装）**。设计 `PARAM-V2-DESIGN` §2.1/§3.2/§3.6/§4/§5；实现 `ENGINE §22.54`；台账 `PENDING §E-83`。

| # | 交付 | 结果 / 证据 |
|---|---|---|
| P1 后端 | **`text_expr`：参数作为 OCR 文字** | `TextSpec{Literal,Expr}` 替换 4 处（click_target.find sel/inline、requires/expect、drag from/to）；`text`/`text_expr` 互斥必填其一；运行时 step 进入一次求值、必须非空字符串、失败不点击/不 fallthrough；expect 轮询不重算；T3/T4 覆盖；**`flow_ops` 4→7**（`click_target.text_expr`/`expect.text_expr`/`drag.text_expr`）安装门禁；`ParamDecl.role` 可选。测试：flow 70 / runtime 167+2 / task 116 |
| P3 UI | **构建端参数动作库/面板** | 动作库「参数」组（参数文字点击/等待参数文字，生成标准 flow）；`click_target` 定位器「文字来源」三态（text / text_expr=param / 表达式，保留未知字段）；构建页 `#builder-params` 面板（GET /task/config、配置项…复用+保存回刷、`#builder-param-issues` 近似提示）；表达式「插入参数 ▾」；保存前 text/text_expr 互斥校验。UI **1727 checks**（+94） |
| P0-1b | **发布通知增强** | exchange event 增 `min_engine`/`flow_ops`；Runner `compatible` 真实预检；Studio→Runner loopback `POST /runner/notify`（纯 std TCP，best-effort，响应 `exchange.notify`）。测试：oasd 385 / exchange 8 项新增 |
| E2E | 参数链路 + P0 复跑 | 临时任务 `paramtest`：lint 0 errors → publish → manifest `flow_ops=click_target.text_expr`、`params_schema.own[0].role=ocr_text`（debug+release 均过）；两 release 新包 P0 双端 E2E **ALL PASS**（`_verify\p1-param-e2e-20261105\`、`_verify\p0-1a-e2e-20261105\`） |

**本批未做**：Runner 使用端参数 UI（P2，group 分组/使用开关/留空提示；按 D2 澄清**不做预设管理**）；远程更新 feed（D8 澄清：本地 exchange 已有，远程网页/GitHub 拉取+下载安装置下一步）；Studio 权威 `GET /task/lint` + expects/drag 结构化编辑；新仓拆分。

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

1. **P2 Runner 参数 UI（下一优先）**：`meta.params` 按 `group` 分节、每参「使用」开关、bool 三态、留空=不注入（回退 default）、必填阻止保存；**按 D2 澄清不做预设值集合管理**。
2. **D8 远程更新 feed**：Runner `updates.feed` 源列表 = 本地路径/HTTP URL；本地 exchange 已通；远程（GitHub raw/Release）拉取 index + 下载 `.oaspkg` 到本地再安装（HTTPS 策略待定：外部 `curl` vs 引入 TLS 依赖）。
3. **Studio 权威 lint / 补齐**：`GET /task/lint` + 构建页「检查」按钮；`requires/expect/drag` 的 `text_expr` 结构化编辑；结构化参数声明表单。
4. **P0-1b 剩余**：`.studio-writer.lock`、重试队列；**新仓 `oas-runner`**（D7，需用户建 GitHub 空仓）+ `serve.rs` move-only + tag。
5. 旧待办不变：产品验收 `§D.4` 19 步 / P2 清理 / Phase 5；**不 tag / 不 release**（除非用户明确说发布）。

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
- **参数 UI 缺口（P2，待实现）**：Runner 使用端尚未按 `group` 分节、无每参「使用」开关/bool 三态（当前空值已被 collect 跳过 = 不注入，但缺少显式交互与必填阻止提示）；Studio `requires/expect/drag` 的 text_expr 仍走高级 JSON；无权威 `GET /task/lint`（`#builder-param-issues` 为前端近似）；**不做预设值集合管理**（用户 D2 澄清）。
- **更新检测（D8）**：目前只有本地 exchange 目录（同一目录共享）与本地路径安装；**远程网页/GitHub 拉取未实现**（下一步）。
- **exchange/通知**：默认关闭（需两端 live.json 配同一目录）；`.studio-writer.lock` 未做（并发 publish 有 RMW 窗口）；notify 为 best-effort（文件索引唯一真相）。
- **bundled 导入为简化版**：仅空数据根首启；已有数据时一律 skip（不 merge）；不注册 `installed.json`；素材冲突不处理。
- **数据入库**：本批把 `assets/tasks/frames` 当前数据提交（用户要求 ①）；`scheduler.json` 仍是本机运行配置未提交（设计 §6.6 拟运行态化）。
- **参数 UI 缺口（待实现）**：任务构建页无参数入口/提示；唯一入口 = 任务库「配置项…」JSON 对话框；`save_flow` 不跑 lint；Runner `group` 未渲染、数字 enum 非下拉（方案见 `PARAM-V2-DESIGN`）。
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
cargo test --workspace              # 774/0（oasd 385 / task 116 / flow 70 / runtime 167+2）
node apps\oasd\ui\ui_pure_test.js   # 1727 / 159 / 157 / 0 / 6 nav
# P0 双端交互 / 参数链路复跑：
#   powershell -NoProfile -ExecutionPolicy Bypass -File D:\OAS-dev\_verify\p0-1a-e2e-20261105\e2e.ps1
#   powershell -NoProfile -ExecutionPolicy Bypass -File D:\OAS-dev\_verify\p1-param-e2e-20261105\e2e.ps1
```

## 7. 证据

- **参数 v2 后端 / P3 UI / P0-1b（seq 113）**：`D:\OAS-dev\_verify\param-v2-backend-20261105\`（workspace 774/0、oasd 385/0、build exit 0/5 warnings、release-build、clean-copy 769/0）· `D:\OAS-dev\_verify\p1-param-e2e-20261105\`（`e2e.ps1`/`e2e-log.txt` ALL PASS；manifest flow_ops/role 证据）· `D:\OAS-dev\_verify\p0-3-studio-params-20261105\`（3 张构建端 UI 截图）· `D:\OAS-dev\_verify\p0-1a-e2e-20261105\`（P0 复跑 ALL PASS）。
- **P0 首批落地（seq 112）**：`D:\OAS-dev\_verify\p0-1a-e2e-20261105\`（`e2e.ps1` + `e2e-log.txt` ALL PASS；两包 SHA 记录）· 设计 `_verify\dual-app-design-20261105\RUNNER-SPLIT-DESIGN.md §5/§6`。
- **本设计轮（seq 111）**：`D:\OAS-dev\_verify\dual-app-design-20261105\`（`PARAM-V2-DESIGN.md` / `RUNNER-SPLIT-DESIGN.md` / `DECISION-SHEET.md` / `prototype\`（2 HTML + 4 PNG + `DESIGN-NOTES.md`））。
- **双端发行包 / 分离度 E2E**：`D:\OAS-dev\_verify\release-both-20261105\`（`REPORT.md` / `DESIGN-VS-NOW.md` / `F1-FIX.md` / `run-e2e.ps1` / `e2e-summary.json`（75/75）/ `e2e-studio.json` / `e2e-runner.json` / `zip-checks.json` / `package-files.json` / `*-entries.txt` / `studio-8788.png` / `runner-8789.png` / `logs\`）。
- **Lead 独立复核**：`D:\OAS-dev\_verify\lead-recheck-20261105\`（`recheck.ps1` + `recheck-log.txt`：zip/BOM/路由/端口/进程/`scheduler --status` 全 PASS；`D:\OAS-dev\_verify\release-both-20261105\logs\lead-cargo-test-workspace.log` 721/0）。
- **参数 UI 审计**：`D:\OAS-dev\_verify\ui-params-audit-20261105\PARAMS-UI-AUDIT.md`（398 行；结论/缺口/方案 A~E/测试清单）。
- **最终审计 / PR7 / PR1~PR6**：`_verify\runner-studio-final-audit-20261105\`、`_verify\runner-studio-pr7-20261105\`、`_verify\runner-studio-pr1~pr6-20261104\`。
- **现行口径**：`docs/RUNNER-STUDIO-SPLIT-PLAN.md`（§A.9~§A.17）、`docs/ENGINE-DESIGN.md` §22.44~§22.52、`docs/PENDING-LEDGER.md §E-79/§E-80`、`packaging/README-DEPLOY.md`、`packaging/CROSS-PLATFORM.md`。
