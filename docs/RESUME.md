# RESUME — 断点续接卡（session-memory 强制 6）

> 规则：本文件**每次断点覆盖重写**；≤150 行；另一个会话只读本文件 + `RESUME-INDEX.md` 最新行
> 就能继续。细节放 `ENGINE-DESIGN.md` / `DELIVERY-PLAN.md`。

```yaml
task_id: oas-core-frame-asset-relation
checkpoint_seq: 76
updated_at: 2026-11-01T18:45:00+08:00
trigger: 编辑器 UX/选择模型/实时帧/区域有效性/排序/分类改版计划（未实现）
status: planned
```

## 0. 当前状态（磁盘实测）

- 分支 `dev`；`main` = `385d611`（v1.2.3 恢复节点）。
- 测试：`cargo test --workspace` = **356 passed / 0 failed**。
- 任务：`tasks/返回庭院`；`task lint` = 1 task / 0 errors。
- 帧：22 帧已恢复（git tracked）；`next_id=35`；31~34 是清空前新录制帧，PNG 不可恢复，observations 仍保留。
- 编辑器 UX 计划：`docs/EDITOR-UX-PLAN.md`（本轮仅设计/文档，未改代码）。
- 已确认：实时关闭时单素材多位置全部测试；帧分类重命名 `to` 为空禁止；操作区域用 `action_shape`（方案 B）；反选=整个屏幕−选择区域；有效性比较真实识别 bbox。
- 待确认：是否新增 `action_source`。
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
- [x] 全量测试 356 passed；task lint 1/0；
- [x] 文档记录事故与恢复边界。
- [x] 位置帧继承：`asset-position-inherit-host` 源素材 picker；已有素材编辑弹窗也可继承；只追加 positions。
- [x] 全量测试 356 passed；task lint 1/0。

## 3. 下一步

1. Phase A（可见问题）：画布 16:9 fit、结果/日志/提示移出画布、位置帧详情侧栏、点测/拖测 humanize 采样；
2. Phase B：实时帧/位置帧测试来源（关闭时全部位置帧都测）、识别/操作区域重叠校验；
3. Phase C：抽 `SelectionTool`、操作区域 `action_shape` 多边形 + 全屏反选；
4. Phase D：素材拖动排序、帧分类重命名（`to` 为空禁止）；
5. 先确认是否新增 `action_source`（其余字段/语义已确认）。

## 4. 约束

- 不自行 tag/release；`main` 保持 `385d611`。
- `Asset.positions` 是唯一关系；不要再加 interface/额外关系表。
- 帧分类只是自由分组，不是强制界面。
- OCR/region 素材缩略图只认 `positions`；不回退当前帧。
- 旧 backend 文档冻结；所有更新只写 `oas-core/docs/`。

## 5. 证据

- `apps/oasd/src/serve.rs`（`/relations/frame-assets`、`/relations/record`、`/relations/annotate`、delete）；
- `apps/oasd/src/relations.rs`（派生关系 + observation/annotation 读取 + coverage + 测试）；
- `apps/oasd/ui/app.js`（关系浏览器、录制/标注弹窗、覆盖概览）；
- `apps/oasd/ui/builder.js`（任务构建关系提示）；
- `relations/README.md`（observations/annotations schema）；
- `docs/EDITOR-UX-PLAN.md`（本轮待实现设计）；
- `frames/frames.json`（22 帧，无 interface_id）；
- 测试：`cargo test --workspace` = 356 passed；清空安全提交 `951253b`。
