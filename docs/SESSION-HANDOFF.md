# 会话交接单（SESSION HANDOFF）★ 一页纸

> ★ 新会话先读 `docs/START-HERE.md` → `docs/RESUME.md` → `docs/RESUME-INDEX.md` 最新行（**seq 133，✅ done：点测/拖测卡死修复 + 文字素材多候选**），然后按需看 `docs/ENGINE-DESIGN.md §22.72`（seq133）/ §22.71（seq132）/ §22.70（seq131）/ §22.69（seq130）/ §22.68（seq129）/ §22.67（V2.0）、`docs/ACCEPTANCE-2026-11-06.md`（seq130 追加）、`PENDING-LEDGER §E-102/§E-101/§E-100`、`VARIABLES-V2-DESIGN §10`、`OPERATION-REGION-DESIGN §8.9`。
> **状态：done（seq 133）——ADB 操作超时（点测/拖测卡死修复）+ OCR 文字素材多候选**：`9323ace`；workspace **984/0**、UI **2428/190/0 dead/158 runner/6 nav**；冒烟假 ADB 超时 15.1s 返回 400 且服务恢复、OCR 多候选 pass 置顶；只推送不发布（未 tag/未 release）。

## 0. 当前状态（2026-11-07 08:2x，seq 131 done）

| 项 | 值 |
|---|---|
| 分支 / HEAD | oas-core `dev`；代码 `9323ace`（dev/main 已同步）；oas-runner 独立仓仍 `768a8b7`（pin `ba6813e`，未随 seq126~130 更新）；未 tag / 未 release |
| 任务 | **seq133 已完成**：adb 超时修复（真实点测/拖测卡死）+ OCR 文字素材多候选（全部行/pass 优先）；#1 常驻弹窗识别记档不实现——素材设置菜单「加入集合」（template 专用）+ `#collection-add-dialog`（已有集合/新建集合；跨集合先移出旧集合；保留 threshold/action_shape；保存后刷新两侧库并提示 tasks_rewritten）；任务库/全局库/帧详情三处接线 |
| 测试 | `cargo test --workspace` **972 passed / 0 failed**（本轮无 Rust 改动）；UI **2428 checks / 190 ids / 0 dead / 158 runner ids / 6 nav**；workspace **984/0**；cargo check --features ocr exit 0；冒烟 `_verify\seq133-ocr-candidates\`（假 ADB 超时 400/15.1s；OCR 多候选 count=51/pass 置顶） |
| 权威文档 | **`ENGINE §22.72`（seq133）· `ACCEPTANCE-2026-11-06.md`（seq133 追加）· `PENDING §E-102/§E-101` · `OPERATION-REGION-DESIGN §8.9`** |
| 发行包 | 按用户「推送即可，不用发布」**未重打/未上传**；本地产物不含 seq126~130 新能力（package_schema=2 旧包被拒绝） |
| 独立仓 | `https://github.com/passioki/oas-runner`（`768a8b7`）；pin `ba6813e`（未打独立包；如需独立 Runner 需 bump pin + 根启动脚本 + V2 变量模型重打） |
| 服务 | 无 `oasd`/`oasd-runner` 进程；8787~8809 无监听（冒烟后确认） |
| 工作区 | `scheduler.json`、用户任务 `tasks/*`、`assets/assets.json` / `frames/frames.json` / 新增图片等均**未提交**（用户数据，**不要提交/覆盖**）；用户可能在 Studio 中删除/编辑任务（不要恢复/提交）；seq131 代码已提交；**cargo incremental 已清理，下次构建稍慢（deps 缓存仍在）** |

## 0.1 新会话恢复步骤（直接照做）

1. 调 skill `session-memory`；读 `START-HERE` → `RESUME.md`（seq 124）→ `RESUME-INDEX` 最新行。
2. 用磁盘核实：`git log -1`、`git status --porcelain`、`cargo test --workspace`、`node apps/oasd/ui/ui_pure_test.js`；与本文 §0 不一致先修文档。
3. seq124 已 done；若用户提新需求，先读 `PARAM-ASSET-IMPLEMENTATION.md` 了解现行模型（ocr_var/资产级 op_region/派生参数/候选/test_action），再看 `ENGINE §22.64.8` 的「后续可选」清单，避免重复实现。
4. 遗留项（不阻塞）：drag 测试动作、素材行长按测试入口、识别文字 ROI 接画布、S8 派生行只读卡片、候选 op_rect 预览、全屏 OCR 性能、真机 E2E、Phase 2（去 meta.params）。
5. 发布策略：**只推送不发布**；如要发布需先重打 Studio/Runner 包、视需要 bump oas-runner pin/版本（新包不兼容旧 Runner：`action_source` deprecated + `ocr_var` kind 会触发旧引擎拒绝）。

## 0.2 本轮（seq124）文档产出

- `ENGINE-DESIGN.md §22.64`（8 子节：映射/schema/运行时/API/UI/测试/兼容/风险后续）；`OPERATION-REGION-DESIGN.md §8`（资产级 op_region 现行口径，§6/§7 标历史）。
- `PARAM-ASSET-IMPLEMENTATION.md`：状态改「已实现」+「实现结果（Lead 验收）」偏差/验证/证据/后续；`PARAM-ASSET-REDESIGN.md` 标已实现（§1b 用户权威要求保留）。
- `PENDING §E-93` 全项完成归档 + §D.6 最终验收 + 后续清单；`RESUME`/`RESUME-INDEX`/本交接单/`START-HERE` 更新 seq124 done。
- 审计：`_verify\paramasset-audit-20261106\{01,02,03}-*.md`；实现证据：`_verify\backend-p0-20261106\`；E2E：`_verify\seq124-e2e-20261106\`；Lead 验收：`_verify\seq124-lead-verify\`。

## 1. 最近 3 轮关键变更（历史细节见 PLAN/ENGINE/PENDING）

- **seq 111（2026-11-05 18:2x，设计轮，只读）**：用户四项新要求 → 3 路只读设计 + 可点原型 + 决策单（`_verify\dual-app-design-20261105\`）：① 参数 v2（动作库消费类动作 + `text_expr` 让参数当 OCR 文字；group/预设；使用端每参使用开关、留空=不注入）；② 使用端独立新仓（推荐两仓 + Phase 0）；③ Studio 发布→Runner 通知（exchange 文件真相 + loopback POST）；④ 发行包带数据（Runner bundled-data 首启不覆盖）。**等用户裁定 D1~D12 后 P0 实施**。详见 `PENDING-LEDGER §E-81` / `RESUME §1.5`。
1. **seq 110（2026-11-05 13:3x）**：① `scripts/package.ps1` 旧「全量包」→ **Studio-only**（UI 白名单 8 文件 + 真实 tasks/assets + 空 frames/runs/logs + 4 启停脚本 + `surface: studio` + stage 审计 + `/` 分隔 zip；旧 9 项不符全消）；② 两包产出 + **分离度 E2E 75/75**（62 分离度 + 13 F1 回归）；③ **F1 修复**：PS5.1 `Set-Content -Encoding UTF8` 写 BOM → Runner 包 `scheduler.json` 首启 `invalid_config`；改 `Write-Utf8NoBom`（`WriteAllText` + `UTF8Encoding($false)`）+ stage 首字节守卫；④ `scripts/dev-serve.*` 双入口化（默认 Studio 8788，`-Surface runner` → 8789；8787 退役）；⑤ **参数 UI 审计**（只读）：确认「设置变量」≠ 任务参数、meta.params 唯一入口在任务库 JSON 对话框、构建页 0 入口/0 提示，推荐 A+C。详见 `PENDING-LEDGER §E-80` / `ENGINE §22.52` / `PLAN §A.17`。
2. **seq 109（2026-11-05 12:26）**：新会话恢复校验——HEAD `e9404a4`、721/0、UI 1559/159/151、服务未监听、包哈希一致；文档与磁盘零差异。
3. **seq 108（2026-11-05 03:3x）**：最终审计 P0=0、4 个后端 P1 全修 `68e5768`（replay 租约 / worker panic / install·activate 调度互斥 / installed.json 降级）；文档 P1 由 seq 108 docs 修复；handoff-ready。

## 2. 启动与验证（当前服务已停）

```bat
cd D:\OAS-dev\oas-core
cargo test --workspace                 REM 721/0（oasd 355 / task 111 / flow 62 / runtime 157+2）
node apps\oasd\ui\ui_pure_test.js      REM 1559 / 159 studio / 151 runner / 0 dead / 6 nav
cargo build -p oasd --features ocr     REM 不带 feature 会报 no OCR

scripts\dev-serve.bat                  REM Studio 8788（默认；-NoOpen/-DryRun/-Release/-SkipBuild）
scripts\dev-serve.bat -Surface runner  REM Runner 8789（内嵌调度；-NoScheduler 关闭）
target\debug\oasd.exe studio --config config\live.json   REM 等价 Studio（8788）
target\debug\oasd.exe runner --config config\live.json   REM 等价 Runner（8789）

REM 打包（release exe 已存在）：scripts\package.ps1 -SkipBuild / scripts\package-runner.ps1
REM release 分离度 E2E：powershell -NoProfile -ExecutionPolicy Bypass -File D:\OAS-dev\_verify\release-both-20261105\run-e2e.ps1
```

- 打包用户：`start-studio.bat`（8788）/ `start-runner.bat`（8789；**固定 `--port 8789`**）；`stop-runner.bat` 先 POST 停止再强杀。
- 本机实机：MuMu `127.0.0.1:16384`（不是 `emulator-5554`）；`GET /instance/devices` 为空时自动 `adb connect`。
- Studio 无调度/重置记忆；调度/重置只在 Runner「队列与调度」；`oasd serve` = Studio deprecated 别名；**8787 已退役**。

## 3. 数据目录与用户数据边界

| 路径 | 说明 |
|---|---|
| `tasks/<id>/{meta,flow,assets}.json` | 任务（用户数据）；`state/`、`meta.json.bak` 已 ignore |
| `assets/`、`frames/` | 全局素材/帧（用户数据）；`frames/.backup/` 是清空安全网 |
| `scheduler.json`(+`.bak`)、`runs/` | 调度配置/运行记录（用户数据） |
| `config/live.json`、`config/values.json` | 本机配置 / Runner 运行值（values 已 ignore） |
| `config/device.lease`、`<state>.lock`、`packages/`、`CWD/logs/` | 运行态（已 ignore / 不入 git） |
| `dist/` | 构建产物（含两端 release zip），不入 git |

★ **提交红线**：`assets/`、`frames/`、`tasks/`、`scheduler.json(.bak)` 不提交；`library/` 已 ignore。**Studio 包内嵌工作区真实数据快照（3 任务 / assets 31 条 + 5 图）——对外分发前确认数据边界（未来可加 `-NoData`）。**

## 4. 已知限制（不阻塞交接）

- **参数 UI 缺口（待用户裁定）**：任务构建页无 meta.params 入口/提示；唯一入口 = 任务库「配置项…」JSON 对话框；`save_flow` 不 lint、Studio 无 `/task/lint` ⇒ 发布 400 T3 才首次反馈；Studio 试运行 UI 无 params 输入；Runner `group` 未渲染、数字 enum 非下拉（P2）。方案见 `D:\OAS-dev\_verify\ui-params-audit-20261105\PARAMS-UI-AUDIT.md`（推荐 A+C）。
- **F1 已修（2026-11-05）**：PS 5.1 `Set-Content -Encoding UTF8` = BOM ⇒ `serde_json` 拒绝 ⇒ Runner 包 `scheduler.json` 首启 `invalid_config`；脚本已改无 BOM + stage 守卫；历史包须重建（`START-HERE §3 E17`）。
- **包/registry**：`installed.json` 损坏不自动隔离/repair；`/runner/tasks` 不并集 installed-but-missing；`manifest.asset_refs`/`dependencies` 未交叉校验；`move_dir` copy fallback；`PackageError::Other`→400；无跨进程 packages 锁；Phase 1 仅本机绝对路径安装。
- **租约/调度**：pass 级不是 attempt 级；stale 回收 TOCTOU；install/activate 调度检查 check-then-act；panic 安全依赖 unwind；`scheduler.json` 缺失/坏时内嵌线程退出；日志环形缓冲仅内存；24h soak 未跑完。
- **UI/打包**：PR7 的 8 条 UI P2；`start-runner.bat`/`stop-runner.bat` 固定 8789；`README-DEPLOY.md` 示例路径 `D:\OAS-dev-live`（F3，仅文档）；`.gitignore *.tmp` 不匹配 runtime tmp。
- **跨平台**：v1 仅 Windows x64；Android 使用端仅设计预留（`packaging/CROSS-PLATFORM.md`）。

## 5. 下一位接手第一步

1. 读 `START-HERE` → 本文件 → `RESUME` → `RESUME-INDEX` **seq 110**；用 `git log -1` / `git status --porcelain` 核实 HEAD 与工作区（**本批 scripts+docs 可能刚提交，以实测为准**）。
2. 跑 §2 验证命令（721/0、1559/159/151/6）；（可选）复跑仓外 `run-e2e.ps1` 的 75 条 release 分离度断言。
3. **先等用户裁定参数 UI 方案（A+C 推荐）**，再改 `builder.js`/`app.js`/`serve.rs`；改完必须补 UI pure test + `cargo test -p oasd`。
4. 继续 `RUNNER-STUDIO-SPLIT-PLAN §D.4` 19 步产品验收（V1 已由本批 release E2E 补强；建议补真实产品任务 + 真机）。
5. **不 tag / 不 release**（除非用户明确说发布）；改调度/队列语义先改 `RUNNER-STUDIO-SPLIT-PLAN` + `ENGINE-DESIGN §22.34.7`，再改代码。

## 6. 证据路径

- 双端发行包 / 分离度 E2E：`D:\OAS-dev\_verify\release-both-20261105\`（`REPORT.md` / `DESIGN-VS-NOW.md` / `F1-FIX.md` / `run-e2e.ps1` / `e2e-summary.json`（75/75）/ `zip-checks.json` / `package-files.json` / `studio-8788.png` / `runner-8789.png` / `logs\`）。
- Lead 独立复核：`D:\OAS-dev\_verify\lead-recheck-20261105\`（`recheck.ps1` + `recheck-log.txt` ALL PASS）；`cargo test` 日志 `release-both-20261105\logs\lead-cargo-test-workspace.log`（721/0）。
- 参数 UI 审计：`D:\OAS-dev\_verify\ui-params-audit-20261105\PARAMS-UI-AUDIT.md`。
- 最终审计 / PR7 / PR1~PR6：`_verify\runner-studio-final-audit-20261105\`、`_verify\runner-studio-pr7-20261105\`、`_verify\runner-studio-pr1~pr6-20261104\`。
- 现行文档：`docs/RUNNER-STUDIO-SPLIT-PLAN.md` §A.9~§A.17、`docs/ENGINE-DESIGN.md` §22.44~§22.52、`docs/PENDING-LEDGER.md §E-79/§E-80`。
