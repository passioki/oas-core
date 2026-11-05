# OAS dev launcher (Studio / Runner)
#
# Runs the DEV binary from the repository root against the dev-tracked data
# (tasks/, assets/, frames/, runs/, library/). It never uses dist/stage and
# never runs scripts/package*.ps1.
#
# One binary, two surfaces (PR1):
#   scripts\dev-serve.bat                     -> Studio (editing), 8788
#   scripts\dev-serve.bat -Surface runner     -> Runner (usage),  8789
#
# Options:
#   -Surface studio|runner   surface to start (default: studio)
#   -Port <n>                default: 8788 for studio, 8789 for runner
#   -Release                 build/use target\release (default: debug)
#   -SkipBuild               reuse the existing exe
#   -NoOpen                  do not open the default browser
#   -NoScheduler             Runner only: disable the embedded scheduler thread
#   -DryRun                  print the exact command and exit
#
# Notes (see docs/START-HERE.md §3):
#   * Builds always pass --features ocr, otherwise OCR routes fail loudly.
#   * Stop old 8787/8788/8789 processes before rebuilding oasd.exe.
[CmdletBinding()]
param(
    [ValidateSet("studio", "runner")]
    [string]$Surface = "studio",
    [int]$Port = 0,
    [switch]$Release,
    [switch]$SkipBuild,
    [switch]$NoOpen,
    [switch]$NoScheduler,
    [switch]$DryRun,
    [string]$Model = "D:\OAS-dev\backend\toolkit\Lib\site-packages\rapidocr\models\PP-OCRv6_rec_small.onnx",
    [string]$DetModel = "D:\OAS-dev\backend\toolkit\Lib\site-packages\rapidocr\models\PP-OCRv6_det_small.onnx",
    [string]$AdbPath = "D:\OAS-dev\backend\toolkit\Lib\site-packages\adbutils\binaries\adb.exe"
)

$ErrorActionPreference = "Stop"

# Default port per surface: Studio 8788 / Runner 8789 (PR1; 8787 is retired).
if ($Port -eq 0) {
    $Port = if ($Surface -eq "runner") { 8789 } else { 8788 }
}
if ($Port -lt 1 -or $Port -gt 65535) {
    throw "invalid -Port: $Port"
}
if ($NoScheduler -and $Surface -ne "runner") {
    throw "-NoScheduler only applies to -Surface runner"
}

# Repository root = parent of scripts/
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

# Dev data dirs live directly in the repo, not under dist/.
$DataDirs = @("tasks", "assets", "frames", "runs", "library")
if ($Surface -eq "runner") {
    $DataDirs += @("packages", "logs")
}
foreach ($d in $DataDirs) {
    $p = Join-Path $Root $d
    if (-not (Test-Path $p)) {
        New-Item -ItemType Directory -Force -Path $p | Out-Null
        Write-Host "[dev] created $p"
    }
}

$Ui = Join-Path $Root "apps\oasd\ui"
if (-not (Test-Path $Ui)) {
    throw "missing UI dir: $Ui"
}
if (-not (Test-Path $Model)) {
    throw "missing OCR model: $Model"
}
if (-not (Test-Path $DetModel)) {
    throw "missing OCR det model: $DetModel"
}

if (-not $SkipBuild) {
    $buildArgs = @("build", "-p", "oasd", "--features", "ocr")
    if ($Release) { $buildArgs += "--release" }
    Write-Host "[dev] cargo $($buildArgs -join ' ')"
    & cargo @buildArgs
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed (exit $LASTEXITCODE)" }
}

$Profile = if ($Release) { "release" } else { "debug" }
$Exe = Join-Path $Root "target\$Profile\oasd.exe"
if (-not (Test-Path $Exe)) {
    throw "missing $Exe (run without -SkipBuild)"
}

# Warn about port conflicts instead of silently starting on the wrong port.
$listeners = Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue
if ($listeners) {
    $pids = ($listeners | Select-Object -ExpandProperty OwningProcess -Unique) -join ","
    throw "port $Port is already in use (PID: $pids). Stop it or use -Port <other>."
}

if (Test-Path $AdbPath) {
    $env:ADB_PATH = $AdbPath
} else {
    Write-Warning "ADB path not found: $AdbPath (device connection may fail)"
}

# Same flags for both surfaces; `oasd runner` accepts --library-dir/--frames-dir
# for symmetry and simply ignores the authoring-only ones.
$serveArgs = @(
    $Surface,
    "--port", "$Port",
    "--ui", $Ui,
    "--tasks-dir", (Join-Path $Root "tasks"),
    "--assets-dir", (Join-Path $Root "assets"),
    "--frames-dir", (Join-Path $Root "frames"),
    "--runs-dir", (Join-Path $Root "runs"),
    "--library-dir", (Join-Path $Root "library"),
    "--model", $Model,
    "--det-model", $DetModel
)
if ($NoOpen) { $serveArgs += "--no-open" }
if ($NoScheduler -and $Surface -eq "runner") { $serveArgs += "--no-scheduler" }

$mode = if ($Surface -eq "runner") { "usage (Runner)" } else { "editing (Studio)" }
Write-Host ""
Write-Host "[dev] surface = $Surface - $mode"
Write-Host "[dev] repo    = $Root"
Write-Host "[dev] exe     = $Exe"
Write-Host "[dev] data    = tasks / assets / frames / runs / library (repo root)"
Write-Host "[dev] url     = http://127.0.0.1:$Port/"
if ($Surface -eq "runner") {
    if ($NoScheduler) {
        Write-Host "[dev] scheduler = disabled (--no-scheduler)"
    } else {
        Write-Host "[dev] scheduler = embedded thread (start with -NoScheduler to disable)"
    }
}
Write-Host "[dev] dist/stage is NOT used"
Write-Host ""

if ($DryRun) {
    Write-Host "[dev] dry-run command:"
    Write-Host ("  " + $Exe + " " + ($serveArgs -join " "))
    exit 0
}

& $Exe @serveArgs
exit $LASTEXITCODE
