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
