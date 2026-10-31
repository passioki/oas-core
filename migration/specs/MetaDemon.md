# MetaDemon 迁移规格

- 远程源：`runhey/OnmyojiAutoScript@dev` · commit `8b4dc669b47838f42d283f14aa394844c1963c86`（只读 `D:\OAS-dev\_verify\remote-oas`）
- 状态：**🔄**（入口→找鬼王/共享鬼王→战斗→返回覆盖；疲劳买茶、策略切队、星合成、通知未迁移）
- 已读文件：
  - `tasks/MetaDemon/script_task.py`
  - `tasks/MetaDemon/config.py`
  - `tasks/MetaDemon/assets.py`
  - `tasks/MetaDemon/md/{image,ocr}.json`
  - `tasks/MetaDemon/md/image_2025.json`
  - `tasks/Component/RightActivity/right_activity.py`
- 共享阅读：`tasks/Component/GeneralBattle/{general_battle.py,battle.py,battle_wait.py}`、`tasks/Component/GeneralRoom/general_room.py`、`tasks/GameUi/default_pages.py`（导航图）

## 1. 上游行为矩阵

| # | 条件 / 输入 | 行为 |
|---|---|---|
| 1 | 御魂切换 | enable/enable_switch_by_name（配置） |
| 2 | 入口 | `RightActivity.enter(I_RIGHT_ENTER)`：回庭院→`I_TOGGLE_BUTTON` 打开右侧活动面板→点活动入口 → 循环（`I_UI_BACK_RED`/`I_ENTER_ENSURE`/`I_ENTER2`）直到 `I_FIND_DEMON` 或 `I_BATTLE_DEMON` |
| 3 | 星合成（meta_crafting_card） | `crafting()`：1 星合成 2 星，最多 4 轮，`I_CRAFTING_START`/`I_CRAFTING_CARD_STAR_1`/`I_CRAFTING_EMPTY(_NEW)`/奖励弹窗 |
| 4 | 战斗循环 | `boss_timer`(50s 超时)；每轮：找自己鬼王 `I_BOSS_EMPTY`+`I_BOSS_CONVENE` 或共享 `I_BOSS_EMPTY_1/2`（含 OCR `O_BOSS_EMPTY_1`）→点鬼王→等 `I_BATTLE_DEMON`→`battle_boss`（5/6 星按 full-name OCR + 血量阈值选策略组预设） |
| 5 | 疲劳 | `O_MD_EXHAUSTION` OCR >100：`auto_tea` ⇒ 买茶（上游 `buy_tea` 是 pass 占位）；否则退出 |
| 6 | 找鬼王 | `find_demon()`：点 `I_FIND_DEMON` 直到 `I_BATTLE_DEMON` 且 `I_BOSS_EMPTY` 消失；5/6 星时 OCR `O_MD_FULL_NAME` 并推送通知 |
| 7 | 结束 | `I_UI_BACK_YELLOW`→庭院；`set_next_run(success=True)` |

## 2. oas-core 映射与覆盖

| 上游行为 | 本迁移实现 | 状态 |
|---|---|---|
| 庭院→右侧活动面板 | `click(I_TOGGLE_BUTTON)` + 循环 `I_RA_CLOSE`/`I_RIGHT_ENTER`/`I_MD_ENTER`/`I_ENTER2`/`I_ENTER_ENSURE`/`I_UI_CONFIRM` 直到进入超鬼王主界面 | ✅ |
| 找鬼王/战斗 | 循环：`I_BATTLE_DEMON` 出现 ⇒ 点击并 `battle_cycle`；否则点 `I_FIND_DEMON`；主循环 `max_battles` 上限 + 页面丢失即退出 | ✅（简化） |
| 疲劳 OCR 判定/买茶 | 未迁移：需 OCR 数值比较与「购买确认」链；上游买茶本身是 pass 占位 | 🔄 TODO |
| 5/6 星策略 OCR→配置组→预设切队 | 未迁移：需要「OCR 名与本局配置策略表模糊匹配 + 预设组/队伍切换」原语（配置数据不在 flow 内） | 🔄 TODO |
| 星合成 crafting | 未迁移（配置默认关闭；多轮合成/奖励弹窗时序） | 🔄 TODO |
| 通知推送 / boss_mark / 血量阈值 | 未迁移（通知层 / 战斗标记原语） | 不迁移 |

## 3. Flow 结构要点

- 右侧活动面板目前只有 `I_TOGGLE_BUTTON/I_RA_OPEN/I_RA_CLOSE` 三个素材，入口链靠循环点击多个候选按钮（`I_RIGHT_ENTER`/`I_MD_ENTER`/`I_ENTER2`/`I_ENTER_ENSURE`）推进。
- 战斗优先级：自己鬼王 `I_BOSS_EMPTY` / 共享 `I_BOSS_EMPTY_1/2` 先点，再统一走 `I_BATTLE_DEMON` 开战。

## 4. 素材（全部为全局 `assets/assets.json`）

所有引用素材都是全局 `assets/assets.json` 的 id（本任务不新增素材、无 images/）。导入语义：`template` 的 `roi` 是旧仓搜索窗（roiBack），点击落在**匹配到的模板外接框**；`region` 是旧仓 RuleClick 的精确操作区域；OCR 资产为 `ocr_lines`（旧 RuleOcr）。

- `庭院_探索灯笼`
- `legacy_Component_RightActivity_I_TOGGLE_BUTTON`
- `legacy_Component_RightActivity_I_RA_OPEN`
- `legacy_Component_RightActivity_I_RA_CLOSE`
- `legacy_MetaDemon_I_RIGHT_ENTER`
- `legacy_MetaDemon_I_MD_ENTER`
- `legacy_MetaDemon_I_MD_MAIN`
- `legacy_MetaDemon_I_ENTER2`
- `legacy_MetaDemon_I_ENTER_ENSURE`
- `legacy_MetaDemon_I_FIND_DEMON`
- `legacy_MetaDemon_I_BATTLE_DEMON`
- `legacy_MetaDemon_I_BOSS_EMPTY`
- `legacy_MetaDemon_I_BOSS_EMPTY_1`
- `legacy_MetaDemon_I_BOSS_EMPTY_2`
- `legacy_MetaDemon_I_BOSS_CONVENE`
- `legacy_Component_GeneralBattle_{...}`
- `legacy_GlobalGame_I_UI_CONFIRM`
- `legacy_GlobalGame_I_UI_BACK_{RED,YELLOW}`

## 5. 差异 / 待办

1. 疲劳 OCR 判定与买茶未迁移
2. 5/6 星 full-name OCR→策略组→预设切队未迁移（配置数据不可进 flow）
3. 星合成 crafting、通知、boss_mark、血量阈值未迁移
4. 无实机/回放证据（限时活动）
