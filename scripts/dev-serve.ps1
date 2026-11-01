# OAS dev launcher
# Runs the DEV binary from the repository root and uses the dev-tracked data:
#   tasks/, assets/, frames/, runs/, library/
# It never uses dist/stage and never runs scripts/package.ps1.
[CmdletBinding()]
param(
    [int]$Port = 8787,
    [switch]$Release,
    [switch]$SkipBuild,
    [switch]$NoOpen,
    [switch]$DryRun,
    [string]$Model = "D:\OAS-dev\backend\toolkit\Lib\site-packages\rapidocr\models\PP-OCRv6_rec_small.onnx",
    [string]$DetModel = "D:\OAS-dev\backend\toolkit\Lib\site-packages\rapidocr\models\PP-OCRv6_det_small.onnx",
    [string]$AdbPath = "D:\OAS-dev\backend\toolkit\Lib\site-packages\adbutils\binaries\adb.exe"
)

$ErrorActionPreference = "Stop"

if ($Port -lt 1 -or $Port -gt 65535) {
    throw "invalid -Port: $Port"
}

# Repository root = parent of scripts/
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

# Dev data dirs live directly in the repo, not under dist/.
$DataDirs = @("tasks", "assets", "frames", "runs", "library")
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

$serveArgs = @(
    "serve",
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

Write-Host ""
Write-Host "[dev] repo   = $Root"
Write-Host "[dev] exe    = $Exe"
Write-Host "[dev] data   = tasks / assets / frames / runs / library (repo root)"
Write-Host "[dev] editor = http://127.0.0.1:$Port/"
Write-Host "[dev] dist/stage is NOT used"
Write-Host ""

if ($DryRun) {
    Write-Host "[dev] dry-run command:"
    Write-Host ("  " + $Exe + " " + ($serveArgs -join " "))
    exit 0
}

& $Exe @serveArgs
exit $LASTEXITCODE
