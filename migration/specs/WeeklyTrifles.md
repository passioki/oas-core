# WeeklyTrifles（每周琐事）迁移规格

- 源仓库：`runhey/OnmyojiAutoScript@dev` · commit `8b4dc669b47838f42d283f14aa394844c1963c86` · 只读副本 `D:/OAS-dev/_verify/remote-oas`
- 目标：`tasks/WeeklyTrifles/`（统一任务模板 + 全局 `legacy_*` 素材）
- 迁移结论：**🔄** —— 行为矩阵见 §3，缺口见 §5

## 1. 读取的源文件

- `tasks/WeeklyTrifles/script_task.py`
- `tasks/WeeklyTrifles/config.py`
- `tasks/WeeklyTrifles/assets.py`
- `tasks/GameUi/game_ui.py, tasks/GameUi/page.py (imported)`
- `tasks/GlobalGame/assets.py (I_UI_BACK_RED/BLUE/YELLOW)`

## 2. 入口 / 出口

- 入口：`appear('庭院_探索灯笼')`；flow 首段 `ensure_main()` 回庭院。
- 出口：
  - `done` → result `success`
  - `failed` → result `retryable`

## 3. 行为矩阵

| # | 行为 | 远程实现（源码位置） | 新 DSL 映射 | 状态 |
|---|---|---|---|---|
| 1 | 图鉴分享 | script_task.py:125-173 _share_collect | main→COLLECTION→loop I_WT_SHIKIAGMI/I_WT_SHARE→share(I_WT_COLLECT_WECHAT)→back | 覆盖（近似） |
| 2 | 地鬼分享 | script_task.py:175-225 _share_area_boss | world→AREA_BOSS→loop C_WT_AB_CLICK/I_WT_DAY_BATTLE/I_WT_SHARE_AB→share→back | 覆盖（近似） |
| 3 | 秘闻分享 | script_task.py:227-267 _share_secret | — | 缺口（TODO 参数占位） |
| 4 | 摸鱼存储 | script_task.py:73-123 _save_touch_fish | main→GUILD→I_GUILD_GOTO_TF→I_WT_LAST_SAVE→ticket>=cost→SAVE_ALL/TF_CONFIRM | 部分 |
| 5 | 破碎符咒 | script_task.py:269-354 _broken_amulet | main→SUMMON→O_BA_AMOUNT→loop BM_ENTER/BM_AGAIN→随机点→BM_CONFIRM | 部分（固定 10 次/轮） |
| 6 | 初始分享奖励点 | script_task.py:43-71 click_share(I_WT_QR_CODE/C_WT_WECHAT) | weekly_share(): loop 点 I_WT_QR_CODE 直到 I_REWARD 出现 | 覆盖（近似） |

## 4. 参数映射

| key | type | default | 说明 |
|---|---|---|---|
| `share_collect` | bool | `True` | 远程配置/迁移决策 |
| `share_area_boss` | bool | `True` | 远程配置/迁移决策 |
| `share_secret` | bool | `True` | 远程配置/迁移决策 |
| `save_touch_fish` | bool | `False` | 远程配置/迁移决策 |
| `broken_amulet` | int | `100` | 远程配置/迁移决策 |

## 5. 缺口 / TODO

- 秘闻分享（_share_secret, script_task.py:227-267）未迁移：需要 I_WT_ENTER_SE/I_WT_SE_SHARE/I_WT_SE_RANK/I_WT_SE_JADE 多级判定 + I_UI_BACK_BLUE 返回链；本轮只保留参数与 TODO。
- 分享流程远程有 Timer(3/7s) 判停与 ui_reward_appear_click；新 flow 用固定轮数 bounded loop 近似。
- 地鬼分享「O? 本周已领取」判定远程用 I_WT_AB_JADE 出现与否（本 flow 部分覆盖），且 C_WT_AB_CLICK 为 RuleClick 区域。
- 摸鱼存储：远程 O_WT_LUCKY_TICKETS 是 DigitCounter 返回 (cu,res,total) 且 O_WT_SAVE_COST 为花费；新 flow 用 ocr_digit 比较两个数值，OCR 失败(Null)时比较为 false → 走「不足」分支。
- 破碎符咒：远程按 10/50 次按钮位置切换数量(I_BMT_CHECK + O_BA_TIMES 的 '10次'/'50次' OCR 定位)，新 flow 固定按 10 次累计；O_BA_AMOUNT OCR 失败保护未覆盖。
- 分享到微信的 C_WT_WECHAT / QR 码流程为无人值守 best-effort（远程同样等待超时后放弃）。

## 6. 素材契约与通用模式

- 导入器 v3：模板 `roi` = remote `roiBack`，无 `action_roi` ⇒ `click` 落在匹配到的模板矩形（= 旧引擎行为）；`RuleClick` → `kind=region`；`RuleOcr` → `ocr_lines`；`RuleSwipe` → `_FROM`/`_TO` 两个 region；Component 素材 `legacy_Component_<Module>_<CONST>`。
- P-NAV / P-BATTLE v2 / P-RETURN / OCR 语义同批一 README。

## 7. 证据

- oasd task lint 绿；覆盖：图鉴分享、地鬼分享、摸鱼存储、破碎符咒召唤四条分支（有界循环 + 素材点击/OCR 计数）；秘闻分享显式 TODO；未真机/回放验证。
- 未真机/未回放：无设备、无对应场景回放帧；T4 素材引用全部解析到全局 `assets/assets.json`。
