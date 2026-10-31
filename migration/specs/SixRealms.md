# SixRealms 迁移规格

- 远程源：`runhey/OnmyojiAutoScript@dev` · commit `8b4dc669b47838f42d283f14aa394844c1963c86`（只读 `D:\OAS-dev\_verify\remote-oas`）
- 状态：**🔄**（入口→月海→开始（守族/首技能）→主地图覆盖；岛屿决策/小游戏/商店/首领战未迁移（缺 OCR 包含匹配与按框点击原语））
- 已读文件：
  - `tasks/SixRealms/script_task.py`
  - `tasks/SixRealms/config.py`
  - `tasks/SixRealms/assets.py`
  - `tasks/SixRealms/common.py`
  - `tasks/SixRealms/oas_ocr.py`
  - `tasks/SixRealms/moon_sea/{moon_sea.py,map.py,skills.py,l101.py,l102.py,l103.py,l104.py,l105.py}`
  - `tasks/GameUi/default_pages.py`
  - `tasks/GameUi/page.py`
- 共享阅读：`tasks/Component/GeneralBattle/{general_battle.py,battle.py,battle_wait.py}`、`tasks/Component/GeneralRoom/general_room.py`、`tasks/GameUi/default_pages.py`（导航图）

## 1. 上游行为矩阵

| # | 条件 / 输入 | 行为 |
|---|---|---|
| 1 | 御魂切换 | enable/enable_switch_by_name（配置） |
| 2 | 入口 | `goto_page(page_six_gates)`（探索→玩法→六道之门，入口按活动期从 `I_EXPLORATION_TO_*` 里点） |
| 3 | 月海开始 | `_start()`：等 `I_MENTER` 点击→等 `I_MSTART`；守族选择（`I_MSHUTEN`/`I_MSHOUZU_SELECT` 直到 `I_MSHOUZU`）；开始（`I_MSTART`/`I_MSTART_CONFIRM(2)`/`I_UI_CONFIRM(_SAMLL)`/`I_MSKIP`/`I_MCONINUE`）；`I_SELECT_SHIKIGAMI_AND_CONFIRM` 时上滑选式神并确认；等 `I_M_STORE`（商店图标）后点 `I_MFIRST_SKILL` 选首个柔风技能 |
| 4 | 岛屿决策 `decide()` | 整屏 OCR `map_ocr`：按文本包含 宁息/神秘/回混范/蜜馨屡战/星之 判断岛屿类型；`island106` = `I_BOSS_FIRE`；同时抓「回合/国」行取剩余回合数；未识别 ⇒ 随机从 C_ISLAND_ENTER_1..6 进入 |
| 5 | 进岛 | `enter_island`：按 OCR box 位置改写 `C_ISLAND_ENTER.roi_front` 后点击（动态区域） |
| 6 | 各岛小游戏 l101~l105 | 记忆/模仿/战斗等各自子流程（`moon_sea/l101.py` 等），含技能选择、商店买技能、战斗 |
| 7 | 首领 | `I_BOSS_FIRE` ⇒ `boss_team_lock`（I_BOSS_TEAM_LOCK/UNLOCK）→ `boss_battle`（胜利后 I_BOSS_SHARE→I_BOSS_SHUTU→I_MSTART；双倍 I_BOSS_USE_DOUBLE；跳过 I_BOSS_SKIP 第二 BOSS） |
| 8 | 结束 | `_run_moon_sea` 限次(`limit_count`=1)/限时(30min) → 退出六道 → 回庭院 |

## 2. oas-core 映射与覆盖

| 上游行为 | 本迁移实现 | 状态 |
|---|---|---|
| 庭院→探索→玩法→六道之门 | `cp(680,152)`→`cp(1216,186)`→`click(I_EXPLORATION_TO_MOON_SEA)`→`expect I_CHECK_MOON_SEA` | ✅ |
| 月海开始 | 循环点 `I_MENTER`/确认，等 `I_MSTART`；守族选择（`I_MSHUTEN`/`I_MSHOUZU_SELECT` 直到 `I_MSHOUZU`）；开始循环（`I_MSTART`/`I_MFIRST_SKILL`/`I_MSKIP`/确认）直到 `I_M_STORE` | ✅（不含 select_shikigami 钩子） |
| 岛屿决策 | 未迁移：`island_name()` 用 `'星' in text` 等**子串匹配**；表达式无 contains；且 `enter_island` 要按 OCR box 动态改 ROI 后点击 —— 需要 `ocr_contains(sel,text)` + `click_target ocr_text` 在 `O_OCR_MAP` 上的行框点击原语（部分可行，但 5 类优先级 + 回合数 + 商店/随机进入的组合未做） | 🔄 TODO |
| 小游戏 l101~l105 | 未迁移（每个都有独立状态机/技能/战斗；依赖岛屿决策） | 🔄 TODO |
| 首领战 | 未迁移（依赖岛屿流程 + 队伍锁定/双倍/跳过时序） | 🔄 TODO |
| 御魂切换/限时 | 未迁移（配置项/时间上限） | 不迁移 |

## 3. Flow 结构要点

- 月海入口 `I_EXPLORATION_TO_MOON_SEA` 在六道活动期出现于探索·玩法页；本任务假定当前为月海活动期。
- 本 flow 在进入主地图后只做回退并 `finish(no_target)`，不猜测岛屿决策——见 TODO。

## 4. 素材（全部为全局 `assets/assets.json`）

所有引用素材都是全局 `assets/assets.json` 的 id（本任务不新增素材、无 images/）。导入语义：`template` 的 `roi` 是旧仓搜索窗（roiBack），点击落在**匹配到的模板外接框**；`region` 是旧仓 RuleClick 的精确操作区域；OCR 资产为 `ocr_lines`（旧 RuleOcr）。

- `庭院_探索灯笼`
- `世界_章节栏`
- `legacy_GameUi_I_CHECK_EXPLORATION`
- `legacy_GameUi_I_CHECK_MOON_SEA`
- `legacy_GameUi_I_EXPLORATION_TO_MOON_SEA`
- `legacy_Exploration_I_CHECK_PLAY_TITLE`
- `legacy_SixRealms_I_MENTER`
- `legacy_SixRealms_I_MSTART`
- `legacy_SixRealms_I_MSHUTEN`
- `legacy_SixRealms_I_MSHOUZU`
- `legacy_SixRealms_I_MSHOUZU_SELECT`
- `legacy_SixRealms_I_MFIRST_SKILL`
- `legacy_SixRealms_I_M_STORE`
- `legacy_SixRealms_I_MSKIP`
- `legacy_SixRealms_I_BACK_EXIT`
- `legacy_GlobalGame_I_UI_CONFIRM`
- `legacy_GlobalGame_I_UI_BACK_YELLOW`

## 5. 差异 / 待办

1. 岛屿决策需 OCR 包含匹配 + 按 OCR 行框点击原语，未迁移
2. l101~l105 小游戏、商店、首领战未迁移
3. 无实机/回放证据（六道活动期限制）

## 8. Polish round (2026-11-01)

- 入口链修复：删除无守卫的硬编码 `click_point(1216,186)`（玩法 tab，对六道之门入口并非必需）；探索灯笼点击改为带 `requires=page_main / expect=page_exploration` 的 `click_point`。
- 六道之门入口按远程 `exploration_to_six_gates` 语义改为「六种入口变体（月海/香界/季灵/净佛/真言/孔雀）逐个 appear→click」，再 `wait_until` 任意页面 marker；不再用单变体 + 自守卫。
- 回庭院标记统一 `legacy_GameUi_I_CHECK_MAIN`；修正日志中「玩法 tab」的过期描述。



