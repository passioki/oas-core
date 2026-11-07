# 验收反馈整改（2026-11-06，seq124 后）：理解 / 计划 / 实施结果

> 状态：**已整改完成（seq126，2026-11-06 19:2x）**。用户 13 条反馈按批次 A/B/C/D/B3a/B3b/D2/E/F 落地；本文 §1~§5 保留当时的理解/核实/计划原文，§1 表新增「整改状态」列，实施结果/最终验证见文末「## 实施结果（Lead 验收）」。
> 输入：用户 13 条验收反馈 + 3 路只读排查报告（仓外）：
> `_verify\acceptance-20261106\01-bug-triage.md`（#3/#5/#6/#9/#11）、`02-refactor-design-inputs.md`（#1/#2/#4/#10）、`03-infra-ops.md`（#7/#8/#12）。
> 初始基线（整改前）：HEAD `d9c3dcc`；workspace 904/0；UI 2512 checks/194 ids/0 dead/6 nav；E2E 27/26/0/1。

## 1. 逐条理解与整改方向

| # | 用户反馈 | 核实结论（报告证据） | 整改方向 | 量 | 批次 | 整改状态 |
|---|---|---|---|---|---|---|
| 1 | 动作属性页仍有「文字（参数）」下拉 + 「插入参数」按钮 | 属实：locator 文字来源 param 选项 `builder.js:2714-2729`；插入参数组件 `builder.js:2594-2683` + 4 处挂载 | 删除 param 选项与「参数」措辞；旧 flow 的 `text_expr:param()` 以只读「旧参数来源」回显；「插入参数」改为「插入识别文字配置项（素材）」仅列 ocr_var 派生行（写同样的 `param(key)`）；条件编辑器「文字（参数）」行改为选 ocr_var 素材（生成 `text_visible(param(key))`）；goal battles 的 `param('max_battles')` 改字面量 loop | S+M | D | ✅ D（07ed361） |
| 2 | 构建页/任务库仍有配置项参数 | 属实：`#builder-params` `index.html:341-356`+`builder.js:1153-1249`；任务库配置项对话框 `app.js:802-806,2029-2152`；T3 去声明 `builder.js:1284-1293,1446-1453`；Runner 通用参数组+高级 JSON `runner.html:524-543,190-200,459-461` | 删除用户侧参数入口（面板/配置项对话框/去声明/通用参数组/高级 JSON 参数）；服务端 `meta.params` 派生模型与 `/task/config` 保留（内部/诊断）；Runner 只保留「使用端OCR文字配置项」分节 | M | D | ✅ D（07ed361）+ D2（c3739f6） |
| 3 | 多候选只对参数 ocr 生效；引用非参数 OCR 素材时 pick 只有 1 候选 | 部分属实：模板/集合/ocr_var/派生 ocr_line 链节点可多候选 ✅；**普通 ocr_line/ocr_lines 作链节点运行时报错**（校验却放行，`oas-task:63-69` vs `runtime:3145-3148`）；**根目标运行时只取最佳**（只有编辑器 test_action 可传 candidate_index）；候选 API 与 test_action/链枚举口径三处分裂（`candidates_for_asset` 未接线） | ① 统一候选枚举（`candidates_for_asset` 接到 serve 候选 API + `preview_root_candidate`，单行 OCR 编辑器路径改 det+rec）；② 普通 OCR 素材作链节点：允许保存/读取「期望文字」（否则不允许引用并明确提示）；③ pick=score/nearest 对模板多实例、同文多处 OCR 真正生效；④ #11 修复后「队伍2/3」不再是候选（同文只有一处时 1 候选是正确的，需用户确认语义） | M | C | ✅ C 并入 B（4ccd182）；strict 候选 A（c16a536） |
| 4 | 操作区去除「相对偏移」概念，它=修改当前素材操作区；引用素材只作识别链节点、不得改其设置 | 属实且现状与用户要求冲突：现状=模型 B（链末端被引用素材的操作区决定落点，`runtime:3036-3108`；UI 保存时 PATCH 被引用素材 `app.js:5701-5715`） | **改为模型 A**：链仅作识别门槛（逐级识别，全中才动作），**最终操作区=当前素材自己的操作区**；删除 asset 级 offset（schema/校验/runtime/UI/测试）；操作区对话框：当前素材可选「当前素材操作区（用统一框选编辑）」/「引用素材链（只读查看被引用素材已有链，不可在此修改）」；保存只写当前素材 | M | B | ✅ B（4ccd182）+ B3a（9a3c77f）+ B3b（303a7ac）；**seq127 修正（fd3339f）** |
| 5 | 参数类素材不支持阈值 | 后端/运行时/候选全链路已支持；**仅 Studio UI 阻断**：`app.js:assetThresholdKind` 只认 template/ocr_line/ocr_lines（`app.js:6699-6704`），菜单却显示可用 | UI 放开 `ocr_var`→OCR 置信度阈值；补 ui_pure_test 断言。无需改 Rust | S | A | ✅ A（c16a536） |
| 6 | 多边形保存/展示回退矩形（素材行预览、点选后画布） | 部分属实：保存链路基本保留多边形；**显示层恒矩形**（缩略图按 roi 裁矩形 `asset_ui.js:21-80`；`pickAssetRoi` 清空本地 polygon 不恢复 `app.js:6999-7017`）；`/task/asset/update` **无 polygon 字段**（OCR/非模板移动识别区时已存 polygon 不平移）；模板缩放 ROI 会显式回退矩形并弹窗（`app.js:6518-6583`，属设计限制） | ① 缩略图叠加 polygon 轮廓；② 选中/编辑素材时恢复并绘制已存 polygon；③ `/task/asset/update` 增加 `polygon`（支持清空/平移/校验），OCR/region 识别区移动时同步平移 polygon；④ 可选：模板缩放保持多边形 | S/M | A/B | ✅ A（c16a536）+ B（4ccd182）+ B3b（303a7ac） |
| 7 | Runner 端添加启动脚本 | 发行包里有、仓库根没有；dev 入口 `scripts\dev-serve.bat -Surface runner` 可用；packaging/dist-stage 的 start-runner.bat 停在 seq118 且双击失败无提示 | 新增 oas-core `start-runner.bat`（委托 dev-serve，固定 8789，自检 exe/模型/端口，失败 pause）+ oas-runner 根 `start-runner.bat` + 增强 packaging 两个 bat | S | E | ✅ E core（c16a536）；oas-runner 仓脚本待办 |
| 8 | GitHub About 大量「?」 | 存储值就是字面 `?`（非渲染问题）且不可逆；`updated_at`=2026-11-06T05:39Z；topics 正常、homepage=null | 用 UTF-8 无 BOM JSON + `gh api PATCH --input`（或 curl --data-binary）重写 description；建议同时设 homepage；**需用户授权**（安装 gh 或提供 repo 管理 PAT） | S | E | ✅ E（仓外 API 修复并读回验证；无 commit） |
| 9 | 改识别文字后操作区被重置；Runner 改是否同样 | 属实（明确条件）：ocr_var **无 roi/全屏** 时走 `/task/save_asset replace:true`，payload 不带 `op_region/action_shape/threshold`，replace 整项覆盖→操作区链清空；有 roi 走 update 不丢。**Runner 端不会**（只写 values.json，不碰 assets.json）。同类 bug：模板识别区覆盖保存也不带 op_region | ① UI replace payload 携带原 op_region/action_shape/threshold；② 服务端 replace 对 payload 缺省字段从磁盘现有 item 继承（显式 null=清空）——兜底任意调用方；③ 补 Rust+UI 测试（ocr_var 全屏 replace 后 op_region 不变） | S/M | A | ✅ A（c16a536） |
| 10 | 识别区/操作区不能用多边形框选；建议统一框选（拖动=矩形，点击=多边形多点+双击收尾；只留 3 按钮：识别区域/操作区域/反选；素材行接入） | 可行；识别区多边形后端已是**真实掩膜**（模板 masked NCC、OCR `mask_frame_for_ocr`），不是仅采样限制；缺口=UI 不载入/不显示已存 polygon、`/task/asset/update` 无 polygon；**反选仅对操作区域可行**（识别区掩膜补集无业务语义） | 统一画布工具状态机：`canvasTool{none,recognition,action}` + 手势区分 rect/polygon；3 按钮；素材行「识别区/操作区」接入同一入口；保存：识别区矩形→roi，多边形→roi=bbox+polygon；操作区 action_shape rect/polygon+invert；后端 update 支持 polygon；ocr_var 放开 polygon 并给 `ocr_read_var` 补掩膜 | M | B | ✅ B（4ccd182）+ B3b（303a7ac） |
| 11 | 全屏识别「队伍1」把队伍2/3 也当候选 | 属实：`ocr_text_matches` 对长度≥2 且长度差≤1 允许 **1 个编辑距离**（`runtime:969-999`）→队伍1/2/3 互相匹配；候选、点击、等待共用；阈值救不了（同屏同字体分都高） | ① 加**数字保护**：归一化后数字串不同→不匹配（队伍1≠队伍2；召喚=召唤仍保留）；② 候选枚举走严格模式（normalized 相等，不做编辑距离），运行 click/wait 保留数字保护后的 fuzzy 或可开关；③ 单测覆盖；④ 修复后 #3 的「假多候选」一并消失 | S/M | A | ✅ A（c16a536） |
| 12 | oas-core/runner 文件夹十几 GB | `D:\OAS-dev` 合计 24.6GB：`_verify` 10.4GB（历轮 E2E 各复制一份 target）> oas-core 9.66GB（target 9.27：incremental 4.36+deps 3.48）> oas-runner 2.60GB（target 2.51）> backend 1.31 > frontend 0.46；git 仅 63MB | 可回收 ~15-18GB：停 8788 服务后 `cargo clean`（core/runner）、删 dist/stage(-0.22GB)、_verify 只留最近 1-2 轮（其余归档/删）、可选 flutter clean。**需用户批准**（本机当前有 oasd PID 占 8788） | S | E | ◐ E 部分完成（_verify 已清 9.68GB；cargo clean/dist 待最终收尾） |
| 13 | 配置交互不应只有 OCR 变量，还应支持其他变量传递；编辑端创建、任务应用、发布后使用端个性化修改；后期多账号/多配置/多模拟器 | 现状：ocr_var + meta.params 派生（C1′）；无通用标量变量、无 profile；Runner 只有按任务 values | 设计「变量系统 v2」：变量类型（ocr_text/string/int/bool/enum）+ 默认值/可被使用端改/分组/描述；编辑端创建并应用到 flow；发布 manifest 带变量 schema；Runner 安装后「使用端配置」只显示可改变量；values.json 按变量存值；**Profile**（账号/模拟器/配置预设）= 命名值集合，调度条目/运行指定 profile；与 Phase 2（彻底去 meta.params）合并设计 | 设计 L / 实现 L | F | ✅ F 设计（VARIABLES-V2-DESIGN.md；V2.0 实现未排期） |

## 2. 共性问题（作为整改原则）

1. **操作区模型 A**：链=识别门槛；落点=当前素材自身操作区；被引用素材配置只读、绝不在他人对话框里改。
2. **统一框选工具**：一个画布工具按手势出矩形/多边形；识别区/操作区/反选三按钮；素材行入口复用。
3. **候选口径单一化**：一个公共枚举函数服务「素材行识别 / 操作区 pick / 编辑端 test_action / 运行期」；根目标运行时保持最佳（编辑器可选候选）。
4. **replace 保存必须继承缺省字段**：服务端兜底，避免再次静默丢 op_region/action_shape/threshold/polygon。
5. **OCR 文本匹配默认严格**：数字保护 + 候选严格；fuzzy 仅作显式可选兜底。
6. **参数概念从 Studio 用户界面彻底移除**：保留内部派生模型直到变量系统 v2；旧 flow 可读可跑、只读回显。

## 3. 批次计划（待确认后执行）

| 批次 | 内容 | 依赖 | 预估 |
|---|---|---|---|
| **A（先做，低风险高收益）** | #5 ocr_var 阈值、#9 replace 丢操作区（UI+服务端继承+测试）、#11 数字保护+候选严格、#6 显示层（缩略图/选中恢复 polygon） | 无需新设计；#9/#11 为数据正确性问题 | 1 轮 |
| **B（操作区/框选重构）** | #4 模型 A + 删除 offset；#10 统一框选工具；#6 保存层（update polygon/平移）；素材行入口接入；后端 ocr_var polygon+掩膜 | 需确认 B-1/B-2/C-1..C-4 | 1~2 轮 |
| **C（候选口径统一）** | #3 `candidates_for_asset` 接线（serve 候选 API + test_action 根候选）、普通 OCR 链节点支持（期望文字）、同文多候选/模板多实例验证 | 需确认 #3 语义 | 1 轮 |
| **D（参数 UI 清理）** | #1 + #2 全部用户侧参数入口删除；条件编辑器改「识别文字配置项」；goal battles 字面量；Runner 通用参数区隐藏；T3 文案/去声明删除 | 需确认 A-2/A-3 | 1 轮 |
| **E（运维）** | #7 启动脚本；#8 GitHub About（需授权）；#12 清空间（需批准，先停 8788 服务） | 需授权/批准 | 0.5 轮 |
| **F（变量系统 v2）** | #13 + Phase 2 设计文档（变量类型/profile/多实例）；实现另排 | 需确认方向 | 设计 1 轮 |

验证口径：每批 `cargo test --workspace`（≥904/0 只增）、`node apps\oasd\ui\ui_pure_test.js`（≥2512 checks/0 dead/6 nav）、E2E 27 断言扩容；UI 改动须用 **Firefox 手测**（用户默认浏览器）+ 关键路径截图；用户数据（assets/frames/tasks/scheduler.json）不提交、不在未经确认时批量改写。

## 4. 剩余可选项回应（用户已答复）

| 我上轮项 | 用户答复 | 计划 |
|---|---|---|
| 1 素材行「长按测试」入口 | 可以做 | 放入批次 D（UI 小改，复用 test_action long_press + hold_ms） |
| 2 配置项派生行只读卡片 | 好的 | **被 #2 覆盖**：参数 UI 整体移除后此面板不存在；若保留诊断入口再实现只读卡片 |
| 3 识别文字 ROI 接画布 | 用户质疑：现有「识别结果→存为OCR文字配置」已带画布绘制，识别区可通过素材行设置更改；问是 bug 还是出入；新建参数素材是否应独立 | **不是 bug，是入口分散**：对话框内 ROI 只是数字输入；已有两条等价路径（候选保存带框 / 素材行识别区）。计划：① 在素材库工具栏加**独立「新建使用端OCR文字配置项」**入口（用户预期正确，参数=素材应可独立新建）；② 对话框 ROI 增加「去画识别区」跳转统一框选工具（随批次 B）；保留从识别结果保存作为便捷路径 |
| 4 全屏 OCR 性能 | 暂缓 | 记入 backlog，等 #13 变量系统决定是否长期使用全屏 |
| 5 真机 E2E | 暂缓 | 记入 backlog |
| 6 拖测接入 test_action | 可做；澄清：只是把多候选筛选结果中的单候选传给拖测，不要复杂化 | 批次 C 小项：候选枚举→选中一个→拖测按该候选真实识别区/操作区执行（dry/real），不扩展链语义 |
| 7/8 候选 op_rect、更新检测鉴权/GC | 需结合本次验收再评估 | 整改后再评估 |
| 9 Phase 2 | 好的 | 与批次 F（#13 变量系统 v2）合并设计 |
| 10 发布/tag | 先不发布 | 不 tag/不 release；文档保持「只推送不发布」 |
| 11 浏览器 DOM smoke | 用户用 Firefox 作默认浏览器，不懂步骤 | **不是用户操作**：这是内部回归自检（本轮 index.html 坏注释导致运行时崩溃而 UI 测试没抓到）。计划改为：UI 改动用 Firefox 手测 + 内部加"真实浏览器加载无 console error"检查（开发侧，不打扰用户） |

## 5. 需用户确认（开工前）

1. **#4 模型 A**：确认「链只作识别门槛、落点=当前素材操作区、被引用素材设置只读不得改」；以及 OCR/ocr_var 的操作区是否应**跟随识别文字位置**（推荐：是；实现为相对真实识别区平移，而不是绝对坐标）。
2. **#4 链递归**：被引用素材自己已配置的链是否继续作为后续门槛（只读跟随），还是链扁平化为单层？region 素材是否仍可作门槛（A 下它不贡献操作框）？
3. **#3 语义**：修复 #11 后，「队伍1」素材的正确候选只有真正匹配「队伍1」的行（通常 1 个；同文多处才多个）——这是否符合你的预期？还是你希望链节点把**识别区内的所有文字行**都作为候选（与 #11 矛盾）？另外普通 OCR 素材作链节点需给它设置「期望文字」，是否接受？
4. **#10**：反选只用于操作区域（识别区不支持，语义上=掩膜补集，无业务场景）；模板编辑多边形是否要求**不重新截图**（需给 update 加 polygon 路径）；ocr_var 是否允许多边形识别区（后端需补掩膜）。
5. **#1/#2**：条件编辑器里的参数引用改为「识别文字配置项（素材）」是否可行？「完成条件」的循环次数改字面量是否 OK？Runner 的通用参数区/高级参数 JSON 入口是否直接删除（而非隐藏）？
6. **#8**：需要你授权其一：① 安装 `gh` CLI 并用你已登录的 git 凭据 PATCH About；② 提供一个有 repo 管理权限的 GitHub PAT（仅这一次使用）。另请确认新的中文简介文案（草案见报告 §2.3）。
7. **#12**：是否批准清理 ~15-18GB（先停 8788 上的 oasd）：`cargo clean` core/runner target、删 dist/stage、`_verify` 仅保留最近 1-2 轮（其余可移仓外/删）。
8. **#13**：确认变量系统方向（变量类型/使用端可改/Profile 多账号多模拟器）与优先级——建议先出设计文档，评审后再实现。

## 6. 证据路径

- 排查报告：`_verify\acceptance-20261106\{01-bug-triage.md,02-refactor-design-inputs.md,03-infra-ops.md}`
- 当前基线：`_verify\seq124-lead-verify\workspace-final.log`、`_verify\seq124-e2e-20261106\e2e-summary.json`
- 用户数据快照（只读核对）：`assets/assets.json`（已有 ocr_var「队伍1」「预设识别」+ `mode:asset` 链；无 offset 数据）

## 实施结果（Lead 验收，2026-11-06 19:2x）

> 13 条反馈已按批次整改完成（#12 为部分完成：清理已执行，cargo clean/dist stage 最终收尾待做）；实现记录 `ENGINE §22.65`；操作区现行语义 `OPERATION-REGION-DESIGN §8.8`；变量系统设计 `VARIABLES-V2-DESIGN.md`。

### A. 批次与提交（dev=main=origin=`303a7ac`；**未 tag/未 release**）

| 批次 | 内容 | commit |
|---|---|---|
| A | #5 ocr_var 阈值 / #9 replace 继承（UI+服务端）/ #11 数字保护+候选 strict / #6 显示层 / #7 根启动脚本（core 侧） | `c16a536` |
| B（含 C） | #4 操作区模型 A + 删 offset；#6/#10 polygon 能力 + update v3；#3 普通 OCR 链节点 + 候选统一；#11/#3 strict 候选接线 | `4ccd182` |
| D | #1/#2 用户侧参数 UI 移除（builder/app/runner） | `07ed361` |
| D2 | goal battles 删自动补 `params.max_battles` | `c3739f6` |
| B3a | 操作区对话框模型 A + 删 offset 绘制 + 拖测候选/长按测试入口 | `9a3c77f` |
| B3b | 统一画布框选（3 按钮/手势矩形+多边形）+ polygon 保存层 | `303a7ac` |
| E | #7 启动脚本（core 侧随 A 提交）/ #8 GitHub About 修复 / #12 清理 | 仓外操作，无独立 commit |
| E2E | seq126 脚本 54 断言 | 仓外 `_verify/seq126-e2e-20261106/` |
| F | #13 变量系统 V2 设计 | `docs/VARIABLES-V2-DESIGN.md` |

### B. 最终验证（Lead 实测）

- `cargo test --workspace` = **929 passed / 0 failed**（23 suites；oas-task 147 / oas-runtime 212 / oasd 449；整改前 904/0）。
- UI pure test = **2217 checks / 177 ids / 0 dead / 158 runner ids / 6 nav**（本机复跑核对；整改前 2512/194/158/0/6，checks 下降为批次 D 删除参数 UI 的功能删除）。
- E2E seq126 = **54 total / 53 PASS / 0 FAIL / 1 SKIP**（唯一 skip=`test_action_real_click`，无 ADB）——`_verify/seq126-e2e-20261106/e2e-summary.json`（19:25:30 最终 run）+ `README.md` 54 条断言清单。
- #8 API 读回验证：description 已是中文（「OAS（OnmyojiAutoScript）— 阴阳师自动化引擎（Rust）…v0.1.0 开发中。」）、homepage=`https://github.com/passioki/oas-core/releases`、topics 5 个未变、`updated_at=2026-11-06T11:25:09Z`。
- #12 清理：`_verify` 10,403MB → **722.5MB**（释放 **9.68GB**）；`D:\OAS-dev` 24.6GB → **7.94GB**。

### C. 仍待办（不阻塞本轮，详见 `ENGINE §22.65.8`）

1. **Firefox 浏览器手测**：批次 D/B3a/B3b 的 UI 改动需真实浏览器加载无 console error + 关键路径截图（内部自检，非用户操作）。
2. **oas-runner 独立仓**：根 `start-runner.bat`、packaging 脚本增强、pin bump 到 `303a7ac` 与重打包；当前发行包/dist stage 仍停 seq118 `5b0b107`。
3. **变量系统 V2.0 实现**（#13/F 设计已交付、10 项按推荐口径；实现排期在批次 D 之后）。
4. **loop 模板 `max_rounds`**：`templates/loop.json:49`、`index.html:454` 仍用 `param('max_rounds')`；参数 UI 移除后该模板轮数无用户配置入口（旧 flow 仍可跑）→ 字面量化或待 V2.1 变量。
5. **T5 手写 polygon 校验**：`assets_shape_issues`（`oas-task/lib.rs:3405-3600`）不校验 `item.polygon`，仅保存路径 `validate_recognition_polygon`（`:1033`）校验。
6. **真机 E2E**：`test_action_real_click` 恒 SKIP（无 ADB）；真实点击/长按/滑动落点待验。
7. **#12 最终收尾**：cargo clean（core/runner）、dist stage/旧 zip 清理（需先停服务）。
8. 更新检测鉴权/GC、tag/Release 策略仍 backlog。

## seq127 追加验收（2026-11-06 20:0x）

> 用户 20:0x 复核 seq126 后追加 3 条反馈，Lead 用用户数据副本实测确认并提交 `fd3339f`（dev=main=origin；未 tag/未 release）。实现记录 `ENGINE §22.66` / `OPERATION-REGION-DESIGN §8.9`；**seq126 §B 的「模型 A 落点」与 ROI-only 候选口径被本节取代**（#4 行已标 seq127 修正）。

### A. 用户 3 条反馈与根因

| # | 反馈 | 根因 | 修复 |
|---|---|---|---|
| 1 | 模板/集合多候选未生效：全屏识别 `test`@frame#6 只回 1 个候选，应 3 个 | 候选枚举仍按素材 ROI（`match_template_all` 只在 ROI 内扫） | oas-perceive 新增全帧金字塔多候选 `match_template_all_full`/`match_template_masked_all_full`；runtime 候选 API/链 pick/test_action 根候选改全帧（普通 click best 仍 ROI+金字塔回退） |
| 2 | 操作区链末端语义：选「引用素材」时应落在**最后一个被引用素材自己的操作区** | seq126 #4 误改为模型 A（落点=根素材 self） | 恢复模型 B：`walk`/`resolve`/`preview` 返回链末端节点自身 self 几何；根 self/无链走历史路径；offset 删除/region 不可节点/防环保留 |
| 3 | 组合场景（链+多候选+pick score/nearest）结果错误 | 候选与末端几何两处各自错 | 链节点识别改全帧候选 → pick 选中候选后其自身几何即末端落点（同一 resolver，编辑器与引擎一致） |

### B. 修复结果（Lead 实测）

- 候选：`test`@frame#6 **count 1→3**，y=**144/295/445**，score=**1.0/0.973/0.905**（ROI 外候选也返回）。
- 链末端 `test_action dry`：`预设识别`（runtime_text=队伍3）→ `[973,445,35,38]`（=链末端 `test` 候选，非自身 `[573,453,64,29]`）；`队伍1`→`[973,295,35,38]`；`test`（无链）→`[973,144,35,38]`。
- 性能：全帧多候选 1280×720/35×38 **debug 114ms / release 6.4ms**（原 7.4s）。
- Studio 文案按 self/asset 动态显示落点（`opRegionLandingText`，`app.js:2685`；标题/提示/面包屑/保存消息同步）。
- 测试：perceive 14/0、runtime 214/0、workspace **934/0**、UI **2223 checks/177 ids/0 dead/6 nav**、`cargo check -p oasd --features ocr` exit 0。

### C. 跟进（不阻塞）

E2E seq127 脚本（并清理 seq126 模型 A 断言）、masked 极端 mask 兜底、候选 API `domain` 字段语义（仍=声明域而搜索域已全帧）、`test_action` OCR 耗时（全屏 ~343ms 级）；详见 `ENGINE §22.66.6`。

## V2.0 变量系统实施结果（2026-11-06 20:4x）

> 用户剩余项「变量系统 V2.0 实现」（#13/F 设计 → 实现）已完成并提交：`e1b816f`（V2.0a 后端模型层）+ `d780a4b`（V2.0b serve + V2.0c UI）。设计 `docs/VARIABLES-V2-DESIGN.md`（§10 = 实施结果与偏差）；实现记录 `ENGINE §22.67`。

### A. 完成范围（用户剩余项 → 交付）

- **模型**：`meta.variables`（`Variable`：key/type/value_source/asset_id/default/label/description/group/runner_editable/required/scope/min/max/enum）成为唯一存储；旧 `meta.params` 仅只读内存桥（`effective_variables()`，不写盘）+ T10 warning；`validate_variable_values`/`variable_defaults` 供注入复检。
- **sync/lint**：`sync_ocr_var_variables` 维护 `value_source=ocr_text` 变量（key=param_key/default=runtime_text/asset_id）；T8 改目标到 variables；T3/T7 改读 input variables；T10 警告经 `lint_warnings` 返回。
- **package**：`PACKAGE_SCHEMA=2`，`variables_schema{own,call_deps[].child_variables}`；旧 `package_schema=1` 安装**响亮拒绝**（re-publish）。
- **API/UI**：Studio `GET/POST /task/variables`（有效变量+T10；T3/T8 非阻断 issues；保留未知顶层字段；原子写+.bak；不写 params）；`/task/config` 保留 **deprecated 别名**（params 只读镜像，不落盘）；构建页「变量」面板/变量对话框/「插入变量 ▾」/动作目标 key 集；Runner 配置两分节只显示 `runner_editable && scope=input`（runtime/不可改 400），required 橙提示、空态、高级参数 JSON 保持隐藏。
- **迁移**：`oasd task lint`/create/publish/run 注入全部迁到 variables（四层优先级不变）；`values.json` 结构与 `runner_config.rs` **零改动**。

### B. 提交与最终验证（Lead 实测）

- commits：`e1b816f`（V2.0a）、`d780a4b`（V2.0b/c）；HEAD `d780a4b`（dev=`origin/dev`；main=`origin/main`=6ebffdd 待快进）；未 tag/未 release。
- `cargo test --workspace` **949/0**（23 suites；oas-task 157 / oas-runtime 214 / oasd 454；上一轮 934/0）；`cargo check -p oasd --features ocr` exit 0。
- UI **2303 checks / 177 ids / 0 dead / 158 runner ids / 6 nav**（本机实跑核对）。
- E2E seq128 `_verify/seq128-e2e-20261106/`：**77 total / 76 PASS / 0 FAIL / 1 SKIP**（skip=`test_action_real_click` 无 ADB）；覆盖变量 API 读写/5 类校验 400/拒绝后状态不变/T3 声明闭环/识别文字变量 sync/T8 自愈 + seq127 回归 + publish（manifest/包资产回归）；package_schema=2 快照/旧包拒绝与 Runner 可改过滤由 Rust 单测覆盖（`package.rs:2214,2273`、`serve.rs:19951`）。
- Lead 用户数据副本冒烟（真实数据未写）：GET `/task/variables` → effective variables + T10；POST 后 `meta.json` **无 params**；`/task/config` 别名带 `deprecated`。

### C. 仍待（不阻塞）

1. **V2.1**：标量变量在条件/loop/表达式选择器 UI（当前条件仍 ocr_var 素材下拉）、`variable()`/T9、loop `max_rounds` 数值入口。
2. **V2.2**：Profile 多账号/多配置/多模拟器（values.json v2、调度 entry.profile、`/runner/run` profile、多实例部署）。
3. 兼容收尾：`/task/config` deprecated 别名后续删除；`Variable` 行内未知字段不保留（限制）；M1 导入 CLI 未实现（M3 用户重建）。
4. 工程：Firefox 手测、oas-runner 独立仓 pin/重打包、main 快进、tag/Release 策略（未发布）。

## seq129 追加验收：动作库一致性（拖动素材端点）

> 用户 2026-11-06 21:26 反馈 3 条（逐字）：「动作库里的拖动（拟人）的UI并不成熟，没有选择素材的选项（请与点击素材动作的UI保持一致）。请关闭/删除拖动动作属性里坐标输入的代码，这个应该是通过建立操作区域素材来完成的。请排查其余动作库动作，尽量和点击素材动作的UI和代码逻辑保持一致。」实现 `dce16c7`（拖动 v2 + 动作库一致性）+ `82dbca3`（附带修复全局素材 T8 lint 假红）。记录 `ENGINE §22.68`。

### A. 动作库审计结论（逐动作）

| 动作 | 审计 | 处置 |
|---|---|---|
| `click`/`click_target`/`long_press` | 均已「选择素材」+ 操作区域，实现同源 | 文案/选择器统一（「素材（识别+操作区域）」） |
| `drag`（拖动·拟人） | from/to 是 region/point/ocr_text 坐标端点，无素材选择 → 用户反馈不成熟 | **重写**：新增 `asset` 素材端点（`{"kind":"asset","sel":"<id>"}`，from/to 可用），删除坐标输入 |
| `dismiss_popup` | 有弹窗素材 + 按钮文字 | `action` 标签澄清为「按钮文字（可选，素材库 popup 动作名）」 |
| `swipe`/`curve_swipe`/`click_point` | 坐标手势（低层） | 保留并加提示「坐标手势（低层）；元素拖动请用拖动（拟人）」 |

### B. 实施结果

- **后端**：`DragTarget::Asset { sel }`（`oas-flow:751-807`；解析/校验 `:2165-2187`，拒绝 shape/region 混用；asset 端点计入 `referenced_assets`/T4/publish `asset_refs`）。运行时 `drag_asset_endpoint` 复用点击素材同源（`locate`→根 self 几何 `click_box_for`/`ocr_self_operation_geometry`→`op_region` 链模型 B 末端盒），未命中=响亮 Err 不发送 swipe；采样/方向/24 次 to/duration/margin/audit 不变（`runtime:3612-3665,4391-4406,6244,6298-6325`）。
- **UI**：拖动面板重写为「拖动起点素材（必填）+ 拖动终点素材（可选）」素材搜索选择器（与点击素材一致），写 `from/to={kind:"asset",sel}`；清空终点=方向+距离；**删除坐标 ROI x/y/w/h、拖动端点编辑器/样式/断言**；旧坐标端点只读提示不改写；方向/距离/时长/margin 保留；saveFlow 校验 from 必填/asset 端点必须带 sel（`builder.js:3645-3805,3677-3700,4862`）。
- **package**：`flow_ops` 新 marker `drag.asset`（`SUPPORTED_FLOW_OPS` 13→14，`package.rs:90-105`；derive `:617-619`；旧 Runner 安装期拒绝 `:2624`）。
- **附带修复（冒烟发现）**：全局素材模式下 flow 引用全局 ocr_var（如 `预设识别`）时 T8 假红且 publish/install 阻断 → `lint_into_with_calls_and_global_assets`/`merge_global_assets_for_lint`（global base + 任务本地覆盖；T5 仍本地、T8 合并视图、T4 local∪global），`lint_one`/publish/install（install 另合并包内快照、包内优先）统一修复（`oas-task/lib.rs:3153-3176,3237-3240,3364,3370,3853-3876`；`task.rs:213-240`；`package_cmd.rs:753-827,1471`）。

### C. 验证 / 冒烟 / 遗留

- `cargo test --workspace` **968/0**（oas-task 160 / oas-flow 82 / oas-runtime 222 / oasd 458；上一轮 949/0）；UI **2323 checks/177 ids/0 dead/158 runner/6 nav**（本机实跑核对）；`cargo check -p oasd --features ocr` exit 0。
- 冒烟 `_verify/seq129-smoke/`：保存 drag(asset `lib:test`→`lib:预设识别`) flow **200** → lint **0**（`lint.json`；修复前 T8 假红）→ publish **200**（`dist/seq129-smoke.oaspkg`，package_schema=2、variables_schema 含全局 `预设识别`）→ 全新 Runner install **200**（`install-result.json`；包快照初始化全局 store 2 assets）→ 安装后 lint **0**。
- 遗留：旧 flow 拖动坐标/OCR 端点后端兼容运行但 Studio 只读（建议重建为素材端点）；swipe/curve_swipe/click_point 保留低层坐标手势（UI 已提示）；旧 `.oaspkg`（package_schema=1）需重发布；main 分支待快进、未 tag/未 release。

## seq130 追加验收（23:04 反馈 5 项）

> 用户 2026-11-06 23:04 反馈 5 项已修复并提交 `a3556e3`（HEAD；dev=`origin/dev`，main=`origin/main`=2c81386 待快进；未 tag/未 release）。实现记录 `ENGINE §22.69`。验证：workspace **972/0**（runtime 226 / flow 82 / task 160 / oasd 458）、UI **2370 checks/178 ids/0 dead/158 runner/6 nav**、`cargo check -p oasd --features ocr` exit 0；dry 预览证据 `_verify/seq130-click-repro/test_action-dry.json`。

| # | 用户反馈（23:04） | 根因 | 修复 | 验证 |
|---|---|---|---|---|
| 1 | 框选应可移动（已有框可拖动平移） | 画布已有框无命中/平移状态机 | `app.js` hit-test/平移/clamp/Esc 撤销（`:470-660,549,562,577,614,634`）；保留新建手势 | UI 断言 `:8426-8491` |
| 2 | 点击叠加分布显示不全/图例缺失 | 旧记录只有 actual 或 target，miss 未区分；缓存/面板未刷新 | `normalizeClickPoint`（`:346-368`）兼容 actual/target/miss + 热力/实点/目标环/miss 叉 + 缓存键/面板刷新/图例 | UI 断言 `:8352-8390` |
| 3 | 条件行把已声明变量误判为「旧参数」 | 只查 `meta.params`，未看 `meta.variables`/`ocr_var.param_key` | `declaredParamKeySet`/`paramKeyDeclaredIn`（`builder.js:246,262,278`）；已声明=变量来源，仅未声明才旧参数只读（`:3231-3240`） | UI 断言 `:5084-5110` |
| 4 | 工作台素材需自动刷新 | 无轮询/可见性暂停/去重 | 8s 轮询（`app.js:4830,4877,4900-4916`）+ 可见/聚焦/visibilitychange 暂停恢复 + 去重 + 保留滚动/选中 | UI 断言 `:8393-8423` |
| 5 | 真实点击点测/实际运行点击失效（OCR 落到 authors 旧操作区） | target rect 已 Q1 平移，但采样 shape 被 `translated_click_shape` 拉回作者绝对 `action_shape`（仅 template 平移） | 新 `click_geometry_for(sel,matched)`（`runtime:2777`，rect+shape 同源，OCR 用 `ocr_self_operation_geometry :3533`；roi=None 绝对；template/collection/region 不变）；统一 Click/click_target/long_press/interrupts/dismiss_popup/region.asset 调用点（`:2951,3890,4767,5071,5189,5391`）；serve preview 同源未改 | 单测 `:12062,12087`；dry 预览 `action_box=[1103,245,120,50]`、`real_rect=[1131,257,68,30]` |

- 遗留：① 全屏 `ocr_var`（`roi=None`）操作区仍绝对坐标（文档化限制）；② 旧 `shape_aware`/`translated_click_shape` 分支保留（未删）；③ 浏览器 Firefox 手测清单（框选移动/clamp/Esc、叠加图例、条件行列、8s 刷新暂停恢复、OCR 真实点测落点）；④ main 待快进、未 tag/未 release。

## seq131 追加验收（2026-11-07 08:10）：恢复「加入素材集合」入口

- 反馈：把素材添加为集合的 UI 没了；按钮可放到素材设置里。
- 原因：3eb9fa8 清理未使用 UI 时移除了「新建集合」（素材多选 + 按钮）；后端集合 API 仍在。
- 修复 eaf68fe：素材设置菜单「加入集合」（template 专用，置灰规则）+ #collection-add-dialog（已有集合/新建集合；跨集合自动移出旧集合；保留 threshold/action_shape；保存后刷新并提示改写任务数）；三处素材列表接线。
- 验证：UI **2391 checks / 190 ids / 0 dead / 158 runner / 6 nav**；node --check 全绿；数据副本 API 冒烟 POST create 200 / GET 回显集合 / DELETE 200。
## seq132 追加验收（2026-11-07 08:26）：集合成员扩展到所有素材

- 反馈：目前只有模板图能加入集合，请扩展到所有素材。
- 实现 7229c59：成员允许 template/ocr_line/ocr_lines/ocr_var/region（嵌套集合拒绝）；runtime/serve/UI 全链路支持（识别、click/click_target 展开、候选 API、集合 match、test_action、drag；region=固定区域兜底 1.0；获胜成员 op_region 链生效）。
- 验证：workspace **980/0**、UI **2407/190/0 dead/158 runner/6 nav**；Lead 冒烟（用户数据副本 frame6，无真实点击）：混合集合 candidates count=5（含 member/member_kind）、collection/match winner OCR、test_action dry index 0/1 正常。
- 遗留：click_target 成员不继承集合级 locator 覆盖；无期望文字普通 OCR 成员 load 期报错；混合真实点击（OCR/region）未真机验证；region 恒 1.0 压过其它分数。
## seq133 追加验收（2026-11-07 12:06）：点测/拖测卡死修复 + 文字素材多候选

- 反馈：#3「素材『预设队伍更换_通用战斗准备』点击测试（真实模式）…」卡住，点测/拖测都用不了；#2 模板多候选而文字素材没有多识别；#1 常驻弹窗识别线程设想（低优先，记档不实现）。
- #3 根因/修复：Adb::run 无超时 → 服务线程永久阻塞。9323ace 给所有 adb 调用加超时（15s/screencap 20s，超时杀进程树）、OCR 锁毒化兜底；UI 45s AbortController 超时恢复/in-flight/错误提示。冒烟假挂起 adb：test_action real 400/15.1s，服务恢复。
- #2 修复：OCR 候选=区域全部识别行（text/score/pass，pass 优先→score desc→y→x；期望行 index0），runtime preview 与候选 API 同序一致；运行期 click/wait 仍 fuzzy。冒烟 预设队伍更换_通用战斗准备 count=51/pass 置顶、预设分组识别_式神录 count=8 且 test_action index 0/1 一致。
- 验证：workspace **984/0**、UI **2428/190/0 dead/158 runner/6 nav**、cargo check --features ocr exit 0。
- 遗留：集合内 OCR 成员候选仍只列 pass 行；global 模式 ocr_var 期望回退 runtime_text；其余非热路径无界等待未改。

## seq134 追加验收（2026-11-07 素材识别统一：strict/全帧候选/运行期回退/链与集合/多边形显示）

> 用户 5 项裁决（Lead 规格 `_verify/seq134-audit/final-spec.md`）；实现 `4815cc4`（统一）+ `6a416ec`（删旧分支，行为不变）；HEAD `6a416ec`（dev=`origin/dev`，main=`origin/main`=7a89fbe 待快进；未 tag/未 release）。记录 `ENGINE §22.73`。

### A. 用户裁决（现行口径）

① OCR 期望文字=**严格匹配**（归一化完全相等）；② 运行期补 **ROI 未命中→全帧 strict 回退**；③ OCR 候选=**全帧**（ROI 命中仅标注/优先）；④ **无期望文字**普通 OCR 限 ROI、**region 不可作链节点**；另补充多边形显示。目标：图像/文字/集合/递归引用共用一套识别与候选，禁止私有实现。

### B. 实施结果

- **runtime 单源**：`recognition_candidates`/`recognize_best`/`pick_candidate`（`runtime:2659/2712/3522`）；统一 `AssetMatchCandidate`（member/member_kind/domain/pass=纯阈值，`:1349-1386`）；OCR 期望严格+全帧候选+ROI 命中标注（`:2616-2654`）；运行期 ROI strict→全屏 strict 回退（`:2792-2860`）；无期望 ROI-only（`:2637/2862`）；collection 全成员递归（`:2680-2690`）；链节点统一+candidate_count/index（`:3693-3770`）；`6a416ec` 删除旧私有分支。
- **oas-task**：`expand_collection_steps` 按 kind 生成成员 locator、region 用 `RegionRef`、保留 locator 覆盖；load_flow/lint/publish 使用**展开后** flow；collection 作 `op_region` terminal 与 runtime 一致（`oas-task/lib.rs:1570-1712,1986-1993,3386-3405,513-517,582`）。
- **serve**：`editor_interp` 单解释器（`:3102-3124`）；candidates 统一 `recognition_candidates`（`:4976-4998`；候选行 domain 与顶层 domain 语义区分）；test_action 同源同 index（`:5197-5198`）；collection match 混合 best（`:4502-4510`）。
- **UI**：候选单一渲染器（`app.js:1708,2510-2600`）、错误体 `{ok:false,errors}` 显示（`:4297-4311`）、`screen_fallback` 显示（`:4319-4349`）、死函数删除；多边形缩略图真实裁剪（`asset_ui.js:55-110`）、画布多边形主显示（`app.js:747-870`）。

### C. 验证 / 冒烟 / 遗留

- workspace **998/0**（oas-task 165 / oas-runtime 240 / oasd 462；上轮 984/0）；UI **2456 checks/190 ids/0 dead/158 runner/6 nav**（本机复跑核对）；`cargo check -p oasd --features ocr` exit 0。
- 冒烟（用户数据副本，无真实点击/ADB）：`_verify/seq134-serve-smoke/` 中 test@#6 **count=3**、严格 OCR **1 行**（非 seq133 的 51）且 ROI 未命中→`domain=screen_fallback`、混合集合 **6 候选**带 member/member_kind/text、test_action index 与候选一致；`_verify/seq134-runtime-smoke/` strict miss 响亮 400；`_verify/seq134-final/workspace*.log`。
- 旧文更正：§22.64.3 候选单源声明在 seq134 才真正落地；§22.72 #2 已被取代（#3 ADB 超时保留）；模型 A 旧口径已由 §22.66/`OPERATION-REGION-DESIGN §8.9` 更正；seq127 模板全帧候选仍有效。
- 遗留：① 保存的 PNG 文件本身仍矩形 ROI 裁剪（缩略图/画布按 polygon 遮罩显示）；② runtime 保留 `matching_ocr_lines` 等公开 helper 仅供历史语义测试；③ 真实点击/Firefox 手测未做；④ 未 tag/未 release、用户数据未提交。

## seq136 追加验收（2026-11-07：11 项 + 唯一全局地图/导航 + 全局弹窗 D0）

> 用户 11 项反馈 + 地图/弹窗补充全部落地。提交链 `8dba49a` A / `4cc5611` B / `62a946a` C+D0 / `4c9ea8f` M1 / `9cfc04e` M2 / `f70eca4` M3 / `ccd99de` M4；HEAD `dbaece2`（dev=`origin/dev`，main=`origin/main`=c0a672b 待 docs 后快进；未 tag/未 release）。记录 `ENGINE §22.75`、`OPERATION-REGION-DESIGN §8.10`、`_verify/seq136-map/DESIGN.md`。

### A. 11 项落地

| # | 反馈 | 结果 |
|---|---|---|
| 1 | 操作区「固定」绝对坐标（仅根/链终端） | `OpRegion::Fixed`/`AssetOpRegion::Fixed`（`oas-task/lib.rs:116-138`、`runtime:405-426,4569-4595`），不随 real-roi 平移，无 shape loud |
| 2 | 操作区统一 ClickShape；矩形=4 点多边形；不再退化 | `ClickShape`（`runtime:44-48`）；`resolve_candidate_operation`（`:3509-3541`）；serve `click_shape_json`；候选/test_action 输出 exact `action_shape`；UI rect→polygon 保存 |
| 3 | 删除动作属性冗余「素材（识别+操作区域）」 | 单素材 picker 推导 locator；legacy 冗余 sel 规范化（`builder.js:3949`，测试 `ui_pure_test:8990-9015`） |
| 4 | 删除任务不被 Runner 队列阻断 | `prune_scheduler_task` 清理 `queues[].members` 并返回 `removed_from_queues`（`serve:7326-7539`；测试 `:20020/:20110`） |
| 5 | 识别/等待动作属性共用 if 条件行控件 | `renderConditionEditor` 复用（`builder.js:4646`）；wait_until 比较/与或非/表达式兜底，picker 统一 `:4521-4536` |
| 6 | 竖排「个人」候选缺失（#16/#73） | ROI det+rec 与全帧合并去重（`runtime:974-992,3455+`；60% 小框重叠、ROI 优先）；count=1 domain=roi（`_verify/seq136-probe`/`seq136-smoke`） |
| 7 | T3 修复 runtime 变量误报 | T3 用全部已声明变量（`oas-task/lib.rs:3466-3495`，`declared_variable_keys`） |
| 8 | 构建页可编辑源码框 | `index.html:385`；`builder.js:5138-5271`（校验/错误行列/未知 op 拒绝/dirty/应用走 saveFlow）；UI 测试 `:9231-9251` |
| 9 | 流程图 SVG 自适应铺满 | `fitFlowGraphSize`（`builder.js:2207`）+ `ResizeObserver`（`app.js:9218-9225`）；测试 `:9018-9043` |
| 10 | 同层拖动排序 + step id 跟随 | `builder.js:1146/1161/1179/1211/1286`；测试 `:9062-9084` |
| 11 | 帧-素材联系→唯一全局地图+navigate | D1（下）|

### B. 地图 D1 / 弹窗 D0

- **地图**：唯一 `<assets-dir>/map.json`（`oas-task/src/map.rs`：schema `:134-165`、校验 `:325-490`、原子写+`.bak` `:216`、`to_runtime :243`；**不入包**）；节点=页面（parent/return_asset/priority/frames/识别特征 ocr_text|asset+min_hits）、边有向不可逆（click_asset|key|swipe+wait）、返回边由 parent+return_asset 派生（`runtime/map.rs build_graph:121`）；寻路三层直达→LCA→hub（`plan_route:309`，确定性 tie-break，不可达 loud）；`navigate` 状态机（`runtime:2730+`：PopupGuard/重识别校验/不一致重规划/rounds3/hops16/PlanOnly/预算取消）；flow `Step::Navigate`（`oas-flow:2833-2844`）+ `flow_ops` 15 项 + `manifest.map_requirements`（`package.rs:148-170`）；serve `/map*`（`serve:5306-5700`，Studio-only）、T11（`task.rs:213-300`）、publish 同口径/install warning、素材 delete/rename 引用拦截（400）。
- **弹窗**：全局唯一 `<assets>/popups.json`（与地图/任务分离；`runtime PopupConfig/PopupRule/PopupFeature :495-540`），`PopupGuard` 钩子（步骤前/后、点击前/后、wait 周期；priority/cooldown/max_attempts/close 后重识别，`runtime:2449-2538`）；API GET/POST `/popups`+`/popups/test`（`serve:2415-2417/5724-5900`）；Studio/Runner/CLI 均加载（CLI `run.rs:579-594`；本轮无独立配置 UI）。
- **UI**：Studio nav 6 项（`index.html:45-51`）+ `#page-map`（`:469`）；`map_ui.js` canonical（`/map.js` 静态同步，测试 `ui_pure_test:5681-5688`）；builder `navigate`（`OP_META:23`/FIELDS:98/DEFAULTS:133/校验:779）；下线帧-素材关系 UI，`/relations/*` 保留 deprecated。

### C. 验证 / E2E / 风险

- workspace **1039/0**（flow 83 / runtime 264 / oas-task 167+ / oasd 471 等；`_verify/seq136-final/workspace*.log`）；UI **2652 checks/195 ids/0 dead/158 runner/6 nav**；`cargo check/build -p oasd --features ocr` exit 0。
- E2E `_verify/seq136-map/E2E-REPORT.md`：**18 checks/0 failed**（map CRUD/.bak/validate/state/plan/不可达 400/navigate dry/probe/T11 两态/publish map_requirements/install warning/素材 delete+rename 400/无残留监听）；**real 导航 SKIP（无 adb）**，同状态机由 fake-frame 单测覆盖；D0 冒烟 frame6 命中 close box [979,150,27,31] point(991,169)；B 冒烟 polygon+fixed 候选 shape=polygon、fixed 不平移。
- 风险/遗留：真实设备导航/replay 多跳 E2E 未跑；`map_requirements` 只收根 flow（call_task 子任务未纳入）；CLI popups 复用 serve 内部函数（耦合）；T11 只校验 target 存在（map 资产引用由 `/map validate` 覆盖）；map 损坏时素材引用保护可能失效；relations deprecated；边编辑无内联改字段；`/map/navigate real` 同步阻塞；地图文件需用户分发、不进包；用户数据未提交。
- 旧文更正：`§22.41` 帧-素材关系被地图取代（UI 下线、API deprecated）；`§22.73` seq134 的点击几何/OCR 合并/fixed 以 §22.75 为准；`OPERATION-REGION-DESIGN §8.10` 为 polygon/fixed 现行口径。

## seq137 追加验收（2026-11-07：九条反馈 → 三路审计 → 修复 45a458b）

> 用户 2026-11-07 21:07 九条验收反馈；三路只读审计 `_verify/seq137-audit/{map-audit,builder-audit,task-drift-audit}.md` 确认为**接错线/状态漂移/测试假绿**；修复 `45a458b`。HEAD `45a458b`（dev=`origin/dev`，main=`origin/main`=b3e3013 待 docs 后快进；未 tag/未 release）。记录 `ENGINE §22.76`、`_verify/seq137-audit/FIX-EVIDENCE.md`。

### A. 九条反馈 → 审计事实（摘）

| 反馈 | 审计确认 |
|---|---|
| 地图素材/边/返回素材选不了 | `loadAssets` 调 `GET /library`（0 项），全局素材在 `GET /task/assets`（96 items+5 collections）；`assetSelect` 回退死代码 |
| 点节点/“从当前帧”78s 假死 | `/map/probe` 对前 40 素材逐个 `recognize_best`（OCR 各 2 次 det+rec）实测 78.3s；`screen_lines` 无缓存；`/map/state` 本身仅一次 det+rec（383–568ms） |
| 地图 OCR asset 特征 2.8–3.0s | identify 用 `recognition_candidates` 全枚举；OCR 资产每次 ROI+全帧 2 次 det+rec；`wait_until_node` 每 150ms 全帧 OCR；PopupGuard `min_hits` 不早退（+6.45s/3 轮） |
| map 门禁不合理 | `map_requirements` 只收根 flow（不展开 call_task）而 T11 用展开 flow；publish 无 map/target 硬失败、install 仅 warning；map 不入包 |
| 画布操作区残留 | 清理只清识别区，未清 `actionPoly/actionPolyClosed` |
| 操作区漂移/点测仍矩形 | UI 二次平移 `candidateActionPolygon`（fixed 再平移、OCR 不平移、链 shape 忽略）；点测只读 `action_box` 矩形 |
| 列表同级拖拽无效 | 行只有 dragstart/dragend，drop 只挂根/子容器；SVG `<g draggable>` 不可靠 + 正则假绿 |
| 流程图看不清/源码非 Rust | `fitFlowGraphSize` scale/offset 未使用；源码=flow JSON；无 flow→Rust 投影 |
| #7 个人突破报错 | **T8 陈旧变量**（非 T7）：sync 用展开 call_task flow 写父 meta、T8 用未展开父 flow 判 stale；删了会加回；publish 被阻断 |

### B. 修复（45a458b）

- **地图性能**：OCR 行缓存 `ocr_roi_cache`（(frame_seq,ROI,mask)，`runtime:2083/2388/2399`）共享 screen_lines/OCR 资产/identify/popup/probe；`asset_feature_passes`（`:2411-2444`）cheap 判定 + 每帧缓存；identify 两阶段 `min_hits` 早退（`:2632-2705`）；PopupGuard 早退+诊断（`:2452-2478/2605`）；`wait_until_node` 指数退避（`:2791-2820`）；`/map/probe`（`serve:5653-5760`）默认 1 次 OCR、支持 node/assets、阈值 pass。实测 probe **78.3s→1.453s debug**、OCR asset identify **2.8–3.0s→1.093s debug**（release 估 0.4–0.6s）。
- **去 map 门禁**：删除 T11（`task.rs` 测试 `navigate_no_longer_gates_lint_on_the_global_map`）；publish 不读 map/不再因 target 失败；`map_requirements`/`map_nodes` deprecated 且不填（`package.rs:148-152/288-290`）；install 不再 warning；`load_nav_map`（`run.rs:540-574`）只查本 flow 字面量 target、runtime loud 保留。
- **地图 UI**：`loadAssets` 改 `GET /task/assets`（`map_ui.js:382-410`）；`assetSelect` 先 append 再设值（`:509`）；父节点全列表（`:551-556`）；OCR 特征页面内列表（`:631-640`）；canvas DPR（`:925-929`）；节点预览改 `/map/state`（`:949-957`）；`map.js`/`map_ui.js` 字节同步（`map.js:2`）。
- **画布/点测 shape**：`clearCanvasSelection` 统一清识别+操作区（`app.js:2175-2189`；调用点 `:7524/:8092/:9097/:9105/:9117`）；删除前端二次平移，候选/点测/长按/拖测消费服务端 `action_shape`（`:1905/2254/2271-2273`；点测 `:3491/:3533`）；`action_box` 仅回退/辅助。
- **拖拽/大图/源码**：列表真实同级 DnD（`builder.js:1681/1706/1282/1840/1887/1181/1213`；删 SVG 假拖拽 `:2516`）；`#flow-page` 全窗口大页（`index.html:417-429`；`builder.js:2522-2590`）；源码默认折叠 + flow→Rust **只读投影**（`index.html:388-405`；`builder.js:2599-2748`，header 标注非可执行/无 Rust→flow）。
- **#7 T8**：sync 改 `load_flow_for_derived_vars`（raw 父 flow，仅 collection 展开；`oas-task/lib.rs:1857-1868/1590-1601`），T8 同口径（`:3421-3426`）；个人突破 save `removed:["p_93445f68","p_c60bcfa1"]` → 2 vars、lint 0/T8 0（二次 save 幂等）。

### C. 验证 / 证据 / 遗留

- workspace **1043/0**（runtime 267 / oasd 472 / oas-task 172 / flow 83；`_verify/seq137-final/workspace*.log`）；UI **2713 checks/195 ids/0 dead/158 runner/6 nav**；`cargo check/build -p oasd --features ocr` exit 0。
- E2E `_verify/seq136-map/e2e-results.json`：**19 checks/0 failed**（probe 缩面、lint 无 map gate 两态、publish 无 map 200、install 无 requirement 200、.bak/validate/plan/navigate dry/素材引用保护；timing state 123ms/probe 124–129ms；real 导航 SKIP 无 adb）。
- 冒烟/证据：`_verify/seq137-perf/perf-result.json`（probe 1453ms/node 1060ms/state_asset_ocr 1093ms）、`_verify/seq137-perf-root/personal-breakthrough-result.json`（T8 0、before 4→after 2）、`_verify/seq137-audit/repro-item7-*.json`（旧 T8 复现）。
- 遗留：`target_expr` 未实现；Rust 投影只读非可执行、无 Rust→flow；`map_requirements` deprecated 未删；probe 默认不返回素材 hints（需 node/assets）；真实设备导航/Firefox 未做；`text_visible`/`verify_expect` 仍 fuzzy+无缓存；全局弹窗 UI 未做；地图边无内联改；canvas 只读无缩放；`map.js`/`map_ui.js` 双文件机制；UI pure test 无浏览器 DnD 自动化；用户数据未提交；未 tag/release。
