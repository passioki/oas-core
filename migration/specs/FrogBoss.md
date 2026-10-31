# FrogBoss（呱太竞猜）迁移规格

- 源：`runhey/OnmyojiAutoScript@dev` commit `8b4dc66`；只读 `_verify/remote-oas`
- 结论：**默认策略（majority）主链路已表达**（活动入口 → 状态循环 → 下注 → 确认 → 领奖/下一局 → 退出）；
  `oas`/`dashen` 策略依赖在线预测与本地 jsonl 历史，DSL 无法表达（⛔ 部分），状态 **🔄**。

## 1. 读取的源文件

| 文件 | 作用 |
|---|---|
| `tasks/FrogBoss/script_task.py` | 入口、状态循环、7 种下注策略、confirm_bet、next_run 时间窗 |
| `tasks/FrogBoss/config.py` | `strategy_frog`（默认 Majority）、`before_end_frog`（15min） |
| `tasks/FrogBoss/assets.py` | 28 素材：投注左右/成功/失败/休息/金袋/下一局/记录 |
| `tasks/FrogBoss/frog_oas.py`、`record_reader.py` | Oas 策略：HTTP 预测 + 指纹 + jsonl 历史（网络/文件） |
| `tasks/Component/RightActivity/right_activity.py` | 右侧活动入口 `I_TOGGLE_BUTTON` |
| `tasks/Component/GeneralBattle/assets.py` | 通用领奖 `I_REWARD` / `C_REWARD_2` |

## 2. 入口/出口

- 入口：庭院 → `Component_RightActivity.I_TOGGLE_BUTTON` → `FrogBoss.I_FROG_BOSS_ENTER` → `I_FROG_CHECK`。
- 出口：`done`（已下注/已处理）、`skipped`（休息中）；`failed`（超时）。
- 旧仓 `next_run()` 按小时窗（10-22 每两小时）设置下次运行；新引擎由 scheduler.json 的窗口负责（见 §6）。

## 3. 行为矩阵

| # | 状态 | 判定 | 行为 |
|---|---|---|---|
| S1 | 已下注 | `I_BETTED` | 结束 |
| S2 | 休息中 | `I_FROG_BOSS_REST` | `skipped` 结束 |
| S3 | 竞猜成功 | `I_BET_SUCCESS` | 循环点 `I_BET_SUCCESS_BOX`/`I_REWARD`/`I_NEXT_COMPETITION` 直到休息或左右入口 |
| S4 | 竞猜失败 | `I_BET_FAILURE` | 点 `I_NEXT_COMPETITION` 到消失 |
| S5 | 可下注 | `I_BET_LEFT` & `I_BET_RIGHT` | 按策略选边 → 点击 → 确认 |
| S6 | 确认下注 | `I_GOLD_30`/`I_GOLD_30_CHECK`/`I_BET_SURE`/`I_UI_CONFIRM(_SAMLL)` | 选 30 勾玉金袋（上部袋身）→ 提交最多 3 次，直到 `I_BETTED`/`I_FROG_BOSS_REST` |
| S7 | 空闲兜底 | `I_NEXT_COMPETITION` | 点下一局 |
| S8 | 退出 | 返回 | 循环返回按钮到庭院 |

## 4. 参数映射

| 旧配置 | 新 params | 说明 |
|---|---|---|
| `strategy_frog`（默认 Majority） | `strategy`（string，默认 frog_majority） | 支持 frog_majority/frog_minority/frog_bilibili/always_red/always_blue；oas/dashen 回退 majority 并告警 |
| `before_end_frog`（15min） | 未映射 | 属于调度间隔，交给 scheduler.json |

## 5. DSL 映射

- 下注计数：`ocr_digit(O_LEFT_COUNT)` / `ocr_digit(O_RIGHT_COUNT)` + 比较表达式。
- 选边：`click(I_BET_LEFT/I_BET_RIGHT)`；分策略用嵌套 if。
- 确认：`click_region(C_REWARD_2)`（关金袋说明）、`click(I_GOLD_30)` 上部内联 ROI、`click(I_BET_SURE)`+计数 `submits`。
- 领奖：`click(I_BET_SUCCESS_BOX)` / `click(Component_GeneralBattle.I_REWARD)`。

## 6. 差异 / 未覆盖

1. **Oas 策略 ⛔**：`frog_oas.py` 需要 HTTP 预测 API + 画面指纹 + 本地 jsonl 历史（`data/frog_oas/*.jsonl`）；DSL 无网络/文件/图像指纹原语。当前回退 majority。
2. **Dashen 策略 ⛔**：抓取 27 个 UP 主 feed 的 163 API + 正则/时间窗判定；同样无法表达。当前回退 majority。
3. **时间窗调度**：旧 `next_run()` 按小时算下次运行；新由 scheduler 窗口/quota 表达。
4. **记录页读取**（`record_oas_history_page`，Oas 策略专用）未迁移。
5. **`I_FROG_LAST_*`/`O_FROG_LAST_TIME`/`O_TIME_REMAIN`** 未使用（旧仓仅日志/记录辅助）。
6. **无实机/replay 证据**（模拟器 offline）。
