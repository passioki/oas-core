# AreaBoss（地域鬼王）迁移规格

- 源：`runhey/OnmyojiAutoScript@dev` commit `8b4dc66`；只读 `_verify/remote-oas`
- 结论：**主链路已表达**（进页 → 筛选 → 声望动态次数 → 逐个鬼王详情 → 难度/层数 → 挑战 → 通用战斗 → 回庭院）；
  悬赏鬼王（按 OCR 名称找热门目标）未迁移，状态 **🔄**。

## 1. 读取的源文件

| 文件 | 作用 |
|---|---|
| `tasks/AreaBoss/script_task.py` | 主流程：声望→次数、筛选、悬赏鬼王、鬼王详情、难度/层数、挑战、通用战斗 |
| `tasks/AreaBoss/config.py`、`config_boss.py` | `Boss` 配置：boss_reward(默认 false)/reward_floor(DEFAULT)/use_collect(false)；general_battle/switch_soul |
| `tasks/AreaBoss/assets.py` | 74 素材：筛选、难度（极/普通）、层数列表、I_AB_LEVEL_60/HANDLE、奖励头像、声望 OCR、战斗/关闭红叉 |
| `tasks/Component/GeneralBattle/*` | 通用战斗（准备/胜负/奖励） |
| `tasks/Component/SwitchSoul/*` | 御魂切换（未迁移） |
| `tasks/GameUi/page.py` | page_area_boss 导航（探索→地域鬼王） |

## 2. 入口 / 出口

- 入日：`page_area_boss`。新 flow：庭院→探索（`I_MAIN_GOTO_EXPLORATION`）→ `GameUi.I_EXPLORATION_GOTO_AREA_BOSS` → expect `GameUi.I_CHECK_AREA_BOSS`。
- 出口：`done`(success，正常打完 N 只)；`no_target`(blocked，列表无鬼王)；`no_resource`(resource 预留)；`failed`(retryable)。
- 收尾：循环点 `GlobalGame.I_UI_BACK_YELLOW` 直到 `GameUi.I_CHECK_MAIN`。

## 3. 行为矩阵

| # | 状态 | 判定 | 行为 |
|---|---|---|---|
| S1 | 筛选面板 | `I_AB_FILTER_OPENED` | 未开则点 `I_FILTER` 打开 |
| S2 | 集合切换 | `param use_collect` | 点 `C_AB_FAMOUS_BTN`（热门）或 `C_AB_COLLECTION_BTN`（收藏） |
| S3 | 可挑战数量 | `ocr_digit(O_AB_REPUTATION)`：≥10000→3，≥2000→2，否则 1 | 旧 `_get_dynamic_boss_count` 等价 |
| S4 | 无鬼王 | `I_BATTLE_1/2/3` 都不在 | `no_target`(blocked) |
| S5 | 开详情 | 点 `I_BATTLE_n`（挑战按钮/头像） | expect `I_AB_CLOSE_RED` 出现 |
| S6 | 难度（极/普通） | `I_AB_DIFFICULTY_JI` / `I_AB_DIFFICULTY_NORMAL` | 按 `floor_mode` 切换 |
| S7 | 极·层数 | `C_AB_JI_FLOOR_SELECTED` → `I_AB_JI_FLOOR_LIST_CHECK` | 找 `I_AB_JI_FLOOR_ONE/TEN`，找不到下滑（`S_AB_FLOOR_DOWN` 方向下） |
| S8 | 普通·等级 | `I_AB_LEVEL_HANDLE` / `I_AB_LEVEL_60` | 向右拖动滑条到 60 级 |
| S9 | 挑战 | `I_FIRE` | 点击进入通用战斗 |
| S10 | 战斗 | Component_GeneralBattle 准备/胜负/奖励 | `battle_until(I_AB_CLOSE_RED)` |
| S11 | 关详情 | `I_AB_CLOSE_RED` | 点到消失回列表 |

## 4. 参数映射

| 旧配置 | 新 params | 说明 |
|---|---|---|
| `boss_reward`（默认 false） | `boss_reward`（bool，默认 false） | true 时旧仓要找「挑战人数最多的悬赏鬼王」；新 flow 只告警跳过（见 §6-1） |
| `use_collect`（默认 false） | `use_collect`（bool，默认 false） | 收藏/热门 |
| `reward_floor`（DEFAULT/ONE/TEN/NORMAL_LV1/NORMAL_LV60） | `floor_mode`（string，默认 default；可选 ji_1/ji_10/normal_1/normal_60） | 旧枚举 → 字符串参数 |
| `general_battle.lock_team_enable` | 取消 | 旧仓强制 false |
| `switch_soul.*` | 未映射 | 无御魂原语 |

## 5. DSL 映射

- 次数分支：`ocr_digit(O_AB_REPUTATION)` + 嵌套 if 展开 3/2/1 次 boss_fight。
- 挑战：`click(I_BATTLE_n)` → `click(I_FIRE)` → `battle_until(I_AB_CLOSE_RED)` → 关红叉。
- 滑动：RuleSwipe 已导入为 `_FROM`/`_TO` region；楼层下滑用 `drag`（region + direction=down + distance）；等级滑条用 inline region + direction=right。

## 6. 差异 / 未覆盖（🔄 TODO）

1. **悬赏鬼王**（`fight_reward_boss`）：旧仓用 `O_AB_BOSS_NAME` OCR + 通用字判定（`check_common_chars`）在奖励/热门列表里找「挑战人数最多」的鬼王，再逐页比对名字；DSL 只有精确文本相等，无「公共子串/相似度」→ `boss_reward=true` 时跳过并告警。
2. **等级滑条**：旧仓以 `I_AB_LEVEL_HANDLE.front_center()` 作为 swipe 起点（动态坐标）；DSL 不能把模板命中点当拖拽起点 → 用固定 inline region [230,265,120,40] + 右拖近似，可能点不到手柄。
3. **`I_AB_NUM_CHALLENGE_RAIL` / `O_AB_NUM_OF_CHALLENGE` / `I_AB_GROUP_RANK_NONE` / `get_num_challenge` / `is_group_ranked`** 未使用（旧仓按挑战人数/已排行筛选目标）；
4. **switch_soul 御魂切换** 未迁移。
5. **无实机/replay 证据**（模拟器 offline）；筛选/难度/层数素材阈值需真机复验。
