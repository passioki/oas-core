# Hunt（狩猎战（麒麟/阴界之门））

远程源：`runhey/OnmyojiAutoScript@dev` `tasks/Hunt`（commit `8b4dc66`）。

**状态：✅** — 两条分支（麒麟 Mon-Thu / 阴界之门 Fri-Sun）均覆盖；时间窗口/御魂切换属调度器/配置职责

## 行为（抽象）

1. `check_datetime()` 判定麒麟/阴界；不在时段则 `custom_next_run` 到当日/次日 19:00
2. 若配置了切换御魂组（`kirin_group_team`/`netherworld_group_team` ≠ -1,-1）→ 式神录切御魂
3. `goto_page(page_hunt_kirin)` → `kirin()`
4. 循环：`I_PREPARE_HIGHLIGHT` 出现 ⇒ 开战；点 `I_UI_CONFIRM` / `I_KIRIN_CHALLAGE` / `I_KIRIN_START_CHALLAGE`；`I_KIRIN_END` ⇒ 今日已挑战，点返回退出
5. 通用战斗 `run_general_battle`（准备→自动战斗→胜负/奖励结算→回御魂页）
6. `goto_page(page_hunt)` → `netherworld()`
7. 循环：在房间则等 `I_FIRE` 并点（开战）；否则点 `I_NW` / `I_UI_CONFIRM` / `I_NW_CHALLAGE`；`I_NW_DONE` ⇒ 今日已挑战，点 `I_UI_BACK_RED` 退出
8. 重写 `battle_wait`：`I_WIN` 处理胜利；`I_FALSE` 失败；战斗中若出现 `I_PREPARE_HIGHLIGHT` 继续点准备（失败后可续战）
9. `plan_tomorrow_hunt()` 安排次日 19:00（麒麟/阴界按明日星期）；`TaskEnd`

## 目录 / 契约

```
tasks/Hunt/
├─ meta.json       # params/exits/capabilities/schedule_hint
├─ flow.json       # oas-flow DSL（仅既有 op）
├─ migration.json  # 源文件、差异、证据
└─ README.md
```

参数：`max_battles=1`（1~5）

## 运行 / 校验

```powershell
cd D:\OAS-dev\oas-core
.\target\release\oasd.exe task lint --tasks-dir tasks --task Hunt   # => errors=0
.\target\release\oasd.exe run --task tasks\Hunt --param max_battles=1
```

## 已知限制

1. 无实机/回放证据（狩猎战时段限制）
2. 时间窗/次日排期/御魂切换属调度器与玩家配置职责，未在 flow 内实现
3. 阴界之门房间时序（匹配/自动匹配）未实机复核

详见 `migration/specs/Hunt.md`、`tasks/Hunt/migration.json`、`migration/status/Hunt.json`。
