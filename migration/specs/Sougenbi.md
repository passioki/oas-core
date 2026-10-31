# Sougenbi（业原火）迁移规格

- 源仓库：`runhey/OnmyojiAutoScript@dev` · commit `8b4dc669b47838f42d283f14aa394844c1963c86` · 只读副本 `D:/OAS-dev/_verify/remote-oas`
- 目标：`tasks/Sougenbi/`（统一任务模板：meta.json + flow.json + assets.json + images/，素材引用全局 `legacy_*`）
- 迁移结论：**🔄** —— 见 §3 行为矩阵与 §5 缺口

## 1. 读取的源文件

- `tasks/Sougenbi/script_task.py`
- `tasks/Sougenbi/config.py`
- `tasks/Sougenbi/assets.py`
- `tasks/Component/GeneralBattle/general_battle.py (imported)`
- `tasks/Component/SwitchSoul/switch_soul.py (imported)`
- `tasks/GameUi/game_ui.py, tasks/GameUi/page.py (imported)`

## 2. 入口 / 出口

- 入口：`meta.entry.requires = appear('庭院_探索灯笼')`；flow 首段用 `ensure_main()` 从任意页面回庭院。
- 出口：
  - `done` → result `success`
  - `resource_empty` → result `resource`
  - `failed` → result `retryable`

## 3. 行为矩阵

| # | 行为 | 远程实现（源码位置） | 新 DSL 映射 | 状态 |
|---|---|---|---|---|
| 1 | 进入业原火 | script_task.py:44-52 goto_page(page_soul_zones); appear/click I_S_SOUGENBI/I_S_CHECK_SOUGENBI | ensure_main→goto_world→click I_EXPLORATION_GOTO_SOUL_ZONE→loop click I_S_SOUGENBI→appear I_S_CHECK_SOUGENBI | 覆盖 |
| 2 | 选择贪/嗔/痴 | script_task.py:56-70 三枚举映射 image_target/click_target/number_target | 3 分支：click_target(region C_C_*) + wait_until(I_S_FIRE_*) | 覆盖 |
| 3 | 券数=0 退出 | script_task.py:91-93 number_target.ocr()==0 → break | ocr_digit(legacy_Sougenbi_O_S_GREED/ANGER/FOOLERY)==0 → finish(resource_empty) | 覆盖（出口从 success 改为 resource） |
| 4 | 阵容锁 | script_task.py:71 check_lock(lock_team_enable, I_S_TEAM_LOCK, I_S_TEAM_UNLOCK) | param('lock_team') 两分支点击对应图标直到目标状态出现 | 覆盖 |
| 5 | 挑战循环/次数上限 | script_task.py:80-102 while+count | loop n=param('limit_count') | 覆盖 |
| 6 | 战斗 | script_task.py:101 run_general_battle(config) | P-BATTLE（准备→自动→安全点→回页 marker） | 部分：通用骨架 |
| 7 | 回庭院 | script_task.py:104-111 直到 I_CHECK_EXPLORATION | ensure_main（back 模板 + I_CHECK_MAIN） | 覆盖 |
| 8 | 加成开关 | script_task.py:24-35,113-124 open_buff/gold_*/exp_* | — | 缺口：GeneralBuff 未迁移 |
| 9 | 御魂预设切换 | script_task.py:37-42 run_switch_soul* | — | 缺口：SwitchSoul 未迁移 |
| 10 | 时间上限 | script_task.py:88 limit_time | — | 缺口：无时钟原语 |

## 4. 参数映射

| key | type | default | 说明 |
|---|---|---|---|
| `sougenbi_class` | string | `痴` | 远程 sougenbi_config 同名字段（时间上限未映射） |
| `limit_count` | int | `30` | 远程 sougenbi_config 同名字段（时间上限未映射） |
| `lock_team` | bool | `False` | 远程 sougenbi_config 同名字段（时间上限未映射） |

## 5. 缺口 / TODO

- limit_time（30 分钟时间上限）无 DSL 时钟原语 → 只保留 limit_count 次数上限（调度层可用 --budget-wall 兜底）。
- 加成开关（GeneralBuff: gold_50/gold_100/exp_50/exp_100）未迁移：Component/GeneralBuff 素材未导入全局。
- 御魂预设切换（SwitchSoul: run_switch_soul / run_switch_soul_by_name）未迁移：DSL 无预设/御魂切换原语。
- 战斗用通用 P-BATTLE 骨架（准备→自动→安全点直到回页），不含 GeneralBattle 的绿标/预设阵容/失败分支/奖励统计页处理。
- class 页检测与券 OCR ROI 来自 remote dev 分支，未在当前游戏版本真机验证。

## 6. 通用模式与素材语义（本批任务共用）

- **P-NAV**：`legacy_GameUi_I_CHECK_MAIN` 守卫 + `click` 页面跳转素材（模板点击 = 匹配矩形）+ `expect` 目标页 marker。
- **P-BATTLE**：`战斗_准备按钮`→点击→`战斗_自动按钮`→每 2.5s 点 (640,690) 直到任务页 marker 回来。
- **P-RETURN**：`legacy_GlobalGame_I_UI_BACK_YELLOW` / `legacy_GameUi_I_BACK_YOLLOW` / 坐标兜底 → `I_CHECK_MAIN`。
- 素材契约（导入器 v2）：模板 `roi` = remote `roiBack`，**无 `action_roi`** ⇒ `click` 落在匹配到的模板矩形（= 旧引擎 `RuleImage.coord()` 行为）；`RuleClick` → `kind=region` 精确点击区；`RuleOcr` → `ocr_lines`（`ocr()`/`ocr_digit()`/`text_visible()`/`click_target ocr_text`）。

## 7. 证据

- oasd task lint 绿；已映射：导航/贪嗔痴选择/阵容锁开关/顶部券 OCR(resource_empty)/挑战→准备→通用战斗→回页循环(limit_count)。未真机/回放验证。
- 未真机/未回放：迁移会话无可用设备与对应场景回放帧；素材 T4 引用全部解析到全局 `assets/assets.json`。

## 8. Polish round (2026-11-01)

- 突破券=0 的退出路径修正：原 flow 在挑战循环内直接 `finish(resource_empty)`（停在业原火页）；现改为置位 + break，循环结束后照常回庭院，再 `finish(resource_empty)` —— 与远程「break 循环 → 返回探索/庭院」一致。
- `meta.entry.requires` 从 `庭院_探索灯笼` 改为 `legacy_GameUi_I_CHECK_MAIN`（page_main 标记）。



