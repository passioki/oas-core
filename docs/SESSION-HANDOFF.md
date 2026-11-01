# 会话交接单（SESSION HANDOFF）★ 一页纸

> ★★★ 新会话先读 `docs/START-HERE.md` → `docs/RESUME.md` → `docs/RESUME-INDEX.md` 最新行。
> 当前是 **0.1.0 恢复节点完成态**：数据/文档已快照，tag `v0.1.0` 已创建，main/dev 已同步，本地包已构建。不要再灌 34/18 旧素材。

## 0. 当前状态（2026-11-02 01:12）

| 项 | 值 |
|---|---|
| 分支/HEAD | `dev` = `main` = **tag `v0.1.0` target**；origin 已同步；包构建 commit `42567c1` |
| 恢复节点 | tag `v0.1.0`（本地 release 构建 + main/dev 同步）；GitHub Release 未创建 |
| 版本 | `0.1.0`；tag `v0.1.0`；本地包 SHA256 `E6112A33…5F5687`；无 GitHub Release |
| 测试 | `cargo test --workspace` = **393 passed / 0 failed**；UI pure test = **203 checks / 0 dead ids**；task lint = **1/0** |
| 数据 | 全局素材 **16 条 / 1 图**；帧 **28 个 / current_id=20 / next_id=41** |
| 任务 | 只有 `返回庭院`（flow 11 steps / meta 400 B）；`tasks/测试` 已删除（随 `cd0ac34` 提交） |
| 工作区 | **clean**；docs/config、packaging/scripts、F/G、B-cargo-test.log 均已提交 |
| 服务 | 开发 serve 8787 由当前会话按需启动；发布/清理前先确认停服 |
| 文档 | 7 份历史文档已归档 `docs/archive/`；顶层 10 份 + `RELEASE-NODE-2026-11-02.md` |

## 1. 最近发生了什么

- `7a3869f`：记录 ResizeObserver 循环修复（`a2b4b12`）；用户 00:37 验收“没问题”。
- `cd0ac34`：**最终恢复节点数据快照**（00:58）——16 素材（新增 `管理分组`）、frames current_id=20、删除 `tasks/测试`。
- 2026-11-02 01:0x：独立 docs 审计（F/G）+ 文档归档与 C0 更新（本文件所在批次，**未 commit**）。
- 用户 00:48 指令：构建 release，同步提交到 `main` 和 `dev`，作为关键恢复节点。

## 2. 下一步

```text
1. 如需 GitHub Release：对 tag v0.1.0 创建发布页（当前未创建）
2. 真机完整 UI 手势回归（若需要）→ 可补 collection 真实点击
3. drag step shape（扩 DSL 或明确只 support bbox）
4. SelectionTool 结构债
```

## 3. 关键事实（防止再记错）

- 当前素材是 **16/1**，不是 34/18；34 是 `§E-70` 已推翻的错误恢复，archive 2827/2099 未导入（用户裁定素材可重建）。
- 当前帧 **28 / current_id=20 / next_id=41**，不是 40。
- `tasks/测试` 已删除并提交；task lint 是 **1/0**，不是 2/0。
- `action_roi`/`roi_back` 的 **JSON/UI 读取已删除**；runtime 内部仍保留 `click_back` 作为 region/collection 的 bbox 快捷路径——不要写成“全仓无 roi”。
- **drag/swipe 不写点击分布**：`record_click()` 只在 `perform_click()` 路径调用。
- 拖测 `margin`：默认 **0.15**，合法 **0~0.45**，作用于起点（配置了 `to` 时也作用于终点）候选矩形内缩；不是像素/拖动距离。
- **collection 仍是集合匹配 dry-run**，没有真实 `/input/tap`；与“素材行真实点击”不是同一条路径。
- 真实 UI 手势：用户 00:37 说已测试；助手独立证据只有真机 `/input/tap`，两者要区分。
- E2E 36/36 的证据边界是 `4d5759f`（23:0x），不含 23:39 点击分布/反选/真实开关与 00:02 RO 修复。

## 4. 活跃的坑

- 不要在未确认前覆盖 `assets/`、`frames/`、`tasks/`；`assets/_deleted-tasks/` 是唯一副本安全网，绝不删。
- 不要用 task 历史再拼“全局库”；不要再跑 `scripts/recover_historical_assets.py`。
- 中文文档/代码用 `read`/`edit`/`write` 工具；PowerShell 直接重写 UTF-8 会 mojibake。
- `_verify/n44/...` 是仓外路径（`D:\OAS-dev\_verify\n44\...`），不是 `oas-core/_verify`。
- `ENGINE §22.39 R11`：唯一 `assets/`，任务只引用全局 id；`ASSET-DATA-MODEL.md` 是数据模型入口。
