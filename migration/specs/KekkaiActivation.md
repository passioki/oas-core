# KekkaiActivation（结界挂卡）迁移规格

- 源：`runhey/OnmyojiAutoScript@dev` commit `8b4dc66`；只读 `_verify/remote-oas`
- 结论：**收取/激活已选卡部分已表达**；**核心「无卡时选最优卡挂卡」⛔ 架构阻塞**（见 §6），状态 **⛔ blocked**。

## 1. 源文件

| 文件 | 作用 |
|---|---|
| `tasks/KekkaiActivation/script_task.py`（435 行） | run：进寮结界→(exchange_before 换满级式神)→收卡经验→run_activation（进入挂卡界面→检测卡状态→选卡/激活→按剩余时长排下次运行）→(exchange_max)→回庭院 |
| `tasks/KekkaiActivation/config.py` | card_type(TAIKO 太鼓/FISH 斗鱼)、min_taiko_num(8)/min_fish_num(16)、exchange_before/exchange_max、shikigami_class、card_not_found_count |
| `tasks/KekkaiActivation/assets.py` | 42 素材：挂卡页 I_A_CHECK_CARD/I_A_AUTO_INVITE/I_SHI_CARD、卡状态 I_A_EMPTY/I_A_INVITE/I_A_ACTIVATE_YELLOW/GRAY/I_A_DEMOUNT、收卡经验 I_A_HARVEST_*、卡类型 I_CARDS_TAIKO/FISH/MOON_*、O_CARD_ALL_TIME |
| `tasks/KekkaiUtilize/script_task.py` | 父类：goto_realm/realm_goto_grown/check_max_lv |
| `tasks/Component/ReplaceShikigami/*` | 满级换卡 |

## 2. 入口/出口

- 入口：寮 → 寮结界（同 KekkaiUtilize）。
- 出口：回结界 → `goto_page(page_main)`。
- 新 flow：庭院→阴阳寮→寮结界→收卡经验→挂卡界面→已生效/已选未激活卡处理→回庭院。
- 退出表：`done`(success)、`no_target`(blocked，无卡且选卡未迁移)、`no_resource`。

## 3. 行为矩阵

| # | 状态 | 判定 | 行为 |
|---|---|---|---|
| S1 | 寮结界 | `I_REALM_SHIN`/`I_SHI_DEFENSE` | 进入结界 |
| S2 | 收卡经验 | `I_A_HARVEST_EXP` 等 8 种候选 | 点击领取 |
| S3 | 进入挂卡页 | `I_A_CHECK_CARD`/`I_A_AUTO_INVITE`；点击 `I_SHI_CARD`（多尺度） | 进入 |
| S4 | 已有生效卡 | `I_A_INVITE` | 读剩余时长（旧 `O_CARD_ALL_TIME.ocr_duration`）；按剩余时长排下次运行（**时长解析/调度未迁移**） |
| S5 | 已选未激活 | `I_A_ACTIVATE_YELLOW`/`I_A_ACTIVATE_GRAY` | 点激活→确认→直到 `I_A_INVITE`→读时长 |
| S6 | 无卡 | `I_A_EMPTY` | 旧 `screening_card`：切太鼓/斗鱼分类→`O_CHECK_CARD_NUMBER.detect_and_ocr` 行框→按第一个数字过滤/选最大→点击行框→确认挂卡（**⛔ 无法表达**） |
| S7 | 无合适卡 | `card_not_found_count` 计数 + 切卡类型 + 180 分钟重试（配置持久化，**未迁移**） |
| S8 | 换满级式神 | `I_RS_LEVEL_MAX`/`I_RS_NO_ADD`（ReplaceShikigami） | **未迁移** |
| S9 | 回庭院 | — | 返回按钮链 |

## 4. 参数映射

| 旧配置 | 新 params | 说明 |
|---|---|---|
| `card_type`（默认太鼓） | `card_type`（string，默认 taiko） | 仅记录；选卡主功能未实现 |
| `min_taiko_num`(8)/`min_fish_num`(16) | 未映射 | 依赖选卡 |
| `exchange_before`/`exchange_max` | 未映射 | 依赖 ReplaceShikigami |
| `shikigami_class` | 未映射 | 同上 |
| `card_not_found_count` | 未映射 | 配置持久化+重试调度 |

## 5. DSL 映射

- 收卡经验：8 种 `I_A_HARVEST_*` 逐个 `if appear → click`。
- 挂卡页：`I_SHI_CARD` 点击 + `I_A_CHECK_CARD/I_A_AUTO_INVITE` 守卫。
- 已选未激活：`I_A_ACTIVATE_YELLOW` 循环点击 + `GlobalGame.I_UI_CONFIRM`，直到 `I_A_INVITE`。
- 退出：`I_UI_BACK_RED` 回结界 → 返回链回庭院。

## 6. 差异 / 未覆盖（⛔ 具体阻塞）

1. **⛔ 选卡主功能**：`screening_card`/`check_card_num`：
   - `O_CHECK_CARD_NUMBER.detect_and_ocr` 返回**多行 OCR + 每行 box**；需按「含勾玉/体力」过滤、取每行第一个数字、`>= min_num`、按数字最大排序、**点击最大行的 box**；
   - DSL 无「多行 OCR 逐行数值 + 行框点击」原语（`ocr_text` locator 只做精确文本匹配）。
2. **⛔ 剩余时长解析**：`O_CARD_ALL_TIME.ocr_duration` 需要把识别文本解析为 `timedelta` 并设置下次运行；DSL 只有 `ocr_digit`（首数字），无时长解析/流程内调度。
3. **⛔ 未找到卡的重试状态机**：`card_not_found_count` 写回配置、切换卡类型、180 分钟后重试——DSL 无流程内调度/持久化配置写入。
4. **⛔ 式神育成换满级**：ReplaceShikigami 多步操作无原语。
5. **多尺度模板匹配**：`appear_then_click_multi_scale(I_SHI_CARD)` 未迁移（普通模板匹配近似）。
6. **无实机/replay 证据**（模拟器 offline）。
