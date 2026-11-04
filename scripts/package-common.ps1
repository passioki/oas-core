# Shared release-info helpers for scripts/package.ps1 and
# scripts/package-runner.ps1. Keep this file ASCII-only so Windows PowerShell
# 5.1 can parse it without a BOM.
#
# The two packaging scripts must agree on:
#   * where the version comes from (the -Version parameter; caller default),
#   * the `git: <short-hash> <subject>` line,
#   * the BUILD-INFO.txt field order.
# Everything release-specific (android test env, task list, surface, ...) is
# passed in by the caller via -ExtraLines.

function Get-OasGitDescription {
    param(
        [Parameter(Mandatory = $true)][string]$Root
    )
    $git = "unknown"
    try {
        $line = & git -C $Root log -1 --format="%h %s" 2>$null
        if ($LASTEXITCODE -eq 0 -and $line) {
            $git = (($line | Select-Object -First 1).ToString()).Trim()
        }
    } catch {
        $git = "unknown"
    }
    if ([string]::IsNullOrWhiteSpace($git)) { $git = "unknown" }
    return $git
}

function Get-OasBuildInfoLines {
    param(
        [Parameter(Mandatory = $true)][string]$Version,
        [Parameter(Mandatory = $true)][string]$Root,
        [Parameter(Mandatory = $true)][string]$BuildCommand,
        [string[]]$ExtraLines = @()
    )
    $lines = @(
        "OAS v$Version",
        "git: $(Get-OasGitDescription -Root $Root)",
        "built_at: $(Get-Date -Format o)",
        "build: $BuildCommand"
    )
    if ($ExtraLines -and $ExtraLines.Count -gt 0) {
        $lines += $ExtraLines
    }
    return $lines
}
