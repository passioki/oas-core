# OtherWorldTwilight 迁移规格

- 远程源：`runhey/OnmyojiAutoScript@dev` · commit `8b4dc669b47838f42d283f14aa394844c1963c86`（只读 `D:\OAS-dev\_verify\remote-oas`）
- 状态：**🔄**（单人模式覆盖；队长/队员（建房/邀请/等待）未迁移）
- 已读文件：
  - `tasks/OtherWorldTwilight/script_task.py`
  - `tasks/OtherWorldTwilight/config.py`
  - `tasks/OtherWorldTwilight/page.py`
  - `tasks/OtherWorldTwilight/owt/image.json`
  - `tasks/GameUi/page.py`
  - `tasks/GameUi/default_pages.py`
- 共享阅读：`tasks/Component/GeneralBattle/{general_battle.py,battle.py,battle_wait.py}`、`tasks/Component/GeneralRoom/general_room.py`、`tasks/GameUi/default_pages.py`（导航图）

## 1. 上游行为矩阵

| # | 条件 / 输入 | 行为 |
|---|---|---|
| 1 | 御魂切换（方式一/二） | `switch_soul.enable`/`enable_switch_by_name` ⇒ 式神录切御魂 |
| 2 | `user_status=LEADER`（默认） | `run_leader()`：进彼世逢魔页→锁队→点 I_OWT_TEAM 创建队伍→`create_room()`→`ensure_private()`→`create_ensure()`→邀请循环（`check_and_invite`/`run_invite`）→`run_general_battle`；达到 `limit_count`(30) 或 `limit_time`(30min) 退出→exit_room/exit_team |
| 3 | `user_status=MEMBER` | `run_member()`：主界面等邀请/进房→`wait_battle`→`run_general_battle`；限次数/限时后 exit_room/exit_battle |
| 4 | `user_status=ALONE` | `run_alone()`：进彼世逢魔页→锁队→循环：在本页点 `I_OWT_FIRE` 挑战；在战斗准备/战斗/结算页走通用战斗；未知页 10s 超时回彼世逢魔页；限次数/限时退出 |
| 5 | 结束 | 回庭院；`set_next_run(finish, success)` |

## 2. oas-core 映射与覆盖

| 上游行为 | 本迁移实现 | 状态 |
|---|---|---|
| 庭院→町中→逢魔之时→彼世逢魔 | `I_MAIN_GOTO_TOWN`→`I_TOWN_GOTO_DEMON_ENCOUNTER`→`I_DE_TO_OTHER_WORLD` | ✅ |
| ALONE 模式 | 锁队（I_OWT_UNLOCK→点）→循环 `I_OWT_FIRE` + `battle_cycle`，`max_battles` 上限 | ✅ |
| LEADER/MEMBER 模式 | 未迁移：需要 建房（I_CREATE_ROOM/CREATE_ENSURE/ENSURE_PRIVATE）+ 邀请（I_ADD_2→I_INVITE_ENSURE）+ 房间等待 的完整子流程，素材已导入但组合/时序待下一轮 | 🔄 TODO |
| 御魂切换/邀请配置 | 未迁移（配置职责） | 不迁移 |
| 返回庭院 | `back_to_main` 轮询返回按钮 | ✅ |

## 3. Flow 结构要点

- `if appear(I_OWT_TEAM)` 判断当前是组队页（队长视角）→ 直接 `finish(failed)` 并记录 TODO；否则走单人循环。
- 锁定队伍：点击 `I_OWT_UNLOCK`（未锁定的解锁图标）把它点成锁定态。

## 4. 素材（全部为全局 `assets/assets.json`）

所有引用素材都是全局 `assets/assets.json` 的 id（本任务不新增素材、无 images/）。导入语义：`template` 的 `roi` 是旧仓搜索窗（roiBack），点击落在**匹配到的模板外接框**；`region` 是旧仓 RuleClick 的精确操作区域；OCR 资产为 `ocr_lines`（旧 RuleOcr）。

- `庭院_探索灯笼`
- `legacy_GameUi_I_MAIN_GOTO_TOWN`
- `legacy_GameUi_I_TOWN_GOTO_DEMON_ENCOUNTER`
- `legacy_GameUi_I_CHECK_TOWN`
- `legacy_GameUi_I_CHECK_DEMON_ENCOUNTER`
- `legacy_OtherWorldTwilight_I_DE_TO_OTHER_WORLD`
- `legacy_OtherWorldTwilight_I_OWT_FIRE`
- `legacy_OtherWorldTwilight_I_OWT_TEAM`
- `legacy_OtherWorldTwilight_I_OWT_LOCK`
- `legacy_OtherWorldTwilight_I_OWT_UNLOCK`
- `legacy_GlobalGame_I_UI_BACK_{RED,YELLOW}`
- `legacy_Component_GeneralBattle_{I_PREPARE_HIGHLIGHT,I_BATTLE_INFO,I_WIN,I_FALSE,I_GREED_GHOST,I_OVER_GHOST,I_REWARD}`

## 5. 差异 / 待办

1. 队长/队员模式（建房/邀请/等待开战）未迁移
2. limit_time 时间上限未在 flow 内实现（仅 max_battles 次数上限）
3. 无实机/回放证据
