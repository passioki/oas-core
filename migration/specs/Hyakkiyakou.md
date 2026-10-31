# Hyakkiyakou（百鬼夜行）迁移规格

- 源仓库：`runhey/OnmyojiAutoScript@dev` · commit `8b4dc669b47838f42d283f14aa394844c1963c86` · 只读副本 `D:/OAS-dev/_verify/remote-oas`
- 目标：`tasks/Hyakkiyakou/`（统一任务模板 + 全局 `legacy_*` 素材）
- 迁移结论：**⛔** —— 行为矩阵见 §3，缺口见 §5

## 1. 读取的源文件

- `tasks/Hyakkiyakou/script_task.py`
- `tasks/Hyakkiyakou/config.py`
- `tasks/Hyakkiyakou/agent/agent.py`
- `tasks/Hyakkiyakou/slave/hya_slave.py`
- `tasks/Hyakkiyakou/slave/hya_device.py`
- `tasks/Component/SwitchOnmyoji/switch_onmyoji.py (imported)`

## 2. 入口 / 出口

- 入口：`appear('庭院_探索灯笼')`；flow 首段 `ensure_main()` 回庭院。
- 出口：
  - `done` → result `success`
  - `no_target` → result `blocked`
  - `failed` → result `retryable`

## 3. 行为矩阵

| # | 行为 | 远程实现（源码位置） | 新 DSL 映射 | 状态 |
|---|---|---|---|---|
| 1 | 进入百鬼夜行 | script_task.py run(): goto_page(page_hyakkiyakou)（page_town_goto_hyakkiyakou） | main→I_MAIN_GOTO_TOWN→I_TOWN_GOTO_HYAKKIYAKOU→expect I_CHECK_KYAKKIYAKOU | 覆盖 |
| 2 | 阴阳师切换 | script_task.py SwitchOnmyoji | — | 缺口 |
| 3 | 撒豆主循环 | script_task.py/HyaSlave：Tracker 推理 → Agent 决策 → hya_device 窗口消息点击 | — | ⛔ 架构阻塞 |
| 4 | 奖励/统计/训练 | tasks/Hyakkiyakou/debugger.py, train/* | — | 缺口 |

## 4. 参数映射

- 无参数。

## 5. 缺口 / TODO

- BLOCKER：远程核心 = oashya Tracker（ONNXRuntime YOLO，式神/豆子/加速/冻结/概率UP 等 226 类目标检测 + 跟踪），配合 hya_device 的**窗口后台截图 / 窗口消息点击**逐帧决策撒豆。当前引擎只有 ADB 截屏/点击 + 模板/OCR，没有模型推理原语，也没有窗口后台控制通道。
- assets 中的 226 类 hya_* 图（beans/resf/resr/train 等）是 ML 模型输入/训练素材，不是模板匹配素材；即使全部导入也无法还原检测+跟踪策略。
- SwitchOnmyoji（切换晴明/神乐等阴阳师）未迁移（素材已导入，逻辑未做）。
- 奖励/邀请/训练模式等分支未迁移。

## 6. 素材契约与通用模式

- 导入器 v3：模板 `roi` = remote `roiBack`，无 `action_roi` ⇒ `click` 落在匹配到的模板矩形（= 旧引擎行为）；`RuleClick` → `kind=region`；`RuleOcr` → `ocr_lines`；`RuleSwipe` → `_FROM`/`_TO` 两个 region；Component 素材 `legacy_Component_<Module>_<CONST>`。
- P-NAV / P-BATTLE v2 / P-RETURN / OCR 语义同批一 README。

## 7. 证据

- oasd task lint 绿；仅迁移入口导航（庭院→町中→百鬼夜行）。核心玩法被架构阻塞：无 oashya/Tracker 模型推理、无窗口后台截图/消息控制、无实时决策循环。
- 未真机/未回放：无设备、无对应场景回放帧；T4 素材引用全部解析到全局 `assets/assets.json`。
