# ExperienceYoukai — 行为矩阵（迁移规格）

源：`runhey/OnmyojiAutoScript@dev` · `tasks/ExperienceYoukai/`

## 源文件

| 文件 | 作用 |
|------|------|
| tasks/ExperienceYoukai/script_task.py | run / battle_wait 重写 / experience_exit |
| tasks/ExperienceYoukai/config.py | buff_exp_50_click、buff_exp_100_click、switch_soul |
| tasks/ExperienceYoukai/assets.py | I_EXP_WIN |
| tasks/Component/GeneralRoom/general_room.py | create_room / ensure_public / create_ensure / check_zones |
| tasks/Component/GeneralInvite/general_invite.py | is_in_room / click_fire |
| tasks/Component/GeneralBattle/general_battle.py | run_general_battle / battle_before / battle_wait |

## 行为矩阵

| 旧仓行为 | 新 flow 覆盖 | 备注 |
|----------|--------------|------|
| switch_soul（可选） | ❌ | 默认关闭；组件资产已导入，流程未写 |
| buff 经验 50/100（可选） | ❌ | 默认关闭；需要锚点定位 |
| goto_page(page_team) | ✅ 主界面→组队 | expect I_CHECK_TEAM |
| check_zones('经验妖怪') | ◑ text_visible + 行内 OCR 点击 + swipe | RuleList 未导入 |
| create_room（≤3 次点击，失败=没券） | ✅ loop 8；无确认框 → no_ticket | 旧仓失败 → exit_task |
| ensure_public | ✅ 复选框循环 | |
| create_ensure | ✅ 点确认直到消失 | |
| 等待 50s / 有人就开 | ◑ 20×2.5s 有界等待 | 无 wall-clock |
| click_fire 直到离开房间 | ✅ I_FIRE / I_FIRE_FAIL | |
| run_general_battle（准备/胜利/失败/奖励） | ✅ 准备高亮 → 自动按钮 → 结算 | 绿标/预设未迁移 |
| count<2 两轮 | ✅ loop 2 | |
| experience_exit 回庭院 | ✅ 黄返回 | |
| set_next_run | meta.exit / scheduler | |

## 素材

见 README；全部为全局素材库 id（任务 + Component + GameUi + GlobalGame）。

## 出口

`done` / `no_ticket`(resource) / `failed`。

## 缺口 / TODO

1. GeneralBuff 动态开关区域（OCR 文本框右侧）需要锚点定位 op（ENGINE §22.14 未实现）→ buff 未迁移。
2. RuleList 未导入 → check_zones 用 text_visible + 行内 OCR 点击等价实现。
3. 50 秒等待/20 秒表情计时器无 wall-clock → 有界循环近似。
4. buff/换御魂/绿标/预设队伍/随机点击未迁移（默认配置均关闭）。
5. 未真机验证。
