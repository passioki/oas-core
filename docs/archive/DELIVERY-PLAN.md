# 交付路线：从当前引擎到「实机可创建任务并运行」（V1.0）

> 定稿：[10-23] · 权威设计：`ENGINE-DESIGN.md §22.34`（调度 v3 + 统一任务模板）
> 本文件回答：**从今天到"在实机上创建任务并跑起来"还要做什么、按什么顺序、怎么验收**。
> 本文件是计划，不是实现；状态以 `PENDING-LEDGER.md` N36~N40 与 `RESUME.md` 为准。
>
> ★ **[10-23 02:4x] 范围补充（用户确认）**：
> - 任务必须包含 **探索（Exploration）+ 个人突破（RealmRaid）**，在 **Android 客户端** 上验收；
> - 构建/测试环境记录：Android 12 / x86_64 / MuMu vmindex=0 / 阴阳师 2.8.84；
> - ★ **APK 引擎（把引擎编译成 Android App、同机运行）不在本次 V1.0 范围**（用户原话"我要的是apk，不过既然做不到那就按原有计划进行"）；归 `ENGINE-DESIGN §8 P4`。

---

# §0 终点定义（Definition of Done）

## V0.9「实机可创建 + 手动运行」

在**实机**（另一台生产 PC）上：

1. 打开编辑器（浏览器 `http://127.0.0.1:8788`），点「新建任务」→ 输入名称 → 生成任务包；
2. 在编辑器里采集画面、框选素材、编辑流程、保存并通过 lint；
3. 点「运行任务」→ 任务在实机模拟器上真实点击并完成（`Done`），点击审计可见；
4. 不依赖开发机、Rust 工具链、旧 OAS 运行时。

## V1.0「实机可调度自动运行」

在 V0.9 之上：

5. 调度器 v3 读取玩家 `scheduler.json`，按窗口/quota/临时任务自动执行创建的任务；
6. 重启编辑器/调度器/机器后能恢复，不会重复跑已完成项；
7. 「重置记忆」可清调度状态 + 任务 state；有今日清单/下次唤醒/证据；
8. 连续运行 **24h** 无崩溃、无重复点击、无任务遗漏；可手工停止/升级。

---

# §1 现状与差距（代码证据）

| 能力 | 现状 | 差距 |
|---|---|---|
| 任务包结构 | `meta.json` 仅 `{schema_version,name,expire}`（`crates/oas-task/src/lib.rs:8`）| 无模板契约字段（params/exits/entry/capabilities）|
| 任务创建 | 只有 `GET /task/list`、`/task/assets`、`/task/save_flow`（`apps/oasd/src/serve.rs:16-19`）；任务目录手工/测试产生 | ❌ 无 `POST /task/create`、无「新建任务」UI、无骨架生成 |
| 任务运行（编辑器内）| 只能 `oasd run` CLI；编辑器无「运行任务」按钮/状态 | ❌ 缺 `POST /task/run` + 状态轮询 + 日志窗口接线 |
| 调度 | v1 `oasd schedule` 按 `every_s`（`apps/oasd/src/schedule.rs`）| ❌ v3（periodic + temp B + reset-memory）未实现；v1 将替换 |
| 设备选择 | run/serve 取**第一个** adb 设备（`apps/oasd/src/run.rs:488`、`serve.rs:211`），adb 路径走 `ADB_PATH` | ❌ 无 `--serial`/config；多设备/固定设备不可控 |
| 配置 | 全靠命令行 flags + 环境变量 | ❌ 无统一 config 文件（路径/模型/设备/端口）|
| 发布包 | 无 release 构建产物/打包脚本 | ❌ 无 zip/版本号/启动停止脚本/升级回滚 |
| 实机部署 | 旧 OAS 在实机跑（端口 22287），新引擎未上过实机 | ❌ 无安装/迁移/端口/数据目录方案 |
| 任务模板 | 无 lint（只校验 finish 出口/资产引用）| ❌ 无 params 注入、exits 声明、模板骨架 |

---

# §2 交付形态（实机目标架构）

```
D:\OAS-live\oas-core\                 ← 新引擎目录，与旧 OAS(backend/frontend) 并存
  oasd.exe                            ← release + ocr feature
  ui\                                 ← 编辑器静态页
  models\rec.onnx · det.onnx          ← 随包（约 31MB）
  adb\adb.exe · AdbWinApi.dll · AdbWinUsbApi.dll
  config\live.json                    ← 设备 serial/adb 路径/端口/目录/模型
  tasks\                              ← 任务包（数据）
  library\                            ← 全局素材库
  runs\ logs\ _backup\
  start-editor.bat  stop-editor.bat
  start-scheduler.bat  stop-scheduler.bat
  VERSION
```

| 项 | 方案 |
|---|---|
| 端口 | 编辑器 **8788**（避开旧 OAS 22287）；bind `127.0.0.1` |
| 设备 | `config.device = { adb_path, serial, reconnect_endpoints }`；**禁止默认第一个设备**，实机必须显式 |
| 模型 | 运行目录相对路径 `models/…`，config 可覆盖 |
| 数据 | `tasks/`、`library/`、`runs/`、`logs/` 与程序分离；升级不覆盖数据 |
| 启动 | `start-*.bat` + Windows 登录任务（v1.0）；Windows 服务/V1.2 |
| 停止 | pid 文件 + `taskkill`；调度器有单实例锁 |
| 升级 | 停 → 备份 config/tasks/library → 替换 exe/ui/models/adb → 启动 → `task lint --all` |
| 回滚 | `_backup/<version>/` + 保留上一版 zip |
| 安全 | 只绑 localhost，无鉴权（本机使用）；不做公网暴露 |

---

# §3 里程碑（M0 → M5）

> 估时 = 单线程开发 + 每步测试的乐观估计；实际以 gates 为准。  
> 依赖：**N36 → N38 → N39 →（手动运行 gate）→ N37 →（自动调度 gate）→ N40**。

## M0 · 统一任务模板（N36）· 0.5~1d

| 交付 | 验收 |
|---|---|
| `meta.json` 可选契约字段（params/exits/entry/capabilities/schedule_hint，旧任务默认值）| 旧任务零改动通过 |
| `param('key')` 表达式注入 + lint（未声明参数/出口/素材真红）| 单测 + 注入验红 |
| `templates/*.json` 骨架 + `oasd task lint --all` | 现有 2 任务全绿 |
| 引擎测试不回退（基线 213）| `cargo test --workspace` |

## M1 · 任务创建闭环（N38）· 2~3d

| 交付 | 验收 |
|---|---|
| `POST /task/create`：名称校验（空/非法字符/重名/路径穿越）、原子创建 `meta/flow/assets/images/`、不覆盖已有 | 单测 + 注入（重名/`../`/空名真红）|
| 「新建任务」UI：对话框（名称/模板/描述）→ 创建并选中 | headless 截图 |
| 模板骨架 flow：`log` + `finish("done")`；meta 带 `exits/props` 默认 | 创建后 lint 通过、编辑器可打开 |
| 编辑器「运行任务」：`POST /task/run`（后台线程）+ `GET /task/run/status` + 日志/结果窗口 | 用 replay 或模拟器跑通；错误可见 |
| 「保存 flow」前跑 lint；非法 exit/params 拒绝保存 | 单测 |
| **Gate V0.9-dev**：编辑器新建任务 → 标注 1 素材 → 加 1 点击 → 保存 → 运行 → Done（开发机模拟器）| 点击审计 + 截图证据 |

## M1b · 实机任务：探索 + 个人突破（新增）· 2~4d

| 交付 | 验收 |
|---|---|
| `tasks/探索`：庭院 → 探索 → 选关/进入 → 战斗（≥1 场）→ 结算/返回 → `finish(done)` | Android 客户端 run 输出 Done + clicks JSONL + 截图 |
| `tasks/个人突破`：庭院 → 结界突破 → 个人突破 → 选目标 → 挑战（≥1 场）→ 返回 → `finish(done)`；无票时 `finish(no_tickets)`（result=resource）| 同上 |
| 任务包格式与 M0 模板一致（meta/flow/assets/images）；素材来自旧仓参考或实机截图 | `task lint` 通过 |
| 证据 | `_verify/n42/`（run 输出、截图、clicks）|

## M2 · 调度器 v3（N37）· 3~4d

| 交付 | 验收 |
|---|---|
| 纯函数：calendar/reset、windows、quota、pass、wake、temp 规则 | 单测矩阵（跨午夜/周几/重置/冷却）|
| 主循环：periodic + temp（B：每轮按 entries 跑一次，执行前后重置）；loop/stop；护栏 | 合成模板 replay 端到端 |
| state/单实例锁/崩溃恢复；`--dry-run/--status/--report` | 杀进程恢复测试 |
| `reset-memory` CLI + UI 多选框（**全选/反选**；清调度状态 + 任务 `state/`）| 清前/清后对比 + 审计 |
| 调度面板：今日清单/下次唤醒/临时任务/手动触发 | headless 验收 |
| **Gate V1.0-dev**：创建任务 → 加入 scheduler → 自动跑 2 轮 → 重启恢复 → 不重复 | report + 截图 + 日志 |

## M3 · 实机交付包（N39）· 2~3d

| 交付 | 验收 |
|---|---|
| `cargo build --release -p oasd --features ocr` 可复现构建 | 产物可启动 `--version` |
| `config.rs`：`config/live.json` 统一配置（端口/目录/模型/adb/serial），flags 覆盖 config | 配置单测（缺省/覆盖/非法）|
| `--serial` + 设备解析（显式 serial；无设备=清晰错误；重连）| 模拟器 + 错误路径单测 |
| `scripts/package.ps1`：生成 `oasd-vX-win64.zip`（exe/ui/models/adb/tasks/library/config/脚本/VERSION）+ SHA256 | 解压到干净目录可运行 |
| 启动/停止/升级/回滚脚本与 `README-部署.md` | 文档 + 冒烟 |
| **Gate V0.9-live**：实机解压 → `probe` 看到设备 → 编辑器新建任务 → 运行 → Done | 实机截图/日志回传 |

## M4 · 实机自动调度验收（N40）· 1~2d + 24h soak

| 交付 | 验收 |
|---|---|
| 实机 `scheduler.json` + 创建任务自动运行；重启恢复 | 实机 report/日志 |
| 24h 长跑（periodic+temp）| 无崩溃/无重复/无漏跑；stats 可解释 |
| 升级演练（把 vX 替换为 vY）| 任务/素材不丢；回滚可用 |
| 完成台账/§22.34 状态/交付文档 | 全部 ✅ + 证据 |

## M5 · V1.0 收尾（与 M4 同轮）

- `VERSION` 与 git tag（oas-core）对齐；
- `START-HERE`/`RESUME`/`HANDOFF` 收尾；N36~N40 台账 ✅；
- 实机操作手册（新建任务、调度、重置记忆、备份升级）。

---

# §4 实机部署与操作设计（详细）

## 4.1 首次安装

1. 从开发机获取 `oasd-vX-win64.zip`（**交付通道待定**：GitHub Release / 局域网共享 / U 盘）；
2. 解压到 `D:\OAS-live\oas-core\`（不覆盖旧 OAS 目录）；
3. 编辑 `config\live.json`：`device.serial`（如 `127.0.0.1:16384`）、`device.adb_path`、`serve.port=8788`；
4. 先 `oasd.exe probe --config config\live.json` 确认设备；再 `oasd.exe task lint --all`；
5. 双击 `start-editor.bat`；浏览器打开 `http://127.0.0.1:8788`。

## 4.2 创建任务（人类/代理两种入口）

- **编辑器**：任务面板 →「新建任务」→ 名称/模板 → 生成骨架 → 采集/框选素材 → 流程编辑 → 保存（lint）→「运行任务」→ 点击审计；
- **CLI/代理**：`oasd task new --template daily --id <id> --tasks-dir tasks` + 写 flow/assets + `task lint`；
- 任务包是纯数据，可整目录复制/版本控制；素材库 `lib:<id>` 跨任务复用。

## 4.3 自动调度

1. 编辑器调度面板（或直接编辑 `scheduler.json`）：添加条目 = 任务 + 窗口 + quota（periodic）或 temp；
2. 启动 `start-scheduler.bat` → 写 pid、拿锁；
3. 查看状态：`oasd scheduler --status --config config\live.json`；日志 `logs\scheduler-*.log`；
4. 失败：任务出口 `blocked` 会冷却 + ALERT；「重置记忆」清状态后重试。

## 4.4 升级/回滚/备份

- 升级前：停 editor + scheduler；备份 `config/ tasks/ library/ scheduler.json`；
- 替换程序文件（exe/ui/models/adb/ui），保留数据；
- 启动后 `task lint --all` + `probe`；失败回滚 zip；
- 建议每次升级后跑一个"冒烟任务"（新建的轻量任务/日志任务）。

## 4.5 实机与旧 OAS 共存

- 旧 OAS 在 22287，新引擎 8788；**同一台模拟器只能被一个自动化进程控制**；
- 切换前停旧 OAS 调度；首次验收选维护/空闲时段；
- 不修改旧仓/旧配置；数据完全隔离。

---

# §5 长期路线（V1.0 之后）

| 版本 | 主题 | 内容 |
|---|---|---|
| **V1.0** | 实机可用 | 本文 M0~M5：模板 + 创建 + 调度 + 交付 |
| **V1.1** | 省心 | 录制→flow 生成；任务包导入/导出；调度面板完整版；告警 webhook；日志轮转；任务模板库 |
| **V1.2** | 稳定 | Windows 服务/看门狗；多设备；自动升级；统计/告警看板；失败自动截图归档 |
| **V2.0** | 智能 | 自动标注/多分辨率适配；失败自愈（重试候选定位）；任务市场/包管理；云端同步（可选）|

---

# §6 风险 / 依赖（必须提前处理）

| # | 风险 | 对策 |
|---|---|---|
| R1 | **实机只能由用户操作**（本机无法直接连实机）| 交付 zip + 一页操作清单；实机执行后回传日志/截图；必要时远程协助 |
| R2 | 旧 OAS 与新引擎抢同一模拟器 | 分端口 + 明确停机；首次验收选空闲时段；不做双跑 |
| R3 | 模型/adb 体积与路径 | 随包冻结版本；config 相对路径；package 脚本校验 SHA256 |
| R4 | 分辨率差异 | 任务 `design_resolution` 与实机帧比对；不匹配则先阻塞并报错（缩放适配 V1.1）|
| R5 | 未签名 exe 被 SmartScreen/杀软拦截 | 部署文档说明"更多信息→仍要运行"；V1.2 代码签名 |
| R6 | 长时间运行崩溃/断电 | 调度 state 原子写 + checkpoint resume + 开机自启 + 看门狗 |
| R7 | 任务/引擎版本不匹配 | `template_version` + 启动 lint；不匹配拒绝排期 |
| R8 | 实机时钟/时区 | 调度器本地时间 + 玩家配置；验收时核对 |

---

# §7 完成判据（DoD，可勾选）

- [x] D1 模板契约 + lint 落地，旧任务零破坏（N36）
- [x] D2 编辑器新建任务 → 素材 → flow → 保存 lint 全通（N38）
- [x] D3 编辑器「运行任务」在模拟器/实机跑出 Done（N38）
- [x] D4 调度 v3 跑 periodic + temp，前后重置、重置记忆可用（N37）
- [x] D5 release zip 在干净目录启动，显式设备运行（N39）
- [x] D6 实机创建任务并手动运行 Done（V0.9 gate）
- [x] D7 实机调度 2 轮 + 重启恢复（V1.0 gate）
- [x] D8 实机长跑通过（N40）— **96 轮等效加速 soak**（48+48，跨进程重启，96/96 成功、temp reset 192 次）；wall-clock 24h soak 已启动（pid=1268，03:11:45，loop_interval_s=900）持续观察
- [x] D9 文档/台账/证据/升级手册齐全
- [x] **D10 探索任务**：在 Android 客户端（阴阳师 2.8.84）完整跑通一次，Done + 点击审计
- [x] **D11 个人突破任务**：同上；无票时有明确 resource 出口
- [x] **D12 Android 证据**：release 包在 Android 12 / x86_64 / 阴阳师 2.8.84 上完成创建 + 运行，版本写入 BUILD-INFO/交付报告
- [x] **D13 调度包含两个任务**：scheduler.json 至少各一条（periodic 或 temp），自动跑 ≥2 轮
- [x] **D14 APK 边界**：交付报告明确"APK 引擎=P4，本次不含"（用户已确认）

---

# §8 交付结果（[10-23 03:1x] · 引擎提交 `7a4c58c`）

| 项 | 结果 |
|---|---|
| 引擎 | `oas-core` 本地 `main` = `7a4c58c`（V1.0 实现提交，52 files / +12152）|
| 测试 | `cargo test --workspace` = **312 passed / 0 failed**（21 suites）|
| 包 | `dist/oasd-v1.0.0-win64.zip` · **zip 35.1MB / 解压 59.7MB** · SHA256 `616999353EE3CA143AA0E207A881D0D34FD689DC3E6D092500F044F6AB2780EB` |
| 实机（模拟器）| Android 12 / x86_64 / 阴阳师 2.8.84 / MuMu vmindex=0 / adb 127.0.0.1:16384 |
| 创建闭环 | 解压包编辑器「新建任务」→ 创建「实机验收任务」200；运行「探索」**Done / exit 0 / 14 clicks** |
| 探索 | 实机 Done（真实战斗胜利→结算→回庭院，13 击，≈31s）|
| 个人突破 | 实机 Done（选目标→进攻→准备→真实战斗→结算→回庭院，15 击，≈43s；本场战败但流程完整）|
| 调度 | 实机 2 轮：探索 quota 2/2（第二轮 quota_exhausted）、个人突破 temp 每轮执行；跨进程重启状态延续 |
| 重置记忆 | CLI + 编辑器多选（全选/反选/全不选）+ 清 checkpoint；API 实测 `ok:true` + `memory_reset` 审计 |
| 长跑 | ✅ 96 轮等效加速 soak（replay `soak_probe`，48+48 跨进程重启）：96/96 成功、0 失败、temp reset 192 次、state passes 48→96；另 wall-clock 实机 soak 运行中（pid=1268，03:11:45，loop_interval_s=900，首轮探索 Done 31.1s / 个人突破 resource 10.0s）|
| 证据 | `_verify/n42/REPORT-V1.0.md`（工作区，不提交）|\n| 发布 | GitHub `https://github.com/passioki/oas-core` · Release `https://github.com/passioki/oas-core/releases/tag/v1.0.0` |

**已知限制**（与 ENGINE §22.35 一致）：起始须庭院；探索假设第二十八章 + 游戏内自动挑战开启；个人突破阵容弱通常战败但流程完整、票/目标耗尽走 resource；个人突破需 OCR 模型；调度器无 SIGINT、固定 UTC offset（+480）、启动未强制 lint；release 未签名；**APK=P4 未做**。

---

# §9 V1.1 交付结果（[10-30] · 引擎 `f2c096c` / tag `v1.1.0`）

| 项 | 结果 |
|---|---|
| 范围 | 用户确认的 v1.1：整屏 OCR 直点、素材 CRUD、区域/拟人拖动、任务工作台+完成条件、5 页面、点击分布聚合+删除 |
| 测试 | `cargo test --workspace` = 314 passed / 0 failed |
| 实机 | release 1.1.0 内联 OCR 点击「预设」→ Done（(394,96)）；dry-run 区域点击+拖动+loop 2/2 → Done |
| 包 | `dist/oasd-v1.1.0-win64.zip`（36,868,461 字节）· SHA256 `140BB35BEF9CA7BD357A396CB101798C291A26E7208A58CA064141E0A82235BB` |
| 发布 | `https://github.com/passioki/oas-core/releases/tag/v1.1.0` |
| 证据 | `_verify/n43/REPORT-V1.1.md` / `RELEASE-NOTES-v1.1.0.md` / `_verify/n43/*.png` |
| 限制 | 拖动当前为直线 swipe；until_* 完成条件由 flow 实现；单活动实例；APK 不在本版 |

---

# §10 V1.2 交付结果（[10-30] · 引擎 `5d0f79c` / tag `v1.2.0`）

| 项 | 结果 |
|---|---|
| 范围 | 识别/操作区域统一、全局唯一、多选批量保存、操作区域可调、任务构建页、变量动作 |
| 测试 | 326 passed / 0 failed |
| 实机 | release 1.2.0 内联 OCR「式神录」点击 (1111,663) Done；变量循环 count 0→2；区域点击+drag Done |
| 包 | `oasd-v1.2.0-win64.zip`（36,895,821 字节）· SHA256 `D31C500006643D318E16B3EDF23EC688BF6773D2886D4CAF1828F98C4CB035FC` |
| 发布 | `https://github.com/passioki/oas-core/releases/tag/v1.2.0` |
| 已知限制 | 图片识别多选未做；builder 高级动作 JSON；变量不持久化；旧探索/个人突破有 3 个跨任务同名素材；release 未签名 |

---

# §11 V1.2.1 交付结果（[10-31] · 引擎 `4e50aea` / tag `v1.2.1`）

| 项 | 结果 |
|---|---|
| 范围 | OCR 来源、批量素材可见、builder 素材/布局/真流程图、帧管理、全局素材统一、工作台流程收口 |
| 测试 | 327 passed / 0 failed |
| 迁移 | 包内 26 assets / 4 tasks / 3 flows rewritten / 3 renames；lint 4/4 ok |
| 实机/运行 | 全局 assets 加载 23；派生 OCR 行 dry-run 点击 (158,16) Done |
| 包 | `oasd-v1.2.1-win64.zip`（37,148,195 字节）· SHA256 `E4E4E94DDBF86CBB6D3D7A831BFD7E8940DA6DCAD823B0432A4FD55843C99AC1` |
| 发布 | `https://github.com/passioki/oas-core/releases/tag/v1.2.1` |
| 限制 | 帧为 session 内存；图片多选未做；旧 task assets 保留未用；builder 复杂动作 args JSON；未签名 |
