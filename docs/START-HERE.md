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
★ 当前主线 = 新引擎 **oas-core**（Rust workspace，`D:\OAS-dev\oas-core`，`main`，远端 `https://github.com/passioki/oas-core`）
★ 旧仓 `backend\` / `frontend\` = **已冻结，只作资源库**（资产/知识/历史证据）
  ⇒ ★ 不在旧仓上开发；旧仓只读；旧仓文档里的"待办"与引擎无关的不要动
★ 开发工作区：`D:\OAS-dev\`（oas-core + backend + frontend 并排）；oas-core 文档已独立到 `oas-core\docs`
★ 文档通道：oas-core/docs 随 oas-core `dev` 提交；backend 文档仓已冻结，仅历史参考；oas-core `main` + tag/release 已推 GitHub
★ 实机：另一台电脑（本机没有 D:\OAS-live）⇒ 本机只做开发/回放/模拟器验证
★ 唯一真相 = git + 磁盘证据；**不信"我记得"** ⇒ 必须贴 `文件:行号` / commit / 测试输出
```

## 0.1 当前状态快照（2026-11-01 · V1.2.3 已发布）

| 项 | 状态 |
|---|---|
| 引擎 | `oasd`（serve/run/queue/stats/scheduler）+ 编辑器 UI + 素材库；`oasd --version` = **1.2.3** |
| 测试 | `cargo test --workspace` = **352 passed / 0 failed**（legacy shipped 数据测试已改为临时 fixture，不再依赖 `tasks/` 现状） |
| 当前任务 | `tasks/返回庭院`（OCR + 返回按钮 + 弹窗取消 + 重启兜底）；素材 1 个 `预设` OCR；其他任务已由用户清理 |
| 引擎特性 | flow DSL、素材/掩膜/多边形、点击定位链、HB1/HB7、整帧搜索、**单识别 ROI + 操作区域**、库、queue、stats、checkpoint/resume、reconnect、多帧投票、OCR+模板互证、**统一任务模板/task lint、任务创建/编辑器运行、调度 v3、交付包** |
| 编辑器 | 工作台（素材/图片/任务/点击分布/调度）+ 顶部页面（任务构建/任务库/实例/调度/设置）、标题行常驻结果/日志窗口、新建任务对话框、重置记忆多选、调试页 `/dashboard.html`；流程编辑统一在任务构建 |
| 纵向 | ① 弹窗 ② 重试/恢复 ③ queue ④ 流程编辑器 ⑤ 统计 ⑥ 周期调度（v1 原型）· **V1.0 已交付**（模板+创建+调度 v3+实机包）|
| 横向 | H1/H2 暂停；H4/H5 已完成；H3 暂缓 |
| V1.0 交付 | ★ 提交 `678e915` · Release `v1.0.0` · 探索/个人突破实机 Done · 调度 2 轮实机通过 · 长跑（96 轮等效）通过 |
| V1.1 交付 | ★ tag `v1.1.0` / 提交 `f2c096c` · Release `https://github.com/passioki/oas-core/releases/tag/v1.1.0` · 整屏 OCR 直点/素材 CRUD/区域拖动/工作台完成条件/5 页面/点击聚合 · 314 tests 全绿 · zip SHA256 `140B…` |
| V1.2 交付 | ★ tag `v1.2.0` / 提交 `5d0f79c` · Release `https://github.com/passioki/oas-core/releases/tag/v1.2.0` · 识别/操作区域统一、批量保存+全局唯一、任务构建页、变量动作 · 326 tests 全绿 · zip SHA256 `D31C…` |
| V1.2.1 交付 | ★ tag `v1.2.1` / 提交 `4e50aea` · Release `https://github.com/passioki/oas-core/releases/tag/v1.2.1` · 统一全局素材库迁移、OCR 来源一致、builder 真流程图、帧管理、工作台流程收口 · 327 tests 全绿 · zip SHA256 `E4E4…` |
| V1.2.2 交付 | ★ tag `v1.2.2` / 提交 `9debc0b` · Release `https://github.com/passioki/oas-core/releases/tag/v1.2.2` · 帧持久化、图片匹配操作区域、builder 结构化/变量 checkpoint、legacy 清理、工作台 flow 删除 · 330 tests 全绿 · zip SHA256 `B683…` · ✅ **R1~R21 台账已全部关闭**（R9 `3aa1a0a` · R21 `d455f6e`） |
| **V1.2.3 交付** | ★ tag `v1.2.3` / 提交 `385d611` · Release `https://github.com/passioki/oas-core/releases/tag/v1.2.3` · 单识别 ROI（旧 source union 迁移）、实时帧/ScreenCache、停止运行、实时步骤日志、撤销/重做、详细 OCR 日志 · **350 tests** · zip SHA256 `F135922D…C12E` |
> ✅ **v1.2.3 验收 U-01~U-34 主体完成**：U-01 A/B（`1ee41d0`）· U-02~U-04（`cbfbab4`）· U-05~U-10（`4480d05`）· U-11~U-21（`e01d572`；U-20 多选整组拖动已由 `e73b408` 补齐）· U-22~U-27（`346adba`）· U-28/U-29（`00ac85c`/`460ad32`）· U-30~U-32（`af66179`）· U-33 逻辑运算审计（`cdd9dde`）· U-34 素材条件逻辑（`c2de9c6`）
> ✅ **U-35~U-56（v1.2.3 开发）**：U-35 run OCR provider · U-39~U-43 UI/OCR/一致性 · U-44~U-46 实时帧/WS/识别区域 · U-47~U-50 简繁容忍/单 ROI/详细日志 · U-51~U-53 停止运行/嵌套 flow/撤销重做 · U-54 缓存帧审计 · U-55 ScreenCache 截图抽象（`e3721e6`）· U-56 近一周审计 + ROI 迁移 union（`d360d05`）· 当前 **350 tests**
> ★ 发布规则：**不自行 tag/release**；只在用户明确说“发布”时发布；平时只构建 release 测试。  
> ★ [11-01] 用户已明确要求发布 v1.2.3；main/tag/Release 已推送，作为恢复节点。

## 0.2 当前开发工作区（2026-11-01）

| 项 | 值 |
|---|---|
| 恢复节点 | `main` = `385d611`（v1.2.3） |
| 当前分支 | `dev`（文档/编辑器/任务数据提交；不自动合并 main） |
| 文档 | 已独立到 `oas-core/docs`；当前设计以 ENGINE-DESIGN + PENDING-LEDGER + RESUME 为准 |
| 截图 | 两阶段：点击立即抓取 → 弹窗预览 → 名称/分类 → 保存；无当前帧跳转 |
| Enter | 输入框 Enter = 确认输入；弹窗 Enter = 保存截图 |
| 实时操作 | 实时开启时画布单击/拖拽 = tap/swipe；实时暂停 = ROI 编辑 |
| OCR 缩略图 | 只认 `positions[0]`，无位置帧显示占位；首次保存自动用当前帧 |
| 帧-素材关系 | 唯一关系 = `Asset.positions`；帧行显示关联素材数；素材位置面板显示帧；**无 interface 概念** |
| 新 DSL | `key`、`restart_app` 已实现，前端 builder/flow_model 已补齐 |
| 帧归档 | 已补庭院/设置/式神录/探索/町中/御魂/斗技/结界突破/逢魔/悬赏/宠物小屋/退出弹窗/加载/启动参考帧 |
| 待办 | 活动页参考帧继续补；帧-素材关系派生视图继续完善；不引入 interface/ui-map |

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
| ⓪ | `docs/RESUME.md` + `docs/RESUME-INDEX.md` | ★ **长任务/压缩后恢复的第一入口**（skill 强制 6）：当前断点、验收清单、下一步精确命令、证据索引 |
| ① | `docs/SESSION-HANDOFF.md` | 当前在哪、下一步、最近提交、活跃坑 |
| ② | `docs/ENGINE-DESIGN.md` **§22** | 引擎规格/进度/判据；**调度 v3 设计看 §22.34** |
| ③ | `docs/PENDING-LEDGER.md` 的 §E-24/§E-25/§E-26 段 + 最新 §E-60 | 引擎/版本/用户裁定（R1~R21）台账 + interface 回退与帧-素材关系 |
| ④ | 旧仓路径/流程（仅在碰旧仓时） | `D:\OAS-dev\backend\docs\LAYOUT.md` · `DEV-WORKFLOW.md`（冻结） |

## 1.3 用代码核实（不许信文档自述）

```bat
REM ① 我的工作区/提交
cd D:\OAS-dev\oas-core
git log -1 --format="%h %ci %s"
git status --porcelain

REM ② 引擎测试（当前基线：352 passed / 0 failed）
cargo test --workspace

REM ③ OCR 构建（serve/run 需要；不带 feature 会报 "this build has no OCR"）
cargo build -p oasd --features ocr

REM ④ 启动编辑器（根目录 = oas-core；先确保 8787 没被占用）
set ADB_PATH=D:\OAS-dev\backend\toolkit\Lib\site-packages\adbutils\binaries\adb.exe
target\debug\oasd.exe serve --port 8787 --tasks-dir tasks --runs-dir runs --library-dir library ^
  --model D:\OAS-dev\backend\toolkit\Lib\site-packages\rapidocr\models\PP-OCRv6_rec_small.onnx ^
  --det-model D:\OAS-dev\backend\toolkit\Lib\site-packages\rapidocr\models\PP-OCRv6_det_small.onnx

REM ⑤ 旧仓守卫（仅作参考；旧仓已冻结，171 passed 是旧仓基线，不是引擎验收）
cd D:\OAS-dev\backend
run-tests.bat tests/test_comment_drift.py tests/test_task_flow_docs.py tests/module/config/test_standby_placeholder.py ... -q
```

> ★ **端口不要混**：上面 `8787` 是**开发手动 serve**；`dist\stage\start-editor.bat` 固定启动 **8788**（带 OCR 模型/config）。  
> 用户版编辑器地址是 **http://127.0.0.1:8788/**；`8787` 没有进程时 Firefox 报 `can't connect` 是端口/书签不对，不是程序崩溃。  
> ★ [10-31] `oasd serve` 默认在监听成功后**自动打开默认浏览器**；headless/服务用 `--no-open`。Windows 服务在 Session 0 不会自动弹到用户桌面，服务化时应由用户登录任务/launcher 打开。
> ★ `D:\OAS-dev\backend\toolkit` 只是本机外部开发工具包（ADB/OCR 模型）；oas-core 源码、构建、测试、发布不依赖 backend。若要完全脱离旧仓，可把 toolkit/模型复制到 oas-core 自带 `tools/` 或用环境变量覆盖。

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
- `temp`：**临时任务**（旧仓 `period=none` 的等价概念）：
  - **无**窗口/无 quota；不记忆"还剩几次/完成到哪"；
  - **B 触发方式**：临时条目可以放在 `entries` 里；调度器**每次启动/每一轮**按数组顺序跑一遍；
  - **执行前、执行后各重置一次执行状态**；门票/次数是否还有，由任务 flow 在这次启动时自己判断；"完成"= 门票耗尽/达到次数（任务自然结束），不是跨次标记。
- **删除**：`one_shot` 与 `continue`（"还有余量下次继续"）——用户裁定不需要。
- `rest`（可选，后期）：暂停 N 分钟的条目；v1 不优先。

**重置记忆功能**

- 入口：CLI + 编辑器「重置记忆」按钮；
- 弹**任务选择框**（多选、**全选/反选**）；
- 清除范围：**两者都清** = 调度器条目状态 + 任务 `state/`（checkpoint/计数等）；
- 重置后任务执行状态初始化（等价第一次运行）；原子写 + 审计事件 `memory_reset`。

**调度铁律**：窗口只回答"能不能跑"；执行顺序只由 `entries` 数组顺序决定；排序是动作不是状态；调度器不内置任何游戏时间/清单。

---

## 1.6 ★★★ 连续运行 / 断点续接（长任务必用）

> 规则全文：skill `session-memory` **强制 6（Continuity Protocol）**；当前任务断点：`docs/RESUME.md`。

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
   oas-core commit = <hash> · 分支 = dev · worktree 干净 = 是/否
   cargo test --workspace = __ passed / __ failed（当前基线 = 352）
   serve 状态：/state = <captured/device/ocr/det>
   oas-core docs commit = <hash> · dev 是否已推 = 是/否

【3. 差异（文档说 A、代码是 B）】
   | 文档说 | 代码是 | 差异 |

【4. 下一步（按 ENGINE §22.34 计划 + 我的判断）】
   1. …  2. …  3. …
   ★ 建议先做：… 因为 …

【5. 未完成台账】（N36/N37 等，见 PENDING-LEDGER §E-24）

【6. 需要用户裁定的事】

【7. 本轮完成定义（说完成前必须勾完）】
   ☐ …  ☐ …  ☐ …

【8. 连续性（长任务必填）】
   断点：seq=<N> · 阶段=<P0/P1/…> · 状态=in_progress/blocked/done
   下一步：<可直接执行的命令 / 文件:行号>
   RESUME：docs/RESUME.md（updated_at=…）· 索引：docs/RESUME-INDEX.md
```

---

# §3 ★ 活跃的坑（引擎相关）

| # | 坑 | 一句话 |
|---|---|---|
| **E1** | serve 构建必须带 `--features ocr` | 否则编辑器识别报 `this build has no OCR`；`/state` 会显示 `ocr:false` |
| **E2** | 重建 `oasd.exe` 前必须先停 8787 | 否则 `failed to remove target\debug\oasd.exe` |
| **E3** | `library/` 已 gitignore，`tasks/*/state` 已 gitignore | 不要把它们提交进去 |
| **E4** | `_verify/`、`runs/` 不在 git | 验收证据放 `_verify/nNN/`，不提交 |
| **E5** | 中文文档/代码一律用 `read`/`edit`/`write` 工具 | 不要用 PowerShell `Get-Content|Set-Content` 重写 UTF-8 文本（会 mojibake）|
| **E6** | adb 路径走 `ADB_PATH` | `D:\OAS-dev\backend\toolkit\Lib\site-packages\adbutils\binaries\adb.exe` |
| **E7** | 旧仓已冻结 | 旧仓 `backend/frontend` 只读；不要在旧仓上开发/提交 |
| **E8** | 新引擎的验收看 ENGINE §22 判据 | 不要拿旧仓 pytest 基线当引擎验收 |
| **E9** | headless Edge 截图做 UI 验收 | `msedge --headless=new --screenshot=...`；需要时加 `--user-data-dir` |
| **E10** | 测试数会随提交增长 | 当前 **352**；以 `cargo test --workspace` 实测为准 |

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
     - oas-core：提交到 `dev`，`main` 保持 v1.2.3 恢复节点，不自动合并
     - oas-core docs：git add docs && git commit && git push origin dev
★ ⑤ 验收证据进 _verify/nNN/，并写进文档判据
```

---

# §6 ★ 常用命令速查（引擎）

```bat
REM ---------- 构建/测试 ----------
cd D:\OAS-dev\oas-core
cargo test --workspace   REM 当前基线 352 passed
cargo build -p oasd --features ocr              REM serve/run 的 OCR 构建

REM ---------- 编辑器 ----------
REM 开发启动（根数据 assets/ + frames/，debug + OCR；端口 8787）
scripts\dev-serve.bat
REM   根数据：assets/（全局素材）、frames/（持久帧）；tasks/ 只放 flow/meta
REM   浏览器：http://127.0.0.1:8787/           （编辑器）
REM           http://127.0.0.1:8787/dashboard.html （点击分布/长跑统计）

REM 手动 serve（调试时；参数按实际路径补全）
target\debug\oasd.exe serve --port 8787 --ui apps\oasd\ui --tasks-dir tasks --assets-dir assets --frames-dir frames --runs-dir runs --library-dir library --no-open ^
  --model …\PP-OCRv6_rec_small.onnx --det-model …\PP-OCRv6_det_small.onnx

REM ---------- 运行/调度 ----------
target\debug\oasd.exe run --task tasks\S1切预设 --replay <recording-dir>
target\debug\oasd.exe queue --tasks tasks\A,tasks\B --replay <dir> --report report.json
target\debug\oasd.exe stats --runs runs --html runs\stats.html
REM   ⚠ 现行 schedule（v1 every_s）是原型，将被 v3 替换；设计见 ENGINE §22.34

REM ---------- 旧仓（冻结，仅参考） ----------
cd D:\OAS-dev\backend
run-tests.bat …                                  REM 旧仓守卫基线 171 passed（参考）
```

---

# §7 设计文档索引

| 主题 | 位置 |
|---|---|
| **长任务断点/续接（先读）** | `RESUME.md` · `RESUME-INDEX.md`（skill 强制 6） |
| **帧-素材关系** | `../frames/frames.json` + 全局/任务 `assets.json` 的 `positions`；无 interface 文件 |
| **当前任务（返回庭院）** | `../tasks/返回庭院/flow.json` · 全局素材 `../assets/` · 帧 `../frames/` |
| 引擎规格/进度 | `ENGINE-DESIGN.md` §22 |
| **调度 v3 + 统一任务模板设计定稿** | `ENGINE-DESIGN.md` **§22.34** |
| **到实机可用的交付路线（M0~M5/DoD/部署）** | `DELIVERY-PLAN.md` |
| 当前状态/下一步/坑 | `SESSION-HANDOFF.md` |
| 台账/证据 | `PENDING-LEDGER.md` §E-24 / §E-25 / §E-26（R1~R18）|
| 旧仓历史（冻结，仍在旧仓）| `D:\OAS-dev\backend\docs\SESSION-HANDOFF.md` 历史段 · `D:\OAS-dev\frontend\docs\scheduler-architecture.md`（旧调度设计，参考）|
