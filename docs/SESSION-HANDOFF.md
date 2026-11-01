# 会话交接单（SESSION HANDOFF）★ 一页纸

> ★★★ 新会话先读 `docs/START-HERE.md` → `docs/RESUME.md` → `docs/RESUME-INDEX.md` 最新行。
> 当前处于**数据恢复审计阻塞态**，不要继续灌素材/改数据。

## 0. 当前状态（2026-11-01 20:50）

| 项 | 值 |
|---|---|
| 分支/HEAD | `dev` / `1f73f17` |
| 恢复节点 | `main` = `385d611`（v1.2.3） |
| 测试 | `cargo test --workspace` = **358 passed / 0 failed** |
| 工作区 | **不干净**：`frames/frames.json` 改动；`tasks/返回庭院` 删除未提交 |
| 任务 | `tasks/` 空（用户删除 `返回庭院`） |
| 当前 `assets/` | **34 个素材 / 18 图（助手错误恢复，非权威）** |
| 迁移归档 | `archive/migration-20261101 = 29b696f`：2827 素材 / 2099 图 / 55 任务 |
| 版本 | dev Cargo `0.1.0` |
| 服务 | 8787 可能仍运行 358 构建；恢复前先停 |

## 1. 关键事实

- 2026-11-01 16:21:33 `f6be0f0` = 用户所说 4:21 干净版本：22 帧 / 返回庭院 3 素材 / 无全局 `assets/`。
- 18:42~19:14 的 4 个提交均为 **docs-only**；19:30:51 `9618cb5` 才是第一笔实现提交。
- 2026-11-01 09:02 用户要求回滚，09:11 执行：dev `29b696f → 385d611`，删除 `assets/`、`migration/`、53 个迁移任务、2825 素材。
- 归档分支保留迁移历史，是“大量素材”的权威来源。
- 当前 34 个素材是助手从 `tasks/*/assets.json` git 历史拼出的，不是 archive 全量，已被写入 `assets/` 并提交（`0bc69ba`/`1f73f17`）。
- 助手还错误地恢复过旧 `/library` UI、加自定义全局列表，随后又删 UI，造成仓库/文档混乱。
- 详细审计：`docs/RECOVERY-AUDIT.md`。

## 2. 阻塞（等待用户裁定）

1. 恢复 `archive/migration-20261101` 的 2827 素材到当前 dev？
2. 是否同时恢复 55 个 archive 任务？
3. 当前 `tasks/` 删除、`frames/frames.json` 改动是否保留？
4. 旧 `library/` 最终形态。

## 3. 下一步（用户确认后才做）

```text
1. 备份 assets/ tasks/ frames/
2. 从 archive 导入前先写 schema 迁移（action_roi → action_shape/action_source）
3. 按用户选择恢复素材/任务
4. task lint + cargo test + 全局 API + UI 验证
```

## 4. 活跃的坑

- 不要在未确认前覆盖任何数据。
- 不要再用 task 历史拼“全局库”。
- `ENGINE §22.39 R11`：唯一 `assets/`，任务只引用全局 id。
- `_verify/n44/session-current-both-full.txt` 有 09:02 回滚原话与 09:11 删除证据。
