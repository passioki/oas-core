#requires -Version 5.1
<#
.SYNOPSIS
    一键提交并推送 OAS 仓库中的全部可版本化改动（代码 / 文档 / 资源 / 任务 / 帧 / 调度）。

.DESCRIPTION
    默认提交范围：git add -A（自动遵循 .gitignore），覆盖
      apps/ crates/ docs/ scripts/ packaging/ tools/ templates/ relations/
      assets/（images + JSON） frames/ tasks/ scheduler.json Cargo.* 等。

    永远不提交的瞬态文件（即使没有被 .gitignore 覆盖）：
      *.bak *.tmp *.log
      scheduler.json.bak
      scheduler.state.json / scheduler.events.jsonl / *.lock（调度运行状态）
      logs/（任意层级日志目录）
      _verify/**/target/（构建产物）

    注意：上述规则只对“未跟踪/不在 HEAD 中”的文件生效。仓库里已经跟踪的
    文件（例如 _verify/ 下的历史 .log）即使匹配排除模式也会保留在暂存区，
    避免把用户明确跟踪的文件悄悄排除。

    流程：预检 -> git add -A -> 剔除瞬态 -> 清单摘要 -> 确认 -> commit
          -> push 当前分支 -> 仅 dev（或显式 -SyncMain）fast-forward 同步 main。
    无 staged 改动但本地领先上游时，走“只推送”路径补推已有 commit
    （用于 push/main 同步失败后的重跑）。

.PARAMETER Message
    提交信息。默认自动生成：
    "chore(data): sync all resources/tasks/scheduler/code/docs <yyyy-MM-dd HH:mm>"。

.PARAMETER Yes
    跳过交互确认。在无交互终端（stdin 被重定向）中运行时必须指定。

.PARAMETER DryRun
    只预览：打印状态、分支、排除项与将要执行的命令；不写索引、不 commit、不 push。

.PARAMETER NoPush
    只提交，不推送（也不做 main 同步）。若没有 staged 改动，则直接退出 0
    （不会补推已有 commit）。

.PARAMETER SyncMain
    显式允许在非 dev 分支上自动同步 main。默认只有 dev 分支会自动
    fast-forward 并推送 main；其它分支只推送自身，打印 WARN 后仍成功退出。

.PARAMETER Remote
    远程名，默认 origin。

.PARAMETER Branch
    目标分支，默认当前分支。与当前分支不一致时会报错退出（请先手动切换），
    以避免把改动误提交/误推送到别的分支。

.EXAMPLE
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts\push-all.ps1 -DryRun

.EXAMPLE
    scripts\push-all.bat -Yes -Message "chore(data): sync assets"

.EXAMPLE
    scripts\push-all.bat -Yes -SyncMain -Message "chore(data): release feature branch to main"

.NOTES
    退出码：0 = 成功 / 没有需要提交的改动 / 只推送成功 / 远程无 main 而跳过同步；
            1 = 取消 / 非交互终端未加 -Yes；
            2 = 预检失败（非 git 仓库、冲突、远程不存在等）；
            3 = detached HEAD；4 = commit 失败；5 = push 或 main 同步失败。

    安全约束：不执行 git push --force，不执行 git clean，不执行 git reset --hard，
    不修改用户数据内容，不主动打印凭证。脚本会原样转发 git 的 stdout/stderr，
    因此成功/失败时可能显示远程名或 remote URL（通常不含密码；若把令牌写进
    remote URL 则可能出现在 git 错误输出中，建议使用 credential helper）。
#>

[CmdletBinding()]
param(
    [string]$Message = "",
    [switch]$Yes,
    [switch]$DryRun,
    [switch]$NoPush,
    [switch]$SyncMain,
    [string]$Remote = "origin",
    [string]$Branch = ""
)

$ErrorActionPreference = "Stop"

# ---------------------------------------------------------------- 退出码
$EXIT_OK        = 0
$EXIT_CANCELLED = 1
$EXIT_PRECHECK  = 2
$EXIT_DETACHED  = 3
$EXIT_COMMIT    = 4
$EXIT_PUSH      = 5

$script:RepoRoot = ""

# ---------------------------------------------------------------- 输出辅助
function Write-Section {
    param([string]$Text)
    Write-Host ""
    Write-Host ("== " + $Text + " ==") -ForegroundColor Cyan
}

function Write-Item {
    param([string]$Text)
    Write-Host ("   " + $Text)
}

function Write-Ok {
    param([string]$Text)
    Write-Host ("   [OK] " + $Text) -ForegroundColor Green
}

function Write-Warn {
    param([string]$Text)
    Write-Host ("   [WARN] " + $Text) -ForegroundColor Yellow
}

function Write-Fail {
    param([string]$Text)
    Write-Host ("   [ERROR] " + $Text) -ForegroundColor Red
}

# ---------------------------------------------------------------- git 调用
# 返回 { ExitCode; StdOut[string[]]; Lines[string[]]; Text }。
# 本地把 $ErrorActionPreference 降为 Continue，避免 PS5.1 在 2>&1 捕获 stderr 时抛错。
function Invoke-GitRaw {
    param([string[]]$GitArgs)

    $oldEap = $ErrorActionPreference
    $output = @()
    $code = 0
    try {
        $ErrorActionPreference = "Continue"
        $output = @(& git -C $script:RepoRoot -c core.quotepath=false @GitArgs 2>&1)
        $code = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $oldEap
    }

    $stdout = New-Object System.Collections.Generic.List[string]
    $all = New-Object System.Collections.Generic.List[string]
    foreach ($item in $output) {
        if ($item -is [System.Management.Automation.ErrorRecord]) {
            $all.Add($item.ToString())
        }
        else {
            $s = [string]$item
            $stdout.Add($s)
            $all.Add($s)
        }
    }

    return [pscustomobject]@{
        ExitCode = $code
        StdOut   = $stdout.ToArray()
        Lines    = $all.ToArray()
        Text     = ($all.ToArray() -join "`n")
    }
}

# 取 NUL 分隔的路径清单（避免引号/空格/中文转义问题）。
function Get-GitPathList {
    param([string[]]$GitArgs)

    $r = Invoke-GitRaw -GitArgs $GitArgs
    if ($r.ExitCode -ne 0) {
        Write-Fail ("git " + ($GitArgs -join " ") + " 失败（退出码 " + $r.ExitCode + "）")
        if ($r.Text) { Write-Host $r.Text -ForegroundColor DarkGray }
        exit $EXIT_PRECHECK
    }

    $list = New-Object System.Collections.Generic.List[string]
    if ($r.StdOut.Count -gt 0) {
        $raw = ($r.StdOut -join "`n")
        foreach ($p in ($raw -split "`0")) {
            if ($p -ne "") { $list.Add($p) }
        }
    }
    return $list.ToArray()
}

function Get-CurrentBranchName {
    $r = Invoke-GitRaw -GitArgs @('symbolic-ref', '-q', '--short', 'HEAD')
    if ($r.ExitCode -ne 0 -or $r.StdOut.Count -eq 0) { return "" }
    $b = $r.StdOut[0].Trim()
    if ($b -eq "HEAD") { return "" }
    return $b
}

# HEAD 里已跟踪的路径集合（DryRun 与暂存排除共用）。
# 必须用 ls-tree HEAD 而不是 ls-files：git add -A 之后 ls-files 也会列出“刚被
# 暂存的未跟踪文件”，无法区分“原本就跟踪”和“本次新纳入”。
function Get-HeadTrackedMap {
    $map = @{}
    $r = Invoke-GitRaw -GitArgs @('ls-tree', '-r', '-z', '--name-only', 'HEAD')
    if ($r.ExitCode -ne 0) { return $map }
    $raw = ($r.StdOut -join "`n")
    foreach ($p in ($raw -split "`0")) {
        if ($p -ne "") { $map[$p] = $true }
    }
    return $map
}

# 本地分支相对“推送目标”领先的提交数；-1 表示远端分支尚不存在（推送会创建）。
# 对比 Remote/Branch 而不是 @{u}：脚本 push 的目标就是 Remote/Branch，而且新
# 分支可能从未设置 upstream，@{u} 会直接报错。
function Get-AheadCount {
    param([string]$RemoteName, [string]$TargetBranch)

    $upstream = $RemoteName + '/' + $TargetBranch
    $has = Invoke-GitRaw -GitArgs @('rev-parse', '--verify', '--quiet', ('refs/remotes/' + $upstream))
    if ($has.ExitCode -ne 0) { return -1 }
    $r = Invoke-GitRaw -GitArgs @('rev-list', '--count', ($upstream + '..HEAD'))
    if ($r.ExitCode -ne 0 -or $r.StdOut.Count -eq 0) { return -1 }
    try { return [int]($r.StdOut[0].Trim()) } catch { return -1 }
}

# main 同步策略：默认只有 dev 自动同步；其它分支必须显式 -SyncMain。
function Test-MainSyncAllowed {
    param([string]$TargetBranch)
    if ($TargetBranch -eq "main") { return $false }
    if ($TargetBranch -eq "dev") { return $true }
    return [bool]$SyncMain
}

# 远程是否存在 main 分支：$true / $false；$null = git ls-remote 失败（无法确认）。
function Test-RemoteMainExists {
    param([string]$RemoteName)
    $r = Invoke-GitRaw -GitArgs @('ls-remote', '--heads', $RemoteName, 'main')
    if ($r.ExitCode -ne 0) { return $null }
    foreach ($line in $r.StdOut) {
        if ($line -match '\srefs/heads/main$') { return $true }
    }
    return $false
}

# ---------------------------------------------------------------- 瞬态排除规则
# 返回排除原因（字符串）或 $null（表示应提交）。
function Get-TransientReason {
    param([string]$RelPath)

    if (-not $RelPath) { return $null }
    $p = $RelPath.Replace('\', '/')
    $segments = @($p.Split('/') | Where-Object { $_ -ne "" })
    if ($segments.Count -eq 0) { return $null }
    $name = $segments[$segments.Count - 1]

    foreach ($pattern in @('*.bak', '*.tmp', '*.log')) {
        if ($name -like $pattern) { return ("瞬态文件（" + $pattern + "）") }
    }

    foreach ($exact in @(
            'scheduler.state.json',
            'scheduler.events.jsonl',
            'scheduler.state.json.lock',
            'scheduler.events.jsonl.lock',
            'scheduler.state.lock')) {
        if ($name -eq $exact) { return ("调度运行状态（" + $exact + "）") }
    }

    for ($i = 0; $i -lt ($segments.Count - 1); $i++) {
        if ($segments[$i] -eq 'logs') { return "运行日志目录（logs/）" }
    }

    if ($p -like '_verify/*') {
        for ($i = 0; $i -lt ($segments.Count - 1); $i++) {
            if ($segments[$i] -eq 'target') { return "构建产物（_verify/**/target/）" }
        }
    }

    return $null
}

# 只读预览：git add -A 将会影响的路径（已变更 + 未跟踪且未被 ignore），并分组：
#   Keep        = 会提交（普通文件 + 已跟踪的瞬态文件）
#   Drop        = 未跟踪的瞬态文件（会从暂存区 reset 掉）
#   TrackedKept = 已在 HEAD 跟踪的瞬态文件（保留在暂存区，不 reset）
function Get-PlannedChanges {
    $paths = New-Object System.Collections.Generic.List[string]
    $argSets = @(
        @('diff', '--name-only', '--no-renames', '-z'),
        @('diff', '--cached', '--name-only', '--no-renames', '-z'),
        @('ls-files', '--others', '--exclude-standard', '-z')
    )
    foreach ($argSet in $argSets) {
        foreach ($p in @(Get-GitPathList -GitArgs $argSet)) {
            if (-not $paths.Contains($p)) { $paths.Add($p) }
        }
    }

    $tracked = Get-HeadTrackedMap
    $keep = New-Object System.Collections.Generic.List[string]
    $drop = New-Object System.Collections.Generic.List[string]
    $trackedKept = New-Object System.Collections.Generic.List[object]
    foreach ($p in $paths) {
        $reason = Get-TransientReason -RelPath $p
        if ($null -eq $reason) {
            $keep.Add($p)
        }
        elseif ($tracked.ContainsKey($p)) {
            $trackedKept.Add([pscustomobject]@{ Path = $p; Reason = $reason })
        }
        else {
            $drop.Add($p)
        }
    }

    return [pscustomobject]@{
        Keep        = $keep.ToArray()
        Drop        = $drop.ToArray()
        TrackedKept = $trackedKept.ToArray()
    }
}

function Restore-TargetBranch {
    param([string]$TargetBranch)

    $cur = Get-CurrentBranchName
    if ($cur -eq $TargetBranch) { return $true }

    Write-Item ("恢复分支 " + $TargetBranch)
    $r = Invoke-GitRaw -GitArgs @('checkout', $TargetBranch)
    if ($r.ExitCode -ne 0) {
        Write-Fail ("恢复分支失败，请手动执行 git checkout " + $TargetBranch)
        if ($r.Text) { Write-Host $r.Text }
        return $false
    }
    return $true
}

# 仅在策略允许时被调用（dev 或显式 -SyncMain）：fetch -> checkout main ->
# merge --ff-only -> push main -> checkout 回原分支。
# 任一步失败：尽量切回原分支，以退出码 5 结束（绝不 force push / 绝不强行 merge）。
function Sync-MainBranch {
    param([string]$TargetBranch, [string]$RemoteName)

    Write-Section "同步 main（fast-forward）"

    $fetch = Invoke-GitRaw -GitArgs @('fetch', $RemoteName)
    if ($fetch.ExitCode -ne 0) {
        Write-Fail ("git fetch " + $RemoteName + " 失败（退出码 " + $fetch.ExitCode + "）；未切换分支")
        if ($fetch.Text) { Write-Host $fetch.Text }
        exit $EXIT_PUSH
    }
    Write-Ok ("已 fetch " + $RemoteName)

    $localMain = Invoke-GitRaw -GitArgs @('rev-parse', '--verify', '--quiet', 'refs/heads/main')
    if ($localMain.ExitCode -ne 0) {
        $remoteMainRef = 'refs/remotes/' + $RemoteName + '/main'
        $remoteMain = Invoke-GitRaw -GitArgs @('rev-parse', '--verify', '--quiet', $remoteMainRef)
        if ($remoteMain.ExitCode -ne 0) {
            # 调用方已用 ls-remote 预检过；这里只是并发/竞态兜底：不把已成功的
            # dev 推送变成失败退出。
            Write-Warn "本地和远程都没有 main 分支，跳过 main 同步（当前分支已推送）"
            return
        }
        Write-Item ("本地没有 main，从 " + $RemoteName + "/main 创建跟踪分支")
        $co = Invoke-GitRaw -GitArgs @('checkout', '-b', 'main', '--track', ($RemoteName + '/main'))
    }
    else {
        $co = Invoke-GitRaw -GitArgs @('checkout', 'main')
    }
    if ($co.ExitCode -ne 0) {
        Write-Fail "切换到 main 失败；未做任何合并，仍在原分支"
        if ($co.Text) { Write-Host $co.Text }
        exit $EXIT_PUSH
    }
    Write-Ok "已切换到 main"

    $merge = Invoke-GitRaw -GitArgs @('merge', '--ff-only', $TargetBranch)
    if ($merge.ExitCode -ne 0) {
        Write-Fail ("main 无法 fast-forward 到 " + $TargetBranch + "（存在分叉），按约定不强行合并")
        if ($merge.Text) { Write-Host $merge.Text }
        if (-not (Restore-TargetBranch -TargetBranch $TargetBranch)) { exit $EXIT_PUSH }
        exit $EXIT_PUSH
    }
    Write-Ok ("main 已 fast-forward 到 " + $TargetBranch)

    $pushMain = Invoke-GitRaw -GitArgs @('push', $RemoteName, 'main')
    if ($pushMain.ExitCode -ne 0) {
        Write-Fail ("推送 main 失败（退出码 " + $pushMain.ExitCode + "）")
        if ($pushMain.Text) { Write-Host $pushMain.Text }
        if (-not (Restore-TargetBranch -TargetBranch $TargetBranch)) { exit $EXIT_PUSH }
        exit $EXIT_PUSH
    }
    Write-Ok ("已推送 " + $RemoteName + "/main")

    if (-not (Restore-TargetBranch -TargetBranch $TargetBranch)) { exit $EXIT_PUSH }
    Write-Ok ("已切回分支 " + $TargetBranch)
}

# push 当前分支；失败保留本地 commit 并以 5 退出。成功时原样打印 git stdout
# （可能包含 remote URL，但不含密码；见脚本头 NOTES）。
function Push-Branch {
    param([string]$TargetBranch, [string]$RemoteName)

    Write-Section ("推送 " + $RemoteName + "/" + $TargetBranch)
    $pushResult = Invoke-GitRaw -GitArgs @('push', $RemoteName, $TargetBranch)
    if ($pushResult.ExitCode -ne 0) {
        Write-Fail ("git push " + $RemoteName + " " + $TargetBranch + " 失败（退出码 " + $pushResult.ExitCode + "）")
        Write-Item "commit 已保存在本地，未推送；工作区未做任何回滚。"
        Write-Item "排除网络/认证问题后重跑本脚本：无新改动时会走「只推送」路径补推。"
        if ($pushResult.Text) { Write-Host $pushResult.Text }
        exit $EXIT_PUSH
    }
    Write-Ok ("已推送 " + $RemoteName + "/" + $TargetBranch)
    foreach ($line in $pushResult.StdOut) {
        if ($line.Trim() -ne "") { Write-Item $line.Trim() }
    }
}

# 按策略决定并执行 main 同步。跳过时只打印 WARN（脚本继续成功）。
# 返回 $true 表示本次确实完成 main fast-forward + push。
function Invoke-MainSyncPolicy {
    param([string]$TargetBranch, [string]$RemoteName)

    if ($TargetBranch -eq "main") {
        Write-Ok "当前分支就是 main，无需额外同步"
        return $false
    }
    if (-not (Test-MainSyncAllowed -TargetBranch $TargetBranch)) {
        Write-Warn ("分支 " + $TargetBranch + " 不自动同步 main（仅 dev）；如需发布请手动或加 -SyncMain")
        return $false
    }
    $remoteMain = Test-RemoteMainExists -RemoteName $RemoteName
    if ($remoteMain -eq $false) {
        Write-Warn ("远程 " + $RemoteName + "/main 不存在；跳过 main 同步（当前分支已推送）")
        return $false
    }
    if ($null -eq $remoteMain) {
        if ($SyncMain) {
            Write-Fail ("无法确认远程 " + $RemoteName + "/main（git ls-remote 失败）；已跳过 main 同步")
            exit $EXIT_PUSH
        }
        Write-Warn ("无法确认远程 " + $RemoteName + "/main（git ls-remote 失败）；已跳过 main 同步")
        return $false
    }
    Sync-MainBranch -TargetBranch $TargetBranch -RemoteName $RemoteName
    return $true
}

# DryRun 的 main 同步计划：不访问网络（不执行 ls-remote），只打印确定性命令。
function Write-MainSyncPlan {
    param([string]$TargetBranch, [string]$RemoteName)

    if ($TargetBranch -eq "main") {
        Write-Item "当前分支就是 main：不同步 main"
        return
    }
    if (-not (Test-MainSyncAllowed -TargetBranch $TargetBranch)) {
        Write-Item ("[WARN] 分支 " + $TargetBranch + " 不自动同步 main（仅 dev）；如需发布请手动或加 -SyncMain")
        return
    }
    Write-Item ("git ls-remote --heads " + $RemoteName + " main（无 main 则 WARN 跳过，仍 exit 0）")
    Write-Item ("git fetch " + $RemoteName)
    Write-Item "git checkout main"
    Write-Item ("git merge --ff-only " + $TargetBranch)
    Write-Item ("git push " + $RemoteName + " main")
    Write-Item ("git checkout " + $TargetBranch)
}

# 收尾摘要（commit 路径与“只推送”路径共用）。
function Show-Completion {
    param(
        [string]$CommitHash,
        [string]$TargetBranch,
        [string]$RemoteName,
        [bool]$NoPushFlag,
        [bool]$PushedFlag,
        [bool]$MainUpdatedFlag,
        [bool]$PushOnlyFlag,
        [int]$ExcludedUntracked,
        [int]$ExcludedTracked
    )

    Write-Section "完成"
    if ($PushOnlyFlag) {
        Write-Item ("commit : " + $CommitHash + "（未新建提交；补推已有 commit）")
    }
    else {
        Write-Item ("commit : " + $CommitHash)
    }
    Write-Item ("branch : " + $TargetBranch)
    if ($NoPushFlag) {
        Write-Item "push   : 已跳过（-NoPush）"
    }
    elseif ($PushedFlag) {
        Write-Item ("push   : " + $RemoteName + "/" + $TargetBranch + " 已推送")
    }
    else {
        Write-Item "push   : 未执行"
    }
    if ($NoPushFlag) {
        Write-Item "main   : 未同步（-NoPush）"
    }
    elseif ($TargetBranch -eq "main") {
        Write-Item "main   : 当前分支就是 main"
    }
    elseif ($MainUpdatedFlag) {
        Write-Item ("main   : 已 fast-forward 并推送到 " + $RemoteName + "/main")
    }
    else {
        Write-Item "main   : 未同步（未满足条件或远程无 main，见上方 WARN）"
    }
    if ($ExcludedUntracked -gt 0) {
        Write-Item ("排除项 : " + $ExcludedUntracked + " 个未跟踪瞬态文件未提交（仍保留在工作区）")
    }
    if ($ExcludedTracked -gt 0) {
        Write-Item ("已跟踪 : " + $ExcludedTracked + " 个匹配排除规则的文件仍在 HEAD 中，已保留并提交")
    }

    Write-Item "剩余工作区（git status --short）："
    $finalStatus = Invoke-GitRaw -GitArgs @('status', '--short')
    if ($finalStatus.StdOut.Count -eq 0) {
        Write-Item "  （干净）"
    }
    else {
        foreach ($line in $finalStatus.StdOut) { Write-Item ("  " + $line) }
    }
    Write-Item "提示：未跟踪且被排除的文件（如 scheduler.json.bak）属于预期残留，不应提交。"
}

# ================================================================ 1. 定位仓库根
$ScriptDir = $PSScriptRoot
if (-not $ScriptDir) {
    $ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
}
if (-not $ScriptDir) {
    Write-Fail "无法定位脚本所在目录"
    exit $EXIT_PRECHECK
}

$Root = Split-Path -Parent $ScriptDir
if (-not (Test-Path -LiteralPath $Root -PathType Container)) {
    Write-Fail ("仓库根不存在：" + $Root)
    exit $EXIT_PRECHECK
}
$script:RepoRoot = $Root

Write-Host ""
Write-Host "OAS 一键提交/推送（push-all）" -ForegroundColor Cyan
Write-Item ("脚本目录 : " + $ScriptDir)
Write-Item ("仓库根   : " + $Root)

if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
    Write-Fail "未找到 git 可执行文件（请安装 Git for Windows 并加入 PATH）"
    exit $EXIT_PRECHECK
}

$probe = Invoke-GitRaw -GitArgs @('rev-parse', '--show-toplevel')
if ($probe.ExitCode -ne 0 -or $probe.StdOut.Count -eq 0) {
    Write-Fail ("不是 git 仓库（或 .git 不可用）：" + $Root)
    if ($probe.Text) { Write-Host $probe.Text -ForegroundColor DarkGray }
    exit $EXIT_PRECHECK
}

$topRaw = $probe.StdOut[0].Trim()
$topFull = [System.IO.Path]::GetFullPath($topRaw).TrimEnd('\', '/')
$rootFull = [System.IO.Path]::GetFullPath($Root).TrimEnd('\', '/')
if (-not ($topFull -ieq $rootFull)) {
    Write-Fail ("仓库根不匹配：脚本位于 " + $rootFull + "，但 git 顶层是 " + $topFull)
    Write-Item "请把脚本放在仓库根的 scripts/ 目录下再运行。"
    exit $EXIT_PRECHECK
}
Write-Ok ("git 仓库根校验通过：" + $topFull)

# ================================================================ 2. 预检
Write-Section "预检"

$conflicts = @(Get-GitPathList -GitArgs @('diff', '--name-only', '--diff-filter=U', '-z'))
if ($conflicts.Count -gt 0) {
    Write-Fail ("存在未解决的合并冲突（" + $conflicts.Count + " 个文件），请先解决后再运行")
    foreach ($c in $conflicts) { Write-Item $c }
    exit $EXIT_PRECHECK
}
Write-Ok "无未解决冲突"

$headCheck = Invoke-GitRaw -GitArgs @('rev-parse', '--verify', '--quiet', 'HEAD')
if ($headCheck.ExitCode -ne 0) {
    Write-Fail "仓库还没有任何提交（unborn HEAD）；请先手动完成首次提交"
    exit $EXIT_PRECHECK
}

$currentBranch = Get-CurrentBranchName
if (-not $currentBranch) {
    Write-Fail "当前处于 detached HEAD，无法确定要推送的分支。请先 git checkout <branch>"
    exit $EXIT_DETACHED
}

$targetBranch = $currentBranch
if ($Branch -ne "") {
    if ($Branch -cne $currentBranch) {
        Write-Fail ("-Branch " + $Branch + " 与当前分支 " + $currentBranch + " 不一致")
        Write-Item "为避免误提交/误推送，请先手动切换到目标分支，或不传 -Branch。"
        exit $EXIT_PRECHECK
    }
    $targetBranch = $Branch
}
Write-Ok ("当前分支：" + $targetBranch)

$remoteList = Invoke-GitRaw -GitArgs @('remote')
if ($remoteList.ExitCode -ne 0) {
    Write-Fail ("git remote 执行失败（退出码 " + $remoteList.ExitCode + "）")
    exit $EXIT_PRECHECK
}
$remoteNames = @($remoteList.StdOut | ForEach-Object { $_.Trim() } | Where-Object { $_ -ne "" })
if (-not ($remoteNames -contains $Remote)) {
    Write-Fail ("远程 '" + $Remote + "' 不存在。已配置远程：" + ($remoteNames -join ", "))
    exit $EXIT_PRECHECK
}
Write-Ok ("远程 " + $Remote + " 可用")

$commitMessage = $Message
if ($null -eq $commitMessage -or $commitMessage.Trim() -eq "") {
    $commitMessage = "chore(data): sync all resources/tasks/scheduler/code/docs " + (Get-Date -Format "yyyy-MM-dd HH:mm")
}

# 非交互终端：无法回答 y/N，必须显式 -Yes；提前在写索引之前退出。
$interactive = $true
try {
    $interactive = -not [Console]::IsInputRedirected
}
catch {
    $interactive = $false
}
if (-not $interactive) { $env:GIT_TERMINAL_PROMPT = "0" }

if ($DryRun) {
    # ------------------------------------------------------------ DRY-RUN（只读）
    $planned = Get-PlannedChanges
    $ahead = Get-AheadCount -RemoteName $Remote -TargetBranch $targetBranch

    if ($targetBranch -eq "main") { $syncMode = "当前分支就是 main，不同步" }
    elseif (Test-MainSyncAllowed -TargetBranch $targetBranch) {
        if ($targetBranch -eq "dev") { $syncMode = "允许（dev 自动同步）" }
        else { $syncMode = "允许（-SyncMain 显式指定）" }
    }
    else { $syncMode = "跳过（非 dev 且未 -SyncMain，WARN 后 exit 0）" }

    Write-Section "DRY-RUN 预览（不写索引 / 不 commit / 不 push）"
    Write-Item ("仓库根   : " + $rootFull)
    Write-Item ("当前分支 : " + $targetBranch)
    Write-Item ("远程     : " + $Remote)
    $headHash = Invoke-GitRaw -GitArgs @('rev-parse', '--short', 'HEAD')
    if ($headHash.ExitCode -eq 0 -and $headHash.StdOut.Count -gt 0) {
        Write-Item ("HEAD     : " + $headHash.StdOut[0].Trim())
    }
    Write-Item ("提交信息 : " + $commitMessage)
    Write-Item ("main 同步 : " + $syncMode)
    Write-Item "main 规则 : 默认仅 dev 自动同步 main；其它分支需 -SyncMain；远程无 main 则 WARN 跳过（仍 exit 0）"
    Write-Item "无 staged : 本地领先上游且未 -NoPush 时走「只推送」路径补推已有 commit"

    $trackingRef = 'refs/remotes/' + $Remote + '/' + $targetBranch
    $hasTracking = (Invoke-GitRaw -GitArgs @('rev-parse', '--verify', '--quiet', $trackingRef)).ExitCode -eq 0
    if ($hasTracking) {
        $lr = Invoke-GitRaw -GitArgs @('rev-list', '--left-right', '--count', ($Remote + '/' + $targetBranch + '...HEAD'))
        if ($lr.ExitCode -eq 0 -and $lr.StdOut.Count -gt 0) {
            $parts = @($lr.StdOut[0].Trim() -split '\s+')
            if ($parts.Count -ge 2) {
                Write-Item ("相对 " + $Remote + "/" + $targetBranch + " : 落后 " + $parts[0] + " 个提交，领先 " + $parts[1] + " 个提交")
            }
        }
    }
    else {
        Write-Item ("远程 " + $Remote + "/" + $targetBranch + " 尚不存在（推送时将新建）")
    }

    Write-Section "工作区状态（git status --short，最多 40 行）"
    $st = Invoke-GitRaw -GitArgs @('status', '--short')
    if ($st.StdOut.Count -eq 0) {
        Write-Item "（干净）"
    }
    else {
        $limit = 40
        $i = 0
        foreach ($line in $st.StdOut) {
            if ($i -ge $limit) { break }
            Write-Item $line
            $i++
        }
        if ($st.StdOut.Count -gt $limit) {
            Write-Item ("... 其余 " + ($st.StdOut.Count - $limit) + " 行省略")
        }
    }

    Write-Section ("将提交（" + $planned.Keep.Count + " 个文件）")
    if ($planned.Keep.Count -eq 0) {
        Write-Item "（无）"
    }
    else {
        $showKeep = [Math]::Min(30, $planned.Keep.Count)
        for ($i = 0; $i -lt $showKeep; $i++) { Write-Item $planned.Keep[$i] }
        if ($planned.Keep.Count -gt $showKeep) {
            Write-Item ("... 还有 " + ($planned.Keep.Count - $showKeep) + " 个文件")
        }
    }

    Write-Section ("已排除的瞬态文件（" + $planned.Drop.Count + " 个，不会提交）")
    if ($planned.Drop.Count -eq 0) {
        Write-Item "（当前没有匹配排除规则的未跟踪改动）"
    }
    else {
        foreach ($p in $planned.Drop) {
            Write-Item ($p + "    <- " + (Get-TransientReason -RelPath $p))
        }
    }

    Write-Section ("已跟踪但匹配排除规则（" + $planned.TrackedKept.Count + " 个，保留在暂存区）")
    if ($planned.TrackedKept.Count -eq 0) {
        Write-Item "（无）"
    }
    else {
        foreach ($t in $planned.TrackedKept) {
            Write-Item ($t.Path + "    <- " + $t.Reason + "（已在 HEAD 跟踪，不按瞬态剔除）")
        }
    }

    Write-Section "将执行（计划）"
    if ($planned.Keep.Count -eq 0) {
        Write-Item "（无 staged 改动：不执行 git add / commit）"
        if ($NoPush) {
            Write-Item "（-NoPush：不推送，也不同步 main）"
            if ($ahead -ne 0) {
                Write-Item ("本地仍有未推送 commit（与 " + $Remote + "/" + $targetBranch + " 不同）；本次不会推送")
            }
        }
        elseif ($ahead -ne 0) {
            Write-Item ("git push " + $Remote + " " + $targetBranch + "    <- 「只推送」路径：补推本地已有 commit")
            if ($ahead -lt 0) {
                Write-Item "（远程分支尚不存在；push 将创建它）"
            }
            else {
                Write-Item ("（本地领先 " + $Remote + "/" + $targetBranch + " " + $ahead + " 个提交）")
            }
            Write-MainSyncPlan -TargetBranch $targetBranch -RemoteName $Remote
        }
        else {
            Write-Item "（无 staged 改动且不领先上游：不 commit、不 push，exit 0）"
        }
    }
    else {
        Write-Item "git add -A"
        if ($planned.Drop.Count -gt 0) {
            Write-Item ("git reset -q -- <" + $planned.Drop.Count + " 个未跟踪排除项>（已跟踪的排除项不 reset）")
        }
        if ($planned.TrackedKept.Count -gt 0) {
            Write-Item ("（" + $planned.TrackedKept.Count + " 个已跟踪的排除项保留在暂存区并提交）")
        }
        Write-Item ('git commit -m "' + $commitMessage + '"')
        if ($NoPush) {
            Write-Item "（-NoPush：不推送，也不同步 main）"
        }
        else {
            Write-Item ("git push " + $Remote + " " + $targetBranch)
            Write-MainSyncPlan -TargetBranch $targetBranch -RemoteName $Remote
        }
    }

    Write-Host ""
    if ($planned.Keep.Count -eq 0 -and $ahead -ne 0 -and -not $NoPush) {
        Write-Item "无 staged 改动：将走「只推送」路径补推本地领先 commit"
    }
    elseif ($planned.Keep.Count -eq 0) {
        Write-Item "没有需要提交的改动"
    }
    Write-Ok "DRY-RUN 结束：未修改索引、未 commit、未 push"
    exit $EXIT_OK
}

if (-not $Yes -and -not $interactive) {
    Write-Section "需要确认，但当前终端不可交互"
    Write-Fail "stdin 已被重定向，无法询问 y/N；本次未暂存、未提交、未推送"
    Write-Item "非交互运行请追加 -Yes；只想预览请用 -DryRun。"
    $preview = Invoke-GitRaw -GitArgs @('status', '--short')
    if ($preview.StdOut.Count -gt 0) {
        Write-Item ("当前工作区改动行数：" + $preview.StdOut.Count)
    }
    exit $EXIT_CANCELLED
}

# ================================================================ 3. 暂存全部 + 剔除瞬态
Write-Section "暂存全部可版本化改动"
$addResult = Invoke-GitRaw -GitArgs @('add', '-A')
if ($addResult.ExitCode -ne 0) {
    Write-Fail ("git add -A 失败（退出码 " + $addResult.ExitCode + "）")
    if ($addResult.Text) { Write-Host $addResult.Text }
    exit $EXIT_PRECHECK
}
Write-Ok "git add -A 完成（自动遵循 .gitignore）"

$trackedInHead = Get-HeadTrackedMap
$stagedAll = @(Get-GitPathList -GitArgs @('diff', '--cached', '--name-only', '--no-renames', '-z'))
$excludedUntracked = New-Object System.Collections.Generic.List[string]
$excludedTracked = New-Object System.Collections.Generic.List[object]
foreach ($p in $stagedAll) {
    $reason = Get-TransientReason -RelPath $p
    if ($null -eq $reason) { continue }
    if ($trackedInHead.ContainsKey($p)) {
        # P2-a: 已在 HEAD 中跟踪的文件（例如 _verify/ 下的历史 .log）不是“瞬态
        # 产物”，不能因为匹配排除模式就从暂存区剔除。
        $excludedTracked.Add([pscustomobject]@{ Path = $p; Reason = $reason })
    }
    else {
        $excludedUntracked.Add($p)
    }
}

if ($excludedTracked.Count -gt 0) {
    Write-Section ("已跟踪的瞬态文件（" + $excludedTracked.Count + " 个，保留在暂存区）")
    foreach ($t in $excludedTracked) {
        Write-Warn ($t.Path + "    <- " + $t.Reason + "；已在 HEAD 跟踪，不按瞬态剔除")
    }
}

if ($excludedUntracked.Count -gt 0) {
    Write-Section ("剔除未跟踪的瞬态文件（" + $excludedUntracked.Count + " 个）")
    foreach ($p in $excludedUntracked) {
        Write-Item ($p + "    <- " + (Get-TransientReason -RelPath $p))
    }

    $chunkSize = 40
    for ($i = 0; $i -lt $excludedUntracked.Count; $i += $chunkSize) {
        $take = [Math]::Min($chunkSize, $excludedUntracked.Count - $i)
        $chunk = @($excludedUntracked.GetRange($i, $take).ToArray())
        # :(literal) 防止文件名中的 * ? [ ] 被当作 pathspec 通配（P2-10）。
        $resetPaths = @($chunk | ForEach-Object { ':(literal)' + $_ })
        $resetArgs = @('reset', '-q', '--') + $resetPaths
        $resetResult = Invoke-GitRaw -GitArgs $resetArgs
        if ($resetResult.ExitCode -ne 0) {
            Write-Fail ("git reset 剔除失败（退出码 " + $resetResult.ExitCode + "）")
            if ($resetResult.Text) { Write-Host $resetResult.Text }
            exit $EXIT_PRECHECK
        }
    }

    # 安全复核：未跟踪的瞬态文件绝不允许残留；已跟踪文件是刻意保留的。
    $stagedCheck = @(Get-GitPathList -GitArgs @('diff', '--cached', '--name-only', '--no-renames', '-z'))
    $leak = @($stagedCheck | Where-Object {
            ($null -ne (Get-TransientReason -RelPath $_)) -and (-not $trackedInHead.ContainsKey($_))
        })
    if ($leak.Count -gt 0) {
        Write-Fail ("剔除后仍有未跟踪的瞬态文件在暂存区（" + $leak.Count + " 个），已中止，未提交")
        foreach ($p in $leak) { Write-Item $p }
        exit $EXIT_PRECHECK
    }
    Write-Ok "未跟踪的瞬态文件已从暂存区剔除"
}
else {
    Write-Item "没有需要剔除的未跟踪瞬态文件"
}

$staged = @(Get-GitPathList -GitArgs @('diff', '--cached', '--name-only', '--no-renames', '-z'))
if ($staged.Count -eq 0) {
    Write-Section "结果"
    Write-Item "没有需要提交的改动"
    $ahead = Get-AheadCount -RemoteName $Remote -TargetBranch $targetBranch
    if ($NoPush) {
        if ($ahead -ne 0) {
            Write-Item ("本地与 " + $Remote + "/" + $targetBranch + " 不同（-NoPush 不推送）；如需补推请去掉 -NoPush 重跑")
        }
        Write-Ok "未做任何提交/推送"
        exit $EXIT_OK
    }
    if ($ahead -eq 0) {
        Write-Ok ("本地 " + $targetBranch + " 已与 " + $Remote + "/" + $targetBranch + " 同步，无需推送")
        exit $EXIT_OK
    }

    # P2-b：上次 push / main 同步失败后重跑——没有新 staged 改动，但本地领先
    # 上游，走“只推送”路径补推已有 commit（仍按 dev 规则同步 main）。
    Write-Section "只推送（补推本地已有 commit）"
    if ($ahead -lt 0) {
        Write-Item ("远程 " + $Remote + "/" + $targetBranch + " 尚不存在（或无法比较）；将创建并推送")
    }
    else {
        Write-Item ("本地领先 " + $Remote + "/" + $targetBranch + " " + $ahead + " 个提交")
    }
    Push-Branch -TargetBranch $targetBranch -RemoteName $Remote
    $mainUpdated = Invoke-MainSyncPolicy -TargetBranch $targetBranch -RemoteName $Remote
    $hashResult = Invoke-GitRaw -GitArgs @('rev-parse', '--short', 'HEAD')
    $commitHash = ""
    if ($hashResult.ExitCode -eq 0 -and $hashResult.StdOut.Count -gt 0) {
        $commitHash = $hashResult.StdOut[0].Trim()
    }
    Show-Completion -CommitHash $commitHash -TargetBranch $targetBranch -RemoteName $Remote `
        -NoPushFlag $false -PushedFlag $true -MainUpdatedFlag $mainUpdated -PushOnlyFlag $true `
        -ExcludedUntracked $excludedUntracked.Count -ExcludedTracked $excludedTracked.Count
    exit $EXIT_OK
}

# ================================================================ 4. 提交清单摘要
Write-Section "提交清单"
$nameStatus = Invoke-GitRaw -GitArgs @('diff', '--cached', '--name-status', '--no-renames')
if ($nameStatus.ExitCode -ne 0) {
    Write-Fail ("git diff --cached --name-status 失败（退出码 " + $nameStatus.ExitCode + "）")
    exit $EXIT_PRECHECK
}

$added = 0
$modified = 0
$deleted = 0
$other = 0
$rows = New-Object System.Collections.Generic.List[object]
foreach ($line in $nameStatus.StdOut) {
    if (-not $line) { continue }
    $tab = $line.IndexOf("`t")
    if ($tab -lt 1) { continue }
    $statusCode = $line.Substring(0, 1)
    $relPath = $line.Substring($tab + 1)
    switch ($statusCode) {
        'A' { $added++ }
        'M' { $modified++ }
        'D' { $deleted++ }
        default { $other++ }
    }
    $rows.Add([pscustomobject]@{ Status = $statusCode; Path = $relPath })
}

$countLine = "新增 " + $added + "   修改 " + $modified + "   删除 " + $deleted
if ($other -gt 0) { $countLine = $countLine + "   其他 " + $other }
Write-Item $countLine
Write-Item ("涉及文件 " + $rows.Count + " 个，暂存区共 " + $staged.Count + " 个")

$showCount = [Math]::Min(20, $rows.Count)
for ($i = 0; $i -lt $showCount; $i++) {
    Write-Item ($rows[$i].Status + "  " + $rows[$i].Path)
}
if ($rows.Count -gt $showCount) {
    Write-Item ("... 还有 " + ($rows.Count - $showCount) + " 个文件未显示")
}

$statResult = Invoke-GitRaw -GitArgs @('diff', '--cached', '--stat')
if ($statResult.ExitCode -eq 0 -and $statResult.StdOut.Count -gt 0) {
    Write-Item "统计（截断）："
    if ($statResult.StdOut.Count -gt 14) {
        for ($i = 0; $i -lt 12; $i++) { Write-Item ("  " + $statResult.StdOut[$i]) }
        Write-Item "  ..."
        Write-Item ("  " + $statResult.StdOut[$statResult.StdOut.Count - 1])
    }
    else {
        foreach ($statLine in $statResult.StdOut) { Write-Item ("  " + $statLine) }
    }
}

# ================================================================ 5. 确认
if (-not $Yes) {
    Write-Host ""
    Write-Host ("   即将提交 " + $staged.Count + " 个文件到分支 " + $targetBranch + "，并推送到 " + $Remote + "/" + $targetBranch) -ForegroundColor Yellow
    $answer = ""
    try {
        $answer = Read-Host "   确认继续? 输入 y 确认，其他任意输入取消"
    }
    catch {
        Write-Fail "读取确认失败（非交互终端？请使用 -Yes）"
        exit $EXIT_CANCELLED
    }
    if ($answer -notmatch '^\s*(y|yes|是)\s*$') {
        Write-Section "已取消"
        Write-Item "未提交、未推送；改动仍保留在暂存区。"
        Write-Item "如需取消暂存：git reset -q -- ."
        exit $EXIT_CANCELLED
    }
}

# ================================================================ 6. commit
Write-Section "提交"
$commitResult = Invoke-GitRaw -GitArgs @('commit', '-m', $commitMessage)
if ($commitResult.ExitCode -ne 0) {
    Write-Fail ("git commit 失败（退出码 " + $commitResult.ExitCode + "）")
    if ($commitResult.Text) { Write-Host $commitResult.Text }
    exit $EXIT_COMMIT
}
foreach ($line in $commitResult.StdOut) {
    if ($line.Trim() -ne "") { Write-Item $line.Trim() }
}

$hashResult = Invoke-GitRaw -GitArgs @('rev-parse', '--short', 'HEAD')
$commitHash = ""
if ($hashResult.ExitCode -eq 0 -and $hashResult.StdOut.Count -gt 0) {
    $commitHash = $hashResult.StdOut[0].Trim()
}
Write-Ok ("已提交 " + $commitHash + "（分支 " + $targetBranch + "）")

# ================================================================ 7. 推送
$pushed = $false
$mainUpdated = $false

if ($NoPush) {
    Write-Section "推送"
    Write-Item "已跳过推送（-NoPush）"
}
else {
    Push-Branch -TargetBranch $targetBranch -RemoteName $Remote
    $pushed = $true
    $mainUpdated = Invoke-MainSyncPolicy -TargetBranch $targetBranch -RemoteName $Remote
}

# ================================================================ 8. 收尾
Show-Completion -CommitHash $commitHash -TargetBranch $targetBranch -RemoteName $Remote `
    -NoPushFlag ([bool]$NoPush) -PushedFlag $pushed -MainUpdatedFlag $mainUpdated -PushOnlyFlag $false `
    -ExcludedUntracked $excludedUntracked.Count -ExcludedTracked $excludedTracked.Count

exit $EXIT_OK
