# 素材数据模型（v0.1，现行）

> 目的：固定“全局素材 / 任务素材 / 旧模板库”三者的边界，避免再出现两套素材库 UI。
> 相关：`ENGINE-DESIGN.md §22.38/§22.39`（R11）、`START-HERE.md`。
> 当前快照（2026-11-02，HEAD `cd0ac34`）：`assets/` = **16 条 / 1 图**；任务只有 `返回庭院`；帧 28 个（`current_id=20`）。


## 1. 唯一素材存储：`assets/`

```text
assets/
  assets.json
  images/aNNN.png
```

- 全局素材只有这一份；
- 素材 id 全局唯一；
- 模板图片固定用顺序文件名 `aNNN.png`，不把 id 当路径；
- 旧任务目录里的 `tasks/<task>/assets.json` 是历史形态，v0.1 起不再作为长期存储。

## 2. 任务素材 = 全局素材的子集

任务不拥有第二份素材，只保存：

```text
tasks/<task>/meta.json
tasks/<task>/flow.json
```

任务素材定义为：

```text
task 的 flow 引用的全局 asset id 集合
```

因此：

- **没有任务** → 任务页素材库为空；
- **任务 A 的素材库** → 只显示 A.flow 引用的全局素材；
- **素材页的全局素材库** → 显示 `assets/` 全部素材；
- 同一素材可以被多个任务引用，不复制。

## 3. 旧 `library/` 的定位

`library/` 是 v1.1 的旧全局模板库：

```text
library/assets.json + images/aNNN.png
flow 用 lib:<id> 引用
```

v1.2.1 起统一为 `assets/`。  
v0.1 起：

- 编辑器不再显示旧 `library/` 面板；
- 后端 `/library`、`lib:` 引用保留仅用于历史数据兼容；
- 新素材一律进 `assets/`。

**不要**再把 `/library` 当成编辑器素材库。

## 4. 删除任务

正确语义：

```text
删除任务 = 删除 meta/flow + 释放引用
素材仍保留在 assets/
```

当前实现为安全过渡：

1. 删除任务前把该任务旧 `assets.json` + `images/` 备份到
   `assets/_deleted-tasks/<task>-<timestamp>/`；
2. 备份失败则拒绝删除；
3. 已迁移到全局库的素材不受影响。

## 5. 历史丢失与恢复（34/18 已推翻，只作历史）

> ⚠ 2026-11-01 `§E-70` 裁定：下面的 34/18 是助手从 `tasks/*/assets.json` git 历史拼出的**错误恢复**，不是 archive 全量；用户 21:02 决定“先不管素材，素材可以重建”。**当前 `assets/` = 16 条 / 1 图**，不要再用这组脚本恢复。

- 旧任务删除时曾直接 `remove_dir_all`，任务目录里的 `assets.json` 随任务消失；
- 11-01 曾按 task 历史恢复 34 个唯一素材 / 18 个模板图，合并进 `assets/assets.json`（**已被 §E-70 推翻**）；
- 当前权威迁移归档：`archive/migration-20261101 = 29b696f`（2827 素材 / 2099 图 / 57 个任务目录），**未导入当前 dev**；
- 历史恢复报告：`assets/history-recovery-report.json`（`after=34`，与当前 16 不符；属数据区，由数据 owner 处理）；
- 历史工具（**不要重跑**）：
  - `scripts/audit_asset_history.py`（只读审计）
  - `scripts/recover_historical_assets.py`（带备份恢复）

## 6. UI 对应关系

| 页面 | 库 | 数据源 |
|---|---|---|
| 素材页 | 全局素材库 | `assets/` 全部素材 |
| 任务页 | 任务素材库 | `assets/` 中被当前任务 flow 引用的子集 |
| 任务构建 | 素材选择器 | 全部全局素材 |
