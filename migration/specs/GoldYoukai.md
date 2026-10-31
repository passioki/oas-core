# GoldYoukai — 行为矩阵（迁移规格）

源：`runhey/OnmyojiAutoScript@dev` · `tasks/GoldYoukai/`

## 源文件

| 文件 | 作用 |
|------|------|
| tasks/GoldYoukai/script_task.py | run / battle_wait 重写 / gold_exit |
| tasks/GoldYoukai/config.py | buff_gold_50_click、buff_gold_100_click、switch_soul |
| tasks/GoldYoukai/assets.py | I_GOLD_WIN |
| tasks/Component/GeneralRoom/general_room.py | create_room / ensure_public / create_ensure / check_zones |
| tasks/Component/GeneralInvite/general_invite.py | is_in_room / click_fire |
| tasks/Component/GeneralBattle/general_battle.py | run_general_battle / battle_wait |

## 行为矩阵（与 ExperienceYoukai 同构）

| 旧仓行为 | 新 flow 覆盖 | 备注 |
|----------|--------------|------|
| switch_soul / buff 金币 50·100 | ❌ | 默认关闭；buff 需锚点定位 |
| goto_page(page_team) | ✅ | |
| check_zones('金币妖怪') | ◑ text_visible + 行内 OCR + swipe | RuleList 未导入 |
| create_room / 没券退出 | ✅ / no_resource | |
| ensure_public / create_ensure | ✅ | |
| 50s 等待 → 开战 | ◑ 有界 20×2.5s | |
| click_fire | ✅ | |
| run_general_battle | ✅ 准备高亮 → 自动 → 结算 | 绿标/预设未迁移 |
| count<2 | ✅ loop 2 | |
| gold_exit 回庭院 | ✅ | |

## 素材

见 README。

## 出口

`done` / `no_resource` / `failed`。

## 缺口 / TODO

同 ExperienceYoukai：GeneralBuff 动态开关（锚点定位缺失）、RuleList、50s 计时、表情防挂机、真机验证。
