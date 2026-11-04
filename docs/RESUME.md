# RESUME — 断点续接卡（session-memory 强制 6）

> 规则：本文件每次断点覆盖重写；≤150 行；另一个会话只读本文件 + `RESUME-INDEX.md` 最新行就能继续。
> 细节放 `docs/RUNNER-STUDIO-SPLIT-PLAN.md`（**§A.9 PR1 + §A.10 PR2 实现记录**）· `PENDING-LEDGER.md §E-79`（§F PR1 / §G PR2）· `ENGINE-DESIGN.md §22.44/§22.45`。

```yaml
task_id: oas-core-runner-studio-split
checkpoint_seq: 102
updated_at: 2026-11-04T19:5x+08:00
trigger: PR2（任务配置模型 + Runner 运行值）实现/评审修复/测试/E2E 完成 → docs 收尾（本子进程只改 docs）
status: in_progress
next_action: Lead 提交 PR2 代码/UI + 本批 docs（父 da9da9d）；随后开 PR3 优先级与类型保真（RunOptions.params 改 Value + 四层合并 + entry_run_params 不猜类型）
```

## 0. 目标（本阶段）

1. **Q1~Q7 已批准**（2026-11-04，全部按推荐）——见计划文档 §D.2。
2. **PR1 ✅ 已提交**（`da9da9d`）：`Surface` 拆分 + `oasd studio|runner`（8788/8789）+ Runner 只读 API/UI 骨架 + Runner-only 发行包 + 跨平台约束。
3. **PR2 ✅ 已实现并验证（代码未提交）**：`meta.params` 扩 5 字段 + `config/values.json` + Studio/Runner 配置 API/UI + 运行值校验；范围**不含** PR3 的优先级注入/类型保真。
4. 本子进程只做 **docs 收尾 8 份**（不改代码/脚本/用户数据，不 commit、不跑 cargo/serve）。

## 1. 状态（2026-11-04 19:5x，seq 102）

| 项 | 值 |
|---|---|
| 分支 / HEAD | `dev`；HEAD = **`da9da9d`**（`docs: Studio/Runner split PR1 implemented + approved decisions + distribution/cross-platform corrections (seq 101)`）；**PR2 代码/UI/`.gitignore` 未提交，父 = `da9da9d`**；本批 docs 提交后为新 HEAD（不写死 hash）；**未 tag / 未 release** |
| 任务状态 | **in_progress**（seq 102）；PR1 ✅ 已提交、PR2 ✅ 已实现待提交、PR3~PR6 待做 |
| 双入口 | **Studio `http://127.0.0.1:8788/`**（`oasd studio`；`oasd serve` = deprecated 别名）· **Runner `http://127.0.0.1:8789/`**（`oasd runner`）；均只绑 `127.0.0.1` |
| 测试 | `cargo test --workspace` **562/0**（PR1 538，**+24** = meta 9 + runner_config 7 + serve 8）；oasd **255/0**、task **72/0**、runtime **142** 不变；`cargo build -p oasd --features ocr` exit 0（6 warnings，与基线一致）；UI pure test **1338 checks / 231 referenced ids / 0 dead ids**（PR1 1239/224/0，+99；`runner_ui_audit` **41 ids / 0 dead / 5 nav**）；`node --check` 3/3；`git diff --check` clean |
| 分包 | `dist/oasd-runner-v0.1.0-win64.zip` **37,192,710 B**（PR2 release 重建；Runner-only）+ SHA256 **A10DEDC1A5D0FAB15289ACBB9F904C5909AFA148B3317D2408350B12B4533F5D** + `.sha256`；**不入 git** |
| 上一节点 | seq 101 = Q1~Q7 批准 + PR1 实现（代码/脚本/UI + docs 8 份）→ 已提交 `da9da9d`（oasd 240/0、workspace 538/0、UI 1239/224/0、runner 包 37,141,295 B） |
| 工作区（非我改动，勿碰） | 代码：`apps/oasd/src/{runner_config.rs(新),serve.rs,main.rs}`、`crates/oas-task/src/{meta.rs,lib.rs}`、`.gitignore`；UI：`apps/oasd/ui/{index.html,app.js,runner.html,runner.js,runner.css,ui_pure_test.js}`；用户数据：`assets/assets.json`、`frames/frames.json`、`scheduler.json(.bak)`、`tasks/切换预设御魂/`；**均不提交用户数据** |
| 文档（本轮） | `RUNNER-STUDIO-SPLIT-PLAN.md`（状态 + §A.10 + §C.1~C.3 + §D.3/D.4 + §E）、`PENDING-LEDGER §E-79`（状态/§E/§G/台账/footer）、`ENGINE-DESIGN`（§22.44 指针 + **§22.45** 新增）、`RESUME`（本卡）、`RESUME-INDEX` 102、`START-HERE`、`SESSION-HANDOFF`、`README`（**8 份**） |

## 2. 验收清单（本轮 PR2）

- ☑ **ParamDecl 五字段**（`meta.rs`）：`label`(≤80) / `description`(≤500) / `enum`(JSON `enum`，仅 string/int/float/number，非空/去重/类型匹配/`default∈enum`) / `required` / `group`(≤80)；全 `Option + skip_serializing_if`，老 meta 零破坏；lint 扩展
- ☑ **`validate_param_values`**（lib 导出）：未知 key（给允许列表）/ type / min / max / enum / `required:true && default==None` 缺失；错误带 `params[i].field` + key
- ☑ **values 存储**（`apps/oasd/src/runner_config.rs`，`main.rs:30` 挂载）：`config/values.json`（live config 同目录；无 `--config` 回落 `CWD/config/values.json`）；缺文件=空、坏 JSON/schema 不符=带路径 Err；RMW + Mutex + tmp/rename 原子、保留其它任务；纯 std Path
- ☑ **Studio API**：`GET /task/config?task=`（坏任务名 400 / 缺失 404 / 坏 meta 400）、`POST /task/config`（原始 meta 只替换 params、写前 `meta.json.bak`、400 带 `params[i].field`）；Runner 请求 404
- ☑ **Runner API**：`GET/PUT /runner/tasks/<id>/config`；PUT 校验失败 400 + `errors` 含 key 且**不写盘**；成功返回 values/updated_ms；Studio 请求 404
- ☑ **Studio UI**：任务库「配置项…」→ `#task-config-dialog` JSON 编辑器（2 空格）、本地 parse 失败不发请求、服务端 errors 逐行、成功刷新任务库；纯函数 `taskConfigParamsToJson`/`parseTaskConfigParamsText`/`taskConfigSummary`
- ☑ **Runner UI**：已安装任务「配置」→ `#runner-config-dialog` 表单（int/float/number→number、bool→checkbox、string+enum→select、string→text；label/description/required/min/max/enum 提示；初始值 `values[key] ?? default`）；PUT 保存；纯函数 4 个；无 group 分组（P2）
- ☑ **评审**：无 P0；P1-1（模块迁移/提交范围验证）已解决（workspace 562/0）；P2 修复：`meta` 未用变量警告、Studio 写 meta 前 `.bak`、`.gitignore` 增 `/config/values.json`/`*.tmp`/`/tasks/*/meta.json.bak`
- ☑ **测试**：workspace **562/0** / oasd **255/0** / task **72/0** / UI **1338/231/0** / audit 41/0/5 / `node --check` 3/3 / `git diff --check` clean / OCR build exit 0（6 warnings）
- ☑ **Lead E2E（临时 `pr2demo`，Studio 8790 / Runner 8791）**：Studio GET/POST 200、meta 其它字段保留、中文 label 往返、生成 `meta.json.bak`；Runner GET/PUT 200、中文 `结界` 往返、未知 key/缺 required/超 max/类型错 400 含 key、两端 config 404；CDP：Studio 对话框 + Runner 表单（结界/2 → 庭院/4）
- ☑ **Runner 包（PR2）**：37,192,710 B / SHA256 `A10DEDC1…B4533F5D`（含新 runner.html/js/css；仍 Runner-only）
- ☐ **Lead 提交** PR2 代码/UI + 本批 docs（不含用户数据；`dist/` 不入 git）

## 3. 下一步（精确动作）

1. **Lead 提交 PR2**：代码 `crates/oas-task/src/{meta,lib}.rs`、`apps/oasd/src/{runner_config.rs,serve.rs,main.rs}`、`.gitignore`；UI 6 文件（`index.html`/`app.js`/`runner.html`/`runner.js`/`runner.css`/`ui_pure_test.js`）；docs 本批 8 份；父 `da9da9d`。提交前复核 `git status` **不含** `assets/`、`frames/`、`tasks/`、`scheduler.json(.bak)`。
2. **PR3 优先级与类型保真**（下一步开发，计划文档 §D.3/§C.3）：`RunOptions.params` 改 `BTreeMap<String, Value>`；`run.rs` 读 `config/values.json` 作第 3 层（CLI > entry > values > `meta.default`）；`entry_run_params` 不再字符串化后猜类型（`{"code":"007"}` 保持字符串）；合并后按 `meta.params` **防御性复检**（错误带任务名+key）。
3. **PR4**：`call_task.params`/`inherit` + loader 生成 `with_params` + runtime params 栈 + `Flow::call_task_uses` + lint T7。
4. **PR5**：`package.rs`（zip+manifest+checksums）+ `publish` + `/runner/install|uninstall` + 素材合并 + `installed.json` + 裸目录纳管；**PR6**：内嵌调度线程 + 调度启停 + 设备租约（可延后）。
5. 每 PR 收口：`cargo test --workspace` 全绿（**PR2 后基线 562**）+ UI pure test（**1338/231/0**）+ `node --check` + `git diff --check`；每 PR 用计划文档 §D.4 对应验收项收口（PR1 已收 V1、PR2 已收 V4 的 11~12）。

## 4. 本轮裁定 / 约束（带锚点）

- **配置 schema 由 Studio 独占编辑**：`POST /task/config` 只替换 `params`、保留 meta 其它/未知字段、写前 `.bak`（计划文档 §C.1、§A.10.3）。
- **运行值由 Runner 独占写**：`config/values.json`，Studio 不写；单写者 + 原子 RMW（§C.2、§A.10.2）。**多进程同时写不安全**。
- **PR2 不做优先级注入**：`run.rs` 仍不读 values；四层合并/类型保真 = PR3（§C.3、§A.10.8）。不要按「Runner 配置已影响运行」验收。
- **已知限制（P2/P3）**：坏 meta 的 `GET /task/config` 400；POST 回写 key 字典序 + `min/max` 整数序列化 `1.0`；values 无 `.bak`/rev；`meta_error` 行配置按钮/UI 迟到响应未守卫；`group` 未渲染（§A.10.8）。
- **分发**：终端用户只下载 Runner-only 包 + `.oaspkg`；Studio 不发给终端用户（§A.8.1）。**跨平台**：v1 只 Windows，Runner 核心/API/任务包/schema 平台无关（§A.8.2）。
- **PR1 遗留校正**：旧 `#page-schedule` DOM/对话框**仍未删**（PR2 未覆盖，顺延 PR6）；`.gitignore` 的 values/tmp/bak 已由 PR2 加入。

## 5. 阻塞 / 待用户裁定

- **无技术阻塞**。唯一等待项 = Lead 提交 PR2（边界见 §3.1 / 计划文档 §A.10 / ENGINE §22.45）。
- PR3 起按计划推进；若用户改 Q1~Q7 任一项，先回写计划文档 §D.2 与稿源再动代码。

## 6. 证据

- **PR2**：`D:\OAS-dev\_verify\runner-studio-pr2-20261104\`（实现/评审/测试报告 + `studio-config.png` / `runner-config.png` + 本轮 `DOCS-REPORT.md`）。
- **PR1**：`D:\OAS-dev\_verify\runner-studio-pr1-20261104\`（`PR1-*-REPORT.md` / 测试日志 / `runner-*.png` / `studio-nav.png`）。
- **设计稿源**：`_verify\arch-runner-studio-20261104\RUNNER-STUDIO-CONFIG-DESIGN.md`（659 行 / 235+ `file:line`）。
- **台账**：`docs/PENDING-LEDGER.md §E-79`（§F PR1 / **§G PR2**）；**现行口径**：`docs/RUNNER-STUDIO-SPLIT-PLAN.md`（§A.9/§A.10/§C.1~C.3/§D.3/§D.4）、`docs/ENGINE-DESIGN.md §22.44/§22.45`、`packaging/CROSS-PLATFORM.md`、`packaging/README-DEPLOY.md`。

## 7. 连续性最小验证命令（恢复后先跑）

```text
cd D:\OAS-dev\oas-core
git log -1 --format="%h %ci %s"          # 期望 da9da9d（PR2 提交前）
git status --porcelain                   # 期望：PR2 代码/UI + docs 8 份 + 用户数据（勿提交 assets/frames/tasks/scheduler.json）
node apps\oasd\ui\ui_pure_test.js        # 期望 1338 checks / 231 ids / 0 dead（runner audit 41/0/5）
cargo test --workspace                   # 期望 562 passed / 0 failed（oasd 255 / task 72 / runtime 142）
```
