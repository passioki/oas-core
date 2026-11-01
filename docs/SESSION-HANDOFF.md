# 会话交接单（SESSION HANDOFF）★ 一页纸

> ★★★ 新会话先读 `docs/START-HERE.md`（唯一入口）。
> 当前断点以 `docs/RESUME.md` + `docs/RESUME-INDEX.md` 为准；本文件只放“当前状态 + 下一步 + 活跃的坑”。

## 0. 当前状态（2026-11-01）

| 项 | 值 |
|---|---|
| 恢复节点 | oas-core `main` = `385d611`（历史 v1.2.3；当前 dev 版本 **0.1.0**） |
| 开发分支 | `dev`；编辑器/帧/素材/任务数据提交在此分支 |
| 文档入口 | `docs/START-HERE.md` |
| 测试 | `cargo test --workspace` = **357 passed / 0 failed** |
| 任务 | `返回庭院` 仍在用户编辑中（flow 被删除）；不强行恢复 |
| 帧 | 当前 28 帧；旧 22 帧恢复记录 + 用户新增 35~40 |
| 操作区 | `action_shape`（rect/polygon，invert=整个屏幕−区域）+ `action_source`；旧 `action_roi` 已删 |
| 关系 | **唯一关系 = `Asset.positions`（帧-素材）**；只读关系浏览器已可用；无 interface/ui-map |
| 帧安全 | 清空列表需确认；清空前自动备份到 `frames/.backup/clear-<时间>/` |
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
- Phase 3：`relations/annotations.json` + `/relations/annotate`/delete；alternative/requires/similar/overlay；自反/重复/未知节点校验。
- 关系浏览器增加覆盖概览、人工标注、删除；任务构建 inspector 显示选中素材的帧/transition/标注提示。
- 帧清空事故：22 个 git tracked 帧已恢复，`next_id=35`；31~34 新录制帧 PNG 无法恢复，observations 保留。
- 清空列表：前端 confirm + 后端 `confirm:true` + 自动备份 `frames/.backup/`。
- 画布：16:9 stage + 可收起结果/日志 + 帧详情侧栏；点测/拖测随机采样。
- 操作区域：`ActionShape`/`action_source` schema；当前任务 assets 已迁移；UI 支持矩形、多边形、全屏反选；旧 `action_roi`/`roi_back` 删除。
- 素材拖动排序 + 帧分类重命名（空名禁止）已实现。
- 单素材测试带 live/frame_id 来源；真实识别 bbox 有效性提示。
- 文档回退并更新测试基线 357。

## 2. 下一步

1. runtime 精确 polygon/invert 采样（当前 bbox fallback）；
2. live 批量测试；
3. 真实动作录制回归；
4. 任务构建进一步关系感知；
5. 活动页参考帧/标注。

## 3. 活跃的坑

- 不要重新引入 interface/`ui-map`/第二份关系表；用户已明确否决。
- `Asset.positions` 是唯一关系；帧分类只是自由分组。
- 帧行关联素材数依赖当前任务 `taskAssets`；未选任务时显示 0 是正常。
- 重新构建前必须停 8787 dev serve（E2）。
- 旧 backend toolkit 仍是本机 ADB/OCR 模型路径；源码/构建不依赖 backend。
- 旧 backend 文档冻结，不要回写。
