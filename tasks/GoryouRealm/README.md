# GoryouRealm（御灵）

## 来源（只读旧仓）

- `tasks/GoryouRealm/script_task.py`（88 行）— 进入御灵、选御灵、循环挑战（次数/时间/票数限制）、回探索
- `tasks/GoryouRealm/config.py` — `limit_time`（默认 30 分钟）、`limit_count`（默认 30）、`goryou_class`（默认 RANDOM）
- `tasks/GoryouRealm/assets.py` — 4 个 RuleClick（御灵卡片区域）、I_GR_FIRE/LOCK/UNLOCK、O_GR_TICKET
- `tasks/Component/GeneralBattle/general_battle.py` — `run_general_battle` / `check_lock`
- `tasks/GameUi/default_pages.py` — `page_goryou_realm` 与 `page_exploration→page_goryou_realm`

## 行为摘要（旧仓）

1. 可选换御魂（`switch_soul_config.enable`）、`goto_page(page_goryou_realm)`。
2. 选御灵：随机（默认）或配置的 4 种之一，点对应 `C_GR_C_*` 区域直到 `I_GR_FIRE` 出现。
3. `check_lock(lock_team_enable, I_GR_LOCK, I_GR_UNLOCK)`：按配置锁定/解锁阵容。
4. 循环：
   - `limit_count`（默认 30）到 → break；
   - `limit_time`（默认 30min）到 → break；
   - `OCR O_GR_TICKET == 0` → break；
   - 点 `I_GR_FIRE`（挑战）直到消失 → `run_general_battle()`（点准备 → 自动战斗 → 结算）。
5. 点 `I_UI_BACK_YELLOW` 回探索页。

## 流程（flow.json）

| # | op | 说明 | 守卫 |
|---|----|------|------|
| 1 | click `庭院_探索灯笼` | 庭院 → 探索 | expect `世界_章节栏` |
| 2 | click `I_EXPLORATION_GOTO_GORYOU_REALM` | 探索 → 御灵 | expect `I_CHECK_GORYOU` |
| 3 | 分支点击 `C_GR_C_1..4` | 按 `param('goryou_class')` 选御灵 | requires 探索页 |
| 4 | loop 等 `I_GR_FIRE` | 御灵页加载完成 | |
| 5 | 锁定/解锁阵容 | `param('lock_team')` 分支 | 模板守卫 |
| 6 | loop×`param('max_battles')` | 票 OCR==0 → 记 `empty` 结束；否则挑战→准备→自动战斗→结算循环 | OCR/模板 |
| 7 | 黄返回 ×2 | 御灵 → 探索 → 庭院 | 页面守卫 |
| 8 | if `var('empty')==1` → finish `resource_empty`；否则 finish `done` | | |

战斗结算沿用「个人突破」已验证模式：点 `战斗_准备按钮` → `战斗_自动按钮` 出现即进入战斗 →
循环点 (640,690) 直到 `I_GR_FIRE` 重新出现（对胜负结算都成立）。

## 素材

| 全局 id | 旧 const | 用途 |
|---------|----------|------|
| legacy_GoryouRealm_I_GR_FIRE | I_GR_FIRE | 挑战按钮 / 战斗前页面守卫 |
| legacy_GoryouRealm_I_GR_LOCK / I_GR_UNLOCK | 锁定状态 | 阵容锁定状态 |
| legacy_GoryouRealm_C_GR_C_1..4 | 御灵卡片 RuleClick | 选御灵 |
| legacy_GoryouRealm_O_GR_TICKET | O_GR_TICKET | `ocr_digit` 票数 |
| 庭院_探索灯笼 / 世界_章节栏 / 战斗_准备按钮 / 战斗_自动按钮 | core | 导航 / 战斗 |
| legacy_GameUi_I_CHECK_GORYOU / I_EXPLORATION_GOTO_GORYOU_REALM | page 图 | 导航 |
| legacy_GlobalGame_I_UI_BACK_YELLOW | 通用 | 返回 |

## 出口

- `done` → success：次数上限/时间到/票数恢复停（未用尽）等正常结束。
- `resource_empty` → resource：御灵票为 0。
- `failed` → retryable：页面/战斗守卫失败。

## 与旧仓的差异 / 已知缺口

1. **时间限制不迁移**：DSL 表达式没有 wall-clock（`now()`/`elapsed()`），`limit_time`（默认 30 分钟）无法表达。
   以 `param('max_battles')`（默认 30 = 旧仓 limit_count 默认）作为唯一循环上限；如需按时间限流须引擎新增时间函数。
2. **RANDOM 御灵选择不迁移**：DSL 无随机 op；`param('goryou_class')` 默认 1（暗神龙），RANDOM 行为记录为缺口。
3. **票数判定等价**：旧仓 `O_GR_TICKET.ocr(image) == 0`；新引擎用 `ocr_digit('legacy_GoryouRealm_O_GR_TICKET') == 0`（ocr_lines → 首个数字）。
4. 旧仓 `run_general_battle` 还包含绿标/预设队伍/buff/随机点击/战败判定；本流程只迁移「点准备→自动战斗→结算」主链，
   其余为 `GeneralBattleConfig` 可选功能（默认全 false），记录为未迁移项。
5. 未真机验证。

## Lint

```powershell
cd D:\OAS-dev\oas-core
.\target\release\oasd.exe task lint --tasks-dir tasks --task GoryouRealm
```

<!-- round2:start -->
## Polish round 2（2026-11-01 · Lead 实机反馈）

实机反馈（Lead）：导航已通；进入御灵页后 4 张御灵卡片显示「40级 / 御灵等级」要求，`I_GR_FIRE` 不出现。

本次修复：
1. 选御灵后不再直接 `appear(I_GR_FIRE)` 硬失败，改为有界等待 `wait_fire(10)`；
2. 配置御灵不可用时**依次尝试其余 3 张卡片**（每张点击后 4×1s 等挑战按钮）；
3. 4 张都不可用 → 记录「40级/御灵等级不足」证据（`text_visible('40级')` 辅助日志）→
   `finish(no_target)`（meta 新增 `no_target: blocked`），**不再卡死/无限重试**；
4. 卡片可用后继续原有循环挑战逻辑（票 OCR、准备、自动战斗、结算、回庭院）。

剩余阻塞：御灵卡片解锁条件（等级/御灵等级）无法在流程内提升，只能 blocked 退出等待玩家满足；未验证 4 张卡全部尝试的真实耗时。
<!-- round2:end -->
