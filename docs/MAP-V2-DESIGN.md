# 全局地图 V2 设计：所在地 / 路标 / 路径（seq138 计划，先设计不实现）

> 状态：**设计计划**（2026-11-07 22:2x，HEAD `f6f2ad1`，seq137 已验收）。依据用户 22:28 反馈：现行地图节点 priority/frames/hub 不清楚；工作台帧与地图帧两套状态割裂；地图维护流程不清爽。用户提出新语义：**所在地 / 路标 / 路径**。
> 本文只设计；实现另开任务，不改用户数据。

## 1. 用户新语义（原文理解）

- 定义地图时，只需要定义**所在地**：名称、所在地判定条件、回程路标、去程路标（各自可以有多个）。
- **路标含义**：去往某个路标后，该路标就变成“所在地”。
- 还要定义**去各个路标的路径**：通过现成的素材/集合，动作一般都是点击。

理解为：`所在地`=一个可识别页面；`路标`=当前所在地上的一个入口/按钮/集合；点击该路标的路径后，目标所在地成为当前所在地。`回程路标`/`去程路标`只是按用途分组的有向出口，均可多个；不是自动反向边。

## 2. 目标模型（v2 schema 草案）

```json
{
  "schema_version": 2,
  "fallback_location": "庭院",
  "locations": [
    {
      "id": "庭院",
      "name": "庭院",
      "recognize": {
        "min_hits": 1,
        "features": [
          {"kind": "ocr_text", "text": "庭院"},
          {"kind": "asset", "asset": "庭院_卷轴打开状态"}
        ]
      },
      "return_landmarks": [
        {"id": "庭院_返回上一级", "name": "返回上一级",
         "target": "庭院",
         "path": {"kind": "click_asset", "asset": "返回上一级_宠物小屋"}}
      ],
      "outbound_landmarks": [
        {"id": "庭院_探索", "name": "探索入口",
         "target": "探索",
         "path": {"kind": "click_asset", "asset": "探索入口_庭院", "wait": {"settle_ms": 600}}},
        {"id": "庭院_召唤", "name": "召唤入口",
         "target": "召唤",
         "path": {"kind": "click_asset", "asset": "召唤入口"}}
      ]
    }
  ]
}
```

- **所在地 Location**：`id/name/recognize{min_hits,features[ocr_text|asset]}`；识别条件复用 seq134 单源（全屏 strict OCR + 素材候选/阈值）。
- **路标 Landmark**：`id/name/target/path`；`target` 必须是所在地 id；`path` 通过现成素材/集合，V1 动作=点击（`click_asset`；collection 允许作为点击目标）；后续可扩展 key/swipe。
- **回程/去程**只是当前所在地下的两个路标列表（UI 分组）；二者都是**有向出口**，不自动补反向边；回程路标通常 target=上一级/庭院，去程 target=其它所在地。
- **不再有**：`priority`（识别冲突改为 features/min_hits/声明顺序，必要时再用高级 priority）、`frames`（地图不再保存样例帧，统一用工作台当前帧）、`hub`（兜底路由改为可选 `fallback_location`，默认空；寻路失败时若配置则先回它，否则 loud error）。如用户仍需要“多个 hub”，后续再扩展。
- **寻路**：当前所在地识别 → 从该所在地的 landmarks（return+outbound 合并）沿 target 做有向图 BFS/Dijkstra（边权=wait/代价）→ 到达目标所在地。由于 return 是显式路标，不再依赖 parent/LCA；可选 fallback_location 只作为无路径/识别未知时的最后兜底。路径运行时不落盘，加载/保存时校验引用。
- **迁移**：v1 `map.json`（nodes/edges/parent/return_asset）加载时在内存转 v2：node→location，edge→outbound landmark，parent+return_asset→return landmark（能确定路径素材的才生成；无法确定的保留 raw/警告）；priority/frames/hub 丢弃或进 advanced；保存时写 v2，先 `.bak`，不自动改写未保存用户数据。

## 3. 工作台整合（替代独立地图页）

用户要求：地图塞到工作台「素材/图片/任务/点击分布」那一栏，复用工作台 UI，快速交互。

- 工作台新增「地图」tab（与素材/图片/任务/点击分布同级），删除独立「地图」导航页/按钮（或保留 URL deep link 只能从工作台打开）。
- **共享当前帧**：工作台选中的帧是唯一帧状态；地图预览、识别当前所在地、从当前帧建所在地、路标命中框全部使用该帧；工作台上一帧/下一帧/切帧立即刷新地图预览。地图不再保存每节点 frames。
- 布局：左=所在地列表/树；中=当前所在地编辑（名称/识别条件/回程路标/去程路标）；右=当前帧预览+识别/路标命中高亮；下方=目标所在地选择+路径预览+dry/real 调试。素材/集合选择复用工作台现有选择器；操作区/识别区绘制复用工作台画布组件。
- 路标编辑：路标名称、目标所在地（下拉/搜索）、路径素材（素材/集合 picker）、可选等待；回程/去程分两个列表；新增/删除/内联编辑。识别条件复用工作台条件行组件（asset/OCR/比较/min_hits）。
- 路径预览/调试：识别当前所在地，选择目标，显示路标链（从哪个所在地点击哪个路标到哪个所在地），dry 只预览；real 临时任务执行（无设备时 SKIP），失败给明确位置。
- 移除 priority/frames/hub 的用户可见字段；如确有高级需求，放在折叠「高级」并解释。

## 4. 实施计划（子进程拆分）

| 阶段 | 内容 | 写作用域 | 验收 |
|---|---|---|---|
| V2-A 设计/契约 | 本文件 + PENDING 台账；确认 v2 schema/迁移/取舍（priority/frames/hub/fallback） | docs/ | 用户确认 |
| V2-B 后端模型/运行时 | oas-task map v2 schema/validate/migrate v1；runtime 识别/路标路径 BFS/执行 adapt；flow `navigate` target 仍字面量（动态 target_expr 另项）；serve `/map` v2 API、plan/state/navigate；素材引用保护；tests | crates/oas-task/map.rs、crats/oas-runtime/map.rs+lib.rs、apps/oasd/src/{serve,run,task,package_cmd}.rs | workspace 只增；v1→v2 迁移单测；多跳/回程/无路径/识别未知/弹窗中断 |
| V2-C 工作台地图 tab | 删除独立地图页/导航；`map_ui.js`→工作台 tab；共享当前帧；所在地/路标/路径编辑；路径预览/调试；复用条件行/素材 picker/canvas；UI 测试 | apps/oasd/ui/** | UI checks 只增、0 dead；手测工作台切帧地图同步 |
| V2-D 文档/遗留 | ENGINE §22.77、ACCEPTANCE、PENDING§E-106、RESUME/INDEX/HANDOFF/START-HERE；旧地图 UI/priority/frames/hub 文案更正 | docs/ | 文档与代码一致 |

## 5. 当前遗留（seq137 之后，必须带进后续）

1. **navigate target 动态参数**：`target_expr`/param() 未实现；现在 target 只能字面量节点，发布不再门禁，子任务可调用但目标不能传参。优先做（用户 #3 闭环）。
2. **Rust 源码框**：当前只是 flow→Rust **只读投影**，不可执行/不可编辑编译；项目无 Rust→flow 编译器。需用户决定：保持只读投影，还是投入 Rust 源码 DSL 编译器（L/XL）。
3. **真实设备导航**：`/map/navigate real` 与 replay 多跳 E2E 未跑（无 adb），Firefox 手测未做。
4. **全局弹窗 UI**：D0 只有配置/API/runtime，无 Studio 弹窗配置面板；导航/map 无弹窗可视化。
5. **地图 UI 细节**：边只能新增/删除无内联改；canvas 只读无缩放/拖动画布；`/map/probe` 默认无素材 hints（UI 传 node/assets）；map.js/map_ui.js 双文件机制；`map_requirements` 字段 deprecated 未彻底删。
6. **地图性能**：已降 probe 78.3s→1.45s、OCR asset identify 2.8–3.0s→1.09s（debug）；release 未实测；wait 退避/Popup 早退已做，但 text_visible/verify_expect 仍 fuzzy 无缓存。
7. **操作区 fixed/polygon**：UI 已消费服务端 action_shape；fixed 自然路径（编辑固定操作区后保留 mode）仍需手测；drag rect 仍 bbox+margin（设计保留）。
8. **relations**：UI 已下线，`/relations/*` 服务端 deprecated 保留；旧证据/标注不迁移。
9. **UI 测试**：无浏览器级 DnD/画布自动化；当前为纯函数/静态断言，用户手测清单需覆盖。
10. **用户数据**：assets/frames/tasks/scheduler 的改动未提交；禁止提交/覆盖。

## 6. 需用户确认的点

1. 是否按本设计**删除 priority/frames/hub**（推荐；高级选项可不显示）；`fallback_location` 是否保留为可选高级？
2. 地图 v1（若有）是否允许内存迁移→保存 v2（推荐，不自动改未保存数据），还是直接重扫？
3. 工作台内地图 tab 放在「素材/图片/任务/点击分布」同一栏的哪个位置（推荐最后）；独立地图页是否彻底删除。
4. 路标路径 V1 是否只支持点击（素材/集合），key/swipe 作为高级后续？（推荐）
5. Rust 源码：保持只读投影 / 实现可编辑 Rust→flow 编译器（选后者需明确投入）。
6. navigate target 动态参数（target_expr）是否下一批优先做？（推荐是）