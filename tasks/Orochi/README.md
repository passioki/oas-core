# Orochi（八岐大蛇（御魂））

远程源：`runhey/OnmyojiAutoScript@dev` `tasks/Orochi`（commit `8b4dc66`）。

**状态：🔄** — 单人模式主链路覆盖；层数选择（RuleList）、队长/队员/野队、御魂切换、真蛇检测联动未迁移

## 行为（抽象）

1. 方式一/方式二 + 按层 `orochi_switch_soul`（十/悲鸣/神罚/虚无）
2. 开战前后：回庭院→开加成→勾选御魂加成→关面板
3. 御魂页→选层→锁队→`I_FORM_TEAM` 建房→私密→邀请循环→通用战斗；限次数/限时→exit_room/exit_team→回庭院
4. 等邀请/进房→`wait_battle`→通用战斗；限次/限时→退出房间/战斗→回庭院（若在战斗则先退）
5. 御魂页→选层→锁队→循环：`I_OROCHI_FIRE` 挑战→通用战斗；限次/限时退出→返回
6. 建房（公开）→循环：等待开始→限次/限时→`I_OROCHI_WILD_FIRE` 点击→通用战斗；失败则 exit
7. 重写 `battle_wait`：`I_WIN` 点（随机 C_WIN 区域）→ 贪吃鬼/御魂溢出/`I_REWARD` 处理 → `I_FALSE` 失败
8. `check_true_orochi_enable`：战后回御魂页检测 `I_FIND_TS`，命中则把 TrueOrochi 设为立刻运行（调度写回）
9. `set_next_run(success)` + `TaskEnd`

## 目录 / 契约

```
tasks/Orochi/
├─ meta.json       # params/exits/capabilities/schedule_hint
├─ flow.json       # oas-flow DSL（仅既有 op）
├─ migration.json  # 源文件、差异、证据
└─ README.md
```

参数：`max_battles=30`（1~99）

## 运行 / 校验

```powershell
cd D:\OAS-dev\oas-core
.\target\release\oasd.exe task lint --tasks-dir tasks --task Orochi   # => errors=0
.\target\release\oasd.exe run --task tasks\Orochi --param max_battles=30
```

## 已知限制

1. 层数选择需 RuleList（13 项滚动）原语，未迁移
2. 队长/队员/野队模式未迁移
3. 御魂加成/御魂切换/真蛇调度联动未迁移
4. 无实机/回放证据

详见 `migration/specs/Orochi.md`、`tasks/Orochi/migration.json`、`migration/status/Orochi.json`。
