# 一键提交 / 推送（scripts/push-all.ps1）

把仓库里**全部可版本化改动**（代码、文档、`assets/` 图片与 JSON、`frames/`、`tasks/`、`scheduler.json` 等）
一次性暂存、提交并推送，避免手工逐目录 `git add` 时漏掉数据目录。

- 平台：Windows + PowerShell 5.1（脚本为 UTF-8 with BOM，勿另存为 ANSI）。
- 入口：`scripts/push-all.ps1`（逻辑）与 `scripts/push-all.bat`（双击 / 命令行包装；
  `.bat` 把全部参数**原样透传**给 `.ps1`，并把脚本退出码原样返回给调用方）。
- 安全默认：**只有 `dev` 分支会自动同步 `main`**；其它分支只推送自身，需发布到 `main` 时显式加 `-SyncMain`。

## 快速开始

```powershell
# 1) 只预览：打印状态、分支、将提交/排除的清单与将执行的命令，不做任何写操作
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\push-all.ps1 -DryRun

# 2) 正式执行（会显示清单并询问 y/N）
scripts\push-all.bat

# 3) 免确认（无交互终端 / CI 必须加 -Yes）
scripts\push-all.bat -Yes

# 4) 只提交到本地，不推送
scripts\push-all.bat -Yes -NoPush

# 5) 非 dev 分支确实要发布 main：显式加 -SyncMain
scripts\push-all.bat -Yes -SyncMain
```

## 参数

| 参数 | 默认 | 说明 |
| --- | --- | --- |
| `-Message <text>` | `chore(data): sync all resources/tasks/scheduler/code/docs <yyyy-MM-dd HH:mm>` | 提交信息 |
| `-Yes` | 关 | 跳过 y/N 确认；stdin 被重定向时必须使用 |
| `-DryRun` | 关 | 只预览，不写索引、不 commit、不 push（也不访问网络） |
| `-NoPush` | 关 | 只提交，不推送（也不同步 `main`）；若没有 staged 改动则直接退出 0，不补推 |
| `-SyncMain` | 关 | 显式允许**非 dev** 分支自动同步 `main`；`dev` 不需要它（默认已允许） |
| `-Remote <name>` | `origin` | 远程名；必须已存在 |
| `-Branch <name>` | 当前分支 | 目标分支；与当前分支不一致时报错（避免误推），detached HEAD 直接退出 3 |

## 提交范围与瞬态排除

1. `git add -A` 暂存全部改动（自动遵循 `.gitignore`），覆盖
   `apps/` `crates/` `docs/` `scripts/` `packaging/` `tools/` `templates/` `relations/`
   以及 `assets/`（图片 + JSON）、`frames/`、`tasks/`、`scheduler.json`、`Cargo.*` 等。
2. 再把下列**未跟踪**的瞬态文件从暂存区 `git reset -q --` 剔除（即使它们没有被 ignore）：
   `*.bak`、`*.tmp`、`*.log`、`scheduler.json.bak`、
   `scheduler.state.json`、`scheduler.events.jsonl`、`scheduler.state.json.lock` 等调度状态、
   任意层级 `logs/` 目录、`_verify/**/target/`。
3. **已跟踪文件不剔除**：判断依据是文件是否存在于 `HEAD`（`git ls-tree -r HEAD`），
   不是 `git ls-files`（`add -A` 后它会把刚暂存的新文件也算作“已跟踪”）。
   例如仓库里已跟踪的 `_verify/audit-20261101/B-cargo-test.log` 即使被修改、匹配 `*.log`，
   也会保留在暂存区并提交；脚本会打印「已跟踪的瞬态文件（N 个，保留在暂存区）」。
4. 剔除后二次校验：暂存区若仍残留**未跟踪**的瞬态文件则中止（退出 2），绝不提交。

> 说明：`/library/`、`/runs/`、`/dist/`、`/target/`、`/logs` 等已由 `.gitignore` 忽略，
> 因此 `add -A` 不会包含它们。**`library/` 当前不纳入版本控制**（`.gitignore` 第 10 行 `/library/`）；
> 主素材库是 `assets/`（会提交）。若将来要把 `library/` 纳入版本控制，需先调整 `.gitignore`，
> 否则脚本不会提交它。

## main 自动同步规则

推送当前分支成功后：

- 当前分支是 `main`：不做额外同步。
- 当前分支是 `dev`：自动
  `git fetch <remote>` → `git checkout main` → `git merge --ff-only dev` → `git push <remote> main` → 切回 `dev`。
- 其它分支：打印
  `WARN 分支 X 不自动同步 main（仅 dev）；如需发布请手动或加 -SyncMain`，
  跳过 main 同步，脚本**仍以 0 成功退出**。
- 加 `-SyncMain`：允许任意非 `main` 分支执行上述 main 同步。
- `-NoPush`：不推送，也不同步 `main`。
- **远程没有 `main` 分支**：脚本先用 `git ls-remote --heads <remote> main` 判断；
  不存在时打印 `WARN 远程 <remote>/main 不存在；跳过 main 同步（当前分支已推送）`，
  **成功退出 0**（不再退出 5）。
- `git ls-remote` 无法确认（网络/认证失败）：自动模式打印 WARN 并跳过（退出 0）；
  若这是显式 `-SyncMain` 请求，则以 5 失败（不静默吞掉发布失败）。
- 同步只用 `merge --ff-only`；分叉时绝不强行合并，切回原分支后退出 5。
- 注意：`git checkout main` 后即使 `git push main` 失败，**本地 `main` ref 可能已被 fast-forward**
  （脚本只负责切回原分支，不回滚本地 main）；下次重跑会重新尝试推送。

## 流程

1. 预检：git 可用、脚本位于仓库根 `scripts/` 下、无未解决冲突、HEAD 存在、分支名、远程存在。
2. `git add -A` → 剔除未跟踪瞬态（已跟踪的保留）→ 二次校验。
3. **无 staged 改动时**：
   - `-NoPush`：打印提示并退出 0（不补推）。
   - 本地相对 `<remote>/<branch>` 没有领先 commit：打印已同步并退出 0。
   - 本地领先（或远程分支尚不存在）：进入**「只推送」路径**补推已有 commit
     （用于上次 push / main 同步失败后的重跑），推送后仍按上面的 dev 规则同步 `main`，退出 0。
4. 打印新增/修改/删除计数 + 前 20 个文件 + 截断的 `--stat`。
5. 确认（`-Yes` 跳过；无交互终端且未加 `-Yes` 时提前退出 1，不会暂存）。
6. `git commit -m <message>`，失败退出 4。
7. `git push <remote> <branch>`，失败退出 5（本地 commit 保留）。
8. 按「main 自动同步规则」处理 `main`。
9. 收尾：打印 commit hash、分支、推送结果、main 状态、`git status --short`。

## 失败后的重跑 / 恢复

| 情况 | 本次行为 | 重跑时 |
| --- | --- | --- |
| push 失败（网络/认证） | 本地 commit 保留，退出 5 | 无新改动也会走「只推送」补推，然后按 dev 规则同步 main |
| main 同步失败（分叉/推送失败） | 切回原分支，退出 5；本地 main 可能已 ff | 同上；若仍是分叉需先手动处理 main |
| 远程没有 main | WARN 跳过，退出 0 | 每次都会 WARN 跳过，但不再失败 |
| 确认时取消 | 退出 1，改动仍在暂存区 | 重新运行；或 `git reset -q -- .` 取消暂存 |
| 没有任何改动且已同步 | 打印「没有需要提交的改动」，退出 0 | 无需操作 |

`-DryRun` 只读：不写索引、不 commit、不 push，也不访问网络（不执行 `git ls-remote` /
`git fetch` / `git push`）。它会分组打印「将提交」/「已排除的未跟踪瞬态」/
「已跟踪但匹配排除规则（保留在暂存区）」，并明确打印这些分支计划：
`main 规则 : 默认仅 dev 自动同步 main；其它分支需 -SyncMain；远程无 main 则 WARN 跳过（仍 exit 0）`
与
`无 staged : 本地领先上游且未 -NoPush 时走「只推送」路径补推已有 commit`。
DryRun 前后 `git status --porcelain`、`git rev-parse HEAD`、`git diff --cached` 不变，
也不执行 `git ls-remote` / `git fetch` / `git push`。

## 退出码

| 码 | 含义 |
| --- | --- |
| 0 | 成功；或没有需要提交的改动；或只推送补推成功；或远程无 main 而跳过同步 |
| 1 | 用户取消 / 非交互终端未加 `-Yes` |
| 2 | 预检失败（非 git 仓库、仓库根不匹配、冲突、远程不存在、`-Branch` 与当前分支不一致等）；`git add` / `git reset` 失败；剔除后仍有未跟踪瞬态文件 |
| 3 | detached HEAD（无法确定推送分支） |
| 4 | `git commit` 失败（如钩子拦截） |
| 5 | `git push` 或 main 同步失败；显式 `-SyncMain` 时无法确认远程 main |

## 安全约束

- 不使用 `git push --force`、`git clean`、`git reset --hard`。
- 不修改用户数据内容。
- 失败时不会丢弃已产生的本地 commit，也不会重置用户改动。
- **关于远程 URL / 凭证**：脚本自身不主动打印远程 URL，也不读取/打印凭证；
  但它会**原样转发 git 的 stdout/stderr**，所以 `git push` / `git fetch` / `git ls-remote`
  的成功或失败输出**可能包含远程名与 remote URL**（git 通常不回显密码）。
  如果 remote URL 里内嵌了令牌，错误输出可能包含它——请使用 credential helper，
  不要把凭据写进 remote URL。
