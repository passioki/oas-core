# RESUME — 断点续接卡（session-memory 强制 6）

> 规则：本文件每次断点覆盖重写；≤150 行；另一个会话只读本文件 + `RESUME-INDEX.md` 最新行
> 就能继续。细节放 `ENGINE-DESIGN.md` / `SCHEDULER-RUNNER-PLAN.md` / `EDITOR-UX-PLAN.md §16` / `PENDING-LEDGER.md §E-76`。

```yaml
task_id: oas-core-editor-frames
checkpoint_seq: 96
updated_at: 2026-11-04T00:4x+08:00
trigger: 编辑器帧页三项需求（帧搜索 / 帧详情素材库 / 素材测试帧错位修复）+ 只读评审 P1/P2 修复；UI 828/224/0（无 Rust 改动，workspace 仍 501/0）
status: in_progress
```

## 0. 目标（本轮）

把「图片帧页搜索 + 当前帧素材列表 + 素材测试框选不更新修复」落到完成态：诊断根因 → 修复 → 只读评审 → P1/P2 修复 → 文档同步（**推翻**旧「实时关=位置帧 / 单素材多位置逐个测」口径）。

## 1. 当前状态（2026-11-04 00:4x）

| 项 | 值 |
|---|---|
| 分支/HEAD | `dev`；HEAD = 本轮 docs 提交（父 = 本轮代码提交；**代码提交的父 `5d8eae3`**）；hash 以 `git log` 为准；未 tag/release |
| 任务状态 | **in_progress**（seq 96；代码 + docs 待 Lead 提交后 done） |
| 本轮代码 | 仅 UI 3 文件 `apps/oasd/ui/{index.html,app.js,ui_pure_test.js}`（未提交；Lead 先提交代码，再提交 docs） |
| 测试 | `cargo test --workspace` = **501 passed / 0 failed**（21 binary；oasd **228**；**无 Rust 改动**，本轮不新增 Rust 测试）；UI pure test **828 checks / 224 referenced ids / 0 dead ids**；`node --check app.js/ui_pure_test.js` exit 0；task lint **1/0**；`git diff --check` clean |
| 上一节点 | seq 95：调度器产品化 P5 实机验收收口（done；501/0，UI 662/219/0） |
| 工作区 | 用户数据改动**不属于本轮提交**：`assets/assets.json`、`frames/frames.json`、`assets/images/a002.png`/`a003.png`/`a004.png`、`frames/frame-41.png`/`frame-42.png`、`scheduler.json`、`scheduler.json.bak` |
| 文档 | 本轮改 8 份：`EDITOR-UX-PLAN`（§4/§11/§15.1/§16）· `PENDING-LEDGER`（§E-76 + U-32b/§E-66 注）· `AUDIT-SUBPROCESS`（P1-7 注）· `RESUME` · `RESUME-INDEX` · `START-HERE` · `README` · `SESSION-HANDOFF` |

## 2. 本轮三项需求（已实现，未提交）

1. **帧页搜索**：`#frame-search`（名称 / `#id` / 分类；大小写不敏感 + trim；`12`/`#12` 精确 ID）+ `#frame-search-clear` + `#frame-search-status`；**纯前端过滤**（不改 `/frames` 请求）；先搜索后按分类分组，组空不渲染，全空「没有匹配的帧」；切分类/刷新保留搜索词；纯函数 `frameMatchesQuery`/`filterFramesByQuery`/`frameSearchStatusText`。
2. **帧详情素材库**：点击具体帧（或当前帧刷新）后 `#frame-detail` 增加「该帧素材：N 条」+ `#frame-assets-list`；显示**全局素材库 + 任务素材**中 `positions` 含该帧 id 的全部素材，按 id 去重、来源合并「全局/任务」；行与全局/任务素材库同一套 `asset-row`：抽出共享 `assetRowElement`/`renderAssetRows`，三处真实复用（`renderGlobalAssets`/`renderAssetList`/新 `renderFrameAssets`），行 DOM 仍由 `OASAssetUI.createRow` 生成；帧详情行只保留测试类动作；空态「该帧没有素材记录」。
3. **素材测试帧错位修复**：见 §3。

## 3. 根因与修复（需求③，已修）

**根因**：实时关闭时 `testFrames(it)` **优先返回素材 `positions`（位置帧）**，忽略用户选中/画布帧；`positions` 保存必填 ≥1 ⇒ 永远测**静态截图**，模板在截图里仍在保存位置，识别框必然 = 存储 ROI；`live=1` 每次抓当前屏幕所以准确。**后端 `/task/asset/match` 正确**（按 `frame_id` 取帧重新匹配，`x/y` = 本次匹配点），不是缓存位置。实测 `悬赏封印_关闭弹窗`：`frame_id=23` → score 1.0 @ (1208,68)；`frame_id=26` → score 0.9933 @ (1082,75)。

**修复**：`testFramePlan`（优先级 **live > 显式 frameId > 当前帧 > positions 兜底**）；`testFrames(it, frameId)` 薄封装；`runAssetTest` + 五个测试函数（match/click/drag/ocr/collection）透传 frameId；帧详情行测试传 `f.id`；`selectFrame` 成功切帧后、`loadFrame()` 前清 `boxes/selectedBox`；`matchBoxValid`（x/y≥0 且 w/h>0）⇒ miss（x=y=-1）不画框；单行 OCR `recognizedBox:null`+`roiBox`，文案「识别区域（存储 ROI，无独立检测框）」，**不再把存储 ROI 冒充真实识别框**（多行/derived det 框不变）。

**P1/P2**：全局素材库保存/删除/改位置/改名/阈值/ROI/操作区/集合/排序后，`refreshGlobalAssets()` 成功即调 `renderFrameAssetViews()`（最新缓存本地重绘帧列表 + 当前帧详情/素材列表；0 额外请求、0 递归，不复用 `refreshFrames()` 以免重复 GET `/frames`）；`fallbackFullscreenLines` 透传 frameId；删除当前帧清旧框。

## 4. 新口径（现行，必须与旧文档区分）

| 场景 | 用的帧 |
|---|---|
| 单素材测试（匹配/点测/拖测/OCR/集合） | **当前选中帧**（帧详情行 = 该行帧） |
| 批量帧测试 | **所有位置帧**（`POST /task/assets/batch_test`，覆盖全部可见素材） |
| 实时 = 开 | 实时帧，**恒优先**；真实点击强制 live |

- **没有「单素材多帧」入口**；旧「实时关=位置帧 / 单素材多位置逐个测」已被 `EDITOR-UX-PLAN §16` + `PENDING-LEDGER §E-76` 取代；`AUDIT-SUBPROCESS` P1-7 已加注。
- `/ocr` 协议无 bbox ⇒ **单行 OCR 没有真实检测框**（多行/derived 才有 det 框）。
- `frame_source` 线上仍回 `position:#<id>`（后端把显式 frame_id 当位置帧），UI 标签显示「当前帧 #N」；两者会同时出现在结果行（P2 文案待统一）。

## 5. 证据

- **本轮**：`D:\OAS-dev\_verify\ui-frames-20261103\` —— `MATCH-BUG-DIAGNOSIS.md`（根因）· `FRAMES-UI-REPORT.md`（需求①②）· `MATCH-FIX-REPORT.md`（需求③）· `REVIEW-UI.md`（只读评审：P0=0，P1×2，P2 若干）· `FIX-P1-REPORT.md`（P1-1 + P2-2/P2-4；802→828）· `lead-final-ui.log`（828/224/0）· `lead-final-workspace.log`（501/0）· `api-frames.json`/`api-assets.json`/`match-23.json`/`match-26.json`/`editor-page.png`。
- **文档收尾**：`D:\OAS-dev\_verify\ui-frames-20261103\DOCS-REPORT.md`（逐处 before→after + 残留检查）。
- **台账**：`docs/PENDING-LEDGER.md §E-76`（需求→实现→根因→修复→测试→E2E→遗留）；计划/现行口径：`docs/EDITOR-UX-PLAN.md §16`。
- **历史**：调度器 P5 `_verify/p5-realdevice-20261103/`；O-1/O-2 `_verify/release-v0.1.0/`、`_verify/final-review-20261102/`。

## 6. 下一步（= 产品 backlog，按需排序）

1. **`SelectionTool` 抽取**（结构债，唯一剩下的 P1 级重构）。
2. **collection `real=1` 真机 E2E**（O-1 遗留；MuMu 已可用）。
3. **drag Plan B**（asset 引用/运行时跟随识别，替代生成时 shape 快照）。
4. **P2-10**：保存 flow 时校验解析错误（此前明确不做）。
5. **运行记录/口径收口**：点击聚合关联、分页/保留、run_id 跨进程唯一、detail 契约、`--status` 的 `run_file_errors`；事件 attempt/entry、stats vs report、UI 失败口径。
6. **本轮遗留（P2/P3）**：帧搜索 Enter/防抖（P3）、分类+搜索状态计数跨分类（P2）、纯数字查询既精确又子串（P2）、全局模式来源重复计数（P2）、`frame_source` 文案、切帧不清 ROI、单行 OCR 画布无框（P2 沟通项）、`renderAssetRows` DRY 复制；**无真实浏览器自动化 E2E**（P3）。
7. **版本决策**：是否升 0.2.0（用户定）；墙钟 24h soak 仍未跑完（只有 96 轮加速 soak 成立）。

## 7. 环境（E11~E14，详见 START-HERE §3）

- PowerShell **5.1**（执行策略 Undefined）；`node` 不在 PATH，用 bundled node（v24.21.0）；PS 重定向日志 UTF-16LE；中文文件用 `read`/`edit`/`write`。
- 实机：本轮 serve **8787**、真机 `emulator-5554`（与 P5 的 MuMu `127.0.0.1:16384` 不同，以 `adb devices` 实测为准）；`GET /instance/devices` 会自动 `adb connect`，响应带 `auto_connect`。

## 8. 节点（seq 96）

本节点 seq 96：**编辑器帧页三项需求 + 素材测试帧错位修复**——帧搜索/帧详情素材库/单测=当前帧（批量=位置帧）；UI **662→744→802→828**（828/224/0），workspace 仍 **501/0**（无 Rust 改动）；代码 + docs 由 Lead 分两次提交。上一节点 seq 95：调度器产品化 P5 done（实机 A–F + adb 自动连接；501/0，UI 662）。
