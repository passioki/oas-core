# KekkaiUtilize（寮结界寄养/蹭卡与收取）迁移规格

- 源：`runhey/OnmyojiAutoScript@dev` commit `8b4dc66`；只读 `_verify/remote-oas`
- 结论：**收取类行为已表达**（寮结界 → 寄养收获 → 体力/经验盒子 → 寮资金/寮体力 → 回庭院）；
  **核心「蹭卡/寄养育成」⛔ 架构阻塞**（见 §6），状态 **⛔ blocked**。

## 1. 源文件

| 文件 | 作用 |
|---|---|
| `tasks/KekkaiUtilize/script_task.py`（761 行） | goto_realm、蹭卡 check_utilize_add、育成换卡 check_max_lv、收获 check_utilize_harvest、盒子 check_box_ap_or_exp、寮资金/体力 recive_guild_ap_or_assets |
| `tasks/KekkaiUtilize/config.py` | utilize_rule / select_friend_list / shikigami_class/order / min_run_interval / harvest_guild_max_times / utilize_enable / guild_ap_enable / guild_assets_enable / box_ap_enable / box_exp_enable / box_exp_waste |
| `tasks/KekkaiUtilize/assets.py` | 59 素材：结界主界面 I_REALM_SHIN / I_SHI_GROWN / I_SHI_CARD / I_GUILD_REALM；盒子 I_BOX_AP/_EXP、I_AP_EXTRACT/I_EXP_EXTRACT；寄养 I_UTILIZE_ADD/I_UTILIZE_EXP/O_UTILIZE_RES_TIME；卡片 I_U_TAIKO_*/I_U_FISH_*/I_U_MOON_*；好友 I_CHECK_FRIEND_REALM_*；O_BOX_EXP/O_CARD_NUM |
| `tasks/Component/ReplaceShikigami/*` | 育成换式神（I_RS_LEVEL_MAX 等） |
| `tasks/GameUi/page.py` | page_guild / page_main 导航 |

## 2. 入口/出口

- 入口：`page_guild` → `goto_realm`（I_GUILD_REALM → I_REALM_SHIN / I_SHI_DEFENSE；先关 I_PLANT_TREE_CLOSE）。
- 出口：`goto_page(page_main)`；或 `utilize_enable=false` 时直接 success 结束。
- 新 flow 从庭院出发：庭院→阴阳寮（`I_MAIN_GOTO_GUILD`）→ 寮结界；收尾回庭院。
- 退出表：`done`(success)、`no_resource`(resource 预留)。

## 3. 行为矩阵

| # | 状态 | 判定 | 行为 |
|---|---|---|---|
| S1 | 寮结界主界面 | `I_REALM_SHIN` / `I_SHI_DEFENSE` | 关闭种植树弹窗 → 进入结界 |
| S2 | 寄养收获 | `I_UTILIZE_EXP` | 领取收获（奖励弹窗循环） |
| S3 | 蹭卡入口 | `I_UTILIZE_ADD` | 进入育成→蹭卡页（**主功能：选卡/选友/时长 → ⛔**） |
| S4 | 育成满级换卡 | `I_RS_LEVEL_MAX` / `I_RS_NO_ADD`（ReplaceShikigami） | 下阵/换卡/按类型上阵（**未迁移**） |
| S5 | 体力盒子 | `I_BOX_AP` → `I_AP_EXTRACT` → `I_UI_REWARD` | 点击收取直到奖励消失，返回结界 |
| S6 | 经验盒子 | `I_BOX_EXP`/`I_BOX_EXP_MAX`、`O_BOX_EXP`、确认/取消（box_exp_waste） | 领取；达上限不领（OCR 判断未迁移） |
| S7 | 寮资金/体力 | `I_GUILD_AP`、`I_GUILD_ASSETS`、`I_GUILD_ASSETS_RECEIVE`、`I_GUILD_EXPAND` | 收取并领奖 |
| S8 | 回庭院 | — | 返回按钮链 |

## 4. 参数映射

| 旧配置 | 新 params | 说明 |
|---|---|---|
| `utilize_enable`（默认 true） | 固定行为（尝试寄养收获，主功能未实现） | ⛔ |
| `box_ap_enable`/`box_exp_enable`/`box_exp_waste` | `harvest_boxes`（bool，默认 true） | 收盒子开关（简化） |
| `harvest_guild_max_times` | 固定 1 轮 | 未参数化 |
| `shikigami_class`/`shikigami_order`/`select_friend_list`/`utilize_rule`/`min_run_interval` | 未映射 | 属蹭卡/育成主功能 |

## 5. DSL 映射

- 结界入口：`loop` + `appear(I_REALM_SHIN/I_SHI_DEFENSE)` / click `I_GUILD_REALM` / `I_PLANT_TREE_CLOSE`。
- 收取：`if appear(I_UTILIZE_EXP/I_BOX_AP/I_BOX_EXP...)` → `click` + `loop` 点奖励 `GlobalGame.I_UI_REWARD/I_UI_AWARD`。
- 寮资金：`I_GUILD_EXPAND`/`I_GUILD_ASSETS_RECEIVE`/`I_GUILD_ASSETS`/`I_GUILD_AP` 依次点击。

## 6. 差异 / 未覆盖（⛔ 具体阻塞）

1. **⛔ 蹭卡（寄养）主功能**：旧 `run_utilize` 需要
   (a) `O_UTILIZE_RES_TIME.ocr` → **timedelta 时长解析**（DSL 只有 `ocr_digit` 首数字，无时长）；
   (b) `O_CARD_NUM`/卡类型模板 → 选最优卡（旧用 CardClass 优先级 + 卡片模板 I_U_*）；
   (c) 好友列表 OCR（`O_FRIEND_NAME_*`/`O_F_LIST_*`/`O_ONLINE`）+ **名称相似匹配**选好友；
   (d) 进入好友结界/确认/回调流程。
   → DSL 均无对应原语，故本任务只实现「蹭卡入口检测 + 告警」。
2. **⛔ 式神育成换卡**：`check_max_lv` 需 `ReplaceShikigami` 组件（满级下阵、按 class 筛选、上阵 7 号位、长按/多选）——DSL 无长按/列表选择/多步状态机原语。
3. **经验盒子上限判断**：`O_BOX_EXP` 三元组 OCR + `box_exp_waste` 确认/取消选择，未迁移。
4. **min_run_interval / 下一次运行时间**：旧仓按卡剩余时长 `set_next_run`；新引擎无流程内调度原语（调度层窗口兜底）。
5. **无实机/replay 证据**（模拟器 offline）。
