# Exploration 迁移规格

- 远程源：`runhey/OnmyojiAutoScript@dev` · commit `8b4dc669b47838f42d283f14aa394844c1963c86`（只读 `D:\OAS-dev\_verify\remote-oas`）
- 状态：**🔄**（默认配置主链路（庭院→探索→第二十八章→打怪/BOSS/小纸人→返回）覆盖；高级选项与组队/滑屏尽头检测未迁移）
- 已读文件：
  - `tasks/Exploration/script_task.py`
  - `tasks/Exploration/config.py`
  - `tasks/Exploration/base.py`
  - `tasks/Exploration/solo.py`
  - `tasks/Exploration/page.py`
  - `tasks/Exploration/assets.py`
  - `tasks/Exploration/version.py`
  - `tasks/Exploration/res/{image,click,ocr,long_click,swipe}.json`
  - `tasks/GameUi/page.py`
- 共享阅读：`tasks/Component/GeneralBattle/{general_battle.py,battle.py,battle_wait.py}`、`tasks/Component/GeneralRoom/general_room.py`、`tasks/GameUi/default_pages.py`（导航图）

## 1. 上游行为矩阵

| # | 条件 / 输入 | 行为 |
|---|---|---|
| 1 | 预处理 `pre_process()` | 御魂切换（方式一/二）；加成勾选（金/经验 50/100）；`goto_page(page_mainline)`（探索大地图·主线 tab） |
| 2 | 场景识别 `get_current_scene()` | WORLD（`I_CHECK_EXPLORATION` 且无设置按钮）/ ENTRANCE（`I_E_EXPLORATION_CLICK`）/ MAIN（`I_E_SETTINGS_BUTTON` 或自动轮换图标）/ BATTLE_PREPARE / BATTLE_FIGHTING / TEAM / UNKNOWN |
| 3 | `user_status=ALONE`（默认） | `run_solo()`：WORLD → 开右箭头→选章→进图；ENTRANCE → 点进入；MAIN → 自动轮换设置、捡宝箱、清剧情弹窗、领小纸人奖励、打 BOSS/UP 怪；无目标则 `_search_next_area()` 右滑找怪（滑到尽头判定 `is_swipe_unchanged`） |
| 4 | `LEADER` | 组队：邀请好友（`invite_friend`：好友列表 OCR/分类 tab/选中/确认）+ 同 MAIN 循环 + 队友跑路检测（`I_TEAM_EMOJI`） |
| 5 | `MEMBER` | 在 WORLD 等邀请（`check_then_accept`）；接受后进战斗/领奖；超时 50s 退出 |
| 6 | 战斗 `fire()` | 点战斗图标→若仍在探索页说明没点到；`run_general_battle`→`minions_cnt += 1` |
| 7 | 退出条件 `check_exit()` | `minions_cnt >= minions_cnt`（默认 30）或超 `limit_time`（30min）；可选：突破票 >= 阈值时切 RealmRaid/MemoryScrolls（`scrolls_enable`） |
| 8 | 退出 `quit_explore()` | 点 `I_BACK_YOLLOW`/`I_UI_BACK_BLUE`/`I_E_EXIT_CONFIRM` 回探索大世界；`post_process`：关加成、回庭院、`set_next_run` |

## 2. oas-core 映射与覆盖

| 上游行为 | 本迁移实现 | 状态 |
|---|---|---|
| 庭院→探索大地图→第二十八章 | `cp(680,152)`→世界页；`if appear(章节_第二十八章)` ⇒ `cp(1140,462)`→`cp(1115,630)` 进章节地图（沿用 `tasks/探索` 真机验证链路） | ✅（固定 28 章） |
| 章节地图主循环 | `loop max_battles`：小纸人 `I_BATTLE_REWARD` 先点；有 BOSS/普通怪则 `click_target`（优先级 BOSS→普通）→`battle_cycle`；无怪则右滑（`swipe` 1100,380→250,380） | ✅（简化） |
| 自动轮换/候补/UP 定向/宝箱/剧情弹窗 | 未迁移：`auto_rotate` 的式神录设置链；UP 定向要找 `I_UP_*` 图标附近最近的战斗图标（加权距离）；宝箱 `I_TREASURE_BOX_CLICK`；`I_UI_CONFIRM` 弹窗清理 | 🔄 TODO |
| 滑屏尽头检测 `is_swipe_unchanged` | 未迁移：需要帧差分对比原语（阈值 15，比较滑屏前后中上部区域） | 🔄 TODO |
| 组队 LEADER/MEMBER、好友邀请 | 未迁移（好友列表 OCR/分类 tab/邀请确认 + TEAM 场景状态机） | 🔄 TODO |
| 加成/御魂切换/突破票联动/limit_time | 未迁移（配置项；`activate_realm_raid` 是调度写回） | 不迁移 |
| 其他章节 / 章节滑屏选关 | 未迁移：`open_expect_level` 用 OCR 整列 level 名 + 滑屏找目标章；本 flow 固定第二十八章 | 🔄 TODO |

## 3. Flow 结构要点

- 复用 `tasks/探索` 的真机验证素材链（`庭院_探索灯笼`/`世界_章节栏`/`章节_第二十八章`/`入口_探索按钮`/`章节_固定阵容`/`退出确认_确认`）。
- `max_battles` 默认 30 对齐上游 `minions_cnt=30`；上游 `limit_time=30min` 未在 flow 内实现。
- 无怪时滑屏一次后回到循环下一轮再判定（滑屏后同样会先检查小纸人/战斗图标）。

## 4. 素材（全部为全局 `assets/assets.json`）

所有引用素材都是全局 `assets/assets.json` 的 id（本任务不新增素材、无 images/）。导入语义：`template` 的 `roi` 是旧仓搜索窗（roiBack），点击落在**匹配到的模板外接框**；`region` 是旧仓 RuleClick 的精确操作区域；OCR 资产为 `ocr_lines`（旧 RuleOcr）。

- `庭院_探索灯笼`
- `世界_章节栏`
- `章节_第二十八章`
- `入口_探索按钮`
- `章节_固定阵容`
- `章节_战斗图标`
- `退出确认_确认`
- `legacy_Exploration_I_BATTLE_REWARD`
- `legacy_Exploration_I_BOSS_BATTLE_BUTTON`
- `legacy_Exploration_I_NORMAL_BATTLE_BUTTON`
- `legacy_Component_GeneralBattle_{...}`

## 5. 差异 / 待办

1. 仅第二十八章；其他章节滑屏选关未迁移
2. UP 定向找怪、自动候补、宝箱、剧情弹窗、加成、突破票联动未迁移
3. 滑屏尽头检测需帧差分原语，未迁移
4. 组队 LEADER/MEMBER + 好友邀请未迁移
5. 无本任务实机/回放证据（仅复用 tasks/探索 素材链）
