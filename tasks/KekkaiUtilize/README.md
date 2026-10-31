# KekkaiUtilize（寮结界寄养/收取）

> 移植自 `runhey/OnmyojiAutoScript@dev`（commit `8b4dc66`）。行为/阻塞项见 [migration/specs/KekkaiUtilize.md](../../migration/specs/KekkaiUtilize.md)。
> **状态 ⛔**：收取类行为已迁移；核心「蹭卡/寄养」受 DSL 限制未实现。

## 运行

```powershell
cd D:\OAS-dev\oas-core
# 起始画面=庭院
.\target\release\oasd.exe task lint --tasks-dir tasks --task KekkaiUtilize
.\target\release\oasd.exe run --task tasks\KekkaiUtilize --seed 7
```

## 已实现流程

| # | 行为 | 守卫 |
|---|---|---|
| 1 | 庭院→阴阳寮→寮结界 | `I_MAIN_GOTO_GUILD` → `I_GUILD_REALM` → `I_REALM_SHIN`/`I_SHI_DEFENSE` |
| 2 | 寄养收获 | `I_UTILIZE_EXP` → 奖励弹窗 |
| 3 | 蹭卡入口检测 | `I_UTILIZE_ADD`（仅告警，主功能 ⛔） |
| 4 | 体力盒子 | `I_BOX_AP` → `I_AP_EXTRACT` → `I_UI_REWARD` |
| 5 | 经验盒子 | `I_BOX_EXP`/`I_BOX_EXP_MAX` → `I_EXP_EXTRACT` → 确认/奖励 |
| 6 | 寮资金/体力 | `I_GUILD_EXPAND`/`I_GUILD_ASSETS_RECEIVE`/`I_GUILD_ASSETS`/`I_GUILD_AP` |
| 7 | 回庭院 | 返回按钮链 |

## 退出

| exit | result | 场景 |
|---|---|---|
| done | success | 收取完成 |
| no_resource | resource | 预留 |

## 已知限制（⛔）

1. 蹭卡/寄养主功能：需要 OCR 时长解析、最优卡选择、好友列表 OCR 相似匹配、进入好友结界确认等原语，DSL 均不具备。
2. 式神育成换卡（ReplaceShikigami：满级下阵/按类型上阵/长按多选）无原语。
3. 经验盒子上限 OCR 三元组判断未迁移。
4. 下一次运行时间（按卡剩余时长）交给调度层；无实机证据。
