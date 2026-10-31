# TrueOrochi 迁移规格

- 远程源：`runhey/OnmyojiAutoScript@dev` · commit `8b4dc669b47838f42d283f14aa394844c1963c86`（只读 `D:\OAS-dev\_verify\remote-oas`）
- 状态：**🔄**（主链路（玩法 tab 次数判定→御魂页→真蛇入口→建房/挑战→多阶段战斗）覆盖；十层×10 触发与第二次存储入口未迁移）
- 已读文件：
  - `tasks/TrueOrochi/script_task.py`
  - `tasks/TrueOrochi/config.py`
  - `tasks/TrueOrochi/assets.py`
  - `tasks/TrueOrochi/st/{image,click}.json`
  - `tasks/Orochi/script_task.py`
  - `tasks/Orochi/page.py`
  - `tasks/Exploration/page.py`
- 共享阅读：`tasks/Component/GeneralBattle/{general_battle.py,battle.py,battle_wait.py}`、`tasks/Component/GeneralRoom/general_room.py`、`tasks/GameUi/default_pages.py`（导航图）

## 1. 上游行为矩阵

| # | 条件 / 输入 | 行为 |
|---|---|---|
| 1 | 启动 | `goto_page(page_gameplay)`（探索大地图·玩法 tab） |
| 2 | 本周次数 | `appear(I_ST_MODULE)` 为假 ⇒ 本周已打完：`check_times(True)` + 结束 |
| 3 | 御魂切换 | 方式一/方式二（配置） |
| 4 | 真蛇检测 | `goto_page(page_orochi)` + 0.5s → `appear(I_FIND_TS)` |
| 5 | 未找到真蛇 | `find_true_orochi=false` ⇒ 结束；否则切十层 `check_layer(TEN)` + 解锁 → 循环最多 10 次：点 `I_OROCHI_FIRE` 打十层，每次战后重新检测 `I_FIND_TS`；仍未出现 ⇒ 失败结束 |
| 6 | 真蛇战斗 | `run_true_orochi_battle()`：等 `I_ST_CREATE_ROOM`；点 `I_UI_CONFIRM`/`I_ST_FIRE`/`I_FIND_TS`；进入准备后点 `I_ST_FIRE_PREPARE`→点 `I_ST_AUTO_FALSE` 切自动；战斗循环：`I_GREED_GHOST` 处理、`I_ST_FRAME` 点按（多阶段推进）、280s 超时×3 ⇒ `GameStuckError` |
| 7 | 第二次入口 | 战后回御魂页重查 `I_FIND_TS`；若还在 ⇒ 再打一次（支持存储两次） |
| 8 | 结束 | `check_times(battle)` 设定 success/failure interval（3 天 / 1 天）；`TaskEnd` |

## 2. oas-core 映射与覆盖

| 上游行为 | 本迁移实现 | 状态 |
|---|---|---|
| 庭院→探索→玩法 tab | `cp(680,152)`→`cp(1216,186)` 且 `expect I_CHECK_PLAY_TITLE` | ✅ |
| 本周次数 | `if not appear(I_ST_MODULE)` ⇒ 回庭院 + `finish(no_resource)` | ✅ |
| 御魂副本→八岐大蛇页 | `I_EXPLORATION_GOTO_SOUL_ZONE`→`Orochi_I_OROCHI`→`I_OROCHI_CHECK_11` | ✅ |
| 真蛇入口检测+战斗 | `if appear(I_FIND_TS)` ⇒ 建房/确认/挑战循环 → `battle_cycle(rounds=150)` → 回庭院 | ✅（≤1 场） |
| 十层×10 触发真蛇 | 未迁移：需要层数 RuleList（13 项含滚动）选层 + 十层循环战斗 + 每轮重查 I_FIND_TS | 🔄 TODO |
| 第二次存储入口再战 | 未迁移（战后未重新导航回御魂页判定） | 🔄 TODO |
| 御魂切换/绿标/自动切层 | 未迁移（配置项） | 不迁移 |

## 3. Flow 结构要点

- 真蛇战斗有多阶段，`battle_cycle(rounds=150)` 用 GeneralBattle 资产处理准备/胜负/贪吃鬼/奖励，回到 `I_ST_CREATE_ROOM` 或 `I_FIND_TS` 即视为结算完成。
- `I_ST_MODULE` 只在玩法 tab 出现，作为「本周是否还有次数」的唯一判据。

## 4. 素材（全部为全局 `assets/assets.json`）

所有引用素材都是全局 `assets/assets.json` 的 id（本任务不新增素材、无 images/）。导入语义：`template` 的 `roi` 是旧仓搜索窗（roiBack），点击落在**匹配到的模板外接框**；`region` 是旧仓 RuleClick 的精确操作区域；OCR 资产为 `ocr_lines`（旧 RuleOcr）。

- `庭院_探索灯笼`
- `世界_章节栏`
- `legacy_GameUi_I_CHECK_EXPLORATION`
- `legacy_GameUi_I_CHECK_SOUL_ZONES`
- `legacy_GameUi_I_EXPLORATION_GOTO_SOUL_ZONE`
- `legacy_Exploration_I_CHECK_PLAY_TITLE`
- `legacy_TrueOrochi_I_ST_MODULE`
- `legacy_TrueOrochi_I_FIND_TS`
- `legacy_TrueOrochi_I_ST_CREATE_ROOM`
- `legacy_TrueOrochi_I_ST_FIRE`
- `legacy_TrueOrochi_I_ST_FIRE_PREPARE`
- `legacy_TrueOrochi_I_ST_AUTO_FALSE`
- `legacy_TrueOrochi_I_ST_FRAME`
- `legacy_Orochi_I_OROCHI`
- `legacy_Orochi_I_OROCHI_CHECK_11`
- `legacy_Component_GeneralBattle_{...}`
- `legacy_GlobalGame_I_UI_CONFIRM`

## 5. 差异 / 待办

1. 十层×10 触发真蛇未迁移（需 RuleList 选层 + 循环战斗）
2. 第二次存储真蛇入口未迁移
3. 御魂切换/绿标配置未迁移
4. 无实机/回放证据
