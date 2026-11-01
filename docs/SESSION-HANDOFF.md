# 会话交接单（SESSION HANDOFF）★ 一页纸

> ★★★ 新会话先读 `docs/START-HERE.md`（唯一入口）。
> 当前断点以 `docs/RESUME.md` + `docs/RESUME-INDEX.md` 为准；本文件只放“当前状态 + 下一步 + 活跃的坑”。

## 0. 当前状态（2026-11-01）

| 项 | 值 |
|---|---|
| 恢复节点 | oas-core `main` = `385d611`（v1.2.3，tag/Release 均已发布） |
| 开发分支 | `dev`；编辑器/图/任务数据提交在此分支；main 保持恢复节点 |
| 文档入口 | `docs/START-HERE.md`；界面图设计/状态 `docs/UI-MAP.md`；生成视图 `docs/UI-MAP.generated.md` |
| 测试 | `cargo test --workspace` = **356 passed / 0 failed** |
| 任务 | `task lint --tasks-dir tasks --all` = 1 task / 0 errors（`返回庭院`） |
| 全局图 | `ui-map/ui-map.json` = 15 interfaces / 14 transitions；lint ok |
| 帧 | 22 帧，均有 `interface_id`；新增设置/退出确认/加载/启动参考帧 |
| 新 DSL | `key` / `restart_app` 前端+后端可用 |
| 工作区 | `D:\OAS-dev\oas-core` |

## 1. 本轮完成

- 全局界面关系图数据/CLI/生成：`lint`、`render`、`flow`、`migrate-frames`；
- `Frame.interface_id` 与现有帧迁移；截图弹窗选界面；帧列表显示/修改界面；
- 前端动作库补齐 `key` / `restart_app`；
- 任务构建顶部有只读“界面关系图”视图（界面/OCR/动作/帧数/关系）；
- 从图生成静态 recovery flow（`tasks/返回庭院/flow.generated.json`），未替换手写 flow；
- legacy shipped task 测试改为临时 fixture，完整测试恢复全绿。

## 2. 下一步

1. 评审 `flow.generated.json`，决定是否替换手写 `flow.json`；
2. 补齐活动页参考帧/素材并设置 interface；
3. 视需要增加图编辑（先只读）；
4. 发布前跑完整测试 + `ui-map lint` + `task lint`。

## 3. 活跃的坑

- `flow.generated.json` 是生成评审稿，**不要直接当现行 flow**；手写 `flow.json` 仍为运行入口。
- 图生成器只支持 region/ocr_click/key/restart_app/wait；新增 action 要同时改 `ui_map.rs` 和测试。
- 全局图 `ui-map/ui-map.json` 与 `--frames` 一致性用 `oasd ui-map lint --frames frames/frames.json` 校验。
- 重新构建前必须停 8787 dev serve（E2）。
- 旧 backend toolkit 仍是本机 ADB/OCR 模型路径；源码/构建不依赖 backend。
- 旧 backend 文档冻结，不要回写。
