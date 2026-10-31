# Nian（年兽）迁移规格

- 源仓库：`runhey/OnmyojiAutoScript@dev` · commit `8b4dc669b47838f42d283f14aa394844c1963c86` · 只读副本 `D:/OAS-dev/_verify/remote-oas`
- 目标：`tasks/Nian/`（统一任务模板：meta.json + flow.json + assets.json + images/，素材引用全局 `legacy_*`）
- 迁移结论：**🔄** —— 见 §3 行为矩阵与 §5 缺口

## 1. 读取的源文件

- `tasks/Nian/script_task.py`
- `tasks/Nian/config.py`
- `tasks/Nian/assets.py`
- `tasks/Component/GeneralRoom/general_room.py (imported)`
- `tasks/Component/GeneralInvite/general_invite.py (imported)`
- `tasks/Component/GeneralBattle/general_battle.py (imported)`
- `tasks/GameUi/game_ui.py, tasks/GameUi/page.py (imported)`

## 2. 入口 / 出口

- 入口：`meta.entry.requires = appear('庭院_探索灯笼')`；flow 首段用 `ensure_main()` 从任意页面回庭院。
- 出口：
  - `done` → result `success`
  - `no_target` → result `blocked`
  - `failed` → result `retryable`

## 3. 行为矩阵

| # | 行为 | 远程实现（源码位置） | 新 DSL 映射 | 状态 |
|---|---|---|---|---|
| 1 | 进入组队+选年兽区域 | script_task.py:34-52 goto_page(page_team); check_zones('年兽') | click I_MAIN_GOTO_TEAM→expect I_CHECK_TEAM；loop: text_visible('年兽')→inline OCR 点击，否则上滑 | 部分（列表滚动为有界近似） |
| 2 | 冷却检测 | script_task.py:123-144 I_N_CHECK + O_N_BATTLE_AGAIN(keyword 再次挑战) + O_N_CD 正则 | if text_visible('再次挑战') → finish(blocked)；O_N_CD 时间解析未映射 | 部分 |
| 3 | 自动匹配 | script_task.py:45-55 循环点 I_GR_AUTO_MATCH，cd_count>=4 退出 | loop: 点 legacy_Nian_I_AUTO_MATCHING；var('match_clicks')>=4 → blocked | 覆盖 |
| 4 | 等待战斗 | script_task.py:62-118 check_take_over_battle/is_in_room/wait_battle | wait_until(准备页, 180s) | 部分（无房间分支） |
| 5 | 战斗 | script_task.py:71-87 run_general_battle | P-BATTLE | 部分（通用骨架） |
| 6 | 匹配超时取消 | script_task.py:101-113 480s 超时 → 取消匹配 | — | 缺口：无时钟 + GeneralInvite 素材 |

## 4. 参数映射

- 无参数（远程该任务无可调配置项）。

## 5. 缺口 / TODO

- 冷却剩余时间解析（远程 re.search(r'(\d+)时(\d+)分后可') + timedelta）无字符串解析原语 → 只能以 blocked 退出让调度器稍后重试，不设置精确 next_run。
- GeneralRoom.check_zones（L_TEAM_LIST 滚动列表 OCR + O_GR_ZONES_NAME）未迁移：用 inline OCR [26,106,360,549] 点击可见的「年兽」行 + 上滑 8 次替代；若年兽行多次滚动后仍不可见会失败。
- GeneralInvite（房间/邀人）未迁移：只处理「自动匹配」路径；匹配 240s 安全点击、480s 取消、被秒开接管战斗未覆盖。
- 普通战斗用 P-BATTLE 通用骨架，不含绿标/预设阵容。

## 6. 通用模式与素材语义（本批任务共用）

- **P-NAV**：`legacy_GameUi_I_CHECK_MAIN` 守卫 + `click` 页面跳转素材（模板点击 = 匹配矩形）+ `expect` 目标页 marker。
- **P-BATTLE**：`战斗_准备按钮`→点击→`战斗_自动按钮`→每 2.5s 点 (640,690) 直到任务页 marker 回来。
- **P-RETURN**：`legacy_GlobalGame_I_UI_BACK_YELLOW` / `legacy_GameUi_I_BACK_YOLLOW` / 坐标兜底 → `I_CHECK_MAIN`。
- 素材契约（导入器 v2）：模板 `roi` = remote `roiBack`，**无 `action_roi`** ⇒ `click` 落在匹配到的模板矩形（= 旧引擎 `RuleImage.coord()` 行为）；`RuleClick` → `kind=region` 精确点击区；`RuleOcr` → `ocr_lines`（`ocr()`/`ocr_digit()`/`text_visible()`/`click_target ocr_text`）。

## 7. 证据

- oasd task lint 绿；覆盖：组队入口→年兽区域（inline OCR 列表点击）→冷却文本检测→自动匹配→准备/通用战斗→回庭院；cooldown 剩余时间解析/房间邀请/匹配超时取消未覆盖。
- 未真机/未回放：迁移会话无可用设备与对应场景回放帧；素材 T4 引用全部解析到全局 `assets/assets.json`。
