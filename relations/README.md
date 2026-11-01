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
