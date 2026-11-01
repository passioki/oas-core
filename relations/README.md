# relations/ — 帧-素材关系证据

本目录只保存**不能从现有数据推导**的关系证据；`Asset.positions` 仍是唯一
asset↔frame 关系。

## observations.jsonl

追加式 JSONL，每行一条证据。当前只支持 `transition`：

```json
{
  "schema_version": 1,
  "id": "obs-...",
  "type": "transition",
  "from_frame": 4,
  "to_frame": 6,
  "task": "返回庭院",
  "asset": "预设",
  "source": "recorded",
  "created_at": 1793500000000
}
```

写入方式：

```text
图片页 → 录制关系 → 选择动作素材 → 录制一次
```

后端会：

1. 抓前帧并入库；
2. 点击素材的动作区域；
3. 等待；
4. 抓后帧并入库；
5. 追加一条 observation。

约束：

- 不手写重复关系；派生关系（帧→素材、素材共现、帧共享素材）由
  `/relations/frame-assets` 实时计算；
- 同一素材/帧的重复录制可以保留多条证据，前端只展示记录；
- 删除帧/素材不会自动删证据；证据是历史观察，不应当作当前真相。

## annotations.json

人工标注，保存 `alternative` / `requires` / `similar` / `overlay`：

```json
{
  "schema_version": 1,
  "annotations": [
    {
      "id": "ann-...",
      "type": "alternative",
      "a": "asset:返回庭院/预设",
      "b": "asset:返回庭院/确认",
      "note": "功能等价",
      "created_at": 1793500000000000000
    }
  ]
}
```

节点 ID：

```text
frame:<frame_id>
asset:<task>/<asset_id>      任务素材
asset:global/<asset_id>      全局素材库
```

API：

```text
POST /relations/annotate
POST /relations/annotate/delete
GET  /relations/frame-assets?task=<task>   （返回派生关系 + transitions + annotations + coverage）
```

规则：

- `alternative`：两个素材或两个帧；
- `similar` / `overlay`：两个帧；
- `requires`：任意两个已存在节点；
- 不允许自反；重复标注会被拒绝；
- 标注删除通过 annotation id。

## 覆盖概览

`GET /relations/frame-assets` 返回：

```json
"coverage": {
  "frames_total": 22,
  "frames_with_assets": 5,
  "assets_total": 1,
  "assets_with_positions": 1,
  "transitions": 0,
  "annotations": 0
}
```

用于判断哪些帧/素材还没有进入关系网络。

## 已知缺失证据

- `observations.jsonl` 里曾记录 `31→32`、`33→34` 两条 transition；
- 对应 PNG 在 2026-11-01 的一次误清空中丢失，无法恢复；
- 帧 ID 31~34 不复用（frames.json `next_id=35`），重新录制会生成新帧；
- 缺失帧的 transition 仅作为历史证据保留，不应视为可执行路径。

## 位置帧继承

- 新素材保存区、已有素材位置编辑弹窗都可以从其他素材继承位置帧；
- 继承是**复制 positions 并追加**，最终保存到目标素材自己的 `positions`；
- 不新增 asset→asset 引用关系，也不会在源素材后续变化时自动同步。


