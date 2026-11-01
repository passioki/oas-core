# RESUME — 断点续接卡（session-memory 强制 6）

> 规则：本文件**每次断点覆盖重写**；≤150 行；另一个会话只读本文件 + `RESUME-INDEX.md` 最新行
> 就能继续。细节放 `ENGINE-DESIGN.md` / `DELIVERY-PLAN.md`。

```yaml
task_id: oas-core-frame-asset-relation
checkpoint_seq: 73
updated_at: 2026-11-01T16:20:00+08:00
trigger: Phase 3/4/5：人工标注、覆盖概览、任务构建关系提示
status: in_progress
```

## 0. 当前状态（磁盘实测）

- 分支 `dev`；`main` = `385d611`（v1.2.3 恢复节点）。
- 测试：`cargo test --workspace` = **356 passed / 0 failed**。
- 任务：`tasks/返回庭院`；`task lint` = 1 task / 0 errors。
- 帧：22 帧，无 `interface_id`；新增设置/退出/加载/启动参考帧保留。
- 素材关系：唯一关系 = `Asset.positions`；帧行显示关联素材数量；只读关系浏览器显示帧→素材、素材→帧、素材共现、帧共享素材、录制 transition、人工标注与覆盖概览。
- 新 DSL：`key` / `restart_app` 前后端保留。
- 已删除：`ui-map/`、`docs/UI-MAP*`、`flow.generated.json`、`Frame.interface_id`、界面选择/图视图、`ui_map.rs`。

## 1. 用户裁定（现行）

- **不需要“界面”概念**；接口分类太生硬。
- **帧-素材关系是唯一关系**：素材出现在哪些帧 = `Asset.positions`。
- 不新增 interface 字段、不新增持久化关系表；关系由帧/素材数据派生。
- 保留帧 26/28/29/30，只删 interface 字段。
- 回退用新提交，不改写 git 历史。

## 2. 本轮完成

- [x] 前端/后端移除 `Frame.interface_id`；
- [x] 删除 `/ui-map`、`/frames/interface`、界面选择器、帧「界」按钮、界面图视图；
- [x] 删除 `ui_map.rs` / `oasd ui-map` 命令 / ui-map 数据/文档/生成 flow；
- [x] `frames.json` 清除 `interface_id`；
- [x] 保留两阶段截图、Enter、缩略图、key/restart、返回庭院 flow、测试 fixture；
- [x] 帧行增加“关联素材数”派生显示（基于 `Asset.positions`）；
- [x] 新增 `GET /relations/frame-assets`（纯派生，不持久化）；
- [x] 新增只读“帧-素材关系”浏览器：帧→素材、素材→位置帧、素材共现、帧共享素材；
- [x] Phase 2：`POST /relations/record` 抓前帧 → 点素材动作区域 → 等待 → 抓后帧；
- [x] `relations/observations.jsonl` 追加 transition 证据；关系浏览器显示 transition；
- [x] Phase 3：`relations/annotations.json` + `/relations/annotate`/`delete`；alternative/requires/similar/overlay；校验自反/重复/未知节点；
- [x] 关系快照增加 `coverage`；浏览器显示覆盖概览、人工标注、删除；
- [x] Phase 4：任务构建 inspector 显示选中素材的帧/transition/标注提示；
- [x] 全量测试 356 passed；task lint 1/0；
- [x] 文档更新：START-HERE/RESUME/HANDOFF/README/ENGINE-DESIGN/PENDING。

## 3. 下一步

1. 真实动作录制回归（需停在正确画面）；
2. 任务构建进一步关系感知：按当前帧过滤素材、transition 推荐；
3. 继续补活动页参考帧/标注；
4. 发布前跑完整测试 + `task lint`。

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
- `frames/frames.json`（22 帧，无 interface_id）；
- 测试：`cargo test --workspace` = 356 passed。
