# GoldYoukai（金币妖怪）

## 来源（只读旧仓）

- `tasks/GoldYoukai/script_task.py`（126 行）— run / gold_exit / battle_wait 重写
- `tasks/GoldYoukai/config.py` — `buff_gold_50_click` / `buff_gold_100_click`（默认 False）、`switch_soul`
- `tasks/GoldYoukai/assets.py` — `I_GOLD_WIN`
- `tasks/Component/GeneralRoom/general_room.py` — create_room / ensure_public / create_ensure / check_zones
- `tasks/Component/GeneralInvite/general_invite.py` — is_in_room / click_fire
- `tasks/Component/GeneralBattle/general_battle.py` — run_general_battle / battle_wait

## 行为摘要（旧仓）

与 ExperienceYoukai 同构：`while count < 2` 里 `goto_page(page_team)` → `check_zones('金币妖怪')` → 创建房间
（失败=没券则退出任务）→ 公开 → 确认 → 等 50 秒/有人 → 点挑战 → 战斗 → 结算；结束后 `gold_exit()` 回庭院。

## 流程（flow.json）

| # | op | 说明 | 守卫 |
|---|----|------|------|
| 1 | 进组队页 | 主界面点组队 | expect `I_CHECK_TEAM` |
| 2 | loop×2 | 每轮：选金币妖怪 → 创建公开房间 → 等队友/开战 → 战斗 → 退出房间 | 见 ExperienceYoukai |
| 3 | 黄返回 | 组队页 → 庭院 | expect `I_CHECK_MAIN` |
| 4 | finish done | | |

## 素材

| 全局 id | 来源 | 用途 |
|---------|------|------|
| legacy_GoldYoukai_I_GOLD_WIN | 任务 | 胜利结算 |
| legacy_Component_GeneralRoom_* | Component/GeneralRoom | 房间创建/公开/退出/副本名 OCR |
| legacy_Component_GeneralInvite_* | Component/GeneralInvite | 进房判定 / 挑战 |
| legacy_Component_GeneralBattle_* | Component/GeneralBattle | 战斗准备/胜利/失败/奖励 |
| legacy_GameUi_I_MAIN_GOTO_TEAM / I_CHECK_TEAM / I_CHECK_MAIN | GameUi | 导航 |
| legacy_GlobalGame_I_UI_BACK_YELLOW / I_UI_CONFIRM / I_UI_REWARD / C_UI_REWARD | GlobalGame | 返回/确认/奖励 |

## 出口

- `done`(success)
- `no_resource`(resource)：创建房间失败（没券，旧仓直接结束任务）。
- `failed`(retryable)

## 与旧仓的差异 / 已知缺口

1. 50 秒等待计时器 → 有界 20×2.5s；无 wall-clock。
2. `check_zones` 的 RuleList 滚动 → `text_visible` + 行内 OCR 点击 + 列表 swipe（列表 ROI 来自 `L_TEAM_LIST.roi_back`）。
3. **金币 50/100 加成未迁移**：GeneralBuff 动态开关区域需要锚点定位（DSL 缺，ENGINE §22.14 未实现），默认关闭。
4. **换御魂未迁移**（默认关闭）。
5. 战斗主链只迁移「准备→自动战斗→结算→页面回归」；绿标/预设队伍/随机点击未迁移。
6. 20 秒表情防挂机未迁移。
7. 未真机验证。

## Lint

```powershell
cd D:\OAS-dev\oas-core
.\target\release\oasd.exe task lint --tasks-dir tasks --task GoldYoukai
```
