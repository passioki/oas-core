# RESUME — 断点续接卡（session-memory 强制 6）

> 规则：本文件**每次断点覆盖重写**；≤150 行；另一个会话只读本文件 + `RESUME-INDEX.md` 最新行
> 就能继续。细节放 `ENGINE-DESIGN.md` / `DELIVERY-PLAN.md` / `UI-MAP.md`。

```yaml
task_id: oas-core-editor-recovery-ui-map
checkpoint_seq: 68
updated_at: 2026-11-01T14:30:00+08:00
trigger: 截图两阶段/Enter/缩略图验收后，代码审计 + 文档更新；下一步前端 key/restart + 全局界面关系图
status: in_progress
```

## 0. 当前状态（磁盘实测）

- 分支：`dev`；`main` = `385d611`（v1.2.3 恢复节点，tag/Release 已发布）。
- 最近提交：`38c41a7 dev-data: latest frames and 预设 OCR asset`。
- 当前任务：`tasks/返回庭院` 一个任务；`task lint` = 1 task / 0 errors。
- 全局数据：`assets/assets.json`（含 `预设` OCR 素材）、`frames/frames.json`（21+ 帧）。
- 测试：`cargo test --workspace -- --skip shipped_legacy_tasks_pass_lint_untouched` = **351 passed / 0 failed**。
- 已知失败：不带 skip 的完整测试有 1 条：
  `oasd task::tests::shipped_legacy_tasks_pass_lint_untouched`；
  原因：测试硬编码旧 shipped 任务 `S1切预设`/`任务1`，当前 dev 数据已被用户替换为 `返回庭院`。
- 引擎：`oasd --version` = 1.2.3；serve/run 需要 `--features ocr`。
- 文档：`docs/START-HERE.md` + `docs/UI-MAP.md` 已更新；全局 `ui-map` 尚未实现。

## 1. 本轮完成（已验收）

### 截图 / 编辑器

- [x] 统一图片页「截取当前帧入库」；
- [x] 点击立即抓取，弹窗显示本次截取预览；
- [x] 名称/分类确认后保存；保存前不写帧；
- [x] 保存后不切当前帧；实时暂停时自动恢复实时；
- [x] 名称/分类搜索框 Enter = 确认输入，焦点转到保存按钮；
- [x] 弹窗 Enter = 直接保存截图；
- [x] OCR/region 素材缩略图只认 `positions[0]`，不再回退当前帧；
- [x] 无位置帧显示「未设位置帧」，失效显示「位置帧失效」；
- [x] 首次保存素材无位置帧时自动使用当前帧；
- [x] 实时开启时画布点击/拖拽 = tap/swipe；暂停 = ROI 编辑。

### 返回庭院 / DSL

- [x] `tasks/返回庭院`：OCR 庭院判定 + 设置/式神录特殊分支 + 左上返回区域 + 退出弹窗取消 + 重启兜底；
- [x] 引擎新增 `key` / `restart_app`（DSL/parser/runtime/InputSink/ADB/wrappers）；
- [x] 实机验证庭院、设置页、探索地图、退出弹窗、重启启动链路。

## 2. 下一步（按顺序）

1. **前端补齐 `key` / `restart_app`**
   - `apps/oasd/ui/builder.js`：`OP_META / FIELDS / DEFAULTS / stepSummary`；
   - `apps/oasd/ui/flow_model.js`：`OPS / OP_LABELS / defaultArgs`；
   - 字段：keycode（BACK=4/HOME=3/自定义）、package。

2. **全局界面关系图**
   - 新建 `ui-map/ui-map.json`（全局唯一，不属于任务）；
   - `Frame.interface_id` 一帧一界面；按 category 迁移，无法映射归 `unknown`；
   - 保持 `Asset.positions` 为唯一 asset↔frame 关系，通过 Frame 推导素材所属界面，禁止重复存 asset.interface_id；
   - OCR 标记 / 区域动作 / 边 / 优先级 / 重启兜底；
   - 生成 `docs/UI-MAP.md` 的人类可读 Mermaid/表格。

3. **帧/素材归档**
   - 补齐庭院、设置、式神录、探索、退出确认、启动/登录、未知界面参考帧；
   - 每帧指定界面；素材位置帧显示界面归属；
   - OCR/区域素材维护操作区域。

4. **校验 + flow 生成**
   - 校验：节点 ID、OCR marker、边引用、所有节点可达 courtyard、ROI、未知兜底；
   - 第一阶段用 `ui-map.json → 返回庭院 flow` 生成，不做 runtime 动态图执行器；
   - 后续再考虑前端只读图视图。

5. **测试收尾**
   - 决定 `shipped_legacy_tasks_pass_lint_untouched`：
     - 恢复旧任务数据；或
     - 改成 fixture / 数据无关测试。
   - 更新 `ENGINE-DESIGN` 的 `key`/`restart_app` 与 UI-MAP 设计记录。

## 3. 约束

- 不自行 tag/release；`main` 保持 `385d611`。
- 所有文档更新只写 `oas-core/docs`，不回写旧 backend。
- 素材缩略图不得回退当前帧；OCR/区域素材只认 `positions`。
- 全局 UI 关系图以 JSON 为唯一真相；Obsidian/Mermaid 只是视图。
- 不引入神经网络式黑盒控制；使用可编辑有向标签图。
- 集合优先级/成员拖动排序：继续暂缓。

## 4. 证据

- 截图预览/提交端点：`apps/oasd/src/serve.rs`（`/capture/preview`、`/capture/preview.png`、`/capture/commit`）；
- 截图 UI：`apps/oasd/ui/app.js`、`asset_ui.js`、`index.html`、`style.css`；
- 返回庭院：`tasks/返回庭院/flow.json`；
- 新增 DS 步骤：`crates/oas-flow/src/lib.rs`、`crates/oas-runtime/src/lib.rs`、`crates/oas-types/src/lib.rs`、`crates/oas-adapter-pc/src/lib.rs`；
- 设计稿：`docs/UI-MAP.md`；
- 提交：`a141b72`（Enter 行为）、`38c41a7`（帧/素材数据）。
