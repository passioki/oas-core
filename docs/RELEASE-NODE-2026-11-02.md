# 关键恢复节点 / 发布索引 — 2026-11-02

> 状态：**0.1.0 本地恢复节点已发布（tag `v0.1.0`；本地包 + 文档归档；GitHub Release 未创建）**。本文件是当前节点的一页索引；细节见 `RESUME.md` / `PENDING-LEDGER §E-72~§E-73`。
> HEAD：`dev` / `main` = **tag `v0.1.0` target**；包构建 commit = `42567c1`（docs-only 之后是 tag 版本）。

## 1. 当前状态（实测）

| 项 | 值 |
|---|---|
| 版本 | `0.1.0`（`Cargo.toml` workspace；`oasd --version` = 0.1.0） |
| 分支 | `dev` = `main` = tag `v0.1.0` target；`origin/dev` / `origin/main` 已同步；包构建 commit = `42567c1` |
| 测试 | cargo **393 passed / 0 failed**；UI **203 checks / 0 dead ids**；task lint **1/0** |
| 任务 | 只有 `返回庭院`（flow 11 steps / meta 400 B）；`tasks/测试` 已删除并提交 |
| 素材 | 全局 `assets/` = **16 条 / 1 图**（15 `ocr_line` + 1 `template`；全部 `action_source=fixed`；有 `action_shape`/`positions`） |
| 帧 | **28 个**，`current_id=20`，`next_id=41`；唯一关系 = `Asset.positions` |
| 工作区 | docs/config 本次修改未提交；F/G 审计与 `B-cargo-test.log` 未跟踪；packaging/scripts 由父进程改 |
| 发布 | tag `v0.1.0` 已创建并推送；本地包 `dist/oasd-v0.1.0-win64.zip`（01:08，E611…5687）；main/dev 已同步；GitHub Release 未创建。`dist/` 另有历史 v1.2.2 / v1.2.3 包 |

## 2. 关键 commit（由旧到新）

| commit | 内容 |
|---|---|
| `385d611` | `main` 的 v1.2.3 恢复节点（tag `v1.2.3`） |
| `e749c10` | 提交用户 assets/frames/task 数据 |
| `a804848` | 点击分布/反选/真实测试开关 |
| `0cb6c7c` | 验收修复与真机 tap 的记录（docs） |
| `a2b4b12` | ResizeObserver 循环修复（rAF 合并/幂等 fit） |
| `7a3869f` | 记录 RO 修复（docs） |
| `cd0ac34` | 最终恢复节点数据快照（16 素材、`current_id=20`、删除 `tasks/测试`） |
| `42567c1` | 0.1.0 文档归档/scripts package assets 修复；本地包 BUILD-INFO 的 git 字段 |

> 更早的 v1.0~v1.2.1 交付记录见 `ENGINE-DESIGN §22.35~§22.39`；历史路线归档在 `docs/archive/DELIVERY-PLAN.md`。

## 3. 测试与证据边界

- cargo 393（12 个有测试 suite）；UI 203；lint 1/0：2026-11-02 01:00 复核实测。
- E2E 36/36：`_verify/audit-20261101/D-e2e.md`，执行于 23:03–23:05、HEAD `4d5759f`；**不含** 23:39 点击分布/反选/真实开关与 00:02 RO 修复。
- 真机证据：`emulator-5554` 的 `POST /input/tap`（`_verify/audit-20261101/E-acceptance.md`）；没有完整 UI 手势回放。
- 用户 00:37 验收：RO/真实模式“没问题”；与助手实测分开记录。

## 4. 已知开口

| # | 开口 | 影响 | 证据/位置 |
|---|---|---|---|
| 1 | `collection` 集合测试仍 dry-run（只返回 `action_box`，无 `/input/tap`） | 集合匹配不能真实点击 | `serve.rs handle_collection_match` |
| 2 | drag 生成的 flow step 仍是 bbox（CODE-AUDIT P1-1） | 多边形/反选拖动在 flow 中丢失 shape | `apps/oasd/ui/app.js` |
| 3 | `SelectionTool` 未抽出（两套几何按语义保留） | 结构债，不影响当前行为 | AUDIT-SUBPROCESS P1-8 |
| 4 | 打包脚本/部署说明版本已改为 0.1.0（工作区改动） | 发布前必须复核版本名/说明一致 | F C0 #35~#37（非 docs 范围） |
| 5 | ~~打包 stage assets 来源~~ | 已修复：根 `assets/` 原样复制，stage=16 items；见 `scripts/package.ps1` | F C1 #1（已关闭） |

## 5. Release 产物（发布流程回填）

> ⚠ 本文件不预写未生成产物；本地包由父进程构建。tag/Release/main 同步完成后回填下表。

| 项 | 值 |
|---|---|
| tag | `v0.1.0`（已推送） |
| Release URL | 未创建 GitHub Release（用户只要求本地 release 构建 + tag；本地包见 dist） |
| zip 文件名 | `oasd-v0.1.0-win64.zip`（`dist/`，本地构建） |
| zip SHA256 | `E6112A33BE78D00850B4F1D0D5AEAC8E0A8172130231BB87145B43A0595F5687` |
| 构建时间/机器 | 2026-11-02 01:08 / 本机 |
| main 同步 commit | 与 `dev` 同为 tag `v0.1.0` target |

> 以上为 2026-11-02 01:08 本地构建实测（`dist/stage/BUILD-INFO.txt`：git `42567c1`、built_at 01:08:05、tasks=返回庭院）。tag 指向 docs-only 更新后的同一代码树；以 `dist/*.sha256` 旁文件为准。

发布前置（建议顺序）：

1. 复核 `scripts/package.ps1` 与 `packaging/*` 的版本说明 = `0.1.0`（已在工作区改动；owner：scripts/packaging，非 docs）。
2. `cargo test --workspace` + `node apps/oasd/ui/ui_pure_test.js` + `oasd task lint --tasks-dir tasks --all` 全绿（当前 393 / 203 / 1-0）。
3. 本地构建已完成（01:08，E611…）；`dist/stage/assets/assets.json` = 16 条；`packaging/start-editor.bat` 版本=0.1.0。
4. 已执行：tag `v0.1.0`；`main`/`dev` 同步并推送；未创建 GitHub Release。
5. 回填本表；`RESUME-INDEX` seq87 标 done。

## 6. 恢复与备份

- 本地备份：`D:\OAS-dev\_backups\oas-core-20261102-005826\`（docs 等）。
- 历史 docs 归档：`docs/archive/`（7 份，`git mv`，未删除）。
- 当前数据快照：`cd0ac34`；0.1.0 tag 已创建；工作区 clean。
- **不要恢复 34/18**：那是已被 `§E-70` 推翻的错误恢复；archive 2827/2099 保留备查但未导入。
