# OAS Studio / Runner 分离 + TaskPackage + 任务配置（运行期参数）/ 子任务配置 · 设计稿 v1

> **状态：设计稿 v1，待用户拍板 §D.2 的 7 项决策（Q1~Q7）；未开始实现（零代码改动）。**
> 待拍板项 = **§D.2 的 7 项决策（Q1~Q7，每项已给推荐）**；用户确认后按 **§D.3 的 PR1→PR6** 顺序开工，用 **§D.4 的 19 步验收清单**收口。
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
5. **子任务配置用「作用域」而不是「静态改写」**：`call_task` 增加 `params` 映射 + `inherit`，loader 展开时生成 `with_params` 包装 step，runtime 用 **params 栈**实现（新增 Step 变体，向后兼容旧任务）。
6. **最大风险不是分离，而是 `call_task` 是 load-time 展开**（`crates/oas-task/src/lib.rs:1058-1091`）：调用点身份在 runtime 完全消失，`with_params` 只是「作用域」不是「调用栈」⇒ 本期**不做**「子任务独立超时/重试/运行记录」（见 §D.1 **R1**）。

---

# A. 总体架构（事项 2）

## A.1 现状事实（审计基线，先钉死）

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

| 维度 | Studio（编辑端） | Runner（使用端） |
|---|---|---|
| 命令 | `oasd studio [...]`（新增；`oasd serve` 保留为 deprecated 别名 + 一行迁移提示） | `oasd runner [...]`（新增） |
| 默认端口 | **8788**（与 `packaging/live.example.json:5`、`start-editor.bat:11` 一致；顺手把 `main.rs:388` 的 8787 修成 8788，消除现状不一致） | **8789**（新，`config.runner.port`） |
| 绑定 | `127.0.0.1` 固定 | `127.0.0.1` 固定（**不建议**暴露 LAN：`README-DEPLOY.md:82` 明确不要公网；真要 LAN 再加 `runner.bind` + token，见 §D.2 Q1） |
| 根 UI | `GET /` → `ui/index.html`（现有 6 页 nav，**去掉「调度」页**） | `GET /` → `ui/runner.html`（新；nav 只有 **5 项**：概览 / 已安装任务 / 队列与调度 / 运行记录 / 设置） |
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
| `POST /task/config`（新） | S | 编辑 `meta.params` schema（§C.1） |
| `POST /task/publish`、`GET /task/package`（新） | S | 发布 TaskPackage（§B.3） |
| `GET /runs`、`/runs/clicks`、`/runs/aggregate` | **B（只读）** | 点击分布属编辑分析，但运行记录 Runner 也要看；写接口分离 |
| `POST /runs/delete` | R | 运维删档 |
| `GET /runs/scheduler`、`/runs/scheduler/run` | R | 调度 Run 一等实体（`schedule.rs:104-111`） |
| `GET /scheduler/config`、`GET /scheduler/status` | **B（只读）** | Studio 需要「该任务是否被调度引用」的只读视图（删除守卫依赖） |
| `POST /scheduler/config`、`POST /scheduler/reset` | R | 写调度配置 / 清记忆 = 运维 |
| `GET /config` | B | 部署配置只读 |
| `GET /instance`、`/instance/devices`、`POST /instance` | B | 设备是 Studio 采集与 Runner 执行**共用**的唯一物理资源；写 `config/live.json` 两端都允许（写入即 `restart_required`，`serve.rs:4847`） |
| `GET /stats` | B | 统计只读 |
| `GET /runner/health`、`POST /runner/scheduler/start`、`/stop`（新） | R | 守护线程与健康 |
| `GET /runner/tasks`、`GET/PUT /runner/tasks/<id>/config`（新） | R | 已安装任务 + 运行值 |
| `POST /runner/install`、`/runner/uninstall`、`/runner/packages/activate`、`POST /runner/run`（新） | R | 包安装 / 卸载 / 回滚 / 临时运行 |

> **一句话**：**Studio 无调度写/运维（无调度页）**，**Runner 无创作/构建 API**（拿不到 `/task/save_flow` 等）；`get_route()` 单测把这条钉死。

## A.4 职责矩阵

| 能力 | Studio | Runner | 现状与证据 |
|---|---|---|---|
| 创作（帧 / 素材 / OCR / 流程编辑） | ✅ 主 | ❌ | `serve.rs:1215-1310` |
| 构建 / 校验（lint T1-T6，+T7 新增） | ✅ 主 | ❌（**安装时复检**） | `oas-task` `lint`（`lib.rs:2035-2052`）；`oasd task lint`（`task.rs:360-371`） |
| **发布（只产出 TaskPackage）** | ✅ 主 | ❌ | **需新增**（§B.3） |
| 安装 / 更新 / 卸载 / 回滚 | ❌ | ✅ 主 | **需新增**（§B.4） |
| 任务配置 **schema**（声明/默认值/范围） | ✅ 主（写 `meta.params`） | ❌ | 已有 `meta.params` + lint（`meta.rs:433-470`） |
| 任务配置 **运行值**（填值） | ❌ | ✅ 主 | **需新增**存值文件（§C.2）；现状只有调度 entry 的 params JSON 输入框（`ui/index.html:618-621`） |
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
  "engine": { "template_version": 1, "min_engine": "0.1.0", "flow_ops": ["call_task","with_params"] },
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
- `engine.template_version` 对应 `meta.template_version` + `TEMPLATE_VERSION`（`meta.rs:27,356-372`）；**`flow_ops` 是 `with_params` 的兼容闸门**（包里用了新 op，老 Runner 拒绝安装）。
- `params_schema.own` 是 `meta.params` 的**快照**，不是第二权威（安装后仍以 `tasks/<id>/meta.json` 为准，避免双份 schema 漂移）。
- `dependencies` 是 `call_task` 的静态闭包（深度上限沿用 `MAX_CALL_DEPTH=8`，`lib.rs:707`）。
- `signature: null`：v1 不做签名（§A.7）。

## B.3 发布流程（Studio）

1. **入口**：CLI `oasd studio publish --task <id> [--out dist/packages] [--force] [--include-deps]` + UI「任务库 → 发布…」按钮（`POST /task/publish`，返回 `{path, sha256, warnings[]}`）。
2. **校验（发布前必须全绿，复用现有能力）**：

   | 检查 | 复用 |
   |---|---|
   | 任务名/ID 合法 | `validate_task_name`（`lib.rs:535`）、`meta.task_id` 校验（`meta.rs:378-380`） |
   | lint T1/T3/T4/T5/T6 | `TaskBundle::lint()`（`lib.rs:2035`）/ `lint_task`（`lib.rs:2563`） |
   | `param('k')` 都声明 | T3（`lib.rs:2146-2170`） |
   | `template_version` ≤ 引擎 | `meta.rs:356-372` |
   | 素材引用存在 | T4 + `flow.validate_assets`（`lib.rs:2181`） |
   | `call_task` 目标存在（**现状没有**） | **新增 lint T7**（§C.4.5） |
   | 无 `state/`、无 `.clicks.jsonl` | 新增（`tasks/*/state` 已在 `.gitignore:9`） |

3. **产出命名**：`dist/packages/<task_id>-v<task_version>.oaspkg` + 同名 `.sha256`；已存在同名文件 → 默认拒绝（提示 `--force` 或 `meta.version += 1`）。
4. **产物是否入 git**：**不入**。`dist/` 已被忽略（`.gitignore:13`）；`packages/`（Runner 侧）也应忽略。将来若要「发布历史」，只提交 `manifest.json` 到 `dist/packages/manifests/`（可选，Phase 4 再议，§D.2 Q7）。
5. **发布幂等**：同输入 → 同 `checksums.sha256`；`created_at_ms` / `package_id` 会变（含时间），要可复现则加 `--reproducible`（固定 mtime、去掉时间字段）。

## B.4 安装 / 更新 / 卸载（Runner）

**新增模块**：`crates/oas-task/src/package.rs`（打包/解包/manifest，纯逻辑可单测）+ `apps/oasd/src/runner.rs`（HTTP 编排与落盘）。`oasd` 侧不直接依赖 `zip` 的话可把 zip 读写放 `oas-task`（它已依赖 serde/oas-flow）。

### B.4.1 安装 API
```
POST /runner/install            # {path:"D:/x.oaspkg"} 或 {upload:true} + application/octet-stream 流式
  -> {ok:true, package_id, task_id, version, replaced:bool, warnings:[...], backup:"packages/backup/..."}
POST /runner/uninstall          # {task_id, purge_config?:bool}
POST /runner/packages/activate  # {package_id}  回滚到某个历史版本（Phase 3）
GET  /runner/packages           # 已安装清单（installed.json + 校验状态）
```

### B.4.2 落盘流程（严格顺序，任一步失败即回滚）
1. 收包 → `packages/incoming/<sha256>.part` → 校验顶层 sha256（与请求头 `X-Package-Sha256` 或文件名比对；不匹配则删）。
2. 解压到 `packages/staging/<package_id>/`：逐 entry 做 zip-slip 校验（§A.7），**不覆盖任何已有目录**。
3. 读 `manifest.json` + `checksums.sha256`，逐文件比对 sha256（缺文件/多文件都报错）。
4. 校验 `task/meta.json` + `task/flow.json`：`template_version` ≤ 引擎（`meta.rs:356-372`）；**对 staging 目录跑一次 `lint`**（做法：把 staging 挂成临时 tasks-dir，跑 `TaskBundle::open` + `lint()`，`lib.rs:845-860`）。
5. 依赖解析：`dependencies` 里 `included:true` → 从 `deps/<task_id>/` 装（深度/环沿用 `expand_call_steps` 的判定，`lib.rs:739-744`）；`included:false` → 要求目标已安装且 `version >= 声明`，否则**拒绝安装**并列出缺失。
6. 素材合并（§B.4.3）。
7. 提交：`tasks/<task_id>` 已存在 → 移到 `packages/backup/<task_id>-<old_ver>-<ts>/`；再把 staging 的 `task/` 原子 rename 到 `tasks/<task_id>`。
8. 写 `packages/installed.json`（原子 tmp+rename，参考 `schedule.rs:569` 的 `write_json_atomic`）。

`installed.json`：
```json
{ "schema_version": 1,
  "packages": [ { "package_id":"…", "task_id":"每日任务", "version":3, "sha256":"…",
                  "installed_at_ms":0, "source":{"commit":"f16fea1","signed":false},
                  "backup":"packages/backup/每日任务-2-1793…" } ] }
```

### B.4.3 包内素材 vs 全局素材库的合并（冲突策略）
现状：全局 store = `assets/assets.json` + `assets/images/aNNN.png`，id 全局唯一、文件名与 id 无关（`lib.rs:15-19`），`rev` 表示版本（`library.rs:151-164` 已有 `replace → rev+1` 先例）。
- **同 id + 同 sha256** → 复用，不改文件；
- **同 id + 不同 sha256** → 按现有迁移策略自动改名 `<id>_2`（`packaging/README-DEPLOY.md:18` 的 `assets migrate` 行为），**并重写安装态 `tasks/<id>/flow.json` 的引用**（改的是安装态 flow，不是用户编辑态创作源——但两者同目录，见下方 ⚠）；
- **新 id** → 追加，`rev` 从 1 起；
- 合并后**不回写**包内 `assets/assets.json`（包是只读输入）。

> ⚠ 这是「安装态与编辑态共用 `tasks/`」的直接后果：**安装会改写本机 `tasks/<id>/flow.json` 的素材引用**；若用户仍在 Studio 编辑同名任务，会出现「我改的 flow 被安装改了」。缓解：安装前若 `tasks/<id>/meta.json` 存在且 `mtime` 比 `installed.json` 记录的新 → 返回 **409 `dirty_task`**，要求 `force:true`（§D.2 Q6 备注 / §D.1 R3）。

### B.4.4 卸载
- **拒绝条件（复用现有语义，别新造）**：① 被 `scheduler.json` 的 entry 或队列成员引用（`serve.rs:4526-4529` 的 `scheduler_queues_with_member` 同类判断）；② 正在运行（`run_state.running`，`serve.rs:6580`；或调度 `state.running`）；③ 被其他已安装包的 `dependencies` 引用（查 `installed.json`）。
- 执行：`tasks/<id>/` 先移到 `packages/backup/uninstall-<id>-<ts>/`（**不直接删**，与 `backup_task_assets` 的「永不丢素材」原则一致，`serve.rs:4530-4542`），再更新 `installed.json`；`purge_config:true` 才删该任务在 `config/values.json` 里的值。

### B.4.5 裸目录兼容（重要）
Runner 首次上线时用户手上只有 `tasks/`（无包）⇒ **Runner 必须能直接识别未安装的 `tasks/<id>/`**（状态 `unmanaged`），并在 UI 里允许「纳管」（生成 `installed.json` 条目，`package_id` 空、`source=local`）。否则用户升级后「任务全没了」。同理 `GET /runner/tasks` = `installed.json` ∪ `tasks/*`（`list_tasks`，`lib.rs:2484`）。

## B.5 包内依赖（call_task 目标怎么打）

推荐 **C（声明优先，缺失即拒，可选内嵌）**：

| 策略 | 行为 | 问题 |
|---|---|---|
| A 递归全打 | 所有 `call_task` 目标都进 `deps/` | 包体积不可控；同名任务重复；循环依赖要靠 `MAX_CALL_DEPTH` 兜 |
| B 只写引用 | 包只含自己 + 依赖列表 | 安装常失败（对方没装），体验差 |
| **C 声明 + 可选内嵌（推荐）** | `dependencies` 总是写全；`--include-deps` 决定是否把目标放进 `deps/`；装包时「内嵌优先，否则要求已安装足够版本」 | 需写清「内嵌版本会覆盖本机版本」的提示 |

全局素材引用：**快照进包**（`assets/`）而不是「只写 id」。理由：T4 只校验本机存在（`lib.rs:2181`），换机安装时 id 可能不存在 ⇒ 包必须自带被引用素材。快照只含**实际被引用**的 id（`flow.validate_assets` 的闭包），不含整个 store。

## B.6 最小实现切分（Phase 1 垂直切片）

**Phase 1（能跑通「Studio 发布 → Runner 安装 → 配置 → 跑」）**：
- 新增 `crates/oas-task/src/package.rs`：`Manifest` / `build_package(dir, out)` / `read_package(zip)` / `verify_checksums`。
- 新增 `apps/oasd/src/package_cmd.rs`（或并入 `task.rs`）：`oasd studio publish`。
- `serve.rs`：`Surface` 字段 + `required_surface` + 6 条 Runner 路由（`/runner/tasks`、`/runner/tasks/<id>/config`、`/runner/install`（**仅 path 模式**）、`/runner/health`、`/runner`）。
- `ui/runner.html` + `ui/runner.js`（只做：任务列表、参数表单、安装、健康）。
- `config.rs`：加 `runner: {port: 8789}`（`#[serde(default)]`，老 `live.json` 不炸）。

**Phase 2**：`with_params` 作用域 + `call_task.params` + lint T7（§C.4）。
**Phase 3**：配置优先级 + `config/values.json` + 四入口统一（§C.3）。
**Phase 4**：内嵌调度线程 + 健康/启停 + 设备租约。
**Phase 5**：上传模式、回滚、卸载守卫、`deps/` 内嵌与冲突改名。

> Phase 1 允许「安装 = 从 path 解压 + 校验 + 覆盖 tasks/」，**不做上传**（4 MiB 限制与流式改造推迟）——切片代码量最小且能立刻验收。

---

# C. 任务配置（运行期参数）+ 子任务配置

## C.1 配置模型：`meta.params` 够用，扩展 5 个可选字段

现状（`meta.rs:45-58`）：`{key, type, default?, min?, max?}`，`type ∈ {int,float,number,bool,string}`（`meta.rs:38-39`），lint 校验重复/类型/default 类型/min≤max（`meta.rs:433-470`），T3 要求 flow 里 `param('k')` 都已声明（`lib.rs:2146-2170`）。

扩展（全部 `#[serde(default)]`，老 `meta.json` 零破坏）：

| 字段 | 类型 | 用途 | 校验 |
|---|---|---|---|
| `label` | string | Runner 表单显示名（中文） | 非空 ≤ 40 字 |
| `description` | string | 一行说明/示例（「识别到该文字时触发」） | ≤ 200 字 |
| `enum` | string[] | 下拉选项（`type` 仍为 `string`） | 非空、去重、元素须符合 `type` |
| `required` | bool | Runner 必须填（无默认值时强制） | 与 `default` 不冲突（有默认即可不填） |
| `group` | string | 表单分组（基础/高级） | ≤ 20 字 |

`enum`/`required`/`label` 属**纯展示与表单约束**，不影响 runtime；`min/max` 目前**只校验 default，不校验运行值**（`meta.rs:462-470` 是唯一使用点）→ 现存缺口，运行值校验放到 §C.2 的写入 API。

**Studio 是 schema 的唯一编辑者**：`POST /task/config {task, params:[...]}`（新）→ 写 `meta.json` 的 `params` 数组，走 `Meta::from_json` + `lint_issues` 校验后原子写。已有先例：`POST /task/goal` 会自动补 `params.max_battles`（`serve.rs:4396-4420`），说明「程序化维护 params 数组」是被接受的模式。

## C.2 Runner 侧配置值存储

- 文件：**`config/values.json`**（Runner 独占写，Studio 只读或不读）

```json
{ "schema_version": 1,
  "updated_ms": 0,
  "tasks": {
    "每日任务": { "values": { "max_rounds": 5, "识别关键词": "突破" },
                  "updated_ms": 0, "slots": { "scheduler:e1": { "max_rounds": 3 } } }
  } }
```

- `values` = 任务级运行值（对应「任务1 单独运行时用任务1 的配置」）。
- `slots` = **按调度 entry 覆盖**（可选，Phase 3；等价于今天把值写在 `Entry.params`，`scheduler.rs:719`）。保留 `Entry.params` 不动，`slots` 只是「同一任务多条 entry 不同配置」的可视化编辑面；写 entry 仍是写 `scheduler.json`（单一权威，`scheduler.rs:826-840`）。
- 并发写：单写者（Runner 进程内 `Mutex` + 原子 tmp/rename，复用 `write_json_atomic`，`schedule.rs:569`）。**Studio 不写此文件**。
- 与 `Entry.params` 的关系：**不是替代**，是「任务默认值 / entry 覆盖值」两层；两者都不在 `scheduler.json` 之外产生第三份真值。

## C.3 运行期解析优先级（四入口统一）

**根任务（standalone / queue / scheduler / 临时运行）解析顺序（高 → 低）**：

| 优先级 | 来源 | 现状 | 改动 |
|---|---|---|---|
| 1 | CLI `--param k=v` 显式覆盖 | ✅ 已有（`run.rs:224-239`，注释明确「Last one wins」） | 无 |
| 2 | 调度 entry `params`（含队列 entry，成员共享） | ✅ 已有（`schedule.rs:3733-3750`，注入点 `schedule.rs:1692`） | 无 |
| 3 | **Runner 任务配置**（`config/values.json`） | ❌ 不存在 | 新增：`run.rs` 的合并点加一层 |
| 4 | `meta.params[].default` | ✅ 已有（`run.rs:461-481`） | 无 |

四种入口映射（全部落到同一个 `RunOptions.params` 合并点 `run.rs:461-481`）：

| 入口 | 传参路径 | 备注 |
|---|---|---|
| 单独运行（CLI） | `oasd run --task 任务1 --param k=v` | 无 entry/values 时只看 1/4 |
| 临时运行（UI） | `POST /runner/run {task, params:{}}`（新） | 参数直接进优先 1（「这次手工跑」的语义） |
| 队列 | `scheduler.json` 的 queue entry `params` | 成员**共享**该 entry 的 params（现状即如此：`execute_attempt(entry)` 用的是队列 entry，`schedule.rs:1692`）——要「每成员不同配置」需把 `queues[].members` 扩成对象，见 §D.2 备注 |
| 调度 | entry `params` | 同上 |

**必须一起修的两个现存问题（否则优先级做出来也不对）**：

1. **类型丢失**：`entry_run_params` 先把 JSON 标量转成字符串（`schedule.rs:3738-3749`），`run.rs` 再 `parse_param_value` 猜回（`run.rs:257-265`：`"true"→bool`、能 `parse::<f64>()` 就当数字）。于是 entry `{"code":"007"}` 变成 `Num(7.0)`，而 `meta.params[].type="string"` 声明的是字符串 ⇒ 运行时 `param('code')` 类型错。**修法**：`RunOptions.params` 改 `BTreeMap<String, Value>`（内部已是 `Value`，`run.rs:125-127` 只是被 CLI 字符串堵住了），entry/values 直接传 `Value`，只有 `--param` 保留字符串解析。
2. **运行值不校验 schema**：`min/max/enum/required/type` 对运行值不生效（§C.1）。**修法**：在 `config/values.json` 写入 API 里把 `meta.rs:239-260` 的 `check_param_default` 泛化成 `check_param_value(kind, value, min, max, enum)`，并在 `run.rs` 合并后做一次「防御性复检」（错误信息带任务名与 key）。

## C.4 子任务配置语义（核心）

### C.4.1 现状（必须先看清）
- `call_task` 只是 `{"op":"call_task","args":{"task":"任务1"}}`，解析时**只读 `task`**，其他 args 被静默忽略（`oas-flow` `lib.rs:2004-2006`；「unknown args are ignored」有测试注释 `lib.rs:2532-2533`）。
- loader 在 `load_flow` 时把目标 `flow.json` 的 steps **展开**进父 flow，并 `strip_finish_steps`（`oas-task` `lib.rs:709-718,745-760`），深度上限 8、环检测（`lib.rs:707,726-744`）。
- runtime **完全不感知** `call_task`；万一漏网就报错（`oas-runtime` `lib.rs:3376-3379`）。
- 展开后**子任务 steps 与父任务共享同一个 params 命名空间**（`Interpreter.params`，`oas-runtime` `lib.rs:978`；`param()` 直接查这一张表 `lib.rs:3524-3531`）。
- **子任务 `meta.params` 的 default 不参与运行**：`run.rs` 只加载**根** bundle 的 meta 取默认值（`run.rs:464-476`）⇒ 子任务声明 `param('触发文字')` 但父任务没声明/没传 → 运行时报 `param('触发文字') has no value`。
- lint 用**未展开**的 flow（`lib.rs:2110`，`load_flow_raw` 另有入口 `lib.rs:934-948`），所以 T3 只检查父任务自己的 `param()`；`call_task` 是**叶子**，不收集 param/asset/finish（`oas-flow` `lib.rs:2313,2401`）；`ends_with_finish` 也不认 call_task（`lib.rs:2186`）⇒ 父 flow 末尾必须自己 `finish`。
- **没有**跨任务引用校验：`call_task` 指向不存在的任务能过 lint，直到运行加载才失败（`lib.rs:745-749`）。

### C.4.2 方案对比

| 方案 | 做法 | 嵌套/多调用点/不同映射 | 风险 | 结论 |
|---|---|---|---|---|
| **A 作用域 step**（推荐） | loader 展开时生成 `with_params` 包装 step，runtime 用 params 栈 | 天然支持：每个调用点一个作用域块，嵌套靠栈 | 需新增 flow op（老 Runner「unknown instruction」） | ✅ 采用，配 `engine.flow_ops` 闸门 |
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

**loader 展开后生成**（只在内存里，不落盘）：

```json
{"op":"with_params","args":{
  "params":{"触发文字":"param('识别关键词')"},
  "defaults":{"max_rounds":3},
  "body":[ /* 任务1 的 steps（已 strip_finish） */ ]}}
```

- `params` 的值是 **Expr 字符串**，在**调用方作用域**求值（所以 T3 会要求父任务声明 `识别关键词`——正确）。
- `defaults` 来自子任务 `meta.params[].default`（用现成 `json_to_param_value`，`meta.rs:224-237` 转成 `oas_flow::Value`），**只在该 key 当前不存在时**填充。
- 生成位置：`expand_call_steps`（`lib.rs:720-772`）在 `out.extend(target_steps)` 处（`lib.rs:760`）改为 `out.push(with_params_step)`；`deps` 递归天然嵌套（深度上限/环检测不动）。

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
3. 子任务 `meta.params[].default`（**修掉今天的缺口**：现在子任务默认值根本不生效）；
4. 都没有 → **响亮失败**，错误信息必须带「子任务名 + key + 提示（在父任务 meta.params 声明或增加 params 映射）」。当前文案只有 `param('{key}') has no value: declare it in meta.params … or pass --param`（`oas-runtime` `lib.rs:3524-3531`），在子任务场景会误导用户去改父任务的 CLI 参数。

冲突规则：显式映射 **覆盖** 同名继承（确定、可解释）；`inherit:"none"` 关闭第 2 步（严格隔离，适合「子任务必须自带配置」）。

**为什么不默认 `inherit:"none"`**：那会破坏所有现存的「父传子」隐式共享用例（今天 100% 是共享的），且用户已有任务里子任务参数靠父任务同名声明才能跑通，改成 none 会立刻炸一批。

### C.4.5 lint 规则（新增 T7，T3 语义不变）

- **T7（新）`call_task` 校验**：在 `lint_into` 里对**原始 flow** 的每个 `call_task` 步骤：
  - 目标任务存在（`tasks/<task>/meta.json` + `flow.json` 可读）→ 否则「call_task `任务1` 不存在」；
  - `params` 的 key ⊆ 子任务 `meta.params[].key` → 否则「子任务未声明参数 X」；
  - `params` 的 value（父 key）⊆ 父任务 `meta.params[].key` → 否则「父任务未声明 X」（也可交给 T3 的 `param_uses` 收集，二选一；建议 T7 独立报错以便文案精准）；
  - `inherit` 值合法。
- **T3 保持不变**：lint 用未展开 flow（`lib.rs:2110`），父任务里看不见子任务的 `param()` ⇒ **不会**要求父任务声明子任务的 key（否则每加一个子任务就要往父任务抄一遍参数）。
- **需要新增收集器**：`Flow::call_task_uses()`（返回 `{task, params:{child:parent}, path}`），与 `param_uses_at`（`oas-flow` `lib.rs:2176`）同款实现，放 `oas-flow` 里以便 lint 与 UI 共用。
- ⚠ **代价**：lint 从「单任务自洽」变成「要读兄弟任务」。必须处理「lint 单个任务时兄弟目录不可读」（例如发布包只带自己）：T7 在目标缺失时报 `error`，消息里区分「目标任务不存在」与「无法读取目标（权限/路径）」。

### C.4.6 运行时 flow op 兼容闸门
`with_params` 是新 op：老 Runner 解析会命中 `unknown instruction`（`oas-flow` `lib.rs:2014-2019`）。因此：
- `manifest.engine.flow_ops` 必须包含它；
- **Runner 安装时**检查自己是否支持（常量 `SUPPORTED_FLOW_OPS`），不支持则拒绝安装并提示升级；
- `meta.json`/`flow.json` 的 `schema_version` **不动**（仍是 1，`oas-task` `lib.rs:45`）——op 是加法，不是 schema 变更。

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
| `meta.params` 新增 `label/description/enum/required/group` | 向前兼容（`#[serde(default)]`，与 `meta.rs:145-186` 全可选风格一致） | 老 Studio 读到会忽略（`Meta` 用 serde，**未开 `deny_unknown_fields`**，`meta.rs:149-150`；对比 `OasConfig` 是开着的，`config.rs:23`） |
| `call_task.args.params` | ⚠ 老 loader **静默忽略**（`oas-flow` `lib.rs:2004-2006` 只取 `task`） | 老 Studio/老 Runner 遇到新任务会「少传参数」且**不报错** ⇒ `manifest.engine.flow_ops` 必须做安装闸门；Studio 保存带 `params` 的 `call_task` 时，本机不支持则提示 |
| 新增 `with_params` op | 老 Runner 解析失败（`oas-flow` `lib.rs:2014-2019`） | 同上，`flow_ops` 闸门 |
| `RunOptions.params` 改 `BTreeMap<String,Value>` | 内部类型，非磁盘契约 | `--param` 仍解析字符串（`run.rs:257`），行为不变 |
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
| **R3** | 安装改写 `tasks/` 与 Studio 编辑态冲突（同一目录） | §B.4.3、`.gitignore` 未忽略 `tasks/` | `mtime` dirty 检测 + **409** + `force`；安装前备份；UI 明确「安装将覆盖本机同名任务」 |
| **R4** | 包解压路径穿越 / 压缩炸弹 / 磁盘写满 | `serve.rs:50`（4 MiB） | §A.7 全套校验 + staging 配额 + 失败清理 |
| **R5** | 无签名 → 用户装到被篡改的包 | `README-DEPLOY.md:83`（未签名 exe 已被接受） | v1 sha256 + `source` 标记 + UI 提示；Phase 5 再评估签名 |
| **R6** | 配置值并发写 / 与 entry params 双份真值 | `schedule.rs:569`（原子写已有） | 单写者 + `values` vs `Entry.params` 语义分层（§C.2） |
| **R7** | `entry_run_params` 字符串化导致类型丢失（`"007"` → `7.0`） | `schedule.rs:3738-3749` + `run.rs:257-265` | §C.3 问题 1：端到端传 `Value` |
| **R8** | lint 变成跨任务读取（T7），单任务 lint 可能因兄弟目录缺失误报 | `lib.rs:2110`、`lib.rs:745-749` | T7 消息区分「不存在 / 不可读」；发布流程要求 `--include-deps` 或已装依赖 |
| **R9** | 4 MiB 上限 + 整包读内存 → 大包上传失败 / OOM | `serve.rs:50,1169-1183` | Phase 1 只支持 path 安装；上传前先做流式落盘改造 |
| **R10** | 端口/路径配置不一致的现存坑（serve 默认 8787 vs 8788；CLI 与 serve 的 `--runs-dir` 必须一致） | `main.rs:388`、`config.rs:44` | Phase 1 顺手修 8787→8788；`/runner/health` 回显 `tasks_dir`/`runs_dir` 便于比对 |

## D.2 需要拍板的决策点（7 项，含推荐）★ 用户确认这 7 项后即可开工

| # | 决策 | 推荐 | 理由 |
|---|---|---|---|
| **Q1** | Runner 的命令名与端口 | **`oasd runner` + 8789**，仍只绑 `127.0.0.1` | 与 Studio 8788 相邻好记；不开放 LAN 与 `README-DEPLOY.md:82` 一致；将来要 LAN 再加 `runner.bind` + token |
| **Q2** | `oasd serve` 的去向 | **保留为 deprecated 别名**（内部等价 `studio`），stderr 一行提示；v2 再删 | 现有 `start-editor.bat`/文档/用户习惯全指向 `serve`（`README-DEPLOY.md:48`），直接删会造成「升级后启动失败」 |
| **Q3** | 包容器与后缀 | **zip + `.oaspkg`** | 单文件、hash 友好、`flate2` 已在锁文件、Windows 双击可用；后缀区分「这是任务包」而不是「发行包」（发行包是 `oasd-v*-win64.zip`，`scripts/package.ps1:15`） |
| **Q4** | 配置优先级 | **CLI `--param` > entry params > Runner values > `meta.params.default`** | 显式 > 场景 > 常备 > 声明；与现有「CLI wins」注释一致（`run.rs:461-463`） |
| **Q5** | 子任务未映射 key 的默认 | **同名继承 + 子任务 default 兜底**，`inherit:"none"` 可关闭 | 与今天「共享命名空间」完全兼容（`oas-runtime` `lib.rs:978`），并顺手修掉「子任务 default 不生效」的缺口 |
| **Q6** | Runner 是否内嵌调度 | **内嵌线程**（保留 `oasd scheduler` CLI） | 锁天然互斥（`schedule.rs:775-833`）、少一个用户步骤、无需管理员权限；坏处「UI 崩 = 调度停」可用 `oasd scheduler` 兜底 |
| **Q7** | 发布产物是否入版本控制 | **不入**（`dist/`、`packages/` 都忽略） | 二进制包会让仓库膨胀；`dist/` 已被忽略（`.gitignore:13`）；溯源靠 `manifest.source.commit` + `checksums.sha256` |

> **备注（不单列决策，但需知道）**：同任务多 entry 不同参数目前已支持（`Entry.params` 每 entry 独立）；但**队列成员的 per-member 参数不支持**（成员共享队列 entry 的 params，`schedule.rs:1692`）。若需要「队列里每个任务不同配置」，需把 `queues[].members` 从 `Vec<String>` 扩成对象数组（`scheduler.rs:696`）——**建议先不做**，用「每个成员各建一条 task entry」绕开。
> **配套备注（R3/B.4.3）**：安装覆盖本机同名任务时的 `force` / 409 `dirty_task` 策略已写在 §B.4.3，属实现细节，不单列决策。

## D.3 分阶段实施计划（6 个 PR；PR6 可延后）

| PR | 范围 | 关键文件 | 交付 / 验证 |
|---|---|---|---|
| **PR1 Surface 拆分** | `Surface` 枚举 + `required_surface` 表 + `oasd studio`/`runner` 命令 + `ui/runner.html+js` 骨架 + `config.runner.port` + 8787→8788 | `serve.rs`（252-267、1206-1352）、`main.rs`（89-103、387-469）、`config.rs`、`ui/runner.*`、`packaging/start-{studio,runner}.bat` | 单测：`get_route(&studio_server,"/runner/tasks")==404`、`get_route(&runner_server,"/task/save_flow")==404`（复用 `serve.rs:7632` 钩子）；起两个进程断言端口不通；UI：`runner.html` nav 只 5 项 |
| **PR2 配置模型** | `meta.params` 5 个可选字段 + `config/values.json` + Runner config API/表单 + 值校验（type/min/max/enum/required） | `crates/oas-task/src/meta.rs`、`apps/oasd/src/{serve,runner}.rs`、`ui/runner.js` | 单测：字段解析 / 校验错误路径；`PUT` 后文件原子写；非法值 400 文案含 key |
| **PR3 优先级 + 类型** | `RunOptions.params` 改 `Value` + 四入口合并 + values 注入 | `run.rs`（125-127、461-481、257-265）、`schedule.rs`（3733-3750）、`serve.rs`（6495-6597 加 `params`） | 单测：4 层优先级矩阵（含 CLI 覆盖 entry）；`{"code":"007"}` 保持字符串；e2e：entry 传值 → `log` 打出原值 |
| **PR4 子任务配置** | `call_task.params`/`inherit` + `with_params` op + loader 生成 + runtime 栈 + `Flow::call_task_uses` + lint T7 | `oas-flow`（Step 828-976、2004-2006、2176、2313）、`oas-task`（720-772、2110-2170）、`oas-runtime`（2242、3376-3379）、`ui/flow_model.js`（渲染 call_task 的 params 表单） | 单测：单调用 / 多调用不同值 / 嵌套三层 / 未映射走同名 / default 兜底 / `inherit:"none"` / **错误后 params 恢复**；lint T7 三类错误；E2E：任务2→任务1 取父配置 |
| **PR5 打包 / 安装** | `package.rs`（zip+manifest+checksums）+ `oasd studio publish` + `POST /runner/install` + `POST /runner/uninstall` + 素材合并 + `installed.json` + 裸目录纳管 | 新 `crates/oas-task/src/package.rs`、`apps/oasd/src/runner.rs`、`serve.rs` 路由、`ui/runner.js` | 单测：zip-slip（`../`、绝对路径、盘符）、sha 不匹配、缺文件、`template_version` 过高；集成：publish→install→lint 全绿；卸载守卫（调度引用 409） |
| **PR6（可延后）** | 内嵌调度线程 + `/runner/health` + 启停 + 设备租约 + 上传模式 + 回滚 | `schedule.rs`（加 `stop: Arc<AtomicBool>`）、`runner.rs`、`serve.rs` | E2E：`runner` 启动即跑 pass；`POST /stop` 后 ≤5s 退出且状态干净；`oasd scheduler` 同时启动 → 锁冲突明确报错 |

> **顺序建议**：PR1 → PR2 → PR3 → **PR4** → PR5 → PR6。PR4 与 PR5 无强依赖，可并行；但 PR4 的 lint T7 影响发布校验，最好在 PR5 之前或同期合入。

## D.4 验收清单（用户可操作，19 步）

前置：`cargo build -p oasd --features ocr`；准备两个任务 `任务1`（有参数 `触发文字`）与 `任务2`（调用 `任务1` 并映射 `触发文字←识别关键词`）。

**V1 两端不同入口**
1. `oasd studio --port 8788 --config config/live.json` → `http://127.0.0.1:8788/`，nav 有「工作台/任务构建/任务库/实例/设置」，**没有**「调度」项。
2. `oasd runner --port 8789 --config config/live.json` → `http://127.0.0.1:8789/`，nav 只有「概览/已安装任务/队列与调度/运行记录/设置」。
3. 在 8789 打开 `http://127.0.0.1:8789/task/save_flow`（或 devtools 发 POST）→ **404**；在 8788 请求 `GET /runner/tasks` → **404**。
4. `GET :8788/instance` 与 `GET :8789/instance` 都能看到同一 `device_serial`（共享配置）。

**V2 Studio 发布**
5. Studio 任务库 →「发布…」→ 返回 `path=dist/packages/任务2-v1.oaspkg`、`sha256`、`warnings=[]`。
6. 手工改包里 `task/flow.json` 后再发布 → 校验失败并给出 `T3`/`T7` 路径（不是裸异常）。
7. 重复发布同版本 → 拒绝，提示 `--force` 或升 `meta.version`。

**V3 Runner 安装**
8. Runner「安装包」→ 选 `任务2-v1.oaspkg` → 列表出现 `任务2` + `任务1`（依赖自动装）+ `installed.json` 两条。
9. 篡改包里任一图片后再装 → 拒绝（sha256 不匹配），`tasks/` 未被改动。
10. 卸载 `任务1`（被 `任务2` 依赖）→ **409** 且给出引用方。

**V4 配置**
11. Runner「任务2 → 配置」显示字段 `识别关键词`（label/说明/当前值 `突破`），改成 `结界` → 保存 → 刷新后仍在。
12. 填 `int` 字段越界值（如 `max_rounds=999` 超 `max`）→ **400**，文案指名 key 与范围。

**V5 调度与执行**
13. Runner「队列与调度」加 entry（`任务2`，periodic）→ 保存成功；`GET :8789/scheduler/status` 显示 `lock` 与下次唤醒。
14. 启动调度 → 运行记录出现一次 Run；`runs/scheduler/<run_id>.json` 有实体。
15. 运行日志/点击审计里能看到子任务步骤执行（父 flow 展开后），且**没有**「`param('触发文字')` has no value」。

**V6 子任务取父配置（核心）**
16. 把 `任务2` 的 `识别关键词` 设为 `结界` → 触发一次运行 → 任务1 的动作按 `结界` 走（用 `log` 步骤打出 `param('触发文字')` 核对）。
17. 在 Runner「临时运行 任务1」（不传参）→ 任务1 用的是**自己的** values/default（`""`）→ 触发 `required` 提示，而**不是**继承 `结界`。
18. 把 `任务2` 的映射改成 `inherit:"none"` 且不映射 → 任务1 的 `param('触发文字')` 报错，文案提示「在 call_task.params 里映射或给子任务 default」。

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

# E. 与现状文档的关系 / 开工条件

- **本文只是设计**，代码零改动。现状（Studio 仍是 `oasd serve` 一个 8788 干所有事、无 TaskPackage、无 Runner、子任务 default 不生效）**在实现前保持不变**。
- 台账登记：`PENDING-LEDGER.md §E-79`（状态 = 🔄 设计待拍板）。
- **开工条件**：用户逐项确认 §D.2 的 **Q1~Q7**（可全按推荐，也可改其中任意项）；确认后按 §D.3 **PR1→PR6** 顺序实施，每个 PR 用 §D.4 对应验收项收口。
- 若用户改其中某项（例如 Q1 改端口 / Q6 改子进程调度），先回写本文 §D.2 与稿源，再动代码。
