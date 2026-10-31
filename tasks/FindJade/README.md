# FindJade（⛔ 迁移阻塞）

## 结论

本任务**无法**用当前 oas-core 引擎（oas-flow DSL = 屏幕识别/点击）表达，迁移状态 ⛔。

FindJade 自身没有游戏 UI 逻辑：
- `run()` 遍历 `sup_account_list`（小号配置），`is_need_login()` 按「上次完成时间 + 13 小时/18 点时段」判断是否需要登录；
- `SwitchAccount(config, device, accountInfo).switchAccount()` 完成**换账号登录**（旧仓用第二台模拟器/登录页自动化）；
- 然后 `CreatObjectFromModule("WantedQuests", ...)`：动态 import WantedQuests 的 ScriptTask、运行时用 `type(...)` 注入
  `need_invite_vip` / `get_invite_vip_name` / `next_run` / `invite_success_callback` / `get_config` 覆盖方法，
  以「邀请小号协作」的方式跑悬赏封印；
- `TaskEnd` 时更新账号登录历史并 save_config；失败按 10 分钟后重试。

## 来源（只读旧仓）

- `tasks/FindJade/script_task.py`
- `tasks/FindJade/config.py`
- `tasks/FindJade/assets.py`
- `tasks/FindJade/WantedQuestsEx.py`
- `tasks/FindJade/README.md`
- `tasks/Component/SwitchAccount/switch_account.py`
- `tasks/Component/SwitchAccount/login_account.py`

## 阻塞原因（具体）

1. 需要「多账号切换/登录」能力：当前 oas-core DSL 只有屏幕点击/识别，没有账号切换、登录页流程、多模拟器编排。
2. 需要「运行时动态构造子任务并改方法」（旧仓 `CreatObjectFromModule` + `type()` 注入）：与引擎「DSL 无外部代码」安全设计冲突（T6）。
3. 依赖 `WantedQuests` 任务（不在本迁移批次的 assigned tasks 中）及其协作邀请逻辑。
4. 依赖 wall-clock 时段判断与 `last_complete_time` 持久化历史（DSL 无时间函数）。

## 精确 TODO（解锁条件）

1. 先完成 `WantedQuests` 迁移（含悬赏协作邀请），并给 DSL 增加 `call_task` 参数化调用能力。
2. 在引擎层实现账号切换（登录/登出/渠道登录）或明确由外部脚本切换后以 `entry` 校验登录态。
3. 给表达式增加时间函数（`now()`/`elapsed()`）或在调度器层实现时段与间隔限制。
4. 完成后把 FindJade 重写为「切号（外部）→ call_task WantedQuests(invite=true)」的编排任务。

## 现有 flow

`log(原因) × N` + `finish(failed)`，`meta.exits.failed.result = "blocked"`：
调度器会把它记为 blocked（人工介入），**不会伪装成功**。

## 素材

无新增素材需求（旧仓该任务没有可用的 UI 素材，或素材不属于屏幕自动化）。

## Lint

```powershell
cd D:\OAS-dev\oas-core
.\target\release\oasd.exe task lint --tasks-dir tasks --task FindJade
```
