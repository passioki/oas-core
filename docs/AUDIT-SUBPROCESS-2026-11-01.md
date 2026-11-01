# 子进程独立代码审计（2026-11-01 16:00 → 21:50）

> 本报告汇总 3 个独立子进程的只读审计，不照抄上一轮 `CODE-AUDIT-2026-11-01.md` 的结论。
> 原始报告：
> - `_verify/audit-20261101/A-session-git.md`（session↔git↔需求）
> - `_verify/audit-20261101/B-runtime.md`（runtime/task/server + probe）
> - `_verify/audit-20261101/C-ui.md`（UI/前端）
> HEAD：`983c740`；`cargo test --workspace` = **361 passed / 0 failed**（B 实测）。
> 结论：**361 全绿不代表路径正确；上一轮“P1-2 已修”不成立。**

## 0. 先看结论

1. 代码主方向符合 18:40 的 7 项要求；4 个 docs-only 提交真实；实现从 19:30 `9618cb5` 开始。
2. `9618cb5` 未经用户要求把数据删除混进功能提交：删 `tasks/返回庭院/flow.json`、`images/a001.png`，`meta.json` 10 字段→3。任务目前不可运行。
3. `1f73f17` 的 34 素材恢复范围错误：只扫 task 历史，漏 archive 2827/2099，且 20 个来自已回滚迁移分支。
4. 上一轮修复只解决了一部分；**仍有 1 个 P0 用户可见问题、8 个 P1 确定缺陷**，另有 2 个未完成结构项。
5. 测试缺口：UI 无自动化测试；多个“已修”落在不可达代码或只修了一个入口；多个测试是假绿。

## 1. P0（用户可见/数据/语义）

### P0-1 新素材保存后从两个库同时“消失”
- `app.js:2283-2295`：任务库按 flow 引用过滤，新素材尚未被引用 → 不显示。
- `app.js:3353-3392`：全局库只在 boot 加载；唯一刷新绑定 `#btn-library-refresh` 在 `index.html` 不存在（`app.js:131/5631`）。
- 保存成功后只 `refreshTaskAssets(task)`（`app.js:3017`），不刷新全局库；结果只写默认收起的结果窗口。
- 用户会再次看到“保存成功但素材库为空/素材丢了”。
- 证据：C-01。

### P0-2 批量保存把“没有显式操作区”强制写成 fixed
- `app.js:2966-2972`（OCR 批量）与 `:2988-2999`（非 OCR）都写 `action_source:'fixed'` + rect；非 OCR 固定 `{kind:'rect'}`。
- `action_source=recognition`（无 shape、跟随真实识别 bbox）是 19:12 用户裁定；当前 UI 没有任何地方写入 recognition。
- 后果：OCR 识别目标移动后，操作区仍点旧位置，正好违背第 5 条要防的问题。
- 证据：A-F04、B-06、C-07；上一轮 `CODE-AUDIT` 漏项。

### P0-3 操作区多边形/反选无法可靠保存
- 实际保存入口 `saveAssetUnified` 用 `c.actionShape || rect`（`app.js:2970`）。
- `actionPoly` 双击闭合只置 `actionPolyClosed`（`app.js:4661-4670`），**不写候选、不调用 updateAssetActionRoi**。
- `saveSelectedCandidates` 里的 `c.actionShape` 修复因 `#btn-batch-save` 不存在而不可达（`app.js:228/1256`）。
- 结果：画了多边形/反选后保存，退回矩形；已有素材多边形闭合也没有保存请求。
- 证据：C-02、C-06、B-06。

## 2. P1（确定性逻辑缺陷）

### P1-1 UI 模板点测/拖测/有效性使用未平移的 action_shape
- `app.js:1456-1481` `randomPointInAction`、`:1523-1595` `testAssetClick`、`:1601-1641` `testAssetDrag`、`:1413-1454` `actionValidity` 都用保存时绝对 shape；runtime 已按 `matched - roi` 平移（`lib.rs:1286`）。
- 模板换位置帧后，UI dry-run 与 runtime 点击不一致。
- 证据：B-01。

### P1-2 `click_target` 的 OCR 路径忽略精确 shape
- `lib.rs:2315-2323`（`ocr_lines` 直接用识别行框）、`:2357-2382`（单行只用 bbox/invert 无效），没有 `click_box_for`/`adjust_plan_for_shape`。
- 编辑器给 OCR 生成的是 `click_target`（`app.js:1643-1658`）；B probe 实测全屏反选 OCR 仍点排除区。
- 证据：B-02。

### P1-3 精确 shape 采样失败被静默吞掉
- `ClickShape::sample`（`lib.rs:111-138`）失败返回 bbox 中心/None；`adjust_plan_for_shape`（`:1326-1335`）`None` 时直接返回原 plan，不报错。
- B probe：全屏反选有效区为空仍点击 (52,29) 且 run=Done；共线 polygon sample 返回 `(20,10) contains=false`。
- 证据：B-03。

### P1-4 region 精确 shape 被 ±6px jitter 带出
- `lib.rs:2592-2598` region sample 后调用 `humanize.point_click`，默认 jitter 6px（`humanize.rs:328`），未再校验 shape。
- B probe：1px 宽 region 点击 x=5 在 shape 外。
- 证据：B-04。

### P1-5 action_shape 缺越界/零面积校验
- `oas-task/lib.rs:169-205` 仅查自交/非负；probe：64×48 帧接受 `[100,100,10,10]`，runtime clamp 到 (63,47) 仍 Done；3 点共线 polygon 也保存成功。
- 证据：B-05。

### P1-6 主“试识别/多行识别”不走 live/frame_id
- `app.js:4813-4825` `btnOcr`、`:4983-4994` `btnLines`、`:4779-4790` fallback 只拼 x/y/w/h；服务端已支持 `frame_for_test`。
- 实时开着时主识别仍测旧 State.frame。
- 证据：C-04。

### P1-7 单素材多位置只测第一个
- `app.js:1510-1521` 的 `testFrameQuery/testFrameLabel` 只取 `positions[0]`；18:56 用户明确“3 都测”。
- batch 测全部，但任务页按钮不传 assets 过滤（`serve.rs:2946`）。
- 证据：C-05。

### P1-8 SelectionTool 未抽 + 串状态
- `polygon/polyMode` 与 `actionPoly/actionPolyMode` 两套；`btnClear` 不清 actionPoly/invert/mode（`app.js:4762-4777`）；模式互切不清状态。
- 会产生“新画矩形保存成旧 polygon”“识别多边形画不出来”等具体 bug。
- 证据：C-06、B-01 相关。

## 3. 其他值得记录

- `boxCenter` 在 `app.js:564` 与 `:1492` 重复声明，后者覆盖前者；多行识别→生成点击指令输出 null 坐标。**该 bug 在 `f6be0f0` 已存在，不是 4pm 后引入**，但仍应修（P1）。
- 全局库只读/无刷新/不渲染 collections（C-09）；任务/全局素材 UI 位置最终正确，但管理能力缺失。
- `ActionShape::contains`（`oas-task:140-166`）仍是旧 `.max(1)` 算法，当前无调用者（潜伏）。
- `update_asset_meta` 对 region 拒绝 action_shape（`oas-task:1420-1423`），UI 编辑 region 必失败。
- batch region 只校验 `item.roi`，不校验 action_shape（`serve.rs:3019-3031`）。
- `frame_for_test` 非法 frame_id 静默回退；live 用缓存帧，不是“每次强制抓取”。
- task delete backup 无测试；`assets.json` 缺失时 images 不备份。
- 文档 `README/RESUME/SESSION-HANDOFF/PENDING` 与 HEAD/任务/帧/修复状态不一致；`CODE-AUDIT` 的 P1-2 过度声明。

## 4. 已被独立验证为“确实修复”的项

- P0-1 `actionValidity` fixed 独立区 → `valid:true`；浏览器实测。
- P0-2 polygon 有效性 → `polygonOverlapsRect`；浏览器实测。
- P0-3 runtime 斜边 → f64；新测试可抓住旧算法。
- P0-4 runtime template 平移 → `translated_click_shape`；新测试可抓住旧实现（只测 rect）。
- P1-3 collection live → `frame_for_test(&q)` + `frame_source`；代码正确，无 serve 层测试。
- P1-6 自交校验 → 已修；越界/零面积仍缺。
- P1-4 OCR 帧标签仅单素材路径修了；主 OCR 按钮未修。

## 5. 建议修复顺序

1. **数据/提交**：恢复 `8ee414c` 的 `返回庭院` flow/meta（或用户确认不恢复）；不要把数据混入代码修复。
2. **P0-1/P0-3**：素材保存后刷新全局库；统一 shape 保存入口，消除死代码/矩形回退。
3. **P0-2**：默认 `recognition`，只有显式编辑操作区才写 `fixed+shape`。
4. **P1-1/P1-2/P1-3/P1-4/P1-5**：runtime exact shape 的 loud failure、OCR click_target、UI 平移、region jitter、bounds。
5. **P1-6/P1-7/P1-8**：主 OCR live、多位置循环、SelectionTool。
6. **P2/P3**：全局库刷新/管理、boxCenter、分类刷新、清空帧缓存、确认文案、autosave 竞态、死代码。
7. 补 UI 自动化测试；否则 361 Rust tests 仍会假绿。

> 本文件只做审计记录；未改代码/数据。下一轮修复前先备份并确认优先级。

## 6. 修复状态（2026-11-01 22:20）

> 用户确认“P0/P1 一起做、恢复 flow/meta、纳入审计报告”后，由两个实现子进程完成。

- 代码提交：`b910a2b fix(editor/runtime): close P0/P1 audit findings`
- 数据提交：`45c1f32 data: restore 返回庭院 flow/meta from 8ee414c`
- 审计报告提交：`30d3145 docs: add independent subprocess audit reports`
- 测试：`cargo test --workspace` = **381 passed / 0 failed**（runtime 110 / task 55 / oasd 131）
- UI：`node apps/oasd/ui/ui_pure_test.js` = **42 checks passed**；页面浏览器实测全局 34、任务页“显示全部”34、全局刷新按钮存在、无 JS 错误。
- `oasd task lint --tasks-dir tasks --all` = `linted=1 errors=0`。

| 项 | 状态 |
|---|---|
| P0-1 双库消失/刷新 | ✅ 保存后刷新两库；全局库刷新/管理；任务页未引用提示+显示全部 |
| P0-2 批量保存语义 | ✅ 默认 recognition；仅显式操作写 fixed+shape；region 保持语义 |
| P0-3 polygon/反选保存 | ✅ 候选/已有素材闭合写回；矩形清旧 polygon；清除/模式互切复位；死代码删除 |
| P1-1 UI 模板 shape 平移 | ✅ `translateActionShape` 用于点测/拖测/有效性 |
| P1-2 runtime click_target OCR | ✅ Lines/Single 走精确 shape；polygon/invert 测试 |
| P1-3 采样失败 loud error | ✅ sample 返回 Result；空有效区不点击 |
| P1-4 region jitter | ✅ 抖动后校验/重采样，窄 shape 测试 |
| P1-5 shape 校验 | ✅ 越界/零面积/collection；保存/加载一致 |
| P1-6 主 OCR live | ✅ btnOcr/btnLines/fallback 带 `testFrameQuery` 并显示 `frame_source` |
| P1-7 多位置循环 | ✅ click/OCR/match/drag/collection 逐帧；任务批量传 assets 子集 |
| P1-8 SelectionTool | ⚠ 行为串状态已修（统一 reset）；两套几何存储未强行合并 |
| boxCenter | ✅ 只保留正确对象版本 |
| OCR 识别 polygon 保存 | ⚠ 后端仍有限制（非操作区 action_shape；需要单独扩展字段） |
| `frame_for_test live` 强制重抓 | ⬜ 仍取缓存 live_frame（P2） |
