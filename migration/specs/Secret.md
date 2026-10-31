# Secret（秘闻副本）迁移规格

- 源仓库：`runhey/OnmyojiAutoScript@dev` · commit `8b4dc669b47838f42d283f14aa394844c1963c86` · 只读副本 `D:/OAS-dev/_verify/remote-oas`
- 目标：`tasks/Secret/`（统一任务模板 + 全局 `legacy_*` 素材）
- 迁移结论：**🔄** —— 行为矩阵见 §3，缺口见 §5

## 1. 读取的源文件

- `tasks/Secret/script_task.py`
- `tasks/Secret/config.py`
- `tasks/Secret/assets.py`
- `tasks/Component/GeneralBattle/general_battle.py (imported)`
- `tasks/Component/SwitchSoul/switch_soul.py (imported)`
- `tasks/Component/GeneralBuff/general_buff.py (imported)`
- `tasks/WeeklyTrifles/assets.py (I_WT_SE_SHARE)`

## 2. 入口 / 出口

- 入口：`appear('庭院_探索灯笼')`；flow 首段 `ensure_main()` 回庭院。
- 出口：
  - `done` → result `success`
  - `failed` → result `retryable`

## 3. 行为矩阵

| # | 行为 | 远程实现（源码位置） | 新 DSL 映射 | 状态 |
|---|---|---|---|---|
| 1 | 进入秘闻页 | script_task.py:53 goto_page(page_secret_zones) | main→world→I_EXPLORATION_GOTO_SECRET_ZONES→expect I_CHECK_SECRET_ZONES | 覆盖 |
| 2 | 进入副本 | script_task.py:57 ui_click(I_SE_ENTER, I_SE_FIRE) + I_SE_PLACEMENT 检查 | loop 点 I_SE_ENTER 直到 I_SE_FIRE；if I_SE_PLACEMENT 才继续 | 覆盖 |
| 3 | 按层选择 | script_task.py:143-220 find_battle（OCR 列表 + 滑动） | — | 缺口 |
| 4 | 战斗循环 | script_task.py:64-128 click_battle→run_general_battle，I_SE_FINISHED_1 结束 | loop max_battles：点 I_SE_FIRE→P-BATTLE→检测 I_SE_FINISHED_1 | 覆盖（层选择缺口） |
| 5 | 加成/御魂 | script_task.py:47-52,133-139 + buffs | — | 缺口 |
| 6 | 回庭院 | script_task.py:130-132 | ensure_main | 覆盖 |

## 4. 参数映射

| key | type | default | 说明 |
|---|---|---|---|
| `max_battles` | int | `20` | 远程配置/迁移决策 |

## 5. 缺口 / TODO

- 层选择 find_battle（O_SE_LAYER_*/O_SE_NO_PASS 的 OCR 定位、向下滑动、'尚未' 文本判定）未迁移 —— flow 直接点挑战(即默认当前层)，可能重复打已通关层。
- 秘闻已完成/本周已分享判定：远程用 I_WT_SE_SHARE 与 O_SE_TOTAL_TIME 文本 contains '尚未'；flow 只用 I_SE_FINISHED_1（缺口）。
- 加成开关（secret_gold_50/100 open_buff）未迁移；御魂预设切换未迁移。
- layer_9/layer_10 的开关（con.layer_9/layer_10）未映射为参数，flow 统一按 max_battles。
- P-BATTLE v2 不含 GeneralBattle 的 DE_WIN/特殊奖励分支。

## 6. 素材契约与通用模式

- 导入器 v3：模板 `roi` = remote `roiBack`，无 `action_roi` ⇒ `click` 落在匹配到的模板矩形（= 旧引擎行为）；`RuleClick` → `kind=region`；`RuleOcr` → `ocr_lines`；`RuleSwipe` → `_FROM`/`_TO` 两个 region；Component 素材 `legacy_Component_<Module>_<CONST>`。
- P-NAV / P-BATTLE v2 / P-RETURN / OCR 语义同批一 README。

## 7. 证据

- oasd task lint 绿；覆盖：导航→秘闻页→进入副本→按层挑战/战斗循环→完成检测→回庭院；层选择 OCR/加成/御魂切换/秘闻分享检测未覆盖。未真机/回放验证。
- 未真机/未回放：无设备、无对应场景回放帧；T4 素材引用全部解析到全局 `assets/assets.json`。
