# 参数→素材 / 操作区递归 / 候选识别 实施方案

> 状态：**已实现（seq124，2026-11-06；实现记录 `ENGINE §22.64` / `OPERATION-REGION-DESIGN §8`；最终验证与偏差见文末「实现结果（Lead 验收）」）**。依据：`PARAM-ASSET-REDESIGN.md §1b`（用户 2026-11-06 12:4x 权威逐字要求，取代其 §1/§5）、`OPERATION-REGION-DESIGN.md §6/§7`、三份审计（`_verify/paramasset-audit-20261106/01~03`）。
> 基线：仓库 `D:\OAS-dev\oas-core`，分支 `dev`，HEAD `c33a822`（代码基线 `ba6813e`）。本文所有代码行号为 2026-11-06 实测。
> D1 经历两次 Lead 修正，**最终口径 = C1′（Rust 侧单一派生/同步函数 + 后端自动同步，不做 UI 双写，不写用户数据）**；B（虚拟参数）/A（彻底去 meta.params）列为 Phase 2，不在本轮。
> 本文为设计阶段文档（行号以代码基线 `ba6813e` 为准）；实现结果、与设计的偏差、最终验证数字见文末「实现结果（Lead 验收）」，实现行号以代码/`ENGINE §22.64` 为准。
> **实现并行状态（2026-11-06 12:3x）**：Lead 已派发 P0a 候选 API（task-5：`crates/oas-perceive`、`crates/oas-runtime`、`serve.rs`）、P0b 两状态降级（task-6：`crates/oas-task`、`serve.rs`）、UI P0 动作库去参数+删两状态（task-7：`apps/oasd/ui/*`）。本文行号以代码基线 `ba6813e`（我实测时工作区）为准；P0 合入后行号会漂移，实施时**以函数名定位**。本文与 P0 重叠处标注「P0」并沿用 P0 已定契约（§⑤-4、§0-D4 已对齐）。

## 0. 决策摘要（Lead 已定，不得推翻）

| 决策 | 最终口径 |
|---|---|
| D1 参数→素材 | 新增素材 kind `ocr_var`（中文名「使用端OCR文字配置项」）。素材是唯一 UI 真相；`meta.params` 仍是运行时唯一 `ParamDecl` 来源；由**唯一同步函数**从 `ocr_var` 素材生成/维护带 `asset_id` 标记的派生参数行（C1′）。运行链路（lint/注入/values/Runner）零改动。旧 `meta.params` `role=ocr_text` 读时合成虚拟素材仅展示，用户确认「迁移」才落盘。 |
| D2 操作区域 | `assets.json items[].op_region`：`self`（默认=现有 action_shape/click_box_for 语义）\| `offset:[dx,dy,w,h]`（相对真实识别区域左上角，w/h=绘制尺寸）\| `asset`+`pick:score\|nearest`（引用素材，递归解析其 op_region）。末端必须 self/offset；配置与保存防环（图环 + 深度≤16），运行时 visited 兜底；任一节点未识别=本步失败不点击。 |
| D3 候选识别 | 后端返回**全部候选+评分**（模板/集合/OCR/ocr_var；搜索域=素材识别区域，空=全屏）；素材行「识别」画全部 box；素材行测试动作演练测全部候选；真实点测前可选候选（下拉 #序号+评分）。 |
| D4 废弃两状态 | 审计确认 `action_source` 引擎不读（`oas-runtime` grep 0 命中），全帧金字塔回退（`runtime:739-756`）与素材级 shape 平移（`runtime:419-444`）已存在 ⇒ 删除两状态校验/UI 规则（P0b/P0c 已派发）。字段**降级为兼容字段**：`action_source` 保留在 `AssetDecl`（`#[serde(default=...)]` 容忍旧 JSON 读取），但新保存不再产生两状态数据、不再校验、UI 不再写（与 P0b task-6 口径一致）。保留 `action_shape` 作为「当前素材操作区域」= `op_region.mode=self` 的数据源；默认落点 A（有 shape 仍用 shape，零行为变化）。 |
| D5 量尺 | 被事项 4 取代：改为画布绘制「模拟识别区域 + 操作区域」，**不实现**剪贴板量尺。 |
| D6 动作库 | 删除 `PARAM_ACTIONS` 全部预设与「参数」组；动作目标=素材库素材（含 ocr_var）；动作不再配置操作区域（由目标素材 `op_region` 决定）；flow 旧 `region` 字段保留兼容解析但新 UI 不生成。 |

### 0.1 D1 方案演进（记录，供实现理解口径来源）

| 方案 | 做法 | 结论 |
|---|---|---|
| C1 双写 | 素材保存时 UI/后端同时写 assets.json + meta.params | ❌ 双源漂移风险；Lead 否决 |
| B 虚拟参数 | 不写 meta.params，各消费点（lint/注入/Runner/Studio）内存派生 | ❌ 改造面过大：lint 仅拿 external ids 拿不到 AssetDecl，T7/子任务/多入口派生链风险高；Phase 2 |
| **C1′（本轮）** | `meta.params` 仍唯一来源；**唯一同步函数**后端自动派生/维护 `asset_id` 行；素材唯一 UI 真相；运行链路零改动 | ✅ 本文实施方案 |
| A 全迁移 | 去 meta.params，值改存 asset_values，flow 免 text_expr | Phase 2 |

## ① 概念模型与术语表

- **识别区域（recognition ROI）**：素材在 `assets.json` 的 `roi`；模板=截图源矩形，OCR=识别裁剪框；`ocr_var` 可空（空=全屏 OCR）。
- **真实识别区域（real rect）**：运行时本次真正匹配/识别到的矩形（模板匹配 bbox / OCR 行 bbox / ocr_var 命中行 bbox）。多候选时每个候选一个真实识别区域。
- **操作区域（operation box）**：动作实际落点采样的矩形；来源=目标素材的 `op_region` 链解析结果；`self` 时等于现有 `click_box_for()` 语义（有 `action_shape`/`click_back` 则用它，否则真实识别区域）。
- **`ocr_var`（使用端OCR文字配置项）**：新增素材 kind；=`ocr_line/ocr_lines` 全部机制 + `runtime_text`（默认识别文字）+ `param_key`（参数键，迁移期=旧 key）+ `roi` 可空。使用端在 Runner 改值，值仍存 `values.json tasks.<task>.values.<param_key>`。
- **派生参数行**：`meta.params[]` 中带 `asset_id` 的行（`type=string`、`role=ocr_text`、`default=runtime_text`），由同步函数维护、Studio 只读展示，不是用户手写行。
- **操作区链**：`op_region.mode=asset` 的逐级引用；解析顺序=动作目标素材→引用素材→…→末端 self/offset。
- **候选（candidate）**：同一素材在当前帧中 ≥阈值的全部命中，`{index,rect,score,text?}`；`index` 为接口返回顺序（模板：score desc→y→x，见 `perceive:582-608`；OCR：det 行序）。
- **虚拟素材**：旧任务 `meta.params role=ocr_text` 在 `GET /task/assets` 中内存合成的只读 `ocr_var` 条目（`virtual:true`），不落盘。

## ② Schema（最终态 + 旧字段兼容）

### 2.1 `assets.json`（`oas-task/src/lib.rs:388-400`，`schema_version` 保持 1，新增字段全 Optional）

```json
{
  "schema_version": 1,
  "design_resolution": [1280, 720],
  "items": [
    {
      "id": "O_预设分组", "kind": "ocr_var",
      "runtime_text": "御魂", "param_key": "group",
      "roi": [40, 300, 400, 60],
      "label": "预设分组",
      "threshold": 0.6,
      "action_shape": {"kind": "rect", "roi": [30, 290, 420, 80], "invert": false},
      "op_region": {"mode": "self"},
      "positions": [12], "rev": 3
    },
    {
      "id": "I_战斗按钮", "kind": "template", "file": "images/a001.png",
      "roi": [900, 600, 120, 60], "threshold": 0.85,
      "op_region": {"mode": "offset", "offset": [10, 5, 150, 80]},
      "positions": [12], "rev": 2
    },
    {
      "id": "I_副本入口", "kind": "template", "file": "images/a002.png",
      "roi": [100, 100, 80, 40],
      "op_region": {"mode": "asset", "asset": "I_战斗按钮", "pick": "nearest"}
    },
    {"id": "O_分组标签", "kind": "ocr_lines", "roi": [0, 0, 1280, 720], "threshold": 0.5},
    {"id": "I_操作区", "kind": "region", "roi": [500, 500, 100, 100]}
  ],
  "collections": []
}
```

新增字段（`AssetDecl`，`oas-task/src/lib.rs:284-348`；全部 `#[serde(default, skip_serializing_if=...)]`）：

| 字段 | 类型 | 适用 kind | 规则 |
|---|---|---|---|
| `runtime_text` | string? | `ocr_var` | 必填、trim 非空、≤200 字符、无控制字符；= 使用端未覆盖时的默认识别文字 |
| `param_key` | string? | `ocr_var` | 必填；meta 参数键语法（`is_identifier`，`meta.rs:243-250`，≤64）；任务内唯一；迁移旧参数时=旧 key |
| `label` | string? | `ocr_var` | 可空展示名；派生参数行 label 用它，空则用 id |
| `op_region` | object? | template/collection/ocr_line/ocr_lines/ocr_var | 缺省=`self`；`region` 素材不允许（自身即区域） |
| `roi` | [4]? | `ocr_var` 放宽 | 空=全屏 OCR；其余 kind 规则不变（`lib.rs:2651-2673`、`1349-1355`） |
| `file/polygon/ocr_mode/ocr_source_roi` | — | `ocr_var` 禁用 | T5 拒绝 |
| `action_source` | **降级兼容字段（P0b）** | 全部 | 保留在结构体（`#[serde(default = "default_action_source")]`）只为容忍旧 assets.json 读取；删除全部两状态校验与 UI 规则，新保存不再写（`skip_serializing_if`/默认值）。集合的 `action_source` 同口径（`lib.rs:384-385`）。`action_shape` 保留=操作区数据（self 模式来源） |

`op_region` JSON：

```json
{"mode":"self"}
{"mode":"offset","offset":[dx,dy,w,h]}                    // w,h 必填且 >0；dx,dy ∈ ±1e6
{"mode":"asset","asset":"<素材id>","pick":"score"}         // pick 可空，默认 score
{"mode":"asset","asset":"<素材id>","pick":"nearest"}
```
- 链节点（`asset` 引用的素材）只允许 template/collection/ocr_line/ocr_lines/ocr_var（可识别）；`region` 素材禁止作节点（校验错误）。
- 环检测：保存/配置时 DFS（`visited` 集合 + 深度上限 `MAX_OP_REGION_DEPTH=16`，同 `MAX_REGION_COORD` 风格放 oas-task）；末端不是 self/offset → 报错。

### 2.2 `meta.json`（`oas-task/src/meta.rs:190-226`）

```json
{
  "schema_version": 1, "name": "切换预设御魂",
  "params": [
    {"key": "group", "type": "string", "default": "御魂", "role": "ocr_text",
     "asset_id": "O_预设分组", "label": "预设分组", "group": "识别文字",
     "description": "游戏内预设分组名"},
    {"key": "loop_n", "type": "int", "default": 3}
  ],
  "exits": {"done": {"result": "success"}, "failed": {"result": "retryable"}}
}
```
- `ParamDecl` 新增 `asset_id: Option<String>`（`meta.rs:53-90`，`skip_serializing_if=None`）：存在=派生行，`asset_id`=来源素材 id；旧 meta 无此字段照常解析。
- 派生行固定：`type="string"`、`role="ocr_text"`、`default=runtime_text`、`label=素材 label/id`；`group/description` 保留用户值；其余字段（min/max/enum/required）清空。
- 同步函数**只增删改 `asset_id` 行**，绝不碰用户手写行；派生 key 与手写行 key 冲突=保存失败（响亮）。

### 2.3 `values.json`（`apps/oasd/src/runner_config.rs:44-71`，**零改动**）

```json
{"schema_version":1,"updated_ms":176...,"tasks":{"切换预设御魂":{"values":{"group":"结界","loop_n":5},"updated_ms":176...}}}
```
值 key=`param_key`；旧文件/旧 Runner 天然兼容。**不新增 `asset_values`**。

## ③ 内存 / 运行时模型（`oas-runtime`）

### 3.1 AssetSet 扩展（`runtime:350-384`）

```rust
// 现有 ocr: HashMap<String,(Rect,bool)> 新增并行表，避免破坏既有 insert_ocr/ocr_roi 契约：
ocr_vars: HashMap<String, OcrVar>,        // id -> {roi: Option<Rect>, runtime_text: String, param_key: String}
pub fn insert_ocr_var(&mut self, id, roi: Option<Rect>, runtime_text: String, param_key: String)
pub fn ocr_var(&self, id) -> Option<&OcrVar>
```
- `ocr_roi(id)` 对 ocr_var 返回 `Option<Rect>`（空=全屏）；`ocr_var_expected_text(id, params)`：优先注入参数 `params[param_key]`（运行链路已有 `BTreeMap<String,Value>`），否则 `runtime_text`；两者都空=响亮失败。
- `load_runtime_assets`（`lib.rs:1301-1484`）：`ocr_var` 分支插到 `kind!="template"` 前（1343）：允许 `roi=None`；`ocr_var` 强制走 det+rec（候选行需要 bbox），等价 `insert_ocr_lines` 语义；`op_region` 原样进 `AssetSet`（新增 `op_regions: HashMap<String, OpRegion>`，仅数据）。其余校验：`runtime_text/param_key` 必填；`file/polygon/ocr_mode` 为空。
- `click_box_for`（`runtime:1811-1905`）OCR 分支（1825-1852）对 ocr_var 追加：单行识别的 ocr_var 也有 `last_ocr_rect` 可用（det+rec 路径总是设置）。

### 3.2 操作区链解析（伪代码；新公共函数，供 runtime 与 serve 演练共用）

```text
fn resolve_operation_box(rt, target_asset_id, flow_region: Option<&OpRegion>, params) -> Result<(Rect, Option<ClickShape>)>
  // 1) flow 旧 region 覆盖优先（D6 兼容：只读旧 flow，新 UI 不生成）
  if let Some(r) = flow_region { if r.asset().is_some() { return resolve_legacy_asset_region(r) } }
  // 2) 目标素材的 op_region 链
  let real = recognize_node(target_asset_id, pick_ref=None, params)?       // 根节点多候选：OCR 取最高分 / 模板取最佳
  return walk(target_asset_id, real, params, visited={}, depth=0)

fn walk(asset_id, real: RealRect, params, visited, depth) -> Result<Box>
  if depth > 16 || !visited.insert(asset_id): return Err(step fail "op_region 链环/过深")
  let op = assets.op_region(asset_id).unwrap_or(Self)
  match op.mode:
    Self:
      return op_box_for(asset_id, real)          // = click_box_for 语义：action_shape/click_back 平移后，否则 real
    Offset { dx,dy,w,h }:
      return Rect{ x: real.rect.x+dx, y: real.rect.y+dy, w, h }   // 相对【真实识别区域】左上角；尺寸=绘制尺寸
    Asset { asset: next, pick }:
      let ref = rect_centre(real.rect)           // 参考点=上一节点（当前节点）真实识别区域中心
      let real_next = recognize_node(next, pick_ref=(pick==nearest ? Some(ref) : None), params)?
      return walk(next, real_next, params, visited, depth+1)
  // 任一 recognize_node 失败（未声明/无候选/低于阈值）= 本步失败，不点击、不 fallback

fn recognize_node(asset_id, pick_ref, params) -> Result<RealRect>
  template/collection:
      cands = operation_candidates(asset_id)     // runtime:1917-1981，roi 内多候选 + NMS/上限 64
      if cands.empty: Err("no match >= threshold")
      pick: pick_ref=None -> 最高分（同 locate 语义）；Some(r) -> 中心距 r 最近，平分比 score（复用 2046-2072 逻辑）
      return 选中候选 {rect=匹配 bbox, pos, member}
  ocr_line/ocr_lines/ocr_var:
      text = expected_text(asset_id, params)     // ocr_var: param(param_key) → runtime_text；其他: 由 flow text_expr 解析
      lines = ocr_read_rect(匹配域: asset.roi 或全屏, multiline=true)  // 2183-2211/ocr_read 2887
      cands = ocr_line_candidates(lines, text, asset.threshold)        // 590-601
      pick: 同规则（nearest 参考点=pick_ref/自身 ROI 中心）；无候选=Err
      return {rect=行 bbox, line}
```
- 兼容：`op_region` 缺省或 `self` 且素材无 shape ⇒ 与今天 `click_box_for` 路径完全一致（点击位置零变化）。
- `drag.from/to` 端点、`click_target`、`long_press` 只替换「单层几何解析」调用点即可（3903-3990、2456-2560、2300-2330），不改 locator 解析。
- 帧复用：一次动作只 `observe_screen/reobserve_screen` 一次，链内所有识别复用同一帧（与现有 `ocr_read` 缓存语义一致）。
- 失败语义：链上任一节点 miss ⇒ `Err(format!("{site}: op_region chain `{asset}` no match ..."))`，日志 `region=op_region_chain steps=[...]`。

### 3.3 候选枚举与 `ocr_var` 读取

```text
candidates_for(asset):
  template/collection: Matcher::match_template_all(tpl, roi=asset.roi, threshold)
                       // roi=None(旧库素材) => 全图多候选（all_search_window 613-632 已支持，无金字塔，成本高，文档标注）
  ocr_line(单行)/ocr_var: det+rec 行 → filter ocr_text_matches(expected_text) + score>=threshold
                          // 搜索域=asset.roi，空=全屏；ocr_var expected=param(param_key) 或 runtime_text
  region: [{rect: 固定区域, score: null}]
  返回 index=数组序；模板排序 see perceive:582-608；OCR 按 det 行序
```
- `/lines`（`serve:3149-3250`）无阈值参数（det 内部丢 <0.5，`apps/oasd/src/ocr.rs:98-102`），候选 API 由服务端按素材 threshold 过滤，前端不再自行过滤。
- 全屏 OCR 成本：`ocr_var.roi=null` 时 det+rec 全帧（设计分辨率 1280x720）预计 0.3~1s/次；仅识别/演练入口触发，运行链路按 flow 步骤触发。**风险项**，见 §风险。

## ④ 后端改动清单（文件:行号 + 函数）

| # | crate/文件 | 锚点 | 改动 |
|---|---|---|---|
| B1 | `crates/oas-task/src/lib.rs` | `KINDS` 55 | 增加 `"ocr_var"`（数组 4→5） |
| B2 | 同上 | `AssetDecl` 286-348 | 加 `runtime_text/label/param_key`；加 `op_region: Option<OpRegion>`；`action_source` 降级为兼容字段（P0b 口径：容忍读、不再写/校验）；实现 `ActionShape` 复用 |
| B3 | 同上 | 新 `pub enum OpRegion{Self_,Offset{dx,dy,w,h},Asset{asset,pick}}` + `parse/serialize`（serde adjacently-tagged，或手写 `serde_json::Value` 校验） | 与 flow `OpRegion`（`oas-flow:689-716`）同名不同层：assets 版只 3 种（无 ocr/offset-2 值语义差异：offset 必须 4 值） |
| B4 | 同上 | `load_runtime_assets` 1301-1484 | ocr_var 分支（roi 可空、强制 det+rec、op_region 入集）；删 action_source 相关注释/接线（1375-1390 保留 shape 接线） |
| B5 | 同上 | `save_asset_impl` 1828-2100+ | ocr_var 校验（runtime_text/param_key/label、roi 可空、禁 file/polygon/ocr_mode）；`op_region` 校验（环/深度/节点存在/kind/末端）；两状态校验（1989-2012）与 `region` 的 `action_source="fixed"` 赋值（1984-1987）按 P0b 删除/降级 |
| B6 | 同上 | `update_asset_meta` 1699-1806 | 参数改为 `op_region`/`runtime_text`/`param_key`/`label`；删 `action_source` 形参 1704 与 1772-1786；保留 `action_shape` 语义（画布拖操作区=写 shape，`onAction`） |
| B7 | 同上 | `assets_shape_issues` 2555-2742 | 两状态规则（2626-2650、2712-2739）按 P0b 删除；加 ocr_var 规则、op_region 链规则；派生行一致性由 T8（见 B9） |
| B8 | 同上 | 新 `pub fn sync_ocr_var_params(bundle_meta:&mut Meta, assets:&AssetsFile, flow:&Flow) -> Result<SyncReport,String>`（放 `lib.rs` 或新 `ocr_var.rs`） | 见 §2.2/§⑧；`SyncReport{added,updated,removed,derived:[ParamDecl]}`；纯函数+单独原子写回由调用方 `TaskBundle::write_meta`（新增方法，写前 `.bak`） |
| B9 | 同上 | `lint_into_full` 2315-2462（assets 在 2352 已加载、flow 在 2374） | 新 **T8**：`meta.params` 派生行 vs 素材 `runtime_text`/`param_key`/引用集合不一致 → `LintIssue{code:"T8", path:"meta.json:params[i]"}`；同时 T3 无需改（派生行已写入 meta） |
| B10 | 同上 | 新 `pub fn migrate_params_to_ocr_vars(&TaskBundle, &mut AssetsFile, &mut Meta) -> Result<MigrateReport,String>` | 旧 `role=ocr_text` 参数→虚拟/真实 ocr_var；只由显式 API 调用；备份/回滚在 serve 层 |
| B11 | 同上 | `delete_asset` 1601、`rename`、`update_asset_positions` 1810 | 删除/重命名被 op_region 引用的素材：保存时校验块（B5/B6 已覆盖）＋删除前检查引用返回 400；rename 重写引用（新增） |
| B12 | `crates/oas-task/src/meta.rs` | `ParamDecl` 53-90 | 加 `asset_id: Option<String>`；`lint_issues` 加派生行约束（role=ocr_text+asset_id 时 type 必须 string、default 必须 string） |
| B13 | `crates/oas-task/src/package.rs` | `SUPPORTED_FLOW_OPS` 81-95 / `derive_flow_ops` 606-627；`build_package` 1457-1504 | 默认**不改**：flow 仍用 `text_expr:param('k')`，行为不变；`min_engine=ENGINE_VERSION`（1476）天然拦住旧 Runner。**可选门禁**见 §风险拍板项 3 |
| B14 | `crates/oas-runtime/src/lib.rs` | `AssetSet` 350-384 / 520-564 | 加 `ocr_vars`、`op_regions`、`insert_ocr_var`、`ocr_var`、`op_region` |
| B15 | 同上 | `ocr_read` 2887-2977；`ocr_read_rect` 2183-2211 | ocr_var：roi 空→全屏 Rect；expected text 从 params 取 param_key（注入 map 已是 `self.params`），det+rec；`last_ocr_rect` 语义不变 |
| B16 | 同上 | `operation_region_for_asset` 1995-2089 + `region_asset_target` 2097-2117 + `operation_candidates` 1917-1981 | 抽公共 `pub fn resolve_op_region_chain(frame, assets, target, region, params, ocr:&mut dyn OcrProvider) -> Result<ResolvedRegion>`；`operation_region_for_asset` 改为调用它（保留 flow `region` 单层旧语义）；新增 `pub fn candidates_for_asset(...)` 供 serve 候选 API |
| B17 | 同上 | `Step::ClickTarget` 3603-3990（OCR 分支 3690-3840/3841-3990）；`resolve_press_target` 2340-2560；`drag_target` 2225-2330 | 几何解析统一改调 `resolve_op_region_chain`；`region_log` 650-659 扩展链日志；成功路径日志加 `region_chain=[id...]` |
| B18 | `crates/oas-perceive/src/lib.rs` | 665-762 | 无改动（`match_template_all/masked_all` 已够；`roi=None` 全图已支持） |
| B19 | `apps/oasd/src/serve.rs` | 路由 1877-1903；`route_owner` 153-226 | 新增路由（§⑤）并登记 Studio/Runner 归属；`/task/asset/update` 保留兼容但忽略 `action_source` |
| B20 | 同上 | `handle_save_asset` 3265、`handle_save_assets` 4529、`handle_asset_update` 4814、`handle_asset_delete`、`handle_asset_rename`、`handle_collection_save` 4331 | 成功写盘后触发同步：ocr_var 素材变更→扫描 `tasks_dir` 中 flow 引用该素材的任务→`sync_ocr_var_params`→`write_meta`（`.bak`）；响应加 `synced:[{task,added,updated,removed}]` |
| B21 | 同上 | `handle_save_flow` 6050-6120 | flow 保存成功后同步当前任务派生参数；响应加 `sync`；`rewrite_tasks_to_collection` 1380 与资产迁移后同样补同步 |
| B22 | 同上 | `handle_task_assets` 5518-5592 | items 输出天然带新字段；追加 `virtual_items`（旧 role=ocr_text 合成视图，`virtual:true`，不写盘） |
| B23 | 同上 | `handle_task_config_get` 9567-9586 / `post` 9593-9724 | GET 原样返回（派生行含 `asset_id`）；POST 保护派生行：加载现有 meta，丢弃客户端对 `asset_id` 行的修改（400 提示「派生行只读」），合并用户行后调 B8 重算 |
| B24 | 同上 | 新 handlers | `GET /task/asset/candidates`、`POST /task/asset/test_action`、`POST /task/asset/validate_op_region`、`POST /task/migrate_ocr_params`（§⑤）；`handle_asset_match` 4249-4329 保持旧返回（兼容） |
| B25 | `apps/oasd/src/run.rs` | 526-670 | **零改动**（派生行已在 meta.params；四层注入/复检天然生效）。仅确认 ocr_var 任务的 params 非空不再走 legacy 分支（651-665） |
| B26 | `apps/oasd/src/task.rs` | `lint_one` 208-228 | 零改动（T5/T8 在 oas-task；global store 已传 external ids） |
| B27 | `apps/oasd/src/package_cmd.rs` | 安装 lint 760-774、flow_ops 门禁 1191-1214/1386-1415、`plan_asset_merge` 859-960 | 零改动（T5 会按新 kind 校验；asset merge 用 `AssetsFile` 反序列化天然接受新字段）。若加合成门禁见 §风险 |
| B28 | `apps/oasd/src/runner_config.rs` | — | 零改动（values schema 不变） |
| B29 | 文档同步 | `OPERATION-REGION-DESIGN.md` §7、`PARAM-ASSET-REDESIGN.md`、`ENGINE-DESIGN` 变更记录 | 实现 PR 内同步标注「被本文取代」 |

## ⑤ API 契约

1) `GET /task/assets?task=<name>`（Studio，`serve:5518`）响应追加（`items[]` 每项）：`runtime_text`、`param_key`、`label`、`op_region`；新增：
```json
{"ok":true,"task":"…","design_resolution":[1280,720],"count":6,"items":[…],"collections":[…],
 "virtual_items":[{"id":"O__group","kind":"ocr_var","virtual":true,"runtime_text":"御魂","param_key":"group","label":"group","roi":null}]}
```
2) `POST /task/save_asset`（Studio，`serve:3265`）请求新增：`kind:"ocr_var"`、`runtime_text`、`param_key`、`label?`、`op_region?`、`roi?`（模板/OCR 规则不变；`file/polygon/ocr_mode` 对 ocr_var 返回 400）。响应：
```json
{"ok":true,"item":{…},"synced":[{"task":"切换预设御魂","added":["group"],"updated":[],"removed":[]}]}
```
3) `POST /task/asset/update`（`serve:4814`）新增可选字段 `op_region`、`runtime_text`、`param_key`、`label`；`action_shape` 保留（画布拖操作区）；**`action_source` 忽略不报错**；响应同上含 `synced`。
4) `GET /task/asset/candidates?task=&id=[&frame_id][&live=1]`（Studio；**P0a task-5 已定契约，本文件沿用**）：
```json
{"ok":true,"task":"…","id":"I_战斗按钮","kind":"template","threshold":0.85,"domain":"roi","roi":[900,600,120,60],
 "count":2,"candidates":[{"index":0,"x":902,"y":601,"w":120,"h":60,"score":0.973,"pass":true},
                         {"index":1,"x":300,"y":200,"w":120,"h":60,"score":0.901,"pass":true}],
 "elapsed_ms":12.3,"frame_source":"live"}
```
`index`=稳定序号（模板 score desc→y/x asc，集合成员按声明序拼接后统一排序）；`action_box`=候选操作区（平移后 shape/匹配 bbox）。ocr_var 阶段扩展：OCR/ocr_var 候选追加 `{"text":"御魂","line":true,"action_box":[…]?}`，`domain:"screen"` 表示 roi 空=全屏；P0a 首版 OCR 空 roi 报 400（ocr_var 落地时放开）。op_region 链预览（`op_rect`/`op_shape`）在 P0a 之后随运行时候选 helper 追加。
5) `POST /task/asset/test_action`（Studio 演练/点测）：
```json
{"task":"…","id":"I_战斗按钮","gesture":"click|long_press|drag","candidate_index":1,
 "frame_id":12,"live":false,"real":false,"from":{…},"to":{…}}
```
响应（dry）：`{"ok":true,"real":false,"steps":[{"asset":"I_战斗按钮","mode":"self","rect":[…],"score":0.97},{"asset":"I_操作区","mode":"offset","real_rect":[…]}],"action_box":[…],"point":{"x":..,"y":..},"long_press_ms":3000}`；`real:true` 走既有 `/input/tap`（`app:3241-3269` 链路同源）：`{"ok":true,"real":true,"tap_ok":true,"click_point":{…}}`；链失败：`400 {"ok":false,"errors":["…chain…"]}`，不点击。
6) `POST /task/asset/validate_op_region`：请求 `{"task","id","op_region"}`；响应 `{"ok":true,"chain":[{"asset":"I_A","mode":"asset"},{"asset":"I_B","mode":"self"}],"depth":2}` 或 `400 {"errors":[{"path":"assets.json: items[3].op_region.asset","msg":"cycle: I_A -> I_B -> I_A"}]}`（供素材行对话框实时校验，服务端为唯一权威）。
7) `POST /task/save_flow`（`serve:6050`）成功响应追加：`{"sync":{"added":[],"updated":["group"],"removed":[]}}`。
8) `GET /task/config?task=`（`serve:9567`）：`params` 含派生行（带 `asset_id`）；响应追加 `"derived_keys":["group"]` 便于 UI 只读渲染。
9) `POST /task/config`（`serve:9593`）：`params` 中带 `asset_id` 且与现状不同的行 → 400 `{"errors":["params[i].asset_id 派生行只读：请改素材或「迁移」"]}`；请求只含用户行时正常保存后自动重算派生行。
10) `POST /task/migrate_ocr_params`（Studio）：请求 `{"task":"…","dry_run":true}`；响应 dry：`{"ok":true,"dry_run":true,"candidates":[{"param_key":"group","action":"create_asset","asset_id":"O_group","runtime_text":"御魂"}]}`；真跑：备份 `meta.json.bak`+`assets.json.bak`（全局库追加 `assets.json.bak.<ts>`），写 assets、删旧手写行、`sync` 生成派生行、失败全回滚；响应 `{"ok":true,"migrated":[…],"backup":"…"}`。flow/values.json 不动。
11) `GET /runner/tasks/<id>/config`（`serve:9484-9511`）/ `PUT`（9519-9562）：**契约不变**；派生行随 `params` 返回（Runner UI 据 `role=ocr_text && asset_id` 分组）。
12) `GET /task/lint`：新 T8 行 `code=T8`，`path=meta.json:params[i]`，msg 含素材 id 与字段差异。

## ⑥ Studio UI 流程

**S1 素材行（`asset_ui.js`）**：`KIND_LABELS` 加 `ocr_var: '使用端OCR文字配置项（ocr_var）'`（8-14）；`ASSET_MENU_ITEMS` 末尾追加 `{key:'text', label:'识别文字', action:'onEditText', manage:true}`（170-195；**只能追加**，且必须同步 `ui_pure_test.js` 的 `ASSET_MENU_LABELS` 精确数组断言 1319-1439，9→10 项）；ocr_var 行禁用「识别区」之外的 `region` 语义不变；行详情（520-539）追加 `识别文字「runtime_text」 · 操作区[链摘要]`；虚拟素材行（`virtual:true`）加「待迁移」badge，点「识别文字」提示先迁移。

**S2 识别文字对话框（app.js）**：字段=`runtime_text`（必填≤200）、`param_key`（新素材创建时自动生成 `p_<hash8>`，迁移后只读，提示「改键会使 Runner 已保存的值失效」）、`label`、`group`（写入派生行）、`roi`（可空=全屏，按钮「画识别区」复用画布）；保存=`POST /task/save_asset`（replace:true）或 `/task/asset/update`；保存后刷新素材库+配置项面板。

**S3 素材行「识别」= 全部候选（app.js）**：按钮→`GET /task/asset/candidates`（按 `testFramePlan` 3152-3220 选帧）→ `boxes`=全部候选（复用 `draw()` 420-512 多 box 渲染）→ `#result` 输出 `#0 score=0.973 [x,y,w,h]`…；模板/集合/OCR/ocr_var 统一；ocr_var 空 roi 时提示「全屏 OCR（较慢）」并显示 `domain=screen`。

**S4 测试动作（素材行设置：点测/拖测）**：演练=先取候选列表→逐候选调 `POST /task/asset/test_action`（dry）→逐个画候选真实识别区域+操作区域+目标点；真实=候选下拉 `#序号 score`（新 DOM `#candidate-select`，放结果区）→选中→`real:true` 调 test_action（沿用 `confirmAssetTestGestures` 3305-3316 二次确认）；集合测试/批量帧测试同样逐候选展开（`testAssetCollection` 4575-4688、`batchTestAssets` 4710-4768 改循环）。

**S5 操作区对话框（素材行设置「操作区」，新 `#op-region-dialog`）**：按用户描述的递归逐级三选一：
1. 打开时显示素材自身 `op_region` 当前值 + 面包屑 `素材1 ▸ 素材2 ▸ …`。
2. 三选一单选（仅末级节点可保存退出）：
   - 「当前素材操作区域」（self，默认；文案「= 现有操作区域/真实识别区域」）
   - 「相对偏移」（选中后切到画布绘制，S6）
   - 「引用素材库素材」（搜索框样式与现有 `createSearchPicker`（`asset_ui.js:397+`）一致，候选=可识别素材：template/collection/ocr_line/ocr_lines/ocr_var）
3. 选择引用素材 B 后，对话框内**递归展开 B 的 op_region 三选一**（同一 DOM 内追加一节，带缩进/面包屑），并提示「修改 B 的操作区会影响其它引用 B 的素材/动作」；每级可选 `pick`（`score`=评分最高 / `nearest`=离上一节点真实识别区域中心最近），仅引用级可设。
4. 任何一级都可回退（删除该级、把上一级改回 self/offset）；「确定」前调 `POST /task/asset/validate_op_region` 服务端校验环/深度/节点存在；成功后保存引用级素材 B 的**新 op_region**（注意：B 的配置是全局的）+ 当前素材 A 的 `op_region={mode:"asset",asset:B,pick}`（两次 update，同一对话框一次提交）。
5. 末端只有 self/offset 才能保存；`region` 素材不出现在可引用列表。

**S6 offset 绘制（工作台画布 `#screen`，`index.html:59`）**：进入绘制模式→画布先画「模拟识别区域」（默认取素材位置帧 ROI，可重画；蓝虚线）→再画「操作区域」（橙实线，尺寸=本次绘制尺寸）→实时显示 `offset=[dx,dy,w,h]`（dx=操作区.x−模拟识别区.x，dy 同理；w/h=绘制宽高）→可微调数字→确认写 `op_region={"mode":"offset","offset":[dx,dy,w,h]}`。`draw()` 增加两组 box + 标签；`#canvas-footer`（61-66）加模式提示与「取消/完成」按钮；绘制期间禁用其它选择模式（参考 `SELECTION_MODES` 2776/2810-2817）。

**S7 动作目标选择（`builder.js`，P0c task-7 已删除参数预设）**：删除完成项以 task-7 为准（`PARAM_ACTIONS` 36-40、`PARAM_GROUP` 35、`GROUP_ORDER` 参数项 31、`ACTION_TITLES` 两条 43-49、`actionCatalog/actionMetaOf/actionAvailable/paramActionContext/buildActionStep` 56-110 的参数分支、`pickParamForAction`/`addParamAction` 1837-1885、`renderPalette` 1624-1680 的参数组渲染、`prepareActionStep` 311-327）。本设计新增/保留：`chooseOcrAssetDialog`（141-297）重写为**素材库搜索选择**（复用 `createSearchPicker`），不再出现参数/操作区选项（D6）；动作目标生成 `find:[{kind:"ocr_text",sel:<素材 id>,text_expr:"param('<param_key>')"}]`（ocr_var 与普通 OCR 素材同写法；派生行的 key 可正常插入 param()）；「插入参数 ▾」（2455-2539）保留给表达式字段；定位器 `region` 编辑器 `renderRegionEditor`（2797 起）只对**旧 flow 已带 region 的步骤**显示（高级区，不生成新 region）。

**S8 Studio 配置项面板（app.js `#task-config-dialog`，index.html:484-513；`renderTaskConfigForm` 1774 / `loadTaskConfig` 1855 / `openTaskConfigDialog` 1913）**：存在派生行时渲染只读卡片：key/type/default 禁用 + badge「来自素材 <asset_id>」+「打开素材」链接；`group/description` 可编辑；保存 POST 时仅提交用户行+派生行原样（服务端 B23 保护）。检测到旧 `role=ocr_text` 参数（无 `asset_id`）时显示横幅「检测到 N 个旧参数（OCR 识别文字），可迁移为素材」→ 按钮 `POST /task/migrate_ocr_params`（dry 预览→确认）→ 成功后重新加载素材库与参数面板。

**S9 D4 两状态删除（app.js）**：删 `actionValidity`（3045-3088）及其调用（testAssetClick 3362、3394、testAssetCollection 5506 附近）；删 `action_source` 出站（`candidateActionPayload` 2761-2767、5073-5079、`updateAssetActionRoi` 5430-5458）；`collectionAssetRows` 4203-4205 去掉字段；画布拖操作区仍写 `action_shape`（默认 self 数据源）；文案改为「操作区来源：当前素材操作区 / 相对偏移 / 引用素材的链」。

**S10 素材行菜单回调接线**：`app.js` 三处素材库（4463-4485、5915+、6158-6168）补 `onEditText`/op-region 新回调；`refreshTaskAssets`（4234）与全局库刷新同步。

## ⑦ Runner UI 流程

1. Runner 配置弹窗（`runner.html:523-539` + `runner.js:2344-2480`）不改结构；`runnerParamRoleText`（483-486）已识别 `role=ocr_text`，新增：`param.asset_id` 存在时把该行放入独立分组「使用端OCR文字配置项」（`runnerParamGroups` 466 增前置分组），行标题用 `param.label`（=素材展示名），副标题「素材 <asset_id> · 当前默认「runtime_text」」。
2. 输入控件仍为文本输入（`role=ocr_text` 提示「识别文字」）；「使用」开关/留空=不注入语义不变（`runnerParamUseState` 523、`runnerParamCollectUse` 559）。
3. 保存 `PUT /runner/tasks/<id>/config` body `{values:{group:"结界"}}` 不变；写入 `config/values.json tasks.<task>.values.group`（runner_config.rs:141-157 RMW）；错误按 key 回显（384 已有）。
4. 临时运行/调度 params（runner.js 889-961）不受影响；`/runner/run` 合并 values 逻辑零改动。
5. 若安装包内的任务无 flow 引用任何 ocr_var，则该分节不出现（派生行只按 flow 引用生成）。

## ⑧ 迁移与兼容

### 8.1 同步函数（C1′ 核心）

```rust
// crates/oas-task/src/ocr_var.rs（新文件，或 lib.rs 追加；纯函数，I/O 由调用方）
pub struct SyncReport { pub added: Vec<String>, pub updated: Vec<String>, pub removed: Vec<String> }
pub fn sync_ocr_var_params(meta: &mut Meta, assets: &AssetsFile, flow: &Flow) -> Result<SyncReport, String>
// 1. referenced = flow.referenced_assets() ∩ {items|kind==ocr_var}.id
// 2. expected: 每个引用素材 -> ParamDecl{key:param_key,type:"string",default:runtime_text,
//    role:"ocr_text",asset_id:Some(id),label:asset.label.or(id),
//    group/description: 沿用既有 asset_id 行的值}；校验 param_key 唯一/合法/runtime_text 合法
// 3. diff：新增缺失行；更新 type/role/default/label（group/description 保留）；
//    删除 asset_id 行中来源素材已不再被 flow 引用的（不碰无 asset_id 的用户行）
// 4. 冲突（派生 key 与用户行 key 重复 / 派生 asset 已不存在）= Err，不写
```
- **触发点（全部后端，不依赖前端）**：① `save_flow` 成功后（serve B21）；② 保存/更新/删除/重命名 `ocr_var` 素材成功后，扫描 `tasks_dir` 所有任务，命中 flow 引用者即同步（serve B20）；③ `POST /task/config` 保存用户参数后（B23）；④ 迁移 API（B10/B24）。`oasd task lint`/发布只做 T8 一致性告警，不自动写。
- 写盘：`TaskBundle::write_meta`（新增）统一 `.bak` 备份 + 原子写（沿用 `write_atomic` 风格）；一次同步失败=API 400/500 且不改 meta。

### 8.2 旧数据读时合成（不写用户数据）
- `meta.params` 中 `role=ocr_text`、无 `asset_id` 的参数 → `GET /task/assets` 返回 `virtual_items`（id=`O_<sanitized key>` 且不与现有 id 冲突；`kind:"ocr_var"`、`virtual:true`、`runtime_text`=default（非 string 则跳过并显示 issue）、`param_key`=key、`roi:null`）；Studio 素材库渲染为只读行。
- 运行链路对旧任务**零改动**：物理参数仍在 meta.params，`param()`/注入/values/Runner 原样工作。
- 虚拟条目不可编辑/不可保存；编辑入口提示「先迁移」。

### 8.3 迁移步骤（显式用户操作，默认 dry-run）
1. `POST /task/migrate_ocr_params {"task","dry_run":true}` → 预览清单；2. 确认 → 真跑：备份 meta/assets → 为每个 `role=ocr_text` 且 default 为 string 的物理参数创建 `ocr_var` 素材（`roi:null`、`runtime_text=default`、`param_key=旧 key`、`label=label`、`positions:[]`）→ 从 meta.params 删除这些手写行 → 调 `sync_ocr_var_params` 生成派生行（同 key）→ 写盘；任何失败整体回滚（meta/assets 双还原）。3. flow/`text_expr`/`values.json` **全部不变**；旧 Runner 的 values 天然继续可用。4. 非 string/缺 default 的旧参数不自动迁移，报告 skipped 交用户处理。

### 8.4 兼容矩阵

| 场景 | 结果 |
|---|---|
| 旧 assets.json（含 `action_source`）→ 新引擎 | serde 兼容字段容忍读取（P0b）；T5 不再校验两状态；新保存不再产生该字段数据（保留默认值/跳过序列化由 P0b 定），仅在用户主动保存时变化，不自动重写 |
| 旧 meta.params（role=ocr_text）→ 新引擎 | 读时虚拟素材仅展示；运行/Runner 零变化；迁移可选 |
| 旧 values.json → 新引擎 | 零改动（key=param_key=旧 key） |
| 旧 flow（`region`/`action_source`/`text_expr`）→ 新引擎 | flow 解析零改动；`region` 四模式保留、优先级=flow region > 素材 op_region；新 UI 不生成 |
| 旧 package → 新引擎 | 安装/运行不变（无 ocr_var）；旧 assets 字段容忍 |
| 新 package（含 ocr_var）→ 旧 Runner | `min_engine=ENGINE_VERSION`（package.rs:1476 + package_cmd:1204-1213）拒绝安装；若同版本开发包误装，旧引擎 T5 kind 白名单/运行期 ocr_var 无 ROI 加载会响亮失败（非静默）；**op_region 被旧引擎静默忽略的风险见 §风险拍板 3** |
| 新引擎 → 旧任务保存 | 保存 flow/素材触发同步，只改`.bak`+对应文件；用户数据不迁移不自动重写 |
| Phase 2（B/A） | 虚拟参数（无 meta.params 写入、lint/注入/values 从 assets 派生）与 flow 免 `text_expr` 新写法列为下一阶段；本轮不实现 |

### 8.5 旧 Runner 门禁与 flow_ops 新标记（本轮口径）

- **flow 格式零变化**：ocr_var 仍写成 `{"kind":"ocr_text","sel":"<素材id>","text_expr":"param('<param_key>')"}`，`SUPPORTED_FLOW_OPS`（package.rs:81-95）**不新增标记**，`derive_flow_ops`（606-627）零改动；新旧引擎都能解析同一 flow。
- **门禁 1（主）**：publish 写入 `manifest.engine.min_engine = ENGINE_VERSION`（package.rs:1476），旧 Runner 安装时 `engine_too_old`（package_cmd:1204-1213）拒绝——版本递增即可靠。
- **门禁 2（task-local 素材模式）**：旧引擎安装期 lint 的 T5 kind 白名单（oas-task/lib.rs:2584-2593）会拒绝含 `ocr_var` 的 `task/assets.json`；全局素材快照模式 T5 覆盖不到 store（package_cmd:776-799），由门禁 1 兜底。
- **门禁 3（可选，拍板项 3）**：若要求「同版本号也能拒绝」，在 publish/install 共用的 derived 函数里按**包内 assets 快照**追加合成标记（`asset.ocr_var`：快照含 ocr_var；`asset.op_region`：快照含非 self 的 op_region），旧 Runner 的 `declared != recomputed`（package_cmd:1393-1415）会 400 拒绝；新 Runner 需同步扩展 `SUPPORTED_FLOW_OPS` 与重算函数。默认不加（本轮），风险见 §风险 5。

### 8.6 用户数据边界（实现任务纪律）

- 工作区 `assets/`、`frames/`、`tasks/`、`scheduler.json` 是用户未提交数据：实现/测试任务**不得覆盖、不得提交**；E2E 用临时目录或 `#[cfg(test)]` 临时根（参考 `serve.rs` 测试的 `td` 临时目录模式）。
- 同步/迁移只在**用户主动保存/确认**时写盘；不得在任务打开、lint、发布预览时自动迁移用户数据。

## ⑨ 测试计划

- **Rust `oas-task`**：KINDS/T5（ocr_var 必填、roi 可空、禁 file/polygon/ocr_mode/action_source 容忍）；`OpRegion` 解析/序列化（3 模式、offset 4 值、pick 值域）；环/深度/末端/节点 kind 校验；`sync_ocr_var_params`（新增/更新/删除派生行、保留 group、不碰用户行、key 冲突报错、空 flow、两任务同素材）；迁移函数（dry/real/回滚/skipped）；T8（一致=绿；`runtime_text` 与派生行不一致=lint issue，级别见拍板项 4）；`write_meta` 原子性。
- **Rust `oas-runtime`**：ocr_var 读取（param 覆盖/runtime_text 默认/全屏 ROI/无候选 miss/低于 threshold）；`resolve_op_region_chain`（self 零变化回归、offset 相对真实识别区域、asset 单候选、nearest 最近参考点=上一节点中心、多级链、环 visited、深度>16、中间节点 miss=Err 不点击、region 节点拒绝、flow region 优先级、模板/集合/OCR/ocr_var 各作节点）；`click_box_for` 回归（保留 action_shape 平移）。
- **Rust `oasd`（serve）**：新路由 method/path/400/404；候选 API 与 runtime 同源一致性；`test_action` dry/real（409 守卫、不点击语义）；sync 触发点（save_flow/save_asset/update/delete/rename）、响应 `synced`；`/task/config` 派生行保护；迁移 API 备份/回滚；虚拟 items；T8 出现在 `/task/lint`；package install 含 ocr_var 的兼容用例（本引擎可装可跑）。
- **UI pure test（`apps/oasd/ui/ui_pure_test.js`）**：新增纯函数登记 `PURE_FUNCTIONS`（141-189）：`opRegionChainSpec/opRegionSummary/candidateLabelText/offsetFromDraw/runtimeTextValid`；断言：菜单项顺序（末尾追加「识别文字」）、kind label、候选格式化（#i score）、offset 计算、链校验（环/深度/末端）、`action_source` 相关断言删除（335-387、4655-4930 等审计列表）。基线（2026-11-06 实测）：`ui_pure_test: 2214 checks`、`dom_id_audit: 164 ids/0 dead`、`runner_ui_audit: 158 ids/0 dead/6 nav` —— **只增不减**；新增 DOM id 同步审计。
- **E2E**：Studio 新建 ocr_var→识别文字→候选识别（全屏/ROI）→操作区 self/offset（画布绘制）/递归 asset（含 nearest）→动作库选目标生成步骤→保存 flow 触发派生参数→配置项面板只读派生行；Runner 改识别文字→`values.json` 落盘→运行实时/回放读取新值；迁移旧任务（role=ocr_text）后 flow 不变可跑；旧 assets/旧 flow 回归；真实设备点测/长按（有设备时）。
- **基线命令**：`cargo test --workspace`（`docs/START-HERE.md:32` 基线 **841 passed / 0 failed**，23 suites：perceive 11 / flow 78 / runtime 186 / task 122 / oasd 412；本实现只增）；`node apps/oasd/ui/ui_pure_test.js`；`cargo build -p oasd --features ocr`。

## ⑩ 分阶段任务拆分（供 Lead 直接建共享任务）

| # | 任务名 | 写作用域（文件/目录） | 依赖 | 验收标准 | 预计改动点 |
|---|---|---|---|---|---|
| T1 | 数据模型 + op_region 校验 + 派生同步 + T8/迁移核心 | `crates/oas-task/src/{lib.rs,meta.rs}`、新增 `crates/oas-task/src/ocr_var.rs` | P0b（两状态降级先合入，避免同文件冲突） | `cargo test -p oas-task` 全绿；B1-B12 单测覆盖（§⑨ Rust 第一条）；旧 assets/meta 反序列化测试通过 | KINDS 55；AssetDecl 286-348；OpRegion 新类型；load_runtime_assets 1301；save_asset_impl 1828；update_asset_meta 1699；assets_shape_issues 2555；lint_into_full 2315（T8）；sync/migrate 新文件；write_meta 新方法；ParamDecl 53-90 |
| T2 | 运行时：ocr_var 读取/候选枚举/操作区链解析 | `crates/oas-runtime/src/lib.rs`（+必要时 `humanize.rs` 不改） | T1、P0a（候选 helper 已在则复用） | `cargo test -p oas-runtime`；§⑨ 第二条全绿；既有 region/click 回归零变化 | AssetSet 350-384/520-564；ocr_read 2887；ocr_read_rect 2183；operation_candidates 1917；operation_region_for_asset 1995→公共链函数；region_asset_target 2097；click 分支 3603-3990；press 2340-2560；drag 2225-2330；region_log 650-659；`candidates_for_asset` 新公共 API |
| T3 | 后端 API + 同步触发 + 迁移/校验端点 | `apps/oasd/src/serve.rs`（路由/route_owner/新 handler）、`apps/oasd/src/task.rs`（如需） | T1,T2、P0a/P0b（serve.rs 段） | `cargo test -p oasd`；§⑤ 契约测试全绿；`GET/PUT /runner/tasks/<id>/config` 回归；`/task/save_flow`、素材 CRUD 触发 synced 断言 | 路由 1877-1903/route_owner 153-226；handle_save_asset 3265；save_assets 4529；asset_update 4814；delete/rename/collection 4331/4712/4757/1380；save_flow 6050；task_assets 5518；task_config 9567/9593；新 handler candidates/test_action/validate_op_region/migrate；B20-B24 |
| T4 | Studio 素材行 + 识别文字 + 候选识别 + 操作区递归对话框 + offset 画布 | `apps/oasd/ui/{app.js,asset_ui.js,index.html,style.css}`、`apps/oasd/ui/ui_pure_test.js`（本任务段） | T3、P0c | UI pure test 基线只增；浏览器手测 S1-S6/S10 全通过；`node apps/oasd/ui/ui_pure_test.js` 0 dead | asset_ui.js 8-14/170-195/212-247/520-539；app.js 420-512（draw）、3152-3220（帧）、2465（候选列表）、3329-3477（点测）、3479-3613（拖测）、3651-3821（OCR 测）、4186-4195、5430-5458、5482-5537、5685-5795（payload）、1774/1855/1913（配置项面板）；index.html 59/61-66/117/263/484-513；新 `#op-region-dialog`/`#candidate-select`/绘制模式 |
| T5 | Studio 动作库目标选择 + 旧 region 编辑器收口 | `apps/oasd/ui/builder.js`、`style.css`、`ui_pure_test.js`（本任务段） | T4（同一测试文件顺序写）、P0c | 动作库无「参数」组/预设；新步骤只选素材目标、无操作区控件；旧 flow 步骤可编辑；UI pure test 更新且只增 | builder.js 4-30（OP_META/GROUP_ORDER）、56-110（目录/预设残留）、141-327（目标选择对话框/准备逻辑）、1624-1680（调色板）、1837-1885（旧参数选择）、2455-2539（插入参数）、2797+（region 编辑器保留旧步骤）、4179+（参数面板联动）；style.css 990-1079 |
| T6 | Runner UI：识别文字分节 | `apps/oasd/ui/{runner.js,runner.html,runner.css}`、`ui_pure_test.js`（本任务段） | T3（可与 T4/T5 并行，测试文件串行） | Runner 配置弹窗按素材分节显示/保存；`values.json` 旧格式兼容；runner_ui_audit 只增；无 Rust 改动 | runner.js 466-486（分组/角色）、2344-2480（渲染/加载）、2203-2208（badge）；runner.html 523-539 |
| T7 | 迁移与兼容落地（旧数据/旧包/旧 flow 回归 + 迁移入口按钮） | `apps/oasd/ui/app.js`（迁移横幅段）、`apps/oasd/src/serve.rs`（迁移 handler 复核）、测试 fixtures `apps/oasd/src/serve.rs` 测试模块（只加） | T3,T4 | 旧任务迁移后 flow/values 不变且可跑；旧 assets/旧 flow/旧 package 安装回归通过；用户数据不自动改写（git status 验证） | app.js S8 横幅/参数面板；serve 迁移 handler + 测试；全局库/任务库两模式各一例（global assets 与 task-local） |
| T8 | 测试补齐 + 文档回填 | `crates/oas-task/src/*`（测试段）、`crates/oas-runtime/src/lib.rs`（测试段）、`apps/oasd/src/serve.rs`（测试段）、`docs/{PARAM-ASSET-IMPLEMENTATION.md,OPERATION-REGION-DESIGN.md,PARAM-ASSET-REDESIGN.md,ENGINE-DESIGN.md}` | T1-T7 | workspace 测试 0 失败且总数≥基线；UI 2214+checks/164+ids/0 dead/6 nav；文档记录实现行号与变更 | 各 crate 测试模块；文档状态行/行号；`START-HERE.md`/`RESUME*.md` seq 记录（Lead 决定是否改） |

写作用域提示：T1/T2/T3 后端按 crate/文件互斥；T4/T5/T6 均触碰 `ui_pure_test.js` —— **同一 writer 串行**（T4→T5→T6 顺序执行）或由 T4 owner 统一收口测试文件；T7 只动 app.js 迁移段 + serve 测试段（与 T4 的 app.js 段需错开或 T7 依赖 T4 完成后开工，建议 T7 放 T4 之后）。
**与已派发 P0 的关系**：P0a（task-5 候选 API）、P0b（task-6 两状态降级）、P0c（task-7 UI 去参数/删两状态）先合入；T2/T3 的候选枚举与 `action_source` 容忍直接继承 P0 结果（不重复实现），T4/T5 在 P0c 之后开工；若 P0 契约与本文有出入，以 Lead 最终裁决为准并在实现任务中记录。

## 风险 / 需 Lead 拍板

**风险**
1. **多任务连锁写盘**：ocr_var 是全局素材，一次保存会改多个任务的 meta.json（T3/B20）。需 `.bak`+原子写+失败回滚；serve 是否单线程处理需确认（若并发，需加写锁，参考 `runner_config.rs:134 WRITE_LOCK`）。
2. **双源一致性**：素材 `runtime_text` vs 派生行 `default` 只能靠「单一同步函数 + 保存触发 + T8」保证；绕过 Studio 手改 flow/assets 会产生漂移（T8 拦截，不自动修）。
3. **同步函数扫描成本**：素材保存时扫描全部任务（任务数少，O(n) 解析 flow，可接受）；若未来任务数百，需按引用索引优化（Phase 2）。
4. **全屏 OCR 性能**：`roi=null` 的 ocr_var 每次识别跑全帧 det+rec（0.3~1s），编辑器候选与运行都会变慢；候选 API 有 `live` 时应提示。
5. **旧 Runner 静默忽略 `op_region`**：同版本开发包/未 bump 版本时旧引擎会按「真实识别区域」点击（动作错误但无异常）；min_engine 版本门禁只在版本递增时可靠 → 拍板项 3。
6. **`action_source` 降级而非删除**：旧 Studio 页面若与旧 serve 混跑会 400（同版本发布可控）；旧 Runner 是否读该字段未审计（仓外），但引擎不读已确认（P0b 保留字段读容忍，风险降低）。
7. **ocr_var 强制 det+rec**：单行 rec 的 12~20ms 快路径不再适用（候选/真实区域需要 bbox）；单行 ocr_var 仍可用 roi 收紧成本。
8. **迁移不可逆性**：迁移写盘后回滚依赖 `.bak`；建议默认 dry-run + 明确确认文案。
9. **`region` 素材作链节点被禁止**可能不符合用户对「素材库所有素材」的直觉（用户原话"素材库所有素材（可搜索/候选）"）；实现按「可识别素材」限制，需 UI 文案解释。

**需 Lead 拍板**
1. **op_region.offset 基准**：D2 定「相对真实识别区域左上角」（用户原话"识别区域"可指存储 roi）——确认取**真实识别区域**（推荐，与 D2 一致）。
2. **nearest 参考点**：D2 定「上一节点真实识别区域中心」；根节点（动作目标素材）多候选时取最高分（OCR 最高置信/模板最佳）——确认。
3. **旧 Runner 安装门禁**：是否额外加合成 flow_ops 标记（`asset.ocr_var`/`asset.op_region`，在 publish 时按包内 assets 快照注入，install 用同一函数重算；旧 Runner 因 declared≠recomputed 拒绝安装）。推荐加（防静默错点）；不加则依赖 min_engine + T5 + 运行期失败。
4. **T8 级别**：lint 中做**错误**（发布阻断）还是**警告**（仅提示，靠保存同步自愈）？默认建议错误。
5. **迁移入口**：仅 Studio 参数面板按钮，还是同时提供 CLI（如 `oasd task migrate-ocr-params --tasks-dir --dry-run`）供数据 owner 批量？建议两者，CLI 可选。
6. **`param_key` 何时可改**：新素材自动生成 `p_<hash8>`，迁移=旧 key；建议创建后只读（改键会丢 Runner 已存值）。确认。
7. **派生行可编辑字段**：按本文=仅 `group/description`；`label` 跟随素材；确认。
8. **命名**：`ocr_var` vs `ocr_runtime_text`（用户允许更贴切名）；本文用 `ocr_var`。
9. **D6 收口范围**：老 flow 中由旧「参数预设」生成的标准步骤（click_target/long_press/wait_until）**保留可编辑**，只是不再提供预设入口——确认。
10. **Phase 2 边界**：B/A（虚拟参数、flow 免 text_expr 新写法、values asset_values）本轮不做，是否需要在本文档末尾追加 Phase 2 草案（可另行任务）。

## ★ Lead 裁决（2026-11-06 12:5x，本轮实现按此执行）

| # | 拍板项 | 裁决 |
|---|---|---|
| 1 | offset 基准 | ✅ 取**真实识别区域**左上角（运行时匹配/识别 bbox；ocr_var=命中行 bbox），与事项6"操作区随真实识别区域位移"一致。 |
| 2 | nearest 参考点 | ✅ 上一节点**真实识别区域中心**；根节点（动作目标）多候选取最高分（模板最佳/OCR 最高置信）；平分比 score。 |
| 3 | 旧 Runner 门禁 | ✅ 本轮**不加**合成 flow_ops 标记；依赖 `min_engine` 版本门禁 + T5 kind 白名单 + 运行期 ocr_var 失败（响亮）。列为发布前检查项（Phase 2/发布清单）。 |
| 4 | T8 级别 | ✅ **lint 错误**（阻断发布）；保存触发同步自愈；漂移报错要求重新保存/迁移，不自动改。 |
| 5 | 迁移入口 | ✅ 本轮仅 Studio 参数面板按钮 + `POST /task/migrate_ocr_params`；CLI 后续再加。 |
| 6 | param_key 可改 | ✅ 创建/迁移后**只读**；UI 提示改键会丢 Runner 已存值。 |
| 7 | 派生行可编辑字段 | ✅ 仅 `group/description` 可编辑；`label` 跟随素材；其余只读。 |
| 8 | 命名 | ✅ `kind:"ocr_var"`；UI 全名「使用端OCR文字配置项」，行内短标「识别文字配置」。 |
| 9 | D6 收口范围 | ✅ 老 flow 中旧参数预设生成的标准步骤保留可编辑（click_target/long_press/wait_until），只是不再提供参数预设入口。 |
| 10 | Phase 2 | ✅ 本轮不做；§0.1 已记录。 |
| 11 | **风险9 修正**（`region` 素材作链节点） | ✅ **允许 `region` 素材作为链的末端固定区域节点**：其 `roi` 直接作为操作区域，不参与识别/候选 pick、不能再配置 `op_region`；可识别素材（template/collection/ocr_line/ocr_lines/ocr_var）作普通节点。UI 搜索框列出全部素材并标注类型，文案说明。 |
| 12 | 候选 API 文字来源 | Studio 侧：派生参数 default > `runtime_text`；运行侧：注入参数（values 覆盖）> `runtime_text`。抽取同一 expected-text helper，禁止两套口径。 |
| 13 | 链校验权威 | `POST /task/asset/validate_op_region` 服务端为唯一权威；UI 实时校验只做提示。 |

## ★★ Lead 简化令（用户 12:5x，覆盖本文旧兼容/迁移相关条目）

> 用户原话：**"不需要兼容旧参数，我们是全新开发，不需要兼容旧配置，如有冲突可以删除旧配置，当前配置都是测试配置，并不重要"**。
> 据此（优先级高于本文其它节）：
> 1. **取消旧参数/旧配置/旧包兼容与迁移交付**：不做 `virtual_items`、不做 `POST /task/migrate_ocr_params`、不做旧 values/meta/flow 兼容保证；旧测试任务可随意重建或删除（B-T1 的 `migrate_params_to_ocr_vars` 纯函数保留但不暴露 API）。
> 2. **旧 Runner 兼容不需要**：新包默认新 Runner；`action_source` deprecated 字段只需"旧 JSON 读得动"，不再为此加保护性测试。
> 3. **保留**：C1′ 派生参数同步（ocr_var 仍是 UI 唯一真相，`meta.params` 派生行由单一 sync 函数维护，运行链路继续 `param()`/values.json——这是 Phase 1 内部桥，不是"旧配置兼容"）；`test_action` / `validate_op_region` / 候选 API / op_region 保存 / 识别文字对话框。
> 4. **测试配置处理**：`tasks/切换预设御魂` 等旧 params/flow 不作为验收对象；完成新 UI 后用新模型重建一份测试任务做 E2E 即可（用户数据仍不入 git、不自动改写）。
> 5. Phase 2（彻底去 `meta.params`、虚拟参数、flow 免 text_expr）仍列为后续，不在本轮。

## 实现任务映射（Lead 按 §⑩ 建看板任务；B-T3 已按简化令删 virtual/migrate 端点）

- **B-T1** = §⑩ T1（oas-task：模型/op_region/同步/T8/迁移核心）→ backend-dev
- **B-T2** = §⑩ T2（oas-runtime：ocr_var 读取/链解析/候选 helper）→ backend-dev（B-T1 后）
- **B-T3** = §⑩ T3（serve：API+同步触发+迁移/校验端点）→ backend-dev（B-T2 后）
- **U-T4** = §⑩ T4（Studio 素材行/识别文字/候选/操作区递归/offset 画布）→ ui-dev（P0c + B-T3 后）
- **U-T5** = §⑩ T5（builder 目标选择/旧 region 收口）→ ui-dev（U-T4 后）
- **U-T6** = §⑩ T6（Runner 识别文字分节）→ ui-dev（B-T3 后，可与 U-T4 并行但 ui_pure_test.js 串行）
- **U-T7** = §⑩ T7（迁移入口/兼容回归）→ ui-dev/backend-dev（U-T4 后）
- **D-T8** = §⑩ T8（测试补齐+文档回填）→ Lead + doc-syncer

## 实现结果（Lead 验收，2026-11-06 13:2x）

> 本文设计已按 Lead 裁决 13 条 + 用户 12:5x 简化令实现完毕；实现记录 `ENGINE §22.64`，现行操作区口径 `OPERATION-REGION-DESIGN §8`。台账/断点由 Lead 在 `PENDING-LEDGER`/`RESUME*` 回填。下列为与设计稿的**偏差**与**最终证据**（设计正文保留，作为历史方案记录）。

### A. 与设计稿的偏差 / 未实现项（以简化令与实装为准）

| 设计条目 | 实装状态 |
|---|---|
| §⑤-1 `virtual_items`（旧 role=ocr_text 参数合成虚拟素材） | **未做**（用户简化令取消旧兼容；虚拟素材仅留前端防御分支，后端不返回） |
| §⑤-10 `POST /task/migrate_ocr_params`（旧参数迁移） | **未暴露**：迁移纯函数 `migrate_params_to_ocr_vars` 保留在 `crates/oas-task/src/ocr_var.rs:276`（有 dry/real/回滚单测），无 API、无 UI |
| §⑤-5 `test_action` 的 gesture | 实装仅 **click / long_press**；**drag 明确 400**（`apps/oasd/src/serve.rs:5025-5031`），素材行「拖测」仍走旧前端链路 |
| §⑥ S4「素材行所有测试动作」 | 点测/识别/集合测试已统一候选；**素材行无「长按测试」入口**（API 支持），**拖测未接候选语义** |
| §⑥ S2「识别文字对话框：按钮『画识别区』复用画布」 | **未接画布**：`roi` 为 4 整数数字输入 + 「全屏 OCR」勾选（`app.js:3981-3989,4004-4017`） |
| §⑥ S8「配置项面板派生行只读卡片（来自素材 badge / 打开素材 / 迁移横幅）」 | **只读卡片未做**；仅保证派生行只读+保存时原样回传（前端 `app.js:1134-1188`，后端 400 保护 `serve.rs:11799-11820`） |
| §⑤-4 候选 API 追加 `op_rect`/链预览 | **未做**：候选只返回 `action_box`（该候选的操作区）；链几何仅在 `test_action`/validate 可见 |
| §⑧ 迁移/旧数据/旧包兼容交付 | **取消**（用户简化令）：不做 `virtual_items`、不做迁移 API、不保证旧 values/meta/flow；旧测试任务不作为验收对象 |
| §8.5 门禁 3（拍板项 3）合成 flow_ops 标记（`asset.ocr_var`/`asset.op_region`） | **未加**（Lead 裁决 3）：依赖 `min_engine` + T5 kind 白名单 + 运行期响亮失败，列为发布前检查项 |
| §⑨ E2E 的迁移/旧包回归 | 不做旧兼容；改为**新模型 E2E**（见 B） |
| Phase 2（去 `meta.params` / 虚拟参数 / `values.json.asset_values` / flow 免 `text_expr`） | **未做**，后置 |

### B. 最终验证（Lead 实测）

- `cargo test --workspace` = **904 passed / 0 failed**（23 suites；起 841）：**oas-task 144**、**oas-runtime 203**、**oasd 436**（分别 122/186/412 → +22/+17/+24）；`cargo build -p oasd --features ocr` exit 0。
- UI pure test = **2511 checks / 194 ids / 0 dead / 6 nav**（起 2214/164/158/0/6）；`runner_ui_audit` **158 ids / 0 dead**。
- E2E `_verify/seq124-e2e-20261106/e2e-summary.json` = **27 total / 26 passed / 0 failed / 1 skipped**，exit 0；SKIP=`test_action_real_click`（无 ADB；real 路径由 serve 单测 tap seam 覆盖）。
- 未 tag / 未 release；代码=HEAD `c33a822` 之后全部未提交改动。

### C. 证据路径

- 用户权威要求原文：`docs/PARAM-ASSET-REDESIGN.md §1b`；设计+裁决：本文 §0/§⑤/「★ Lead 裁决」13 条/「★★ Lead 简化令」。
- 只读审计：`D:\OAS-dev\_verify\paramasset-audit-20261106\{01-region-two-states-audit,02-param-action-audit,03-asset-candidates-measure-audit}.md`。
- E2E：`D:\OAS-dev\_verify\seq124-e2e-20261106\`（`README.md`/`e2e-summary.json`/`lead-run3.log`/`fixtures/`）。
- 实现记录：`docs/ENGINE-DESIGN.md §22.64`、`docs/OPERATION-REGION-DESIGN.md §8`。

### D. 未覆盖 / 后续（与 ENGINE §22.64.8 同一口径）

1. `test_action` 补 drag 手势（当前 400）并把素材行拖测统一到候选语义。
2. 素材行「长按测试」入口（API 已支持）。
3. 识别文字对话框 ROI 接画布绘制（复用工作台画布/偏移绘制模式）。
4. S8 配置项面板派生行只读卡片 + 「来自素材」链接（旧参数迁移横幅已随简化令取消）。
5. 候选 API/画布补 `op_rect` 链几何预览。
6. 全屏 OCR 性能提示与实测（候选/演练入口）。
7. 真机 E2E（点击/长按落点、ADB 设备）与同版本旧 Runner 的发布门禁检查项。
8. Phase 2（去 `meta.params`、flow 免 `text_expr`、`values.json.asset_values`）与旧数据迁移。

