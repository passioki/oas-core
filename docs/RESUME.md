# RESUME — 断点续接卡（session-memory 强制 6）

> 规则：本文件每次断点覆盖重写；≤150 行；另一个会话只读本文件 + `RESUME-INDEX.md` 最新行
> 就能继续。细节放 `EDITOR-UX-PLAN.md §18` / `PENDING-LEDGER.md §E-78` / `ENGINE-DESIGN.md §16.5·§22.42`。

```yaml
task_id: oas-core-asset-ux-ocr-threshold
checkpoint_seq: 99
updated_at: 2026-11-04T14:52+08:00
trigger: 素材库 UX 六项（行选中态/菜单保持、识别区画布拖拽+共享保存、模板 replace 统一、帧详情与全局库一致、批量测试移页）+ OCR 每素材最低置信度；只读评审 P0×2/P1×4 全修；UI 1112/227/0 · workspace 516/0
status: in_progress
```

## 0. 目标（本轮）

用户的 **素材库 UX 六项** + **OCR 阈值（新增能力）**；实现后由只读评审（`REVIEW.md`）发现 2 个 P0 + 4 个 P1，本轮全部修复并回归。**代码与 docs 均未提交**；用户数据未动。

## 1. 当前状态（2026-11-04 14:5x）

| 项 | 值 |
|---|---|
| 分支/HEAD | `dev`；HEAD = **`b4c2b0c`**（seq 98 docs 提交）；本轮代码未提交，**代码父 = `b4c2b0c`**，docs 随后（新 HEAD）；hash 以 `git log` 为准；未 tag/release |
| 任务状态 | **in_progress**（seq 99；代码/docs 待 Lead 提交后 done） |
| 本轮代码 | UI **5 文件** `apps/oasd/ui/{asset_ui.js,app.js,index.html,style.css,ui_pure_test.js}` + Rust **2 文件** `crates/oas-runtime/src/lib.rs`、`crates/oas-task/src/lib.rs` |
| 测试 | UI pure test **1112 checks / 227 referenced ids / 0 dead ids**（907→1003→1112；旧断言不弱化）；`cargo test --workspace` **516 passed / 0 failed**（runtime **136** / task **59** / oasd **228**；501→512→516，新增 15 条 = 阈值 11 + 0/expect 三态 4，原 11 条保留）；`cargo build -p oasd --features ocr` **exit 0**（6 个既有 warnings）；task lint **1/0**；`node --check` 3/3；`git diff --check` clean |
| 上一节点 | seq 98 素材行两按钮 + 一键提交/推送脚本（HEAD `b4c2b0c`）；seq 97 帧页布局回归、seq 95 调度器 P5 |
| 工作区 | **用户数据不提交**：`assets/assets.json`、`assets/images/a00{2..5}.png`、`frames/frames.json`、`frames/frame-4{1,2}.png`、`scheduler.json`、`scheduler.json.bak`、`tasks/关闭弹窗/` |
| 文档 | 本轮改 8 份：`EDITOR-UX-PLAN`（§16.2/§17 订正 + 新增 §18）· `PENDING-LEDGER`（§E-76/§E-77 取代说明 + 新增 §E-78）· `ENGINE-DESIGN`（§16.5 + §22.42）· `RESUME` · `RESUME-INDEX` · `START-HERE` · `SESSION-HANDOFF` · `README` |

## 2. 六条需求（现行）

| # | 需求 | 现行行为 |
|---|---|---|
| 1 | 行选中态 / 菜单保持 | 选中按 **asset id** 跨 surface 高亮（`.asset-row.selected`，背景 `rgb(27,42,58)`）；点行 main/识别/设置都选中；**点菜单项不收起**；仅选另一个素材时关旧菜单并迁高亮；再次「设置」/Esc 收起；disabled 不改变选中/不回调；重渲染恢复选中并延迟恢复已选行菜单 |
| 2 | 识别区 = 画布拖拽 | 菜单「识别区」→ 载入原 ROI、进入 ROI 拖拽（**不再 prompt**）；保存按钮移到**共享 `#canvas-footer`**，旁有「识别区编辑中：<素材id>」；进入编辑后画布底部显示「保存识别区（覆盖素材）」；实时开启时自动暂停并提示 |
| 3 | 模板识别区统一 | 模板 ROI = 保存时裁剪/搜索几何（模板图 = ROI 截图）；旧服务端拒绝模板 ROI。现同一画布手势：**模板保存走 `/task/save_asset` + `replace:true`（当前帧重新截图覆盖，rev+1）**；OCR/region 走 `/task/asset/update {roi}`；confirm 带 id/rev/位置帧数/不可撤销 |
| 4 | OCR 阈值 | OCR 素材复用 `assets.json` `threshold` = **最低识别置信度**；未设/0 = 不过滤（旧行为），`t>0` 时 `score<t` = 未命中；模板/collection 默认 0.9 不变；详见 §3 |
| 5 | 帧详情与全局库一致 | 删除「来源（全局/任务）」列/badge 与 `assetSourceLabel`；`manage:true`，9 项设置全可用（不再灰）；行 scope：含 global → `GLOBAL_SCOPE_TASK`（global 优先），仅 task → 当前任务；所有回调支持可选 `scopeTask` |
| 6 | 批量帧测试移页 | `#btn-assets-batch-test` 从任务页移到素材页、紧邻「刷新全局库」；对象 = **当前可见素材集合**（全局 = 过滤后 `globalAssetsCache`；任务 = 可见任务素材含 show-all），显式传 assets ids；空集合文案按 surface 区分；全局模式 task=`GLOBAL_SCOPE_TASK` |

## 3. OCR 阈值语义（权威）

- **字段**：`AssetDecl.threshold` 复用；模板 = NCC 匹配线，OCR = 最低识别置信度。
- **未设 / 0 = 不按置信度过滤**（与旧行为一致）；Rust `ocr_threshold` 读取时 `filter(>0)`，显式 0 与缺失都 `None`（P1-4）。`t>0` 时 `score<t` 视为未命中/失败。
- **生效路径**：click、条件 appear、`expect appear/vanish`、`click_target` 的 `ocr_text`、expect `ocr_text`、drag `ocr_text`。
- **不生效**：`ocr()` / `ocr_digit()` / `ocr_to_var` 取值路径（设计：不把「读到的值」变成「没读到」）。
- `expect appear/vanish` 用 `selector_threshold`：OCR 未设/0 → **0.0**，显式 `t>0` → `t`；模板/collection 仍 0.9/显式；`click_target` 模板兜底同策略。
- `ocr_lines` / derived 判定用**最高行分**；适配层已丢弃 `<0.5` 的行 ⇒ `<0.5` 阈值无额外效果。
- UI：模板未设预填 **0.85**；OCR 未设/0 显示「未设或 0（不限制）」、默认空；prompt「留空 = 取消；0 = 不限制」。

## 4. 评审 P0/P1 修复

- **P0-1**：保存按钮原先只在 `panel-assets` 内 → 移到共享 `#canvas-footer`（`index.html:64-69` / `style.css:571-589`），任务库/帧详情都可见；新增 `#asset-roi-edit-label`。
- **P0-2**：切素材/切帧后仍写旧素材（模板 replace 会数据损坏）→ 新增 `cancelRecognitionRoiEdit(reason)`，覆盖 pickAssetRoi/切换编辑目标/rename/threshold/positions/delete/collection/action/test/step/saveAssetUnified/selectFrame/deleteFrame/clearFrames/实时开关/Esc/清除区域/切任务/画布模式切换；保存前按 `{id,scope}` 重查**最新**素材，找不到即取消。
- **P1-1/P1-2**：模板 replace payload 缺 polygon/OCR 字段、用编辑起点快照 → `assetRoiReplacePayload` 以最新素材为底带全字段，只换 roi + `replace:true`；polygon bbox 与新 ROI 一致则保留，否则回传新 ROI 矩形。
- **P1-3**：批量测试改为当前可见集合（不再 task flow 子集），显式 assets ids。
- **P1-4**：OCR 0 与未设不等价 → Rust 归一化（`ocr_threshold` 过滤 `>0`；expect appear/vanish 用 `selector_threshold`；click_target 模板兜底同策略；其余路径走归一化访问器）。
- **P2 已做**：Esc/切帧/实时开关取消编辑、实时开时进入 ROI 编辑自动暂停并提示、重渲染恢复已选菜单、模板 positions 空前端拦截、ROI 按当前画布帧越界校验、覆盖 confirm 信息、识别区菜单 title 说明模板重截。

## 5. 测试与 E2E

- UI：**1112 / 227 / 0**（907→1003 实现→1112 修复后）；`node --check` 3/3。
- Rust：**516/0**（501→512 阈值→516 修复）；runtime 133→136、task 58→59、oasd 228；新增 15 条，原 11 条保留。
- 构建/静态：OCR build exit 0（6 既有 warnings）；task lint 1/0；`git diff --check` clean。
- **Lead CDP 实测**（真实 serve 8787 + device）：行选中背景 `rgb(27,42,58)`；设置→识别区后菜单保持展开、画布底部保存按钮可见可用、标签「识别区编辑中：预设」；切到另一素材后按钮隐藏/禁用、标签清空；模板识别区 title 含「模板：保存会用当前帧重新截图覆盖，rev+1」；OCR 阈值 prompt 显示「未设或 0（不限制）」且默认空；帧详情行无来源文案、9 项设置全 enabled；批量按钮在 `panel-assets`、上一兄弟「刷新全局库」、不在 `panel-task`。截图 `_verify/asset-ux-20261104/ux-roi-shared.png` 等。
- 边界：**无真实浏览器自动化回归**（CDP 实测 + UI pure tests）。

## 6. 遗留（P2/P3，不阻塞）

1. `region` 识别区保存仍会把 polygon/invert 降级为矩形（P2-5，服务端既有路径）；
2. 模板 ROI 移动后 polygon **有意**重同步为矩形（避免旧 bbox 反推覆盖，不是回归）；
3. `ocr()`/取值路径不加阈值（设计）；`ocr_lines` 用最高行分、适配层丢 `<0.5` 行；
4. legacy 非全局模式批量测试仍需任务名；
5. 帧详情 task-only 且无任务时仍报错（P2-10）；
6. 菜单恢复在极端多列表同时可见时可能开在最后可见 surface（不丢选中）；
7. 无真实浏览器自动化回归（本轮 Lead CDP 实测 + UI pure tests）；
8. docs 已同步本轮（本任务）。

## 7. 下一步（= 产品 backlog）

1. **`SelectionTool` 抽取**（结构债，唯一剩下的 P1 级重构）。
2. **collection `real=1` 真机 E2E**（MuMu `127.0.0.1:16384` 已可用）。
3. **drag Plan B**（asset 引用/运行时跟随识别，替代生成时 shape 快照）。
4. **P2-10**：保存 flow 时校验解析错误（此前明确不做；帧详情 task-only 无任务报错同源）。
5. **运行记录/口径收口**：点击聚合关联、分页/保留、run_id 跨进程唯一、detail 契约、`--status` 的 `run_file_errors`；事件 attempt/entry、stats vs report、UI 失败口径。
6. **本轮 P2/P3**：region ROI polygon 保留、脚本 `OutputEncoding`、`library/` 版本化决策、真实浏览器自动化 E2E。
7. **版本决策**：是否升 0.2.0（用户定）；墙钟 24h soak 仍未跑完（只有 96 轮加速 soak 成立）。

## 8. 证据

- **本轮**：`D:\OAS-dev\_verify\asset-ux-20261104\`（`UX-REPORT.md` · `OCR-THRESHOLD-REPORT.md` · `REVIEW.md` · `FIX-REPORT.md` · `THRESH-FIX-REPORT.md` · `ux-selected.png` / `ux-roi-shared.png` / `ux-frame.png` · `probe-*.js` · `*.log`）+ `DOCS-REPORT.md`（文档逐处 before→after + 残留检查）。
- **上一节点 seq 98**：`D:\OAS-dev\_verify\asset-row-20261104\`（`ASSET-ROW-REPORT.md`/`REVIEW.md`/`FIX-REPORT.md`/截图/`DOCS-REPORT.md`）。
- **台账**：`docs/PENDING-LEDGER.md §E-78`（+ §E-77/§E-76）；**现行口径**：`docs/EDITOR-UX-PLAN.md §18`（素材库 UX）、`docs/ENGINE-DESIGN.md §16.5`（OCR 阈值）。
- **历史**：调度器 P5 `_verify/p5-realdevice-20261103/`；帧页 `_verify/ui-frames-20261103/`；O-1/O-2 `_verify/release-v0.1.0/`。

## 9. 节点（seq 99）

本节点 seq 99：**素材库 UX 六项 + OCR 阈值**——UI 5 文件 + Rust 2 文件；评审 P0×2/P1×4 全修 + P2 顺手；UI **907→1003→1112/227/0**，workspace **501→512→516/0**（runtime 136 / task 59 / oasd 228）；代码父 `b4c2b0c`，docs 随后（新 HEAD）。上一节点 seq 98：素材行两按钮 + push-all（已提交 `b4c2b0c`）。
