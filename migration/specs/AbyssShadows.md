# AbyssShadows（狭间暗影）迁移规格

- 源：`runhey/OnmyojiAutoScript@dev` commit `8b4dc66`；只读 `_verify/remote-oas`
- 结论：**主战斗链路已表达**（庭院→神社→狭间→神龙暗域→6 目标战斗→切换孔雀暗域→6 目标→回庭院），
  周几判定/自动换御魂/战斗时长策略等未完整迁移，状态 **🔄**。

## 1. 源文件

| 文件 | 作用 |
|---|---|
| `tasks/AbyssShadows/script_task.py`（569 行） | run：周几判定→进狭间→选神龙暗域→等待集结→按 首领/副将/精英 循环战斗（2/4/6 次）→切暗域（龙→孔雀→白藏主→黑豹）→回庭院 |
| `tasks/AbyssShadows/config.py` | AbyssShadowsTime（周五/六/日 19:00）、AbyssShadowsCombatTime（智能伤害/战斗秒数）、general_battle/switch_soul |
| `tasks/AbyssShadows/assets.py` | 64 素材：导航 I_ABYSS_NAVIGATION/I_ABYSS_MAP/I_ABYSS_SHADOWS；暗域选择 I_ABYSS_DRAGON/PEACOCK/FOX/LEOPARD + C_ABYSS_*；目标点击框 C_BOSS/C_GENERAL_1/2/C_ELITE_1..3_CLICK_AREA；战斗 I_ABYSS_FIRE/I_EQUIPPING/I_ENSURE_BUTTON/I_ABYSS_GOTO_ENEMY；状态 I_WAIT_TO_START/I_CHANGE_AREA/区域标记 |
| `tasks/Component/GeneralBattle/*` | 战斗/退出（I_EXIT/I_WIN/I_UI_CONFIRM） |
| `tasks/GameUi/page.py` | page_guild / page_main |

## 2. 入口/出口

- 入口：`page_guild` → `I_RYOU_SHENSHE`（神社）→ `I_ABYSS_SHADOWS`（狭间入口，找不到时 swipe）→ `I_ABYSS_NAVIGATION`。
- 出口：`goto_page(page_main)`；周几不符则直接退（旧仓 custom_next_run 到周五）。
- 新 flow：庭院→阴阳寮→神社→狭间→神龙暗域→…→回庭院。
- 退出表：`done`(success)、`no_target`(blocked)、`failed`(retryable)。

## 3. 行为矩阵

| # | 状态 | 判定 | 行为 |
|---|---|---|---|
| S1 | 进入狭间 | `I_RYOU_SHENSHE`、`I_ABYSS_SHADOWS` | 点入口；未找到则上滑 |
| S2 | 选择暗域 | `I_ABYSS_DRAGON` + `C_ABYSS_DRAGON/PEACOCK/FOX/LEOPARD` | 点对应区域 → `I_ABYSS_NAVIGATION` |
| S3 | 等待集结 | `I_WAIT_TO_START` | 等待其消失（旧 `wait_until_disappear`） |
| S4 | 找目标 | `I_ABYSS_MAP`、目标点击框 `C_*_CLICK_AREA` | 点战报/目标区域直到 `I_ABYSS_GOTO_ENEMY` |
| S5 | 前往 | `I_ABYSS_GOTO_ENEMY` + `I_ENSURE_BUTTON` | 点到 `I_ABYSS_FIRE` 出现 |
| S6 | 战斗 | `I_ABYSS_FIRE` → `I_EQUIPPING`（准备） | 进入准备界面 |
| S7 | 战斗过程 | 智能伤害开启：等待 `boss/general/elite_combat_time` 秒后主动退出（`I_EXIT`→确认→`I_WIN`） | 默认（关闭）用通用战斗样板直到回 `I_ABYSS_NAVIGATION` |
| S8 | 计数 | 首领×1、副将×2、精英×3/暗域；总需 2/4/6 | 未打满则切换下一暗域 |
| S9 | 切暗域 | `I_CHANGE_AREA` | 龙→孔雀→白藏主→黑豹 |
| S10 | 回庭院 | 返回按钮 | — |

## 4. 参数映射

| 旧配置 | 新 params | 说明 |
|---|---|---|
| `abyss_shadows_combat_time.CombatTime_enable` + 三个秒数 | `combat_time`（int，默认 0=关闭） | 0 表示走通用战斗；>0 时等待 N 秒后主动退出（用 `loop(param N, wait 1)` 实现动态等待） |
| 周几判定（周五/六/日） | 未映射 | DSL 无日历；由 scheduler.json 窗口表达 |
| `switch_soul_config` | 未映射 | 无御魂原语 |
| `general_battle_config` | 未映射 | lock/preset/green/anti-detect 无原语 |

## 5. DSL 映射

- 入口：`click(I_RYOU_SHENSHE)` → 循环 `click(I_ABYSS_SHADOWS)`；未出现时 `drag` 上滑（`S_TO_ABBSY_SHADOWS` FROM/TO）。
- 选暗域：`click_target(region C_ABYSS_DRAGON/...)` + `I_ABYSS_DRAGON` 守卫。
- 战斗：目标区域 `click_target(region C_*_CLICK_AREA)` → `I_ABYSS_GOTO_ENEMY` 循环 → `I_ABYSS_FIRE` → `I_EQUIPPING` → 通用战斗 / 计时退出。
- 暗域计数：flow 内固定打 2 个暗域（神龙+孔雀）覆盖旧仓 2/4/6 的总量要求。

## 6. 差异 / 未覆盖（🔄 TODO）

1. **周几判定**：旧仓只在周五/六/日运行（`datetime.now().weekday() in [4,5,6]`）并 `custom_next_run` 到下一个开放日；DSL 无日历 → 由调度器窗口（周五/六/日 19:00）表达。
2. **只打 2 个暗域**：旧仓在 2/4/6 总量未满时依次切 龙→孔雀→白藏主→黑豹，可能打 4 个暗域；新 flow 固定 2 个（神龙+孔雀），若目标被别人击杀导致数量不足则提前结束（`failed` 不触发，计数日志仅提示）。TODO：展开 4 暗域或按计数循环。
3. **战斗退出细节**：旧 `run_general_battle_back` 在智能伤害模式下有 4s 等待准备/多次点击；新 flow 用 `wait(param combat_time)` + `I_EXIT` 循环近似。
4. **`I_IS_ATTACK`/`I_ABYSS_MAP_EXIT`/`I_ABYSS_SHIKI` 等状态**未全部使用。
5. **switch_soul 御魂切换**未迁移。
6. **无实机/replay 证据**（模拟器 offline）。
