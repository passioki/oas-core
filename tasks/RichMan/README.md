# RichMan（每周商店扫货）

## 来源（只读旧仓）

- `tasks/RichMan/script_task.py`（48 行）— run：`execute_tt` → `execute_shrine` → `execute_guild` → `execute_mall`
- `tasks/RichMan/config.py` — 6 大块配置（全部 `enable=False` 默认关闭）：
  `thousand_things` / `shrine` / `guild_store` / `consignment` / `scales` / `bondlings` /
  `special_room` / `honor_room` / `friendship_points` / `medal_room` / `charisma`
- `tasks/RichMan/thousand_things.py` / `shrine.py` / `guild.py` / `mall/*.py`（10 个模块）
- `tasks/RichMan/assets.py` — 120 项（模板/区域/OCR）
- `tasks/Component/Buy/buy.py` — buy_one / buy_more / buy_one_or_more / buy_check_money
- `tasks/Component/GeneralRoom`、`GameUi/default_pages.py` — 页面导航参考

## 行为摘要（旧仓）

1. `execute_tt`（千物宝箱，enable 默认 False）：`goto_page(page_travel)` → 点 `I_TT_ENTER` 进宝箱页 →
   可选 `earn_money`（借用式神→配置→确认→领奖）；买蓝票/黑碎/体力（先 OCR 余额，再点商品→加号→确认→领奖）。
2. `execute_shrine`（神龛，默认 False）：`goto_page(page_summon)` → 点 `C_C_SHRINE` 区域进神龛 →
   黑蛋/五星白蛋/四星白蛋：OCR 金币门槛 → 点商品→购买→确认→等「获得奖励」。
3. `execute_guild`（功勋商店，默认 False）：`goto_page(page_guild)` → 点 `I_GUILD_SHRINE`/`I_GUILD_STORE`
   进商店 → 蓝票/黑碎/皮肤券：OCR 余额、剩余数量 → `Buy.buy_more/buy_one`。
4. `execute_mall`（**总是执行**）：`goto_page(page_mall)` → 寄售屋/密卷屋/契灵 → 杂货铺
   （特殊/荣誉/友情点/勋章/魅力）→ `back_mall()` 回商城页。
5. 结束（旧仓停在商城页，不回庭院）。

## 流程（flow.json）

| # | op | 说明 | 守卫 |
|---|----|------|------|
| 1 | if `param('tt_enable')` | 庭院→旅行→千物宝箱：赚金币（可选）+ 买蓝票/黑碎/体力 + 回庭院 | `I_CHECK_TRAVEL`/`I_TT_CHECK` |
| 2 | if `param('shrine_enable')` | 庭院→召唤→神龛：黑蛋/五星/四星白蛋 + 回庭院 | `I_CHECK_SUMMON`/`I_S_NEXT_PERIOD` |
| 3 | if `param('guild_enable')` | 庭院→寮→功勋商店：蓝票/黑碎/皮肤券 + 回庭院 | `I_CHECK_GUILD`/`I_GUILD_CLOSE_RED` |
| 4 | 商城（总是） | 庭院→商城（推荐页→商城页） | `I_CHECK_MALL` |
| 4a | 各商店 if 参数 | 寄售屋 / 密卷屋 / 契灵 / 特殊 / 荣誉 / 友情点 / 勋章 / 魅力：进入→购买→回商城 | 各自 CHECK 模板 |
| 5 | finish done | 旧仓结束时停留商城页 | |

- 购买统一走 Component/Buy 的三种模式（代码级复用 `buy_more_steps` / `buy_one_steps` / `buy_one_or_more_steps`）：
  点商品卡 → 数量窗口(出现 `I_BUY_PLUS`)拉满确认 / 单买窗口(出现 check 图)确认 → 等「获得奖励」；
  出现 `I_UI_CONFIRM_SAMLL` = 达购买上限；出现 `I_BUY_RMB` = 人民币窗口 → 取消。
- 所有金额门槛用 `ocr_digit`（OCR 资产：O_TT_TOTOL / O_S_TOTAL / O_GUILD_TOTAL / O_*_MALL_RESOURCE_* / O_SCA_RES_* / O_BL_CHECK_MONEY）。

## 参数

见 `meta.json`：`tt_*` / `shrine_*` / `guild_*` / `consignment_enable` / `scales_*` / `bondlings_*` /
`special_*` / `honor_*` / `friendship_*` / `medal_*` / `charisma_*`，与旧仓 6 大块配置一一对应（默认全 False）。
数量类旧配置（如 `skin_ticket: int`、`red_daruma: int`）改为 bool 开关 + 「拉满」语义，记录为差异。

## 素材

| 全局 id | 来源 | 用途 |
|---------|------|------|
| legacy_RichMan_I_TT_* / O_TT_* | 任务 | 千物宝箱导航/购买/领奖/余额 |
| legacy_RichMan_I_S_* / O_S_* / C_C_SHRINE | 任务 | 神龛 |
| legacy_RichMan_I_GUILD_* / O_GUILD_* | 任务 | 功勋商店 |
| legacy_RichMan_I_MALL_* / I_SIDE_* / C_* | 任务 | 商城导航/寄售屋/密卷屋/契灵 |
| legacy_RichMan_I_SP_* / I_HONOR_* / I_FS_* / I_ME_* / I_CH_* | 任务 | 杂货铺 5 个子商店商品 |
| legacy_RichMan_O_MALL_RESOURCE_* / O_LEGACY_MALL_RESOURCE_* / O_SCA_* / O_BL_* / O_CON_NUMBER / O_HONOR_* / O_SP_RES_NUMBER | 任务 OCR | 余额/剩余数量 |
| legacy_Component_Buy_* | Component/Buy | 通用购买弹窗（plus/add/one/more/cancel/success/RMB） |
| legacy_GameUi_I_MAIN_GOTO_{MALL,TRAVEL,SUMMON,GUILD} / I_CHECK_{MALL,MALL_RECOMMEND,TRAVEL,SUMMON,GUILD,MAIN} | GameUi | 导航 |
| legacy_GlobalGame_I_UI_{REWARD,BACK_YELLOW,BACK_RED,CONFIRM,CONFIRM_SAMLL} / C_UI_REWARD | GlobalGame | 领奖/返回/确认 |

## 出口

`done`(success) / `failed`(retryable)。旧仓没有资源类出口。

## 与旧仓的差异 / 已知缺口

1. **数量配置改为「拉满」**：旧仓按 `remain_number`/余额计算购买个数（多段 `ocr` + 循环）；本 flow 的
   `buy_more` 统一「点两次加号=拉满」，余额门槛用 `ocr_digit` 判断。差异：可能多买/少买，但不会超上限
   （达到上限会弹 `I_UI_CONFIRM_SAMLL` 并收口）。数量型参数（`skin_ticket`/`red_daruma`/`challenge_pass`/
   `scales.demon_souls` 等）改为 bool 开关。
2. **密卷屋/契灵/寄售屋的复杂逻辑简化**：旧仓 `_scales_*` 有「剩余次数/余额/单价/多段 20 连买」，
   `_bondlings_*` 有 `DigitCounter` 余额解析；本 flow 用 `ocr_digit` 首数字 + 拉满购买近似。
3. **旧仓 `check_remain` 动态 ROI**（`O_GUILD_REMAIN.roi[0] = 图片 roi_front[0]-38 …`）依赖运行时改 ROI，
   DSL 无此能力 → 功勋商店剩余数量判断未迁移（改由购买上限弹窗兜底）。
4. **旧仓 `_go_*` 各种滑动找商品**（`S_GUILD_STORE`/`S_SP_DOWN`/`S_BUY_*`）在 flow 里用固定 swipe/模板循环近似；
   RuleSwipe 已导入为 `_FROM/_TO` 区域 id，但 flow 用推导坐标（见下）。
5. `execute_mall` 的 `back_mall()` 用「回到 `I_CHECK_MALL`」循环近似；旧仓是导航图 goto_page。
6. 旧仓结束时停在商城页；flow 保持同样的结束位置（不回庭院）。
7. 未真机验证。

## Lint

```powershell
cd D:\OAS-dev\oas-core
.\target\release\oasd.exe task lint --tasks-dir tasks --task RichMan
```
