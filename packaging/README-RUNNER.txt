OAS Runner 使用端（终端用户发行包）
=====================================

这个包只包含「使用端（Runner）」：安装任务包、填写任务配置、排队/调度、执行、
查看运行记录。它不包含 Studio 编辑端（没有编辑器 UI、没有任务构建/素材/帧库），
需要制作或修改任务时请使用单独的 Studio 发行包（oasd-v<版本>-win64.zip）。

版本能力说明：当前 Runner 版本提供运行入口、状态页（概览 / 已安装任务 / 安装包 /
队列与调度 / 运行记录 / 设置）、任务包安装/卸载/回滚、「已安装任务 → 配置」表单，
以及**内嵌调度**（启动时自动运行；「队列与调度」页可启动/停止/临时运行，并可完整编辑
条目/队列/窗口/配额/参数、查看运行记录与重置记忆）。
任务运行值保存在 config\values.json（本机独占，不入版本库），调度与临时运行都会
优先使用这里的值（临时运行卡里填的参数会覆盖保存值）。安装包能力：在「安装包」页
填写本机 `.oaspkg` 的**绝对路径**后安装；安装会校验 sha256、任务 lint 与
engine.flow_ops（老引擎会拒绝需要新语法的新包），并自动初始化/合并包内全局素材；
覆盖同版本需要勾选 force，旧版本会备份到 packages\backup\ 并可回滚。调度计划的
编辑入口是「队列与调度」页（条目/队列/窗口/配额/参数图形化编辑；高级 JSON 仍保留），
保存后立即写回 scheduler.json；调度运行中保存会提示重启调度生效。
日志：logs\runner.log、logs\scheduler.events.jsonl；调度状态见「队列与调度」页。


1. 首次使用
------------
1) 解压整个 zip 到一个新目录，例如 D:\OAS-runner\（不要覆盖旧版本目录）。
2) 用记事本编辑 config\live.json：
   - device.serial ：adb 设备 serial（模拟器如 127.0.0.1:16384，真机用 adb devices 查看）
   - device.adb_path：默认 adb/adb.exe；其他平台可留空改用系统 adb / ADB_PATH
   - runner.port   ：默认 8789（一般不用改）
3) 先确认设备在线（可选）：
   oasd.exe probe --serial 127.0.0.1:16384
4) 双击 start-runner.bat，浏览器打开 http://127.0.0.1:8789/
5) 在 Runner 页面「安装包」填写 `.oaspkg` 的**本机绝对路径**安装；在「已安装任务 → 配置」
   填写运行值；在「队列与调度」编辑 entry/queue、启动/停止调度或临时运行。
   安装/配置写入本目录的 tasks\、config\、packages\。
6) 停止：双击 stop-runner.bat，或直接关闭 start-runner 打开的窗口。


2. 目录说明
------------
oasd.exe          同一个二进制；本包用 `oasd.exe runner` 启动使用端
ui\runner.*       Runner 使用端页面（不含 Studio 编辑页面）
models\           OCR 模型（执行需要，请勿删除）
adb\              Windows adb（v1 随包；macOS/Linux 用系统 adb）
config\           live.json / live.example.json
packages\         已安装任务包、备份（Runner 独占写）
tasks\            已安装任务（任务包解压落点）
runs\             运行记录
logs\             runner.log 等日志
scheduler.json    调度/队列计划（初始为空，页面里添加）
assets\           全局素材（安装任务包时合并）
VERSION / BUILD-INFO.txt  版本与构建信息


3. 常见问题
------------
- 页面打不开：看 logs\runner.log；确认 8789 没有被别的程序占用。
- 设备连不上：先 `oasd.exe probe --serial <serial>`；MuMu 端口常见 16384。
- 升级：停止 Runner，备份 config\ tasks\ packages\ scheduler.json runs\ logs\，
  解压新包覆盖程序文件（oasd.exe、ui\、models\、adb\），保留数据目录。
- 未签名 exe 可能触发 SmartScreen：更多信息 -> 仍要运行（正式分发建议签名）。
- 本引擎默认只绑 127.0.0.1，不要暴露到公网。
- 同一台模拟器/真机同一时间只能被一个自动化进程控制：跑 Runner 时不要再开
  Studio 的试运行或旧的 oasd scheduler。


4. 非 Windows 平台（macOS / Linux）
-----------------------------------
没有 .bat 启动脚本；在包根目录直接运行：

   ./oasd runner --port 8789 --config config/live.json --ui ui

UI 是普通 Web 页面，用浏览器打开 http://127.0.0.1:8789/ 即可。adb 使用系统
安装的 adb（PATH 中），或在 config\live.json 的 device.adb_path 指定绝对路径。
详见 packaging\CROSS-PLATFORM.md（源码仓库内）。


5. 与 Studio 的关系
-------------------
Studio：制作/编辑任务、素材、流程、发布 .oaspkg（开发与编辑人员使用，端口 8788）。
Runner：只消费 .oaspkg 并执行（终端用户使用，端口 8789）。
两者共用同一份数据目录布局；单机部署时可以并存，但不要同时对同一台设备执行。
