# 新会话入口（START HERE）★ **只要读这一个文件**

> ★★★ **给用户的一句话**（新会话开场直接粘贴）：
>
> ```
> 先 skill session-memory，然后读 docs/START-HERE.md，严格按它执行，最后按它的模板回报。
> ```
>
> ★ **不需要再说别的** —— 本文件把"读什么、做什么、回报什么"全写死了。

---

# §0 ★ 我是谁 / 这是什么项目（2026-10-24 口径）

```
★ 项目：OAS（OnmyojiAutoScript）—— 阴阳师自动化引擎 + 数据驱动编辑器
★ 当前主线 = 新引擎 **oas-core**（Rust workspace，`D:\OAS-dev\oas-core`，`dev`；发布时同步 `main`；远端 `https://github.com/passioki/oas-core`）
★ 旧仓 `backend\` / `frontend\` = **已冻结，只作资源库**（资产/知识/历史证据）
  ⇒ ★ 不在旧仓上开发；旧仓只读；旧仓文档里的"待办"与引擎无关的不要动
★ 开发工作区：`D:\OAS-dev\`（oas-core + backend + frontend 并排）；oas-core 文档已独立到 `oas-core\docs`
★ 文档通道：oas-core/docs 随 oas-core `dev` 提交；backend 文档仓已冻结，仅历史参考；oas-core `main` + tag/release 已推 GitHub
★ 实机：另一台电脑（本机没有 D:\OAS-live）⇒ 本机只做开发/回放/模拟器验证
★ 唯一真相 = git + 磁盘证据；**不信"我记得"** ⇒ 必须贴 `文件:行号` / commit / 测试输出
```

## 0.1 当前状态快照（2026-11-07 08:2x · **seq 132 · ✅ done：集合成员扩展到所有素材（template/OCR/ocr_var/region）**）

| 项 | 状态 |
|---|---|
| 分支/HEAD | `dev`；代码 HEAD = **`7229c59`**（seq132；dev/main 已同步）；PR1~PR7 全部 ✅（哈希见 `PENDING-LEDGER §E-79`）；**未 tag / 未 release** |
| 引擎 | `oasd`（studio/runner/run/queue/stats/scheduler）+ 编辑器 UI + 素材库；`oasd --version` = **0.1.0**（dev；旧 v1.x 为历史内部 tag） |
| 测试 | `cargo test --workspace` **980 passed / 0 failed**（23 suites；perceive **14** / flow **82** / runtime **226** / task **160** / adapter-pc 5 / oasd **458**）；UI pure test **2407 checks / Studio 190 ids / Runner 158 ids / 0 dead / 6 nav**；集合 API 冒烟 `_verify\seq131-collection-smoke\`（create/list/delete 200）；`cargo check -p oasd --features ocr` exit 0 |
| 分包 | **Studio** `dist/oasd-v0.1.0-win64.zip` = **76,670,889 B**，SHA256 `F21843810EF94859DCA57CF172944FDB618DD19B058F11770DF8D236C3471B2F` · **oas-core Runner** `37,860,300 B`，SHA256 `163679D6099AA35ED67A54AD0AAA11AACFCF9275B657C00AEA9C5E2708CC9065` · **oas-runner 独立包** `37,870,658 B`，SHA256 `F6A6C3012AB20BF6CBB4564EC5E9735DF728C0C35DA8950751727415B392D46C`（均不入 git；**本批未重打，且旧 package_schema 包会被新引擎拒绝**） |
| 独立仓 | **`oas-runner`** = `https://github.com/passioki/oas-runner`（`768a8b7`；pin `ba6813e`，**未随 seq126~130 更新、未打独立包**） |
| 最终审计 | **`docs/AUDIT-2026-11-05.md`**（代码审计/文档漂移对照/残余风险/交接/关机） |
| 双入口 | **Studio `http://127.0.0.1:8788/`**（`oasd studio`，编辑/构建/发布，**无调度页/无重置记忆**）· **Runner `http://127.0.0.1:8789/`**（`oasd runner`，nav 6 项：概览/已安装任务/安装包/队列与调度/运行记录/设置；**完整调度 UI + 重置记忆**；启动即内嵌调度，`--no-scheduler` 关闭）；`oasd serve` = Studio 的 deprecated 别名 |
| 当前状态 | **所有服务已停止**（本轮验证/E2E/冒烟后确认无监听、无 `oasd` 进程）。启动命令见 §1.3/§6；**开发启动器 = `scripts\dev-serve.bat`（默认 Studio 8788；`-Surface runner` → Runner 8789）**；仓库根 `start-runner.bat` 为 Runner 快捷启动 |
| 发布 | GitHub Release `v0.1.0` 仍是**历史节点**（tag `v0.1.0` → `e8edbef`，**未移动**）；**本批未 tag / 未 release** |
| 当前任务 | **seq131 已完成并推送**：恢复「加入素材集合」入口——素材行「设置→加入集合」（template 专用）+ `#collection-add-dialog`（选择已有集合/新建集合；跨集合自动移出旧集合；保留 threshold/action_shape；保存后刷新两侧素材库并提示 tasks_rewritten）；任务库/全局库/帧详情三处接线。见 `PENDING §E-99`。工作区 `tasks/` 用户测试配置（不入库；用户可能随时编辑/删除，**不要提交/恢复**） |
| 数据 | 已提交基线：全局素材 **16 条 / 1 图**（15 `ocr_line` + 1 `template`）；帧 **28 个 / `current_id=20` / `next_id=41`**。工作区未提交用户数据：`assets/assets.json`、`frames/frames.json`、`scheduler.json`(+`.bak`)、未跟踪 `frames/frame-43.png`/`frame-44.png`、`tasks/切换预设御魂/`——**用户数据不入库，勿提交** |
| 引擎特性 | flow DSL、素材/掩膜/多边形、点击定位链、HB1/HB7、整帧搜索、单识别 ROI + 操作区域、库、queue、stats、checkpoint/resume、reconnect、多帧投票、OCR+模板互证、统一任务模板/task lint、任务创建/编辑器运行、调度 v3、交付包 `.oaspkg`、Studio/Runner 双入口、内嵌调度/设备租约、子任务参数配置 |
| 已知限制 | 见 `RESUME.md §5` / `SESSION-HANDOFF.md §4`；核心：`installed.json` 损坏无自动隔离、租约 pass 级、install/activate 调度检查有 check-then-act 小窗口、panic 安全依赖 unwind、PR7 8 条 UI P2、`start-runner.bat` 固定 8789、跨平台仅设计预留 |
| 历史交付 | V1.0~V1.2.3 是 reset 前内部交付记录（tag/Release 在 GitHub 历史里）；当前 dev = 0.1.0，恢复节点见 `RELEASE-NODE-2026-11-02.md` |

## 0.2 当前任务台账（2026-11-05）

- **Studio/Runner 分离 + TaskPackage + 任务配置/子任务配置**（`oas-core-runner-studio-split`，seq 108，**handoff-ready**）。
- **Q1~Q7 已批准**（2026-11-04，全部按推荐）；**PR1~PR7 全部 ✅ 已提交**；**最终审计 4 个代码 P1 已修复（`68e5768`）**；交接 docs 紧随其后（seq 108）。
- 台账：`PENDING-LEDGER.md §E-79`（§F PR1 / §G PR2 / §H PR3 / §I PR4 / §J PR5 / §K PR6 / §L PR7 / **§M 最终审计与修复**）；现行口径：`RUNNER-STUDIO-SPLIT-PLAN.md`（§A.9~§A.16）、`ENGINE-DESIGN.md` §22.44~§22.51。
- **2026-11-05 13:3x（seq 110）双端发行包 + 分离度验收**：`scripts/package.ps1` 重写为 **Studio-only**（旧全量包 9 项不符全消）；Studio/Runner 两 zip 产出 + **E2E 75/75**（62 分离度 + 13 F1 回归）；**F1 修复**（PS5.1 BOM → Runner 包 `scheduler.json` 首启 `invalid_config`）；`scripts/dev-serve.*` 双入口化（8787 退役）；**参数 UI 审计完成、待裁定**（`_verify/ui-params-audit-20261105/PARAMS-UI-AUDIT.md`，推荐 A+C）。台账 `§E-80`；实现记录 `ENGINE §22.52` / `PLAN §A.17`。
- **2026-11-05 18:2x（seq 111）双端架构升级设计轮（只读，未改代码）**：用户四项新要求 → 3 路只读设计 + 可点原型 + 决策单（仓外 `_verify\dual-app-design-20261105\`）：① **参数 v2**（动作库消费类动作 + `text_expr` 让参数可作为 OCR 文字；group/预设；Runner 每参使用开关、留空=不注入）② **使用端独立新仓**（推荐两仓：oas-core / 新仓 `oas-runner`，git 依赖 tag；Phase 0 现仓先做）③ **发布通知**（exchange 目录真相 + loopback POST 加速）④ **发行包带数据**（Studio 全数据 + Runner bundled-data 首启不覆盖）。**待用户回复 `DECISION-SHEET.md` D1~D12**（未回复按推荐）。
- **2026-11-05 18:4x（seq 112）P0 首批落地（已实现+验证）**：① **两包带数据**（用户要求①）——Studio 包补 frames/relations/templates，Runner 包新增 `bundled-data/`（manifest+sha；`-NoData` 可精简）；② **Runner 首启导入**（`bundled_data.rs`：空数据根才导入、永不覆盖、幂等、失败回滚）；③ **exchange 发布通知**（用户要求④）——Studio publish 写 `publish-index.json`+incoming，Runner `GET /runner/updates` + 安装包页「新发布（来自 Studio）」徽标/列表/一键安装/忽略，`POST /task/publish` 响应带 exchange 结果；④ 测试 **746/0**、UI **1633/159/157/0/6**、两 release 仓外 E2E **ALL PASS**。台账 `§E-82`；实现记录 `ENGINE §22.53`。
- **2026-11-05 19:4x（seq 114）使用端参数 UI + 更新检测 feeds + oas-runner 独立仓**：① 按用户指令创建 **`https://github.com/passioki/oas-runner`**（public，initial `e1d603a`；Phase 1 搬迁前骨架）；② Runner「配置」参数页：按 `meta.params[].group` 分节、每参使用开关、bool 三态、留空/不使用=不注入、required 阻止保存、摘要（**不做预设**，D2）；③ 更新检测（D8）：`config.updates.feeds` 支持**本地路径/HTTP(S) URL**，Runner `GET /runner/updates` 合并交换所+feeds（来源/需下载字段），`POST /runner/updates/install {event_id}` 远程用**系统 curl** 下载+sha256 校验+安装+ack；`scripts/make-update-feed.ps1` 生成 feed（可上传 GitHub Release/静态站点）；④ 验证 workspace **788/0**、UI **1799/159/158/0/6**、本地+HTTP feed E2E 与 P0/参数 E2E **ALL PASS**。台账 `§E-84`；实现记录 `ENGINE §22.55`。
- **2026-11-05 20:0x（seq 115）Phase 1a + Studio 权威 lint**：① **oasd lib+bin**（新 `apps/oasd/src/{lib.rs,cli.rs}`；`main.rs` 641 行→404B；`pub fn run_as(surface)`；20 模块 `pub mod`；测试迁 lib target 后 workspace **794/0**、warnings **5→1** 无新增）；② **oas-runner 独立薄应用**（`apps/runner` 3 行 main = `oasd::cli::run_as("runner")` + Runner UI + Cargo.lock；path 依赖构建 exit 0、`:8792` smoke `/runner/health`/`/runner/tasks` 200、`/app.js` 404；无需 `[workspace]` 空表）；③ **Studio 权威 lint**：`GET /task/lint`（Studio-only；CLI/HTTP 共享 `lint_one`；发现 200、缺任务 404、坏文件 400 带路径、Runner 404）+ 构建页「检查」按钮/计数/面板/T3 去声明/降级；④ 测试 UI **1868/159/158/0/6**、P0/参数/更新 feed E2E 全绿；新包 SHA 见 §0.1。台账 `§E-85`；实现记录 `ENGINE §22.56`。
- **2026-11-05 20:1x（seq 116）Phase 1b：oas-runner 独立打包**：① oas-core `oasd::cli::run_as_with_args`（默认参数注入，用户可覆盖；`run_as` 委托）；② oas-runner main 默认 `--ui ui`、pin `rev=802f58b`；③ 新仓 **`scripts/package-runner.ps1`**（数据/工具链探测、cargo release build、stage exe/ui/models/adb/**bundled-data 生成**/live/README/BUILD-INFO/start-stop bat、BOM/白名单/rev 审计、`/` zip、`-DryRun`）；④ **独立包 smoke 50/50 PASS**；⑤ 两仓提交：oas-runner `91785b8`、oas-core 本批 docs；未 tag / 未 release。台账 `§E-86`；实现记录 `ENGINE §22.57`。
- **2026-11-05 20:4x（seq 117）更新检测收尾 + 参数编辑补齐**：① **feed TTL 缓存**：`updates.cache_secs` 默认 60（0=不缓存）；命中不 curl/不读文件、失败不刷新（下次重试）；`GET /runner/updates?refresh=1` 绕过、坏值 400；② **远程包复用**：`packages/remote` size+sha256 匹配 → `downloaded:false,reused_cache:true`；篡改删重下；③ **`POST /runner/updates/cache/clear`**（Runner-only 清包+缓存）；④ **参数编辑补齐**：文字来源三态覆盖 click_target.find/requires/expect/drag.from/drag.to，T3「去声明…」预填 key；⑤ 测试 workspace **801/0**、UI **1919**、P0/更新 feed/参数 E2E **ALL PASS**；oas-runner rev bump `78be71d` 独立包 **37,871,134 B / `B127C0FC…66DB`**。台账 `§E-87`；实现记录 `ENGINE §22.58`。
- **2026-11-05 21:1x（seq 118）动作目录统一 + 配置项成熟 UI + 最终审计/交接**：① **动作目录单一事实源** `actionCatalog()`（OP_META+PARAM_ACTIONS）——调色板与 if/loop「是/否/循环体」分支下拉全部统一，参数预设未声明时禁用+原因（修复用户反馈「动作库有、衔接下拉没有」）；② **配置项结构化表单**：帮助块 + 每参卡片（字段帮助+示例、类型控件、enum/min/max/required/role、上下移/删除、空态示例）+ 保存前语义校验 + 高级 JSON 双向同步 + T3 prefill 行；③ **最终审计** `docs/AUDIT-2026-11-05.md` + 全套文档同步（防漂移）；④ 验证 workspace **801/0**、UI **2028/164/158/0 dead/6 nav**、三路 E2E ALL PASS；oas-runner 独立包 `F6A6C301…D46C`（pin `5b0b107`）。台账 `§E-88`；实现记录 `ENGINE §22.59`。
- **2026-11-06 07:3x（seq 119）参数插入/条件表达式修复 + 参数点击素材选择**：① 「插入参数 ▾」防粘连（cond 末尾自动 ` and `、标识符边界补空格；`exprInsertText`/`insertTextAtField`）；② `repairConditionExpr` 自愈历史 `truetext_visible(...)`，`normalizeStepArgs` 载入/应用/保存前执行并清 if/while 遗留 `args.sel`；③ 参数点击不再静默选第一个 OCR 素材：上下文复用→唯一素材直用→**多个弹选择对话框**（可取消），调色板/分支共用 `prepareActionStep()`；④ 用户任务 `tasks/切换预设御魂/flow.json` 条件修为 `text_visible(param('group'))`（保留其两参数/if 分支，工作区未提交）；⑤ UI **2039/164/158/0 dead/6 nav**、只推送不发布（未重打 release/tag）。台账 `§E-89`；实现记录 `ENGINE §22.60`。
- **2026-11-06 08:1x（seq 120，设计）操作区域/动作能力设计草案**：`docs/OPERATION-REGION-DESIGN.md`（现状对照/统一 region 模型/长按建议/曲线限制/分期计划 + 4 个待确认问题）；commit `531b77b`。
- **2026-11-06 09:0x（seq 121）region 操作区域 + long_press + curve_swipe + 等待语义说明**：① `region{mode:asset|ocr|nearest|offset,offset:[dx,dy(,w,h)]}` 挂 click_target OCR/drag OCR 端点/long_press.find；nearest=行 bbox 中心距 **OCR 识别 ROI 中心**最近；offset=行 bbox 左上角偏移；无 region=现状；② `long_press` 独立 op（hold_ms 默认 **3000**、hold_range 随机拟人、InputSink 默认同点 swipe、ADB `input swipe x y x y ms`、不记 clicks）；③ `curve_swipe`（points 2..=32 多段独立 swipe，日志注明）；④ flow_ops **7→11**（新 `long_press`/`curve_swipe`/`click_target.region`/`drag.region`）；⑤ 构建页动作库长按/曲线/参数长按、操作区域下拉+offset、保存前校验、**等待动作帮助**（阻塞等待→下一步；超时失败；不触发独立动作）；⑥ workspace **824/0**、UI **2146/164/158/0 dead/6 nav**、E2E `regiontest` lint 0/publish flow_ops 4 新标记；oas-runner pin `644249c`（`7e64156`）；只推送不发布。台账 `§E-91`；实现记录 `ENGINE §22.61`。
- **2026-11-06 09:4x（seq 122）A 方案：region.asset（全库操作区域素材）+ 多候选 nearest**：① `region.asset`（仅 asset/nearest；T4 收集；flow_ops 11→13 +`click_target.region_asset`/`drag.region_asset`；旧 Runner 拒绝）；② oas-perceive `match_template_all`/masked_all（NCC→3×3 局部极大→阈值→NMS→≤64；best 不变）；③ runtime `operation_region_for_asset`：模板/集合 best/nearest（nearest=候选匹配 bbox 中心离**参数 OCR 匹配行 bbox 中心**最近）；固定 region/OCR 资产取自身操作区；无候选/未匹配=响亮失败；④ 三处接入 click/drag/long_press，无 asset 旧行为逐字节不变；⑤ UI 对话框/结构化编辑器加「操作区域素材（全库，可选）」+ 校验/风险提示；⑥ workspace **841/0**、UI **2214/164/158/0 dead/6 nav**、E2E `regionasset` lint0/publish flow_ops 含 region_asset；oas-runner pin `ba6813e`（`768a8b7`）；只推送不发布。台账 `§E-92`；实现记录 `ENGINE §22.62`。
- **2026-11-06 12:0x（seq 123，仅文档交接）用户 6 项新要求（未实现）**：① 动作库删参数预设（「参数文字点击」→「参数文字动作」，最终参数不属于动作库）；② **参数=素材的一种**（使用端可改识别文字；初始识别区为空；改名「使用端OCR文字配置项」待定）；③ 素材行点识别返回**所有候选+评分**（后端全 API；测试动作演练所有候选；真实点击可选候选）；④ **量尺**（两点直线距离复制剪贴板）；⑤ **操作区域三方式+递归套娃**（参数真实识别区/素材操作区单或多候选最近/相对偏移；素材链依次识别、任一缺失本步失败；防环）；⑥ **废弃操作区“与真实识别区有无交集”两状态**（先审计全图回退+相对位移）。**本轮仅更新文档**（`PARAM-ASSET-REDESIGN.md` / `PENDING §E-93` / `ENGINE §22.63` / `RESUME` seq 123 / 交接单）；代码仍 seq 122（workspace 841/0、UI 2214、`ba6813e`）。新会话先审计 + 确认设计文档 §5 的 7 个问题，**先不要继续改 A 方案 UI**。
- **2026-11-06 12:1x→13:3x（seq 124，✅ 已实现并推送）参数→素材 / 操作区递归 / 候选识别 / 废弃两状态**：用户 12:1x 给出 6 事项权威逐字澄清（`PARAM-ASSET-REDESIGN §1b`；事项4 由量尺改为「相对偏移时画模拟识别区/操作区，操作区大小按绘制」），12:5x 简化令「全新开发、不兼容旧参数/旧配置/旧包」。① 3 路只读审计（`_verify\paramasset-audit-20261106\`）：事项6 确认两状态只在数据/UI 层、`oas-runtime` 0 读 `action_source`、模板全帧金字塔回退+操作区平移已存在 ⇒ 删除；② 逐项实现：动作库删 `PARAM_ACTIONS`；新素材 kind **`ocr_var`**（`runtime_text`/`param_key`/`label`/`roi` 可空=全屏 OCR）；**资产级 `op_region`** 三方式 `self`/`offset`/`asset+pick`（递归链、DFS 深度16+运行时 visited 防环、region 末端固定区，运行时依次识别、任一 miss=本步失败不点击）；`meta.params` 由单一 sync 派生带 `asset_id` 行（C1′，运行/values/`param()` 链路零改动）+ T8 lint；候选 API `GET /task/asset/candidates`（模板/集合/OCR/ocr_var，空 roi=全屏）+ `POST /task/asset/test_action`（dry/real，click+long_press；drag 400 留后续）+ `POST /task/asset/validate_op_region`；③ Studio：素材行「识别=全部候选」、识别文字对话框、操作区递归对话框+offset 画布绘制、动作目标=素材库搜索选择、旧 region 仅旧 flow 兼容；Runner：派生行「使用端OCR文字配置项」分节；`action_source` 降级 deprecated 不再写；④ 验证：workspace **904/0**、UI **2512/194/0 dead/6 nav**、E2E **27/26/0/1**（无 ADB skip）；代码 commit `aab6372`；文档 `ENGINE §22.64` / `OPERATION-REGION-DESIGN §8` / `PARAM-ASSET-IMPLEMENTATION.md`「实现结果」；**未 tag/未 release**，后续项（drag 测试/长按测试入口/ROI 画布/S8 只读卡片/op_rect/真机 E2E/Phase2）见 `PENDING §E-93 §D.6`。
- **2026-11-06 18:1x→19:4x（seq 126，✅ 已实现并推送）13 条验收反馈整改 + 变量系统 v2 设计**：用户 18:1x 提 13 条验收反馈 → 3 路只读排查（`_verify\acceptance-20261106\`）+ 计划确认「全按推荐」→ 批次 A（#11 OCR 数字保护+候选严格、#9 replace 继承+payload、#5 ocr_var 阈值、#6 多边形显示、#7 Runner 启动脚本）/ B（#4 操作区**模型 A**：链只作识别门槛、落点=根素材 self、被引用素材只读、offset 删除、OCR self 跟随识别文字；polygon v3 三态 + ocr_var 掩膜；普通 OCR 链节点期望文字 + preview det+rec）/ D（Studio/Runner 用户界面**彻底移除参数概念**，服务端派生模型保留内部桥；goal battles 字面量；D2 不再自动补 params.max_battles）/ B3a（操作区对话框模型 A、删 offset 绘制、拖测候选、素材行长按测试）/ B3b（**统一画布框选**：识别区域/操作区域/反选 3 按钮，拖动=矩形/单击=多边形/双击收尾，polygon 保存层不重截图）/ E（GitHub About 中文重写、`D:\OAS-dev` 24.6GB→7.79GB 清理）/ F（`VARIABLES-V2-DESIGN.md` 变量系统 v2 设计，10 项按推荐）；④ 验证：workspace **929/0**、UI **2217/177/0 dead/6 nav**、E2E **54/53/0/1**；commits `c16a536`/`4ccd182`/`07ed361`/`c3739f6`/`9a3c77f`/`303a7ac`；文档 `ENGINE §22.65` / `OPERATION-REGION-DESIGN §8` / `ACCEPTANCE-2026-11-06.md` / `VARIABLES-V2-DESIGN.md`；**未 tag/未 release**。
- **2026-11-06 20:0x→20:2x（seq 127，✅ 已修复推送）用户验收纠正：多候选全帧 + 操作区链末端模型 B**：用户 20:0x 反馈 3 条（① 多候选仍未生效：素材 `test` 在 #6 帧应有多个却只显示 1 个；② 引用素材时落点应为**最后一个被引用素材自己的操作区**，只有 self 才用自身；③ 组合场景 `预设识别`→`test` nearest→点击 test 不可用）。根因（用户数据副本实测）：`test`=template、roi=[973,144,35,38]，候选枚举只搜素材 roi → 1 个；全帧搜索 3 个（y=144/295/445，score 1.0/.973/.905）；上一轮把链误改为模型 A。修复 `fd3339f`：perceive 全帧多候选（4× 金字塔 coarse→refine→NMS；1280×720/35×38 debug 114ms/release 6.4ms，原 7.4s）+ runtime 候选/链 pick 全帧 + walk/resolve/preview 恢复末端 self 几何 + UI 文案动态；Lead 实测 `test_action 预设识别→[973,445,35,38]`、队伍1→`[973,295,35,38]`、test→`[973,144,35,38]`。验证：workspace **934/0**、UI **2223/177/0 dead/6 nav**、E2E seq127 **63/62/0/1**；**当前 V2.0 变量系统实施中**。
- **2026-11-06 20:3x→20:4x（seq 128，✅ 已实现并推送）变量系统 V2.0 实现（用户剩余项 #1）**：按 `VARIABLES-V2-DESIGN.md`（10 项按推荐）分三批：V2.0a `e1b816f`（`Variable`/`meta.variables` 唯一真相、旧 params 内存桥/T10、`sync_ocr_var_variables`、T3/T7/T8、loader 子任务/run 注入、`PACKAGE_SCHEMA=2`+`variables_schema` 旧包拒绝）；V2.0b `d780a4b`（`GET/POST /task/variables`、`/task/config` deprecated 别名、Runner config variables/PUT 仅 `runner_editable&&scope=input`、去 params 双写、test_action/create/publish/CLI 迁移）；V2.0c `d780a4b`（Studio 变量面板/变量对话框/「插入变量 ▾」/动作目标 key 集；Runner 两分节可改过滤/required 提示/空态/高级 JSON 隐藏）；④ 验证：workspace **949/0**、UI **2303/177/0 dead/6 nav**、E2E seq128 **77/76/0/1**、Lead 用户数据副本冒烟（POST 后 meta.json 无 params）；文档 `ENGINE §22.67`/`VARIABLES-V2-DESIGN §10`/`ACCEPTANCE` V2.0 结果；**未 tag/未 release**。
- **2026-11-06 21:26→22:0x（seq 129，✅ 已实现并推送）动作库一致性：拖动改素材端点 + 删除坐标输入 + 全局素材 lint 修复**：用户 3 条反馈（拖动 UI 无素材选择/应与点击素材一致；删除拖动坐标输入，坐标通过操作区域素材完成；排查其余动作库动作）。实现 `dce16c7`：`DragTarget::Asset{sel}`（from/to，旧 region/point/ocr_text 兼容）；runtime `drag_asset_endpoint` 复用点击素材同源（素材识别→根 self 几何→op_region 链模型 B 末端盒），未命中 Err 不 swipe，采样/方向/24 次 to/duration/margin/audit 不变；package flow_ops `drag.asset`（旧 Runner 安装拒绝）；UI `structuredDrag` 重写为起点/终点素材选择器（与点击素材一致）+ 清空终点 + 方向/距离/时长/margin，**删除坐标 ROI/端点编辑器/样式/断言**，旧端点只读提示，动作库文案/提示统一审计（click/click_target/long_press/dismiss_popup/swipe/curve_swipe/click_point）。附带修复 `82dbca3`：全局素材 T8 lint 假红（`lint_into_with_calls_and_global_assets`/`merge_global_assets_for_lint`，lint/publish/install 合并 global+包快照视图，install 包内优先）。验证：workspace **968/0**、UI **2323/177/0 dead/6 nav**、冒烟 `_verify\seq129-smoke\` save→lint→publish→全新 Runner install→安装后 lint 全绿；文档 `ENGINE §22.68`/`ACCEPTANCE` seq129 追加。
- **2026-11-06 23:04→23:1x（seq 130，✅ 已实现并推送；用户要求完成后关机）点击几何同源修复 + 框选可移动/叠加/旧参数误判/自动刷新**：用户 5 条反馈（框选可否点框移动/点击叠加分布失效/条件行旧参数来源 param('p_93445f68')/任务页素材自动刷新/真实点测与运行点击失效）。#5 根因（runs.clicks 日志）：OCR 点击 target rect 已 Q1 随识别行平移，但采样 `adjust_plan_for_shape→translated_click_shape` 只平移 template、OCR 返回作者绝对 action_shape → 点击被拉回作者区（用户素材识别行 y245/538，点击 y72..122）。修复 `a3556e3`：`click_geometry_for` rect+shape 同源（OCR 用 `ocr_self_operation_geometry`；roi=None 绝对；template/collection/region 不变），统一 click/click_target/long_press/interrupts/dismiss_popup/region.asset；dry 预览 action_box=[1103,245,120,50]（用户数据，识别行 y257）。#1 画布点框选中+整体平移（rect/polygon/反选/候选编辑区，clamp/Esc/未保存提示）；#2 点击叠加 actual/target/miss 热力+目标环+miss 叉+缓存/刷新/图例；#3 param(key) 已声明→变量来源（仅未声明旧参数只读）；#4 任务/构建页素材 8s 轮询+可见/聚焦/隐藏暂停+保持状态。验证：workspace **972/0**、UI **2370/178/0 dead/6 nav**。
- **2026-11-07 08:10→08:2x（seq 131，✅ 已实现并推送）恢复「加入素材集合」入口**：用户反馈加入集合 UI 消失（原因：`3eb9fa8` 清理未使用「多选素材+新建集合」UI 时移除；后端 `POST /task/asset/collection` 仍在）。修复 `eaf68fe`：素材设置菜单「加入集合」（template 专用，位于位置帧后；其它 kind 置灰说明）+ `#collection-add-dialog`（已有集合下拉/新建集合 id/成员与当前集合提示；跨集合先移出旧集合、剩 0 成员删除，被 flow 引用按 400 显示；保留 threshold/action_shape；成功后刷新任务库/全局库并提示 tasks_rewritten）；任务库/全局库/帧详情三处 onCollectionAdd 接线。验证：UI **2391 checks/190 ids/0 dead/158 runner/6 nav**、node --check 全绿；数据副本 API 冒烟 create/list/delete 200。
- 下一步（下次开机）= **① Firefox 手测**（框选点框拖动/撤销保存、点击叠加热力+目标环+miss 叉、条件行不再误报旧参数、任务页素材自动刷新、真实点测与运行点击落点随识别行）；② 全屏 ocr_var（roi=None）绝对坐标限制说明、旧 `shape_aware` 死分支清理；③ **V2.1**（变量表达式/条件/循环/T9）、**V2.2**（Profile/多模拟器）；④ oas-runner 独立仓根启动脚本 + pin bump 重打独立包；⑤ 真机 E2E、T5 手写 polygon 校验、更新检测鉴权/GC、tag/Release 策略。**不 tag / 不 release**（除非用户明确要求发布）。

---

# §1 ★★★ 强制步骤（顺序不许变）

## 1.1 加载 skill（上下文压缩后必做）

```
★ 调用 `skill`，name = `session-memory`
★ 它的五条铁律（常驻区块 / 台账 / 逐步法 / 完成定义 / 双向读取）在压缩后会丢
★ 若技能目录没有 `session-memory`：装 `passioki/dsh-skills`（install.bat），或直接读
  START-HERE + SESSION-HANDOFF + ENGINE-DESIGN 三份磁盘文档
★ 工作目录必须是 `D:\OAS-dev\oas-core`（否则相对路径落空）
```

## 1.2 读这 4 份文档（按顺序）

| 序 | 文件 | 读它为了什么 |
|---|---|---|
| ⓪ | `docs/RESUME.md` + `docs/RESUME-INDEX.md` | ★ **长任务/压缩后恢复的第一入口**（skill 强制 6）：当前断点（seq 108）、验收清单、下一步精确命令、证据索引 |
| ① | `docs/SESSION-HANDOFF.md` | 当前在哪、下一步、最近 3 轮变更、活跃坑（一页纸） |
| ② | `docs/ENGINE-DESIGN.md` **§22** | 引擎规格/进度/判据；§22.34 调度 v3 **已交付**；当前素材/关系模型看 §22.38/§22.39；**PR1~PR7 + 最终审计实现记录看 §22.44~§22.51**（§22.51 = 最终审计与交接）；**seq124 参数→素材/op_region 见 §22.64；seq126 验收整改/模型 A/统一框选看 §22.65** |
| ③ | `docs/PENDING-LEDGER.md` 的 **§E-66~§E-79** | 编辑器 UX/版本重置/任务素材架构/34 事故/静态审计/文档归档/Release+O-1/O-2/调度器产品化/帧页/两按钮+push-all/素材库 UX/**§E-79 = Studio·Runner 分离**（Q1~Q7 已批准、PR1~PR7 全部 ✅、**§M 最终审计与修复**）；§E-24~§E-26 为历史交付台账 |
| ④ | `docs/EDITOR-UX-PLAN.md` + `docs/ASSET-DATA-MODEL.md` + 旧仓路径/流程（仅在碰旧仓时） | 画布/选择/实时帧/区域有效性/排序/分类改版及**实现后状态（§15）**；全局素材/任务素材边界；旧仓 `D:\OAS-dev\backend\docs\LAYOUT.md` · `DEV-WORKFLOW.md`（冻结） |

## 1.3 用代码核实（不许信文档自述）

```bat
REM ① 我的工作区/提交
cd D:\OAS-dev\oas-core
git log -1 --format="%h %ci %s"     REM 期望 68e5768（或其后的 seq 108 docs 提交）
git status --porcelain              REM 期望只有用户数据（assets/frames/tasks/scheduler.json）

REM ② 引擎测试（当前基线 721/0；oasd 355 / oas-task 111 / flow 62 / runtime 157+2）
cargo test --workspace

REM ③ OCR 构建（Studio/Runner/run 需要；不带 feature 会报 "this build has no OCR"）
cargo build -p oasd --features ocr

REM ④ 启动 Studio（编辑端；8788；当前服务已停，交接后关机）
target\debug\oasd.exe studio --config config\live.json

REM ⑤ 启动 Runner（使用端；8789；启动即内嵌调度，--no-scheduler 关闭）
target\debug\oasd.exe runner --config config\live.json

REM ⑥ 旧仓守卫（仅作参考；旧仓已冻结，171 passed 是旧仓基线，不是引擎验收）
cd D:\OAS-dev\backend
run-tests.bat tests/test_comment_drift.py tests/test_task_flow_docs.py tests/module/config/test_standby_placeholder.py ... -q
```

> ★ **双入口不要混**：`oasd studio` 默认 **8788**（编辑端，`http://127.0.0.1:8788/`），`oasd runner` 默认 **8789**（使用端，`http://127.0.0.1:8789/`）；**`oasd serve` 仍可用但已 deprecated**（等价 `studio`，stderr 一行提示）。`scripts\dev-serve.bat` = 开发启动器：默认 **Studio 8788**；`scripts\dev-serve.bat -Surface runner` = **Runner 8789**（支持 `-NoOpen`/`-DryRun`/`-Release`/`-SkipBuild`/Runner 的 `-NoScheduler`）；**8787 已退役**，旧书签报 `can't connect` 请改 8788/8789。
> ★ 用户版 Studio 地址是 **http://127.0.0.1:8788/**（`start-studio.bat`，`start-editor.bat` 兼容转调）；Runner 包地址是 **http://127.0.0.1:8789/**（`start-runner.bat`，**固定 `--port 8789`，改 `runner.port` 不生效**）。**终端用户只下载 Runner-only 包 + 任务包，不需要 Studio**。
> ★ [10-31] `oasd serve` 默认在监听成功后**自动打开默认浏览器**；headless/服务用 `--no-open`。Windows 服务在 Session 0 不会自动弹到用户桌面，服务化时应由用户登录任务/launcher 打开。
> ★ `D:\OAS-dev\backend\toolkit` 只是本机外部开发工具包（ADB/OCR 模型）；oas-core 源码、构建、测试、发布不依赖 backend。若要完全脱离旧仓，可把 toolkit/模型复制到 oas-core 自带 `tools/` 或用环境变量覆盖。
> ★ 实机 MuMu serial = **`127.0.0.1:16384`**（不是 `emulator-5554`）；`GET /instance/devices` 列表为空时会自动 `adb connect`。

## 1.4 先回报，再动手

★ 读完、核实完，必须先把 §2 的**引擎版回报模板**发给用户，等确认方向后才改代码。
★ 改动前先出「设计 vs 现状」对照（✅符合 / ❌不符 + `文件:行号`）。

## 1.5 ★★★ 当前设计决策（调度 v3 + 统一任务模板，用户 [10-24] 定稿）

> 详见 **ENGINE-DESIGN §22.34**；这里是入口摘要。

**三层分离（机制 / 契约 / 值）**

| 层 | 内容 | 不许写死游戏数据 |
|---|---|---|
| 统一任务模板 | `tasks/<id>/{meta.json, flow.json}` + 全局 `assets/{assets.json, images/}`；`meta.json` 含 params/entry.requires/exits/capabilities/resources/schedule_hint；flow 统一骨架 `preflight → main → settle → finish(exit)`；`oasd task new --template …` + `task lint` | ✅ 零游戏常量 |
| 调度器 v3 | 周期时钟、护栏、候选过滤、唤醒规划、执行器、状态/报告 | ✅ 零游戏常量 |
| 玩家配置 | `scheduler.json`：重置时间、active_hours、blackouts、entries（顺序/窗口/quota/params）| ✅ 全部玩家自己填 |

**生命周期（用户敲定）**

- `periodic`：时间窗 + 每日/每周 quota 的周期任务（斗技/寮活动/签到等）。
- `temp`：**临时任务**（旧仓 `period=none` 的等价概念）：无窗口/无 quota；不记忆"还剩几次"；每轮按 `entries` 顺序跑一遍；执行前后各重置状态；"完成" = 门票耗尽/达到次数。
- **删除**：`one_shot` 与 `continue`（用户裁定不需要）。
- `rest`（可选，后期）：暂停 N 分钟的条目；v1 不优先。

**重置记忆功能（PR6 起在 Runner）**

- 入口：Runner「队列与调度 → 重置记忆」dialog（多选/全选/反选/全不选 + `include_checkpoint`）+ CLI `oasd scheduler reset-memory`；**Studio 已无重置入口**。
- 清除范围：调度器条目状态 + 任务 `state/`（checkpoint/计数等）；原子写 + 审计事件 `memory_reset`。

**调度铁律**：窗口只回答"能不能跑"；执行顺序只由 `entries` 数组顺序决定；排序是动作不是状态；调度器不内置任何游戏时间/清单。

---

## 1.6 ★★★ 连续运行 / 断点续接（长任务必用）

> 规则全文：skill `session-memory` **强制 6（Continuity Protocol）**；当前任务断点：`docs/RESUME.md`（seq 108）。

| 项 | 约定 |
|---|---|
| 断点卡 | `docs/RESUME.md`（**每次覆盖重写**，≤150 行；含验收清单 + 下一步精确命令）|
| 断点索引 | `docs/RESUME-INDEX.md`（**只追加**；每行 seq/阶段/文档锚点/证据/下一步）|
| 触发 | 每个阶段/验收项完成、测试红→绿、提交前、用户裁定后、开始长命令/子代理批次前、**每轮回复结束前**；无容量信号时再按"10 次工具调用 / 25k 字符 / 5 回合"兜底 |
| 压缩/新会话后 | ★ 先读 RESUME + INDEX 最新行 → 用 `git log/status` + 最小验证命令核实 → 修正差异 → 直接从"下一步精确动作"继续（不重做已完成项）|
| 完成 | 验收清单全 ☑ + 证据可核查 + RESUME 状态 `done` + INDEX 追加终态行 + 台账更新 |
| 阻塞 | 同一阻塞连续 3 轮未解且需外部输入才停；写进 RESUME 阻塞字段再问用户 |

★ 初始化/查看：`python <skill>/scripts/continuity.py init|status`（配置走 `session-memory.config.json`，默认 `docs/RESUME.md`）。

---

# §2 ★★ 回报模板（引擎版，逐项填）

```
📖 已看过 design_spec 文档 · ENGINE §22.x / §22.34 · 当前 commit <hash>

【1. 我已加载】
   skill `session-memory`：✅/❌
   读过的文档：SESSION-HANDOFF ✅ · ENGINE-DESIGN §22 ✅ · PENDING-LEDGER §E-24 ✅

【2. 代码与我核实的台账】
   oas-core commit = <hash> · 分支 = dev · worktree 干净 = 是/否（只有用户数据算"否"）
   cargo test --workspace = __ passed / __ failed（当前基线 = 721；oasd 355 / oas-task 111 / flow 62 / runtime 157+2）
   serve 状态：/state = <captured/device/ocr/det>（Studio 8788 / Runner 8789 双入口；`oasd serve` 为 Studio 兼容别名）
   oas-core docs commit = <hash> · dev 是否已推 = 是/否

【3. 差异（文档说 A、代码是 B）】
   | 文档说 | 代码是 | 差异 |

【4. 下一步（按 ENGINE §22.34 计划 + 我的判断）】
   1. …  2. …  3. …
   ★ 建议先做：… 因为 …

【5. 未完成台账】（见 PENDING-LEDGER §E-79 / §E-78 / §E-77 / §E-76；§E-24~§E-26 为历史；§E-79 = Studio/Runner 分离：Q1~Q7 已批准、**PR1~PR7 全部 ✅ 已提交**、**§M 最终审计 P1 已修（68e5768）**、下一步 = 产品验收 + P2 清理 + Phase 5 选择）

【6. 需要用户裁定的事】

【7. 本轮完成定义（说完成前必须勾完）】
   ☐ …  ☐ …  ☐ …

【8. 连续性（长任务必填）】
   断点：seq=<N> · 阶段=<P0/P1/…> · 状态=in_progress/blocked/done/handoff-ready
   下一步：<可直接执行的命令 / 文件:行号>
   RESUME：docs/RESUME.md（updated_at=…）· 索引：docs/RESUME-INDEX.md
```

---

# §3 ★ 活跃的坑（引擎相关）

| # | 坑 | 一句话 |
|---|---|---|
| **E1** | 构建必须带 `--features ocr` | 否则编辑器识别报 `this build has no OCR`；`/state` 会显示 `ocr:false` |
| **E2** | 重建 `oasd.exe` 前必须先停 8787/8788/8789 | 否则 `failed to remove target\debug\oasd.exe` |
| **E3** | `library/` 已 gitignore，`tasks/*/state` 已 gitignore | 不要把它们提交进去 |
| **E4** | `_verify/` 部分入库 | `_verify/audit-20261101/{A-session-git,B-runtime,C-ui,D-e2e,E-acceptance}.md` 已入库；其余 `_verify/nNN/`、`_verify/runner-studio-*`、`_verify/runner-studio-final-audit-*` 是仓外工作区证据（`D:\OAS-dev\_verify`），不入 git |
| **E5** | 中文文档/代码一律用 `read`/`edit`/`write` 工具 | 不要用 PowerShell `Get-Content|Set-Content` 重写 UTF-8 文本（会 mojibake）|
| **E6** | adb 路径走 `ADB_PATH` | `D:\OAS-dev\backend\toolkit\Lib\site-packages\adbutils\binaries\adb.exe` |
| **E7** | 旧仓已冻结 | 旧仓 `backend/frontend` 只读；不要在旧仓上开发/提交 |
| **E8** | 新引擎的验收看 ENGINE §22 判据 | 不要拿旧仓 pytest 基线当引擎验收 |
| **E9** | headless Edge 截图做 UI 验收 | `msedge --headless=new --screenshot=...`；需要时加 `--user-data-dir` |
| **E10** | 测试数会随提交增长 | 当前 **721/0**（oasd **355** / task 111 / flow 62 / runtime 157+2；UI **1559 checks / 159 Studio ids / 151 Runner ids / 0 dead / 6 nav**）；2026-11-05 **seq 108 = 最终审计 4 个代码 P1 修复 +8 测试 = 721**（713→721；oasd 347→355）；seq 107 = PR7（Rust 零改动 713/0，UI 1559/151/0，包 37,684,830 B）；seq 106 = PR6（713/0、UI 1167/159/0、包 37,653,462 B）；seq 105 = PR5（677/0、UI 1442/237/0、包 37,591,694 B）；seq 104 = PR4（624/0）；seq 103 = PR3（583/0）；seq 102 = PR2（562/0）；seq 101 = PR1（538/0）；以 `cargo test --workspace` 实测为准 |
| **E11** | 本会话 PowerShell 实为 **5.1**，执行策略 Undefined | 直接 `.\scripts\dev-serve.ps1` 报 UnauthorizedAccess ⇒ 用 `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\dev-serve.ps1 -NoOpen` |
| **E12** | `node` 不在 PATH | UI 测试用 bundled node：`C:\Users\passioki\.dsh\dsh-runtimes\dsh-primary-runtime\dependencies\node\bin\node.exe`（v24.21.0）|
| **E13** | PS 5.1 重定向日志是 **UTF-16LE** | 读日志用 `-Encoding Unicode` 或先转 UTF-8，否则中文/换行乱码 |
| **E14** | 实机 adb 为空 → 先确认 MuMu 运行中（serial **`127.0.0.1:16384`**，不是 `emulator-5554`） | 真机任务（`/input/tap`、collection `real=1` E2E）需先启动模拟器；**P5 起 `GET /instance/devices` 在列表为空时会自动 `adb connect`**；仍为空再查 adb server / MuMu 网络设置 |
| **E15** | PR3 配置注入语义（2026-11-04） | 四层优先级 **CLI `--param` > entry params > Runner values > `meta.params.default`**；`oasd run` **不读** values；Studio `POST /task/run` body params 最高且**不注入** values；`meta.params` **非空**才在合并后做 schema 复检；旧配置字符串化数字/布尔不再猜回类型；`--param 'code=\"007\"'` 保持字符串；坏 `values.json` 全局 exit 2；PR6 `/runner/run` 自行合并 values。详见 `RUNNER-STUDIO-SPLIT-PLAN §A.11.7` / `ENGINE §22.46` |
| **E16** | 最终审计修复（2026-11-05 `68e5768`） | replay/dry-run **不取设备租约**（scheduler replay pass 输出 `scheduler_lease_skipped`）；run worker panic 被 catch_unwind 兜住（`outcome=Panic`、running/租约清理）；install/activate 在调度运行或 live lock 下 **409 `scheduler_running`**；`installed.json` 损坏时 `/runner/tasks` 降级 200 + warning、生命周期 API 500 `installed_registry_corrupt`。详见 `ENGINE §22.51` |
| **E17** | **PS 5.1 的 `Set-Content -Encoding UTF8` 会写 UTF-8 BOM** | `serde_json` 拒绝 BOM ⇒ Runner 包 `scheduler.json` 首启 `scheduler_mode=invalid_config`（F1，2026-11-05 已修）；打包脚本改 `[System.IO.File]::WriteAllText(..., UTF8Encoding($false))` + stage 首字节守卫；**新脚本勿再用 `Set-Content -Encoding UTF8` 写 JSON**（`ENGINE §22.52.3`） |
| **E18** | **PS 5.1 无 BOM 的 UTF-8 `.ps1` 会按 GBK 解析中文字面量** | 中文脚本/含中文的测试脚本必须存成 **UTF-8 with BOM**（或用 `WriteAllText(..., UTF8Encoding($true))` 重存）；否则 `powershell -File` 里 `'返回庭院'` 变乱码 → API 400（P0 E2E 踩过）。检查：前 3 字节 `EF BB BF` |
| **E19** | **exchange 默认关闭** | P0 发布通知需两端 `config/live.json` 配同一 `exchange.dir`（推荐绝对路径）；未配时行为同旧版；运行时 `config/exchange/`、`config/notifications.json` 已 ignore。API：`GET /runner/updates` / `POST /runner/updates/ack`（`ENGINE §22.53`） |
| **E20** | **参数作为 OCR 文字 = `text_expr`** | `click_target.find[]`/`requires`/`expect`/`drag.from/to` 的 `kind:"ocr_text"` 可写 `text_expr:"param('key')"`（与 `text` 互斥必填其一）；运行时 step 进入一次求值、非空字符串，否则响亮失败且不点击；旧包安装门禁：flow_ops 含 3 个新标记（`click_target.text_expr`/`expect.text_expr`/`drag.text_expr`），老 Runner 会拒绝（`ENGINE §22.54`） |
| **E22** | **oasd lib+bin / oas-runner Phase 1a** | `apps/oasd` 现在是 lib+thin bin（CLI 在 `oasd::cli`；`run_as(surface)` 供独立 app 复用）；oas-runner 仓 `apps/runner` 是 3 行 Runner app（`oasd::cli::run_as("runner")`），当前 **path 依赖**（换机需布局一致；Phase 1b 改 git rev）。Runner 独立二进制默认 UI 仍为 `--ui` 默认 `apps/oasd/ui`，打包需显式传 `--ui`（`ENGINE §22.56`） |

---

# §4 ★ 铁律（session-memory skill + 用户明令）

```
★ ① 用户的话 = 规格，不是建议 ⇒ 不许降级成"打补丁"
★ ② 读写双向：不只看用户说了什么，还要看我提了什么选项、用户裁的是哪个
★ ③ 禁止拿"首行预览/截断"当读过 —— 必须读正文
★ ④ 边读边写（防上下文压缩丢失），不攒到最后
★ ⑤ 改代码前先出「设计 vs 现状」对照（✅/❌ + 文件:行号）
★ ⑥ 逐步法：一次只改 1~2 个边界 ⇒ 立刻 test ⇒ 再下一步
★ ⑦ 完成定义：说"完成"前先列勾选清单；没勾完不许说完成
★ ⑧ 台账：每条回复先更新台账文件，并把相关表贴进回复
★ ⑨ 不许阴奉阳违
★ ⑩ 文档不是流水账，是围绕设计/实现/经验教训的有逻辑集合体
```

---

# §5 ★ 每轮结束（强制）

```
★ ① 更新 PENDING-LEDGER（状态变了就改；完成必须有可核查证据）
★ ② 更新 SESSION-HANDOFF（当前状态/下一步/新坑）
★ ③ ★ 更新断点：RESUME.md（覆盖重写）+ RESUME-INDEX.md（追加一行）
★ ④ 有实质进展就 commit：
     - oas-core：提交到 `dev` 并同步 `main`；0.1.0 恢复节点 tag `v0.1.0` 已完成（GitHub Release 已上传）
     - oas-core docs：git add docs session-memory.config.json && git commit && git push origin dev
★ ⑤ 验收证据进 _verify/nNN/，并写进文档判据
```

---

# §6 ★ 常用命令速查（引擎）

```bat
REM ---------- 构建/测试 ----------
cd D:\OAS-dev\oas-core
cargo test --workspace          REM 当前 721/0（oasd 355 / oas-task 111 / flow 62 / runtime 157+2；seq 108 = 最终审计修复 +8）
cargo build -p oasd --features ocr     REM serve/run 的 OCR 构建
REM ---------- 打包（两包已产出；-DryRun 可预览）----------
scripts\package.ps1 -SkipBuild          REM Studio-only -> dist\oasd-v0.1.0-win64.zip
scripts\package-runner.ps1              REM Runner-only -> dist\oasd-runner-v0.1.0-win64.zip

REM ---------- Studio / Runner 双入口（当前服务已停止）----------
REM Studio 编辑端（8788；oasd serve = deprecated 别名，等价 studio）
target\debug\oasd.exe studio --config config\live.json
REM Runner 使用端（8789；启动即内嵌调度，--no-scheduler 关闭；安装/配置/队列/调度/临时运行/运行记录全在 Runner）
target\debug\oasd.exe runner --config config\live.json
REM   浏览器：Studio http://127.0.0.1:8788/（编辑器）· Runner http://127.0.0.1:8789/（使用端）
REM   Studio 仍可开 http://127.0.0.1:8788/dashboard.html（点击分布/长跑统计）
REM   开发启动器 scripts\dev-serve.bat = Studio 8788（默认）/ -Surface runner = Runner 8789；发行包脚本 start-studio.bat=8788 / start-runner.bat=8789（固定 --port 8789）

REM ---------- 运行/调度 ----------
target\debug\oasd.exe run --task tasks\返回庭院 --replay <recording-dir>
target\debug\oasd.exe queue --tasks tasks\A,tasks\B --replay <dir> --report report.json
target\debug\oasd.exe scheduler --plan scheduler.json --status
REM   ⚠ 调度计划入口是 --plan <scheduler.json>；--config 是全局 live 配置（main.rs 全局摘走并立即解析），
REM     误传 scheduler.json 会 exit 2（config error: unknown field 'config'），不会跑任何 plan
target\debug\oasd.exe stats --runs runs --html runs\stats.html
REM   ✅ schedule v3 已交付（V1.0）；设计/交付见 ENGINE §22.34/§22.35，Round 语义见 §22.34.7
REM   P4：run 实体在 <runs-dir>\scheduler\（默认 runs\scheduler）；serve 与 scheduler 的 --runs-dir/CWD 必须一致，
REM       否则"调度器写、UI 看不到"；API = GET /runs/scheduler?limit=&task=&outcome= · GET /runs/scheduler/run?run_id=

REM ---------- 旧仓（冻结，仅参考） ----------
cd D:\OAS-dev\backend
run-tests.bat …                                  REM 旧仓守卫基线 171 passed（参考）
```

---

# §7 设计文档索引

| 主题 | 位置 |
|---|---|
| **长任务断点/续接（先读）** | `RESUME.md` · `RESUME-INDEX.md`（skill 强制 6；最新 **seq 126，done**） |
| **帧-素材关系** | `../frames/frames.json` + 全局 `assets.json` 的 `positions`；无 interface 文件 |
| **素材数据模型** | `ASSET-DATA-MODEL.md`（全局 `assets/`、任务素材=flow 引用子集、旧 library 仅后端兼容） |
| **编辑器 UX / 选择模型计划** | `EDITOR-UX-PLAN.md`（画布、位置详情、多边形/反选、实时帧、点测/拖测、区域有效性、素材排序、分类重命名；**素材库 UX 现行为 §18**；**§18.10 = 三项遗留修复**） |
| **★ Studio / Runner 分离 + TaskPackage + 任务配置** | `RUNNER-STUDIO-SPLIT-PLAN.md`：**Q1~Q7 已批准；PR1~PR7 全部 ✅ 已提交（PR7 代码 `5e45c81`）+ 最终审计修复 `68e5768`**；§A.9~§A.15 = PR1~PR7 实现记录，**§A.16 = 最终审计与修复记录**，**§A.17 = 双端发行包/分离度/dev-serve/参数 UI 审计（2026-11-05，seq 110）**；§A.8.1 = 两种发行包；§D.3 PR 计划、§D.4 19 步验收；分发/跨平台见 `packaging/CROSS-PLATFORM.md`、`packaging/README-RUNNER.txt`、`packaging/README-DEPLOY.md` |
| **当前任务（返回庭院）** | `../tasks/返回庭院/flow.json` · 全局素材 `../assets/` · 帧 `../frames/` |
| **引擎规格/进度** | `ENGINE-DESIGN.md` §22（**§22.51 最终审计；§22.52 发行包/参数审计；§22.53 P0；§22.54 参数 v2/构建端/P0-1b；§22.55 使用端参数 UI/更新检测；§22.56 Phase 1a/Studio lint；§22.57 Phase 1b 独立打包；§22.58 更新检测 TTL/复用/清理 + 参数编辑补齐；§22.59 动作目录统一 + 配置项成熟 UI；§22.60 参数插入/条件修复/素材选择；§22.61 region 操作区域 + long_press + curve_swipe + 等待语义；§22.62 A 方案 region.asset + 多候选 nearest；§22.63 用户新要求记录；§22.64 seq124 参数→素材/op_region/候选 API/test_action/废弃两状态；§22.65 ★ seq126 验收整改：操作区模型 A / polygon+统一框选 / 参数 UI 移除 / 变量系统设计**） |
| **seq131 现行口径** | `ENGINE §22.70`（加入集合入口恢复）· `ACCEPTANCE-2026-11-06.md`（seq131 追加）· `PENDING §E-99` · `OPERATION-REGION-DESIGN §8.9`（模型 B） |
| **seq124/126/127/128/129 实现记录（历史）** | `PARAM-ASSET-IMPLEMENTATION.md`（seq124 设计+裁决）· `PARAM-ASSET-REDESIGN.md §1b`（用户权威逐字要求）· `OPERATION-REGION-DESIGN.md §8`（模型 A→B 修正）· `ENGINE §22.64/§22.65/§22.66/§22.67/§22.68/§22.69` |
| **调度 v3 + 统一任务模板（已交付 V1.0）** | `ENGINE-DESIGN.md` **§22.34 / §22.35**；Round 语义裁决见 **§22.34.7** + `SCHEDULER-RUNNER-PLAN.md` |
| **发布/恢复节点（当前）** | `RELEASE-NODE-2026-11-02.md` · Release `https://github.com/passioki/oas-core/releases/tag/v0.1.0` |
| 到实机可用的交付路线（历史） | `archive/DELIVERY-PLAN.md` |
| 当前状态/下一步/坑 | `SESSION-HANDOFF.md` |
| 台账/证据（当前） | `PENDING-LEDGER.md` §E-66~**§E-99**（**§E-99 = seq131 恢复加入集合入口（✅）；§E-98 = seq130 点击几何同源/框选移动/叠加/自动刷新（✅）；§E-97 = seq129 动作库一致性（✅）**；…） · **设计/验收 `docs/VARIABLES-V2-DESIGN.md` / `docs/ACCEPTANCE-2026-11-06.md` / `docs/OPERATION-REGION-DESIGN.md`** · 实现/验收证据 `_verify\seq131-collection-smoke\` / `_verify\seq130-click-repro\` / `_verify\seq130-final\` / `_verify\acceptance-20261106\seq130-ui-progress.md\` / `_verify\seq128-e2e-20261106\` / `_verify\seq127-verify\` |
| **★ P0 双端交互（已实现，2026-11-05 seq 112）** | 数据随包（Studio 全数据 / Runner `bundled-data` + 首启导入）+ 发布通知 exchange（`config/live.json` 的 `exchange.dir` 两端指同一目录；Runner「安装包」页「新发布（来自 Studio）」+ nav 徽标；`GET /runner/updates` / `POST /runner/updates/ack`）；实现/证据见 `ENGINE §22.53` / `PENDING §E-82` / `_verify\p0-1a-e2e-20261105\`；包装说明见 `packaging\README-RUNNER.txt §6`、`packaging\README-DEPLOY.md` |
| **★ 双端架构升级设计（待裁定，2026-11-05 seq 111）** | `_verify\dual-app-design-20261105\`（仓外）：**`PARAM-V2-DESIGN.md`**（1096 行：参数 v2/动作库/`text_expr`/Runner UI/Studio/lint/7 PR/14 待裁定）· **`RUNNER-SPLIT-DESIGN.md`**（926 行：两仓拆分/Phase 0~2/发布通知 exchange/数据随包/14~19 人日）· **`DECISION-SHEET.md`**（D1~D12 推荐）· `prototype\`（`runner-params.html` / `studio-params.html` + 4 截图 + `DESIGN-NOTES.md`）；见 §0.2 / `§E-81` |
| **参数配置 UI 审计（待裁定，2026-11-05）** | `_verify\ui-params-audit-20261105\PARAMS-UI-AUDIT.md`（仓外，398 行）：结论/缺口清单/方案 A~E/推荐 A+C；见 §0.2 / `§E-80` |
| 旧仓历史（冻结，仍在旧仓）| `D:\OAS-dev\backend\docs\SESSION-HANDOFF.md` 历史段 · `D:\OAS-dev\frontend\docs\scheduler-architecture.md`（旧调度设计，参考） |
