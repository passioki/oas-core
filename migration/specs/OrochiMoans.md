# OrochiMoans — 行为矩阵（迁移规格 · 阻塞）

源：`runhey/OnmyojiAutoScript@dev` · `tasks/OrochiMoans/`

## 源文件

- `tasks/OrochiMoans/config.py`
- `tasks/OrochiMoans/assets.py`
- `tasks/Orochi/config.py`
- `tasks/Orochi/script_task.py`

## 行为矩阵

| 旧仓行为 | 新 flow 覆盖 | 说明 |
|----------|--------------|------|
| 见 README「阻塞原因」 | ❌ | 旧仓该目录只有 config.py + 空的 assets.py，没有 `script_task.py`，无法抽象出任何屏幕行为。 |
| 见 README「阻塞原因」 | ❌ | 「御魂悲鸣」（悲鸣）是 `Orochi` 任务的御魂 zone 选项（`tasks/Orochi/config.py: ELEVEN='悲鸣'`），不是独立任务 |
| 见 README「阻塞原因」 | ❌ | `Orochi` 任务不在本批次 assigned tasks 中。 |

## 阻塞原因

1. 旧仓该目录只有 config.py + 空的 assets.py，没有 `script_task.py`，无法抽象出任何屏幕行为。
2. 「御魂悲鸣」（悲鸣）是 `Orochi` 任务的御魂 zone 选项（`tasks/Orochi/config.py: ELEVEN='悲鸣'`），不是独立任务。
3. `Orochi` 任务不在本批次 assigned tasks 中。

## TODO

1. 等待 `Orochi` 任务完成迁移后，用 `call_task` 调用并传入「悲鸣」zone 参数（Orochi 是否有 zone 参数由该任务迁移决定）。
2. 或在 scheduler 中把原 OrochiMoans 条目映射到 Orochi 任务的悲鸣 zone 配置。
3. 本批次先以 ⛔ blocked 登记，避免伪造独立行为。
