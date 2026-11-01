# RESUME — 断点续接卡（session-memory 强制 6）

> 规则：本文件每次断点覆盖重写；≤150 行；另一个会话只读本文件 + `RESUME-INDEX.md` 最新行
> 就能继续。细节放 `ENGINE-DESIGN.md` / `ASSET-DATA-MODEL.md` / `RELEASE-NODE-2026-11-02.md`。

```yaml
task_id: oas-core-release-node-0.1.0
checkpoint_seq: 87
updated_at: 2026-11-02T01:12:00+08:00
trigger: 0.1.0 本地 release + tag + main/dev 同步；docs 归档
status: done
```

## 0. 当前状态（2026-11-02 01:12 实测）

- `dev` = `main` = **tag `v0.1.0` target**；`origin/dev` / `origin/main` 已同步。
- 包构建 commit：`42567c1`（`release: v0.1.0 docs cleanup, package assets fix, audit evidence`）；tag 后面只多 docs 回填。
- 版本：`0.1.0`；`oasd --version` = 0.1.0。
- 测试：`cargo test --workspace` = **393 passed / 0 failed**；UI pure test = **203 checks / 0 dead ids**；task lint = **1/0**（仅 `返回庭院`）。
- 数据：全局素材 **16 条 / 1 图**（15 `ocr_line` + 1 `template`）；帧 **28 / `current_id=20` / `next_id=41`**；任务只有 `返回庭院`（flow 11 steps / meta 400 B）；`tasks/测试` 已删除。
- 工作区：**clean**；`dist/` 为 ignored 产物。
- 发布：本地 `dist/oasd-v0.1.0-win64.zip`（37,089,992 B）
  - SHA256 `E6112A33BE78D00850B4F1D0D5AEAC8E0A8172130231BB87145B43A0595F5687`
  - stage `assets` = 16，`task lint` = 1/0
  - tag `v0.1.0` 已创建；**未创建 GitHub Release**（用户要求本地 release 构建 + 同步分支，不要求发布页）。
- 文档：顶层 10 份 + `RELEASE-NODE-2026-11-02.md`；7 份历史文档归档 `docs/archive/`（`git mv`，未删除）。
- `session-memory.config.json`：`spec`/`progress`/`index` = `archive/...`。

## 1. 本次验收清单

- ☑ 三方审计（session↔docs↔code）：F 文档审计 37 条、G 仓库卫生审计、A/B/C/D/E 历史审计证据入库。
- ☑ 7 份历史文档归档（DESIGN-SPEC、DESIGN-SPEC-PROGRESS、SESSION-INDEX、DELIVERY-PLAN、RECOVERY-AUDIT、CODE-AUDIT、MIGRATION-NOTES）。
- ☑ 现行文档事实同步：START-HERE / README / RESUME / SESSION-HANDOFF / ASSET-DATA-MODEL / EDITOR-UX-PLAN / ENGINE-DESIGN §22.34 / PENDING / RESUME-INDEX。
- ☑ 新增 `RELEASE-NODE-2026-11-02.md`（发布/恢复节点索引）。
- ☑ `scripts/package.ps1` 默认版本 0.1.0 + 根 `assets/` 原样入包；packaging 版本文案 0.1.0。
- ☑ 本地 release 包构建 + stage assets=16 + stage lint 1/0。
- ☑ `v0.1.0` tag；`main` 快进同步 `dev`；两个分支推送。
- ☑ 本地备份：`D:\OAS-dev\_backups\oas-core-20261102-005826\`（docs + git bundle + 旧 dist zip + 审计报告）。

## 2. 清理记录（审慎、有备份）

- 归档（未删）：7 份历史文档 → `docs/archive/`，git 历史完整。
- 本地清理（已备份）：`target/m0-smoke/`、`target/tmp/`、`apps/oasd/logs/`、旧 `dist` 四个 zip（v1.0.0/v1.1.0/v1.2.0/v1.2.1）。
- 保留：`target/release`（打包依赖）、`dist` v1.2.2/v1.2.3 恢复包、`assets/_deleted-tasks/测试-*`、`assets/.backup/`、仓外 `_verify` 历史证据。
- 未动：`tasks/assets/frames/relations`、Cargo 源、docs 权威 10 份、当前审计 A~G。

## 3. 已验证

```text
cargo test --workspace = 393 passed / 0 failed
node apps/oasd/ui/ui_pure_test.js = 203 checks / 0 dead ids
oasd task lint --tasks-dir tasks --all = 1 task / 0 errors
dist/stage/assets/assets.json = 16
dist/stage task lint = 1/0
zip SHA256 = E6112A33...5F5687
```

## 4. 已知开口（不阻塞 0.1.0 恢复节点）

1. collection 集合测试仍 **dry-run**：`handle_collection_match` 只返回 `action_box`，无 `/input/tap`。
2. drag 生成的 flow step 仍是 **bbox region**；多边形/反选拖动没有进入 flow 语义。
3. `SelectionTool` 未抽出；识别掩膜/操作区两套几何按语义保留。
4. 真实 UI 手势：用户 00:37 说已测试；助手独立证据只有真机 `/input/tap`（emulator-5554）。
5. E2E 36/36 证据停在 `4d5759f`，不覆盖 23:39 点击分布/反选/真实开关与 00:02 RO 修复；RO 已由单独浏览器验证。

## 5. 下一步

1. 真机完整 UI 手势回归（若需要）→ 可补 collection 真实点击。
2. drag step shape（扩 DSL 或明确只 support bbox）。
3. SelectionTool 结构债。
4. 如需 GitHub Release：对 tag `v0.1.0` 创建发布页（当前未创建）。

## 6. 历史事实（只读）

- 4:21 干净基线：`f6be0f0`（22 帧 / 返回庭院 3 素材 / 无全局 assets）；18:42~19:14 docs-only；19:30 `9618cb5` 起实现。
- 11-01 09:02 用户要求回滚，09:11 执行；archive 分支 `archive/migration-20261101 = 29b696f`（2827 素材 / 2099 图 / 57 个任务目录）。
- 34/18 是 11-01 错误恢复（§E-70 已推翻）；archive 2827/2099 未导入（用户 21:02：“先不管素材，素材可以重建”）。

## 7. 证据

- 本次审计：`_verify/audit-20261101/F-docs-audit.md`、`G-repo-hygiene.md`。
- 已入库审计：`_verify/audit-20261101/{A-session-git,B-runtime,C-ui,D-e2e,E-acceptance}.md`、`B-cargo-test.log`。
- session 导出（仓外）：`D:\OAS-dev\_verify\n44\session-current-both-full.txt`。
- 备份：`D:\OAS-dev\_backups\oas-core-20261102-005826\`。
- 发布节点：`docs/RELEASE-NODE-2026-11-02.md`；tag：`v0.1.0`。

## 8. 阻塞

- 无技术阻塞。0.1.0 恢复节点已完成；产品开口见 §4。
