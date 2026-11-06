# 操作区域与动作能力设计（Operation Region / Actions）

> 状态：**P1 已实现（seq 121，commit `644249c`）**；用户已确认 §6 的 4 项决策。P2（模板多候选 nearest/真机验收）待后续。
> 触发：用户反馈参数文字点击的素材选择对话框「并不完善」，并给出操作区域的 3 类来源 + 动作能力问题
> （长按？任意曲线滑动？）。
> 本文先对照**当前引擎事实**，再给统一模型与分期实现计划；P1 实现记录见 `ENGINE §22.61` / `PENDING §E-91`。

## 1. 当前引擎事实（可核查）

### 1.1 操作区域取自哪里

| 场景 | 当前行为 | 代码位置 |
|---|---|---|
| 素材/模板点击 | 素材声明了 `action_shape`（或旧 `roi_back`/click_back）→ 按匹配偏移平移后作为操作区域；否则点击**匹配到的模板 bbox** | `crates/oas-runtime/src/lib.rs` `click_box_for()` ~1644-1738；`AssetDecl.action_shape/action_source` 在 `crates/oas-task/src/lib.rs` ~286-310 |
| 素材/模板集合 | 集合成员逐一匹配，取 **score 最高**成员；点击用该成员自己的 `action_shape`（平移后） | `locate()` ~1519-1544；`translated_click_shape()` ~1576-1589 |
| OCR 文字点击 | 若 OCR 素材声明了 `action_shape`/click_back → 用素材操作区域；否则用**本次 OCR 识别到的文字行 bbox**；再否则用素材 `roi` | `click_box_for()` ~1655-1685；OCR locator 分支 ~2764-2839 |
| 拖动端点 | `region`=固定 roi（可带 polygon shape）；`ocr_text`=**第一条**文字匹配行的 bbox；`point`=点 | `drag_target()` ~1846-1905 |

结论：
- 用户列的第 **1a（素材本身操作区域）**：已支持（素材级，非 flow 级）。
- 第 **1b（多匹配取最近）**：**未支持**。当前模板取最高分；OCR 点击取最高分、OCR 拖动取第一条（两者还不一致）。
- 第 **2（相对位移区域）**：**未支持**为 flow 级通用能力。历史决策明确「无 offset，用独立区域操作」；
  现有 `roi_back`/`action_shape` 是**素材级**、相对素材识别 roi 平移（模板匹配时按 match offset 平移），
  没有「按本次匹配/识别结果再偏移」的 per-action 表达。
- 第 **3（参数 OCR 真实识别区域）**：**部分支持**——只有素材没有声明 `action_shape`/click_back 时才会用真实文字框；
  素材一旦有操作区域，无法在单次动作里强制用识别框。

### 1.2 动作能力

| 动作 | 现状 | 备注 |
|---|---|---|
| 点击 | `click_target`（find 冗余链：模板/OCR/区域/坐标记忆） | 支持素材操作区域 / 识别框（取决于素材配置） |
| 拖动 | `drag`（from/to 三形态 + 方向 + 距离/时长随机） | 一次 `input swipe`**直线**；文档明说曲线抖动只进审计、不发送 |
| 滑动 | `swipe`（固定起终点、时长） | 一次直线 `input swipe` |
| 长按 | **没有** | 输入层只有 `click`/`swipe`/`key`/`restart_app` |
| 任意曲线滑动 | **做不到（单次连续真实曲线）** | ADB `input swipe` 只有直线；低层注入（sendevent / `input motionevent`）设备/系统版本相关、不可移植 |

输入层事实：`InputSink` 仅 `click/swipe/key/restart_app`，`AdbInput::swipe` = `adb shell input swipe x1 y1 x2 y2 ms`
（`crates/oas-adapter-pc/src/lib.rs` ~283-313）。

## 2. 建议的统一模型

### 2.1 区域来源（per-locator / per-endpoint，flow 内表达）

在 `click_target.find[]`、`drag.from/to`、未来的 `long_press.find[]` 上增加可选 **region**：

```json
{
  "kind": "ocr_text",
  "sel": "O_分组",
  "text_expr": "param('group')",
  "region": {
    "mode": "asset" | "ocr" | "nearest" | "offset",
    "reference": { "mode": "asset_roi_center" | "screen_center" | "point", "x": 0, "y": 0 },
    "offset": [10, -5, 200, 80]
  }
}
```

- `mode=asset`：沿用素材/集合成员的操作区域（含 action_shape 平移；模板=匹配对象，OCR=素材操作区域）。
- `mode=ocr`：**强制**用本次 OCR 识别到的文字行 bbox（忽略素材 action_shape；仅 OCR locator 有效）。
- `mode=nearest`：当同一素材/同一 OCR 搜索区内有 **多个 ≥ 阈值**的匹配对象时，先枚举全部候选，再按
  `reference` 选最近的一个；用该候选自身的操作区域（模板=该候选平移后的 action_shape/bbox；OCR=该行 bbox）。
- `mode=offset`：在基准矩形（模板匹配 bbox 或 OCR 行 bbox / 素材操作区域）上叠加 `offset:[dx,dy,w,h]`，
  即「相对位移区域」。建议 `offset` 相对**基准矩形左上角**；`w/h` 可选，缺省沿用基准尺寸。
- 缺省（不写 `region`）= 当前行为，旧 flow 零变化；该字段受 flow_ops 门禁保护（新 op 标记）。

### 2.2 多匹配候选（nearest 的前置）

- 模板：当前 `match_template` 只返回最佳；需新增 `match_template_all`（NCC 局部极大值 + NMS）供 `nearest` 使用，
  阈值沿用素材/集合阈值。成本主要在 `oas-perceive`/obs 匹配层。
- OCR：`ocr_read` 已返回全部行，无需新能力；只需统一 click/drag 的候选筛选（当前 click=max score、drag=first，需统一）。
- 集合：枚举每个成员的候选位置（集合本身是“多图同目标”，nearest 也要覆盖成员间候选）。

### 2.3 动作扩展到哪些动作

区域语义适用于**需要点/手势落点**的动作：
1. `click_target`（点击）
2. `drag` 的 from/to（拖动端点）
3. **新增 `long_press`**（长按；见 §3.1）
4. 未来的「曲线/路径手势」（见 §3.2）
`swipe`/`click_point` 是明确坐标动作，不套 region（保持确定性）；`appear`/`expect` 只做校验，不需要操作区域。

## 3. 长按与曲线

### 3.1 长按（建议新增）

建议新增独立 op `long_press`（不塞进 `click_target`，避免 click 语义/日志/门禁混在一起）：

```json
{"op":"long_press","args":{"find":[{"kind":"ocr_text","sel":"O_x","text_expr":"param('k')"}],"hold_ms":800}}
```

- 复用 click_target 的 locator 解析链 + 区域模型（素材操作区域/识别框/nearest/offset）。
- 输入层新增 `long_press(point, ms)`：PC/ADB 实现用 `adb shell input swipe x y x y ms`（同一坐标 = 按住），
  其余后端（replay/fake）记录同一点 swipe；`hold_ms` 范围建议 100..=5000，默认 800。
- 风险：不同 App 对手势判定可能依赖加速度/轨迹；同一坐标 sweep 是最接近长按的通用做法，真机 E2E 需验证。

### 3.2 任意曲线滑动（结论：单次连续真曲线做不到）

- 现状：引擎一次拖动/滑动只发一条 `input swipe`（直线），文档明确曲线抖动仅审计、不发送。
- 可选方案：
  - **A（不建议）分段直线近似**：把曲线采样成 N 段，发 N 次 swipe。视觉接近曲线，但每次是**独立手势**，
    可能触发多次滚动/误触；需要专门 op 且语义要写清。
  - **B（仅限模拟器/Android 11+）低层注入**：`input motionevent` 或 `sendevent` 逐点发送（真单手势连续曲线），
    但设备/系统/权限依赖强，不适合作为通用引擎能力。
  - **C（现状）**：保持直线 + 起终点/时长/抖动随机（拟人化），不追求曲线。
- 建议：先不做真曲线；若游戏必须曲线，优先评估 B 作为「实验后端」而不是默认能力。

## 4. 分期实现计划（待确认后开工）

| 期 | 内容 | 影响面 |
|---|---|---|
| P1 | `long_press` op（locator 链 + hold_ms + InputSink/ADB/replay + flow_ops + 动作库/属性面板 + Runner 透传） | oas-flow/oas-runtime/oas-obs/oas-adapter-pc/oas-task/builder/UI tests |
| P1 | OCR 区域模式 `region.mode=asset/ocr`（含 drag 端点）——**复用现有几何**，不依赖多候选 | flow 解析/T3/T4/T3 path/runtime/build UI/flow_ops |
| P2 | `region.mode=offset`（相对位移区域） | 同 P1 + 语义/边界校验 |
| P2 | `region.mode=nearest` + 参考点（需确认参考点）+ 模板多候选（match_all/NMS） | oas-perceive/obs 匹配层 + runtime + flow + UI |
| P3 | 曲线：按确认方案（大概率仅实验后端） | adapter/输入层 |

兼容性：所有新字段可选，旧 flow 逐字节不变；新 op/flow_ops 受安装门禁保护（老 Runner 拒绝新包）。

## 5. 待用户确认（阻塞实现）

1. **nearest 的“最近”参考点是什么？**（候选 A：素材识别 roi 中心；B：屏幕中心；C：指定坐标；D：上一次点击/操作位置）
2. **长按**：用独立 `long_press` op（推荐）还是 `click_target` 增加 `hold_ms`？默认时长/范围？
3. **相对位移**：`offset:[dx,dy,w,h]` 相对匹配/识别 bbox **左上角**是否可以？还是相对中心/素材 roi？
4. **曲线**：接受现状（直线拟人）？还是需要「多段近似」（A）或模拟器专用低层注入（B）？

## 6. ✅ 用户已确认（2026-11-06 08:3x，seq 121 实施）

1. **nearest 参考点 = 参数 OCR 识别真实区域的 ROI 中心**（即本次 OCR 所用的素材识别 ROI / inline roi 的中心）；仅用于 OCR 多匹配场景，选择“离该 ROI 中心最近”的匹配行，再使用该匹配行的操作区域。
2. **长按 = 独立 `long_press` op**；默认时长 **3000ms**；支持**随机范围可调**（拟人）。
3. **相对偏移**：`offset:[dx,dy,w,h]` 相对匹配/识别 bbox **左上角**（w/h 可省略时沿用 bbox 尺寸）。
4. **曲线 = 多段近似**：允许多段直线模拟曲线；明确风险（每段是独立手势，App 可能视为多次滑动）。

### 6.1 实施规格（P1，本轮）

**`region` 字段**（可选；旧 flow 零变化；新 package 通过 flow_ops 门禁）：
```json
"region": {"mode":"asset|ocr|nearest|offset","offset":[dx,dy,w,h]}
```
- 可挂在 `click_target.find[]` 的 `kind:"ocr_text"`、`drag.from/to` 的 `kind:"ocr_text"`、以及 `long_press.find[]`。
- `asset`：素材操作区域（`action_shape`/`click_back`；无则回退 OCR 行 bbox）。
- `ocr`：强制使用本次 OCR 匹配行的 bbox（忽略素材 action_shape）。
- `nearest`：多行匹配时，选**行 bbox 中心距 OCR 识别 ROI 中心最近**的一行；操作用该行 bbox。
- `offset`：在基础 bbox（ocr/nearest 选中的行；asset 模式为素材操作区域）左上角叠加 `[dx,dy,w,h]`；w/h 缺省继承基础尺寸。
- 无 `region` = 现状。

**`long_press` op**：
```json
{"op":"long_press","args":{
  "find":[{"kind":"ocr_text","sel":"O_x","text_expr":"param('k')","region":{...}}],
  "hold_ms":3000,
  "hold_range":[2500,3500]   // 可选；存在则在此区间随机（拟人），范围 100..=10000
}}
```
- locator 解析同 click_target；长按前同样做屏内校验；`hold_ms` 默认 3000；`hold_range` 优先于 `hold_ms`（min≤max）。
- 输入层新增 `long_press(point, ms)`：ADB 用同坐标 `input swipe x y x y ms`；replay/fake 记录。
- 新增 flow_ops：`long_press`、`click_target.region`、`drag.region`（含 region 字段时）。

**`curve_swipe` op（多段近似）**：
```json
{"op":"curve_swipe","args":{"points":[[x,y],[x,y],...],"duration_ms":[600,1000]}}
```
- `points` 2..=32 个屏内点；按 N-1 段依次 `input swipe`，总时长随机后按段均分（可加少量抖动）；日志记录段数/总时长；**文档明确每段为独立手势**。
- 无 locator/asset 依赖；flow_ops 新增 `curve_swipe`。

### 6.2 UI（本轮）

- 动作库新增「长按」`long_press`（含「参数文字长按」预设）与「曲线滑动（多段近似）」`curve_swipe`。
- 定位对象/拖动端点新增「操作区域」选择（默认/素材操作区域/OCR识别区域/最近匹配/相对偏移 + `[dx,dy,w,h]` 输入）。
- **等待动作解释**：动作库与属性面板明确「等待参数文字/等到条件」是**阻塞等待**步骤：条件成立后继续执行**下一个 flow 步骤**（不触发独立动作）；超时则本步失败。UI 增加 help/title。
- 测试：UI pure test 新增 long_press/curve_swipe/region 纯函数与静态断言；基线 2039 只能增；0 dead/nav 6 不变。
