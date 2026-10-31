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

## 2026-11-01 静态修整（polish round）

- 修复：入口链：旧 flow 点 I_RA_CLOSE 会**收起**右侧活动面板，且混入 I_MD_ENTER/I_MD_MAIN 等远端入口链未使用的按钮 → 改为镜像 RightActivity.enter()：点 I_TOGGLE_BUTTON 直到 I_RIGHT_ENTER 出现，再点 I_RIGHT_ENTER 直到其消失，进活动后再处理 UI_BACK_RED/ENTER_ENSURE/ENTER2
- 修复：鬼王点击：旧 flow 在「空位图标不出现」时 click(I_BOSS_EMPTY)；旧引擎是点击 roi_front 坐标、新引擎 click 需匹配成功（会响亮失败）→ 改为 click_point(88,176) 点卡位中心，并镜像上游条件 `not appear(I_BOSS_EMPTY) and appear(I_BOSS_CONVENE)`
- 修复：补奖励弹窗处理：上游每轮 ui_reward_appear_click() = appear(I_UI_REWARD)→click(C_UI_REWARD)；flow 增加 I_UI_REWARD + C_UI_REWARD 分支
- 修复：删除原先重复两份的战斗/结算代码（flow 864→629 行），统一为单条 battle 链（准备/胜负/贪吃鬼/御魂溢出/奖励）
- 仍存在（blocker/未迁移）：共享鬼王 I_BOSS_EMPTY_1/I_BOSS_EMPTY_2 未迁移：旧引擎 click 是「按 roi_front 坐标点未匹配模板」，新引擎 click 需匹配成功；且上游条件叠加 ocr_appear(O_BOSS_EMPTY_1) 复合判断（blocker）
- 校验：`oasd task lint --tasks-dir tasks --task MetaDemon` => errors=0
