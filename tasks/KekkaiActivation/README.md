# KekkaiActivation（结界挂卡）

> 移植自 `runhey/OnmyojiAutoScript@dev`（commit `8b4dc66`）。行为/阻塞项见 [migration/specs/KekkaiActivation.md](../../migration/specs/KekkaiActivation.md)。
> **状态 ⛔**：收卡经验/已选卡激活已迁移；核心「无卡时选最优卡」受 DSL 限制未实现。

## 运行

```powershell
cd D:\OAS-dev\oas-core
.\target\release\oasd.exe task lint --tasks-dir tasks --task KekkaiActivation
.\target\release\oasd.exe run --task tasks\KekkaiActivation --seed 7
```

## 已实现流程

| # | 行为 | 守卫 |
|---|---|---|
| 1 | 庭院→阴阳寮→寮结界 | `I_MAIN_GOTO_GUILD` → `I_GUILD_REALM` → `I_REALM_SHIN` |
| 2 | 收卡经验 | `I_A_HARVEST_EXP/FISH4/KAIKO_4` |
| 3 | 进入挂卡界面 | `I_SHI_CARD` → `I_A_CHECK_CARD`/`I_A_AUTO_INVITE` |
| 4 | 已有生效卡 | `I_A_INVITE` → `ocr O_CARD_ALL_TIME` 记录 |
| 5 | 已选未激活 | `I_A_ACTIVATE_YELLOW` → 确认 → `I_A_INVITE` |
| 6 | 无卡 | `I_A_EMPTY` → `no_target`(blocked)（选卡 ⛔） |
| 7 | 卡面动画/灰显 | `I_A_DEMOUNT` 等待；`I_A_ACTIVATE_GRAY` → `failed`(retryable) |
| 7 | 回庭院 | 返回按钮链 |

## 退出

| exit | result | 场景 |
|---|---|---|
| done | success | 激活/已有卡 |
| no_target | blocked | 无卡（需选卡逻辑） |
| failed | retryable | 激活灰显/激活超时/进入挂卡界面失败 |

## 已知限制（⛔）

1. 选卡主功能：需要多行 OCR 逐行数值 + 行框点击 + 按卡类型/最小数量过滤排序（`O_CHECK_CARD_NUMBER.detect_and_ocr`）→ DSL 无原语。
2. 剩余时长解析（`O_CARD_ALL_TIME.ocr_duration` → timedelta）+ 流程内调度 → 无原语。
3. 未找到卡计数/切换卡类型/180 分钟重试 → 无持久化/调度原语。
4. 式神育成换满级（ReplaceShikigami）→ 无原语。
5. 无实机证据。

## 本轮静态修正（静态润色轮）

- 收卡经验候选从 3 个补全为旧仓 8 个（I_A_HARVEST_EXP/FISH4/KAIKO_4/KAIKO_3/KAIKO_6/FISH_6/MOON_3/FISH_3）
- 新增卡面动画等待（I_A_DEMOUNT 消失），避免动画期间误判为未知状态
- 新增 I_A_ACTIVATE_GRAY（激活灰显=当前卡不可激活）与激活后仍未出现 I_A_INVITE 的显式失败分支（failed/retryable）
- 进入挂卡界面 15 次失败增加显式守卫（failed），并 gate 后续步骤
- 删除死出口 no_resource（flow 从不产生）

> 注：`migration/specs/KekkaiActivation.md` 的 exit/行为表述如与本轮冲突，以本节与 flow.json 为准。
