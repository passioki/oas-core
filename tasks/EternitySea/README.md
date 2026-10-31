# EternitySea（永生之海）

远程源：`runhey/OnmyojiAutoScript@dev` `tasks/EternitySea`（commit `8b4dc66`）。

**状态：🔄** — 单人模式主链路覆盖；队长/队员（建房/邀请/等待）未迁移，层数选择用坐标近似

## 行为（抽象）

1. 两套 switch_soul 配置（enable/enable_switch_by_name）
2. `_prepare_eternity_sea`（御魂副本→永生之海→选层→锁队）→建房→私密→邀请循环→通用战斗；limit_count(30)/limit_time(30min)→`_wait_room_stable`→exit_room/exit_team→`_back_to_soul_zones`→回庭院
3. 进永生之海→等邀请/进房→`wait_battle`→通用战斗→退出房间/战斗→回庭院
4. 御魂副本→永生之海→选层→**必须锁队**（否则 RequestHumanTakeover）→循环：本页点 `I_ETERNITY_SEA_FIRE` 挑战→通用战斗；次数/时间上限退出
5. 永生之海页无 HOME，点 `I_BACK_BOTTOM` 回御魂副本选择页；确认弹窗 `I_UI_CONFIRM`；再回庭院

## 目录 / 契约

```
tasks/EternitySea/
├─ meta.json       # params/exits/capabilities/schedule_hint
├─ flow.json       # oas-flow DSL（仅既有 op）
├─ migration.json  # 源文件、差异、证据
└─ README.md
```

参数：`layer=4`（1~4）; `max_battles=30`（1~99）

## 运行 / 校验

```powershell
cd D:\OAS-dev\oas-core
.\target\release\oasd.exe task lint --tasks-dir tasks --task EternitySea   # => errors=0
.\target\release\oasd.exe run --task tasks\EternitySea --param layer=4
```

## 已知限制

1. 队长/队员模式未迁移（建房/邀请/等待）
2. 选层为坐标近似（RuleList 未导入）
3. 御魂切换与时间上限未迁移
4. 无实机/回放证据

详见 `migration/specs/EternitySea.md`、`tasks/EternitySea/migration.json`、`migration/status/EternitySea.json`。
