# 编辑器 UX / 选择模型改版计划（2026-11-01）

> 状态：**Phase A/C/D 首轮已实现；Phase B 单测已实现，runtime polygon 精确采样与 live 批量待补**。版本已重置 `0.1.0`。
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
- 旧字段 `action_roi` 保留读取兼容：
  - 读取时若无 `action_shape`，转换为 `kind=rect, invert=false`；
  - 新保存以 `action_shape` 为主；
  - `action_roi` 可作为旧客户端 fallback，不作为新数据唯一真相。

### 运行时点击采样

- 非反选：
  - 在 shape 内部随机采样；
- 反选：
  - 在整个画面内随机采样；
  - 拒绝落在 shape 内部的点；
  - 保证最终点有效；
- 多边形使用点在多边形内测试；
- 旧 `action_roi` 无 shape 时仍按矩形处理。

### 验收

- 画一个区域并反选后，保存能读回 `action_shape.invert=true`；
- dry-run 目标点落在“整个屏幕减去选择区域”内；
- 旧素材没有新字段仍可加载/运行；
- 反选区域不会因为外接矩形 bbox 而错误缩小。

## 4. 实时帧 / 位置帧测试语义

### 目标

每个测试请求显式带帧来源：

```text
live=1        使用当前实时帧（每次测试前强制抓新帧）
live=0/false  使用素材位置帧
```

规则：

- 实时开启：
  - 识别、点测、拖测、模板匹配、OCR 都使用**实时帧**；
  - 不允许使用旧缓存帧；
  - 结果记录 `frame_source=live` 与 `frame_seq`。
- 实时关闭：
  - 素材测试使用**位置帧**；
  - **已确认：单素材多位置时逐个位置帧测试并分别返回结果**；
  - 结果记录 `frame_source=position`、`frame_id`、位置序号。
- 页面/日志显示明确标签：

```text
[实时帧] #live
[位置帧] #4 我的式神
```

### 验收

- 实时开/关各跑一次，同一素材结果来源不同；
- 实时关闭时不再隐式用当前选中帧代替位置帧；
- 多位置素材能列出每帧通过/失败。

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
- dry-run 和真实 runtime 使用同一采样实现。

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

### `action_source`（待确认）

拟新增：

```json
"action_source": "recognition" | "fixed"
```

含义：

- `recognition`：操作区域应跟随真实识别结果；
- `fixed`：用户明确固定操作区域，允许和真实识别 bbox 不重叠。

旧数据：

- 无显式操作区域 → `recognition`；
- 有显式操作区域但无 `action_source` → 先按 `fixed` 兼容，并标“旧数据来源未知”。

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

1. 实时关闭时：单素材多位置**全部测试**。
2. 帧分类重命名：`to` 为空**禁止**。
3. 操作区域字段采用 **方案 B：`action_shape`**。
4. 反选 = **整个屏幕 − 选择区域**，不是外接矩形减去 shape。
5. 有效性比较对象 = **OCR/模板真实识别出的 bbox**，不是配置的识别 ROI。
6. 新增正式字段 `action_source`（`recognition` / `fixed`）。
7. **不保留旧兼容**：删除 `action_roi`/`roi_back`；旧数据一次性迁移。
8. 没有外部用户，可以接受 schema 断代。

## 12. 仍待确认

1. 是否把版本号从 `1.2.3` 重置到 `<1` 的 `0.1.0`（需决定是否保留旧 v1.x tag 作为历史）。

## 13. 当前代码证据

- 画布：`apps/oasd/ui/index.html` `#canvas-wrap`/`#screen`；`apps/oasd/ui/style.css` flex/max-height；
- 位置详情：当前无独立详情区；
- 选择/多边形：`app.js` `polygon/polyBBox/polyMode`；
- 点测/拖测：`app.js` `testAssetClick`/`testAssetDrag`；
- 帧来源：测试接口当前主要走 `State.frame`，未区分 live/position；
- 素材排序：当前只有列表渲染，无 reorder；
- 帧分类：当前只有 category set/rename 不涉及分类本身；
- 旧 `action_roi`：全仓约 105 处引用，需一次性收口。

## 14. 版本号评估

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

**待确认：是否重置到 0.1.0。**


