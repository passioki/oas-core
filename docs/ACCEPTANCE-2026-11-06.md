# 验收反馈（2026-11-06，seq124 后）整改理解与计划

> 状态：**待用户确认关键口径后开工**（本文只是理解+计划，未改代码）。
> 输入：用户 13 条验收反馈 + 3 路只读排查报告（仓外）：
> `_verify\acceptance-20261106\01-bug-triage.md`（#3/#5/#6/#9/#11）、`02-refactor-design-inputs.md`（#1/#2/#4/#10）、`03-infra-ops.md`（#7/#8/#12）。
> 当前基线：HEAD `d9c3dcc`；workspace 904/0；UI 2512 checks/194 ids/0 dead/6 nav；E2E 27/26/0/1。

## 1. 逐条理解与整改方向

| # | 用户反馈 | 核实结论（报告证据） | 整改方向 | 量 | 批次 |
|---|---|---|---|---|---|
| 1 | 动作属性页仍有「文字（参数）」下拉 + 「插入参数」按钮 | 属实：locator 文字来源 param 选项 `builder.js:2714-2729`；插入参数组件 `builder.js:2594-2683` + 4 处挂载 | 删除 param 选项与「参数」措辞；旧 flow 的 `text_expr:param()` 以只读「旧参数来源」回显；「插入参数」改为「插入识别文字配置项（素材）」仅列 ocr_var 派生行（写同样的 `param(key)`）；条件编辑器「文字（参数）」行改为选 ocr_var 素材（生成 `text_visible(param(key))`）；goal battles 的 `param('max_battles')` 改字面量 loop | S+M | D |
| 2 | 构建页/任务库仍有配置项参数 | 属实：`#builder-params` `index.html:341-356`+`builder.js:1153-1249`；任务库配置项对话框 `app.js:802-806,2029-2152`；T3 去声明 `builder.js:1284-1293,1446-1453`；Runner 通用参数组+高级 JSON `runner.html:524-543,190-200,459-461` | 删除用户侧参数入口（面板/配置项对话框/去声明/通用参数组/高级 JSON 参数）；服务端 `meta.params` 派生模型与 `/task/config` 保留（内部/诊断）；Runner 只保留「使用端OCR文字配置项」分节 | M | D |
| 3 | 多候选只对参数 ocr 生效；引用非参数 OCR 素材时 pick 只有 1 候选 | 部分属实：模板/集合/ocr_var/派生 ocr_line 链节点可多候选 ✅；**普通 ocr_line/ocr_lines 作链节点运行时报错**（校验却放行，`oas-task:63-69` vs `runtime:3145-3148`）；**根目标运行时只取最佳**（只有编辑器 test_action 可传 candidate_index）；候选 API 与 test_action/链枚举口径三处分裂（`candidates_for_asset` 未接线） | ① 统一候选枚举（`candidates_for_asset` 接到 serve 候选 API + `preview_root_candidate`，单行 OCR 编辑器路径改 det+rec）；② 普通 OCR 素材作链节点：允许保存/读取「期望文字」（否则不允许引用并明确提示）；③ pick=score/nearest 对模板多实例、同文多处 OCR 真正生效；④ #11 修复后「队伍2/3」不再是候选（同文只有一处时 1 候选是正确的，需用户确认语义） | M | C |
| 4 | 操作区去除「相对偏移」概念，它=修改当前素材操作区；引用素材只作识别链节点、不得改其设置 | 属实且现状与用户要求冲突：现状=模型 B（链末端被引用素材的操作区决定落点，`runtime:3036-3108`；UI 保存时 PATCH 被引用素材 `app.js:5701-5715`） | **改为模型 A**：链仅作识别门槛（逐级识别，全中才动作），**最终操作区=当前素材自己的操作区**；删除 asset 级 offset（schema/校验/runtime/UI/测试）；操作区对话框：当前素材可选「当前素材操作区（用统一框选编辑）」/「引用素材链（只读查看被引用素材已有链，不可在此修改）」；保存只写当前素材 | M | B |
| 5 | 参数类素材不支持阈值 | 后端/运行时/候选全链路已支持；**仅 Studio UI 阻断**：`app.js:assetThresholdKind` 只认 template/ocr_line/ocr_lines（`app.js:6699-6704`），菜单却显示可用 | UI 放开 `ocr_var`→OCR 置信度阈值；补 ui_pure_test 断言。无需改 Rust | S | A |
| 6 | 多边形保存/展示回退矩形（素材行预览、点选后画布） | 部分属实：保存链路基本保留多边形；**显示层恒矩形**（缩略图按 roi 裁矩形 `asset_ui.js:21-80`；`pickAssetRoi` 清空本地 polygon 不恢复 `app.js:6999-7017`）；`/task/asset/update` **无 polygon 字段**（OCR/非模板移动识别区时已存 polygon 不平移）；模板缩放 ROI 会显式回退矩形并弹窗（`app.js:6518-6583`，属设计限制） | ① 缩略图叠加 polygon 轮廓；② 选中/编辑素材时恢复并绘制已存 polygon；③ `/task/asset/update` 增加 `polygon`（支持清空/平移/校验），OCR/region 识别区移动时同步平移 polygon；④ 可选：模板缩放保持多边形 | S/M | A/B |
| 7 | Runner 端添加启动脚本 | 发行包里有、仓库根没有；dev 入口 `scripts\dev-serve.bat -Surface runner` 可用；packaging/dist-stage 的 start-runner.bat 停在 seq118 且双击失败无提示 | 新增 oas-core `start-runner.bat`（委托 dev-serve，固定 8789，自检 exe/模型/端口，失败 pause）+ oas-runner 根 `start-runner.bat` + 增强 packaging 两个 bat | S | E |
| 8 | GitHub About 大量「?」 | 存储值就是字面 `?`（非渲染问题）且不可逆；`updated_at`=2026-11-06T05:39Z；topics 正常、homepage=null | 用 UTF-8 无 BOM JSON + `gh api PATCH --input`（或 curl --data-binary）重写 description；建议同时设 homepage；**需用户授权**（安装 gh 或提供 repo 管理 PAT） | S | E |
| 9 | 改识别文字后操作区被重置；Runner 改是否同样 | 属实（明确条件）：ocr_var **无 roi/全屏** 时走 `/task/save_asset replace:true`，payload 不带 `op_region/action_shape/threshold`，replace 整项覆盖→操作区链清空；有 roi 走 update 不丢。**Runner 端不会**（只写 values.json，不碰 assets.json）。同类 bug：模板识别区覆盖保存也不带 op_region | ① UI replace payload 携带原 op_region/action_shape/threshold；② 服务端 replace 对 payload 缺省字段从磁盘现有 item 继承（显式 null=清空）——兜底任意调用方；③ 补 Rust+UI 测试（ocr_var 全屏 replace 后 op_region 不变） | S/M | A |
| 10 | 识别区/操作区不能用多边形框选；建议统一框选（拖动=矩形，点击=多边形多点+双击收尾；只留 3 按钮：识别区域/操作区域/反选；素材行接入） | 可行；识别区多边形后端已是**真实掩膜**（模板 masked NCC、OCR `mask_frame_for_ocr`），不是仅采样限制；缺口=UI 不载入/不显示已存 polygon、`/task/asset/update` 无 polygon；**反选仅对操作区域可行**（识别区掩膜补集无业务语义） | 统一画布工具状态机：`canvasTool{none,recognition,action}` + 手势区分 rect/polygon；3 按钮；素材行「识别区/操作区」接入同一入口；保存：识别区矩形→roi，多边形→roi=bbox+polygon；操作区 action_shape rect/polygon+invert；后端 update 支持 polygon；ocr_var 放开 polygon 并给 `ocr_read_var` 补掩膜 | M | B |
| 11 | 全屏识别「队伍1」把队伍2/3 也当候选 | 属实：`ocr_text_matches` 对长度≥2 且长度差≤1 允许 **1 个编辑距离**（`runtime:969-999`）→队伍1/2/3 互相匹配；候选、点击、等待共用；阈值救不了（同屏同字体分都高） | ① 加**数字保护**：归一化后数字串不同→不匹配（队伍1≠队伍2；召喚=召唤仍保留）；② 候选枚举走严格模式（normalized 相等，不做编辑距离），运行 click/wait 保留数字保护后的 fuzzy 或可开关；③ 单测覆盖；④ 修复后 #3 的「假多候选」一并消失 | S/M | A |
| 12 | oas-core/runner 文件夹十几 GB | `D:\OAS-dev` 合计 24.6GB：`_verify` 10.4GB（历轮 E2E 各复制一份 target）> oas-core 9.66GB（target 9.27：incremental 4.36+deps 3.48）> oas-runner 2.60GB（target 2.51）> backend 1.31 > frontend 0.46；git 仅 63MB | 可回收 ~15-18GB：停 8788 服务后 `cargo clean`（core/runner）、删 dist/stage(-0.22GB)、_verify 只留最近 1-2 轮（其余归档/删）、可选 flutter clean。**需用户批准**（本机当前有 oasd PID 占 8788） | S | E |
| 13 | 配置交互不应只有 OCR 变量，还应支持其他变量传递；编辑端创建、任务应用、发布后使用端个性化修改；后期多账号/多配置/多模拟器 | 现状：ocr_var + meta.params 派生（C1′）；无通用标量变量、无 profile；Runner 只有按任务 values | 设计「变量系统 v2」：变量类型（ocr_text/string/int/bool/enum）+ 默认值/可被使用端改/分组/描述；编辑端创建并应用到 flow；发布 manifest 带变量 schema；Runner 安装后「使用端配置」只显示可改变量；values.json 按变量存值；**Profile**（账号/模拟器/配置预设）= 命名值集合，调度条目/运行指定 profile；与 Phase 2（彻底去 meta.params）合并设计 | 设计 L / 实现 L | F |

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
