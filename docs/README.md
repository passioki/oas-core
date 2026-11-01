# OAS-CORE 文档入口

> 本目录是 **oas-core 的独立项目文档**，从旧 backend 文档仓迁移而来。
> 旧仓库 `D:\OAS-dev\backend` 已冻结，仅作为资产/历史资源；oas-core 的构建、运行、测试、发布不依赖它。

## 当前有效文档（顶层 10 份 + 发布节点）

| 文件 | 用途 |
|---|---|
| `START-HERE.md` | ★ 新会话唯一入口 |
| `RESUME.md` | 断点续接卡（每次覆盖重写） |
| `RESUME-INDEX.md` | 断点索引（只追加） |
| `SESSION-HANDOFF.md` | 一页交接：当前状态/下一步/坑 |
| `RELEASE-NODE-2026-11-02.md` | ★ 当前发布/恢复节点索引：状态、关键 commit、测试、已知开口、release 产物（待填） |
| `ENGINE-DESIGN.md` | 引擎规格（§22 为引擎交付/设计；§22.34 调度 v3 已交付） |
| `PENDING-LEDGER.md` | 未完成台账（当前看 §E-66~§E-73） |
| `EDITOR-UX-PLAN.md` | 编辑器 UX/选择模型/实时帧/区域有效性改版（含实现后状态 §15） |
| `ASSET-DATA-MODEL.md` | 全局 `assets/`、任务素材=flow 引用子集、旧 `library/` 兼容边界 |
| `AUDIT-SUBPROCESS-2026-11-01.md` | 子进程独立复核（范围 21:50 / 361 tests；当前状态见顶部横幅与 §10） |

> **历史文档已归档**到 `docs/archive/`（7 份，`git mv`，未删除）：
> `DESIGN-SPEC.md`、`DESIGN-SPEC-PROGRESS.md`、`SESSION-INDEX.md`、`DELIVERY-PLAN.md`、
> `RECOVERY-AUDIT.md`、`CODE-AUDIT-2026-11-01.md`、`MIGRATION-NOTES.md`。
> 归档原因/替代文档见 `archive/README.md`；`session-memory.config.json` 的
> `spec`/`progress`/`index` 已改为 `archive/...`（只读历史输入，不是现行设计权威）。

## 当前基线（2026-11-02 01:12，tag `v0.1.0`）

- 版本：`0.1.0`（`oasd --version` = 0.1.0）；`dev` = `main` = tag `v0.1.0` target；`origin/dev`/`origin/main` 已同步。
- 包构建 commit：`42567c1`；本地 `dist/oasd-v0.1.0-win64.zip`，SHA256 `E6112A33…5F5687`；**未创建 GitHub Release**（只构建本地包 + tag）。
- 当前任务：只有 `返回庭院`（flow 11 steps / meta 400 B）；`tasks/测试` 已删除。帧 **28 个**，`current_id=20`，`next_id=41`；**唯一关系 = `Asset.positions`（帧-素材）**。
- 全局素材：**16 条 / 1 图**（15 `ocr_line` + 1 `template`；全部 `action_source=fixed`，有 `action_shape` 与 `positions`）。34/18 是 11-01 错误恢复（§E-70 已推翻）；`archive/migration-20261101` 的 2827 素材 / 2099 图 / 57 个任务目录是权威迁移归档，但未导入当前 dev（用户裁定素材可重建）。
- 测试：`cargo test --workspace` = **393 passed / 0 failed**；UI pure test = **203 checks / 0 dead ids**；task lint = **1/0**；E2E 36/36（证据边界：`4d5759f`，不含 23:39 点击分布/反选/真实开关与 00:02 RO 修复）。
- 编辑器首轮 + 验收修复：16:9 stage、帧详情侧栏、结果/日志收起、随机点测/拖测、`action_shape`+`action_source`、操作区多边形/全屏反选、runtime exact shape、素材拖动排序、帧分类重命名、live/position 批量测试、点击分布（只画点位/热力）、真实/演练开关、ResizeObserver 循环修复。
- 已回退：interface 抽象、`ui-map/`、`Frame.interface_id`、界面图视图（用户裁定不需要）。
- 帧安全：清空列表必须确认；清空前自动备份到 `frames/.backup/clear-<时间>/`（含 frames.json + PNG）。
- 已知开口：`collection` 集合测试仍是 dry-run（没有 `/input/tap`）；drag 生成的 flow step 仍是 bbox；`SelectionTool` 未抽出（识别掩膜/操作区两套几何按语义保留）。
- 本地备份：`D:\OAS-dev\_backups\oas-core-20261102-005826\`；历史 docs 归档 `docs/archive/`（7 份，`git mv`，未删）。

## 维护规则

1. 所有文档更新都在 **oas-core 仓库**完成，不再回写 backend 文档仓。
2. 文档随 `oas-core dev` 提交；0.1.0 恢复节点已同步 `main`/`dev` 并打 tag `v0.1.0`。
3. 新会话固定：
   - 工作目录 `D:\OAS-dev\oas-core`
   - 先加载 skill `session-memory`
   - 再读 `docs/START-HERE.md`
4. skill 配置：仓库根目录 `session-memory.config.json`（`spec`/`progress`/`index` 已指向 `docs/archive/`）。
5. 旧 backend 文档仍位于 `D:\OAS-dev\backend\docs`，仅供历史查证；其中的 `DEV-WORKFLOW.md`、`LAYOUT.md`、旧架构/验收文档已不迁入。

## 迁移来源

- 源仓库：`https://github.com/passioki/OnmyojiAutoScript.git`（旧 OAS backend）
- 源提交：`2861d8ba`（v1.2.3 发布文档节点）
- 迁移日期：2026-11-01
- 详细对照：`archive/MIGRATION-NOTES.md`（历史归档）
