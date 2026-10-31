# RyouToppa（阴阳寮结界突破 / 寮突）

> 移植自 `runhey/OnmyojiAutoScript@dev`（commit `8b4dc66`）的旧仓任务；行为抽象见 [migration/specs/RyouToppa.md](../../migration/specs/RyouToppa.md)。

## 目录

```
tasks/RyouToppa/
├─ meta.json     # 统一模板契约：entry/params/exits/capabilities/resources/schedule_hint
├─ flow.json     # oas-flow DSL（只用现有 op；引用全局 legacy_* 素材）
└─ README.md     # 本文件
```

素材全部来自全局库 `assets/assets.json`（`legacy_RyouToppa_*` / `legacy_GameUi_*` / `legacy_GlobalGame_*` / `legacy_RealmRaid_I_FIRE`），
本任务不声明 task-local assets（`oasd run` 在有 `assets/` 全局库时按全局 id 解析）。

## 运行

```powershell
cd D:\OAS-dev\oas-core
# 需要先连上模拟器（MuMu / adb），起始画面 = 庭院
.\target\release\oasd.exe task lint --tasks-dir tasks --task RyouToppa
.\target\release\oasd.exe run --task tasks\RyouToppa --seed 7 --param max_attacks=6
# 寮管理要自动开局时：--param open_ryou_raid=true
# 要锁定阵型：--param lock_team=true
```

`oasd task lint` 需要显式 `--tasks-dir tasks`（当前 CLI 契约；README 中同源任务一致）。

## 流程（flow.json）

| # | 行为 | 守卫 |
|---|---|---|
| 1 | 庭院→探索：`click(GameUi.I_MAIN_GOTO_EXPLORATION)` | requires 庭院 / expect 探索 |
| 2 | 探索→结界突破：`click(I_EXPLORATION_GOTO_REALM_RAID)` | requires 探索 / expect 个人突破页 |
| 3 | 个人突破页→寮突页签：`click(RyouToppa.I_RYOU_TOPPA)` | requires 个人突破页 / expect 寮突页 |
| 4 | 寮突状态判定（loop 20s）：100% / 未开(+可开局) / 无权限 / 已开启 | `I_SUCCESS_PENETRATION` / `I_SELECT_RYOU_BUTTON` / `I_NO_SELECT_RYOU` / `I_RYOU_REWARD(_90)` |
| 5 | 未开且 `open_ryou_raid`：选择寮→选第一个寮→开始突入 | 每步 requires/expect；无权限则记 `no_guild` |
| 6 | 阵型锁定开关对齐 `lock_team` | 只点存在的那个按钮 |
| 7 | 主循环 `while var('attacks') < param('max_attacks')`：等待列表页 `I_TOPPA_RECORD`；OCR 机会=0 → `no_resource`；8 个区域依次检查/进攻 | 区域标记用 `if appear`，进攻用 `RealmRaid.I_FIRE` + `C_AREA_n`(region) |
| 8 | 战斗样板：点「进攻」→ `战斗_准备按钮` → `wait_until 战斗_自动按钮` → 点 (640,690) 直到 `I_TOPPA_RECORD` 回来 | 每场 +1 attacks |
| 9 | 无任何可攻区域：上滑刷新 2 次，最多 3 轮 → `failed` | swipe |
| 10 | 回庭院（带 appear 容错）→ `finish(exit)` | 退出表 |

## 退出（meta.exits）

| exit | result | 场景 |
|---|---|---|
| done | success | 正常打完/寮突已 100% 已攻破 |
| no_resource | resource | 进攻机会 0 |
| no_target | blocked | 寮突未开启且无管理员权限 |
| failed | retryable | 3 轮刷新后无任何可攻区域 |

## 参数（meta.params）

| key | type | 默认 | 说明 |
|---|---|---|---|
| max_attacks | int | 6 | 单次 run 最多进攻次数（旧 `limit_count=50` 的收敛版） |
| lock_team | bool | false | 是否锁定阵型（旧 `lock_team_enable`） |
| open_ryou_raid | bool | false | 是否以寮管理员身份开局（旧 `ryou_access`） |

## 素材

见迁移规格 §9（66 个全局素材 = 62 legacy + 4 核心战斗素材）：导航守卫用 `legacy_GameUi_*` / `legacy_GlobalGame_*`；区域点击框用 `legacy_RyouToppa_C_AREA_1..8`（kind=region，随机点）；
区域状态标记 `legacy_RyouToppa_I_AREA_n_*`（32 个 template）；机会 OCR `legacy_RyouToppa_number`（ocr_lines, ROI 271,560,48,31）；
战斗样板复用引擎核心素材 `战斗_准备按钮`/`战斗_自动按钮`（P1 实机标定）。

## 前置条件 / 已知限制

1. **起始画面必须是庭院**（`meta.entry.requires` = `GameUi.I_CHECK_MAIN`）。
2. 需要 OCR 模型：机会判定依赖 `ocr_digit('legacy_RyouToppa_number')`；跑时加 `--model/--det-model`（否则 `ocr_digit` 响亮报错）。
3. **无票判定**只取 OCR 第一个数字（当前/恢复/总数 三段的「当前」段）；旧仓 `cu+res==total` 逻辑未迁移 → 可能提前一轮退出。
4. 夜间（21:00–05:00）无限进攻窗口未实现（引擎无时钟表达式）→ 夜间仍按机会 0 退出。
5. 旧仓「御魂/阵容切换」「30 分钟时间上限」「战斗胜利判定」未迁移：
   - 御魂切换：DSL 无原语（默认配置本来就是关闭）；
   - 时间上限：由 `--budget-wall`/调度窗口兜底；
   - 胜负：本任务不依赖战斗结果（只计数）。
6. 区域进攻点击失败 5 次后旧仓会换区域；新 flow 点 `I_FIRE` 失败即响亮报错（不猜）。
7. **无实机/replay 证据**：迁移时模拟器 offline，仅 lint + 行为矩阵证据；首跑重点验证页面守卫、`I_TOPPA_RECORD` 判定与寮突战斗准备页版式。
