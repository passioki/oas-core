# MetaDemon（超鬼王（MetaDemon））

远程源：`runhey/OnmyojiAutoScript@dev` `tasks/MetaDemon`（commit `8b4dc66`）。

**状态：🔄** — 入口→找鬼王/共享鬼王→战斗→返回覆盖；疲劳买茶、策略切队、星合成、通知未迁移

## 行为（抽象）

1. enable/enable_switch_by_name（配置）
2. `RightActivity.enter(I_RIGHT_ENTER)`：回庭院→`I_TOGGLE_BUTTON` 打开右侧活动面板→点活动入口 → 循环（`I_UI_BACK_RED`/`I_ENTER_ENSURE`/`I_ENTER2`）直到 `I_FIND_DEMON` 或 `I_BATTLE_DEMON`
3. `crafting()`：1 星合成 2 星，最多 4 轮，`I_CRAFTING_START`/`I_CRAFTING_CARD_STAR_1`/`I_CRAFTING_EMPTY(_NEW)`/奖励弹窗
4. `boss_timer`(50s 超时)；每轮：找自己鬼王 `I_BOSS_EMPTY`+`I_BOSS_CONVENE` 或共享 `I_BOSS_EMPTY_1/2`（含 OCR `O_BOSS_EMPTY_1`）→点鬼王→等 `I_BATTLE_DEMON`→`battle_boss`（5/6 星按 full-name OCR + 血量阈值选策略组预设）
5. `O_MD_EXHAUSTION` OCR >100：`auto_tea` ⇒ 买茶（上游 `buy_tea` 是 pass 占位）；否则退出
6. `find_demon()`：点 `I_FIND_DEMON` 直到 `I_BATTLE_DEMON` 且 `I_BOSS_EMPTY` 消失；5/6 星时 OCR `O_MD_FULL_NAME` 并推送通知
7. `I_UI_BACK_YELLOW`→庭院；`set_next_run(success=True)`

## 目录 / 契约

```
tasks/MetaDemon/
├─ meta.json       # params/exits/capabilities/schedule_hint
├─ flow.json       # oas-flow DSL（仅既有 op）
├─ migration.json  # 源文件、差异、证据
└─ README.md
```

参数：`max_battles=5`（1~20）

## 运行 / 校验

```powershell
cd D:\OAS-dev\oas-core
.\target\release\oasd.exe task lint --tasks-dir tasks --task MetaDemon   # => errors=0
.\target\release\oasd.exe run --task tasks\MetaDemon --param max_battles=5
```

## 已知限制

1. 疲劳 OCR 判定与买茶未迁移
2. 5/6 星 full-name OCR→策略组→预设切队未迁移（配置数据不可进 flow）
3. 星合成 crafting、通知、boss_mark、血量阈值未迁移
4. 无实机/回放证据（限时活动）

详见 `migration/specs/MetaDemon.md`、`tasks/MetaDemon/migration.json`、`migration/status/MetaDemon.json`。
