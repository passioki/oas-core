# RESUME — 断点续接卡（session-memory 强制 6）

> 规则：本文件**每次断点覆盖重写**；≤150 行；另一个会话只读本文件 + `RESUME-INDEX.md` 最新行
> 就能继续。细节放 `ENGINE-DESIGN.md` / `DELIVERY-PLAN.md`。

```yaml
task_id: oas-core-docs-independent
checkpoint_seq: 67
updated_at: 2026-11-01T09:30:00+08:00
trigger: 文档从旧 backend 仓迁入 oas-core/docs（独立文档/skill 适配）
status: in_progress
```

## 0. 当前状态（磁盘实测）

- oas-core `main` = `385d611`（v1.2.3 恢复节点）；`dev` = `4b4795d`（docs migration，后续还会有记录提交）。
- oas-core 文档已独立到 `D:\OAS-dev\oas-core\docs`；旧 backend 文档仓已冻结，不再作为 oas-core 文档入口。
- 源文档提交：旧仓 `2861d8ba`（v1.2.3 release 文档节点）。
- `cargo test --workspace` = **350 passed / 0 failed**。
- `oasd task lint --tasks-dir tasks --all` = **4 tasks / 0 errors**。
- `oasd --version` = **1.2.3**。

## 1. 本轮完成

### 验收清单

- [x] 从旧仓 `2861d8ba` 复制 10 份有效文档到 `oas-core/docs`
- [x] 新增 `docs/README.md` / `docs/MIGRATION-NOTES.md`
- [x] 新增 `session-memory.config.json`，`continuity.py status` 可读取
- [x] `START-HERE.md` 改为 oas-core 文档入口/工作目录
- [x] `RESUME`/`RESUME-INDEX`/`PENDING-LEDGER`/`SESSION-INDEX` 记录文档迁移
- [x] 提交并推送 docs 到 oas-core `dev`（docs commit `4b4795d`）
- [x] `cargo test --workspace` = 350 passed
- [x] `task lint --all` = 4/0

### 文档迁移

- 复制有效文档到 `oas-core/docs/`：
  - `START-HERE.md`
  - `RESUME.md` / `RESUME-INDEX.md`
  - `SESSION-HANDOFF.md`
  - `ENGINE-DESIGN.md`
  - `PENDING-LEDGER.md`
  - `DESIGN-SPEC.md` / `DESIGN-SPEC-PROGRESS.md`
  - `SESSION-INDEX.md`
  - `DELIVERY-PLAN.md`
- 新增：
  - `docs/README.md`
  - `docs/MIGRATION-NOTES.md`
  - `session-memory.config.json`
- 未复制旧 backend 专用文档（architecture/acceptance/android/deprecated/scheduler/旧 workflow/旧 layout 等）；它们仍在 `D:\OAS-dev\backend\docs`，仅历史参考。

### skill / 入口适配

- `session-memory.config.json` 指向 `docs/` 下的 spec/progress/index/ledger/resume。
- `START-HERE.md`：
  - 工作目录改为 `D:\OAS-dev\oas-core`；
  - 文档通道从 backend 文档仓改为 oas-core docs；
  - 报告模板的“后端文档仓 commit”改为“oas-core docs commit”；
  - 旧仓历史路径标为冻结参考；
  - 标注 `D:\OAS-dev\backend\toolkit` 只是外部开发工具包，不是 oas-core 构建依赖。

## 2. 下一步

1. 提交 `docs/` + `session-memory.config.json` 到 oas-core `dev` 并 push；
2. 更新 `docs/START-HERE.md` 的文档 commit 字段（本提交）；
3. 后续新会话固定：
   - 工作目录 `D:\OAS-dev\oas-core`
   - 先 `skill session-memory`
   - 读 `docs/START-HERE.md`
4. 按 `PENDING-LEDGER.md` 继续实际开发任务。

## 3. 约束

- `main` 保持 `385d611` 恢复节点；文档提交只在 `dev`，不自动合并/发布。
- 不再向 `D:\OAS-dev\backend` 写文档。
- 旧 backend 文档中的旧 workflow/layout/architecture 不复制、不再作为现行口径。

## 4. 证据

- `docs/README.md`、`docs/MIGRATION-NOTES.md`
- 源提交：旧仓 `2861d8ba`
- 测试：350 passed；lint：4/0
- 文档迁移 commit：`4b4795d`（后续记录提交在 dev）
