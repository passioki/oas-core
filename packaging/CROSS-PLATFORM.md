# Runner 跨平台 / Android 使用端设计约束

> 状态：**v1 只发布 Windows x64 发行包**（`oasd-runner-v<version>-win64.zip`）。
> 本文把「以后可能做 Android 使用端」的边界写死，避免 Runner 核心、HTTP API、
> 任务包格式、配置 schema 在 v1 里埋下平台绑定。**v1 不实现 Android**。
> 关联：`docs/RUNNER-STUDIO-SPLIT-PLAN.md`（Studio/Runner 分离）、`packaging/README-DEPLOY.md`。

## 1. 目标与范围

- **现在（v1）**：Windows 10/11 x64；`oasd.exe runner` + `ui/runner.*`；adb 随包。
- **下一步可移植目标**：macOS / Linux 桌面（同一 Rust 核心 + 系统 adb + Web UI）。
- **未来候选**：Android 使用端（两种形态，见 §5），**只保证核心/契约可移植，不排期**。
- **非目标**：v1 不做 Android 打包、不做 iOS、不做 LAN 多机调度、不做云端。

## 2. 必须平台无关的契约

以下四层是 Runner 的产品边界。任何一层引入平台假设，都会让 Android/桌面移植
从「加一个 adapter」变成「重写一半」。

### 2.1 Runner 核心（Rust）

- 只用 `std` + 现有 crate；不引入 Windows-only API（注册表、COM、Win32 服务控制、
  `%USERPROFILE%` 展开、盘符拼接）作为**核心逻辑**。平台相关代码集中在 adapter 层。
- 设备控制走 trait/adapter（现有 `oas-types` 的 `ScreenSource` / `InputSink` 思路）：
  核心只依赖「截图 / 点击 / 滑动 / 按键 / 应用启动」抽象，不直接 `Command::new("adb")`。
- 时间统一 epoch 毫秒 / UTC，展示层再转本地时区；不依赖 `GetLocalTime` 之类平台调用。
- 文件锁、原子写（tmp + rename）、随机端口等已有实现继续保持跨平台语义；Android
  上 `rename` 同分区原子性可用。

### 2.2 路径

- 配置里的路径**只允许**：相对包根的路径（`tasks`、`models/...`）或前导斜杠的
  URI 风格（`/data/...` 仅在明确平台段使用）。**禁止**把 `C:\`、`D:/` 写进示例、
  默认值、任务包、manifest。
- 内部统一用 `/` 作为序列化分隔符；落地到文件系统时按平台转换。Windows 反斜杠
  只在 `.bat` / 用户输入兼容层出现。
- 配置解析继续「相对配置文件所在目录（包根）」；不依赖进程 CWD，也不依赖
  「可执行文件旁」这一 Windows 习惯（Android 上 exe 与数据目录天然分离）。
- 文件名/任务 id 统一 UTF-8；不假设大小写不敏感（Linux/Android 区分大小写）。
  禁止 `:` `*` `?` `"` `<` `>` `|` 等 Windows 非法字符出现在任务 id/素材名。

### 2.3 HTTP API

- 只使用 HTTP/1.1 + JSON + 静态文件；不依赖 Windows 专属协议、命名管道、注册表。
- 绑定地址：桌面默认 `127.0.0.1`；Android 上同样只绑 loopback（见 §6），端口可由
  系统分配（随机端口）后通过启动参数/环境变量告诉外层。
- API 契约（`/runner/*`、`/scheduler/*`、`/instance`、`/runs` 等）与平台无关；
  不允许出现 `"os":"windows"` 之类的**分支字段**。设备能力差异通过能力探测/错误码
  表达（如 `adb_unavailable`），而不是平台 if/else。
- UI 是普通 Web 页面（`ui/runner.html`），桌面用浏览器、Android 用 WebView 加载
  `http://127.0.0.1:<port>/`，不依赖任何桌面浏览器 API。

### 2.4 任务包（`.oaspkg`）与配置 schema

- 包格式 = zip + `manifest.json` + `checksums.sha256`，与 OS 无关；解压校验只用
  相对路径，禁止绝对路径/盘符/`..`/符号链接（zip-slip 规则见设计稿 §A.7）。
- **schema 里不出现平台必填字段**。设计稿 `manifest.platform`（`os`/`needs_adb`/
  `design_resolution`）只能作为**可选、只读的元数据**：安装/执行不得依据它拒绝或
  改变行为；v1 的 `platform.os` 不参与任何判定，未来可删除而不破坏兼容。
- `config/live.json` 的 `serve` / `runner` / `paths` / `ocr` / `device` / `scheduler`
  字段与平台无关；`device.adb_path` 是配置项，不是硬编码。
- 任务/素材逻辑坐标基于设计分辨率（1280×720 等），与设备像素密度解耦；Android
  端不得把「屏幕物理尺寸」写进 schema。

### 2.5 可移植性验收清单（新增代码时自查）

- [ ] `cargo test --workspace` 不依赖 Windows 路径/注册表/COM。
- [ ] 配置、manifest、日志里不出现盘符或 `\` 分隔符。
- [ ] 新增设备能力走 adapter trait，不在 `serve`/`runner` 里 `cfg!(windows)` 分叉业务。
- [ ] 新增 API 字段在 Android/桌面语义一致；平台能力用「可选字段 + 能力错误」表达。
- [ ] UI 不调用桌面浏览器专属 API（`window.chrome.*`、文件系统 API 等）。
- [ ] 任务包在「无 adb」环境能安装、能查看配置，只是执行时报可解释错误。

## 3. Windows v1 打包事实

| 项 | v1 做法 | 跨平台含义 |
|---|---|---|
| 二进制 | `oasd.exe`（同一二进制，`runner` 子命令） | macOS/Linux 用同源编译产物，无 `.exe` |
| adb | 随包 `adb/adb.exe` + `AdbWinApi.dll` + `AdbWinUsbApi.dll` | 其他平台用**系统 adb**（`adb` 在 PATH） |
| adb 配置 | `device.adb_path = "adb/adb.exe"` | 其他平台可留空走 `ADB_PATH`/PATH，或填绝对路径 |
| 启动 | `start-runner.bat`（仅 Windows） | macOS/Linux 直接命令行（见 §4）；Android 见 §7 |
| UI | 浏览器打开 `http://127.0.0.1:8789/` | WebView 打开同一 URL |
| 模型 | `models/*.onnx` 随包 | 模型与平台无关；Android 上放 app 私有目录 |

> Windows 包里的 adb 二进制**不要**被核心代码硬编码引用；统一读 `device.adb_path`
> 或 `ADB_PATH`，这样 Android/Linux 不需要改核心。

## 4. 桌面其他平台（macOS / Linux）

- 编译：`cargo build --release -p oasd --features ocr`；产物 `oasd`。
- 运行：`./oasd runner --port 8789 --config config/live.json --ui ui`。
- adb：安装系统 `adb`；如不在 PATH，配置 `device.adb_path` 为绝对路径。
- 停止：Ctrl+C（或后续提供 `stop-runner.sh`，v1 不提供）。
- 路径：包根相对路径语义与 Windows 完全一致；不要复制 `start-runner.bat`。

## 5. Android 使用端：两种候选

### 候选 ①：手机上直接运行 Rust 核心（on-device Runner）

- 交叉编译 `aarch64-linux-android`（必要时 `armv7-linux-androideabi` / `x86_64-linux-android`
  覆盖模拟器），产出 `.so` 或可执行文件；
- Kotlin/Java 壳通过 **JNI** 调用 Rust：启动/停止 Runner、传数据目录、拿本地端口；
- **复用同一套 Runner HTTP API 与 `.oaspkg` 包格式**：手机内 WebView 加载
  `http://127.0.0.1:<随机端口>/`，任务包/配置/调度/运行记录逻辑与桌面一致；
- 设备控制 adapter 换成 Android 原生能力（MediaProjection 截图、AccessibilityService
  点击/滑动、`PackageManager` 启动应用），不是 adb；
- 适合「手机自己跑脚本」的形态；缺点是前台服务/电池/权限成本高（§6）。

### 候选 ②：手机作为控制器，驱动另一台设备（当前架构直接复用）

- 手机只跑「Runner UI + HTTP 客户端」，真正的执行仍在 PC/模拟器/另一台设备；
- 需要把当前 `127.0.0.1` 绑定改成 LAN + token（设计稿 Q1 的后续项），并解决发现/
  配对/证书；v1 明确**不做** LAN，所以这条是更远的候选；
- 优点：核心零改动；缺点：依赖网络与另一台执行机，不是「单机使用端」。

> 建议：先做候选 ① 的**可行性 spike**（JNI + 本地 HTTP + WebView + 一个 MediaProjection
> 截图/点击 adapter），再决定是否产品化；候选 ② 等 LAN 安全模型确定后再评估。
> **v1 不实现任何一条**，只保证 §2 的契约可移植。

## 6. Android 需要平台适配的边界

| 边界 | 桌面现状 | Android 需要 | 对核心的影响 |
|---|---|---|---|
| 设备控制 adapter | adb 子进程（`oas-adapter-pc`） | MediaProjection（截图）+ AccessibilityService（点击/滑动/返回/Home）+ 应用启动 | 新增 adapter 实现；核心 trait 不变 |
| 前台服务 / 电池 | 无 | 执行期间必须前台服务 + 常驻通知；请求忽略电池优化；Doze 下定时调度会被推迟 | 调度循环要能被系统暂停/恢复；时间基准用 `elapsedRealtime`/epoch，不假设一直运行 |
| 通知 | 无 | 前台服务通知、任务完成/失败通知、权限被撤销提示 | 通知是平台层，不进 HTTP schema |
| 文件权限 | 普通目录写 | scoped storage：任务包/模型/日志放 app 私有目录；导入 `.oaspkg` 用 SAF `content://` 流式读入 | 安装 API 需接受「输入流」而非仅本地路径（桌面 path 模式保留） |
| adb 不可用时的控制通道 | adb 是唯一通道 | 无 adb；用 MediaProjection/Accessibility；若目标 app 禁止截屏则只能报可解释错误 | 核心不得假设「有 adb 就一定有设备」；能力探测 + 错误码 |
| 屏幕/坐标 | 固定 1280×720 设计坐标 | 刘海/挖孔/手势导航/旋转；截图与点击坐标系可能不同 | 坐标换算放 adapter；任务包仍用设计分辨率 |
| 后台启动 | 无 | Android 12+ 前台服务启动限制、精确闹钟权限 | 调度「窗口/quota」语义不变，唤醒精度由平台决定 |
| 日志/诊断 | `logs/*.log` 文件 | logcat + app 私有日志文件；导出分享 | 日志接口抽象，不硬编码路径 |
| 更新 | 覆盖 zip | APK 升级；任务包独立于 APK 升级 | 版本/迁移逻辑与平台无关 |

### 明确不做的「捷径」

- 不在 Android 上内置 adb/自建 adb server 去控制本机（需要 root/无线调试，体验与合规都差）；
- 不把 `Runtime.exec("adb ...")` 作为 Android 主通道；
- 不为 Android 复制一份 `runner` 业务逻辑；只换 adapter + 壳。

## 7. Android 启动 / 日志 / 路径 / 端口占位

这些是未来实现的**约定**，v1 不写代码：

- **启动**：App 启动 → 前台服务 → JNI 启动 Rust Runner（传入 app 私有数据根）→
  Runner bind `127.0.0.1:0`（随机端口）→ 把实际端口回传 Kotlin → WebView 加载
  `http://127.0.0.1:<port>/`。端口不写死 8789（避免与其他 app/多实例冲突），
  启动参数仍允许显式 `--port` 便于调试。
- **路径**：数据根 = `context.filesDir`（或 `noBackupFilesDir`）；
  布局与桌面包一致：`tasks/`、`packages/`、`runs/`、`logs/`、`config/`、`assets/`、
  `models/`；任务包导入先落 `packages/incoming/` 再走同一套校验/安装流程。
- **日志**：`logs/runner.log` + logcat tag（如 `OASRunner`）；前台服务通知显示
  「Runner 运行中 / 调度已停止」；导出日志走系统分享。
- **停止**：通知栏「停止」→ 置停止标志 → join 调度线程 → 关 HTTP → 停前台服务；
  进程被杀后下次启动按 `scheduler.state.json` 恢复（与桌面一致）。
- **权限**：启动时引导「悬浮窗/无障碍/通知/忽略电池优化」；拒绝时 UI 明确说明
  哪项能力不可用，而不是静默失败。
- **配置**：`config/live.json` 仍可编辑（高级设置页或导入）；`device.adb_path`
  在 Android 上无意义，adapter 选择由构建变体决定，不写进 schema。

## 8. v1 决策摘要

1. **只发 Windows x64**；macOS/Linux 只保证「直接命令行可跑」的源码路径，不发官方包。
2. **不实现 Android**；但核心/HTTP API/任务包/配置 schema 按 §2 保持平台无关。
3. Windows 包内 adb 仅作为**默认 adapter 的二进制**，不是核心依赖。
4. `manifest.platform` 等平台字段一律可选、只读、不参与判定。
5. 将来 Android 优先评估「候选 ① on-device Runner」，复用同一 HTTP API 与包格式；
   「候选 ② 手机当控制器」依赖 LAN + token，暂不设计。
