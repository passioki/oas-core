# GuildBanquet（阴阳寮宴会等待）迁移规格

- 远程源：`runhey/OnmyojiAutoScript@dev` · commit `8b4dc669b47838f42d283f14aa394844c1963c86`（只读 checkout `D:\OAS-dev\_verify\remote-oas`）
- 已读文件：
  - `tasks/GuildBanquet/script_task.py`（199 行）
  - `tasks/GuildBanquet/config.py`（44 行）
  - `tasks/GuildBanquet/assets.py`（`I_FLAG`）
  - 依赖：`tasks/GameUi/page.py` 的 `page_guild`/`page_main` 页面图（`tasks/GameUi/default_pages.py`）
- 本地目标：`tasks/GuildBanquet/{meta.json,flow.json,README.md,migration.json}`

## 1. 上游行为矩阵

| # | 触发/条件 | 行为 | 备注 |
|---|---|---|---|
| 1 | `run_time.day_1/day_2 + run_time_1/run_time_2` 周配置 | 调度器决定何时运行 | 纯调度职责（下次运行时间由 `set_next_run`/`custom_next_run` 写回） |
| 2 | 启动 | `goto_page(page_guild)`：庭院 →（`I_MAIN_GOTO_GUILD`）→ 阴阳寮页 | GameUi 导航图 |
| 3 | 阴阳寮页 `appear(I_FLAG)` | 宴会进行中：进入等待循环 | `I_FLAG` 为右上角宴会旗标 |
| 4 | 等待循环 | 每 ~10s 检测一次 `I_FLAG`（10s 内复用上次结果）；旗标消失 ⇒ 宴会结束；每 230s 记一次超时，累计 3 次（≈11.5min）后按超时结束（注释：宴会最长 15min） | 轮询 + 超时上限 |
| 5 | 等待结束 | `set_config()`：把本次结束时间 -14:55 写回当天配置；`goto_page(page_main)`；`plan_next_run()` 安排下一次（day_1/day_2 周几） | 全部为调度配置写回 |
| 6 | 启动时 `I_FLAG` 未出现 | 若当前 <22:00：`set_next_run` 5 分钟后重试（可能宴会尚未开始）；然后回庭院、结束；若 ≥22:00：`set_next_run(success=False)` 判失败 | 两类时间相关出口 |
| 7 | 异常 | `set_config` 内异常 ⇒ 直接 `TaskEnd` | 静默结束 |

上游无战斗、无资源消耗、无 OCR。

## 2. oas-core DSL 映射

| 上游 | 本迁移 | 说明 |
|---|---|---|
| `goto_page(page_guild)` | `click(legacy_GameUi_I_MAIN_GOTO_GUILD)` + `requires appear(庭院_探索灯笼)` + `expect appear(legacy_GameUi_I_CHECK_GUILD)` | 复用全局素材（GameUi 导航资产）与已验证庭院素材 |
| `appear(I_FLAG)` 等待循环 | `loop n=param('max_wait_min')*12` { `wait 5`; `if not appear(legacy_GuildBanquet_I_FLAG) break` } | 上限代替上游 3×230s；5s 粒度近似 10s 检测 |
| 旗标消失/超时后结束 | `click(legacy_GlobalGame_I_UI_BACK_YELLOW)`（回庭院）→ `finish(done)` | |
| 启动时旗标不存在 | log + 回庭院 → `finish(window_closed)`（result=retryable） | 由调度器决定下次重试（上游是 +5min 重排） |
| `set_config`/`plan_next_run`/`set_next_run` | **不迁移**（调度器职责） | `schedule_hint.quota.per=week, times=2` 仅供 UI 默认 |

## 3. 覆盖判定

- ✅ 覆盖：阴阳寮导航、宴会进行中等待至旗标消失、超时上限、宴会未开始的分支出口、回庭院、退出码。
- 🔄/不迁移（有意）：`set_config` 写回宴会开始时间、跨周 `custom_next_run` 排期、<22:00 的 5 分钟重试——这些是**调度器/玩家配置**职责，按 ENGINE §22.34 三层分离不进 flow；`schedule_hint` 只给默认值。
- 差异：
  1. 等待轮询粒度 10s（上游，含缓存复用）→ 5s 简单轮询；
  2. 超时上限：上游 3×230s 超时后「按结束处理」；本 flow 为 `max_wait_min`（默认 15，上限 30）分钟硬上限，到上限后同样按结束处理（done），不区分 timeout 日志；
  3. 上游 `I_FLAG` 的 `appear` 检测 10s 内复用旧结果属性能优化，未复刻。
- 证据：仅静态（`oasd task lint` 0 errors + 本行为矩阵）。**无实机/回放证据**——宴会只在每周固定时段开，当前时段无法验证。

## 4. 素材（全部为全局 `assets/assets.json` 引用，不新增任务素材）

| id | 用途 |
|---|---|
| `庭院_探索灯笼` | 庭院起点守卫（现有真机标定素材） |
| `legacy_GameUi_I_MAIN_GOTO_GUILD` | 庭院 → 阴阳寮 |
| `legacy_GameUi_I_CHECK_GUILD` | 阴阳寮页守卫 |
| `legacy_GuildBanquet_I_FLAG` | 宴会进行中旗标（导入自 `tasks/GuildBanquet/assets.py`） |
| `legacy_GlobalGame_I_UI_BACK_YELLOW` | 返回庭院 |

## 5. 待办 / 未覆盖

1. 宴会开始时间写回（`set_config`）与跨周排期：需调度器/配置层支持（offline 无等价 flow op）。
2. 实机标定：`legacy_GuildBanquet_I_FLAG` 的 ROI 来自旧仓采集，当前客户端 1280×720 尚未用真机帧复核阈值（0.8）。
3. <22:00 的 5 分钟快速重试语义：本 flow 用 `window_closed(regretable)` 出口交给调度器冷却策略实现。
