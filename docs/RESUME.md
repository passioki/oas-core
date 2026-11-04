# RESUME — 断点续接卡（session-memory 强制 6）

> 规则：本文件每次断点覆盖重写；≤150 行；另一个会话只读本文件 + `RESUME-INDEX.md` 最新行
> 就能继续。细节放 `PUSH-ALL.md` / `EDITOR-UX-PLAN.md §16~§17` / `PENDING-LEDGER.md §E-77`。

```yaml
task_id: oas-core-asset-row-pushall
checkpoint_seq: 98
updated_at: 2026-11-04T12:50+08:00
trigger: ① 一键提交/推送脚本 scripts/push-all.ps1/.bat（评审 P1：任意分支自动同步 main → 仅 dev 或 -SyncMain；P2：已跟踪瞬态不剔除、push 失败重跑补推）② 素材行统一「识别 + 设置」两按钮 + 9 项设置菜单（评审 P1：模板「识别区」死动作）→ UI 907/225/0 · workspace 501/0 · 脚本 AST 5714/0 · 隔离集成 59/0
status: in_progress
```

## 0. 目标（本轮）

两个需求一起收口，**均未提交**：① 一键提交/推送脚本（`scripts/push-all.ps1` + `.bat` + `docs/PUSH-ALL.md`）；
② 全局/任务/帧详情三处素材行统一为「识别 + 设置」两按钮（设置 = 行内 9 项菜单）。附评审 P1/P2 修复。

## 1. 当前状态（2026-11-04 12:5x）

| 项 | 值 |
|---|---|
| 分支/HEAD | `dev`；HEAD = **`a4b6960`**（seq 97 docs 提交）；本轮改动未提交，提交后**代码/脚本父 = `a4b6960`**，再提交 docs；hash 以 `git log` 为准；未 tag/release |
| 任务状态 | **in_progress**（seq 98；代码/脚本/docs 待 Lead 提交后 done） |
| 本轮代码 | UI **4 文件** `apps/oasd/ui/{asset_ui.js,app.js,style.css,ui_pure_test.js}` + 新增 `scripts/push-all.ps1`（UTF-8 BOM，1004 行）/ `scripts/push-all.bat` / `docs/PUSH-ALL.md` |
| 测试 | UI pure test **907 checks / 225 referenced ids / 0 dead ids**（835→888→907，+72）；`cargo test --workspace` **501/0**（oasd 228；无 Rust 改动）；脚本 AST **5714 tokens / 0 errors**；隔离 git 副本集成 **59 checks / 0 failures**；task lint **1/0**；`node --check` 3/3；`git diff --check` clean |
| 上一节点 | seq 97 帧页布局回归修复（UI 835/225/0，已提交 `a4b6960`）；seq 96 帧页三项 + 测试帧错位修复 |
| 工作区 | **用户数据不提交**：`assets/assets.json`、`assets/images/a00{2..5}.png`、`frames/frames.json`、`frames/frame-4{1,2}.png`、`scheduler.json`、`scheduler.json.bak`、`tasks/关闭弹窗/` |
| 文档 | 本轮改 8 份：`PUSH-ALL`（核对/补充）· `EDITOR-UX-PLAN`（§16.2 + 新增 §17）· `PENDING-LEDGER`（§E-77 + §E-76 取代说明）· `RESUME` · `RESUME-INDEX` · `START-HERE` · `SESSION-HANDOFF` · `README` |

## 2. 需求②：素材行两按钮（现行）

### 2.1 结构

- 三处素材库（任务 / 全局 / 帧详情）**所有普通素材行 =「识别」+「设置」两个按钮**；集合行（`template_set`）同样两按钮。
- 「识别」= 旧 `onTest`（集合走 `onCollectionTest || onTest`）；「设置」在**行内**展开 9 项（`.asset-row-menu`，`flex:1 1 100%`），不是绝对定位弹层（避免被 `.asset-list { overflow:auto }` 裁掉）。
- 再次点「设置」/ 执行任一可用项 / `Esc` 收起；`actions:false` 同时隐藏两按钮；`compact` 只隐藏点测/拖测/步骤。

### 2.2 九项顺序（严格，测试断言）

**点测 / 拖测 / 识别区 / 操作区 / 重命名 / 阈值 / 位置帧 / 删除 / 步骤**

| # | 菜单 | 回调 | 行为 |
|---|---|---|---|
| 1 | 点测 | `onClickTest` | dry-run 点击测试 |
| 2 | 拖测 | `onDragTest` | dry-run 拖动测试 |
| 3 | 识别区 | `onEditRoi` → 新 `editAssetRoi` | `prompt` 输入 `x,y,w,h` |
| 4 | 操作区 | `onAction` | 画布操作区编辑（沿用） |
| 5 | 重命名 | `onRename` → 新 `renameAsset` | `prompt` 新 id |
| 6 | 阈值 | `onThreshold` → 新 `editAssetThreshold` | 仅 template，0~1 |
| 7 | 位置帧 | `onPosition` | 沿用 `editAssetPositions` |
| 8 | 删除 | `onDelete`（集合走 `onCollectionDelete`） | 沿用删除 |
| 9 | 步骤 | `onStep` | 生成 `click_target` |

### 2.3 禁用规则

- 非 `template`：阈值 disabled；`template`：**识别区 disabled**（服务端拒绝模板 ROI 更新，title 说明须重新截图保存）；`region`：操作区 disabled；
- `manage:false`（帧详情）：识别区/操作区/重命名/阈值/位置帧/删除 6 项 disabled（识别/点测/拖测/步骤可用）；
- `template_set`：其余 8 项 disabled，仅删除可用；
- disabled 点击**不触发回调**；`editAssetRoi` 对 template/template_set 有硬 guard（prompt/POST 之前 return）。

### 2.4 清理/CSS/验证

- 删除旧 `editAsset`（重命名+阈值+ROI 三合一）与两处 `onEdit` 死代码；`parseAssetRoiText` 只收 4 个十进制整数（`x,y>=0`、`w,h>0`、每项 `<=100000`）。
- 窄侧栏：`.asset-main { min-width:0 }` + `.asset-test { align-self:flex-start }`；headless 320px 实测 `row=320 / main=208 / 按钮 52x26 / sameRow=true`。
- CDP 实测：全局行 `topButtons=[识别,设置]`；模板行仅「识别区」disabled；OCR 行「阈值」disabled；帧详情管理项 disabled。截图 `_verify/asset-row-20261104/asset-row-menu2.png`、`asset-row-320.png`。

## 3. 需求①：一键提交/推送脚本

- 参数：`-Message`（默认 `chore(data): sync all resources/tasks/scheduler/code/docs <yyyy-MM-dd HH:mm>`）、`-Yes`、`-DryRun`、`-NoPush`、`-Remote`（默认 origin）、`-Branch`（默认当前分支；不一致 exit 2；detached exit 3）、`-SyncMain`。
- 流程：仓库根 = `scripts/` 上级 → 预检 → `git add -A` → 剔除**未跟踪**瞬态（`*.bak`/`*.tmp`/`*.log`、`scheduler.json.bak`、scheduler state/events/lock、任意层级 `logs/`、`_verify/**/target/`；**已跟踪文件不剔除**）→ 无 staged 但本地领先上游 ⇒「只推送」补推 → 清单 + 确认 → commit（失败 4）→ push（失败 5，保留本地 commit）→ 仅 `dev` 自动 `fetch/checkout main/ff-only/push/切回`；其它分支 WARN + exit 0（除非 `-SyncMain`）；远程无 main → WARN + exit 0。
- 安全：不 force / 不 clean / 不 reset --hard；`-DryRun` 只读（不写索引、不 commit/push、不访问网络）。
- 提交范围：`git add -A` 覆盖 code/docs/`assets/`/`frames/`/`tasks/`/`scheduler.json` 等；**`library/` 被 `.gitignore` 忽略，不会提交**。

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\push-all.ps1 -DryRun   # 只预览
scripts\push-all.bat                                                              # 显示清单 + y/N
scripts\push-all.bat -Yes                                                         # 免确认
scripts\push-all.bat -Yes -NoPush                                                 # 只提交
```

## 4. 评审修复（P1/P2）

- **P1-1 模板「识别区」死动作**：菜单 disabled + title + `editAssetRoi` 硬 guard（template/template_set 不发请求不 prompt）；region 经服务端确认允许，保持可用。
- **P1-2 任意分支自动同步 main**：默认仅 `dev`；其它分支 WARN + exit 0；新增 `-SyncMain`；远程无 main 先 `ls-remote` 预检 → WARN + exit 0；显式 `-SyncMain` 无法确认才 exit 5。
- **P2**：已跟踪瞬态不再 unstage（用 `ls-tree HEAD` 判定）；push 失败重跑走「只推送」补推；320px 两按钮同排；ROI 100000 sanity cap；删 `editAsset`/`onEdit`；帧详情补 `onDelete`（仍 disabled）。

## 5. 遗留（P2/P3，不阻塞）

- ROI 编辑用 `prompt`（未做画布框选保存入口）；ROI 边界只做 100000 sanity cap，未按 `design_resolution` 精确校验 `x+w`/`y+h`（P2）。
- 脚本控制台中文编码依赖本机（PS 5.1，未强制 `OutputEncoding`）；`library/` 未纳入版本控制；**未在真实仓库实测 push 失败后的分支恢复**（隔离测试覆盖逻辑，真实仓只用 `-DryRun`）。
- 无真机/真实浏览器自动化回归（本轮 CDP headless）；设置菜单全局互斥/Esc 焦点/aria（P3）。

## 6. 下一步（= 产品 backlog）

1. **`SelectionTool` 抽取**（结构债，唯一剩下的 P1 级重构）。
2. **collection `real=1` 真机 E2E**（MuMu `127.0.0.1:16384` 已可用）。
3. **drag Plan B**（asset 引用/运行时跟随识别，替代生成时 shape 快照）。
4. **P2-10**：保存 flow 时校验解析错误（此前明确不做）。
5. **运行记录/口径收口**：点击聚合关联、分页/保留、run_id 跨进程唯一、detail 契约、`--status` 的 `run_file_errors`；事件 attempt/entry、stats vs report、UI 失败口径。
6. **本轮 P2/P3**：ROI 画布保存入口、ROI design 精确边界、脚本 `OutputEncoding`、`library/` 版本化决策、真实浏览器自动化 E2E。
7. **版本决策**：是否升 0.2.0（用户定）；墙钟 24h soak 仍未跑完（只有 96 轮加速 soak 成立）。

## 7. 证据

- **本轮**：`D:\OAS-dev\_verify\asset-row-20261104\`（`ASSET-ROW-REPORT.md` · `REVIEW.md` · `FIX-REPORT.md` · `asset-row-320.png` · `asset-row-menu2.png` · `probe-asset-menu.js` · `probe-asset-row.js` · `temp-pushall-tests.ps1` · `logs/`）+ `DOCS-REPORT.md`（文档逐处 before→after + 残留检查）。
- **上一节点 seq 97**：`D:\OAS-dev\_verify\ui-frames-20261103\frames-fixed.png` / `frames-fixed-detail.png` / `DOCS-LAYOUT-REPORT.md`。
- **台账**：`docs/PENDING-LEDGER.md §E-77`（+ §E-76）；**现行口径**：`docs/EDITOR-UX-PLAN.md §17`（素材行）、`docs/PUSH-ALL.md`（脚本）。
- **历史**：调度器 P5 `_verify/p5-realdevice-20261103/`；O-1/O-2 `_verify/release-v0.1.0/`。

## 8. 节点（seq 98）

本节点 seq 98：**素材行两按钮（识别/设置，9 项菜单）+ 一键提交/推送脚本**——UI 4 文件 + `scripts/push-all.ps1`/`.bat` + `PUSH-ALL.md`；评审修 P1-1 模板识别区、P1-2 仅 dev 同步 main（+`-SyncMain`）及 P2；UI **835→907 checks / 225 ids / 0 dead**（+72），workspace 仍 **501/0**，脚本 AST **5714/0**、隔离集成 **59/0**。上一节点 seq 97：帧页布局回归（已提交 `a4b6960`，UI 835/225/0）。
