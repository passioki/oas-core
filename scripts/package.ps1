# OAS release packaging (Windows)
# Usage:  pwsh -File scripts/package.ps1 [-Version 0.1.0] [-SkipBuild]
# Output: dist/oasd-v<version>-win64.zip + .sha256 + BUILD-INFO.txt
param(
  [string]$Version = "1.1.0",
  [switch]$SkipBuild
)
$ErrorActionPreference = "Stop"

$Root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
Set-Location $Root

$OutDir = Join-Path $Root "dist"
$Stage = Join-Path $OutDir "stage"
$Zip = Join-Path $OutDir ("oasd-v{0}-win64.zip" -f $Version)

Write-Host "== OAS package v$Version =="
if (-not $SkipBuild) {
  Write-Host "-- cargo build --release (features ocr)"
  cargo build --release -p oasd --features ocr
  if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
}
$Exe = Join-Path $Root "target\release\oasd.exe"
if (-not (Test-Path $Exe)) { throw "missing $Exe (run without -SkipBuild)" }

if (Test-Path $Stage) { Remove-Item $Stage -Recurse -Force }
New-Item -ItemType Directory -Force -Path $Stage | Out-Null
foreach ($d in @("ui", "models", "adb", "tasks", "library", "config", "runs", "logs")) {
  New-Item -ItemType Directory -Force -Path (Join-Path $Stage $d) | Out-Null
}

Copy-Item $Exe (Join-Path $Stage "oasd.exe")

# editor UI (static)
Copy-Item (Join-Path $Root "apps\oasd\ui\*") (Join-Path $Stage "ui") -Recurse -Force

# OCR models: bundle the same versions used on dev (frozen with the release)
$ModelDir = "D:\OAS-dev\backend\toolkit\Lib\site-packages\rapidocr\models"
foreach ($m in @("PP-OCRv6_rec_small.onnx", "PP-OCRv6_det_small.onnx")) {
  $src = Join-Path $ModelDir $m
  if (-not (Test-Path $src)) { throw "missing model $src" }
  Copy-Item $src (Join-Path $Stage "models\$m")
}

# adb binaries
$AdbDir = "D:\OAS-dev\backend\toolkit\Lib\site-packages\adbutils\binaries"
foreach ($f in @("adb.exe", "AdbWinApi.dll", "AdbWinUsbApi.dll")) {
  $src = Join-Path $AdbDir $f
  if (Test-Path $src) { Copy-Item $src (Join-Path $Stage "adb\$f") }
}
if (-not (Test-Path (Join-Path $Stage "adb\adb.exe"))) { throw "adb.exe missing" }

# config template + a ready-to-edit live.json (default MuMu endpoint; player edits serial/port)
Copy-Item (Join-Path $Root "packaging\live.example.json") (Join-Path $Stage "config\live.example.json")
Copy-Item (Join-Path $Root "packaging\live.example.json") (Join-Path $Stage "config\live.json")

# scheduler v3 plan: ship the dev-verified plan when present; otherwise an empty one
$PlanSrc = Join-Path $Root "scheduler.json"
if (Test-Path $PlanSrc) {
  Copy-Item $PlanSrc (Join-Path $Stage "scheduler.json")
} else {
  @'
{
  "schema_version": 3,
  "config": {
    "calendar": { "day_reset": "05:00", "week_reset": { "weekday": 0, "time": "05:00" } },
    "guards": { "active_hours": [], "blackouts": [] }
  },
  "entries": []
}
'@ | Set-Content -Encoding UTF8 (Join-Path $Stage "scheduler.json")
}

# task bundles + global library are DATA: ship real content, never runtime state
if (Test-Path (Join-Path $Root "tasks")) {
  Copy-Item (Join-Path $Root "tasks\*") (Join-Path $Stage "tasks") -Recurse -Force
  Get-ChildItem (Join-Path $Stage "tasks") -Directory -Recurse -Force -Filter "state" |
    Remove-Item -Recurse -Force
}
if (Test-Path (Join-Path $Root "library")) {
  Copy-Item (Join-Path $Root "library\*") (Join-Path $Stage "library") -Recurse -Force -ErrorAction SilentlyContinue
}

# start/stop scripts + readme
foreach ($f in @("start-editor.bat", "stop-editor.bat", "start-scheduler.bat", "stop-scheduler.bat", "README-DEPLOY.md")) {
  Copy-Item (Join-Path $Root "packaging\$f") (Join-Path $Stage $f)
}

# data placeholders (readme only if the dirs are still empty)
if (-not (Get-ChildItem (Join-Path $Stage "tasks") -ErrorAction SilentlyContinue)) {
  "# OAS tasks (data)" | Set-Content -Encoding UTF8 (Join-Path $Stage "tasks\README.txt")
}
"" | Set-Content -Encoding ASCII (Join-Path $Stage "runs\.keep")
"" | Set-Content -Encoding ASCII (Join-Path $Stage "logs\.keep")

# version / build info
"$Version" | Set-Content -Encoding UTF8 (Join-Path $Stage "VERSION")
$git = "unknown"
try { $git = (git -C $Root log -1 --format="%h %s") } catch {}
$android = "Android 12 x86_64 (MuMu vmindex=0, adb 127.0.0.1:16384); Onmyoji 2.8.84"
$taskNames = (Get-ChildItem (Join-Path $Stage "tasks") -Directory -ErrorAction SilentlyContinue |
  Select-Object -ExpandProperty Name) -join ", "
@(
  "OAS v$Version",
  "git: $git",
  "built_at: $(Get-Date -Format o)",
  "build: cargo build --release -p oasd --features ocr",
  "android_test_env: $android",
  "tasks: $taskNames"
) | Set-Content -Encoding UTF8 (Join-Path $Stage "BUILD-INFO.txt")

# zip
if (Test-Path $Zip) { Remove-Item $Zip -Force }
Compress-Archive -Path (Join-Path $Stage "*") -DestinationPath $Zip -CompressionLevel Optimal
$hash = (Get-FileHash $Zip -Algorithm SHA256).Hash
"$hash  $(Split-Path -Leaf $Zip)" | Set-Content -Encoding ASCII ($Zip + ".sha256")

Write-Host "OK: $Zip"
Write-Host "SHA256: $hash"
