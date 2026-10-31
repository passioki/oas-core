# DemonRetreat（首领退治）

远程源：`runhey/OnmyojiAutoScript@dev` `tasks/DemonRetreat`（commit `8b4dc66`）。

**状态：✅** — 周六活动主链路（进神社→退治→集结/迟到→三轮战斗→领奖→回庭院）覆盖；周六窗口由调度器负责

## 行为（抽象）

1. 按 `weekday()` 计算到周六的天数 → `custom_next_run` → `TaskEnd`（跳过）
2. 可选御魂切换（enable / enable_switch_by_name）
3. `goto_page(page_guild)`→循环：点 `I_SHRINE`（神社）/`I_HUNT`（首领退治）/`I_QUIT_BACK`；`I_HUNT_CHECK` 或 `I_DEMON_GATHER` 或准备页 ⇒ 进入成功；点 `I_REWARD_ALL` ⇒ 已打完只领奖；`I_RANK_LSIT` 3s/20s 重试；超过 5 次 `I_HUNT` 未进入 ⇒ 失败
4. `demon_retreat()`：OCR `O_LATER_ENTER_CHECK` 与「集结」的交集为空 ⇒ 迟到，点 `I_ENTER_FIRE` 直接开战；否则等 `I_DEMON_GATHER` 消失后开战
5. `run_demon_battle`：点准备 `I_PREPARE_HIGHLIGHT`（轮间会重现）→ `battle_wait`：`I_WIN` 胜、`I_FALSE` 败、轮间准备续点、3min 超时延长
6. `I_PRAY` 祈福 / `I_HUNT` / `I_REWARD_ALL` 一键领取 → `ui_reward_appear_click` 点奖励；`I_RANK_LSIT` ⇒ 无奖励可领
7. 回庭院；`custom_next_run` 下周六 / 失败标记

## 目录 / 契约

```
tasks/DemonRetreat/
├─ meta.json       # params/exits/capabilities/schedule_hint
├─ flow.json       # oas-flow DSL（仅既有 op）
├─ migration.json  # 源文件、差异、证据
└─ README.md
```

参数：无

## 运行 / 校验

```powershell
cd D:\OAS-dev\oas-core
.\target\release\oasd.exe task lint --tasks-dir tasks --task DemonRetreat   # => errors=0
.\target\release\oasd.exe run --task tasks\DemonRetreat
```

## 已知限制

1. 无实机/回放证据（周六活动期限制）
2. 周六日期判断与下次排期属调度器职责，未在 flow 内实现
3. 御魂切换/绿标/预设配置未迁移

详见 `migration/specs/DemonRetreat.md`、`tasks/DemonRetreat/migration.json`、`migration/status/DemonRetreat.json`。
