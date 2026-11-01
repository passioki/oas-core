# C — 前端 UI 逻辑审计（D:\OAS-dev\oas-core）

- 审计对象：HEAD `983c740`；重点 `apps/oasd/ui/app.js`、`asset_ui.js`、`builder.js`、`index.html`、`style.css`
- 审计时间：2026-11-01 21:35 起；范围为 16:00 之后进入的 UI 改动，对照 session 18:40~20:38 的明确口径
- 方式：只读静态审计 + `node --check` + 与 `063c329`/文档/session 交叉核对；**未启动服务器、未修改代码或数据、未 commit**
- 说明：本报告的行号均指 HEAD `983c740`。工作区当前仍有 `frames/frames.json` 修改、`tasks/返回庭院/{assets,meta}.json` 删除，审计未触碰这些数据。
- 语法检查：`app.js`、`asset_ui.js`、`builder.js`、`dashboard.js`、`flow_model.js` 均通过 `node --check`；因此下列问题都是**逻辑/状态/契约问题，不是语法错误**。

## 0. 结论摘要

当前 UI 主方向（画布/侧栏、action_shape、素材行复用、排序、分类重命名）已经落地，但存在 **1 个 P0 级可见性问题** 和 **7 个 P1 级缺陷/需求未真正闭环**。最严重的不是“样式”，而是：

1. **新建素材保存后从任务库和全局库同时消失**：任务库按 flow 引用过滤，新素材尚未被引用；全局库又只在启动时加载，没有可用刷新入口。用户会再次看到“素材库为空/素材丢了”。
2. **操作区多边形/反选无法可靠保存**：`063c329` 声称修好的 P1-2 只修了矩形路径；`saveSelectedCandidates` 因 `#btn-batch-save` 不存在而根本不可达，实际保存入口 `saveAssetUnified` 仍会退回矩形。
3. **主“试识别/多行识别”按钮没有把 live/frame_id 传给后端**，实时开着时仍测旧 `State.frame`；`拖测` 完全不请求帧；单素材多位置只测第一个，和 session 18:56“3 都测”不符。
4. **`boxCenter` 被重复定义**，后一个数组版本覆盖前一个对象版本，导致“生成点击指令”得到 `{x:null,y:null}`（已用 Node 复现）。
5. **选择状态机仍是双份**（P1-5 未修），并且 `清除`/模式切换没有清理 action polygon/invert，造成旧操作区串到新素材。
6. **识别多边形掩膜没有写进实际保存 payload**：`saveAssetUnified` 不含 `polygon`，只有不可达的 `doSaveAsset` 有；画了识别多边形等于没保存。

## 1. `063c329` 已修项独立验证

| 审计项 | 独立结论 | 证据 |
|---|---|---|
| P0-1 `fixed` 独立区误判无效 | ✅ 已修 | `app.js:1428-1432`；`fixed` 不重叠返回 `valid:true` + `independent:true` |
| P0-2 polygon 有效性用整屏 | ✅ 已修 | `app.js:1354-1373` 新增 `polygonOverlapsRect`；`:1426` polygon 分支使用它 |
| P1-2 批量保存丢 polygon/invert | ⚠ **只修了矩形路径，实际未闭环** | `saveSelectedCandidates`（`:1277`）绑定在 `#btn-batch-save` 上，但该 id 在 `index.html` 中不存在（`:228`/`:5713`），函数不可达；实际入口 `saveAssetUnified` 虽读 `c.actionShape`（`:2970`），但多边形闭合路径从不写回它。详见 C-02 |
| P1-3 collection 使用 live/position | ✅ 已修 | `testAssetCollection` 带 `testFrameQuery(it)` 并显示 `s.frame_source`（`app.js:2453-2474`）；服务端 `serve.rs:3218` 用 `frame_for_test(&q)` |
| P1-4 OCR 帧标签 | ⚠ 部分 | `testFrameLabel`（`:1516-1521`）已用于 `testAssetOcr`（`:1691`）；但主 OCR 按钮不请求 live（C-04），`testAssetClick` 又用了一个作用域失效的 `frameSource` 回退（C-17） |
| P1-6 shape 自交校验 | ✅ 后端已修 | `crates/oas-task/src/lib.rs:169-205`，`polygon_self_intersects` + 越界/字段互斥校验；UI 收到错误会显示 |
| P1-1 drag 步骤仍只有 bbox | ❌ 未修 | `app.js:1626-1635` 的 `from` 仍为 `{kind:'region', roi: action}`；见 C-21 |
| P1-5 SelectionTool 未抽 | ❌ 未修 | 两套状态 `polygon/polyMode` 与 `actionPoly/actionPolyMode`，四组事件分支；见 C-06 |

补充：`063c329` 的 `testFrameLabel`/`actionValidity` 本身逻辑基本正确；问题集中在“修在死代码里”“只修了一个入口”“其他入口没同步”。

## 2. 缺陷清单

### C-01 [P0] 新建素材保存后从两个素材库同时“消失”

- 文件：
  - `apps/oasd/ui/app.js:2283-2295`（任务库在全局模式下按 flow 引用过滤）
  - `apps/oasd/ui/app.js:3017`（保存成功后只刷新任务库）
  - `apps/oasd/ui/app.js:3353-3392`（全局库只读加载，不参与保存后刷新）
  - `apps/oasd/ui/app.js:131`、`:5631`、`:5634`（唯一的全局刷新绑定是 `#btn-library-refresh`，该 id 在 `index.html` 中不存在；全局库只在启动时加载一次）
  - `apps/oasd/ui/index.html:102-104`（全局库没有刷新按钮，也没有管理按钮）
- 触发步骤：
  1. 当前仓库 `tasks/` 为空（或选中的任务 flow 尚未引用新素材）。
  2. 在素材页识别/画 ROI，勾选结果，点“保存素材”。
  3. `saveAssetUnified` 成功后调用 `refreshTaskAssets(task)`；在全局模式下它请求 `/task/assets` 得到全部全局素材，再用 `collectFlowAssetIds` 过滤，新素材不在 flow 中 → 任务库为空。
  4. 全局库只在 boot 时调用过 `loadGlobalAssets`；`#btn-library-refresh` 不存在，保存后不会刷新 → 仍显示保存前的数量和列表。
  5. 结果：新素材只有**刷新整个页面后**才出现在“素材库（全局）”；任务库则要等 flow 引用它才出现。用户看到的就是“保存成功但素材没了”。
- session 依据：
  - 20:10“素材页面里的素材库似乎应该是任务页面里的……素材页面的素材库少了素材库。”
  - 20:21“任务素材是全局素材的子集。”
  - 20:33 助手承诺“任务页只取该任务 flow 引用的子集；素材页始终显示全部全局素材。”
  - 但 20:33 的“始终显示”没有配套刷新，实际只在页面加载时显示。
- 建议：
  - 保存/删除/改位置/改 ROI 后同时刷新全局库；给全局库加显式“刷新”按钮，或复用 `/task/assets?task=` 的返回直接更新两个列表。
  - 任务库过滤后，对“存在于全局但未被当前 flow 引用”的素材给出提示/计数，而不是静默全部隐藏。
  - 后端 `/task/assets` 返回 `scope:"global"` 一类显式字段，不要让前端靠 `task === "（全局素材库）"` 这个本地化字符串判断（`app.js:2286`）。

### C-02 [P1] 操作区多边形/反选在真实保存路径中被丢弃（P1-2 实际未闭环）

- 文件：
  - `apps/oasd/ui/app.js:2966-2972`：`saveAssetUnified` 的 OCR 行写 `action_shape: c.actionShape || 矩形`、`action_source:'fixed'`。
  - `apps/oasd/ui/app.js:1019`：`makeResultRow` 初始 `actionShape: null`。
  - `apps/oasd/ui/app.js:4614-4617`：只有“操作区矩形拖拽 mouseup”才把 `currentActionShape()` 写回候选。
  - `apps/oasd/ui/app.js:4661-4670`：操作区多边形双击闭合只置 `actionPolyClosed`、写日志，**没有**写回 `saveCandidates[activeCandidate].actionShape`，也没有调用 `updateAssetActionRoi(pendingActionAsset)`。
  - `apps/oasd/ui/app.js:1375-1391`：`currentActionShape()` 优先返回已闭合的 `actionPoly`，即使刚刚拖了新矩形。
  - `apps/oasd/ui/app.js:2728-2750`：已有素材的操作区保存只走 `backMode` mouseup；多边形模式会先把 `backMode` 关掉（`:4725`），因此多边形闭合后没有保存请求。
- 触发步骤（以下三条路径都会丢失 shape）：
  1. 多行识别 → “调操作区域” → 点“操作区域多边形” → 画多边形 → 双击闭合 → 保存素材：`c.actionShape` 仍为 null，保存退回 `c.action` 矩形，多边形丢失。
  2. 已有素材点“操作区” → 点“操作区域多边形” → 画完闭合：`pendingActionAsset` 仍在，但没有任何 `updateAssetActionRoi` 调用，服务端不会收到新 shape。
  3. 已有素材是 polygon shape（`editAssetActionRoi` 会 `loadActionShapeForEdit` 回显）→ 直接拖一个新矩形 → `currentActionShape()` 仍优先返回旧 polygon，保存的是旧形状，无法把 polygon 改成 rect。
  - 反选同理：`actionInvert` 只影响 `currentActionShape()`，而候选保存不读它；除非先经过一次矩形 mouseup 把 shape 写回，否则反选不会进入 payload。
- session 依据：
  - 18:40 第 2 条：“区域选择后，可以反选……需要加入多边形。如有必要，框选可以抽为一个函数。”
  - 19:04 第 1 条：“反选的意思是整个屏幕-选择区域，方案 B。”
  - `docs/CODE-AUDIT-2026-11-01.md` 第 6 节把 P1-2 标为“✅ 已修（candidate actionShape + 批量 payload）”，但候选写入只在矩形 mouseup 发生；`saveSelectedCandidates` 整段又不可达，实际入口仍会退回矩形。
- 建议：
  - 在保存入口统一读取“当前候选/当前素材的有效 shape”，而不是读可能为 null 的 `c.actionShape`；候选对象上增加显式 `actionShapeDirty`。
  - `actionPoly` 双击闭合时：若 `activeCandidate >= 0` 写回候选；若 `pendingActionAsset` 存在立即调用 `updateAssetActionRoi`；否则给出“未绑定素材”的提示。
  - `currentActionShape()` 增加“最近一次绘制来源”优先级（rect/polygon 二选一），或在进入 rect 拖拽时清掉 `actionPoly`。
  - 给这条路径补前端测试；只修死代码不算修复。

### C-03 [P1] `boxCenter` 重复定义，生成点击指令输出 null 坐标

- 文件：`apps/oasd/ui/app.js:564-567`（对象版本，读 `b.box`）与 `apps/oasd/ui/app.js:1492-1494`（数组版本，读 `box[0]`）；调用点 `:570`、`:582-583`。
- 原因：同一作用域内两个 `function boxCenter` 声明，后声明的数组版本覆盖前一个；`selectedBox` 是 `{box:[x,y,w,h],...}`，数组版本得到 `undefined + NaN` → `NaN`；`JSON.stringify` 把 `NaN` 写成 `null`。
- 触发步骤：多行识别 → 点击任意识别区块 → 点“生成点击指令”或“复制步骤到剪贴板”。
- 实际输出（Node 复现，等价函数）：
  ```json
  {"op":"click_point","args":{"x":null,"y":null}}
  ```
- session 依据：这是既有工作台功能回归；session 18:40 第 4 条要求修点测/拖测随机，但没有要求破坏“生成点击指令”，且 `063c329` 未触及该函数。
- 建议：把数组版本改名为 `boxCenterOfArray`/`rectCenter`，或删除未使用的第二个定义；补一个最小函数测试。

### C-04 [P1] 主“试识别/多行识别”没有走实时帧/位置帧

- 文件：
  - `apps/oasd/ui/app.js:4813-4825`：`btnOcr` 只拼 `x/y/w/h`，没有 `live=1`、`frame_id`。
  - `apps/oasd/ui/app.js:4983-4994`：`btnLines` 同样只拼 `x/y/w/h`。
  - `apps/oasd/ui/app.js:4779-4790`：`fallbackFullscreenLines` 的 `/lines?x=0...` 也没有帧参数。
  - 对比：`serve.rs:1672-1685` 已支持 `live`/`frame_id`，`/ocr` 与 `/lines` 都调用 `frame_for_test(&q)`（`serve.rs:1761`、`:2357`），且响应包含 `frame_source`（`:1791`、`:2409`）。
- 触发步骤：
  1. 保持“实时：开”（默认就是开）。
  2. 在画布上框一个 ROI，点“试识别”或“多行识别”。
  3. 页面显示“识别中…”，但后端用的是 `State.frame`（可能是实时流未覆盖的旧分析帧/当前选中帧），不是用户眼前的实时画面；结果也没有“实时帧”标签。
  4. 识别失败触发全屏回退时，回退同样测旧帧。
- session 依据：
  - 18:40 第 3 条：“实时开着时，测试/识别/点测/拖测等所有操作都是测实时帧，实时没开着测得是位置帧。”
  - 19:31 助手只声称“单素材测试请求”带 live；主 OCR 按钮不在其验证范围，实际没带。
- 建议：`btnOcr`/`btnLines`/`fallbackFullscreenLines` 统一复用 `testFrameQuery()`；结果显示服务端 `frame_source`（不要只显示“识别中”）；`/ocr`/`/lines` 已有能力，不需要改后端。
- 同一路径也没有把画布上的识别 polygon 传给 `/ocr`/`/lines`；服务端只在请求带已保存素材的 `task`+`id` 时才从 assets 读取 polygon 掩膜（`serve.rs:1766` `asset_ocr_polygon`）。因此“新画多边形 → 试识别”阶段掩膜不生效，结果会包含多边形外的文字；保存后的素材测试才生效，前后不一致。

### C-05 [P1] 单素材多位置只测第一个位置帧

- 文件：
  - `apps/oasd/ui/app.js:1510-1514`：`testFrameQuery` 只取 `it.positions[0]`。
  - `apps/oasd/ui/app.js:1516-1521`：`testFrameLabel` 也只显示第一个。
  - `apps/oasd/ui/app.js:1536`（template match）、`:1557`（OCR click test）、`:1690`（OCR test）、`:2776`（asset match）都只发一个 `frame_id`。
- 触发步骤：给一个素材设置 3 个位置帧；暂停实时；在素材库点“识别/点测/测试/匹配”。结果只覆盖 `positions[0]`，其余两帧完全没有结果或标签。
- session 依据：
  - 18:42 助手问“实时关闭时单素材多位置：全部位置帧都测，还是只测第一个？”；18:56 用户答“**3 都测**”。
  - `docs/EDITOR-UX-PLAN.md §4` 也写明“已确认：单素材多位置时逐个位置帧测试并分别返回结果”。
  - `CODE-AUDIT-2026-11-01.md §4` 却写“单素材与 batch 符合”；本审计认为该结论不成立（单素材只测第一个，batch 才测全部）。
- 建议：单素材测试循环 `it.positions`，逐帧请求并汇总“帧 #id 通过/失败”；或直接复用 batch_test 的单素材子集。至少要显示“本次仅测 #id，其余 N 帧未测”。
- 相关不一致：任务库的“批量帧测试（dry）”只发 `{task, live}`（`app.js:2504-2508`），没有把当前任务可见的 `taskAssets` 作为 `assets` 过滤器；后端在全局模式下遍历 `af.items` 全部素材（`serve.rs:2946`）。因此任务页按钮实际测的是 34 个全局素材，而不是任务 flow 引用的子集，结果里会出现不属于当前任务的素材。

### C-06 [P1] 选择状态机仍是双份，`清除`/模式切换留下旧操作区（P1-5 未修）

- 文件：
  - `apps/oasd/ui/app.js:4488-4533`：`mousedown` 的优先级是 actionPoly → polygon → backMode；三套状态互不复位。
  - `apps/oasd/ui/app.js:4702-4718`：`btnPoly` 进入识别多边形模式时只清 `polygon/roi/roiBack`，**没有**清 `actionPolyMode`。
  - `apps/oasd/ui/app.js:4720-4735`：`btnActionPoly` 会 `backMode=false`，但不清 `activeCandidate`/`pendingActionAsset`。
  - `apps/oasd/ui/app.js:4745-4760`：`btnBackMode` 只切自己，不清 `actionPolyMode`/`polyMode`。
  - `apps/oasd/ui/app.js:4762-4777`：`btnClear` 清了 `roi/roiBack/polygon/selectedBox`，但**没有清** `actionPoly/actionPolyClosed/actionInvert/actionPolyMode/backMode/polyMode`。
  - `apps/oasd/ui/app.js:1375-1391`：`currentActionShape` 的 polygon 优先级让旧 polygon 覆盖新矩形。
- 触发步骤：
  1. 画一个操作区多边形并双击闭合 → 点“清除识别/操作区域”（`actionPoly` 未被清）→ 重新做一次 OCR → 点“调操作区域”后拖一个新矩形 → mouseup 时 `currentActionShape()` 仍返回旧 polygon，新素材保存的是旧多边形/旧反选。
  2. 先点“操作区域多边形”，再点“多边形框选”：`actionPolyMode` 仍为 true，`mousedown` 仍走 action polygon 分支，识别多边形完全画不出来。
  3. 先点“操作区域多边形”，再点“绘制操作区域”：`actionPolyMode` 仍优先，拖拽不会进入 `backMode`。
- session 依据：
  - 18:40 第 2 条：“框选可以抽为一个函数。”
  - `CODE-AUDIT-2026-11-01.md` P1-5：“两套状态……没有统一 SelectionTool”，本轮确认仍未修；而且不只是结构问题，已经产生具体串状态 bug。
- 建议：立即抽 `SelectionTool`（mode/target/points/closed/invert）并统一 reset；迁移顺序：先做一个 `resetSelection()` 在 `btnClear`、各模式切换、任务/素材切换时调用，再逐步合并事件处理。
- 相关：C-02 的 polygon 保存问题与这里同源，修 SelectionTool 时应一并覆盖。

### C-07 [P1] OCR/模板新素材被迫写成 `fixed`，且 recognition 采样仍用 ROI 而非真实 bbox

- 文件：
  - `apps/oasd/ui/app.js:1272-1281`：`saveSelectedCandidates` 固定 `action_source:'fixed'`（但该函数不可达，见 C-19）。
  - `apps/oasd/ui/app.js:2966-2972`：实际保存入口同样固定 `fixed`；只要 `c.action` 存在（它总是默认等于 `c.recognize`），就一定写 `action_shape`。
  - `apps/oasd/ui/app.js:2735-2740`：编辑已有素材操作区固定写 `fixed`。
  - `apps/oasd/ui/app.js:1460-1462`：`randomPointInAction` 在没有 shape 时从 `it.roi` 采样；`assetActionBox`（`:1483-1489`）同样回退 `it.roi`。
  - UI 里没有任何地方写入 `action_source:'recognition'`（grep 结果只有读取和 `fixed` 写入）。
- 触发步骤：
  1. 多行识别 → 勾选一行 → 不改操作区 → 保存素材。
  2. 打开 `assets/assets.json`：新素材为 `action_source:"fixed"` + `action_shape` = 当时的行 bbox，识别 ROI 可能是整段多行区域。
  3. 把界面上的目标下移 50px 再点测：OCR 能在新位置识别到该行，但操作区仍是旧 bbox；`fixed` 不要求重叠，所以只显示“独立操作区域”，不会警示；实际点击仍会点旧位置。
  4. 对于旧/迁移的 `recognition` 素材（无 shape），点测的随机点从 `it.roi` 取，可能落在识别 ROI 内但不在真实识别行 bbox 上；结果却显示“有效（操作区=真实识别框）”。
- session 依据：
  - 18:40 第 5 条：“识别出来的区域需与操作区域有重叠……为了防止识别区域过大、识别目标变了位置，从而导致原先的操作区域失效。”
  - 19:04：“识别到的区域是 OCR 真实识别的框选，不是 action_roi。”
  - 19:12/19:14：`action_source` 的 `recognition` 语义 = 跟随真实识别 bbox；`fixed` = 用户明确固定。
- 建议：
  - 新保存默认 `action_source='recognition'`，只有用户实际拖过/画过操作区时才写 `fixed` + shape；不要用 `c.action` 的默认值冒充“用户显式区域”。
  - 增加“恢复为跟随识别（recognition）”按钮；`editAsset` 改 ROI 时若保留 fixed shape，要提示旧 shape 可能失效。
  - recognition 素材的 dry-run 点应在“本次识别 bbox”内采样（或复用 `testAssetClick` 已算出的 `recognizedBox`），不要用配置 ROI。
  - 给 `action_source` 的三种路径（新保存、编辑、迁移）补测试。

### C-07b [P1] 识别多边形（recognition polygon）在保存链路中丢失

- 文件：
  - `apps/oasd/ui/app.js:2966-2978`：`saveAssetUnified` 的 OCR payload 只有 id/kind/roi/action_shape/action_source/positions/ocr_mode/ocr_text，**没有 `polygon`**。
  - `apps/oasd/ui/app.js:2988-2999`：其他行（template/region）payload 也没有 `polygon`。
  - 唯一写 `payload.polygon` 的是 `doSaveAsset`（`:3049-3053`），但它绑定的 `#btn-asset` 在 `index.html` 中不存在，函数不可达。
  - `apps/oasd/ui/app.js:4963-4971` 的 `btnAsset` JSON 生成也含 polygon，但同样不可达。
  - 后端仍支持 `AssetDecl.polygon`（`crates/oas-task/src/lib.rs:258`），运行时用于 `click_polygon`/OCR 掩膜（`:1143`）。
- 触发步骤：
  1. 画一个识别多边形（“多边形框选”），点“试识别/多行识别”。
  2. 勾选结果，点“保存素材”（实际入口 `saveAssetUnified`）。
  3. 打开 `assets/assets.json`：没有 `polygon` 字段；后续素材测试、runtime OCR/点击都不会使用这个掩膜。用户画的多边形只在画布上“看起来存在”，保存后消失。
- session 依据：
  - 18:40 第 2 条要求多边形；“识别侧继续沿用现有 polygon 掩膜语义”是 19:05/`EDITOR-UX-PLAN §3` 的明确口径。
  - `063c329` 未涉及 `saveAssetUnified` 的 polygon；旧 `doSaveAsset` 有，但已因 DOM 删除而不可达。
- 建议：`saveAssetUnified` 两端 payload 都加 `polygon`（仅 `polygon && polyClosed && length>=3` 时）；补一条“画 polygon → 保存 → 读回”的回归测试；同时处理 C-04 的“测试阶段掩膜不生效”。

### C-08 [P2] 对已有素材点“识别”会覆盖尚未保存的候选结果

- 文件：
  - `apps/oasd/ui/app.js:1678-1827`：`testAssetOcr` 在成功（`:1794`）和失败回退（`:1718`、`:1731`、`:1770`）时调用 `renderOcrLines`。
  - `apps/oasd/ui/app.js:1041-1064`：`renderOcrLines` → `replaceResults`。
  - `apps/oasd/ui/app.js:1026-1032`：`replaceResults` 直接 `saveCandidates = rows`，无确认、无备份。
- 触发步骤：
  1. 先“多行识别”，得到 5 行候选，取消勾选其中 3 行、改好名称，但还没保存。
  2. 去素材库点某个已保存素材的“识别”做测试。
  3. 测试完成后候选列表被该素材识别出的行替换；之前 5 行的勾选/改名全部丢失。
  4. 如果测试失败触发全屏回退，也会替换成整屏识别结果。
- session 依据：session 没有要求“测试已有素材时重建候选”；这属于用户要求排查的状态丢失。与 18:40 第 4 条“点测每次同一点”之类的测试操作相邻，容易被误触发。
- 建议：素材测试结果写入独立的只读结果区（或日志），不要调用 `renderOcrLines`；如确需复用候选，先 `confirm` 并保留旧候选快照。

### C-09 [P2] 全局素材库是只读、无刷新、无集合，当前无任务状态下无法管理任何素材

- 文件：
  - `apps/oasd/ui/app.js:3353-3392`：`loadGlobalAssets` 只渲染 `s.items`，忽略 `s.collections`；`renderRows` 参数 `manage:false, actions:false`（`:3371-3380`）。
  - `apps/oasd/ui/index.html:102-104`：全局库只有 status + list，无刷新/编辑/删除/测试/排序控件。
  - `apps/oasd/ui/app.js:228`、`:5713`：`btnBatchSave` 不存在，批量保存入口死。
  - `apps/oasd/ui/app.js:131`、`:5631`：`btnLibraryRefresh` 不存在，唯一的全局刷新绑定死。
- 触发步骤：
  1. 当前 `tasks/` 为空（真实工作区状态）：任务页素材库为空；素材页全局库只显示只读行。
  2. 想删除一个未被任何任务引用的全局素材、或测试它、或调整它的位置/操作区：没有入口。
  3. 想刷新全局库：没有按钮，只能整页刷新。
- session 依据：
  - 20:10/20:21 用户要求任务页跟随任务、全局库独立；20:28 用户追问旧 `library/` 与全局库区别、样式重复。
  - 20:11 助手曾承诺“库素材 id 输入 / 存入库 / 素材库下拉 / 刷新 / 全屏匹配”，但 20:33 又把旧 library UI 移除；当前 HEAD 既没有旧 library 控件，也没有新的全局管理控件。
- 建议：
  - 全局库至少提供“刷新 / 测试 / 改 / 位置 / 操作区 / 删 / 排序”，与任务库复用同一 `OASAssetUI` 行；未引用素材的删除由后端全局引用检查保护（已有）。
  - 补上 `s.collections` 的渲染（`template_set` 行），否则全局集合完全不可见。
  - 若产品决定全局库只读，则必须在任务库提供“全部全局素材”视图，不能让无任务/未引用素材进入死区。

### C-10 [P2] 素材拖动排序与按分类分组渲染的语义不一致，跨分类拖动会重排分组或无效

- 文件：
  - `apps/oasd/ui/app.js:2397-2437`：`attachAssetDragReorder` 用 `taskAssets` 的**扁平顺序**生成 `itemIds`，drop 后把整份 `order` 发给 `/task/assets/reorder`。
  - `apps/oasd/ui/asset_ui.js:348-366`：`renderRows` 在 `group:true, groupBy:'category'` 时按分类建组，组顺序由 items 中首次出现的分类决定。
  - `apps/oasd/ui/app.js:2373-2379`：任务库就是用 `groupBy:'category'` 渲染的。
- 触发步骤：
  1. 任务库有分类 A=[a1,a2]、B=[b1]。
  2. 把 b1 拖到 a1 上：后端顺序变成 `[b1,a1,a2]`，重渲染后 B 组跑到 A 组前面；视觉上不是“把 b1 放进 A”，而是整组换位。
  3. 把 a2 拖到 b1 上：顺序仍是 `[a1,a2,b1]`，画面完全不变，用户会以为拖动失败。
- session 依据：18:40 第 6 条“素材也可以拖动排序”；20:10 要求任务素材库跟随任务。当前实现只在任务库有拖拽，全局库没有。
- 建议：排序按“分类内位置”持久化（或至少按当前分组计算组内 before/after），不要用扁平数组的全局 splice；如果要全局排序，就取消分组展示或在全局库提供排序。跨分类拖动应明确是“改分类”还是“移动顺序”，不要由 drop 位置隐式决定。另外 `asset_ui.js:259` 对所有非集合行都设了 `draggable=true`，但全局库没有绑定 reorder，拖动光标存在而动作无效，应同步关闭或接入。

### C-11 [P2] 帧分类改名/改分类后，任务素材库的分类筛选和分组不刷新

- 文件：
  - `apps/oasd/ui/app.js:3693-3711`（`renameFrameCategory` 只 `refreshFrames`）
  - `apps/oasd/ui/app.js:3713-3732`（`applyFrameCategoryFromSelect` 只 `refreshFrames`）
  - `apps/oasd/ui/app.js:4008-4026`（`setFrameCategory`/`moveFrameToCategory` 只 `refreshFrames`）
  - `apps/oasd/ui/app.js:2373-2379`：任务素材库按服务端返回的 `category` 分组/筛选；该 `category` 是 `/task/assets` 用帧分类即时派生的（`serve.rs:4422-4444`）。
- 触发步骤：
  1. 任务库有若干素材，按分类“式神录”分组。
  2. 图片面板把分类“式神录”重命名为“式神”。
  3. 帧列表/筛选立即更新；任务库仍显示“式神录”分组，点击分类筛选也仍是旧选项，直到保存素材/改位置/切换任务等触发 `refreshTaskAssets`。
- session 依据：18:40 第 7 条“帧分类添加重命名的功能”；18:42 计划要求“过滤/分组/当前帧分类同步刷新”。
- 建议：分类 mutation 成功后除 `refreshFrames()` 外，再单独刷新任务素材（注意 `refreshTaskAssets` 内部会 `refreshFrames`，要避免递归；可拆 `refreshTaskAssetsOnly`）。

### C-12 [P2] 清空/删除最后一帧后，位置帧选择器仍列出已删除帧

- 文件：`apps/oasd/ui/app.js:3525-3538` 空列表分支直接 `return`；`refreshAssetPositionFrames()` 只在函数末尾 `:3676` 调用，走不到。
- 触发步骤：
  1. 图片面板有若干帧，素材编辑器的“位置帧”下拉已加载这些帧。
  2. 点“清空列表”确认（或删除最后一帧）。
  3. 帧列表变空、`currentFrameId=null`，但位置帧选择器仍显示旧帧；此时保存素材可能把已删除的 frame id 写进 positions，或被服务端拒绝。
- session 依据：18:40 第 1 条要求位置帧详情/位置设计；这是“状态不一致”，不是新需求。
- 建议：空列表分支也调用 `refreshAssetPositionFrames()`（或在 `finally` 中统一刷新），并清空 `assetFramesCache`。

### C-13 [P2] 删除任务确认文案仍说“assets/images 会被删除，不可恢复”，与后端备份/保护行为冲突

- 文件：`apps/oasd/ui/app.js:801-809`；后端 `apps/oasd/src/serve.rs:4167-4196` 明确先备份 `assets.json + images/` 到 `assets/_deleted-tasks/<task>-<ts>/`，失败则拒绝删除，并返回 `asset_backup`。
- 触发步骤：
  1. 任务页点某个任务“删除”。
  2. 弹窗写“任务目录（flow/assets/images）会被删除，不可恢复。”
  3. 实际后端会备份；如果用户因为这句话不敢删/误以为素材又丢了，会造成和 20:28 同类的信任问题。
  4. 删除成功后 UI 只 `refreshLibrary()`，不 `refreshTaskList`/不清空当前任务；当前工作台仍显示已删除任务的素材/flow。
- session 依据：
  - 20:21 用户问“是不是删除任务导致任务素材被删除”；20:28“素材不止这一次被删除了”；20:38“你搞乱了我的仓库”。
  - 20:33 助手承诺“先备份 → 备份失败拒绝删除”。
- 建议：文案改为“任务目录会删除；素材会先备份到 assets/_deleted-tasks/…，可从备份恢复”，成功后显示 `r.asset_backup`；删除当前任务时清空 `taskName/taskSelect/taskAssets` 并刷新任务列表。

### C-14 [P2] builder 自动保存竞态：保存期间的新编辑会被标记为“已保存”

- 文件：`apps/oasd/ui/builder.js:1779-1804`。
- 原因：
  - `:1784` `if (B.saving) return;` 会直接丢掉本轮定时保存；
  - `:1790` 请求体是调用时的 `B.flow` 快照；
  - `:1797` 成功后却执行 `B.lastSavedFlow = JSON.stringify(B.flow)`。如果用户在 `await` 期间改了 flow，新内容被误标为已保存，不会再触发 autosave。
- 触发步骤：编辑一步触发 800ms autosave；在请求返回前继续改代码/步骤；保存成功后停止编辑 → 最后一次修改不会写盘，刷新后丢失。
- session 依据：不在 7 项需求内，但属于“保存/状态丢失”，且 builder 是任务 flow 的唯一编辑器。
- 建议：请求前 `const snapshot = JSON.stringify(B.flow)`；成功后 `B.lastSavedFlow = snapshot`；若 `snapshot !== JSON.stringify(B.flow)` 则再次 `scheduleAutoSave()`；`B.saving` 时记录 pending 而不是丢弃。

### C-15 [P2] 切换任务时 `refreshTaskAssets`/`loadTaskFlow` 无请求序号，旧响应可覆盖新任务

- 文件：`apps/oasd/ui/app.js:2249-2308`（`refreshTaskAssets`）、`:3254-3292`（`loadTaskFlow`）。
- 触发步骤：任务下拉 A → B 快速切换；A 的 `/task/assets`/`/task/flow` 响应晚于 B 返回，`taskAssets`/`taskFlowRaw` 被 A 覆盖，界面标题/任务名已是 B，但素材/flow 是 A；此时保存/删除会作用到当前 `taskName`（B）和旧数据混合。
- session 依据：20:10~20:28 任务/素材归属是核心；这类竞态会再次造成“素材跟错任务”。
- 建议：`const token = ++refreshSeq`，响应后 `if (token !== refreshSeq) return;`；或使用 `AbortController` 取消前一个请求。加载失败时也要清空旧 `taskAssets`，不要保留上一任务的列表。

### C-16 [P2] “保存素材”没有防重复提交

- 文件：`apps/oasd/ui/app.js:2931-3021`、`:5541`；`saveAssetUnified` 全程没有 `btnSaveAsset.disabled = true`，成功后靠 `replaceResults` 才间接禁用。
- 触发步骤：勾选结果后快速双击“保存素材”。两个 POST 并发：一个成功，另一个报 `already exists`，用户看到失败/部分成功；批量场景可能产生重复错误提示或覆盖竞争。
- session 依据：用户要求保存/过滤一致；21:18 的修复未覆盖并发。
- 建议：进入函数即禁用按钮，`finally` 恢复（或按候选状态重算）；后端对同一批 id 的并发写也要有幂等/锁，前端至少防抖。

### C-17 [P2] 识别/匹配测试没有展示真实 bbox + 有效性；点测 frame_source 回退分支是死代码

- 文件：
  - `apps/oasd/ui/app.js:1692-1695`：`testAssetOcr` 只显示识别区域/操作区域/帧标签；`:1787-1793` 结果没有 recognized_bbox，也没有 `actionValidity`。
  - `apps/oasd/ui/app.js:2770-2802`：`testAssetMatch` 不计算有效性。
  - `apps/oasd/ui/app.js:1580-1583`：`testAssetClick` 的 `typeof s !== 'undefined' && s.frame_source` 中 `s` 是 if/else 块内 `const`，外层不存在该绑定，`typeof s` 永远 `undefined`，服务端 `frame_source` 永远不会被采用。
  - `apps/oasd/ui/app.js:1568`：多行派生素材的 `line = find(expected) || s.lines[0]`；期望文字没找到时用第一行冒充目标，仍显示“有效”，而不是“未匹配到期望文字”。
  - `apps/oasd/ui/app.js:1534-1551`：template 点测只判断 `s.error`，不判断 `s.pass`/`s.threshold`；低分匹配也会被当作 recognized bbox 继续算有效性和目标点。
- 触发步骤：对已有 OCR/模板素材点“识别/测试”；结果区只有文本/得分，没有“真实识别框 + 与操作区是否重叠 + 有效/无效/独立”；点测日志的帧来源只靠本地回退，遇到服务端回退到 current 时会标错。目标文字消失但屏幕上有别的文字时，点测会把第一行当目标并报“有效”；模板低分匹配也会被当作有效定位。
- session 依据：
  - 18:40 第 5 条要求识别/操作区域有效性；
  - 19:05 明确“测试结果会显示实际匹配行和 bbox，而不是只显示配置 ROI”；
  - 目前只有 `testAssetClick` 部分做到。
- 建议：抽一个共享的 `renderTestResult({recognizedBox, actionShape, validity, frameSource})`，OCR/匹配/点测都调用；`testFrameLabel` 与服务端 `frame_source` 统一；删掉 `typeof s` 死分支。

### C-18 [P2] 保存/测试反馈只写进默认收起的结果/日志窗口，主界面无提示

- 文件：
  - `apps/oasd/ui/index.html:39`：`#canvas-windows` 默认 `collapsed`；
  - `apps/oasd/ui/app.js:3011-3014`：保存结果只写 `resultEl`；`log()`（`:295-308`）只写 `logEl`，不更新 header `status`；
  - `apps/oasd/ui/app.js:5686-5692`：只有手动点“显示结果/日志”才展开。
- 触发步骤：默认页面（结果/日志收起）→ 保存素材/点测 → 画面无任何 toast/状态变化；用户以为没生效，随后发现素材没在任务库出现（叠加 C-01），就会判断为“素材丢了”。
- session 依据：18:40 第 1 条要求“画面不遮挡”，19:31 改成默认收起是合理的；但收起后必须给可见反馈。
- 建议：保存/测试/删除等操作在 header `#status` 或画布下方给一行短提示 + 成功/失败色；失败时自动展开结果窗口；不要只写隐藏区域。

### C-21 [P2] 拖测没有走实时/位置帧，且生成的 drag 步骤仍只有 bbox（P1-1）

- 文件：`apps/oasd/ui/app.js:1601-1641`（`testAssetDrag` 全程无 fetch，只用 `assetActionBox` 的 bbox 和本地随机）；`:1626-1635` 的 `from` 固定 `{kind:'region', roi: action}`。
- 触发步骤：
  1. 开实时，画布/素材布局与位置帧不同；点“拖测” → 起终点仍按素材保存时的 box 计算，不是实时帧上的目标，也没有任何识别/帧来源标签。
  2. 素材是 polygon/invert：UI 从 polygon/全屏反选取起点，但复制出的 step 的 `from` 是 bbox 矩形，运行时仍按矩形拖。
- session 依据：18:40 第 3 条（拖测也要测实时帧）、第 4 条（拖测随机）、`CODE-AUDIT` P1-1 未修。
- 建议：拖测至少请求一次 `testFrameQuery` 对应的帧并显示来源；step 的 `from` 支持 action shape 引用（或明确标注“仅 UI dry-run，运行时仍按 bbox”），不要静默不一致。

### C-19 [P3] 大量旧 DOM id / 旧 library / 旧 flow 编辑器死引用；`063c329` 的批量修复在死代码里

- 文件（代表性）：
  - `apps/oasd/ui/app.js:128-135`：`library-id`、`btn-library-save`、`library-select`、`btn-library-refresh`、`btn-library-match`；`index.html` 均无对应 id。
  - `apps/oasd/ui/app.js:3325-3451`：`loadLibrary`/`saveLibraryAsset`/`matchLibrary`；`loadLibrary` 因 `librarySelect` 为 null 直接 return，其余函数没有入口（绑定在 `:5630-5632` 的 null 元素上）。
  - `apps/oasd/ui/app.js:19-21`：`asset-id`/`asset-kind`/`btn-asset`；`doSaveAsset`（`:3024`）不可达。
  - `apps/oasd/ui/app.js:228`、`:5713`：`btn-batch-save` 不存在，`saveSelectedCandidates`（含 `063c329` 的 candidate actionShape 修复）不可达。
  - `apps/oasd/ui/app.js:4909-4928`：`btn-match-save` 不存在，但 `doMatch` 结果文案仍让用户点“用匹配结果建素材”（`:4889`）。
  - `apps/oasd/ui/app.js:24`、`:116-123`、`:85-89`：`flowJson`/`flowSteps`/`flow-op`/`hdr-drawers`/`result-pop`/`log-pop` 等旧工作台 DOM 全部不存在；旧 flow 可视化代码保留。
  - 自动扫描结果：`app.js` 共引用 182 个 `getElementById` id，其中 32 个不在 `index.html`。
- 影响：多数有 `if (el)` 保护，**不会启动报错**，但会制造“按钮在文档里承诺、DOM 里不存在”的假功能；并且唯一的全局库刷新绑定就是死 id，直接导致 C-01。
- session 依据：20:33 助手明确“旧 library/ UI 已移除；只保留后端 /library 与 lib: 兼容”；20:11 又曾声称 `#library-select` 存在。代码没有按 20:33 的口径清理干净。
- 建议：删除不可达函数/常量/旧 DOM 分支（或恢复必要控件）；至少把 `btn-library-refresh` 换成实际存在的全局刷新按钮；`saveSelectedCandidates` 要么接入 UI 并补 positions，要么删除，避免审计把死代码当修复。

### C-20 [P3] 帧详情缺“文件”字段，且只有当前帧一份详情

- 文件：`apps/oasd/ui/app.js:3490-3511`（分类/尺寸/来源/时间/大小/状态/关联素材，无文件名）；服务端 `/frames`（`serve.rs:1274-1296`）也未返回 `file`。
- 触发步骤：选中一帧查看详情；计划中的“文件：frame-4.png”不显示；多选帧时没有“当前选择 #4”的概念（UI 本身也没有多选）。
- session 依据：18:40 第 1 条/`EDITOR-UX-PLAN §1` 的详情字段包含“文件”。
- 建议：`/frames` 返回基名 `file`，详情补一行；多选如不做，就在计划中删掉该承诺。

## 3. 需求覆盖对照

| session 要求 | 现状 | 结论 |
|---|---|---|
| 1. 位置帧详情、画布不被遮挡/底部裁切 | 16:9 stage + `fitCanvasToStage` + ResizeObserver；结果/日志默认收起；详情移到图片侧栏 | ✅ 基本完成；详情缺“文件”，默认收起导致反馈不可见（C-18/C-20） |
| 2. 反选/多边形、框选抽函数 | action_shape/invert UI + 采样 + 后端校验已做；polygonOverlapsRect 已修 | ⚠ 保存链路丢 action polygon/invert（C-02）；识别 polygon 掩膜保存丢失（C-07b）；SelectionTool 未抽且串状态（C-06） |
| 3. 实时开/关所有测试用对应帧 | 单素材 click/OCR/match 带 live/frame_id；collection 已修 | ❌ 主 OCR 按钮不带（C-04）；拖测不请求帧（C-21）；多位置只测第一个（C-05） |
| 4. 点测/拖测随机 | 点测/拖测起点已随机，拖测有横向抖动 | ⚠ 点测在 recognition 素材上仍从 ROI 采样（C-07）；无 seed/帧标签；drag step 仍 bbox（C-21） |
| 5. 真实识别 bbox 与操作区有效性 | `actionValidity` + `polygonOverlapsRect` 已实现；点测显示 | ⚠ 只有点测显示；OCR/匹配测试没有（C-17）；OCR/模板新保存为 fixed（C-07） |
| 6. 素材拖动排序 | 任务库支持 reorder 接口 | ⚠ 分组渲染与扁平顺序冲突，跨分类拖拽异常（C-10）；全局库无排序（C-09） |
| 7. 帧分类重命名 | 接口 + 按钮 + 空值禁止已做 | ✅ 功能可用；❌ 任务素材分类不刷新（C-11） |
| 20:10 后任务/全局素材库 UI | 任务库移到任务页、全局库回素材页、复用 asset_ui 行 | ⚠ 位置/复用正确；全局库只读且不刷新，新素材双库消失（C-01/C-09） |

## 4. 测试覆盖缺口（为什么这些问题没被 361 个 Rust 测试拦住）

1. `apps/oasd/ui` 没有自动化测试：`flow_model_test.html` 只覆盖 `flow_model.js`；`app.js`/`asset_ui.js`/`builder.js` 的纯逻辑（`actionValidity`、`polygonOverlapsRect`、`currentActionShape`、`collectFlowAssetIds`、排序计算、DOM 状态机）全部无测试。
2. `node --check` 只能发现语法错误；`boxCenter` 重复声明、`typeof s` 作用域、`actionPoly` 优先级、保存期间 autosave 竞态都不会被发现。
3. `063c329` 的“浏览器实测”没有可复跑脚本；P1-2 的“已修”落在不可达的 `saveSelectedCandidates`，说明验证只看了 diff，没有从实际按钮点到保存结果。
4. Rust 侧的 `action_shape`/`polygon` 测试覆盖了数据模型与 runtime，但覆盖不到“前端是否把 shape 放进 payload”“是否带 live 参数”“多位置是否循环”。
5. 建议：把 `actionValidity`、`polygonOverlapsRect`、`actionShapeBBox`、`currentActionShape`、排序 order 计算抽成可在 `flow_model_test.html` 同类页面运行的纯函数测试；至少补 5 条浏览器/Node 测试：
   - polygon/invert 保存 payload；
   - recognition polygon 保存/读回；
   - recognition vs fixed 默认；
   - `boxCenter` 类型；
   - clear 后 actionPoly 状态；
   - 多位置循环与 live query。

## 5. 建议修复顺序

1. **C-01/C-09**：先让素材“看得见、刷得新、管得了”——保存后刷新全局库、任务库显示未引用提示/全部视图，全局库补刷新与管理入口。这是当前用户最容易再次判定“素材丢了”的点。
2. **C-02/C-06**：统一 shape 保存与 SelectionTool；先修 `currentActionShape` 的 polygon 优先级和 `btnClear`/模式切换 reset，再抽函数。
3. **C-03**：改名/删除重复 `boxCenter`，一行修复。
4. **C-04/C-05/C-21**：主 OCR、拖测、多位置统一走 `testFrameQuery` + `frame_source` 显示。
5. **C-07/C-07b/C-17**：默认 recognition、改 ROI/操作区的联动、所有测试统一显示 recognized_bbox + validity，并把识别 polygon 写入保存 payload。
6. **C-08/C-11/C-12/C-13**：状态/缓存/文案一致性。
7. **C-10/C-14/C-15/C-16**：排序语义、autosave/任务切换/重复提交竞态。
8. **C-19/C-20**：清理死代码/死 id，补帧详情文件字段。

## 6. 审计限制

- 未启动 `serve`，未在浏览器中实际点击；所有“触发步骤”为静态代码路径推导，其中 `boxCenter` 与 `typeof` 作用域已用 Node 复现。
- 未读取 `frames/frames.json`、`assets/assets.json` 的内容来核对具体数据；数据状态以工作区现状（`tasks/` 空、`assets/` 存在）为准。
- 未修改任何代码/数据；本文件是唯一写入。
