# Hyakkiyakou（百鬼夜行）

> 来源：`runhey/OnmyojiAutoScript@dev`（只读 checkout `D:/OAS-dev/_verify/remote-oas`，commit `8b4dc669b478`）。
> 迁移方式：行为抽象 + 既有 oas-flow DSL（ENGINE-DESIGN §22），无 Python 代码拷贝。

## 读取的源文件

- `tasks/Hyakkiyakou/script_task.py`
- `tasks/Hyakkiyakou/config.py`
- `tasks/Hyakkiyakou/agent/agent.py`
- `tasks/Hyakkiyakou/slave/hya_slave.py`
- `tasks/Hyakkiyakou/slave/hya_device.py`
- `tasks/Component/SwitchOnmyoji/switch_onmyoji.py (imported)`

## 行为摘要

庭院→町中→百鬼夜行：仅迁移入口导航与页面检查；核心撒豆玩法依赖 oashya（YOLO 式神检测+跟踪）与窗口消息控制，当前引擎无模型推理/后台控制原语 → blocked。

## 参数 / 出口

- 无参数

- exit `done` → `success`
- exit `no_target` → `blocked`
- exit `failed` → `retryable`

## 行为矩阵（详见 migration/specs/Hyakkiyakou.md）

- 进入百鬼夜行：覆盖
- 阴阳师切换：缺口
- 撒豆主循环：⛔ 架构阻塞
- 奖励/统计/训练：缺口

## 已知缺口 / TODO

- BLOCKER：远程核心 = oashya Tracker（ONNXRuntime YOLO，式神/豆子/加速/冻结/概率UP 等 226 类目标检测 + 跟踪），配合 hya_device 的**窗口后台截图 / 窗口消息点击**逐帧决策撒豆。当前引擎只有 ADB 截屏/点击 + 模板/OCR，没有模型推理原语，也没有窗口后台控制通道。
- assets 中的 226 类 hya_* 图（beans/resf/resr/train 等）是 ML 模型输入/训练素材，不是模板匹配素材；即使全部导入也无法还原检测+跟踪策略。
- SwitchOnmyoji（切换晴明/神乐等阴阳师）未迁移（素材已导入，逻辑未做）。
- 奖励/邀请/训练模式等分支未迁移。

## 复用的通用模式

- **P-NAV**：`legacy_GameUi_I_CHECK_MAIN` → `click legacy_GameUi_I_*_GOTO_*`（模板点击 = 匹配到的模板矩形）→ 目标页 marker。
- **P-BATTLE v2**：`Component_GeneralBattle_I_PREPARE_HIGHLIGHT` → 点准备 → 等 `I_WIN/I_FALSE/I_REWARD/I_REWARD_GOLD` → 逐一点掉胜负横幅与奖励 → 等任务页 marker。
- **P-RETURN**：`GlobalGame_I_UI_BACK_*` / `GameUi_I_BACK_YOLLOW` / 坐标兜底 → `I_CHECK_MAIN`。
- **OCR**：`RuleOcr` 已导入为 `ocr_lines`；计数 `ocr_digit`，条件 `text_visible`，按行点击 `click_target {kind:ocr_text, sel|roi, text}`。运行需 `--model/--det-model`。

## 证据

- `oasd task lint --tasks-dir tasks --task Hyakkiyakou` → 0 errors；状态见 `migration/status/Hyakkiyakou.json`。
- 未真机 / 未回放验证（迁移会话无可用设备与对应场景回放）。
