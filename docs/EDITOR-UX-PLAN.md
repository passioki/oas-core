# 编辑器 UX / 选择模型改版计划（2026-11-01）

> 状态：**Phase A/B/C/D 行为层已实现**（runtime template/OCR/region 主点击 exact shape；live/position 批量已实测）。版本 `0.1.0`。
> ⚠ 未完成的结构项：`SelectionTool` 未抽出（识别掩膜/操作区两套几何按语义保留）；UI dry-run 与 runtime 仍是两套采样实现。**O-1（collection 真实点击）/ O-2（drag action shape）经两轮评审后在 dev `d1a7fcd` 收口**（第一轮 4 个 P1 + P2 已修）；**实现后状态与最终口径见 §15**。
> 目标：让素材/帧编辑更可维护、测试更可信、画面与操作区域不互相遮挡。
> 不引入 interface；唯一关系仍是 `Asset.positions`。

## 0. 总原则

- **画面归画面，信息归侧栏**：不在截图内部长期覆盖提示/坐标/图例。
- **同一语义只实现一次**：矩形/多边形框选、点击采样、区域重叠判断都抽成共享函数。
- **实时/位置帧语义显式化**：每个测试结果必须带 `frame_source`、帧 id、实际采样点。
- **不保留旧兼容**：`action_roi`/`roi_back` 已删除，当前本地 assets 一次性迁移。
- **先修可观察问题，再做数据模型扩展**。

## 1. 画面布局、位置帧详情、悬浮遮挡

### 现状问题

- `#canvas-wrap` 是 flex+overflow hidden，画布上方还有结果/日志窗口；
- `#screen` 只设 `max-width/max-height`，在窄高窗口下可能出现底部裁切；
- 结果/日志、提示、点击分布图例/状态分布在画布周围，容易遮挡或挤压画面；
- 选择位置帧后没有稳定的“帧详情”区域。

### 目标

1. 画布区域使用固定的 16:9 stage：

```text
#canvas-stage
  width = min(可用宽, 可用高 * 16/9)
  height = width * 9/16
  canvas 填满 stage
```

2. 任何画布尺寸变化（窗口缩放/侧栏拖动/面板切换）统一重算，不允许裁切。
3. 画布外放置：
   - 结果/日志窗口；
   - 位置帧详情；
   - 点击分布图例/状态；
   - 操作提示；
   - “当前点击位置”指示。
4. 位置帧详情放在图片侧栏的帧行下方/侧栏底栏，而不是画布上：

```text
#4 我的式神
分类：式神录
尺寸：1280x720
来源：capture
时间：...
文件：frame-4.png
大小：...
当前：是
关联素材：预设
```

5. 选中多个帧时显示“当前选择 #4”，信息只针对当前帧。

### 验收

- 常见窗口尺寸（1280x720、1366x768、1920x1080、窄窗口）底部最后一行像素可见；
- 画布内无固定 UI 覆盖（临时框选文案除外）；
- 截图与位置帧详情可同时看到，不互相遮挡。

## 2. 框选抽成共享选择工具

### 目标

把现有矩形 ROI / 操作区域 / 多边形逻辑抽成：

```text
SelectionTool:
  mode: rect | polygon
  target: roi | action
  points
  closed
  invert
```

共享行为：

- 起点/终点归一化；
- 多边形顶点新增/拖动/删除/闭合；
- bbox 计算；
- 命中测试；
- 反选状态；
- 最小尺寸校验；
- 鼠标/触控/取消。

只保留一套 canvas 事件处理，避免 ROI 和 action 两套逻辑分叉。

> **现状（2026-11-02）**：`SelectionTool` 未抽出；`polygon/polyMode` 与 `actionPoly/actionPolyMode` 仍是两套状态，按语义保留（见 §15.2）。

## 3. 反选 + 操作区域多边形（已确认：方案 B）

### 用户明确的反选定义

```text
反选 = 整个屏幕 − 选择区域
```

不是“识别外接矩形 − 多边形”。反选后的有效操作区域覆盖整个画面，只是排除用户画出的区域。

### 目标

- 识别区域、操作区域都支持：
  - 矩形；
  - 多边形；
  - 反选（整个屏幕减去选择区域）。
- 操作区域使用统一 `action_shape` 对象（方案 B）：

```json
{
  "action_shape": {
    "kind": "rect",
    "roi": [80, 90, 340, 220],
    "invert": false
  }
}
```

```json
{
  "action_shape": {
    "kind": "polygon",
    "points": [[80,90],[420,90],[420,310],[80,310]],
    "invert": true
  }
}
```

语义：

```text
invert=false  有效操作区域 = shape 自身
invert=true   有效操作区域 = 整个画面 − shape
```

- 反选范围的“整个画面”按素材/任务设计分辨率（当前 1280x720）计算；
- 识别侧继续沿用现有 `polygon` 掩膜语义，不把操作多边形混进识别掩膜；
- 旧字段 `action_roi` **已删除，不保留读取兼容**（见 §10；JSON/UI 已不读取）。
- runtime 内部 `click_back` 仅作为 region/collection 的 bbox 快捷路径保留，不是旧 JSON 字段兼容层。

### 运行时点击采样

- 非反选：
  - 在 shape 内部随机采样；
- 反选：
  - 在整个画面内随机采样；
  - 拒绝落在 shape 内部的点；
  - 保证最终点有效；
- 多边形使用点在多边形内测试；
- 反选/多边形均按 `action_shape` 精确采样；runtime 内部 `click_back` 只覆盖 region/collection 的 bbox 快捷路径，新数据不写 `action_roi`。

### 验收

- 画一个区域并反选后，保存能读回 `action_shape.invert=true`；
- dry-run 目标点落在“整个屏幕减去选择区域”内；
- 旧素材没有新字段仍可加载/运行；
- 反选区域不会因为外接矩形 bbox 而错误缩小。

## 4. 实时帧 / 位置帧测试语义（2026-11-04 更新）

> **2026-11-04 更新（§E-76）**：本节原「实时关闭 → 位置帧 / 单素材多位置逐个测 / 不用当前选中帧」口径已被**推翻**；现行口径见 §16.4。下面已按新实现改写，旧口径保留为一行历史注记。

### 目标

每个测试请求显式带帧来源：

```text
live=1        使用当前实时帧（每次测试前强制抓新帧）
live=0/false  使用显式/当前选中帧；单素材测试 = 当前帧，批量 = 素材 positions
```

规则：

- 实时开启：
  - 识别、点测、拖测、模板匹配、OCR 都使用**实时帧**；
  - 不允许使用旧缓存帧；
  - 结果记录 `frame_source=live` 与 `frame_seq`；
  - **live 恒优先**（即使同时给了显式帧 id）；真实点击强制 live。
- 实时关闭：
  - **单素材测试使用当前选中帧**（帧详情素材行使用该行帧 id；无当前帧时才兜底素材 `positions`）；
  - **批量帧测试使用所有位置帧**（`POST /task/assets/batch_test`，每个素材按其 `positions` 逐帧）；
  - 结果记录 `frame_source=position`、`frame_id`；后端对显式 `frame_id` 仍回 `position:#<id>`，UI 标签显示「当前帧 #N」。
- 页面/日志显示明确标签：

```text
[实时帧] #live
[当前帧] #26 悬赏封印_关闭弹窗
[位置帧] #4 我的式神        （仅批量 / 无当前帧兜底）
```

### 验收

- 实时开/关各跑一次，同一素材结果来源不同；
- 实时关闭时按**当前选中帧**测试（旧「不再隐式用当前选中帧代替位置帧」的验收已作废）；
- 批量帧测试能列出每个位置帧通过/失败；没有「单素材多帧」入口。

## 5. 点测/拖测固定点问题

### 根因（已审计）

- [app.js](D:/OAS-dev/oas-core/apps/oasd/ui/app.js) `testAssetClick` 直接使用 `boxCenter(action)`；
- `testAssetDrag` 起点也使用 `boxCenter`，只随机距离，X 固定；
- dry-run 没有复用 runtime 的 humanize 采样。

### 目标

抽出共享采样函数：

```text
sampleActionPoint(asset, rng) -> {x,y}
sampleDrag(asset, direction, rng) -> {from,to}
```

规则：

- 点击：在有效操作区域/多边形内随机，不固定中心；
- 拖动：
  - 起点随机；
  - 终点沿方向 + 横向抖动；
  - 终点仍落在允许区域/方向边界内；
- 每次测试记录 seed/采样点；
- **现状（2026-11-02）**：dry-run（UI JS）与真实 runtime（Rust）是两套采样实现，语义对齐但未共用同一函数；这是已知结构项。

### 验收

连续 5 次点测/拖测，坐标应出现合理变化；
无论怎么采样，点都在操作区域/多边形的有效集合内。

## 6. 识别区域 vs 操作区域有效性判断

### 先澄清三个不同概念

用户指出的关键误解：

> “识别到的区域”不是配置的识别 ROI，而是 OCR/模板**真实识别出来的 bbox**。

因此必须区分：

```text
1. roi
   配置的识别输入区域（OCR 送到哪里识别 / 模板从哪里裁剪）

2. recognized_bbox
   本次 OCR/template 真实识别出来的目标框

3. action_shape
   实际点击/拖动允许的操作区域
```

例如：

```text
roi = [0,0,1280,720]                 整屏识别
recognized_bbox = [600,300,80,40]    OCR 实际找到「主线」的位置
action_shape = rect [580,280,120,80] 操作区域
```

重叠判断：

```text
recognized_bbox ∩ effective_action_region
```

**不是**：

```text
roi ∩ action_shape
```

### 有效性规则

- 没有显式操作区域：
  - 运行时操作区域 = `recognized_bbox`；
  - 有效；
- 有显式操作区域：
  - `recognized_bbox` 与有效操作区域相交 → 有效；
  - 不相交：
    - `action_source=fixed` → 允许，显示“独立操作区域”；
    - `action_source=recognition` → 无效/醒目警告。

### 反选时的重叠

```text
effective_action_region = 整个屏幕 − action_shape
```

判断：

```text
recognized_bbox 是否有任何部分落在 effective_action_region
```

如果实际识别目标完全在排除区域内，则视为不重叠。

### `action_source`（已确认，§10.2 已落地）

已新增（`§10.2` 确认，JSON/UI 已落地）：

```json
"action_source": "recognition" | "fixed"
```

含义：

- `recognition`：操作区域应跟随真实识别结果；
- `fixed`：用户明确固定操作区域，允许和真实识别 bbox 不重叠。

旧数据迁移（一次性，已完成）：

- 无显式操作区域 → `recognition`；
- 有旧 `action_roi` → `fixed` + `action_shape{kind:rect,invert:false}`；
- 迁移后不再读取旧字段（见 §10）。

### 防错目的

防止：

```text
识别 ROI 很大
→ 识别目标实际移动到别处
→ 旧操作区域仍在原位置
→ 静默点击错误位置
```

### 验收

- 测试结果显示实际 `recognized_bbox`（不是配置 roi）；
- 显示实际 bbox 与有效操作区域的重叠结果；
- `fixed` 独立区域不误报；
- `recognition` 不重叠必须醒目提示无效。


## 7. 素材拖动排序

- 前端素材列表支持拖拽排序；
- 后端新接口：

```text
POST /task/assets/reorder {task, ids:[...]}
```

- 顺序存 `assets.json` 的 items 数组顺序；
- 支持部分列表重排、非法 id 拒绝；
- 保存后刷新当前任务素材；
- 不做跨任务迁移。

## 8. 帧分类重命名

- 接口：

```text
POST /frames/category/rename {from,to}
```

- 把 `from` 分类下所有帧改为 `to`；
- 空分类/未分类处理规则：
  - `from` 不能是空；
  - **已确认：`to` 为空时禁止重命名**，不提供“移动到未分类”的隐式行为；
- 同步更新：
  - 帧分类筛选；
  - 当前帧分类；
  - 帧列表分组；
- 冲突处理：直接合并到目标分类，显示影响帧数。

## 9. 实施顺序（建议）

### Phase A（先修可见问题）

1. 画布 16:9 fit + 结果/日志/提示移出画布；
2. 位置帧详情侧栏；
3. 点测/拖测 humanize 采样修复。

### Phase B（语义修正）

4. 实时帧/位置帧测试来源；
5. 识别/操作区域重叠校验（先显示，不阻断保存）。

### Phase C（选择模型）

6. 抽 `SelectionTool`；
7. 操作区域多边形 + 全屏反选；
8. 删除 `action_roi`，本地数据一次性迁移到 `action_shape` + `action_source`；
9. 回归旧素材（迁移后数据）。

### Phase D（维护操作）

9. 素材拖动排序；
10. 帧分类重命名。

## 10. 字段设计（已确认：方案 B）

### 10.1 操作区域使用 `action_shape`

用户已确认采用 **方案 B：统一 shape 对象**。

新素材结构：

```json
{
  "roi": [100, 100, 300, 200],
  "action_shape": {
    "kind": "polygon",
    "points": [[80,90],[420,90],[420,310],[80,310]],
    "invert": false
  }
}
```

矩形操作区：

```json
{
  "action_shape": {
    "kind": "rect",
    "roi": [80, 90, 340, 220],
    "invert": false
  }
}
```

反选语义：

```text
invert = false  → 有效操作区域 = shape 自身
invert = true   → 有效操作区域 = 整个屏幕 − shape
```

用户明确：

> 反选 = 整个屏幕 − 选择区域。
> 不是“识别外接矩形 − 多边形”。

因此：

- 反选时的采样范围是整个画面（当前 1280x720）；
- 不是外接矩形，不会因 bbox 变小而缩小可点击区域；
- 矩形反选 = 整个屏幕减去矩形；
- 多边形反选 = 整个屏幕减去多边形；
- 选择区域仍然用于显示和命中排除。

### 数据策略：不保留旧兼容

用户明确：**目前没有旧客户端使用人，不需要兼容旧格式**。

因此：

- **删除 `action_roi`**；
- 删除 `roi_back` 旧别名；
- 不再保留“`action_roi` 作为 fallback”的双字段路径；
- 操作区域唯一 canonical 字段：

```json
"action_source": "recognition" | "fixed",
"action_shape": { ... }           // source=fixed 时必填
```

- `action_source=recognition`：不存 `action_shape`，操作区域 = 实际识别 bbox；
- `action_source=fixed`：`action_shape` 必填，操作区域 = shape / 全屏反选；
- 旧 `action_roi` 只在一次性迁移脚本中读取，迁移完成后从 schema/代码/编辑器删除；
- 识别掩膜 `polygon` 保持原义，不参与操作区域反选。

### 为什么删除 action_roi

- 两套字段会产生“shape 与 bbox 不一致”的第二真相；
- 没有外部兼容需求，保留 fallback 只会增加长期维护量；
- 105 处引用可以集中收敛到统一 shape 适配层，一次改完；
- 新保存/运行/批测全部只认 `action_shape` + `action_source`。

### 迁移步骤

1. 加一次性迁移：`action_roi` → `action_shape{kind:rect,invert:false}` + `action_source=fixed`；
2. 无 `action_roi` 的素材/集合 → `action_source=recognition`；
3. 编辑器/保存/runtime/批测/集合全部改用新字段；
4. 删除 `action_roi`、`roi_back`、旧 fallback；
5. 全量测试 + 真实素材回归。



### 10.2 `action_source`：操作区域来源（已确认新增）

`action_source` 是操作区域的来源/语义，**确认写入 `assets.json`**：

```json
"action_source": "recognition"
```

或：

```json
"action_source": "fixed"
```

含义：

- `recognition`：操作区域跟随真实识别 bbox；
  - 不存 `action_shape`；
  - 运行时使用本次 OCR/模板识别出来的 bbox；
  - 识别目标移动，操作区域随之移动。
- `fixed`：用户明确固定操作区域；
  - 必须提供 `action_shape`；
  - 可能和识别内容不重叠（例如识别标题文字，但固定点击下方按钮）；
  - 是独立操作区域。

校验规则：

```text
action_source = recognition:
    真实识别 bbox 与有效操作区域必须重叠

action_source = fixed:
    不要求重叠；显示“独立操作区域”即可
```

注意：这里比较的是 **OCR/模板实际识别出来的 bbox**，不是配置的识别 ROI。

数据迁移：

- 有旧 `action_roi` → 一次性转成
  `action_source=fixed` + `action_shape{kind:rect, invert:false}`；
- 无旧 `action_roi` → `action_source=recognition`；
- 迁移完成后代码/编辑器不再读取 `action_roi`；
- 不保留旧客户端兼容。

## 11. 已确认事项

1. ~~实时关闭时：单素材多位置**全部测试**。~~ **2026-11-04 取代（§16.4）：单素材测试 = 当前选中帧；批量帧测试 = 所有位置帧。**
2. 帧分类重命名：`to` 为空**禁止**。
3. 操作区域字段采用 **方案 B：`action_shape`**。
4. 反选 = **整个屏幕 − 选择区域**，不是外接矩形减去 shape。
5. 有效性比较对象 = **OCR/模板真实识别出的 bbox**，不是配置的识别 ROI。
6. 新增正式字段 `action_source`（`recognition` / `fixed`）。
7. **不保留旧兼容**：删除 `action_roi`/`roi_back`；旧数据一次性迁移。
8. 没有外部用户，可以接受 schema 断代。

## 12. 已确认（原“仍待确认”）

1. 版本号已从 `1.2.3` 重置到 `0.1.0`（`§E-67` 已执行；`oasd --version` = 0.1.0）；旧 v1.x tag 保留为历史。

## 13. 实现前代码证据（2026-11-01 19:30 前快照，历史，勿作现状）

> ⚠ 以下记录是 Phase A~D 实现**之前**的代码状态，仅保留作对照。当前实现状态见 §15。

- 画布：`apps/oasd/ui/index.html` `#canvas-wrap`/`#screen`；`apps/oasd/ui/style.css` flex/max-height；
- 位置详情：当前无独立详情区；
- 选择/多边形：`app.js` `polygon/polyBBox/polyMode`；
- 点测/拖测：`app.js` `testAssetClick`/`testAssetDrag`；
- 帧来源：测试接口当前主要走 `State.frame`，未区分 live/position；
- 素材排序：当前只有列表渲染，无 reorder；
- 帧分类：当前只有 category set/rename 不涉及分类本身；
- 旧 `action_roi`：全仓约 105 处引用，需一次性收口。

## 14. 版本号评估（2026-11-01 19:20 评估记录；已执行 0.1.0）

当前：

```text
workspace.package.version = 1.2.3
tags: v1.0.0 / v1.1.0 / v1.2.0 / v1.2.1 / v1.2.2 / v1.2.3
```

用户判断：软件目前还不能实际使用，按语义化版本应小于 1。

### 建议

重置到：

```text
0.1.0
```

理由：

- 没有外部客户端/兼容责任；
- 当前 API/数据 schema 还在快速断代；
- `0.x` 明确表达“初始开发、随时可能 breaking”；
- 继续用 1.x 会让维护者误以为已经稳定。

### 处理旧 tag

不删除：

- v1.0.0 ~ v1.2.3 保留为历史内部构建；
- 文档标注“v1.x 是 reset 前的内部里程碑，不代表可用版本”；
- 下一次正式发布从 `v0.1.0` 开始；
- `main` 的 v1.2.3 恢复点不强制改历史，只改后续 dev 的 `workspace.package.version`。

### 为什么不是 0.2/0.3

没有 0.x 版本线，`0.1.0` 是最清晰的起点；如果强调“已经迭代很多”，也可以选 `0.3.0`，但那会暗示存在 0.1/0.2 历史，不如 0.1.0 直接。

**已执行（2026-11-01 19:30）：`workspace.package.version = 0.1.0`；`oasd --version` = 0.1.0；旧 v1.x tag 保留（§E-67）。**

## 15. 实现后状态（2026-11-02）

> 本节是 §1~§14 计划/快照的**现行口径**；与旧段落冲突时以本节 + 代码为准。

### 15.1 已实现（行为层）

- 画布 16:9 stage、帧详情侧栏、结果/日志收起（`a804848`/`a2b4b12`）；
- 随机点测/拖测、`action_shape`+`action_source`、操作区多边形/全屏反选、runtime template/OCR/region 主点击 exact shape；
- `live=1` 强制抓帧；**单素材测试 = 当前选中帧、批量帧测试 = 所有位置帧**（2026-11-04 更正，旧“单素材多位置逐个测”已推翻，见 §16）、素材拖动排序、帧分类重命名；
- 点击分布只画点位/热力（无左上指示条）、真实/演练开关（`#btn-test-mode`，默认演练；真实模式 `/input/tap` `/input/swipe` + 二次确认）；
- ResizeObserver 循环修复（`a2b4b12`）：rAF 合并 + 幂等 fit；用户 00:37 验收“没问题”。

### 15.2 未完成/边界（11-02 终态复核）

- ✅ `collection` 集合测试真实点击（O-1，dev `d1a7fcd`）：`GET /task/asset/collection/match` 可选 `real`（严格 `0/1/true/false`）；缺省/`real=0` dry-run 响应与 editor 日志逐字不变；`real=1` 要求 live 帧；匹配通过后在当前帧精确 `action_shape` 内采样（polygon 内 / invert 排除区外，`translate_click_shape` + `ClickShape::sample`）；未过阈值 → **409** + `clicked:false`；成功 → 200 + `real/clicked/click_point/tap_ok`；无 live/越界/无 box → 400，adb 失败 → 500；UI 真实模式 dry 预览 → 二次确认 → 只发一次 `real=1`，409 显示「未执行」。
- ✅ drag 生成的 flow step 带 action shape（O-2，dev `d1a7fcd`）：`from`/`to` 的 region target 可选 `shape`（`rect`/`polygon` + `invert`）；polygon/invert 走与点击同源的 `ClickShape::sample`；**plain rect 操作区也保留**（`from.roi` = action shape bbox，仅 polygon/invert 内联 `shape`）；**无 shape 的旧 flow 行为完全不变**（roi bbox 兜底）。
- ✅ 解析/运行时防护（O-2 评审 P1）：坐标 0..=1e6/i32、rect x/y≥0、x+w/y+h checked、顶点 3..=256、拒绝自交、i128 面积；bbox/contains i64 安全；退化/帧外几何 loud；`shape` 仅 region target（point/ocr_text → FlowError）；`margin` 仅 rect 非反选保留（polygon/invert 记 `margin=ignored`）。
- ⚠ **混版本风险**：旧 runtime 静默忽略 drag `shape`、按 roi bbox 采样（**invert 会点到排除区**）⇒ invert 拖动需本版 runtime。
- ⚠ **P2-10**：保存 flow 不校验解析错误——本轮明确不做；模板移动/多位置仍是「生成时 shape 快照」，Plan B（asset 引用 + 运行时跟随识别）未做。
- ⚠ collection `real=1` **未做真机 E2E**（`adb devices` 为空、MuMu 未运行）；成功路径由 `#[cfg(test)]` tap seam 覆盖。
- `SelectionTool` 未抽出（仍 open）：`polygon/polyMode` 与 `actionPoly/actionPolyMode` 两套状态按语义保留；
- UI dry-run（JS）与 runtime（Rust）是两套采样实现，未共用同一函数（drag 的 `regionDragStep` 与 runtime `ClickShape::sample` 语义对齐，但仍是两份实现）。

### 15.3 00:37~00:38 会话口径（补入文档）

| 项 | 结论 |
|---|---|
| drag/swipe 是否写点击分布 | **不写**。`record_click()` 只在 `perform_click()` 路径调用；`Step::Drag` 走 `input.swipe`，不产生 click 记录。 |
| 拖测 `margin` | 默认 **0.15**，合法范围 **0~0.45**；作用于**起点**（配置了 `to` 时也作用于终点）候选矩形的**内缩**；不是像素、不是拖动距离。**有 drag shape 时仅 rect 非反选生效**；polygon/invert 忽略并在 detail 记 `margin=ignored`。 |
| collection 真实点击 | **00:38 时未实现**；11-02 经两轮评审在 dev `d1a7fcd` 收口（O-1：`real` 严格取值、强制 live 帧、精确 shape 内采样、未过阈值 409；默认 dry-run 不变）；仍缺真机 E2E。 |
| 真实 UI 手势 | 用户 00:37 说“测试过了”；助手独立证据只有真机 `emulator-5554` 的 `POST /input/tap`，不是完整 UI 手势回放。 |
| drag action shape | 11-02 经两轮评审在 dev `d1a7fcd` 收口（O-2）：`shape` 为**可选**字段，无 shape 旧 flow 不变；plain rect 操作区保留（`from.roi`=shape bbox，仅 polygon/invert 内联 shape）；解析校验齐全；`margin` 仅 rect 非反选生效，polygon/invert 记 `margin=ignored`；detail 记 `from_shape/from_invert/to_shape/to_invert`。 |

## 16. 2026-11-04 帧页三项需求 + 素材测试帧错位修复 + 布局回归修复（现行口径）

> 台账：`PENDING-LEDGER.md §E-76`；证据：`D:\OAS-dev\_verify\ui-frames-20261103\`。本节是 §4 / §11 / §15 相关口径的最新版本；与旧段落冲突时以本节 + 代码为准。

### 16.1 图片帧页搜索（需求①）

- `#frame-search`（名称 / `#id` / 分类；大小写不敏感 + `trim`；`12` / `#12` 精确 ID）+ `#frame-search-clear` + `#frame-search-status`；
- **纯前端过滤**：只重绘 `framesCache`，不改 `/frames` 请求；先搜索、再按分类分组，空组不渲染，全空显示「没有匹配的帧」；切分类/刷新保留搜索词；
- 纯函数 `frameMatchesQuery` / `filterFramesByQuery` / `frameSearchStatusText`（`app.js`）。

### 16.2 当前帧素材（需求②）

- 选中具体帧（或刷新当前帧）后，`#frame-detail` 增加「该帧素材：N 条」+ `#frame-assets-list`；
- 数据 = **全局素材库 + 任务素材**中 `positions` 含该帧 id 的全部素材；按 id 去重、来源合并「全局/任务」；空态「该帧没有素材记录」；
- 行样式与全局/任务素材库**同一套** `asset-row`：抽出共享 `assetRowElement` / `renderAssetRows`，三处（`renderGlobalAssets` / `renderAssetList` / 新 `renderFrameAssets`）真实复用；行 DOM 仍由 `OASAssetUI.createRow` 生成；
- 帧详情行与两个素材库共用同一套 `asset-row`，并同样显示「识别」+「设置」两按钮；帧详情行测试传该行帧 id（`f.id`）。~~该视图传 `manage:false`，6 个管理项 disabled~~ **2026-11-04 下午 §18.5 取代：帧详情改为 `manage:true`、删除来源 badge，9 项全部可用；写/测试按行来源选 scope**。

### 16.3 素材测试帧错位（需求③，已修复）

**根因**：实时关闭时 `testFrames(it)` 优先返回素材 `positions`（位置帧），忽略用户选中/画布帧；`positions` 保存必填 ≥1 ⇒ 永远测静态截图，模板在截图里仍在保存位置，识别框必然 = 存储 ROI（`live=1` 每次抓当前屏幕所以准确）。**后端 `/task/asset/match` 正确**：按 `frame_id` 取帧重新匹配，`x/y` = 本次匹配点（实测 `悬赏封印_关闭弹窗`：`frame_id=23` → score 1.0 @ (1208,68)；`frame_id=26` → score 0.9933 @ (1082,75)），不是缓存位置。

**修复**：

- 纯函数 `testFramePlan`，优先级 **live > 显式 frameId > 当前帧 currentFrameId > positions 兜底**；`testFrames(it, frameId)` 薄封装；
- `runAssetTest` 与五个测试函数（match/click/drag/ocr/collection）全部透传 `frameId`；帧详情素材行测试传 `f.id`；
- `selectFrame` 成功切帧后、`loadFrame()` 前清 `boxes/selectedBox`；
- `matchBoxValid`（x/y≥0 且 w/h>0）：miss（x=y=-1）不再画框；
- 单行 OCR：`recognizedBox:null` + `roiBox`，结果文案「识别区域（存储 ROI，无独立检测框）」；**`/ocr` 协议无 bbox ⇒ 单行 OCR 没有真实检测框**（多行/derived 才有 det 框，不变）；
- P1：全局素材库保存/删除/改位置/改名/阈值/ROI/操作区/集合/排序后，`refreshGlobalAssets()` 成功即调 `renderFrameAssetViews()`（用最新缓存本地重绘帧列表 + 当前帧详情/素材列表；0 额外请求、0 递归，不复用 `refreshFrames()` 以免重复 GET `/frames`）；
- P2：`fallbackFullscreenLines` 透传 frameId；删除当前帧清旧框。

### 16.4 现行测试帧口径（取代旧 P1-7 / U-32b 的「单素材多位置」）

| 场景 | 用的帧 | 入口 |
|---|---|---|
| 单素材测试（匹配/点测/拖测/OCR/集合） | **当前选中帧**（帧详情行 = 该行帧） | 素材行按钮 / 帧详情行 |
| 批量帧测试 | **当前可见素材集合**（每素材仍按自己的 `positions`；§18.6 起按钮在素材页） | `POST /task/assets/batch_test`，显式 `assets` ids |
| 实时 = 开 | 实时帧，**恒优先** | 所有测试（真实点击强制 live） |

- **没有「单素材多帧」入口**：旧「单素材多位置逐个测」已推翻；
- `/ocr` 无 bbox ⇒ 单行 OCR 没有真实检测框（见 16.3）；
- `frame_source` 线上仍回 `position:#<id>`（后端把显式 `frame_id` 当位置帧），UI 标签显示「当前帧 #N」；两者会同时出现在结果行（P2 文案待统一）。

### 16.5 遗留（P2/P3，不阻塞）

- 帧搜索无 Enter/防抖（P3）；分类 + 搜索状态计数跨分类（P2）；纯数字查询既精确又子串（P2）；全局模式下来源可能重复计数（P2）；
- `frame_source` 文案（`position:#N` vs「当前帧 #N」）待统一（P2）；切帧只清测试框、不清 ROI 覆盖层（P2）；单行 OCR 点测画布无框（P2 沟通项）；
- 本轮 UI 验收 = headless Edge 截图 + CDP 1440×900 布局实测 + API/纯测试；仍**无真实浏览器自动化 E2E**（P3）。

### 16.6 布局回归修复（2026-11-04）

**用户报告**：图片页「丢失了原先按分类展示图片帧的区域页面」。

**根因（实测）**：上一轮给帧页新增搜索框 / 搜索状态行 / 帧详情 / 该帧素材列表（约 **+296px** 固定内容），而 `#panel-images` 是固定高度 flex 列（`height: calc(100vh - 150px)`），`#panel-images .frames-list` 规则为 `flex: 1; min-height: 0` ⇒ 1440×900 下帧列表计算高度 **0px**：12 个分类组 / 30 行内容仍在（`scrollHeight=1832`）但不可见。

**修复（UI 4 文件：`app.js` / `index.html` / `style.css` / `ui_pure_test.js`）**：

- `style.css`：`#panel-images .frames-list { flex: 1 1 auto; min-height: 200px; overflow-y: auto; }`（帧列表永不为 0）；新增 `#frame-details` / `#frame-details > summary` / `[open]` 样式；
- `index.html`：`#frame-detail` + `#frame-assets-status` + `#frame-assets-list` 包进 `<details id="frame-details" class="frame-details">`，summary「帧详情 / 该帧素材（点击一帧展开）」，**默认折叠**（不再挤占帧列表高度）；
- `app.js`：`selectFrame()` 成功后 `frameDetails.open = true`（点帧即展开该帧详情/素材）；`renderFramesList()` 在搜索词为空时隐藏 `#frame-search-status`（查询时才显示，省一行）；
- `ui_pure_test.js`：**+7 checks**（`<details>` 结构 / summary 文案、`selectFrame` 展开、空查询隐藏状态行、fake DOM `style`/`display` 断言）。

**验证（Lead，CDP 1440×900）**：修复前帧列表 `height=0`（12 组已渲染但不可见）→ 修复后 `height=257px`、12 组可见、details 折叠；点击一帧 #1 → details 打开、详情文本 /「该帧素材 1 条（全局 1 · 任务 0）」/ 1 行素材，帧列表仍 200px 可见。截图 `D:\OAS-dev\_verify\ui-frames-20261103\frames-fixed.png`（分组列表恢复）、`frames-fixed-detail.png`（点帧展开素材）。`node --check` 两文件 exit 0；`git diff --check` clean；无 Rust 改动 ⇒ workspace 仍 **501/0**；UI **835 checks / 225 ids / 0 dead**。

## 17. 2026-11-04 素材行「识别 + 设置」两按钮（同轮，现行口径）

> 台账：`PENDING-LEDGER.md §E-77`；证据：`D:\OAS-dev\_verify\asset-row-20261104\`（`ASSET-ROW-REPORT.md` · `REVIEW.md` · `FIX-REPORT.md` · 截图 · 隔离脚本测试）。本节取代「素材行一排旧按钮（测试/点测/拖测/识别/区域/改/帧/删/操作区/步骤）」的旧描述（U-30b/U-44/R19 时代的布局仅作历史）。
>
> **2026-11-04 下午更新（§18）**：两按钮 + 9 项菜单结构继续有效；但本节里「识别区 = prompt 输入坐标」「template 识别区 disabled」「帧详情 `manage:false` 只读」「阈值仅模板」等实现细节已被 **§18** 取代（现为画布拖拽 + 共享保存按钮、模板 replace 重截、帧详情全可用、OCR 也可设阈值）。

### 17.1 行结构

- 任务素材库、全局素材库、帧详情素材列表三处**所有普通素材行统一 2 个操作按钮**：**「识别」+「设置」**；集合行（`template_set`）同样两按钮。
- 「识别」= 旧 `onTest`（集合行走 `onCollectionTest || onTest`）；title 按 kind 说明（模板匹配 / OCR 试识别 / region 选中操作区域）。
- 「设置」在**行内展开** 9 项菜单（`.asset-row-menu`，`flex:1 1 100%`），不是绝对定位弹层——`.asset-list` 是 `overflow:auto`，弹层会在列表底部被裁掉。展开时行变高、列表自身滚动。
- 再次点「设置」收起；点任一可用项先收起再执行；菜单/按钮聚焦时 `Esc` 收起。`actions:false`（builder 列表）同时隐藏「识别 + 设置」；`compact` 仍隐藏点测/拖测/步骤三项。
- 菜单是 `asset_ui.js` 的 `ASSET_MENU_ITEMS` 常量 + `assetMenuState()` 纯函数；disabled 项**保留可见**并用 title 解释原因（视觉统一，9 项永远同序）。

### 17.2 九项顺序与映射（强约束，测试断言）

顺序严格为：**点测 / 拖测 / 识别区 / 操作区 / 重命名 / 阈值 / 位置帧 / 删除 / 步骤**。

| # | 菜单 | opts 回调 | 行为 |
|---|---|---|---|
| 1 | 点测 | `onClickTest` | 点击测试（dry-run，显示目标点）；帧详情行传该行帧 id |
| 2 | 拖测 | `onDragTest` | 拖动测试（dry-run，方向/起终点） |
| 3 | 识别区 | `onEditRoi` → 新 `editAssetRecognitionRoi` | **§18.2 起 = 画布拖拽**（旧 `prompt` 输入已删除）；保存按钮在共享 `#canvas-footer` |
| 4 | 操作区 | `onAction` | 进入画布操作区编辑（沿用旧 `editAssetActionRoi`） |
| 5 | 重命名 | `onRename` → 新 `renameAsset` | `prompt` 新 id；只改名，不动阈值/ROI |
| 6 | 阈值 | `onThreshold` → 新 `editAssetThreshold` | 模板 NCC 0~1；**§18.4 起 OCR 也用此菜单**设最低置信度（不设/0 = 不限制） |
| 7 | 位置帧 | `onPosition` | 沿用 `editAssetPositions`（搜索 + 候选弹窗） |
| 8 | 删除 | `onDelete`（集合走 `onCollectionDelete`） | 沿用删除流程 |
| 9 | 步骤 | `onStep` | 生成 `click_target` 步骤 |

- 旧 `editAsset` 三合一（重命名 + 阈值 + ROI）与 `renderAssetList`/`renderGlobalAssets` 的两处 `onEdit` 死代码**已删除**；全仓库无 `onEdit` / `function editAsset(`（测试静态断言）。
- 三个新函数成功后统一 `await refreshTaskAssetsForScope(task); await refreshGlobalAssets();`，帧详情/帧列表经既有 `renderFrameAssetViews()` 跟随刷新。

### 17.3 禁用规则

| 场景 | 禁用项 | 说明 |
|---|---|---|
| 非 `template` | 阈值 | ~~只有模板图有阈值~~ **§18.4 取代：`ocr_line`/`ocr_lines` 也可设 OCR 最低置信度；只有 region / 集合禁用** |
| `template` | ~~**识别区**~~ | **§18.3 起启用**：同一画布手势，保存走 `/task/save_asset` + `replace:true` 用当前帧重新截图覆盖（rev+1）；旧「服务端拒绝、菜单 disabled」已删除 |
| `region` | 操作区 | region 素材本身就是操作区域 |
| `manage:false`（帧详情） | ~~识别区 / 操作区 / 重命名 / 阈值 / 位置帧 / 删除~~ | **§18.5 起帧详情 `manage:true`：9 项全部可用**；每行按来源选 API scope（含 global → 全局库；仅 task → 当前任务） |
| `template_set`（集合行） | 其余 8 项 | 仅「删除」可用（`manage:true` 时） |
| `compact` | 点测 / 拖测 / 步骤 | 不渲染（不是 disabled） |

- 规则优先级（§18 现行）：`template_set` → `manage:false`（目前无 surface 传）→ `region 操作区` → 阈值 kind（template / OCR 可设，region/集合禁用）。
- disabled 项**点击不触发回调**（测试断言 `onEditRoi` 零调用）。
- 旧 `editAssetRoi` 的 prompt 版与 template/template_set 硬 guard **已随 §18.2 删除**；现在保存入口是画布 + `saveAssetRecognitionRoi`，模板走 `/task/save_asset` + `replace:true`（不是 ROI PATCH）。
- ROI 校验（§18.2）：画布框 `w,h>0`、`x,y>=0`、`x+w`/`y+h` 不超过当前画布帧尺寸；旧 `parseAssetRoiText`（4 个十进制整数、100000 cap）已随 prompt 一起删除。

### 17.4 窄侧栏（320px）

- 根因：`.asset-main` 作为 flex item 默认 `min-width:auto`，在 320px 侧栏不肯收缩 ⇒「设置」被挤到第二行、两个按钮被拉成高块。
- 修复：`.asset-row .asset-main { min-width: 0 }` + `.asset-row .asset-test { align-self: flex-start }`（行仍 stretch，保证菜单独占第二行）；`.asset-main-text { min-width: 0 }`。
- headless Edge 实测（真实 `style.css` + `asset_ui.js`，360×900 画布 / stage 320px）：`row=320 / main=208 / 识别=52x26 / 设置=52x26 / sameRow=true / wrap=false`（菜单开合同样成立）；截图 `_verify/asset-row-20261104/asset-row-320.png`。

### 17.5 验证

- UI pure test：**835 → 907 checks（+72，旧断言不弱化）/ 225 referenced ids / 0 dead ids**；新增两按钮、9 项顺序、5 类禁用规则、菜单开合/Esc/执行后收起、disabled 零回调、三处 surface 回调、新函数端点与刷新、ROI/阈值解析边界、`editAsset`/`onEdit` 已删等断言。
- `node --check asset_ui.js/app.js/ui_pure_test.js` **3/3 exit 0**；`git diff --check` clean；task lint **1/0**；无 Rust 改动 ⇒ `cargo test --workspace` 仍 **501/0**（oasd 228）。
- CDP 实测（seq 98）：全局素材行 `topButtons=[识别,设置]`；模板行 9 项中仅「识别区」disabled；OCR 行「阈值」disabled；帧详情行（`manage:false`）6 个管理项 disabled。**以上禁用口径已被 §18 取代**（模板识别区启用、OCR 阈值启用、帧详情全可用）；seq 99 新实测见 §18.8。

### 17.6 遗留（P2/P3）

- ~~ROI 编辑用 `prompt` … 未做画布框选入口~~ **已由 §18.2 完成**（画布拖拽 + 共享保存按钮 + 帧边界校验）；旧 `100000` sanity cap 与 `parseAssetRoiText` 已删除。
- 设置菜单的全局互斥/Esc 焦点细节/aria 完善（P3，仍未做）；本轮仍用 CDP headless 验证，**无真实浏览器自动化 E2E**。

## 18. 2026-11-04 下午 素材库 UX 六项 + OCR 阈值（现行口径）

> 台账：`PENDING-LEDGER.md §E-78`；证据：`D:\OAS-dev\_verify\asset-ux-20261104\`（`UX-REPORT.md` · `OCR-THRESHOLD-REPORT.md` · `REVIEW.md` · `FIX-REPORT.md` · `THRESH-FIX-REPORT.md` · `ux-selected.png` / `ux-roi-shared.png` / `ux-frame.png` · `probe-*.js` · `*.log`）。
> 本节是素材库 UX 的最新版本；§17 的「两按钮 + 9 项菜单」结构仍是现行，但 §17 里的识别区 prompt / 模板识别区 disabled / 帧详情 `manage:false` / 阈值仅模板 / 批量测试在任务页均已被本节取代。冲突时以本节 + 代码为准。

### 18.1 行选中态与设置菜单保持（需求①）

- **选中按 asset id 记在模块状态**（`asset_ui.js` `selectedAssetKey`，不是 DOM）：同一 asset id 在任务库 / 全局库 / 帧详情三个 surface 同时高亮；列表重渲染（保存/刷新/删除）后 `createRow` 按状态补回 `.selected`。
- 样式 `.asset-row.selected`：背景 `#1b2a3a`（CDP 实测 **`rgb(27, 42, 58)`**）+ 左亮条 + 主文本/设置按钮提亮；无布局位移（inset shadow）。
- 点击行主体（main）/「识别」/「设置」都选中该行；**disabled 菜单项不改变选中、不触发回调**。
- **菜单最多一个**；点菜单项**不收起**（同一素材的设置面板保持展开），仅当选中**另一个素材**时关闭旧菜单并把高亮迁过去；再次点「设置」或 `Esc` 收起。
- 列表重渲染后恢复已选行菜单（`openMenuKey` + 延迟一 tick）；先在当前可见 surface 上恢复，极端多列表同时可见时可能开在最后一个可见 surface——**只影响菜单位置，不丢选中**（遗留⑥）。

### 18.2 识别区 = 画布拖拽 + 共享保存按钮（需求②）

- 菜单「识别区」→ `editAssetRecognitionRoi(it, scopeTask?)`：把原 ROI 载入画布、`dragTarget='roi'`、清掉 action/polygon 模式；进入编辑时若实时预览开着会**自动暂停并提示**（否则画布拖拽会被当成设备 tap/swipe）。
- **不再弹窗输入坐标**：旧 `editAssetRoi`（prompt `x,y,w,h`）与 `parseAssetRoiText` 已删除，无死代码。
- 保存按钮从 `panel-assets` 移到**共享 `#canvas-footer`**（`index.html:64-69` / `style.css:571-589`）：任意面板（任务素材库 / 全局素材库 / 帧详情）都能看到并点击；按钮文案「保存识别区（覆盖素材）」，默认 hidden+disabled，进入编辑才显示。
- 旁边新增 `#asset-roi-edit-label`：`识别区编辑中：<素材id>`；切到别的素材或取消后按钮隐藏、标签清空。
- `cancelRecognitionRoiEdit(reason)` 是**唯一退出入口**（P0-2）：`pickAssetRoi` / 切换识别区或操作区编辑 / 重命名 / 阈值 / 位置帧 / 删除 / 集合动作 / 点测拖测 / 生成步骤 / 测试 / 切帧 / 删帧 / 清空帧 / 实时开关 / Esc / 清除区域 / 切任务 / 画布模式切换都会调用；清编辑目标 + 画布 ROI + 按钮状态，并写结果/日志说明原因。
- 保存校验：`w,h>0`、`x,y>=0`、`x+w`/`y+h` **不超过当前画布帧尺寸**（P2-9），失败 `alert` 且不发请求；保存前按 `{id, scope}` 从缓存重查**最新**素材（P1-2），找不到则取消并提示。

### 18.3 模板识别区统一：replace = 用当前帧重新截图覆盖（需求③）

- **概念**：模板 ROI 本质是「裁剪 / 搜索几何」——模板图 = 保存时按该 ROI 裁出来的图；单独改 ROI 会让模板图过期，所以服务端 `update_asset_meta` **拒绝**模板 ROI 更新（旧提示「模板没有识别区」）。
- **统一交互**：模板与 OCR/region 都用同一画布拖拽手势；保存分两条：
  - `template` → `POST /task/save_asset` + **`replace:true`**：用**当前帧**按新 ROI 重新截图并覆盖模板图，`rev+1`；confirm 文案含素材 id、`rev+1`、位置帧数量、不可撤销；前端先拦截 `positions` 为空（P2-8），服务端仍要求 ≥1；
  - `ocr_line` / `ocr_lines` / `region` → `POST /task/asset/update {task,id,roi}` 直接更新 ROI。
- **payload** `assetRoiReplacePayload(task, latest, roi)`（P1-1/P1-2）：以**最新素材对象**为底（不是打开编辑时的快照），带 `id/kind/threshold/positions/action_source/action_shape/polygon/ocr_mode/ocr_source_roi/ocr_text`（存在才带），只替换 `roi` 并加 `replace:true`；`action_source=recognition` 不带 `action_shape`、`fixed` 必带（服务端契约）。
- **polygon 规则**：旧 polygon 的 bbox 与新 ROI 一致则原样保留；不一致则回传**新 ROI 矩形**（避免服务端用旧 bbox 反推、把识别/点击掩膜覆盖掉）。
- 菜单 title 已说明「模板：保存会用当前帧重新截图覆盖，rev+1」；模板识别区**不再 disabled**。
- 遗留：`region` 识别区保存仍会把 polygon/invert 降级为矩形（P2-5）；模板 ROI 移动后 polygon **有意**重同步为矩形（不是 bug），见 §18.9。

### 18.4 OCR 阈值（需求④）

- 素材（`ocr_line` / `ocr_lines` / derived）复用 `assets.json` 的 **`threshold`** 字段，语义 = **最低识别置信度**；模板的 `threshold` 仍是 NCC 匹配线。二者在 UI 同一个「阈值」菜单里按 kind 分支。
- **未设置或 0 = 不按置信度过滤**（与旧行为完全一致）；显式 `t>0` 时 `score<t` 视为未命中/失败。**0 与未设置在 Rust 侧等价**（`ocr_threshold` 读取时过滤 `>0`，P1-4）。
- UI：模板未设时预填/显示默认 **0.85**；OCR 未设或 0 显示「未设或 0（不限制）」、输入框默认空；prompt 明确「留空 = 取消；0 = 不限制」。阈值菜单对 `template` / `ocr_line` / `ocr_lines` 启用，`region` / 集合禁用。
- 生效路径、取值路径不加阈值、`ocr_lines` 最高行分等完整语义见 `ENGINE-DESIGN.md §16.5` / `§22.42`。

### 18.5 帧详情素材库与全局库一致（需求⑤）

- 删除「来源（全局 / 任务）」**列 / badge** 与 `assetSourceLabel()`；帧详情行传 **`manage:true`**，行 DOM / 样式 / 9 项设置菜单与设置页的全局库完全一致（不再灰、不再只读）。
- **行作用域按来源**（`frameAssetScope(row, currentTaskName)`）：含 `global` → `GLOBAL_SCOPE_TASK`（**global 优先**，即使同时被 task 引用）；仅 `task` → 当前任务名。所有管理 / 测试回调支持可选 `scopeTask`；不传时两库原行为不变。
- 计数行保留「该帧素材 N 条（全局 X · 任务 Y）」；空态「该帧没有素材记录」。来源 badge 删除后，该计数行是唯一来源统计。
- 遗留：帧详情里 task-only 行在**没有选中任务**时仍会报「请先填任务名」（P2-10）。

### 18.6 批量帧测试移页 + 可见素材集合（需求⑥）

- `#btn-assets-batch-test` 从**任务页**移到**素材页**，紧邻「刷新全局库」（`index.html:143`，上一兄弟 = 刷新按钮；任务页已无此 id）。
- 测试对象改为**当前可见素材集合**（P1-3）：
  - 全局模式 = 过滤后的 `globalAssetsCache`（应用搜索/分类，排除 `template_set`）；
  - 任务模式 = 任务可见素材（`show-all` 已由列表应用）；
  - 显式把素材 id 列表放进 `POST /task/assets/batch_test` 的 `assets`（后端 `handle_assets_batch_test` 接受该过滤），不再固定为「当前任务 flow 引用子集」。
- 空集合文案按 surface 区分；全局模式 `task = GLOBAL_SCOPE_TASK`；结果首行写「对象：<surface>可见 N 个」。
- 遗留：legacy 非全局模式仍需任务名（没有任务上下文时报错，与旧行为一致）。

### 18.7 评审 P0/P1/P2 修复（REVIEW.md）

| 级别 | 问题 | 修复 |
|---|---|---|
| P0-1 | 保存按钮只在 `panel-assets` 内，任务库 / 帧详情进入编辑后不可见 | 移到共享 `#canvas-footer` + `#asset-roi-edit-label`；三 surface 可用（`index.html:64-69` / `style.css:571-589`） |
| P0-2 | 切素材 / 切帧后仍写旧素材（模板 replace 会数据损坏） | 新增 `cancelRecognitionRoiEdit(reason)` 覆盖所有离开路径；保存前按 `{id,scope}` 从缓存重查最新素材，找不到即取消 |
| P1-1 | 模板 replace payload 丢 `polygon` / OCR 字段 | `assetRoiReplacePayload` 以最新素材为底带全字段；polygon bbox 一致保留、否则回传新 ROI 矩形 |
| P1-2 | 模板保存用编辑起点快照，可回退阈值 / 位置帧 | 同 P0-2：保存前重查最新对象，只换 ROI |
| P1-3 | 批量测试移到素材页但仍测 task flow 子集 | 改为当前可见集合 + 显式 `assets` ids |
| P1-4 | OCR `0` 与未设在 `expect appear/vanish` 不等价 | Rust 归一化：`ocr_threshold` 过滤 `>0`；`selector_threshold` 统一 appear/vanish；`click_target` 模板兜底同策略；其余路径走归一化访问器 |
| P2 | Esc / 切帧 / 实时开关不取消编辑；实时开时进入编辑未暂停；重渲染丢菜单；模板空 positions；ROI 越界；confirm 信息不足；title 未说明重截 | 全部已修（Esc/切帧/实时取消、实时自动暂停并提示、菜单按 id 恢复、positions 前端拦截、帧尺寸校验、confirm 含 id/rev/位置帧数/不可撤销、菜单 title 说明重截） |

### 18.8 测试与验证

- UI pure test：**907 → 1003（实现）→ 1112 checks / 227 referenced ids / 0 dead ids**（修复后；旧断言不弱化）；`node --check` 3/3 exit 0。
- Rust：workspace **501 → 512（OCR 阈值）→ 516 passed / 0 failed**；runtime **133→136** unit、task **58→59**、oasd **228**；新增 **15** 条（阈值 11 + 0 语义/expect 三态 4，原 11 条保留）。
- `cargo build -p oasd --features ocr` exit 0（6 个既有 warnings）；task lint **1/0**；`git diff --check` clean。
- **Lead CDP 实测**（真实 serve 8787 + device）：行选中背景 `rgb(27,42,58)`；设置→识别区后菜单保持展开、画布底部保存按钮可见可用、标签「识别区编辑中：预设」；切到另一素材后保存按钮隐藏/禁用、标签清空；模板识别区 title 含「模板：保存会用当前帧重新截图覆盖，rev+1」；OCR 阈值 prompt 显示「未设或 0（不限制）」且默认空；帧详情行无来源文案、9 项设置全 enabled；批量按钮在 `panel-assets`、上一兄弟「刷新全局库」、不在 `panel-task`。截图 `_verify/asset-ux-20261104/ux-roi-shared.png` / `ux-selected.png` / `ux-frame.png` 等。
- 边界：**无真实浏览器自动化回归**（本轮 = Lead CDP 实测 + UI pure tests）。

### 18.9 遗留（P2/P3，不阻塞）

1. `region` 识别区保存仍会把 polygon/invert 降级为矩形（P2-5，服务端 region ROI 保存路径既有）；
2. 模板 ROI 移动后 polygon **有意**重同步为矩形（避免旧 bbox 反推覆盖；不是回归）；
3. `ocr()` / `ocr_digit()` / `ocr_to_var` 等**取值路径不加阈值**（设计：避免把「读到的值」变成「没读到」）；`ocr_lines` 判定用**最高行分**，适配层已丢弃 `<0.5` 的行 ⇒ `<0.5` 阈值对 `ocr_lines`/derived 无额外效果；
4. legacy 非全局模式的批量测试仍需任务名；
5. 帧详情 task-only 且无任务时仍报错（P2-10）；
6. 菜单恢复在极端多列表同时可见时可能开在最后可见 surface（不丢选中）；
7. 无真实浏览器自动化回归（本轮 Lead 用 CDP 实测 + UI pure tests）；
8. docs 已同步本轮（本任务）。
