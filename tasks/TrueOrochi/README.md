# TrueOrochi（真·八岐大蛇）

远程源：`runhey/OnmyojiAutoScript@dev` `tasks/TrueOrochi`（commit `8b4dc66`）。

**状态：🔄** — 主链路（玩法 tab 次数判定→御魂页→真蛇入口→建房/挑战→多阶段战斗）覆盖；十层×10 触发与第二次存储入口未迁移

## 行为（抽象）

1. `goto_page(page_gameplay)`（探索大地图·玩法 tab）
2. `appear(I_ST_MODULE)` 为假 ⇒ 本周已打完：`check_times(True)` + 结束
3. 方式一/方式二（配置）
4. `goto_page(page_orochi)` + 0.5s → `appear(I_FIND_TS)`
5. `find_true_orochi=false` ⇒ 结束；否则切十层 `check_layer(TEN)` + 解锁 → 循环最多 10 次：点 `I_OROCHI_FIRE` 打十层，每次战后重新检测 `I_FIND_TS`；仍未出现 ⇒ 失败结束
6. `run_true_orochi_battle()`：等 `I_ST_CREATE_ROOM`；点 `I_UI_CONFIRM`/`I_ST_FIRE`/`I_FIND_TS`；进入准备后点 `I_ST_FIRE_PREPARE`→点 `I_ST_AUTO_FALSE` 切自动；战斗循环：`I_GREED_GHOST` 处理、`I_ST_FRAME` 点按（多阶段推进）、280s 超时×3 ⇒ `GameStuckError`
7. 战后回御魂页重查 `I_FIND_TS`；若还在 ⇒ 再打一次（支持存储两次）
8. `check_times(battle)` 设定 success/failure interval（3 天 / 1 天）；`TaskEnd`

## 目录 / 契约

```
tasks/TrueOrochi/
├─ meta.json       # params/exits/capabilities/schedule_hint
├─ flow.json       # oas-flow DSL（仅既有 op）
├─ migration.json  # 源文件、差异、证据
└─ README.md
```

参数：无

## 运行 / 校验

```powershell
cd D:\OAS-dev\oas-core
.\target\release\oasd.exe task lint --tasks-dir tasks --task TrueOrochi   # => errors=0
.\target\release\oasd.exe run --task tasks\TrueOrochi
```

## 已知限制

1. 十层×10 触发真蛇未迁移（需 RuleList 选层 + 循环战斗）
2. 第二次存储真蛇入口未迁移
3. 御魂切换/绿标配置未迁移
4. 无实机/回放证据

详见 `migration/specs/TrueOrochi.md`、`tasks/TrueOrochi/migration.json`、`migration/status/TrueOrochi.json`。

## Polish round (2026-11-01)

- 入口链补守卫：探索灯笼点击加 `requires=page_main / expect=page_exploration`；玩法 tab 改用远程 RuleClick 资产 `legacy_Exploration_C_CLICK_PALY_TITLE` 并 `expect=legacy_Exploration_I_CHECK_PLAY_TITLE`（真蛇次数入口 I_ST_MODULE 在玩法 tab 上）。
- 八岐大蛇入口改为 `requires=legacy_GameUi_I_CHECK_SOUL_ZONES`，页面判定从单一 `I_OROCHI_CHECK_11` 改为 `CHECK_10/11/12/13` 任一（与远程 `any_of` 一致）。
- 真蛇战斗段加准备页门禁：未出现准备页时不再无条件 `wait_until(BATTLE_INFO)` 超时，而是回庭院 `no_target` 退出。
- 回庭院/结束标记统一 `legacy_GameUi_I_CHECK_MAIN`；`meta.entry.requires` 对齐。
- lint: 0 errors; no real-device evidence.


