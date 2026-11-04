# RESUME — 断点续接卡（session-memory 强制 6）

> 规则：本文件每次断点覆盖重写；≤150 行；另一个会话只读本文件 + `RESUME-INDEX.md` 最新行就能继续。
> 细节放 `docs/RUNNER-STUDIO-SPLIT-PLAN.md`（**§A.9 PR1 + §A.10 PR2 + §A.11 PR3 + §A.12 PR4 实现记录**）· `PENDING-LEDGER.md §E-79`（§F PR1 / §G PR2 / §H PR3 / **§I PR4**）· `ENGINE-DESIGN.md §22.44/§22.45/§22.46/§22.47`。

```yaml
task_id: oas-core-runner-studio-split
checkpoint_seq: 104
updated_at: 2026-11-04T21:5x+08:00
trigger: PR4（子任务配置语义：call_task.params/inherit + with_params + loader/runtime + T7 + T6/严格 meta/checkpoint 指纹）实现/评审修复/测试/E2E 完成 → docs 收尾（本子进程只改 docs）
status: in_progress
next_action: Lead 提交 PR4 的 7 个 Rust 文件 + 本批 docs（父 048dff5）；随后开 PR5 TaskPackage 发布/安装 + engine.flow_ops 闸门
```

## 0. 目标（本阶段）

1. **Q1~Q7 已批准**（2026-11-04，全部按推荐）——见计划文档 §D.2。
2. **PR1 ✅ 已提交**（`da9da9d`）：`Surface` 拆分 + `oasd studio|runner` + Runner 只读 API/UI 骨架 + Runner-only 发行包 + 跨平台约束。
3. **PR2 ✅ 已提交**（`dfe8776`）：`meta.params` 扩 5 字段 + `config/values.json` + Studio/Runner 配置 API/UI + 运行值校验。
4. **PR3 ✅ 已提交**（代码 `c09df25` + seq 103 docs `048dff5`）：类型保真 + 四层优先级 + 合并后 schema 复检 + CLI 引号逃生口 + 测试 seam。
5. **PR4 ✅ 已实现并验证（代码未提交）**：子任务配置语义（§C.4 方案 A）+ 评审 P1 处理 + lint T7 + checkpoint 指纹。
6. 本子进程只做 **docs 收尾 8 份**（不改代码/脚本/用户数据，不 commit、不跑 cargo/serve）。

## 1. 状态（2026-11-04 21:5x，seq 104）

| 项 | 值 |
|---|---|
| 分支 / HEAD | `dev`；HEAD = **`048dff5`**（`docs: typed params + four-layer config priority final state (seq 103)`）；**PR4 代码未提交（7 个 Rust 文件，父 `048dff5`）**；本批 docs 提交后为新 HEAD（不写死 hash）；**未 tag / 未 release** |
| 任务状态 | **in_progress**（seq 104）；PR1 ✅ / PR2 ✅ / PR3 ✅ 已提交、**PR4 ✅ 已实现待提交**、PR5~PR6 待做 |
| 双入口 | **Studio `http://127.0.0.1:8788/`**（`oasd studio`；`oasd serve` = deprecated 别名）· **Runner `http://127.0.0.1:8789/`**（`oasd runner`）；均只绑 `127.0.0.1` |
| 测试 | `cargo test --workspace` **624/0**（21 targets；PR3 583 → PR4 618 → 评审修复 624）；oas-flow **62**（57→62）、oas-task **89**（72→89）、oas-runtime **157 lib + 2 integration**（142→157）、oasd **280**（276→280）；OCR build + release build exit 0（**6 warnings**）；UI pure test **1338 checks / 231 ids / 0 dead**（PR4 无 UI 改动；`runner_ui_audit` **41/0/5**）；`git diff --check` clean |
| 分包 | `dist/oasd-runner-v0.1.0-win64.zip` **37,246,079 B**（PR4 release 重建；Runner-only）+ SHA256 **07B6D930FA8EBEE29DBAA0F2C594347097916A36EABED073E4FD12D7852E506B** + `.sha256`；**不入 git** |
| 上一节点 | seq 103 = PR3 配置注入 + 类型保真 → 已提交 `c09df25` + `048dff5`（oasd 276/0、task 72/0、runtime 142、workspace 583/0、UI 1338/231/0、runner 包 37,189,387 B / SHA `BC2FFF90…534CCE0D`） |
| 工作区（非我改动，勿碰） | 代码：`crates/oas-flow/src/lib.rs`、`crates/oas-task/src/lib.rs`、`crates/oas-runtime/src/lib.rs`、`apps/oasd/src/{task.rs,run.rs,checkpoint.rs,schedule.rs}`（PR4，未提交）；用户数据：`assets/assets.json`、`frames/frames.json`、`scheduler.json(.bak)`、`tasks/切换预设御魂/`；**均不提交用户数据** |
| 文档（本轮） | `RUNNER-STUDIO-SPLIT-PLAN.md`（状态 + **§A.12** + §C.4/§C.4.6 订正 + §D.2/§D.3/§D.4/§E）、`PENDING-LEDGER §E-79`（状态/§D/§E + **§I.1~I.8** + 台账/footer）、`ENGINE-DESIGN`（§22.44/§22.45 指针 + **§22.47** 新增）、`RESUME`（本卡）、`RESUME-INDEX` 104、`START-HERE`、`SESSION-HANDOFF`、`README`（**8 份**） |

## 2. 验收清单（本轮 PR4）

- ☑ **DSL**：`Step::CallTask{task, params, inherit}`（`inherit` 缺省 `All`，只接受 `"all"`/`"none"`；`params` 必须对象且值为表达式字符串）；旧无参 `call_task` 兼容
- ☑ **内部 op**：`Step::WithParams{label, params, defaults, block, body}`（args `task`/`params`/`defaults`/`block`/`body`）；不落盘，作者只写 `call_task`；所有遍历器递归 `body`
- ☑ **collector**：`Flow::call_task_uses()` 返回 task/inherit/child→Option<parentKey>/computed_params/path
- ☑ **loader 展开**：读目标 flow（预解析）+ 目标 meta defaults → 递归展开 + strip finish → 内存 `with_params{task,params,defaults,block,body}`；**不落盘**
- ☑ **严格失败**：目标 meta 缺失/坏 JSON/坏 default/目标 flow 无法解析 → loader 硬错误（`call_task \`目标\` (called from \`调用者\`)`）；不再 warning 降级；`load_flow_with_warnings` 保留但恒空
- ☑ **子默认生效**：子任务 `meta.params[].default` 参与运行（旧无参调用也获得）；根 defaults 仍归 run.rs
- ☑ **runtime**：snapshot → 调用方求值 → `inherit:none` 按 block 临时移除父同名 → defaults 只补缺 → overrides 覆盖（映射 > 同名继承 > 子默认）→ exec(body, depth+1) → **成功/失败/break/continue/finish 全路径恢复**
- ☑ **上下文**：label 栈让缺参错误带 `subtask '<label>'`；日志 `subtask '<label>' (N step(s))`；嵌套 A→B→C / 同父多调用点正确；子任务 finish 仍 strip
- ☑ **T7**：`TaskBundle::lint_with_calls` / `lint_into_with_calls(_and_assets)` + CLI `task lint --tasks-dir`；目标存在/meta+flow 可解析/child key ⊆ 目标声明/父 key ⊆ 父声明/inherit 合法；**允许常量映射**；计算 `param('a'+'b')` 拒绝；T3 不变；T7 不查环、未接 serve
- ☑ **T6 修复（P1）**：`args.params`/`args.defaults` 的 **key 豁免**（`code` 等参数名不误报），但值仍按 `CODE_ESCAPE_PREFIXES` 检测（`"shell: rm -rf"` 仍 T6）
- ☑ **checkpoint 指纹（P1）**：`load_flow_with_fingerprint` = 展开后 JSON 的 64-bit FNV-1a；`Checkpoint.flow_fingerprint`；name/steps/指纹三元校验；**旧格式 checkpoint 安全拒绝**（需删除）；子任务改动（step 数不变）使旧断点失效
- ☑ **测试**：workspace **624/0** / flow **62** / task **89** / runtime **157+2** / oasd **280**；UI **1338/231/0**（audit 41/0/5）；OCR build + release build exit 0（6 warnings）；`git diff --check` clean
- ☑ **Lead E2E（临时任务 + replay，不碰用户数据）**：`parent→child` 映射/父同名继承覆盖子默认/子默认；`parent_none`（inherit none）父同名被屏蔽；`parent_multi` 两调用点互不影响；ASCII lint T7 `errors=0`；T7 三类错误带路径
- ☑ **限制已写文档（§A.12.8 / §I.8 / §22.47.8）**：PR5 flow_ops 闸门、checkpoint 粒度、T7 边界、meta key ASCII、子任务 finish strip、T6 前缀启发式
- ☐ **Lead 提交** PR4 代码（7 文件）+ 本批 docs（不含用户数据；`dist/` 不入 git）

## 3. 下一步（精确动作）

1. **Lead 提交 PR4**：`crates/oas-flow/src/lib.rs`、`crates/oas-task/src/lib.rs`、`crates/oas-runtime/src/lib.rs`、`apps/oasd/src/{task.rs,run.rs,checkpoint.rs,schedule.rs}` + docs 本批 8 份；父 `048dff5`。提交前复核 `git status` **不含** `assets/`、`frames/`、`tasks/`、`scheduler.json(.bak)`；`dist/` 与 `packages/` 不入 git。
2. **PR5 TaskPackage 发布/安装**（下一步开发，计划文档 §B/§D.3）：`package.rs`（`.oaspkg` zip + `manifest.json` + `checksums.sha256`）+ `oasd studio publish` + `/runner/install|uninstall` + 素材合并 + `installed.json` + 裸目录纳管。
3. **PR5 必做 `manifest.engine.flow_ops` 安装闸门**：从**原始 flow** 的 `call_task.args.{params,inherit}` 推导 flow_ops（不能扫描 `with_params`，它不落盘），老 Runner 不支持则拒绝安装并提示升级；PR4 前新包不要发给老 Runner。
4. **PR6**（可延后）：内嵌调度线程 + 调度启停 + 设备租约 + 临时运行（`/runner/run` **必须自行合并 values 层**，body params 覆盖 values）。
5. 每 PR 收口：`cargo test --workspace` 全绿（**PR4 后基线 624**）+ UI pure test（**1338/231/0**）+ `node --check` + `git diff --check`；按 §D.4 对应验收项收口。
6. **不 tag / 不 release**（除非用户明确说发布）。

## 4. 本轮裁定 / 约束（带锚点）

- **子任务默认继承**：未映射 key 走「同名继承 → 子任务 default → 响亮失败」；`inherit:"none"` 关闭同名继承（block = 目标声明 key 全集）。默认 `all` 是为保持旧任务行为（§C.4.4、§A.12.3）。
- **映射求值在调用方作用域**：映射 entries 之间不互相引用；显式映射覆盖同名继承（§A.12.3）。
- **严格 loader**：坏 meta/default/目标 flow = 硬错误，不再 warning 降级；`inherit:none` 不会因坏 meta 静默退化为 `all`（§A.12.2）。
- **checkpoint 失效**：展开后指纹纳入校验；旧格式（无 `flow_fingerprint`）明确拒绝，需删除旧 `state/checkpoint.json`；子任务改动会让旧断点失效（§A.12.6）。
- **T7 边界**：只查直接边、不查环；只保证目标 meta/flow 可读且可解析；允许常量映射；serve 创建/校验尚未接（§A.12.4）。
- **`with_params` 是内部实现**：不落盘；手写会被 T3 检查 body 参数 key（作者应只写 `call_task`）（§A.12.8-③）。
- **meta param key 仍须 ASCII**（PR2 规则；任务名可中文）；中文 param key 会报 META（§A.12.8-④）。
- **分发/数据边界**：终端用户只下载 Runner-only 包 + `.oaspkg`；用户数据（`assets/`/`frames/`/`tasks/`/`scheduler.json`）不提交；`dist/` 不入 git。

## 5. 阻塞 / 待用户裁定

- **无技术阻塞**。唯一等待项 = Lead 提交 PR4（边界见 §3.1 / 计划文档 §A.12 / ENGINE §22.47）。
- PR5 起按计划推进；若用户改 Q1~Q7 任一项，先回写计划文档 §D.2 与稿源再动代码。

## 6. 证据

- **PR4**：`D:\OAS-dev\_verify\runner-studio-pr4-20261104\`（`PR4-BACKEND-REPORT.md` / `REVIEW.md` / `FIX-REPORT.md` / `workspace-fix.log` / `e2e/` + 本轮 `DOCS-REPORT.md`）。
- **PR3**：`D:\OAS-dev\_verify\runner-studio-pr3-20261104\`（`PR3-BACKEND-REPORT.md` / `REVIEW.md` / `FIX-REPORT.md` / `workspace-final.log` / `e2e/` + `DOCS-REPORT.md`）。
- **PR2**：`D:\OAS-dev\_verify\runner-studio-pr2-20261104\`（实现/评审/测试报告 + `studio-config.png` / `runner-config.png` + `DOCS-REPORT.md`）。
- **PR1**：`D:\OAS-dev\_verify\runner-studio-pr1-20261104\`（`PR1-*-REPORT.md` / 测试日志 / `runner-*.png` / `studio-nav.png`）。
- **设计稿源**：`_verify\arch-runner-studio-20261104\RUNNER-STUDIO-CONFIG-DESIGN.md`（659 行 / 235+ `file:line`）。
- **台账**：`docs/PENDING-LEDGER.md §E-79`（§F PR1 / §G PR2 / §H PR3 / **§I PR4**）；**现行口径**：`docs/RUNNER-STUDIO-SPLIT-PLAN.md`（§A.9~§A.12）、`docs/ENGINE-DESIGN.md §22.47`、`packaging/CROSS-PLATFORM.md`、`packaging/README-DEPLOY.md`。

## 7. 连续性最小验证命令（恢复后先跑）

```text
cd D:\OAS-dev\oas-core
git log -1 --format="%h %ci %s"          # 期望 048dff5（PR4 提交前）
git status --porcelain                   # 期望：PR4 7 个 Rust 文件 + docs 8 份 + 用户数据（勿提交 assets/frames/tasks/scheduler.json）
node apps\oasd\ui\ui_pure_test.js        # 期望 1338 checks / 231 ids / 0 dead（runner audit 41/0/5）
cargo test --workspace                   # 期望 624 passed / 0 failed（flow 62 / task 89 / runtime 157+2 / oasd 280）
```
