# OtherWorldTwilight（彼世逢魔（超鬼王? 否：逢魔之时·彼世））

远程源：`runhey/OnmyojiAutoScript@dev` `tasks/OtherWorldTwilight`（commit `8b4dc66`）。

**状态：🔄** — 单人模式覆盖；队长/队员（建房/邀请/等待）未迁移

## 行为（抽象）

1. `switch_soul.enable`/`enable_switch_by_name` ⇒ 式神录切御魂
2. `run_leader()`：进彼世逢魔页→锁队→点 I_OWT_TEAM 创建队伍→`create_room()`→`ensure_private()`→`create_ensure()`→邀请循环（`check_and_invite`/`run_invite`）→`run_general_battle`；达到 `limit_count`(30) 或 `limit_time`(30min) 退出→exit_room/exit_team
3. `run_member()`：主界面等邀请/进房→`wait_battle`→`run_general_battle`；限次数/限时后 exit_room/exit_battle
4. `run_alone()`：进彼世逢魔页→锁队→循环：在本页点 `I_OWT_FIRE` 挑战；在战斗准备/战斗/结算页走通用战斗；未知页 10s 超时回彼世逢魔页；限次数/限时退出
5. 回庭院；`set_next_run(finish, success)`

## 目录 / 契约

```
tasks/OtherWorldTwilight/
├─ meta.json       # params/exits/capabilities/schedule_hint
├─ flow.json       # oas-flow DSL（仅既有 op）
├─ migration.json  # 源文件、差异、证据
└─ README.md
```

参数：`max_battles=30`（1~99）

## 运行 / 校验

```powershell
cd D:\OAS-dev\oas-core
.\target\release\oasd.exe task lint --tasks-dir tasks --task OtherWorldTwilight   # => errors=0
.\target\release\oasd.exe run --task tasks\OtherWorldTwilight --param max_battles=30
```

## 已知限制

1. 队长/队员模式（建房/邀请/等待开战）未迁移
2. limit_time 时间上限未在 flow 内实现（仅 max_battles 次数上限）
3. 无实机/回放证据

详见 `migration/specs/OtherWorldTwilight.md`、`tasks/OtherWorldTwilight/migration.json`、`migration/status/OtherWorldTwilight.json`。
