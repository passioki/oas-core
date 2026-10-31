# Orochi 迁移规格

- 远程源：`runhey/OnmyojiAutoScript@dev` · commit `8b4dc669b47838f42d283f14aa394844c1963c86`（只读 `D:\OAS-dev\_verify\remote-oas`）
- 状态：**🔄**（单人模式主链路覆盖；层数选择（RuleList）、队长/队员/野队、御魂切换、真蛇检测联动未迁移）
- 已读文件：
  - `tasks/Orochi/script_task.py`
  - `tasks/Orochi/config.py`
  - `tasks/Orochi/page.py`
  - `tasks/Orochi/assets.py`
  - `tasks/Orochi/o/image.json`
  - `tasks/GameUi/page.py`
- 共享阅读：`tasks/Component/GeneralBattle/{general_battle.py,battle.py,battle_wait.py}`、`tasks/Component/GeneralRoom/general_room.py`、`tasks/GameUi/default_pages.py`（导航图）

## 1. 上游行为矩阵

| # | 条件 / 输入 | 行为 |
|---|---|---|
| 1 | 御魂切换 | 方式一/方式二 + 按层 `orochi_switch_soul`（十/悲鸣/神罚/虚无） |
| 2 | 御魂加成（soul_buff_enable） | 开战前后：回庭院→开加成→勾选御魂加成→关面板 |
| 3 | `LEADER`（默认） | 御魂页→选层→锁队→`I_FORM_TEAM` 建房→私密→邀请循环→通用战斗；限次数/限时→exit_room/exit_team→回庭院 |
| 4 | `MEMBER` | 等邀请/进房→`wait_battle`→通用战斗；限次/限时→退出房间/战斗→回庭院（若在战斗则先退） |
| 5 | `ALONE` | 御魂页→选层→锁队→循环：`I_OROCHI_FIRE` 挑战→通用战斗；限次/限时退出→返回 |
| 6 | `WILD`（野队） | 建房（公开）→循环：等待开始→限次/限时→`I_OROCHI_WILD_FIRE` 点击→通用战斗；失败则 exit |
| 7 | 战斗结算 | 重写 `battle_wait`：`I_WIN` 点（随机 C_WIN 区域）→ 贪吃鬼/御魂溢出/`I_REWARD` 处理 → `I_FALSE` 失败 |
| 8 | 真蛇联动 | `check_true_orochi_enable`：战后回御魂页检测 `I_FIND_TS`，命中则把 TrueOrochi 设为立刻运行（调度写回） |
| 9 | 结束 | `set_next_run(success)` + `TaskEnd` |

## 2. oas-core 映射与覆盖

| 上游行为 | 本迁移实现 | 状态 |
|---|---|---|
| 庭院→御魂副本→八岐大蛇 | `cp(680,152)`→`cp(1216,186)`→`I_EXPLORATION_GOTO_SOUL_ZONE`→`I_OROCHI`→`I_OROCHI_CHECK_11` | ✅ |
| 锁队 | `I_OROCHI_UNLOCK` 出现则点击（点成锁定） | ✅ |
| 单人循环挑战 | `loop max_battles`：`I_OROCHI_FIRE` ⇒ 点击 → `battle_cycle(rounds=100)` | ✅ |
| 层数选择 | 未迁移：`L_LAYER_LIST` 是 RuleList（13 项、需滚动），未导入且 flow 无列表原语 → 用游戏内当前层 | 🔄 TODO |
| LEADER/MEMBER/WILD | 未迁移（建房/邀请/等待/野队匹配子流程） | 🔄 TODO |
| 御魂加成 / 御魂切换 / 真蛇调度联动 | 未迁移（配置项 + 调度器写回） | 不迁移 |

## 3. Flow 结构要点

- `I_OROCHI_FIRE` 与 `I_OROCHI_CHECK_11` 都作为「回到御魂页」的结算完成判据（不同层/结算态二选一）。
- 战斗结算用 GeneralBattle 资产；真蛇入口 `I_FIND_TS` 未在本 flow 中检测（见 TODO）。

## 4. 素材（全部为全局 `assets/assets.json`）

所有引用素材都是全局 `assets/assets.json` 的 id（本任务不新增素材、无 images/）。导入语义：`template` 的 `roi` 是旧仓搜索窗（roiBack），点击落在**匹配到的模板外接框**；`region` 是旧仓 RuleClick 的精确操作区域；OCR 资产为 `ocr_lines`（旧 RuleOcr）。

- `庭院_探索灯笼`
- `世界_章节栏`
- `legacy_GameUi_I_CHECK_EXPLORATION`
- `legacy_GameUi_I_CHECK_SOUL_ZONES`
- `legacy_GameUi_I_EXPLORATION_GOTO_SOUL_ZONE`
- `legacy_Exploration_I_CHECK_PLAY_TITLE`
- `legacy_Orochi_I_OROCHI`
- `legacy_Orochi_I_OROCHI_FIRE`
- `legacy_Orochi_I_OROCHI_LOCK`
- `legacy_Orochi_I_OROCHI_UNLOCK`
- `legacy_Orochi_I_OROCHI_CHECK_11`
- `legacy_Orochi_I_FORM_TEAM`
- `legacy_Component_GeneralBattle_{...}`

## 5. 差异 / 待办

1. 层数选择需 RuleList（13 项滚动）原语，未迁移
2. 队长/队员/野队模式未迁移
3. 御魂加成/御魂切换/真蛇调度联动未迁移
4. 无实机/回放证据

## 8. Polish round (2026-11-01)

- 入口链修复：删除无守卫的硬编码 `click_point(1216,186)`（玩法 tab，非必需）；探索灯笼点击加 `requires=page_main / expect=page_exploration`；御魂入口 `requires` 从玩法 tab 标记改为 `legacy_GameUi_I_CHECK_EXPLORATION`。
- 八岐大蛇入口守卫改为 `legacy_GameUi_I_CHECK_SOUL_ZONES`，页面判定从单一 `CHECK_11` 改为 `CHECK_10/11/12/13` 任一。
- 阵容锁方向修正：默认 `lock_team_enable=false` 的语义是「确保解锁」——原 flow 在 `I_OROCHI_UNLOCK` 出现时点 UNLOCK（反而上锁）；现改为 `I_OROCHI_LOCK` 出现时点 LOCK 解锁，否则记录已解锁。
- 回庭院标记统一 `legacy_GameUi_I_CHECK_MAIN`；`meta.entry.requires` 对齐。


