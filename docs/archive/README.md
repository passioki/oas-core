# docs/archive — 历史文档归档（2026-11-02）

> 本目录保存**不再指导当前操作**的历史文档；它们仍有证据/追溯价值。
> 归档方式：`git mv`（保留 git 历史）；**未删除**。返回：[../README.md](../README.md) · [../START-HERE.md](../START-HERE.md)。
> `session-memory.config.json` 的 `spec` / `progress` / `index` 已指向本目录；这些是**只读历史输入**，不是 oas-core 0.1.0 的现行设计权威。
> 归档前本地备份：`D:\OAS-dev\_backups\oas-core-20261102-005826\docs\`。

## 归档清单

| 文件 | 原用途 | 为什么不再现行 | 现行替代 |
|---|---|---|---|
| `DESIGN-SPEC.md` | 旧 backend/frontend 设计规格（10-06~10-12） | 文首已声明非现行；oas-core 引擎规格以 ENGINE-DESIGN 为准 | `../ENGINE-DESIGN.md` §22；`../ASSET-DATA-MODEL.md` |
| `DESIGN-SPEC-PROGRESS.md` | 旧规格覆盖台账（47 段） | 旧仓历史；与 0.1.0 无关 | 当前进度看 `../PENDING-LEDGER.md` §E-66~§E-73 |
| `SESSION-INDEX.md` | 旧 6287 条会话分类索引 | 停在 11-01 19:45；仅用于按主题查旧对话 | 当前断点看 `../RESUME.md` / `../RESUME-INDEX.md` |
| `DELIVERY-PLAN.md` | V1.0 交付路线/DoD（M0~M5） | V1.0~V1.2.1 已交付、版本已 reset 0.1.0 | `../RELEASE-NODE-2026-11-02.md` |
| `RECOVERY-AUDIT.md` | 11-01 数据恢复审计（34 素材事故） | 当前 16/1；用户已决定不恢复 archive 全量 | `../ASSET-DATA-MODEL.md` §5；`../PENDING-LEDGER.md` §E-70 |
| `CODE-AUDIT-2026-11-01.md` | 首轮 16:00~21:00 代码审计 | 已被 AUDIT-SUBPROCESS 取代；P1 编号与后续冲突 | `../AUDIT-SUBPROCESS-2026-11-01.md` |
| `MIGRATION-NOTES.md` | 旧 backend → oas-core 文档迁移记录 | 一次性迁移已完成 | `../README.md` |

## 使用注意

1. 归档文档内的相对链接按**本目录**解析（7 份原为同目录同级文档，迁入后仍互为同级）。文中出现的 `DESIGN-SPEC.md`/`SESSION-INDEX.md` 等即本目录文件。
2. 历史文档中的 HEAD、测试数、素材数（如 34/18、2/0、current_id=40）是**当时快照**，不是当前事实；当前基线见 `../RELEASE-NODE-2026-11-02.md`。
3. 2026-11-02 归档时**未改正文**（保留原始证据）；`PENDING-LEDGER.md` / `RESUME-INDEX.md` 里指向这些文件的旧引用按历史说明理解。
4. 如需恢复为顶层文档：`git mv docs/archive/<name>.md docs/<name>.md`，并同步 `session-memory.config.json` 与各入口链接。
