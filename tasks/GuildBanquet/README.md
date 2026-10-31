# GuildBanquet（阴阳寮宴会等待）

远程源：`runhey/OnmyojiAutoScript@dev` `tasks/GuildBanquet`（commit `8b4dc66`）。

## 行为（抽象）

1. 庭院 → 阴阳寮（`legacy_GameUi_I_MAIN_GOTO_GUILD`，守卫 `庭院_探索灯笼` / `legacy_GameUi_I_CHECK_GUILD`）。
2. 阴阳寮页检测宴会旗标 `legacy_GuildBanquet_I_FLAG`：
   - 可见 ⇒ 宴会进行中：每 5s 轮询，旗标消失即结束；上限 `param('max_wait_min')`（默认 15 分钟）。
   - 不可见 ⇒ 宴会未开始/已结束：记 warn，回庭院，`finish(window_closed)`（result=retryable，交给调度器冷却重试；上游是 +5min `set_next_run`）。
3. 结束（或等待超时）后回庭院，`finish(done)`。

上游的「把结束时间 -14:55 写回宴会配置」「按 day_1/day_2 跨周排期」属于**调度/玩家配置**职责，按 ENGINE §22.34 三层分离不在 flow 内实现；`meta.schedule_hint` 只给 UI 默认（每周 2 次）。

## 目录

```
tasks/GuildBanquet/
├─ meta.json       # exits: done/window_closed/failed；params: max_wait_min
├─ flow.json       # 上述流程（DSL 原生，无任务内素材）
├─ migration.json  # 源文件/差异/证据
└─ README.md
```

引用素材全部来自**全局** `assets/assets.json`（本任务不新增素材、无 images/）。

## 运行 / 校验

```powershell
cd D:\OAS-dev\oas-core
.\target\release\oasd.exe task lint --tasks-dir tasks --task GuildBanquet   # => lint task=GuildBanquet errors=0
# 实机（需先到庭院）：
.\target\release\oasd.exe run --task tasks\GuildBanquet --param max_wait_min=15
```

## 已知限制

1. **无实机/回放证据**：宴会只在每周固定时段开放，迁移时无法落到宴会时段验证。
2. 等待轮询 5s（上游 10s 且缓存结果复用）；超时上限用 `max_wait_min` 代替上游 3×230s 后按结束处理。
3. 宴会开始时间写回与跨周排期未迁移（调度器职责，见 `migration.json.differences`）。
4. `legacy_GuildBanquet_I_FLAG` 阈值 0.8 来自旧仓采集，当前客户端尚未真机复核。
