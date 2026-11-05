# OAS 部署说明（Studio 编辑端 / Runner 使用端）

> 仓库产出**两种发行包**，面向两类人：
> - **Studio（编辑端）**：开发/编辑人员使用，含编辑器 UI（工作台、任务构建、素材、帧库），端口 **8788**，启动脚本 `start-studio.bat`（`start-editor.bat` 保留为兼容别名）。
> - **Runner（使用端）**：终端用户使用，只含运行 UI + OCR 模型 + adb，端口 **8789**，启动脚本 `start-runner.bat`；用于安装 `.oaspkg` 任务包、填写任务配置、排队/调度/执行。**Runner 包不包含 Studio 编辑 UI；终端用户只需要下载 Runner 包 + 任务包。**
>
> 目标环境：Windows 10/11 x64；Android 模拟器（MuMu 12）或 Android 真机（adb）；阴阳师 Android 客户端。

## 两种发行包（必读）

| 维度 | Studio 编辑端 | Runner 使用端 |
|---|---|---|
| 发行包 | `oasd-v<version>-win64.zip` | `oasd-runner-v<version>-win64.zip` |
| 发放对象 | 开发 / 编辑 / 发布任务的人 | **终端用户**（只下载这个 + `.oaspkg`） |
| HTTP 入口 | `http://127.0.0.1:8788/`（`ui/index.html`） | `http://127.0.0.1:8789/`（`ui/runner.html`） |
| 命令 | `oasd studio --port 8788 ...`（`oasd serve` = 兼容别名） | `oasd runner --port 8789 ...` |
| 启动脚本 | `start-studio.bat` / 兼容 `start-editor.bat` | `start-runner.bat` |
| 停止脚本 | `stop-studio.bat` / 兼容 `stop-editor.bat` | `stop-runner.bat` |
| UI 资源 | `index.html` / `app.js` / `builder.js` / `flow_model.js` / `asset_ui.js` / `dashboard.*` | 仅 `runner.html` / `runner.js` / `runner.css` + 共享 `style.css` |
| 能做什么 | 创作帧/素材/流程、lint、发布 `.oaspkg`、试运行 | 安装/卸载任务包、填运行参数、排队/调度、执行、看运行记录 |
| 不能做什么 | 不是给终端用户的操作入口（不需要安装任务包） | **没有编辑 UI**：不能改流程/素材/帧库；这些在 Studio 完成并发布 |
| 数据目录 | tasks/ assets/ frames/ library/ runs/ logs/（不含 `scheduler.json`/调度脚本；调度归 Runner） | tasks/ assets/ packages/ runs/ logs/ + `scheduler.json`（不含 frames/ library/） |

两种包共用同一个 `oasd.exe` 二进制，只是启动的子命令、端口和随包 UI 不同。单机部署时可以并存，但**不要同时对同一台设备执行**（Runner 调度与 Studio 试运行会抢设备）。

## 0. V0.1.0 新增

- **帧持久化**：帧图/名称/分类/顺序写入 `frames/frames.json` + `frames/frame-<id>.png`；重启编辑器后列表仍在；当前帧像素按需加载；
- **图片匹配建素材**：图识别结果可一键设为识别区域，调整操作区域后保存模板（`action_roi`）；
- **builder 结构化属性**：`click_target` / `drag` 提供结构化表单（定位方式/素材、方向/距离/时长），复杂场景保留高级 args JSON；
- **变量 checkpoint**：`set_var`/`inc_var`/`ocr_to_var` 变量随检查点保存，`--resume` 恢复；
- **旧素材清理**：`oasd assets cleanup-legacy [--apply]` 把旧任务 `assets.json/images` 归档到 `assets/_legacy-backup/`；
- **工作台流程面板删除**：流程编辑只在「任务构建」；工作台生成步骤改为复制到剪贴板。

## 0.1 V1.2.1 修复与统一

- **统一素材库**：所有任务共用一个 `assets/`（`assets.json` + `images/`）；不再分任务素材/全局素材；名称全局唯一；删除/改名会检查所有任务 flow 引用；
- **旧数据迁移**：`oasd assets migrate` 或素材面板「迁移到统一素材库…」；重名自动加 `_2/_3` 后缀，并按每个任务重写 flow 引用（迁移报告在 `assets/migration-report.json`）；
- **OCR 来源一致**：多行识别保存的行素材带 `ocr_mode:"lines"` + 原识别区域 + 原文字；运行时沿用 det+rec，不再降级成单行 rec；
- **批量保存可见性修复**：任务名输入/任务下拉统一 active task；批量保存后素材立即可见；
- **任务构建页**：动作属性顶部固定素材/操作区域选择器；三栏布局修复；真控制流程图（if 分支、loop/while 循环体+回边），不再是步骤复述；
- **工作台移除流程面板**：流程编辑统一在「任务构建」；工作台只做帧/识别/测试；
- **帧管理**：可重命名、自由文本分类、拖动排序、分组展示；移除 20 帧上限。

## 0.2 V1.2 内容

- 识别区域/操作区域统一、多选批量保存、全局唯一、任务构建页、变量动作 `set_var/inc_var/ocr_to_var`。

## 0.3 V1.1 内容

- 整屏多行 OCR 直接点击；素材 CRUD；独立区域操作/拟人拖动；任务工作台+完成条件；5 个页面；点击分布聚合+删除；editor.log。

## 1. 首次安装

### 1.1 Runner（终端用户；只装这个）

1. 解压 `oasd-runner-v<version>-win64.zip` 到例如 `D:\OAS-runner\`（**不要覆盖旧 OAS 目录**）；
2. 编辑 `config/live.json`，按实际修改：
   - `device.serial`：adb 设备 serial（如 `127.0.0.1:16384`）；
   - `device.adb_path`：默认相对路径 `adb/adb.exe`，可改绝对路径；
   - `runner.port`：默认 **8789**（不要与 Studio 的 8788 冲突）。注意 `start-runner.bat` 固定以 8789 启动；要换端口请直接运行 `oasd.exe runner --port <N> ...`（stop-runner.bat 也按 8789 停止）。
3. 先验证设备（可选）：
   ```bat
   oasd.exe probe --serial 127.0.0.1:16384
   ```
4. 双击 `start-runner.bat` → 浏览器打开 `http://127.0.0.1:8789/`；
5. 在 Runner「安装包」页填写 `.oaspkg` 的**本机绝对路径** → 安装 → 「已安装任务 → 配置」填运行值 → 「队列与调度」编辑 entry/queue、启动/停止调度或临时运行；
6. 日志在 `logs/runner.log`；停止用 `stop-runner.bat`（或关闭启动窗口）。

> Runner 包不包含任务构建、素材、帧库、流程编辑页面；这些属于 Studio。Runner 只消费 Studio 发布的 `.oaspkg`。
> 版本信息见包根 `VERSION` / `BUILD-INFO.txt`。

### 1.2 Studio（开发/编辑/发布；不发给终端用户）

1. 解压 `oasd-v<version>-win64.zip` 到例如 `D:\OAS-dev-live\`（**不要覆盖旧 OAS 目录**）；
2. 复制 `config/live.example.json` → `config/live.json`，按实际修改：
   - `device.serial`：adb 设备 serial（如 `127.0.0.1:16384`）；
   - `device.adb_path`：相对 `adb/adb.exe` 或绝对路径；
   - `serve.port`：默认 **8788**（**避免与旧 OAS 22287 / Runner 8789 冲突**）。
3. 先验证设备：
   ```bat
   oasd.exe probe --serial 127.0.0.1:16384
   ```
4. 校验任务包：
   ```bat
   oasd.exe task lint --tasks-dir tasks --all
   ```
5. 启动编辑器：双击 `start-studio.bat`（旧的 `start-editor.bat` 仍可用，会转调它）→ 浏览器打开 `http://127.0.0.1:8788/`。

> `oasd serve` 仍保留为 deprecated 兼容别名（等价 `oasd studio`），老脚本/快捷方式不会立刻失效。

## 2. 创建并运行任务（任务工作台，Studio）

1. 顶部「任务库」→「新建任务…」→ 填名称/模板/描述 → 创建；
2. 按工作台步骤：① 基础 → ② 画面/素材（框选/多行 OCR）→ ③ 素材操作（生成点击/区域点击/拖动）→
   ④ 流程 → ⑤ 完成条件（打 N 场，自动生成循环）→ ⑥ 测试运行 → ⑦ 保存/加入调度；
3. 多行 OCR：框选整屏/区域 →「多行识别」→ 在结果行点「生成点击步骤/追加到流程」；
4. 点「运行任务」→ 结果/日志窗口查看；点击审计在 `runs/`（点击分布页默认聚合）。

## 3. 调度 / 队列

1. **Runner 使用端负责调度**：Runner 内嵌调度线程（单进程，无需再双击 `start-scheduler.bat`）——`start-runner.bat` 启动后自动开始跑 `scheduler.json`；「队列与调度」页可启动/停止、临时运行，并完整编辑条目/队列（生命周期、窗口、quota、retry/cooldown、params、启用/排序）、查看运行记录与重置记忆；高级 JSON 编辑仍保留。旧的 `start-scheduler.bat`（headless CLI）保留给无 UI 场景；两者共用 `<state>.lock`，**天然互斥**，不要同时跑。日志 `logs/runner.log` + `logs/scheduler.events.jsonl`（Runner 包根目录下）。
2. 调度计划仍是 `scheduler.json`（schema v4；v3 读取时自动升级）：
   - `periodic`：窗口 + 每日/每周 quota；
   - `temp`：临时任务，每轮按 entries 顺序跑一次，执行前后重置，无完成记忆；
   - `queues`：队列成员串行、整轮计次（规则见 ENGINE §22.34）。
3. headless / CI 仍保留独立 CLI（兼容）：
   ```bat
   start-scheduler.bat
   ```
   它和 Runner 内嵌调度共用 `<state>.lock`，**天然互斥**：Runner 在跑时不要再启动 `start-scheduler.bat`。
4. 重置记忆：用 CLI（`oasd.exe scheduler reset-memory --tasks 探索,个人突破 --include-checkpoint`）；
   Runner 页面不提供重置入口（避免误清使用端运行记忆）。

## 4. 停止 / 升级 / 回滚

- 停止：Studio 用 `stop-studio.bat`（兼容 `stop-editor.bat`）；Runner 用 `stop-runner.bat`；旧 headless 调度用 `stop-scheduler.bat`；
- 升级：
  1. 停止对应进程；
  2. 备份数据：
     - Runner：`config/ tasks/ packages/ scheduler.json runs/ logs/` → `_backup/`；
     - Studio：`config/ tasks/ library/ assets/ frames/ scheduler.json` → `_backup/`；
  3. 解压新 zip 覆盖**程序文件**（exe/ui/models/adb + 启停脚本），保留数据目录；
  4. 启动后执行 `task lint --all` + `probe`（Studio），或打开 Runner 健康/任务列表确认；
- 回滚：恢复上一版程序文件 + `_backup/` 数据。

## 5. 注意事项

- **Runner 包没有编辑 UI**：终端用户不要用 Runner 改流程/素材；制作任务请用 Studio 发布 `.oaspkg`；
- Studio 默认 8788，Runner 默认 8789，二者都只绑 `127.0.0.1`，不要暴露公网；
- 同一台模拟器/真机同一时间只能被一个自动化进程控制：跑 Runner 调度时先停 Studio 试运行、旧 OAS 调度和 headless scheduler；
- 未签名 exe 可能触发 SmartScreen：更多信息 → 仍要运行（正式分发建议签名）；
- 任务/素材/任务包是纯数据，可整目录复制；`assets/images/` 与 `assets.json` 不要手工改名；
- macOS/Linux 没有 `.bat`：直接 `./oasd runner --port 8789 --config config/live.json --ui ui`（UI 是 Web）；Android 使用端尚未实现，设计约束见源码仓库 `packaging/CROSS-PLATFORM.md`。
