# RESUME — 断点续接卡（session-memory 强制 6）

> 规则：本文件每次断点覆盖重写；≤150 行；另一个会话只读本文件 + `RESUME-INDEX.md` 最新行就能继续。
> 细节放 `docs/RUNNER-STUDIO-SPLIT-PLAN.md`（设计稿 v1）· `PENDING-LEDGER.md §E-78/§E-79` · `EDITOR-UX-PLAN.md §18.10` · `ENGINE-DESIGN.md §16.5/§22.43`。

```yaml
task_id: oas-core-runner-studio-split
checkpoint_seq: 100
updated_at: 2026-11-04T18:25+08:00
trigger: 三项遗留修复（region keep_action_shape / 模板 polygon 平移 / OCR 取值路径阈值）+ Studio·Runner 分离与任务配置设计稿 v1 落 docs
status: in_progress
next_action: 等用户确认设计稿 §D.2 的 7 项决策（Q1~Q7，含推荐）；确认后按 PR1→PR6 实施
```

## 0. 目标（本阶段）

1. **收尾三项遗留**（本阶段代码已落、未提交）：① region 识别区保存不再把操作多边形降级为矩形；② 模板 ROI **移动**保留 polygon 形状；③ OCR **取值路径**应用每素材阈值。
2. **产出设计稿**（事项 2/3，**只设计不实现**）：Studio / Runner 分离 + TaskPackage + 任务配置（运行期参数）/ 子任务配置。
3. **落 docs**：新建计划文档（1 份）+ 8 份现行文档订正 + 断点/入口同步；**不 commit**（Lead 提交）。

## 1. 状态（2026-11-04 18:2x）

| 项 | 值 |
|---|---|
| 分支 / HEAD | `dev`；HEAD = **`f16fea1`**（`chore(data): sync all resources/tasks/scheduler/code/docs 2026-11-04 15:08`）；**本轮代码未提交，代码父 = `f16fea1`**，docs 随后（新 HEAD）；hash 以 `git log` 为准；**未 tag / 未 release** |
| 任务状态 | **in_progress**（seq 100）；代码/docs 待 Lead 提交后 done；**设计稿待拍板**（不是阻塞，是等用户输入） |
| 本轮代码 | **5 文件**：`apps/oasd/src/serve.rs`、`apps/oasd/ui/app.js`、`apps/oasd/ui/ui_pure_test.js`、`crates/oas-runtime/src/lib.rs`、`crates/oas-task/src/lib.rs`（`asset_ui.js`/`index.html`/`style.css` 未改） |
| 测试 | `cargo test --workspace` **527 passed / 0 failed**（runtime **142** / task **63** / oasd **229**；516→527，+11 = 取值路径阈值 6（runtime）+ region keep_action_shape 2 与模板 polygon 平移 2（task）+ serve 布尔解析 1（oasd））；UI pure test **1138 checks / 227 referenced ids / 0 dead ids**（1112→1138，+26）；`node --check` 3/3 = 0；`git diff --check` = 0 |
| 上一节点 | seq 99 素材库 UX 六项 + OCR 阈值（代码父 `b4c2b0c`，评审 P0×2/P1×4 全修，516/0、UI 1112/227/0） |
| 工作区（非我改动，勿碰） | `assets/assets.json`、`frames/frames.json`、`scheduler.json`、`scheduler.json.bak`、`tasks/切换预设御魂/`（用户数据）；代码 5 文件由另一子进程写入 |
| 文档（本轮） | 新建 `RUNNER-STUDIO-SPLIT-PLAN.md`；改 `EDITOR-UX-PLAN`（§18.3/§18.4/§18.8/§18.9 + 新增 §18.10）、`ENGINE-DESIGN`（§16.5 + 新增 §22.43）、`PENDING-LEDGER`（§E-78 订正 + 新增 §E-79）、`RESUME`、`RESUME-INDEX`、`START-HERE`、`SESSION-HANDOFF`、`README`（**9 份**） |

## 2. 验收清单（本阶段）

- ☑ region `keep_action_shape` 落地：`oas-task::update_asset_meta` 可选布尔（缺省走旧 B-08 rect 同步）+ serve 解析（非布尔 400）+ UI region 保存传 `true` + region ROI 帧内校验
- ☑ 模板 polygon 平移：`assetRoiPolygonPlan`（`dx/dy` 平移、bbox==新 ROI、不越界）+ 回退矩形时明确提示（缩放/旧 bbox 不一致/越界）
- ☑ OCR 取值路径阈值：`ocr_value_read` 接入 `ocr()`/`ocr_digit()`/`ocr_to_var`（低分 → `Null`/空串；未设/0 不变；`wait_until` 轮询保留；多行最低行分）
- ☑ 测试：workspace **527/0**、UI **1138/227/0**（Lead 实测；本子进程独立复跑 UI pure test = 1138/227/0 一致）、`node --check` = 0、`git diff --check` = 0
- ☑ 设计稿 v1：`docs/RUNNER-STUDIO-SPLIT-PLAN.md`（架构/包/配置/风险/7 决策/6 PR/19 步验收）
- ☑ docs 残留检查：顶层文档不再有「region 识别区会降级 / 模板 polygon 重同步矩形 / OCR 取值不设阈值」的**现行**说法（均标注已修）
- ☐ **用户拍板设计稿 §D.2 的 7 项决策**（Q1~Q7）← **当前唯一等待项**
- ☐ Lead 提交本轮代码（5 文件，父 `f16fea1`）→ 提交 docs（新 HEAD）→ `push-all.ps1`

## 3. 下一步（精确动作）

1. **向用户呈报 7 项决策**（`docs/RUNNER-STUDIO-SPLIT-PLAN.md §D.2`）：Q1 `oasd runner`+8789 / Q2 `serve` 留 deprecated 别名 / Q3 zip+`.oaspkg` / Q4 优先级 CLI>entry>values>default / Q5 同名继承+子 default 兜底（`inherit:"none"` 可关）/ Q6 内嵌调度线程 / Q7 发布产物不入 git。用户可全按推荐，也可逐项改；**改哪项先回写计划文档 §D.2 与稿源**。
2. 用户确认后按 **PR1→PR6** 实施（每 PR 的验证点见 §D.3）：PR1 `Surface`+`required_surface`+`oasd studio/runner`+`ui/runner.*`+8787→8788；PR2 `meta.params` 5 字段 + `config/values.json`；PR3 `RunOptions.params` 改 `Value` + 四入口；PR4 `call_task.params`/`inherit`+`with_params`+lint T7；PR5 `package.rs`+publish+install；PR6 内嵌调度/设备租约（可延后）。
3. 每个 PR 收口：`cargo test --workspace` 全绿（当前基线 **527**）+ UI pure test（当前 **1138/227/0**）+ `node --check` + `git diff --check`；**不提交用户数据**（`assets/`/`frames/`/`tasks/`/`scheduler.json`）。

## 4. 本轮新裁定 / 约束（带锚点）

- **region 的「操作区=绝对区域」语义**：识别区（`roi`）移动**不得**改写 `action_shape` ⇒ `keep_action_shape`（`ENGINE-DESIGN §22.43`、`EDITOR-UX-PLAN §18.10`）。模板 `fixed` 操作区**仍不随 ROI 平移**（同类问题，未改，见 §E-78 §H-4）。
- **模板 polygon 只在「移动」时平移**：缩放 → 回退矩形 + 提示（本轮需求只要求移动）。
- **OCR 阈值现在也作用于取值路径**，但低分是**空结果不是硬错误**（保 `wait_until` 轮询；要硬错误改 `ocr_value_read` 一处）。
- **Studio/Runner 分离不拆二进制**：加 `Surface` + 路由归属表；**Studio 无调度页 / Runner 无创作 API**；数据目录**共享**（单机单实例）。
- **子任务 `call_task` 是 load-time 展开** ⇒ 本期不做「子任务一等运行单元」（风险 R1，`RUNNER-STUDIO-SPLIT-PLAN §D.1`）。

## 5. 阻塞 / 待用户裁定

- **只差用户拍板**：设计稿 §D.2 的 Q1~Q7（推荐已给）。不属技术阻塞；未拍板前**不动代码**。
- 无需其他外部输入（设备/权限/账号均不涉及本轮）。

## 6. 证据

- **本轮三项遗留**：`D:\OAS-dev\_verify\leftovers-20261104\`（`DOCS-REPORT.md` = 逐处 before→after + 残留检查 + 设计稿整理说明；其余报告/日志由实现/回归子进程产出）。
- **设计稿稿源**：`D:\OAS-dev\_verify\arch-runner-studio-20261104\RUNNER-STUDIO-CONFIG-DESIGN.md`（659 行 / 235+ `file:line`）。
- **上一节点 seq 99**：`D:\OAS-dev\_verify\asset-ux-20261104\`（UX/OCR 阈值/评审/修复报告 + 截图）。
- **台账**：`docs/PENDING-LEDGER.md §E-78`（已修 + 保留遗留）、**§E-79**（设计稿待拍板）；**现行口径**：`docs/RUNNER-STUDIO-SPLIT-PLAN.md`、`docs/EDITOR-UX-PLAN.md §18.10`、`docs/ENGINE-DESIGN.md §16.5/§22.43`。
- **历史**：调度器 P5 `_verify/p5-realdevice-20261103/`；帧页 `_verify/ui-frames-20261103/`；0.1.0 `_verify/release-v0.1.0/`。

## 7. 连续性最小验证命令（恢复后先跑）

```text
cd D:\OAS-dev\oas-core
git log -1 --format="%h %ci %s"          # 期望 f16fea1（本轮未提交前）
git status --porcelain                   # 期望：代码 5 文件 + docs 9 份 + 用户数据（assets/frames/scheduler.json/tasks/…）
node apps\oasd\ui\ui_pure_test.js        # 期望 1138 checks / 227 ids / 0 dead
cargo test --workspace                   # 期望 527 passed / 0 failed（runtime 142 / task 63 / oasd 229）
```
