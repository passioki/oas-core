# OrochiMoans（⛔ 迁移阻塞）

## 结论

本任务**无法**用当前 oas-core 引擎（oas-flow DSL = 屏幕识别/点击）表达，迁移状态 ⛔。

旧仓结构：
- `tasks/OrochiMoans/config.py` 仅 `class OrochiMoans(ConfigBase): scheduler`；
- `tasks/OrochiMoans/assets.py` 空类 `pass`；
- 没有 `script_task.py`（inventory 标记 has_script=false）；
- `module/config/config_model.py` 里作为独立配置项注册；
- `tasks/Orochi/config.py` 中 `ELEVEN = '悲鸣'`（御魂种类），说明「悲鸣」是 Orochi 任务的副本/御魂选项。

结论：OrochiMoans 是**遗留的空壳配置**（调度项），没有独立可迁移的屏幕行为。

## 来源（只读旧仓）

- `tasks/OrochiMoans/config.py`
- `tasks/OrochiMoans/assets.py`
- `tasks/Orochi/config.py`
- `tasks/Orochi/script_task.py`

## 阻塞原因（具体）

1. 旧仓该目录只有 config.py + 空的 assets.py，没有 `script_task.py`，无法抽象出任何屏幕行为。
2. 「御魂悲鸣」（悲鸣）是 `Orochi` 任务的御魂 zone 选项（`tasks/Orochi/config.py: ELEVEN='悲鸣'`），不是独立任务。
3. `Orochi` 任务不在本批次 assigned tasks 中。

## 精确 TODO（解锁条件）

1. 等待 `Orochi` 任务完成迁移后，用 `call_task` 调用并传入「悲鸣」zone 参数（Orochi 是否有 zone 参数由该任务迁移决定）。
2. 或在 scheduler 中把原 OrochiMoans 条目映射到 Orochi 任务的悲鸣 zone 配置。
3. 本批次先以 ⛔ blocked 登记，避免伪造独立行为。

## 现有 flow

`log(原因) × N` + `finish(failed)`，`meta.exits.failed.result = "blocked"`：
调度器会把它记为 blocked（人工介入），**不会伪装成功**。

## 素材

无新增素材需求（旧仓该任务没有可用的 UI 素材，或素材不属于屏幕自动化）。

## Lint

```powershell
cd D:\OAS-dev\oas-core
.\target\release\oasd.exe task lint --tasks-dir tasks --task OrochiMoans
```
