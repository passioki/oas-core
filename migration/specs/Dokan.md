# Dokan（寮道馆突入）迁移规格

- 源：`runhey/OnmyojiAutoScript@dev` commit `8b4dc66`；只读 `_verify/remote-oas`
- 结论：**⛔ 架构阻塞（核心不可表达）**。当前只迁移「进入道馆场景 + 15 种场景中的简单分支（等待/关闭/投票/加入战斗）+ 回庭院」；
  道馆选择与战斗主链路需要 DSL 不具备的动态扫描/几何/状态机原语，详见 §6。

## 1. 源文件

| 文件 | 作用 |
|---|---|
| `tasks/Dokan/script_task.py`（1186 行） | 主状态机（15 种 `DokanScene`）：找馆/创建馆/入寮境/集结/馆主战（两阵容）/普通战斗/CD 加油/放弃投票/失败投票/结束；`dokan_battle_1`、`find_challengeable`、`find_all_element`、`creat_dokan`、`anti_detect`、`is_battle_end` 等 |
| `tasks/Dokan/config.py` | dokan_attack_priority、dokan_auto_cheering_while_cd、random_delay、try_start_dokan、find_dokan_score(4.6)、min_people_num、min_bounty、find_dokan_refresh_count、switch_preset_enable/preset_group_1/2、green_mark_shikigami_name；AttackAccountConfig（remain_attack_count/attack_date/daily_attack_count/attack_dokan_master 6 种策略） |
| `tasks/Dokan/dokan_scene.py` | `DokanSceneDetector`：用多组图像/OCR/区域颜色把当前画面判定为 15 种场景之一 |
| `tasks/Dokan/ex_green_mark.py`、`utils.py` | 绿标扩展、`detect_safe_area2`（最大纯色安全点击区） |
| `tasks/Dokan/assets.py` | 144 素材：I_RYOU_DOKAN_* 场景标记、I_RYOU_DOKAN_ATTACK_PRIORITY_0..4、C_DOKAN_* 点击区、O_DOKAN_MAP/ATTACKING/GATHERING/BOSS_WAITING、S_DOKAN_LIST_UP |
| `tasks/Component/GeneralBattle/*`、`SwitchSoul` | 战斗/御魂/预设 |

## 2. 入口/出口

- 入口：`page_guild` → `I_RYOU_SHENSHE`（神社）→ `I_RYOU_DOKAN`（道馆）→ 道馆场景（`DokanSceneDetector` 判定）。
- 出口：`goto_main()`：从任意道馆界面点退出（`I_RYOU_DOKAN_QUIT_BATTLE_ENSURE`/`I_RYOU_DOKAN_DOKAN_QUIT`/`I_UI_BACK_BLUE`/`I_BACK_Y`）回庭院。
- 新 flow：庭院→寮→神社→道馆→场景轮询分支→回庭院。
- 退出表：`done`(success)、`no_target`(blocked，选馆/入寮境不可表达)、`failed`(retryable)。

## 3. 行为矩阵（旧仓 15 场景，节选）

| 场景 | 判定素材 | 行为 | 迁移 |
|---|---|---|---|
| RYOU（寮主界面） | 场景检测 | goto 神社 | 部分 |
| SHENSHE（神社） | 场景检测 | 点道馆入口 | 部分 |
| FINDING_DOKAN 寻找道馆 | `I_RYOU_DOKAN_FINDING_DOKAN` | `find_dokan`：扫描列表（赏金/人数模板+OCR+评分）→点符合条件的馆；可创建道馆 | ⛔ |
| FOUND_DOKAN 已找到 | `I_RYOU_DOKAN_FOUND_DOKAN` | `enter_dokan`：`O_DOKAN_MAP.ocr_full` 取关键词坐标 → 点（x, y-20） | ⛔ |
| GATHERING 集结中 | `I_RYOU_DOKAN_GATHERING` | 选攻击优先级（`I_RYOU_DOKAN_ATTACK_PRIORITY_0..4`）、换御魂、等待 | 部分（只等待） |
| BOSS_WAITING 馆主等待 | `I_DOKAN_BOSS_WAITING` | 换御魂、必要时放弃突破、等待 | 部分 |
| MASTER_BATTLING 馆主战可挑战 | `I_RYOU_DOKAN_MASTER_BATTLE` | 点 `I_RYOU_DOKAN_START_CHALLENGE`；按 `attack_dokan_master` 策略 | 部分（只点挑战） |
| BATTLE_MASTER_FIRST/SECOND | `I_RYOU_DOKAN_BATTLE_MASTER_FIRST/SECOND` | 预设阵容（preset_group_2）+ `dokan_battle` 打两套阵容 | ⛔ |
| IN_FIELD 普通战斗 | `I_RYOU_DOKAN_IN_FIELD` | preset_group_1 + `dokan_battle`（绿标/锁定/自动战斗） | ⛔ |
| BATTLE_OVER / WIN | `I_RYOU_DOKAN_BATTLE_OVER`/`I_RYOU_DOKAN_WIN` | 点 `C_DOKAN_TOPPA_RANK_CLOSE_AREA` 退出奖励 | 部分 |
| TOPPA_RANK 突破排名 | `I_RYOU_DOKAN_TOPPA_RANK` | 点关闭 | 部分 |
| CD 中 | `I_RYOU_DOKAN_CD` | `start_cheering` 自动加油 | 部分（只等待） |
| FIGHTING 战斗中 | `I_RYOU_DOKAN_FIGHTING` | 等待 | 部分 |
| ABANDON_VOTE 放弃突破投票 | `I_RYOU_DOKAN_ABANDONED_TOPPA_ABANDONED` | 点放弃 | ✅ |
| FAILED_VOTE 失败投票 | `I_RYOU_DOKAN_FAILED_VOTE_BATTLE_AGAIN/KEEP_BOUNTY` | 再战/保留赏金（按 daily_attack_count） | 部分（保留赏金） |
| FINISHED 已结束 | `I_RYOU_DOKAN_FINISHED` | 更新剩余次数、退出 | 部分 |

## 4. 参数映射

| 旧配置 | 新 params | 说明 |
|---|---|---|
| `try_start_dokan`（默认 false） | `try_start_dokan`（bool，默认 false） | 仅记录；创建道馆流程未迁移 |
| `find_dokan_score`/`min_people_num`/`min_bounty`/`find_dokan_refresh_count` | 未映射 | 属 find_dokan 动态扫描 |
| `dokan_attack_priority` | 未映射 | 需点 `I_RYOU_DOKAN_ATTACK_PRIORITY_n`；flow 未选 |
| `attack_count_config`（remain_attack_count/attack_date/daily_attack_count/attack_dokan_master） | 未映射 | 需跨日持久化与 6 种策略计算 |
| `switch_soul_config`/`switch_preset_enable`/`preset_group_*` | 未映射 | 御魂/预设无原语 |
| `dokan_auto_cheering_while_cd` | 未映射 | 加油需识别加油按钮/分数 |
| `random_delay`/`anti_detect_click_fixed_random_area` | 未映射 | 引擎点击已人类化；安全区检测无原语 |

## 5. DSL 映射

- 入口：`home_to_guild` → 循环点 `I_RYOU_SHENSHE` → `I_RYOU_DOKAN`。
- 场景轮询：`while var('loops') < 60` + `if appear(场景素材)` 分支：
  - FINISHED → done；BATTLE_OVER/WIN/TOPPA_RANK → 点关闭区域；
  - GATHERING/BOSS_WAITING/FIGHTING → 等待；
  - START_CHALLENGE → 点击加入战斗（战斗本体未迁移）；
  - CD → 等待（加油未迁移）；
  - ABANDONED_TOPPA_ABANDONED / FAILED_VOTE_KEEP_BOUNTY → 点击；
  - FINDING_DOKAN / FOUND_DOKAN → `no_target`(blocked) 并记录 ⛔ 原因。
- 退出：`I_RYOU_DOKAN_QUIT_BATTLE_ENSURE`/`I_RYOU_DOKAN_DOKAN_QUIT`/`I_UI_BACK_BLUE`/`GameUi.I_BACK_Y` 循环到庭院。

## 6. 差异 / 未覆盖（⛔ 具体阻塞）

1. **⛔ 道馆选择 `find_dokan`/`find_challengeable`**：需要
   - `find_all_element` 对整列表做模板扫描得到多个命中框（`I_RIGHTPAD_POINT_BOUNTY`、`I_CENTER_POINT_PEOPLE_NUMBER`）；
   - 对每个候选点 OCR 赏金/人数、计算 score、比较 `find_dokan_score`/`min_people_num`/`min_bounty`；
   - 动态修改模板 `roi_back`（按候选框移动搜索区）并重复点击使挑战按钮出现；
   - `C_DOKAN_CANCEL_SELECT_DOKAN` 多点直到取消。
   DSL 无「多命中扫描 + 每个命中的子区域 OCR + 评分排序 + 动态 ROI」原语。
2. **⛔ 进入寮境 `enter_dokan`**：`O_DOKAN_MAP.ocr_full(keyword)` 返回关键词文本框，点 `(x+w/2, y-20)`——需要在 OCR 结果中**按关键词找框并计算偏移点**；DSL 只有 `ocr_text` 精确匹配 locator（可点匹配行，但不能点「行上方 20px」）。
3. **⛔ 战斗 `dokan_battle`/`dokan_battle_1`**：两套馆主阵容、绿标（`ExtendGreenMark` 按式神名点绿标）、锁定阵容、`is_battle_end`（图像/OCR 组合判定）、`detect_safe_area2` 最大纯色区随机点击（防封）——无原语。
4. **⛔ 跨日状态 `attack_count_config`**：`remain_attack_count`/`attack_date` 持久化 + `init_attack_count` + `attack_dokan_master_count` 6 种策略计算——无配置读写原语（只能靠任务外部调度）。
5. **⛔ 创建道馆 `creat_dokan`/自动加油 `start_cheering`**：多步确认/OCR 分数/加油按钮状态机——无原语。
6. **无实机/replay 证据**（模拟器 offline）。
