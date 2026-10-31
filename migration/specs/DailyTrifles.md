# DailyTrifles 迁移规格

- 远程源：`runhey/OnmyojiAutoScript@dev` · commit `8b4dc669b47838f42d283f14aa394844c1963c86`（只读 `D:\OAS-dev\_verify\remote-oas`）
- 状态：**🔄**（友情点领取 + 商店礼包屋签到覆盖；每日召唤/吉闻/买体力未迁移）
- 已读文件：
  - `tasks/DailyTrifles/script_task.py`
  - `tasks/DailyTrifles/config.py`
  - `tasks/DailyTrifles/page.py`
  - `tasks/DailyTrifles/assets.py`
  - `tasks/DailyTrifles/{love,store,summonRecall}/*.json`
  - `tasks/Component/Summon/summon.py`
  - `tasks/RichMan/assets.py`
- 共享阅读：`tasks/Component/GeneralBattle/{general_battle.py,battle.py,battle_wait.py}`、`tasks/Component/GeneralRoom/general_room.py`、`tasks/GameUi/default_pages.py`（导航图）

## 1. 上游行为矩阵

| # | 条件 / 输入 | 行为 |
|---|---|---|
| 1 | `one_summon`（默认 False） | 召唤页：普通召唤 `summon_one(draw_mystery_pattern)` + 跨月神秘图案重置；今忆召唤 `summon_recall()`（3 组预设 OCR 选择→票数 OCR→抽票→确认弹窗，最多 3 轮，失败推送通知）；`back_summon_main()` |
| 2 | `guild_wish`（默认 False） | `pass`（上游空实现） |
| 3 | `friend_love`（默认 False） | 好友页→切 tab 到 `I_L_LOVE`→点 `I_L_COLLECT` 领取→`ui_reward_appear_click` 领奖→回庭院 |
| 4 | `luck_msg`（默认 False） | 好友页→吉闻（OCR `O_LUCK_MSG` keyword=吉闻）→点 `I_CLICK_BLESS`/`I_ONE_CLICK_BLESS`→领奖→回庭院 |
| 5 | `store_sign`（默认 False） | 商店→礼包屋（`I_ROOM_GIFT`→`I_GIFT_RECOMMEND`）→`I_GIFT_SIGN` 签到领奖→回商店/庭院 |
| 6 | `buy_sushi_count > 0`（默认 -1） | 商店杂货铺 Special：选中行动态 ROI 读价格 `O_STORE_SUSHI_PRICE` ⇒ `count=(price-60)/20` 与目标比较，不够则买（`I_STORE_COST_TYPE_JADE`/`I_SPECIAL_SUSHI`） |
| 7 | 结束 | `set_next_run('DailyTrifles', success=True, finish=False)` |

## 2. oas-core 映射与覆盖

| 上游行为 | 本迁移实现 | 状态 |
|---|---|---|
| 庭院→好友 / 商店导航 | `click(I_MAIN_GOTO_FRIENDS)`/`click(I_MAIN_GOTO_MALL)` + 页面守卫 | ✅ |
| `friend_love` | 循环切 tab（`I_FRIENDSHIP_UP`/`I_L_FRIENDS`/`I_FRIEND_TAB`）直到 `I_L_LOVE`；点 `I_L_COLLECT`；`I_UI_REWARD` 出现即点并结束；`I_UI_BACK_RED` 回庭院 | ✅ |
| `store_sign` | 商店→`I_ROOM_GIFT`→`I_GIFT_RECOMMEND`→`I_GIFT_SIGN` 签到→循环返回按钮回庭院 | ✅ |
| `one_summon` / 今忆召唤 | 未迁移：Summon 组件素材已随 Component 导入，但今忆召唤需要「OCR 票数→比较→多轮抽票→确认弹窗」组合与预设组选择；普通召唤的式神录/神秘图案链也未组合 | 🔄 TODO |
| `luck_msg` | 未迁移：`O_LUCK_MSG` 是 keyword=吉闻 的包含匹配单行 OCR；flow 需「OCR 入口文本→点击入口」精确链路，且吉闻 tab 定位依赖颜色/OCR 组合 | 🔄 TODO |
| `buy_sushi_count` | 未迁移：价格 ROI 是运行时按选中行动态改写（`O_STORE_SUSHI_PRICE.roi`），flow 无「派生 ROI」原语；且需数字比较与购买确认链 | 🔄 TODO |
| `guild_wish` | 上游空实现，不迁移 | — |

## 3. Flow 结构要点

- 参数 `friend_love`/`store_sign` 默认 1（便于单独运行验证）；上游这两项默认 False（由调度配置开启），差异已记录。
- 所有子流程按 `if param(...) != 0` 条件执行，骨架可插拔。

## 4. 素材（全部为全局 `assets/assets.json`）

所有引用素材都是全局 `assets/assets.json` 的 id（本任务不新增素材、无 images/）。导入语义：`template` 的 `roi` 是旧仓搜索窗（roiBack），点击落在**匹配到的模板外接框**；`region` 是旧仓 RuleClick 的精确操作区域；OCR 资产为 `ocr_lines`（旧 RuleOcr）。

- `庭院_探索灯笼`
- `legacy_GameUi_I_MAIN_GOTO_FRIENDS`
- `legacy_GameUi_I_CHECK_FRIENDS`
- `legacy_GameUi_I_MAIN_GOTO_MALL`
- `legacy_GameUi_I_CHECK_MALL`
- `legacy_GameUi_I_CHECK_MALL_PURPLE`
- `legacy_DailyTrifles_I_FRIENDSHIP_UP`
- `legacy_DailyTrifles_I_L_FRIENDS`
- `legacy_DailyTrifles_I_L_LOVE`
- `legacy_DailyTrifles_I_FRIEND_TAB`
- `legacy_DailyTrifles_I_L_COLLECT`
- `legacy_DailyTrifles_I_ROOM_GIFT`
- `legacy_DailyTrifles_I_GIFT_RECOMMEND`
- `legacy_DailyTrifles_I_GIFT_SIGN`
- `legacy_GlobalGame_I_UI_REWARD`
- `legacy_GlobalGame_I_UI_BACK_{RED,YELLOW}`

## 5. 差异 / 待办

1. 每日召唤（普通/今忆）未迁移
2. 吉闻（OCR 入口点击）未迁移
3. 买体力（动态 ROI 价格 OCR + 比较）未迁移
4. 无实机/回放证据
