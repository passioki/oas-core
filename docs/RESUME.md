# RESUME — 断点续接卡（session-memory 强制 6）

> 规则：本文件每次断点覆盖重写；≤150 行；另一个会话只读本文件 + `RESUME-INDEX.md` 最新行
> 就能继续。细节放 `ENGINE-DESIGN.md` / `SCHEDULER-RUNNER-PLAN.md` / `EDITOR-UX-PLAN.md §16` / `PENDING-LEDGER.md §E-76`。

```yaml
task_id: oas-core-editor-frames
checkpoint_seq: 97
updated_at: 2026-11-04T01:0x+08:00
trigger: 帧页布局回归（用户报告「分类展示图片帧的区域页面没了」）：根因 fixed-height flex + frames-list min-height:0 ⇒ 1440x900 下 0px；修复 min-height 200 / details 默认折叠 / 点帧展开 / 空搜索隐藏状态行；UI 835/225/0（无 Rust 改动，workspace 仍 501/0）
status: in_progress
```

## 0. 目标（本轮）

修复并收口「图片页丢失按分类展示图片帧的区域」布局回归：诊断根因 → 修复 → Lead CDP 实测 → docs 同步。seq 96 的帧页三项需求（帧搜索 / 帧详情素材库 / 素材测试帧错位修复）**口径不变**。

## 1. 当前状态（2026-11-04 01:0x）

| 项 | 值 |
|---|---|
| 分支/HEAD | `dev`；HEAD = 本轮 docs 提交（父 = 本轮代码提交；代码提交父 = **修复前 HEAD**，即 seq 96 docs 提交）；hash 以 `git log` 为准；未 tag/release |
| 任务状态 | **in_progress**（seq 97；代码 + docs 待 Lead 提交后 done） |
| 本轮代码 | 仅 UI 4 文件 `apps/oasd/ui/{app.js,index.html,style.css,ui_pure_test.js}`（未提交；Lead 先提交代码，再提交 docs） |
| 测试 | `cargo test --workspace` = **501 passed / 0 failed**（21 binary；oasd **228**；**无 Rust 改动**，本轮不新增 Rust 测试）；UI pure test **835 checks / 225 referenced ids / 0 dead ids**（828→835，+7）；`node --check app.js/ui_pure_test.js` exit 0；task lint **1/0**；`git diff --check` clean |
| 上一节点 | seq 96：编辑器帧页三项需求 + 素材测试帧错位修复（已提交；UI 828/224/0，workspace 501/0） |
| 工作区 | 用户数据改动**不属于本轮提交**：`assets/assets.json`、`frames/frames.json`、`assets/images/a002.png`/`a003.png`/`a004.png`、`frames/frame-41.png`/`frame-42.png`、`scheduler.json`、`scheduler.json.bak`、`tasks/关闭弹窗/` |
| 文档 | 本轮改 7 份：`EDITOR-UX-PLAN`（§16 标题 + §16.5 + 新增 §16.6）· `PENDING-LEDGER`（§E-76 触发/改动/§I/测试/台账/G）· `RESUME` · `RESUME-INDEX` · `START-HERE` · `SESSION-HANDOFF` · `README`（当前基线同步） |

## 2. 本轮：布局回归（已修，未提交）

### 2.1 用户报告

「图片页面，丢失了原先按分类展示图片帧的区域页面。」帧分类组仍在数据里，但页面上看不到。

### 2.2 根因（实测确认）

上一轮给帧页新增搜索框 / 搜索状态行 / 帧详情 / 该帧素材列表（约 **+296px** 固定内容），而 `#panel-images` 是固定高度 flex 列（`height: calc(100vh - 150px)`），`#panel-images .frames-list` 规则为 `flex: 1; min-height: 0` ⇒ **1440×900 下帧列表计算高度 = 0px**：12 个分类组 / 30 行内容仍在（`scrollHeight=1832`）但不可见。

### 2.3 修复（UI 4 文件）

- `style.css`：`#panel-images .frames-list { flex: 1 1 auto; min-height: 200px; overflow-y: auto; }`（帧列表永不为 0）；新增 `#frame-details` / summary / `[open]` 样式。
- `index.html`：`#frame-detail` + `#frame-assets-status` + `#frame-assets-list` 包进 `<details id="frame-details" class="frame-details">`，summary「帧详情 / 该帧素材（点击一帧展开）」，**默认折叠**（不再挤占帧列表高度）。
- `app.js`：`selectFrame()` 成功后 `frameDetails.open = true`（点帧即展开该帧详情/素材）；`renderFramesList()` 在搜索词为空时隐藏 `#frame-search-status`（查询时才显示，省一行）。
- `ui_pure_test.js`：+7 checks（`<details>` 结构 / summary 文案、`selectFrame` 展开、空查询隐藏状态行、fake DOM `style`/`display` 断言）。

### 2.4 验证（Lead，CDP 1440×900）

- 修复前：帧列表 `height=0`（12 组已渲染但不可见）→ 修复后：`height=257px`、12 组可见、details 折叠；
- 点击一帧 #1 → details 打开、详情文本 /「该帧素材 1 条（全局 1 · 任务 0）」/ 1 行素材，帧列表仍 **200px** 可见；
- 截图：`D:\OAS-dev\_verify\ui-frames-20261103\frames-fixed.png`（分组列表恢复）、`frames-fixed-detail.png`（点帧展开素材）；
- `node --check` 两文件 exit 0；`git diff --check` clean；无 Rust 改动 ⇒ workspace 仍 **501/0**。

## 3. 现行口径（seq 96，不变）

| 场景 | 用的帧 |
|---|---|
| 单素材测试（匹配/点测/拖测/OCR/集合） | **当前选中帧**（帧详情行 = 该行帧） |
| 批量帧测试 | **所有位置帧**（`POST /task/assets/batch_test`） |
| 实时 = 开 | 实时帧，**恒优先**；真实点击强制 live |

- 帧搜索纯前端（名称 / `#id` / 分类）；帧详情 = 全局 + 任务素材中 `positions` 命中该帧者，按 id 去重；
- **没有「单素材多帧」入口**；旧「实时关=位置帧 / 单素材多位置逐个测」已被 `EDITOR-UX-PLAN §16` + `PENDING-LEDGER §E-76` 取代；
- `/ocr` 协议无 bbox ⇒ **单行 OCR 没有真实检测框**（多行/derived 才有 det 框）；
- `frame_source` 线上仍回 `position:#<id>`，UI 标签显示「当前帧 #N」（P2 文案待统一）。

## 4. 证据

- **本轮布局**：`D:\OAS-dev\_verify\ui-frames-20261103\frames-fixed.png` · `frames-fixed-detail.png` · CDP 1440×900 实测（0→257px / 12 组可见 / 点击展开 1 条素材）· `DOCS-LAYOUT-REPORT.md`（文档逐处 before→after + 残留检查）。
- **上一轮（seq 96）**：同目录 `MATCH-BUG-DIAGNOSIS.md` · `FRAMES-UI-REPORT.md` · `MATCH-FIX-REPORT.md` · `REVIEW-UI.md` · `FIX-P1-REPORT.md` · `lead-final-ui.log`（828/224/0）· `lead-final-workspace.log`（501/0）· `api-*.json` · `match-23/26.json` · `editor-page.png`。
- **台账**：`docs/PENDING-LEDGER.md §E-76`（§A~§I + 台账表）；**计划/现行口径**：`docs/EDITOR-UX-PLAN.md §16`（§16.6 = 布局回归）。
- **历史**：调度器 P5 `_verify/p5-realdevice-20261103/`；O-1/O-2 `_verify/release-v0.1.0/`。

## 5. 下一步（= 产品 backlog，不变）

1. **`SelectionTool` 抽取**（结构债，唯一剩下的 P1 级重构）。
2. **collection `real=1` 真机 E2E**（O-1 遗留；MuMu 已可用）。
3. **drag Plan B**（asset 引用/运行时跟随识别，替代生成时 shape 快照）。
4. **P2-10**：保存 flow 时校验解析错误（此前明确不做）。
5. **运行记录/口径收口**：点击聚合关联、分页/保留、run_id 跨进程唯一、detail 契约、`--status` 的 `run_file_errors`；事件 attempt/entry、stats vs report、UI 失败口径。
6. **帧页遗留（P2/P3）**：帧搜索 Enter/防抖、分类+搜索状态计数跨分类、纯数字查询既精确又子串、全局模式来源重复计数、`frame_source` 文案、切帧不清 ROI、单行 OCR 画布无框、`renderAssetRows` DRY 复制；**仍无真实浏览器自动化 E2E**（P3）。
7. **版本决策**：是否升 0.2.0（用户定）；墙钟 24h soak 仍未跑完（只有 96 轮加速 soak 成立）。

## 6. 环境（E11~E14，详见 START-HERE §3）

- PowerShell **5.1**（执行策略 Undefined）；`node` 不在 PATH，用 bundled node（v24.21.0）；PS 重定向日志 UTF-16LE；中文文件用 `read`/`edit`/`write`。
- 实机：seq 96 serve **8787**、真机 `emulator-5554`（与 P5 的 MuMu `127.0.0.1:16384` 不同，以 `adb devices` 实测为准）；`GET /instance/devices` 会自动 `adb connect`，响应带 `auto_connect`。

## 7. 节点（seq 97）

本节点 seq 97：**帧页布局回归修复 + docs 收尾**——根因 `#panel-images` 固定高 flex + frames-list `min-height:0` ⇒ 1440×900 下 0px；修复 = `min-height:200px` + `#frame-details` 默认折叠 + 点帧展开 + 空搜索隐藏状态行；UI **828→835 checks / 224→225 ids / 0 dead**，workspace 仍 **501/0**（无 Rust 改动）；代码 4 文件（父 = 修复前 HEAD）+ docs（新 HEAD）由 Lead 分两次提交。上一节点 seq 96：编辑器帧页三项需求 + 素材测试帧错位修复（已提交，UI 828/224/0）。
