# 代码审计：2026-11-01 16:00 → 21:00

> 范围：`f6be0f0`（16:21 干净基线）→ HEAD `983c740` 的 git 改动，对照 session 中 18:40~19:15 的 7 项要求与后续 UI 反馈。
> 结论：**代码主方向对，但实现中存在确定不一致/缺陷。本文件只记录了首轮 4 P0/4 P1+结构项；后续独立子进程审计又发现多项漏项，见 `AUDIT-SUBPROCESS-2026-11-01.md`。**

## 1. 提交边界

```
f6be0f0 16:21   干净基线（22 帧 / 返回庭院 3 素材）
18:42~19:14     4 个 docs-only 提交
19:30 9618cb5   第一笔实现（7 项 + action_shape + 0.1.0）
19:52 c557ff1   runtime exact click
20:01 33ad968   runtime exact region + live batch
20:11 1a021f8   任务/全局素材 UI 位置
20:23 0bc69ba   全局素材恢复（数据动作）
20:33 1f73f17   34 素材恢复 + UI
```

## 2. 确定缺陷

### P0-1 固定操作区不重叠被错误标为无效

- 文件：`apps/oasd/ui/app.js:1380-1396`
- 现象：
  - `action_source=fixed` 且与识别框不重叠时返回 `valid:false`；
  - 文案却写“独立操作区域（fixed，允许不重叠）”。
- 用户要求（18:40 第 5 条）：
  > “如果显示指定了操作区域非识别内容，那么也当做有效”
- 修复：`fixed` 不重叠时 `valid=true`，仅标记独立。

### P0-2 多边形有效性判断实际用了整屏

- 文件：`apps/oasd/ui/app.js:1388-1391`
- 现象：
  - `effective` 初始化为整屏；
  - 只有 `kind=rect` 才改成 shape roi；
  - `kind=polygon` 时 `rectsOverlap(recognizedBox, 整屏)` 恒为 true。
- 影响：多边形操作区的重叠校验失效。
- 修复：对 polygon 用 bbox 或真实多边形与 bbox 的相交判断。

### P0-3 runtime 多边形命中算法错误

- 文件：`crates/oas-runtime/src/lib.rs:75-98`
- 现象：
  ```rust
  (p.x < (xj - xi) * (p.y - yi) / (yj - yi).max(1) + xi)
  ```
  `max(1)` 把负斜率边改成除以 1，导致斜边判定错误。
- 影响：多边形操作区采样/点击可能落在区域外；单元测试只测了轴对齐矩形，未覆盖斜边。
- 修复：`if yj != yi { ... }`，或统一使用浮点射线法；补斜边测试。

### P0-4 template 平移后仍用未平移 shape 校验

- 文件：`crates/oas-runtime/src/lib.rs:1282-1301`、`1353-1377`
- 现象：
  - `click_box_for` 把 shape bbox 按 `p - roi` 平移；
  - `adjust_plan_for_shape` 却用**原始未平移**的 shape 判断/采样。
- 影响：模板/按钮在屏幕上移动后，合法点被丢弃或在旧位置采样。
- 修复：按同一 match offset 平移 shape（rect/points 都平移），或传入已平移 shape。

### P1-1 拖测生成的 drag 步骤仍只带 bbox

- 文件：`apps/oasd/ui/app.js:1548-1581`
- 现象：
  - 拖测 UI 用 `randomPointInAction` 展示了 polygon/invert 点；
  - 生成 step 的 `from` 仍是 `{kind:'region', roi: action}`（bbox）。
- 影响：用户以为拖测/flow 使用多边形/反选，运行时实际按矩形。
- 修复：drag 步骤需要支持 action shape（新 DSL 字段或引用素材），否则明确提示“仅 UI dry-run”。

### P1-2 批量保存丢失 polygon/invert

- 文件：`apps/oasd/ui/app.js:1270-1276`、`2911-2950`
- 现象：
  - `saveSelectedCandidates` / `saveUnifiedAssets` 固定写 `action_shape: {kind:'rect'...}`；
  - 只有单素材 `doSaveAsset` 使用 `currentActionShape()`。
- 影响：先画多边形/反选，再批量保存 → 变成矩形。
- 修复：候选结果保存 action shape，或批量保存也调用 `currentActionShape()`。

### P1-3 collection 测试不使用 live/position 来源

- 文件：`apps/oasd/src/serve.rs:3218-3225`
- 现象：
  - `handle_collection_match` 直接读 `st.frame`；
  - `/task/asset/match`、`/ocr`、`/lines`、`batch_test` 已支持 `live/frame_id`，collection 没同步。
- 影响：第 3 条“实时开着时所有测试都测实时帧”对 collection 不成立。
- 修复：复用 `frame_for_test(&q)`。

### P1-4 单素材 OCR 测试的帧标签仍是 currentFrameId

- 文件：`apps/oasd/ui/app.js:1635-1655`
- 现象：
  - 请求可能带 `live=1` 或 `frame_id=11`；
  - UI 的 `frameLine` 仍显示 `currentFrameId`。
- 影响：日志/结果与真实测试帧不一致。
- 修复：使用 server 返回 `frame_source` 或实际 `it.positions[0]`。

## 3. 结构/设计不一致

### P1-5 “框选抽为函数”没有真正落地

- 用户 18:40 第 2 条要求：
  > “框选可以抽为一个函数”
- 现状：
  - `polygon/polyMode`、`actionPoly/actionPolyMode` 是两套状态；
  - mousedown/mousemove/dblclick 各有一组分支；
  - 没有统一 `SelectionTool`。
- 影响：修改一个模式容易漏另一个；与计划 `EDITOR-UX-PLAN §2` 不符。
- 建议：下一轮先抽，不再继续加功能。

### P1-6 action_shape 校验不完整

- 文件：`crates/oas-task/src/lib.rs:169-201`
- 缺失：
  - polygon 自交校验；
  - polygon/rect 是否越界；
  - 与旧 recognition polygon 的校验不一致。
- 影响：坏 shape 可写入；运行时采样失败。
- 修复：复用 `polygon_self_intersects` 和 frame bounds。

## 4. 符合项

- 画布 16:9 stage、结果/日志收起、帧详情侧栏：符合。
- 单素材/批量 live/position：单素材与 batch 符合；collection 除外。
- 点测/拖测随机点：UI 采样符合。
- 素材拖动排序、帧分类重命名：符合。
- 版本 0.1.0、action_source/action_shape 字段：符合用户 19:12 裁定。
- 测试：审计时 `cargo test --workspace` = 358 passed；修复后 361 passed。

## 5. 建议修复顺序

1. P0-1/P0-2：`actionValidity`；
2. P0-3/P0-4：runtime polygon 命中 + template 平移；
3. P1-3/P1-4：collection/live 与 UI 标签；
4. P1-2：批量保存 shape；
5. P1-1：drag 的 shape 语义（需要 DSL 设计确认）；
6. P1-5/P1-6：SelectionTool 与 shape 校验。

> 本文件是审计记录；P0/P1 修复见第 6 节（2026-11-01 21:20）。

## 6. 修复状态（2026-11-01 21:20）

| 项 | 状态 | 证据 |
|---|---|---|
| P0-1 fixed 独立区误判无效 | ✅ 已修 | `actionValidity` 返回 `valid:true`；浏览器实测 |
| P0-2 polygon 有效性用整屏 | ✅ 已修 | 新增 `polygonOverlapsRect`；浏览器实测 |
| P0-3 runtime 斜边命中 | ✅ 已修 | `ClickShape::contains` 去 `.max(1)`；新增斜边测试 |
| P0-4 template 平移 shape | ✅ 已修 | `translated_click_shape`；新增平移测试 |
| P1-2 批量保存丢 shape | ⚠ **部分** | OCR 分支用 `c.actionShape`；非 OCR 仍写死 rect；polygon 闭合不写候选；`saveSelectedCandidates` 因 `#btn-batch-save` 不存在而不可达（见 `AUDIT-SUBPROCESS` P0-2/P0-3） |
| P1-3 collection live | ✅ 已修 | `frame_for_test(&q)` + `frame_source` |
| P1-4 OCR 帧标签 | ⚠ **部分** | 单素材 `testFrameLabel` 已修；主 `btnOcr/btnLines` 仍不带 live/frame_id |
| P1-6 shape 自交校验 | ✅ 自交已修 | `polygon_self_intersects` + 测试；**越界/零面积仍缺** |
| P1-1 drag shape | ⬜ 未修 | 需要 drag DSL 增加 shape asset 语义 |
| P1-5 SelectionTool | ⬜ 未修 | 需要统一重构，风险较高 |

测试：`cargo test --workspace` = **381 passed / 0 failed**（`b910a2b` 后；新增 20 个测试）。

## 7. 子进程复核修正（2026-11-01 21:50）

> 首轮审计漏了以下确定问题；以 `AUDIT-SUBPROCESS-2026-11-01.md` 为准。

- **P0**：新素材保存后任务库/全局库同时不显示（全局库无刷新入口）；批量保存强制 `fixed`（无显式操作区也写矩形）；操作区多边形/反选无法可靠保存（`actionPoly` 闭合不写候选、非 OCR 仍硬编码 rect、`saveSelectedCandidates` 不可达）。
- **P1**：UI 模板点测/拖测/有效性未平移 shape；runtime `click_target` OCR 忽略 shape；精确 shape 采样失败静默放过；region 采样后被 jitter 带出；shape 缺越界/零面积校验；主 OCR 按钮不带 live/frame_id；单素材多位置只测第一个；SelectionTool 未抽且串状态。
- **P1/P2**：`boxCenter` 重复定义（f6 已存在）；全局库只读不刷新；`ActionShape::contains` 潜伏旧算法；region update 失败；batch region 假绿；frame_for_test 静默回退；task delete backup 无测试。
- **验证修正**：P1-2 不是“已修”，P1-4 只修了单素材路径，P1-6 只修自交。
