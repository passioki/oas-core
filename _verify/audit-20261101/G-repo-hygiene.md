# G — 仓库卫生审计（只读）· 关键恢复节点前的审慎清理清单

- 审计对象：`D:\OAS-dev\oas-core`
- 快照：HEAD `7a3869f`（`dev`）；`main`/`origin/main` = `385d611`（v1.2.3 恢复节点）
- 审计时刻：2026-11-02 00:49 ~ 00:58（Asia/Shanghai）
- 口径：**只读**。本次仅新增本文件；未删除、未移动、未修改任何既有文件，未 commit。
- 数据语义：`tasks/`、`assets/`、`frames/` 仅做**计数与体积统计**，未改动内容。
- 结论性质：**待批准的清单**，不是执行记录。§6 的备份命令与 §5 的删除动作全部**未执行**。

> ⚠ 审计期间仓库是**活的**：`cargo`（PID 29488，00:50:49 启动）与 `oasd`（PID 33124，22:58:46 启动）仍在运行，
> `logs/editor.log` 最近写入 00:45。**任何 target/日志/数据目录的清理或备份都必须在停服、停构建之后做**（见 §8）。

---

## 1. 目录盘点总表

`tracked` 取自 `git ls-files`；`untracked` 取自 `git status --porcelain --untracked-files=all`；`ignored` 取自 `git status --ignored`。

| 目录 | 体积 | 文件数 | 最近修改 | tracked | untracked | ignored | 性质 |
|---|---:|---:|---|---|---|---|---|
| `target/` | 12,645,195,515 B（11.78 GiB） | 46,398 | 11-02 00:49 | 0 | 0 | ✅ 全目录 | Cargo 构建产物 |
| `dist/` | 222,237,903 B（211.9 MiB） | 12 | 11-01 01:17 | 0 | 0 | ✅ 全目录 | 发布包 zip + sha256 |
| `frames/` | 36,566,573 B（34.9 MiB） | 29 | 11-02 00:19 | 29 | 0 | — | **用户数据**（28 帧 PNG + frames.json） |
| `docs/` | 1,765,329 B（1.68 MiB） | 17 | 11-02 00:14 | 17 | 0 | — | **权威文档** |
| `crates/` | 1,652,485 B（1.58 MiB） | 34 | 11-01 22:56 | 34 | 0 | — | Cargo 源 |
| `apps/` | 1,314,462 B（1.25 MiB） | 29 | 11-02 00:12 | 28 | 0 | `apps/oasd/logs/` | 源 + 内嵌 UI + 遗留日志 |
| `_verify/` | 127,316 B | 5 | 11-01 23:53 | 5 | 0（+本文件） | — | **当前审计证据** |
| `logs/` | 111,118 B | 1 | 11-02 00:45 | 0 | 0 | ✅ 全目录 | 运行日志（活文件） |
| `assets/` | 23,212 B | 7 | 11-02 00:36 | 3 | 0 | `.backup/`、`_deleted-tasks/` | **用户数据** + 恢复备份 |
| `scripts/` | 17,689 B | 5 | 11-01 20:31 | 5 | 0 | — | 打包/开发/审计脚本 |
| `packaging/` | 7,837 B | 6 | 10-31 00:38 | 6 | 0 | — | 随包分发 |
| `tools/` | 7,455 B | 3 | 10-22 01:19 | 3 | 0 | — | 开发辅助 py |
| `tasks/` | 6,612 B | 2 | 11-01 21:55 | 5 | 0 | — | **用户数据**（含 3 个 worktree 删除） |
| `templates/` | 3,619 B | 3 | 10-23 02:41 | 3 | 0 | — | 任务模板 |
| `relations/` | 3,288 B | 2 | 11-01 16:21 | 2 | 0 | — | 关系证据（append-only） |
| `runs/` | 1,302 B | 1 | 11-01 12:50 | 0 | 0 | ✅ 全目录 | 点击 JSONL 产出 |
| `library/` | 0 B | 0 | — | 0 | 0 | `/library/` | **空目录**（迁移后状态） |
| `.git/` | 56,162,338 B（53.6 MiB） | 97 | — | — | — | — | 4 packs / 5,078 objects / reflog 242 |
| 根文件 | 25,378 B | 5 | — | 5 | 0 | — | `.gitignore` `Cargo.toml` `Cargo.lock` `scheduler.json` `session-memory.config.json` |

**合计**：12,963,203,197 B / 46,640 files；**untracked = 0**（写本文件前）。

### 1.1 tracked 分布（共 145 个文件）
`crates 34 · frames 29 · apps 28 · docs 17 · packaging 6 · scripts 5 · tasks 5 · _verify 5 · 根 5 · tools 3 · templates 3 · assets 3 · relations 2`

### 1.2 `.gitignore` 覆盖（7 个 ignored 条目，全部是运行/产物）
`/target`、`/runs`、`/library/`、`/dist`、`/logs`、`**/logs/`、`/scheduler.state.json*`、`/frames/.backup/`、`/assets/.backup/`、`/assets/_deleted-tasks/`、`/tasks/*/state`

### 1.3 仓外相邻目录（**不属于本仓库，但与本审计强相关**）
| 路径 | 体积 | 文件数 | 说明 |
|---|---:|---:|---|
| `D:\OAS-dev\_verify\` | 5,442,650,939 B（5.19 GB） | 24,876 | 历史验证证据库，**非 git 仓库**；docs 中 67 处 `_verify/…` 引用**实际解析到这里** |
| `D:\OAS-dev\backend\` | 1.28 GB | — | 打包脚本硬引用 OCR 模型 / adb |
| `D:\OAS-dev\frontend\` | 0.45 GB | — | 同名前端仓 |

> ★ 关键认知修正：`docs/**` 与 `_verify/audit-20261101/**` 里的 `_verify/n43/REPORT-V1.1.md` 一类路径**不是仓库相对路径**。
> 以「仓根」解析时 67 条引用里仅 6 条存在；以「`D:\OAS-dev`」解析时 `n4/n42/n43/n44/n5/n6/n8/p0_live` 等**全部存在**。
> ⇒ 历史证据**没有丢失**，只是放在仓外；因此**不能**把仓内 `_verify/` 判为"残缺旧报告"而清理。

---

## 2. 引用与重复扫描（证据）

| 检查 | 方法 | 结果 |
|---|---|---|
| 重复文件内容 | 全仓（除 `target/`、`.git/`、`frames/`）按 size + SHA256 分组 | **0 组重复** |
| 临时/备份文件 | 全仓（除 `target/`、`.git/`）匹配 `*.bak|*.tmp|*.old|*.orig|*.swp|*.rej|*~|.DS_Store|Thumbs.db` | **0 个** |
| 压缩包 | `*.zip|*.7z|*.tar|*.gz|*.rar` | 仅 `dist/` 6 个 release zip |
| `.jsonl/.log/.state.json` | 全仓 | `logs/editor.log`、`apps/oasd/logs/editor.log`、`relations/observations.jsonl`、`runs/*.clicks.jsonl` |
| `dist` 完整性 | 6 个 zip 的 SHA256 vs 同名 `.sha256` vs `docs` 记录 | **6/6 全匹配**（见 §4 B 组） |
| `_verify/n*` 历史 | 67 条 distinct 引用逐一 `Test-Path` | 仓根解析 6 存在 / 61 不存在；`D:\OAS-dev` 解析**全部存在** |
| 孤儿构建目录 | `rg 'm0-smoke'` 全仓 | **0 命中**（`target/m0-smoke` 无任何引用） |
| 根目录残留 | `git status --untracked-files=all` | **0 个未跟踪文件**；5 个 tracked 配置均在使用 |

---

## 3. 分类结论（tracked / untracked / ignored）

- **tracked（145）**：全部为源、文档、数据、证据、配置。**无一是"临时文件"**。
- **untracked（0，本报告除外）**：仓库工作区**干净**，没有散落的草稿/临时脚本。
- **ignored（7 条目）**：`target/`、`dist/`、`logs/`、`runs/`、`apps/oasd/logs/`、`library/`（空）、`assets/.backup/`、`assets/_deleted-tasks/`。
  → 全仓 12,963,203,197 B（12.96 GB / 12.07 GiB）里，**几乎全部膨胀都发生在这 7 个 ignored 区**，**tracked 内容仅约 41.5 MB（39.6 MiB）**。

> 结论：仓库"脏"不在源，而在 **① Cargo 构建产物 ② 历史 release zip ③ 两个恢复备份**。清理面很窄、很安全。

---

## 4. 候选清单（可执行，但**未执行**）

风险分级：**零**=纯产物可重建 / **低**=可重建且无引用 / **中**=可重建但有代价或需留档 / **高**=不可重建或属证据 / **极高**=唯一副本。

### A 组 — `target/`（ignored，11.78 GiB，最大的空间来源）

| 候选 | 体积 | 文件数 | tracked | 被引用 | 重建方式 | 风险 | 建议 |
|---|---:|---:|---|---|---|---|---|
| `target/debug/incremental/` | 7,560,566,731 B（7.04 GiB） | 39,812 | 否 | 无 | 下次 `cargo build/test` 自动重建 | **低** | **可删（首选）** |
| `target/debug/{deps,build,examples,.fingerprint}` | 3,705,364,037 B（3.45 GiB） | 3,238 | 否 | 无 | `cargo build` / `cargo test` 全量重编译 | 中（见 §7 网络风险） | **保留**（除非确定可联网重编） |
| `target/release/` | 772,568,293 B（736.8 MiB） | 1,200 | 否 | `scripts/package.ps1` 读 `target\release\oasd.exe` | 重新 `cargo build --release -p oasd --features ocr` | 中 | **保留**（`-SkipBuild` 打包依赖它） |
| `target/m0-smoke/` | 304,561,712 B（290.5 MiB） | 1,057 | 否 | **rg 0 命中**（孤儿 target dir，10-23 遗留） | 无重建价值；需要时用同名 `--target-dir` 重编 | **零** | **删除**（零风险） |
| `target/tmp/` | 304,672,323 B（290.6 MiB） | 1,067 | 否 | 无（内含 10-23 一次性 `fix_*.py`/`patch_*.py`/`*.log` + `n37build/`） | **不可重建**（一次性脚本），但无留存价值 | 低 | **直接删**（如想留，先打包 9 个 py/log 再删） |

> `target/debug/deps/` 里 `libort_sys-*.rlib` 各 **362 MB**（ONNX Runtime 绑定），是 debug 体积的主因。

### B 组 — `dist/`（ignored，212 MiB，全部为历史 release 包）

| 候选 | 体积 | tracked | 被引用 | 重建方式 | 风险 | 建议 |
|---|---:|---|---|---|---|---|
| `oasd-v1.0.0-win64.zip` | 36,804,854 B | 否 | docs 记 SHA256 `6169…2780EB` | `scripts/package.ps1 -Version 1.0.0`（源码在 tag `v1.0.0`） | 低 | **备份后删** |
| `oasd-v1.1.0-win64.zip` | 36,868,461 B | 否 | docs 记 `140B…35BB` | 同上（`v1.1.0`） | 低 | **备份后删** |
| `oasd-v1.2.0-win64.zip` | 36,895,821 B | 否 | docs 记 `D31C…35FC` | 同上（`v1.2.0`） | 低 | **备份后删** |
| `oasd-v1.2.1-win64.zip` | 37,148,195 B | 否 | docs 记 `E4E4…99AC1` | 同上（`v1.2.1`） | 低 | **备份后删** |
| `oasd-v1.2.2-win64.zip` | 37,179,387 B | 否 | docs 记 `B683…8994` | 同上（`v1.2.2`） | 中 | **保留**（上一版热备） |
| `oasd-v1.2.3-win64.zip` | 37,340,651 B | 否 | docs 记 `F135922D…C12E`；= 当前发布 / 恢复节点 | 同上（`v1.2.3`） | **高**（恢复节点实物） | **保留** |
| 6 个 `.sha256` 旁文件 | 各 89 B | 否 | 与 zip 同源 | — | 零 | **保留**（留档成本≈0） |
| `dist/stage/` | **不存在** | — | docs 多处引用（`START-HERE.md:120`、`PENDING-LEDGER.md:1308/1574/1590` 的 **8788** 端口） | `pwsh -File scripts/package.ps1 -Version <v>` 会重建 stage + zip | — | **不处置**；但登记为**文档漂移**（文档指向一个当前不存在的运行目录，见 §9） |

- 4 个旧包合计 **147,717,331 B（140.9 MiB）**。
- 6/6 zip 的实测 SHA256 与旁文件、与 `docs/ENGINE-DESIGN.md`/`DELIVERY-PLAN.md`/`START-HERE.md` 记录**三方一致** ⇒ 本地包是**未被篡改的发布实物**。
- 未联网核验 4 个 GitHub Release 资产是否仍可下载；**删除前需人工确认**（见 §9-Q1）。

### C 组 — 日志

| 候选 | 体积 | tracked | 被引用 | 重建 | 风险 | 建议 |
|---|---:|---|---|---|---|---|
| `logs/editor.log` | 111,118 B | 否 | `apps/oasd/src/serve.rs:528` 硬编码 `logs/editor.log`；`docs/PENDING-LEDGER.md:1968` 有日志规范 | 运行即重建 | 低 | **保留**；如需瘦身，停 `oasd` 后截断/轮转，勿删目录 |
| `apps/oasd/logs/editor.log` | 84,029 B | 否 | **0 引用**（`cwd=apps/oasd` 启动的副作用产物） | 运行即重建 | 零 | **删除**（纯冗余） |

### D 组 — `runs/`

| 候选 | 体积 | tracked | 被引用 | 重建 | 风险 | 建议 |
|---|---:|---|---|---|---|---|
| `runs/editor-返回庭院-1793508557757.clicks.jsonl` | 1,302 B | 否 | `tools/click_heatmap.py` 的直接输入格式；`-g '!runs'` 不计入构建 | 重跑 `oasd run --clicks-out runs/<name>.clicks.jsonl` | 低 | **保留**（1.3 KB，是点击分布证据样本） |

### E 组 — `_verify/`（当前审计证据，**必须保留**）

| 候选 | 体积 | tracked | 被引用 | 重建 | 风险 | 建议 |
|---|---:|---|---|---|---|---|
| `_verify/audit-20261101/{A-session-git,B-runtime,C-ui,D-e2e,E-acceptance}.md` | 127,316 B | ✅ 5/5 | `docs/AUDIT-SUBPROCESS-2026-11-01.md:5-7,165,180` 等 | **不可重建**（本轮审计原始产物） | **高** | **保留（不动）** |
| `_verify/audit-20261101/B-cargo-test.log` | **缺失** | 否 | `B-runtime.md:8` 与 `:295` 两次引用为"361 passed"的证据 | 源件**存在**于 `D:\OAS-dev\_verify\audit-20261101\B-cargo-test.log`（63,348 B，11-01 21:38） | **高**（证据链断点） | **复制进仓**（补证据，不是删除项） |
| 本文件 `G-repo-hygiene.md` | 新增 | 否（未 commit） | — | 手工 | — | 待父代理决定是否入库 |

> 历史 `_verify/n*` **已不在仓内**且**从未被提交过**（`git log --all -- _verify/` 仅 3 次提交、0 次删除）⇒ 仓外那 5.19 GB 是**唯一副本**，见 §4-M。

### F 组 — `library/`（空目录）

| 候选 | 体积 | tracked | 被引用 | 重建 | 风险 | 建议 |
|---|---:|---|---|---|---|---|
| `library/` | 0 B / 0 files | 否（git 不跟踪空目录） | `scripts/package.ps1` 会 `mkdir` 并 `Copy-Item library\*`（带 `SilentlyContinue`） | `New-Item -ItemType Directory library` | 零 | **保留**（0 成本；删了打包也会重建）。`docs/PENDING-LEDGER.md` 记 N43/v1.2.1 已把旧库迁入全局 `assets/`，空是**迁移后的正常态** |

### G 组 — `assets/.backup/`（恢复备份）

| 候选 | 体积 | tracked | 被引用 | 重建 | 风险 | 建议 |
|---|---:|---|---|---|---|---|
| `assets/.backup/recover-20261101-203006/assets.json` | 5,343 B | 否（ignored） | **被 tracked 文件引用**：`assets/history-recovery-report.json:2` 的 `"backup"` 字段指向该绝对路径 | **不可重建**（恢复前快照） | 中 | **保留** = 保留 `history-recovery-report.json` 的可回滚性。若一定要清，先改报告字段或整目录进备份 |

### H 组 — `assets/_deleted-tasks/`（**最高风险，绝对不动**）

| 候选 | 体积 | tracked | 被引用 | 重建 | 风险 | 建议 |
|---|---:|---|---|---|---|---|
| `assets/_deleted-tasks/测试-1793551516369/{flow.json,meta.json,assets.json}` | 1,666 B | 否（ignored） | 写入路径 `apps/oasd/src/serve.rs:4277`（`backup_task_assets`）；机制见 `docs/ASSET-DATA-MODEL.md:71`、`_verify/.../A-session-git.md:288` | **不可重建** | **极高** | **不动 + 必须纳入备份** |

**独占性证据**（只读比对 blob）：

| 文件 | 备份副本 blob | `HEAD:tasks/测试/` blob | 是否同内容 |
|---|---|---|---|
| `flow.json` | `84d1bd1c…7c94`（1,168 B） | `7aa7a3d6…8eee`（495 B） | **不同** ⇒ 备份里是**更新的编辑版本，唯一副本** |
| `meta.json` | `0fcd8b5a…be3f` | 同 | 相同 |
| `assets.json` | `64ef7797…44fb` | 同 | 相同 |

> 语境：`git status` 显示 `tasks/测试/{assets,flow,meta}.json` 处于 **worktree 已删、未提交**（` D`）状态。
> 也就是说 —— 这个 ignored 备份目录是"用户删任务"这条未提交动作的**安全网**，且 `flow.json` 比 HEAD 版本更新。
> **一旦删除，那 1,168 B 的编辑内容永久丢失。**

### I 组 — `frames/.backup/`

**不存在**（`Test-Path` = False）。`.gitignore` 里已预留 `/frames/.backup/`，文档描述的 `clear-<时间>/` 机制当前无残留 ⇒ **本组无候选，无动作**。

### J 组 — 根目录遗留配置/临时目录

| 候选 | 体积 | tracked | 判断 |
|---|---:|---|---|
| `.gitignore` / `Cargo.toml` / `Cargo.lock` | 22,267 B | ✅ | 必需 |
| `scheduler.json` | 394 B | ✅ | **必需**：`package.ps1` 把它作为"dev 已验证的 v3 计划"打进发布包 |
| `session-memory.config.json` | 297 B | ✅ | **必需**：session-memory skill 配置（`docs/README.md:44`、`MIGRATION-NOTES.md:45`） |
| 其余 | — | — | **无遗留**：0 个未跟踪文件、0 个临时目录 |

⇒ **J 组无可清理项**（仓库根是干净的）。

### K 组 — 重复/遗留文档（只看仓库文件，不评估内容）

- **内容重复：0**（SHA256 全仓扫描无同内容文件）。
- 形态上"疑似多份"的仅 3 份同日审计文档：`docs/CODE-AUDIT-2026-11-01.md`、`docs/RECOVERY-AUDIT.md`、`docs/AUDIT-SUBPROCESS-2026-11-01.md`（各 8.4 KB / 5.6 KB / 14.7 KB，均 tracked、均在 `docs/README.md` 体系内）。
- 体积偏大但**权威**：`docs/SESSION-INDEX.md` 923,775 B / 6,377 行；`docs/PENDING-LEDGER.md` 290,361 B；`docs/DESIGN-SPEC.md` 220,251 B；`docs/ENGINE-DESIGN.md` 185,736 B。
- **建议：17 份 docs 全部不动。** 审计/台账文档的合并属"文档审计"结论，不是仓库卫生问题（本次按任务要求不评估内容）。

### L 组 — `.git/`

`56,162,338 B` / 97 files；4 packs；`in-pack 5,078`；loose 37；reflog 242 条；`garbage 0`、`prune-packable 0`。
⇒ **不动**。可选低收益动作：清理已合并分支 `archive/migration-20261101` + `git gc --prune=now`（预计省 <10 MB，收益远小于风险，建议放到下次发布后）。

### M 组 — 仓外 `D:\OAS-dev\_verify\`（5.19 GB，超出仓库范围，**仅登记**）

不计入本仓清理收益，但它是**同类问题的大头**，且被 docs 引用，建议父代理单独裁定：

| 分组 | 体积 | 性质 | 处置倾向 |
|---|---:|---|---|
| `*_edge_profile` ×11（`n26/n27/n28/n29/n30/n31/n32/n33/n34/n35/n36_edge_profile`） | 96,593,101 B（92.1 MiB）/ 2,856 files | **Edge 浏览器 profile 缓存**，纯垃圾 | 可直接删 |
| 根目录 `commit-*.txt` ×50 | ≈70 KB | 一次性提交信息草稿 | 可直接删 |
| 根目录 `_tmp_probe*.py` / `fix_*.py` / `inject_*.py` / `*.log` | ≈1 MB | 一次性探针/补丁脚本与日志 | 可删 |
| `n42 499 MiB`、`n43 861 MiB`、`n44 792 MiB`、`p1 479 MiB`、`u01 118 MiB`、`n5 228 MiB` | ≈2.9 GB | 含**被 docs 引用的报告**（`n42/REPORT-V1.0.md`、`n43/REPORT-V1.1|V1.2|V1.2.1.md`、`n44/session-current-both-full.txt` 等） | **只删截图/中间产物，保留 .md/.txt/.jsonl 报告** |
| `ui_assetui/ui_fix_ep/ui_stage7/ui_stage8` 各 ≈480 MiB | ≈1.9 GB | UI 快照目录，与 `p1` 高度同构（疑似重复快照） | 保留最新 1 份，其余删除 |
| `audit-20261101/`（6 files，214 KB：`B-cargo-test.log`、`cargo-test.log`、`runtime.diff.txt`、`serve.diff.txt`、`task.diff.txt`、`user-1600-2135.txt`） | 214,165 B | **本轮审计的原始日志**，其中 `B-cargo-test.log` 是仓内断链的证据 | **必须保留**（并复制 `B-cargo-test.log` 进仓） |

---

## 5. 清理建议汇总（**全部待批准，未执行**）

| 优先级 | 动作 | 对象 | 释放 | 风险 | 前置条件 |
|---|---|---|---:|---|---|
| P0 | 删除 | `target/m0-smoke/` | 290.5 MiB | 零 | 停 `cargo`（PID 29488） |
| P0 | 删除 | `target/tmp/` | 290.6 MiB | 零 | 同上 |
| P0 | 删除 | `apps/oasd/logs/` | 82 KiB | 零 | 停 `oasd`（PID 33124） |
| P1 | 删除 | `target/debug/incremental/` | 7.04 GiB | 低（下次编译自动重建，仅变慢） | 同上 |
| P1 | 备份后删除 | `dist/oasd-v{1.0.0,1.1.0,1.2.0,1.2.1}-win64.zip`（保留 `.sha256`） | 140.9 MiB | 低 | 先完成 §6 备份 + 确认 §9-Q1 |
| P2 | 保留 | `target/{debug/deps,build,examples},target/release` | — | — | 见 §7 网络风险 |
| P2 | 保留 | `dist/oasd-v1.2.2/v1.2.3`、`logs/editor.log`、`runs/`、`library/` | — | — | — |
| P3 | **绝对不动** | `assets/_deleted-tasks/**`、`assets/.backup/**`、`_verify/audit-20261101/*.md`、`tasks/`、`assets/assets.json`、`frames/`、`docs/`、`relations/` | — | 极高 | — |
| P3 | 补充 | 复制 `D:\OAS-dev\_verify\audit-20261101\B-cargo-test.log` → `_verify/audit-20261101/` | +62 KiB | — | 只增不删 |

---

## 6. 备份方案（**建议执行、本次未执行**）

### 6.1 备份根

```
D:\OAS-dev\_backups\oas-core-<yyyyMMdd-HHmmss>\
```
- `D:\OAS-dev\_backups\` **当前不存在**，需新建。
- D: 可用空间 **29.81 GB**，占用 26.09 GB ⇒ 下列 L0+L1（约 305 MiB）绰绰有余；L2（+5.19 GB）也可容纳但没必要全量。

### 6.2 分层备份内容

| 层 | 内容 | 体积 | 是否需要 |
|---|---|---:|---|
| **L0 最小集** | `docs/`、`_verify/`、`assets/`（含 `.backup`、`_deleted-tasks`）、`frames/`、`tasks/`、`relations/`、`templates/`、`scripts/`、`tools/`、`packaging/`、`apps/`、`crates/`、根 5 文件 | 41,498,391 B（39.6 MiB） | **必须** |
| **L1 推荐集** | L0 + `.git/`（→ 可 `git bundle`）+ `dist/`（6 zip + sha256） | 320,034,716 B（305.2 MiB） | **推荐**（覆盖"恢复节点"所需的源码历史 + 发布实物） |
| **L1−** | 去掉 `dist` 的旧 4 包，只留 v1.2.2/v1.2.3 | 172,317,385 B（164.3 MiB） | 折中 |
| **L2 证据集** | `D:\OAS-dev\_verify\` 全量 | 5,442,650,939 B（5.19 GB） | **不推荐全量**；建议只备份 `audit-20261101/` + `n42,n43,n44` 下的 `*.md/*.txt/*.jsonl/*.diff`（约数十 MB） |
| **L3** | `target/` | 12.6 GB | **不备份**（可重建，见 §7） |
| **L4** | `D:\OAS-dev\backend\toolkit\...`（OCR 模型 + adb） | 1.28 GB 中相关部分 | 打包依赖，**不在本仓备份范围**；若整机迁移需单独备份 |

### 6.3 命令草案（**待批准，勿直接跑**）

```powershell
# 0) 前置门禁：确认 cargo / oasd 已退出
Get-Process cargo,oasd -ErrorAction SilentlyContinue   # 必须为空

$ts  = Get-Date -Format 'yyyyMMdd-HHmmss'
$src = 'D:\OAS-dev\oas-core'
$dst = "D:\OAS-dev\_backups\oas-core-$ts"
New-Item -ItemType Directory -Force -Path $dst | Out-Null

# 1) L1：整仓除 target（robocopy 便于增量与重试）
robocopy $src $dst /E /XD "$src\target" /R:1 /W:1 /NFL /NDL /NP
#   → 约 305 MiB；含 .git、dist、全部数据与文档

# 2) 源码历史的可搬运副本（可选，便于跨机恢复）
git -C $src bundle create "$dst\..\oas-core-$ts.bundle" --all

# 3) 清单与校验（关键：证明备份与 HEAD/worktree 一致）
git -C $src rev-parse HEAD            | Set-Content "$dst\BACKUP-INFO.txt"
git -C $src status --porcelain         | Add-Content "$dst\BACKUP-INFO.txt"
Get-ChildItem "$src\dist\*.zip" | ForEach-Object {
  '{0}  {1}' -f (Get-FileHash $_.FullName -Algorithm SHA256).Hash, $_.Name
} | Add-Content "$dst\BACKUP-INFO.txt"
Get-ChildItem $dst -Recurse -File | Measure-Object Length -Sum |
  ForEach-Object { "files={0} bytes={1}" -f $_.Count, $_.Sum } | Add-Content "$dst\BACKUP-INFO.txt"

# 4) 关键不可重建项二次校验（大小 + blob id）
git -C $src hash-object -- "$src\assets\_deleted-tasks\测试-1793551516369\flow.json"
#   期望 84d1bd1c2d688ced29a705b392163e05b97c7c94

# 5) 回滚/恢复
#    robocopy $dst $src /E /R:1 /W:1
#    git clone "$dst\..\oas-core-$ts.bundle" D:\OAS-dev\oas-core-restored
```

### 6.4 备份验收标准
1. `$dst\.git` 存在且 `git -C $dst rev-parse HEAD` = `7a3869f`。
2. `$dst\assets\_deleted-tasks\测试-1793551516369\flow.json` 存在且 blob = `84d1bd1c…7c94`。
3. `$dst\dist\` 的 6 个 zip 逐个 SHA256 = 同目录 `.sha256`。
4. `$dst\_verify\audit-20261101\` 至少有 A~E 五份 `.md`（本 G 文件随后补入）。
5. `BACKUP-INFO.txt` 记录 `files=` / `bytes=`，与源站 `320,034,716 B`（L1 期望值）同量级。

---

## 7. 空间收益与 release/build 影响

### 7.1 收益（三档）

| 方案 | 动作组合 | 释放 | 清理后仓体积 |
|---|---|---:|---:|
| **S1 零风险** | 删 `target/m0-smoke` + `target/tmp` + `apps/oasd/logs` | **609,318,064 B（581.1 MiB）** | ≈11.51 GiB（12,353,885,133 B） |
| **S2 推荐** | S1 + `target/debug/incremental` + 4 个旧 zip（备份后） | **8,317,602,126 B（7.75 GiB）** | ≈4.33 GiB |
| **S3 激进** | S2 + 整个 `target/`（`cargo clean`）+ `logs/editor.log` 截断 | **≈11.91 GiB（12,793,107,993 B）** | **≈162.2 MiB**（170,095,204 B，仅剩源码+文档+数据+证据+dist 2 包+.git） |

### 7.2 对 release / build 的影响

| 动作 | 对 `cargo build/test` | 对 `scripts/package.ps1` | 对已发布 zip 内容 |
|---|---|---|---|
| 删 `target/m0-smoke`、`target/tmp` | 无（非默认 target dir） | 无 | 无 |
| 删 `target/debug/incremental` | 下一次 debug 编译变慢（不重编依赖） | 无 | 无 |
| 删 4 个旧 zip | 无 | **无**（只写新 zip） | 无 |
| **`cargo clean`（整 `target/`）** | **全量重编 99 crates**，其中 `ort-sys` 需重新获取 ONNX Runtime（`target/debug/deps/libort_sys-*.rlib` 各 362 MB 即为证据）；`C:\Users\passioki\.cargo\registry` 仅缓存 **60/99** 个 crate ⇒ **大概率需要联网下载** | 无 `-SkipBuild` 时必须先编译成功 | 无 |
| 删 `dist/stage`（已不存在） | 无 | 无（脚本自建） | 无 |
| 删日志/`runs`/`library` | 无 | 脚本自建目录与 `.keep` | 无 |

- **发布包内容零影响**：`package.ps1` 只挑 `target/release/oasd.exe` + `apps/oasd/ui` + 外部 models/adb + `packaging/*` + `tasks` + `library` + `assets` + `frames` + `config` + `scheduler.json`；**从不包含** `target/debug`、`logs/`、`runs/`、`dist/*.zip`。
- **唯一实质代价**：整 `target` 清空后的重编译时间 + 可能的网络依赖。在"恢复节点"语境下，**不建议做 `cargo clean`**。

---

## 8. 必须保留（红线清单）

1. **Cargo 源**：`Cargo.toml`、`Cargo.lock`、`crates/**`（34）、`apps/**`（28）。删了就无法重建任何东西。
2. **用户数据**（统计值留档，不改）：`tasks/返回庭院/{flow,meta}.json`；`tasks/测试/**`（worktree 已删、HEAD 仍在，**未提交**）；`assets/assets.json`（16 条，全部 `kind:"ocr_line"`）、`assets/images/a001.png`、`assets/history-recovery-report.json`；`frames/frames.json`（28 帧 / `next_id=41`）+ 28 个 PNG；`relations/observations.jsonl`。
3. **恢复安全网**：`assets/_deleted-tasks/测试-1793551516369/**`（唯一副本，§4-H）、`assets/.backup/recover-20261101-203006/**`（§4-G）。
4. **docs 权威**：`docs/**` 全部 17 份。
5. **当前审计证据**：`_verify/audit-20261101/{A,B,C,D,E}.md` +（补入的）`B-cargo-test.log`。
6. **发布/工具链配置**：`scripts/package.ps1`、`scripts/dev-serve.*`、`scripts/*asset*.py`、`packaging/**`、`scheduler.json`、`session-memory.config.json`、`.gitignore`。
7. **外部硬引用（不在仓内，严禁清理）**：
   - `D:\OAS-dev\backend\toolkit\Lib\site-packages\rapidocr\models\PP-OCRv6_rec_small.onnx`、`PP-OCRv6_det_small.onnx`
   - `D:\OAS-dev\backend\toolkit\Lib\site-packages\adbutils\binaries\{adb.exe,AdbWinApi.dll,AdbWinUsbApi.dll}`
   - 缺失则 `package.ps1` 直接 `throw`。
8. **仓外证据**：`D:\OAS-dev\_verify\audit-20261101\B-cargo-test.log` 及 `n42/n43/n44` 下的报告类文件（docs 有引用）。

---

## 9. 执行前置门禁与待裁定问题

### 9.1 门禁（当前**不满足**，因此本次一条都没执行）
| 门禁 | 当前状态 | 要求 |
|---|---|---|
| `cargo` 退出 | ❌ **PID 29488 运行中**（00:50:49 启动，正在构建/测试） | 必须为空，否则删 `target/**` 会破坏进行中的构建 |
| `oasd` 退出 | ❌ **PID 33124 运行中**（22:58:46 启动，持有 `logs/editor.log`，最近写入 00:45） | 必须为空，否则日志截断/数据拷贝会出现不一致快照 |
| 备份完成 | ❌ `D:\OAS-dev\_backups\` 不存在 | §6 完成并通过 §6.4 验收 |
| 旧 zip 可再获取 | ⚠ 未核验 | 人工确认 4 个 Release 页仍可下载 |
| 数据未提交状态已知情 | ⚠ `assets.json`、`frames.json` 已改；`tasks/测试` 已删（均未 commit） | 清理动作**不得**触碰这些路径 |

### 9.2 待裁定（本次**不擅自决定**）
- **Q1**：`dist/` 4 个旧 zip —— 是否允许"备份后删除"？若不接受任何删除，则只做 S1（581 MiB）。
- **Q2**：仓外 `D:\OAS-dev\_verify\`（5.19 GB）是否纳入本轮清理？若纳入，建议先删 11 个 `*_edge_profile`（92 MiB）+ 50 份 `commit-*.txt` + 一次性 `_tmp_probe*.py`。
- **Q3**：`_verify/audit-20261101/B-cargo-test.log` 是否由父代理从仓外复制进仓（**补断链，非删除**）？
- **Q4**：`tasks/测试` 的未提交删除要保留还是撤销？在裁定前，`assets/_deleted-tasks/测试-1793551516369/` **必须原样存在**。
- **Q5**：本报告 `G-repo-hygiene.md` 是否随 A~E 一起提交？

### 9.3 本次审计对仓库状态的唯一改动
```
git status --porcelain   # 新增 1 行：?? _verify/audit-20261101/G-repo-hygiene.md
```
除此之外：0 删除、0 移动、0 修改、0 commit。
