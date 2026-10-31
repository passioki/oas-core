# BondlingFairyland（契灵之境）迁移规格

- 源仓库：`runhey/OnmyojiAutoScript@dev` · commit `8b4dc669b47838f42d283f14aa394844c1963c86` · 只读副本 `D:/OAS-dev/_verify/remote-oas`
- 目标：`tasks/BondlingFairyland/`（统一任务模板 + 全局 `legacy_*` 素材）
- 迁移结论：**🔄** —— 行为矩阵见 §3，缺口见 §5

## 1. 读取的源文件

- `tasks/BondlingFairyland/script_task.py`
- `tasks/BondlingFairyland/battle.py`
- `tasks/BondlingFairyland/config.py`
- `tasks/BondlingFairyland/config_battle.py`
- `tasks/BondlingFairyland/assets.py`
- `tasks/BondlingFairyland/general_invite.py (imported)`
- `tasks/Component/GeneralBattle, GeneralRoom, SwitchSoul (imported)`

## 2. 入口 / 出口

- 入口：`appear('庭院_探索灯笼')`；flow 首段 `ensure_main()` 回庭院。
- 出口：
  - `done` → result `success`
  - `no_target` → result `blocked`
  - `failed` → result `retryable`

## 3. 行为矩阵

| # | 行为 | 远程实现（源码位置） | 新 DSL 映射 | 状态 |
|---|---|---|---|---|
| 1 | 进入契灵之境 | script_task.py run(): goto_page(page_mall) 设置商店开关 → goto_page(page_bondling_fairyland) | main→world→I_EXPLORATION_GOTO_BONDLING_FAIRYLAND→expect I_CHECK_BONDLING_FAIRYLAND | 覆盖（商店设置未迁移） |
| 2 | 进入结契区 | script_task.py run_alone/navigate: I_BALL_AREA/I_CHECK_AREA | loop 点 I_BALL_AREA 直到 I_CHECK_AREA | 部分 |
| 3 | 挑战 | script_task.py run_alone：I_BALL_FIRE→I_UI_CONFIRM→run_general_battle | loop：点 I_BALL_FIRE→确认→P-BATTLE→回页 | 部分 |
| 4 | 结契捕捉 | script_task.py run_catch + capture_setting + capture/*.png | — | 缺口（核心） |
| 5 | 契灵石头/商店 | script_task.py run_stone/run() mall 设置 | — | 缺口 |
| 6 | 搜索/组队/邀请 | script_task.py run_leader/run_member/run_search + general_invite | — | 缺口 |

## 4. 参数映射

| key | type | default | 说明 |
|---|---|---|---|
| `limit_count` | int | `5` | 远程配置/迁移决策 |

## 5. 缺口 / TODO

- 结契捕捉流程（run_catch + capture_setting：自动结契/极简模式/低中高品质/首次/连续开关、I_CAP_SUCCESS/I_CAP_FAILURE/I_CAP_AGAIN、回合数/球数 OCR）未迁移 —— 这是该任务的核心之一。
- 契灵种类识别（get_bondling_class: I_TOMB_GUARD/I_SNOWBALL/I_LITTLE_KURO/I_AZURE_BASAN）与球数/石板数 OCR 未迁移。
- 契灵石头购买 run_stone（I_BUY_PLUS/I_STONE_SURE）未迁移。
- 搜索模式 run_search（I_BF_SEARSH/I_BF_STORE）与队长/成员/邀请（GeneralRoom+GeneralInvite 全套）未迁移；仅单刷挑战骨架。
- BondlingBattle（战斗内策略）未迁移；用 P-BATTLE v2 通用骨架。
- limit_time/次数上限部分保留（limit_count）。

## 6. 素材契约与通用模式

- 导入器 v3：模板 `roi` = remote `roiBack`，无 `action_roi` ⇒ `click` 落在匹配到的模板矩形（= 旧引擎行为）；`RuleClick` → `kind=region`；`RuleOcr` → `ocr_lines`；`RuleSwipe` → `_FROM`/`_TO` 两个 region；Component 素材 `legacy_Component_<Module>_<CONST>`。
- P-NAV / P-BATTLE v2 / P-RETURN / OCR 语义同批一 README。

## 7. 证据

- oasd task lint 绿；已迁移：导航→契灵之境→结契区→挑战/确认→P-BATTLE→回庭院；结契捕捉小游戏等核心分支未迁移，flow 显式 blocked。
- 未真机/未回放：无设备、无对应场景回放帧；T4 素材引用全部解析到全局 `assets/assets.json`。
