# 恢复审计：session ↔ git ↔ 代码（2026-11-01）

> 目的：解释“仓库被搞乱”到底从哪一步开始，区分**用户要求的新改动**与**助手引入的改动**。
> 依据：完整 DSH transcript `_verify/n44/session-current-both-full.txt`、git 提交时间、当前代码/数据。

## 1. 时间线（事实）

| 时间 | 事件 | 结论 |
|---|---|---|
| 16:21:33 | `f6be0f0` feat(assets): inherit position frames from another asset | **用户所说的 4:21 干净版本**；数据为 22 帧 / `返回庭院` 3 素材 / 无全局 `assets/` |
| 18:40:38 | 用户提出 7 项编辑器问题 | 只进入讨论，未改代码 |
| 18:42:19 | `f157dae` docs plan | docs-only |
| 18:57:30 | `2b22b77` docs clarify | docs-only |
| 19:05:23 | `7e16063` docs action shape B | docs-only |
| 19:14:09 | `8ee414c` docs version/field | docs-only |
| 19:15:29 | 用户：“好的，开始吧” | 正式开始实现 |
| 19:30:51 | `9618cb5` feat(editor) | **第一笔代码+数据提交**；包含 7 项实现、action_shape 迁移、版本 0.1.0；同时带入用户当时工作区的 28 帧 / 11 素材 / flow 删除 |
| 19:52:56 | `c557ff1` runtime exact click | 实现延伸，符合需求 |
| 20:01:46 | `33ad968` runtime region/live batch | 实现延伸，符合需求 |
| 20:10 / 20:21 | 用户指出任务/全局素材库位置与层级 | `1a021f8` 调整 UI |
| 20:23:49 | `0bc69ba` recover 11 task assets globally | **超出用户要求的恢复动作** |
| 20:33:09 | `1f73f17` recover 34 historical assets | **错误：把 task 历史拼成全局库，不是 archive 全量** |

## 2. 关键边界

```
f6be0f0
  = 16:21 干净代码/数据基线
  = 22 帧 / 返回庭院 3 素材 / 无全局 assets/

f6be0f0..8ee414c
  = docs-only（计划/裁定），代码和数据未变

8ee414c..9618cb5
  = 正式实现开始
  = 7 项需求 + action_shape + 0.1.0
  = 同时提交用户工作区的 28 帧 / 11 素材 / flow 删除
```

## 3. 代码 vs 7 项需求

| 需求 | 代码/证据 | 判定 |
|---|---|---|
| 画布完整、信息不盖画面、位置帧详情 | `index.html` canvas-stage/frame-detail；`style.css`；UI 实测 | ✅ 符合 |
| 框选抽函数、多边形、反选=整屏−区域 | `SelectionTool` 未完全抽象；但 UI/采样已有 action shape；`ActionShape` | ⚠ 部分：行为有，函数抽象不彻底 |
| 实时开→实时帧；实时关→位置帧 | 单素材/批量 API `live/frame_id`；UI 实测 | ✅ 符合 |
| 点测/拖测不再固定点 | `randomPointInAction` / `randomPointInBox`；实测不同点 | ✅ 符合 |
| 真实识别 bbox 与操作区重叠有效性 | `actionValidity`；实测输出 | ✅ 符合 |
| 素材拖动排序 | `reorder_assets` API + UI | ✅ 符合 |
| 帧分类重命名 | `/frames/category/rename` + UI | ✅ 符合 |

## 4. 数据 vs 正确基线

当前 `assets/` 34 的来源：

```text
11 个 = 用户 19:30 工作区里的返回庭院素材（合理）
23 个 = 从 git 全历史 tasks/*/assets.json 拼出的旧任务素材（不属于当前用户素材）
```

因此当前 34 不是“用户认为的全局库”，也不是 archive 全量。

真正的大迁移归档：

```text
archive/migration-20261101 = 29b696f
2827 素材 / 2099 图片 / 55 任务
```

该分支是 11-01 09:11 回滚前主动归档的；当前 dev 回滚到 `385d611` 时 `assets/` 被删除是预期行为。

## 5. 正确性结论

1. **代码主方向没有错**：7 项 + action_shape + 0.1.0 是从 19:30 开始按后续会话实现的。
2. **数据被搞乱**：`0bc69ba`/`1f73f17` 的“恢复”没有 session 依据；34 不是基线数据，也不是 archive 全量。
3. **UI 中途反复**：`1a021f8` 曾错误恢复旧 `/library` 面板，`1f73f17` 又移除；这是助手引入的重复劳动。
4. **未提交状态**：`frames/frames.json`、`tasks/返回庭院` 删除仍在工作区，不能算一致。

## 6. 建议修复（待用户确认）

### 方案 A（最小修数据）

- 备份当前 `assets/`；
- 将 `assets/` 裁剪为 **9618cb5 的 11 个用户素材**；
- 移除 23 个历史拼接素材和 18 张恢复图；
- 保留 archive 2827 不动；
- 保留代码改动，重新跑 UI/接口验证。

### 方案 B（全量迁移恢复）

- 从 `archive/migration-20261101` 导入 2827 素材 / 2099 图；
- 迁移旧 `action_roi` schema；
- 是否同时恢复 55 个任务，需单独决定。

### 方案 C（回到干净基线）

- 代码回到 `8ee414c` 或 `f6be0f0`；
- 数据恢复 `f6be0f0`（22 帧 / 3 素材 / 无全局 assets）；
- 丢弃 19:30 之后所有实现。
- 不推荐：会丢失 7 项新功能和用户后续帧。

## 7. 18:00~20:00 提交素材扫描（2026-11-01）

| 提交 | 时间 | 素材变化 | 提交时素材快照 |
|---|---|---|---|
| `f157dae` | 18:42 | 无 | task 3 素材 / 1 图；`assets/` 无 |
| `2b22b77` | 18:57 | 无 | 同上 |
| `7e16063` | 19:05 | 无 | 同上 |
| `8ee414c` | 19:14 | 无 | 同上 |
| `9618cb5` | 19:30 | **有**：task assets 3→11；删除 `images/a001.png`；删除 `flow.json` | task 11 个 OCR；0 图；`assets/` 无 |
| `c557ff1` | 19:52 | 无 | task 11；`assets/` 无 |
| `33ad968` | 20:01（边界外） | 无 | task 11；`assets/` 无 |
| `0bc69ba` | 20:23（边界外） | **首次创建全局 `assets/`（11）** | global 11 |
| `1f73f17` | 20:33（边界外） | **扩成 34（错误恢复）** | global 34 |

结论：**18:00~20:00 没有任何全局素材；唯一素材变化是 19:30 把任务内 3 个素材替换成 11 个 OCR，并删掉旧图/flow。**

