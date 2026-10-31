# Duel（斗技）迁移规格

- 源仓库：`runhey/OnmyojiAutoScript@dev` · commit `8b4dc669b47838f42d283f14aa394844c1963c86` · 只读副本 `D:/OAS-dev/_verify/remote-oas`
- 目标：`tasks/Duel/`（统一任务模板 + 全局 `legacy_*` 素材）
- 迁移结论：**🔄** —— 行为矩阵见 §3，缺口见 §5

## 1. 读取的源文件

- `tasks/Duel/script_task.py`
- `tasks/Duel/config.py`
- `tasks/Duel/assets.py`
- `tasks/Component/GeneralBattle/general_battle.py (imported)`
- `tasks/Component/SwitchSoul/switch_soul.py (imported)`
- `tasks/Component/SwitchOnmyoji/switch_onmyoji.py (imported)`
- `tasks/GameUi/game_ui.py, tasks/GameUi/page.py (imported)`

## 2. 入口 / 出口

- 入口：`appear('庭院_探索灯笼')`；flow 首段 `ensure_main()` 回庭院。
- 出口：
  - `done` → result `success`
  - `failed` → result `retryable`

## 3. 行为矩阵

| # | 行为 | 远程实现（源码位置） | 新 DSL 映射 | 状态 |
|---|---|---|---|---|
| 1 | 进入斗技 | script_task.py:60-67 prepare_duel(): goto_page(page_duel) | main→I_MAIN_GOTO_TOWN→I_TOWN_GOTO_DUEL→expect I_CHECK_DUEL | 覆盖 |
| 2 | 开始战斗 | script_task.py:94-122 enter_battle(): 点 I_D_BATTLE/I_D_BATTLE2/I_D_BATTLE_PROTECT 直到准备页 | loop：点三类战斗按钮直到准备页出现 | 覆盖 |
| 3 | 准备阶段 | script_task.py:124-158 battle_prepare(): I_D_AUTO_ENTRY/I_D_PREPARE/ban 检查 | loop：点 I_D_AUTO_ENTRY 或 I_D_PREPARE；ban/名仕分支未迁移 | 部分 |
| 4 | 战斗等待 | script_task.py:160-260 wait_battle(): 结算/胜利/失败/奖励 | P-BATTLE v2（准备/胜负/奖励→回页） | 部分 |
| 5 | 分数/名仕/时间 | script_task.py:35-92,71-92 can_start_duel | — | 缺口：无时钟/分数目标逻辑 |
| 6 | 回庭院 | script_task.py:55 goto_page(page_main) | ensure_main | 覆盖 |

## 4. 参数映射

| key | type | default | 说明 |
|---|---|---|---|
| `max_battles` | int | `6` | 远程配置/迁移决策 |

## 5. 缺口 / TODO

- 目标分数/名仕(celeb)判定（O_D_SCORE OCR、I_BAN、O_D_BAN_NAME、C_DUEL_CLICK_5 自动上阵）未迁移：flow 只按 max_battles 次数循环，不打到分数目标不退出。
- 12:00-23:00 时间窗无时钟原语 → scheduler windows。
- 荣誉满退出（O_D_HONOR）、练习模式检测（I_BATTLE_WITH_TRAIN）、结算分享（I_D_VICTORY_SHARE）未迁移。
- 御魂/式神/阴阳师切换（switch_soul/switch_onmyoji/switch_all_soul）未迁移。
- P-BATTLE v2 不含斗技专有的准备阶段处理（名仕 ban、I_D_CHECK_BAN 等）。

## 6. 素材契约与通用模式

- 导入器 v3：模板 `roi` = remote `roiBack`，无 `action_roi` ⇒ `click` 落在匹配到的模板矩形（= 旧引擎行为）；`RuleClick` → `kind=region`；`RuleOcr` → `ocr_lines`；`RuleSwipe` → `_FROM`/`_TO` 两个 region；Component 素材 `legacy_Component_<Module>_<CONST>`。
- P-NAV / P-BATTLE v2 / P-RETURN / OCR 语义同批一 README。

## 7. 证据

- oasd task lint 绿；覆盖：主界面→战斗→准备页→自动上阵/准备→P-BATTLE→回页循环；分数目标/名仕/ban/荣誉/时间窗未迁移。未真机/回放验证。
- 未真机/未回放：无设备、无对应场景回放帧；T4 素材引用全部解析到全局 `assets/assets.json`。
