# 会话交接单（SESSION HANDOFF）★ 一页纸

> ★★★ 新会话先读 `docs/START-HERE.md`（唯一入口）。
> 当前断点以 `docs/RESUME.md` + `docs/RESUME-INDEX.md` 为准；本文件只放“当前状态 + 下一步 + 活跃的坑”。

## 0. 当前状态（2026-11-01）

| 项 | 值 |
|---|---|
| 恢复节点 | oas-core `main` = `385d611`（v1.2.3，tag/Release 已发布） |
| 开发分支 | `dev`；编辑器/帧/素材/任务数据提交在此分支 |
| 文档入口 | `docs/START-HERE.md` |
| 测试 | `cargo test --workspace` = **355 passed / 0 failed** |
| 任务 | `task lint --tasks-dir tasks --all` = 1 task / 0 errors（`返回庭院`） |
| 帧 | 22 帧；无 `interface_id`；新增设置/退出/加载/启动参考帧 |
| 关系 | **唯一关系 = `Asset.positions`（帧-素材）**；只读关系浏览器已可用；无 interface/ui-map |
| 新 DSL | `key` / `restart_app` 前后端保留 |
| 工作区 | `D:\OAS-dev\oas-core` |

## 1. 本轮完成

- 按用户裁定回退 interface 抽象：
  - 删除 `Frame.interface_id`、`/frames/interface`、`/ui-map`、界面选择器、帧「界」按钮、界面图视图；
  - 删除 `ui_map.rs`、`oasd ui-map` 命令、`ui-map/`、`docs/UI-MAP*`、`flow.generated.json`；
  - 从 `frames/frames.json` 清除 interface 字段。
- 保留：两阶段截图、Enter 行为、缩略图只认 positions、key/restart、返回庭院 flow、测试 fixture。
- 帧行显示关联素材数量（由 `Asset.positions` 派生）。
- 新增 `GET /relations/frame-assets` 与只读关系浏览器：帧→素材、素材→帧、素材共现、帧共享素材。
- Phase 2：`POST /relations/record` 抓前帧 → 点素材动作区域 → 等待 → 抓后帧；追加 `relations/observations.jsonl` transition 证据。
- 文档回退并更新测试基线 355。

## 2. 下一步

1. Phase 3：人工标注 alternative/requires/similar；
2. 任务构建关系感知（按帧过滤素材、显示 transition 提示）；
3. 真实动作录制回归 + 继续补活动页参考帧；
4. 发布前跑完整测试 + `task lint`。

## 3. 活跃的坑

- 不要重新引入 interface/`ui-map`/第二份关系表；用户已明确否决。
- `Asset.positions` 是唯一关系；帧分类只是自由分组。
- 帧行关联素材数依赖当前任务 `taskAssets`；未选任务时显示 0 是正常。
- 重新构建前必须停 8787 dev serve（E2）。
- 旧 backend toolkit 仍是本机 ADB/OCR 模型路径；源码/构建不依赖 backend。
- 旧 backend 文档冻结，不要回写。
