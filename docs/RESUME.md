# RESUME — 断点续接卡（session-memory 强制 6）

> 规则：本文件每次断点覆盖重写；≤150 行；另一个会话只读本文件 + `RESUME-INDEX.md` 最新行
> 就能继续。细节放 `ENGINE-DESIGN.md` / `ASSET-DATA-MODEL.md` / `RELEASE-NODE-2026-11-02.md`。

```yaml
task_id: oas-core-0.1.0-release-and-o1-o2
checkpoint_seq: 89
updated_at: 2026-11-02T13:50+08:00
trigger: 终态（O-1/O-2 第二轮评审修复 d1a7fcd + docs 订正到最终事实）
status: done
```

## 0. 目标（一句话）

把 0.1.0 恢复节点发布到 **GitHub Release**（tag `v0.1.0` 不移动），收口 **O-1 collection 真实点击** / **O-2 drag action shape**，并把两轮评审后的**最终事实**回填 `docs/`。

## 1. 当前状态（2026-11-02 13:5x）

| 项 | 值 |
|---|---|
| `dev` | `d1a7fcd`（`fix: address O-1/O-2 review findings (P1-1..P1-4, P2)`，6 files +1436/−225；父 `54c715b`）；docs 终态订正提交 `906bbed`（父 `d1a7fcd`），**已推送 origin** |
| `main` | = `dev` = `origin/dev` = `origin/main` = `906bbed`（**已同步 origin**，2026-11-02 13:55 实测；实质代码提交 `54c715b` + `d1a7fcd`，docs 终态订正提交 `906bbed`）；tag `v0.1.0` → `e8edbef`（未移动）；包构建 commit `42567c1` |
| tag `v0.1.0` | annotated `cf123b8` → commit `e8edbef`（**未移动**）；发布包构建 commit `42567c1` |
| GitHub Release | `https://github.com/passioki/oas-core/releases/tag/v0.1.0` · id `431080249` · draft=false / prerelease=false · repo latest |
| 附件 | `oasd-v0.1.0-win64.zip`（asset `700544587`，37,089,992 B，digest `sha256:e6112a33…5f5687`）+ `.sha256`（asset `700545142`，89 B） |
| 测试 | workspace **426 passed / 0 failed**；oasd **153** / flow **57** / runtime **125**（+2 integration = 127）；UI **259 checks / 0 dead ids**；lint **1 task / 0 errors**；`cargo build -p oasd --features ocr` exit 0（6 条既有 warning） |
| DSL E2E | 临时 task lint：**linted=8 errors=5**——3 个合法变体（polygon+invert / 旧 flow 无 shape / rect+invert）ok；5 个非法变体（自交、坐标>1e6、负坐标 rect、point 上带 shape、点数<3）全部报错且路径精确 |
| 发布包基线 | v0.1.0 包（`42567c1` 构建）内仍是 **393 tests / 203 UI checks**（历史事实，**不含 O-1/O-2**） |
| 数据 | 全局素材 **16 条 / 1 图**（15 `ocr_line` + 1 `template`）；帧 **28 / current_id=20 / next_id=41**；任务仅 `返回庭院`（flow 11 steps / meta 400 B） |
| 服务 | serve **8787**：最终构建已重建，Lead 在最终提交后以 `-SkipBuild` 重启（使用本版二进制） |
| 工作区 | 本轮 docs 订正 8 份已提交（`906bbed`），仅 `docs/` |
| 版本 | `0.1.0`（`oasd --version` = 0.1.0） |

## 2. 验收清单（全部 ☑）

- ☑ **GitHub Release 上传**：URL/id、draft=false / prerelease=false、创建后即 latest — 证据 `D:\OAS-dev\_verify\release-v0.1.0\REPORT.md`。
- ☑ **附件两份**均 `state=uploaded`：zip 37,089,992 B、旁文件 89 B — 同上。
- ☑ **tag 未移动**：`cf123b8` → `e8edbef`，包 commit `42567c1` — `git rev-parse 'v0.1.0^{commit}'`。
- ☑ **O-1**：`real` 严格取值 + 强制 live 帧 + 精确 shape 内采样 + 未过阈值 409；缺省 dry-run 响应/日志逐字不变 — `serve.rs` +13 tests。
- ☑ **O-2**：drag `from`/`to` 可选 `shape`；解析校验齐全、plain rect 操作区保留、runtime polygon/invert 走 `ClickShape::sample`；旧 flow 不变 — flow +8 / runtime +12 tests。
- ☑ **两轮评审**：第一轮 4 个 P1（P1-1 shape 采样 / P1-2 409 / P1-3 plain rect / P1-4 溢出）+ P2；修复提交 `d1a7fcd`，第二轮复核通过。
- ☑ **最终回归**：workspace **426/0**、UI **259/0 dead ids**、lint **1/0**、OCR build exit 0、DSL E2E **8/5** — Lead 13:35 实跑。
- ☑ **代码提交**：`54c715b`（初版 6 files +1413/−103）、`d1a7fcd`（评审修复 6 files +1436/−225）。
- ☑ **docs 8 份订正到最终事实**（seq 89）：RELEASE-NODE / PENDING-LEDGER §E-74 / RESUME / RESUME-INDEX / START-HERE / EDITOR-UX-PLAN / README / SESSION-HANDOFF — 证据 `CORRECTIONS-REPORT.md`。
- ☑ **docs push + main ff 同步 origin**（2026-11-02 13:55 实测；`dev` = `main` = `origin/dev` = `origin/main` = `906bbed`）。

## 3. O-1 最终交付（collection 真实点击，dev `d1a7fcd`）

- `GET /task/asset/collection/match` 可选 `real`：**只接受 `0/1/true/false`**，其它值 **400**；缺省 / `real=0` 的 dry-run **响应与 editor 日志逐字不变**。
- `real=1` **要求 live 帧**（frame_source 非 live → 400 `real click requires live frame`，提示 `live=1`）。
- 匹配通过后在**当前帧精确 action shape** 内采样：shape 平移复用 `AssetSet::translate_click_shape`（成员 shape 优先、集合 shape 绝对兜底），采样复用 `ClickShape::sample`；polygon 在多边形内、invert 在排除区外；采样不可用/越界 → **400 loud**，绝不回退 bbox/整屏中心。无 exact shape 的 legacy 素材仍取历史 action_box 中心（在其 click_back 矩形内）。
- `real=1` 且匹配未过阈值 → **409** + `clicked:false`（不点击）；成功 → **200** + `real/clicked/click_point/tap_ok`；adb 失败 → **500**。点击与 `POST /input/tap` 共用 `perform_live_tap`。
- UI：真实模式（顶部「素材测试」开关）先 dry 预览 → 二次确认（提示「服务端将重新匹配并点击最新操作区（shape 内采样）；实际点击点以返回 click_point 为准」）→ 只发一次 `real=1`；409 显示「真实点击未执行：重新匹配未过阈值（未点击）」；演练模式明确「演练（未点击）」，绝不真实点击。
- 代码：`serve.rs:1266`（路由）· `3303-3534`（handler）· `2137`（`perform_live_tap`）· `5923-6001`（exact shape/采样）· `6018`（error body）；`app.js:2941-3074`。
- 遗留：**无真机 E2E**（MuMu 未运行；成功路径用 `#[cfg(test)]` tap seam 覆盖）。

## 4. O-2 最终交付（drag action shape，dev `d1a7fcd`）

- DSL：drag `from`/`to` 的 **region** target 可选 `shape`：`{"kind":"rect"|"polygon","roi":[x,y,w,h],"points":[[x,y],…],"invert":bool}`；`shape:null` = 缺省；旧 flow 无 shape 行为不变。
- 解析校验（`FlowError` path 精确到 `steps[i].args.from.shape...`）：坐标 0..=1e6 且 i32 可表示；rect x/y≥0、x+w/y+h checked；顶点 3..=256、**拒绝自交**、i128 鞋带面积；`shape` 只允许 region target（point/ocr_text 上出现 → FlowError）。
- runtime：polygon/invert 用与点击同源的 `ClickShape::sample`（失败 loud，不回退）；bbox/contains i64 安全；退化/帧外几何 loud；direction/旧 distance 语义不变；`margin` 仅 **rect 非反选**保留（polygon/invert 忽略并记 `margin=ignored`）；`to=None` 的 distance 终点只受帧约束（注释/测试已明确）。
- UI：`generateRegionDrag` 现在 **plain rect 操作区也保留**（`from.roi` = action shape bbox；仅 polygon/invert 内联 `shape`）；`testAssetDrag` 生成时快照当帧 shape；`builder.js` 改 roi 时保留 `from.shape`，并区分「polygon/invert 按 shape 采样、margin 忽略」与「rect 非反选以 roi 为准」。
- 代码：`oas-flow/src/lib.rs:626-654`（类型）· `1136-1431`（解析）· `2604+`（测试）；`oas-runtime/src/lib.rs:252-310`（DragEndpoint）· `1733-1821`（target/sampler）· `3035-3155`（执行/detail）；`app.js:911-925` · `builder.js:1030-1060`。
- 测试：flow 49→**57**（+8）、runtime 113→**125**（+12，含评审 4 个溢出/非法复现样例；另 2 个 integration）。
- 风险：**混版本**——旧 runtime 静默忽略 `shape`、按 bbox 采样，invert 会点到排除区 ⇒ invert 拖动需本版 runtime；Plan B（asset 引用 + 运行时跟随识别）未做（生成时快照）；P2-10「保存 flow 不校验解析错误」本轮明确不做。

## 5. 已知开口（不阻塞本节点）

1. `SelectionTool` 未抽出（识别掩膜/操作区两套几何按语义保留）。
2. E2E 36/36 证据停在 `4d5759f`；不含 23:39 点击分布/反选/真实开关与 00:02 RO 修复。
3. collection `real=1` **真机 E2E 未做**（MuMu 未运行、`adb devices` 为空）。
4. **混版本**：旧 runtime 忽略 drag `shape`（invert 会点到排除区）；invert 拖动需本版 runtime。
5. **P2-10**：保存 flow 不校验解析错误 — 本轮明确不做。
6. 模板移动/多位置仍是生成时 shape 快照；Plan B 未做。
7. 活动页参考帧未补齐。
8. v0.1.0 发布包（`42567c1`）**不含 O-1/O-2**。
9. 第二轮评审残留 P2（不阻断）：**P2-12** 显式闭合环/重复顶点 polygon 会被自交检查误判；**P2-11** 极端 `distance`（i32::MAX）在 debug 可溢出 panic（既有、非 shape 路径）；P2-13~16 注释/UI note 措辞与少量 `as i32` 截断；UI 接线测试仍以静态断言为主 — 详见 `D:\OAS-dev\_verify\final-review-20261102\REVIEW2.md`。

## 6. 下一步精确动作

1. ✅ **已执行（2026-11-02 13:55）**：docs 提交 `906bbed` → `git push origin dev` → `git checkout main && git merge --ff-only dev` → `git push origin main`；**不移动 tag `v0.1.0`**。
2. 真机：启动 MuMu（`emulator-5554`）→ 对一条 collection 做 `real=1` E2E；可选真实 UI 手势复验。
3. 结构债：`SelectionTool` 抽取（抽取后须 ≥3 处调用，避免过度抽象）；P2-10 视用户裁定。

## 7. 最终验证（Lead 2026-11-02 13:35 实跑）

```text
cargo test --workspace = 426 passed / 0 failed
cargo test -p oasd = 153 · oas-flow = 57 · oas-runtime = 125 (+2 integration = 127)
node ui_pure_test.js = 259 checks / 0 dead ids
oasd task lint --tasks-dir tasks --all = 1 task / 0 errors
cargo build -p oasd --features ocr = exit 0（6 条既有 warning）
DSL E2E（临时 task lint）= linted=8 errors=5（3 合法 ok / 5 非法精确报错）
serve 8787 = 最终构建（Lead 以 -SkipBuild 重启）
```

## 8. 环境（E11~E14，详见 START-HERE §3）

- PowerShell **5.1** + 执行策略 Undefined ⇒ `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\dev-serve.ps1 -NoOpen`。
- `node` 不在 PATH；bundled `C:\Users\passioki\.dsh\dsh-runtimes\dsh-primary-runtime\dependencies\node\bin\node.exe`（v24.21.0）。
- PS 5.1 重定向日志是 **UTF-16LE**（`-Encoding Unicode` 或转 UTF-8）。
- `adb devices` 为空（MuMu `emulator-5554` 未运行）。

## 9. 证据

- 发布：`D:\OAS-dev\_verify\release-v0.1.0\REPORT.md`。
- 文档订正：`D:\OAS-dev\_verify\docs-backfill-20261102\CORRECTIONS-REPORT.md`（上一轮 `DOCS-REPORT.md` 为中间态）。
- 节点索引：`docs/RELEASE-NODE-2026-11-02.md`；台账：`docs/PENDING-LEDGER.md §E-74`。
- 历史审计：`_verify/audit-20261101/{A-session-git,B-runtime,C-ui,D-e2e,E-acceptance}.md`。
- 备份：`D:\OAS-dev\_backups\oas-core-20261102-005826\`。

## 10. 阻塞

- **无**（push 与 main 同步已完成）。真机 E2E 待 MuMu 启动。
