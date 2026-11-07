# 全局地图 V2 设计：所在地 / 路标 / 路径（seq138：V2-B/C 已实现，设计原文保留）

> 状态：**V2-B/C 已实现**（2026-11-07 23:xx）——V2-B `8394af7`（地图 V2 后端：schema2/所在地/路标/路径/fallback_task/runtime BFS/serve/run）+ V2-C `8134376`（工作台地图 tab：独立页删除、共享帧、所在地-路标-路径编辑+搜索预览）。测试 workspace **1044/0**、UI **2717/195/0 dead/158/6（Studio nav 5）**。本文件保留设计原文 + §9 实现结果；剩余 `target_expr`（T1）与 Rust 编译器（rustc 原生）**延后**（本轮不做，源码框保持 flow JSON+Rust 只读投影）。
> 依据：用户 22:28 反馈 + 22:53 澄清/裁决。现行地图 v1 的 priority/frames/hub 语义不清、工作台帧与地图帧割裂、维护流程不清爽；改为「所在地 / 路标 / 路径」，并并入工作台。

## 1. 用户确认后的语义（按 22:53 澄清）

- **路标就是另一个所在地的引用**：路标与所在地对应旧模型里的两个相邻节点；路标不是独立实体，目标所在地的完整定义（名称/判定条件）在地图的所在地表中已有。
- 所在地引用路标时只需要：**名称 + 路径**。名称用于索引目标所在地；**不再有独立的 target/id 字段**（用户指出这是冗余）。
- **路径 = 动作 + 素材**：动作目前就是点击（素材/集合）；V1 路标路径只支持点击，key/swipe 后续高级。
- **所在地定义**：名称 + 判定条件（全屏 OCR 文字/素材特征 + min_hits）。
- 回程路标 / 去程路标只是当前所在地下的两组相邻所在地引用（都可以多个）；二者都是有向出口，不自动补反向边。
- **失败兜底**：按用户裁决，直接重启游戏回到初始界面庭院；可通过调用「返回庭院」任务实现。配置为 `fallback_task`（默认 `返回庭院`），识别未知/无路径/执行失败时先调用它，再重识别重规划；仍失败则 loud error。若该任务不存在，报明确错误（不静默重启）。
- **删除 v1 的 priority/frames/hub 用户可见字段**：
  - priority：识别冲突用 features/min_hits/声明顺序，确有需要放折叠「高级」；
  - frames：不再保存样例帧，统一使用工作台当前帧；
  - hub：删除；兜底改为上面的 `fallback_task`。
- **独立地图页彻底删除**：地图并入工作台，作为与「素材/图片/任务/点击分布」同级 tab，放在**最后**。
- **Rust 源码：用户选择“编译器”**，不是只读投影；详见 §5 与依赖顺序。

## 2. v2 schema 草案

```json
{
  "schema_version": 2,
  "fallback_task": "返回庭院",
  "locations": {
    "庭院": {
      "recognize": {
        "min_hits": 1,
        "features": [
          {"kind": "ocr_text", "text": "庭院"},
          {"kind": "asset", "asset": "庭院_卷轴打开状态"}
        ]
      },
      "return_landmarks": [],
      "outbound_landmarks": [
        {"name": "探索",
         "path": {"action": "click", "asset": "探索入口_庭院", "wait": {"settle_ms": 600}}},
        {"name": "召唤",
         "path": {"action": "click", "asset": "召唤入口"}}
      ]
    },
    "探索": {
      "recognize": {"min_hits": 1, "features": [{"kind": "ocr_text", "text": "探索"}]},
      "return_landmarks": [
        {"name": "庭院", "path": {"action": "click", "asset": "返回上一级_宠物小屋"}}
      ],
      "outbound_landmarks": []
    }
  }
}
```

- `locations` 以**所在地名称**为 key（名称全局唯一，作为路标索引）；显示名=key；如需要可用 `label` 做展示别名。
- `recognize`：复用 seq134 单源（全屏 strict OCR + 素材候选/阈值）；`min_hits` 1..=features.len()。
- `return_landmarks` / `outbound_landmarks`：`{name, path}`；`name` 必须指向 locations 中另一个所在地（校验存在、非自身；允许环，路径搜索已处理）。
- `path`：`{action:"click", asset:"<素材/集合 id>", wait?}`；素材/集合引用由现有素材库校验；V1 动作仅 click，后续可扩展 key/swipe。
- 无 priority/frames/hub；无显式 edges/目标字段；边由路标引用派生，有向、不补反向。
- 路径运行时不落盘；加载/保存时校验所有所在地/路标/素材引用；保存原子写+`.bak`。
- v1 迁移：用户表示自行判断（22:53 确认项 2）。实现采用**内存迁移**：读取 v1 nodes/edges/parent/return_asset → v2 locations/landmarks；能确定路径素材的生成路标，无法确定的保留 raw + warning；保存时写 v2 并备份；不自动改写未保存用户数据。若当前无 v1 用户 map，则只保留迁移代码与测试，不做额外 UI。

## 3. 工作台整合（替代独立地图页）

- 工作台新增「地图」tab，位置在「素材 / 图片 / 任务 / 点击分布」**最后**；独立地图导航页/按钮**彻底删除**（不再保留 deep link 页面）。
- **唯一帧状态=工作台当前帧**：工作台选帧/上一帧/下一帧/切帧立即刷新地图预览、当前所在地识别、路标命中高亮；“从当前帧建所在地/识别/预览”直接用该帧。地图不再存 frames。
- 布局：左=所在地列表；中=当前所在地编辑（名称/判定条件/回程路标/去程路标）；右=当前帧预览+识别/路标命中；下=目标所在地选择+路径预览+dry/real 调试。
- 路标编辑：只填**名称（目标所在地，下拉/搜索已有所在地）+ 路径（动作+素材/集合 picker）**；不在此处重复定义目标所在地的识别条件。可新增/删除/内联修改；回程/去程分两组。
- 复用工作台素材/集合选择器、条件行/识别条件编辑器、画布叠加组件；避免第二套 UI。
- 路径预览/调试：识别当前所在地 → 目标所在地 → 显示路标链；dry 只预览不点击；real 用临时任务/executor（无设备时 SKIP 并说明）；失败明确当前所在地/路标/目标。

## 4. 实施计划与依赖

| 阶段 | 内容 | 依赖 | 验收 |
|---|---|---|---|
| V2-A 确认 | 本文件 + 台账；确认语义/删除 priority/frames/hub/fallback_task/独立页删除 | 用户已确认大部分 | 计划锁定 |
| V2-B 后端模型/运行时 | oas-task map v2 schema/校验/v1→v2 内存迁移；runtime 所在地识别、路标有向图 BFS、navigate 执行适配、fallback_task；serve `/map` v2、state/plan/navigate；测试 | V2-A | workspace 只增；迁移/多跳/回程/无路径/识别未知/fallback |
| V2-C 工作台地图 tab | 删独立地图页；地图 tab 最后；共享帧；所在地/路标编辑器；路径预览/调试；复用组件；UI 测试 | V2-B（API 契约） | UI checks 只增/0 dead；切帧同步手测 |
| V2-D 文档/台账 | ENGINE §22.77、ACCEPTANCE、PENDING/RESUME/INDEX/HANDOFF/START-HERE；旧地图文案更正 | V2-B/C | 文档代码一致 |
| **T1 target_expr** | `navigate`/路标目标支持 param()/变量动态目标（用户 #3 闭环）；发布不门禁；版本门控 | **必须 V2-B 地图基础完成后** | 动态目标可传参；runtime 解析；测试 |
| **R1 Rust 编译器** | 可编辑 Rust 源码→任务 IR/flow 的编译器（见 §5）；不是只读投影 | **地图/动作 schema 稳定；建议 T1 后** | Rust 源码可编辑、编译、执行；错误定位；测试 |

## 5. Rust 源码编译器（用户选择“编译器”）

- 目标：源码框不再只是 flow JSON/只读投影，而是**可编辑 Rust 任务源码**；保存时用项目内编译器编译为任务 IR/flow（或直接执行），错误定位到 Rust 源码行/列。
- 方案候选（实现前出详细设计）：
  1. **Rust DSL → flow/IR 编译器**（推荐 V1）：定义受限 Rust API（`TaskCtx` + `oas_task!`/宏），使用 `syn` 解析函数体，映射到现有 `oas-flow::Step`；执行仍由 Rust 引擎解释。可复用现有 flow 校验/发布；工作量中等，风险可控。
  2. **Rust 源码 → 动态库/native 编译**（XL）：每任务用 `rustc/cargo` 编译成 cdylib 加载；运行时 ABI/热加载/沙箱复杂，不建议 V1。
- 本次用户选择编译器，按方案 1 设计：新 crate `oas-task-dsl`（syn）+ 源码框从只读投影升级为“校验并编译”；保留 flow JSON 作为编译产物/高级视图。
- 依赖：地图 V2 schema/动作 schema 先稳定（V2-B），target_expr（T1）后再接编译器，避免反复改。

## 6. 当前遗留（seq137 后必须带进后续）

1. **target_expr 动态目标**：未实现；必须在地图 V2 基础完成后做（用户 22:53 确认：地图要改，先完成前置基础）。
2. **Rust 编译器**：用户选择编译器；当前只有只读投影，需按 §5 实现（依赖地图/动作基础）。
3. 真实设备导航/replay 多跳 E2E 未跑（无 adb）；Firefox 手测未做。
4. 全局弹窗配置 UI 未做（D0 仅 API/runtime）。
5. 地图 UI：边无内联改、canvas 只读无缩放、`/map/probe` 默认无素材 hints（UI 传 node/assets）、`map.js/map_ui.js` 双文件、map_requirements deprecated 未彻底删。
6. 性能：debug probe 1.45s、OCR asset identify 1.09s（release 未实测）；`text_visible`/`verify_expect` 仍 fuzzy 无缓存。
7. 操作区 fixed 自然路径手测、drag rect bbox+margin 保留；relations 服务端 deprecated 清理；UI 测试无浏览器 DnD/画布自动化；用户数据未提交。

## 7. 已确认项（22:53）

- 删除 priority/frames/hub；失败兜底=重启游戏回庭院（调用 `fallback_task` 默认「返回庭院」）。
- v1 迁移由实现自行判断（推荐内存迁移，不自动改未保存数据）。
- 工作台地图 tab 放最后；独立地图页彻底删。
- 路标路径 V1 只支持点击素材/集合。
- Rust 源码采用“编译器”方案（见 §5）。
- target_expr 必须等地图前置基础完成后再做，需与地图改版一起规划。

## 8. 23:03 补充确认（覆盖/补充前文）

- **v1 迁移只迁移代码/测试**：用户澄清“如果 v1 map 是用户数据，那不用管，只迁移代码即可”。因此不处理/不自动迁移任何现有用户 map 数据；保留 v1→v2 转换函数与单测，供以后需要时使用。
- **素材搜索+候选**：路径/路标中的素材使用现有素材搜索样式，且支持候选（复用 `assetSearch`/候选 API/选择器）；不是裸文本输入。
- **帧搜索**：帧同样支持搜索，复用现有帧搜索选择逻辑；路标/路径/所在地编辑中涉及帧时使用同一搜索+候选体验。
- **预览**：完成所在地定义后，路标/路径涉及的素材/帧要有预览（素材缩略图/候选框、帧缩略图/全图预览），便于确认路径指向。
- **Rust 源码**：用户想要的是 rustc 原生源码；本轮**不做编译器**，优先级调低；源码框保持 flow JSON 可编辑 + Rust 只读投影，不实现 Rust 编译器/原生产物。
- 其余（所在地/路标/路径语义、删 priority/frames/hub、fallback_task 默认返回庭院、工作台地图 tab 放最后、独立地图页彻底删、路标路径 V1 仅点击、target_expr 地图基础后）按原计划。

## 9. 实现结果（V2-B `8394af7` + V2-C `8134376`，2026-11-07 23:xx）

- **V2-B 后端**：`oas-task/src/map.rs` V2 schema2（`locations` 名称 key、`Landmark{name,path}`、`MapPath{action:"click",asset,wait?}`、`fallback_task` 默认返回庭院；上限 500/2000/6）→ validate/load/save_atomic(.bak)/to_runtime；v1→v2 **仅内存迁移代码+测试**（不重写用户数据，priority/frames/hub 丢/警告）；`oas-runtime/src/map.rs` 路标有向图 BFS（return+outbound，不补反向、无 parent/LCA/hub）；`runtime/lib.rs` 所在地识别（seq134 单源+seq137 缓存/早退、min_hits、hits/total/声明序、无 priority）、navigate V2（逐路标 click→settle→popup→重识别校验→不符重规划、PlanOnly/Execute/预算/取消）、`fallback_task` 注入后失败执行一次+重试（缺失 loud）；serve `/map` GET/POST/validate/state(location，node 兼容)/plan/navigate/probe 全 v2+v1 body 内存迁移、素材引用保护；run 加载 v2/本 flow 字面量 target/sibling fallback flow。
- **V2-C UI**：独立地图页/导航彻底删除（Studio nav 6→5），地图=工作台 rail 最后 tab `#panel-map`（全宽/关闭回素材）；共享工作台当前帧（`oas:frame-changed`，上一帧/下一帧/帧搜索实时刷新，地图不存 frames）；所在地编辑（名称/别名/判定条件 ocr_text|asset + min_hits/回程+去程路标）；路标只填目标所在地名称+路径（click+素材/集合搜索选择器 OASAssetUI.createSearchPicker+缩略图+settle，内联改删）；纯函数 `window.OAS_MAP`；预览/路径/调试（probe 限素材、state、plan、navigate dry/real、fallback_task、保存 .bak/dirty、本地+服务端校验）；builder navigate 目标改 V2 locations（v1 兼容回退）；`map.js`=map_ui 字节同步。
- **验证**：workspace **1044/0**（runtime 268/oasd 472/oas-task 172/flow 83；`_verify/seq138-final/workspace-v2b.log`）、UI **2717 checks/195 ids/0 dead/158 runner/6 nav（Studio nav 5）**、cargo check/build ocr；V2-B 冒烟 `_verify/seq138-v2b-smoke`（GET /map schema2/4 locations/fallback；plan 2 hops；navigate dry；state；v1 validate 200；probe node；素材 delete 400）；测试矩阵见 `_verify/seq138-v2b/REPORT.md`、`_verify/acceptance-20261106/seq138-v2c-progress.md`。
- **剩余（延后）**：`navigate.target_expr`（T1，地图基础后）；**Rust 编译器（rustc 原生）本轮不做、优先级调低**（§5 方案仅保留设计，源码框=flow JSON+Rust 只读投影，无 Rust→flow）；真实设备导航/replay E2E、Firefox 手测；全局弹窗 UI；地图边内联改/canvas 缩放/map.js 双文件/map_requirements deprecated；`text_visible`/`verify_expect` fuzzy；无浏览器自动化；v1 用户数据不迁移；用户数据未提交、未 tag/release。
