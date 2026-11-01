# RESUME — 断点续接卡（session-memory 强制 6）

> 规则：本文件**每次断点覆盖重写**；≤150 行；另一个会话只读本文件 + `RESUME-INDEX.md` 最新行
> 就能继续。细节放 `ENGINE-DESIGN.md` / `ASSET-DATA-MODEL.md` / `RELEASE-NODE-2026-11-02.md`。

```yaml
task_id: oas-core-release-node-0.1.0
checkpoint_seq: 86
updated_at: 2026-11-02T01:05:00+08:00
trigger: 文档归档 + C0 现行文档更新 + 0.1.0 恢复节点索引
status: in_progress
```

## 0. 当前状态（磁盘 + git 实测，2026-11-02 00:58 后）

- 分支 `dev`，HEAD **`cd0ac34`**（`data: final recovery-node data snapshot`，00:58:21）；`origin/dev` = `7a3869f`（本地领先 1）。
- `main`/`origin/main` = **`385d611`**（v1.2.3，发布前恢复点）；用户 00:48 已要求构建 0.1.0 release 后同步 main/dev；本地包已构建，**tag/Release 与 main 同步尚未执行**。
- 测试：`cargo test --workspace` = **393 passed / 0 failed**；UI pure test = **203 checks / 0 dead ids**；task lint = **1/0**（仅 `返回庭院`）。
  - 2026-11-02 01:00 复核实测：`cargo test --workspace` 12 个有测试 suite 合计 393；`node apps/oasd/ui/ui_pure_test.js` 203 checks；`oasd task lint --tasks-dir tasks --all` → `linted=1 errors=0`。
- 数据：全局素材 **16 条 / 1 图**（15 `ocr_line` + 1 `template`）；帧 **28 个 / `current_id=20` / `next_id=41`**；任务只有 `返回庭院`（flow 11 steps / meta 400 B）；`tasks/测试` 已删除并随 `cd0ac34` 提交。
- 工作区：本次文档维护的 `docs/**` 修改 + `session-memory.config.json` 修改 **未提交**（子进程不 commit）；另有 `_verify/audit-20261101/{F-docs-audit,G-repo-hygiene}.md` 与 `B-cargo-test.log` 未跟踪（父进程决定是否入库）。
- 发布：`Cargo.toml` = `0.1.0`，`oasd --version` = `0.1.0`；**没有新的 tag/Release**；本地 0.1.0 包已构建（`dist/oasd-v0.1.0-win64.zip`，01:01；SHA256 见同名 `.sha256` 旁文件）。
- 文档：7 份历史文档已 `git mv` 到 `docs/archive/`；顶层保留 10 份 + 新增 `RELEASE-NODE-2026-11-02.md`；`session-memory.config.json` 的 `spec`/`progress`/`index` 指向 `archive/...`。

## 1. 验收清单（本次文档维护 + 恢复节点）

- ☑ 7 份历史文档归档（`git mv`，未删除，git 历史保留）：DESIGN-SPEC、DESIGN-SPEC-PROGRESS、SESSION-INDEX、DELIVERY-PLAN、RECOVERY-AUDIT、CODE-AUDIT-2026-11-01、MIGRATION-NOTES。
- ☑ `session-memory.config.json` 的 `spec`/`progress`/`index` = `archive/...`。
- ☑ 现行文档事实同步：HEAD `cd0ac34` / 393 / 203 / lint 1/0 / 16 素材 1 图 / 28 帧 current 20 / main 待同步 / version 0.1.0。
- ☑ `EDITOR-UX-PLAN` 三处矛盾修正 + 实现前证据标历史；`ENGINE-DESIGN §22.34` 定点修正；`PENDING-LEDGER` 追加/标注；`RESUME-INDEX` 追加 84~86。
- ☑ `RELEASE-NODE-2026-11-02.md` 建立（本地包与 SHA256 已回填；tag/Release 待填）。
- ☐ 父进程 review + 提交 docs/config 到 `dev`（子进程按指令未 commit/push）。
- ☐ 0.1.0 release 收尾：本地包已构建、stage 已确认 16 个全局素材；**tag/Release + 同步 `main`/`dev` 待完成**。
- ☐ 产品剩余开口（不阻塞文档节点，发布前确认是否接受）：collection 真实点击、drag step shape、SelectionTool。

## 2. 本次已做（2026-11-02 01:0x）

1. 归档 7 份历史文档到 `docs/archive/`（`git mv`），新增 `docs/archive/README.md`。
2. 更新 `README.md`、`START-HERE.md`、`RESUME.md`（本文件）、`SESSION-HANDOFF.md`、`ASSET-DATA-MODEL.md`。
3. `EDITOR-UX-PLAN.md`：`action_roi` 兼容段、`action_source` 待确认、版本待确认三处矛盾修正；§13 标“实现前快照”；新增 §15 实现后状态与 00:37~00:38 口径。
4. `ENGINE-DESIGN.md §22.34`：标题改“V1.0 已交付”；任务包 schema 改为 `tasks/<id>/{meta,flow}` + 全局 `assets/`；N36/N37 标已交付；DELIVERY-PLAN 链接改 archive。
5. `PENDING-LEDGER.md`：顶部归档说明；E-66/68/70/72 状态标注；新增“验收补记（00:37~00:38）”与 §E-73 文档归档/恢复节点记录（append-only，未改旧证据正文）。
6. `RESUME-INDEX.md`：追加 seq 84/85/86（旧行未改）。
7. `AUDIT-SUBPROCESS-2026-11-01.md`：顶部范围横幅；任务数/真实设备/`CODE-AUDIT` 链接标历史。

## 3. 下一步精确动作

```powershell
# 0) 父进程先看工作区（子进程未 commit）
cd D:\OAS-dev\oas-core
git status --short
git diff --stat -- docs session-memory.config.json

# 1) 审阅后提交文档（父进程执行）
git add docs session-memory.config.json
git commit -m "docs: archive historical docs and sync 0.1.0 recovery-node state"

# 2) 发布包已由父进程构建（01:01）：dist/oasd-v0.1.0-win64.zip + .sha256
#    复核 scripts/package.ps1 与 packaging 版本 = 0.1.0；dist/stage/assets/assets.json = 16 条

# 3) 用户确认发布后：tag/release v0.1.0，main 快进/同步到恢复节点，dev 同步推送
```

- 继续产品工作的入口：`apps/oasd/src/serve.rs` 的 `handle_collection_match`（collection 真实点击）、`apps/oasd/ui/app.js` 的 drag step 生成（当前仍 bbox）、`SelectionTool`（两套几何未合并）。
- 发布前复核：`_verify/audit-20261101/D-e2e.md` 的执行范围是 `4d5759f`，**未覆盖** 23:39 点击分布/反选/真实开关与 00:02 RO 修复；不要把 E2E 36/36 当最终全量状态。

## 4. 已知开口 / 边界

1. `collection` 集合测试仍是 **dry-run**：`handle_collection_match` 只返回 `action_box`，没有 `/input/tap`；它和“素材行真实点击”不是同一条路径（session 00:38）。
2. drag 生成的 flow step 仍是 **bbox region**（CODE-AUDIT P1-1 未修）；`AUDIT-SUBPROCESS` 的 P1-1 是另一件事（UI 模板 shape 未平移，已修）——引用时写明编号来源。
3. `SelectionTool` 未抽出；`polygon`/`actionPoly` 两套状态按语义保留。
4. 真实 UI 手势：用户 00:37 说已测试；助手独立证据只有真机 `/input/tap`（`emulator-5554`），不是完整 UI 手势回放。文档区分“用户验收”与“助手实测”。
5. 数据：当前 16/1；34/18 是 11-01 错误恢复（§E-70 推翻）；archive 2827/2099 未导入（用户 21:02 “先不管素材，素材可以重建”）。

## 5. 历史事实（只读，不再指导操作）

- 4:21 干净基线：`f6be0f0`（22 帧 / 返回庭院 3 素材 / 无全局 assets）；18:42~19:14 为 docs-only；19:30 `9618cb5` 起进入实现。
- 11-01 09:02 用户要求回滚，09:11 执行；archive 分支 `archive/migration-20261101 = 29b696f`（2827 素材 / 2099 图 / 57 个任务目录，docs 旧写 55 需按 git 为准）。
- 34 素材事故的完整审计已归档：`docs/archive/RECOVERY-AUDIT.md`；首轮代码审计：`docs/archive/CODE-AUDIT-2026-11-01.md`。

## 6. 证据

- 本次审计：`_verify/audit-20261101/F-docs-audit.md`（37 条 C0）、`G-repo-hygiene.md`（仓库卫生）。
- 已入库审计：`_verify/audit-20261101/{A-session-git,B-runtime,C-ui,D-e2e,E-acceptance}.md`。
- session 导出：`D:\OAS-dev\_verify\n44\session-current-both-full.txt`（仓外证据，不是 `oas-core/_verify`）。
- 备份：`D:\OAS-dev\_backups\oas-core-20261102-005826\`（docs 已备份）。
- 发布节点：`docs/RELEASE-NODE-2026-11-02.md`。

## 7. 阻塞

- 无技术阻塞。发布（tag/Release/同步 main）需要用户确认，且先修 scripts/packaging 版本号——不在本次 docs 子进程权限内。
