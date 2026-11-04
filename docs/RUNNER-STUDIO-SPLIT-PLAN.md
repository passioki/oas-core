# OAS Studio / Runner 分离 + TaskPackage + 任务配置（运行期参数）/ 子任务配置 · 设计稿 v1（已批准；PR1 ✅ / PR2 ✅ / PR3 ✅ / PR4 ✅ / PR5 ✅ 已实现）

> **状态：Q1~Q7 已获用户批准（2026-11-04，全部按推荐）；PR1 ✅ 已提交 `da9da9d`；PR2 ✅ 已提交 `dfe8776`；PR3 ✅ 已提交（代码 `c09df25` + seq 103 docs `048dff5`）；PR4 ✅ 已提交（HEAD `442f5cb`，含 seq 104 docs）；PR5 ✅ 已实现并验证（2026-11-04，**代码未提交，见 §A.13**）；PR6 待做。**
> 批准范围 = **§D.2 的 7 项决策（Q1~Q7，每项已标「✅ 用户批准（按推荐）」）**；后续按 **§D.3 的 PR6** 继续，用 **§D.4 的 19 步验收清单**收口（PR1 已验收 V1 子集，见 §A.9.8；PR2 已验收 V4 的 11~12，见 §A.10.7；PR3 无独立 V 步骤，注入链路见 §A.11.6~§A.11.7；PR4 覆盖 V6 的 16 核心路径，17~18 见 §A.12.7；PR5 覆盖 V2/V3，见 §A.13.8）。
> **2026-11-04 用户补充两条约束**（详见 **§A.8**）：① **分发纠正**——终端使用者只需要下载「使用端（Runner）」+ 任务包，**不需要下载编辑端（Studio）**；Studio 是创作/构建工具，不发给终端用户。② **跨平台**——后续可能有 **Android 使用端**；v1 仍只 Windows 打包，但 **Runner 核心 / HTTP API / 任务包 / 配置 schema 必须平台无关**。
> **PR1 实现记录见 §A.9**（Surface/路由、CLI、Config、Runner 只读 API、UI、测试、E2E、Runner-only 打包、遗留）；**PR2 实现记录见 §A.10**（ParamDecl 扩展 / `config/values.json` / Studio+Runner 配置 API 与 UI / 测试 / E2E / 评审修复 / 已知限制）；**PR3 实现记录见 §A.11**（`RunOptions.params` 类型保真 / 四层优先级 / 合并后 schema 复检 / CLI 引号逃生口 / 测试 seam / E2E / P2 限制）；**PR4 实现记录见 §A.12**（`call_task.params`/`inherit` / `with_params` 契约 / loader 严格展开 / runtime 作用域恢复 / T7 / T6 修复 / checkpoint 指纹 / E2E / 已知限制）；**PR5 实现记录见 §A.13**（`.oaspkg` 包格式/manifest/checksums 加固 / `flow_ops` 闸门 / Studio publish / Runner install·uninstall·activate / 素材合并与初始化 / UI / 评审修复 / 测试/E2E/包 / 已知限制）。
>
> 稿源（权威原稿）：`D:\OAS-dev\_verify\arch-runner-studio-20261104\RUNNER-STUDIO-CONFIG-DESIGN.md`（659 行 · 235+ 处 `file:line` 证据）。
> 本文是它的**整理稿**：保留全部架构/包格式/配置语义/风险/决策/分阶段/验收；只精简了审计过程与部分附录（附录 A 复用清单、附录 B 速查保留）。**决策点与验收项一项未删**，细节以原稿为准。
>
> 台账：`PENDING-LEDGER.md §E-79`。仓库 `D:\OAS-dev\oas-core`，分支 `dev`，HEAD 相对 `f16fea1`（设计期快照）。
> 范围 = 用户**事项 2（使用端 / 编辑端分离）** + **事项 3（任务配置入口 + 子任务配置语义）**。
> 约束：**单机、单活动实例、无跨机分发**（`ENGINE-DESIGN.md` 单活动实例约定 · `SCHEDULER-RUNNER-PLAN.md` §37）。
> ⚠ 行号说明：稿源行号取自 2026-11-04 18:20 前后快照（审计期间工作树被另一进程并发修改，`HEAD` 仍 `f16fea1`）。复核时**优先用符号名**（`Select-String -Pattern 'fn handle_task_run' apps/oasd/src/serve.rs`），行号会漂移。

---

## 0. 结论摘要（TL;DR）

1. **分离 = 「同一份代码、两个 surface」，不是两个二进制**：现有 HTTP 服务是手写 `match (method, path)` 路由表（`serve.rs:1206-1352`），加一个 `surface` 字段 + 一张**路由归属表**即可做到「不同入口 / 不同端口 / 不同 UI / nav 集合不同」，并可用现成的 `get_route()` 测试钩子（`serve.rs:7632`）在**不开端口**的情况下纯函数化断言。
2. **Runner 内嵌调度线程**（复用现成 `schedule::run_scheduler` 主循环），保留 `oasd scheduler` CLI 给 headless/CI；两者共用 `<state>.lock`（`schedule.rs:775-833`）⇒ **天然互斥**，不需要新互斥机制。
3. **TaskPackage = `.oaspkg`（zip 容器）+ `manifest.json` + `checksums.sha256`**；v1 只做 **sha256 完整性 + 来源标记**，**不做签名**；发布产物落 `dist/packages/`（`dist/` 已在 `.gitignore:13` 忽略）。
4. **任务配置模型不需要新造一套**：`meta.params`（`crates/oas-task/src/meta.rs:45-58`）已够用，扩展 `label/description/enum/required/group` 五个**可选**字段即可；Studio 编辑 schema，Runner 只填值，值存 Runner 独占文件 `config/values.json`。
5. **子任务配置用「作用域」而不是「静态改写」**：`call_task` 增加 `params` 映射 + `inherit`，loader 展开时生成 `with_params` 包装 step，runtime 用 **params 栈**实现（新增 Step 变体，向后兼容旧任务）。**✅ PR4 已实现（2026-11-04，§A.12）**。
6. **最大风险不是分离，而是 `call_task` 是 load-time 展开**（`crates/oas-task/src/lib.rs:1058-1091`）：调用点身份在 runtime 完全消失，`with_params` 只是「作用域」不是「调用栈」⇒ 本期**不做**「子任务独立超时/重试/运行记录」（见 §D.1 **R1**）。

---

# A. 总体架构（事项 2）

## A.1 现状事实（审计基线 = PR1 前，先钉死）

> ℹ 本表是 **2026-11-04 18:20 审计时的现状**，用于解释「为什么要做」；**PR1 已改变其中 A1-1/A1-3/A1-4/A1-8 的部分事实**（现在有 8788/8789 双入口、`oasd studio|runner`、Runner UI 骨架与只读 API）。**现行实现状态以 §A.9~§A.13 为准**，本表保留为决策依据。

| # | 事实 | 证据 |
|---|---|---|
| A1-1 | 单进程 `oasd serve` 同时供编辑工作台 + 任务库 + 实例 + 调度 + 运行记录，UI nav **6 项** | `serve.rs:1206-1352`；`apps/oasd/ui/index.html:47-56`（工作台/任务构建/任务库/实例/调度/设置） |
| A1-2 | 手写 TCP + 裸 `match (method, path)`，无框架/无中间件 | `serve.rs:1141`、`serve.rs:1206`、`serve.rs:1346-1351`（手写 404） |
| A1-3 | 只绑 `127.0.0.1`；端口来自 `--port`（CLI 默认 **8787**），`config.serve.port` 默认 **8788** 但**不参与 bind** | `serve.rs:983-986`、`main.rs:388`、`config.rs:44-45`、`serve.rs:4735`（只回显） |
| A1-4 | 调度器是**独立 CLI 守护进程**，serve 不托管、不拉起 | `main.rs:98`；`serve.rs:6928-6932`（status 只读文件） |
| A1-5 | serve 能**单次**触发任务执行（编辑器试运行），线程 + 全局单跑锁 | `serve.rs:6521-6623`（`POST /task/run`，**无 params 字段**）、`serve.rs:6580-6589`（已运行 → 409） |
| A1-6 | 调度运行中 serve 拒绝写 `scheduler.json` | `serve.rs:6880-6886`（读 `<state>.lock` 判 pid）→ 409 |
| A1-7 | 单实例锁 = `create_new` 锁文件 + 陈旧回收 | `schedule.rs:775-833`（`SchedulerLock`）、`schedule.rs:880-884` |
| A1-8 | 部署包已有「两个入口」雏形：`start-editor.bat`（8788）+ `start-scheduler.bat`（CLI 守护） | `packaging/start-editor.bat:10-20`、`packaging/start-scheduler.bat:17-26`、`packaging/README-DEPLOY.md:48,63` |
| A1-9 | 「编辑器与守护进程抢同一台设备」目前**只是文档规则** | `packaging/README-DEPLOY.md:81`；`SCHEDULER-RUNNER-PLAN.md:37` 列为未做补强 |
| A1-10 | UI 数据面白名单（会写 `editor.log` 的路径）已是「surface 雏形」 | `serve.rs:1197-1203`（`LOGGED_PREFIXES`） |
| A1-11 | 请求体上限 4 MiB，且整包读入内存 | `serve.rs:50`（`MAX_BODY_BYTES`）、`serve.rs:1169-1183` |
| A1-12 | `config/live.json` 的 paths 只有 tasks/runs/library；assets/frames 走 CLI flag | `config.rs:51-67`、`main.rs:431-442` |

**结论**：A1-1/A1-4/A1-8 说明「运行」与「编辑」在**进程层面已经分开**（serve vs scheduler CLI），但**入口 / UI / 职责边界没有分开**——用户看到的是「一个 8788 干所有事」。事项 2 的实质工作 = 把「运行」那一半产品化成一个有**独立入口、独立 UI、独立职责边界**的 **Runner**，并把包 / 安装 / 配置链路补齐。

## A.2 进程 / 命令 / 端口 / UI 拆分

> 设计目标表。**PR1 已实现**命令（`oasd studio` / `oasd runner` / `serve` 兼容别名）、默认端口 8788/8789、根 UI 与静态面隔离；`start-studio.bat`/`start-runner.bat` 已落 `packaging/`。逐项实现证据见 §A.9.2。

| 维度 | Studio（编辑端） | Runner（使用端） |
|---|---|---|
| 命令 | `oasd studio [...]`（新增；`oasd serve` 保留为 deprecated 别名 + 一行迁移提示） | `oasd runner [...]`（新增） |
| 默认端口 | **8788**（与 `packaging/live.example.json:5`、`start-editor.bat:11` 一致；顺手把 `main.rs:388` 的 8787 修成 8788，消除现状不一致） | **8789**（新，`config.runner.port`） |
| 绑定 | `127.0.0.1` 固定 | `127.0.0.1` 固定（**不建议**暴露 LAN：`README-DEPLOY.md:82` 明确不要公网；真要 LAN 再加 `runner.bind` + token，见 §D.2 Q1） |
| 根 UI | `GET /` → `ui/index.html`（现有 6 页 nav，**去掉「调度」页**） | `GET /` → `ui/runner.html`（新；nav **6 项**：概览 / 已安装任务 / **安装包（PR5 新增）** / 队列与调度 / 运行记录 / 设置；PR1 时 5 项） |
| UI 资源 | `index.html`/`app.js`/`builder.js`/`flow_model.js`/`asset_ui.js`/`style.css` | `runner.html`/`runner.js`/`runner.css`（新）+ 复用 `style.css` |
| 启动脚本 | `packaging/start-studio.bat`（新，替 `start-editor.bat` 内容） | `packaging/start-runner.bat`（新，替 `start-scheduler.bat` 内容） |
| 进程数 | 1 | 1（内嵌调度线程，见 §A.6） |
| 日志 | `logs/editor.log`（现有 `editor_log`，`serve.rs:1202`） | `logs/runner.log` + 现有 `logs/scheduler.events.jsonl`（`start-scheduler.bat:25`） |

**「不同地址 / 入口」的三重保证（缺一不可）**：

1. **命令不同** —— `main.rs:89-103` 的 dispatch 表加 `"studio"` / `"runner"` 两个分支（`serve` 保留）。
2. **端口不同** —— 两个进程各 bind 自己的端口；`GET /instance` 回显的 `serve_port`（`serve.rs:4734`）改成回显**本 surface 的实际端口**。
3. **路由白名单不同** —— 同一个 `serve::Server` 加 `surface: Surface` 字段（`serve.rs:252-267`），在 `handle()` 进入 `match` 之前先做归属判定：

```rust
// apps/oasd/src/runner.rs（新）或 serve.rs 顶部
#[derive(Clone, Copy, PartialEq)] pub enum Surface { Studio, Runner }

/// 纯函数路由归属表；「共享只读」用 Surface::Both 表示。
pub fn required_surface(method: &str, path: &str) -> Option<Surface> { ... }
```

`handle()` 里：`if let Some(need) = required_surface(method, &path_only) { if need != Surface::Both && need != self.surface { return 404 } }`。
**落地成本极低**：路由表已存在（`serve.rs:1206-1352`），`required_surface` 与匹配臂一一对应；且 `serve.rs:7632` 已有的 `get_route(&server, "/runs")` 测试钩子可以**不开端口**断言「Studio 拿不到 `/runner/*`，Runner 拿不到 `/task/save_flow`」。

> ❌ **反面方案（不推荐）**：把 `serve.rs` 拆成 `studio.rs` + `runner.rs` 两文件、复制 handler。458 KB 的 `serve.rs` 里 handler 与 `Server` 字段/工具函数强耦合（`scheduler_options()` `serve.rs:6739-6751`、`write_text_atomic` `serve.rs:7151`），拆文件等于 200+ 处可见性改动，风险远大于收益。

## A.3 路由分组表（Studio-only / Runner-only / 共享）

依据 `serve.rs:1206-1352` 逐条归类。`S`=Studio-only，`R`=Runner-only，`B`=共享（只读优先）。

| 路由 | 归属 | 说明 |
|---|---|---|
| `GET /`、`/app.js`、`/builder.js`、`/flow_model.js`、`/asset_ui.js`、`/style.css`、`/dashboard.html`、`/dashboard.js` | **S / R**（同路径按 surface 返回不同文件） | Runner 的 `/` → `runner.html`；`style.css` 共享 |
| `GET /ws/live` | S | 实时帧是编辑能力 |
| `GET /frame.png`、`/state`；`POST /capture*`；`POST /input/tap`、`/input/swipe` | S | 工作台画面 / 截帧入库 / 手动点击滑动 |
| `GET /frames`、`/frames/image`、`POST /frames/*` | S | 帧库 |
| `GET /ocr`、`/lines`、`/match` | S | 识别调试 |
| `POST /task/save_asset`、`/task/save_assets`、`/task/asset/*`、`/task/assets/reorder`、`/task/assets/batch_test`、`GET /task/assets`、`/task/asset/image`、`/task/asset/match`、`/task/asset/collection/match` | S | 素材创作 |
| `GET /assets/conflicts`、`POST /assets/migrate` | S | 素材库维护（创作态） |
| `POST /relations/record`、`/relations/annotate`、`/relations/annotate/delete`、`GET /relations/frame-assets` | S | 关系标注 |
| `GET /task/flow`、`POST /task/save_flow` | S | 流程编辑（**Runner 必须拿不到**） |
| `POST /task/create`、`POST /task/goal` | S | 建任务 / 完成条件（后者自动补 `params.max_battles`，`serve.rs:4396-4420`） |
| `POST /task/run`、`/task/run/stop`、`GET /task/run/status` | S | **编辑器试运行**（见 §A.6.4 设备租约） |
| `GET /task/library`、`POST /task/delete` | S | 任务库 = 创作态增删；删除守卫已含「队列成员拒绝」（`serve.rs:4526-4529`）与调度条目剪枝（`4550-4566`） |
| `GET /library`、`POST /library/save_asset` | S | 全局模板库 |
| `GET /task/config`、`POST /task/config`（新；**PR2 已实现**） | S | 读/编辑 `meta.params` schema（§C.1） |
| `POST /task/publish`、`GET /task/package`（新） | S | 发布 TaskPackage（§B.3） |
| `GET /runs`、`/runs/clicks`、`/runs/aggregate` | **B（只读）** | 点击分布属编辑分析，但运行记录 Runner 也要看；写接口分离 |
| `POST /runs/delete` | R | 运维删档 |
| `GET /runs/scheduler`、`/runs/scheduler/run` | R | 调度 Run 一等实体（`schedule.rs:104-111`） |
| `GET /scheduler/config`、`GET /scheduler/status` | **B（只读）** | Studio 需要「该任务是否被调度引用」的只读视图（删除守卫依赖） |
| `POST /scheduler/config`、`POST /scheduler/reset` | R | 写调度配置 / 清记忆 = 运维 |
| `GET /config` | B | 部署配置只读 |
| `GET /instance`、`/instance/devices`、`POST /instance` | B | 设备是 Studio 采集与 Runner 执行**共用**的唯一物理资源；写 `config/live.json` 两端都允许（写入即 `restart_required`，`serve.rs:4847`） |
| `GET /stats` | B | 统计只读 |
| `GET /runner/health`、`POST /runner/scheduler/start`、`/stop`（新） | R | 健康（PR1 ✅）；调度启停 = PR6 |
| `GET /runner/tasks`、`GET/PUT /runner/tasks/<id>/config`（新） | R | 已安装任务 + 运行值（PR1 ✅ / PR2 ✅；PR5 给 `/runner/tasks` 增 `installed/package_id/installed_at_ms/backup/unmanaged/meta_version`） |
| `GET /runner/packages`；`POST /runner/install`、`/runner/uninstall`、`/runner/packages/activate`（新，**PR5 ✅**） | R | 已安装清单 / 安装（Phase 1 仅本机绝对路径）/ 卸载（先备份）/ 回滚 |
| `POST /runner/run`（新） | R | Runner 临时运行 = **PR6**（必须自行合并 values 层） |

> **一句话**：**Studio 无调度写/运维（无调度页）**，**Runner 无创作/构建 API**（拿不到 `/task/save_flow` 等）；`get_route()` 单测把这条钉死。
>
> **PR1/PR2/PR5 落地范围（2026-11-04）**：本表已作为 `route_owner`/`required_surface` 的唯一依据落地（S/R/B 三类 + 静态文件按 surface 取）。PR1 handler = `GET /runner/health`、`GET /runner/tasks`；**PR2 handler = `GET/POST /task/config` 与 `GET/PUT /runner/tasks/<id>/config`**；**PR5 handler = Studio `POST /task/publish`、`GET /task/package` + Runner `GET /runner/packages`、`POST /runner/install`、`POST /runner/uninstall`、`POST /runner/packages/activate`（共 6 条新路由：2 Studio + 4 Runner；`/runner/tasks` 为既有路由增字段）**；只剩 `/runner/run`、`/runner/scheduler/start|stop` 无 handler（PR6）。共享只读路由 `/runs*`、`/scheduler/config|status`、`/instance*`、`/config`、`/stats` 两端一致。路由归属与两端互斥 404 由 `required_surface` 单测钉死（PR5 新增 8 条路由的归属断言，见 §A.13.8）。

## A.4 职责矩阵

| 能力 | Studio | Runner | 现状与证据 |
|---|---|---|---|
| 创作（帧 / 素材 / OCR / 流程编辑） | ✅ 主 | ❌ | `serve.rs:1215-1310` |
| 构建 / 校验（lint T1-T6，+T7 新增） | ✅ 主 | ❌（**安装时复检**） | `oas-task` `lint`（`lib.rs:2035-2052`）；`oasd task lint`（`task.rs:360-371`） |
| **发布（只产出 TaskPackage）** | ✅ 主 | ❌ | **需新增**（§B.3） |
| 安装 / 更新 / 卸载 / 回滚 | ❌ | ✅ 主 | **需新增**（§B.4） |
| 任务配置 **schema**（声明/默认值/范围） | ✅ 主（写 `meta.params`） | ❌ | 已有 `meta.params` + lint（`meta.rs:433-470`） |
| 任务配置 **运行值**（填值） | ❌ | ✅ 主 | **PR2 已新增**存值文件 `config/values.json` + Runner 配置表单（§C.2/§A.10）；PR2 前只有调度 entry 的 params JSON 输入框（`ui/index.html:618-621`） |
| 排队（queue） | ❌ | ✅ | 已有 `scheduler.json.queues[]`（`scheduler.rs:685-702`）；`oasd queue` 是另一条**临时串跑** CLI（`queue.rs:1-21`），两者不要混 |
| 调度（windows/quota/lock/pass） | ❌ | ✅ | `scheduler.rs:664-764`、`schedule.rs:1-60` |
| 执行（`run::execute`） | 仅「试运行」 | ✅ 主 | `run.rs:461-481`；Studio 试运行 `serve.rs:6521-6623` |
| 运行记录 | 只读（点击分布） | ✅ 主 | `runs/scheduler/<run_id>.json`（`schedule.rs:104-111`） |
| 设备 / 实例配置 | 读+写（采集需要） | ✅ 权威 | `config.rs:85-104`；`GET /instance/devices` 自动 connect（`serve.rs:4755+`） |
| 素材库（全局 assets） | ✅ 主 | 只读消费（安装时合并） | `global_assets.rs:24`；合并见 §B.4.3 |
| 帧库 / 关系 / 模板库 library | ✅ 主 | ❌ | `frames/`、`relations/`、`library/`（`library.rs:14`） |

## A.5 迁移与兼容

1. **命令兼容**：`oasd serve` → 内部等价 `oasd studio`，stderr 打一行 `note: 'oasd serve' is deprecated, use 'oasd studio'`；**不改行为**。`oasd scheduler` 完全不变（`start-scheduler.bat` 继续可用）。
2. **数据目录：共享，不分离**。单机单实例下 `tasks/`、`assets/`、`frames/`、`runs/`、`scheduler.json`、`scheduler.state.json` 仍是**一份**；理由：
   - 现有 `paths` 配置只有 3 项（`config.rs:51-67`），拆目录意味着「Studio 发布到 A、Runner 从 B 读」，反而要求用户记住两份路径；
   - 磁盘上 `tasks/` 已是 git 跟踪的创作源（`.gitignore` 未忽略 `tasks/`），发布包只是构建产物；
   - 若拆目录，`POST /task/run`（Studio 试运行）会跑「编辑态」任务而 Runner 跑「安装态」任务，用户会看到两份不同结果 → 更难解释。
   **新增目录（Runner 独占写入）**：`packages/`（staging/backup/installed.json）、`config/values.json`。
3. **单机单实例约束的落地**：
   - 调度：`SchedulerLock` 已是跨进程锁（`schedule.rs:775-833`），Runner 内嵌线程与 `oasd scheduler` CLI **自动互斥**；
   - 设备：**新增设备租约**（§A.6.4），否则 `POST /task/run`（Studio）与 Runner 调度会同时点同一台机器。
4. **老用户迁移路径（三步，且都可回退）**：
   `start-editor.bat` → `start-studio.bat`（同端口 8788，行为不变）；`start-scheduler.bat` → `start-runner.bat`（多一个 8789 页面，调度行为不变）；现有 `tasks/` 无需重新打包（**无包也能被 Runner 直接识别**，见 §B.4.5「裸目录兼容」）。

## A.6 Runner 的调度执行

### A.6.1 推荐方案：Runner 进程内**线程**托管调度，不新增守护进程
- 复用 `schedule::run_scheduler(opts, out)`（现有阻塞主循环，`schedule.rs:3179` 附近；`schedule.rs:1188` 为 passes 循环）在 `std::thread` 里跑；`out` 闭包把行写入 `logs/runner.log` 与内存环形缓冲（供 `/runner/health` 展示）。
- 理由：① **锁天然互斥**（仍用 `<state>.lock`，`schedule.rs:780-786`；`/runner/health` 直接读 `lock_held_by_live_process` 说明「谁在跑」）；② **服务端不会被阻塞**（`serve.rs:999-1007` 是同步 `incoming()` 循环，但调度在独立线程；重活已有先例 `serve.rs:6627` 起线程跑 run）；③ **少一个用户步骤**（今天要双击两个 bat，`README-DEPLOY.md:48,63`，最常见故障是「scheduler 没起 / 起了两份」）；④ **不引入 Windows 服务/管理员权限**（`README-DEPLOY.md:83` 已说明 exe 未签名，装服务会放大阻力；开机自启留给任务计划程序 + `start-runner.bat`）。
- ❌ 不推荐「Runner 用子进程拉起 `oasd scheduler`」：多一层进程生命周期管理（僵尸 / 日志重定向 / 退出码），且 `main.rs:43-53` 已记录过 `--config`/`--plan` 的坑，再套一层 CLI 只会放大它。

### A.6.2 与现状的差异（需新增，均小改）

| 需要 | 现状 | 改动 |
|---|---|---|
| 可停止 | `SchedulerOptions` 无 cancel/stop（`schedule.rs:335-370`），只能杀进程或 `--max-passes` | 加 **`stop: Option<Arc<AtomicBool>>`**，pass 循环检查；`POST /runner/scheduler/stop` = 置位 + join ≤ 5s（当前 attempt 跑完即停，**不中断**正在执行的任务，符合「单活动实例」） |
| 状态可见 | `GET /scheduler/status` 已有 `status_json`（`serve.rs:6930`、`schedule.rs:2686`） | Runner 概览页直接用；`/runner/health` 只多 `lock_pid` / `thread_alive` |
| 自动启动 | 无 | `oasd runner --no-scheduler` 可关；默认启动即跑 |

### A.6.3 启停 / 健康 / 日志 / 锁
- 启动：`oasd runner` → 校验 `config/live.json` → 起 HTTP（8789）→ 起调度线程（若 `scheduler.json` 不存在，线程空转并报 `blocked`，**不崩**）。
- 停止：`POST /runner/scheduler/stop`、`stop-runner.bat`（发 POST 后结束进程）、Ctrl+C。
- 健康：`GET /runner/health` → `{ ok:true, surface:"runner", scheduler:{running,lock_pid,passes,last_pass_ms,next_wake_ms}, device:{...}, tasks_dir, packages_dir }`。
- 日志：`logs/runner.log`（追加，`start-runner.bat` 用 `>>` 同 `start-scheduler.bat:25`）+ `logs/scheduler.events.jsonl`（现有审计事件）。
- **两个锁，语义不同，不要混**：`<state>.lock`（调度守护，`schedule.rs:780`）、`config/device.lease`（设备占用，**新增**，复用同一 `create_new` + pid 回收模式）。

### A.6.4 设备租约（解决 A1-9）

```
config/device.lease = {"pid":1234,"surface":"studio","mode":"editor-run","started_ms":...}
```

- `POST /task/run`（Studio）与 Runner 每次 attempt 前都必须 `acquire`；释放用 `Drop`（同 `SchedulerLock` `schedule.rs:829-833`）。
- 复用 `process_alive` / `read_lock_pid` 语义（`schedule.rs:835-840`）。
- 冲突返回 **409** + 明确文案：「Runner 正在调度（pid 1234），Studio 试运行已拒绝」。这是把 `SCHEDULER-RUNNER-PLAN.md:37` 的「文字规则」变成机制。

## A.7 安全

| 面 | 现状 | 设计 |
|---|---|---|
| 绑定面 | 硬编码 `127.0.0.1`（`serve.rs:985`）；`config.serve.bind` 只校验不生效（`config.rs:178-179`、`serve.rs:4735`） | 沿用：**两个 surface 都只绑 127.0.0.1**；`bind` 字段要么删除要么真正生效后加 token 鉴权，**不要留「看似可配实则无效」的字段** |
| 请求体 | 4 MiB 上限 + 整包读内存（`serve.rs:50,1169-1183`） | 安装包上传单独放宽：`/runner/install` 用 **64 MiB** 且**流式落盘**到 `packages/incoming/<sha>.part`（新增分支，不改其他路由上限） |
| 路径 | 删除任务已做 canonicalize + 父目录校验 + `validate_task_name`（`serve.rs:4508-4522`、`lib.rs:535`） | 安装/卸载复用同一套；解压目标路径 `canonicalize` 后必须仍在 `packages/staging/` 下 |
| zip-slip | 无 zip 支持（`Cargo.lock` 无 `zip`/`sha2`，有 `flate2`） | 解压前逐条校验 entry name：拒绝绝对路径、`..`、盘符、`\\`、符号链接；限制条目数（≤ 2000）、解压后总大小（≤ 512 MiB）、压缩比（> 100:1 直接拒） |
| 覆盖安装 | 无 | 先备份 `tasks/<id>/` → `packages/backup/<id>-<old_ver>-<ts>/`；**同 id 同 version 覆盖要求 `force:true`**；失败自动回滚 |
| 包来源 | 无 | `manifest.source = {repo, commit, dirty}` + `files[].sha256` + 顶层 `package_sha256`；**v1 不做签名**（单机本地、无 PKI、用户既是作者也是使用者；密钥保管成本 > 收益）→ UI 明确标注「来源：本机 Studio / 未签名」，`installed.json` 留 `source` |
| 上传目录写权限 | — | `packages/` 需在部署包预建，`.gitignore` 加 `/packages/`（参照 `.gitignore:13` 的 `/dist`） |

> **PR1 已落地（安全相关）**：`runner.port` / `runner.bind` 配置校验（`port != 0`、`port != serve.port`、`bind` 仅 loopback：`127.0.0.0/8`、`::1`、`localhost`）；**无 token 前不开放 LAN**。zip-slip / 大小与压缩比配额 / 安装备份**已由 PR5 落地**（§A.13.1/§A.13.4）：zip-slip 双向拒绝、条目 ≤2000、总解压 ≤512 MiB、单条 ≤256 MiB、压缩比 >100:1 拒绝、encrypted 拒绝、`take(declared+1)` 防声明大小欺骗。

## A.8 分发与跨平台（用户 2026-11-04 补充）

### A.8.1 分发纠正：终端用户只下载 Runner + 任务包

> **用户原话口径**：**终端使用者只需要下载「使用端（Runner）」+ 任务包，不需要下载编辑端（Studio）**；Studio 是创作/构建工具，不发给终端用户。

| 发行包 | 面向谁 | 包含 | 不包含 | 入口 |
|---|---|---|---|---|
| `oasd-v<version>-win64.zip`（Studio 包） | 开发 / 编辑 / 发布任务的人 | Studio UI（工作台/任务构建/任务库/实例/设置）、完整数据目录、发布工具 | —（这是完整开发包） | `start-studio.bat` → **8788**（`start-editor.bat` 兼容转调；`oasd serve` = deprecated 别名） |
| `oasd-runner-v<version>-win64.zip`（Runner-only 使用端包） | **终端用户**（只下载这个 + `.oaspkg`） | `oasd.exe`、`ui/runner.{html,js,css}` + `ui/style.css`、OCR models、adb、`config/live.{json,example.json}`、`start-runner.bat`/`stop-runner.bat`、`README-RUNNER.txt`、空 `tasks/packages/runs/logs/assets`、空 v4 `scheduler.json`、`VERSION`/`BUILD-INFO.txt` | **Studio 编辑 UI**（`index.html`/`app.js`/`builder.js`/`flow_model.js`/`asset_ui.js`/`dashboard.*`）、`frames/`、`library/`、`start-studio.bat`/`start-editor.bat`/`stop-studio.bat`、开发机 `config/live.json`、用户运行态数据 | `start-runner.bat` → **8789**（`oasd runner`） |

- **职责链**：Studio 创作/构建/校验 → 发布 `.oaspkg` → 终端用户拿到 **Runner 包 + `.oaspkg`** → Runner 安装/配置/排队/调度/执行/看运行记录。**Studio 不随使用端分发**；Runner 包里没有编辑入口。
- **打包脚本**：新增 `scripts/package-runner.ps1`（Runner-only 清单 + 禁用文件名审计 + `/` 分隔 zip entry）+ `scripts/package-common.ps1`（共享版本/BUILD-INFO 读取）；`scripts/package.ps1`（Studio 包）复用同一 helper，BUILD-INFO 字段顺序不变。`packaging/README-DEPLOY.md`「两种发行包（必读）」与 `packaging/README-RUNNER.txt` 已按此口径改写。
- **PR1 产物事实**：`dist/oasd-runner-v0.1.0-win64.zip` = **37,141,295 B**（用重建后的 PR1 release 二进制打包；zip 内 **23 entries、0 反斜杠 entry**）。产物在 `dist/`，**不入 git**（Q7）。
- **PR2 产物事实**：Runner 包已用 PR2 release 重建 → `dist/oasd-runner-v0.1.0-win64.zip` = **37,192,710 B**，SHA256 **A10DEDC1A5D0FAB15289ACBB9F904C5909AFA148B3317D2408350B12B4533F5D**（含新 `runner.html/js/css`；zip 内仍只有 Runner，无 Studio UI）。
- **安装能力分期**：PR1 Runner 只有只读页面（概览/已安装任务/队列与调度/运行记录/设置，5 项）；`.oaspkg` 真正安装 = **PR5 ✅（§A.13）**，Runner nav 增「安装包」页（**6 项**）；**运行参数保存 = PR2 ✅（§A.10），值注入/四层优先级 = PR3 ✅（§A.11）**；调度编辑/启停 = **PR6**。`README-RUNNER.txt` 已在 PR5 更新（安装可用、force/素材/回滚说明）；PR2 已订正「运行值保存」措辞，PR3 已把「后续版本接入调度/队列运行」变为现实，PR5 打包同步订正。

### A.8.2 跨平台约束（为 Android 使用端预留）

> 权威细则：**`packaging/CROSS-PLATFORM.md`**（167 行：平台无关契约 / Windows v1 事实 / macOS·Linux / Android 两种候选 / 适配边界 / v1 决策摘要）。
> 用户口径：**要注意跨平台特性，后续可能会有 Android 使用端**；**v1 仍只 Windows 打包**，但下面四层必须平台无关。

| 层 | PR1 已满足 / 约束 | 证据 |
|---|---|---|
| Runner 核心（Rust） | 新逻辑只用 `std` + `Path::join` + HTTP/JSON；无盘符、`cmd`、注册表、COM 硬编码；平台相关只在既有 adapter（`process_alive` 的 `cfg(windows)/cfg(unix)`） | PR1 后端报告 §6；`serve.rs` 新代码无 `cfg(windows)` 业务分支 |
| 路径 | 静态文件一律 `Path::new(ui_dir).join(file)`；`/runner/tasks` 用 `Path::join` 扫目录；非 UTF-8 目录名跳过不 panic | `serve.rs` `send_ui_file` / tasks scanner |
| HTTP API | `/runner/health`、`/runner/tasks` 返回纯 JSON（路径为展示字符串）；`port` 为实际绑定端口；**`--port 0` 由 `local_addr()` 回填**，供 Android/嵌入壳取随机端口 | PR1 后端报告 §6.1 |
| 任务包 / 配置 schema | `.oaspkg` = zip + 相对路径；zip 写入统一 `/` 分隔（PS 5.1 `Compress-Archive` 会写 `\`，脚本已绕开）；配置 schema 不出现平台必填字段；`manifest.platform` 只能可选只读、不参与安装/执行判定 | `scripts/package-runner.ps1`；`CROSS-PLATFORM.md §2.4` |
| UI | `runner.html/js/css` 纯标准 Web（无 CDN/字体、无浏览器专属 API、无硬编码端口/盘符），可由桌面浏览器或 Android WebView 加载同一 URL；窄屏 ≤720px 单列 | PR1 UI 报告 §4/§6；`runner.css` |
| 设备控制 | 仍走既有桌面 adb adapter；**Android 不能走 adb**（MediaProjection 截图 + AccessibilityService 点击），未来只换 adapter，不复制 Runner 业务 | `CROSS-PLATFORM.md §5/§6` |

**Android 使用端路线（v1 不实现，只写边界）**：
1. **候选 ①（优先评估）on-device Runner**：交叉编译 `aarch64-linux-android` → JNI/Kotlin 壳启动 Rust Runner（数据根 = app 私有目录、`bind 127.0.0.1:0` 随机端口）→ 手机内 WebView 加载 `http://127.0.0.1:<port>/`；复用同一套 `/runner/*` API、`.oaspkg`、`scheduler.json` 语义；设备控制换 Android 原生 adapter。
2. **候选 ②（更远）手机当控制器**：手机只跑 Runner UI，执行仍在 PC/另一设备；依赖 LAN + token 安全模型，v1 明确不做。
3. **明确不做**：v1 不做 Android 打包、不做 iOS、不做 LAN 多机调度、不在 Android 内置/调用 adb 去控制本机、不为 Android 复制一份 runner 业务逻辑。
4. **平台适配边界**（详见 `CROSS-PLATFORM.md §6`）：前台服务/电池/Doze、通知、scoped storage/SAF 导入、无 adb 的控制通道、刘海/旋转/坐标换算、后台启动限制、logcat 日志、APK 升级与任务包独立升级。

## A.9 PR1 实现记录（2026-11-04，已验证；已提交 `da9da9d`）

> 基线：分支 `dev`，HEAD = **`f84b56e`**（seq 100 docs 提交，含设计稿 v1）；**PR1 代码父 = `f84b56e`**，当时全部改动未提交（后由 Lead 提交为 `da9da9d`）。范围 = **PR1**：Surface 拆分 + Runner UI 骨架 + Runner-only 发行包 + 跨平台约束；**不含** PR2~PR6 功能（PR2 见 §A.10）。
> 证据：`D:\OAS-dev\_verify\runner-studio-pr1-20261104\`（`PR1-BACKEND-REPORT.md` / `PR1-UI-REPORT.md` / `PR1-PACKAGING-REPORT.md` / `oasd-test.log` / `workspace-test.log` / `runner-*.png` / `studio-nav.png`）。

### A.9.1 后端：Surface 与路由归属（`apps/oasd/src/serve.rs`）

- `Surface{Studio, Runner}`（`as_str/default_port/other_default_port`）+ `RouteOwner{Studio,Runner,Both}` + 纯函数 `route_owner(method, path_only)` / `required_surface(...)`，严格按 **§A.3 路由归属表**；`Server` 新增 `surface`/`port` 字段（`new(adb)` 签名不变，默认 Studio/0，旧测试零改）。
- `handle()` 在 **`/ws/live` 特判、静态文件、业务 match 之前**做归属 gate：请求 surface 不匹配 ⇒ **纯 404**（`{"error":"not found"}`），handler 不执行。`Both` 与未知路由不拦截（未知路由落到原 404）。
- 静态面：Studio `GET /` → `ui/index.html`，`app.js` / `builder.js` / `flow_model.js` / `asset_ui.js` / `style.css` / `dashboard.*` 仅 Studio；Runner `GET /` → `ui/runner.html`，`runner.js` / `runner.css` 仅 Runner，`style.css` 共享。**Runner 的 `/app.js` 404；Studio 的 `/runner.js` 404**。
- 新逻辑纯 `std` + `Path::join` + HTTP/JSON；`send_ui_file` 用 `Path::new(ui_dir).join(file)`，无盘符/`cmd`/反斜杠硬编码；tasks 扫描对非 UTF-8 目录名跳过不 panic。

### A.9.2 CLI / 端口（`apps/oasd/src/main.rs`）

| 命令 | 行为 |
|---|---|
| `oasd studio` | 默认 **8788**（修掉旧 CLI 8787 与 `live.example.json` 8788 的不一致） |
| `oasd runner` | 默认 **8789** |
| `oasd serve` | **deprecated 别名**（内部等价 `studio`），stderr 打一行 `note: 'oasd serve' is deprecated, use 'oasd studio'`；v2 再删（Q2） |

- 端口优先级：**CLI `--port` > 对应 surface 的 config（Studio `serve.port` / Runner `runner.port`）> 8788/8789**。
- 启动 banner 打印 surface、**实际绑定端口**、双入口提示（`separate entry points: studio default 8788, runner default 8789`）；端口恰为另一端默认值时额外提示。
- `GET /instance` 的 `serve_port` 改为回显**本进程实际绑定端口**（bind 后写回），不再回显 `config.serve.port`。
- `--port 0` 由 `local_addr()` 回填实际端口（为 Android/嵌入壳预留）。

### A.9.3 Config（`apps/oasd/src/config.rs`）

```rust
pub struct OasConfig { pub serve: ServeConfig, pub runner: RunnerConfig, ... }
pub struct RunnerConfig { pub port: u16, pub bind: String }  // 默认 8789 / "127.0.0.1"
```

- `OasConfig` 仍 `#[serde(default, deny_unknown_fields)]`；未写 `runner` 的旧 `live.json` 完全兼容（`#[serde(default)]`）。
- 校验：`runner.port != 0`；`runner.port != serve.port`（防两个 surface 混用端口）；`runner.bind` 非空且必须 loopback（`127.0.0.0/8` / `::1` / `localhost`）——**PR1 无 token，不开放 LAN**。
- `POST /instance` 重写 `live.json` 时新 `runner` 段随之下发（预期加法）。

### A.9.4 Runner 只读 API（PR1 最小面）

两者仅 `Surface::Runner` 可达；Studio 请求被 gate 直接 404。

`GET /runner/health`：

```json
{ "ok": true, "surface": "runner", "version": "0.1.0", "port": 8789,
  "scheduler_mode": "loop", "tasks_dir": "...", "runs_dir": "...",
  "state_path": ".../scheduler.state.json", "lock": { "held": false } }
```

- `port` = 实际绑定端口；`scheduler_mode` 只读 `scheduler.json.config.mode`（`not_configured | invalid_config | unreadable | unknown | <mode>`），不取锁/不写文件；`lock` 复用 `schedule::lock_held_by_live_process`（与守护进程同一把 `<state>.lock`，`pid` 仅 `held` 时出现）。

`GET /runner/tasks`（扫描 tasks-dir 直接子目录，按任务名排序）：

```json
{ "ok": true, "dir": "<tasks-dir>", "tasks": [ {
  "task": "返回庭院", "has_meta": true, "has_flow": true, "has_assets": false,
  "params": ["…"], "version": 1, "unmanaged": true } ] }
```

- `has_assets` = `assets.json` 或 `images/` 任一存在；`version` 仅 meta 有时出现；`unmanaged:true` 是 PR1 占位（裸目录兼容；**PR5 已接 `installed.json`**，§A.13.4）。
- **坏 `meta.json` 不 500**：该 task `params=[]` + `meta_error` 文本，整体仍 200；任务目录不存在 = 空列表 200。

### A.9.5 UI（`apps/oasd/ui/`）

| 文件 | 状态 | 事实 |
|---|---|---|
| `runner.html` | 新增 | 标题「OAS Runner（使用端）」；**PR1 时严格 5 项 nav**：概览 / 已安装任务 / 队列与调度 / 运行记录 / 设置（**PR5 起 6 项，新增「安装包」**，见 §A.13.6）；徽标 = API / 版本 / 端口 / 调度锁；刷新按钮；33 个 DOM id（PR5 后 55 个） |
| `runner.css` | 新增 | 复用 `style.css` 风格与按钮；≤720px 单列；**无外部 CDN/字体** |
| `runner.js` | 新增 | 唯一 `runnerFetchJson`（**只 GET** + 8s 超时 + 错误归一化）；读 `/runner/health`、`/runner/tasks`、`/scheduler/status`、`/runs/scheduler`、`/config`、`/instance`；全部 `textContent`/`createElement`，**无 `innerHTML`/`eval`/POST**；支持 `#overview/#tasks/#queue/#runs/#settings` 深链；**不含创作/构建 API 与 Studio 脚本**（不加载 `app.js`/`builder.js`/`flow_model.js`/`asset_ui.js`）（PR2 起新增 PUT 配置值 + 配置对话框，见 §A.10.5） |
| `index.html` | 修改 | 删除 nav「调度」按钮与左侧 rail `panel-schedule` 入口；旧调度页 DOM 保留为**兼容占位**（无操作按钮，仅提示「调度/运行请用 Runner（8789）」）；header 加 Runner 入口提示 |
| `app.js` | 修改 | 新增 `RUNNER_BASE_URL='http://127.0.0.1:8789/'` + `openRunnerPage(hash)`；`activatePage('schedule')` 改为**提示并打开 Runner `#runs`**（不再调 `refreshSchedulePage`，避免 Runner-only 404）；任务库卡片删除「调度设置…」、运行记录按钮改为在 Runner 打开；深链 `?reset=1` 改为提示去 Runner；旧调度面板按钮/绑定移除 |
| `ui_pure_test.js` | 修改 | 新增 Studio/Runner 静态契约 + Runner id 审计 + Runner 纯函数行为测试 |

### A.9.6 测试

| 命令 / 套件 | 结果 |
|---|---|
| `cargo test -p oasd` | **240 passed / 0 failed**（基线 229，**+11**） |
| `cargo test --workspace` | **538 passed / 0 failed**（基线 527，**+11**；oasd 240） |
| UI pure test | **1239 checks / 224 referenced ids / 0 dead ids**（基线 1138/227/0，**+101**）；`runner_ui_audit`：**33 ids / 0 dead / 5 nav** |
| `node --check` | `runner.js` / `app.js` / `ui_pure_test.js` 全过 |
| `git diff --check` | exit 0 |

新增 11 条 Rust 测试覆盖：归属表（Studio-only/Runner-only/共享/静态/未知）、两端隔离 404、Studio 创作路由非 404、Runner health/tasks 形状、坏 meta 不 500、活锁 pid、共享实例与 `/stats`、静态根与两端静态资源隔离、端口优先级（CLI > config > 默认）。

### A.9.7 Lead E2E（真实两端进程，debug exe）

- `oasd studio --port 8788 ...` 与 `oasd runner --port 8789 ...` **同时启动成功**（各自 banner 打印 surface/端口/双入口提示）。
- **Studio 8788**：`GET /` 200（index；无 nav/rail 调度入口）、`/app.js` 200、`/frames` 200、`/task/library` 200、`/task/flow?task=返回庭院` 200、`/scheduler/status` 200（共享读）、`/runs/scheduler` **404**、`/runner/health|tasks` **404**、`POST /task/save_flow` 400（路由存在）。
- **Runner 8789**：`GET /` 200（runner.html；PR1 时 5 nav，**PR5 起 6 nav**）、`/runner.js|css` 200、`/style.css` 200、`/app.js` **404**、`/frames` **404**、`/task/library` **404**、`/scheduler/status` 200、`/runs/scheduler` 200、`/instance` 200、`/stats` 200。
- `/runner/health` 返回 `{ok:true,surface:"runner",port:8789,tasks_dir,runs_dir,state_path,scheduler_mode:"loop",lock:{held:false},version:"0.1.0"}`；`/runner/tasks` 返回本机 3 个任务（`unmanaged:true`）。
- 截图：`_verify/runner-studio-pr1-20261104/`（`runner-overview/tasks/queue/runs/settings.png`、`studio-nav.png`、`runner-8789.png` 等）。

### A.9.8 Runner-only 发行包（PR1）

- 产物：`dist/oasd-runner-v0.1.0-win64.zip` = **37,141,295 B** + 同名 `.sha256` + `dist/BUILD-INFO.txt`；**用重建后的 PR1 release 二进制打包**。
- zip 内 **23 entries / 0 反斜杠 entry**；解压 23 entries 验证 OK；包内只有 Runner（`oasd.exe`、`ui/runner.{html,js,css}` + `style.css`、`models`、`adb`、`config/live.example.json`+`live.json`、`start-runner.bat`/`stop-runner.bat`、`README-RUNNER.txt`、空 `packages/tasks/runs/logs/assets`、空 v4 `scheduler.json`、`VERSION`/`BUILD-INFO.txt`）；**无** `index/app/builder/flow_model/asset_ui/dashboard`，**无** `frames/`/`library/`。
- 启动脚本：新 `packaging/start-runner.bat`（8789）/`start-studio.bat`（8788）/`stop-{runner,studio}.bat`；`start-editor.bat` 兼容转调 `start-studio.bat`。
- **阻塞已解除**：release exe 已用 PR1 代码重建，`runner` 子命令存在。（`oasd runner --help` 会被 serve parser 当未知参数；正式文档不写 help 用法，只写启动命令。）
- Q7：`dist/`、`packages/` 均不入 git。

### A.9.9 已知遗留（PR1 不做，已分期）

1. **Studio 旧 `#page-schedule` DOM 与 sched-entry 对话框仍保留**（已不可达，仅兼容占位）⇒ 原计划 **PR2 删除**，**PR2 未覆盖、仍未删**（`ui/index.html:408`、`:595`），顺延到 PR6 收拾。
2. Runner 的**安装/卸载**已由 **PR5 ✅（§A.13）** 实现；**队列编辑/调度启停/临时运行**（PR6）仍未实现；**运行参数配置已在 PR2 实现**（§A.10；Runner「已安装任务 → 配置」表单 + `GET/PUT /runner/tasks/<id>/config`），PR1 的「只读页」约束已被 PR2 取代。
3. **设备租约**（Studio 试运行 vs Runner 调度抢设备）⇒ **PR6**（`config/device.lease`）。
4. `docs/` 旧截图/脚本说明里的 `start-editor.bat` 需逐步改名为 `start-studio.bat`（本轮只保证兼容转调）。
5. `.gitignore` 的 `/config/values.json`、`*.tmp`、`/tasks/*/meta.json.bak` 已由 **PR2** 加入；`/packages/`、`/dist/packages/` 已由 **PR5** 加入（`/dist` 已覆盖产物）。
6. PR1 的 Runner 页面是**只读骨架**；`README-RUNNER.txt` 已在 PR5 更新。**PR2 起「配置」已可用**（§A.10.5），**PR3 起值注入已接入 scheduler/queue**（§A.11），**PR5 起安装/卸载/回滚已可用**（§A.13），只剩调度编辑（PR6）未实现。

### A.9.10 验收清单映射（§D.4）

| 验收组 | PR1 状态 |
|---|---|
| **V1 两端不同入口（1~4）** | ✅（1~3 已验收；4 的共享路由契约已测：Runner `/instance` 200、两端 `/scheduler/status` 200，同一 `device` 可在复跑时核对）：8788/8789 双进程、Studio 无调度 nav、**Runner 6 nav（PR5 起）**、互拿对方路由 404 |
| V2 Studio 发布（5~7） | ✅ PR5（§A.13.8；`POST /task/publish` + 历史 + 409/400） |
| V3 Runner 安装（8~10） | ✅ PR5（§A.13.8；path 模式安装 + 校验失败拒绝 + 卸载守卫 409；依赖自动装与 `installed.json` 由集成/E2E 覆盖） |
| V4 配置（11~12） | ✅ PR2（临时任务 `pr2demo` 验收；Studio 8790 / Runner 8791；见 §A.10.7） |
| V5 调度与执行（13~15） | ⬜ PR6 |
| V6 子任务取父配置（16~18） | ✅ PR4 的 16（§A.12.7）；17 待 PR6、18 由 runtime 单测覆盖 |
| V7 不动用户数据（19） | ✅ PR1~PR5 期间均未提交用户数据（`assets/`/`frames/`/`scheduler.json`/`tasks/` 均未动） |

## A.10 PR2 实现记录（2026-11-04，已验证；已提交 `dfe8776`）

> 基线：分支 `dev`，HEAD = **`da9da9d`**（PR1 + seq 101 docs 已提交）；**PR2 全部改动当时未提交**（后已提交为 `dfe8776`）。范围 = **任务配置模型（schema）+ Runner 运行值存储/API/UI**；**不含** PR3 的优先级注入/类型保真，也不含子任务配置（PR4）。**注：PR3 已在此后完成，类型保真/四层优先级/合并后复检见 §A.11。**
> 证据：`D:\OAS-dev\_verify\runner-studio-pr2-20261104\`（实现/评审/测试报告 + Lead E2E 截图 `studio-config.png` / `runner-config.png`）；docs 收尾见同目录 `DOCS-REPORT.md`。
> 提交边界：`crates/oas-task/src/{meta.rs,lib.rs}`、`apps/oasd/src/{runner_config.rs(新),serve.rs,main.rs}`、`.gitignore`、`apps/oasd/ui/{index.html,app.js,runner.html,runner.js,runner.css,ui_pure_test.js}`；**不含用户数据**（`assets/`/`frames/`/`tasks/`/`scheduler.json`）。

### A.10.1 ParamDecl 扩展（`crates/oas-task/src/meta.rs`）

- `ParamDecl` 新增 **5 个全可选字段**（全部 `Option<…>` + `#[serde(skip_serializing_if = "Option::is_none")]` ⇒ 老 `meta.json` 零破坏、不写入空字段）：

| 字段 | 类型 | JSON 名 | 约束 |
|---|---|---|---|
| `label` | string | `label` | 非空 ≤ **80** 字 |
| `description` | string | `description` | ≤ **500** 字 |
| `enum` | Value[] | **`enum`**（Rust 字段 `enum_values` + `#[serde(rename="enum")]`） | 仅 `string/int/float/number`；非空、元素去重、元素类型匹配 `type`、`default` 必须 ∈ enum |
| `required` | bool | `required` | `required:true` 且 **无 `default`** ⇒ Runner 必须填（有 default 不强制） |
| `group` | string | `group` | 非空 ≤ **80** 字（UI 分组**未渲染**，见 §A.10.8） |

- lint 扩展：上述长度、枚举非空/去重/元素类型、`default ∈ enum`、`required` 语义均进 `lint_issues`；老 meta 不受影响。
- 新增并导出 **`pub fn validate_param_values(params, values) -> Vec<String>`**（`oas-task` lib 导出）：
  - **未知 key 拒绝**，错误信息列出允许的 key；
  - 逐 key 校验 `type`（`int` 必须是整数；`float`/`number` 接受 number；`bool`/`string` 严格）、`min`/`max`、`enum`；
  - **`required == true && default == None` 且缺失**才报「缺少必填参数」；有 default 的 required 可省略；
  - 错误文案带 `params[<i>].<field>` 与 key，供 API 400 逐行回显。
- 注意：`min/max` 仅对 `int/float/number` 有意义；**`enum` 不支持 `bool`**（bool 用默认值/`required` 表达）。

### A.10.2 Runner 运行值存储（`apps/oasd/src/runner_config.rs`，新）

- 从 crate root 挂载：`main.rs:30 mod runner_config;`；`serve.rs:63 use crate::runner_config;`（评审 P1-1 的模块迁移已解决）。
- 位置：**live config 同目录** `config/values.json`（`--config config/live.json` ⇒ `config/values.json`）；**无 `--config` 回落 `CWD/config/values.json`**。
- 结构（`schema_version` 1）：

```json
{ "schema_version": 1, "updated_ms": 1750000000000,
  "tasks": { "pr2demo": { "values": { "target_text": "结界", "retry": 2 },
                          "updated_ms": 1750000000000 } } }
```

- 读：缺失 = 空（`tasks:{}`）；坏 JSON / `schema_version` 不符 = **带路径的 Err**（不静默覆盖）。
- 写：**read-modify-write** + 进程内 `Mutex` + 兄弟 tmp 文件 + `rename` **原子写**；只改目标任务，**保留其它任务 values**；成功后刷新顶层与任务 `updated_ms`。
- 全 `std::path::{Path,PathBuf}` + `join`，无盘符/`cmd`/反斜杠硬编码 ⇒ 跨平台（未来 Android/嵌入壳共用同一文件契约）。
- **无 `.bak` / 无 rev**、**假设单进程单写者**（Studio 不写该文件）——已知限制，见 §A.10.8。

### A.10.3 API（`apps/oasd/src/serve.rs`）

**Studio-only**

- `GET /task/config?task=<name>` → `{ok,task,params:[…]}`；**坏任务名 400 / 任务或 meta 缺失 404 / 坏 meta 400**（带 `meta.json` 路径与原因）。
- `POST /task/config`，body `{task, params:[…]}`：`Meta::from_json` + lint 校验后，用**原始 meta JSON 对象只替换 `params`** 再原子写（保留 `name`/`category`/`template_version`/`version`/`entry`/`exits` 及未知字段）；写前生成 `meta.json.bak`；失败 400 带 `meta.json:params[i].<field>`。
- 路由归属：`/task/config` ∈ **Studio-only**（`route_owner` 的 GET/POST 都已覆盖）；Runner 请求 404。

**Runner-only**

- `GET /runner/tasks/<id>/config` → `{ok,task,params,values,updated_ms:null|<ms>}`；任务或 meta 缺失 404、坏 meta 400、坏 `values.json` 500（带路径）。
- `PUT /runner/tasks/<id>/config`，body `{values:{…}}`：先 `validate_param_values`（**错误 400 + `errors[]` 含 key，不写盘**）→ RMW 写 `config/values.json` → 返回 `{ok,task,params,values,updated_ms}`；404/400/500 语义明确。
- 路由归属：`/runner/tasks/<id>/config` ∈ **Runner-only**（`route_owner` 已覆盖 GET/POST/PUT）；Studio 请求 404。两端互拿对方 config 路由均 404（E2E 已验）。

### A.10.4 Studio UI（`apps/oasd/ui/{index.html,app.js}`）

- 任务库卡片新增「**配置项…**」按钮（位于「打开」与运行记录之间）→ `#task-config-dialog`：标题「任务配置项 · <task>」、`#task-config-json` JSON 数组编辑器（2 空格缩进）、`#task-config-status` / `#task-config-error`、保存/取消/**Esc** 关闭。
- 流程：GET `/task/config` 填充 → 用户改 JSON → 保存时**本地 `JSON.parse` 失败不发请求** → POST `/task/config` → 服务端 `errors[]` 逐行显示 → 成功关闭并刷新任务库。
- 纯函数（新增并注册进 `ui_pure_test.js`）：`taskConfigParamsToJson` / `parseTaskConfigParamsText` / `taskConfigSummary`（「N 项配置：key(type)…」）。
- **Studio 只写 schema（`meta.json`），不写 `config/values.json`**。

### A.10.5 Runner UI（`apps/oasd/ui/{runner.html,runner.js,runner.css}`）

- 「已安装任务」每行新增「**配置**」按钮 → `#runner-config-dialog` + `#runner-config-form` 动态表单：

| `type` | 控件 |
|---|---|
| `int` / `float` / `number` | `input[type=number]`（`int` 步进 1） |
| `bool` | `checkbox` |
| `string` + `enum` | `select`（选项来自 enum） |
| `string` | `text` |

- 注：**数字类型的 `enum` 仍渲染 number input**（仅 hint 提示可选值），只有字符串 enum 变 `select`（P2，见 §A.10.8）。
- 提示：`label` 作字段名、`description` 作说明、`required` 标「必填」、`min`/`max`/`enum` 进提示与约束；初始值 = **`values[key] ?? default`**。
- 保存 PUT `{values}`：400 按 `errors[]` 逐行（含 key）、404/读取失败/保存失败分别提示；成功关闭并刷新列表。纯函数：`runnerConfigFieldKind` / `runnerConfigInitialValue` / `runnerConfigCollectValues` / `runnerConfigUrl`。
- 全 `textContent`/`createElement`，无 `innerHTML`/`eval`/外部 CDN；**`group` 未做分组渲染**（已知 P2）。

### A.10.6 测试（Lead 实测）

| 命令 / 套件 | 结果 |
|---|---|
| `cargo test -p oasd` | **255 passed / 0 failed**（PR1 240，**+15**：runner_config 7 + serve 8） |
| `cargo test -p oas-task` | **72 passed / 0 failed**（PR1 63，**+9**：`meta` 字段解析/校验/`validate_param_values`） |
| `cargo test --workspace` | **562 passed / 0 failed**（PR1 538，**+24** = meta 9 + runner_config 7 + serve 8；runtime 142 不变） |
| `cargo build -p oasd --features ocr` | exit 0（**6 warnings**，与基线一致） |
| UI pure test | **1338 checks / 231 referenced ids / 0 dead ids**（PR1 1239/224/0，**+99**；`runner_ui_audit` **41 ids / 0 dead / 5 nav**） |
| `node --check` | 3/3 通过；`git diff --check` clean |

新增覆盖：ParamDecl 新字段解析/lint/序列化、`values.json` 缺失/坏文件/保留其它任务/原子写、`validate_param_values`（未知 key/类型/min-max/enum/required）、Studio GET/POST、Runner GET/PUT、两端 404、坏 meta/坏 values 错误路径。

### A.10.7 Lead E2E（临时 tasks/config，不碰用户数据）

- 环境：临时任务 `pr2demo`（`params`：`target_text` string required + `retry` int 0..5）；临时 live config；**Studio 8790 / Runner 8791** 同时启动。
- **Studio**：`GET/POST /task/config` 200；POST 后 `meta.json` 的 `name`/`category`/`template_version`/`version`/`entry`/`exits` 等字段保留、仅 `params` 更新；中文 `label`/值往返正常；坏 params 400 带 `meta.json:params[0].type`；写前生成 `meta.json.bak`。
- **Runner**：GET 配置 200（params + `values:{}`）；PUT 合法 200 且写入 `values.json`（中文 `结界` 往返）；GET 读回一致；**未知 key / 缺 required / 超 max / 类型错均 400 且 `errors` 含 key**；Studio 请求 Runner config 404、Runner 请求 `/task/config` 404。
- **UI（CDP）**：Studio 任务库「配置项…」→ 对话框打开、JSON 含中文 label、status「已加载：2 项配置…」；Runner 已安装任务「配置」→ 表单渲染 text/number 两字段（值 结界/2）→ 填入 庭院/4 保存成功 → status「已保存任务配置：pr2demo（2 项）」→ `values.json` 变为 庭院/4。
- 截图：`_verify/runner-studio-pr2-20261104/studio-config.png`、`runner-config.png`。
- 验收映射：§D.4 **V4（11~12）✅**（临时任务口径；正式任务待用户自己的配置验收）。

### A.10.8 评审与修复（无 P0）

- **P1-1 = 提交范围/模块迁移后的验证**（已解决）：`runner_config` 移到 `apps/oasd/src/runner_config.rs` 并由 `main.rs:30` 挂载；Lead 按当前树重跑 workspace **562/0**，提交时 `main.rs` 与新文件一起入。
- **P2 已修**：`meta` 未用变量警告（`serve.rs`）；Studio 写 meta 前生成 `meta.json.bak`；`.gitignore` 增 `/config/values.json`、`*.tmp`、`/tasks/*/meta.json.bak`。
- **仍保留的已知限制（P2/P3，不阻塞）**：
  1. `GET /task/config` 对坏 meta 返回 **400** ⇒ Studio 不能在坏 schema 上直接修，需先修 `meta.json`。
  2. `POST /task/config` 回写会把 params 对象的 key **按字典序重排**；`min`/`max` 的整数值序列化为 `1.0`（语义等价；测试按 `1.0` 断言）。
  3. `config/values.json` **无 `.bak`/无 rev**；**多进程同时写不保证安全**（Runner 内 Mutex 只覆盖本进程）；values **以任务名为 key**，卸载/改名会留孤儿条目（PR5 清理；同名重装会继承旧值）。
  4. Runner 任务行有 `meta_error` 时「配置」按钮未特判（点开必 400）；UI **迟到响应未守卫**（保存后快速切任务，旧响应可能关闭/污染新对话框）。
  5. `group` 未渲染分组（只存不用）；**数字 `enum` 不是下拉**（number input + hint），只有字符串 enum 是 select；表单不展示未知/保留 key。
  6. ~~**优先级/类型保真未做**：`run.rs` 不读 `values.json`（PR3）；`entry_run_params` 仍字符串化后猜类型。~~ ⇒ **PR3 已修（§A.11）**：`RunOptions.params` 改 `Value`、四层优先级落地、合并后 schema 复检。仅保留 `runner_config` 复制了一份 `write_text_atomic`（与 `serve.rs` 重复，未来抽公共 helper）。
- **文档漂移**：`packaging/README-RUNNER.txt` 第 10-11 行仍写「单任务运行时优先使用这里的值（后续版本会把它接入调度/队列运行）」——**PR3 已接入调度/队列**（`oasd run` 仍不读 values）；下一轮打包时订正（本轮 docs 子进程不改 `packaging/`）。

## A.11 PR3 实现记录（2026-11-04，已验证；已提交）

> 基线：分支 `dev`，HEAD = **`dfe8776`**（PR2 代码/UI + seq 102 docs 已提交）。**PR3 改动 = 4 个 Rust 文件**（`apps/oasd/src/{run.rs,schedule.rs,queue.rs,serve.rs}`，已提交 `c09df25`；seq 103 docs `048dff5`）；**不含** PR4 子任务配置。
> 范围 = **`RunOptions.params` 类型保真 + 四层优先级注入 + 合并后 schema 复检 + CLI 引号逃生口 + 测试 seam**。
> 证据：`D:\OAS-dev\_verify\runner-studio-pr3-20261104\`（`PR3-BACKEND-REPORT.md`、`REVIEW.md`、`FIX-REPORT.md`、`workspace-final.log`、`e2e/`）；docs 收尾见同目录 `DOCS-REPORT.md`。

### A.11.1 类型保真（`RunOptions.params` 改 JSON 值）

- `RunOptions.params: ParamMap`，`pub type ParamMap = BTreeMap<String, serde_json::Value>`（`run.rs:134` / `:139`）。**CLI `--param` 是唯一的文本猜类型点**（`parse_param_value`，`run.rs:285`）：`true`/`false` → bool；有限数字 → number；其余 → string；`NaN`/`inf` 仍为字符串。
- `json_param_value`（`run.rs:296`）是 JSON → runtime `oas_flow::Value` 的**唯一转换入口**：bool/number/string 1:1；`null`/object/array 保持旧行为的 **compact JSON 字符串**（runtime 无对象类型）。
- `schedule.rs` 的 `entry_run_params`（`:3778`）直接 `entry.params.clone()`，**删除**旧的标量字符串化；队列/调度不再「先转字符串再猜回」。
- `run.rs:548-559` 是**执行前唯一合并点**：`meta.params[].default` 先填充，`opts.params`（JSON）逐 key 覆盖，最后只转换一次。

| 来源 | PR3 前 | PR3 后 |
|---|---|---|
| entry `{"code":"007"}` | `"007"` 被猜回 `Num(7.0)`（R7 缺陷） | **`Str("007")`** |
| entry `{"n":3}` / `{"n":3.5}` | `Num(3.0)` / `Num(3.5)` | 同（保持） |
| entry `{"flag":true}` | `Bool(true)` | 同（保持） |
| entry `{"x":null}` / 对象 / 数组 | compact JSON 字符串 | 同（显式兼容旧行为） |
| CLI `--param code=007` | `Num(7.0)` | 同（CLI 文本无类型；要字符串用 §A.11.4 引号） |

### A.11.2 四层优先级（高 → 低：CLI > entry > Runner values > default）

| 入口 | 1（最高） | 2 | 3 | 4（兜底） | 说明 |
|---|---|---|---|---|---|
| `oasd run`（CLI） | `--param` | — | — | `meta.params[].default` | 不读 values（与 PR3 前一致） |
| scheduler task entry | — | `entry.params`（JSON 原类型） | 该 task 的 `config/values.json` | default | `attempt_run_params` 在 `mark_running` **之前**合并（`schedule.rs:1630`） |
| scheduler queue entry | — | 队列 entry `params`（成员共享） | **按被执行成员**查各自 values | default | 每成员用自己的 values，不串值 |
| `oasd queue`（CLI） | — | —（无 entry 层） | 按 bundle 目录名查 values | default | `queue.rs:221-225` |
| Studio `POST /task/run` | body `params` 对象（JSON 原类型） | — | **刻意不注入** | default | Studio 不依赖使用端运行值；非对象 body **400** |
| Runner 临时运行 `/runner/run` | PR6 实现 | | | | **PR6 必须自行把 values 合并到 body params 之下**（`run.rs` 不自动读 values） |

- 显式层在 `RunOptions.params` 内已合并好（schedule：values 铺底、entry 覆盖；queue：仅 values；Studio：仅 body；CLI：仅 `--param`），`run.rs` 只做「显式层 > default」最后一步。
- 测试 seam：`SchedulerOptions.values_path`（`schedule.rs:377`）与 `QueueOptions.values_path`（`queue.rs:44`），默认 `None → runner_config::runner_values_path()`；测试统一 pin 临时路径（hermetic），生产路径无测试分支。
- 坏 `config/values.json` 仍是**全局快速失败**（scheduler/queue exit 2，`schedule.rs:1630` 在写 `running` 前读）；这是既有契约，不是本 PR 新增。

### A.11.3 合并后 schema 复检（评审 P1 修复）

- 位置：`run.rs:560-584`，**合并之后、`Interpreter::with_params`/执行之前**，属于 setup 阶段；失败时**没有任何 step 执行或点击**（测试断言 `step_log` 为空）。
- `meta.params` **为空** → 整段跳过，保持旧行为：旧任务/任意 `--param`/未知 key 仍可注入（兼容）。
- `meta.params` **非空** → 合并后的运行值（`BTreeMap<String, oas_flow::Value>`）经 `checkpoint::value_to_json` 转成 JSON，调用 PR2 的 **`oas_task::validate_param_values`**：未知 key（错误带 **allowed 列表**）、`type`、`min`/`max`、`enum`、`required==true && default==None` 缺失；错误前缀 **`task config:`**，带任务目录与 `params[i].key`。
- 错误路径（错误发生在任何 step/点击前）：
  - CLI `oasd run`：`run failed: task config: …`，**exit 2**；
  - scheduler：单 entry 失败（日志 `scheduler_run_error … task config: …`），`run_scheduler` 仍成功结束、state 的 `running` 清理，本次 pass 其他 entry 照常；
  - `oasd queue`：单任务失败，queue 整体 **exit 1**；
  - 坏 `values.json` 仍**全局 exit 2**（语义未改，见 §A.11.7-⑥）。

### A.11.4 CLI 引号逃生口（字符串保真）

- `parse_param_value`（`run.rs:285-304`）：raw 长度 ≥ 2 且首尾均为 `"` 时按 **JSON 字符串字面量**解析（转义解码）：`"007"` → `Str("007")`、`"a\"b"` → `Str("a\"b")`、`""` → `Str("")`；首尾引号但 JSON 解析失败 → 去掉外层引号后仍作为字符串（不退回数字猜测）。
- **Windows PowerShell 5.1 实际写法**：`--param 'code=\"007\"'`（单引号阻止 PS 解析，`\"` 阻止 Windows argv 剥掉嵌入引号）；`cmd.exe` 可用 `--param code=\"007\"`；POSIX shell 用 `--param 'code="007"'`。若只写 `--param 'code="007"'`，到达进程 argv 会变成 `code=007`（引号被剥掉），仍是数字。
- 未加引号的 `--param code=007` 仍解析为 `Num(7.0)`；对 `type:string` 的任务会在 schema 复检被拒——这正是逃生口存在的意义。

### A.11.5 测试（Lead 实测）

| 项 | 结果 |
|---|---|
| `cargo test -p oasd` | **276 passed / 0 failed**（PR2 255 → PR3 原始 +14 → 评审修复 +7） |
| `cargo test --workspace` | **583 passed / 0 failed**（21 个 test target；PR2 562 → 576 → 583） |
| `cargo test -p oas-task` / runtime | task **72**、runtime **142** 不变 |
| UI pure test | **1338 checks / 231 referenced ids / 0 dead ids**（PR3 无 UI 改动；`runner_ui_audit` 41/0/5） |
| 构建 | `cargo build -p oasd --features ocr` exit 0、release build exit 0（**6 warnings**，与基线一致） |
| 其它 | `node --check` 3/3；`git diff --check` clean |

- PR3 原始 **14 条**：run 4 新 + 2 更新、schedule 4 新、queue 3 新、serve 3 新（类型保真/四层/values seam/Studio 不读 values/`/task/run` 400）。
- 评审修复 **7 条**：run 6（schema 类型错/未知 key 允许列表/required 无 default 缺失/required 有 default 通过/min-max-enum/引号字符串）+ scheduler 1（单 entry 失败不拖垮 scheduler）。
- 关键用例：`entry_params_keep_json_scalar_types_end_to_end`、`four_layer_priority_entry_then_values_then_default`、`runner_values_apply_when_the_entry_has_no_params`、`queue_injects_the_runner_values_layer_and_keeps_json_types`、`studio_run_does_not_inject_the_runner_values_layer`、`cli_quoted_param_forces_a_string_and_passes_the_schema_check`。

### A.11.6 Lead E2E（临时任务 `pr3param` + replay，不碰用户数据）

- 任务参数：`code` string default `def-code`、`n` int default `1`、`flag` bool default `false`、`k` string default `def`；Runner values `{n:9,k:"runner"}`；scheduler entry params `{code:"007",n:3,flag:true}`。
- **scheduler `--once` + replay**：step log 实际为 **`Str("007")`**（字符串保真）、**`Num(3.0)`**（entry 覆盖 values 的 9）、**`Bool(true)`**、**`Str("runner")`**（values 层），`Outcome Done`；`params=4 cli_params=4`。
- **CLI `run`（引号成功路径）**：`--param 'code=\"007\"' --param n=3 --param flag=true` → **`Str("007")`/`Num(3.0)`/`Bool(true)`**、`k=Str("def")`（default；CLI 不读 values）、`Outcome Done`；证据 `e2e/var-esc.out.txt`。
- **CLI 未加引号**：`--param code=007` → **exit 2 + schema 复检错误** `params[0].key "code": "7.0" is not a valid "string" value`（`e2e/cli-bad.out.txt`；`e2e/cli-quoted.out.txt` 记录的是引号被 argv 剥掉的失败尝试）。
- **坏 values**：手改 values 加未知 key `junk` → scheduler **单条失败**（`scheduler_run_error … unknown parameter "junk" (allowed: code, n, flag, k)`）、`passes=1/failures=1`、调度器不崩；恢复 values 后正常（`e2e/sched-bad.out.txt`）。
- 证据：`_verify/runner-studio-pr3-20261104/e2e/`（`sched2.out.txt`、`var-esc.out.txt`、`cli-bad.out.txt`、`sched-bad.out.txt`、`scheduler.report.json`）；评审/修复见同目录 `REVIEW.md` / `FIX-REPORT.md`。
- 验收映射：PR3 无独立 §D.4 V 步骤；V5（调度执行）/V6（子任务取父配置）仍分别待 PR6/PR4；PR3 只保证「值进入运行链路且类型正确」。

### A.11.7 已知限制（P2，写文档，不阻塞）

1. **字符串化的旧配置会变字符串**：旧 `scheduler.json` 里把数字/布尔写成字符串（entry `"3"`/`"true"`）过去会被猜回 number/bool，现在按类型保真保持 `Str("3")`/`Str("true")`。这是 R7 的预期修复；**迁移需改用原生 JSON 类型**（`3`/`true`）。
2. **`--param code=007` 仍是 number 7**：CLI 文本无类型信息；要字符串用 §A.11.4 的引号逃生口。
3. **`cli_params=` 日志字段语义已变**：现在统计**所有显式层**的 key 数（CLI/entry/values/Studio body 合并数），字段名保留；`params=` 仍是合并后总数。不要再用 `cli_params=` 推断 CLI 参数个数。
4. **非标量参数仍是 compact JSON 字符串**：`null`/object/array 经 `json_param_value` 转成字符串注入（runtime 无对象类型）；复检也按该字符串值进行。
5. **PR6 `/runner/run` 必须自己合并 values**：`runner_values_layer` 只被 scheduler/queue 调用，`run.rs` 不自动读 `config/values.json`；临时运行需「body params 覆盖 values 层」并补 E2E。
6. **坏 `values.json` 全局退出、无单任务降级**：scheduler/queue 在首个任务前 exit 2；修好文件后需重启调度。注意「单任务参数不合 schema」（该 entry/task 失败）与「坏 values 文件」（全局退出）是两种不同语义。
7. **Studio UI 参数输入未加**：PR3 只落 `POST /task/run {params}` 后端契约；`ui/index.html`/`app.js` 的输入控件属后续，UI 当前仍不能手填临时参数。
8. **int/float 在 runtime 不可区分**：`oas_flow::Value::Num(f64)` 层面都是数字；schema 复检能区分 `int`/`float`（经 JSON number），但运行期 `param()` 拿到的是 `Num`，无整型/浮点标记。

★ **提交边界（§A.11 / PR3）**：代码 = `apps/oasd/src/{run.rs,schedule.rs,queue.rs,serve.rs}`（4 文件）；docs 本批 8 份左右；**不含**用户数据（`assets/`/`frames/`/`tasks/`/`scheduler.json`）与 `dist/`；父 `dfe8776`，docs 随后（不写死 hash）。

---

## A.12 PR4 实现记录（2026-11-04，已验证；代码未提交）

> 基线：分支 `dev`，HEAD = **`048dff5`**（PR3 代码 `c09df25` + seq 103 docs 已提交）。**PR4 改动 = 7 个 Rust 文件**：`crates/oas-flow/src/lib.rs`、`crates/oas-task/src/lib.rs`、`crates/oas-runtime/src/lib.rs`、`apps/oasd/src/{task.rs,run.rs,checkpoint.rs,schedule.rs}`（Lead 待提交）。
> 范围 = **`call_task.params`/`inherit` + `with_params` 作用域（§C.4 方案 A）+ loader 展开/runtime 栈 + `Flow::call_task_uses` + lint T7 + 评审 P1 处理（T6 误报 / 严格 meta / checkpoint 指纹 / T7 强化；P1-4 `flow_ops` 闸门按计划转 PR5）**；**不含** PR5 的 TaskPackage/manifest `flow_ops` 闸门。
> 证据：`D:\OAS-dev\_verify\runner-studio-pr4-20261104\`（`PR4-BACKEND-REPORT.md`、`REVIEW.md`、`FIX-REPORT.md`、`workspace-fix.log`、`e2e/`）；docs 收尾见同目录 `DOCS-REPORT.md`。

### A.12.1 flow DSL（`oas-flow`）

- `Step::CallTask { task: String, params: BTreeMap<String, Expr>, inherit: CallInherit }`。
  - `inherit` 缺省 **`All`**；JSON 只接受 `"all"` / `"none"`（字符串以外或其它值 → 解析错误，带 `args.inherit` 路径）。
  - `args.params` 必须是 JSON 对象，值必须是**表达式字符串**（不是裸字面量）；`{"task":"任务1","params":{"触发文字":"param('识别关键词')"}}` 合法；旧无参 `call_task` 完全兼容（`params` 为空、`inherit=All`）。
- 新增内部/高级 `Step::WithParams { label, params, defaults, block, body }`，JSON args = `task`（label 字符串，可省略）/ `params`（映射）/ `defaults`（字面值表）/ `block`（临时隐藏的父 key 列表）/ `body`（steps）。
  - **`with_params` 是 loader 的实现细节，不落盘**；解析器接受手写（测试/工具），但作者应只写 `call_task`——手写时 T3 仍要求父 `meta.params` 声明 body 里所有 `param()` key，且**不会**扣除本作用域 `params`/`defaults` 提供的 key（见 §A.12.8-③）。
- `Flow::call_task_uses()` 收集 `CallTaskUse { task, inherit, params: child→Option<parentKey>, computed_params, path }`：
  - `params` 的 `Option<String>` 只在映射值**恰好**是 `param('字面量')` 时给出父 key；
  - `computed_params` 标记“调用了 `param(...)` 但不是 `param('字面量')`”（如 `param('a'+'b')`），供 T7 拒绝；
  - **所有遍历器递归 `body`**：`param_uses` / `assets` / `finish` / `ends_with_finish` / serialize 都能看到 `with_params` 里的 steps；未知 op 列表补齐已实现的 `key`、`restart_app`。

### A.12.2 loader 展开（`oas-task`）

- `expand_call_steps` 对每个 `call_task`：
  1. 先读目标 `flow.json` 并 **`Flow::from_json` 预解析**（坏目标 DSL 在 loader 就以目标名报错，而不是在父 `steps[i].args.body[k]` 炸）；
  2. 读目标 `meta.json` 的 `params[].default`；
  3. 递归展开目标 steps 并 `strip_finish`（深度上限 8 / 环检测不变）；
  4. 在**内存**中包成 `with_params`（不落盘）：

```json
{"op":"with_params","args":{
  "task":"<目标>",
  "params":{"<child>":"<expr>"},
  "defaults":{"<有 default 的 key>":<字面值>},
  "block":["<inherit:none 时目标声明的 key>"],
  "body":[ /* 目标 steps（已 strip finish） */ ]}}
```

- **严格失败（评审 P1-2）**：目标 `meta.json` 缺失 / 不可读 / 坏 JSON / 坏 schema、`default` 不能转成标量、目标 `flow.json` 无法解析 → **loader 硬错误**，错误形如 `call_task \`<目标>\` (called from \`<调用者>\`)`（附具体原因/路径）；**不再 warning 降级**。
- `load_flow_with_warnings` API 保留，但严格化后 **warnings 恒为空**；`oasd run` 仍打印 `warning=…`（防未来回退时被吞）。
- **修复现存缺口**：子任务 `meta.params[].default` 现在生效（旧无参 `call_task` 也会获得子默认）；**根任务 defaults 仍归 `run.rs`**（loader 不碰根）。
- 子任务 `finish` 仍被 strip（父流程出口不变）；`break`/`continue` 语义不变。

### A.12.3 runtime 作用域与恢复（`oas-runtime`）

- `exec_one` 新增 `WithParams` 分支，顺序固定：
  1. `snapshot = self.params.clone()`；
  2. 在**调用方作用域**求值整张 `params` 映射（映射之间不互相引用；求值失败先恢复 snapshot 再返回错误）；
  3. `inherit:"none"` 时按 `block` **临时移除**父同名值（targeted hide，不是整体隔离）；`inherit:"all"` 不做此步；
  4. `defaults` **只补缺**（`!self.params.contains_key(k) && !overrides.contains_key(k)`）；
  5. `overrides` 最后写入（**显式映射 > 同名继承 > 子默认**）；
  6. `exec(body, depth+1, ctl)`；**成功/失败/break/continue/finish 全路径**在返回前 `self.params = snapshot`（嵌套作用域靠快照叠加）；
  7. 结果日志 `subtask '<label>' (N step(s))`（无 label 时 `scope (N step(s))`）。
- `label` 栈：子任务体内缺参错误带 `subtask '<label>'` 上下文；无 label 的旧作用域文案不变。
- 嵌套 A→B→C：C 的映射在 B 的有效作用域上求值；同一父任务多个 `call_task` 调用点各自独立、互不影响。
- 子任务 `finish` 已被 loader strip ⇒ **子任务不能用 finish 改变父流程出口**（既有语义，本期不改）。

### A.12.4 lint T7（`oas-task` + CLI）

- 新 API：`TaskBundle::lint_with_calls(tasks_dir)` / `lint_into_with_calls(_and_assets)`；CLI `oasd task lint --tasks-dir <dir>` 已接线（单任务 `--task` / `--all`）。
- 对原始 flow 的每个 `call_task` 检查：
  - 目标任务存在且 `meta.json` + `flow.json` **可读且可解析**（不可解析 → 明确报“无法解析目标 flow.json: …”）；
  - `params` 的 child key ⊆ 目标 `meta.params[].key`（否则 T7「子任务未声明参数 X」，带路径）；
  - 映射里 `param('key')` 的父 key ⊆ 父任务 `meta.params[].key`（否则 T7「父任务未声明 X」）；
  - **允许常量映射**（无 `param()` 引用）；**计算表达式**如 `param('a'+'b')` 拒绝（无法静态校验父 key）；
  - `inherit` 值合法。
- **T3 语义不变**：lint 仍看未展开 flow，父任务不会被要求声明子任务的 key；T7 与 T3 不重复报错。
- **T7 不查环**（环/深度由 loader 负责）；**serve 的创建/校验 API 尚未接 T7**（当前只有 CLI `task lint`；**PR5 的发布/安装流程已接 T7**，§A.13.3）。

### A.12.5 T6 修复（评审 P1-1）

- `scan_forbidden_keys` 对 `args.params` / `args.defaults` 的 **key 豁免**：参数名恰好是 `code`/`script`/`cmd` 等不再误报“外部代码”。
- 但 `args.params` 的 **value 仍按 `CODE_ESCAPE_PREFIXES` 检测**：`"shell: rm -rf"` / `"python: …"` 仍报 T6（路径 `…args.params.<key>`）；`defaults` 是字面值，只豁免 key、不扫值。
- 测试：`params.code=param('x')` 全绿；同一调用点 value 改成 `"shell: rm -rf"` 仍报 T6；结构键（如 `log.args.code`）行为不变。

### A.12.6 checkpoint 指纹（评审 P1-3）

- `TaskBundle::load_flow_with_fingerprint() -> Result<(Option<Flow>, u64, Vec<String>), String>`：指纹 = **展开后**flow JSON 文本的 **64-bit FNV-1a**（稳定序列化、无随机种子、跨进程一致）；`flow.json` 不存在时指纹 `0`。
- `Checkpoint` 新增 `#[serde(default)] flow_fingerprint: Option<u64>`；`validate_against` 同时校验 **name / steps / 指纹**。
- **旧格式 checkpoint（无字段）安全拒绝**：明确报 `pre-PR4 checkpoint format`，不 panic、不静默接受；需删除旧 `state/checkpoint.json`。
- `run.rs` 启动时一次 `load_flow_with_fingerprint`，`--resume` 用三元组校验；**子任务内容改动（即使目标 step 数不变、只改日志文本）也会让旧 checkpoint 失效**。
- 粒度限制：一个 `call_task` 仍是一个顶层 `with_params` step；子任务中途崩溃会重跑整个块（安全但变粗）；body 变量在重跑时回到 scope 前（见 §A.12.8-②）。

### A.12.7 测试 / E2E / 包

| 项 | 结果 |
|---|---|
| `cargo test --workspace` | **624 passed / 0 failed**（21 targets；PR3 583 → PR4 618 → 评审修复 624） |
| 分 crate | oas-flow **62**（57→62）、oas-task **89**（72→89）、oas-runtime **157 lib + 2 integration**（142→157）、oasd **280**（276→280） |
| UI | pure test 不变 **1338 checks / 231 ids / 0 dead**（`runner_ui_audit` 41/0/5；PR4 无 UI 改动） |
| 构建 | `cargo build -p oasd --features ocr` exit 0、release build exit 0（**6 warnings**）；`git diff --check` clean |
| Runner 包 | 按 PR4 release 重建 `dist/oasd-runner-v0.1.0-win64.zip` = **37,246,079 B**，SHA256 **`07B6D930FA8EBEE29DBAA0F2C594347097916A36EABED073E4FD12D7852E506B`**（仍 Runner-only，不入 git） |

- **Lead E2E（临时任务 + replay，不碰用户数据）**：
  - `parent` → `child`：child params `触发文字` default 子默认、`max_rounds` default 1、`note` default child-note；parent params `识别关键词` default 父默认、`max_rounds` default 5；parent flow `call_task child params {"触发文字":"param('识别关键词')"}`（`inherit` 默认 all）。`--param 识别关键词=结界` → child 内 `Str("结界")`（映射）、`Num(5.0)`（父同名 max_rounds 继承覆盖子默认 1）、`Str("child-note")`（子默认）；with_params 步骤 `subtask 'child' (3 step(s))`；父作用域仍 `结界/5`。
  - `parent_none`（`inherit:"none"`，映射 触发文字，父 `--param max_rounds=9`）→ child `Num(1.0)`（父同名被屏蔽，回落子默认）、父 `Num(9.0)`。
  - `parent_multi` 两次调用不同映射 → child 日志 `甲`、`乙` 互不影响。
  - ASCII `parent_ascii`→`child_ascii`：lint T7 `errors=0`；运行映射 `hello` + 同名继承 `5`。
  - T7 错误：缺任务「call_task 不存在」、子未声明 key、父未声明 key 均报 T7 带路径；中文 param key 触发 META 报错（已知规则，见 §A.12.8-④）。
  - 证据：`_verify/runner-studio-pr4-20261104/e2e/*.out.txt`、`REVIEW.md`、`FIX-REPORT.md`。
- **§D.4 V6 映射**：16（任务2→任务1 取父配置）已由 replay E2E 覆盖；17（Runner 临时运行任务1）依赖 PR6 的 `/runner/run`；18（`inherit:"none"` 后缺参）的隔离/缺参路径由 runtime/lint 单测覆盖，Runner 端到端待 PR6。

### A.12.8 已知限制（写文档，不阻塞）

1. **PR5 必须做包 manifest `engine.flow_ops` 闸门**（**✅ 已在 PR5 实现，§A.13.2**）：老 Runner 的 loader 只读 `args.task`，会**静默忽略** `call_task.params`/`inherit`（少传参数、`inherit:"none"` 不隔离），**不会** unknown-instruction 报错；`with_params` 不落盘所以也不能靠它兜底。PR5 发布/安装从**原始 flow** 的 `call_task.args.{params,inherit}` 推导 `flow_ops` 并拒绝老 Runner（旧稿 §C.4.6 的“老 Runner 解析失败”描述已更正）。
2. **checkpoint 粒度**：子任务中途崩溃仍会重跑整个 `with_params` 块（安全但变粗）；body 变量回滚保持；旧 checkpoint 需删除。
3. **T7 不查环**（loader 环/深度检测负责）；T7 尚未接 serve 的创建/校验 API；`with_params` 是内部实现，手写会被 T3 检查 body 参数 key（作者应只写 `call_task`）。
4. **meta 参数 key 仍须 ASCII 标识符**（PR2 既有规则；任务名可中文）；lint 会对中文 param key 报 META。
5. **子任务 finish 被 strip**：不能用子任务 finish 改变父流程出口（既有语义）。
6. **T6 值检测是前缀启发式**：只识别 `code:/script:/eval:/exec:/python:/shell:/cmd:/powershell:/unsafe:` 前缀；未来若出现合法常量以这些前缀开头需调整列表。
7. **warnings API 严格化后恒为空**：`load_flow_with_warnings` 保留仅为兼容/未来非致命 warning。

★ **提交边界（§A.12 / PR4）**：代码 = 上述 **7 个 Rust 文件**；docs 本批 8 份左右；**不含**用户数据（`assets/`/`frames/`/`tasks/`/`scheduler.json`）与 `dist/`；父 `048dff5`，docs 随后（不写死 hash）。

---

## A.13 PR5 实现记录（2026-11-04，已验证；代码未提交）

> 基线：分支 `dev`，HEAD = **`442f5cb`**（PR4 代码 + seq 104 docs 已提交）。**PR5 改动 = 新增 2 文件 + 修改 14 文件**：新增 `crates/oas-task/src/package.rs`、`apps/oasd/src/package_cmd.rs`；修改 `crates/oas-task/{Cargo.toml,src/lib.rs}`、`apps/oasd/src/{serve.rs,main.rs,config.rs}`、`Cargo.lock`、`.gitignore`、UI 6 文件（`index.html`/`app.js`/`runner.html`/`runner.js`/`runner.css`/`ui_pure_test.js`）、`packaging/README-RUNNER.txt`（Lead 待提交）。
> 范围 = **TaskPackage `.oaspkg` 发布/安装 + `manifest.engine.flow_ops` 闸门**（Phase 1 垂直切片：Studio 发布 → Runner 安装 → 配置 → 跑）；**不含** PR6 的内嵌调度/队列执行/设备租约/`/runner/run`/上传模式/显式纳管/素材 GC。
> 证据：`D:\OAS-dev\_verify\runner-studio-pr5-20261104\`（`PR5-BACKEND-REPORT.md` / `PR5-UI-REPORT.md` / `REVIEW.md`（331 行） / `FIX-REPORT.md` / `e2e/*.out.txt` / 截图 `studio-publish.png`、`studio-publish-result.png`、`runner-packages.png`）。

### A.13.1 包格式 / 核心（`crates/oas-task/src/package.rs`，新）

- 依赖：`zip 0.6.6`（`default-features=false`，仅 `deflate`）+ `sha2 0.10.9`；纯 Rust，无 `time`/`bzip2`/`zstd`；`Cargo.lock` 已更新。
- `.oaspkg` = **zip**，包根固定契约：

```
manifest.json           必需
checksums.sha256        必需（每行 `<sha256>  <relative/path>`，两空格）
task/meta.json          必需
task/flow.json          必需
task/assets.json        可选（legacy 任务素材；统一素材模式下为空）
task/images/aNNN.png    可选
deps/<task_id>/         可选（call_task 目标的递归闭包）
assets/assets.json      可选（被引用全局素材快照）
assets/images/aNNN.png  可选
README.md               可选（人读发布说明）
```

- **只含相对路径、统一 `/`**；写入端拒绝反斜杠/盘符/绝对路径/`.`/`..`/空路径段；**zip-slip 在 `read_package` 与 `extract_to` 两处都校验**。
- **manifest**（`Manifest`，§B.2）：`package_schema=1` / `package_id` / `task_id` / `task_version` / `name` / `created_at_ms` / `tool{version,git?,dirty,surface}` / `source{repo,commit}` / `engine{template_version,min_engine,flow_ops}` / `files[{path,sha256,bytes}]` / `params_schema{own,call_deps[{child,mapping,inherit,child_params}]}` / `dependencies[{task_id,version,included}]` / `asset_refs[{id,kind,sha256}]` / `platform{os,needs_adb,design_resolution?}` / `signature:null`。
- `package_id = pkg-<task_id>-<task_version>-<sha256(checksums.sha256)[:12]>`（**内容摘要**，不是最终 zip hash，避免自指）；发布同时写同名 **`.oaspkg.sha256` sidecar**（`sha256sum -c` 兼容；sidecar 写失败只 warning，不影响包）。
- 公开 API：`build_package` / `read_package`（不落盘全量校验）/ `extract_to` / `verify_checksums` / `validate_for_publish` / `derive_flow_ops(_for_files)` / `unsupported_flow_ops` / `version_at_least`。
- **checksums 集合严格相等**：缺文件、多文件、重复行、`manifest.files` 与 `checksums.sha256` 数量/哈希不一致都拒绝；`manifest.json`/`checksums.sha256` 不得列进 checksums。
- **解压炸弹 / 声明大小欺骗加固（评审 P1-2）**：条目 ≤ **2000**；单条声明 ≤ **256 MiB**；总声明 ≤ **512 MiB**；单条声明/压缩比 > **100:1** 拒绝（<1 MiB 小文件豁免，避免 deflate 头开销误伤）；每个条目经 `Read::take(declared+1)`，声明撒谎读到第 declared+1 字节即拒；**encrypted 条目拒绝**（zip 0.6.6 `PASSWORD_REQUIRED` 映射为明确错误）；`read_package` 与 `extract_to` 两处生效。
- **运行态文件排除**：打包时跳过 `state/`、`.backup`/`*.backup`、`*.clicks.jsonl`、`*.tmp`、`*.bak`；`validate_for_publish` 对**非空 `state/`**、点击日志报错；**空 `state/` 目录仅 warning**（现有 `tasks/返回庭院/state/` 空目录不再阻塞发布，评审偏差修复）。
- **reproducible**（`--reproducible`）：固定 zip 时间戳 + `created_at_ms=0` + 无 wall-clock ⇒ 同输入同 hash；普通模式记录当前时间。
- 素材快照只含 **实际被引用 id**（`flow.validate_assets` 闭包，含 `deps` 递归），不含整个 store；不打包 `library/`、`frames/`、`runs/`。

### A.13.2 `flow_ops` 闸门（设计 §C.4.6，PR5 必做项）

- 实现常量 `SUPPORTED_FLOW_OPS = ["call_task", "call_task.params", "call_task.inherit.none", "with_params"]`（**按语义粒度**拆，不是设计示例里的 2 项；老 Runner 四个都不支持）。
- 发布时**从原始 flow** 推导（`derive_flow_ops`）：扫描 `steps`（递归 `then`/`else`/`body`/`args.body`），对 `call_task` 看 `args.params` 是否非空、`args.inherit=="none"`；手写 `with_params` 也计入（防绕过）；**不扫描**内存展开的 `with_params`（它不落盘）。根 flow + 所有内嵌 `deps/` flow **并集**写入 `manifest.engine.flow_ops`。
- 安装时**从实际打包 flow 重算**并与 manifest **严格相等**（不是“包含即可”）：篡改 manifest 把 `flow_ops` 改空 → **400 `manifest.engine.flow_ops does not match the packaged flows`**。
- 重算结果必须 ⊆ Runner 的 `SUPPORTED_FLOW_OPS`，否则 **400 `unsupported_flow_ops`**，提示升级 Runner。
- `min_engine` 安装时校验：逐段数字比较（`version_at_least`，忽略前导 `v`/后缀，pre-release 不排序）；过高 → **400 `engine_too_old`**。`template_version` 沿用 meta 检查（过高拒绝）。
- `manifest.task_id`/`task_version` 必须与 `task/meta.json` 一致；内嵌依赖版本必须与 `dependencies[].version` 一致；`deps/` 目录集合必须与 `included:true` 集合相等。

### A.13.3 Studio 发布（CLI + HTTP）

- CLI：`oasd studio publish --task <id> [--tasks-dir <dir>] [--assets-dir <dir>] [--out <dir|file>] [--force] [--include-deps] [--reproducible]`。
  - 默认 `out=dist/packages`、`assets-dir=tasks_dir.parent/assets`；`--out` **只允许相对 `dist/` 下**（拒绝绝对路径、盘符、`..`、`dist` 之外），默认文件名 `<task_id>-v<version>.oaspkg`。
  - 成功打印 `published:` / `sha256:` / `package_id:` / `engine_flow_ops:` / `dependencies: N (included: M)` / 逐条 `warning:`；失败 exit 1、参数错误 exit 2。
- HTTP（Studio-only）：`POST /task/publish {task, out?, force?, include_deps?, reproducible?}` → 200 `{ok,task_id,task_version,package_id,path,sha256,warnings[]}`；lint 失败 **400**（逐条 errors）、同名已存在且无 force **409**、其他 400。
- `GET /task/package?task=<id>`（Studio-only）：扫 `dist_dir` 的 `<task>-v*.oaspkg`，返回 `{ok,task,dir,packages[]}`（每行 `file/path/sha256/package_id/task_id/task_version/created_at_ms/flow_ops`；坏包给 `error` 字段仍 200；**版本降序**、同版本按 `created_at_ms`、再按文件名；上限 100）。
- 校验 = 发布前 **lint T1/T3/T4/T5/T6/T7**（发布流程已接 T7；`call_task` 目标必须存在且可解析）+ 运行态排除 + 模板版本 + 素材引用存在；**lint 失败不产半成品**（写 tmp 后 rename；`--force` 才覆盖）。
- `include_deps` 默认 **false**（CLI；UI 默认勾选 true）；`manifest.dependencies` 总是写全，`included` 由是否实际打进 `deps/` 决定。
- 发布成功包含 `package_id`/`engine_flow_ops`/`dependencies`/`warnings`（HTTP 以上字段由 manifest + path/sha 组合，CLI 逐行打印）。

### A.13.4 Runner 安装 / 卸载 / 回滚

- API（全部 Runner-only）：
  - `POST /runner/install {path, force?}`：**Phase 1 仅本机绝对 `.oaspkg` 路径**；相对路径/不存在/非 `.oaspkg` → 400；`upload:true` → 400（上传/流式 defer Phase 5）。成功 200 `{ok,package_id,task_id,version,replaced,backup,warnings,installed_tasks,phase:"path",note}`。
  - `POST /runner/uninstall {task_id, purge_config?, force?}`：成功 200 `{ok,task_id,backup,removed_package,config_purged}`。
  - `POST /runner/packages/activate {task_id? | package_id?, backup?}`：恢复 `installed.json` 记录的最近 backup，或显式 `backup`；成功 200 `{ok,task_id,version,package_id,restored_from,backup}`。
  - `GET /runner/packages`：`{ok,dir,installed_file,packages[],unmanaged[],phase:"path",note}`。
- **安装严格顺序**：① 读包 + 全量校验（zip-slip/大小/checksums）→ ② `flow_ops` 声明闸门（见 §A.13.2）→ ③ staging 解压 + `verify_checksums` → ④ 根 meta/template/task_id/version → ⑤ `deps/` 集合与内嵌版本、非内嵌依赖已安装且版本足够 → ⑥ 组 `tree/`（依赖先、根最后）→ ⑦ **flow_ops 从 tree 重算并严格相等** → ⑧ **lint**（临时 `lint_tasks/`，含已安装非内嵌依赖的 meta/flow 副本）→ ⑨ `dirty_task` 检查（meta mtime 比 installed_at 新 / 裸目录无记录）→ ⑩ 同 id 同 version 重装必须 force → ⑪ 素材合并**计划** → ⑫ 逐个备份旧任务到 `packages/backup/<id>-<oldver>-<ts>/` → ⑬ 原子 rename 提交（失败回滚）→ ⑭ 应用素材合并（失败回滚任务+素材）→ ⑮ `installed.json` 原子写（失败回滚任务/素材）→ ⑯ 清理 staging。**任一步失败回滚任务/素材/installed**。
- **同版本重装必须 `force`**：无 force → **409 `already_installed`**；版本变化（升/降级）正常安装并备份；目标目录缺失视为修复，不需要 force。裸目录（unmanaged）覆盖也需 `force`（否则 409 `dirty_task`）。
- `installed.json`：`{schema_version:1, packages:[{package_id,task_id,version,sha256,installed_at_ms,source{repo,commit,signed},backup,dependencies}]}`；**原子 RMW**；时间 `now_ms().max(now)` 保证不把刚提交的文件误判成 dirty。
- **卸载**：先备份 `tasks/<id>/` 到 `packages/backup/uninstall-<id>-<ts>/`（**不直接删**），再移除 `installed.json` 条目；`purge_config:true` 才删 `config/values.json` 中该任务值。守卫（无 force）：scheduler entry/queue 引用、正在运行、被其他已安装包 `dependencies` 引用 → **409 `uninstall_refused`**，错误逐项列引用方；`force:true` 可绕过。缺失任务/目录 → 404。
- **activate**：有 installed 记录时恢复其最近 backup（或显式 `backup`）；当前任务目录先移到新 backup。**无 installed 记录时**（典型 = 卸载后）自动扫 `backup/` 找**最新且归属校验通过**的备份恢复，重建 registry 条目（`restored_from`）；`task_id` 目录已存在 → 409 `unmanaged_dir`。显式 `backup` 必须 `backup/` 前缀 + 归属 task_id 一致（防跨任务回滚）。
- `GET /runner/tasks` 增字段：`installed` / `package_id` / `installed_at_ms` / `backup` / `unmanaged` / `meta_version`；只枚举 `tasks/*`（installed 有记录但目录丢失不显示，见 §A.13.9）。

### A.13.5 包内素材 vs 全局素材库（P1-1 修复）

- **全新 Runner（`assets/` 只有 `.keep`、无 `assets.json`）**：安装含素材快照的包时，用包内 `assets/assets.json` **初始化 store**（写 `assets.json` + 拷贝图片），再走正常合并；warning `initialized the global asset store at ... from the package snapshot (N asset(s), M collection(s))`。
- 无快照但 `manifest.asset_refs` 非空 → **400 `missing_asset_snapshot`**（包不完整，提示重新发布带快照）；无引用 → 跳过。
- **同 id + 同定义 + 同字节** → 复用；**同 id 定义或字节不同 → 400 `asset_conflicts`**，错误逐项列 `local sha256` / `package sha256`，建议「在 Studio 重命名该素材后重发，或清空 Runner 全局素材库」，并明确「自动改名 `<id>_2` + flow 重写本版不可用，计划后续版本」。
- 包内新 id → 追加（`rev` 从 1 起；图片名分配避开已用 `images/aNNN.png`）；合并后不回写包；初始化/合并失败回滚新增素材与 store。
- 设计稿 §B.4.3 的「自动改名 + 重写安装态 flow」**Phase 1 不实现**（用户接受，Phase 5 再议，见 §A.13.9-②）。

### A.13.6 UI（`apps/oasd/ui/`）

- **Studio**：任务库「发布…」→ `#task-publish-dialog`（标题含任务名；`include_deps` 默认勾选、`force`、可折叠 `out` 输入；GET `/task/package` 历史列表；POST `/task/publish`；成功显示 `path`/`package_id`/`sha256`/warnings；409/400 逐行显示；发布中禁用）。
- **Runner**：nav 由 5 → **6 项**（概览 / 已安装任务 / **安装包** / 队列与调度 / 运行记录 / 设置）；「安装包」页 = 本机绝对路径 + force 安装、已安装包列表（卸载/回滚按钮、安装时间/版本/package_id/backup）、unmanaged 未纳管列表、操作状态行；任务行显示包徽标（`installed vN` / `未纳管`）。
- 全 `textContent`/`createElement`，无 `innerHTML`/`eval`/外部 CDN；**无跨 surface 调用**（Runner 不请求 Studio API，Studio 不请求 Runner API）；平台中立（无盘符/端口硬编码）。
- 示例路径文案为绝对路径（Lead 已修，避免用户误填相对路径）。

### A.13.7 评审与修复（无 P0；3 个 P1）

| # | 问题 | 处理 |
|---|---|---|
| **P1-1** | 全新 Runner（assets 只有 `.keep`）安装含素材包时丢素材快照 | **已修**：初始化 store；Lead E2E 验证空 assets 目录安装后 `assets.json` + 图片落盘、warning 提示（§A.13.5） |
| **P1-2** | zip 解压炸弹 / 声明大小欺骗 | **已修**：≤2000 条目、压缩比 >100:1 拒绝、`take(declared+1)`、512 MiB 总上限、encrypted 拒绝；`read_package`/`extract_to` 两处生效（§A.13.1） |
| **P1-3** | 素材同 id 变化导致 v2 装不上 | **不实现自动改名/flow 重写**（设计 Phase 5）；错误文案可操作化并记入限制；需用户接受（§A.13.5/§A.13.9-②） |

- 其他设计偏差修复：同版本重装强制 force（409）、空 `state/` 仅 warning、CLI `--assets-dir` 派生、`min_engine` 校验、activate 卸载后恢复、publish `out` 限制、activate 备份归属校验（§A.13.4）。
- 已知 P2（不阻塞）：`PackageError::Other` IO 错误 → HTTP 400（应 500）；`manifest.dependencies` 未与 flow `call_task` 闭包交叉校验（tamper-resistance）；安装不查环/深度（runtime `MAX_CALL_DEPTH` 兜底）且不检查任务正在运行；`installed.json` 损坏时生命周期 API 与 `/runner/tasks` 500（fail-closed）。

### A.13.8 测试 / E2E / 包

| 项 | 结果 |
|---|---|
| `cargo test --workspace` | **677 passed / 0 failed**（21 targets；PR4 624 → PR5 **+53** = 原 36 + 修复 17） |
| 分 crate | oas-task **111**（89→111）、oasd **311**（280→311）、oas-flow **62**、oas-runtime **157 lib + 2 integration** |
| 构建 | `cargo build -p oasd --features ocr` exit 0、release build exit 0（**6 warnings**）；`git diff --check` clean |
| UI | pure test **1442 checks / 237 referenced ids / 0 dead ids**（1338/231/0 → +104）；`runner_ui_audit` **55 ids / 0 dead / 6 nav**（原 41/0/5）；`node --check` 3/3（Lead 另修发布历史读 `task_version`、安装页示例绝对路径文案） |
| Runner 包 | 用 PR5 release 重建：`dist/oasd-runner-v0.1.0-win64.zip` = **37,591,694 B**，SHA256 **`823F93CDB634DA56C91421AAEF39255FBB0522ED9F275116DC2891E59D14031E`**（Runner-only；不入 git） |

- **Lead E2E（临时数据，不碰用户数据）**：
  - 发布 `asset_task`（flow `click sel="返回箭头"`，全局 store 快照）→ `.oaspkg` 含 `assets/assets.json`+`assets/images/a001.png`、`manifest.asset_refs=[返回箭头]`、`package_id=pkg-asset_task-1-…`。
  - 全新 Runner（assets 目录只有 `.keep` 等价）安装 → **200**，warning `initialized the global asset store ... (1 asset(s))`；`assets/assets.json` 含返回箭头 item、`a001.png` 落盘；`tasks/asset_task` 存在。
  - 同版本重复安装无 force → **409 `already_installed`**；force → 200 replaced + backup；卸载 → `backup/uninstall-asset_task-…`；activate（无 installed 记录）→ **200 `restored_from=…`**，任务回到 `tasks/`。
  - 负向：篡改 `manifest.engine.flow_ops=[]` → **400 `flow_ops does not match`**；zip-slip `../evil-slip.txt` → **400** 且包外无文件；Studio 上 `/runner/install` **404**、Runner 上 `/task/publish` **404**。
  - UI CDP：Studio 发布对话框（历史列表/成功显示 path/package_id/sha）、Runner「安装包」页安装成功 + 列表 + 卸载/回滚按钮；截图 `_verify/runner-studio-pr5-20261104/{studio-publish,studio-publish-result,runner-packages}.png`。
- **§D.4 验收映射**：V2（5~7）✅、V3（8~10）✅（依赖自动装与 `installed.json` 两条由安装集成测试 + E2E 覆盖；篡改图片 sha 拒绝、卸载被依赖 409 由单测覆盖）；V7 延续（用户数据未提交）。

### A.13.9 已知限制（写文档，不阻塞）

1. **Phase 1 安装仅 path 模式**：无 `upload`/`incoming`/流式（4 MiB JSON body 限制仍适用于请求）；上传、`/runner/run`、调度启停、设备租约 = Phase 5 / PR6。
2. **素材同 id 变化不能升级**：无自动改名 + flow 重写；冲突时 400 并给建议（重命名素材重发 / 清空 Runner 全局素材库）；需用户接受。
3. **卸载不级联删除内嵌依赖**；activate 只回滚任务目录/registry，**不回收/回滚全局素材**；无素材 GC。
4. **`installed.json` 损坏时**包生命周期 API 与 `/runner/tasks` 会 **500**（fail-closed，无自动隔离）；**无跨进程 packages 锁**（单 Runner 串行假设）。
5. **T4 external 集合实现是替换而非并集**（跨旧任务本地素材 + 全局 store 混合模式可能误报；当前仓库无命中）；`/runner/tasks` 只枚举 `tasks/*`（installed 有记录但目录丢失不显示）；**§B.4.5「纳管」未实现**（unmanaged 可被 force 安装覆盖，但没有显式纳管 API）。
6. **publish `out` 限制为相对 `dist/`**；`GET /task/package` 只扫默认 `dist_dir`，自定义 `out` 的历史不可见；`PackageError::Other` 的 IO 错误 → HTTP 400（应 500，已知 P2）；`manifest.dependencies` 未与 flow `call_task` 闭包交叉校验（tamper-resistance P2）；安装不查环/深度（runtime `MAX_CALL_DEPTH` 兜底）且不检查任务正在运行。
7. **`min_engine` 逐段数字比较**（非严格 semver，pre-release 不排序）；Windows 保留名/大小写重名/前导空格 checksums 属 P3 健壮性（fail-closed）。
8. 文档同步：§A.3/§D.4 的 Runner nav 5 页 → **6 页**；`README-RUNNER.txt` 已在打包文件更新（安装可用、force/素材/回滚说明）。

★ **提交边界（§A.13 / PR5）**：代码/UI/打包配置 = `crates/oas-task/{Cargo.toml,src/package.rs(新),src/lib.rs}`、`apps/oasd/src/{package_cmd.rs(新),serve.rs,main.rs,config.rs}`、`Cargo.lock`、`.gitignore`、UI 6 文件、`packaging/README-RUNNER.txt` + docs；**不含**用户数据（`assets/`/`frames/`/`tasks/`/`scheduler.json`/`scheduler.json.bak`）与 `dist/`；父 `442f5cb`，docs 随后（不写死 hash）。

---

# B. TaskPackage（事项 2 的交付物契约）

## B.1 包格式

- 容器：**zip**，后缀 **`.oaspkg`**（仍是 zip；改后缀只为双击不被解压器接管、UI 能识别）。
- 为什么 zip 而不是目录：单文件便于传输/比对 hash；`flate2` 已在锁文件里，可用 `zip` crate（纯 Rust）。
- 为什么不是 tar/自研：Windows 双击即解压；`zip` crate 的 `ZipArchive` 有成熟 entry name API（便于 zip-slip 校验）。
- 目录结构（包根）：

```
manifest.json                 # 必需，包根
checksums.sha256              # 必需，<sha256>  <relative/path>（\n 分隔）
task/
  meta.json                   # 必需（含 meta.params schema）
  flow.json                   # 必需
  assets.json                 # 可选（legacy 任务素材；unified 模式下应为空）
  images/aNNN.png             # 可选（同上）
deps/<task_id>/               # 可选：递归子任务（call_task 目标）
  meta.json / flow.json / ...
assets/                       # 可选：被引用的全局素材快照
  assets.json
  images/aNNN.png
README.md                     # 可选（人读的发布说明）
```

> **PR5 实现状态（§A.13.1）**：以上结构已按实现落地（`crates/oas-task/src/package.rs`）；条目名一律 `/` 相对路径；打包时排除 `state/`、`*.clicks.jsonl`、`*.tmp`、`*.bak`、`.backup`；空 `state/` 仅 warning。`.oaspkg` 仍是 zip，后缀只影响双击/文件选择器。

## B.2 manifest 字段

```json
{
  "package_schema": 1,
  "package_id": "pkg-<task_id>-<task_version>-<sha256[:12]>",
  "task_id": "每日任务",
  "task_version": 3,
  "name": "每日任务",
  "created_at_ms": 1793000000000,
  "tool": { "version": "0.1.0", "git": "f16fea1", "dirty": false, "surface": "studio" },
  "source": { "repo": "D:/OAS-dev/oas-core", "commit": "f16fea1" },
  "engine": { "template_version": 1, "min_engine": "0.1.0",
              "flow_ops": ["call_task","call_task.params","call_task.inherit.none"] },
  "files": [ { "path": "task/meta.json", "sha256": "…", "bytes": 512 } ],
  "params_schema": {
    "own": [ { "key":"max_rounds","type":"int","default":3,"min":1,"max":99,
               "label":"轮数","description":"每次跑几轮","required":true,"group":"基础" } ],
    "call_deps": [ { "child":"任务1", "mapping": {"trigger_text":"识别关键词"},
                     "inherit": "all", "child_params": ["trigger_text"] } ]
  },
  "dependencies": [ { "task_id":"任务1", "version":2, "included": true } ],
  "asset_refs": [ { "id":"主界面", "kind":"template", "sha256":"…" } ],
  "platform": { "os":"windows-x64", "needs_adb": true, "design_resolution":[1280,720] },
  "signature": null
}
```

字段必要性：
- `task_id` / `task_version` 复用 `meta.task_id`（缺失回退 `name`，`meta.rs:278-281`）与 `meta.version`（`meta.rs:158-159`）——**现有字段，不需要新造**。
- `engine.template_version` 对应 `meta.template_version` + `TEMPLATE_VERSION`（`meta.rs:27,356-372`）；**`flow_ops` 是调用语义的兼容闸门**（列表按实现粒度：`call_task` / `call_task.params` / `call_task.inherit.none` / `with_params`；包里声明了老 Runner 不支持的 op 就拒绝安装，§A.13.2）。
- **实现订正（PR5）**：`engine.min_engine` 是安装闸门（逐段数字比较，过高 → `engine_too_old`）；`source` 只有 `repo`/`commit`（无 `dirty`/`signed`，签名状态在 `installed.json.source.signed=false`）；`params_schema.own` 是原始 JSON 数组；`call_deps` 一行对应**原始根 flow** 的一个 `call_task` 调用点。
- `params_schema.own` 是 `meta.params` 的**快照**，不是第二权威（安装后仍以 `tasks/<id>/meta.json` 为准，避免双份 schema 漂移）。
- `dependencies` 是 `call_task` 的静态闭包（深度上限沿用 `MAX_CALL_DEPTH=8`，`lib.rs:707`）。
- `signature: null`：v1 不做签名（§A.7）。

## B.3 发布流程（Studio）— ✅ PR5 已实现（§A.13.3）

1. **入口**：CLI `oasd studio publish --task <id> [--tasks-dir] [--assets-dir] [--out] [--force] [--include-deps] [--reproducible]` + UI「任务库 → 发布…」按钮（`POST /task/publish`）。HTTP 返回 `{path, sha256, package_id, task_id, task_version, warnings[]}`；CLI 额外打印 `engine_flow_ops` 与 `dependencies (included)`。
2. **校验（发布前必须全绿，复用现有能力）**：

   | 检查 | 实现 |
   |---|---|
   | 任务名/ID 合法 | `validate_task_name` + `meta.task_id` |
   | lint T1/T3/T4/T5/T6 **/T7** | `TaskBundle::lint_with_calls`（**发布流程已接 T7**；目标存在+可解析） |
   | `param('k')` 都声明 | T3 |
   | `template_version` ≤ 引擎 | `check_template_version` |
   | 素材引用存在 | T4 + `flow.validate_assets` |
   | `call_task` 目标存在 | **T7**（PR5 发布已接） |
   | 无 `state/`、无 `.clicks.jsonl` | 新增（**非空 state/ 拒绝；空 state/ 仅 warning**） |

3. **产出命名**：`dist/packages/<task_id>-v<task_version>.oaspkg` + 同名 `.oaspkg.sha256` sidecar；已存在同名文件 → **409 / exit 1**（提示 `--force` 或 `meta.version += 1`）。`--out` **只允许相对 `dist/` 下**（默认 `dist/packages`；传目录则拼默认文件名，传 `.oaspkg` 文件则用之；拒绝绝对路径/盘符/`..`）。
4. **产物是否入 git**：**不入**。`/dist`、`/dist/packages/`、`/packages/` 均已 gitignore（`.gitignore:13,15,16`）。发布历史 = `GET /task/package` 扫 `dist_dir`（默认 `dist/packages`，版本降序，最多 100）；自定义 `out` 的历史不在默认目录中（限制）。
5. **发布幂等**：同输入 → 同 `checksums.sha256`；`created_at_ms`/`package_id` 默认含时间，要可复现加 `--reproducible`（固定 zip 时间戳 + `created_at_ms=0`）。**lint 失败不产半成品**（先写 tmp，成功后 rename；失败清理 tmp）。

## B.4 安装 / 更新 / 卸载（Runner）— ✅ PR5 Phase 1 已实现（§A.13.4）

**模块**：`crates/oas-task/src/package.rs`（打包/解包/manifest，纯逻辑）+ `apps/oasd/src/package_cmd.rs`（HTTP 编排、`installed.json`、卸载/回滚、素材合并）。`oasd` 不直接依赖 `zip`。

### B.4.1 安装 API
```
POST /runner/install            # {path:"D:/x.oaspkg", force?:bool}（Phase 1 仅本机绝对路径；upload:true → 400）
  -> {ok:true, package_id, task_id, version, replaced:bool, warnings:[...], backup:"packages/backup/...", installed_tasks:[...]}
POST /runner/uninstall          # {task_id, purge_config?:bool, force?:bool}
POST /runner/packages/activate  # {task_id? | package_id?, backup?}  回滚到记录的最近 backup / 显式 backup
GET  /runner/packages           # installed.json + unmanaged（Phase 1 path 模式）
```

### B.4.2 落盘流程（严格顺序，任一步失败即回滚）
1. 读包（`read_package` 全量校验，**不落盘**）+ 顶层 sha256。
2. `flow_ops` 声明闸门：`unsupported_flow_ops` + `min_engine`（§A.13.2）。
3. 解压到 `packages/staging/<package_id>/`：逐 entry zip-slip/大小/压缩比/encrypted 校验（§A.13.1），**不覆盖任何已有目录**；`verify_checksums` 再校验一次。
4. 校验 `task/meta.json` + `task/flow.json`：`template_version` ≤ 引擎；`manifest.task_id`/`task_version` 与 meta 一致。
5. 依赖：`included:true` → 从 `deps/<task_id>/` 装（目录集合必须与声明相等、内嵌 meta 版本一致）；`included:false` → 目标已安装且 `version >= 声明`，否则 **400 `missing_dependency`** 并列出缺失。
6. 组 `tree/`（依赖先、根最后），**从实际打包 flow 重算 `flow_ops` 并严格等于 manifest**，再 ⊆ `SUPPORTED_FLOW_OPS`。
7. **对临时 tasks-dir 跑 lint**（T1/T3/T4/T5/T6/T7；非内嵌依赖用已安装任务的 meta/flow 副本），失败 400。
8. **dirty 检查**：目标已存在且无 installed 记录（unmanaged）或 `meta.json` mtime 比 `installed_at_ms` 新 → **409 `dirty_task`**（force 可绕过）。
9. **同 id 同 version 重装必须 force** → 否则 **409 `already_installed`**（版本变化可正常升/降级）。
10. 素材合并**计划**（§B.4.3；无快照 + 有引用 → 400，冲突 → 400）。
11. 提交：逐个把旧 `tasks/<id>` 移到 `packages/backup/<id>-<old_ver>-<ts>/`（含 `entry.json` 快照）→ staging `tree/<id>` 原子 rename 到 `tasks/<id>`；失败回滚已提交任务。
12. 应用素材合并（失败回滚任务 + 素材）。
13. 写 `packages/installed.json`（**原子 tmp+rename**；时间 `now_ms().max(now)` 防止新文件被误判 dirty）；失败回滚任务/素材/registry。
14. 清理 staging；返回 warnings（如 initialized store / 空 state / 排除运行态文件）。

`installed.json`：
```json
{ "schema_version": 1,
  "packages": [ { "package_id":"…", "task_id":"每日任务", "version":3, "sha256":"…",
                  "installed_at_ms":0, "source":{"repo":"D:/OAS-dev/oas-core","commit":"…","signed":false},
                  "backup":"backup/每日任务-2-1793…", "dependencies":[] } ] }
```

### B.4.3 包内素材 vs 全局素材库的合并（冲突策略）— Phase 1 实现

现状：全局 store = `assets/assets.json` + `assets/images/aNNN.png`，id 全局唯一、文件名与 id 无关，`rev` 表示版本。
- **全新 Runner（只有 `.keep`、无 `assets.json`）** → 用包内 `assets/assets.json` **初始化 store**，warning 提示 initialized（P1-1 修复）。
- **同 id + 同定义 + 同字节 sha256** → 复用，不改文件；
- **同 id + 不同定义/字节** → **400 `asset_conflicts`**，逐项列 local/package sha256，建议重命名素材重发或清空 Runner 全局素材库；**不自动改名、不改写 flow**（设计稿的 `<id>_2` + flow 重写 = **Phase 5，未实现**，用户接受）。
- **新 id** → 追加（`rev` 从 1 起；分配未占用的 `images/aNNN.png`）；
- 合并后**不回写**包内 `assets/assets.json`（包是只读输入）；失败回滚新增素材与 store。
- 包声明 `asset_refs` 但无 `assets/assets.json` → **400 `missing_asset_snapshot`**（包不完整，重新发布）。

> ⚠ 安装态与编辑态共用 `tasks/`：安装会覆盖本机同名任务。缓解 = 安装前 **`dirty_task` 409 + force**（§B.4.2-8），旧任务先备份到 `packages/backup/`，不直接删。

### B.4.4 卸载 / 回滚
- **拒绝条件（无 force）**：① 被 `scheduler.json` entry 或队列成员引用；② 正在运行；③ 被其他已安装包 `dependencies` 引用 → **409 `uninstall_refused`**，逐项列出引用方；`force:true` 绕过。
- 执行：`tasks/<id>/` 先移到 `packages/backup/uninstall-<id>-<ts>/`（**不直接删**），再更新 `installed.json`；`purge_config:true` 才删 `config/values.json` 中该任务值（在移动前执行，坏 values 先失败）。
- **activate**：有 installed 记录 → 恢复其最近 backup（或显式 `backup`，必须 `backup/` 前缀 + 归属 task_id 一致）；当前任务先备份。**无 installed 记录时**（如卸载后）→ 自动扫 `backup/` 恢复最新且归属校验通过的备份，重建 registry 条目并返回 `restored_from`；`tasks/<id>` 已存在 → 409 `unmanaged_dir`。

### B.4.5 裸目录兼容（重要）
Runner 首次上线时用户手上只有 `tasks/`（无包）⇒ **Runner 能直接识别未安装的 `tasks/<id>/`**（`unmanaged:true`，`GET /runner/tasks` 标注 `installed/unmanaged`）。裸目录可被安装包 **force 覆盖**（先备份），但**§B.4.5 的显式「纳管」API 尚未实现**（Phase 1 只读展示；用户需用发布/安装流程接入管理）。`GET /runner/tasks` = `tasks/*` + `installed.json` 标注；installed 有记录但目录丢失不显示（限制）。

## B.5 包内依赖（call_task 目标怎么打）

推荐 **C（声明优先，缺失即拒，可选内嵌）**：

| 策略 | 行为 | 问题 |
|---|---|---|
| A 递归全打 | 所有 `call_task` 目标都进 `deps/` | 包体积不可控；同名任务重复；循环依赖要靠 `MAX_CALL_DEPTH` 兜 |
| B 只写引用 | 包只含自己 + 依赖列表 | 安装常失败（对方没装），体验差 |
| **C 声明 + 可选内嵌（推荐）** | `dependencies` 总是写全；`--include-deps` 决定是否把目标放进 `deps/`；装包时「内嵌优先，否则要求已安装足够版本」 | 需写清「内嵌版本会覆盖本机版本」的提示 |

全局素材引用：**快照进包**（`assets/`）而不是「只写 id」。理由：T4 只校验本机存在，换机安装时 id 可能不存在 ⇒ 包必须自带被引用素材。快照只含**实际被引用**的 id（`flow.validate_assets` 的闭包，含 `deps` 递归），不含整个 store。

> **PR5 实现**：`--include-deps` 默认 false（CLI）/ UI 默认勾选；`dependencies` 总是写全。内嵌依赖安装前校验目录集合与 meta 版本；非内嵌依赖要求已安装且 `version >= 声明`。**安装不查环/深度**（依赖打包时按 `MAX_CALL_DEPTH` 递归，运行期由 `MAX_CALL_DEPTH` 兜底）。

## B.6 最小实现切分（Phase 1 垂直切片）— ✅ 已实现，后续项见 defer 清单

**Phase 1（能跑通「Studio 发布 → Runner 安装 → 配置 → 跑」）— ✅ PR5 完成**：
- 新增 `crates/oas-task/src/package.rs`：`Manifest` / `build_package(dir, out)` / `read_package(zip)` / `extract_to` / `verify_checksums` / `validate_for_publish` / `derive_flow_ops`。
- 新增 `apps/oasd/src/package_cmd.rs`：`oasd studio publish` + Studio HTTP publish + Runner install/uninstall/activate。
- `serve.rs`：`Surface` 字段 + `required_surface` + PR5 **6 条新路由**（Studio `POST /task/publish`、`GET /task/package`；Runner `GET /runner/packages`、`POST /runner/install`（**仅 path 模式**）、`POST /runner/uninstall`、`POST /runner/packages/activate`）+ `/runner/tasks` 新字段。
- `ui/runner.html` + `ui/runner.js`（任务列表、参数表单、**安装包页**、健康）。
- `config.rs`：`runner: {port: 8789}`（`#[serde(default)]`，老 `live.json` 不炸）。

**Phase 2**：`with_params` 作用域 + `call_task.params` + lint T7（§C.4）— ✅ PR4。
**Phase 3**：配置优先级 + `config/values.json` + 四入口统一（§C.3）— ✅ PR2/PR3。
**Phase 4**：内嵌调度线程 + 健康/启停 + 设备租约 — PR6。
**Phase 5（PR5 defer 清单）**：上传模式（`upload`/incoming/streaming）、`/runner/run` 临时运行、调度启停、设备租约、**素材同 id 自动改名 `<id>_2` + flow 重写**、**显式「纳管」API**、素材 GC、`installed.json` 损坏自动隔离、跨进程 packages 锁、`dependencies` 与 flow 闭包交叉校验（tamper）。

> Phase 1 的实现就是「从 path 解压 + 全量校验 + 覆盖 tasks/ + 备份 + 合并素材」，**不做上传**（4 MiB 限制与流式改造推迟到 Phase 5）——切片已按计划收口。

---

# C. 任务配置（运行期参数）+ 子任务配置

## C.1 配置模型：`meta.params` 够用，扩展 5 个可选字段（✅ PR2 已实现）

现状（`meta.rs:45-58`）：`{key, type, default?, min?, max?}`，`type ∈ {int,float,number,bool,string}`（`meta.rs:38-39`），lint 校验重复/类型/default 类型/min≤max（`meta.rs:433-470`），T3 要求 flow 里 `param('k')` 都已声明（`lib.rs:2146-2170`）。

扩展（全部 `Option + skip_serializing_if`，老 `meta.json` 零破坏）——**PR2 已实现，长度上限以实现为准（下表已按实现订正）**：

| 字段 | 类型 | 用途 | 实现约束（PR2） |
|---|---|---|---|
| `label` | string | Runner 表单显示名（中文） | 非空 ≤ **80** 字（设计稿早期写 40，实现放宽到 80） |
| `description` | string | 一行说明/示例（「识别到该文字时触发」） | ≤ **500** 字（设计稿早期写 200） |
| `enum` | Value[] | 下拉选项（`type` 为 `string`/`int`/`float`/`number`；**`bool` 不支持**） | 非空、去重、元素类型须匹配 `type`、`default` ∈ enum |
| `required` | bool | Runner 必须填（无默认值时强制） | `required:true && default==None` 才强制；有 default 可省略 |
| `group` | string | 表单分组（基础/高级） | 非空 ≤ **80** 字（设计稿早期写 20；UI 分组**尚未渲染**） |

**运行值校验已落地（PR2）**：`meta.rs` 新增并导出 `pub fn validate_param_values(params, values)`——未知 key 拒绝并给允许列表、按 `type`/`min`/`max`/`enum` 校验、`required==true && default==None` 缺失才报「缺少必填参数」；`min/max` 只对数值类型生效。写入 API（§C.2）调用它，非法值 400 且文案含 key。

**Studio 是 schema 的唯一编辑者**：`GET/POST /task/config`（**PR2 已实现**）——`POST` 用原始 `meta.json` 对象只替换 `params`（保留其它/未知字段），`Meta::from_json` + lint 通过后原子写并生成 `meta.json.bak`。已有先例：`POST /task/goal` 会自动补 `params.max_battles`（`serve.rs`），说明「程序化维护 params 数组」是被接受的模式。

**Studio 是 schema 的唯一编辑者**：`POST /task/config {task, params:[...]}`（新）→ 写 `meta.json` 的 `params` 数组，走 `Meta::from_json` + `lint_issues` 校验后原子写。已有先例：`POST /task/goal` 会自动补 `params.max_battles`（`serve.rs:4396-4420`），说明「程序化维护 params 数组」是被接受的模式。

## C.2 Runner 侧配置值存储（✅ PR2 已实现）

- 文件：**`config/values.json`**（Runner 独占写，Studio 不写）
- 位置 = **live config 同目录**（`--config config/live.json` ⇒ `config/values.json`）；无 `--config` 回落 `CWD/config/values.json`。
- 实际结构（PR2，`schema_version` 1，**尚无 `slots`**）：

```json
{ "schema_version": 1,
  "updated_ms": 0,
  "tasks": {
    "每日任务": { "values": { "max_rounds": 5, "识别关键词": "突破" },
                  "updated_ms": 0 }
  } }
```

- `values` = 任务级运行值（对应「任务1 单独运行时用任务1 的配置」）；`updated_ms` 顶层 + 每任务各一份。
- `slots` = **按调度 entry 覆盖**（可选，Phase 3；等价于今天把值写在 `Entry.params`）——**仍未实现**，留待后续；`Entry.params` 本身不动。**PR3 起 values 已在 scheduler/queue 注入**（§C.3/§A.11.2）。
- 读：缺失 = 空配置；坏 JSON / `schema_version` 不符 = **带路径的 Err**（不静默重置）。
- 并发写：单写者（Runner 进程内 `Mutex` + 兄弟 tmp/rename **原子写** + read-modify-write 保留其它任务）。**Studio 不写此文件**；多进程同时写不保证安全（已知限制）。
- 与 `Entry.params` 的关系：**不是替代**，是「任务默认值 / entry 覆盖值」两层；两者都不在 `scheduler.json` 之外产生第三份真值。**PR3 起实际注入顺序 = entry > values > default**（§C.3；Studio 试运行不读 values）。

## C.3 运行期解析优先级（四入口统一；✅ PR3 已实现，含合并后复检）

> **PR3 已实现并提交（2026-11-04，`c09df25` + `048dff5`）**：`RunOptions.params` 改 `BTreeMap<String, serde_json::Value>`；scheduler/queue 注入 Runner values；`run.rs` 在唯一合并点之后按 `meta.params` 做 schema 复检；`entry_run_params` 不再字符串化。实现细节/测试/E2E 见 **§A.11**；台账 **`PENDING-LEDGER §H`**；规格 **`ENGINE-DESIGN §22.46`**。

**根任务（standalone / queue / scheduler / 临时运行）解析顺序（高 → 低）**：

| 优先级 | 来源 | 状态（PR3 后） | 实现 |
|---|---|---|---|
| 1 | CLI `--param k=v` 显式覆盖 | ✅ 已有；PR3 起文本猜类型仅此一处 + 引号字符串逃生口 | `run.rs:285`（解析）/ `run.rs:548-559`（合并） |
| 2 | 调度 entry `params`（含队列 entry，成员共享） | ✅ 已有；PR3 起直接 clone JSON（不再字符串化） | `schedule.rs:3761/3778` |
| 3 | **Runner 任务配置**（`config/values.json`） | ✅ **PR3 已注入**：scheduler/queue 侧读取并铺底 | `run::runner_values_layer`；`schedule.rs:3766`、`queue.rs:225` |
| 4 | `meta.params[].default` | ✅ 已有 | `run.rs:548-559` |

四种入口映射（全部落到同一个 `RunOptions.params` 合并点 `run.rs:548-559`）：

| 入口 | 传参路径 | 实际图层（PR3） |
|---|---|---|
| 单独运行（CLI） | `oasd run --task 任务1 --param k=v` | **1 + 4**（不读 values） |
| 临时运行（Studio UI） | `POST /task/run {task, params:{}}`（后端契约已有；UI 输入未加） | **1 + 4**（**刻意不注入 values**；body 非对象 400） |
| 临时运行（Runner UI） | `POST /runner/run {task, params:{}}`（**PR6**） | PR6 必须自己实现「body params 覆盖 values 层」 |
| 队列 | `scheduler.json` 的 queue entry `params` | **2（共享 entry）+ 3（按成员）+ 4**；每成员用自己的 values |
| 调度 | entry `params` | **2 + 3 + 4** |
| `oasd queue` | CLI 直接跑队列定义 | **3 + 4**（无 entry params 层） |

**评审发现的两个现存问题——PR3 均已修**：

1. **类型丢失（R7）**：旧 `entry_run_params` 先把 JSON 标量转成字符串，`run.rs` 再 `parse_param_value` 猜回，于是 entry `{"code":"007"}` 变成 `Num(7.0)`。**PR3 修法**：`RunOptions.params` 改 `Value`，entry/values 直接传 JSON，只有 `--param` 保留文本解析；端到端 E2E 已证 `Str("007")`（§A.11.6）。
2. **运行值不校验 schema**：PR2 只在写入 API 校验；PR3 在 `run.rs:560-584` 对**合并后的实际运行值**做一次防御性复检（`meta.params` 为空时跳过，保持旧任务兼容）。未知 key/type/min/max/enum/required 在**任何 step/点击前**失败；CLI exit 2、scheduler 单 entry 失败、queue exit 1（§A.11.3）。

**已知边界（P2，详见 §A.11.7）**：坏 `values.json` 仍全局 exit 2（无单任务降级）；旧配置里字符串化的数字/布尔需迁移为原生 JSON 类型；`--param code=007` 仍是 number（要字符串用引号）；`cli_params=` 现统计所有显式层；非标量参数仍为 compact JSON 字符串；PR6 `/runner/run` 必须自己合并 values；Studio UI 参数输入未加；int/float 在 runtime `Num(f64)` 不可区分。

## C.4 子任务配置语义（核心）✅ PR4 已实现

> **✅ PR4 已实现并提交（2026-11-04，HEAD `442f5cb`；§A.12 为现行实现记录）**：本节方案 A 已落地。与设计稿的差异：① 子任务 `meta.json` 缺失/坏 JSON/坏 default、目标 flow 无法解析 = **loader 硬错误**（不再 warning 降级）；② T7 **允许常量映射**（无 `param()` 引用），计算表达式 `param('a'+'b')` 拒绝；③ `with_params` 是内部 op（不落盘），手写受 T3 限制；④ checkpoint 增加展开后指纹（子任务改动使旧断点失效）。**§C.4.1 是 PR4 前基线**，现行语义以 §A.12 为准；本节保留设计论证。

### C.4.1 现状（PR4 前基线，必须先看清；PR4 已改变以下事实）
- `call_task` 只是 `{"op":"call_task","args":{"task":"任务1"}}`，解析时**只读 `task`**，其他 args 被静默忽略（`oas-flow` `lib.rs:2004-2006`；「unknown args are ignored」有测试注释 `lib.rs:2532-2533`）。
- loader 在 `load_flow` 时把目标 `flow.json` 的 steps **展开**进父 flow，并 `strip_finish_steps`（`oas-task` `lib.rs:709-718,745-760`），深度上限 8、环检测（`lib.rs:707,726-744`）。
- runtime **完全不感知** `call_task`；万一漏网就报错（`oas-runtime` `lib.rs:3376-3379`）。
- 展开后**子任务 steps 与父任务共享同一个 params 命名空间**（`Interpreter.params`，`oas-runtime` `lib.rs:978`；`param()` 直接查这一张表 `lib.rs:3524-3531`）。
- **子任务 `meta.params` 的 default 不参与运行**（**PR4 前基线；PR4 已修，见 §A.12.2**）：`run.rs` 只加载**根** bundle 的 meta 取默认值（`run.rs:464-476`）⇒ 子任务声明 `param('触发文字')` 但父任务没声明/没传 → 运行时报 `param('触发文字') has no value`。
- lint 用**未展开**的 flow（`lib.rs:2110`，`load_flow_raw` 另有入口 `lib.rs:934-948`），所以 T3 只检查父任务自己的 `param()`；`call_task` 是**叶子**，不收集 param/asset/finish（`oas-flow` `lib.rs:2313,2401`）；`ends_with_finish` 也不认 call_task（`lib.rs:2186`）⇒ 父 flow 末尾必须自己 `finish`。
- **没有**跨任务引用校验：`call_task` 指向不存在的任务能过 lint，直到运行加载才失败（`lib.rs:745-749`）。

### C.4.2 方案对比

| 方案 | 做法 | 嵌套/多调用点/不同映射 | 风险 | 结论 |
|---|---|---|---|---|
| **A 作用域 step**（推荐） | loader 展开时生成 `with_params` 包装 step，runtime 用 params 栈 | 天然支持：每个调用点一个作用域块，嵌套靠栈 | 需新增 flow op（**不落盘**；老 Runner 会静默忽略 `params`/`inherit`，见 §C.4.6） | ✅ 采用，配 `engine.flow_ops` 闸门 |
| B 静态改写 | 展开时把子 flow 里 `param('child')` 改写成 `param('parent')` | 多调用点各改一份（可行） | 改的是**表达式字符串**（日志文本里的 `param('…')` 会被误改）；字面量/转义易错；无法「传常量」 | ❌ 不采用 |
| C 仅合并默认值 | 只把子任务 default 灌进命名空间 | 无映射能力 | 无法回答「任务2 换文字再调任务1」 | ❌ 不够 |

### C.4.3 推荐设计（A）：`call_task.params` + `with_params` 作用域

**flow 作者写**（`tasks/任务2/flow.json`）：

```json
{"op":"call_task","args":{
  "task":"任务1",
  "params":{"触发文字":"识别关键词"},   // child_key ← parent_key（父任务 params 里的名字）
  "inherit":"all"                       // 可选，默认 "all"；"none" = 只给映射 + 子默认
}}
```

**loader 展开后生成**（只在内存里，不落盘；**PR4 实际生成字段**）：

```json
{"op":"with_params","args":{
  "task":"任务1",
  "params":{"触发文字":"param('识别关键词')"},
  "defaults":{"max_rounds":3},
  "block":[],
  "body":[ /* 任务1 的 steps（已 strip_finish） */ ]}}
```

- `task` = 目标的 label（用于日志/缺参归属）；`params` 的值是 **Expr 字符串**，在**调用方作用域**求值（所以 T3 会要求父任务声明 `识别关键词`——正确）。
- `defaults` 来自子任务 `meta.params[].default`（只含**有 default** 的 key；用现成 `json_to_param_value` 转成 `oas_flow::Value`），**只在该 key 当前不存在时**填充。
- `block` = `inherit:"none"` 时目标 `meta.params[].key` 全集（runtime 临时隐藏父同名值）；`inherit:"all"` 时 `[]`。
- 生成位置：`expand_call_steps` 在 `call_task` 分支改为 `out.push(with_params_step)`；`deps` 递归天然嵌套（深度上限 8/环检测不动）。**注意**：实际展开前会先 `Flow::from_json` 校验目标 flow，失败直接 loader 硬错误（§A.12.2）。

**runtime 语义**（`oas-runtime` `exec_one` 加一个分支，`lib.rs:2242`）：

```rust
Step::WithParams { params, defaults, body } => {
    let snapshot = self.params.clone();          // 1) 拍快照
    let mut overrides = BTreeMap::new();         // 2) 在「调用方作用域」先求值
    for (k, e) in params { overrides.insert(k.clone(), eval(e, self)?); }
    for (k, v) in defaults {                     // 3) 默认值只补缺
        if !self.params.contains_key(k) && !overrides.contains_key(k) { self.params.insert(k.clone(), v.clone()); }
    }
    for (k, v) in overrides { self.params.insert(k, v); }
    let r = self.exec(body, depth + 1, ctl);     // 4) 跑子任务
    self.params = snapshot;                      // 5) 无论成败都恢复（必须有单测）
    r
}
```

- 嵌套 A→B→C：三层快照叠加，C 的映射在 B 的有效值上求值（`param()` 读到 B 作用域里的值）——符合「子任务用调用方的配置」。
- 同一父任务多调用点传不同值：两个独立 `with_params` 块，互不影响。
- 递归同名任务：靠 `expand_call_steps` 的 `stack` 环检测挡住（`lib.rs:739-744`），不做运行期递归。
- `break/continue/finish` 语义不变：子任务 `finish` 已被 strip（`lib.rs:709-718`）⇒ **子任务无法用 finish 改变父流程出口**（现状，本期不改；写进文档避免误解）。

### C.4.4 未映射 key 的默认行为（推荐）

子作用域内一次 `param(k)` 的解析顺序（高 → 低）：
1. 调用点 `params` 映射（`k` 是本子任务的 key；值来自父作用域的表达式）；
2. **同名继承**：调用方作用域里同名 key 的值（等价于今天的「平铺共享命名空间」——**向后兼容的关键**：现有任务若有同名参数，行为不变）；
3. 子任务 `meta.params[].default`（**PR4 已修**：PR4 前子任务默认值不生效；实现见 §A.12.2）；
4. 都没有 → **响亮失败**，错误信息必须带「子任务名 + key + 提示（在父任务 meta.params 声明或增加 params 映射）」。当前文案只有 `param('{key}') has no value: declare it in meta.params … or pass --param`（`oas-runtime` `lib.rs:3524-3531`），在子任务场景会误导用户去改父任务的 CLI 参数。

冲突规则：显式映射 **覆盖** 同名继承（确定、可解释）；`inherit:"none"` 关闭第 2 步（严格隔离，适合「子任务必须自带配置」）。

> **PR4 实现补充**：`inherit:"none"` 的 `block` = 目标声明 key 全集（逐 key 临时移除、执行后由 snapshot 恢复）；映射值在**调用方作用域**求值且映射 entries 之间不互相引用；缺参错误经 label 栈带 `subtask '<label>'` 上下文（§A.12.3）。

**为什么不默认 `inherit:"none"`**：那会破坏所有现存的「父传子」隐式共享用例（今天 100% 是共享的），且用户已有任务里子任务参数靠父任务同名声明才能跑通，改成 none 会立刻炸一批。

### C.4.5 lint 规则（新增 T7，T3 语义不变）

- **T7（新）`call_task` 校验**：在 `lint_into` 里对**原始 flow** 的每个 `call_task` 步骤：
  - 目标任务存在（`tasks/<task>/meta.json` + `flow.json` 可读）→ 否则「call_task `任务1` 不存在」；
  - `params` 的 key ⊆ 子任务 `meta.params[].key` → 否则「子任务未声明参数 X」；
  - `params` 的 value（父 key）⊆ 父任务 `meta.params[].key` → 否则「父任务未声明 X」（也可交给 T3 的 `param_uses` 收集，二选一；建议 T7 独立报错以便文案精准）；
  - `inherit` 值合法。
- **T3 保持不变**：lint 用未展开 flow（`lib.rs:2110`），父任务里看不见子任务的 `param()` ⇒ **不会**要求父任务声明子任务的 key（否则每加一个子任务就要往父任务抄一遍参数）。
- **需要新增收集器**：`Flow::call_task_uses()`（返回 `{task, inherit, params:{child:Option<parent>}, computed_params, path}`），与 `param_uses_at`（`oas-flow` `lib.rs:2176`）同款实现，放 `oas-flow` 里以便 lint 与 UI 共用。
- ⚠ **代价**：lint 从「单任务自洽」变成「要读兄弟任务」。必须处理「lint 单个任务时兄弟目录不可读」（例如发布包只带自己）：T7 在目标缺失时报 `error`，消息里区分「目标任务不存在」与「无法读取目标（权限/路径）」。
- **PR4 实现订正**：① 目标 flow **必须可解析**（T7 报“无法解析目标 flow.json: …”，不只检查可读）；② **允许常量映射**（值不含 `param()` 引用，如 `"固定文字"`），只有计算 `param('a'+'b')` 被拒；③ T7 **不查环**（loader 负责），不替代 T3；④ 当前接线 = `TaskBundle::lint_with_calls` + CLI `task lint --tasks-dir` + **PR5 的发布/安装流程**；`POST /task/create`、`POST /task/save_flow` 等创建/保存 API 仍未接 T7（限制）。

### C.4.6 运行时 flow op 兼容闸门（✅ PR5 已实现，§A.13.2）

> ⚠ **2026-11-04 订正（PR4 评审 P1-4）**：`with_params` 是 loader 的**内存展开**，**从不落盘**，所以“老 Runner 解析命中 `unknown instruction`”**不是**保护机制。真正的风险是：老 Runner 的旧 loader 只读 `call_task.args.task`，会**静默忽略** `params`/`inherit`，按旧语义继续跑（少传参数、`inherit:"none"` 不隔离）——比解析失败更危险。

**PR5 实现（发布 + 安装双层闸门）**：
- `manifest.engine.flow_ops` 按**语义粒度**记录：`call_task` / `call_task.params` / `call_task.inherit.none` / `with_params`；
- **从原始 flow 的 `call_task.args.{params,inherit}` 推导**（递归 `then`/`else`/`body`/`args.body`；不能扫描 `with_params`，它不在磁盘上），根 flow + 内嵌 `deps` flow 取并集；
- Runner 安装时**从实际打包 flow 重算**并与 manifest **严格相等**（防篡改），且必须 ⊆ `SUPPORTED_FLOW_OPS`，否则 **400 `unsupported_flow_ops`** 提示升级 Runner；`min_engine` 过高 → **400 `engine_too_old`**；
- `meta.json`/`flow.json` 的 `schema_version` **不动**（仍是 1）——op 是加法，不是 schema 变更。

> PR5 的 Lead E2E 已实测：把 manifest 的 `flow_ops` 篡改成 `[]` → 安装 **400 `flow_ops does not match`**（§A.13.8）。

## C.5 Studio / Runner 隔离下的交互流

**边界一句话**：`call_task` **映射**是**创作**（写进父任务 `flow.json`，随包发布）；参数**值**是**运行**（Runner 填，不进包）；子任务的 **schema**（key/类型/默认值/说明）是**子任务作者的创作**，随子任务包发布。

**「任务2 的配置 schema 如何包含子任务需要的 key」**（三选一，推荐 ②+③）：
1. ❌ 自动复制进父 `meta.params`：父 schema 被子任务「污染」，子任务改名/删参数后父任务留下死参数。
2. ✅ **发布时推导 + 手动提升**：发布时分析 call 图，若发现「子任务参数无默认值且父任务未声明、也没有 `params` 映射」→ 报 `warning`，UI 提供一键「提升为父参数」（把子 key 复制进父 `meta.params`，`label` 前缀 `[任务1]`）。**只有用户点了才写**。
3. ✅ **发布时校验为 error**：若父任务 flow 里**明确**映射到父 key，而父 key 未声明 → 直接 error（即 T7）。

**端到端示例（用户可复现）**：
1. **Studio 创作任务1**：`POST /task/create` → `meta.json` 加
   `params:[{"key":"触发文字","type":"string","default":"","label":"触发文字","description":"识别到该文字时触发动作","required":true}]`
   → flow 用 `ocr('主界面')` / `appear` 等现成表达式搭配 `param('触发文字')`（表达式清单见 `oas-flow` `lib.rs:466-522`）。
2. **Studio 创作任务2**：flow 里 `{"op":"call_task","args":{"task":"任务1","params":{"触发文字":"识别关键词"}}}`；`meta.params:[{"key":"识别关键词","type":"string","default":"突破","label":"识别关键词"}]`。
3. **Studio 发布**：`oasd studio publish --task 任务2 --include-deps` → 校验（T1/T3/T7）→ `dist/packages/任务2-v1.oaspkg`（内含 `deps/任务1/`）。
4. **Runner 安装**：UI「安装包」选文件 → `POST /runner/install` → 装 `任务2` + `任务1` + 素材合并 → `installed.json` 两条。
5. **Runner 配置**：`GET /runner/tasks/任务2/config` 返回 schema（`识别关键词`）+ 当前值（`"突破"`）→ 改成 `"结界"` → `PUT` 写 `config/values.json`。
6. **Runner 排队/调度**：`POST /scheduler/config` 加 entry `{entry_id:"e1", task:"任务2", lifecycle:"periodic", …}`。
7. **执行**：Runner 调度线程跑 `任务2` → `RunOptions.params = {识别关键词:"结界"}`（来自 values，优先级 3）→ 跑到 `call_task` → 作用域里 `触发文字 = param('识别关键词') = "结界"` → 任务1 的 flow 读到 `"结界"`。
8. **对照实验（核心）**：Runner 里直接「临时运行 任务1」→ `param('触发文字')` = 任务1 **自己的** values/default（`""`）→ 触发 `required` 提示「未配置」。**这就是「任务1 单独运行时用任务1 的配置」**。

## C.6 兼容 / 迁移

| 变更 | 兼容性 | 处理 |
|---|---|---|
| `meta.params` 新增 `label/description/enum/required/group` | 向前兼容（全部 `Option` + `skip_serializing_if`，与 `meta.rs` 全可选风格一致；缺省 = None） | 老 Studio 读到会忽略（`Meta` 用 serde，**未开 `deny_unknown_fields`**，`meta.rs:149-150`；对比 `OasConfig` 是开着的，`config.rs:23`） |
| `call_task.args.params` | ⚠ 老 loader **静默忽略**（`oas-flow` `lib.rs:2004-2006` 只取 `task`） | **PR5 已做安装闸门**（§A.13.2）：老 Runner `SUPPORTED_FLOW_OPS` 不含 `call_task.params`，安装 400 提示升级；Studio 保存带 `params` 的 `call_task` 时本机不支持仍不提示（限制） |
| 新增 `with_params` op（仅内存） | **不落盘**；老 Runner 根本看不到它，真正风险是**静默忽略** `call_task.params`/`inherit`（§C.4.6） | **✅ PR5 已实现**：`flow_ops` 从**原始 flow** 推导、安装时重算严格相等 + 拒绝老 Runner |
| `RunOptions.params` 改 `BTreeMap<String,Value>` | 内部类型，非磁盘契约 | **✅ PR3 已实现**：`--param` 仍按文本解析（行为不变，另加引号字符串逃生口）；entry/values 直接传 JSON 原类型，不再字符串化 |
| `config/values.json` 新增 | 纯新增文件 | 缺失 = 空配置；带 `schema_version` 便于演进 |
| 旧任务（无 `meta.params`、无 `call_task`） | **完全不变** | `Meta.params` 默认空（`meta.rs:171-172`），`param_defaults` 返回空表（`run.rs:467-472`） |
| `Entry.params` / `--param` | 不变，优先级语义显式化 | 文档 + 单测锁定 |

---

# D. 风险 / 决策 / 分阶段 / 验收

## D.1 风险清单（按严重度）

| # | 风险 | 证据 | 对策 |
|---|---|---|---|
| **R1** | **`call_task` 是 load-time 展开，runtime 无调用点身份**：子任务无法有独立超时/重试/预算/运行记录；日志里子任务步骤与父任务不可区分；改子任务会让父任务 checkpoint 失效（`flow_name + steps 数量` 校验，`run.rs:493-507`） | `oas-task` `lib.rs:1058-1091`、`run.rs:493-507` | **本期接受**（`with_params` 是作用域不是调用栈）；文档写清；若将来要「子任务一等运行单元」→ 独立立项改 runtime 调用栈（大改） |
| **R2** | Studio 试运行与 Runner 调度抢同一台设备 | `README-DEPLOY.md:81`、`SCHEDULER-RUNNER-PLAN.md:37` | 设备租约 §A.6.4（Phase 4）；在此之前至少 Studio 检测到调度锁被占时拒绝 `/task/run` |
| **R3** | 安装改写 `tasks/` 与 Studio 编辑态冲突（同一目录） | §B.4.3、`.gitignore` 未忽略 `tasks/` | **✅ PR5 已实现**：`mtime` dirty 检测 + **409 `dirty_task`** + `force`；安装前备份到 `packages/backup/`；UI 明确「安装将覆盖本机同名任务」 |
| **R4** | 包解压路径穿越 / 压缩炸弹 / 磁盘写满 | `serve.rs:50`（4 MiB） | **✅ PR5 已实现**：zip-slip 双向、≤2000 条目、≤512 MiB 总/≤256 MiB 单条、压缩比 >100:1 拒绝、encrypted 拒绝、`take(declared+1)`、staging 配额、失败清理 |
| **R5** | 无签名 → 用户装到被篡改的包 | `README-DEPLOY.md:83`（未签名 exe 已被接受） | v1 sha256 + `source` 标记 + UI 提示；**PR5 增加 flow_ops 重算闸门**；签名仍 Phase 5 再评估 |
| **R6** | 配置值并发写 / 与 entry params 双份真值 | `schedule.rs:569`（原子写已有） | 单写者 + `values` vs `Entry.params` 语义分层（§C.2） |
| **R7** | `entry_run_params` 字符串化导致类型丢失（`"007"` → `7.0`） | `schedule.rs:3738-3749` + `run.rs:257-265` | **✅ PR3 已修（§C.3 / §A.11.1）**：端到端传 `Value`；旧字符串化配置需迁移（§A.11.7-①） |
| **R8** | lint 变成跨任务读取（T7），单任务 lint 可能因兄弟目录缺失误报 | `lib.rs:2110`、`lib.rs:745-749` | T7 消息区分「不存在 / 不可读」；发布 `--include-deps` 或已装依赖（PR5 已接 T7） |
| **R9** | 4 MiB 上限 + 整包读内存 → 大包上传失败 / OOM | `serve.rs:50,1169-1183` | **PR5 Phase 1 只支持 path 安装**（请求仅传路径，不读包体）；上传流式 = Phase 5 |
| **R10** | 端口/路径配置不一致的现存坑（serve 默认 8787 vs 8788；CLI 与 serve 的 `--runs-dir` 必须一致） | `main.rs:388`、`config.rs:44` | Phase 1 顺手修 8787→8788；`/runner/health` 回显 `tasks_dir`/`runs_dir` 便于比对 |

## D.2 决策点（7 项）★ **2026-11-04 用户已全部批准（按推荐）**

| # | 决策 | 推荐 | 状态 | 理由 |
|---|---|---|---|---|
| **Q1** | Runner 的命令名与端口 | **`oasd runner` + 8789**，仍只绑 `127.0.0.1` | **✅ 用户批准（按推荐）**；PR1 已实现 | 与 Studio 8788 相邻好记；不开放 LAN 与 `README-DEPLOY.md:82` 一致；将来要 LAN 再加 `runner.bind` + token |
| **Q2** | `oasd serve` 的去向 | **保留为 deprecated 别名**（内部等价 `studio`），stderr 一行提示；v2 再删 | **✅ 用户批准（按推荐）**；PR1 已实现 | 现有 `start-editor.bat`/文档/用户习惯全指向 `serve`（`README-DEPLOY.md:48`），直接删会造成「升级后启动失败」 |
| **Q3** | 包容器与后缀 | **zip + `.oaspkg`** | **✅ 用户批准（按推荐）；PR5 ✅ 已实现（§A.13）** | 单文件、hash 友好、`flate2` 已在锁文件、Windows 双击可用；后缀区分「这是任务包」而不是「发行包」（发行包是 `oasd-v*-win64.zip`，`scripts/package.ps1:15`） |
| **Q4** | 配置优先级 | **CLI `--param` > entry params > Runner values > `meta.params.default`** | **✅ 用户批准（按推荐）；PR3 ✅ 已实现（§A.11）** | 显式 > 场景 > 常备 > 声明；与现有「CLI wins」注释一致（`run.rs:548-559`） |
| **Q5** | 子任务未映射 key 的默认 | **同名继承 + 子任务 default 兜底**，`inherit:"none"` 可关闭 | **✅ 用户批准（按推荐）；PR4 ✅ 已实现（§A.12）** | 与今天「共享命名空间」完全兼容（`oas-runtime` `lib.rs:978`），并顺手修掉此前「子任务 default 不生效」的缺口（PR4 已修） |
| **Q6** | Runner 是否内嵌调度 | **内嵌线程**（保留 `oasd scheduler` CLI） | **✅ 用户批准（按推荐）**；PR6 实现（`/runner/health` 已随 PR1 提前落地） | 锁天然互斥（`schedule.rs:775-833`）、少一个用户步骤、无需管理员权限；坏处「UI 崩 = 调度停」可用 `oasd scheduler` 兜底 |
| **Q7** | 发布产物是否入版本控制 | **不入**（`dist/`、`packages/` 都忽略） | **✅ 用户批准（按推荐）**；PR1/PR5 落实（Runner 包与 `.oaspkg` 都在 `dist/`/`packages/`，未入 git） | 二进制包会让仓库膨胀；`dist/`/`packages/` 已被忽略（`.gitignore:13,15,16`）；溯源靠 `manifest.source.commit` + `checksums.sha256` |

> **备注（不单列决策，但需知道）**：同任务多 entry 不同参数目前已支持（`Entry.params` 每 entry 独立）；但**队列成员的 per-member 参数不支持**（成员共享队列 entry 的 params，`schedule.rs:1692`）。若需要「队列里每个任务不同配置」，需把 `queues[].members` 从 `Vec<String>` 扩成对象数组（`scheduler.rs:696`）——**建议先不做**，用「每个成员各建一条 task entry」绕开。
> **配套备注（R3/B.4.3）**：安装覆盖本机同名任务时的 `force` / 409 `dirty_task` 策略已写在 §B.4.3，属实现细节，不单列决策。

## D.3 分阶段实施计划（6 个 PR；**PR1 ✅ / PR2 ✅ / PR3 ✅ / PR4 ✅ / PR5 ✅ 已实现（2026-11-04），PR6 待做**；PR6 可延后）

| PR | 范围 | 关键文件 | 交付 / 验证 |
|---|---|---|---|
| **PR1 Surface 拆分 ✅ 已提交（§A.9）** | `Surface` 枚举 + `route_owner`/`required_surface` 表 + `oasd studio`/`runner` 命令（`serve` 兼容别名）+ `ui/runner.*` 只读骨架 + `config.runner.port/bind` + 8787→8788 + Runner-only 发行包 | `serve.rs`、`main.rs`、`config.rs`、`ui/runner.{html,js,css}`、`ui/index.html`、`ui/app.js`、`ui/ui_pure_test.js`、`scripts/package-runner.ps1`、`packaging/*` | ✅ 单测 **240/0（oasd）/ 538/0（workspace）**；`get_route` 两端隔离 404；Lead 真实双进程 8788/8789 E2E；UI **1239/224/0**，`runner.html` nav **5 项（PR1 时；PR5 起 6 项）** |
| **PR2 配置模型 ✅ 已实现（§A.10）** | `meta.params` 5 个可选字段（label/description/enum/required/group）+ `config/values.json` + Studio `GET/POST /task/config` + Runner `GET/PUT /runner/tasks/<id>/config` + Runner 配置表单 + 运行值校验（type/min/max/enum/required） | `crates/oas-task/src/{meta.rs,lib.rs}`、`apps/oasd/src/{runner_config.rs,serve.rs,main.rs}`、`ui/{index.html,app.js,runner.html,runner.js,runner.css,ui_pure_test.js}`、`.gitignore` | ✅ **oasd 255/0、task 72/0、workspace 562/0（+24）；UI 1338/231/0**；`PUT` 原子写 + 保留其它任务；非法值 400 含 key；Lead E2E 临时任务 `pr2demo`（Studio 8790 / Runner 8791，Studio+Runner UI CDP 通过；V4 11~12 收口） |
| **PR3 优先级 + 类型 ✅ 已提交（§A.11）** | `RunOptions.params` 改 `BTreeMap<String,Value>` + 四入口合并 + Runner values 注入（scheduler/queue）+ 合并后 schema 复检 + CLI 引号字符串逃生口 + `values_path` 测试 seam | `apps/oasd/src/{run.rs,schedule.rs,queue.rs,serve.rs}` | ✅ **oasd 276/0、workspace 583/0（+21）**；`{"code":"007"}` 端到端保持 `Str("007")`；entry > values > default 三层矩阵测试；schema 复检在 step 前失败；scheduler 单 entry 失败不崩；Lead E2E（`pr3param` + replay：`Str("007")/Num(3.0)/Bool(true)/Str("runner")`） |
| **PR4 子任务配置 ✅ 已实现（§A.12）** | `call_task.params`/`inherit` + 内部 `with_params` op + loader 严格展开（目标 meta/flow 解析 + 子默认）+ runtime 作用域（快照/求值/block/defaults/恢复）+ `Flow::call_task_uses` + lint T7 + 评审 P1 处理（T6 参数名豁免 / 严格 meta / checkpoint 展开后指纹 / T7 强化；P1-4 `flow_ops` 闸门→PR5） | `crates/oas-flow/src/lib.rs`、`crates/oas-task/src/lib.rs`、`crates/oas-runtime/src/lib.rs`、`apps/oasd/src/{task.rs,run.rs,checkpoint.rs,schedule.rs}` | ✅ **workspace 624/0（21 targets；583→618→624）、flow 62、task 89、runtime 157+2、oasd 280**；单调用 / 多调用不同值 / 嵌套三层 / 未映射走同名 / default 兜底 / `inherit:"none"` / **错误后 params 恢复**；T7 三类错误 + 常量映射；checkpoint 指纹失效；Lead E2E `parent→child`（映射/继承/子默认/inherit none/多调用点/ASCII） |
| **PR5 打包 / 安装 ✅ 已实现（§A.13）** | `.oaspkg` 包核心（zip+manifest+checksums+加固）+ `flow_ops` 闸门 + `oasd studio publish`（CLI/HTTP/历史）+ Runner `install/uninstall/packages/activate` + 素材合并/初始化 + `installed.json` + 备份/回滚 + UI（Studio 发布对话框、Runner 安装包页） | 新 `crates/oas-task/src/package.rs`、`apps/oasd/src/package_cmd.rs`、`serve.rs`/`main.rs`/`config.rs`、`Cargo.lock`、`.gitignore`、UI 6 文件、`packaging/README-RUNNER.txt` | ✅ **workspace 677/0（+53）、oas-task 111、oasd 311、flow 62、runtime 157+2**；UI **1442/237/0**（runner audit **55/0/6**）；E2E：发布→全新 Runner 安装（初始化素材）→同版本 409→force→卸载→activate 恢复；zip-slip/flow_ops 篡改 400；两端路由 404；Runner 包 **37,591,694 B** / SHA `823F93CD…14031E` |
| **PR6（可延后）** | 内嵌调度线程 + 调度启停 + 设备租约 + `POST /runner/run` 临时运行（**自行合并 values**）+ 上传模式/素材自动改名/显式纳管/素材 GC | `schedule.rs`（加 `stop: Arc<AtomicBool>`）、`serve.rs`、`runner.js` | E2E：`runner` 启动即跑 pass；`POST /stop` 后 ≤5s 退出且状态干净；`oasd scheduler` 同时启动 → 锁冲突明确报错；`/runner/run` 的 body params 覆盖 values 层 |

> **顺序建议**：PR1 ✅ → PR2 ✅ → PR3 ✅ → PR4 ✅ → PR5 ✅ → PR6。PR5 已接 T7（发布/安装）并完成 **`manifest.engine.flow_ops` 安装闸门**（从原始 flow 推导、重算严格相等、拒绝老 Runner）。

## D.4 验收清单（用户可操作，19 步）

> **PR1 状态**：**V1（1~3 已验收；4 的共享路由契约已测）**（真实双进程 8788/8789，见 §A.9.7/§A.9.10）。**PR2 状态**：**V4（11~12）✅**（临时任务 `pr2demo`，Studio 8790 / Runner 8791；见 §A.10.7）。**PR3 状态**：无独立 V 步骤；四层优先级/类型保真/合并后复检由 §A.11.5 测试 + §A.11.6 E2E（CLI/scheduler/replay）覆盖。**PR4 状态**：**V6 的 16 核心路径 ✅**（replay E2E：映射/同名继承/子默认/inherit none/多调用点；见 §A.12.7）；17 依赖 PR6 的 Runner 临时运行、18 的隔离/缺参由 PR4 单测覆盖。**PR5 状态**：**V2（5~7）✅、V3（8~10）✅**（`asset_task` 发布/全新 Runner 安装/409/force/卸载/activate；见 §A.13.8）。**V5 待 PR6；V7 持续检查（PR1~PR5 均未提交用户数据）**。

前置：`cargo build -p oasd --features ocr`；准备两个任务 `任务1`（有参数 `触发文字`）与 `任务2`（调用 `任务1` 并映射 `触发文字←识别关键词`）。

**V1 两端不同入口**
1. `oasd studio --port 8788 --config config/live.json` → `http://127.0.0.1:8788/`，nav 有「工作台/任务构建/任务库/实例/设置」，**没有**「调度」项。
2. `oasd runner --port 8789 --config config/live.json` → `http://127.0.0.1:8789/`，nav **6 项**：「概览/已安装任务/安装包/队列与调度/运行记录/设置」。
3. 在 8789 打开 `http://127.0.0.1:8789/task/save_flow`（或 devtools 发 POST）→ **404**；在 8788 请求 `GET /runner/tasks` → **404**。
4. `GET :8788/instance` 与 `GET :8789/instance` 都能看到同一 `device_serial`（共享配置）。

**V2 Studio 发布**
5. Studio 任务库 →「发布…」（`include_deps` 默认勾选）→ 返回 `path=dist/packages/任务2-v1.oaspkg`、`package_id`、`sha256`、`warnings=[]`。
6. 手工改包里 `task/flow.json` 后再发布 → 校验失败并给出 `T3`/`T7` 路径（不是裸异常）。
7. 重复发布同版本 → **409**，提示 `--force` 或升 `meta.version`。

**V3 Runner 安装**
8. Runner「安装包」→ 填本机绝对路径（或 UI 选择文件）→ 列表出现 `任务2` + `任务1`（依赖自动装）+ `installed.json` 两条。
9. 篡改包里任一图片/checksums 后再装 → 拒绝（sha256 不匹配），`tasks/` 未被改动；篡改 `manifest.engine.flow_ops` → 400 `flow_ops does not match`。
10. 卸载 `任务1`（被 `任务2` 依赖）→ **409** 且给出引用方。

> ✅ **PR5 已验收（临时任务 `asset_task`，2026-11-04，§A.13.8）**：发布 → 全新 Runner（assets 目录为空）安装 200 + 初始化素材 store 的 warning；同版本重复安装无 force **409 `already_installed`**、force 200 replaced + backup；卸载 → `backup/uninstall-…`；activate（无 installed 记录）200 `restored_from=…` 任务回 `tasks/`；zip-slip 400 且包外无文件；Studio 上 `/runner/install` 404、Runner 上 `/task/publish` 404。V3 的依赖自动装/被依赖 409 由安装集成测试覆盖（真实 E2E 未单独打双任务包）。

**V4 配置**
11. Runner「任务2 → 配置」显示字段 `识别关键词`（label/说明/当前值 `突破`），改成 `结界` → 保存 → 刷新后仍在。
12. 填 `int` 字段越界值（如 `max_rounds=999` 超 `max`）→ **400**，文案指名 key 与范围。

> ✅ **PR2 已验收（临时任务 `pr2demo`，2026-11-04）**：Runner 表单渲染 `target_text`（text，required）+ `retry`（number 0..5）；中文值往返、越界/类型错/未知 key/缺 required 均 400 且 `errors` 含 key；Studio 侧 `POST /task/config` 保留 meta 其它字段并生成 `meta.json.bak`；E2E 细节见 §A.10.7。正式任务的用户验收可照 11~12 执行。

**V5 调度与执行**
13. Runner「队列与调度」加 entry（`任务2`，periodic）→ 保存成功；`GET :8789/scheduler/status` 显示 `lock` 与下次唤醒。
14. 启动调度 → 运行记录出现一次 Run；`runs/scheduler/<run_id>.json` 有实体。
15. 运行日志/点击审计里能看到子任务步骤执行（父 flow 展开后），且**没有**「`param('触发文字')` has no value」。

**V6 子任务取父配置（核心）**
16. 把 `任务2` 的 `识别关键词` 设为 `结界` → 触发一次运行 → 任务1 的动作按 `结界` 走（用 `log` 步骤打出 `param('触发文字')` 核对）。
17. 在 Runner「临时运行 任务1」（不传参）→ 任务1 用的是**自己的** values/default（`""`）→ 触发 `required` 提示，而**不是**继承 `结界`。
18. 把 `任务2` 的映射改成 `inherit:"none"` 且不映射 → 任务1 的 `param('触发文字')` 报错，文案为 `param('触发文字') has no value in subtask '任务1': declare it in that task's meta.params (with a default), map it with call_task.params, or pass --param 触发文字=<value>`。

> ✅ **PR4 实现后（2026-11-04）**：16 已用 replay E2E 验证（`parent→child` 映射/同名继承/子默认，见 §A.12.7）；17 需要 PR6 的 `/runner/run`（Runner 临时运行）才能端到端验收；18 的 `inherit:"none"` 隔离与缺参文案由 runtime 单测覆盖（§A.12.3/§A.12.7），未做 Runner UI 端到端。

**V7 不动用户数据**
19. 全程 `git status` 中 `tasks/`/`scheduler.json` 的变更都可解释（安装/配置写入），没有设计外的新文件；`packages/`、`config/values.json` 已加入 `.gitignore`。

---

# 附录 A：现有能力分类（复用 / 新增 / 改语义）

**A. 可直接复用（不改代码）**
- `meta.params` schema + lint（`meta.rs:45-58,433-470`；`lib.rs:2146-2170`）
- `param()` 与 `RunOptions.params`（`oas-flow` `lib.rs:466-478`；`oas-runtime` `lib.rs:1127-1138,3481-3488`）
- 调度 entry params 注入（`schedule.rs:3733-3750,1692`）与 UI 输入框（`ui/index.html:618-621`、`ui/app.js:6991-7001,7052-7071`）
- 单实例锁（`schedule.rs:775-833,880-884`）、原子写（`schedule.rs:569`、`serve.rs:7151`）
- 删除守卫/备份/剪枝（`serve.rs:4526-4570`）、路径校验（`serve.rs:4508-4522`）
- `validate_task_name` / `list_tasks` / `lint_task`（`lib.rs:535,2474,2553`）
- 部署脚本模式（`packaging/*.bat`）、`dist/` 忽略（`.gitignore:13`）

**B. 需新增（纯加法）**
- `Surface` + `required_surface` 路由归属表；`oasd studio`/`oasd runner`；`ui/runner.*`
- `package.rs`（zip/manifest/checksums）、`oasd studio publish`、`/runner/install|uninstall|packages`
- `config/values.json` + Runner 配置 API/表单
- `with_params` Step 变体 + `call_task.params`/`inherit` 解析 + `Flow::call_task_uses` + lint T7
- 调度线程 `stop` 信号、`/runner/health`、设备租约 `config/device.lease`
- 上传流式落盘 + 放宽单路由体积上限

**C. 需改语义（要评审、要回归）**
- `RunOptions.params`：`BTreeMap<String,String>` → `BTreeMap<String,Value>`（类型保真）
- 运行值合并点：增加 Runner values 一层，合并后按 `meta.params` 复检（type/min/max/enum/required）
- 子任务 params 作用域：从「平铺共享」变为「继承 + 映射 + 默认兜底」（默认行为兼容，但**新增「子任务 default 生效」**这一变化）
- lint：从「单任务自洽」扩到「跨任务引用校验」（T7）
- `serve` 命令：语义等同 `studio` + deprecation 提示

# 附录 B：关键证据速查（完整 235+ 条见原稿）

| 主题 | `file:line` |
|---|---|
| 路由表 / UI 文件 / 404 | `apps/oasd/src/serve.rs:1206-1352`、`1207-1214`、`1346-1351` |
| 服务启动与绑定 | `serve.rs:983-1007`；`apps/oasd/src/main.rs:387-469` |
| 调度器 CLI / 主循环 / 锁 | `apps/oasd/src/schedule.rs:1-60`、`3179`、`1188`、`775-833`、`880-884`；`main.rs:98` |
| entry params（结构/序列化/注入） | `scheduler.rs:704-722`、`826-840`；`schedule.rs:3733-3750`、`1692` |
| 队列 | `scheduler.rs:685-702`；`schedule.rs:2058+`、`1692`（成员共享 entry params） |
| 编辑器试运行 | `serve.rs:6521-6623`、`6580-6589`、`6627-6642` |
| `call_task` 展开 / 未展开 flow / lint | `crates/oas-task/src/lib.rs:704-772`（`MAX_CALL_DEPTH` 707）、`934-948`、`1054-1091`、`2110`、`2146-2170` |
| `call_task` 语法 / `param()` | `crates/oas-flow/src/lib.rs:966-972`、`2004-2006`、`410-420`、`466-478`、`2162-2178`；`oas-runtime` `lib.rs:976-978`、`3481-3488` |
| `meta.params` | `crates/oas-task/src/meta.rs:39,45-58,171-172,224-237,239-260,279-305,433-470` |
| 运行期合并 | `apps/oasd/src/run.rs:125-127,224-239,257-265,461-481,670` |
| 部署 / 单活动实例 | `packaging/README-DEPLOY.md:33-48,58-67,69-77,79-84`；`start-editor.bat:10-20`；`start-scheduler.bat:17-26`；`ENGINE-DESIGN.md` 单活动实例段；`SCHEDULER-RUNNER-PLAN.md:37` |

---

# E. 与现状文档的关系 / 执行状态

- **Q1~Q7 已于 2026-11-04 获用户全部批准（按推荐）**；**PR1 ✅ 已提交**（`da9da9d`），见 **§A.9**；**PR2 ✅ 已提交**（`dfe8776`），见 **§A.10**；**PR3 ✅ 已提交**（代码 `c09df25` + seq 103 docs `048dff5`），见 **§A.11**；**PR4 ✅ 已提交**（HEAD `442f5cb`，含 seq 104 docs），见 **§A.12**；**PR5 ✅ 已实现并验证（代码未提交，见 §A.13）**。本文已从「设计待定稿」进入「已批准 + 分阶段执行」状态。
- **已实现（PR1）**：`Surface` + `route_owner`/`required_surface`、`oasd studio`（8788）/`oasd runner`（8789）/`oasd serve` 兼容别名、`config.runner`、`GET /runner/health`、`GET /runner/tasks`、`ui/runner.*` 骨架、Studio 去调度入口、Runner-only 发行包、`packaging/CROSS-PLATFORM.md`。
- **已实现（PR2，§A.10）**：`ParamDecl` 五字段（label/description/enum/required/group）+ lint + `validate_param_values`；`config/values.json`（原子 RMW，Runner 独占）；Studio `GET/POST /task/config`（保留 meta 其它字段 + `.bak`）；Runner `GET/PUT /runner/tasks/<id>/config`（值校验 400 含 key）；Studio「配置项…」JSON 编辑器 + Runner 配置表单；`.gitignore` 三项；PR2 时 Runner 包为 **37,192,710 B**（PR3/PR4/PR5 已重建，见下）。
- **已实现（PR3，§A.11）**：`RunOptions.params` 改 `BTreeMap<String, serde_json::Value>`；CLI `--param` 为唯一文本猜类型点 + 引号字符串逃生口；scheduler/queue 注入 `config/values.json`（entry > values > default；Studio 刻意不注入）；合并后 `validate_param_values` 复检（`meta.params` 为空则跳过）；`SchedulerOptions/QueueOptions.values_path` 测试 seam；测试 **workspace 583/0**（oasd **276**）。
- **已实现（PR4，§A.12）**：`Step::CallTask{params,inherit}` + 内部 `with_params`；loader 严格展开（目标 flow 预解析 + 子任务 `meta.params[].default` 生效）；runtime 作用域（快照→调用方求值→`inherit:none` 隐藏同名→defaults 补缺→overrides→**全路径恢复**）；`Flow::call_task_uses`；lint **T7**（目标存在/可解析、child/parent key、常量映射、计算 key 拒绝）+ CLI `task lint --tasks-dir`；评审 P1 处理（T6 参数名豁免、严格 meta、checkpoint 展开后指纹、T7 强化；**P1-4 `flow_ops` 闸门→PR5**）；Runner 包按 PR4 release 重建 = **37,246,079 B**，SHA256 `07B6D930…852E506B`；测试 **workspace 624/0**（flow 62 / task 89 / runtime 157+2 / oasd 280）。
- **已实现（PR5，§A.13）**：`package.rs`（`.oaspkg` zip + manifest + checksums + zip-bomb 加固 + reproducible）+ `package_cmd.rs`（publish/install/uninstall/activate）；Studio CLI/HTTP publish + 历史；Runner path 模式安装（严格顺序 + 回滚）、卸载守卫 + 备份、activate（含卸载后恢复）；**`flow_ops` 从原始 flow 推导 + 安装重算严格相等 + `min_engine`**；素材快照/初始化/冲突 400；Studio 发布对话框 + Runner「安装包」页（nav 6）；评审 P1-1/P1-2 已修、P1-3 明确不实现（自动改名+flow 重写 defer Phase 5）；Runner 包 **37,591,694 B**，SHA256 `823F93CD…14031E`；测试 **workspace 677/0**（oas-task **111** / oasd **311** / flow 62 / runtime 157+2），UI **1442/237/0**（runner audit **55/0/6**）。
- **未实现（PR6）**：Runner 内嵌调度线程/调度启停/设备租约/`POST /runner/run` 临时运行（**必须自行合并 values 层**）；上传模式、素材自动改名 + flow 重写（Phase 5）、显式纳管 API、素材 GC。**Runner 的调度编辑仍未实现**；安装/卸载/回滚与配置/值注入/子任务语义均已可用（PR2~PR5）。
- 台账登记：`PENDING-LEDGER.md §E-79`（状态 = **✅ Q1~Q7 已批准；PR1~PR4 ✅ 已提交；PR5 ✅ 已实现（未提交）；PR6 待做**；PR3 明细见 §H、PR4 见 §I、PR5 见 §J）。
- **下一步**：按 §D.3 做 **PR6 内嵌调度/队列执行 + `/runner/run` 临时运行（自行合并 values）+ 设备租约 + 调度健康/启停**；每个 PR 用 §D.4 对应验收项收口，测试基线以最新 `cargo test --workspace` 为准（**PR5 后 = 677/0**，flow **62**、task **111**、runtime **157 lib + 2 integration**、oasd **311**；UI **1442/237/0**）。
- 若用户改其中某项（例如 Q1 改端口 / Q6 改子进程调度），先回写本文 §D.2 与稿源，再动代码。
