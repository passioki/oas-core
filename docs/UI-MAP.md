# UI-MAP — 全局界面关系图（设计稿）

> 状态：**设计已确认，尚未实现**（2026-11-01）。
> 本文件是全局设计，不属于某个任务的私有文档。

## 1. 目标

把游戏里的界面、弹窗、加载页以及它们之间的进入/返回/关闭关系，建成一张**全局有向标签图**：

```text
界面节点   ← 怎么识别
导航边     ← 从哪个界面做什么动作到哪个界面
```

用途：

- 人类可读：Mermaid / Obsidian / 表格；
- 机器可执行：生成 recovery flow；
- 可校验：所有界面可达目标界面，页面不漏；
- 可维护：新增页面只加节点/边，不复制一堆 flow 步骤。

这不是神经网络；Obsidian 只是可视化形式。

## 2. 数据文件与唯一真相

```text
ui-map/ui-map.json       全局唯一数据（git 跟踪）
docs/UI-MAP.md           本设计文档 + 生成的人类可读索引
frames/frames.json       帧元数据；Frame.interface_id 指向界面
assets/assets.json       素材；Asset.positions 指向帧
```

关键关系：

```text
Asset.positions
    │
    ▼
Frame
    │ interface_id
    ▼
Interface
    │ transitions
    ▼
Interface
```

因此：

```text
素材出现在哪些帧
帧属于哪个界面
⇒ 素材与哪些界面相关
```

**不得**在 `AssetDecl` 再存第二份 `interface_id`；素材所属界面由上述关系推导。

## 3. 节点模型

```json
{
  "id": "courtyard",
  "type": "screen",
  "title": "庭院",
  "markers": [
    { "kind": "ocr_text", "text": "町中" },
    { "kind": "ocr_text", "text": "召唤" }
  ],
  "match": "any",
  "actions": {
    "default_back": { "kind": "region", "roi": [5, 5, 72, 72] }
  }
}
```

字段：

- `id`：稳定唯一 ID（英文/拼音）；
- `type`：`screen` / `popup` / `loading` / `startup` / `unknown`；
- `title`：人类可读名称；
- `markers`：OCR 识别标记；尽量使用文字，不用易变背景图；
- `match`：`any` / `all`；
- `actions`：该界面可用的操作定义，供边引用；
- 参考帧不重复存：由 `frames[].interface_id` 聚合得到。

### 一帧一界面

第一版规则：

- 一个 Frame 只属于一个 Interface；
- 弹窗单独做节点；
- 需要“同一帧多个界面标签”时再增加 `interface_tags`，不要现在引入多归属复杂度。

现有帧按 `category` 迁移到界面；无法映射的归 `unknown`。

## 4. 边模型

```json
{
  "from": "settings",
  "to": "courtyard",
  "label": "点击右上角关闭 X",
  "action": "close",
  "priority": 10
}
```

动作类型：

- `ocr_click`：OCR 文字点击；
- `region`：区域随机点；
- `key`：Android keycode；
- `restart_app`：重启包；
- `wait`：等待。

所有区域点击使用随机点，不用固定单点。

## 5. 全局兜底

```json
{
  "goal": "courtyard",
  "unknown": {
    "action": "default_back",
    "on_fail": "restart"
  },
  "restart": {
    "kind": "restart_app",
    "package": "com.netease.onmyoji"
  }
}
```

规则：

- 未知界面：先尝试通用返回区域；
- 多次失败：重启游戏；
- 重启后：识别启动/登录界面并点击已知的进入按钮；
- 仍失败：任务 `finish failed`，不无限乱点。

## 6. 已知界面初表

| ID | 名称 | 类型 | OCR 标记 |
|---|---|---|---|
| `courtyard` | 庭院 | screen | 町中 / 召唤 |
| `settings` | 设置 | screen | 设置 |
| `shikigami` | 式神录 | screen | 式神录 |
| `exploration` | 探索/町中地图 | screen | 探索 + 地图特征 |
| `exit_confirm` | 退出确认 | popup | 确定退出阴阳师 |
| `startup` | 启动/登录 | startup | 点击屏幕 / 进入游戏 / 开始游戏 |
| `unknown` | 未知界面 | unknown | — |

后续按需增加御魂、觉醒、结界突破、召唤、商店、好友、活动页等。

## 7. 校验规则

- 节点 ID 唯一；
- 每个非 unknown 节点至少一个 OCR marker；
- 边引用的 from/to 存在；
- 边引用的 action 存在；
- 每个非目标节点有出边；
- 所有节点可达 `goal`；
- ROI 在 1280x720 设计分辨率内；
- 不引用不存在的帧；
- 未知界面必须有兜底；
- 重启包名非空。

## 8. 与 flow 的关系

第一阶段：

```text
ui-map.json → 生成返回庭院 flow
```

人改图，flow 重新生成；runtime 不需要立刻支持动态图执行器。

第二阶段：

- 图稳定后，再考虑 runtime 图执行器；
- flow 仍保留可审计日志。

## 9. 前端计划

- `builder.js` / `flow_model.js` 补齐 `key`、`restart_app`；
- 任务构建增加“界面关系图”只读视图：
  - 节点；
  - 边；
  - OCR 标记；
  - 参考帧数量/缩略图；
  - 操作区域；
- 后续支持编辑节点/边。

## 10. 实现状态

- [x] 设计方向确认（全局界面关系图、位置帧统一、JSON 唯一真相）；
- [ ] `ui-map/ui-map.json` 数据文件；
- [ ] `Frame.interface_id` 与迁移；
- [ ] 全局已知界面节点/边；
- [ ] 校验器；
- [ ] Mermaid 生成；
- [ ] flow 生成器；
- [ ] 前端图视图；
- [ ] 截图/素材归档。
