# DailyTrifles（每日杂务）

远程源：`runhey/OnmyojiAutoScript@dev` `tasks/DailyTrifles`（commit `8b4dc66`）。

**状态：🔄** — 友情点领取 + 商店礼包屋签到覆盖；每日召唤/吉闻/买体力未迁移

## 行为（抽象）

1. 召唤页：普通召唤 `summon_one(draw_mystery_pattern)` + 跨月神秘图案重置；今忆召唤 `summon_recall()`（3 组预设 OCR 选择→票数 OCR→抽票→确认弹窗，最多 3 轮，失败推送通知）；`back_summon_main()`
2. `pass`（上游空实现）
3. 好友页→切 tab 到 `I_L_LOVE`→点 `I_L_COLLECT` 领取→`ui_reward_appear_click` 领奖→回庭院
4. 好友页→吉闻（OCR `O_LUCK_MSG` keyword=吉闻）→点 `I_CLICK_BLESS`/`I_ONE_CLICK_BLESS`→领奖→回庭院
5. 商店→礼包屋（`I_ROOM_GIFT`→`I_GIFT_RECOMMEND`）→`I_GIFT_SIGN` 签到领奖→回商店/庭院
6. 商店杂货铺 Special：选中行动态 ROI 读价格 `O_STORE_SUSHI_PRICE` ⇒ `count=(price-60)/20` 与目标比较，不够则买（`I_STORE_COST_TYPE_JADE`/`I_SPECIAL_SUSHI`）
7. `set_next_run('DailyTrifles', success=True, finish=False)`

## 目录 / 契约

```
tasks/DailyTrifles/
├─ meta.json       # params/exits/capabilities/schedule_hint
├─ flow.json       # oas-flow DSL（仅既有 op）
├─ migration.json  # 源文件、差异、证据
└─ README.md
```

参数：`friend_love=1`（0~1）; `store_sign=1`（0~1）

## 运行 / 校验

```powershell
cd D:\OAS-dev\oas-core
.\target\release\oasd.exe task lint --tasks-dir tasks --task DailyTrifles   # => errors=0
.\target\release\oasd.exe run --task tasks\DailyTrifles --param friend_love=1
```

## 已知限制

1. 每日召唤（普通/今忆）未迁移
2. 吉闻（OCR 入口点击）未迁移
3. 买体力（动态 ROI 价格 OCR + 比较）未迁移
4. 无实机/回放证据

详见 `migration/specs/DailyTrifles.md`、`tasks/DailyTrifles/migration.json`、`migration/status/DailyTrifles.json`。

<!-- round2:start -->
## Polish round 2（2026-11-01 · Lead 实机反馈）

实机反馈（Lead）：`friend_love` 与 `store_sign` 已实机 Done；缺失 `summon_one` / `summon_recall` / `luck_msg` / `buy_sushi`。

本轮补齐（全部按旧仓 `tasks/DailyTrifles/script_task.py` + `Component/Summon/summon.py`）：

1. **每日召唤（默认分支）**：庭院→召唤页 → `Component_Summon.I_BLUE_TICKET` 可见 →
   `ocr_digit(O_ONE_TICKET) > 0` 才抽 → 点蓝票直到 `I_ONE_TICKET` → 画符（swipe 由
   `Component_Summon.S_RANDOM_SWIPE_1` 的 FROM/TO 区域中心推出：(643,186)→(656,498)）→
   处理 `I_SM_CONFIRM` / `I_SM_CONFIRM_2` 确认。
2. **今忆召唤（param summon_recall=1）**：召唤页左侧三个 OCR 位置（`O_SELECT_SM2/3/4` 的 roi 中心
   (48,235)/(48,323)/(48,416)，坐标由旧仓 OCR Area 推出）尝试打开今忆页 → `I_RECALL_TICKET` 可见 →
   `ocr_digit(O_RECALL_TICKET_AREA) > 0` → 点票直到 `I_RECALL_ONE_TICKET` → 画符 → 确认
   `I_RECALL_SM_CONFIRM` / `Component_Summon.I_SM_CONFIRM_2`。
3. **吉闻**：庭院→好友 → 等 `I_LUCK_TITLE`（`I_FRIENDSHIP_UP` 弹窗先关）；用
   `click_target(ocr_text, O_LUCK_MSG, "吉闻")` 打开吉闻 → 点 `I_CLICK_BLESS` / `I_ONE_CLICK_BLESS` →
   出现奖励就点操作区。
4. **买体力**：庭院→商城→杂货铺→特殊 → `I_SPECIAL_SUSHI` → `I_STORE_COST_TYPE_JADE`；
   按 `param('buy_sushi_count')` 次数循环（旧仓按动态 ROI 读价格/已购次数，DSL 无法表达，见 gap）。

新增 params（默认 0/-1 = 保持原来的 no-op 行为）：`one_summon`、`summon_recall`、`luck_msg`、`buy_sushi_count`。
两条已实机 Done 的子流程（friend_love / store_sign）**原样保留**，未改动。

剩余阻塞/差异：
- `O_STORE_SUSHI_PRICE` 在旧仓是**运行时动态 ROI**（`roi=(0,0,0,0)`，代码里按商品图标位置重算），
  importer 因此没有导入；`buy_sushi_count` 从「读取游戏内已购次数」降级为「本次运行购买次数」。
- `DailyTrifles.I_SM_CONFIRM_2` 的图片在上游缺失（`summonRecall/sm_sm_confirm_2.png` 不存在），
  改用已导入的 `Component_Summon.I_SM_CONFIRM_2`（同图同尺寸）。
- 画符 swipe 用字面量坐标（RuleSwipe 只导入为 `_FROM/_TO` 区域 id，DSL 无法取区域中心）。
- summon_recall 的左侧菜单坐标来自旧仓 OCR 区域，需实机确认当前版本是否仍相同。
- 本次只做了流程/守卫修复 + lint，**未在真机复测新增分支**（Lead 已实机验证的两条子流程除外）。
<!-- round2:end -->

<!-- round3:start -->
## Round 3（2026-11-01 · Lead 实机反馈）

实机反馈（Lead）：好友入口 `click_point(905,641)` 可用；吉闻分支失败原因是
`O_LUCK_MSG` OCR 只识别到一行文本、内容不是「吉闻」，`click_target(ocr_text,"吉闻")` 直接报错。

本次修复（只用现有 DSL / 现有 legacy id 与坐标，未新增素材）：
1. **移除** `O_LUCK_MSG` 的 OCR 文本 `click_target`（不再依赖 OCR 文本内容）；
2. 好友页用**直接 UI 点击**吉闻按钮：`O_LUCK_MSG` ROI=(15,38,70,70)，候选点 (50,55) / (50,73)
   交替点击（`inc_var('luck_try')` + `var % 2`），最多 5 轮；`I_FRIENDSHIP_UP` 弹窗先关；
3. 每轮点击后用 `I_LUCK_TITLE` **验证**是否进入吉闻页（`var('luck_open')` 标记）；
4. 进入后：`I_CLICK_BLESS` / `I_ONE_CLICK_BLESS` 点击 + 「获得奖励」操作区点击（12 轮有界）；
5. 未打开吉闻页 → 记录 warn，**no-op 继续**，不失败；最后统一回庭院；
6. 全程保持 `finish(done)` 语义（吉闻不可达不再中断任务）。

剩余：吉闻按钮真实位置（候选点哪一个是正解）需实机复测；祝福奖励是否已领完需实机确认。
<!-- round3:end -->

