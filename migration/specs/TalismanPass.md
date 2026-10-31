# TalismanPass — 行为矩阵（迁移规格）

源：`runhey/OnmyojiAutoScript@dev` · `tasks/TalismanPass/`

## 源文件

| 文件 | 作用 |
|------|------|
| tasks/TalismanPass/script_task.py | run / get_all / get_flower / in_task / harvest_soul |
| tasks/TalismanPass/config.py | LevelReward 枚举（ONE/TWO/THREE）、harvest_soul |
| tasks/TalismanPass/assets.py | 14 模板（GET_ALL、红点、档位、SOUL 奖励） |
| tasks/GameUi/default_pages.py | page_daily 定义与连接 |
| tasks/GameUi/navigator.py | goto_page / ui_click / ui_get_reward / ui_reward_appear_click |

## 行为矩阵

| 旧仓行为 | 新 flow 覆盖 | 备注 |
|----------|--------------|------|
| goto_page(page_daily) | ✅ click I_MAIN_GOTO_DAILY + expect I_CHECK_DAILY | 从庭院出发 |
| in_task() 判定 | ◑ 省略，直接以 GET_ALL / 红点分支判断 | 旧仓 in_task 只在收全奖励前用 |
| ui_get_reward(I_TP_GET_ALL) | ✅ click + collect_reward | 奖励动画用 GlobalGame 通用 UI |
| 红点 → 点档位 → 溢出确认 → 收奖励 → 补点 GET_ALL | ✅ loop 12 内顺序判断 | 顺序与旧仓 while 一致 |
| 2s 无动作退出 | ✅ 12 轮有界 | 计时语义差异 |
| harvest_soul: SOUL1→SOUL2→SOUL3 循环 | ✅ loop 30 + 内层领取 loop 40 | |
| 5s 无奖励退出 | ✅ 有界 30 轮 | |
| 关闭/回庭院 | ✅ 黄返回直到 I_CHECK_MAIN | |

## 素材

全部全局 `legacy_TalismanPass_*` + `legacy_GameUi_I_*` + `legacy_GlobalGame_*`（见 README）。

## 出口

`done` / `failed`。

## 缺口 / TODO

1. `appear_multi_scale` 无 DSL op → SOUL_1 在多分辨率/缩放下可能漏识别；需要引擎新增 multi-scale appear 或在旧仓标定单尺度阈值。
2. 计时器语义（2s/5s）改为有界循环；若需精确等价，需要引擎给 DSL 增加时间函数（`now()`/`elapsed()`）。
3. 未真机验证。
