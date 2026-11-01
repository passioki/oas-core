# D. 真实 headless 浏览器 E2E 回归（8787）

- **执行时间**：2026-11-01 23:03–23:05（Asia/Shanghai）
- **仓库**：`D:\OAS-dev\oas-core`，HEAD `4d5759f4192d19b61a8d28ba450f281c01644c69`
- **服务器**：`oasd` PID 33124，启动于 22:58:46，监听 `http://127.0.0.1:8787`；测试前后未启动/重启/停止
- **浏览器**：Microsoft Edge `156.0.4314.58`，`--headless=new`，CDP `127.0.0.1:9435`
- **视口/截图**：`--window-size=1440,1000`；截图实际 1416×908
- **只读约束**：全程仅 GET；未点击保存/删除/批量测试/运行任务；实时预览先暂停后才操作画布；未 commit/push
- **结论**：**36 / 36 项 PASS，0 FAIL**；`window.__oasErrors=[]`，console error=0，uncaught exception=0，非 GET 请求=0

> 仓库内唯一写入：本报告。测试前工作区已有未提交改动/删除（`assets/assets.json`、`assets/images/*` 等），本进程没有触碰；测试涉及的 6 个数据文件 SHA-256 和 mtime 均与测试前一致；自动化脚本与截图仅写在系统临时目录。

---

## 0. 先看结论

1. **全局素材库搜索/分类本轮修复可用**：初始 14 行 → 搜索“返回”1 行（返回箭头）→ 分类“式神录”3 行 → 搜索+分类组合 1 行 → 清空恢复 14 行；状态行始终为“全局素材库：共 14 / 显示 N · 搜索：… · 分类：…”。
2. **刷新按钮真实生效**：点 `#btn-library-refresh` 后 header 变为“全局素材库已刷新”，Network 记录到新的 `GET /task/assets?task=`，行数保持 14，无 JS 错误。
3. **任务素材库跟随任务**：`返回庭院` 为 0 行（flow 引用 0/14，14 个未引用提示）；`测试` 为 1 行（`觉醒材料`，13 个未引用提示）。`#task-assets-hint`、`#task-show-all-assets` 的文案/行数/勾选行为均正确。
4. **任务库搜索/分类过滤生效**：`返回庭院` 勾选“显示全部”后 14 行 → 搜索“返回”1 行；分类“式神录”3 行；取消勾选回 0 行。`测试` 搜索“觉醒”1 行、无匹配 0 行、分类下拉只列实际分类。
5. **P1-8 模式互斥/reset 可用**：四按钮（多边形框选 / 操作区域多边形 / 绘制操作区域 / 反选）依次切换时按钮文案互斥，不卡死；反选“开”后切“多边形框选”会自动清回“关”。
6. **清除确实清掉旧 actionPoly/invert**：闭合的 3 顶点操作区多边形 + 反选显示在 `#roi`；点清除后 `#roi=未选择`、反选=关、三按钮回默认、`#result=—`。画布橙色像素从闭合时 3630（精确色 2984）降到 14（精确色 0），无旧多边形残留。
7. **DOM 收敛**：11 个旧死 id 全部不存在；14 个本轮关键新 id 全部存在。
8. **旁证观察（不计入 36 项）**：普通打开页面时 `#task-select`/`#task-name` 会自动选到“测试”，但任务素材区仍显示“（未选择任务；任务素材库为空）”、hint 为“—”，需手动切换一次任务才加载；建议后续修 boot 顺序。

---

## 1. 方法与可复现命令

### 1.1 启动与连接

Edge 参数：

```text
"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"
  --headless=new --remote-debugging-port=9435
  --user-data-dir=%TEMP%\oas-e2e-profile-<ts>
  --no-first-run --no-default-browser-check --disable-gpu --no-proxy-server
  --window-size=1440,1000 --force-device-scale-factor=1 about:blank
```

CDP 流程：`Page.enable`、`Runtime.enable`、`Log.enable`、`Network.enable` → `Page.navigate http://127.0.0.1:8787/` → 等待 `document.readyState=complete` 且 boot 完成 → 断言/交互 → `Page.captureScreenshot` → `Browser.close`。

### 1.2 交互方式

- 下拉/输入：设置 `value` 后派发 `input` 或 `change`（与真实用户触发同一监听器）。
- 按钮：DOM `element.click()`（真实浏览器事件处理链路）。
- 画布多边形：`Input.dispatchMouseEvent` 真实鼠标事件点击画布帧坐标 `(300,180)`、`(480,180)`、`(390,330)` 三个顶点；闭合使用同一 `dblclick` handler 的合成 `MouseEvent`（`detail=2`），不触发保存。
- 所有断言的 DOM 文本、行数、状态值均来自页面实时 `Runtime.evaluate`。

### 1.3 证据文件

```text
SHOT_DIR = C:\Users\passioki\AppData\Local\Temp\oas-e2e-20261101-230112\shots
RESULTS  = C:\Users\passioki\AppData\Local\Temp\oas-e2e-20261101-230112\results.json
CHECKS   = C:\Users\passioki\AppData\Local\Temp\oas-e2e-20261101-230112\shots\checks.txt
SUMMARY  = C:\Users\passioki\AppData\Local\Temp\oas-e2e-20261101-230112\summary.txt
SCRIPT   = C:\Users\passioki\AppData\Local\Temp\oas-e2e-20261101-230112\e2e.mjs
```

已人工查看 `04-global-combined.png`、`06-task-return-hint.png`、`09-task-test.png`、`11-selection-poly-closed.png`、`12-selection-after-clear.png`，与断言一致。

---

## 2. 全局素材库（G1–G9，全部 PASS）

初始分组：`探索总界面（11）`、`式神录（3）`；分类下拉 options：`全部分类`、`式神录`、`探索总界面`。

| # | 检查项 | 实际输出 | 结果 | 证据 |
|---|---|---|---|---|
| G1 | `#global-asset-search` 存在，placeholder | `按素材 id 搜索` | PASS | DOM |
| G2 | 初始状态/行数 | `全局素材库：共 14 / 显示 14`；14 个 `.asset-row`；2 个分组 | PASS | `01-initial-global.png` |
| G3 | 分类下拉含“全部分类”+实际分类 | `全部分类` / `式神录` / `探索总界面` | PASS | DOM |
| G4 | 搜索“返回” | 1 行 `[返回箭头]`；状态 `共 14 / 显示 1 · 搜索：返回` | PASS | `02-global-search.png` |
| G5 | 分类“式神录” | 3 行 `[预设, 返回箭头, 育成]`；状态 `共 14 / 显示 3 · 分类：式神录` | PASS | `03-global-category.png` |
| G6 | 搜索+分类组合 | 1 行 `[返回箭头]`；状态 `共 14 / 显示 1 · 搜索：返回 · 分类：式神录` | PASS | `04-global-combined.png` |
| G7 | 清空搜索/分类 | 14 行；状态 `共 14 / 显示 14`；两个控件 value 均为空 | PASS | DOM |
| G8 | 点 `#btn-library-refresh` | header=`全局素材库已刷新`；新增 `GET /task/assets?task=`；14 行 | PASS | `05-global-refresh.png` |
| G9 | 刷新后无 JS 错误 | `window.__oasErrors=[]` | PASS | Runtime |

补充：刷新前后行数、分组、分类 options 均保持不变；刷新不是只改文案，Network 确实出现了新的 `GET /task/assets?task=`。

---

## 3. 任务素材库（T1–T9，全部 PASS）

### 3.1 任务“返回庭院”

| # | 检查项 | 实际输出 | 结果 | 证据 |
|---|---|---|---|---|
| T1 | 选择任务后任务库/hint | hint=`未引用提示：全局库共 14 个素材，其中 14 个未被当前任务 flow 引用（勾选「显示全部全局素材」可查看/管理）`；0 行；head=`任务素材库 返回庭院 · design 1280x720 · 0 个素材 · flow 引用 0/14` | PASS | `06-task-return-hint.png` |
| T1b | `#asset-search`/`#asset-category-filter` 存在 | 均存在；分类 options 仅 `全部分类`（当前任务素材为空） | PASS | DOM |
| T2 | 勾选 `#task-show-all-assets` | 14 行；hint 追加`（当前显示全部全局素材）`；head 追加`· 已显示全部` | PASS | `07-task-showall.png` |
| T3 | 任务库搜索“返回” | 1 行 `[返回箭头]`；head 保留`flow 引用 0/14 · 已显示全部` | PASS | `08-task-filter.png` |
| T4 | 任务库分类“式神录” | 3 行 `[预设, 返回箭头, 育成]`；head 追加`· 分类：式神录` | PASS | DOM |
| T5 | 取消勾选“显示全部” | 0 行；hint 恢复为“勾选…可查看/管理”，无“当前显示全部” | PASS | DOM |

> `返回庭院` 的 flow 使用原始 ROI + `text_visible`，当前 `collectFlowAssetIds` 只统计 `sel` / `appear` / `ocr` / `require_template` 引用，因此 flow 引用数为 0。hint 的“14 个未引用”与这一口径一致；不是搜索/分类丢失。

### 3.2 任务“测试”

| # | 检查项 | 实际输出 | 结果 | 证据 |
|---|---|---|---|---|
| T6 | 切到“测试” | 1 行 `[觉醒材料]`；hint=`…其中 13 个未被当前任务 flow 引用…`；head=`flow 引用 1/14` | PASS | `09-task-test.png` |
| T7 | 搜索“觉醒” | 1 行 `[觉醒材料]` | PASS | DOM |
| T8 | 搜索“不存在” | 0 行；空提示`（没有匹配的素材）` | PASS | DOM |
| T9a | 分类下拉只列实际分类 | `["全部分类","探索总界面"]`（“测试”仅引用 1 个探索总界面素材） | PASS | DOM |
| T9 | 分类“探索总界面” | 1 行；head 追加`· 分类：探索总界面` | PASS | DOM |

---

## 4. SelectionTool / reset（S1–S8，全部 PASS）

按钮 id/文本（读 `index.html` 确认）：

- `#btn-poly`：`多边形框选（点顶点，双击闭合）`
- `#btn-action-poly`：`操作区域多边形（点顶点，双击闭合）`
- `#btn-back-mode`：`绘制操作区域（默认=识别区域）`
- `#btn-clear`：`清除识别/操作区域`
- `#btn-action-invert`：`操作区域反选：关/开`

| # | 检查项 | 实际输出 | 结果 | 证据 |
|---|---|---|---|---|
| S1 | 选素材“觉醒材料”设置 ROI | `#roi = 识别框 x=43 y=673 w=377 h=32 · 操作区域 矩形 62x75 @ 58,628` | PASS | `10-selection-base.png` |
| S1b | 四按钮初始文案 | 三个模式按钮为默认文本；反选=`操作区域反选：关` | PASS | DOM |
| S2 | 点“多边形框选” | `btn-poly=退出多边形模式（…）`；`action-poly`/`back-mode` 仍默认 | PASS | DOM |
| S3 | 切“操作区域多边形” | `btn-poly` 恢复默认；`action-poly=退出操作区域多边形（…）`；`back-mode` 默认 | PASS | DOM |
| S4 | 切“绘制操作区域” | `action-poly` 恢复默认；`back-mode=退出操作区域编辑（拖拽画操作区域）` | PASS | DOM |
| S5 | 点反选 | `操作区域反选：开` | PASS | DOM |
| S6 | 从操作区域矩形+反选切“多边形框选” | `btn-poly=退出多边形模式`；`action-poly`/`back-mode` 回默认；反选自动回`关` | PASS | DOM |
| S7 | 画操作区多边形并闭合 | `#roi=识别框 … · 操作区域 多边形 3 顶点（反选：屏幕−区域）`；未绑定素材时 `#result` 提示“没有绑定候选/素材”，未发起保存 | PASS | `11-selection-poly-closed.png` |
| S8 | 点“清除” | `#roi=未选择`；反选=`关`；三模式按钮全回默认；`#result=—`；header=`已清除识别/操作区域与选择状态` | PASS | `12-selection-after-clear.png` |

**actionPoly 残留的额外像素证据**（同一画布、实时已暂停）：

| 时点 | 橙色系像素 | 精确 `#ffb020` 附近像素 |
|---|---:|---:|
| 选中素材后（基线） | 529 | 0 |
| 3 顶点多边形闭合+反选 | 3630 | 2984 |
| 点“清除”后 | 14 | 0 |

闭合时截图可见橙色多边形 + 全屏虚线反选边框；清除后两者均消失，`#roi` 也从“操作区域 多边形 3 顶点（反选）”回到“未选择”。因为页面没有把 `selectionActiveMode()` 暴露到 `window`，此处用 DOM 文案 + canvas 像素双重判定，而不是调用内部全局函数。

---

## 5. JS 错误 / DOM id / 网络只读（E1–E5，全部 PASS）

| # | 检查项 | 实际输出 | 结果 |
|---|---|---|---|
| E1 | `window.__oasErrors` 最终值 | `[]` | PASS |
| E2 | console error / uncaught exception | `Runtime.consoleAPICalled(type=error)=0`；`Runtime.exceptionThrown=0` | PASS |
| E3 | 写请求 | 非 GET 请求 `[]`；`postRequests=[]` | PASS |
| E4 | 旧死 id 不存在 | `library-id`、`btn-library-save`、`library-select`、`btn-library-match`、`btn-batch-save`、`btn-asset`、`btn-match-save`、`flowJson`、`hdr-drawers`、`result-pop`、`log-pop` 均 `false`（11/11） | PASS |
| E5 | 关键新 id 存在 | `global-asset-search`、`global-asset-category-filter`、`global-assets-list`、`global-assets-status`、`btn-library-refresh`、`asset-search`、`asset-category-filter`、`task-assets-hint`、`task-show-all-assets`、`btn-poly`、`btn-action-poly`、`btn-back-mode`、`btn-clear`、`btn-action-invert` 缺失列表 `[]`（14/14） | PASS |

`Log.enable` 已开启；除上述 console/exception 外，`window.__oasErrors` 在页面加载、搜索、分类、刷新、任务切换、画布绘制、清除各阶段均为空。

---

## 6. 只读服务端探测（未 POST）

测试前/后各调用一次：

| 请求 | 结果 |
|---|---|
| `GET /task/list` | `count=2`，`tasks=["测试","返回庭院"]`，`tasks_dir=D:\OAS-dev\oas-core\tasks` |
| `GET /task/assets?task=` | `count=14`，`collections=[]`，`design_resolution=[1280,720]`，`task="（全局素材库）"` |
| `GET /task/assets?task=返回庭院` | `count=14`，`task="（全局素材库）"`（统一全局库；任务页在前端按 flow 引用过滤） |
| `GET /task/assets?task=测试` | `count=14`，同上 |

分类分布：`探索总界面=11`，`式神录=3`；id 顺序与页面初始 14 行一致（`觉醒材料…契灵之境,预设,返回箭头,育成`）。

数据文件只读校验（SHA-256 完整值一致，mtime 在毫秒精度一致）：

| 文件 | 大小 | SHA-256 前缀 |
|---|---:|---|
| `assets/assets.json` | 6412 | `0A2C00A0D9F14C16` |
| `tasks/返回庭院/flow.json` | 6212 | `627855CA91B577AF` |
| `tasks/返回庭院/meta.json` | 400 | `BAD11E9CB1EE1C6D` |
| `tasks/测试/flow.json` | 495 | `58BD63BCE56B1158` |
| `tasks/测试/meta.json` | 409 | `F4D21052B773A025` |
| `tasks/测试/assets.json` | 89 | `34E6A4EE1C281D08` |

`/task/list` 与 `/task/assets?task=` 的数量与测试前一致；测试后再次读取仍为 tasks=2、assets=14。

---

## 7. 未覆盖 / 限制 / 观察

### 7.1 按要求未做

- **未保存、未删除、未批量帧测试、未运行任务**：这些会写数据或触发设备动作；本轮只做只读回归。
- **未点击素材行“测试/识别/点测/拖测/步骤/批量帧测试”**：避免 OCR/设备/可能的后续保存副作用；因此“真实保存后多边形/反选落盘”不在本轮覆盖内。
- **未做真实设备点击/滑动**：页面 boot 时是 `emulator-5554` 在线状态，但进入画布前已暂停实时预览（`实时：暂停`），避免 tap/swipe。
- **未验证 8791 临时服**：本次针对用户指定的 8787；8791 的上轮结果不作为本轮证据。

### 7.2 观察 O1（不计入 36 项，建议后续处理）

普通打开 `http://127.0.0.1:8787/`，在 boot 完成（`readyState=complete`、全局库 14 行、`#task-select` 已有 2 个 option）后：

- `#task-select` value / `#task-name` value = `测试`（自动选中首个任务）
- 但 `#assets-list` = `（未选择任务；任务素材库为空）`
- `#task-assets-hint` = `—`

原因路径（静态对照）：boot 中先 `await refreshTaskAssets()`（此时 `taskSelect` 还没有 option），随后 `refreshTaskList()` 只设置 select/input 的值，没有再触发一次任务素材加载；手动 `change` 任务后 T1/T6 均正确。该现象在 23:02 和 23:05 两次 fresh boot 均复现，属于 UI 一致性缺口，不影响本轮“手动选择任务后过滤生效”的 PASS 结论。

### 7.3 方法上的说明

- 搜索/分类输入采用 `value + input/change` 事件，而非逐键模拟；与真实用户触发的是同一监听器。
- 画布 3 个顶点使用 CDP 真实鼠标事件；闭合事件使用同一 `dblclick` handler 的合成事件（避免双击再添加第 4 个顶点）。若要求“全部点击均为 CDP 原生事件”，可后续补一版。
- 截图存放在系统临时目录（非仓库），因为任务限定“只写报告”；`results.json`/`checks.txt` 含完整逐项原始记录。

---

## 8. 证据清单

```text
SHOT_DIR\01-initial-global.png          初始全局库 14 行 / 2 分组
SHOT_DIR\02-global-search.png           搜索“返回”后 1 行
SHOT_DIR\03-global-category.png         分类“式神录”后 3 行
SHOT_DIR\04-global-combined.png         搜索+分类 1 行（已人工查看）
SHOT_DIR\05-global-refresh.png          刷新全局库后
SHOT_DIR\06-task-return-hint.png        返回庭院：0 行 + 14 未引用提示（已人工查看）
SHOT_DIR\07-task-showall.png            显示全部后 14 行
SHOT_DIR\08-task-filter.png             任务库搜索“返回”1 行
SHOT_DIR\09-task-test.png               测试：1 行 + 13 未引用（已人工查看）
SHOT_DIR\10-selection-base.png          选中素材后的 ROI/操作区域矩形
SHOT_DIR\11-selection-poly-closed.png   3 顶点操作区多边形 + 反选（已人工查看）
SHOT_DIR\12-selection-after-clear.png   清除后无多边形/反选（已人工查看）
```

---

## 9. 最终判定

- **全局素材库搜索/分类/组合/清空/刷新**：PASS（9/9）
- **任务素材库跟随/提示/显示全部/搜索/分类**：PASS（11/11）
- **SelectionTool 模式互斥/reset/清除**：PASS（8/8）
- **JS 错误/旧死 id/关键 id/只读网络**：PASS（5/5）
- **总计**：36 PASS / 0 FAIL；浏览器可用，无伪造；未修改任何数据或代码。
