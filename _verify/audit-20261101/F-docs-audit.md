# F — session ↔ docs ↔ 实际代码 三方审计（2026-11-02）

> **性质**：独立只读审计。除本文件外未改代码/数据/文档，未 commit、未删除、未移动。
> **目标**：为“关键恢复节点”核对 11-01 16:00 以后（重点 18:40 / 19:12 / 20:10~20:38 / 21:02~21:25 / 22:31 / 23:39 / 00:02）的会话承诺、docs 记载、实际代码三者是否一致，并给出可备份的旧文档清理建议。
> **证据**：
> - session：`D:\OAS-dev\_verify\n44\session-current-both-full.txt`（29030 行，USER=224 / ASST=274，最后一条 USER 00:48:35）。
> - git：`D:\OAS-dev\oas-core`，`dev` @ `7a3869f`；`main` @ `385d611`；`origin/dev` @ `7a3869f`。
> - 代码/数据实测时间：2026-11-02 01:0x（Asia/Shanghai）。

---

## 0. 结论速览（8 条）

1. **代码主方向没有反**：18:40 七项中，画布/帧详情、实时/位置帧、区域有效性、素材排序、分类重命名已实现；**多边形/反选行为可用但 `SelectionTool` 未抽出**，**拖测生成的 flow step 仍是 bbox**，两项只能算部分完成。
2. **当前工作区不是 docs 所写的状态**：HEAD `7a3869f`；`cargo test --workspace` 实测 **393 passed / 0 failed**；UI pure test **203 checks / 0 dead ids**；`task lint` 实测 **1 task / 0 errors**（不是 2/0）；全局素材 **16 个 / 1 张图**（不是 34、也不是 14/15）；帧 **28 个，current_id=20**（不是 40）；`tasks/测试` 在当前 worktree 已删除（未提交）。
3. **最严重的 docs 漂移集中在 5 份“现行”文档**：`START-HERE`、`RESUME`、`SESSION-HANDOFF`、`README`、`ASSET-DATA-MODEL` 仍在用 20:xx~23:18 的 34/14 素材、2 个任务、旧 HEAD、旧“数据阻塞”口径；新会话按 `START-HERE §0.2/§1.2` 会先读恢复审计/审计流水，而不是当前 `RESUME`。
4. **`EDITOR-UX-PLAN` 内部自相矛盾**：§3 说 `action_roi` “保留读取兼容”，§10 说“删除、不保留兼容”；§6 说 `action_source` “待确认”，§10.2 已确认；§12 说版本重置“待确认”，§E-67 已执行。§13“当前代码证据”还是实现前快照。
5. **`ENGINE-DESIGN` 有两处与现行实现冲突**：§22.34 标题仍写“设计定稿，待实现”、§22.34.6 仍写“待实现 N36/N37”，但 V1.0 已交付；§22.34.1 的任务包仍写 `tasks/<id>/{meta,flow,assets,images}`，与 `ASSET-DATA-MODEL` 的“任务只存 meta/flow，素材统一在全局 `assets/`”冲突。
6. **审计链本身有口径冲突**：`CODE-AUDIT` 的 P1-1 是“drag step 仍 bbox”（未修）；`AUDIT-SUBPROCESS` 的 P1-1 是“UI 模板 shape 未平移”（已修）。`PENDING §E-72` 写“P1-1~P1-7 已修”容易被误读为 drag 也已修；必须在台账里显式保留 drag 开口。
7. **发布配置先于文档需要修**：`scripts/package.ps1:5` 默认版本仍是 **1.2.3**；`packaging/README-DEPLOY.md:1` 与 `packaging/start-editor.bat:1` 仍写 **V1.2.2**。如果现在直接构建“0.1.0 恢复节点”，会产出 `oasd-v1.2.3-win64.zip` 或版本说明错配。另：`package.ps1` 不复制根 `assets/`，而是从 `tasks/`+`library/` 迁移；当前任务没有 task assets、`library/` 为空，打包 stage 可能没有这 16 个全局素材，发布前必须检查。
8. **清理建议**：17 份 docs 中保留 10 份现行文档，**7 份归档到 `docs/archive/`**（CODE-AUDIT、RECOVERY-AUDIT、MIGRATION-NOTES、DELIVERY-PLAN、DESIGN-SPEC-PROGRESS、SESSION-INDEX、DESIGN-SPEC）。这 7 份合计 **1,192,977 B / 1,765,329 B = 67.6%**，顶层 docs 从 17 份降到 10 份。**不要直接删除**；先 zip 备份、`git mv`，并同步改 `session-memory.config.json` 与 5 处主入口链接。

---

# A. 事实差异表

## A0. 当前基线（实测）

| 项 | 实测值 | 证据 |
|---|---|---|
| 仓库/分支/HEAD | `oas-core` · `dev` · **`7a3869f`** | `git rev-parse HEAD`；`git branch -avv` |
| main | `385d611`（v1.2.3） | `git log -1 main` |
| worktree | `M assets/assets.json`、`M frames/frames.json`、`D tasks/测试/{assets,flow,meta}.json` | `git status --porcelain` |
| cargo tests | **393 passed / 0 failed** | 2026-11-02 实测；12 个有测试的 suite 合计 393（3+2+49+3+16+6+2+113+2+55+2+140） |
| UI pure test | **203 checks / 0 dead ids** | `node apps/oasd/ui/ui_pure_test.js` |
| task lint | **1 task / 0 errors** | `oasd task lint --tasks-dir tasks --all` → `linted=1 errors=0` |
| 全局素材 | **16 items / 1 image**；15 `ocr_line` + 1 `template`；全部 `action_source=fixed`、有 `action_shape`、有 `positions` | `assets/assets.json` 实测；HEAD 为 15，worktree 新增 `管理分组` |
| 帧 | **28 个 PNG / 28 条记录，current_id=20，next_id=41** | `frames/frames.json` + `frames/*.png` |
| 任务 | worktree 只有 `返回庭院`（flow 11 steps / meta 400B）；`tasks/测试` 已删除但未提交 | `tasks/` 目录 + git status |
| archive | `archive/migration-20261101 = 29b696f`；**2827 素材 / 2099 图 / 57 个 task 目录（57 meta、56 flow）** | `git show archive...:assets/assets.json`；`git ls-tree`；docs 写“55 任务”，需复核 |
| docs | 17 个 `.md`，合计 1,765,329 B | `docs/*.md` |
| session | 29030 行，USER 224 / ASST 274，末条 00:48:35 | 导出文件头/尾 |

> 注：`cargo test` 的 pwsh 外层 `exit code 1` 是把 cargo stderr 当 `NativeCommandError` 的管道假象；测试本身 393 passed / 0 failed。

## A1. 18:40 七项：代码现状

| # | 用户要求（18:40） | 实际代码/数据 | 判定 | 证据 |
|---|---|---|---|---|
| 1 | 画布不裁切、信息不覆盖画面、位置帧详情 | 16:9 `#canvas-stage`、`#frame-detail` 在侧栏、结果/日志可收起；RO 修复后 resize 幂等 | **已实现** | `index.html:33,139`；`app.js:4487,4527,5998-6008` |
| 2 | 反选 = 全屏−区域；多边形；框选抽函数 | `action_shape` rect/polygon + `invert` 全链路可用；但 `polygon/polyMode` 与 `actionPoly/actionPolyMode` 仍是两套状态，没有 `SelectionTool` 符号 | **部分** | `app.js:199-201,1282,1429,1551-1621`；全仓无 `SelectionTool`；`AUDIT-SUBPROCESS:138` 自述“两套几何按语义保留” |
| 3 | 实时开→实时帧；实时关→位置帧；所有测试 | `testFrames()` 单素材逐位置帧；batch/OCR/match/collection match 均走 `frame_for_test`；`live=1` 真实抓帧、失败 400 不回退缓存 | **已实现** | `app.js:1649-1667`；`serve.rs:1722,2922,3259`；`AUDIT-SUBPROCESS §7` |
| 4 | 点测不再固定同一点；拖测 X 有变化 | UI 点测/拖测起点随机、拖测横抖；runtime 也随机采样；但**拖测生成的 flow step 仍是 `{kind:'region', roi: bbox}`**，且 UI 与 runtime 是两套采样实现，不是计划说的“共用同一采样函数” | **部分** | `app.js:1794-1934`（点测）、`1936-2033`（拖测）、`2009-2017`（step from=region）；`lib.rs:1654`（runtime 采样） |
| 5 | 识别 bbox 与操作区重叠有效性；显式独立操作区允许 | `fixed` 不重叠 → `valid:true, independent:true`；`recognition` 不重叠 → 无效；polygon 用多边形/bbox 相交；反选按四角是否在排除区外判断 | **已实现** | `app.js:1551-1593` |
| 6 | 素材拖动排序 | `POST /task/assets/reorder` + UI 行拖拽 | **已实现** | `serve.rs:1205,4001`；`app.js:2910` |
| 7 | 帧分类重命名 | `POST /frames/category/rename`；空 `to` 拒绝；合并/影响帧数 | **已实现** | `serve.rs:1187,1556`；`app.js:3986` |

## A2. 后续验收项（20:10 以后）

| 时间 | 事项 | 实际 | 判定/备注 |
|---|---|---|---|
| 19:12~19:30 | `action_shape` 方案 B、`action_source`、删除 `action_roi`、版本 1.2.3→0.1.0 | `Cargo.toml` 0.1.0；`oasd --version` = 0.1.0；JSON 旧字段不再读取；runtime 内部仍保留 `click_back` 作为 region/collection 的 bbox 快捷路径 | **已实现**；docs 应写“JSON/UI 删除；runtime 内部 bbox 快捷路径保留”，不要写成“全仓无 roi_back” |
| 20:10~20:38 | 任务素材库移到任务页；素材页恢复全局库；全局搜索/分类；任务页只显示 flow 引用子集 | 代码有 `#global-asset-search`/`#global-asset-category-filter`/`#btn-library-refresh`；任务页未引用提示 + “显示全部”；旧 `library` UI 已移除 | **代码已实现**；当前数据 16、任务 1，旧 docs 的“34/14”全部过期 |
| 21:02~21:25 | 16:00~21:00 代码审计（4 P0 / 4 P1） | P0-1~P0-4、P1-2/3/4/6 后续已修；`AUDIT-SUBPROCESS` 又补 3 P0 + 8 P1；`b910a2b` 修主体 | **基本完成**；P1-8 SelectionTool 仅行为复位，几何双存储保留；drag step 仍 bbox |
| 22:31 | 全局库搜索/分类；1-6 清理 | `#global-asset-search`/`#global-asset-category-filter`；OCR polygon 保存；`live=1` 强制抓帧；死 DOM id 25→0；derived OCR mask `4d5759f` | **已实现**；UI 203、cargo 393 |
| 23:39 | 点击分布去左上指示条；反选覆盖矩形/素材行、颜色/虚线；真实/演练开关 | `drawClickStats()` 已删、`drawClickOverlay()` 只画点/热力；`ACTION_EXCLUDED_FILL` 冷/暖色；`#btn-test-mode` 默认演练，真实模式 `/input/tap` `/input/swipe` + 二次确认；单素材真实点击/滑动已做 | **已实现**；collection 仍 dry-run；真实 UI 手势用户 00:37 说“测试过了”，assistant 只独立验证了 `/input/tap` |
| 00:02 | ResizeObserver loop | `scheduleCanvasFit()` rAF 合并；`fitCanvasToStage` 幂等；RO 回调/window resize 走调度；无全局错误屏蔽 | **已实现**；UI 203；用户 00:37 验收“没问题” |
| 00:37~00:38 | 拖动是否叠加点击记录；drag margin | `record_click()` 只被 `perform_click()` 调用；`Step::Drag` 走 `input.swipe`，不写 clicks；margin 默认 0.15、范围 0~0.45、作用于起/终点候选矩形内缩 | **会话已解释，但未进 docs**；建议补到 `ENGINE-DESIGN` 或 `EDITOR-UX-PLAN` 的 FAQ |
| 00:38 | collection 真实点击 | `handle_collection_match` 只返回 `action_box`，没有 `/input/tap`；仍是集合匹配 dry-run | **未实现/未承诺**；`AUDIT-SUBPROCESS §9`、`E-acceptance:66` 已记录；用户可能把它与素材真实点击混淆，docs 必须写清 |

## A3. docs 具体过期行（按文件）

| 文件:行 | 文档现在说 | 实际/应为 | 判定 |
|---|---|---|---|
| `START-HERE.md:31` | 393 tests | 393（正确） | ✅ |
| `START-HERE.md:32` | `返回庭院` + `测试` | worktree 只有 `返回庭院`；`测试` 已删除未提交 | **过期** |
| `START-HERE.md:52` | 恢复节点 main=385d611 | 当前仍真；但 00:48 已要求构建新 release 并同步 main/dev | **即将过期** |
| `START-HERE.md:55` | “当前计划：先读 AUDIT-SUBPROCESS + RECOVERY-AUDIT … 剩真实 UI 手势 + collection 真实点击” | RECOVERY 是历史，AUDIT 是审计流水，二者都不是当前计划；真实 UI 手势用户已验收；collection 真点击仍 open；当前应先读 `RESUME` | **过期/指针错** |
| `START-HERE.md:60` | 旧 `action_roi`/`roi_back` 已删除 | JSON/UI 已删；runtime 内部 `click_back` bbox 快捷路径仍用于 region/collection/旧测试 | **表述过度** |
| `START-HERE.md:65-66` | 当前 assets=34（错误恢复） | 16（15 HEAD + `管理分组`），1 图 | **过期** |
| `START-HERE.md:68` | 待办含“按 EDITOR-UX-PLAN 做编辑器 UX 改版” | 首轮已实现，剩 SelectionTool/drag 等结构项 | **过期** |
| `START-HERE.md:224` | `_verify/` 不在 git，不提交 | `_verify/audit-20261101/{A-E}.md` 已在 git（30d3145）；F 也将纳入 | **过期** |
| `START-HERE.md:258` | main 保持 v1.2.3，不自动合并 | 用户 00:48 要求构建 release 并同步 main/dev | **与新指令冲突** |
| `START-HERE.md:308/310` | DELIVERY-PLAN 是现行路线；台账读 §E-24/25/26 | DELIVERY 是历史；当前台账应指 §E-66~§E-72 | **过期** |
| `README.md:26` | 当前基线（2026-11-01） | 11-02 | **过期** |
| `README.md:29` | 当前任务 `返回庭院` + `测试` | 只有 `返回庭院` | **过期** |
| `README.md:30` | task lint 2/0 | 1/0 | **过期** |
| `README.md:32` | 编辑器首轮（未含 23:39/00:02 验收） | 应补点击分布/反选/真实测试开关/RO | **不完整** |
| `README.md:39` | main 保持 385d611 直到下一次发布 | 下一次发布已由用户 00:48 触发 | **即将过期** |
| `README.md:20-24` | 把 EDITOR-UX-PLAN、RECOVERY、CODE-AUDIT、AUDIT 都列入“当前有效文档” | 其中 RECOVERY/CODE 是历史；EDITOR/AUDIT 仍现行；应分栏或标历史/移 archive | **分类错误** |
| `RESUME.md:8-11` | checkpoint_seq=84 / updated 23:18 | 用户 00:37 已验收；00:48 新任务；需 seq 85 | **过期** |
| `RESUME.md:16` | HEAD `a2b4b12` | `7a3869f` | **过期** |
| `RESUME.md:17` | task lint 2/0 | 1/0 | **过期** |
| `RESUME.md:18-21` | current_id=40；assets 14/1；`tasks/测试` 用户新建 | current_id=20；assets 16/1；`测试` 已删除未提交 | **过期** |
| `RESUME.md:23` | 修复清单到 `a2b4b12` | 还缺 `e749c10`、`0cb6c7c`、`7a3869f`，且 7a3869f 是 docs | **不完整** |
| `RESUME.md:25` | 剩真实设备/人工点选；用户数据未提交 | 真机/真实模式用户已验收；仍有 M assets/M frames/D 测试 | **半过期** |
| `RESUME.md:47/56-57/85` | 34 个错误恢复素材是当前状态 | 当前 16；34 是历史错误恢复，应标历史 | **过期** |
| `RESUME.md:75` | 真实设备/人工点选（当前无设备） | emulator-5554 在线；用户已验收 | **过期** |
| `RESUME.md:80-100` | 恢复路线 A/B/C 待裁定、阻塞 | 用户 21:02 说“先不管素材，素材可以重建”；23:39 已提交用户数据；当前不阻塞 | **过期** |
| `RESUME.md:104` | `_verify/n44/...` | 实际在 `D:\OAS-dev\_verify\n44\...`（oas-core 外） | **路径错** |
| `SESSION-HANDOFF.md:4-6` | 数据恢复审计阻塞态；21:55 | 已不阻塞；当前 00:48 后 | **过期** |
| `SESSION-HANDOFF.md:10` | HEAD `983c740` | `7a3869f` | **过期** |
| `SESSION-HANDOFF.md:12` | task lint 2/0 | 1/0 | **过期** |
| `SESSION-HANDOFF.md:13` | `tasks/返回庭院` 删除未提交 | `返回庭院` 已恢复；当前是 `tasks/测试` 删除 | **过期** |
| `SESSION-HANDOFF.md:15/27` | 34 个素材 / 18 图 | 16 / 1 图；34 是历史错误恢复 | **过期** |
| `SESSION-HANDOFF.md:19` | 审计“3 P0+8 P1；P1-2/4/6 部分修复” | 之后已修主体；P1-8 几何、drag 仍 open | **过期** |
| `SESSION-HANDOFF.md:32-46` | 等用户裁定 archive 恢复路线 | 用户已决定不恢复素材；应改为当前收尾/发布 | **过期** |
| `ASSET-DATA-MODEL.md:75-85` | “已从 git 恢复 34 个 / 18 图 / 合并进 assets” | 这是 §E-70 已推翻的错误恢复；当前 16/1；archive 2827 才是权威 | **过期且误导** |
| `EDITOR-UX-PLAN.md:3` | Phase A/B/C/D 已实现；runtime template/OCR/region 全 exact | 行为大体真；SelectionTool 未抽、drag step 仍 bbox | **过度声明** |
| `EDITOR-UX-PLAN.md:138-141` | `action_roi` 保留读取兼容 | 与 §10“删除、不保留兼容”直接矛盾；实际 JSON 已删 | **自相矛盾** |
| `EDITOR-UX-PLAN.md:220` | dry-run 与 runtime 共用同一采样实现 | UI JS 与 runtime Rust 是两套 | **未实现** |
| `EDITOR-UX-PLAN.md:293-299` | `action_source` 待确认 | §10.2 已确认、§E-67 已执行 | **过期** |
| `EDITOR-UX-PLAN.md:534` | 版本 0.1.0 待确认 | 已完成 | **过期** |
| `EDITOR-UX-PLAN.md:536-545` | “当前代码证据”说无详情区/无 reorder/105 处 action_roi | 是 19:30 前快照；现已实现/已删 | **过期** |
| `ENGINE-DESIGN.md:1715` | 22.34 “设计定稿，待实现” | V1.0 已交付（§22.35） | **过期标题** |
| `ENGINE-DESIGN.md:1729` | 任务包 `meta+flow+assets.json+images/` | 现行 `ASSET-DATA-MODEL`：任务只 meta/flow，素材全局 assets/ | **冲突** |
| `ENGINE-DESIGN.md:1815` | “待实现：N36/N37” | N36/N37 已完成（§E-24 台账） | **过期** |
| `PENDING-LEDGER.md:2691` | 版本重置待确认 | §E-67 已执行 | **过期** |
| `PENDING-LEDGER.md:2743-2762` | E-68 恢复旧 `library-select`/`btn-library-save` 等 | 后续 `1f73f17` 又删除旧 library UI；E-68 只是中间态 | **被后续推翻** |
| `PENDING-LEDGER.md:2874-2908` | E-70 当前 assets 34、HEAD 1f73f17、⛔ 阻塞 | 历史快照；当前 16、7a3869f、不阻塞 | **状态过期** |
| `PENDING-LEDGER.md:2995` | “P1-1~P1-7 已修” | 其中的 drag step 仍 bbox（CODE-AUDIT P1-1）；P1 编号与另一份审计冲突 | **歧义/部分不实** |
| `PENDING-LEDGER.md:3030-3036` | RO 修复，用户需硬刷新 | 用户 00:37 已验收 | **缺验收记录** |
| `RESUME-INDEX.md:92` | 最后一行 seq 83 | RESUME 已到 seq 84；84+ 未追加 | **断档** |
| `AUDIT-SUBPROCESS.md:8` | HEAD 983c740 / 361 | 报告范围是 21:50；当前 7a3869f/393 | **范围未标清** |
| `AUDIT-SUBPROCESS.md:152/167` | task lint 2/0 | 当前 1/0 | **过期** |
| `AUDIT-SUBPROCESS.md:159/168` | 真实设备未做/无设备 | §9 已做真机 tap；用户 00:37 已验收真实模式 | **前后冲突** |
| `CODE-AUDIT-2026-11-01.md:3/8/etc` | HEAD 983c740 / 361 | 历史审计；已被 AUDIT-SUBPROCESS 取代 | **历史，可归档** |
| `RECOVERY-AUDIT.md:4` | `_verify/n44/...` | 实际在 `D:\OAS-dev\_verify\n44` | **路径错** |
| `RECOVERY-AUDIT.md:54-77` | 当前 assets 34 / 恢复路线待定 | 历史；当前 16；用户已决定不恢复 | **历史，可归档** |
| `DELIVERY-PLAN.md:1-5` | V1.0 交付路线/DoD | V1.0~V1.2.1 已交付；版本已 reset；历史 | **历史，可归档** |
| `DESIGN-SPEC-PROGRESS.md:1-24` | 10-06~10-12 覆盖 6287 条 | 旧仓历史；与 oas-core 0.1.0 无关 | **历史，可归档** |
| `SESSION-INDEX.md:1-15` | 6287 条分类索引 | 902KB 旧索引，停在 11-01 19:45；非现行 | **历史，可归档** |
| `MIGRATION-NOTES.md:1-10` | 旧仓→oas-core 文档迁移 | 历史 provenance；仍准确 | **历史，可归档** |
| `DESIGN-SPEC.md:1-8` | 旧 backend/frontend 设计规格（10-06~10-12） | 文首已声明非现行；仍被 skill/config 指向 | **历史，可归档（需改 config）** |

## A4. 文档内部/文档之间的关键矛盾

1. **`EDITOR-UX-PLAN §3` vs `§10`**：一个说 `action_roi` fallback 保留，一个说删除且不兼容。以 §10 + §E-67 + 代码为准：JSON/UI 已删除。
2. **`EDITOR-UX-PLAN §6` vs `§10.2`**：`action_source` 一处“待确认”，一处“已确认新增”；以 §10.2 为准。
3. **`EDITOR-UX-PLAN §12` vs `PENDING §E-67`**：版本重置一处“待确认”，一处“已执行”；以 §E-67 为准。
4. **`ENGINE §22.34` vs `§22.35`/`ASSET-DATA-MODEL`**：任务包 schema 与“任务只 meta/flow”冲突；标题“待实现”与 V1.0 交付冲突。
5. **`CODE-AUDIT P1-1` vs `AUDIT-SUBPROCESS P1-1`**：同名不同问题。前者是 drag step bbox（未修），后者是 UI shape 平移（已修）；`PENDING §E-72` 的“P1-1~P1-7 已修”不能覆盖 drag。
6. **`AUDIT-SUBPROCESS §8` vs `§9`**：§8 说“真实设备未做/无设备”，§9 说真机 `emulator-5554` tap 已验证；时间顺序上 §9 更新，但顶部仍只写 361/983c740，容易被误读。
7. **`START-HERE E4` vs 实际 git**：E4 说 `_verify/` 不在 git，实际 `_verify/audit-20261101/{A-E}.md` 已跟踪（30d3145 用户要求“纳入”）。
8. **`START-HERE §0.2` vs 用户 00:48**：前者说 main 保持 385d611，后者要求“构建 release，同步提交到 main 和 dev，作为关键恢复节点”；发布前必须统一。
9. **路径错误**：`RESUME:104`、`RECOVERY-AUDIT:4`、`SESSION-HANDOFF:53`、`PENDING:2262` 把 `_verify/n44` 写成仓库内路径；实际是 `D:\OAS-dev\_verify\n44`（oas-core 外），只有 `oas-core/_verify/audit-20261101` 在仓库内。

---

# B. 文档处置建议（17 份）

## B0. 判定标准

- **现行权威**：新会话/发布/复现必须读，内容必须与 HEAD/数据一致。
- **历史**：记录已完成的阶段/审计/迁移，不再指导当前操作；可归档。
- **重复**：与现行文档大面积重叠，且没有独立证据价值；优先归档/合并。
- **过期**：内容与当前事实冲突；先更新，不能只归档（除非整份降级为历史）。

## B1. 处置表

| 文件 | 分类 | 为什么可移走/删除 | 仍被谁引用 | 独立历史价值 | 建议 | 风险 |
|---|---|---|---|---|---|---|
| `START-HERE.md` | 现行入口 | 不能移；但 §0.2/§1.2/§3/§5 多处过期 | 所有新会话、README、RESUME | 高（入口协议） | **保留，立即更新** | 高（不更新的代价最高） |
| `RESUME.md` | 现行断点 | 不能移；HEAD/数据/下一步全过期 | START-HERE、README、RESUME-INDEX | 高（断点） | **保留，覆盖重写** | 高 |
| `README.md` | 现行索引 | 不能移；文档分类/日期/任务数过期 | 新会话、docs 入口 | 高 | **保留，更新索引** | 中 |
| `SESSION-HANDOFF.md` | 现行一页纸（实为过期） | 内容与 RESUME/START-HERE 重叠且停在 21:55；可先更新，后续可合并进 RESUME | START-HERE §1.2/§5、README | 中（一页交接格式） | **保留，立即更新；第二阶段可合并后删除** | 中（删除需改 START-HERE） |
| `ASSET-DATA-MODEL.md` | 现行模型 | §5 把已被推翻的 34 恢复写成事实 | START-HERE §7、README、PENDING | 高（唯一素材模型） | **保留，重写 §5** | 中 |
| `EDITOR-UX-PLAN.md` | 现行编辑器设计/计划 | 不能移；但状态过度声明 + 3 处自相矛盾 + 实现前证据 | START-HERE §0.2/§1.2/§7、README | 高（编辑器设计） | **保留，修矛盾/状态；可改名为 EDITOR-UX-SPEC** | 中 |
| `ENGINE-DESIGN.md` | 现行引擎规格 | 不能移；§22.34 标题/schema 过期 | START-HERE §1.2、PENDING、README | 极高 | **保留，只做定点修正** | 高（大文件，勿重构） |
| `PENDING-LEDGER.md` | 现行台账 | 不能移；E-66/68/70 状态过期、E-72 缺验收 | START-HERE、RESUME、README | 极高（唯一台账） | **保留，追加/订正状态；老段加历史横幅** | 高（append-only，勿改旧证据） |
| `RESUME-INDEX.md` | 现行断点索引 | 不能移；缺 seq 84+ | START-HERE、RESUME | 高 | **保留，追加 84~86** | 中 |
| `AUDIT-SUBPROCESS-2026-11-01.md` | 现行审计流水 | 不能移；是 21:50→00:15 的唯一审计链；但顶部 scope 过期 | START-HERE、README、PENDING E-72 | 高（审计证据） | **保留，顶部加“历史范围/当前状态”横幅；可选移 docs/audits/** | 低 |
| `CODE-AUDIT-2026-11-01.md` | 历史（首轮审计） | 已被 `AUDIT-SUBPROCESS` §6-10 全面取代；P1 编号与后续冲突 | README、RESUME:100、PENDING E-71、AUDIT-SUBPROCESS | 中（首轮审计证据） | **归档 `docs/archive/`** | 低（只改链接） |
| `RECOVERY-AUDIT.md` | 历史（恢复审计） | 34/恢复路线是历史快照；当前数据已变 | README、START-HERE:55、RESUME:26/77、SESSION-HANDOFF、PENDING E-70 | 高（回滚/34 事故证据） | **归档 `docs/archive/`** | 低-中（链接多） |
| `MIGRATION-NOTES.md` | 历史（文档迁移） | 一次性迁移 provenance，已完成 | README:52、PENDING:2318、RESUME-INDEX:76 | 中（来源可追溯） | **归档 `docs/archive/`** | 低 |
| `DELIVERY-PLAN.md` | 历史（V1.0 计划） | V1.0~V1.2.1 已交付，版本已 reset；计划不再指导当前 | README:19、START-HERE:308、ENGINE:1817、PENDING 多处 | 中（DoD/里程碑证据） | **归档 `docs/archive/`** | 低（3 处主链接） |
| `DESIGN-SPEC-PROGRESS.md` | 历史（旧覆盖台账） | 10-06~10-12 旧仓覆盖记录；不是 oas-core 现行 | `session-memory.config.json`、DESIGN-SPEC、SESSION-INDEX | 中（阅读覆盖证据） | **归档 `docs/archive/`；同步改 config** | 中（skill 配置） |
| `SESSION-INDEX.md` | 历史（旧会话索引） | 902KB；停在 11-01 19:45；对当前 0.1.0 无操作价值 | `session-memory.config.json`、DESIGN-SPEC-PROGRESS、DESIGN-SPEC | 中-高（按主题查旧对话） | **归档 `docs/archive/`；不建议本次直接删** | 中（config/历史检索） |
| `DESIGN-SPEC.md` | 历史（旧仓规格） | 文首已声明旧 backend/frontend、非现行；215KB | `session-memory.config.json`、README:16、PENDING 旧段 | 高（旧仓设计史） | **归档 `docs/archive/`；同步改 config** | 中（skill/旧链接） |

## B2. 分批执行（建议）

### 第一批（本次恢复节点可做，收益最大）

```text
docs/archive/
  DESIGN-SPEC.md
  DESIGN-SPEC-PROGRESS.md
  SESSION-INDEX.md
```

- 3 份合计 **1,158,134 B（65.6%）**；顶层 docs 17→14。
- 建议同时新增 `docs/archive/README.md`，逐份写“归档日期 / 原用途 / 为什么不再现行 / 替代文档”，避免 archive 变成第二个垃圾堆。
- 同一提交必须更新 `session-memory.config.json` 的 `spec` / `progress` / `index` 为：
  ```json
  "spec": "archive/DESIGN-SPEC.md",
  "progress": "archive/DESIGN-SPEC-PROGRESS.md",
  "index": "archive/SESSION-INDEX.md"
  ```
  （`docs_dir` 仍是 `docs`，路径相对于 `docs/`。）
- 更新 `README.md`、`DESIGN-SPEC` 的内部链接（若有）。

### 第二批（发布后/下一次文档维护）

```text
docs/archive/
  CODE-AUDIT-2026-11-01.md
  RECOVERY-AUDIT.md
  MIGRATION-NOTES.md
  DELIVERY-PLAN.md
```

- 4 份合计 **34,843 B**；主要工作是更新 5 处主入口链接，历史价值保留。
- 归档后顶层 docs **10 份**：START-HERE、RESUME、README、SESSION-HANDOFF、ASSET-DATA-MODEL、EDITOR-UX-PLAN、ENGINE-DESIGN、PENDING-LEDGER、RESUME-INDEX、AUDIT-SUBPROCESS。
- 如果目标是进一步缩小仓库体积，可在归档一个发布周期后，**先 zip 备份再 `git rm`** `SESSION-INDEX.md` / `DESIGN-SPEC-PROGRESS.md`；**不建议本次直接删**。

## B3. 备份与移动建议（本次不要执行，供父进程采用）

```powershell
# 1) 仓库外本地备份（不要提交）
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$backup = "D:\OAS-dev\_backup\docs-$stamp"
New-Item -ItemType Directory -Force -Path $backup | Out-Null
Copy-Item D:\OAS-dev\oas-core\docs\*.md $backup -Force
Compress-Archive -Path "$backup\*" -DestinationPath "$backup.zip"

# 2) 用 git mv 保留历史（示例）
New-Item -ItemType Directory -Force -Path D:\OAS-dev\oas-core\docs\archive | Out-Null
git -C D:\OAS-dev\oas-core mv docs/DESIGN-SPEC.md docs/archive/
git -C D:\OAS-dev\oas-core mv docs/DESIGN-SPEC-PROGRESS.md docs/archive/
git -C D:\OAS-dev\oas-core mv docs/SESSION-INDEX.md docs/archive/

# 3) 改 config + README/链接后，用 grep 验证没有悬空引用
```

- **不要**用 `Remove-Item`/`del` 直接删；先 `git mv`，历史仍在 git。
- 如果用户要求“本地备份后删除”，也应先 `git mv` 到 archive，再在一个发布周期后决定是否 `git rm`。
- 本报告只给建议，**未执行以上任何命令**。

## B4. 引用关系清单（移动前必须处理）

| 被移动文件 | 当前主要引用点 |
|---|---|
| `DESIGN-SPEC.md` | `session-memory.config.json`（spec）、`README.md:16`、`PENDING-LEDGER.md` 多处旧段、`SESSION-INDEX.md`、`DESIGN-SPEC-PROGRESS.md` |
| `DESIGN-SPEC-PROGRESS.md` | `session-memory.config.json`（progress）、`DESIGN-SPEC.md`、`SESSION-INDEX.md`、`README.md:17` |
| `SESSION-INDEX.md` | `session-memory.config.json`（index）、`DESIGN-SPEC-PROGRESS.md:80,84,96`、`DESIGN-SPEC.md:497`、`PENDING-LEDGER.md:76,353` |
| `CODE-AUDIT-2026-11-01.md` | `README.md:23`、`RESUME.md:100`、`PENDING-LEDGER.md:2915`、`AUDIT-SUBPROCESS.md:3,32,91` |
| `RECOVERY-AUDIT.md` | `README.md:22`、`START-HERE.md:55`、`RESUME.md:26,77`、`SESSION-HANDOFF.md:29`、`PENDING-LEDGER.md:2906` |
| `MIGRATION-NOTES.md` | `README.md:52`、`PENDING-LEDGER.md:2318`、`RESUME-INDEX.md:76` |
| `DELIVERY-PLAN.md` | `README.md:19`、`START-HERE.md:308`、`ENGINE-DESIGN.md:1817`、`PENDING-LEDGER.md:1062-1064,1105,1108`、`RESUME-INDEX.md:11-14` |

---

# C. 需要立即更新的文档清单

## C0. 发布前必须改（P0）

| # | 文件:行 | 现在 | 应改为/动作 |
|---|---|---|---|
| 1 | `START-HERE.md:32` | 当前任务含 `测试` | 改为只有 `返回庭院`（或写“`测试` 已在 worktree 删除，未提交”） |
| 2 | `START-HERE.md:52-55` | main 恢复节点 + 当前计划读 AUDIT/RECOVERY | 改为 `main=385d611`（发布前）/新恢复节点；当前计划 = `RESUME` + `PENDING §E-72`；把 AUDIT/RECOVERY 标历史 |
| 3 | `START-HERE.md:65-66` | assets=34 | 改为 **16（15 HEAD + 1 未提交） / 1 图**；archive 2827 另述 |
| 4 | `START-HERE.md:68` | 待办含“按 EDITOR-UX-PLAN 做改版” | 改为“首轮完成；剩 SelectionTool 几何合并、drag step shape、collection 真实点击” |
| 5 | `START-HERE.md:224` | `_verify/` 不在 git | 改为“`_verify/audit-20261101/` 已纳入；`_verify/nNN/` 其余工作区证据不入 git” |
| 6 | `START-HERE.md:258` | main 保持 v1.2.3，不自动合并 | 按用户 00:48 新指令改为“构建 release 后同步 main/dev，新恢复节点；发布前先更新文档” |
| 7 | `START-HERE.md:310` | 台账读 §E-24/25/26 | 改为“当前编辑器/版本看 §E-66~§E-72；§一~§D 为旧仓历史” |
| 8 | `RESUME.md:8-11` | seq84 / 23:18 | 覆盖重写为 seq85 / 2026-11-02 00:48+，状态 `in_progress` 或 `done` |
| 9 | `RESUME.md:16-25` | HEAD a2b4b12 / 14 素材 / current 40 / 剩真实设备 | HEAD 7a3869f；assets 16/1；frames current 20；`测试` 删除；真机/真实模式已验收；剩 collection 真点击、drag step、SelectionTool |
| 10 | `RESUME.md:47/56-57/85/111` | 34 是当前 | 标历史；当前 16；archive 2827 未导入 |
| 11 | `RESUME.md:80-100` | 恢复路线待裁定/阻塞 | 改为“用户 21:02 决定不恢复、素材可重建；23:39 数据已提交；archive 保留备查；不阻塞” |
| 12 | `RESUME.md:104` | `_verify/n44` | 改为 `D:\OAS-dev\_verify\n44\session-current-both-full.txt`（或 `..\_verify\n44\...`） |
| 13 | `README.md:26` | 2026-11-01 | 2026-11-02 |
| 14 | `README.md:29-30` | `+ 测试`；lint 2/0 | 只有 `返回庭院`；lint 1/0 |
| 15 | `README.md:32` | 编辑器首轮 | 补 23:39 点击分布/反选/真实测试开关 + 00:02 RO 修复 + 用户验收 |
| 16 | `README.md:39` | main 保持 385d611 | 按新 release 节点更新 |
| 17 | `SESSION-HANDOFF.md:4-6,10-19,27,32-46` | 阻塞态/983c740/34/2/0 | 重写为当前 7a3869f、1/0、16/1、已验收、下一步发布/清理；或整份标“历史，见 RESUME” |
| 18 | `ASSET-DATA-MODEL.md:75-85` | 34 已恢复 | 改为“34 是 §E-70 已推翻的错误恢复；当前 16/1；archive 2827 是权威；脚本/报告仅历史” |
| 19 | `EDITOR-UX-PLAN.md:138-141` | action_roi 读取兼容 | 删除该段，统一为 §10“不保留兼容” |
| 20 | `EDITOR-UX-PLAN.md:293-299` | action_source 待确认 | 改为 §10.2 已确认 |
| 21 | `EDITOR-UX-PLAN.md:534` | 版本待确认 | 改为 E-67 已执行 0.1.0 |
| 22 | `EDITOR-UX-PLAN.md:3/89/220` | 已实现/一套事件/共用采样 | 改为“行为完成；SelectionTool 未抽、drag step bbox、UI/runtime 两套采样” |
| 23 | `EDITOR-UX-PLAN.md:536-545` | 实现前证据 | 改为“19:30 前快照，勿作现状”或删除 |
| 24 | `ENGINE-DESIGN.md:1715/1815` | 22.34 待实现 | 改为“V1.0 已交付，见 §22.35” |
| 25 | `ENGINE-DESIGN.md:1729` | task bundle 含 assets/images | 改为“tasks 只 meta/flow；素材全局 assets/（见 ASSET-DATA-MODEL）” |
| 26 | `PENDING-LEDGER.md:2691` | 版本待确认 | 标“已由 §E-67 执行” |
| 27 | `PENDING-LEDGER.md:2743-2762` | E-68 旧 library UI | 加“该中间态已被 1f73f17 移除；现行无 library UI” |
| 28 | `PENDING-LEDGER.md:2874-2908` | E-70 34/1f73f17/阻塞 | 加“后续：用户不恢复；e749c10 提交用户数据；当前 16/1；不阻塞” |
| 29 | `PENDING-LEDGER.md:2995` | P1-1~P1-7 已修 | 加注“此处 P1-1 指 UI 平移；CODE-AUDIT 的 drag P1-1 仍未修” |
| 30 | `PENDING-LEDGER.md:3030-3036` | RO 需用户硬刷新 | 加“00:37 用户验收：没问题” |
| 31 | `RESUME-INDEX.md:92` | 最后 seq83 | 追加 84（验收修复/真机）、85（RO/7a3869f）、86（docs 审计/0.1.0 恢复节点） |
| 32 | `AUDIT-SUBPROCESS.md:8` | HEAD 983c740/361 | 顶部加“报告范围 21:50；当前 7a3869f/393；最新状态见 §10” |
| 33 | `AUDIT-SUBPROCESS.md:152/167` | task lint 2/0 | 标“当时 2 任务；当前 1/0” |
| 34 | `AUDIT-SUBPROCESS.md:159/168` | 真实设备未做 | 标“§9 已做；用户 00:37 已验收真实模式” |
| 35 | `scripts/package.ps1:5` | 默认 `1.2.3` | 改为 `0.1.0` 或强制 `-Version`；否则发布包命名错 |
| 36 | `packaging/README-DEPLOY.md:1` | V1.2.2 | 改为 0.1.0，并补 0.1.0 变更 |
| 37 | `packaging/start-editor.bat:1` | V1.2.2 | 改为 0.1.0（或去掉写死版本） |

## C1. 发布前还应检查（P1，属于发布流程而非 docs）

1. **`package.ps1` 的 assets 来源**：`scripts/package.ps1:28,75-98` 不复制根 `assets/`，而是 `assets migrate --tasks-dir tasks --library-dir library --assets-dir assets`；当前 `tasks/返回庭院` 没有 `assets.json`、`library/` 为空。发布前必须打开 `dist/stage/assets/assets.json` 确认 16 个全局素材没有丢；否则恢复节点的数据不完整。
2. **`packaging/README-DEPLOY.md`** 是 V1.2.2 的部署说明，不应直接随 0.1.0 包发布。
3. **E2E 证据时效**：`_verify/audit-20261101/D-e2e.md` 执行于 23:03–23:05、HEAD `4d5759f`，未覆盖 23:39 的点击分布/反选/真实开关和 00:02 的 RO 修复；`E-acceptance.md` 有单独浏览器验证，但 docs 把“E2E 36/36”当最终状态时要说清证据边界。
4. **`assets/history-recovery-report.json`** 仍写 `after=34`，与当前 16 不符；它在 `assets/` 下，属于数据区，本次 docs 清理不碰；由数据 owner 决定更新/备份/删除。

## C2. 建议追加的会话口径（现在只存在于 session，未进 docs）

- 拖测 `margin`：默认 0.15，合法 0~0.45，作用于起点（以及配置了 `to` 时的终点）矩形内缩；不是像素、不是拖动距离（session 00:38）。
- `drag`/`swipe` 不写点击分布；`record_click()` 只在 `perform_click()` 路径调用（session 00:38；`lib.rs:1034,1082`）。
- `collection` 仍是集合匹配 dry-run；它和“素材行真实点击”不是同一条路径（session 00:38；`serve.rs:3229-3305`）。
- 真实 UI 手势用户 00:37 说已测试；assistant 独立证据是 `/input/tap` 真机 tap，不是完整 UI 手势回放。文档应区分“用户验收”与“助手实测”。

---

# D. 不建议动的文件（本次清理不要碰）

| 对象 | 原因 |
|---|---|
| `docs/ENGINE-DESIGN.md` 主体 | 现行引擎规格；只做 §22.34 定点修正，不要重排/删段 |
| `docs/PENDING-LEDGER.md` 旧段 §一~§D | 历史台账/证据；append-only，不要为了“简洁”删旧记录 |
| `docs/RESUME-INDEX.md` 旧行 | 规则是只追加；不要改历史行 |
| `docs/AUDIT-SUBPROCESS-2026-11-01.md` §1~§9 | 审计过程证据；只加顶部横幅/补注，不改旧结论 |
| `_verify/audit-20261101/{A,B,C,D,E}.md` | 独立审计/验收证据，已入 git；保留 |
| `assets/`、`frames/`、`tasks/`、`relations/` | 用户数据和关系证据；本任务明确不碰 |
| `assets/history-recovery-report.json` | 数据区陈旧报告；由数据 owner 处理，不在 docs 清理范围 |
| `Cargo.toml` / `Cargo.lock` / `.gitignore` | 构建/版本/忽略规则；只按发布需要改版本，不要删 |
| `scheduler.json` | 当前调度配置（空 entries，loop 模式）；不是文档垃圾 |
| `scripts/dev-serve.bat` / `dev-serve.ps1` / `package.ps1` | 开发/发布入口；package 需改默认版本，但不删 |
| `packaging/*` | 发布脚本/部署说明；需更新版本，不删 |
| `tools/click_heatmap.py` | 文件自述“permanent tool”，仍可用于点击分布可视化 |
| `templates/*.json`、`relations/README.md`、`library/`（空且 ignore） | 现行数据/模板/工具；不要因“空”而删 |
| `docs/START-HERE.md` / `RESUME.md` / `README.md` / `SESSION-HANDOFF.md` | 现行入口；更新即可，不要移走 |
| `docs/ASSET-DATA-MODEL.md` / `EDITOR-UX-PLAN.md` | 现行模型/编辑器设计；更新即可 |
| `docs/AUDIT-SUBPROCESS-2026-11-01.md` | 当前审计链；保留 |
| `D:\OAS-dev\_verify\n44\session-current-both-full.txt` | 本次审计原始证据；不要移动/改写 |

## D1. 可列入“后续可选清理”的非文档文件（本次只列，不删）

| 文件 | 判断 | 建议 | 风险 |
|---|---|---|---|
| `scripts/recover_historical_assets.py` | 一次性 34 素材恢复脚本；其“扫全部 git task 历史”前提已被 §E-70 推翻，误跑会再次制造 34/非权威库 | 先备份到 `D:\OAS-dev\_backup\`，再从 repo 移除/移到 archive；同时删 `ASSET-DATA-MODEL §5` 对它的引用 | 中（历史工具，可能有人想复现） |
| `scripts/audit_asset_history.py` | 只读审计脚本；仍可用于查历史 task assets | 保留或随 recovery 脚本一起归档 | 低 |
| `tools/bench_cv2.py` | 一次性 OpenCV 性能试跑；无引用、无测试依赖 | 可备份后删除/归档 | 低 |
| `tools/refgen_cv2.py` | NCC 参考值生成器；无引用，但可复现 perceive 对照 | 保留（体积小）或归档 | 低 |
| `assets/history-recovery-report.json` | 记录 `after=34`，与当前 16 不符 | 数据 owner 备份后更新/删除；不在 docs 清理中处理 | 中（数据区） |
| `packaging/README-DEPLOY.md` | 版本停在 V1.2.2 | 更新为 0.1.0；不要删 | 低 |
| `packaging/start-editor.bat` | 注释 V1.2.2 | 更新注释；不要删 | 低 |

---

# E. 附录：测量与命令

## E1. 实测命令（本次只读）

```text
git -C D:\OAS-dev\oas-core rev-parse HEAD
  -> 7a3869f9f992d74c50928b18bc4cbb0a70fae710
git -C D:\OAS-dev\oas-core branch -avv
  -> dev 7a3869f [origin/dev]; main 385d611 [origin/main]; archive 29b696f
git -C D:\OAS-dev\oas-core status --porcelain
  -> M assets/assets.json
     M frames/frames.json
     D tasks/测试/assets.json
     D tasks/测试/flow.json
     D tasks/测试/meta.json

cargo test --workspace
  -> 393 passed / 0 failed（12 个有测试的 suite：3+2+49+3+16+6+2+113+2+55+2+140）
node apps/oasd/ui/ui_pure_test.js
  -> dom_id_audit: 155 referenced ids, 0 dead ids
  -> ui_pure_test: 203 checks passed
oasd task lint --tasks-dir tasks --all
  -> lint task=返回庭院 ok
  -> linted=1 errors=0
oasd --version
  -> oasd 0.1.0
```

## E2. 数据计数

```text
assets/assets.json:
  16 items
  15 ocr_line + 1 template
  16 action_source=fixed
  16 action_shape
  16 positions
  images: 1 (a001.png)

frames/frames.json:
  28 frames, next_id=41, current_id=20
  categories: 式神录 11, 庭院 2, 探索总界面 2, 御魂 2, 弹窗 2,
              启动 2, 空 1, 町中 1, 斗技 1, 结界突破 1, 逢魔之时 1,
              悬赏封印 1, 设置 1

tasks/:
  返回庭院/flow.json 6212 B / 11 steps
  返回庭院/meta.json 400 B
  （测试/ 已删除未提交）

archive/migration-20261101:
  assets 2827
  images 2099
  task dirs 57（meta 57 / flow 56；docs 写“55 任务”，需复核）
```

## E3. docs 体积

```text
docs/*.md 合计 1,765,329 B（17 份）
建议归档 7 份合计 1,192,977 B（67.6%）
  其中 SESSION-INDEX 923,775
       DESIGN-SPEC 220,251
       DESIGN-SPEC-PROGRESS 14,108
       DELIVERY-PLAN 18,800
       CODE-AUDIT 8,390
       RECOVERY-AUDIT 5,571
       MIGRATION-NOTES 2,082
归档后顶层 10 份 / 约 572,352 B
```

## E4. 报告边界

- 本报告只审计 `docs/*.md`、仓库根/目录的非代码文件、代码关键路径与当前数据；**没有**重新跑 E2E、**没有**重新跑真实设备 UI 手势、**没有**审计 `target/dist/logs/runs` 生成目录。
- 代码引用行号基于 `7a3869f` 工作区；docs 行号基于当前未改 docs。
- 所有“建议”均为计划，本报告没有执行 `git mv`/`git rm`/`Compress-Archive`/任何 commit。
