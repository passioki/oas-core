# FindJade — 行为矩阵（迁移规格 · 阻塞）

源：`runhey/OnmyojiAutoScript@dev` · `tasks/FindJade/`

## 源文件

- `tasks/FindJade/script_task.py`
- `tasks/FindJade/config.py`
- `tasks/FindJade/assets.py`
- `tasks/FindJade/WantedQuestsEx.py`
- `tasks/FindJade/README.md`
- `tasks/Component/SwitchAccount/switch_account.py`
- `tasks/Component/SwitchAccount/login_account.py`

## 行为矩阵

| 旧仓行为 | 新 flow 覆盖 | 说明 |
|----------|--------------|------|
| 见 README「阻塞原因」 | ❌ | 需要「多账号切换/登录」能力 |
| 见 README「阻塞原因」 | ❌ | 需要「运行时动态构造子任务并改方法」（旧仓 `CreatObjectFromModule` + `type()` 注入） |
| 见 README「阻塞原因」 | ❌ | 依赖 `WantedQuests` 任务（不在本迁移批次的 assigned tasks 中）及其协作邀请逻辑。 |

## 阻塞原因

1. 需要「多账号切换/登录」能力：当前 oas-core DSL 只有屏幕点击/识别，没有账号切换、登录页流程、多模拟器编排。
2. 需要「运行时动态构造子任务并改方法」（旧仓 `CreatObjectFromModule` + `type()` 注入）：与引擎「DSL 无外部代码」安全设计冲突（T6）。
3. 依赖 `WantedQuests` 任务（不在本迁移批次的 assigned tasks 中）及其协作邀请逻辑。
4. 依赖 wall-clock 时段判断与 `last_complete_time` 持久化历史（DSL 无时间函数）。

## TODO

1. 先完成 `WantedQuests` 迁移（含悬赏协作邀请），并给 DSL 增加 `call_task` 参数化调用能力。
2. 在引擎层实现账号切换（登录/登出/渠道登录）或明确由外部脚本切换后以 `entry` 校验登录态。
3. 给表达式增加时间函数（`now()`/`elapsed()`）或在调度器层实现时段与间隔限制。
4. 完成后把 FindJade 重写为「切号（外部）→ call_task WantedQuests(invite=true)」的编排任务。
