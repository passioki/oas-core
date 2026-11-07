# RESUME — 断点续接卡（session-memory 强制 6）

> 规则：本文件每次断点覆盖重写；≤150 行；另一个会话只读本文件 + `RESUME-INDEX.md` 最新行就能继续。
> 细节：`docs/RUNNER-STUDIO-SPLIT-PLAN.md`（§A.9~§A.17）· `PENDING-LEDGER.md §E-79/§E-80/§E-81` · `ENGINE-DESIGN.md §22.44~§22.52`。

```yaml
task_id: oas-core-dual-app-upgrade
checkpoint_seq: 138
updated_at: 2026-11-07T23:4x+08:00
trigger: 用户 22:28 反馈 priority/frames/hub 语义不清+工作台帧与地图帧割裂 → 22:53 裁决地图 V2（所在地/路标/路径，删 priority/frames/hub，fallback_task 返回庭院，工作台 tab 最后，独立页删）→ 23:03 补充（v1 只迁代码、素材/帧搜索+预览、Rust 编译器本轮不做、target_expr 地图后）→ V2-B `8394af7` + V2-C `8134376`；workspace 1044/0、UI 2717/195/0/6（Studio nav 5）。
status: done（地图 V2 base 已实现：V2-B 后端 + V2-C 工作台地图 tab）
code_head: `8134376`（dev=origin/dev；main/origin/main=55dc07c 待 docs 后快进；未 tag/未 release）
docs_head: seq138 docs 提交见 `git log -1`
next_action: seq138 地图 V2 base 已完成（未 tag/未 release）。后续：① `navigate.target_expr`/param() 动态目标（T1，地图基础后）；② **Rust 编译器（rustc 原生）本轮不做、优先级调低**（源码框保持 flow JSON+Rust 只读投影，无 Rust→flow）；③ 真实设备导航/replay E2E 未跑、Firefox 手测未做；④ 全局弹窗 UI；⑤ 地图边内联改/canvas 缩放/map.js-map_ui.js 双文件/map_requirements deprecated；⑥ `text_visible`/`verify_expect` fuzzy+无缓存；⑦ UI 无浏览器自动化；⑧ serve `/map/navigate real` 无 fallback sibling、fallback_task 保存不校验存在；⑨ v1 用户数据不迁移；⑩ V2.1/V2.2。详见 `PENDING §E-109`、`ENGINE §22.77`、`docs/MAP-V2-DESIGN.md §9`。
```

## 0. 目标（本阶段）

1. Studio（编辑端，8788）/ Runner（使用端，8789）分离：同一份代码两个 surface。
2. Q1~Q7 已批准；**PR1~PR7 全部 ✅ 已提交**（PR1 `da9da9d` / PR2 `dfe8776` / PR3 `c09df25`+`048dff5` / PR4 `442f5cb` / PR5 `617435d`+`830058d` / PR6 `edc037b` / PR7 `5e45c81`）；最终审计 P1 ✅ `68e5768`。
3. **seq 110（上一轮）= 用户三项要求**：① 分离度检查 + 试发布两端 release；② 更新过时的 `scripts/dev-serve.*`；③ 参数配置在编辑端未被表达 ⇒ 只读审计 + 方案。
4. **seq 111（设计轮）= 用户四项新要求**：① 资产/任务属于代码仓库、发布随包；② 参数与 flow 互动/像动作库动作（参数作为 OCR 识别文字、分组+预设、使用端暴露可选用、留空=不用）；③ 使用端完全独立新仓库、两 release 可单独/同时运行并交互；④ Studio 发布新任务→Runner 收到通知。
5. **seq 112（已实现）**：数据随包（Studio 全数据 / Runner `bundled-data` + 首启导入）+ exchange 发布通知 + UI。
6. **seq 113（已实现）**：① 参数 v2 后端（`text_expr` 参数作为 OCR 文字 + T3/T4/flow_ops 门禁 + `ParamDecl.role`）；② 构建端参数 UI/动作库；③ P0-1b（event engine 字段 + `compatible` 预检 + notify 客户端）。
7. **seq 114（已实现）**：用户「建 oas-runner 空仓，curl vs TLS 自行判断」→ 创建独立仓 `https://github.com/passioki/oas-runner`（`e1d603a`）+ Runner 参数 UI + 更新检测 feeds（本地/HTTP，系统 curl）+ `make-update-feed.ps1`。
8. **seq 115（已实现）**：用户「好的，下一步」→ ① **Phase 1a**：`apps/oasd` 改 **lib+bin**（新 `lib.rs`/`cli.rs`，`main.rs` 404B，`pub fn run_as(surface)`），测试迁到 lib 且全绿、warnings 5→1；② **oas-runner 独立薄应用**：`apps/runner`（3 行 main 调 `oasd::cli::run_as("runner")`）+ Runner UI + Cargo.lock，path 依赖构建/起服 smoke 通过；③ **Studio 权威 lint**：`GET /task/lint`（CLI/HTTP 共享 `lint_one`）+ 构建页「检查」面板（T3 去声明/降级/近似提示折叠）。
9. **seq 116（已实现）**：用户「好的，下一步」→ **Phase 1b 收尾**：① oas-core `oasd::cli::run_as_with_args`（默认参数可注入、用户可覆盖，commit `802f58b`）；② oas-runner 默认 `--ui ui` + pin `rev=802f58b`；③ 新仓 **`scripts/package-runner.ps1`**（cargo build → stage：exe/ui/models/adb/**bundled-data 生成**/live/README/BUILD-INFO/start-stop bat → 审计 → `/` zip；`-DataFrom`/`-ToolkitFrom` 自动探测、`-DryRun`）；④ 独立包 37,854,377 B，解压 smoke 50/50。
10. **seq 117（已实现）**：用户「好的，下一步」→ ① **更新检测收尾**：`updates.cache_secs`（默认 60）+ Server feed 索引 TTL 缓存（命中不 curl）、`?refresh=1`、远程包缓存复用（`reused_cache`）、`POST /runner/updates/cache/clear`；② **参数编辑补齐**：文字来源三态组件覆盖 requires/expect/drag.from/to，T3「去声明…」预填 key；③ oas-runner rev bump `78be71d` 并重打独立包；④ E2E 三路全绿。
11. **seq 118（已实现）**：用户反馈 ①「配置项没有任何解释，不知道怎么填，能干什么」；②「动作库里有参数，但 if 的『是』下拉没有，其他衔接下拉也未更新」；③ 要求重新审计代码/更新文档防漂移/交接后关机。→ ① **动作目录单一事实源** `actionCatalog()`：调色板 + if/loop「是/否/循环体」分支下拉统一，参数预设未声明时禁用+原因；② **配置项结构化表单**（帮助块/每参卡片/逐字段示例/语义校验/高级 JSON/T3 prefill 行）；③ **最终审计** `docs/AUDIT-2026-11-05.md` + 全套文档同步。
12. **seq 119（已实现）**：用户反馈任务「切换预设御魂」加两个参数后与 if 衔接，JSON 多处自动出现 `sel: 觉醒材料`。→ ① 表达式「插入参数」防粘连；② `repairConditionExpr` 自愈历史坏值；③ 参数点击不再静默选第一个 OCR 素材（多个弹选择对话框）；④ `saveFlow` 前 `normalizeFlow` 清 if/while 遗留 `args.sel`；⑤ 用户任务畸形条件已修为 `text_visible(param('group'))`。
13. **seq 120（设计）**：用户指出素材选择对话框不完善并给出 3 类区域来源（素材操作区域/多匹配最近/相对位移/参数 OCR 真实识别区域）+ 长按/曲线问题 → `docs/OPERATION-REGION-DESIGN.md` 设计草案 + 4 个待确认问题。
14. **seq 121（已实现）**：用户确认：nearest 参考点=**OCR 识别 ROI 中心**；长按独立 op、默认 **3000ms**、随机范围；offset 相对 bbox 左上角；曲线用**多段近似**。→ ① 引擎 `region{asset,ocr,nearest,offset}` + `long_press` + `curve_swipe` + flow_ops 7→11；② 构建页动作库/操作区域/长按/曲线面板；③ **等待动作解释**；④ workspace **824/0**、UI **2146**。
15. **seq 122（已实现）**：用户确认 **A 方案**（OCR 识别素材 + 操作区域素材全库）→ ① `region.asset`（仅 asset/nearest；T4/flow_ops +2）；② oas-perceive 多候选匹配（NCC→局部极大→阈值→NMS→≤64，best 路径不变）；③ runtime `operation_region_for_asset`：模板/集合 nearest 按“候选匹配 bbox 中心 ↔ 参数 OCR 行 bbox 中心”距离平方选最近；固定 region/OCR 资产取自身操作区；无候选/未匹配=响亮失败；④ 三处接入 click/drag/long_press；⑤ UI 对话框/结构化编辑器加「操作区域素材（全库，可选）」+ 校验/风险提示；⑥ workspace **841/0**、UI **2214**、E2E `regionasset` flow_ops 含 region_asset；oas-runner pin `ba6813e`。
16. **seq 123（本轮，仅文档交接）**：用户重新梳理并给出 6 项重构要求，**推翻 seq 122 的交互方向**：① 动作库删除参数预设（「参数文字点击」→「参数文字动作」，最终参数不属于动作库）；② **参数是素材的一种**（使用端可变识别文字；初始识别区为空；改名「使用端OCR文字配置项」待定）；③ 素材行点识别返回**所有候选+评分**（后端 API；测试动作演练所有候选；真实点击可选候选）；④ **量尺**（两点直线距离复制剪贴板）；⑤ **操作区域三方式 + 递归套娃**（参数真实识别区/素材操作区单候选或多候选最近/相对偏移；素材链依次识别、任一缺失则本步失败；防环）；⑥ **废弃操作区“与真实识别区有无交集”两状态**（先审计现有全图回退+相对位移）。→ 本轮**未改代码**，新增 `docs/PARAM-ASSET-REDESIGN.md`、`PENDING §E-93`、`ENGINE §22.63`，更新交接文档；下一步先审计+确认设计文档 §5 问题再实现。

## 1. 状态（2026-11-06 12:0x，seq 123）

| 项 | 值 |
|---|---|
| 分支 / HEAD | oas-core `dev`（本批 docs 提交后以 `git log -1` 为准；代码仍 seq 122 `ba6813e`）；oas-runner `main` = `768a8b7`（pin `ba6813e`）；未 tag / 未 release |
| 任务状态 | **awaiting-redesign**：seq 122 A 方案代码已推送但**交互方向被用户推翻**；新 6 项要求**未实现**，先审计 + 确认设计问题再开工 |
| 双入口 | Studio `http://127.0.0.1:8788/` · Runner `http://127.0.0.1:8789/`；oas-runner 独立 app pin oas-core `ba6813e` |
| 测试 | `cargo test --workspace` **841 / 0**（23 suites；perceive **11**+2 / flow **78** / runtime **186**+2 / task **122** / adapter-pc 5 / oasd 412）；UI pure test **2214 checks / Studio 164 / Runner 158 / 0 dead / 6 nav**；debug+release OCR build exit 0（**1 warning**）；`git diff --check` clean |
| 新要求 | 参数→素材/动作库去参数化/素材候选 API+行 UI/量尺/操作区三方式+递归防环/废弃两状态 —— 见 `PARAM-ASSET-REDESIGN.md`、`PENDING §E-93`、`ENGINE §22.63` |
| 服务 | 无 `oasd`/`oasd-runner` 进程；8787/8788/8789/8792~8796 无监听 |
| 数据边界 | `scheduler.json`、用户任务 `tasks/切换预设御魂/{flow,meta}.json`、**用户本轮编辑的 `assets/assets.json` / `frames/frames.json` / 新增 `assets/images/a006.png`** 均未提交（**不要提交/覆盖**）；其余已提交/推送；发行包按「推送即可，不用发布」未重打/未上传 |

## 1.5 2026-11-05 18:2x（seq 111）设计轮产出（只读，未改代码）

> 3 路只读设计（Team：param-architect / split-architect / ui-prototyper）+ Lead 决策单；均写在仓外 `D:\OAS-dev\_verify\dual-app-design-20261105\`。

| 交付 | 内容 | 路径 |
|---|---|---|
| 参数 v2 设计 | 1096 行：设计 vs 现状（参数链路 / 动作库 23 op / **OCR 文字硬缺口：`Locator::OcrText.text` 等都是 String 字面量**（`oas-flow:652-656/712-726/787-791`）/ 分组预设 / 4 层优先级不变）；ParamDecl v2（`role` 可选、`group` 首次消费）；动作库 A1~A7 + `text_expr` DSL + T3 扩扫 + flow_ops 门禁；Runner UI 线框/状态机/API v2；Studio 面板/lint；7 个 PR（~13 人日）；14 项待裁定 | `PARAM-V2-DESIGN.md`（仓外） |
| 双端拆分设计 | 926 行：**推荐方案 A 两仓**（oas-core / 新仓 `oas-runner`，共享 crates 用 git 依赖 tag 锁定）；Phase 0 现仓可做（exchange+notify / 两包数据 / 首启不覆盖导入 / serve.rs move-only / 版本 tag）→ Phase 1 开仓搬 Runner；通知推荐 **C：共享 exchange 目录为唯一真相 + loopback POST 即时加速**（`event_id=package_id` 幂等 + 离线矩阵）；发行包数据边界；共 14~19 人日；4 项待裁定 | `RUNNER-SPLIT-DESIGN.md`（仓外） |
| UI 原型 | 可点静态页：`runner-params.html`（独立参数页：预设下拉/管理、按 group 分节、每参使用勾选/留空=不用、5 态）、`studio-params.html`（构建页动作库「任务参数」组 + 参数面板 + 插入参数 + T3 检查/一键声明）；4 张 1440×900 截图 | `prototype\`（仓外） |
| 决策单 | D1~D12 逐条：选项 / 推荐 / 影响（不施行则按推荐） | `DECISION-SHEET.md`（仓外） |

**本轮结论**：① 数据随仓/随包=设计已定（两包带数据 + Runner `bundled-data` 首启不覆盖）；② 参数不是 flow 步骤而是任务级契约 → 动作库加**消费类动作**（参数文字点击/等待/判断/拖动）+ 表达式插入参数 + `text_expr` 支持「参数作为 OCR 识别文字」；③ 预设 = Runner 侧命名值集合（values.json additive，四层优先级不变）；④ 拆仓=方案 A Phase 0 先行；⑤ 发布通知=exchange 文件为真相 + loopback POST 加速。**未改任何代码/UI；裁定后从 P0 开始实施。**

## 1.6 2026-11-05 18:1x→18:4x（seq 112）P0 首批落地（已实现）

> 用户尚未回复 D1~D12；按决策单「未回复按推荐」先落地 P0 中**不依赖参数设计裁定**的两项（数据随包/发布通知）+ 首启导入。设计依据 `RUNNER-SPLIT-DESIGN §5/§6`；实现记录 `ENGINE §22.53`；台账 `PENDING §E-82`。

| # | 交付 | 结果 |
|---|---|---|
| P0-2a | Studio 包带全数据 | `package.ps1` 新增 frames（frames.json+PNG，剔 `.backup/`）/relations/templates + `data:` BUILD-INFO + stage 审计；Studio 包 76,582,684 B（78 entries） |
| P0-2b | Runner 包带 `bundled-data/` | `package-runner.ps1` 生成 `{manifest.json,tasks,assets}`（BOM-free；逐文件 sha；`-NoData` 可精简）；Runner 包 37,793,723 B |
| P0-3 | Runner 首启导入 | 新 `bundled_data.rs`：manifest sha 校验 → 空数据根复制 → 写 `packages/bundled-import.json`；**永不覆盖**；幂等；失败回滚；`main.rs` Runner 启动前调用 |
| P0-1a | exchange 发布通知（后端+UI） | 新 `exchange.rs` + `config.rs ExchangeConfig`（默认关闭）+ `serve.rs` 3 路由 + `/task/publish` exchange 响应；Runner nav 徽标/「新发布（来自 Studio）」列表（安装→ack/忽略）；Studio 发布状态提示 |
| 验证 | 测试 + E2E | `cargo test --workspace` **746/0**、UI **1633/159/157/0/6**；两 release 仓外 E2E：Studio publish → exchange → Studio 停 → Runner 启动导入 → updates new → install → ack installed **ALL PASS** |

**本批未做**：HTTP notify 客户端/`.studio-writer.lock`/`compatible` 预检（最小集外）；参数 v2；新仓 `oas-runner`；`scheduler.json` 运行态化。

## 1.7 2026-11-05 18:5x→19:1x（seq 113）参数 v2 后端 + 构建端参数 UI + P0-1b（已实现）

> 用户澄清（19:12）：**D2「预设」只是举例**（不做预设值集合）；**D8 通知 = 发布后可被检测（远程网页/GitHub 或本地安装）**。设计 `PARAM-V2-DESIGN` §2.1/§3.2/§3.6/§4/§5；实现 `ENGINE §22.54`；台账 `PENDING §E-83`。

| # | 交付 | 结果 / 证据 |
|---|---|---|
| P1 后端 | **`text_expr`：参数作为 OCR 文字** | `TextSpec{Literal,Expr}` 替换 4 处（click_target.find sel/inline、requires/expect、drag from/to）；`text`/`text_expr` 互斥必填其一；运行时 step 进入一次求值、必须非空字符串、失败不点击/不 fallthrough；expect 轮询不重算；T3/T4 覆盖；**`flow_ops` 4→7**（`click_target.text_expr`/`expect.text_expr`/`drag.text_expr`）安装门禁；`ParamDecl.role` 可选。测试：flow 70 / runtime 167+2 / task 116 |
| P3 UI | **构建端参数动作库/面板** | 动作库「参数」组（参数文字点击/等待参数文字，生成标准 flow）；`click_target` 定位器「文字来源」三态（text / text_expr=param / 表达式，保留未知字段）；构建页 `#builder-params` 面板（GET /task/config、配置项…复用+保存回刷、`#builder-param-issues` 近似提示）；表达式「插入参数 ▾」；保存前 text/text_expr 互斥校验。UI **1727 checks**（+94） |
| P0-1b | **发布通知增强** | exchange event 增 `min_engine`/`flow_ops`；Runner `compatible` 真实预检；Studio→Runner loopback `POST /runner/notify`（纯 std TCP，best-effort，响应 `exchange.notify`）。测试：oasd 385 / exchange 8 项新增 |
| E2E | 参数链路 + P0 复跑 | 临时任务 `paramtest`：lint 0 errors → publish → manifest `flow_ops=click_target.text_expr`、`params_schema.own[0].role=ocr_text`（debug+release 均过）；两 release 新包 P0 双端 E2E **ALL PASS**（`_verify\p1-param-e2e-20261105\`、`_verify\p0-1a-e2e-20261105\`） |

**本批未做**：Runner 使用端参数 UI（P2，group 分组/使用开关/留空提示；按 D2 澄清**不做预设管理**）；远程更新 feed（D8 澄清：本地 exchange 已有，远程网页/GitHub 拉取+下载安装置下一步）；Studio 权威 `GET /task/lint` + expects/drag 结构化编辑；新仓拆分。

## 1.8 2026-11-05 19:2x→19:4x（seq 114）使用端参数 UI + 更新检测 feeds + oas-runner 仓（已实现）

> 用户指令：建 `oas-runner` 空仓，curl vs TLS 自行判断，开始下一步。**决定：HTTPS 下载用系统 `curl`（零新依赖；Win10+/macOS/Linux 自带）**。实现 `ENGINE §22.55`；台账 `PENDING §E-84`。

| # | 交付 | 结果 / 证据 |
|---|---|---|
| 独立仓 | **`oas-runner` 已创建** | `https://github.com/passioki/oas-runner`（public）；initial `e1d603a`（README 目标结构 + `.gitignore`）；Phase 1 搬迁前骨架 |
| Runner 参数 UI | 使用端「配置」对话框 | 按 `meta.params[].group` 分节（first-appearance，空组→「通用」）；每参「使用」开关 + label/key/类型/`role=ocr_text`「识别文字」徽标 + 控件 + 重置；bool **三态**（未设置/是/否）；string+enum/数字 enum→select；**留空/不使用 = 不注入（回退 default）**；required 无 default 阻止保存并逐行报错；顶部「N/M 项已设置 · 分组 G 个」；服务端 400 按 key/label 映射行内；**不做预设值集合**（D2） |
| 更新检测 | feeds（本地 + 远程） | `config.updates{enabled,fetch_timeout_ms,feeds[{name,enabled,index_url,package_base}]}`（旧 live.json 兼容）；`index_url` 本地路径或 `http(s)` URL，`package_base` 本地目录/URL 前缀/可空（事件 `package_url`）；`GET /runner/updates` 合并 exchange+feeds，新增 `source/source_kind/feed/download_required/package_url/errors[]`（坏 feed 200 fail-soft）；新 `POST /runner/updates/install {event_id}`：本地直接装、远程用 `curl -fsSL --max-time --max-filesize` 下载到 `packages/remote/` + size/sha256 校验 + 安装 + 自动 ack installed；URL 无 sha→`sha256_missing`，不匹配→400 `sha256_mismatch` + 删缓存 |
| 发布工具 | `scripts/make-update-feed.ps1` | 把 `dist/packages/*.oaspkg` 变成 feed `publish-index.json`（schema 1；可选 `-PackageBaseUrl`/`-CopyPackages`/`-Repo`/`-Version`）；可上传 GitHub Release/静态站点供 Runner 远程拉取 |
| UI 适配 | 更新列表 | 来源列（exchange→「本地 Studio」，feed 名原样；local/url 标注）、远程事件「需下载」提示、安装按钮走新 endpoint（失败旧后端 404 自动回退 `/runner/install` + ack）；安装包页手动路径安装保留 |
| 验证 | 测试 + E2E | workspace **788/0**（oasd **399** = 385+14）；UI **1799/159/158/0/6**；release build exit 0/5 warnings；**本地 feed + HTTP feed（python 静态站点）E2E ALL PASS**（来源/下载标记、curl 下载安装、sha 校验、installed.json/ack）；P0 双端 E2E、参数链路 E2E 复跑 ALL PASS |

**本批未做**：URL feed TTL/缓存（UI 5s 轮询会重复 curl；建议后端 ≥60s 缓存或 UI 降频）、feed 鉴权/私有仓 token、`packages/remote` GC、cross-source 事件去重已有（exchange 优先）；`oas-runner` 源码搬迁（Phase 1）。

## 1.9 2026-11-05 19:5x→20:0x（seq 115）Phase 1a + Studio 权威 lint（已实现）

> 用户「好的，下一步」→ 目标③源码拆分第一步 + 构建端最后缺口。实现 `ENGINE §22.56`；台账 `PENDING §E-85`。

| # | 交付 | 结果 / 证据 |
|---|---|---|
| Phase 1a-lib | **oasd lib+bin** | 新 `apps/oasd/src/{lib.rs,cli.rs}`；`main.rs` 641 行→**404 B**；`pub fn run()` / **`pub fn run_as(surface)`**（argv 前插 surface，`--config`/`--version` 仍全局解析）；20 模块 `pub mod`；测试迁移后仍全绿；**warnings 5→1**（无新增） |
| Phase 1a-app | **oas-runner 薄应用** | 新仓 `apps/runner/{Cargo.toml,src/main.rs(3 行),Cargo.lock}` + `ui/` 4 文件；`oasd = { path="../../../oas-core/apps/oasd", features=["ocr"] }`；`cargo build` exit 0（`oasd-runner.exe` 38,347,776 B）；smoke `:8792` `/runner/health` 200、`/runner/tasks` 200、`/app.js` 404；无需 `[workspace]` 空表 |
| lint-API | **`GET /task/lint`**（Studio-only） | `task.rs` 抽 `pub(crate) fn lint_one` 供 CLI/HTTP 共用（assets 根不同）；`{ok,task,issues:[{code,path,message}],errors,count}`；发现=200、缺任务/meta/flow=404、坏 meta/flow/全局 assets=400、坏 query=400、Runner 404；+6 测试 |
| lint-UI | **构建页权威检查** | 「检查」按钮 + 计数徽标 + 面板（summary/status/list/error/close）；loadTask 自动 silent lint、手动重跑、保存标 stale；T3「去声明…」复用配置桥、保存后重跑；近似提示折叠降噪；404 降级；+69 checks |
| 回归 | 全绿 | P0 双端 / 参数链路 / 更新 feed E2E **ALL PASS**（新包）；Studio lint 实机 200、oas-runner 薄应用实机 200 |

**本批 Phase 1b 已完成**：oas-runner 切 **git 依赖 + commit rev**（`39d97b8…`）+ Cargo.lock、`ui/`、`docs/BUILD.md`、`packaging/start-runner.bat`/`stop-runner.bat`、`/apps/runner/target/` gitignore。**仍待**：Runner 默认 UI 目录、`package-runner.ps1`/`bundled-data`/`README-RUNNER`/`live.example` 迁入、ort 首次下载说明、`--version` app 名。

## 1.10 2026-11-05 20:1x（seq 116）Phase 1b 收尾：oas-runner 独立打包（已实现）

> 用户「好的，下一步」→ 目标③收尾：新仓自己产出 Runner-only release。实现 `ENGINE §22.57`；台账 `PENDING §E-86`。

| # | 交付 | 结果 / 证据 |
|---|---|---|
| core | **`oasd::cli::run_as_with_args`** | `run_as` 委托；`extra` 默认参数插在进程参数前（用户可覆盖）；commit `802f58b`；workspace 仍 794/0 |
| app | **默认 UI + rev pin** | oas-runner main 无 `--ui`/`--ui=` 时注入 `--ui ui`；Cargo.toml/Cargo.lock pin `rev=802f58b` |
| pkg | **新仓独立 `scripts/package-runner.ps1`** | 662 行：`-DataFrom`/`-ToolkitFrom` 自动探测（扁平/Python 布局）、`cargo build --release`、stage（exe/ui/models/adb/**bundled-data schema1 逐文件 sha**/live/README/BUILD-INFO/start-stop bat）、审计（非 0 字节/UI 白名单/JSON 无 BOM/Cargo.lock rev）、`/` zip、`-DryRun` |
| 产物 | **独立包** | `D:\OAS-dev\oas-runner\dist\oasd-runner-v0.1.0-win64.zip` = **37,854,377 B / SHA `09E8E2B5…AF45` / 41 entries**；BUILD-INFO `surface: runner`、`runner_app: oasd-runner`、`oasd_rev=802f58b`、bundled-data 3 任务/14 文件/47,208 B |
| smoke | **50/50 PASS** | 8795 无 `--ui`：health 200（surface=runner）/`GET /` Runner UI 200/tasks 200/app.js 404/task/lint 404；bundled-data 首启导入 3 任务+assets、重启幂等；`--ui ui_alt` 覆盖；start/stop bat 起停 8789；结束无残留 |

**仍待**：tag/Release 与两仓版本联动（未 tag）、`--version` app 名、`-NoData`/`-SkipToolkit` 实包验证、macOS/Linux；以及更新检测 TTL/鉴权/GC、requires/expect/drag 结构化编辑。

## 1.11 2026-11-05 20:4x（seq 117）更新检测收尾 + 参数编辑补齐（已实现）

> 用户「好的，下一步」→ P2 更新检测性能 + 参数 v2 最后结构化编辑。实现 `ENGINE §22.58`；台账 `PENDING §E-87`。

| # | 交付 | 结果 / 证据 |
|---|---|---|
| TTL | **feed 索引缓存** | `updates.cache_secs` 默认 60（0=不缓存）；Server `Mutex<HashMap<(name,index_url),{fetched_at,index}>>`；命中不 curl/不读文件；失败不刷新（下次重试）；`?refresh=1` 绕过、坏值 400；响应 `cache_secs` |
| reuse | **远程包复用** | `packages/remote/<event_id>.oaspkg` size+sha256 匹配 → `downloaded:false,reused_cache:true`；篡改删重下；缺 sha `sha256_missing` |
| clear | **cache/clear** | `POST /runner/updates/cache/clear`（Runner-only）：删 remote 包+tmp、清 feed 缓存、幂等、Studio 404 |
| UI | **参数编辑补齐** | 文字来源三态覆盖 click_target.find/**requires/expect/drag.from/drag.to** 五处（互斥/保留未知字段/param 未声明不误改）；**T3 去声明预填 key**（`OAS_TASK_CONFIG.open(...,{prefillParam})`、同 key 去重、坏 JSON 降级） |
| 验证 | 测试/产物 | workspace **801/0**（oasd lib 412=405+7）、UI **1919**（+51）、build exit 0/1 warning；P0/更新 feed/参数 E2E **ALL PASS**；oas-runner rev bump `78be71d` 独立包 **37,871,134 B / `B127C0FC…66DB`**（41 entries，8796 smoke health/UI/404/bundled 导入通过）；oas-core 重打包 Studio `239FA854…F51E` / Runner `023CD7F0…EF79` |

**仍待**：tag/Release 与两仓版本联动（未 tag）、`--version` app 名、`-NoData/-SkipToolkit` 实包、macOS/Linux；UI 可选手动刷新/缓存复用文案；产品验收/P2/Phase 5。

## 1.12 2026-11-05 21:1x（seq 118）动作目录统一 + 配置项成熟 UI + 最终审计（已实现）

> 用户反馈两项 + 要求审计/文档/交接/关机。实现 `ENGINE §22.59`；台账 `PENDING §E-88`；审计 `docs/AUDIT-2026-11-05.md`。

| # | 交付 | 结果 |
|---|---|---|
| 动作目录 | **单一事实源** | `actionCatalog()`（OP_META+PARAM_ACTIONS）/`actionAvailable()`/`buildActionStep()`；调色板与 if/loop「是/否/循环体」下拉统一；参数预设未声明时禁用+原因；`Object.keys(OP_META)` 仅剩只读标签用途 |
| 配置项 UI | **结构化表单** | `#task-config-help` 帮助块（用途/写法/优先级/留空语义/随包）+ `+添加参数`/高级 JSON/模式提示 + 每参卡片（key/label/type/group/default/enum/min/max/required/role/description，逐字段帮助+示例、上下移/删除、空态示例）+ 保存前语义校验（key 唯一、type、default 类型、enum、min≤max、长度、role）+ T3 prefill 行高亮；高级 JSON 双向同步/坏 JSON 降级；未知字段 round-trip |
| 审计/交接 | **防漂移** | `docs/AUDIT-2026-11-05.md`（代码审计 + 文档漂移对照表 + 残余风险 + 交接）；全套文档同步 seq 118 |

**验证**：workspace **801/0**；UI **2028/164 ids/158 runner ids/0 dead/6 nav**；`node --check` 3/3；debug+release exit 0（1 warning）；三路 E2E ALL PASS；新包 Studio `F2184381…1B2F` / oas-core Runner `163679D6…9065` / oas-runner `F6A6C301…D46C`。**残余风险**见 AUDIT 文档（配置项表单边界/更新检测鉴权/GC/tag 策略）。

## 1.14 2026-11-06 07:3x（seq 119）参数插入/条件表达式修复 + 参数点击素材选择

> 用户反馈：任务「切换预设御魂」加两个参数后与 if 衔接，JSON 多处自动出现 `"sel": "觉醒材料"`。

| # | 交付 | 结果 |
|---|---|---|
| 插入 | **表达式插入防粘连** | `exprInsertText`：cond 末尾追加自动 ` and `，其余按标识符边界补空格；`insertTextAtField(field,text,mode)` 统一，「插入参数 ▾」/面板插入生效 |
| 自愈 | **旧坏条件修复** | `repairConditionExpr`：`truetext_visible(...)` → `true and text_visible(...)`；`normalizeStepArgs` 载入/应用/**保存前**执行；清 if/while 遗留 `args.sel` |
| 素材 | **参数点击不再静默首选取** | `selectedOcrAssetId()` 上下文复用 → 唯一素材直用 → **多个弹选择对话框**（确认/取消）；调色板与 if/loop 分支共用 `prepareActionStep()`；状态栏说明素材可改 |
| 数据 | **用户任务修复** | `tasks/切换预设御魂/flow.json` 条件修为 `text_visible(param('group'))`（保留用户两参数/if 分支；工作区未提交） |
| 验证 | UI only | UI pure test **2039/164/158/0 dead/6 nav**（+11）；`node --check` 3/3；`git diff --check` clean；Rust 未动（workspace 801/0 基线）；**按用户要求只推送、不发布**（未重打 release/tag） |

## 1.15 2026-11-06 09:0x（seq 121）region 操作区域 + long_press + curve_swipe（已实现）

> 用户确认 seq 120 设计 4 项并追问等待语义。实现 `ENGINE §22.61`；台账 `PENDING §E-91`。

| # | 交付 | 结果 |
|---|---|---|
| region | **4 模式** | `region{mode:asset|ocr|nearest|offset, offset:[dx,dy(,w,h)]}` 挂 click_target OCR（含 inline）/ drag OCR 端点 / long_press.find；nearest=行 bbox 中心距 **OCR 识别 ROI 中心**最近；offset=行 bbox 左上角+偏移（2/4 元）；无 region=现状；非 OCR kind 报错 |
| long_press | **独立长按** | find 复用 locator 链；`hold_ms` 默认 **3000**（100..=10000）；`hold_range` 随机拟人优先；requires/expect；InputSink 默认 `swipe(p,p,ms)` + Adb `input swipe x y x y ms`；不记 clicks |
| curve_swipe | **多段近似** | points 2..=32、duration_ms [600,1000]/100..=10000；逐段独立 swipe（日志注明）；flow_ops 门禁 |
| flow_ops | **7→11** | `long_press`/`curve_swipe`/`click_target.region`/`drag.region`；旧 flow 空集；老 Runner 拒绝 |
| UI | **构建页** | 动作库长按/曲线/参数长按；操作区域下拉+offset；hold/points 面板；保存前校验；**等待语义帮助**（阻塞等待→下一步；超时失败；不触发独立动作） |
| E2E | **新包** | `regiontest` lint 0 errors → publish → manifest flow_ops 含 4 个新标记（`_verify\opregion-e2e-20261106\`） |

**验证**：workspace **824/0**（flow 77 / runtime 178+2 / task 119 / adapter-pc 5 / oasd 412）；UI **2146/164/158/0 dead/6 nav**；debug+release build exit 0（1 warning）；提交 oas-core `644249c`、oas-runner `7e64156`（pin `644249c`）；**只推送、未重打 release/未 tag**。

**剩余**：真机长按/曲线时序验收、模板多候选 nearest（P2）、更新检测鉴权/GC、产品验收/P2/Phase 5。

## 1.16 2026-11-06 09:4x（seq 122）A 方案：region.asset（全库操作区域素材）+ 多候选 nearest

> 用户确认 A：OCR 识别素材 + 操作区域素材（全库）；nearest=候选匹配 bbox 中心离参数 OCR 匹配行 bbox 中心最近；无匹配=本步失败。实现 `ENGINE §22.62`；设计 `OPERATION-REGION-DESIGN §7`；台账 `PENDING §E-92`。

| # | 交付 | 结果 |
|---|---|---|
| flow | **region.asset** | `OpRegion` 增 asset（仅 asset/nearest；ocr/offset 带 asset 报错）；T4 收集；flow_ops **11→13**（+`click_target.region_asset`/`drag.region_asset`），老 Runner 拒绝 |
| perceive | **多候选匹配** | `match_template_all`/masked_all：NCC→3×3 局部极大→阈值→分数降序 NMS（中心距<max(2,min(tw,th)/2)）→≤64；best 路径逐字节不变 |
| runtime | **操作区域解析** | `operation_candidates`/`operation_region_for_asset`：模板/集合 asset=best、nearest=按「候选匹配 bbox 中心 ↔ 参数 OCR 行 bbox 中心」距离平方选最近（并列取高分）；固定 region 资产=其区域；OCR 资产取其操作区；无候选/未声明/无参考点=响亮失败带 site；三处接入 click/drag/long_press；无 asset 旧行为逐字节不变 |
| UI | **全库操作素材选择** | 参数点击/长按对话框与结构化 region 编辑器加「操作区域素材（整个素材库，可选：无/OCR/模板/区域/集合/其他）」；mode=asset/nearest 写 `region.asset`、无选不写、ocr/offset 丢弃；保存前存在性/组合校验；+8 纯函数、UI **2214**（+68） |
| E2E | **regionasset** | lint 0 errors → publish → flow_ops 含 `click_target.region_asset`/`drag.region_asset`（`_verify\opregion-aplan-e2e-20261106\`） |

**验证**：workspace **841/0**（perceive 11+2 / flow 78 / runtime 186+2 / task 122 / oasd 412）；UI **2214/164/158/0 dead/6 nav**；debug+release build exit 0（1 warning）；提交 oas-core `ba6813e`、oas-runner `768a8b7`（pin `ba6813e`）；只推送不发布。

**剩余**：真机验收（操作区域落点/多候选最近/长按曲线时序）、nearest 1280×720 性能实测、更新检测鉴权/GC、发布策略、产品验收。

## 1.17 2026-11-06 12:0x（seq 123）用户新要求（仅记录，未实现）

> **seq 122 的 A 方案交互方向被推翻**；新会话先读 `docs/PARAM-ASSET-REDESIGN.md`（完整需求/影响/待确认）与 `PENDING §E-93`。

| # | 用户要求 | 现状/状态 |
|---|---|---|
| 1 | 动作库「参数文字点击」→「参数文字动作」；参数**不属于动作库**，回退/删除动作库参数相关内容 | 现有 `PARAM_ACTIONS` 参数预设仍在 → ⬜ 待回退 |
| 2 | **参数=素材的一种**（使用端可改识别文字；初始识别区为空；改名「使用端OCR文字配置项」待定） | 现为 `meta.params`/`ParamDecl` → ⬜ 待设计/迁移 |
| 3 | 素材行点「识别」返回**所有候选+评分**；后端返回全部候选；测试动作演练所有候选；真实点击测试可选候选（序号+评分） | 无素材候选 API/UI；`match_template_all` 底层已有 → ⬜ 待实现 |
| 4 | **量尺**：相对偏移时测两点直线距离，直接复制剪贴板 | 无 → ⬜ 待实现 |
| 5 | **操作区域三方式 + 递归套娃**：参数真实识别区/素材操作区（单/多候选最近）/相对偏移；素材1→素材2→…→末端区域；防环；依次识别、任一缺失本步失败 | 现 `region.asset` 仅单层 → ⬜ 待重设计 |
| 6 | **废弃操作区“与真实识别区有无交集”两状态**（先审计现有全图回退+相对位移） | ⬜ 待审计确认 |

**接手顺序**：① 审计事项6/现有全图回退逻辑；② 与用户确认 `PARAM-ASSET-REDESIGN.md §5` 的 7 个问题；③ 按建议顺序实现（素材候选 API → 参数素材类型/迁移 → 动作库去参数化 → 操作区递归/防环 → 量尺 → 文档/测试）。**先不要继续修改 seq 122 的 A 方案 UI。**

## 1.18 2026-11-06 12:1x→13:3x（seq 124）用户权威澄清 + 审计 + 全量实现 + 验收（✅ 已完成）

> 用户 12:1x 主动给出 6 事项权威逐字澄清（`PARAM-ASSET-REDESIGN §1b`）；Lead 决策 D1~D6；三路只读审计 + 实施方案 `PARAM-ASSET-IMPLEMENTATION.md`（Lead 裁决 13 条）；用户 12:5x 简化令（全新开发、不兼容旧参数/旧配置/旧包）后取消迁移/兼容交付。**最终验证：workspace 904/0、UI 2512/194/0 dead/6 nav、E2E 27/26/0/1（无 ADB skip）**；代码 commit `aab6372`，docs 提交见 `git log -1`；**只推送不发布，未 tag/未 release**。

| # | 交付 | 结果 / 证据 |
|---|---|---|
| 审计 | 3 路只读审计（仓外） | `_verify\paramasset-audit-20261106\{01-region-two-states-audit,02-param-action-audit,03-asset-candidates-measure-audit}.md`；**事项6确认**：两状态只在数据/UI 层、`oas-runtime` 0 读 `action_source`；模板全帧金字塔回退+操作区平移已存在（OCR 运行时无全屏回退）⇒ 已删除两状态 |
| P0a | 素材候选 API（后端） | `GET /task/asset/candidates`（serve 路由+handler/OCR 分支；runtime pub helper）；workspace **848/0**（+7） |
| P0b | 废弃两状态（后端） | `action_source` 降级 deprecated（skip_serializing、旧 JSON 容忍）；四处一致性校验删除；serve 写接口忽略旧字段；workspace **850/0**（+2） |
| P0c | 动作库去参数预设 + 删两状态 UI | 产品文件 0 残留；UI **2186**/164/158/0 dead/6 nav（删旧断言所致） |
| B-T1 | oas-task：ocr_var/op_region/派生参数同步/T8 | `KINDS`+ocr_var、`OpRegion` 三模式、`validate_op_region_chain`、`sync_ocr_var_params`、`write_meta(.bak)`、T8 错误级、`ParamDecl.asset_id`；oas-task **143/0** |
| B-T2 | oas-runtime：ocr_var 读取/操作区链/候选 helper | `OcrVar`/`AssetOpRegion`/`ResolvedOpRegion`、`ocr_read_var`（空 roi 全屏、param>runtime_text）、`resolve_asset_op_region`（self/offset/asset+pick、防环、region 末端、miss=Err 不点击）、`candidates_for_asset`；runtime **201/0**（+15） |
| B-T3a/b | serve：新字段/sync 触发/config 保护/候选扩展/test_action/validate | 素材 CRUD+save_flow+config 触发派生参数 sync（.bak+原子写、`synced`）；`/task/config` 派生行只读；候选 ocr_var 空 roi=screen；`test_action` dry/real（click+long_press；drag 400）；`validate_op_region`；oasd **436/0** |
| U-T4 | Studio 素材行/识别文字/全部候选/操作区递归对话框/offset 画布 | 识别文字对话框（新建/编辑）、素材行识别=全部候选、操作区三方式递归对话框（validate+两次 update）、offset 画布绘制+保存、test_action 候选点测+真实候选下拉；UI **2474** checks / **194** ids |
| U-T5 | Studio 动作库目标选择接线 + 旧 region 收口 | 属性面板「动作目标（素材库）」搜索选择、ocr_var→text_expr param(key) locator、保存前目标校验（含 param key 派生校验）、旧 region 仅旧 flow 显示、save_flow sync 提示；UI **2512**（+37） |
| U-T6 | Runner「使用端OCR文字配置项」分节 | `runnerParamGroupsEx` 前置分节 + 素材名/默认值副标题 + 旧参数兼容；+19 checks |
| D-T8 | E2E（仓外新模型任务）+ 文档归档 + 交接 | E2E 27 total/26 PASS/0 FAIL/1 SKIP exit 0（`_verify\seq124-e2e-20261106\`）；`ENGINE §22.64`、`OPERATION-REGION-DESIGN §8`、`PARAM-ASSET-IMPLEMENTATION`「实现结果」、`README` 索引 |

**取消**：U-T7 旧配置迁移/兼容（用户 12:5x 简化令：全新开发、旧配置可删）。**未做（后续可选）**：drag 测试动作、素材行长按测试入口、识别文字 ROI 接画布、S8 派生行只读卡片、候选 op_rect 预览、全屏 OCR 性能、真机 E2E、Phase 2（去 meta.params）。详见 `PENDING-LEDGER §E-93 §D.6`。

## 1.19 2026-11-06 18:1x→18:4x（seq 125）验收反馈 13 项：3 路排查 + 理解/整改计划（awaiting-user-confirmation）

> 用户 seq124 交付后提交 13 条验收反馈 + 剩余可选项答复；Lead 派 3 路只读排查（复用现成成员避开团队上限），产出 `_verify\acceptance-20261106\{01-bug-triage,02-refactor-design-inputs,03-infra-ops}.md`；计划文档 `docs/ACCEPTANCE-2026-11-06.md`；台账 `PENDING §E-94`。**本轮未改代码**。

| 组 | 内容 | 结论要点 |
|---|---|---|
| A 先做 | #5 ocr_var 阈值 / #9 改识别文字丢操作区 / #11 OCR 误配 / #6 多边形显示层 | #5 后端全支持仅 UI 阻断（app.js:6699-6704）；#9 全屏 ocr_var 走 `replace:true` 且 payload 漏 op_region/action_shape/threshold → 链清空（Runner 端只写 values.json 不丢）；#11 `ocr_text_matches` 允许 1 编辑距离（runtime:969-999）→ 队伍1=队伍2/3，候选+点击+等待共用；#6 保存基本保留、显示层恒矩形+update 无 polygon |
| B 重构 | #4 操作区模型 A + 删 offset；#10 统一框选工具；#6 保存层 | 现状=模型 B（链末端被引用素材操作区决定落点，UI 还 PATCH 被引用素材）与用户要求冲突 ⇒ 改模型 A（链只作识别门槛、落点=当前素材操作区、被引用素材只读）；统一框选可行（拖动=矩形/点击=多边形/双击收尾；反选仅操作区） |
| C 候选 | #3 候选口径统一 + 普通 OCR 链节点 | 模板/集合/ocr_var 多候选 ✅；普通 ocr_line/ocr_lines 作链节点运行时失败（校验却放行）；根目标只取最佳；`candidates_for_asset` 未接线 |
| D 参数 UI | #1/#2 参数入口删除 + 条件编辑器改 ocr_var + goal battles 字面量 + Runner 通用参数区 | 10 个用户侧触点已定位（报告 A 节）；服务端派生模型保留（内部/诊断） |
| E 运维 | #7 Runner 启动脚本 / #8 GitHub About 乱码 / #12 仓库体积 | 启动脚本"包里有仓库根没有"；About 存储值即字面 ?（不可逆，需 gh/PAT 授权重写）；D:\OAS-dev 24.6GB（_verify 10.4 + core target 9.27 + runner target 2.51…），可回收 15-18GB（需批准、先停 8788） |
| F 设计 | #13 变量系统 v2（+Phase2） | 通用变量（ocr_text/string/int/bool/enum）+ 使用端可改 + Profile（多账号/模拟器）；先出设计再实现 |

**待用户确认 8 项**（见计划文档 §5）：#4 模型 A/OCR 操作区是否跟随识别文字、链递归/region 门槛、#3 候选语义、#10 反选范围/模板多边形/ocr_var 多边形、#1/#2 条件编辑器与 battles/Runner 通用参数区、#8 gh/PAT 授权、#12 清理批准、#13 变量系统方向。

## 1.20 2026-11-06 18:4x→19:4x（seq 126）验收 13 项整改 + 变量系统 v2 设计（✅ done）

> 用户 18:4x：「全按推荐，尽量用子进程」→ Lead 按批次派子进程完成；每批 Lead 独立验收后提交推送（dev=main=origin）。**最终：workspace 929/0、UI 2217/177/0 dead/158 runner/6 nav、E2E seq126 54/53/0/1、`cargo check -p oasd --features ocr` exit 0。**

| 批次 | 内容 | 提交 |
|---|---|---|
| A | #11 数字保护+严格候选、#9 replace 继承+payload、#5 ocr_var 阈值、#6 多边形显示层、#7 start-runner 脚本（根+packaging） | `c16a536` |
| B（后端+serve） | #4 操作区模型 A（链=识别门槛/落点=根 self/offset 删除/OCR self 跟随/region 禁作链节点）、polygon v3（三态/模板同 ROI/ocr_var mask）、#3 普通 OCR 链节点期望文字 + preview_root_candidate det+rec、公共 self_operation_geometry、候选 API 统一 | `4ccd182` |
| D | #1/#2 参数概念从 Studio/Runner 用户界面全部移除（旧 flow 只读回显、/task/config 内部桥保留、Runner ocr_var-only、battles 字面量） | `07ed361` |
| D2 | goal battles 不再自动补 params.max_battles | `c3739f6` |
| B3a | 操作区对话框模型 A（两选一/只读链/单次 update）、删除 offset 绘制 UI、拖测接入候选、素材行「长按测试」 | `9a3c77f` |
| B3b | #10 统一画布框选（3 按钮/拖动=矩形/单击=多边形/双击收尾/反选仅操作区）、#6 polygon 保存层（update v3/模板不重截图/矩形清空 polygon） | `303a7ac` |
| E | #8 GitHub About 乱码已用 UTF-8 API 重写（中文描述+homepage）；#12 清理 16.8GB（D:\OAS-dev 24.6→7.79GB：_verify 9.68 + incremental 4.37 + oas-runner target 2.51 + dist/stage 0.16） | 外部/本机 |
| F | #13 变量系统 v2 设计 `docs/VARIABLES-V2-DESIGN.md`（10 项按推荐：meta.variables 唯一真相、Runner 可改过滤、values.json v2 Profile、多模拟器隔离部署） | docs |

**剩余（不阻塞）**：Firefox 手测、oas-runner 独立仓 root 脚本/pin bump、`loop n=param('max_rounds')` 模板、T5 手写 polygon 校验、真机 E2E、变量系统 V2.0 实现、更新检测鉴权/GC、发布策略。**未 tag/未 release。**

## 1.21 2026-11-06 20:0x→20:2x（seq 127）用户验收纠正：多候选全帧 + 操作区链末端模型 B（✅ 已修复推送；V2.0 启动）

> 用户 20:0x 反馈 3 条：① 多候选仍未生效（素材 `test` 在 #6 帧应有多个却只显示 1 个）；② 引用素材时落点应为「**最后一个被引用素材自己的操作区**」，只有 self 才用自身（纠正我们上一轮的模型 A）；③ 目标场景：`预设识别` 识别「队伍3」→ 引用 `test` nearest → 点击 `test` 不可用。
> 根因（Lead 用用户数据副本实测）：`test`=template、roi=[973,144,35,38]，候选枚举只搜素材 roi → 只 1 个；全帧搜索实测 3 个（y=144/295/445）。上一轮把操作区链误改为「模型 A（落点=根 self）」。
> 修复提交 `fd3339f`：`oas-perceive` 新增 `match_template_all_full`/`masked_all_full`（4× 金字塔 coarse 全帧扫 → 全分辨率 refine → NMS≤64；1280×720/35×38 debug 114ms / release 6.4ms，原 7.4s）；`oas-runtime` 候选/链 pick 改全帧、`walk_op_region_chain`/`resolve_asset_op_region`/`preview` 恢复末端 self 几何；UI 文案按 self/asset 动态。
> Lead 独立实测（用户数据副本）：candidates test@#6 **count=3**（973/144、973/295、973/445，score 1.0/.973/.905）；`test_action dry id=预设识别` → `action_box=[973,445,35,38]`（test 候选，不是预设识别 [573,453,64,29]）；`队伍1`→`[973,295,35,38]`；`test`→`[973,144,35,38]`。
> 验证：workspace **934/0**（perceive 14 / runtime 214 / oasd 449+）、UI **2223/177/0 dead/6 nav**、`cargo check -p oasd --features ocr` exit 0。文档（task-35）与 E2E seq127（task-34）随后；V2.0 变量系统已启动（task-36 backend V2.0a → V2.0b serve → V2.0c UI）。

## 1.22 2026-11-06 20:3x→20:4x（seq 128）变量系统 V2.0 实现（✅ 已完成并推送）

> 用户剩余项「1 变量系统 V2.0 实现」→ 按 `docs/VARIABLES-V2-DESIGN.md`（10 项已按推荐）分三批：
> - **V2.0a** `e1b816f`：`Variable` 模型 + `Meta.variables` 唯一真相；`effective_variables()` 旧 params 内存桥（不写盘）+T10 warning；`sync_ocr_var_variables`（ocr_text 变量 key=param_key/default=runtime_text/asset_id）；T3/T7 改 input variables、T8 改目标；loader 子任务默认、run.rs 注入改 `variable_defaults()`+`validate_variable_values`；`PACKAGE_SCHEMA=2`+`Manifest.variables_schema`+旧包响亮拒绝。
> - **V2.0b** `d780a4b`：`GET/POST /task/variables`（effective+T10；POST 结构错 400、T3/T8 非阻断 issues、原子写+.bak、写盘无 params、保留未知顶层字段）；`/task/config` deprecated 别名（params 只读镜像）；Runner config GET variables/PUT 仅 `runner_editable&&scope=input`（runtime/未知/不可改 400）；sync 切 variables 并清 params（去双写）；test_action/create/publish/CLI 迁移。
> - **V2.0c** `d780a4b`：Studio 变量面板/变量对话框（识别文字变量绑 ocr_var、param_key 只读；标量全字段）/「插入变量 ▾」/动作目标 key 集 variables 优先；Runner 配置两分节（识别文字变量 / literal 按 group，空组=通用）、隐藏 runtime/不可改、required 橙提示、PUT 只收可改键、高级 JSON 隐藏。
> **验证（Lead）**：workspace **949/0**（task 157 / runtime 214 / oasd 454）；UI **2303/177/0 dead/158 runner/6 nav**；E2E seq128 **77/76/0/1**（`_verify\seq128-e2e-20261106\`；skip=无 ADB）；`cargo check --features ocr` exit 0；用户数据副本冒烟：GET /task/variables effective+T10、POST 后 meta.json 无 params、`/task/config` alias、Runner config variables。
> **偏差/后续**：旧 meta.params 只内存桥（M3 用户重建，不自动改写数据）；`/task/config`+`/runner/tasks.params` deprecated 过渡别名；variables 行内未知字段不保留；T8 sync 硬错误不阻断保存（issues）；M1 导入 CLI 未做；`values.json` 零迁移；条件编辑器仍 ocr_var 下拉（V2.1 扩标量）；V2.1/V2.2 后续。

## 1.23 2026-11-06 21:26→22:0x（seq 129）动作库一致性：拖动改素材端点 + 全局素材 lint 修复（✅ 已完成并推送）

> 用户 21:26 反馈 3 条：① 动作库「拖动（拟人）」UI 不成熟，没有选择素材（应与点击素材一致）；② 关闭/删除拖动属性里的坐标输入，坐标应通过建立操作区域素材完成；③ 排查其余动作库动作，尽量与点击素材 UI/代码逻辑一致。
> 实现 `dce16c7`：**oas-flow** `DragTarget::Asset { sel }`（`{"kind":"asset","sel":"<id>"}`，from/to 可用；旧 region/point/ocr_text 保留兼容）；**oas-runtime** `drag_asset_endpoint` 复用点击素材同源（素材识别 → 根 self 几何 → op_region 链模型 B 末端盒），未命中 Err 不 swipe，采样/方向/24 次 to/duration/margin/audit 不变；**package** `flow_ops` 新 marker `drag.asset`（旧 Runner 安装拒绝）；**UI** `structuredDrag` 重写：起点素材（必填）/终点素材（可选）用与点击素材相同的素材选择器，写 `{kind:"asset",sel}`，清空终点=方向+距离；**删除坐标 ROI x/y/w/h、拖动端点编辑器/样式/断言**；旧坐标端点只读提示不改写；方向/距离/时长/margin 保留；saveFlow 校验；动作库审计：click/click_target/long_press 文案与选择器统一、dismiss_popup action 标签、swipe/curve_swipe/click_point 保留坐标但加「低层坐标手势…元素拖动请用拖动（拟人）」提示。
> 附带修复 `82dbca3`：全局素材模式下 flow 引用全局 ocr_var（如 `预设识别`）时 T8 lint 假红且 publish/install 阻断 → `lint_into_with_calls_and_global_assets`/`merge_global_assets_for_lint`（global base + 任务本地覆盖；T8 合并视图、T5 本地、T4 local∪global）；`lint_one`/publish/install（install 另合并包内快照、包内优先）统一修复。
> **验证**：workspace **968/0**（oas-task 160 / oas-flow 82 / oas-runtime 222 / oasd 458）；UI **2323/177/0 dead/158 runner/6 nav**；`cargo check --features ocr` exit 0；冒烟 `_verify\seq129-smoke\`：保存 drag(asset `test`→`预设识别`) flow 200 → lint **0**（修复前 T8 假红）→ publish 200（package_schema=2、variables_schema 含全局 预设识别）→ 全新 Runner 安装 200（包快照初始化全局 store）→ 安装后 lint 0。
> 遗留：旧 flow 拖动坐标/OCR 端点后端仍兼容运行但 Studio 只读（建议重建为素材端点）；swipe/curve_swipe/click_point 保留低层坐标手势（UI 已提示）；旧 `.oaspkg`（schema=1）需重发布。

## 1.24 2026-11-06 23:04→23:1x（seq 130）点击几何同源修复 + 框选可移动/叠加/旧参数误判/自动刷新（✅ 已完成并推送；按用户要求关机）

> 用户 23:04 反馈 5 条：① 框选区域可否用鼠标点框移动（所有框选）；② 点击叠加分布失效；③ if 加条件行后出现「旧参数来源（只读）：param('p_93445f68')」；④ 任务页素材应自动刷新；⑤ **真实点击点测/实际运行点击失效**（并要求完成后关机）。
> **#5 根因**（Lead 从 `runs/*.clicks.jsonl` 定位）：OCR 点击 target rect 已按 Q1 跟随识别行平移（如用户素材 `预设分组识别_式神录`：roi=[1085,84,160,546] + action_shape=[1057,72,120,50]），但最终采样 `adjust_plan_for_shape→translated_click_shape` 只平移 template、OCR 返回**作者绝对 shape** → 点击被拉回作者区（y≈72..122），而识别行在 y≈245/538。修复：新增 `click_geometry_for(sel,matched)` **rect+shape 同源**（OCR 用 `ocr_self_operation_geometry`；roi=None 全屏保持绝对；template/collection/region 不变），统一 `Step::Click`/`click_target`/`long_press`/`interrupts`/`dismiss_popup`/`region.asset mode=asset`。dry 预览（用户数据副本）：识别行 y=257 → `action_box=[1103,245,120,50]` ✓。
> **#1** 画布点击已有框优先选中 → 拖动整体平移（识别矩形/多边形、操作区 rect/polygon/反选、候选编辑区；clamp/未保存提示/Esc 撤销；保留空白拖动新建/单击加点/双击收尾）。**#2** 点击叠加 normalize actual/target/miss；热力+实际点+目标环+miss 叉；修缓存/面板刷新/图例。**#3** `param(key)` 已声明（meta.variables ∪ 素材库 ocr_var.param_key）→ 变量来源，仅未声明才旧参数只读。**#4** 工作台/构建页素材 8s 自动刷新 + 可见/聚焦/visibilitychange（隐藏暂停、去重、保持滚动/选中）。
> **验证**：workspace **972 passed / 0 failed**（runtime 226 / flow 82 / task 160 / oasd 458）；UI **2370 checks / 178 ids / 0 dead / 158 runner ids / 6 nav**；`cargo check --features ocr` exit 0。**提交** `a3556e3`；**未 tag/未 release**；按用户要求本轮完成后关机。
> **遗留**：全屏 ocr_var（roi=None）操作区仍绝对坐标（Q1 限制）；旧 `shape_aware` 死分支待清；#1 非编辑态预览框不拖动；#4 仅工作台/构建页可见时轮询；浏览器手测清单见 seq130-ui-progress.md。

## 1.25 2026-11-07 08:10→08:2x（seq 131）恢复「加入素材集合」入口（✅ 已完成并推送）

> 用户 08:10：「把素材添加为集合的UI好像没了，请检查原因。按钮可以放到素材设置里。」
> 原因：`3eb9fa8`（10-31）清理未使用「多选素材+新建集合」UI 时把创建集合入口移除了；后端 `POST /task/asset/collection` 与集合测试/删除入口一直在。
> 修复 `eaf68fe`：① 素材设置菜单新增「加入集合」（位于「位置帧」后；template 专用，其它 kind 置灰说明）；② `#collection-add-dialog`：已有集合下拉（成员数/当前集合）+「＋新建集合…」+ 新集合 id；跨集合先移出旧集合（剩 0 成员则删除；被 flow 引用按 400 显示）再写入目标；保存调用 `POST /task/asset/collection`（保留 threshold/action_shape），成功后刷新任务库/全局库并提示 `tasks_rewritten`；③ 任务素材库/全局素材库/帧详情三处 `onCollectionAdd` 接线。
> 验证：UI **2391 checks / 190 ids / 0 dead / 158 runner ids / 6 nav**、node --check 全绿；数据副本 API 冒烟 create/list/delete 均 200。未 tag/未 release。


## 1.26 2026-11-07 08:26→08:5x（seq 132）素材集合成员扩展到所有素材（✅ 已完成并推送）

> 用户 08:26：「目前只有模板图能加入集合，请扩展到所有素材。」
> 实现 `7229c59`：成员允许 `template/ocr_line/ocr_lines/ocr_var/region`（嵌套集合拒绝）。oas-task `save_collection` 放开 + `expand_collection_steps` 按 kind 生成 click_target locator（template / OCR text / ocr_var param 表达式 / region roi；无期望文字普通 OCR 成员 load 期报错）；oas-runtime collection `locate` 按 kind 分派（region=固定 ROI 1.0 / OCR=locate_ocr / 模板）、混合容错；click 几何支持 region 成员；`chain_geometry_for`/`effective_asset` 使获胜成员自己的 op_region 链在 click/click_target/long_press/drag 生效；新增 `collection_candidates_with_ocr` + `collection_candidates` 统一枚举（模板全帧多候选+OCR 行+region 1.0；score desc→y→x、稳定成员序；candidate_index 与候选 API/test_action 同序）；serve `collection_ocr_lines`、候选 API 带 member/member_kind、collection/match 混合 best（template-only 保留旧语义）、test_action 经 runtime preview 自动支持；UI 菜单/对话框对 5 类启用（template_set 禁用）。
> 阈值：模板成员沿用集合级 threshold；混合集合每行 pass 用成员自身阈值（模板 NCC / OCR 最低置信度 0=unset / region 1.0），`threshold` 字段仍回显集合级值。
> 验证：workspace **980/0**（oas-task 161 / oas-runtime 231 / oasd 460）、UI **2407 checks / 190 ids / 0 dead / 158 runner / 6 nav**、cargo check --features ocr exit 0；Lead 冒烟（用户数据副本 frame6，无真实点击）：混合集合 candidates **count=5**（test×3+OCR×2，含 member/member_kind）、collection/match winner OCR、test_action dry index0→test / index1→OCR 成员（op_region 链生效）。
> 遗留：click_target 成员不继承集合级 locator 覆盖；无期望文字普通 OCR 成员 load 期报错；混合集合真实点击（OCR/region）未真机验证；region 恒 1.0 会压过其它分数（有意兜底，可后续加优先级）。


## 1.27 2026-11-07 12:06→12:2x（seq 133）点测/拖测卡死修复 + 文字素材多候选（✅ 已完成并推送）

> 用户 12:06 三条：① 常驻弹窗识别线程与主任务并行设想（**低优先，记档 §E-101 不实现**）；② 模板图识别多候选、文字素材没有多识别；③ 素材「预设队伍更换_通用战斗准备」点击测试（真实模式）卡住，点测/拖测都用不了（允许关闭编辑端后台）。
> **#3 根因**（Studio 日志）：`POST /task/asset/test_action` 12:04:46 起永不返回、其它 GET 正常 → `crates/oas-adapter-pc` `Adb::run` 用 `Command::output()` 无超时，ADB 偶发卡住即永久阻塞服务线程。修复 `9323ace`：`run_with_timeout`（spawn + stdout/stderr 双线程排空 + try_wait deadline；超时 taskkill /T 杀进程树；默认 15s、screencap 20s；错误含 argv）；serve OCR 引擎锁毒化 `lock_recover`（10 处）；UI `ASSET_TEST_TIMEOUT_MS=45000` + AbortController/fetchJsonWithTimeout/postJsonTimed + finally + in-flight，点测/长按/拖测及 `/input/swipe` 超时后恢复并提示。冒烟（假挂起 adb，无真实点击）：test_action real 400 / 15114ms，服务恢复。
> **#2**：OCR 候选=识别区域内全部 det+rec 行（仅按素材 OCR 最低置信度过滤），每行 `text/score/pass`（pass=无期望或 strict 匹配期望），排序 pass 优先→score desc→y→x（期望行存在则 index0=期望行）；runtime `preview_root_candidate` 与候选 API 共用 `ocr_candidate_line_pass`/`ocr_candidate_lines_pass_first`，candidate_index 同序；编辑器 strict、运行期 click/wait 保持 fuzzy。冒烟：`预设队伍更换_通用战斗准备` frame6 count=51（index0=队伍1 pass=true），`预设分组识别_式神录` count=8 且 test_action index 0/1 与候选一致。
> 验证：workspace **984/0**（adapter 8/0 含 Windows 假 exe 挂起超时单测、runtime 232/0、oasd 460/0）、UI **2428 checks / 190 ids / 0 dead / 158 runner / 6 nav**、cargo check --features ocr exit 0。
> 遗留：集合内 OCR 成员候选仍只列 pass 行（保 collection match 语义）；global 模式 ocr_var 期望回退 runtime_text；残余非热路径无界等待（package_cmd git、schedule tasklist/taskkill）未改；常驻弹窗识别线程待排期（§E-101）。

## 1.28 2026-11-07 12:xx→13:xx（seq 134）素材识别统一（strict/全帧候选/运行期回退/链与集合/多边形显示）（✅ 已完成并推送 dev；main 待快进）

> 用户 5 项裁决（Lead 规格 `_verify/seq134-audit/final-spec.md`）：① OCR 期望=严格匹配；② 运行期补 ROI→全屏回退；③ OCR 候选全帧；④ 无期望文字限 ROI、region 不可作链节点；另多边形显示。目标=图像/文字/集合/递归引用共用一套识别与候选，禁止私有实现。
> 实现 `4815cc4`（统一）+ `6a416ec`（删旧分支，行为不变）：**runtime** 单源 `recognition_candidates/recognize_best/pick_candidate`（domain/pass 统一、OCR 行缓存、链 `candidate_count/index`、collection 全成员、Click/Appear/dismiss/interrupts/click_target/long_press/drag ocr_text/preview/test_action/region.asset 单源）；**oas-task** `expand_collection_steps` 按 kind+RegionRef、load_flow/lint/publish 展开后校验、collection 作 op_region terminal；**serve** `editor_interp` 单解释器/候选统一/test_action 同 index/collection match 混合 best；**UI** 单一候选渲染器/`screen_fallback` 显示/删死函数 + 多边形缩略图裁剪/画布主显示。
> 验证：workspace **998/0**（task 165 / runtime 240 / oasd 462）、UI **2456 checks/190 ids/0 dead/158 runner/6 nav**、`cargo check --features ocr` exit 0；冒烟 `_verify\seq134-serve-smoke\`（test#6 count=3、严格 OCR 1 行+screen_fallback、混合集合 6 候选、test_action index 一致）、`_verify\seq134-runtime-smoke\`（strict miss 400）、`_verify\seq134-final\workspace*.log`。
> 遗留：PNG 文件本身仍矩形 ROI 裁剪（缩略图/画布按 polygon 遮罩，文件级 alpha 待 serve 支持）；runtime 保留 `matching_ocr_lines` 等仅供历史语义测试；真实点击/Firefox 手测未做；main 待快进、未 tag/release。

## 1.29 2026-11-07 18:2x→19:3x（seq 136）操作区 polygon/fixed + 11 项 + 唯一全局地图/导航 + 全局弹窗 D0（✅ 已完成并推送 dev；main 待 docs 后快进）

> 用户 11 项反馈 + 地图/弹窗补充全部实现：提交 `8dba49a` A（#6 竖排 OCR ROI+全帧合并去重 / #7 T3 全部已声明变量 / #4 删任务队列独立 `removed_from_queues` / #3 去冗余素材 picker / #9 SVG 自适应 / #10 同层拖动+step id）、`4cc5611` B（#1 `op_region fixed` 绝对 / #2 ClickShape 统一 polygon 不退化）、`62a946a` C+D0（#5 识别等待共用条件行 / #8 可编辑源码框 / 全局 `popups.json`+PopupGuard）、`4c9ea8f` M1（地图核心模型/runtime/navigate op）、`9cfc04e` M2（/map API/T11/publish/install）、`f70eca4` M3（Studio 地图页/builder navigate/下线关系 UI）、`ccd99de` M4（重规划/弹窗中断/稳定/预算补测+CLI popups+E2E）。
> 地图现行口径：唯一 `<assets-dir>/map.json`（不入包、不按任务分）；节点=页面（parent/return_asset/priority/frames/识别特征 ocr_text|asset+min_hits）；边有向不可逆；返回边由 parent+return_asset 派生；三层寻路（直达→LCA→hub）；navigate 状态机（PopupGuard/重识别校验/不一致重规划/rounds3/hops16/PlanOnly/预算取消）；flow `Step::Navigate` + flow_ops 15 + manifest `map_requirements`；`/map*` Studio-only、T11、素材引用保护；关系 UI 下线、`/relations/*` deprecated。
> 验证：workspace **1039/0**（flow 83 / runtime 264 / task 167+ / oasd 471 等）、UI **2652/195/0 dead/158/6 nav**、cargo check/build ocr exit0；E2E `_verify\seq136-map\E2E-REPORT.md` **18/0**（real 导航 SKIP 无 adb）；D0/B/D2/probe 冒烟见 `_verify\seq136*`；`_verify\seq136-final\workspace*.log`。
> 遗留：真实设备导航/replay 多跳 E2E；map_requirements 仅根 flow；CLI popups 分层耦合；T11 不含 map 资产引用；map 损坏时引用保护失效；relations deprecated 待清理；边编辑无内联改字段；`/map/navigate real` 同步阻塞；地图文件需分发；用户数据未提交。

## 1.30 2026-11-07 21:07→21:4x（seq 137）九条反馈 → 三路审计 → 修复（地图性能/UI、去 map 门禁、画布 shape、拖拽/大图/Rust 投影、T8）（✅ 已完成并推送 dev；main 待 docs 后快进）

> 用户 21:07 九条验收反馈；三路只读审计 `_verify/seq137-audit/{map,builder,task-drift}-audit.md` 确认是**接错线/状态漂移/测试假绿**：地图页 `loadAssets` 调 `/library`（0 项，全局素材在 `/task/assets` 96+5）；`/map/probe` 78.3s、OCR asset identify 2.8–3.0s、PopupGuard 不早退、wait 150ms 全帧 OCR；`map_requirements` 只收根 flow + publish/T11 硬门禁；画布不清操作区、UI 二次平移/点测矩形；列表拖拽未实现（SVG 假拖拽+正则假绿）；源码=flow JSON 无 Rust；#7=T8 陈旧变量（sync 展开 call_task vs T8 未展开）。
> 修复 `45a458b`：① 地图性能 `ocr_roi_cache`（frame_seq/ROI/mask）+ `asset_feature_passes` cheap 判定+每帧缓存+identify/PopupGuard min_hits 早退+wait 指数退避+`/map/probe` 默认 1 次 OCR（probe 78.3s→1.453s debug、OCR asset identify→1.093s）；② 去 map 门禁（删 T11、publish 不读 map、install 不 warning、map_requirements/map_nodes deprecated 留空、run 只查本 flow 字面量 target、runtime loud 保留）；③ 地图 UI（/task/assets、assetSelect、父节点全列表、属性/OCR 换行、canvas DPR、预览 /map/state、OCR 列表、map.js/map_ui.js 同步）；④ 画布/点测（clearCanvasSelection 统一清、删二次平移、消费服务端 action_shape）；⑤ 列表真实同级 DnD + 全窗口流程图大页 + 源码折叠 + flow→Rust 只读投影（非可执行/无 Rust→flow）；⑥ #7 sync 改 raw 父 flow、T8 同口径（个人突破 removed 2 vars→lint 0）。
> 验证：workspace **1043/0**（runtime 267/oasd 472/task 172/flow 83）、UI **2713/195/0 dead/158/6**、cargo check/build ocr；E2E **19/0**（`_verify/seq136-map/e2e-results.json`：probe 缩面/lint 无 map gate/publish 无 map/install 无 requirement；real 导航 SKIP）；性能 `_verify/seq137-perf/perf-result.json`、T8 冒烟 `_verify/seq137-perf-root/personal-breakthrough-result.json`、审计 `_verify/seq137-audit/`。
> 遗留：target_expr；Rust 投影只读/无 Rust→flow；map_requirements deprecated；probe 素材 hints 需 node/assets；真机/Firefox；text_visible/verify_expect fuzzy 无缓存；弹窗 UI；地图边内联；canvas 缩放；map.js 双文件；UI pure test 无浏览器 DnD；用户数据未提交。

## 1.31 2026-11-07 22:28→23:30（seq 138）地图 V2（所在地/路标/路径）+ 工作台地图 tab（✅ 已实现：V2-B `8394af7` + V2-C `8134376`；dev=origin/dev，main 待 docs 后快进）

> 用户 22:28 反馈 priority/frames/hub 语义不清、工作台帧与地图帧两套状态、维护流程不清爽 → 22:53 裁决：**路标=相邻所在地引用（名称+路径）**、删 priority/frames/hub、失败兜底 `fallback_task`（默认返回庭院）、独立地图页彻底删并并入工作台最后 tab；23:03 补充：v1 map 若是用户数据**只迁代码不处理数据**、素材/帧都要可搜索+候选+预览、Rust 源码想要 rustc 原生（本轮不做/优先级调低，源码框保持 flow JSON+Rust 只读投影）、target_expr 等地图基础后。设计 `docs/MAP-V2-DESIGN.md`。
> 实现 `8394af7` V2-B（后端）：oas-task map v2 schema2（locations 名称 key、landmark{name,path}、path click+素材/集合+wait、fallback_task；上限 500/2000/6；v1 内存迁移仅代码+测试）+ runtime 所在地识别（seq134 单源+seq137 缓存/早退，min_hits，无 priority）+ 路标有向图 BFS（无 parent/LCA/hub）+ navigate V2（逐路标 click→settle→popup→重识别校验→不符重规划；PlanOnly/Execute/预算/取消；fallback_task 注入/失败一次+重试/缺失 loud）+ serve `/map` 全 v2（GET/POST/validate/state location/node 兼容/plan/navigate/probe、v1 body 内存迁移、素材引用保护）+ run v2 lazy/literal target/sibling fallback。`8134376` V2-C（工作台地图 tab）：独立地图页/导航删（Studio nav 6→5）、rail 最后 tab `#panel-map` 全宽+关闭回素材、共享工作台当前帧（`oas:frame-changed`/上一帧/下一帧/filterFramesByQuery）、所在地/路标编辑（名称/别名/判定条件/回程+去程路标、素材/集合搜索选择器+缩略图+settle、内联改删）、纯函数 `window.OAS_MAP`、probe/state/plan/navigate dry+real/fallback/save .bak/dirty/校验、builder navigate 目标改 V2 locations、map.js=map_ui 字节同步。
> 验证：workspace **1044/0**（runtime 268/oasd 472/oas-task 172/flow 83；`_verify/seq138-final/workspace-v2b.log`）、UI **2717/195/0 dead/158/6（Studio nav 5）**、cargo check/build ocr；V2-B 冒烟 `_verify/seq138-v2b-smoke`（GET /map schema2/4 locations/fallback、plan 2 hops、navigate dry、state、v1 validate 200、probe node、素材 delete 400）；证据 `_verify/seq138-v2b/REPORT.md`、`_verify/acceptance-20261106/seq138-v2c-progress.md`、`docs/MAP-V2-DESIGN.md §9`。
> 遗留：target_expr（T1）；Rust 编译器（rustc 原生）本轮不做/优先级调低（无 Rust→flow）；真机导航/replay E2E、Firefox 手测；全局弹窗 UI；地图边内联改/canvas 缩放/map.js 双文件/map_requirements deprecated；text_visible/verify_expect fuzzy；无浏览器自动化；serve navigate real 无 fallback sibling、fallback 保存不校验；v1 用户数据不迁移；用户数据未提交、未 tag/release。

## 2. 本批交付（seq 110）

| # | 交付 | 结果 / 证据 |
|---|---|---|
| 1 | **Studio-only 发行包** | `scripts/package.ps1` 由旧「全量包」重写为 Studio-only（旧对照 D1-D19：9 项不符全消 + D7/D8 补齐）：Studio UI 白名单 8 文件 + 真实 tasks/assets + 空 frames/runs/logs + 4 启停脚本 + config/models/adb + `surface: studio` BUILD-INFO + stage 审计 + `/` 分隔 zip。证据：`D:\OAS-dev\_verify\release-both-20261105\DESIGN-VS-NOW.md`、`package-files.json` |
| 2 | **Runner 包重建 + F1 修复** | `package-runner.ps1` 新增 `Write-Utf8NoBom`（`[System.IO.File]::WriteAllText` + `UTF8Encoding($false)`）覆盖 `scheduler.json`/`VERSION`/`BUILD-INFO`/`tasks\README.txt` + stage 首字节守卫；`package.ps1` 同类修复。证据：`F1-FIX.md`（修复前 `scheduler_mode=invalid_config` / `scheduler --status` exit 2；修复后 exit 0、`scheduler_mode=stop`、无 `invalid JSON`） |
| 3 | **分离度 E2E 75/75** | 两 zip 解压到仓外、CWD=解压目录、`--no-open`：Studio 8788 单进程、8789/8787 不监听、`/app.js` `/task/library` 200、`/runner.js` `/runner/tasks` `/runner/health` 404；Runner 8789 单进程、`/runner.js` `/runner/tasks` `/runner/health` 200、`/task/library` `/app.js` 404；两包 entry 互斥、0 反斜杠、自包含无仓库路径。证据：`e2e-summary.json`、`REPORT.md`、2 张截图 |
| 4 | **dev-serve 双入口化** | `scripts/dev-serve.ps1` 默认 `studio` 8788；`-Surface runner` → 8789（`-NoScheduler` 可选）；保留 `-Release/-SkipBuild/-NoOpen/-DryRun` + OCR 模型；8787 退役；`dev-serve.bat` 注释/传参说明更新；START-HERE 旧 8787 描述已同步 |
| 5 | **参数 UI 审计（待裁定）** | `_verify/ui-params-audit-20261105/PARAMS-UI-AUDIT.md`（只读，398 行）：**「设置变量」≠ 任务参数**；meta.params 唯一入口 = 任务库「配置项…」JSON 对话框；任务构建页 0 入口/0 提示；`save_flow` 不 lint、Studio 无 lint 路由 ⇒ 发布 400 T3 才首次反馈；**推荐 A+C**（构建页入口/参数面板 + Studio-only `GET /task/lint`） |

## 3. 下一步交接指引（可直接执行）

```bat
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
