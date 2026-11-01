# OAS-CORE 文档入口

> 本目录是 **oas-core 的独立项目文档**，从旧 backend 文档仓迁移而来。
> 旧仓库 `D:\OAS-dev\backend` 已冻结，仅作为资产/历史资源；oas-core 的构建、运行、测试、发布不依赖它。

## 当前有效文档

| 文件 | 用途 |
|---|---|
| `START-HERE.md` | ★ 新会话唯一入口 |
| `RESUME.md` | 断点续接卡（每次覆盖重写） |
| `RESUME-INDEX.md` | 断点索引（只追加） |
| `SESSION-HANDOFF.md` | 一页交接：当前状态/下一步/坑 |
| `ENGINE-DESIGN.md` | 引擎规格（含 §22，调度 v3 在 §22.34） |
| `PENDING-LEDGER.md` | 未完成台账 |
| `DESIGN-SPEC.md` | 设计规格书 |
| `DESIGN-SPEC-PROGRESS.md` | 设计规格覆盖台账 |
| `SESSION-INDEX.md` | 会话分类索引 |
| `DELIVERY-PLAN.md` | V1.0 交付路线/DoD |
| `EDITOR-UX-PLAN.md` | 编辑器 UX/选择模型/实时帧/区域有效性改版计划 |
| `ASSET-DATA-MODEL.md` | 全局 `assets/`、任务素材=flow 引用子集、旧 `library/` 兼容边界 |
| `RECOVERY-AUDIT.md` | session↔git↔代码恢复审计；4:21 基线 / 19:30 实现起点 / archive 2827 |
| `CODE-AUDIT-2026-11-01.md` | 16:00~21:00 代码审计：4 P0 / 4 P1，尚未修复 |

## 当前基线（2026-11-01）

- 版本：dev `0.1.0`；`main` 的 `385d611`（v1.2.3）只作为历史恢复点，旧 v1.x tag 保留但不代表可用版本。
- 当前任务：`tasks/返回庭院`；帧 22 个；**唯一关系 = `Asset.positions`（帧-素材）**。
- 测试：`cargo test --workspace` = **361 passed / 0 failed**。
- 已实现：`key`/`restart_app` 前端、两阶段截图、Enter、OCR 缩略图只用位置帧、位置帧可从其他素材继承、帧行关联素材数、只读帧-素材关系浏览器、transition 录制、人工标注、覆盖概览、任务构建关系提示。
- 编辑器首轮：16:9 stage、帧详情侧栏、结果/日志收起、随机点测/拖测、`action_shape`+`action_source`、操作区多边形/全屏反选、runtime exact shape、素材拖动排序、帧分类重命名、live/position 批量测试。
- 已回退：interface 抽象、`ui-map/`、`Frame.interface_id`、界面图视图（用户裁定不需要）。
- 帧安全：清空列表必须确认；清空前自动备份到 `frames/.backup/clear-<时间>/`；31~34 新录制帧已在一次误清空中丢失（不可恢复），22 个 git tracked 帧已恢复。

## 维护规则

1. 所有文档更新都在 **oas-core 仓库**完成，不再回写 backend 文档仓。
2. 文档随 `oas-core dev` 提交；`main` 保持 v1.2.3 恢复节点 `385d611`，直到下一次发布。
3. 新会话固定：
   - 工作目录 `D:\OAS-dev\oas-core`
   - 先加载 skill `session-memory`
   - 再读 `docs/START-HERE.md`
4. skill 配置：仓库根目录 `session-memory.config.json`。
5. 旧 backend 文档仍位于 `D:\OAS-dev\backend\docs`，仅供历史查证；其中的 `DEV-WORKFLOW.md`、`LAYOUT.md`、旧架构/验收文档已不迁入。

## 迁移来源

- 源仓库：`https://github.com/passioki/OnmyojiAutoScript.git`（旧 OAS backend）
- 源提交：`2861d8ba`（v1.2.3 发布文档节点）
- 迁移日期：2026-11-01
- 详细对照：`MIGRATION-NOTES.md`
