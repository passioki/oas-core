# ExperienceYoukai（经验妖怪）

## 来源（只读旧仓）

- `tasks/ExperienceYoukai/script_task.py`（124 行）— run / experience_exit / battle_wait 重写
- `tasks/ExperienceYoukai/config.py` — `buff_exp_50_click` / `buff_exp_100_click`（默认 False）、`switch_soul`
- `tasks/ExperienceYoukai/assets.py` — `I_EXP_WIN`
- `tasks/Component/GeneralRoom/general_room.py` — create_room / ensure_public / create_ensure / check_zones
- `tasks/Component/GeneralInvite/general_invite.py` — is_in_room / click_fire
- `tasks/Component/GeneralBattle/general_battle.py` — run_general_battle / battle_before / battle_wait

## 行为摘要（旧仓）

1. 可选换御魂（默认关）→ 可选经验加成（默认关）→ `count=0`。
2. `while count < 2`：`goto_page(page_team)` → `check_zones('经验妖怪')` → `create_room()`（失败则 exit_task）
   → `ensure_public()` → `create_ensure()` → 在房间里等 50 秒或有人加入 → `click_fire()` → `run_general_battle()`（count+=1）。
3. `experience_exit()`：回庭院，若开了加成则关掉，`set_next_run`。

## 流程（flow.json）

| # | op | 说明 | 守卫 |
|---|----|------|------|
| 1 | 进组队页 | 若在房间先退出；主界面点组队 | expect `I_CHECK_TEAM` |
| 2 | loop×2 | 每轮： | |
| 2a | check_zones('经验妖怪') | 重置列表到顶 → 循环找副本名（`text_visible`）→ 用行内 OCR 点击 | 列表 ROI (26,106,360,549) |
| 2b | create_room | 点黄色创建按钮直到出现创建确认框 | `I_CREATE_ROOM`/`I_CREATE_ENSURE*` |
| 2c | 无确认框 | 无挑战券 → `finish(no_ticket)` | meta exit resource |
| 2d | ensure_public | 勾选公开复选框 | `I_ENSURE_PUBLIC*` |
| 2e | create_ensure | 点确认直到消失 | |
| 2f | 等队友/开战 | 有界等待（20×2.5s）→ 有人/超时后点挑战直到离开房间 | `I_ADD_5_1`/`I_FIRE` |
| 2g | 战斗 | 准备高亮 → `战斗_自动按钮` → 点继续直到回组队页/房间页 | 胜/负/奖励模板 |
| 2h | 退出房间 | 确认框/房间返回直到组队页 | |
| 3 | 黄返回 | 组队页 → 庭院 | expect `I_CHECK_MAIN` |
| 4 | finish done | | |

## 素材（全部全局库）

| 全局 id | 来源 | 用途 |
|---------|------|------|
| legacy_ExperienceYoukai_I_EXP_WIN | 任务 | 胜利结算 |
| legacy_Component_GeneralRoom_I_CREATE_ROOM / I_CREATE_ENSURE(_2) / I_ENSURE_PUBLIC(_2/_FALSE/_FALSE_2) / I_GR_BACK_YELLOW / O_GR_ZONES_NAME | Component/GeneralRoom | 房间创建/公开/退出/副本名 OCR |
| legacy_Component_GeneralInvite_I_FIRE / I_FIRE_FAIL / I_GI_EMOJI_1/2 / I_GI_SPEAK / I_ADD_5_1 | Component/GeneralInvite | 进房判定 / 挑战 |
| legacy_Component_GeneralBattle_I_PREPARE_HIGHLIGHT / I_WIN / I_DE_WIN / I_FALSE / I_REWARD / C_REWARD_2 | Component/GeneralBattle | 战斗准备/胜利/失败/奖励 |
| legacy_GameUi_I_MAIN_GOTO_TEAM / I_CHECK_TEAM / I_CHECK_MAIN | GameUi | 导航 |
| legacy_GlobalGame_I_UI_BACK_YELLOW / I_UI_CONFIRM / I_UI_REWARD / C_UI_REWARD | GlobalGame | 返回/确认/奖励 |

## 出口

- `done`(success)：完成 2 次挑战并回庭院。
- `no_resource`(resource)：创建房间失败（旧仓此时直接结束任务）。
- `failed`(retryable)：页面/战斗守卫失败。

## 与旧仓的差异 / 已知缺口

1. **50 秒等待计时器**改为「20×2.5s 有界等待」（无 wall-clock，语义近似）；无队友时同样直接开战。
2. **check_zones 的 RuleList 滚动**：`L_TEAM_LIST` 是旧仓 RuleList（importer 不导入 List）；本 flow 用
   「`text_visible(副本名)` + 行内 OCR 点击 + 列表内 swipe」等价实现。列表 ROI 取自 `L_TEAM_LIST.roi_back=(26,106,360,549)`。
3. **buff 加成（经验 50/100）未迁移**：`GeneralBuff` 的开关区域是「OCR 文本框右侧 ± 偏移」动态计算，
   DSL 没有锚点定位（ENGINE §22.14 未实现）→ 记录缺口（默认配置关闭）。
4. **换御魂（switch_soul）未迁移**：`Component/SwitchSoul` 资产已进全局库，但换御魂页面流程（式神录→预设组/队→确认）未写；默认关闭。
5. 战斗只保留「准备→自动战斗→结算→页面回归」主链；绿标/预设队伍/随机点击未迁移（`GeneralBattleConfig` 默认全 false）。
6. 20 秒表情防挂机（`timer_emoji`）未迁移。
7. 未真机验证。

## Lint

```powershell
cd D:\OAS-dev\oas-core
.\target\release\oasd.exe task lint --tasks-dir tasks --task ExperienceYoukai
```
