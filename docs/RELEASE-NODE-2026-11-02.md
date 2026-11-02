# 关键恢复节点 / 发布索引 — 2026-11-02

> 状态：**0.1.0 恢复节点已发布**（tag `v0.1.0` + **GitHub Release 已上传** + 本地包 + 文档归档）；O-1/O-2 经两轮评审后在 `d1a7fcd` 收口（**不在 v0.1.0 包内**）。本文件是当前节点的一页索引；细节见 `RESUME.md` / `PENDING-LEDGER §E-73~§E-74`。
> HEAD：`dev` = `main` = `origin/dev` = `origin/main`（**已同步 origin**，2026-11-02 13:55 实测；最新 docs 提交 = HEAD；实质代码提交 `54c715b` + `d1a7fcd`，docs 终态订正提交 `906bbed`，其后为 docs 同步回填提交 `11d53a5`）；tag `v0.1.0` → `e8edbef`（**未移动**）；包构建 commit = `42567c1`。

## 1. 当前状态（实测）

| 项 | 值 |
|---|---|
| 版本 | `0.1.0`（`Cargo.toml` workspace；`oasd --version` = 0.1.0） |
| 分支 | `dev` = `main` = `origin/dev` = `origin/main`（**已同步 origin**，2026-11-02 13:55 实测；最新 docs 提交 = HEAD；实质代码提交 `54c715b` + `d1a7fcd`，docs 终态订正提交 `906bbed`，其后为 docs 同步回填提交 `11d53a5`）；tag `v0.1.0` → `e8edbef`（**未移动**）；包构建 commit = `42567c1` |
| 测试 | cargo **426 passed / 0 failed**；UI **259 checks / 0 dead ids**；task lint **1/0**（最终值；发布包仍是 393/203 时构建，见 §3） |
| 任务 | 只有 `返回庭院`（flow 11 steps / meta 400 B）；`tasks/测试` 已删除并提交 |
| 素材 | 全局 `assets/` = **16 条 / 1 图**（15 `ocr_line` + 1 `template`；全部 `action_source=fixed`；有 `action_shape`/`positions`） |
| 帧 | **28 个**，`current_id=20`，`next_id=41`；唯一关系 = `Asset.positions` |
| 工作区 | `dev` `d1a7fcd` 时 clean；本轮 docs 订正（8 份，终态，`906bbed`）与同步回填（`11d53a5`）均已提交；最新 docs 提交 = HEAD，仅 `docs/` |
| 发布 | tag `v0.1.0` 已推送且**未移动**（→ `e8edbef`）；**GitHub Release 已上传** `https://github.com/passioki/oas-core/releases/tag/v0.1.0`（release id `431080249`，draft=false / prerelease=false，创建后即 repo **latest**）；附件 `oasd-v0.1.0-win64.zip`（asset `700544587`，37,089,992 B，GitHub digest `sha256:e6112a33…5f5687`）+ `oasd-v0.1.0-win64.zip.sha256`（asset `700545142`，89 B）；包构建 commit 仍 `42567c1`。`dist/` 另有历史 v1.2.2 / v1.2.3 包 |

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
| `e8edbef` | docs：v0.1.0 恢复节点状态定稿（tag `v0.1.0` target，**tag 未移动**） |
| `54c715b` | `feat: collection test real click + drag action-shape (O-1/O-2)`（初版，6 files，+1413/−103） |
| `d1a7fcd` | `fix: address O-1/O-2 review findings (P1-1..P1-4, P2)`（评审修复，6 files，+1436/−225） |

> 更早的 v1.0~v1.2.1 交付记录见 `ENGINE-DESIGN §22.35~§22.39`；历史路线归档在 `docs/archive/DELIVERY-PLAN.md`。

## 3. 测试与证据边界

- cargo 393（12 个有测试 suite）；UI 203；lint 1/0：2026-11-02 01:00 复核实测（**发布包即此基线构建**）。
- **2026-11-02 13:35 最终复核（dev `d1a7fcd`，两轮评审之后）**：`cargo test --workspace` = **426 passed / 0 failed**（oasd 153 / flow 57 / runtime 125 + 2 integration）；`node ui_pure_test.js` = **259 checks / 0 dead ids**；`oasd task lint --tasks-dir tasks --all` = **1 task / 0 errors**；`cargo build -p oasd --features ocr` exit 0（6 条既有 warning）；serve 由 Lead 用最终构建以 `-SkipBuild` 重启于 **8787**。⚠ **v0.1.0 发布包（`42567c1` 构建）仍是 393/203 基线，不含 O-1/O-2**；O-1/O-2 自 `dev` `54c715b` 起，评审修复在 `d1a7fcd`。
- **DSL E2E（Lead 用临时 task lint）**：3 个合法变体 ok（polygon+invert / 旧 flow 无 shape / rect+invert）；5 个非法变体全部报错且路径精确（自交、坐标>1e6、负坐标 rect、point 上带 shape、点数<3）→ `linted=8 errors=5`。
- E2E 36/36：`_verify/audit-20261101/D-e2e.md`，执行于 23:03–23:05、HEAD `4d5759f`；**不含** 23:39 点击分布/反选/真实开关与 00:02 RO 修复。
- 真机证据：`emulator-5554` 的 `POST /input/tap`（`_verify/audit-20261101/E-acceptance.md`）；没有完整 UI 手势回放。
- 用户 00:37 验收：RO/真实模式“没问题”；与助手实测分开记录。

## 4. 已知开口（终态）

| # | 开口 | 影响 | 证据/位置 |
|---|---|---|---|
| 1 | `collection` 集合测试真实点击（O-1）：✅ **dev `d1a7fcd` 最终版**——`real` 严格只收 `0/1/true/false`；`real=1` 强制 live 帧；匹配通过后在当前帧精确 action shape 内采样（`ClickShape::sample` + `translate_click_shape`）；未过阈值 → **409** + `clicked:false`；缺省 dry-run 响应/日志逐字不变 | **不在 v0.1.0 包内**；无 live/越界/无 box → 400，adb 失败 → 500；**真机 E2E 未做**（MuMu 未运行） | `serve.rs:1266`（路由）· `3303-3534`（handler）· `5923-6001`（exact shape/采样）· `8574+`（+13 测试）· `app.js:2941-3074` |
| 2 | drag step 带 action shape（O-2）：✅ **dev `d1a7fcd` 最终版**——`from`/`to` region target 可选 `shape`；坐标 0..=1e6/i32、rect 边界 checked、顶点 3..=256、拒绝自交、i128 面积；point/ocr_text 带 shape → FlowError；plain rect 操作区保留；polygon/invert 走 `ClickShape::sample` | **不在 v0.1.0 包内**；**混版本**：旧 runtime 静默忽略 shape、按 bbox 采样（invert 会点到排除区）⇒ invert 拖动需本版 runtime；**P2-10**（保存 flow 不校验解析错误）本轮明确不做 | `oas-flow/src/lib.rs:626-654/1136-1431` · `oas-runtime/src/lib.rs:252-310/1733-1821/3035-3155` · `app.js:911-925` · `builder.js:1030-1060` |
| 3 | `SelectionTool` 未抽出（识别掩膜/操作区两套几何按语义保留） | 结构债，不影响当前行为 | AUDIT-SUBPROCESS P1-8 |
| 4 | `collection` `real=1` **未做真机 E2E** | 成功路径仅 `#[cfg(test)]` tap seam 覆盖；MuMu 当前未运行、`adb devices` 为空 | `serve.rs` test seam |
| 5 | 模板移动/多位置仍是「生成时 shape 快照」 | Plan B（asset 引用 + 运行时跟随识别）未做；多位置/移动后不跟随 | O-2 风险 |
| 6 | 活动页参考帧未补齐 | 帧归档仍缺活动页参考帧 | START-HERE §0.2 |
| 7 | 第二轮评审残留 P2（不阻断） | P2-12 显式闭合环/重复顶点 polygon 会被自交检查误判；P2-11 极端 `distance`（i32::MAX）在 debug 可溢出 panic（既有、非 shape 路径）；P2-13~16 注释/UI note 措辞与少量 `as i32` 截断；UI 接线测试仍静态断言 | `D:\OAS-dev\_verify\final-review-20261102\REVIEW2.md` |

## 5. Release 产物（已回填）

> 2026-11-02 13:5x 回填（终态）：GitHub Release 已上传，tag 未移动；下表为实测值。发布证据：`D:\OAS-dev\_verify\release-v0.1.0\REPORT.md`。

| 项 | 值 |
|---|---|
| tag | `v0.1.0`（annotated tag `cf123b8` → commit `e8edbef`；已推送且**未移动**） |
| Release URL | `https://github.com/passioki/oas-core/releases/tag/v0.1.0`（release id `431080249`；draft=false / prerelease=false；创建后即 repo **latest**） |
| zip 文件名 | `oasd-v0.1.0-win64.zip`（asset id `700544587`，37,089,992 B）+ `oasd-v0.1.0-win64.zip.sha256`（asset id `700545142`，89 B）；本地同物在 `dist/` |
| zip SHA256 | `E6112A33BE78D00850B4F1D0D5AEAC8E0A8172130231BB87145B43A0595F5687`（GitHub digest `sha256:e6112a33…5f5687`；zip 与旁文件、页面三方一致） |
| 构建时间/机器 | 2026-11-02 01:08 / 本机（`dist/stage/BUILD-INFO.txt`：git `42567c1`、built_at 01:08:05、tasks=返回庭院） |
| main 同步 commit | `dev` = `main` = `origin/dev` = `origin/main`（已同步 origin，2026-11-02 13:55 实测；最新 docs 提交 = HEAD；docs 终态订正提交 `906bbed`，父 `d1a7fcd`；其后为 docs 同步回填提交 `11d53a5`） |

> 以上为 2026-11-02 01:08 本地构建实测（`dist/stage/BUILD-INFO.txt`：git `42567c1`、built_at 01:08:05、tasks=返回庭院）。tag 指向 docs-only 更新后的同一代码树；以 `dist/*.sha256` 旁文件为准。

发布执行记录（回填）：

1. ✅ `scripts/package.ps1` 与 `packaging/*` 版本 = `0.1.0`（已随 `42567c1` 提交）。
2. ✅ 发布前基线 `cargo test --workspace` 393 / UI 203 / lint 1-0（**发布包就是此基线构建**；O-1/O-2 之后的最终 426/259 见 §3）。
3. ✅ 本地构建 01:08（E611…5687）；`dist/stage/assets/assets.json` = 16 条；`packaging/start-editor.bat` 版本 = 0.1.0。
4. ✅ tag `v0.1.0`（→ `e8edbef`，未移动）；**GitHub Release 已上传**（`431080249`，draft=false / prerelease=false，即 latest）；**`dev` / `main` 已同步 origin**（`dev` = `main` = `origin/dev` = `origin/main`，2026-11-02 13:55 实测，最新 docs 提交 = HEAD；见 §7）。
5. ✅ 本表已回填；`RESUME-INDEX` seq 89 标 done（seq 88 为中间态，保留）。

## 6. 恢复与备份

- 本地备份：`D:\OAS-dev\_backups\oas-core-20261102-005826\`（docs 等）。
- 历史 docs 归档：`docs/archive/`（7 份，`git mv`，未删除）。
- 当前数据快照：`cd0ac34`；0.1.0 tag 已创建；工作区 clean（dev `d1a7fcd`）。
- **不要恢复 34/18**：那是已被 `§E-70` 推翻的错误恢复；archive 2827/2099 保留备查但未导入。

## 7. 回填（2026-11-02 13:5x，终态）

- **GitHub Release**：`https://github.com/passioki/oas-core/releases/tag/v0.1.0`，release id `431080249`，标题 `OAS 0.1.0 — 恢复节点（引擎/编辑器/全局素材库）`，draft=false / prerelease=false，创建后即 repo latest。
- **附件**（均 state=uploaded）：`oasd-v0.1.0-win64.zip`（id `700544587`，37,089,992 B，GitHub digest `sha256:e6112a33…5f5687`）；`oasd-v0.1.0-win64.zip.sha256`（id `700545142`，89 B）。
- **tag 未移动**：annotated tag `cf123b8` → commit `e8edbef`；包构建 commit 仍是 `42567c1`。
- **验证证据**：`D:\OAS-dev\_verify\release-v0.1.0\REPORT.md`（发布前 404 → 创建 201 → body 逐字符一致 → zip/旁文件/页面 SHA256 三方一致 → 页面 200 → rate limit 正常）。
- **同步状态**：`dev` = `main` = `origin/dev` = `origin/main`（**已同步 origin**，2026-11-02 13:55 实测；最新 docs 提交 = HEAD；实质代码提交 `54c715b` + `d1a7fcd`，docs 终态订正提交 `906bbed`，其后为 docs 同步回填提交 `11d53a5`）；tag `v0.1.0` → `e8edbef`（未移动）；包构建 commit `42567c1`。
- **最终验证（13:35）**：workspace **426/0**；oasd 153 / flow 57 / runtime 125（+2 integration = 127）；UI **259/0 dead ids**；lint **1/0**；`cargo build -p oasd --features ocr` exit 0；DSL E2E `linted=8 errors=5`。
- **⚠ 包内容边界**：v0.1.0 发布包**不含 O-1（collection 真实点击）/ O-2（drag action shape）**；两者自 `dev` `54c715b` 起，评审修复在 `d1a7fcd`。
- **本轮订正**：8 份 docs 由中间态（`54c715b` / 410 / 245）订正到最终事实（`d1a7fcd` / 426 / 259）；证据 `_verify/docs-backfill-20261102/CORRECTIONS-REPORT.md`。
