# 变量系统 V2 设计（Variable / 使用端个性化 / Profile 多账号·多配置·多模拟器）

> 状态：**设计稿，待评审；本文只设计、不实现**（批次 F 设计）。评审通过后按 §7 拆实现任务。
> 输入：#13 用户反馈（`docs/ACCEPTANCE-2026-11-06.md` §1 #13 行 24 + §5 Q8 行 72）、`docs/PARAM-ASSET-IMPLEMENTATION.md`（seq124 C1′ 现状与简化令）、`ENGINE-DESIGN.md §22.64`、`RUNNER-STUDIO-SPLIT-PLAN.md §C.1~§C.4`、`ASSET-DATA-MODEL.md`。
> 用户原话（#13）：「配置的交互不应当只有一个 ocr 识别变量（参数），还应该有其他变量的传递，是否需要抽成一个函数，编辑端创建，应用在任务中，发布任务后，使用端安装任务后，可对可修改的变量（参数）予以个性化修改，而这些不同的修改就是使用端配置的一部分。后期会涉及到对多账号/多配置/多模拟器的管理。」
> 基线：HEAD `c16a536`（seq126，**批次 A 已提交**：#11/#9/#5/#6 + #7 启动脚本；批次 B/C/D/E/F 未开始）。本文行号为 2026-11-06 18:4x~18:5x 现场实测；后续批次会漂移 → 实施时按**函数名**定位。现行基线（本轮实测/文档）：UI pure test **2537 checks / 194 ids / 0 dead / 6 nav**（批次 A 后实测；seq124 为 2512，见 `ENGINE-DESIGN.md:3293`）；`cargo test --workspace` seq124 = **904/0**（`ENGINE-DESIGN.md:3292`），批次 A 后需以验收记录回填（本设计阶段不跑 cargo）。

## 1. 目标概念模型（变量 vs 素材 / 任务级 vs 使用端）

### 1.1 三层概念

| 层 | 定义 | 存储位置 | 谁写 |
|---|---|---|---|
| **变量（Variable）** | 具名、有类型/默认值/展示信息的**声明**；= 今天「参数」的泛化 | `tasks/<id>/meta.json` 的 `variables[]`（§2） | 编辑端（Studio） |
| **值（Value）** | 某次运行的变量取值；使用端个性化后持久保存 | Runner `config/values.json`（默认档案）+ 运行参数层 | 使用端（Runner）/调度条目/CLI |
| **Profile（配置档案）** | 「命名变量值集合 + 展示/绑定信息（账号、设备）」；= 使用端配置的一部分 | `config/values.json` v2 `profiles{}`（§4） | 使用端（Runner） |

### 1.2 变量的四种类型（与素材的关系）

| 类型 `value_source` | 载体 | 默认值来源 | 使用端可改 | flow 读法 | 现状对应 |
|---|---|---|---|---|---|
| `ocr_text`（识别文字变量） | **绑定一个 `ocr_var` 素材**（`assets.json items[]`，§2.3）；素材提供 ROI/op_region/候选 | 素材 `runtime_text`（编辑端默认文字） | ✅（Runner 改值不改素材） | `text_expr:"param('<key>')"`（OCR 目标）或表达式 `param('<key>')` | C1′：ocr_var 素材 + 派生 `meta.params` 行（`ocr_var.rs:112,211`） |
| `literal` 标量（`string/int/float/bool`，`enum` 用 `enum` 字段） | **无素材**，独立 `variables[]` 行 | 变量 `default` 字面量 | ✅ | `param('<key>')`；V2.1 起可作条件/循环/目标参数 | 今天 `meta.params` 手写行（`meta.rs:54-100`） |
| `runtime`（运行变量） | 无素材；flow 内 `set_var/inc_var/ocr_to_var` 写入（`oas-flow:1174-1187`、`runtime:6127-6164`） | 变量 `default`（可空，V2.1） | ❌ 使用端不可见 | `var('<name>')`（`oas-flow:500-510`、`runtime:6442`） | 今天未声明、纯 flow 局部状态 |
| 系统/设备变量（V2.2+ 预留，如 `device.serial`/`profile.name`） | 无 | Runner 注入 | ❌（只读） | 保留 `var()` 命名空间扩展 | 无 |

规则：**`ocr_text` 变量 ↔ `ocr_var` 素材严格 1:1**（`asset_id` 反向唯一）；标量/运行变量**不得**带 `asset_id`；删除被变量引用的 ocr_var 素材必须拒绝（同 flow 引用规则）。

### 1.3 任务级 vs 全局 / 编辑端默认 vs 使用端值

- 变量是**任务级**的：随任务 `meta.json` 走、随包发布（manifest 快照）。**不引入全局任务变量**（跨任务传值用 `call_task.args.params` 映射 + 子任务变量，见 §2.6）；`ASSET-DATA-MODEL.md` 的「素材全局唯一、任务只引用」模型不变。
- 编辑端只写 `default`（作者意图）；使用端只写**值覆盖**（Profile）；两者不互相改写（Runner 永不改任务 schema/素材）。
- 值优先级（沿用四层，`RUNNER-STUDIO-SPLIT-PLAN.md:1324-1342`，Profile 替换第 3 层）：**CLI `--param`（调试）> 调度 entry `params` > Profile 值（V2.2；V2.0/2.1 为当前任务级 values）> 变量 `default`**。

### 1.4 术语收敛与「抽成一个函数」

- **变量** = 统一术语（UI/文档/API）；「参数」「配置项」仅保留在历史文档与内部字段名（`meta.params` 过渡期、`param()` 函数名不强制改）。
- **「抽成一个函数」的落地**：编辑端只在「变量」面板**声明一次**（类型/默认/可改性/绑定素材），任务 flow 通过统一读取函数 `param('<key>')`（V2.1 加 `variable()` 别名）引用；同一份声明贯穿 lint → 运行注入 → 发布 manifest 快照 → 使用端配置 UI/值存储 → 子任务映射，全链路不再有第二处手写声明（对应 #13「编辑端创建，应用在任务中，发布后使用端个性化」）。
- 「使用端OCR文字配置项」= `ocr_text` 类型变量的**素材外观**（素材行/识别区/操作区仍按素材管理，`ASSET-DATA-MODEL` 语义不变）。

## 2. Schema 草案（含方案对比）

### 2.1 存储方案对比

| 方案 | 做法 | 风险/成本 | 结论 |
|---|---|---|---|
| A 扩展 `meta.params` | 保留 `meta.params`，加 `runner_editable/scope/asset_id`，UI 改名「变量」 | 改动小；但「参数」概念留在数据契约，未交付「抽成函数/通用变量」，Phase 2 仍欠账 | ❌ 不推荐（仅应急可回退） |
| B1 双段（variables 真相 + params 编译桥） | 新 `variables[]` 为唯一真相，编译出 `meta.params` 供 runtime/lint（重演 C1′ 模式） | 两数组 + 同步/漂移/T8 双份；C1′ 已付过一次代价 | ❌ 不推荐（过渡期可接受，但用户已允许一次性重构） |
| **B2 直改（推荐）** | `meta.variables[]` 为唯一存储；runtime/lint/loader/Runner 全部直接读 variables；`meta.params` **退役（不再写；旧数据一次性导入，不面向用户）** | 一次改 `run.rs/lint/loader/package`；旧包/旧数据不兼容（用户 12:5x 简化令允许） | ✅ **推荐**：= 把 Phase 2 与 #13 合并一次交付，无过渡双真相 |
| C 独立 `variables.json` | 任务目录新增文件 | 跨文件原子性、打包额外文件、多处 I/O | ❌ 不推荐 |

B2 与 C1′ 的关系：C1′ 的 `ocr_var` 素材/ROI/op_region/候选/同步机制全部保留，仅把「派生到 `meta.params`」改为「派生到 `variables[]`」（`ocr_var.rs:211` 改写入目标），`sync`/`T8` 骨架复用。**明确回答 task 问题「是否保留 meta.params 作为内部桥」：不保留。** 桥只存在于「批次 D 完成 → V2.0 落地」之间的现有 C1′ 代码；V2.0 一次性拆除（runtime/lint/loader 改读 variables），导入器只**读**旧 `meta.params` 生成 variables（§6.2），不写回。这样避免 B1 的双数组同步/T8 双份，符合 #13 的「抽成函数（声明一次、全链路复用）」。

### 2.2 `meta.json`（V2 最终态）

```json
{
  "schema_version": 1,
  "name": "切换预设御魂",
  "variables": [
    { "key": "group", "type": "string", "value_source": "ocr_text",
      "asset_id": "O_预设分组", "default": "御魂",
      "label": "预设分组", "description": "游戏内预设分组名", "group": "识别文字",
      "runner_editable": true, "required": false },
    { "key": "max_rounds", "type": "int", "default": 3, "min": 1, "max": 99,
      "label": "刷本轮数", "description": "使用端可改", "group": "运行参数",
      "runner_editable": true },
    { "key": "account_tag", "type": "string", "enum": ["主号", "小号"], "default": "主号",
      "label": "账号标记", "runner_editable": true },
    { "key": "battle_count", "type": "int", "scope": "runtime", "default": 0,
      "runner_editable": false }
  ]
}
```

| 字段 | 规则（复用现有校验器） |
|---|---|
| `key` | `is_param_key` 语法（`meta.rs:263`；`[A-Za-z_][A-Za-z0-9_]*` ≤64）；任务内唯一 |
| `type` | `int/float/number/bool/string`（`PARAM_TYPES`，`meta.rs:39`）；`enum` 同现有规则（`meta.rs:74`，bool 不支持 enum） |
| `value_source` | `literal`（默认）\| `ocr_text`；`ocr_text` 必须带 `asset_id` 且素材存在、kind=`ocr_var`、`param_key==key`；`literal` 不得带 `asset_id` |
| `scope` | `input`（默认，使用端可见可改）\| `runtime`（flow 局部，使用端不可见）；`runtime` 强制 `runner_editable=false`、`value_source=literal` |
| `default` | 类型匹配 + min/max/enum 校验（复用 `check_param_type`/`validate_param_values`，`meta.rs:297,383`）；`ocr_text` 的 default 由素材 `runtime_text` 同步（T8） |
| `label/description/group` | 长度上限复用 `meta.rs:103-108`（80/500/80） |
| `runner_editable` | 缺省 `true`（input）/`false`（runtime）；仅「是否出现在使用端配置 UI」；不影响运行（仍可被 entry/CLI 覆盖，调试用） |
| `required` | 语义同现 `ParamDecl.required`（`required && default==None` 时使用端应填；仅 UI 提示 + 运行前告警，不做硬阻断 V2.0） |

兼容字段：`meta.params` 在 V2.0 起**不再写出**；读取时若存在（旧任务/旧包）由导入器转 `variables`（§6.2），无转换则 lint 报 T10「legacy params present」。

### 2.3 `ocr_var` 素材字段（相对 seq124 的调整）

```json
{ "id": "O_预设分组", "kind": "ocr_var", "runtime_text": "御魂", "param_key": "group",
  "label": "预设分组", "roi": [40,300,400,60], "threshold": 0.6,
  "action_shape": {"kind":"rect","roi":[30,290,420,80]},
  "op_region": {"mode":"self"}, "positions": [12], "rev": 3 }
```
- 保留 seq124 全部字段与语义（`ENGINE §22.64.2`：runtime_text ≤200、param_key 只读、roi 可空=全屏、op_region 三模式/链/防环、T5 校验 `lib.rs:423`）。V2 仅约定：`runtime_text` = **变量 default 的唯一来源**；`param_key` = 变量 key；素材行编辑「识别文字」= 改 default（Runner 值不受影响）。
- 不新增字段、不改 `assets.json schema_version`；素材仍全局唯一，跨任务共享（值在任务级，天然隔离）。

### 2.4 `values.json`（V2.0/V2.1 零改动；V2.2 见 §4）

```json
{"schema_version":1,"updated_ms":176...,"tasks":{"切换预设御魂":{"values":{"group":"结界","max_rounds":5},"updated_ms":176...}}}
```
键 = 变量 `key`（ocr_text 变量 key=`param_key`，旧值天然命中），`runner_config.rs:46-71` 结构不变；写路径 `set_task_values`（`runner_config.rs:141`）不变。使用端个性化在 V2.0/2.1 仍是「每任务一套值」；Profile 在 V2.2 把 `tasks` 移入 `profiles.<id>.tasks`。

### 2.5 发布 manifest（`oas-task/src/package.rs`）

- `Manifest`（`package.rs:121-139`）：`params_schema: ParamsSchema`（`171-177`）→ **`variables_schema`**：
```json
"variables_schema": {
  "own": [ {"key":"group","type":"string","value_source":"ocr_text","asset_id":"O_预设分组",
            "default":"御魂","label":"预设分组","runner_editable":true} ],
  "call_deps": [ {"child":"任务1","mapping":{"触发文字":"识别关键词"},"inherit":"all",
                  "child_variables":["触发文字"]} ]
}
```
- `package_schema` 1→2（`package.rs:52`；旧包拒绝，§6.3）；`min_engine=ENGINE_VERSION`（`package.rs:1476`）仍为旧 Runner 门禁；`engine.flow_ops` 不变（flow 语法未变，`SUPPORTED_FLOW_OPS` 仍 13，`package.rs:81-95`）；`asset_refs` 不变。
- `variables_schema.own` 与原 `params_schema.own` 一样是**快照不是第二权威**（安装后以 `tasks/<id>/meta.json` 为准）。

### 2.6 子任务 / 调度条目的引用方式

- **flow 语法零改动**：`call_task.args.params`（子变量键 ← 父表达式）保留原字段名，loader 展开 `with_params`（`oas-task/lib.rs:1207-1430`）时把「子任务 `meta.params[].default`」改为「子任务 `variables[scope=input].default`」；`inherit:"all"/"none"`、T7 父/子键校验（`lib.rs:3691-3692`）改为 variables 键集。
- 调度条目：`scheduler.json entries[]` 新增可选 `profile`（V2.2）：`RawEntry`（`scheduler.rs:1049-1072`）/`Entry`（`scheduler.rs:706-722`）/`validate_entry`（`:1448`）/`to_json`（`:875-960`）各加一个 `Option<String>` 字段；**queue 成员共享 entry 的 profile**（不做 per-member，V2.2 后置）。
- 运行入口：`RunOptions`（`run.rs:94-154`）新增 `profile: Option<String>`（V2.2）；Studio 试运行继续保持「不读使用端 values/profile」（现状语义不变）。

## 3. 使用端 Runner（安装后「使用端配置」只显示可修改变量）

### 3.1 配置 API（`GET/PUT /runner/tasks/<id>/config`）

- GET（`serve.rs:10750` `handle_runner_task_config_get`）响应扩展为：
```json
{"ok":true,"task":"切换预设御魂",
 "variables":[{"key":"group","type":"string","value_source":"ocr_text","asset_id":"O_预设分组",
   "default":"御魂","label":"预设分组","description":"…","group":"识别文字",
   "runner_editable":true,"required":false}],
 "values":{"group":"结界"},"updated_ms":176...,"profile":"default"}
```
- PUT（`serve.rs:10785` `handle_runner_task_config_put`）请求 `{values:{...}}` 不变；校验从 `validate_param_values(&meta.params)` 改为 `validate_variable_values(&meta.variables)`；**只接受 `runner_editable=true` 且 `scope=input` 的键**，其余键 400（`values.runtime_var: 不可由使用端修改`）。
- `GET /task/config`（Studio，`serve.rs:10833` `handle_task_config_get`）在 V2.0 改为 `GET /task/variables`（或保留路径、返回 `variables`；推荐保留路径减改 UI 调用），Studio 仍可读内部全量（诊断）。
- Runner 侧不返回 runtime 变量 / 素材明细（只给变量声明+值），值持久化仍走 `runner_config::set_task_values`（`runner_config.rs:141-157`，单写者+原子写）。

### 3.2 UI（`runner.html:522-543` 配置弹窗 + `runner.js:2389` 渲染 / `2527` 保存）

- 分节：①「识别文字配置项」= `value_source=ocr_text` 的变量（标题/短标沿用 `runnerParamGroupsEx`/`runnerParamRoleText`，`runner.js:464-524`，已被 seq124 实现）；②「使用端变量」= `literal` 且 `runner_editable` 的变量，按 `group` 分组（空组=「通用」）；③ 不渲染 `scope=runtime`/`runner_editable=false`；④ 无变量时保留空态文案「该任务没有使用端可配置的变量」。
- 行：`label`（缺省 key）+ `description` + 默认值 hint（`runnerParamDefaultText`，`runner.js:451`）+「使用」开关 + 输入控件按 type/enum；**留空 = 不注入**（沿用 `runnerParamUseState`/`runnerParamCollectUse`，`runner.js:539-616`）；`required` 且当前档案无值 → 行内橙色提示 + 保存时允许（运行前告警）。
- 与现有「使用端OCR文字配置项」分节关系：**同一机制的两个分节**，OCR 分节在前（保持用户已熟悉的位置），标量在后；替代批次 D 删掉的「通用参数组/高级 JSON 参数」入口。
- 临时运行（`/runner/run` body params，`serve.rs:9238-9300`）/调度 entry params（`schedule.rs:3923-3942`）行为不变：仍可作为最高优先覆盖。

## 4. Profile（多账号 / 多配置 / 多模拟器）

### 4.1 定义

Profile = **命名变量值集合** + 展示信息（名称/账号标记）+ 设备绑定（V2.2 元数据与运行前守卫）。
- 多账号：一个 Profile 一套任务变量值（如「主号」「小号」）。
- 多配置：同一任务不同策略参数（如「速刷」「稳定」）。
- 多模拟器：Profile 绑定设备（serial/label）；V2.2 采用**一个 Runner 实例服务一个设备/Profile**的隔离部署（见 4.6）。

### 4.2 存储方案对比

| 方案 | 做法 | 结论 |
|---|---|---|
| **(a) `config/values.json` v2 扩展（推荐）** | 加 `profiles{}` + `active_profile`；复用原子 RMW/单写者/.bak；`.gitignore:27-29` 已排除该文件 | ✅ 推荐 |
| (b) `config/profiles/*.json` 目录 | 每档案一文件 + 索引 | 多文件事务/一致性问题，无收益 |
| (c) 复用 scheduler 多配置 | 借调条目表达档案 | 语义错位；调度与账号无关 |

### 4.3 `values.json` v2（V2.2）

```json
{ "schema_version": 2, "updated_ms": 176..., "active_profile": "default",
  "profiles": {
    "default": { "name": "默认", "account": "", "device": null,
      "tasks": { "切换预设御魂": { "values": {"group":"御魂"}, "updated_ms": 176... } } },
    "p_acc1": { "name": "账号1", "account": "主号",
      "device": {"serial":"127.0.0.1:16384","label":"MuMu"},
      "tasks": { "切换预设御魂": { "values": {"group":"结界","max_rounds":5}, "updated_ms": 176... } } }
  } }
```
- 合并优先级：`profile.values[task]` 覆盖变量 default（第 3 层）；`entry.params`（2）与 CLI（1）仍在其上；`entry.params` 不写回 values。
- 迁移（v1→v2）：Runner 启动/首次写入时读 v1 `tasks` → 内存映射 `profiles.default.tasks` → 写 v2 + `values.json.bak`；旧 Runner 读 v2 会因 `schema_version` 不符响亮失败（用户已允许不兼容）。
- 值存储粒度：**按 (profile, task, variable)**；同一任务在不同档案不同值，天然实现「使用端个性化」。

### 4.4 继承 / 覆盖规则

- **不做运行时继承链**（避免「档案 A 继承 B」的隐式复杂度）：每个档案扁平存储自己的覆盖；未覆盖的变量回落到任务 default。
- UI 提供「复制自…」创建（拷贝完整值集合）与「与默认档案对比」只读视图；删除档案前提示「已写入的值将丢失」。
- `default` 档案不可删除（v1 迁移基线）。

### 4.5 Profile 选择机制

| 入口 | 选择方式 |
|---|---|
| Runner UI 全局 | 顶部/「设置→配置档案」的**活动档案**选择器（`active_profile`）；「使用端配置」与「临时运行」默认用活动档案 |
| Runner 临时运行 | body 可选 `profile`（缺省=活动档案）；`/runner/run` 用它读 values |
| 调度条目 | `scheduler.json entries[].profile`（V2.2；缺省=活动档案）；plan/attempt 通过 `attempt_run_params(opts, task, entry)`（`schedule.rs:3923`）注入对应档案值 |
| CLI | `oasd run --profile <id>`（缺省=活动档案） |
| Studio | 不适用（编辑端只读 default；试运行不读档案） |

### 4.6 设备 / 账号绑定与多实例

- `profile.device = {serial, label}` 为**元数据 + 运行前守卫**：Runner 运行前校验「活动/条目档案的 serial == 当前连接设备 serial」，不符 → 409/400（可在设置关闭守卫）。设备列表复用 `GET /instance/devices`（Runner 已有）。
- `account` 为自由文本标签（V2.2 不做游戏账号自动识别）；运行记录/日志 optionally 记录 `profile`（审计可选）。
- **多模拟器并发结论**：V2.2 采用「**一 Runner 实例 = 一设备 = 至多一个活动 Profile**」隔离部署（多开 Runner 进程，各自 `--config` 指向不同 config 目录），因为输入/设备租约是进程级单设备：`InputSink` 为单设备 trait（`crates/oas-types/src/lib.rs:46`；ADB 实现 `crates/oas-adapter-pc/src/lib.rs:278,304,318`）、设备租约按 `config/` 目录单锁（`apps/oasd/src/device_lease.rs:68,85,235`）。单 Runner 多设备并发需要设备租约/输入层多路重构 → 明确列为 V2.3+ 后置，不在本设计内。

### 4.7 UI 入口

- Runner 新增「配置档案」管理：列表（名称/账号/设备/更新时间/活动标记）、新建/复制/重命名/删除/激活；入口放「设置」页区块（`runner.html:339-380` 设置页）或「已安装任务」页头选择器；每任务「配置」弹窗顶部显示当前档案并可切换。
- 调度条目表单（`runner.js:2995-3060` `schedEntryToForm`、`4687/4729` params 文本域）加「档案」下拉（选项=档案列表，含「默认」）。
- 新增 Runner API（V2.2）：`GET /runner/profiles`、`POST /runner/profiles`、`PUT /runner/profiles/<id>`、`DELETE /runner/profiles/<id>`、`POST /runner/profiles/<id>/activate`（全部 Runner-only 路由，加入 `route_owner`，`serve.rs:153-226`）。

## 5. 编辑端 Studio（创建变量 / 在 flow 中引用 / 发布校验）

1. **变量面板（新）**：任务库/构建页新增「变量」面板（替换批次 D 删除的「配置项/参数面板」）。列表列：key、类型/来源（识别文字/文本/整数/布尔/枚举/运行变量）、默认值、label/分组、使用端可改、引用计数；操作：新建/编辑/删除（被 flow/素材引用时拒绝或提示）、「插入到表达式」（复用现有插入逻辑 `builder.js:2556-2683`）。
2. **新建流程**：选类型 → 识别文字变量则内部复用「识别文字配置项」对话框（`app.js:5280` `openAssetTextDialog`；新建 ocr_var 素材或绑定已有素材，`param_key` 自动生成/迁移后只读）；标量变量直接表单（key/type/default/min/max/enum/label/description/group/runner_editable）。保存走 `POST /task/variables`（内部复用 `Meta` 校验+原子写；V2.0 起不再保留用户侧 `/task/config` 参数编辑，见批 D）。
3. **flow 引用**：① OCR 目标 = 素材（`builder.js:3084 actionTargetEditor`，现有 `ocr_var→text_expr:"param('<key>')"` 逻辑不变）；② 表达式字段「插入变量 ▾」= 原「插入参数」（`builder.js:2594-2641`）改文案，列出 input 变量（ocr_text 显示素材名）；③ 条件编辑器（批 D 改为「识别文字配置项」后）V2.1 扩展为「变量」下拉（`builder.js:3641-3774`）；④ loop/goal 数量 V2.1 可引用 int 变量（批 D 先把 goal param 改字面量，V2.1 再放开为变量）。
4. **发布校验**（`oas-task` lint）：
   - **T3** 改：`param('key')`（`flow.param_uses_at`，`oas-flow:2956`）必须声明于 `variables`（`scope=input`）；旧 `meta.params` 不再算声明（T10 legacy 报错）。
   - **T8** 保留：`ocr_text` 变量 ↔ `ocr_var` 素材 `runtime_text/param_key/asset_id` 一致（现 `ocr_var.rs:361` 逻辑改为对 variables；同一 sync 单一写者）。
   - **T9（V2.1）**：`var('name')` 读取必须声明为 `scope=runtime` 变量（或 flow 内先 `set_var/ocr_to_var` 写）；未声明 → lint 错误。
   - **T7** `call_task` 映射键 ⊆ 子任务 variables 键（现 `lib.rs:3691-3692` 改键集）。
5. **与批次 D 的过渡约定**：D 只删**用户侧参数入口**与文案（ACCEPTANCE 行 13），保留后端 `meta.params` 派生模型/`/task/config`（内部诊断）/同步函数/Runner 分组渲染基础；V2.0 用变量面板 + 变量 API 替换入口，不返工 D 删掉的东西。D 的条件编辑器「识别文字配置项」在 V2.1 直接扩成「变量」下拉，避免二次改造。
6. **命名**：用户可见统一「变量」；识别文字类显示「识别文字变量」；「参数」仅历史文档/内部代码名保留（不含 UI）。

## 6. 迁移策略

### 6.1 现状 → V2 映射（一次性）

| seq124 现状 | V2 归属 |
|---|---|
| `meta.params[]` 派生行（`asset_id`，`ocr_var.rs:112`） | `variables[]`：`value_source=ocr_text`、`asset_id`、`key=param_key`、`default=runtime_text`（sync 改写入目标） |
| `meta.params[]` 用户手写行（无 `asset_id`） | `variables[]`：`value_source=literal`（`type/default/min/max/enum/label/description/group/required` 全保留） |
| `meta.params[] role=ocr_text` 且无 asset_id（旧任务） | 可导入为 `ocr_text` 变量（需建 ocr_var 素材，重演 `migrate_params_to_ocr_vars`，`ocr_var.rs:276`）；用户简化令允许不迁移 |
| `values.json` | key 不变 → **V2.0/2.1 零迁移**；V2.2 包进 `profiles.default.tasks` |
| ocr_var 素材 / op_region / 候选 / test_action | 全保留，变量只引用 |

### 6.2 迁移选项与推荐

| 方案 | 做法 | 结论 |
|---|---|---|
| M1 导入器 | 纯函数 `variables_from_params(meta, assets)` + 可选 dev CLI（`oasd task import-variables --dry-run`），无 UI/无自动写 | ✅ 作为开发工具保留（有单测） |
| M2 双数组过渡 | 读写两段 + 同步 | ❌ 双真相（B1 已否） |
| M3 不迁移 | 用户重建测试任务（用户原话「当前配置都是测试配置，并不重要」） | ✅ **面向用户的主路径**：不自动改任何用户数据 |

推荐：**M3 为主 + M1 作为内部/开发工具**；V2.0 前一次性 `cargo test`/E2E 用新模型重建任务，不承诺旧任务可跑（与简化令一致）。

### 6.3 旧包 / 旧 Runner 兼容口径

- **新引擎拒绝旧包**：`package_schema` 1→2（`package.rs:52`；安装校验 `:1746`），旧包 400 明确提示「需按 V2 重新发布」；不做旧包兼容读取（用户允许不兼容，避免 `params_schema`↔`variables_schema` 双读代码）。
- **旧 Runner 装新包**：`min_engine`（`package.rs:1476`）+ 新 `package_schema` 双门禁；flow 语法未变（`flow_ops` 仍 13）不影响。
- **flow 文本兼容**：`param()` 函数名、`call_task.args.params`、`text_expr` 写法全部不变 → 已发布 flow 内容无需改写（只有包级 schema 变化）。
- `values.json`：V2.0/2.1 完全不变；V2.2 升 v2，旧 Runner 读 v2 响亮失败（可接受）。

### 6.4 回滚与数据安全

- 任何代码迁移都不自动改写 `tasks/`、`assets/`、`config/values.json`；导入器默认 dry-run、写前 `.bak`（沿用 seq124 口径）。
- V2.2 values v1→v2 在 Runner 首次写时转换，`values.json.bak` 保留；失败=响亮错误不静默重置。

## 7. 分阶段实施计划（V2.0 / V2.1 / V2.2）

> 每阶段独立可交付、可回退；测试基线：`cargo test --workspace` 起 **904/0**（批次 A 后只增）、UI pure test 起 **2537 checks/194 ids/0 dead/6 nav**（本轮实测）；UI 改动需 Firefox 手测（ACCEPTANCE 行 46）。

### V2.0 — 变量模型统一 + Studio/Runner UI + 发布/安装（L）

- **改动点**：`crates/oas-task/src/meta.rs`（新 `Variable` 类型/`validate_variable_values`/`variable_defaults`，复用 `ParamDecl` 校验器 `:297,383,452,464`）；`crates/oas-task/src/ocr_var.rs`（`sync_ocr_var_params` 改写 variables，`:211`；T8 `:361`）；`crates/oas-task/src/lib.rs`（T3 `:3071-3087`、T8 `:3129`、loader 子任务默认 `:1207-1430`、T7 `:3691`）；`crates/oas-task/src/package.rs`（`variables_schema`、`package_schema=2`、`:121-190,1457-1504`）；`apps/oasd/src/run.rs`（注入源改 variables，`:616-665`）；`apps/oasd/src/serve.rs`（`/task/variables`、Runner config GET/PUT `:10750/10785`、publish）；UI：Studio 变量面板（`app.js`）、builder 插入变量（`builder.js:2556-2683`）、Runner 可编辑过滤（`runner.js:464-524,2389-2527`）。
- **验收**：Rust 单测（variables 解析/校验/T3/T8/导入器/子任务默认/T7/包 schema）+ serve 契约测试；UI pure test 新增变量面板断言；E2E：新建 ocr_text 变量+标量变量 → flow 引用 → lint 0 → publish/install → Runner 仅显示可改变量 → 改值运行读到新值；`params` UI 全程不可见；旧包/旧值行为符合 §6。
- **风险**：T3/loader/run 注入同时改，回归面大（用 T3 全量测试 + E2E 兜底）；package_schema bump 使 dev 旧包不可装（可接受）。

### V2.1 — 标量变量表达/条件/运行变量声明（M）

- **改动点**：`oas-flow`：新增 `variable('key')` 别名（`eval_call`，`:488-520`；旧 Runner 由 min_engine 门禁），条件/循环/目标参数表达式接入（`Step::If/Loop/While` 已有表达式）；`oas-task` lint 新 **T9**（`var()` 声明/先写后读，`lib.rs` lint 段）；Studio：条件编辑器变量下拉（`builder.js:3641-3774`）、goal/loop 数量变量（`serve.rs` goal 处理）；Runner：int/enum 控件与 required 提示完善。
- **验收**：T9 单测；条件/循环用变量 E2E；`variable()`/`param()` 等价单测；UI 断言。
- **风险**：`variable()` 是新表达式函数（旧 Runner 解析失败=响亮，且 min_engine 已挡）；T9 可能误报存量 flow（提供只警告开关一版）。

### V2.2 — Profile / 多实例管理（L）

- **改动点**：`apps/oasd/src/runner_config.rs`（values v2 `profiles`/`active_profile`，`:46-71,98-157`）；`apps/oasd/src/scheduler.rs`（`RawEntry`/`Entry`/`validate_entry`/`to_json` 加 `profile`，`:1049-1072,706-722,1448,875-960`）；`apps/oasd/src/schedule.rs`（`attempt_run_params` 按档案读 values，`:3923-3942`）；`apps/oasd/src/serve.rs`（`/runner/profiles*`、`/runner/run` profile 参数 `:9238`、设备守卫）；`apps/oasd/src/run.rs`（`RunOptions.profile`，`:94-154`）；Runner UI（档案管理页/设置区块 `runner.html:339-380`、任务配置弹窗顶部档案选择、调度条目档案下拉 `runner.js:2995-3060,4687/4729`）；`packaging/README-RUNNER.txt` 多实例部署说明。
- **验收**：values v1→v2 迁移单测（含 .bak/坏文件）、档案 CRUD/激活、`/runner/run` 按档案取值、调度 entry.profile、设备守卫、E2E 两个档案同任务不同值；旧 Runner 读 v2 响亮失败验证。
- **风险**：值切换导致「同一任务不同档案」误跑（用运行日志记录 profile + 守卫缓解）；多实例部署需用户手工多开（文档化）；queue per-member profile 后置。

## 8. 与本轮批次 A~E 的衔接

| 批次 | 与 V2 的关系 | V2 预留/要求 |
|---|---|---|
| A（#5 阈值 / #9 replace 继承 / #11 数字保护） | 无变量语义变化；#9 修 replace 丢 `op_region` 与 variables 无关 | 不动；V2 保留 ocr_text default=runtime_text，不触碰 #9 修复 |
| B（#4 操作区模型 A / #10 统一框选） | op_region 与变量无关，ocr_var 仍作链节点 | 不动 op_region schema；V2.0 不改素材字段 |
| C（#3 候选口径统一） | 变量值就是 OCR 期望文字来源（`ocr_var_expected_text`：注入参数>runtime_text，`ENGINE §22.64.3`） | V2 保持「使用端值优先、素材默认兜底」同一口径，不新增第二 expected-text 路径 |
| **D（#1/#2 删除参数 UI）** | **关键衔接**：D 删除用户侧「参数」入口后，V2.0 变量面板是替代入口 | ① D 保留 `meta.params` 派生模型/`/task/config`（内部）/同步函数/Runner 分组渲染骨架（`runnerParamGroupsEx` 等，`runner.js:464-524`）与 ocr 派生分节，**不删基础设施**；② D 不新增「变量」占位 UI（V2.0 直接建）；③ D 条件编辑器改「识别文字配置项」（ACCEPTANCE 行 12）V2.1 扩为变量下拉，不二次返工；④ D 删「高级 JSON 参数」后，V2.2 档案值不经 JSON 文本域，避免回潮 |
| E（运维 #7/#8/#12） | 无交集；V2.2 的 `config/` 数据仍机器本地（`.gitignore:27-29`） | V2.2 变更 `packaging/README-RUNNER.txt` 多实例部署段可并入 E 的文档更新；发行包不加新依赖 |

另外：批次 A 已提交（`c16a536`），B/C 随后；V2 实现必须排在 **D（参数 UI 清理）之后**，避免与 D 的 `builder.js/app.js/runner.js` 写作用域冲突；V2.0 任务写作范围见 §7。

## 9. 待拍板项（少而关键，给推荐）

| # | 问题 | 推荐 |
|---|---|---|
| 1 | Schema：B2 直改（variables 唯一，params 退役，= Phase 2 合并）vs B1 双段 vs A 仅改名 | **B2**（无过渡双真相；用户已允许不兼容旧数据/旧包） |
| 2 | flow 读取函数：V2.0 保留 `param()` 不变、V2.1 加 `variable()` 别名 vs 立即改名 | **保留 `param()`**（flow 零改动），V2.1 加别名并文档推荐新写法 |
| 3 | 旧包：新 Runner 拒绝 `package_schema=1`（重新发布）vs 兼容读取 | **拒绝**（干净；旧包兼容成本高且用户已说不重要） |
| 4 | Profile 存储：`values.json` v2（推荐）vs 独立 profiles 文件 | **values.json v2**（复用原子写/单写者/忽略规则） |
| 5 | Profile 粒度：任务级值 + entry/全局选择（推荐）vs queue 成员级 | **任务级 + entry 级**；per-member 后置 |
| 6 | 多模拟器：一期「一 Runner 一设备/Profile」隔离部署（推荐）vs 单 Runner 多设备并发 | **隔离部署**；单进程多设备列为 V2.3+（需租约/输入层重构） |
| 7 | `runner_editable` 默认：input=true、runtime=false；required 仅提示不硬阻断 | **按此**（V2.0 UI 提示，运行前告警；硬阻断留待用户反馈） |
| 8 | 值优先级：CLI>entry>Profile>default（不变） | **保持四层**（与 `RUNNER §C.3` 一致） |
| 9 | 旧数据：M3 用户重建为主 + M1 导入器仅内部工具 | **M3+M1**（不自动迁移、不提供用户侧迁移 UI） |
| 10 | 命名：用户可见统一「变量」；OCR 类型「识别文字变量」；「参数」仅历史文档 | **按此**（与批 D 删「参数」文案一致） |

## 附：关键代码锚点（现场核实，实施时按函数名）

- 模型/校验：`meta.rs:39`（PARAM_TYPES）、`:54-100`（ParamDecl）、`:263`（is_param_key）、`:297`（check_param_type）、`:383`（validate_param_values）、`:452/464`（declared_param_keys/param_defaults）、`:476`（lint_issues）。
- ocr_var：`lib.rs:61`（KINDS）、`:423`（validate_ocr_var_fields）、`:529`（validate_op_region_chain）；`ocr_var.rs:112`（derived_param）、`:152`（expected_derived_params）、`:211`（sync_ocr_var_params）、`:276`（migrate_params_to_ocr_vars）、`:361`（derived_param_issues/T8）。
- 运行时：`oas-flow:430-520`（EvalContext param/var/eval_call）、`:1174-1187`（SetVar/IncVar/OcrToVar）、`:2947-2960`（param_uses）、`:2968`（call_task_uses）；`runtime:1843/1961/6442`（vars/with_vars/var()）、`:6127-6164`（变量写入）、`:659`（ocr_var_expected_text）、`:4344`（ocr_read_var）、`:1466`（candidates_for_asset）、`:2789`（resolve_asset_op_region）、`:2860`（preview_asset_action）、`:3101`（walk_op_region_chain）。
- 注入/配置：`run.rs:94-154`（RunOptions）、`:355`（runner_values_layer）、`:616-665`（唯一合并点+复检）、`:861`（with_params）；`runner_config.rs:36-71`（values 结构）、`:98/118/141`（read/write/set）；`schedule.rs:3911/3923/3940`（values 路径/attempt_run_params/entry_run_params）；`scheduler.rs:706-722`（Entry）、`:1049-1072`（RawEntry）、`:1448`（validate_entry）、`:875-960`（to_json）。
- 发布/门禁：`package.rs:52`（PACKAGE_SCHEMA）、`:81-95`（SUPPORTED_FLOW_OPS）、`:121-190`（Manifest/ParamsSchema）、`:606`（derive_flow_ops）、`:1457-1504`（build manifest）、`:1476`（min_engine）、`:1746`（install package_schema 校验）。
- serve/UI：`serve.rs:153-226`（route_owner）、`:8458`（/task/run）、`:9238`（/runner/run）、`:10750/10785`（runner config GET/PUT）、`:10833/10872`（/task/config GET/POST）、`:7080`（save_flow）；`runner.js:451/464-524/539-616/2174/2389/2527/2995/4687/4729`；`runner.html:28-37/339-380/410-460/522-543`；`builder.js:186/2556-2683/3084/3641-3774`；`app.js:1907/1988/2050`（配置项对话框）、`:5280`（识别文字对话框）。以上为 18:4x 实测；工作区并行批次会继续漂移，实施时按函数名重新定位。
