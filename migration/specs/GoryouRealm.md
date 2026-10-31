# GoryouRealm — 行为矩阵（迁移规格）

源：`runhey/OnmyojiAutoScript@dev` · `tasks/GoryouRealm/`

## 源文件

| 文件 | 作用 |
|------|------|
| tasks/GoryouRealm/script_task.py | run：进入/选御灵/循环挑战/退出 |
| tasks/GoryouRealm/config.py | GoryouClass 枚举、limit_time、limit_count |
| tasks/GoryouRealm/assets.py | C_GR_C_1..4、I_GR_FIRE/LOCK/UNLOCK、O_GR_TICKET |
| tasks/Component/GeneralBattle/general_battle.py | run_general_battle / check_lock / battle_wait |
| tasks/GameUi/default_pages.py | page_goryou_realm、page_exploration 导航边 |

## 行为矩阵

| 旧仓行为 | 新 flow 覆盖 | 备注 |
|----------|--------------|------|
| switch_soul（可选） | ❌ | `switch_soul_config.enable` 默认 false；Component/SwitchSoul 资产未导入，记录缺口 |
| goto_page(page_goryou_realm) | ✅ 庭院→探索→御灵 | 两条页面边 |
| 随机/指定御灵选择 | ◑ 指定（param 1..4），RANDOM ❌ | DSL 无随机 op |
| 循环点 C_GR_C_* 直到 I_GR_FIRE | ✅ loop 10 等 FIRE | |
| check_lock | ✅ lock_team 参数分支 | |
| limit_count 30 | ✅ param('max_battles') 默认 30 | |
| limit_time 30min | ❌ | DSL 无时间函数 |
| O_GR_TICKET==0 → break | ✅ ocr_digit==0 → var empty | 等价 |
| 点 FIRE 直到消失 → run_general_battle | ✅ click+expect 准备页 | |
| battle_wait（win/false/绿标/随机点击） | ◑ 只保留「准备→自动→结算→页面回归」主链；胜负不判 | 与个人突破模式一致 |
| 黄返回探索 | ✅ | |
| set_next_run / TaskEnd | meta.exit done | 调度由 scheduler v3 负责 |

## 素材

见 README；全部全局 `legacy_GoryouRealm_*` + core + `legacy_GameUi_*` + `legacy_GlobalGame_*`。

## 出口

`done` / `resource_empty`（票=0）/ `failed`。

## 缺口 / TODO

1. 引擎表达式缺时间函数：`limit_time` 不能表达（TODO：给 `EvalContext` 增加 `now()`/`elapsed_s()`，或在 meta.goal 增加时长预算）。
2. 缺随机 op：旧仓默认 RANDOM 御灵无法复现（TODO：DSL 增加 `random(a,b)` 或 meta 里指定）。
3. `GeneralBattleConfig`（绿标/预设队伍/buff/随机点击/战败）未迁移（默认均 false，影响默认配置外的行为）。
4. 未真机验证。
