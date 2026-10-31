# WantedQuests（悬赏封印）

> 来源：`runhey/OnmyojiAutoScript@dev`（只读 checkout `D:/OAS-dev/_verify/remote-oas`，commit `8b4dc669b478`）。
> 迁移方式：行为抽象 + 既有 oas-flow DSL（ENGINE-DESIGN §22），无 Python 代码拷贝。

## 读取的源文件

- `tasks/WantedQuests/script_task.py`
- `tasks/WantedQuests/config.py`
- `tasks/WantedQuests/assets.py`
- `tasks/WantedQuests/explore.py`
- `tasks/Secret/script_task.py (imported secret zone)`
- `tasks/GameUi/game_ui.py, tasks/GameUi/page.py (imported)`

## 行为摘要

庭院→打开悬赏封印界面→领取封印宝箱；任务列表识别、4 类悬赏执行（含协作邀请/追踪/探索战斗）逻辑量大，当前版本未迁移，以 blocked 退出。

## 参数 / 出口

- 无参数

- exit `done` → `success`
- exit `no_target` → `blocked`
- exit `failed` → `retryable`

## 行为矩阵（详见 migration/specs/WantedQuests.md）

- 打开悬赏封印界面：部分
- 领取宝箱：部分（I_TREASURE_BOX_CLICK 未覆盖）
- 追踪/协作邀请：缺口
- 寻找并执行悬赏：缺口
- 探索战斗：缺口

## 已知缺口 / TODO

- 核心逻辑量大且强依赖 OCR 文本处理：find_wq 扫描列表、O_WQ_TYPE/INFO 判定 4 类任务、O_WQ_TEXT_ALL 文本、C_WQ_D* 战斗、协作邀请（GeneralInvite 全套）、追踪开关、探索战斗（WQExplore）——本轮未逐函数映射。
- 远程 pre_work 依赖 I_TRACE_DISABLE/ENABLE 全量追踪判定 + 多种特判（特殊主界面 C_SPECIAL_MAIN、现世任务 I_WQ_TRACE_ONE_REALWORLD）。
- before_end 时间窗口/next_run 排期无时钟原语 → scheduler windows。
- 此状态为「已映射入口、主循环未实现」的 🔄，不应视为可用任务。

## 复用的通用模式

- **P-NAV**：`legacy_GameUi_I_CHECK_MAIN` → `click legacy_GameUi_I_*_GOTO_*`（模板点击 = 匹配到的模板矩形）→ 目标页 marker。
- **P-BATTLE v2**：`Component_GeneralBattle_I_PREPARE_HIGHLIGHT` → 点准备 → 等 `I_WIN/I_FALSE/I_REWARD/I_REWARD_GOLD` → 逐一点掉胜负横幅与奖励 → 等任务页 marker。
- **P-RETURN**：`GlobalGame_I_UI_BACK_*` / `GameUi_I_BACK_YOLLOW` / 坐标兜底 → `I_CHECK_MAIN`。
- **OCR**：`RuleOcr` 已导入为 `ocr_lines`；计数 `ocr_digit`，条件 `text_visible`，按行点击 `click_target {kind:ocr_text, sel|roi, text}`。运行需 `--model/--det-model`。

## 证据

- `oasd task lint --tasks-dir tasks --task WantedQuests` → 0 errors；状态见 `migration/status/WantedQuests.json`。
- 未真机 / 未回放验证（迁移会话无可用设备与对应场景回放）。
