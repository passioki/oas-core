# 会话交接单（SESSION HANDOFF）★ 一页纸

> ★★★ 新会话先读 `docs/START-HERE.md`（唯一入口）。
> 当前断点以 `docs/RESUME.md` + `docs/RESUME-INDEX.md` 为准；本文件只放“当前状态 + 下一步 + 活跃的坑”。

## 0. 当前状态（2026-11-01）

| 项 | 值 |
|---|---|
| 恢复节点 | oas-core `main` = `385d611`（v1.2.3，tag/Release 均已发布） |
| 开发分支 | `dev`；文档迁移提交在此分支；main 保持恢复节点 |
| 文档入口 | ★ `docs/START-HERE.md`；文档已从旧 backend 仓独立到 `oas-core/docs` |
| 文档源 | 旧仓 `D:\OAS-dev\backend\docs` @ `2861d8ba`（只读历史） |
| 引擎 | `oasd --version` = 1.2.3 |
| 测试 | `cargo test --workspace` = 350 passed / 0 failed |
| 任务 | `oasd task lint --tasks-dir tasks --all` = 4 tasks / 0 errors |
| 工作区 | `D:\OAS-dev\oas-core` |

## 1. 本轮完成：文档独立

- 迁入 10 份有效文档到 `oas-core/docs/`；
- 新增 `docs/README.md`、`docs/MIGRATION-NOTES.md`、`session-memory.config.json`；
- `START-HERE.md` 工作目录/文档通道/报告模板已改为 oas-core；
- `RESUME`/`RESUME-INDEX` 建立 seq67 文档迁移断点；
- 旧 backend 文档仓冻结，不再作为 oas-core 文档入口；
- 旧 backend 专用文档（architecture/acceptance/android/deprecated/旧 workflow/layout 等）未迁入。

## 2. 下一步

1. 提交并推送 oas-core `docs/` + `session-memory.config.json` 到 `dev`；
2. 下次新会话固定：
   - 工作目录 `D:\OAS-dev\oas-core`
   - 先 `skill session-memory`
   - 读 `docs/START-HERE.md`
3. 按 `PENDING-LEDGER.md` 继续实际开发任务。

## 3. 活跃的坑

- `D:\OAS-dev\backend\toolkit` 仍是本机 ADB/OCR 模型外部路径；oas-core 源码/构建不依赖 backend，但本机运行 OCR/实机验证仍可能用到。
- 旧 backend 文档里的路径/流程/验收口径已过时，不要混入现行文档。
- 迁移文档不要回写 backend；所有更新只写 oas-core `docs/`。
