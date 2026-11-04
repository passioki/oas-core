# RESUME — 断点续接卡（session-memory 强制 6）

> 规则：本文件每次断点覆盖重写；≤150 行；另一个会话只读本文件 + `RESUME-INDEX.md` 最新行就能继续。
> 细节放 `docs/RUNNER-STUDIO-SPLIT-PLAN.md`（**§A.9 PR1 + §A.10 PR2 + §A.11 PR3 实现记录**）· `PENDING-LEDGER.md §E-79`（§F PR1 / §G PR2 / §H PR3）· `ENGINE-DESIGN.md §22.44/§22.45/§22.46`。

```yaml
task_id: oas-core-runner-studio-split
checkpoint_seq: 103
updated_at: 2026-11-04T20:5x+08:00
trigger: PR3（配置注入运行链路 + 类型保真 + 四层优先级 + 评审修复）实现/测试/E2E 完成 → docs 收尾（本子进程只改 docs）
status: in_progress
next_action: Lead 提交 PR3 的 4 个 Rust 文件 + 本批 docs（父 dfe8776）；随后开 PR4 子任务配置（call_task.params + inherit + with_params 作用域 + loader/runtime + lint T7）
```

## 0. 目标（本阶段）

1. **Q1~Q7 已批准**（2026-11-04，全部按推荐）——见计划文档 §D.2。
2. **PR1 ✅ 已提交**（`da9da9d`）：`Surface` 拆分 + `oasd studio|runner` + Runner 只读 API/UI 骨架 + Runner-only 发行包 + 跨平台约束。
3. **PR2 ✅ 已提交**（`dfe8776`）：`meta.params` 扩 5 字段 + `config/values.json` + Studio/Runner 配置 API/UI + 运行值校验。
4. **PR3 ✅ 已实现并验证（代码未提交）**：类型保真 + 四层优先级 + 合并后 schema 复检 + CLI 引号逃生口 + 测试 seam；范围**不含** PR4 子任务配置。
5. 本子进程只做 **docs 收尾 8 份**（不改代码/脚本/用户数据，不 commit、不跑 cargo/serve）。

## 1. 状态（2026-11-04 20:5x，seq 103）

| 项 | 值 |
|---|---|
| 分支 / HEAD | `dev`；HEAD = **`dfe8776`**（`docs: task config model + Runner runtime values final state (seq 102)`）；**PR3 代码/UI 未提交（4 个 Rust 文件，父 = `dfe8776`）**；本批 docs 提交后为新 HEAD（不写死 hash）；**未 tag / 未 release** |
| 任务状态 | **in_progress**（seq 103）；PR1 ✅ 已提交、PR2 ✅ 已提交、PR3 ✅ 已实现待提交、PR4~PR6 待做 |
| 双入口 | **Studio `http://127.0.0.1:8788/`**（`oasd studio`；`oasd serve` = deprecated 别名）· **Runner `http://127.0.0.1:8789/`**（`oasd runner`）；均只绑 `127.0.0.1` |
| 测试 | `cargo test --workspace` **583/0**（21 targets；PR2 562 → PR3 原始 +14 → 评审修复 +7）；oasd **276/0**（255→269→276）、task **72/0**、runtime **142** 不变；`cargo build -p oasd --features ocr` exit 0、release build exit 0（**6 warnings**，与基线一致）；UI pure test **1338 checks / 231 referenced ids / 0 dead ids**（PR3 无 UI 改动；`runner_ui_audit` **41 ids / 0 dead / 5 nav**）；`node --check` 3/3；`git diff --check` clean |
| 分包 | `dist/oasd-runner-v0.1.0-win64.zip` **37,189,387 B**（PR3 release 重建；Runner-only）+ SHA256 **BC2FFF90F72125885220198253178A051245341A4A16F136FAE34AB3534CCE0D** + `.sha256`；**不入 git** |
| 上一节点 | seq 102 = PR2 配置模型 + Runner 运行值 → 已提交 `dfe8776`（oasd 255/0、task 72/0、workspace 562/0、UI 1338/231/0、runner 包 37,192,710 B / SHA `A10DEDC1…B4533F5D`） |
| 工作区（非我改动，勿碰） | 代码：`apps/oasd/src/{run.rs,schedule.rs,queue.rs,serve.rs}`（PR3，未提交）；用户数据：`assets/assets.json`、`frames/frames.json`、`scheduler.json(.bak)`、`tasks/切换预设御魂/`；**均不提交用户数据** |
| 文档（本轮） | `RUNNER-STUDIO-SPLIT-PLAN.md`（状态 + **§A.11** + §C.3/§D.2/§D.3/§D.4/§E）、`PENDING-LEDGER §E-79`（状态/§D/§E + **§H.1~H.7** + 台账/footer）、`ENGINE-DESIGN`（§22.44/§22.45 指针 + **§22.46** 新增）、`RESUME`（本卡）、`RESUME-INDEX` 103、`START-HERE`、`SESSION-HANDOFF`、`README`（**8 份**） |

## 2. 验收清单（本轮 PR3）

- ☑ **类型保真**：`RunOptions.params` = `BTreeMap<String, serde_json::Value>`（`run.rs:134/139`）；CLI `--param` 是唯一文本猜类型点（`parse_param_value`）；`json_param_value` 唯一转 runtime；`entry_run_params` 直接 clone JSON；`{"code":"007"}` 端到端 `Str("007")`
- ☑ **四层优先级**：CLI `--param` > entry params > Runner values（`config/values.json`）> `meta.params.default`；`oasd run` 不读 values；scheduler/queue 由 `attempt_run_params` 注入（values 铺底、entry 覆盖，在 `mark_running` 前）；Studio `POST /task/run` body `params` 最高且**刻意不注入** values
- ☑ **合并后 schema 复检**：`run.rs:560-584` 在合并后、执行前调 `validate_param_values`；`meta.params` 为空则跳过；未知 key 带 allowed 列表、type/min/max/enum/required；败在任何 step/点击前
- ☑ **CLI 引号逃生口**：成对 `"` 按 JSON 字符串解析；PS 5.1 写法 `--param 'code=\"007\"'`；未加引号 `code=007` 仍是 number（schema `string` 会在复检被拒）
- ☑ **测试 seam**：`SchedulerOptions.values_path`（`schedule.rs:377`）/ `QueueOptions.values_path`（`queue.rs:44`，默认 None）；测试 pin 临时路径（hermetic）
- ☑ **测试**：workspace **583/0** / oasd **276/0** / task **72/0** / runtime **142** / UI **1338/231/0**（audit 41/0/5）/ OCR build + release build exit 0（6 warnings）/ `node --check` 3/3 / `git diff --check` clean
- ☑ **Lead E2E（临时 `pr3param` + replay）**：scheduler `Str("007")/Num(3.0)/Bool(true)/Str("runner")`、Done；CLI 引号成功 / 未引号 exit 2 + `7.0` schema 错误；坏 values 加 `junk` → 单 entry 失败、调度器不崩
- ☑ **Runner 包（PR3）**：37,189,387 B / SHA256 `BC2FFF90…534CCE0D`（仍 Runner-only）
- ☑ **P2 限制已写入文档**：旧字符串化配置迁移、`--param code=007`、`cli_params=` 语义、非标量 compact JSON、PR6 自行合并 values、坏 values 全局退出、Studio UI 参数输入未加、int/float runtime 不可区分（§A.11.7）
- ☐ **Lead 提交** PR3 代码（4 Rust 文件）+ 本批 docs（不含用户数据；`dist/` 不入 git）

## 3. 下一步（精确动作）

1. **Lead 提交 PR3**：`apps/oasd/src/{run.rs,schedule.rs,queue.rs,serve.rs}` + docs 本批 8 份；父 `dfe8776`。提交前复核 `git status` **不含** `assets/`、`frames/`、`tasks/`、`scheduler.json(.bak)`；`dist/` 与 `packages/` 不入 git。
2. **PR4 子任务配置**（下一步开发，计划文档 §C.4/§D.3）：`call_task` 增加 `params:{child_key:parent_expr}` + `inherit`（默认 `"all"`、`"none"` 关闭同名继承）；loader 展开时生成 `with_params` 作用域 step（`defaults` 只补缺、映射在调用方作用域求值）；runtime 加 `with_params` 分支（快照→求值→补默认→执行→**无论成败恢复**）；新增 `Flow::call_task_uses`；lint 新增 **T7**（目标任务存在 / 子任务 key 声明 / 父 key 声明 / inherit 合法；T3 语义不变）；`manifest.engine.flow_ops` 安装闸门。
3. **PR5**：`package.rs`（zip+manifest+checksums）+ `oasd studio publish` + `/runner/install|uninstall` + 素材合并 + `installed.json` + 裸目录纳管；**PR6**：内嵌调度线程 + 调度启停 + 设备租约（可延后；`/runner/run` **必须自行合并 values 层**）。
4. 每 PR 收口：`cargo test --workspace` 全绿（**PR3 后基线 583**）+ UI pure test（**1338/231/0**）+ `node --check` + `git diff --check`；每 PR 用计划文档 §D.4 对应验收项收口（PR1 已收 V1、PR2 已收 V4 的 11~12、PR3 无独立 V）。
5. **不 tag / 不 release**（除非用户明确说发布）。

## 4. 本轮裁定 / 约束（带锚点）

- **四层优先级语义按入口固定**：`oasd run` = CLI > default；scheduler task/queue = entry > values > default；`oasd queue` = values > default；Studio `POST /task/run` = body params > default（不读 values）；Runner `/runner/run` = PR6（§C.3、§A.11.2）。
- **Studio 刻意不依赖使用端运行值**：`config/values.json` 属 Runner；Studio 试运行只接受显式 body `params`；非对象 400（§A.11.2）。
- **CLI 是唯一文本猜类型点**：`true/false`→bool、有限数字→number、其余 string；字符串保真用成对引号（§A.11.1/§A.11.4）。
- **复检只在 `meta.params` 非空时生效**：空 schema 的旧任务/任意 `--param` 保持可注入；required 无 default 缺失现在会在运行前失败（§A.11.3）。
- **错误路径**：CLI exit 2；scheduler 单 entry 失败（`run_scheduler` 仍成功结束、`running` 清理）；queue exit 1；坏 `values.json` 全局 exit 2（§A.11.3/§A.11.7）。
- **`cli_params=` 现统计所有显式层**（CLI/entry/values/Studio body），字段名保留；不能再用它推断 CLI 参数个数（§A.11.7-③）。
- **旧配置迁移**：字符串化数字/布尔（`"3"`/`"true"`）不再猜回类型，改用原生 JSON（§A.11.7-①）。
- **已知限制**：非标量仍 compact JSON；int/float 在 runtime `Num(f64)` 不可区分；Studio UI 参数输入未加；PR6 临时运行需自行合并 values（§A.11.7）。
- **分发/数据边界**：终端用户只下载 Runner-only 包 + `.oaspkg`；用户数据（`assets/`/`frames/`/`tasks/`/`scheduler.json`）不提交；`dist/` 不入 git。

## 5. 阻塞 / 待用户裁定

- **无技术阻塞**。唯一等待项 = Lead 提交 PR3（边界见 §3.1 / 计划文档 §A.11 / ENGINE §22.46）。
- PR4 起按计划推进；若用户改 Q1~Q7 任一项，先回写计划文档 §D.2 与稿源再动代码。

## 6. 证据

- **PR3**：`D:\OAS-dev\_verify\runner-studio-pr3-20261104\`（`PR3-BACKEND-REPORT.md` / `REVIEW.md` / `FIX-REPORT.md` / `workspace-final.log` / `e2e/` + 本轮 `DOCS-REPORT.md`）。
- **PR2**：`D:\OAS-dev\_verify\runner-studio-pr2-20261104\`（实现/评审/测试报告 + `studio-config.png` / `runner-config.png` + `DOCS-REPORT.md`）。
- **PR1**：`D:\OAS-dev\_verify\runner-studio-pr1-20261104\`（`PR1-*-REPORT.md` / 测试日志 / `runner-*.png` / `studio-nav.png`）。
- **设计稿源**：`_verify\arch-runner-studio-20261104\RUNNER-STUDIO-CONFIG-DESIGN.md`（659 行 / 235+ `file:line`）。
- **台账**：`docs/PENDING-LEDGER.md §E-79`（§F PR1 / §G PR2 / **§H PR3**）；**现行口径**：`docs/RUNNER-STUDIO-SPLIT-PLAN.md`（§A.9/§A.10/§A.11）、`docs/ENGINE-DESIGN.md §22.46`、`packaging/CROSS-PLATFORM.md`、`packaging/README-DEPLOY.md`。

## 7. 连续性最小验证命令（恢复后先跑）

```text
cd D:\OAS-dev\oas-core
git log -1 --format="%h %ci %s"          # 期望 dfe8776（PR3 提交前）
git status --porcelain                   # 期望：PR3 4 个 Rust 文件 + docs 8 份 + 用户数据（勿提交 assets/frames/tasks/scheduler.json）
node apps\oasd\ui\ui_pure_test.js        # 期望 1338 checks / 231 ids / 0 dead（runner audit 41/0/5）
cargo test --workspace                   # 期望 583 passed / 0 failed（oasd 276 / task 72 / runtime 142）
```
