# RESUME — 断点续接卡（session-memory 强制 6）

> 规则：本文件**每次断点覆盖重写**；≤150 行；另一个会话只读本文件 + `RESUME-INDEX.md` 最新行
> 就能继续。细节放 `ENGINE-DESIGN.md` / `ASSET-DATA-MODEL.md`。

```yaml
task_id: oas-core-repo-recovery
checkpoint_seq: 82
updated_at: 2026-11-01T22:25:00+08:00
trigger: 修复子进程完成 P0/P1；恢复 返回庭院 flow/meta；纳入审计报告
status: in_progress
```

## 0. 当前状态（磁盘 + git 实测）

- 分支 `dev`，HEAD `30d3145`；`main`/`origin/main` = `385d611`（v1.2.3）。
- 测试：`cargo test --workspace` = **381 passed / 0 failed**（runtime 110 / task 55 / oasd 131）；UI pure test 42 checks；task lint 1/0。
- 工作区**不干净**：
  - `frames/frames.json` modified（用户数据，未动）；
  - `tasks/返回庭院/assets.json` deleted（全局模式下任务不需要 task assets；flow/meta 已恢复）。
- 当前 `tasks/返回庭院`：flow.json + meta.json 已从 `8ee414c` 恢复（提交 `45c1f32`）；flow 使用内联 OCR，不引用 34 个全局素材，所以任务页显示“34 未引用提示 + 显示全部”。
- 当前 `assets/`：**34 个素材 / 18 张模板图**，来自按 task 历史恢复（`0bc69ba`/`1f73f17`），**不是 archive 迁移库**；待用户决定清理/恢复。
- 4:21 干净基线：`f6be0f0`（22 帧 / 返回庭院 3 素材 / 无全局 assets）；18:42~19:14 为 docs-only；19:30 `9618cb5` 起进入实现。详见 `RECOVERY-AUDIT.md`。
- **代码修复**：提交 `b910a2b`；独立审计 P0-1~P0-3、P1-1~P1-7 已修，P1-8 行为复位已修但双状态存储未合并；P2 `live` 强制重抓、OCR 识别 polygon 保存仍有限制。见 `AUDIT-SUBPROCESS-2026-11-01.md §6`。
- 真正的迁移归档在：
  - `archive/migration-20261101` = `29b696f`；
  - 2827 个素材 / 2099 张图 / 55 个任务包；
  - 已被 force-push 前的 rollback 保留，远程同名分支存在。

## 1. 已核实的历史事实

1. `ENGINE §22.39 R11` 用户原话：
   > “不再区分任务素材和全局素材，因为名称是唯一的”
   ⇒ 唯一 `assets/`；任务只持有 meta/flow 并引用全局 id。
2. `2026-11-01 09:02:04` 用户原话：
   > “回滚到恢复节点，请理解和计划，与我确认。当前的迁移全部撤销”
   09:11 执行后：
   - dev → `385d611`；
   - `assets/` 被删除；
   - `migration/` 删除；
   - 53 个迁移任务 + 2825 素材从 dev 移除；
   - 归档分支 `archive/migration-20261101 = 29b696f` 保留。
3. 之后用户继续在 dev 工作，并删除 `返回庭院` 任务；任务目录里的旧 assets 随之删除。
4. 本轮我们（助手）错误地：
   - 从 task 历史中恢复 34 个素材进 `assets/`；
   - 声称“全局素材已恢复”；
   - 删除旧 `library/` UI、添加自定义全局列表、修改多份文档；
   - 没有先读 archive 分支和 09:02 的 rollback 语境。

## 2. 设计 vs 现状

| 项 | 设计（用户口径） | 现状 | 结论 |
|---|---|---|---|
| 唯一素材库 | `assets/` 全量，任务引用子集 | 当前 `assets/` 只有 34 个按 task 历史拼出的素材 | ❌ 不是全量 |
| 迁移归档 | archive 分支保留 2827 素材/55 任务 | 未导入当前 dev | ❌ 未恢复 |
| 旧 `library/` | v1.2.1 后应被统一，最多兼容 | 我们曾恢复其 UI，后又删除 | ⚠ 需最终定案 |
| 任务删除 | 删除任务不应销毁全局素材 | 旧实现 remove_dir_all 会删任务目录 assets；已加备份 | ⚠ 现状待恢复后重验 |
| 任务素材 UI | 任务页只显示任务引用子集 | 无任务时应为空；代码已改但无真实任务验证 | 🔄 |

## 3. 下一步

已完成（2026-11-01 22:20）：

1. 代码修复 `b910a2b`：P0-1~P0-3、P1-1~P1-7 已修；P1-8 行为复位已修，双状态存储未合并；
2. `8ee414c` 的 `返回庭院` flow/meta 已恢复（`45c1f32`）；
3. 审计报告已纳入 git（`30d3145`）；
4. `cargo test --workspace` = 381 passed；UI pure test 42 checks；task lint 1/0。

剩余：

1. OCR 识别 polygon 后端保存限制；
2. `frame_for_test live` 强制重抓（P2）；
3. 素材恢复路线 A/B/C（见 `RECOVERY-AUDIT.md`）；
4. 真实浏览器点选/实机回归；25 个旧 DOM 死 id 清理（P3）。

## 4. 恢复路线（待裁定）

需要用户选一条恢复路线：

- A. 从 `archive/migration-20261101` 导入全量 `assets/`（2827）到当前 dev；是否也导入 55 个任务？
- B. 只保留当前 34 个素材，删除我们错误恢复的 docs/commit 影响；
- C. 回到 `1f73f17` 前/某个干净点，只保留编辑器代码改动，不恢复任何数据。

在执行前必须：

1. 备份当前 `assets/`、`tasks/`、`frames/`；
2. 明确 archive 的 schema（旧 `action_roi`）如何迁移成当前 `action_shape/action_source`；
3. 逐项验证 `task lint`、`cargo test`、全局 API 和 UI；
4. 经用户确认后才覆盖工作区。

## 5. 阻塞/待裁定

- 恢复哪一份素材库：archive 2827 / 当前 34 / 其他备份？
- 是否同时恢复 archive 的 55 个任务包？
- 当前工作区的 `tasks/返回庭院` 删除是否保留？
- 代码修复顺序是否按 `CODE-AUDIT-2026-11-01.md` 执行？

## 6. 证据

- session 导出：`_verify/n44/session-current-both-full.txt`
  - 11-01 09:02 回滚原话；
  - 09:11 删除 assets/migration；
  - R11 10-30 23:36 原话。
- `git branch -avv`：`archive/migration-20261101 = 29b696f`。
- `archive/migration-20261101:assets/assets.json`：2827 items。
- 当前 `assets/assets.json`：34 items。
- 提交：`1f73f17`（错误恢复+UI 改动）、`0bc69ba`、`1a021f8`、`385d611`。

> 结论：**当前仓库的数据状态不可信，恢复方案未定，禁止继续写代码/灌数据。**
