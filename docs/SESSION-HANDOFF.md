# 会话交接单（SESSION HANDOFF）★ 一页纸

> ★★★ 新会话先读 `docs/START-HERE.md`（唯一入口）。
> 当前断点以 `docs/RESUME.md` + `docs/RESUME-INDEX.md` 为准；本文件只放“当前状态 + 下一步 + 活跃的坑”。

## 0. 当前状态（2026-11-01）

| 项 | 值 |
|---|---|
| 恢复节点 | oas-core `main` = `385d611`（v1.2.3，tag/Release 均已发布） |
| 开发分支 | `dev`；文档/编辑器/任务数据提交在此分支；main 保持恢复节点 |
| 最近提交 | `38c41a7`（最新帧 + `预设` OCR 素材）；上一功能提交 `a141b72`（Enter 行为） |
| 文档入口 | ★ `docs/START-HERE.md`；全局界面关系图设计稿 `docs/UI-MAP.md` |
| 引擎 | `oasd --version` = 1.2.3 |
| 测试 | skip legacy 数据测试 = **351 passed / 0 failed**；完整测试有 1 条已知数据依赖失败 |
| 任务 | `task lint --tasks-dir tasks --all` = 1 task / 0 errors（`返回庭院`） |
| 工作区 | `D:\OAS-dev\oas-core` |

## 1. 本轮完成

### 编辑器截图

- 两阶段截图：`/capture/preview` 立即抓图到内存 → 弹窗预览 → `/capture/commit` 保存；
- 弹窗阶段显示本次截取帧；保存后恢复实时，不切当前帧；
- 输入框 Enter = 确认输入；弹窗 Enter = 保存截图；
- OCR/region 缩略图只认 `positions[0]`，删除当前帧回退；
- 首次保存素材无位置帧时自动使用当前帧。

### 返回庭院

- `tasks/返回庭院` 使用 OCR（町中/召唤）判定庭院、左上返回区域、设置右上关闭、退出弹窗取消、重启游戏兜底；
- 新增引擎步骤 `key` / `restart_app`；
- 实机验证通过常见路径。

## 2. 下一步

1. 前端 `builder.js` / `flow_model.js` 补齐 `key` / `restart_app`；
2. 建全局 `ui-map/ui-map.json` + `Frame.interface_id`；
3. 归档所有已知界面参考帧与素材；
4. 写图校验、Mermaid 生成、`ui-map.json → 返回庭院 flow` 生成器；
5. 决定 legacy shipped task 测试：恢复旧数据或改 fixture。

## 3. 活跃的坑

- `shipped_legacy_tasks_pass_lint_untouched` 依赖旧任务 `S1切预设`/`任务1`，当前 `tasks/` 只剩 `返回庭院`，完整测试会红；已用 `--skip` 作为当前基线，但发布/CI 前必须处理。
- 重新构建 `oasd.exe` 前必须停掉 8787 的 dev serve，否则 `failed to remove target\debug\oasd.exe`。
- `D:\OAS-dev\backend\toolkit` 仍是本机 ADB/OCR 模型外部路径；源码/构建不依赖 backend。
- 全局 UI 关系图尚未实现，目前只有 `docs/UI-MAP.md` 设计稿；不要声称已落地。
- OCR/区域素材缩略图必须来自位置帧；素材所属界面最终通过 `Frame.interface_id` 推导，不要在 asset 里重复存。
- 旧 backend 文档冻结，不要回写；所有更新只写 `oas-core/docs/`。
