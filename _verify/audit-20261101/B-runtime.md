# B-runtime 审计报告（runtime / task / server）

- 审计日期：2026-11-01 21:34–21:50（Asia/Shanghai）
- 仓库：`D:\OAS-dev\oas-core`，HEAD `983c740`（分支 `dev`）
- 范围：16:00 至今 runtime/task/server 相关实现；重点 `crates/oas-runtime/src/lib.rs`、`crates/oas-task/src/lib.rs`、`apps/oasd/src/serve.rs`，必要时对照 `apps/oasd/ui/app.js`
- 方式：只读代码审查 + 只读定向/全量 `cargo test` + 仓库外临时 Rust probe（不修改仓库、不提交、不重启服务）
- 对照 session：`_verify/n44/session-current-both-full.txt` 中 11-01 18:40:38、19:12:41、19:15:29、19:50:40、19:54:30、20:10:00 等用户/助手消息
- 测试基线：`cargo test --workspace` = **361 passed / 0 failed**（2026-11-01 21:37 实测；日志 `_verify/audit-20261101/B-cargo-test.log`）

> 说明：PowerShell 把 cargo 写到 stderr 的 warning 当成 `NativeCommandError`，所以整条管道退出码为 1；逐 crate 的 `test result: ok` 均为 0 failed。`cargo`/`rustc` 没有修改任何跟踪文件。

---

## 0. 结论摘要

`063c329` 对 actionValidity、runtime 斜边、template 平移、collection live、自交校验的修复**方向正确**，其中 P0-3/P0-4 的 runtime 修复有可复现证据。但“全部修复/361 全绿”不能等同于行为正确：至少还有 **6 个 P1 级确定性缺陷**、若干 P2/P3，且新增测试存在假绿。

| 编号 | 严重级 | 一句话 |
|---|---|---|
| B-01 | **P1** | UI 的模板点测/拖测/有效性仍用**未平移**的 action_shape；runtime 已平移，dry-run 与真实点击不一致 |
| B-02 | **P1** | `click_target` 的 OCR 路径完全忽略精确 action_shape：`ocr_lines` 用识别行框，`ocr_line` 只用 bbox，反选无效 |
| B-03 | **P1** | 精确形状采样失败被静默放过：全屏反选（有效区为空）仍会点击，probe 实测点击在排除区内且 run=Done |
| B-04 | **P1** | `kind=region` 的精确形状采样后又被 ±6px 抖动带出形状；probe 实测 1px 宽区域点落在区域外 |
| B-05 | **P1** | `validate_action_shape` 仍缺越界/零面积校验：越界 shape 能保存，runtime 把越界点 clamp 到屏幕边缘仍算成功 |
| B-06 | **P1** | P1-2 只修了 OCR 批量保存；非 OCR 批量保存仍硬编码 `{kind:'rect'}`，polygon/invert 丢失 |
| B-07 | P2 | `ActionShape::contains` 仍是旧的整数 `.max(1)` 错算法（当前未被调用，属潜伏 bug） |
| B-08 | P2 | `update_asset_meta` 对 region 拒绝更新 action_shape，UI「编辑操作区域」对 region 必失败 |
| B-09 | P2 | 批量测试的 region 分支只校验 `item.roi`，不校验 action_shape → 假绿 |
| B-10 | P2 | collection 的 action_shape 可保存但不校验、不被 runtime 加载；collection 测试的 action_box 未平移 |
| B-11 | P2 | `frame_for_test` 遇到非法 `frame_id` 静默回退当前帧；live 用缓存帧而非“每次强制抓取” |
| B-12 | P2/P3 | `Step::Appear` / `DismissPopup` 对 OCR 资产仍用 0.9 阈值，而不是 OCR 的 0.0 阈值 |
| B-13 | P3 | 模板 `testAssetClick` 未处理 match 失败（pass=false），会拿 (-1,-1) 当识别框继续算有效性/目标点 |
| B-14 | P3 | task delete backup 无测试，且 `assets.json` 不存在时 `images/` 不会被备份、flow/meta 不在备份内 |

已确认**未修**：P1-1 drag shape、P1-5 SelectionTool。

---

## 1. 已知修复的独立验证（不照抄 `docs/CODE-AUDIT-2026-11-01.md`）

### 1.1 P0-1 actionValidity（`apps/oasd/ui/app.js:1413-1454`）— 已修，主场景正确

- `fixed` 且非 invert、与识别框不重叠时返回 `valid:true`（1427-1434），文案为“独立操作区域（fixed，允许不重叠）”。
- `recognition` 且非 invert 时按 `rectsOverlap`/`polygonOverlapsRect` 判断（1424-1426、1435）。
- **残余问题**：invert 分支只用识别框四角判断“是否在排除区外”（1439-1445）。对凹多边形（U 形）或识别框跨过凹口时，四角可能全在排除区内而框的一部分实际在有效区，导致误报“无效/独立”。这是近似算法，不是本次 P0 的主场景，但属于 P2 残留。
- 无浏览器自动化测试覆盖；仅有代码路径。

### 1.2 P0-3 ClickShape 斜边（`crates/oas-runtime/src/lib.rs:75-100`）— runtime 已修，回归测试有效

- 现在用 `f64` 分母，不再 `.max(1)`（91-93）。我用仓库外 Python 复刻旧算法跑新测试的三个点：旧实现 `(120,120)=false, (200,180)=false`，新实现均 `true`，`(350,350)` 均 `false`。所以 `click_shape_tests::polygon_contains_handles_negative_slope_edges` **确实能抓住旧 bug**，不是假绿。
- **但** `crates/oas-task/src/lib.rs:140-166` 的 `ActionShape::contains` 仍是同一段旧整数 `.max(1)` 算法；probe 对三角形内部点 (120,120)/(200,180) 返回 false。该函数当前全仓无调用者（grep 仅定义），属潜伏 bug/B-07。

### 1.3 P0-4 template 平移（`crates/oas-runtime/src/lib.rs:1283-1336`）— runtime 已修，测试只覆盖 rect

- 新增 `translated_click_shape` 按 `matched - roi` 平移 rect/points；`adjust_plan_for_shape` 传入 `matched`。`click_box_for` 原本已平移 bbox，现在两条路径一致。
- `exact_shape_translates_when_the_template_moved` 能抓住旧实现：旧 `adjust` 用未平移 shape (10,8,20,20) 会把点重采样到旧位置，测试断言 [38,58) 会失败。
- **局限**：测试只测 rect，不测 polygon；且这是 runtime 行为，**UI dry-run 没有同步**（见 B-01）。

### 1.4 P1-3 collection live（`apps/oasd/src/serve.rs:3218`）— 已修

- `handle_collection_match` 已改用 `frame_for_test(&q)`，支持 `live=1`/`frame_id`，响应带 `frame_source`。
- 测试缺口：没有 serve 层测试覆盖 `live=1`、`frame_id` 或 collection 三态；只有代码路径。

### 1.5 P1-4 OCR 帧标签（`apps/oasd/ui/app.js:1516-1521`）— 部分修

- `testFrameLabel` 与 `testFrameQuery` 使用同一 `positions[0]`/`livePreview` 规则，请求和 UI 标签一致。
- **但没有用 server 返回的 `frame_source`**：若 server 因 `frame_id` 不存在/非法而回退，标签仍会显示请求里的位置帧；且单素材多位置只测第一个（batch 才全测）。属 P3 测试/显示缺口，不是主 bug。

### 1.6 P1-6 自交校验（`crates/oas-task/src/lib.rs:169-205`）— 只修了自交

- `validate_action_shape` 新增 `polygon_self_intersects`（195），`self_intersecting_action_polygon_is_rejected` 用 bowtie 验证，有效。
- **仍未校验**：rect/polygon 是否在帧内；polygon 是否零面积（3 点共线）。识别 polygon 路径有零面积和越界检查（1548-1607），action_shape 没有，见 B-05。

### 1.7 P1-2 批量保存 shape —— 只修了 OCR 路径

- OCR 批量保存（`app.js:2970`）使用 `c.actionShape || rect`，已修。
- 非 OCR 批量保存（`app.js:2988-2999`）仍写死 `{kind:'rect', roi:c.action}`，见 B-06。

### 1.8 P1-1 drag / P1-5 SelectionTool —— 确认未修

- P1-1：`testAssetDrag` 生成 step 仍是 `from:{kind:'region', roi:action}`（`app.js:1629`）；runtime `Step::Drag` 只接受 `DragTarget::Region/Point/OcrText`（`lib.rs:1503-1548`、`2697-2711`），没有 action_shape 语义。
- P1-5：`app.js:214` 与 `:254` 仍是 `actionPolyMode` / `polyMode` 两套；mousedown/mousemove/dblclick 仍各分叉（4488/4503/4575/4600/4661 等），无统一 SelectionTool。

---

## 2. 新发现（确定性缺陷）

### B-01（P1）UI 模板点测/拖测/有效性使用未平移 action_shape，与 runtime 不一致

- 位置：
  - `apps/oasd/ui/app.js:1456-1481` `randomPointInAction`：直接按 `it.action_shape` 的 absolute bbox 采样，没有 `matched - roi` 平移参数。
  - `apps/oasd/ui/app.js:1523-1595` `testAssetClick`：模板分支匹配后再次调用 `randomPointInAction(it)`（1547），仍用旧坐标。
  - `apps/oasd/ui/app.js:1601-1641` `testAssetDrag`：`from = randomPointInAction(it)`（1612），并用 `action` bbox 生成 `from`（1629）。
  - `apps/oasd/ui/app.js:1413-1454` `actionValidity`：`recognizedBox` 来自当前匹配（1548-1549），`shape.roi` 是保存时的绝对坐标，没平移就做重叠判断（1421-1435）。
- 复现路径：
  1. 素材在位置帧 A 保存，模板 roi=(22,9,43,39)，action_shape 也以 A 帧为绝对坐标；
  2. 在位置帧 B 打开同一素材，模板匹配到 (40,30)；
  3. 点「点测」：`recognizedBox` 在 (40,30)，但目标点仍落在 A 帧 action_shape 附近；runtime 实际点击会按 (40-22,30-9) 平移。
  4. 若 action_shape 与识别框不重叠，有效性还会误报“独立操作区域”。
- session 依据：18:40 第 3/4/5 条（实时/位置帧测试、点测/拖测随机点、识别框与操作区重叠判断）；19:50 助手声称「template/OCR 主点击路径」已接入 exact shape；这里的 UI dry-run 没有同步。
- 建议：把 `matched - roi` 传入 `randomPointInAction`/`assetActionBox`/`actionValidity`，或由 `/task/asset/match` 直接返回 `action_box` 的当前平移结果；生成 drag 时也使用平移后的 shape。
- 测试缺口：无浏览器断言“模板移动后点测坐标随匹配平移”。runtime 有 rect 平移测试，但 UI 层没有。

### B-02（P1）`click_target` 的 OCR 路径忽略 action_shape / invert，且编辑器会生成这种 step

- 位置：
  - `crates/oas-runtime/src/lib.rs:2315-2323` `Locator::OcrText` + `OcrRead::Lines`：直接用 `line.rect` 做 `template_click`，没有 `click_box_for`、没有 `click_shape`、没有 `adjust_plan_for_shape`。
  - `crates/oas-runtime/src/lib.rs:2357-2382` `OcrRead::Single`：`click_back(sel).unwrap_or(recognition)` 只用 bbox；不处理 polygon、不处理 `invert`、不调用 `adjust_plan_for_shape`。
  - 对照 `Step::Click` 的 OCR 路径（`lib.rs:2136-2144`）确实走 `click_box_for` + `adjust_plan_for_shape`。
  - `apps/oasd/ui/app.js:1643-1658` 对 OCR 素材生成 `click_target`（`kind:'ocr_text'`）；`ocr_lines`/derived 都会进入上述路径。
- 复现路径 A（反选被忽略）：
  1. OCR 素材 `O`，`action_source=fixed`，`action_shape={kind:'rect',roi:[0,0,64,48],invert:true}`（有效区为空）；
  2. flow：`click_target find [{kind:'ocr_text',sel:'O',text:'确认'}]`；
  3. 实测：run=Finished(Done)，点击 (17,12)——在排除区内；`Step::Click` 同形状会走全屏反选逻辑（虽然 B-03 也有问题，但至少路径不同）。
- 复现路径 B（固定操作区被忽略）：
  1. `ocr_lines` 素材识别行在 (10,10,20,10)，操作区固定在下方按钮 [5,40,60,20]；
  2. 点测显示按钮区域，生成 step 后 runtime 实际点识别行 (10..30,10..20)。
- session 依据：18:40 第 5 条明确区分「识别区域 / 操作区域」；19:12 确认 `action_source=fixed` 的 `action_shape` 是正式操作区域；19:50 助手声称 OCR 也走 exact shape。当前 `click_target` 与这些语义冲突。
- 建议：`Locator::OcrText` 匹配到行后统一调用 `click_box_for`/`translated_click_shape`（OCR 不翻译，直接用绝对 shape）+ `adjust_plan_for_shape`；`Lines` 分支至少支持 action_shape 覆盖，`invert` 时按全屏排除。
- 测试缺口：现有 `ocr_asset_action_roi_controls_click_landing` 用的是 `insert_ocr`（单行）且断言范围 [0,70]×[0,40] 很宽，见 §3；没有 polygon/invert、没有 `ocr_lines` 的 click_target 测试。

### B-03（P1）精确形状采样失败被静默吞掉；全屏反选仍会点击排除区

- 位置：
  - `crates/oas-runtime/src/lib.rs:111-138` `ClickShape::sample`：
    - invert 循环 4096 次都找不到 `!contains` 的点时返回 `None`（113-124）；
    - 非 invert 循环 4096 次失败后**无条件返回 bbox 中心**（134-137），不检查该点是否 `contains`。
  - `crates/oas-runtime/src/lib.rs:1326-1335` `adjust_plan_for_shape`：`shape.sample(...)` 返回 `None` 时直接返回原 `plan`（1334），不报错。
  - `perform_click` 只做 frame clamp（`lib.rs:996-998`），不校验 shape。
- 复现路径（全屏反选）：
  1. 模板或 OCR 素材 `action_shape={kind:'rect',roi:[0,0,64,48],invert:true}`；
  2. flow `click sel=...`；
  3. 仓库外 probe 实测：run=Finished(Done)，`audit point=(52,29)`，该点在排除 shape 内，有效区域为空。
  4. 用户期望：既然 `invert=true` 的有效区 = 屏幕 − shape，有效区为空时应 loud error，而不是点击排除区。
- 复现路径（共线多边形）：
  1. `action_shape={kind:'polygon',points:[[10,10],[20,10],[30,10]]}`（当前 `validate_action_shape` 接受，见 B-05）；
  2. `sample` 4096 次都 `contains=false`，fallback 返回 (20,10)；probe 确认 `sample=(20,10) contains=false`。
- session 依据：18:40 第 5 条 / 19:12 反选语义；19:50 助手称“点击落点若不在有效区域，会重新采样”，但失败路径没有 loud failure。
- 建议：`ClickShape::sample` 失败返回 `Result<Point,String>`（或至少让 `adjust_plan_for_shape` 返回 `Result`），全屏反选/退化多边形应报「exact shape has no sampleable point」；非 invert fallback 必须 `contains` 校验，失败再报错，不能返回 bbox 中心。
- 测试缺口：`polygon_and_invert_samples_are_effective` 只测大面积矩形（100×100 在 640×360）和成功路径，没有空有效区、退化多边形、凹多边形 fallback。

### B-04（P1/P2）region 精确形状采样后又被 jitter 带出形状

- 位置：
  - `crates/oas-runtime/src/lib.rs:2592-2598` `Locator::RegionRef`：`shape.sample(fw,fh)` 得到点 `p`，随后 `self.humanize.point_click(p, ...)`，没有把 `p` 限制回 shape。
  - `crates/oas-runtime/src/humanize.rs:328-335` `jitter_point`：默认 `jitter_px=6`（`humanize.rs:50`），对坐标做 ±6 抖动；`point_click` 的 miss 分支甚至会在 12~24px 外点击（287-295）。
  - `Step::Click` 的 `adjust_plan_for_shape` 只覆盖 template/OCR 主路径，不覆盖 `RegionRef`。
- 复现路径：
  1. region 素材 `R`，`action_shape={kind:'rect',roi:[10,20,1,20]}`（或任何宽/高 ≤ 12 的窄形状）；
  2. flow `click_target find [{kind:'region',sel:'R'}]`；
  3. 仓库外 probe 实测：run=Finished(Done)，`audit point=(5,25)`，`x=5` 不在 shape 的 x=10 列；
  4. 同线程的 `test/find region` 不走 `adjust_plan_for_shape`。
- session 依据：19:54–20:02 助手称 `region → exact ClickShape`；18:40 第 5 条要求落点在有效操作区。
- 建议：对 `RegionRef` 采样后也调用 `adjust_plan_for_shape`（或将 sample 结果作为 `plan` 直接 `perform_click`、不叠加 jitter；若要保留 jitter，则 jitter 后重新校验并在失败时重采样/报错）。
- 测试缺口：`saved_region_ref_clicks_inside_the_operation_region` 用 30×20 大矩形且断言含 4px 容差（`lib.rs:5944`），抓不到窄形状/边界抖动。

### B-05（P1）action_shape 校验仍缺越界与零面积；越界点被 clamp 后仍算成功

- 位置：
  - `crates/oas-task/src/lib.rs:169-205` `validate_action_shape`：rect 只检查 `x,y>=0; w,h>0`（175）；polygon 只检查非负（191）、自交（195）；**没有** `x+w<=frame_w`/`y+h<=frame_h`，也没有零面积检查。
  - 对照识别 polygon 的 `save_asset_impl`（`lib.rs:1548-1607`）：有 `area2 == 0`（1568）和 `max_x >= bw || max_y >= bh`（1599）。
  - `crates/oas-runtime/src/lib.rs:996-998` `perform_click` 把越界点 `clamp` 到 `w-1/h-1`，不报错。
- 复现路径（仓库外 probe，均已实测）：
  1. 64×48 frame，模板 `roi=[10,10,10,10]`，`action_shape={kind:'rect',roi:[100,100,10,10]}`；`save_asset` 返回 **Ok**（越界可写入）；
  2. flow `click sel`；run=Finished(Done)，`audit point=(63,47)`，`target=(105,105)`——点击被 clamp 到屏幕右下角；
  3. `action_shape={kind:'polygon',points:[[10,10],[20,10],[30,10]]}`（共线 3 点）也返回 Ok；其 `ClickShape::sample` 返回 (20,10) 且 `contains=false`（B-03）。
- session 依据：P1-6 审计项本身写了“polygon/rect 是否越界”；19:12 的 schema 收口要求 action_shape 是唯一操作几何；越界数据会在 runtime 变成错误位置的点击。
- 建议：`validate_action_shape` 增加 `design_resolution`（或调用方帧尺寸）参数，做 rect/polygon bbox 越界校验；polygon 增加零面积（shoelace）校验；加载时 `assets_shape_issues` 同样校验。runtime 对越界 shape 应 loud error，不要 clamp。
- 测试缺口：`template_requires_frame_roi_and_in_frame_bounds` 只测模板 roi，不测 action_shape；`fixed_polygon_action_shape_is_persisted...` 只测合法 shape；无越界/共线用例。

### B-06（P1）P1-2 只修了 OCR 批量保存；非 OCR 批量保存仍丢失 polygon/invert

- 位置：`apps/oasd/ui/app.js:2988-2999`（`saveAssetUnified` 的非 OCR 分支）：
  ```js
  if (c.kind !== 'region' && c.action) {
    payload.action_shape = { kind: 'rect', roi: [c.action.x, c.action.y, c.action.w, c.action.h] };
    payload.action_source = 'fixed';
  }
  ```
  没有使用 `c.actionShape`；OCR 分支 `:2970` 才用了 `c.actionShape || rect`。
- 复现路径：
  1. 在识别结果候选上画出 L 形/反选操作区（`activeCandidate.actionShape` 已在 `app.js:4616` 记录）；
  2. 勾选该候选 → 「保存素材」（`saveAssetUnified`）；
  3. 服务端收到的 `action_shape` 是 rect bbox，`invert=false`；保存后 polygon/invert 丢失。
  4. region 候选更直接：`:2996` 的 `c.kind !== 'region'` 使 region 完全不发 action_shape；服务端 `save_asset_impl:1629-1632` 自动补 `rect(roi)`。
- session 依据：18:40 第 2 条（多边形/反选）、19:12 方案 B；`docs/CODE-AUDIT-2026-11-01.md` §6 把 P1-2 标为已修。实际只修了 OCR 分支。
- 建议：非 OCR 分支改为 `payload.action_shape = c.actionShape || rect(c.action)`；region 分支也传 `c.actionShape`（若 region 语义允许 polygon），或在 UI 明确 region 只支持 rect。
- 测试缺口：无前端批量保存 shape 的自动化测试；`save_assets_batch` 的 Rust 测试只验证 rect 持久化。

### B-07（P2，潜伏）`ActionShape::contains` 仍是旧的 `.max(1)` 整数算法

- 位置：`crates/oas-task/src/lib.rs:140-166`，第 159 行：
  ```rust
  (x < (xj - xi) * (y - yi) / (yj - yi).max(1) + xi)
  ```
- 该函数当前无调用者（全仓 grep 仅定义），所以不是当前运行时 bug；但它是与 runtime `ClickShape::contains` 同语义的 public API，一旦被复用（例如未来 batch/编辑器校验）就会重现 P0-3。
- 独立 probe：三角形 `[(100,100),(400,100),(100,400)]` 内部点 (120,120)/(200,180) 返回 false；正确的 `ClickShape`/f64 版本返回 true。
- 建议：删除该函数或改为调用 `oas_runtime::ClickShape::contains` 的同一算法；至少补 `#[deprecated]` 和测试。

### B-08（P2）region 素材无法通过 `/task/asset/update` 更新 action_shape

- 位置：
  - `crates/oas-task/src/lib.rs:1420-1423`：
    ```rust
    if kind == "region" {
        return Err("region objects use their action_shape; no update needed".into());
    }
    ```
  - UI `updateAssetActionRoi`（`app.js:2728-2740`）对任意素材（含 region）调用 `/task/asset/update` 并发送 `action_shape`；`editAssetActionRoi` 也允许进入编辑（`app.js:2707-2726`，render 的 `onAction` 在 2387）。
- 复现路径：选中一个 `kind=region` 素材 → 编辑操作区域 → 保存；服务端返回 400「region objects use their action_shape; no update needed」，操作区不变。probe 直接调用 `update_asset_meta` 也返回该错误。
- 建议：region 的 action_shape 就是操作区域，应当允许 update（校验后写回，并同步 roi/action_source）；或 UI 对 region 隐藏「编辑操作区域」，但后者只是掩盖数据模型缺口。
- 测试缺口：无 region + update action_shape 的测试。

### B-09（P2）批量测试 region 分支只用 `item.roi`，不校验 action_shape → 假绿

- 位置：`apps/oasd/src/serve.rs:3019-3031`：
  ```rust
  "region" => {
      let ok = item.roi.map_or(false, |r| { ... bounds ... });
  ```
  没有用 `item.action_shape.bbox()`；而 runtime `load_runtime_assets` 对 region 优先用 `action_shape.bbox()`（`crates/oas-task/src/lib.rs:996-1017`）。
- 影响：region 的 `roi` 在帧内、`action_shape` 越界/完全不同，批量测试仍报 pass；实际 runtime 会按越界 shape 采样并被 clamp（B-05）。这是“测试通过但真实点击错误”的典型假绿。
- 建议：region 分支用与 runtime 相同的 `action_shape.bbox().or(roi)` 作为被测几何；若 shape invert，批量测试至少报告 effective region 而非 bbox。
- 测试缺口：无 region 批量测试用例。

### B-10（P2）collection action_shape 可保存但不校验、runtime 不加载；action_box 未平移

- 位置：
  - `AssetCollection.action_shape`（`crates/oas-task/src/lib.rs:303`）；`save_collection` 直接复制、无 `validate_action_shape`（`lib.rs:881-928`，919）。
  - `load_runtime_assets` 的 collection 只调用 `insert_collection(id,members,threshold)`（`lib.rs:1181-1194`），没有把 `coll.action_shape` 写入 `click_shapes`。
  - `click_box_for`/`translated_click_shape` 只查 winner member 的 shape（`lib.rs:1286-1292`、`1345-1352`），collection 级 shape 完全不生效。
  - `handle_collection_match` 的 `action_box` 直接返回 `assets.click_back(&member)`（`serve.rs:3237、3248`），没有按 `point` 平移，也没有用 winner 的 exact shape。
- 影响：API/数据层允许 collection 级操作几何，但运行期被静默忽略；collection 测试显示的 action_box 可能是旧位置。当前 UI 不写 collection shape，所以用户可见影响小；一旦后续接入就是 P1。
- 建议：要么在 `AssetCollection` 保存时校验并明确“collection shape 仅作 fallback”，在 `load_runtime_assets` 对无成员 shape 的 collection 设置 `click_shapes[coll.id]`（注意 winner 成员优先）；要么删除该字段，避免假能力。
- 测试缺口：`collection_*` 测试只覆盖成员匹配/点击，不覆盖 collection 级 shape；`handle_collection_match` 无平移测试。

### B-11（P2）`frame_for_test` 对非法 frame_id 静默回退；live 不是“每次强制抓取”

- 位置：`apps/oasd/src/serve.rs:1672-1710`：
  - `"frame_id" => frame_id = v.parse::<u64>().ok()`（1678）：`frame_id=abc`/`-1` 被解析成 `None`，随后走 current frame，HTTP 仍 200；
  - live 分支直接返回 `st.live_frame`（1682-1688），只有 `/frame.png?live=1` 才 `do_live_capture()`（1072-1077）；WS 线程 500ms 更新一次（456-491）。
- 影响：测试请求的帧来源可能不是客户端以为的帧；设计说 `live=1` 每次强制重新抓，实际是缓存（最多约 500ms 旧，WS 断开则无限旧）。
- 建议：`frame_id` 解析失败返回 400；`live=1` 时先 `do_live_capture()`（或用 `capture_device_frame`）再取 `live_frame`；若设备不可用，明确 503/400。
- 测试缺口：无 `frame_for_test` 单测；无 live 缓存/非法 id 用例。

### B-12（P2/P3）`Step::Appear`/`DismissPopup` 对 OCR 资产用 0.9 阈值

- 位置：
  - `Step::Appear`：`lib.rs:1982` `threshold_for(sel).unwrap_or(0.9)`；`threshold_for` 只查 collection/template（`lib.rs:236-242`），OCR 资产返回 `None` → 0.9。
  - `Step::DismissPopup`：`lib.rs:2000` 检测阈值、`:2010` action 阈值同样 `unwrap_or(0.9)`。
  - 对照 `Step::Click` 对 OCR 用 0.0（`lib.rs:2098-2101`），`handle_interrupts` 用 `selector_threshold`（`:1674-1680`）。
- 影响：OCR 识别分数 < 0.9 时，`click` 能点、`appear`/`dismiss_popup` 却报未找到；同一资产在不同 step 行为不一致。
- 建议：统一用 `selector_threshold`（OCR/derived 返回 0.0，collection 用 collection 阈值），`Step::Appear`/`DismissPopup` 都改。
- 测试缺口：`appear_supports_ocr_assets` 测的是表达式 `appear()`，不是 `Step::Appear`；无 OCR + dismiss_popup 测试。

### B-13（P3）模板 `testAssetClick` 未处理 match 失败

- 位置：`apps/oasd/ui/app.js:1536-1551`：拿到 `/task/asset/match` 的 `s` 后只检查 `s.error`，不检查 `s.pass`；失败时 `s.x/s.y=-1`，仍设置 `boxes`、`recognizedBox=[-1,-1,w,h]`，随后 `actionValidity`/目标点照常输出。
- 影响：dry-run 可能显示“匹配得分 0.xx @ (-1,-1)”却仍给出目标点和有效性；用户可能误以为素材可用。
- 建议：`if (!s.pass) { resultEl.textContent = 匹配未过阈值...; return; }`；至少不要把 -1 当识别框。
- 测试缺口：无前端失败路径测试。

### B-14（P3）task delete backup 的测试/边界缺口

- 位置：`apps/oasd/src/serve.rs:4146-4238`。修复已实现“先备份 `assets.json` + `images/`，失败则拒绝删除”，方向正确；但：
  - `backup_task_assets` 在 `assets.json` 不存在时直接 `Ok(None)`（4205-4207），若任务只有 `images/`（或 `assets.json` 损坏/被手动删），`remove_dir_all` 会删掉图片且无备份；
  - 只备份 `assets.json` + `images/`，不备份 `flow.json`/`meta.json`（用户恢复素材后无法恢复步骤映射）；
  - 无任何自动化测试（全仓 grep `backup_task_assets` 只有定义/调用）：没有“备份失败拒绝删除”“备份内容完整”“同名 timestamp 冲突”用例。
- session 依据：20:28/20:33 用户明确要求“删除任务导致素材丢失”必须保护；当前任务 `tasks/返回庭院` 删除是未提交状态，未见 `assets/_deleted-tasks/` 实物，无法做端到端验证。
- 建议：至少备份 `flow.json`/`meta.json`；`assets.json` 缺失但 `images/` 存在时也复制 images（或拒绝删除并提示）；timestamp 加随机后缀；补 failpoint 测试（备份 copy 失败 → 任务目录保留）。

---

## 3. 假绿 / 覆盖不足的测试（重点）

| 测试 | 位置 | 为什么是假绿/不足 |
|---|---|---|
| `ocr_asset_action_roi_controls_click_landing` | `lib.rs:5700-5747` | 用 `insert_ocr`（`OcrRead::Single`）和 click_back bbox；断言 `[0,70]×[0,40]` 足够宽，**即使用识别 ROI (10..30,10..20) 也会通过**，无法证明 click_back/action_shape 生效；完全不覆盖 `ocr_lines`/polygon/invert。 |
| `derived_ocr_line_uses_original_source_roi_and_translates_action_region` | `lib.rs:5849-5917` | 只断言 provider ROI 是 source；点击断言 `[0,40]×[8,38]`，识别行 rect `[10,18,20,8]` 也在范围内——即使 click_back 平移被删掉、改用 line.rect 也会通过。 |
| `polygon_and_invert_samples_are_effective` | `lib.rs:6054-6081` | 只测大面积成功路径；没有空有效区（全屏反选）、退化/共线/凹多边形 fallback，也没有断言 `sample` 结果一定 `contains`（B-03）。 |
| `exact_shape_translates_when_the_template_moved` | `lib.rs:4303-4357` | 能抓旧 `adjust` 的 rect 回归，但只测 rect，不测 polygon translation；且使用 `no_miss()` 仍保留 ±6 jitter，断言依赖 seed 7 恰好不越界。 |
| `self_intersecting_action_polygon_is_rejected` | `lib.rs:2918-2933` | 只测 bowtie 自交；不测零面积共线、越界（B-05）。 |
| `action_shape_is_persisted_and_wired_into_the_runtime_operation_region` | `lib.rs:2869-2889` | 只断言 `click_back` 是 bbox 和 JSON 字符串；不测 `ClickShape`/实际点击/平移。 |
| `fixed_polygon_action_shape_is_persisted_and_invert_is_kept` | `lib.rs:2891-2916` | 只断言持久化 + `click_back` bbox；不测 invert 点击、不测 polygon 采样。 |
| `saved_region_ref_clicks_inside_the_operation_region` | `lib.rs:5919-5946` | 30×20 大矩形 + 4px 容差，抓不到 1~12px 窄 region 的 jitter 出界（B-04）。 |
| 现有 serve 测试 | `serve.rs` `mod tests` | 没有 frame_for_test、batch region、collection live、task delete backup 的用例；`handle_collection_match` 只用代码审查。 |

---

## 4. 测试执行记录

```text
cd D:\OAS-dev\oas-core
cargo test --workspace
→ 361 passed / 0 failed（逐 crate 均 ok；日志 B-cargo-test.log）
```

分段：adapter 3 + codec 2 + flow 49 + obs 3 + ocr 16 + perceive 6 + ncc 2 + runtime 102 + replay 2 + task 50 + types 2 + oasd 124。

警告（非失败）：
- `crates/oas-runtime/src/lib.rs:4973` `shared` 未使用；该测试用 `interp.clicks()` 审计行，不是输入 sink，所以 warning 不影响断言，但说明测试没有验证真实 InputSink。
- `apps/oasd/src/task.rs:22` 未使用 `lint_tasks`；`serve.rs:651` 不需要 `mut`；`serve.rs:1728/1729` task/id 赋值未读；`asset_ocr_polygon` 等 dead_code。属于构建卫生，不影响运行。

---

## 5. 建议修复顺序

1. **B-03/B-05**：先让 exact shape 的“不可采样/越界”变成 loud error（runtime `Result` + 校验），否则所有 shape 修复都可能是假绿。
2. **B-02**：统一 `click_target` OCR 路径到 `click_box_for`/`adjust_plan_for_shape`，这是用户可见的“生成步骤与点测不一致”。
3. **B-01/B-06**：UI 平移与批量保存 shape，补前端断言；否则 P0-4/P1-2 只在 runtime/OCR 一半生效。
4. **B-04**：region 采样后校验/重采样，去掉 jitter 出界。
5. **B-09/B-10/B-11/B-12**：server/collection/帧来源/阈值一致性，补 serve 层测试。
6. **B-07/B-08/B-13/B-14**：清理潜伏算法、region update、失败路径、task delete backup 测试。

---

## 6. 附：仓库外 probe 证据（不修改仓库）

以下 probe 均写在 `%TEMP%`，用当前构建的 rlib 编译（`rustc -L dependency=target\debug\deps --extern ...`），只读数据：

1. `ClickShape::sample` 共线多边形：`sample=(20,10) contains=false`。
2. `ClickShape::sample` 全屏反选：`sample=None`；runtime `click` 仍执行，`audit point=(52,29)`，run=Finished(Done)。
3. 越界 action_shape `[100,100,10,10]`（frame 64×48）：`save_asset` 返回 Ok；runtime `click` 结果为 `audit point=(63,47) target=(105,105)`，run=Finished(Done)。
4. `ActionShape::contains` 三角形内部点：`(120,120)->false`、`(200,180)->false`（旧算法）。
5. region `[10,20,1,20]` 的 `click_target`：`audit point=(5,25)`，`x=5` 不在 shape 内。
6. 单行 OCR + 全屏反选 `click_target`：`audit point=(17,12)`，run=Finished(Done)。
7. `task::update_asset_meta(region, action_shape=...)`：`Err("region objects use their action_shape; no update needed")`。
