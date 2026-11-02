# 会话交接单（SESSION HANDOFF）★ 一页纸

> ★★★ 新会话先读 `docs/START-HERE.md` → `docs/RESUME.md` → `docs/RESUME-INDEX.md` 最新行（seq 89）。
> 当前是 **0.1.0 恢复节点发布完成态**：tag `v0.1.0` 未移动、**GitHub Release 已上传**、`dev` `d1a7fcd` 经两轮评审收口 O-1/O-2，`main` 待快进同步。不要再灌 34/18 旧素材。

## 0. 当前状态（2026-11-02 13:5x）

| 项 | 值 |
|---|---|
| 分支/HEAD | `dev` = `d1a7fcd`（父 `54c715b`；+ 本轮 docs 提交 = HEAD）；`origin/dev` = `e8edbef`（待推送）；`main` **待快进同步**；包构建 commit `42567c1` |
| 恢复节点 | tag `v0.1.0`（annotated `cf123b8` → commit `e8edbef`，**未移动**）；**GitHub Release 已上传** |
| 发布 | `https://github.com/passioki/oas-core/releases/tag/v0.1.0`（id `431080249`，draft=false / prerelease=false，即 latest）；附件 zip（asset `700544587`，37,089,992 B，digest `sha256:e6112a33…5f5687`）+ `.sha256`（asset `700545142`，89 B）；本地同物 SHA256 `E6112A33…5F5687` |
| 版本 | `0.1.0`；**包不含 O-1/O-2**（包构建 `42567c1`，O-1/O-2 自 `54c715b` 起，评审修复 `d1a7fcd`） |
| 测试 | `cargo test --workspace` = **426 passed / 0 failed**（oasd 153 / flow 57 / runtime 125 + 2 integration）；UI pure test = **259 checks / 0 dead ids**；task lint = **1/0**（13:35 最终实测；发布包是 393/203 时构建） |
| 数据 | 全局素材 **16 条 / 1 图**；帧 **28 个 / current_id=20 / next_id=41** |
| 任务 | 只有 `返回庭院`（flow 11 steps / meta 400 B）；`tasks/测试` 已删除（随 `cd0ac34` 提交） |
| 工作区 | `d1a7fcd` 时 clean；本轮 docs 订正（8 份，终态）待 Lead 提交，仅 `docs/` |
| 服务 | serve **8787**：最终构建已重建，Lead 以 `-SkipBuild` 重启（使用本版二进制） |
| 文档 | 7 份历史文档已归档 `docs/archive/`；顶层 10 份 + `RELEASE-NODE-2026-11-02.md` |

## 1. 最近发生了什么

- `7a3869f`：记录 ResizeObserver 循环修复（`a2b4b12`）；用户 00:37 验收“没问题”。
- `cd0ac34`：**最终恢复节点数据快照**（00:58）——16 素材（新增 `管理分组`）、frames current_id=20、删除 `tasks/测试`。
- `42567c1`：0.1.0 文档归档 / scripts package assets 修复；本地包 01:08 构建（E611…5687）。
- `e8edbef`：docs 定稿为 tag `v0.1.0` target；**GitHub Release 上传**（13:0x，id `431080249`，即 latest）。
- `54c715b`：**O-1 collection 真实点击 + O-2 drag action shape** 初版（6 files +1413/−103）；146/54/119 tests（中间态）。
- `d1a7fcd`：**O-1/O-2 评审修复**（P1-1..P1-4 + P2，6 files +1436/−225）——exact shape 采样 / 409 / plain rect / 溢出校验；最终 **426/259** 回归。
- 本轮 docs 订正 8 份（RELEASE-NODE / PENDING-LEDGER §E-74 / RESUME / RESUME-INDEX / START-HERE / EDITOR-UX-PLAN / README / 本文件）到最终事实——待 Lead 提交。

## 2. 下一步

```text
1. Lead 提交并推送 docs 订正：git add docs && git commit && git push origin dev
2. main 快进同步 dev：git checkout main && git merge --ff-only dev && git push origin main
   → 之后回填 RELEASE-NODE §5「main 同步 commit」+ §7
3. 真机（先启动 MuMu emulator-5554）：collection real=1 端到端 + 真实 UI 手势复验
4. SelectionTool 结构债（抽后须 >=3 处调用）
5. 可选：P2-10（保存 flow 校验）与 drag 模板移动的 Plan B（asset 引用 + 运行时跟随识别）
```

## 3. 关键事实（防止再记错）

- 当前素材是 **16/1**，不是 34/18；34 是 `§E-70` 已推翻的错误恢复，archive 2827/2099 未导入（用户裁定素材可重建）。
- 当前帧 **28 / current_id=20 / next_id=41**，不是 40。
- `tasks/测试` 已删除并提交；task lint 是 **1/0**，不是 2/0。
- `action_roi`/`roi_back` 的 **JSON/UI 读取已删除**；runtime 内部仍保留 `click_back` 作为 region/collection 的 bbox 快捷路径——不要写成“全仓无 roi”。
- **drag/swipe 不写点击分布**：`record_click()` 只在 `perform_click()` 路径调用。
- 拖测 `margin`：默认 **0.15**，合法 **0~0.45**，作用于起点（配置了 `to` 时也作用于终点）候选矩形内缩；**仅 rect 非反选生效**；有 polygon/invert `shape` 时忽略并记 `margin=ignored`。
- **collection 真实点击已终态收口**（dev `d1a7fcd`）：`real` 严格 `0/1/true/false`；`real=1` 强制 live 帧；匹配通过后在**精确 action shape 内采样**（polygon 内/invert 排除区外）；未过阈值 **409**（不点击）。**默认仍是 dry-run**，响应/日志不变；与“素材行真实点击”共用 `perform_live_tap`；**无真机 E2E**。
- **v0.1.0 发布包不含 O-1/O-2**；混版本 runtime 会忽略 drag `shape`（invert 会点到排除区）⇒ invert 拖动需本版 runtime；**P2-10**（保存 flow 不校验解析错误）本轮不做。
- 最终测试基线 **426/259/1-0**（13:35）；oasd 153 / flow 57 / runtime 125（+2 integration）；DSL E2E `linted=8 errors=5`。
- 真实 UI 手势：用户 00:37 说已测试；助手独立证据只有真机 `/input/tap`，两者要区分。
- E2E 36/36 的证据边界是 `4d5759f`（23:0x），不含 23:39 点击分布/反选/真实开关与 00:02 RO 修复。

## 4. 活跃的坑

- 不要在未确认前覆盖 `assets/`、`frames/`、`tasks/`；`assets/_deleted-tasks/` 是唯一副本安全网，绝不删。
- 不要用 task 历史再拼“全局库”；不要再跑 `scripts/recover_historical_assets.py`。
- 中文文档/代码用 `read`/`edit`/`write` 工具；PowerShell 直接重写 UTF-8 会 mojibake。
- `_verify/n44/...` 是仓外路径（`D:\OAS-dev\_verify\n44\...`），不是 `oas-core/_verify`。
- `ENGINE §22.39 R11`：唯一 `assets/`，任务只引用全局 id；`ASSET-DATA-MODEL.md` 是数据模型入口。
