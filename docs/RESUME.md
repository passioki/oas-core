# RESUME — 断点续接卡（session-memory 强制 6）

> 规则：本文件**每次断点覆盖重写**；≤150 行；另一个会话只读本文件 + `RESUME-INDEX.md` 最新行
> 就能继续。细节放 `ENGINE-DESIGN.md` / `DELIVERY-PLAN.md`。

```yaml
task_id: oas-core-editor-ux
checkpoint_seq: 77
updated_at: 2026-11-01T19:40:00+08:00
trigger: v0.1.0 + action_shape 清理 + 画布/位置详情/排序/分类首轮实现
status: in_progress
```

## 0. 当前状态（磁盘实测）

- 分支 `dev`；版本 `workspace.package.version = 0.1.0`；`main` = `385d611`（历史恢复点）。
- 测试：`cargo test --workspace` = **357 passed / 0 failed**。
- 任务：`tasks/返回庭院` 当前仍在用户手上编辑（flow 删除）；不强行恢复。
- 帧：当前 28 帧（用户新增 35~40）；旧 22 帧恢复记录仍在。
- 编辑器 UX：画布 16:9 stage、结果/日志可收起、帧详情侧栏已实现。
- 操作区域：`action_shape`（rect/polygon + invert=整个屏幕−区域）+ `action_source` 已落地；旧 `action_roi`/`roi_back` 从代码与当前 assets 数据移除。
- 已实现测试来源：单素材测试带 `live=1` 或 `frame_id`；批量测试仍按位置帧（live 批量待补）。
- 已实现：素材拖动排序接口/UI、帧分类重命名接口/UI、点测/拖测随机采样。
- 待补：runtime 对 polygon/invert 的精确采样（目前 runtime bbox fallback）；live 批量；真实动作回归。
- 素材关系：唯一关系 = `Asset.positions`；帧行显示关联素材数量；只读关系浏览器显示帧→素材、素材→帧、素材共现、帧共享素材、录制 transition、人工标注与覆盖概览。
- 位置帧继承：新素材保存区/已有素材位置编辑弹窗都能从其他任务素材追加 `positions`，不新增持久化引用。
- 帧安全：清空列表必须 `confirm:true`；清空前自动备份到 `frames/.backup/clear-<时间>/`。
- 新 DSL：`key` / `restart_app` 前后端保留。
- 已删除：`ui-map/`、`docs/UI-MAP*`、`flow.generated.json`、`Frame.interface_id`、界面选择/图视图、`ui_map.rs`。

## 1. 用户裁定（现行）

- **不需要“界面”概念**；接口分类太生硬。
- **帧-素材关系是唯一关系**：素材出现在哪些帧 = `Asset.positions`。
- 不新增 interface 字段、不新增持久化关系表；关系由帧/素材数据派生。
- 保留帧 26/28/29/30，只删 interface 字段。
- 回退用新提交，不改写 git 历史。

## 2. 本轮完成

- [x] **帧清空事故恢复**：22 个 git tracked 帧 + manifest 已恢复；`next_id=35`；
- [x] 31~34 新录制帧 PNG 无法从 git/回收站/影子副本恢复；保留 `relations/observations.jsonl` 证据；
- [x] 前端「清空列表」增加 `window.confirm`，显示帧数/备份说明；
- [x] 后端 `/frames/clear` 必须 `{confirm:true}`，否则 400；
- [x] 清空前自动备份 `frames.json` + 所有 PNG 到 `frames/.backup/clear-<时间>/`；
- [x] 增加帧清空测试（无确认拒绝 + 备份存在）；
- [x] 全量测试 357 passed；task lint 1/0；
- [x] 文档记录事故与恢复边界。
- [x] 位置帧继承：`asset-position-inherit-host` 源素材 picker；已有素材编辑弹窗也可继承；只追加 positions。
- [x] 全量测试 357 passed；task lint 1/0。
- [x] 版本重置 dev `0.1.0`（旧 v1.x tag 保留）。
- [x] 画布 16:9 stage + 结果/日志收起 + 位置帧详情侧栏。
- [x] 点测/拖测随机采样（同区域多次不同点）。
- [x] `action_shape` + `action_source` schema；当前任务 assets 已迁移；`action_roi`/`roi_back` 代码删除。
- [x] 素材拖动排序 + 帧分类重命名。
- [x] 单素材测试 live/frame_id 来源与真实识别 bbox 有效性提示。
- [x] 全量测试 357 passed。

## 3. 下一步

1. runtime 精确支持 polygon/invert 操作区（当前 bbox fallback）；
2. 批量测试 live 帧来源；
3. 真实动作录制回归 + 完整验收；
4. 任务构建按当前帧过滤素材/推荐 transition；
5. 活动页参考帧/标注继续补。

## 4. 约束

- 不自行 tag/release；`main` 保持 `385d611`。
- `Asset.positions` 是唯一关系；不要再加 interface/额外关系表。
- 帧分类只是自由分组，不是强制界面。
- OCR/region 素材缩略图只认 `positions`；不回退当前帧。
- 旧 backend 文档冻结；所有更新只写 `oas-core/docs/`。

## 5. 证据

- `apps/oasd/src/serve.rs`（`/relations/frame-assets`、`/relations/record`、`/relations/annotate`、delete）；
- `apps/oasd/src/relations.rs`（派生关系 + observation/annotation 读取 + coverage + 测试）；
- `apps/oasd/ui/app.js`（画布 stage、帧详情、关系浏览器、操作区采样、排序/分类）；
- `apps/oasd/ui/asset_ui.js`（拖动排序行）；
- `apps/oasd/ui/index.html` + `style.css`（stage/详情/操作区按钮）；
- `crates/oas-task/src/lib.rs`（`ActionShape`、`action_source`、reorder）；
- `relations/README.md`（observations/annotations schema）；
- `docs/EDITOR-UX-PLAN.md`（现行方案；部分 runtime 待补）；
- `frames/frames.json`（当前 28 帧，无 interface_id）；
- 测试：`cargo test --workspace` = 357 passed；版本 `0.1.0`。
