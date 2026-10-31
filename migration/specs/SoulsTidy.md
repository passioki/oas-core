# SoulsTidy（御魂整理）迁移规格

- 源仓库：`runhey/OnmyojiAutoScript@dev` · commit `8b4dc669b47838f42d283f14aa394844c1963c86` · 只读副本 `D:/OAS-dev/_verify/remote-oas`
- 目标：`tasks/SoulsTidy/`（统一任务模板 + 全局 `legacy_*` 素材）
- 迁移结论：**🔄** —— 行为矩阵见 §3，缺口见 §5

## 1. 读取的源文件

- `tasks/SoulsTidy/script_task.py`
- `tasks/SoulsTidy/config.py`
- `tasks/SoulsTidy/assets.py`
- `tasks/GameUi/game_ui.py, tasks/GameUi/page.py (imported)`
- `tasks/GlobalGame/assets.py (UI_CONFIRM 等)`

## 2. 入口 / 出口

- 入口：`appear('庭院_探索灯笼')`；flow 首段 `ensure_main()` 回庭院。
- 出口：
  - `done` → result `success`
  - `failed` → result `retryable`

## 3. 行为矩阵

| # | 行为 | 远程实现（源码位置） | 新 DSL 映射 | 状态 |
|---|---|---|---|---|
| 1 | 进入御魂界面 | script_task.py:20-23 goto_page(page_shikigami_records); goto_souls:35-50（I_ST_GREED && I_ST_TIDY 为就绪态） | main→I_MAIN_GOTO_SHIKIGAMI_RECORDS→loop 点 I_ST_REPLACE/I_ST_SOULS/区域 C_ST_DETAIL 直到 I_ST_TIDY | 覆盖 |
| 2 | 喂贪吃鬼 | script_task.py:65-83 I_ST_GREED→I_ST_GREED_HABIT→I_ST_FEED_NOW（最多 3 次） | loop：I_ST_GREED→HABIT；feed var≤3 点 I_ST_FEED_NOW，处理 UN_SELECTED/CONFIRM | 覆盖 |
| 3 | 切奉纳界面 | script_task.py:85-101 关闭贪吃鬼（GREED_CLOSE/BONGNA），直到 I_ST_CAT | loop 直到 I_ST_CAT：UNSELECTED/CONFIRM/GREED_CLOSE/BONGNA | 覆盖 |
| 4 | 确保弃置筛选 | script_task.py:107-111 点 I_ST_ABANDONED_SELECTED 直到出现 | loop 点该素材直到出现 | 覆盖 |
| 5 | 按等级排序 | script_task.py:113-125 OCR 排序菜单 | — | 缺口：OCR keyword 未保留 |
| 6 | 奉纳循环 | script_task.py:127-194 堆叠/强化+0 两模式、长按 L_ONE、金币 OCR、神赐/吉运、I_ST_DONATE | loop max_donations：堆叠出现→点 region L_ONE→金币>0→I_ST_DONATE→处理 CONFIRM/GOD_PRESENT/LUCK/REWARD | 部分（长按/等级双保险/奖励等待缺口） |

## 4. 参数映射

| key | type | default | 说明 |
|---|---|---|---|
| `enable_greed` | bool | `True` | 远程配置/迁移决策 |
| `enable_maneki` | bool | `True` | 远程配置/迁移决策 |
| `max_donations` | int | `5` | 远程配置/迁移决策 |

## 5. 缺口 / TODO

- 奉纳前置「按等级排序」在远程用 O_ST_SORT_LEVEL_1/2 的 keyword 命中（ocr_appear_click）实现；导入的 OCR 素材没有 expected text → 新 flow 假设列表已按等级排序（缺口）。
- 奉纳选中御魂远程用 RuleLongClick L_ONE（长按）；DSL 无 long_click 原语 → 用 click_target(region L_ONE) 单击近似。
- O_ST_GOLD 金额：远程要求 int 且 != 0；新 flow 用 ocr_digit==0 判断（OCR 失败 → Null ≠ 0，可能继续）。
- 远程「+0 等级双保险」(O_ST_FIRSET_LEVEL 文本 == 古/+0) 与 stack/level-0 两模式分支未覆盖。
- ui_reward_appear_click / I_ST_GOLD 等待、金币不足保护未覆盖。

## 6. 素材契约与通用模式

- 导入器 v3：模板 `roi` = remote `roiBack`，无 `action_roi` ⇒ `click` 落在匹配到的模板矩形（= 旧引擎行为）；`RuleClick` → `kind=region`；`RuleOcr` → `ocr_lines`；`RuleSwipe` → `_FROM`/`_TO` 两个 region；Component 素材 `legacy_Component_<Module>_<CONST>`。
- P-NAV / P-BATTLE v2 / P-RETURN / OCR 语义同批一 README。

## 7. 证据

- oasd task lint 绿；覆盖：导航→御魂页→贪吃鬼(≤3 次)→奉纳(按堆叠/神赐/吉运/确认) 有界循环；排序/长按/精确金币判断未覆盖。未真机/回放验证。
- 未真机/未回放：无设备、无对应场景回放帧；T4 素材引用全部解析到全局 `assets/assets.json`。
