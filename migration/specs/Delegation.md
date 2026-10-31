# Delegation（式神委派）迁移规格

- 源仓库：`runhey/OnmyojiAutoScript@dev` · commit `8b4dc669b47838f42d283f14aa394844c1963c86` · 只读副本 `D:/OAS-dev/_verify/remote-oas`
- 目标：`tasks/Delegation/`（统一任务模板：meta.json + flow.json + assets.json + images/，素材引用全局 `legacy_*`）
- 迁移结论：**🔄** —— 见 §3 行为矩阵与 §5 缺口

## 1. 读取的源文件

- `tasks/Delegation/script_task.py`
- `tasks/Delegation/config.py`
- `tasks/Delegation/assets.py`
- `tasks/GameUi/game_ui.py, tasks/GameUi/page.py (imported)`
- `tasks/GlobalGame/assets.py (I_UI_CANCEL 等)`

## 2. 入口 / 出口

- 入口：`meta.entry.requires = appear('庭院_探索灯笼')`；flow 首段用 `ensure_main()` 从任意页面回庭院。
- 出口：
  - `done` → result `success`
  - `failed` → result `retryable`

## 3. 行为矩阵

| # | 行为 | 远程实现（源码位置） | 新 DSL 映射 | 状态 |
|---|---|---|---|---|
| 1 | 进入委派页 | script_task.py:20 goto_page(page_delegation) | ensure_main→goto_world→I_EXPLORATION_GOTO_DELEGATION→expect I_CHECK_DELEGATION | 覆盖 |
| 2 | 领取委派奖励 | script_task.py:121-164 check_reward（I_REWARDS_GET/CHAT/DONE/FALSE, I_CHAT_1/2, O_D_DONE） | loop 10：6 类模板弹窗点击；O_D_DONE 下方点击未覆盖 | 部分 |
| 3 | 按名字选委派单 | delegate_one:54-58 设置 O_D_NAME.keyword=name + ocr_appear | if text_visible(name) → click_target(ocr_text, sel=O_D_NAME, text=完整单名) | 部分（精确匹配近似） |
| 4 | 已在委派中跳过 | delegate_one:74-90 I_D_BACK 分支 | if appear(I_D_BACK) → loop 点 I_D_BACK 退出 + log 跳过 | 部分（无 I_REWARDS_MIN 等待） |
| 5 | 进入并填 4 素材槽 | delegate_one:100-103 ui_click(C_D_1..4, I_D_SELECT_1..4) | 4 × loop: 点 region C_D_i 直到 I_D_SELECT_i 出现 | 覆盖 |
| 6 | 开始委派 | delegate_one:104-119 循环点 C_D_5 / I_D_START 直到 I_D_START 消失 | loop 8：点 region C_D_5 + click I_D_START | 覆盖 |
| 7 | 回庭院 | run 结束 set_next_run | ensure_main | 覆盖 |

## 4. 参数映射

| key | type | default | 说明 |
|---|---|---|---|
| `miyoshino_painting` | bool | `False` | 来自远程同名配置 |
| `bird_feather` | bool | `False` | 来自远程同名配置 |
| `find_earring` | bool | `False` | 来自远程同名配置 |
| `cat_boss` | bool | `False` | 来自远程同名配置 |
| `miyoshino` | bool | `False` | 来自远程同名配置 |
| `strange_trace` | bool | `False` | 来自远程同名配置 |

## 5. 缺口 / TODO

- check_reward 中 O_D_DONE「完成」缎带的「点其正下方同样大小的可领取区」（script_task.py:150-164）无对应原语（inline OCR 点击只会点识别行中心）→ 目前只覆盖模板类 I_REWARDS_* 领奖，OCR 完成卡片可能漏领。
- 远程 delegate_one 用 keyword 子串匹配（'画'/'鸟羽'/'接送'/'痕迹'），新引擎 OCR 行匹配是规范化精确相等（容 1 字编辑距离）→ 本 flow 用完整单名（弥助的画/鸟之羽/接送弥助/奇怪的痕迹），若卡片行还带其它文案会不匹配并跳过。
- 远程「已在委派中」分支会等 I_REWARDS_MIN 后返回 False；本 flow 只回退并跳过（无 I_REWARDS_MIN 等待语义）。
- 开始委派后不校验最终是否成功（远程同样只点开始循环），靠下一步页面状态体现。

## 6. 通用模式与素材语义（本批任务共用）

- **P-NAV**：`legacy_GameUi_I_CHECK_MAIN` 守卫 + `click` 页面跳转素材（模板点击 = 匹配矩形）+ `expect` 目标页 marker。
- **P-BATTLE**：`战斗_准备按钮`→点击→`战斗_自动按钮`→每 2.5s 点 (640,690) 直到任务页 marker 回来。
- **P-RETURN**：`legacy_GlobalGame_I_UI_BACK_YELLOW` / `legacy_GameUi_I_BACK_YOLLOW` / 坐标兜底 → `I_CHECK_MAIN`。
- 素材契约（导入器 v2）：模板 `roi` = remote `roiBack`，**无 `action_roi`** ⇒ `click` 落在匹配到的模板矩形（= 旧引擎 `RuleImage.coord()` 行为）；`RuleClick` → `kind=region` 精确点击区；`RuleOcr` → `ocr_lines`（`ocr()`/`ocr_digit()`/`text_visible()`/`click_target ocr_text`）。

## 7. 证据

- oasd task lint 绿；覆盖：导航/领奖循环/6 种委派的选单(OCR 精确行)/4 素材槽/开始/已在委派跳过/回庭院；完成缎带下方点击未覆盖。未真机/回放验证。
- 未真机/未回放：迁移会话无可用设备与对应场景回放帧；素材 T4 引用全部解析到全局 `assets/assets.json`。
