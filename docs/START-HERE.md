# 新会话入口（START HERE）★ **只要读这一个文件**

> ★★★ **给用户的一句话**（新会话开场直接粘贴）：
>
> ```
> 先 skill session-memory，然后读 docs/START-HERE.md，严格按它执行，最后按它的模板回报。
> ```
>
> ★ **不需要再说别的** —— 本文件把"读什么、做什么、回报什么"全写死了。

---

# §0 ★ 我是谁 / 这是什么项目（2026-10-24 口径）

```
★ 项目：OAS（OnmyojiAutoScript）—— 阴阳师自动化引擎 + 数据驱动编辑器
★ 当前主线 = 新引擎 **oas-core**（Rust workspace，`D:\OAS-dev\oas-core`，`dev`；发布时同步 `main`；远端 `https://github.com/passioki/oas-core`）
★ 旧仓 `backend\` / `frontend\` = **已冻结，只作资源库**（资产/知识/历史证据）
  ⇒ ★ 不在旧仓上开发；旧仓只读；旧仓文档里的"待办"与引擎无关的不要动
★ 开发工作区：`D:\OAS-dev\`（oas-core + backend + frontend 并排）；oas-core 文档已独立到 `oas-core\docs`
★ 文档通道：oas-core/docs 随 oas-core `dev` 提交；backend 文档仓已冻结，仅历史参考；oas-core `main` + tag/release 已推 GitHub
★ 实机：另一台电脑（本机没有 D:\OAS-live）⇒ 本机只做开发/回放/模拟器验证
★ 唯一真相 = git + 磁盘证据；**不信"我记得"** ⇒ 必须贴 `文件:行号` / commit / 测试输出
```

## 0.1 当前状态快照（2026-11-05 03:3x · **seq 108 · handoff-ready**）

| 项 | 状态 |
|---|---|
| 分支/HEAD | `dev`；**代码 HEAD = `68e5768`**（最终审计 P1 修复提交）；**交接 docs 提交紧随其后（seq 108），最终 HEAD 以 `git log -1` 为准**；PR1~PR7 全部 ✅（PR1 `da9da9d` / PR2 `dfe8776` / PR3 `c09df25`+`048dff5` / PR4 `442f5cb` / PR5 `617435d`+`830058d` / PR6 `edc037b` / PR7 `5e45c81`）；**未 tag / 未 release**；推送状态以 `git status -sb` 实测为准 |
| 引擎 | `oasd`（studio/runner/run/queue/stats/scheduler）+ 编辑器 UI + 素材库；`oasd --version` = **0.1.0**（dev；旧 v1.x 为历史内部 tag） |
| 测试 | `cargo test --workspace` **721 passed / 0 failed**（基线 713，最终审计修复 **+8**；oasd **355** / oas-task **111** / oas-flow **62** / oas-runtime **157 lib + 2 integration**）；OCR build 与 release build exit 0（**5 warnings**，无新增）；UI pure test **1559 checks / Studio 159 ids / Runner 151 ids / 0 dead / 6 nav**（无 UI 改动，数字延续）；`node --check` 3/3；`git diff --check` clean |
| 分包 | `dist/oasd-runner-v0.1.0-win64.zip` = **37,690,904 B**，SHA256 **`A3BCB4B43EFBF8373A74ADDB44DC4EE5A2AD3847C81D1F61CC039229FE15F960`**（Runner-only；BUILD-INFO `git: 68e5768`）；**不入 git** |
| 双入口 | **Studio `http://127.0.0.1:8788/`**（`oasd studio`，编辑/构建/发布，**无调度页/无重置记忆**）· **Runner `http://127.0.0.1:8789/`**（`oasd runner`，nav 6 项：概览/已安装任务/安装包/队列与调度/运行记录/设置；**完整调度 UI + 重置记忆**；启动即内嵌调度，`--no-scheduler` 关闭）；`oasd serve` = Studio 的 deprecated 别名 |
| 当前状态 | **两个服务均已停止（8788/8789 无监听）；交接后电脑关机（Lead 执行）**。启动命令见 §1.3/§6 |
| 发布 | ✅ GitHub Release `v0.1.0` 已上传（release id `431080249`，latest）；tag `v0.1.0` → `e8edbef`（**未移动**）；本地包 `dist/oasd-v0.1.0-win64.zip`，SHA256 `E6112A33…5F5687`（37,089,992 B，**不含 O-1/O-2**） |
| 当前任务 | 只有 `返回庭院`（flow 11 steps / meta 400 B）；`tasks/测试` 已删除 |
| 数据 | 已提交基线：全局素材 **16 条 / 1 图**（15 `ocr_line` + 1 `template`）；帧 **28 个 / `current_id=20` / `next_id=41`**。工作区未提交用户数据：`assets/assets.json`、`frames/frames.json`、`scheduler.json`(+`.bak`)、未跟踪 `frames/frame-43.png`/`frame-44.png`、`tasks/切换预设御魂/`——**用户数据不入库，勿提交** |
| 引擎特性 | flow DSL、素材/掩膜/多边形、点击定位链、HB1/HB7、整帧搜索、单识别 ROI + 操作区域、库、queue、stats、checkpoint/resume、reconnect、多帧投票、OCR+模板互证、统一任务模板/task lint、任务创建/编辑器运行、调度 v3、交付包 `.oaspkg`、Studio/Runner 双入口、内嵌调度/设备租约、子任务参数配置 |
| 已知限制 | 见 `RESUME.md §5` / `SESSION-HANDOFF.md §4`；核心：`installed.json` 损坏无自动隔离、租约 pass 级、install/activate 调度检查有 check-then-act 小窗口、panic 安全依赖 unwind、PR7 8 条 UI P2、`start-runner.bat` 固定 8789、跨平台仅设计预留 |
| 历史交付 | V1.0~V1.2.3 是 reset 前内部交付记录（tag/Release 在 GitHub 历史里）；当前 dev = 0.1.0，恢复节点见 `RELEASE-NODE-2026-11-02.md` |

## 0.2 当前任务台账（2026-11-05）

- **Studio/Runner 分离 + TaskPackage + 任务配置/子任务配置**（`oas-core-runner-studio-split`，seq 108，**handoff-ready**）。
- **Q1~Q7 已批准**（2026-11-04，全部按推荐）；**PR1~PR7 全部 ✅ 已提交**；**最终审计 4 个代码 P1 已修复（`68e5768`）**；交接 docs 紧随其后（seq 108）。
- 台账：`PENDING-LEDGER.md §E-79`（§F PR1 / §G PR2 / §H PR3 / §I PR4 / §J PR5 / §K PR6 / §L PR7 / **§M 最终审计与修复**）；现行口径：`RUNNER-STUDIO-SPLIT-PLAN.md`（§A.9~§A.16）、`ENGINE-DESIGN.md` §22.44~§22.51。
- 下一步 = **产品验收（§D.4 19 步，V1~V7）/ P2 清理 / Phase 5 选择**；不 tag / 不 release。

---

# §1 ★★★ 强制步骤（顺序不许变）

## 1.1 加载 skill（上下文压缩后必做）

```
★ 调用 `skill`，name = `session-memory`
★ 它的五条铁律（常驻区块 / 台账 / 逐步法 / 完成定义 / 双向读取）在压缩后会丢
★ 若技能目录没有 `session-memory`：装 `passioki/dsh-skills`（install.bat），或直接读
  START-HERE + SESSION-HANDOFF + ENGINE-DESIGN 三份磁盘文档
★ 工作目录必须是 `D:\OAS-dev\oas-core`（否则相对路径落空）
```

## 1.2 读这 4 份文档（按顺序）

| 序 | 文件 | 读它为了什么 |
|---|---|---|
| ⓪ | `docs/RESUME.md` + `docs/RESUME-INDEX.md` | ★ **长任务/压缩后恢复的第一入口**（skill 强制 6）：当前断点（seq 108）、验收清单、下一步精确命令、证据索引 |
| ① | `docs/SESSION-HANDOFF.md` | 当前在哪、下一步、最近 3 轮变更、活跃坑（一页纸） |
| ② | `docs/ENGINE-DESIGN.md` **§22** | 引擎规格/进度/判据；§22.34 调度 v3 **已交付**；当前素材/关系模型看 §22.38/§22.39；**PR1~PR7 + 最终审计实现记录看 §22.44~§22.51**（§22.51 = 最终审计与交接） |
| ③ | `docs/PENDING-LEDGER.md` 的 **§E-66~§E-79** | 编辑器 UX/版本重置/任务素材架构/34 事故/静态审计/文档归档/Release+O-1/O-2/调度器产品化/帧页/两按钮+push-all/素材库 UX/**§E-79 = Studio·Runner 分离**（Q1~Q7 已批准、PR1~PR7 全部 ✅、**§M 最终审计与修复**）；§E-24~§E-26 为历史交付台账 |
| ④ | `docs/EDITOR-UX-PLAN.md` + `docs/ASSET-DATA-MODEL.md` + 旧仓路径/流程（仅在碰旧仓时） | 画布/选择/实时帧/区域有效性/排序/分类改版及**实现后状态（§15）**；全局素材/任务素材边界；旧仓 `D:\OAS-dev\backend\docs\LAYOUT.md` · `DEV-WORKFLOW.md`（冻结） |

## 1.3 用代码核实（不许信文档自述）

```bat
REM ① 我的工作区/提交
cd D:\OAS-dev\oas-core
git log -1 --format="%h %ci %s"     REM 期望 68e5768（或其后的 seq 108 docs 提交）
git status --porcelain              REM 期望只有用户数据（assets/frames/tasks/scheduler.json）

REM ② 引擎测试（当前基线 721/0；oasd 355 / oas-task 111 / flow 62 / runtime 157+2）
cargo test --workspace

REM ③ OCR 构建（Studio/Runner/run 需要；不带 feature 会报 "this build has no OCR"）
cargo build -p oasd --features ocr

REM ④ 启动 Studio（编辑端；8788；当前服务已停，交接后关机）
target\debug\oasd.exe studio --config config\live.json

REM ⑤ 启动 Runner（使用端；8789；启动即内嵌调度，--no-scheduler 关闭）
target\debug\oasd.exe runner --config config\live.json

REM ⑥ 旧仓守卫（仅作参考；旧仓已冻结，171 passed 是旧仓基线，不是引擎验收）
cd D:\OAS-dev\backend
run-tests.bat tests/test_comment_drift.py tests/test_task_flow_docs.py tests/module/config/test_standby_placeholder.py ... -q
```

> ★ **双入口不要混**：`oasd studio` 默认 **8788**（编辑端，`http://127.0.0.1:8788/`），`oasd runner` 默认 **8789**（使用端，`http://127.0.0.1:8789/`）；**`oasd serve` 仍可用但已 deprecated**（等价 `studio`，stderr 一行提示）。`scripts\dev-serve.bat` 是旧开发手动入口（8787，未改）；8787 没进程时 Firefox 报 `can't connect` 是端口/书签不对，不是程序崩溃。
> ★ 用户版 Studio 地址是 **http://127.0.0.1:8788/**（`start-studio.bat`，`start-editor.bat` 兼容转调）；Runner 包地址是 **http://127.0.0.1:8789/**（`start-runner.bat`，**固定 `--port 8789`，改 `runner.port` 不生效**）。**终端用户只下载 Runner-only 包 + 任务包，不需要 Studio**。
> ★ [10-31] `oasd serve` 默认在监听成功后**自动打开默认浏览器**；headless/服务用 `--no-open`。Windows 服务在 Session 0 不会自动弹到用户桌面，服务化时应由用户登录任务/launcher 打开。
> ★ `D:\OAS-dev\backend\toolkit` 只是本机外部开发工具包（ADB/OCR 模型）；oas-core 源码、构建、测试、发布不依赖 backend。若要完全脱离旧仓，可把 toolkit/模型复制到 oas-core 自带 `tools/` 或用环境变量覆盖。
> ★ 实机 MuMu serial = **`127.0.0.1:16384`**（不是 `emulator-5554`）；`GET /instance/devices` 列表为空时会自动 `adb connect`。

## 1.4 先回报，再动手

★ 读完、核实完，必须先把 §2 的**引擎版回报模板**发给用户，等确认方向后才改代码。
★ 改动前先出「设计 vs 现状」对照（✅符合 / ❌不符 + `文件:行号`）。

## 1.5 ★★★ 当前设计决策（调度 v3 + 统一任务模板，用户 [10-24] 定稿）

> 详见 **ENGINE-DESIGN §22.34**；这里是入口摘要。

**三层分离（机制 / 契约 / 值）**

| 层 | 内容 | 不许写死游戏数据 |
|---|---|---|
| 统一任务模板 | `tasks/<id>/{meta.json, flow.json}` + 全局 `assets/{assets.json, images/}`；`meta.json` 含 params/entry.requires/exits/capabilities/resources/schedule_hint；flow 统一骨架 `preflight → main → settle → finish(exit)`；`oasd task new --template …` + `task lint` | ✅ 零游戏常量 |
| 调度器 v3 | 周期时钟、护栏、候选过滤、唤醒规划、执行器、状态/报告 | ✅ 零游戏常量 |
| 玩家配置 | `scheduler.json`：重置时间、active_hours、blackouts、entries（顺序/窗口/quota/params）| ✅ 全部玩家自己填 |

**生命周期（用户敲定）**

- `periodic`：时间窗 + 每日/每周 quota 的周期任务（斗技/寮活动/签到等）。
- `temp`：**临时任务**（旧仓 `period=none` 的等价概念）：无窗口/无 quota；不记忆"还剩几次"；每轮按 `entries` 顺序跑一遍；执行前后各重置状态；"完成" = 门票耗尽/达到次数。
- **删除**：`one_shot` 与 `continue`（用户裁定不需要）。
- `rest`（可选，后期）：暂停 N 分钟的条目；v1 不优先。

**重置记忆功能（PR6 起在 Runner）**

- 入口：Runner「队列与调度 → 重置记忆」dialog（多选/全选/反选/全不选 + `include_checkpoint`）+ CLI `oasd scheduler reset-memory`；**Studio 已无重置入口**。
- 清除范围：调度器条目状态 + 任务 `state/`（checkpoint/计数等）；原子写 + 审计事件 `memory_reset`。

**调度铁律**：窗口只回答"能不能跑"；执行顺序只由 `entries` 数组顺序决定；排序是动作不是状态；调度器不内置任何游戏时间/清单。

---

## 1.6 ★★★ 连续运行 / 断点续接（长任务必用）

> 规则全文：skill `session-memory` **强制 6（Continuity Protocol）**；当前任务断点：`docs/RESUME.md`（seq 108）。

| 项 | 约定 |
|---|---|
| 断点卡 | `docs/RESUME.md`（**每次覆盖重写**，≤150 行；含验收清单 + 下一步精确命令）|
| 断点索引 | `docs/RESUME-INDEX.md`（**只追加**；每行 seq/阶段/文档锚点/证据/下一步）|
| 触发 | 每个阶段/验收项完成、测试红→绿、提交前、用户裁定后、开始长命令/子代理批次前、**每轮回复结束前**；无容量信号时再按"10 次工具调用 / 25k 字符 / 5 回合"兜底 |
| 压缩/新会话后 | ★ 先读 RESUME + INDEX 最新行 → 用 `git log/status` + 最小验证命令核实 → 修正差异 → 直接从"下一步精确动作"继续（不重做已完成项）|
| 完成 | 验收清单全 ☑ + 证据可核查 + RESUME 状态 `done` + INDEX 追加终态行 + 台账更新 |
| 阻塞 | 同一阻塞连续 3 轮未解且需外部输入才停；写进 RESUME 阻塞字段再问用户 |

★ 初始化/查看：`python <skill>/scripts/continuity.py init|status`（配置走 `session-memory.config.json`，默认 `docs/RESUME.md`）。

---

# §2 ★★ 回报模板（引擎版，逐项填）

```
📖 已看过 design_spec 文档 · ENGINE §22.x / §22.34 · 当前 commit <hash>

【1. 我已加载】
   skill `session-memory`：✅/❌
   读过的文档：SESSION-HANDOFF ✅ · ENGINE-DESIGN §22 ✅ · PENDING-LEDGER §E-24 ✅

【2. 代码与我核实的台账】
   oas-core commit = <hash> · 分支 = dev · worktree 干净 = 是/否（只有用户数据算"否"）
   cargo test --workspace = __ passed / __ failed（当前基线 = 721；oasd 355 / oas-task 111 / flow 62 / runtime 157+2）
   serve 状态：/state = <captured/device/ocr/det>（Studio 8788 / Runner 8789 双入口；`oasd serve` 为 Studio 兼容别名）
   oas-core docs commit = <hash> · dev 是否已推 = 是/否

【3. 差异（文档说 A、代码是 B）】
   | 文档说 | 代码是 | 差异 |

【4. 下一步（按 ENGINE §22.34 计划 + 我的判断）】
   1. …  2. …  3. …
   ★ 建议先做：… 因为 …

【5. 未完成台账】（见 PENDING-LEDGER §E-79 / §E-78 / §E-77 / §E-76；§E-24~§E-26 为历史；§E-79 = Studio/Runner 分离：Q1~Q7 已批准、**PR1~PR7 全部 ✅ 已提交**、**§M 最终审计 P1 已修（68e5768）**、下一步 = 产品验收 + P2 清理 + Phase 5 选择）

【6. 需要用户裁定的事】

【7. 本轮完成定义（说完成前必须勾完）】
   ☐ …  ☐ …  ☐ …

【8. 连续性（长任务必填）】
   断点：seq=<N> · 阶段=<P0/P1/…> · 状态=in_progress/blocked/done/handoff-ready
   下一步：<可直接执行的命令 / 文件:行号>
   RESUME：docs/RESUME.md（updated_at=…）· 索引：docs/RESUME-INDEX.md
```

---

# §3 ★ 活跃的坑（引擎相关）

| # | 坑 | 一句话 |
|---|---|---|
| **E1** | 构建必须带 `--features ocr` | 否则编辑器识别报 `this build has no OCR`；`/state` 会显示 `ocr:false` |
| **E2** | 重建 `oasd.exe` 前必须先停 8787/8788/8789 | 否则 `failed to remove target\debug\oasd.exe` |
| **E3** | `library/` 已 gitignore，`tasks/*/state` 已 gitignore | 不要把它们提交进去 |
| **E4** | `_verify/` 部分入库 | `_verify/audit-20261101/{A-session-git,B-runtime,C-ui,D-e2e,E-acceptance}.md` 已入库；其余 `_verify/nNN/`、`_verify/runner-studio-*`、`_verify/runner-studio-final-audit-*` 是仓外工作区证据（`D:\OAS-dev\_verify`），不入 git |
| **E5** | 中文文档/代码一律用 `read`/`edit`/`write` 工具 | 不要用 PowerShell `Get-Content|Set-Content` 重写 UTF-8 文本（会 mojibake）|
| **E6** | adb 路径走 `ADB_PATH` | `D:\OAS-dev\backend\toolkit\Lib\site-packages\adbutils\binaries\adb.exe` |
| **E7** | 旧仓已冻结 | 旧仓 `backend/frontend` 只读；不要在旧仓上开发/提交 |
| **E8** | 新引擎的验收看 ENGINE §22 判据 | 不要拿旧仓 pytest 基线当引擎验收 |
| **E9** | headless Edge 截图做 UI 验收 | `msedge --headless=new --screenshot=...`；需要时加 `--user-data-dir` |
| **E10** | 测试数会随提交增长 | 当前 **721/0**（oasd **355** / task 111 / flow 62 / runtime 157+2；UI **1559 checks / 159 Studio ids / 151 Runner ids / 0 dead / 6 nav**）；2026-11-05 **seq 108 = 最终审计 4 个代码 P1 修复 +8 测试 = 721**（713→721；oasd 347→355）；seq 107 = PR7（Rust 零改动 713/0，UI 1559/151/0，包 37,684,830 B）；seq 106 = PR6（713/0、UI 1167/159/0、包 37,653,462 B）；seq 105 = PR5（677/0、UI 1442/237/0、包 37,591,694 B）；seq 104 = PR4（624/0）；seq 103 = PR3（583/0）；seq 102 = PR2（562/0）；seq 101 = PR1（538/0）；以 `cargo test --workspace` 实测为准 |
| **E11** | 本会话 PowerShell 实为 **5.1**，执行策略 Undefined | 直接 `.\scripts\dev-serve.ps1` 报 UnauthorizedAccess ⇒ 用 `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\dev-serve.ps1 -NoOpen` |
| **E12** | `node` 不在 PATH | UI 测试用 bundled node：`C:\Users\passioki\.dsh\dsh-runtimes\dsh-primary-runtime\dependencies\node\bin\node.exe`（v24.21.0）|
| **E13** | PS 5.1 重定向日志是 **UTF-16LE** | 读日志用 `-Encoding Unicode` 或先转 UTF-8，否则中文/换行乱码 |
| **E14** | 实机 adb 为空 → 先确认 MuMu 运行中（serial **`127.0.0.1:16384`**，不是 `emulator-5554`） | 真机任务（`/input/tap`、collection `real=1` E2E）需先启动模拟器；**P5 起 `GET /instance/devices` 在列表为空时会自动 `adb connect`**；仍为空再查 adb server / MuMu 网络设置 |
| **E15** | PR3 配置注入语义（2026-11-04） | 四层优先级 **CLI `--param` > entry params > Runner values > `meta.params.default`**；`oasd run` **不读** values；Studio `POST /task/run` body params 最高且**不注入** values；`meta.params` **非空**才在合并后做 schema 复检；旧配置字符串化数字/布尔不再猜回类型；`--param 'code=\"007\"'` 保持字符串；坏 `values.json` 全局 exit 2；PR6 `/runner/run` 自行合并 values。详见 `RUNNER-STUDIO-SPLIT-PLAN §A.11.7` / `ENGINE §22.46` |
| **E16** | 最终审计修复（2026-11-05 `68e5768`） | replay/dry-run **不取设备租约**（scheduler replay pass 输出 `scheduler_lease_skipped`）；run worker panic 被 catch_unwind 兜住（`outcome=Panic`、running/租约清理）；install/activate 在调度运行或 live lock 下 **409 `scheduler_running`**；`installed.json` 损坏时 `/runner/tasks` 降级 200 + warning、生命周期 API 500 `installed_registry_corrupt`。详见 `ENGINE §22.51` |

---

# §4 ★ 铁律（session-memory skill + 用户明令）

```
★ ① 用户的话 = 规格，不是建议 ⇒ 不许降级成"打补丁"
★ ② 读写双向：不只看用户说了什么，还要看我提了什么选项、用户裁的是哪个
★ ③ 禁止拿"首行预览/截断"当读过 —— 必须读正文
★ ④ 边读边写（防上下文压缩丢失），不攒到最后
★ ⑤ 改代码前先出「设计 vs 现状」对照（✅/❌ + 文件:行号）
★ ⑥ 逐步法：一次只改 1~2 个边界 ⇒ 立刻 test ⇒ 再下一步
★ ⑦ 完成定义：说"完成"前先列勾选清单；没勾完不许说完成
★ ⑧ 台账：每条回复先更新台账文件，并把相关表贴进回复
★ ⑨ 不许阴奉阳违
★ ⑩ 文档不是流水账，是围绕设计/实现/经验教训的有逻辑集合体
```

---

# §5 ★ 每轮结束（强制）

```
★ ① 更新 PENDING-LEDGER（状态变了就改；完成必须有可核查证据）
★ ② 更新 SESSION-HANDOFF（当前状态/下一步/新坑）
★ ③ ★ 更新断点：RESUME.md（覆盖重写）+ RESUME-INDEX.md（追加一行）
★ ④ 有实质进展就 commit：
     - oas-core：提交到 `dev` 并同步 `main`；0.1.0 恢复节点 tag `v0.1.0` 已完成（GitHub Release 已上传）
     - oas-core docs：git add docs session-memory.config.json && git commit && git push origin dev
★ ⑤ 验收证据进 _verify/nNN/，并写进文档判据
```

---

# §6 ★ 常用命令速查（引擎）

```bat
REM ---------- 构建/测试 ----------
cd D:\OAS-dev\oas-core
cargo test --workspace          REM 当前 721/0（oasd 355 / oas-task 111 / flow 62 / runtime 157+2；seq 108 = 最终审计修复 +8）
cargo build -p oasd --features ocr     REM serve/run 的 OCR 构建

REM ---------- Studio / Runner 双入口（当前服务已停止）----------
REM Studio 编辑端（8788；oasd serve = deprecated 别名，等价 studio）
target\debug\oasd.exe studio --config config\live.json
REM Runner 使用端（8789；启动即内嵌调度，--no-scheduler 关闭；安装/配置/队列/调度/临时运行/运行记录全在 Runner）
target\debug\oasd.exe runner --config config\live.json
REM   浏览器：Studio http://127.0.0.1:8788/（编辑器）· Runner http://127.0.0.1:8789/（使用端）
REM   Studio 仍可开 http://127.0.0.1:8788/dashboard.html（点击分布/长跑统计）
REM   旧开发手动入口 scripts\dev-serve.bat 仍为 8787（未改）；发行包脚本 start-studio.bat=8788 / start-runner.bat=8789（固定 --port 8789）

REM ---------- 运行/调度 ----------
target\debug\oasd.exe run --task tasks\返回庭院 --replay <recording-dir>
target\debug\oasd.exe queue --tasks tasks\A,tasks\B --replay <dir> --report report.json
target\debug\oasd.exe scheduler --plan scheduler.json --status
REM   ⚠ 调度计划入口是 --plan <scheduler.json>；--config 是全局 live 配置（main.rs 全局摘走并立即解析），
REM     误传 scheduler.json 会 exit 2（config error: unknown field 'config'），不会跑任何 plan
target\debug\oasd.exe stats --runs runs --html runs\stats.html
REM   ✅ schedule v3 已交付（V1.0）；设计/交付见 ENGINE §22.34/§22.35，Round 语义见 §22.34.7
REM   P4：run 实体在 <runs-dir>\scheduler\（默认 runs\scheduler）；serve 与 scheduler 的 --runs-dir/CWD 必须一致，
REM       否则"调度器写、UI 看不到"；API = GET /runs/scheduler?limit=&task=&outcome= · GET /runs/scheduler/run?run_id=

REM ---------- 旧仓（冻结，仅参考） ----------
cd D:\OAS-dev\backend
run-tests.bat …                                  REM 旧仓守卫基线 171 passed（参考）
```

---

# §7 设计文档索引

| 主题 | 位置 |
|---|---|
| **长任务断点/续接（先读）** | `RESUME.md` · `RESUME-INDEX.md`（skill 强制 6；最新 seq 108） |
| **帧-素材关系** | `../frames/frames.json` + 全局 `assets.json` 的 `positions`；无 interface 文件 |
| **素材数据模型** | `ASSET-DATA-MODEL.md`（全局 `assets/`、任务素材=flow 引用子集、旧 library 仅后端兼容） |
| **编辑器 UX / 选择模型计划** | `EDITOR-UX-PLAN.md`（画布、位置详情、多边形/反选、实时帧、点测/拖测、区域有效性、素材排序、分类重命名；**素材库 UX 现行为 §18**；**§18.10 = 三项遗留修复**） |
| **★ Studio / Runner 分离 + TaskPackage + 任务配置** | `RUNNER-STUDIO-SPLIT-PLAN.md`：**Q1~Q7 已批准；PR1~PR7 全部 ✅ 已提交（PR7 代码 `5e45c81`）+ 最终审计修复 `68e5768`**；§A.9~§A.15 = PR1~PR7 实现记录，**§A.16 = 最终审计与修复记录**；§D.3 PR 计划、§D.4 19 步验收；分发/跨平台见 `packaging/CROSS-PLATFORM.md`、`packaging/README-RUNNER.txt`、`packaging/README-DEPLOY.md` |
| **当前任务（返回庭院）** | `../tasks/返回庭院/flow.json` · 全局素材 `../assets/` · 帧 `../frames/` |
| **引擎规格/进度** | `ENGINE-DESIGN.md` §22（**§22.51 = 最终审计与交接**） |
| **调度 v3 + 统一任务模板（已交付 V1.0）** | `ENGINE-DESIGN.md` **§22.34 / §22.35**；Round 语义裁决见 **§22.34.7** + `SCHEDULER-RUNNER-PLAN.md` |
| **发布/恢复节点（当前）** | `RELEASE-NODE-2026-11-02.md` · Release `https://github.com/passioki/oas-core/releases/tag/v0.1.0` |
| 到实机可用的交付路线（历史） | `archive/DELIVERY-PLAN.md` |
| 当前状态/下一步/坑 | `SESSION-HANDOFF.md` |
| 台账/证据（当前） | `PENDING-LEDGER.md` §E-66~§E-79（**§E-79 = Studio/Runner 分离：Q1~Q7 已批准、PR1~PR7 全部 ✅、§M 最终审计与修复**；§E-76 帧页、§E-77 两按钮+push-all、§E-78 素材库 UX + OCR 阈值） |
| 旧仓历史（冻结，仍在旧仓）| `D:\OAS-dev\backend\docs\SESSION-HANDOFF.md` 历史段 · `D:\OAS-dev\frontend\docs\scheduler-architecture.md`（旧调度设计，参考） |
