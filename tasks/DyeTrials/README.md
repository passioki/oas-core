# DyeTrials（灵染试炼）

## 来源（只读旧仓）

- `tasks/DyeTrials/script_task.py`（97 行）— run / get_all
- `tasks/DyeTrials/config.py` — `switch_soul_config`（默认关闭）
- `tasks/DyeTrials/assets.py` — I_FP_ACCESS / I_FP_ACCESS_1 / I_TOGGLE_BUTTON / I_FP_CHALLENGE / I_BATTLE_SUCCESS、
  O_BATTLE_NUM（DigitCounter）
- `tasks/Component/GeneralBattle/general_battle.py`、`tasks/Component/SwitchSoul/switch_soul.py`（imported，可选）
- `tasks/Restart/assets.py` — `I_HARVEST_CHAT_CLOSE`（活动聊天关按钮）

## 行为摘要（旧仓）

1. 可选换御魂（默认关）→ `goto_page(page_main)`。
2. `get_all()` 入口循环：`I_FP_CHALLENGE` 出现即进入；否则依次点 `I_FP_ACCESS` / `I_FP_ACCESS_1` / `I_TOGGLE_BUTTON`。
3. 挑战页循环：
   - 60 秒计时（`boss_timer`）到 → 通知「识别超时退出」并 break；
   - `ui_reward_appear_click()`（出现「获得奖励」就点掉）；
   - `RestartAssets.I_HARVEST_CHAT_CLOSE` 出现就点；
   - `I_FP_CHALLENGE` 出现：读 `O_BATTLE_NUM`（当前/剩余/总数），`cu==total==50 && cu+res==total` → break；
     `battle_num>=50` → break；否则 `ui_click_until_disappear(I_FP_CHALLENGE)` 进入战斗，battle_num+1；
   - `I_BATTLE_SUCCESS` 出现就点。

## 流程（flow.json）

| # | op | 说明 | 守卫 |
|---|----|------|------|
| 1 | loop×30 | 入口：CHALLENGE 出现即停；否则依次点 ACCESS/ACCESS_1/TOGGLE | requires appear |
| 2 | appear CHALLENGE | 断言挑战页 | |
| 3 | loop×`param('max_battles')`(默认50) | 奖励/聊天关/成功/挑战 顺序判断；挑战一次 inc battles | |
| 4 | loop×10 | 回庭院（黄返回 / 红返回交替尝试） | 守卫 HOME |
| 5 | finish done | | |

## 素材

| 全局 id | 旧 const | 用途 |
|---------|----------|------|
| legacy_DyeTrials_I_FP_ACCESS / I_FP_ACCESS_1 / I_TOGGLE_BUTTON | 活动入口 | 进入灵染试炼 |
| legacy_DyeTrials_I_FP_CHALLENGE | I_FP_CHALLENGE | 挑战按钮 / 挑战页守卫 |
| legacy_DyeTrials_I_BATTLE_SUCCESS | I_BATTLE_SUCCESS | 战斗成功按钮 |
| legacy_DyeTrials_O_BATTLE_NUM | O_BATTLE_NUM | 旧仓次数读取（本 flow 未用，见差异） |
| legacy_GameUi_I_CHECK_MAIN | page_main | 回庭院守卫 |
| legacy_GlobalGame_I_UI_REWARD / C_UI_REWARD / I_UI_BACK_YELLOW / I_UI_BACK_RED | 通用 | 奖励 / 返回 |
| legacy_Restart_I_HARVEST_CHAT_CLOSE | 活动聊天关 | 关闭聊天提示 |

## 出口

`done`(success) / `failed`(retryable)。

## 与旧仓的差异 / 已知缺口

1. **60 秒 `boss_timer` 超时提示未迁移**：DSL 无 wall-clock；改为「`max_battles` 轮 + 每轮动作」的有界循环。
   旧仓超时会 `config.notifier.push` 通知，新引擎告警由 scheduler 承担（结果分类）。
2. **`O_BATTLE_NUM` DigitCounter 提前退出未实现**：旧仓在 50/50 已打满时提前 break；本 flow 以 `max_battles=50`
   为同一上限（多耗最多一轮时间）。DigitCounter 的 `cu/res/total` 三值语义在 DSL 里没有直接等价（`ocr_digit` 只取首个数字），
   记录为缺口。
3. **换御魂（switch_soul）未迁移**：`Component/SwitchSoul` 资产已在全局素材库（`legacy_Component_SwitchSoul_*`），但「式神录→换御魂→预设组/队→确认」页面流程未写（默认配置关闭，不影响默认行为）。
4. 未真机验证。

## Lint

```powershell
cd D:\OAS-dev\oas-core
.\target\release\oasd.exe task lint --tasks-dir tasks --task DyeTrials
```

<!-- round2:start -->
## Polish round 2（2026-11-01 · Lead 实机反馈）

实机反馈（Lead）：flow 一直循环点 `I_FP_ACCESS` / `I_TOGGLE_BUTTON`，永远到不了 `I_FP_CHALLENGE`。

原因：旧仓入口循环每个按钮都有 `interval` 节流（ACCESS 0.8s / ACCESS_1 1.5s / TOGGLE 3s），
原 flow 每次迭代「谁出现点谁」，导致 `I_TOGGLE_BUTTON` 被反复点击、活动页来回切换。

本次修复：
1. 入口外层最多 6 轮，**每轮只执行一个入口动作**（顺序 ACCESS → ACCESS_1 → TOGGLE），
   每次点击后固定 settle（1.2s / 1.8s / 2.0s），并立即检查 `I_FP_CHALLENGE`；
2. 6 轮后仍无 `I_FP_CHALLENGE` → 记录「活动未开启/入口结构变化」→ 回庭院 → `finish(no_target)`（meta 新增 blocked），
   **不再无界循环**；
3. 到达挑战页后循环挑战逻辑保持（奖励/聊天关/成功/挑战、`max_battles` 上限）。

剩余阻塞：若实机入口结构不是「ACCESS→ACCESS_1→TOGGLE」，需要 Lead 提供实机截图重新标定顺序；
`I_FP_ACCESS_1` 的真实跳转目标与 settle 时长未在实机复测。
<!-- round2:end -->
