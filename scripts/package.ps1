# OAS Studio (authoring) release packaging (Windows)
#
# The Studio package is the AUTHORING/BUILD package: it ships the editor UI
# (workbench / task builder / task library / instances / settings), the full
# data tree needed to author and publish tasks, the frozen OCR models and the
# bundled Windows adb. It deliberately does NOT contain the end-user Runner
# surface (runner.html/js/css, start/stop-runner, README-RUNNER.txt) and does
# NOT ship the scheduler anymore (the embedded scheduler lives in the Runner
# surface; see design docs/RUNNER-STUDIO-SPLIT-PLAN.md A.8.1 and
# docs/ENGINE-DESIGN.md 22.44.6).
#
# Usage:
#   powershell -File scripts/package.ps1 [-Version 0.1.0] [-SkipBuild] [-DryRun]
#   powershell -File scripts/package.ps1 -Build          # opt-in cargo build
#
# Output:
#   dist/oasd-v<version>-win64.zip
#   dist/oasd-v<version>-win64.zip.sha256
#
# Notes:
#   * Build is NOT run by default (same rule as scripts/package-runner.ps1):
#     pass -Build, or build once with
#     `cargo build --release -p oasd --features ocr` before running this script.
#   * -SkipBuild is kept for command-line parity with older invocations; it is
#     a no-op because the script already never builds unless -Build is passed.
#   * The zip is written with System.IO.Compression.ZipArchive so entry names
#     always use `/` separators (Windows PowerShell 5.1 Compress-Archive writes
#     `\`, which non-Windows unzip tools extract as literal backslash names).
#   * The package must be self-contained: every launcher and config path in the
#     zip is relative to the extracted package root.
param(
  [string]$Version = "0.1.0",
  [switch]$SkipBuild,
  [switch]$Build,
  [switch]$DryRun
)
$ErrorActionPreference = "Stop"

$Root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
Set-Location $Root
. (Join-Path $Root "scripts\package-common.ps1")

# Windows PowerShell 5.1 `Set-Content -Encoding UTF8` prepends a UTF-8 BOM
# (EF BB BF). A BOM breaks the engine's serde_json parser ("expected value at
# line 1 column 1"), so every text file this script generates is written as
# BOM-free UTF-8 via [System.IO.File]::WriteAllText.
function Write-Utf8NoBom {
  param(
    [Parameter(Mandatory = $true, ValueFromPipeline = $true)][AllowEmptyString()][string]$Text,
    [Parameter(Mandatory = $true)][string]$Path
  )
  begin { $sb = New-Object System.Text.StringBuilder }
  process { [void]$sb.AppendLine([string]$Text) }
  end {
    $utf8 = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($Path, $sb.ToString(), $utf8)
  }
}

if ($Build -and $SkipBuild) {
  throw "-Build and -SkipBuild are mutually exclusive"
}
if ([string]::IsNullOrWhiteSpace($Version) -or $Version -notmatch '^[0-9A-Za-z][0-9A-Za-z._+-]*$') {
  throw "bad -Version value '$Version' (use e.g. 1.2.3)"
}

$OutDir = Join-Path $Root "dist"
$Stage = Join-Path $OutDir "stage"
$ZipName = "oasd-v{0}-win64.zip" -f $Version
$Zip = Join-Path $OutDir $ZipName
$ShaFile = $Zip + ".sha256"

$Exe = Join-Path $Root "target\release\oasd.exe"
$UiDir = Join-Path $Root "apps\oasd\ui"
$PackagingDir = Join-Path $Root "packaging"

# v1: same frozen OCR models and Windows adb binaries as scripts/package-runner.ps1.
$ModelDir = "D:\OAS-dev\backend\toolkit\Lib\site-packages\rapidocr\models"
$AdbDir = "D:\OAS-dev\backend\toolkit\Lib\site-packages\adbutils\binaries"
$ModelFiles = @("PP-OCRv6_rec_small.onnx", "PP-OCRv6_det_small.onnx")
$AdbFiles = @("adb.exe", "AdbWinApi.dll", "AdbWinUsbApi.dll")

# Studio surface UI = exactly the files served by the Studio surface, and
# nothing from the Runner bundle. Keep this list in sync with
# apps/oasd/src/serve.rs route_owner() (GET /app.js, /asset_ui.js, /builder.js,
# /flow_model.js, /dashboard.html, /dashboard.js are Studio-only; `/` and
# /style.css are per-surface/shared).
$StudioUiFiles = @(
  "index.html", "app.js", "builder.js", "flow_model.js", "asset_ui.js",
  "dashboard.html", "dashboard.js", "style.css"
)
# Runner-only UI / authoring-only test pages must never leak into the Studio
# package (design A.8.1: the two packages are disjoint on their UI bundles).
$ForbiddenUiNames = @(
  "runner.html", "runner.js", "runner.css",
  "flow_model_test.html", "ui_pure_test.js"
)
# Runner-only launchers/readme plus the old headless scheduler: the embedded
# scheduler (and its plan file) belongs to the Runner surface now.
$ForbiddenPackageNames = @(
  "start-runner.bat", "stop-runner.bat", "README-RUNNER.txt",
  "start-scheduler.bat", "stop-scheduler.bat", "scheduler.json"
)

$StageDirs = @("ui", "models", "adb", "config", "tasks", "library", "assets", "frames", "relations", "templates", "runs", "logs")

Write-Host "== OAS Studio package v$Version =="

# ---------------------------------------------------------------- build check
if ($Build) {
  Write-Host "-- cargo build --release -p oasd --features ocr"
  cargo build --release -p oasd --features ocr
  if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
}
$ExeMissing = -not (Test-Path $Exe)
if ($ExeMissing) {
  $hint = "missing $Exe`nBuild it first: cargo build --release -p oasd --features ocr`n(or re-run with -Build)"
  if ($DryRun) {
    Write-Host "[MISSING] $Exe (dry-run; build first or pass -Build)"
  } else {
    throw $hint
  }
}

# --------------------------------------------------------------- source check
$CopyPlan = @()
function Add-CopyItem {
  param(
    [Parameter(Mandatory = $true)][string]$Source,
    [Parameter(Mandatory = $true)][string]$Dest,
    [Parameter(Mandatory = $true)][string]$Kind,
    [switch]$Required
  )
  $script:CopyPlan += [pscustomobject]@{
    Source = $Source; Dest = $Dest; Kind = $Kind; Required = [bool]$Required
  }
}

Add-CopyItem -Source $Exe -Dest "oasd.exe" -Kind "binary" -Required
foreach ($f in $StudioUiFiles) {
  Add-CopyItem -Source (Join-Path $UiDir $f) -Dest (Join-Path "ui" $f) -Kind "studio-ui" -Required
}
foreach ($m in $ModelFiles) {
  Add-CopyItem -Source (Join-Path $ModelDir $m) -Dest (Join-Path "models" $m) -Kind "ocr-model" -Required
}
Add-CopyItem -Source (Join-Path $AdbDir "adb.exe") -Dest "adb\adb.exe" -Kind "adb" -Required
foreach ($f in @("AdbWinApi.dll", "AdbWinUsbApi.dll")) {
  Add-CopyItem -Source (Join-Path $AdbDir $f) -Dest (Join-Path "adb" $f) -Kind "adb" -Required:$false
}
Add-CopyItem -Source (Join-Path $PackagingDir "live.example.json") -Dest "config\live.example.json" -Kind "config" -Required
foreach ($f in @("start-studio.bat", "stop-studio.bat", "start-editor.bat", "stop-editor.bat")) {
  Add-CopyItem -Source (Join-Path $PackagingDir $f) -Dest $f -Kind "launcher" -Required
}
Add-CopyItem -Source (Join-Path $PackagingDir "README-DEPLOY.md") -Dest "README-DEPLOY.md" -Kind "readme" -Required

# Studio UI may pull in one or two extra local assets (e.g. a shared icon).
# Add existing local <script src>/<link href> targets automatically, but never
# the Runner-only or test-only files in $ForbiddenUiNames.
foreach ($htmlName in @("index.html", "dashboard.html")) {
  $htmlPath = Join-Path $UiDir $htmlName
  if (-not (Test-Path $htmlPath)) { continue }
  $html = Get-Content -Raw -Encoding UTF8 $htmlPath
  $refs = [regex]::Matches($html, '(?:src|href)\s*=\s*"([^"]+)"') |
    ForEach-Object { $_.Groups[1].Value }
  foreach ($ref in $refs) {
    if ([string]::IsNullOrWhiteSpace($ref)) { continue }
    if ($ref -match '^(?:[a-zA-Z][a-zA-Z0-9+.-]*:|#|/)') { continue }
    $clean = ($ref -split '[?#]')[0].Replace('/', '\')
    if ([string]::IsNullOrWhiteSpace($clean)) { continue }
    if ($StudioUiFiles -contains $clean) { continue }
    if ($ForbiddenUiNames -contains (Split-Path -Leaf $clean)) { continue }
    $src = Join-Path $UiDir $clean
    if (Test-Path $src) {
      Add-CopyItem -Source $src -Dest (Join-Path "ui" $clean) -Kind "studio-ui-extra" -Required:$false
    }
  }
}

$MissingRequired = @()
foreach ($item in $CopyPlan) {
  if ($item.Required -and -not (Test-Path $item.Source)) {
    $MissingRequired += $item.Source
  }
}
if ($MissingRequired.Count -gt 0) {
  Write-Host "!! missing required Studio package sources:" -ForegroundColor Yellow
  foreach ($m in ($MissingRequired | Select-Object -Unique)) { Write-Host "   - $m" }
  if ($DryRun) {
    Write-Host "[DRY-RUN] required sources missing; plan shown for inspection only."
  } else {
    throw ("Studio package is incomplete: missing $($MissingRequired.Count) required source(s).")
  }
}

# ------------------------------------------------------------- config check
# The shared live.example.json must keep the Studio port 8788 (and the Runner
# section, since the same template is shipped in both packages).
$ConfigJson = Join-Path $PackagingDir "live.example.json"
if (Test-Path $ConfigJson) {
  try {
    $cfg = Get-Content -Raw -Encoding UTF8 $ConfigJson | ConvertFrom-Json
  } catch {
    throw "invalid JSON in packaging/live.example.json: $($_.Exception.Message)"
  }
  if ($null -eq $cfg.serve -or [int]$cfg.serve.port -ne 8788) {
    throw "packaging/live.example.json serve.port must stay 8788"
  }
  if ($null -eq $cfg.runner -or [int]$cfg.runner.port -ne 8789) {
    throw "packaging/live.example.json must keep the runner section (runner.port 8789)"
  }
} elseif ($DryRun) {
  Write-Host "[MISSING] packaging/live.example.json"
} else {
  throw "missing packaging/live.example.json"
}

# --------------------------------------------------------------- dry run
if ($DryRun) {
  Write-Host ""
  Write-Host "Studio package plan (stage: dist\stage):"
  foreach ($item in $CopyPlan) {
    $rel = $item.Dest
    $state = "OK"
    if (-not (Test-Path $item.Source)) { $state = "MISSING" }
    Write-Host ("  [{0,-7}] {1,-34} <- {2}" -f $state, $rel, $item.Source)
  }
  Write-Host "  [GENERATE] config\live.json          <- copy of live.example.json"
  Write-Host "  [COPY]     tasks\*                   <- real task bundle data, state\ stripped"
  Write-Host "  [COPY]     assets\assets.json + assets\images\* <- real global asset store"
  Write-Host "  [COPY]     frames\** (frames.json + PNGs, .backup\ excluded)"
  Write-Host "  [COPY]     relations\** + templates\** <- real authoring data"
  Write-Host "  [GENERATE] library\, runs\, logs\ .keep placeholders (data dirs copied when present)"
  Write-Host "  [GENERATE] VERSION, BUILD-INFO.txt"
  Write-Host ""
  Write-Host "Excluded from the Studio package (Runner / headless-scheduler surface):"
  Write-Host ("  UI files: {0}" -f ($ForbiddenUiNames -join ", "))
  Write-Host "  UI files not copied: any other apps\oasd\ui\* (wildcard copies are never used)"
  Write-Host "  Launchers: start-runner.bat, stop-runner.bat, README-RUNNER.txt"
  Write-Host "  Scheduler: start-scheduler.bat, stop-scheduler.bat, scheduler.json"
  Write-Host ""
  Write-Host "Output: $Zip"
  Write-Host "SHA256: $ShaFile"
  if ($MissingRequired.Count -gt 0) {
    Write-Host "[DRY-RUN] incomplete: $($MissingRequired.Count) required source(s) missing."
    exit 1
  }
  Write-Host "[DRY-RUN] OK: plan is complete; nothing was written."
  exit 0
}

# ------------------------------------------------------------------- stage
Write-Host "-- staging $Stage"
if (Test-Path $Stage) { Remove-Item $Stage -Recurse -Force }
New-Item -ItemType Directory -Force -Path $Stage | Out-Null
foreach ($d in $StageDirs) {
  New-Item -ItemType Directory -Force -Path (Join-Path $Stage $d) | Out-Null
}

foreach ($item in $CopyPlan) {
  if (-not (Test-Path $item.Source)) {
    if ($item.Required) { throw "missing $($item.Source)" }
    Write-Host "-- skip optional missing $($item.Source)"
    continue
  }
  $dest = Join-Path $Stage $item.Dest
  $parent = Split-Path -Parent $dest
  if (-not (Test-Path $parent)) { New-Item -ItemType Directory -Force -Path $parent | Out-Null }
  Copy-Item $item.Source $dest -Force
}

# config: ship the example and a ready-to-edit live.json (never the dev's live.json)
Copy-Item (Join-Path $Stage "config\live.example.json") (Join-Path $Stage "config\live.json") -Force

# task bundles are DATA: ship real content, never runtime state
$RootTasks = Join-Path $Root "tasks"
if (Test-Path $RootTasks) {
  Copy-Item (Join-Path $RootTasks "*") (Join-Path $Stage "tasks") -Recurse -Force
  Get-ChildItem (Join-Path $Stage "tasks") -Directory -Recurse -Force -Filter "state" |
    Remove-Item -Recurse -Force
}
$StageTaskDirs = @(Get-ChildItem (Join-Path $Stage "tasks") -Directory -Force -ErrorAction SilentlyContinue)
if ($StageTaskDirs.Count -eq 0) {
  "# OAS tasks (data)" | Write-Utf8NoBom -Path (Join-Path $Stage "tasks\README.txt")
}

# v0.1.0: the root global asset store (assets/assets.json + assets/images/) is
# DATA and must ship verbatim. Older packages had no global store and relied on
# `assets migrate` to build one from per-task assets + library; keep that path
# only when the root store is absent.
$RootAssets = Join-Path $Root "assets"
$RootAssetsJson = Join-Path $RootAssets "assets.json"
$MigratedLegacy = $false
$RootAssetCount = -1

if (Test-Path $RootAssetsJson) {
  Write-Host "-- copy root global assets (assets.json + images/)"
  Copy-Item $RootAssetsJson (Join-Path $Stage "assets\assets.json") -Force
  $RootImages = Join-Path $RootAssets "images"
  $StageImages = Join-Path $Stage "assets\images"
  New-Item -ItemType Directory -Force -Path $StageImages | Out-Null
  if (Test-Path $RootImages) {
    Copy-Item (Join-Path $RootImages "*") $StageImages -Recurse -Force -ErrorAction SilentlyContinue
  }
  $RootAssetCount = @((Get-Content -Raw -Encoding UTF8 $RootAssetsJson | ConvertFrom-Json).items).Count
} else {
  # Legacy package (pre-global store): migrate the shipped tasks + old library
  # into the unified store. This rewrites task flows (duplicates get _2 suffix).
  Write-Host "-- migrate packaged assets (auto-suffix, rewrite flows)"
  $MigratedLegacy = $true
  $StageExe = Join-Path $Stage "oasd.exe"
  Push-Location $Stage
  try {
    & $StageExe assets migrate --tasks-dir tasks --library-dir library --assets-dir assets
    if ($LASTEXITCODE -ne 0) { throw "assets migrate failed (exit $LASTEXITCODE)" }
    if (-not (Test-Path (Join-Path $Stage "assets\assets.json"))) {
      throw "assets migrate did not produce assets/assets.json"
    }
  } finally {
    Pop-Location
  }
}

# library/ is real authoring data when present; keep the directory visible in
# the zip even when the repo's library is still empty.
$RootLibrary = Join-Path $Root "library"
if (Test-Path $RootLibrary) {
  $libItems = @(Get-ChildItem $RootLibrary -Force -ErrorAction SilentlyContinue)
  if ($libItems.Count -gt 0) {
    Copy-Item (Join-Path $RootLibrary "*") (Join-Path $Stage "library") -Recurse -Force -ErrorAction SilentlyContinue
  }
}
if (-not (Get-ChildItem (Join-Path $Stage "library") -Force -ErrorAction SilentlyContinue)) {
  "" | Set-Content -Encoding ASCII (Join-Path $Stage "library\.keep")
}

# frames/, relations/, templates/ are real authoring data: ship them verbatim
# (P0-2 user requirement: assets/tasks/data belong to the repo and the release
# must carry them). Runtime backups (`frames/.backup/`) are never shipped.
function Copy-DataTree {
  param([string]$SrcDir, [string]$DestDir, [string]$ExcludePattern = '')
  if (-not (Test-Path $SrcDir)) { return 0 }
  $files = @(Get-ChildItem $SrcDir -Recurse -File -Force -ErrorAction SilentlyContinue |
    Where-Object { $ExcludePattern -eq '' -or $_.FullName -notmatch $ExcludePattern })
  foreach ($f in $files) {
    $rel = $f.FullName.Substring($SrcDir.Length + 1)
    $dest = Join-Path $DestDir $rel
    $parent = Split-Path -Parent $dest
    if (-not (Test-Path $parent)) { New-Item -ItemType Directory -Force -Path $parent | Out-Null }
    Copy-Item $f.FullName $dest -Force
  }
  return $files.Count
}

$RootFrames = Join-Path $Root "frames"
$RootRelations = Join-Path $Root "relations"
$RootTemplates = Join-Path $Root "templates"
$FrameCount = Copy-DataTree -SrcDir $RootFrames -DestDir (Join-Path $Stage "frames") -ExcludePattern '\\\.backup\\'
$RelationCount = Copy-DataTree -SrcDir $RootRelations -DestDir (Join-Path $Stage "relations")
$TemplateCount = Copy-DataTree -SrcDir $RootTemplates -DestDir (Join-Path $Stage "templates")
foreach ($pair in @(@("frames", $FrameCount), @("relations", $RelationCount), @("templates", $TemplateCount))) {
  if ($pair[1] -eq 0) {
    "" | Set-Content -Encoding ASCII (Join-Path (Join-Path $Stage $pair[0]) ".keep")
  }
}

# empty runtime directories (zip keeps empty dirs only if they contain a file)
"" | Set-Content -Encoding ASCII (Join-Path $Stage "runs\.keep")
"" | Set-Content -Encoding ASCII (Join-Path $Stage "logs\.keep")

# Verify the stage asset store really carries the source data before lint/zip.
$StageAssetsJson = Join-Path $Stage "assets\assets.json"
if (-not (Test-Path $StageAssetsJson)) { throw "missing stage assets/assets.json" }
$StageAssetCount = @((Get-Content -Raw -Encoding UTF8 $StageAssetsJson | ConvertFrom-Json).items).Count
if ($RootAssetCount -ge 0 -and $StageAssetCount -ne $RootAssetCount) {
  throw "stage assets count $StageAssetCount != root assets count $RootAssetCount"
}
Write-Host "-- stage assets: $StageAssetCount item(s)"
Write-Host "-- stage tasks: $($StageTaskDirs.Count) bundle(s)"

# Lint the packaged task bundles with the packaged binary (read-only check).
$StageExe = Join-Path $Stage "oasd.exe"
Push-Location $Stage
try {
  & $StageExe task lint --tasks-dir tasks --all
  if ($LASTEXITCODE -ne 0) { throw "packaged task lint failed" }
  if ($MigratedLegacy) {
    # N46: package ships no per-task legacy assets after global migration.
    & $StageExe assets cleanup-legacy --tasks-dir tasks --assets-dir assets --apply
    if ($LASTEXITCODE -ne 0) { throw "packaged legacy asset cleanup failed" }
  }
} finally {
  Pop-Location
}

# version / build info (same first four fields as scripts/package-runner.ps1)
"$Version" | Write-Utf8NoBom -Path (Join-Path $Stage "VERSION")
$android = "Android 12 x86_64 (MuMu vmindex=0, adb 127.0.0.1:16384); Onmyoji 2.8.84"
$taskNames = ($StageTaskDirs | Select-Object -ExpandProperty Name) -join ", "
$buildLines = Get-OasBuildInfoLines -Version $Version -Root $Root `
  -BuildCommand "cargo build --release -p oasd --features ocr" `
  -ExtraLines @(
    "surface: studio",
    "package: studio authoring package (editor UI + full data + publishing tools; NOT the end-user package)",
    "entry: oasd.exe studio --port 8788 --config config/live.json --ui ui",
    "ui: index.html app.js builder.js flow_model.js asset_ui.js dashboard.html dashboard.js style.css",
    "models: PP-OCRv6_rec_small.onnx, PP-OCRv6_det_small.onnx",
    "adb: bundled Windows adb (adb/adb.exe)",
    "data: tasks=$($StageTaskDirs.Count) assets=$StageAssetCount frames=$FrameCount relations=$RelationCount templates=$TemplateCount",
    "android_test_env: $android",
    "tasks: $taskNames"
  )
$buildLines | Write-Utf8NoBom -Path (Join-Path $Stage "BUILD-INFO.txt")

# ------------------------------------------------------- stage audit
# 1) nothing Runner-only / scheduler-only may appear anywhere in the stage.
$forbidden = Get-ChildItem $Stage -Recurse -File -Force | Where-Object {
  ($ForbiddenUiNames -contains $_.Name) -or ($ForbiddenPackageNames -contains $_.Name)
}
if ($forbidden) {
  $names = ($forbidden | ForEach-Object { $_.FullName }) -join "; "
  throw "Studio stage contains Runner-only files: $names"
}

# 2) ui/ holds exactly the Studio whitelist (no wildcard leakage).
$uiUnexpected = Get-ChildItem (Join-Path $Stage "ui") -File -Force |
  Where-Object { $StudioUiFiles -notcontains $_.Name }
if ($uiUnexpected) {
  $names = ($uiUnexpected | ForEach-Object { $_.Name }) -join ", "
  throw "Studio stage ui/ contains non-whitelisted file(s): $names"
}

# 3) no runtime state dirs leaked from tasks/, and no frames backup data.
$stateDirs = @(Get-ChildItem (Join-Path $Stage "tasks") -Directory -Recurse -Force -Filter "state" -ErrorAction SilentlyContinue)
if ($stateDirs.Count -gt 0) {
  throw "Studio stage tasks/ still contains state/ directories"
}
$backupDirs = @(Get-ChildItem $Stage -Recurse -Force -Directory -Filter ".backup" -ErrorAction SilentlyContinue)
if ($backupDirs.Count -gt 0) {
  throw "Studio stage must not contain .backup/ directories"
}

# 3.5) repo data that exists must have been copied (user requirement ①).
if ($FrameCount -gt 0 -and -not (Test-Path (Join-Path $Stage "frames\frames.json"))) {
  throw "Studio stage copied $FrameCount frame file(s) but frames\frames.json is missing"
}
if ($RelationCount -gt 0 -and $RelationCount -ne @(Get-ChildItem $RootRelations -Recurse -File -Force).Count) {
  throw "Studio stage relations copy is incomplete"
}
if ($TemplateCount -gt 0 -and $TemplateCount -ne @(Get-ChildItem $RootTemplates -Recurse -File -Force).Count) {
  throw "Studio stage templates copy is incomplete"
}

# 4) required files present.
$requiredStage = @(
  "oasd.exe",
  "ui\index.html", "ui\app.js", "ui\builder.js", "ui\flow_model.js",
  "ui\asset_ui.js", "ui\dashboard.html", "ui\dashboard.js", "ui\style.css",
  "models\PP-OCRv6_rec_small.onnx", "models\PP-OCRv6_det_small.onnx",
  "adb\adb.exe", "config\live.example.json", "config\live.json",
  "start-studio.bat", "stop-studio.bat", "start-editor.bat", "stop-editor.bat",
  "README-DEPLOY.md", "assets\assets.json", "library\.keep",
  "runs\.keep", "logs\.keep", "VERSION", "BUILD-INFO.txt"
)
if ($FrameCount -eq 0) { $requiredStage += "frames\.keep" }
if ($RelationCount -eq 0) { $requiredStage += "relations\.keep" }
if ($TemplateCount -eq 0) { $requiredStage += "templates\.keep" }
if ($StageTaskDirs.Count -eq 0) { $requiredStage += "tasks\README.txt" }
foreach ($rel in $requiredStage) {
  if (-not (Test-Path (Join-Path $Stage $rel))) { throw "stage is missing $rel" }
}

# 5) generated text files are BOM-free UTF-8 (F1 follow-up: a UTF-8 BOM breaks
#    the engine's serde_json parser; Windows PowerShell 5.1 Set-Content added one).
foreach ($rel in @("VERSION", "BUILD-INFO.txt")) {
  $bytes = [System.IO.File]::ReadAllBytes((Join-Path $Stage $rel))
  if ($bytes.Length -ge 3 -and $bytes[0] -eq 0xEF -and $bytes[1] -eq 0xBB -and $bytes[2] -eq 0xBF) {
    throw "stage $rel must be BOM-free UTF-8"
  }
}

Write-Host "-- stage audit OK (no Runner UI / scheduler files; ui/ is the Studio whitelist; text files are BOM-free)"

# --------------------------------------------------------------------- zip
if (-not (Test-Path $OutDir)) { New-Item -ItemType Directory -Force -Path $OutDir | Out-Null }
if (Test-Path $Zip) { Remove-Item $Zip -Force }
if (Test-Path $ShaFile) { Remove-Item $ShaFile -Force }
Write-Host "-- compress $ZipName"
# Write the zip by hand instead of Compress-Archive: on Windows PowerShell 5.1
# Compress-Archive stores entry names with `\`, which non-Windows unzip tools may
# extract as literal backslash filenames. The package format must stay portable,
# so always use `/` entry separators (APPNOTE 4.4.17).
Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem
$zipStream = [System.IO.File]::Open($Zip, [System.IO.FileMode]::Create)
try {
  $archive = [System.IO.Compression.ZipArchive]::new(
    $zipStream,
    [System.IO.Compression.ZipArchiveMode]::Create
  )
  try {
    foreach ($f in (Get-ChildItem $Stage -Recurse -File -Force)) {
      $rel = $f.FullName.Substring($Stage.Length + 1).Replace('\', '/')
      $entry = $archive.CreateEntry($rel, [System.IO.Compression.CompressionLevel]::Optimal)
      $entry.LastWriteTime = [System.DateTimeOffset]$f.LastWriteTime
      $in = [System.IO.File]::OpenRead($f.FullName)
      try {
        $out = $entry.Open()
        try { $in.CopyTo($out) } finally { $out.Dispose() }
      } finally {
        $in.Dispose()
      }
    }
  } finally {
    $archive.Dispose()
  }
} finally {
  $zipStream.Dispose()
}

$hash = (Get-FileHash $Zip -Algorithm SHA256).Hash
"$hash  $ZipName" | Set-Content -Encoding ASCII $ShaFile

$sizeMb = [math]::Round((Get-Item $Zip).Length / 1MB, 1)
Write-Host ""
Write-Host "OK: $Zip ($sizeMb MB)"
Write-Host "SHA256: $hash"
Write-Host "Contents:"
Get-ChildItem $Stage -Recurse -File -Force |
  ForEach-Object { "  " + $_.FullName.Substring($Stage.Length + 1) }
