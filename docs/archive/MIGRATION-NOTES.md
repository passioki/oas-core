# DOCS MIGRATION NOTES

## 来源

- 旧仓库：`https://github.com/passioki/OnmyojiAutoScript.git`
- 本地旧仓：`D:\OAS-dev\backend`
- 源提交：`2861d8ba`（`docs(release): v1.2.3 published; U-32b closed...`）
- 迁移时间：2026-11-01

## 迁入 oas-core 的文档

- `START-HERE.md`
- `RESUME.md`
- `RESUME-INDEX.md`
- `SESSION-HANDOFF.md`
- `ENGINE-DESIGN.md`
- `PENDING-LEDGER.md`
- `DESIGN-SPEC.md`
- `DESIGN-SPEC-PROGRESS.md`
- `SESSION-INDEX.md`
- `DELIVERY-PLAN.md`

## 未迁入（旧 backend/旧前端资料，仍在旧仓）

- `acceptance-checklist.md`
- `android-feasibility.md`
- `architecture.md`
- `ARCH-REVIEW.md`
- `deprecated.md`
- `DEV-WORKFLOW.md`
- `GRIND-REDUCTION.md`
- `LAYOUT.md`
- `scheduler-architecture.md`
- `SESSION-LEDGER.md`
- `TODO-NEXT.md`
- `team-coordination.md`
- `ui-api-mapping.md`
- `oasx-task-list-ui.patch`
- `task-list-prototype.html`

这些是旧实现/旧布局/旧验收资料；需要历史查证时直接只读旧仓，不混入 oas-core 现行文档。

## 适配内容

- 新增 `session-memory.config.json`（仓库根目录）。
- 新增 `docs/README.md`。
- `START-HERE.md`：
  - 工作目录改为 `D:\OAS-dev\oas-core`；
  - 文档通道从 backend 文档仓改为 oas-core docs；
  - 报告模板中的文档 commit 字段改为 oas-core docs commit；
  - 旧仓历史链接保留，标注为冻结参考；
  - backend toolkit（ADB/OCR 模型）标注为外部开发依赖，不是 oas-core 构建依赖。
- `RESUME.md` / `RESUME-INDEX.md`：建立本次文档迁移断点。
- `PENDING-LEDGER.md`：记录文档迁移为独立事项。
- `SESSION-INDEX.md`：追加文档迁移记录。

## 已知遗留

- 文档中的 `D:\OAS-dev\backend\toolkit\...` 是当前开发机的 ADB/OCR 模型外部工具包路径；oas-core 源码不依赖 backend，但本机跑 OCR/实机验证时仍需该路径。后续可把 toolkit/模型下沉到 oas-core 自带 `tools/` 或环境变量。
- 旧 backend 文档仍可能包含过时的路径/流程；oas-core 现行口径以本目录和 `START-HERE.md` 为准。
