# EvoZone（觉醒业原火）迁移规格

- 源仓库：`runhey/OnmyojiAutoScript@dev` · commit `8b4dc669b47838f42d283f14aa394844c1963c86` · 只读副本 `D:/OAS-dev/_verify/remote-oas`
- 目标：`tasks/EvoZone/`（统一任务模板 + 全局 `legacy_*` 素材）
- 迁移结论：**🔄** —— 行为矩阵见 §3，缺口见 §5

## 1. 读取的源文件

- `tasks/EvoZone/script_task.py`
- `tasks/EvoZone/config.py`
- `tasks/EvoZone/assets.py`
- `tasks/Component/GeneralBattle/general_battle.py (imported)`
- `tasks/Component/GeneralRoom/general_room.py (imported)`
- `tasks/Component/GeneralInvite/general_invite.py (imported)`
- `tasks/Component/GeneralBuff/general_buff.py (imported)`
- `tasks/Component/SwitchSoul/switch_soul.py (imported)`

## 2. 入口 / 出口

- 入口：`appear('庭院_探索灯笼')`；flow 首段 `ensure_main()` 回庭院。
- 出口：
  - `done` → result `success`
  - `failed` → result `retryable`

## 3. 行为矩阵

| # | 行为 | 远程实现（源码位置） | 新 DSL 映射 | 状态 |
|---|---|---|---|---|
| 1 | 进入觉醒页 | script_task.py:40,127,271 goto_page(page_awake_zones) | main→world→I_EXPLORATION_GOTO_AWAKE_ZONE→expect I_CHECK_AWAKE | 覆盖 |
| 2 | 选择麒麟 | script_task.py:73-90 evozone_enter(): 按 kirin_type 点 I_*_KIRIN 直到 I_FORM_TEAM | 4 分支 param('kirin_type') → click 对应素材；loop 直到 I_FORM_TEAM | 覆盖（单刷入口） |
| 3 | 选择层数 | script_task.py:92-100 check_layer: list_find(L_LAYER_LIST, layer) | click_target inline OCR（固定层数字面量；param 无法作 text） | 部分 |
| 4 | 阵容锁 | script_task.py:102-122 check_lock() | param('lock_team') 两分支点 EVOZONE_LOCK/UNLOCK | 覆盖 |
| 5 | 挑战循环 | script_task.py:269-317 run_alone(): I_EVOZONE_FIRE→run_general_battle | loop limit_count：点 I_EVOZONE_FIRE→P-BATTLE→回页；宠物奖励点击 | 覆盖（单刷） |
| 6 | 组队 leader/member | script_task.py:124-267 run_leader/run_member | — | 缺口：GeneralRoom/GeneralInvite 流程未迁移 |
| 7 | 觉醒加成 | script_task.py:42-46,61-64 open_buff/awake | — | 缺口 |
| 8 | 御魂切换 | script_task.py:33-38 SwitchSoul | — | 缺口 |
| 9 | 回庭院 | script_task.py:214,266,317 goto_page(page_main) | ensure_main | 覆盖 |

## 4. 参数映射

| key | type | default | 说明 |
|---|---|---|---|
| `kirin_type` | string | `雷麒麟` | 远程配置/迁移决策 |
| `layer` | string | `拾` | 远程配置/迁移决策 |
| `limit_count` | int | `30` | 远程配置/迁移决策 |
| `lock_team` | bool | `False` | 远程配置/迁移决策 |

## 5. 缺口 / TODO

- 层数列表：远程用 RuleList L_LAYER_LIST 的 list_find（OCR 项文本精确匹配数组 壹..拾 后点击）；新 flow 用 click_target inline OCR(roi=[124,84,77,632], text=固定字面量「拾」)；由于 ocr_text 的 text 是字面量，param('layer') 无法直接作为目标行（见下一条 TODO）。
- TODO(DSL)：click_target 的 ocr_text 文本是**字面量**，不能从 param/var 拼接；因此无法把 param('layer') 直接作为 OCR 目标行。需要引擎支持表达式 text 或按 rownum/坐标定位列表项。当前 flow 保留固定行文本（拾）作为默认层数。
- 组队模式（default user_status=leader）：创建房间/邀请队友/成员接管/自动匹配等 GeneralRoom+GeneralInvite 流程未迁移（素材已导入，逻辑量大，未在本轮实现）。
- 觉醒加成开关（GeneralBuff I_AWAKE）未迁移。
- 御魂预设切换（SwitchSoul）未迁移。
- limit_time（30 分钟）无时钟原语；limit_count 保留。
- 战斗为 P-BATTLE v2（准备/胜利/失败/奖励→回页），不含 battle_wait_strategy 的特殊奖励/统计页分支。

## 6. 素材契约与通用模式

- 导入器 v3：模板 `roi` = remote `roiBack`，无 `action_roi` ⇒ `click` 落在匹配到的模板矩形（= 旧引擎行为）；`RuleClick` → `kind=region`；`RuleOcr` → `ocr_lines`；`RuleSwipe` → `_FROM`/`_TO` 两个 region；Component 素材 `legacy_Component_<Module>_<CONST>`。
- P-NAV / P-BATTLE v2 / P-RETURN / OCR 语义同批一 README。

## 7. 证据

- oasd task lint 绿；覆盖：单刷(alone)路径：导航→麒麟选择→层数(inline OCR 列表点击)→阵容锁→挑战/战斗循环→回庭院；组队(leader/member)/加成/御魂切换未迁移。未真机/回放验证。
- 未真机/未回放：无设备、无对应场景回放帧；T4 素材引用全部解析到全局 `assets/assets.json`。

## 8. Polish round (2026-11-01)

- 修复不可达分支（关键）：挑战循环原先 `if appear(I_EVOZONE_FIRE) → break`，导致循环体里的点击挑战/准备/战斗/结算整段永远不执行（任务‘跑完’却从未战斗）。现改为 `if fire → 点击+准备+战斗+结算+回页`，`else` 处理宠物奖励并累计 miss，连续 3 次无挑战按钮才 break。
- `meta.entry.requires` 对齐 `legacy_GameUi_I_CHECK_MAIN`。


