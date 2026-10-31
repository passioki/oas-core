# EternitySea 迁移规格

- 远程源：`runhey/OnmyojiAutoScript@dev` · commit `8b4dc669b47838f42d283f14aa394844c1963c86`（只读 `D:\OAS-dev\_verify\remote-oas`）
- 状态：**🔄**（单人模式主链路覆盖；队长/队员（建房/邀请/等待）未迁移，层数选择用坐标近似）
- 已读文件：
  - `tasks/EternitySea/script_task.py`
  - `tasks/EternitySea/config.py`
  - `tasks/EternitySea/assets.py`
  - `tasks/EternitySea/res/{image,list}.json`
  - `tasks/Orochi/config.py`
  - `tasks/GameUi/page.py`
- 共享阅读：`tasks/Component/GeneralBattle/{general_battle.py,battle.py,battle_wait.py}`、`tasks/Component/GeneralRoom/general_room.py`、`tasks/GameUi/default_pages.py`（导航图）

## 1. 上游行为矩阵

| # | 条件 / 输入 | 行为 |
|---|---|---|
| 1 | 御魂切换 | 两套 switch_soul 配置（enable/enable_switch_by_name） |
| 2 | `user_status=LEADER`（默认） | `_prepare_eternity_sea`（御魂副本→永生之海→选层→锁队）→建房→私密→邀请循环→通用战斗；limit_count(30)/limit_time(30min)→`_wait_room_stable`→exit_room/exit_team→`_back_to_soul_zones`→回庭院 |
| 3 | `user_status=MEMBER` | 进永生之海→等邀请/进房→`wait_battle`→通用战斗→退出房间/战斗→回庭院 |
| 4 | `user_status=ALONE` | 御魂副本→永生之海→选层→**必须锁队**（否则 RequestHumanTakeover）→循环：本页点 `I_ETERNITY_SEA_FIRE` 挑战→通用战斗；次数/时间上限退出 |
| 5 | 返回 | 永生之海页无 HOME，点 `I_BACK_BOTTOM` 回御魂副本选择页；确认弹窗 `I_UI_CONFIRM`；再回庭院 |

## 2. oas-core 映射与覆盖

| 上游行为 | 本迁移实现 | 状态 |
|---|---|---|
| 庭院→探索→玩法→御魂副本 | `cp(680,152)`→`cp(1216,186)`→`I_EXPLORATION_GOTO_SOUL_ZONE` | ✅ |
| 进永生之海 | `click(I_ETERNITY_SEA)`（右上角入口）→ `expect I_FORM_TEAM` | ✅ |
| 选层 1~4 | `click_point(165, 150+87*(layer-1))` 坐标近似（RuleList 未导入、无列表原语）；层数参数 1~4 | 🔄 近似 |
| 锁队 | `I_ETERNITYSEA_UNLOCK` 出现则点击（点成锁定） | ✅ |
| 单人循环挑战 | `loop max_battles`：`I_ETERNITY_SEA_FIRE` ⇒ 点击 → `battle_cycle(rounds=100)`，回本页继续 | ✅ |
| 返回 | `I_BACK_BOTTOM` → 御魂副本页 → `back_to_main` 轮询返回 | ✅ |
| 队长/队员模式 | 未迁移（建房/邀请/等待子流程 + `_wait_room_stable`） | 🔄 TODO |
| 御魂切换 | 未迁移（配置项） | 不迁移 |

## 3. Flow 结构要点

- `layer` 参数映射：壹=150、贰=237、叁=324、肆=411（旧 L_LAYER_LIST 项高 87、首项中心 y=150.5）。
- `I_ETERNITY_SEA_FIRE` 同时是「在本页」与「可挑战」判据，`battle_cycle` 也用它作为结算完成条件。

## 4. 素材（全部为全局 `assets/assets.json`）

所有引用素材都是全局 `assets/assets.json` 的 id（本任务不新增素材、无 images/）。导入语义：`template` 的 `roi` 是旧仓搜索窗（roiBack），点击落在**匹配到的模板外接框**；`region` 是旧仓 RuleClick 的精确操作区域；OCR 资产为 `ocr_lines`（旧 RuleOcr）。

- `庭院_探索灯笼`
- `世界_章节栏`
- `legacy_GameUi_I_CHECK_EXPLORATION`
- `legacy_GameUi_I_CHECK_SOUL_ZONES`
- `legacy_GameUi_I_EXPLORATION_GOTO_SOUL_ZONE`
- `legacy_Exploration_I_CHECK_PLAY_TITLE`
- `legacy_EternitySea_I_ETERNITY_SEA`
- `legacy_EternitySea_I_FORM_TEAM`
- `legacy_EternitySea_I_ETERNITY_SEA_FIRE`
- `legacy_EternitySea_I_ETERNITYSEA_UNLOCK`
- `legacy_EternitySea_I_NEWETERNITYSEA_LOCK`
- `legacy_EternitySea_I_BACK_BOTTOM`
- `legacy_Component_GeneralBattle_{...}`
- `legacy_GlobalGame_I_UI_CONFIRM`

## 5. 差异 / 待办

1. 队长/队员模式未迁移（建房/邀请/等待）
2. 选层为坐标近似（RuleList 未导入）
3. 御魂切换与时间上限未迁移
4. 无实机/回放证据
