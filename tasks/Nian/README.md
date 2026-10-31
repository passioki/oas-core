# Nian（年兽）

> 来源：`runhey/OnmyojiAutoScript@dev`（只读 checkout `D:/OAS-dev/_verify/remote-oas`，commit `8b4dc669b478`）。
> 迁移方式：行为抽象 + 既有 oas-flow DSL（ENGINE-DESIGN §22），无 Python 代码拷贝。

## 读取的源文件

- `tasks/Nian/script_task.py`
- `tasks/Nian/config.py`
- `tasks/Nian/assets.py`
- `tasks/Component/GeneralRoom/general_room.py (imported)`
- `tasks/Component/GeneralInvite/general_invite.py (imported)`
- `tasks/Component/GeneralBattle/general_battle.py (imported)`
- `tasks/GameUi/game_ui.py, tasks/GameUi/page.py (imported)`

## 行为摘要

庭院→组队→在组队区域列表点「年兽」→冷却检测（「再次挑战」文案）→自动匹配（>4 次不可进 → blocked）→等待准备页→自动战斗→回庭院。

## 参数 / 出口

- 无参数

- exit `done` → `success`
- exit `no_target` → `blocked`
- exit `failed` → `retryable`

## 已实现的行为（行为矩阵见 migration/specs/Nian.md）

- 进入组队+选年兽区域：部分（列表滚动为有界近似）
- 冷却检测：部分
- 自动匹配：覆盖
- 等待战斗：部分（无房间分支）
- 战斗：部分（通用骨架）
- 匹配超时取消：缺口：无时钟 + GeneralInvite 素材

## 已知缺口 / TODO

- 冷却剩余时间解析（远程 re.search(r'(\d+)时(\d+)分后可') + timedelta）无字符串解析原语 → 只能以 blocked 退出让调度器稍后重试，不设置精确 next_run。
- GeneralRoom.check_zones（L_TEAM_LIST 滚动列表 OCR + O_GR_ZONES_NAME）未迁移：用 inline OCR [26,106,360,549] 点击可见的「年兽」行 + 上滑 8 次替代；若年兽行多次滚动后仍不可见会失败。
- GeneralInvite（房间/邀人）未迁移：只处理「自动匹配」路径；匹配 240s 安全点击、480s 取消、被秒开接管战斗未覆盖。
- 普通战斗用 P-BATTLE 通用骨架，不含绿标/预设阵容。

## 复用的通用模式

- **P-NAV**：`legacy_GameUi_I_CHECK_MAIN` 守卫 → `click legacy_GameUi_I_MAIN_GOTO_*` / `legacy_GameUi_I_EXPLORATION_GOTO_*`（导入器已改为：模板 `click` = 匹配到的模板矩形，与旧引擎一致）。
- **P-BATTLE**：`战斗_准备按钮` → 点击 → `战斗_自动按钮` → 循环点安全点 (640,690) 直到任务页 marker 回来。
- **P-RETURN**：`legacy_GlobalGame_I_UI_BACK_YELLOW` / `legacy_GameUi_I_BACK_YOLLOW` / 坐标 (48,43) 兜底，直到 `legacy_GameUi_I_CHECK_MAIN`。
- **OCR**：RuleOcr 已由导入器转为 `ocr_lines`；计数用 `ocr_digit`，条件用 `text_visible`，按行点击用 `click_target` + `{kind:ocr_text, sel|roi, text}`。运行需 `--model`/`--det-model`。

## 证据

- `oasd task lint --tasks-dir tasks --task Nian` → 0 errors（见 migration/status/Nian.json）。
- 未真机 / 未回放验证（迁移会话无可用设备与对应场景回放）；素材 id 与 ROI 来自 remote dev 分支，素材均已在全局 `assets/assets.json` 解析（T4 绿）。
