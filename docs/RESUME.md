# RESUME — 断点续接卡（session-memory 强制 6）

> 规则：本文件**每次断点覆盖重写**；≤150 行；另一个会话只读本文件 + `RESUME-INDEX.md` 最新行
> 就能继续。细节放 `ENGINE-DESIGN.md` / `DELIVERY-PLAN.md` / `UI-MAP.md`。

```yaml
task_id: oas-core-ui-map-recovery
checkpoint_seq: 69
updated_at: 2026-11-01T14:45:00+08:00
trigger: 全局界面关系图 + Frame.interface_id + 前端 key/restart + 图 lint/render/flow + legacy 测试 fixture
status: in_progress
```

## 0. 当前状态（磁盘实测）

- 分支 `dev`；`main` = `385d611`（v1.2.3 恢复节点，tag/Release 已发布）。
- 测试：`cargo test --workspace` = **356 passed / 0 failed**（legacy shipped 测试已改为临时 fixture）。
- 任务：`tasks/返回庭院`；`task lint` = 1 task / 0 errors。
- 全局图：`ui-map/ui-map.json` = 15 interfaces / 14 transitions；`ui-map lint` ok。
- 帧：`frames/frames.json` 22 帧，均有 `interface_id`；已补设置/退出确认/加载/启动参考帧。
- 素材：`tasks/返回庭院/assets.json` 有 `预设` OCR；素材位置帧现在显示界面。
- 新 DSL：`key` / `restart_app` 后端 + 前端 builder/flow_model 均已补齐。
- 已生成：`tasks/返回庭院/flow.generated.json` + `docs/UI-MAP.generated.md`（生成物，不自动替换手写 flow）。

## 1. 本轮完成

- [x] 前端 `builder.js` / `flow_model.js` 补 `key` / `restart_app`（动作库、字段、默认值、摘要）；
- [x] 全局 `ui-map/ui-map.json`：15 界面、14 边、OCR marker、区域/OCR 动作、重启兜底；
- [x] `Frame.interface_id`：`SessionFrame`/`FrameMeta`/`/frames`/`/frames/interface`/捕获提交；
- [x] 现有帧迁移：按 category（及极少数 name）补 interface_id；`oasd ui-map migrate-frames`；
- [x] `oasd ui-map lint`：节点唯一/OCR/动作/ROI/可达性/重启兜底；`--frames` 校验帧引用；
- [x] `oasd ui-map render`：Mermaid + 表格 → `docs/UI-MAP.generated.md`；
- [x] `oasd ui-map flow`：从图生成静态 recovery flow → `tasks/返回庭院/flow.generated.json`；
- [x] 截图弹窗加界面选择；帧列表显示界面并提供「界」修改；
- [x] 任务构建只读“界面关系图”视图（界面/OCR/动作/参考帧数/关系）；
- [x] `shipped_legacy_tasks_pass_lint_untouched` 改为临时 fixture 测试（不再依赖用户 tasks/ 数据）；
- [x] 全量测试 356 passed；`ui-map lint` + `task lint` ok。

## 2. 下一步

1. 评审 `tasks/返回庭院/flow.generated.json`，决定是否用它替换手写 flow（当前手写 flow 保留）；
2. 继续补活动页参考帧/素材，并给帧设置正确 interface；
3. 视需要增加图编辑（先只读）；
4. 发布前跑完整测试 + `ui-map lint` + `task lint`。

## 3. 约束

- 不自行 tag/release；`main` 保持 `385d611`。
- 全局 UI 图唯一数据 = `ui-map/ui-map.json`；`docs/UI-MAP.generated.md` 是 disposable。
- Frame 一帧一界面；Asset.positions → Frame → Interface，不在 Asset 重复存 interface。
- OCR/region 素材缩略图只认 `positions`；不回退当前帧。
- 旧 backend 文档冻结，所有更新只写 `oas-core/docs/`。
- 集合优先级/成员拖动排序：继续暂缓。

## 4. 证据

- `apps/oasd/src/ui_map.rs`（图模型/lint/render/flow/migrate-frames/测试）；
- `apps/oasd/src/serve.rs`（Frame.interface_id、`/frames/interface`、`/ui-map`、capture commit）；
- `apps/oasd/ui/builder.js`、`flow_model.js`（key/restart）；
- `apps/oasd/ui/app.js`、`index.html`（截图界面选择、帧界面显示/修改）；
- `ui-map/ui-map.json`、`docs/UI-MAP.md`、`docs/UI-MAP.generated.md`；
- `tasks/返回庭院/flow.generated.json`；
- 测试：`cargo test --workspace` = 356 passed。
