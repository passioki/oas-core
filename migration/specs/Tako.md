# Tako — 行为矩阵（迁移规格）

源：`runhey/OnmyojiAutoScript@dev` · `tasks/Tako/`（Daily Task，石距）

## 源文件

| 文件 | 作用 |
|------|------|
| tasks/Tako/script_task.py | run / exit_task / battle_wait 重写 |
| tasks/Tako/config.py | TakoConfig（4 个加成开关）、switch_soul |
| tasks/Tako/assets.py | 空（无任务自身素材） |
| tasks/Component/GeneralRoom/general_room.py | create_room / ensure_public / create_ensure / check_zones |
| tasks/Component/GeneralInvite/general_invite.py | is_in_room / click_fire |
| tasks/Component/GeneralBattle/general_battle.py | run_general_battle / battle_wait |

## 行为矩阵

| 旧仓行为 | 新 flow 覆盖 | 备注 |
|----------|--------------|------|
| switch_soul（可选） | ❌ | 默认关闭 |
| buff 4 开关（可选） | ❌ | 默认关闭；需锚点定位 |
| goto_page(page_team) | ✅ | |
| 周末/工作日副本选择 | ◑ param('weekend') | DSL 无日期函数 |
| check_zones（含「价悠的石距」OCR 重映射） | ✅ text_visible + 行内 OCR + swipe | RuleList 未导入 |
| create_room；失败 exit_task | ✅ / no_resource | |
| ensure_public / create_ensure | ✅ | |
| 60s 等待或有人 → click_fire | ◑ 有界 20×2.5s | |
| run_general_battle（只一场） | ✅ 准备高亮 → 自动 → 结算 | |
| exit_task 回庭院 | ✅ | |
| set_next_run | meta.exit / scheduler | |

## 素材

见 README；任务自身 assets.py 为空，全部来自 Component/GameUi/GlobalGame 全局素材。

## 出口

`done` / `no_resource` / `failed`。

## 缺口 / TODO

1. DSL 缺日期/星期函数：周末「愤怒的石距」无法自动判定 → 用参数 `weekend`。
2. GeneralBuff 动态开关区域需要锚点定位 op（ENGINE §22.14 未实现）。
3. RuleList 未导入 → check_zones 等价实现。
4. 60 秒等待计时器 → 有界循环。
5. 未真机验证。
