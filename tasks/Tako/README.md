# Tako（石距）

## 来源（只读旧仓）

- `tasks/Tako/script_task.py`（135 行）— run / exit_task / battle_wait 重写
- `tasks/Tako/config.py` — `tako_config`（金银/经验加成 4 个开关，默认全 False）、`switch_soul`
- `tasks/Tako/assets.py` — 空类（任务自身无素材，全部复用 Component/GameUi 资产）
- `tasks/Component/GeneralRoom/general_room.py`、`GeneralInvite/general_invite.py`、`GeneralBattle/general_battle.py`

## 行为摘要（旧仓）

1. 可选换御魂；可选开加成（`open_buff` → gold/exp 开关 → `close_buff`）。
2. `goto_page(page_team)`；按星期选副本：周末（周六/周日）「愤怒的石距」，否则「石距」。
   `check_zones` 对「愤怒的石距」做 OCR 重映射（识别成「价悠的石距」也能选中）。
3. `create_room()`（失败 → exit_task）→ `ensure_public()` → `create_ensure()`。
4. 房间内等待 60 秒或有人加入（`not appear(I_ADD_1)`）→ `click_fire()` → `run_general_battle()`；**只打一场**。
5. `exit_task()`：回庭院，关加成，`set_next_run`。

## 流程（flow.json）

| # | op | 说明 | 守卫 |
|---|----|------|------|
| 1 | 进组队页 | 主界面 → 组队 | expect `I_CHECK_TEAM` |
| 2 | if `param('weekend')` | 周末 → 「价悠的石距」/ 工作日 → 「石距」 | 列表 OCR |
| 3 | create_room → 没券 `no_resource` | | 模板守卫 |
| 4 | ensure_public / create_ensure | | |
| 5 | 等队友（有界）→ 点挑战 | | `I_ADD_1` / `I_FIRE` |
| 6 | 战斗（准备→自动→结算） | 只一场 | |
| 7 | 退出房间 → 回庭院 | | expect `I_CHECK_MAIN` |
| 8 | finish done | | |

## 素材

| 全局 id | 来源 | 用途 |
|---------|------|------|
| legacy_Component_GeneralRoom_* | GeneralRoom | 创建/公开/退出/副本名 OCR |
| legacy_Component_GeneralInvite_* | GeneralInvite | 进房/挑战 |
| legacy_Component_GeneralBattle_* | GeneralBattle | 准备/胜利/失败/奖励 |
| legacy_GameUi_I_MAIN_GOTO_TEAM / I_CHECK_TEAM / I_CHECK_MAIN | GameUi | 导航 |
| legacy_GlobalGame_* | GlobalGame | 返回/确认/奖励 |

## 出口

- `done`(success)
- `no_resource`(resource)：创建房间失败（没券；旧仓 `create_room=False` 时直接 exit_task）
- `failed`(retryable)

## 与旧仓的差异 / 已知缺口

1. **周末判定不迁移**：旧仓 `5 <= weekday <= 6` 依赖 wall-clock，DSL 没有日期函数 → 用 `param('weekend')` 由调度/参数决定。
2. **60 秒等待计时器** → 有界 20×2.5s。
3. **buff 加成（金/经验 50·100）未迁移**：GeneralBuff 动态开关区域需要锚点定位（DSL 缺），默认关闭。
4. **换御魂未迁移**（默认关闭）。
5. `check_zones` RuleList → text_visible + 行内 OCR 点击 + swipe；「愤怒的石距」沿用旧仓 OCR 重映射文本「价悠的石距」。
6. 战斗主链只迁移「准备→自动战斗→结算→页面回归」；绿标/预设/随机点击未迁移。
7. 未真机验证。

## Lint

```powershell
cd D:\OAS-dev\oas-core
.\target\release\oasd.exe task lint --tasks-dir tasks --task Tako
```
