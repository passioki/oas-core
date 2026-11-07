# RESUME — 断点续接卡（session-memory 强制 6）

> 规则：本文件每次断点覆盖重写；≤150 行；另一个会话只读本文件 + `RESUME-INDEX.md` 最新行就能继续。
> 细节：`docs/RUNNER-STUDIO-SPLIT-PLAN.md`（§A.9~§A.17）· `PENDING-LEDGER.md §E-79/§E-80/§E-81` · `ENGINE-DESIGN.md §22.44~§22.52`。

```yaml
task_id: oas-core-dual-app-upgrade
checkpoint_seq: 138
updated_at: 2026-11-07T21:5x+08:00
trigger: 用户 21:07 九条验收反馈（地图接错素材/78s 假死/门禁/画布残留/操作区漂移/拖拽假实现/大图与源码/T8）→ 三路只读审计确认接错线/漂移/假绿 → 修复 45a458b；workspace 1043/0、UI 2713/195/0/6、E2E 19/0。
status: done
code_head: `f6f2ad1`（dev=main=origin；seq137 已完成；未 tag/未 release）
docs_head: seq137 docs 提交见 `git log -1`
next_action: seq137 已完成（dev=main=origin=`f6f2ad1`，未 tag/未 release）。seq138 已规划并锁定语义（见 `docs/MAP-V2-DESIGN.md`、`PENDING §E-106/§E-107`），**未实现**：① 地图 V2 改「所在地（名称+判定条件+回程/去程路标）/路标（相邻所在地引用：名称+路径）/路径（动作+素材，V1 click）」，删除 priority/frames/hub，独立地图页彻底删、地图 tab 放工作台最后、共享工作台当前帧；失败兜底 `fallback_task` 默认返回庭院（重启游戏回庭院）；② 地图 V2 基础完成后再做 `navigate.target_expr` 动态参数；③ Rust 源码选择“编译器”（Rust DSL→任务 IR/flow，非只读投影；依赖地图/动作 schema 稳定，建议 target_expr 后）；④ 真实设备导航/replay E2E、Firefox 手测、全局弹窗 UI、地图边内联改/canvas 缩放/map.js 双文件/map_requirements 彻底删/relations 清理、text_visible fuzzy 缓存；⑤ 用户数据未提交。详见 `PENDING §E-107`、`ENGINE §22.76`。\n```bat
REM 0) 恢复前先核实（磁盘 + 命令，不信摘要）
cd D:\OAS-dev\oas-core
git log -1 --format="%h %ci %s"      REM 本批提交后以实际为准（父 e9404a4）
git status --porcelain               REM 期望只有用户数据（勿提交 assets/frames/tasks/scheduler.json）
cargo test --workspace               REM 721/0（oasd 355 / task 111 / flow 62 / runtime 157+2）
node apps\oasd\ui\ui_pure_test.js    REM 1559 / 159 / 151 / 0 / 6 nav

REM 1) 开发启动（当前已停止；构建需 --features ocr）
cargo build -p oasd --features ocr
scripts\dev-serve.bat                          REM Studio 8788（默认）
scripts\dev-serve.bat -Surface runner          REM Runner 8789（内嵌调度；-NoScheduler 关闭）
scripts\dev-serve.bat -DryRun                  REM 只打印命令

REM 2) 打包（release exe 已存在；两包已产出，-DryRun 可预览）
scripts\package.ps1 -SkipBuild                 REM Studio -> dist\oasd-v0.1.0-win64.zip
scripts\package-runner.ps1                     REM Runner -> dist\oasd-runner-v0.1.0-win64.zip

REM 3) release 分离度 E2E 复跑（仓外脚本，自动解压+起停+断言）
powershell -NoProfile -ExecutionPolicy Bypass -File D:\OAS-dev\_verify\release-both-20261105\run-e2e.ps1
```

1. **更新检测收尾（剩余）**：URL feed 鉴权/私有仓 token、`packages/remote` 自动 GC、UI 更新列表手动刷新（`?refresh=1`）与缓存复用/下载区分文案。
2. **配置项表单边界**：历史非法数值 default、`default:""` 与无默认区分、未知字段仅高级 JSON、`type:"text"` 历史值批量提示、高级模式错误行内定位。
3. **发布策略**：tag/Release 与两仓版本联动（用户未要求，暂不 tag）；`--version` 打印 app 名；`-NoData`/`-SkipToolkit` 实包验证；macOS/Linux。
4. 旧待办不变：产品验收 `§D.4` 19 步 / P2 清理 / Phase 5；**不 tag / 不 release**（git rev 用 commit hash，不需要 tag）。

## 4. 验收进度（§D.4 V1~V7）

| 组 | 状态 | 证据 |
|---|---|---|
| V1 两端不同入口 | ✅ **本批补强**：两端 release 包各自独立运行、互斥 404、单进程/单端口 | `release-both-20261105/e2e-summary.json` 75/75 + `lead-recheck-20261105/recheck-log.txt` ALL PASS |
| V2 Studio 发布 | ✅ | PR5：`_verify/runner-studio-pr5-20261104/e2e/` |
| V3 Runner 安装/卸载/回滚 | ✅（依赖自动装/被依赖 409 由集成测试覆盖） | PR5 同上 |
| V4 任务配置 | ✅ | PR2：`pr2demo`（8790/8791） |
| V5 调度与执行（13~15） | ✅ | PR6 内嵌调度 + PR7 迁移后完整 UI 重验（`_verify/runner-studio-pr7-20261105/`） |
| V6 子任务取父配置（16~18） | 16/17 ✅ / 18 单测 | PR4 + PR6 + PR7 最小任务链多参数 |
| V7 不动用户数据 | ✅ | 本批只提交 scripts/docs（显式路径），用户数据不入库 |

## 5. 已知限制（P2/已知缺陷，写文档、不阻塞）

- **F1 已修（2026-11-05）**：PS 5.1 `Set-Content -Encoding UTF8` 写 BOM ⇒ `serde_json` 拒绝 ⇒ Runner 包 `scheduler.json` 首启 `invalid_config`；打包脚本已改 `WriteAllText` 无 BOM + stage 首字节审计（START-HERE §3 E17）；历史包须用新脚本重建。
- **参数 UI（P2）现状**：Runner 配置对话框已按 `group` 分节、使用开关/bool 三态/留空提示/必填阻止；**不做预设值集合**（D2）。Studio 已有权威 `GET /task/lint` + 构建页「检查」面板（T3 去声明）；`requires/expect/drag` 的 text_expr 仍走高级 JSON。
- **更新检测（D8）现状**：本地 exchange + 本地目录 feed + 远程 HTTP(S) feed（系统 curl）已可用；**feed TTL 缓存（默认 60s）与远程包复用已实现**（`?refresh=1`/`cache/clear`）；UI 更新列表尚未加手动刷新按钮/缓存复用文案；无鉴权/私有仓 token；`packages/remote` 无自动 GC（手动 clear）。
- **oas-runner 仓（Phase 1b）**：已有薄 app（`oasd::cli::run_as_with_args`，默认 `--ui ui`）+ UI + `scripts/package-runner.ps1`（独立打包）+ packaging 文档/示例；依赖 pin oas-core `rev=802f58b`；**仍待** tag/Release 策略、`--version` app 名、`-NoData`/`-SkipToolkit` 实包验证、macOS/Linux。
- **exchange/通知**：默认关闭（需两端 live.json 配同一目录）；`.studio-writer.lock` 未做（并发 publish 有 RMW 窗口）；notify 为 best-effort（文件索引唯一真相）。
- **bundled 导入为简化版**：仅空数据根首启；已有数据时一律 skip（不 merge）；不注册 `installed.json`；素材冲突不处理。
- **数据入库**：本批把 `assets/tasks/frames` 当前数据提交（用户要求 ①）；`scheduler.json` 仍是本机运行配置未提交（设计 §6.6 拟运行态化）。
- **参数 UI 缺口（待实现）**：任务构建页无参数入口/提示；唯一入口 = 任务库「配置项…」JSON 对话框；`save_flow` 不跑 lint；Runner `group` 未渲染、数字 enum 非下拉（方案见 `PARAM-V2-DESIGN`）。
- **installed.json**：损坏不自动隔离/repair；`/runner/tasks` 不并集 installed-but-missing。
- **租约/调度**：pass 级不是 attempt 级；stale 回收 TOCTOU；install/activate 调度检查是 check-then-act 小窗口；panic 安全依赖 unwind；`scheduler.json` 缺失/坏时内嵌线程退出。
- **包**：`manifest.asset_refs`/`dependencies` 未与 flow 闭包交叉校验；`move_dir` copy fallback 合并风险；`PackageError::Other`→400；无跨进程 packages 锁。
- **Runner UI / 打包**：PR7 的 8 条 UI P2；`start-runner.bat` 固定 8789；`README-DEPLOY.md` 示例路径 `D:\OAS-dev-live`（F3，仅文档）；`.gitignore *.tmp` 不匹配 runtime tmp。
- **跨平台**：v1 仅 Windows x64；Android 使用端仅设计预留（`packaging/CROSS-PLATFORM.md`）。

## 6. 连续性最小验证命令（恢复后先跑）

```text
cd D:\OAS-dev\oas-core
git log -1 --format="%h %ci %s"     # 本批提交后以实际为准
git status --porcelain              # 只有用户数据；scripts/docs 应已 clean
cargo test --workspace              # 841/0（perceive 11+2 / flow 78 / runtime 186+2 / task 122 / oasd 412）
node apps\oasd\ui\ui_pure_test.js   # 2214 / 164 / 158 / 0 / 6 nav
# oas-runner 独立构建/打包（新仓）：
#   cargo build --manifest-path D:\OAS-dev\oas-runner\apps\runner\Cargo.toml --target-dir D:\OAS-dev\oas-runner\target
#   powershell -NoProfile -ExecutionPolicy Bypass -File D:\OAS-dev\oas-runner\scripts\package-runner.ps1 -DataFrom D:\OAS-dev\oas-core
# E2E 复跑（仓外脚本）：
#   powershell -NoProfile -ExecutionPolicy Bypass -File D:\OAS-dev\_verify\p0-1a-e2e-20261105\e2e.ps1
#   powershell -NoProfile -ExecutionPolicy Bypass -File D:\OAS-dev\_verify\p1-param-e2e-20261105\e2e.ps1
#   powershell -NoProfile -ExecutionPolicy Bypass -File D:\OAS-dev\_verify\p2-updates-20261105\e2e.ps1
```

## 7. 证据

- **region.asset / 多候选 nearest（seq 122）**：`docs/OPERATION-REGION-DESIGN.md §7`；代码 `ba6813e`；workspace 841/0、UI 2214；E2E `_verify\opregion-aplan-e2e-20261106\`（lint 0 / publish flow_ops 含 region_asset）；oas-runner pin `ba6813e`（`768a8b7`）。
- **region / long_press / curve_swipe（seq 121）**：`docs/OPERATION-REGION-DESIGN.md §6`；代码 `644249c`；workspace 824/0、UI 2146；新建 op/region E2E `_verify\opregion-e2e-20261106\`（lint 0 / publish flow_ops 4 新标记）；oas-runner pin `644249c`（`7e64156`）。
- **参数插入/条件修复/素材选择（seq 119）**：`apps/oasd/ui/{builder.js,ui_pure_test.js}`；UI 2039 checks；用户任务 `tasks/切换预设御魂/flow.json` 条件已修复（工作区未提交）；按用户要求只推送、不发布。
- **动作目录统一 / 配置项表单 / 最终审计（seq 118）**：`docs/AUDIT-2026-11-05.md`；代码 `5b0b107`；UI 2028 checks；截图 `_verify\p7-config-ui-20261105\`（4 张：结构化表单/高级 JSON/空态/T3 预填）；oas-runner 独立包 `37,870,658 B / F6A6C301…D46C`（pin `5b0b107`）。
- **更新检测收尾 / 参数编辑补齐（seq 117）**：oas-core 代码 `78be71d`；E2E 三路 ALL PASS；`_verify\p6-params-gaps-20261105\`（3 张构建页截图：guards/drag endpoints/T3 prefill）· oas-runner 独立包 **37,871,134 B / `B127C0FC…66DB`**（rev `78be71d`）。
- **Phase 1b / oas-runner 独立打包（seq 116）**：`D:\OAS-dev\_verify\phase1b-oas-runner-20261105\`（`REPORT.md`、`smoke-test.ps1`、`smoke-checks.json` 50/50、`logs\*`）· oas-runner `dist\oasd-runner-v0.1.0-win64.zip`（37,854,377 B / `09E8E2B5…AF45`）· oas-core `_verify\phase1a-20261105\logs\workspace-test-seq116.log` 794/0。
- **Phase 1a / Studio 权威 lint（seq 115）**：`D:\OAS-dev\_verify\runner-split-phase1a-20261105\`（baseline 788 / after 788 / final 794、forced-build-ocr、oas-runner-build 日志）· `D:\OAS-dev\_verify\phase1a-20261105\`（`logs\workspace-test.log` 794/0、`logs\release-build.log` exit0/1 warning、smoke）· `D:\OAS-dev\_verify\p5-studio-lint-20261105\`（2 张构建页 lint 截图）。
- **使用端参数 UI / 更新检测（seq 114）**：`D:\OAS-dev\_verify\p2-updates-20261105\`（`e2e.ps1`/`feed-e2e-log.txt` ALL PASS；`logs\workspace-test.log` 788/0；`logs\release-build.log`）· `D:\OAS-dev\_verify\p2-runner-params-20261105\`（2 张 Runner 参数/更新 UI 截图）· 独立仓 `https://github.com/passioki/oas-runner`（initial `e1d603a`）。
- **参数 v2 后端 / P3 UI / P0-1b（seq 113）**：`D:\OAS-dev\_verify\param-v2-backend-20261105\`（workspace 774/0、oasd 385/0、build exit 0/5 warnings、release-build、clean-copy 769/0）· `D:\OAS-dev\_verify\p1-param-e2e-20261105\`（`e2e.ps1`/`e2e-log.txt` ALL PASS；manifest flow_ops/role 证据）· `D:\OAS-dev\_verify\p0-3-studio-params-20261105\`（3 张构建端 UI 截图）· `D:\OAS-dev\_verify\p0-1a-e2e-20261105\`（P0 复跑 ALL PASS）。
- **P0 首批落地（seq 112）**：`D:\OAS-dev\_verify\p0-1a-e2e-20261105\`（`e2e.ps1` + `e2e-log.txt` ALL PASS；两包 SHA 记录）· 设计 `_verify\dual-app-design-20261105\RUNNER-SPLIT-DESIGN.md §5/§6`。
- **本设计轮（seq 111）**：`D:\OAS-dev\_verify\dual-app-design-20261105\`（`PARAM-V2-DESIGN.md` / `RUNNER-SPLIT-DESIGN.md` / `DECISION-SHEET.md` / `prototype\`（2 HTML + 4 PNG + `DESIGN-NOTES.md`））。
- **双端发行包 / 分离度 E2E**：`D:\OAS-dev\_verify\release-both-20261105\`（`REPORT.md` / `DESIGN-VS-NOW.md` / `F1-FIX.md` / `run-e2e.ps1` / `e2e-summary.json`（75/75）/ `e2e-studio.json` / `e2e-runner.json` / `zip-checks.json` / `package-files.json` / `*-entries.txt` / `studio-8788.png` / `runner-8789.png` / `logs\`）。
- **Lead 独立复核**：`D:\OAS-dev\_verify\lead-recheck-20261105\`（`recheck.ps1` + `recheck-log.txt`：zip/BOM/路由/端口/进程/`scheduler --status` 全 PASS；`D:\OAS-dev\_verify\release-both-20261105\logs\lead-cargo-test-workspace.log` 721/0）。
- **参数 UI 审计**：`D:\OAS-dev\_verify\ui-params-audit-20261105\PARAMS-UI-AUDIT.md`（398 行；结论/缺口/方案 A~E/测试清单）。
- **最终审计 / PR7 / PR1~PR6**：`_verify\runner-studio-final-audit-20261105\`、`_verify\runner-studio-pr7-20261105\`、`_verify\runner-studio-pr1~pr6-20261104\`。
- **现行口径**：`docs/RUNNER-STUDIO-SPLIT-PLAN.md`（§A.9~§A.17）、`docs/ENGINE-DESIGN.md` §22.44~§22.52、`docs/PENDING-LEDGER.md §E-79/§E-80`、`packaging/README-DEPLOY.md`、`packaging/CROSS-PLATFORM.md`。
