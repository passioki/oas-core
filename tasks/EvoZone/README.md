# EvoZone（觉醒业原火）

> 来源：`runhey/OnmyojiAutoScript@dev`（只读 checkout `D:/OAS-dev/_verify/remote-oas`，commit `8b4dc669b478`）。
> 迁移方式：行为抽象 + 既有 oas-flow DSL（ENGINE-DESIGN §22），无 Python 代码拷贝。

## 读取的源文件

- `tasks/EvoZone/script_task.py`
- `tasks/EvoZone/config.py`
- `tasks/EvoZone/assets.py`
- `tasks/Component/GeneralBattle/general_battle.py (imported)`
- `tasks/Component/GeneralRoom/general_room.py (imported)`
- `tasks/Component/GeneralInvite/general_invite.py (imported)`
- `tasks/Component/GeneralBuff/general_buff.py (imported)`
- `tasks/Component/SwitchSoul/switch_soul.py (imported)`

## 行为摘要

庭院→探索→觉醒：按 kirin_type 选麒麟，按层数点击列表，锁/解锁阵容，loop limit_count 次「挑战→准备→战斗→回页」；回庭院。仅单刷(alone)路径。

## 参数 / 出口

- `kirin_type` (string) default='雷麒麟'
- `layer` (string) default='拾'
- `limit_count` (int) default=30
- `lock_team` (bool) default=False

- exit `done` → `success`
- exit `failed` → `retryable`

## 行为矩阵（详见 migration/specs/EvoZone.md）

- 进入觉醒页：覆盖
- 选择麒麟：覆盖（单刷入口）
- 选择层数：部分
- 阵容锁：覆盖
- 挑战循环：覆盖（单刷）
- 组队 leader/member：缺口：GeneralRoom/GeneralInvite 流程未迁移
- 觉醒加成：缺口
- 御魂切换：缺口
- 回庭院：覆盖

## 已知缺口 / TODO

- 层数列表：远程用 RuleList L_LAYER_LIST 的 list_find（OCR 项文本精确匹配数组 壹..拾 后点击）；新 flow 用 click_target inline OCR(roi=[124,84,77,632], text=固定字面量「拾」)；由于 ocr_text 的 text 是字面量，param('layer') 无法直接作为目标行（见下一条 TODO）。
- TODO(DSL)：click_target 的 ocr_text 文本是**字面量**，不能从 param/var 拼接；因此无法把 param('layer') 直接作为 OCR 目标行。需要引擎支持表达式 text 或按 rownum/坐标定位列表项。当前 flow 保留固定行文本（拾）作为默认层数。
- 组队模式（default user_status=leader）：创建房间/邀请队友/成员接管/自动匹配等 GeneralRoom+GeneralInvite 流程未迁移（素材已导入，逻辑量大，未在本轮实现）。
- 觉醒加成开关（GeneralBuff I_AWAKE）未迁移。
- 御魂预设切换（SwitchSoul）未迁移。
- limit_time（30 分钟）无时钟原语；limit_count 保留。
- 战斗为 P-BATTLE v2（准备/胜利/失败/奖励→回页），不含 battle_wait_strategy 的特殊奖励/统计页分支。

## 复用的通用模式

- **P-NAV**：`legacy_GameUi_I_CHECK_MAIN` → `click legacy_GameUi_I_*_GOTO_*`（模板点击 = 匹配到的模板矩形）→ 目标页 marker。
- **P-BATTLE v2**：`Component_GeneralBattle_I_PREPARE_HIGHLIGHT` → 点准备 → 等 `I_WIN/I_FALSE/I_REWARD/I_REWARD_GOLD` → 逐一点掉胜负横幅与奖励 → 等任务页 marker。
- **P-RETURN**：`GlobalGame_I_UI_BACK_*` / `GameUi_I_BACK_YOLLOW` / 坐标兜底 → `I_CHECK_MAIN`。
- **OCR**：`RuleOcr` 已导入为 `ocr_lines`；计数 `ocr_digit`，条件 `text_visible`，按行点击 `click_target {kind:ocr_text, sel|roi, text}`。运行需 `--model/--det-model`。

## 证据

- `oasd task lint --tasks-dir tasks --task EvoZone` → 0 errors；状态见 `migration/status/EvoZone.json`。
- 未真机 / 未回放验证（迁移会话无可用设备与对应场景回放）。
