# OAS V1.2 部署说明（实机 / 模拟器）

> 本包 = `oasd.exe` + 编辑器 UI + OCR 模型 + adb + 配置 + 启停脚本；数据在 `tasks/`、`library/`、`runs/`、`logs/`。
> 目标环境：Windows 10/11 x64；Android 模拟器（MuMu 12）或 Android 真机（adb）；阴阳师 Android 客户端。

## 0. V1.2 新增

- **识别区域 / 操作区域统一**：每个素材明确区分识别区域和操作区域（默认操作=识别）；操作区域可拖拽放大/移动，跟随模板匹配位置；纯操作区域 `kind=region` 可单独保存和复用；
- **识别结果多选批量保存**：整屏/区域多行 OCR 结果带复选框，名称默认识别文字、可改、**全局唯一**；可逐条调整操作区域，一次批量保存（全有或全无）；
- **全局唯一命名**：任务素材 + 全局素材库共用一个命名空间；新增/改名/批量保存都会校验；素材面板提供「全局重名扫描」；
- **任务构建大页面**：顶部「任务构建」——动作库（控制/识别/动作/数据/结束）+ 嵌套步骤树 + 动作属性值（素材/操作区域/时间/表达式）+ 实时流程图 + 保存/dry-run；工作台保留用于单帧测试；
- **变量动作**：`set_var` / `inc_var` / `ocr_to_var` + `var('name')` 条件表达式，可做计数/累计/OCR 结果判断。

## 0.1 V1.1 内容

- 整屏多行 OCR 直接点击；素材 CRUD；独立区域操作/拟人拖动；任务工作台+完成条件；5 个页面；点击分布聚合+删除；editor.log。

## 1. 首次安装

1. 解压整个 zip 到例如 `D:\OAS-live\oas-core\`（**不要覆盖旧 OAS 目录**）；
2. 复制 `config/live.example.json` → `config/live.json`，按实际修改：
   - `device.serial`：adb 设备 serial（如 `127.0.0.1:16384`）；
   - `device.adb_path`：相对 `adb/adb.exe` 或绝对路径；
   - `serve.port`：默认 8788（**避免与旧 OAS 22287 冲突**）。
3. 先验证设备：
   ```bat
   oasd.exe probe --serial 127.0.0.1:16384
   ```
4. 校验任务包：
   ```bat
   oasd.exe task lint --tasks-dir tasks --all
   ```
5. 启动编辑器：双击 `start-editor.bat` → 浏览器打开 `http://127.0.0.1:8788/`。

## 2. 创建并运行任务（任务工作台）

1. 顶部「任务库」→「新建任务…」→ 填名称/模板/描述 → 创建；
2. 按工作台步骤：① 基础 → ② 画面/素材（框选/多行 OCR）→ ③ 素材操作（生成点击/区域点击/拖动）→
   ④ 流程 → ⑤ 完成条件（打 N 场，自动生成循环）→ ⑥ 测试运行 → ⑦ 保存/加入调度；
3. 多行 OCR：框选整屏/区域 →「多行识别」→ 在结果行点「生成点击步骤/追加到流程」；
4. 点「运行任务」→ 结果/日志窗口查看；点击审计在 `runs/`（点击分布页默认聚合）。

## 3. 调度

1. 编辑器调度面板或直接编辑 `scheduler.json`（见 ENGINE §22.34.4）：
   - `periodic`：窗口 + 每日/每周 quota；
   - `temp`：临时任务，每轮按 entries 顺序跑一次，执行前后重置，无完成记忆；
2. 双击 `start-scheduler.bat`；日志 `logs/scheduler.log`，报告 `runs/scheduler-report.json`；
3. 重置记忆：编辑器「重置记忆」多选/全选/反选，或
   ```bat
   oasd.exe scheduler reset-memory --tasks 探索,个人突破 --include-checkpoint
   ```

## 4. 停止 / 升级 / 回滚

- 停止：`stop-editor.bat`、`stop-scheduler.bat`；
- 升级：
  1. 停止 editor/scheduler；
  2. 备份 `config/ tasks/ library/ scheduler.json` 到 `_backup/`；
  3. 解压新 zip 覆盖**程序文件**（exe/ui/models/adb），保留数据目录；
  4. 启动后执行 `task lint --all` + `probe`；
- 回滚：恢复上一版程序文件 + `_backup/` 数据。

## 5. 注意事项

- 同一台模拟器只能被一个自动化进程控制：跑本引擎时先停旧 OAS 的调度；
- 本引擎默认只绑 `127.0.0.1`，不要暴露公网；
- 未签名 exe 可能触发 SmartScreen：更多信息 → 仍要运行（正式分发建议签名）；
- 任务/素材是纯数据，可整目录复制；`images/` 与 assets.json 不要手工改名。
