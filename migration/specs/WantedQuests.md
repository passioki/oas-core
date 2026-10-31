# WantedQuests（悬赏封印）迁移规格

- 源仓库：`runhey/OnmyojiAutoScript@dev` · commit `8b4dc669b47838f42d283f14aa394844c1963c86` · 只读副本 `D:/OAS-dev/_verify/remote-oas`
- 目标：`tasks/WantedQuests/`（统一任务模板 + 全局 `legacy_*` 素材）
- 迁移结论：**🔄** —— 行为矩阵见 §3，缺口见 §5

## 1. 读取的源文件

- `tasks/WantedQuests/script_task.py`
- `tasks/WantedQuests/config.py`
- `tasks/WantedQuests/assets.py`
- `tasks/WantedQuests/explore.py`
- `tasks/Secret/script_task.py (imported secret zone)`
- `tasks/GameUi/game_ui.py, tasks/GameUi/page.py (imported)`

## 2. 入口 / 出口

- 入口：`appear('庭院_探索灯笼')`；flow 首段 `ensure_main()` 回庭院。
- 出口：
  - `done` → result `success`
  - `no_target` → result `blocked`
  - `failed` → result `retryable`

## 3. 行为矩阵

| # | 行为 | 远程实现（源码位置） | 新 DSL 映射 | 状态 |
|---|---|---|---|---|
| 1 | 打开悬赏封印界面 | script_task.py:122-160 pre_work(): 点 I_WQ_SEAL/I_WQ_DONE 直到 I_TRACE_DISABLE | loop: 点 I_WQ_SEAL/I_WQ_DONE 直到 I_TRACE_ENABLE/DISABLE 出现 | 部分 |
| 2 | 领取宝箱 | script_task.py:58-73 I_WQ_BOX / I_TREASURE_BOX_CLICK → ui_get_reward | loop: 点 I_WQ_BOX | 部分（I_TREASURE_BOX_CLICK 未覆盖） |
| 3 | 追踪/协作邀请 | script_task.py:151-230 all_cooperation_invite / invite_five / trace_one | — | 缺口 |
| 4 | 寻找并执行悬赏 | script_task.py:59-96 find_wq + execute_mission(4 类) | — | 缺口 |
| 5 | 探索战斗 | tasks/WantedQuests/explore.py WQExplore | — | 缺口 |

## 4. 参数映射

- 无参数。

## 5. 缺口 / TODO

- 核心逻辑量大且强依赖 OCR 文本处理：find_wq 扫描列表、O_WQ_TYPE/INFO 判定 4 类任务、O_WQ_TEXT_ALL 文本、C_WQ_D* 战斗、协作邀请（GeneralInvite 全套）、追踪开关、探索战斗（WQExplore）——本轮未逐函数映射。
- 远程 pre_work 依赖 I_TRACE_DISABLE/ENABLE 全量追踪判定 + 多种特判（特殊主界面 C_SPECIAL_MAIN、现世任务 I_WQ_TRACE_ONE_REALWORLD）。
- before_end 时间窗口/next_run 排期无时钟原语 → scheduler windows。
- 此状态为「已映射入口、主循环未实现」的 🔄，不应视为可用任务。

## 6. 素材契约与通用模式

- 导入器 v3：模板 `roi` = remote `roiBack`，无 `action_roi` ⇒ `click` 落在匹配到的模板矩形（= 旧引擎行为）；`RuleClick` → `kind=region`；`RuleOcr` → `ocr_lines`；`RuleSwipe` → `_FROM`/`_TO` 两个 region；Component 素材 `legacy_Component_<Module>_<CONST>`。
- P-NAV / P-BATTLE v2 / P-RETURN / OCR 语义同批一 README。

## 7. 证据

- oasd task lint 绿；覆盖：打开封印界面/领取宝箱/回庭院；核心任务执行（pre_work 追踪/协作邀请/执行 4 类悬赏/探索战斗）未迁移，flow 显式 blocked。
- 未真机/未回放：无设备、无对应场景回放帧；T4 素材引用全部解析到全局 `assets/assets.json`。
