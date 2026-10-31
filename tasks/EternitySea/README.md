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

参数：`layer=4`（1~4）; `max_battles=30`（1~99）; `lock_team=1`（0~1）

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

## 2026-11-01 静态修整（polish round）

- 修复：进本：旧 flow 单击 I_ETERNITY_SEA + expect I_FORM_TEAM 不稳 → 镜像 `_enter_eternity_sea()`：循环点 I_ETERNITY_SEA 直到 I_FORM_TEAM 出现（含 I_BACK_BOTTOM 兜底）
- 修复：锁队：旧 flow 只「看到 UNLOCK 点一下」未确认锁上；上游 ALONE 要求 lock_team_enable=True 否则 RequestHumanTakeover → 新增 param lock_team（默认 1）+ 镜像 check_lock(True) 有界循环（LOCK 出现即停，UNLOCK 出现则点）；lock_team=0 时记录上游语义并走 failed
- 修复：单人循环：瞬态无 I_ETERNITY_SEA_FIRE 就 break → 改为 warn + wait 重试（受 max_battles 有界）
- 仍存在（blocker/未迁移）：lock_team=0 时按上游 ALONE 的 RequestHumanTakeover 语义走 failed 出口（不静默硬跑）
- 校验：`oasd task lint --tasks-dir tasks --task EternitySea` => errors=0
