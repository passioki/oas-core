# OAS Runner-only release packaging (Windows)
#
# The Runner package is the END-USER package: it contains only the runtime
# surface (HTTP UI on 8789 + task package install + queue/schedule/execute),
# OCR models and the bundled Windows adb. It deliberately does NOT contain the
# Studio authoring UI (index.html / app.js / builder.js / flow_model.js /
# asset_ui.js / dashboard.*) or authoring-only data directories.
#
# Usage:
#   powershell -File scripts/package-runner.ps1 [-Version 0.1.0] [-SkipBuild] [-DryRun]
#   powershell -File scripts/package-runner.ps1 -Build          # opt-in cargo build
#
# Output:
#   dist/oasd-runner-v<version>-win64.zip
#   dist/oasd-runner-v<version>-win64.zip.sha256
#   dist/BUILD-INFO.txt                      (copy of the packaged BUILD-INFO.txt)
#
# Notes:
#   * Build is NOT run by default: the PR1 surface split may be mid-flight and
#     an automatic `cargo build` would fight the Rust subprocess. If
#     target\release\oasd.exe is missing the script tells you how to build.
#   * -SkipBuild is accepted for command-line parity with scripts/package.ps1;
#     it is a no-op because this script already never builds unless -Build is
#     passed.
#   * The `runner` subcommand and ui/runner.* are PR1 backend/UI deliverables.
#     This script fails with an explicit dependency message if they are absent
#     instead of silently shipping a Studio UI under the Runner name.
param(
  [string]$Version = "0.1.0",
  [switch]$SkipBuild,
  [switch]$Build,
  [switch]$DryRun,
  # P0-2: ship the read-only bundled tasks/assets snapshot (default yes);
  # -NoData produces a program-only package (old behaviour).
  [switch]$NoData
)
$ErrorActionPreference = "Stop"

$Root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
Set-Location $Root
. (Join-Path $Root "scripts\package-common.ps1")

# Windows PowerShell 5.1 `Set-Content -Encoding UTF8` prepends a UTF-8 BOM
# (EF BB BF) and the engine's serde_json rejects a BOM in scheduler.json
# ("expected value at line 1 column 1"). Every text file this script generates
# is therefore written as BOM-free UTF-8 via [System.IO.File]::WriteAllText.
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
$Stage = Join-Path $OutDir "stage-runner"
$ZipName = "oasd-runner-v{0}-win64.zip" -f $Version
$Zip = Join-Path $OutDir $ZipName
$ShaFile = $Zip + ".sha256"
$BuildInfoOut = Join-Path $OutDir "BUILD-INFO.txt"

$Exe = Join-Path $Root "target\release\oasd.exe"
$UiDir = Join-Path $Root "apps\oasd\ui"
$PackagingDir = Join-Path $Root "packaging"

# v1: same frozen OCR models and Windows adb binaries as scripts/package.ps1.
$ModelDir = "D:\OAS-dev\backend\toolkit\Lib\site-packages\rapidocr\models"
$AdbDir = "D:\OAS-dev\backend\toolkit\Lib\site-packages\adbutils\binaries"
$ModelFiles = @("PP-OCRv6_rec_small.onnx", "PP-OCRv6_det_small.onnx")
$AdbFiles = @("adb.exe", "AdbWinApi.dll", "AdbWinUsbApi.dll")

# Runner surface needs these UI files and nothing else from the Studio UI.
$RunnerUiFiles = @("runner.html", "runner.js", "runner.css", "style.css")
$ForbiddenUiNames = @(
  "index.html", "app.js", "builder.js", "flow_model.js", "asset_ui.js",
  "flow_model_test.html", "dashboard.html", "dashboard.js", "ui_pure_test.js"
)
$ForbiddenPackageNames = @("start-studio.bat", "stop-studio.bat", "start-editor.bat", "stop-editor.bat")

$StageDirs = @("ui", "models", "adb", "config", "packages", "tasks", "runs", "logs", "assets")

Write-Host "== OAS Runner package v$Version =="

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
foreach ($f in $RunnerUiFiles) {
  Add-CopyItem -Source (Join-Path $UiDir $f) -Dest (Join-Path "ui" $f) -Kind "runner-ui" -Required
}
foreach ($m in $ModelFiles) {
  Add-CopyItem -Source (Join-Path $ModelDir $m) -Dest (Join-Path "models" $m) -Kind "ocr-model" -Required
}
Add-CopyItem -Source (Join-Path $AdbDir "adb.exe") -Dest "adb\adb.exe" -Kind "adb" -Required
foreach ($f in @("AdbWinApi.dll", "AdbWinUsbApi.dll")) {
  Add-CopyItem -Source (Join-Path $AdbDir $f) -Dest (Join-Path "adb" $f) -Kind "adb" -Required:$false
}
Add-CopyItem -Source (Join-Path $PackagingDir "live.example.json") -Dest "config\live.example.json" -Kind "config" -Required
Add-CopyItem -Source (Join-Path $PackagingDir "start-runner.bat") -Dest "start-runner.bat" -Kind "launcher" -Required
Add-CopyItem -Source (Join-Path $PackagingDir "stop-runner.bat") -Dest "stop-runner.bat" -Kind "launcher" -Required
Add-CopyItem -Source (Join-Path $PackagingDir "README-RUNNER.txt") -Dest "README-RUNNER.txt" -Kind "readme" -Required

# Runner UI may pull in one or two extra local assets (e.g. a shared icon).
# Add existing local <script src>/<link href> targets automatically, but never
# the Studio-only files in $ForbiddenUiNames.
$RunnerHtml = Join-Path $UiDir "runner.html"
if (Test-Path $RunnerHtml) {
  $html = Get-Content -Raw -Encoding UTF8 $RunnerHtml
  $refs = [regex]::Matches($html, '(?:src|href)\s*=\s*"([^"]+)"') |
    ForEach-Object { $_.Groups[1].Value }
  foreach ($ref in $refs) {
    if ([string]::IsNullOrWhiteSpace($ref)) { continue }
    if ($ref -match '^(?:[a-zA-Z][a-zA-Z0-9+.-]*:|#|/)') { continue }
    $clean = ($ref -split '[?#]')[0].Replace('/', '\')
    if ([string]::IsNullOrWhiteSpace($clean)) { continue }
    if ($RunnerUiFiles -contains $clean) { continue }
    if ($ForbiddenUiNames -contains (Split-Path -Leaf $clean)) { continue }
    $src = Join-Path $UiDir $clean
    if (Test-Path $src) {
      Add-CopyItem -Source $src -Dest (Join-Path "ui" $clean) -Kind "runner-ui-extra" -Required:$false
    }
  }
}

$MissingRequired = @()
foreach ($item in $CopyPlan) {
  if ($item.Required -and -not (Test-Path $item.Source)) {
    $MissingRequired += $item.Source
  }
}
if (-not (Test-Path $RunnerHtml)) {
  Write-Host "!! ui/runner.html is a PR1 UI deliverable (owned by the Runner UI subprocess)." -ForegroundColor Yellow
}
if ($MissingRequired.Count -gt 0) {
  Write-Host "!! missing required Runner package sources:" -ForegroundColor Yellow
  foreach ($m in ($MissingRequired | Select-Object -Unique)) { Write-Host "   - $m" }
  if ($DryRun) {
    Write-Host "[DRY-RUN] required sources missing; plan shown for inspection only."
  } else {
    throw ("Runner package is incomplete: missing $($MissingRequired.Count) required source(s). " +
      "The `runner` subcommand / ui/runner.* are PR1 deliverables from the backend and UI " +
      "subprocesses; rebuild/re-run once they land.")
  }
}

# ------------------------------------------------------------- config check
$ConfigJson = Join-Path $PackagingDir "live.example.json"
if (Test-Path $ConfigJson) {
  try {
    $cfg = Get-Content -Raw -Encoding UTF8 $ConfigJson | ConvertFrom-Json
  } catch {
    throw "invalid JSON in packaging/live.example.json: $($_.Exception.Message)"
  }
  if ($null -eq $cfg.runner) {
    throw "packaging/live.example.json is missing the `runner` section"
  }
  if ([int]$cfg.runner.port -ne 8789) {
    throw "packaging/live.example.json runner.port must be 8789 (got $($cfg.runner.port))"
  }
  if ($null -eq $cfg.serve -or [int]$cfg.serve.port -ne 8788) {
    throw "packaging/live.example.json serve.port must stay 8788"
  }
} elseif ($DryRun) {
  Write-Host "[MISSING] packaging/live.example.json"
} else {
  throw "missing packaging/live.example.json"
}

# --------------------------------------------------------------- dry run
if ($DryRun) {
  Write-Host ""
  Write-Host "Runner package plan (stage: dist\stage-runner):"
  foreach ($item in $CopyPlan) {
    $rel = $item.Dest
    $state = "OK"
    if (-not (Test-Path $item.Source)) { $state = "MISSING" }
    Write-Host ("  [{0,-7}] {1,-30} <- {2}" -f $state, $rel, $item.Source)
  }
  Write-Host "  [GENERATE] config\live.json          <- copy of live.example.json"
  Write-Host "  [GENERATE] scheduler.json            <- empty schema v4 plan"
  Write-Host "  [GENERATE] tasks\README.txt, packages\.keep, runs\.keep, logs\.keep, assets\.keep"
  Write-Host "  [GENERATE] VERSION, BUILD-INFO.txt"
  if ($NoData) {
    Write-Host "  [DATA]     bundled-data\              <- SKIPPED (-NoData)"
  } else {
    Write-Host "  [DATA]     bundled-data\tasks\** + assets\** + manifest.json <- repo data snapshot"
    Write-Host "  [GENERATE] first-run import into a fresh data root (never overwrites user data)"
  }
  Write-Host ""
  Write-Host "Excluded from the Runner package (Studio authoring surface):"
  Write-Host ("  UI files: {0}" -f ($ForbiddenUiNames -join ", "))
  Write-Host "  UI files not copied: any other apps\oasd\ui\* (wildcard copies are never used)"
  Write-Host "  Directories: frames\, library\ (Studio authoring data)"
  Write-Host "  Launchers: start-studio.bat, start-editor.bat"
  Write-Host ""
  Write-Host "Output: $Zip"
  Write-Host "SHA256: $ShaFile"
  Write-Host "BUILD-INFO: $BuildInfoOut"
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

# Empty scheduler plan: Runner starts clean and reports "blocked" until the
# user adds entries via the Runner UI. schema v4 is the current write format.
@'
{
  "schema_version": 4,
  "config": {
    "mode": "stop",
    "calendar": { "day_reset": "05:00", "week_reset": { "weekday": 0, "time": "05:00" } },
    "guards": { "active_hours": [], "blackouts": [] }
  },
  "queues": [],
  "entries": []
}
'@ | Write-Utf8NoBom -Path (Join-Path $Stage "scheduler.json")

# data placeholders (zip keeps empty dirs only if they contain a file)
"# OAS Runner tasks`r`n`r`nInstall task packages (.oaspkg) from the Runner UI; each package`r`ncreates one task directory here." |
  Write-Utf8NoBom -Path (Join-Path $Stage "tasks\README.txt")
"" | Set-Content -Encoding ASCII (Join-Path $Stage "packages\.keep")
"" | Set-Content -Encoding ASCII (Join-Path $Stage "runs\.keep")
"" | Set-Content -Encoding ASCII (Join-Path $Stage "logs\.keep")
"" | Set-Content -Encoding ASCII (Join-Path $Stage "assets\.keep")

# ---------------------------------------------------- bundled-data (P0-2/P0-3)
# A read-only snapshot of the repo's tasks/ + assets/ that the Runner imports
# on first start into a fresh data root (see apps/oasd/src/bundled_data.rs and
# design RUNNER-SPLIT-DESIGN.md §6.3/§6.5). Runtime state is never shipped.
$DataSummary = "bundled=no"
if (-not $NoData) {
  $RootTasks = Join-Path $Root "tasks"
  $RootAssets = Join-Path $Root "assets"
  if (-not (Test-Path $RootTasks)) { throw "missing tasks dir for bundled-data: $RootTasks" }
  if (-not (Test-Path (Join-Path $RootAssets "assets.json"))) {
    throw "missing assets/assets.json for bundled-data: $RootAssets"
  }
  $BundleStage = Join-Path $Stage "bundled-data"
  New-Item -ItemType Directory -Force -Path (Join-Path $BundleStage "tasks") | Out-Null
  New-Item -ItemType Directory -Force -Path (Join-Path $BundleStage "assets\images") | Out-Null

  Get-ChildItem $RootTasks -Directory -Force | ForEach-Object {
    Copy-Item $_.FullName (Join-Path $BundleStage "tasks") -Recurse -Force
  }
  # Strip runtime artefacts before hashing: state/, *.bak, *.tmp, click logs.
  Get-ChildItem $BundleStage -Recurse -Force -Directory -Filter "state" -ErrorAction SilentlyContinue |
    Remove-Item -Recurse -Force
  Get-ChildItem $BundleStage -Recurse -File -Force | Where-Object {
    $_.Name -like "*.bak" -or $_.Name -like "*.tmp" -or $_.Name -like "*.clicks.jsonl"
  } | Remove-Item -Force

  Copy-Item (Join-Path $RootAssets "assets.json") (Join-Path $BundleStage "assets\assets.json") -Force
  if (Test-Path (Join-Path $RootAssets "images")) {
    Copy-Item (Join-Path $RootAssets "images\*") (Join-Path $BundleStage "assets\images") -Recurse -Force -ErrorAction SilentlyContinue
  }

  $BundleFiles = @(Get-ChildItem $BundleStage -Recurse -File -Force | Where-Object { $_.Name -ne "manifest.json" })
  $BundleEntries = @()
  $BundleBytes = 0
  foreach ($f in $BundleFiles) {
    $rel = $f.FullName.Substring($BundleStage.Length + 1).Replace('\', '/')
    $BundleEntries += [pscustomobject]@{
      path = $rel
      sha256 = (Get-FileHash $f.FullName -Algorithm SHA256).Hash
      bytes = [int64]$f.Length
    }
    $BundleBytes += $f.Length
  }
  $BundleTasks = @(Get-ChildItem (Join-Path $BundleStage "tasks") -Directory -Force |
    Select-Object -ExpandProperty Name)
  $BundleImageCount = @(Get-ChildItem (Join-Path $BundleStage "assets\images") -File -Force -ErrorAction SilentlyContinue).Count
  $AssetsSha = (Get-FileHash (Join-Path $BundleStage "assets\assets.json") -Algorithm SHA256).Hash
  $CommitShort = "unknown"
  try {
    $line = & git -C $Root rev-parse --short HEAD 2>$null
    if ($LASTEXITCODE -eq 0 -and $line) { $CommitShort = ($line | Select-Object -First 1).ToString().Trim() }
  } catch { $CommitShort = "unknown" }
  $RepoUrl = "unknown"
  try {
    $url = & git -C $Root remote get-url origin 2>$null
    if ($LASTEXITCODE -eq 0 -and $url) { $RepoUrl = ($url | Select-Object -First 1).ToString().Trim() }
  } catch { $RepoUrl = "unknown" }

  $Manifest = [ordered]@{
    schema_version = 1
    generated_at_ms = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
    source = [ordered]@{
      repo = $RepoUrl
      commit = $CommitShort
      version = $Version
    }
    tasks = $BundleTasks
    assets = [ordered]@{
      assets_json_sha256 = $AssetsSha
      image_count = $BundleImageCount
    }
    files = $BundleEntries
    total_bytes = [int64]$BundleBytes
  }
  ($Manifest | ConvertTo-Json -Depth 6) | Write-Utf8NoBom -Path (Join-Path $BundleStage "manifest.json")
  $DataSummary = "bundled=yes tasks=$($BundleTasks.Count) assets=$BundleImageCount files=$($BundleEntries.Count) bytes=$BundleBytes source=$CommitShort"
  Write-Host "-- bundled-data: $DataSummary"
}

# version / build info (same first four fields as scripts/package.ps1)
"$Version" | Write-Utf8NoBom -Path (Join-Path $Stage "VERSION")
$buildLines = Get-OasBuildInfoLines -Version $Version -Root $Root `
  -BuildCommand "cargo build --release -p oasd --features ocr" `
  -ExtraLines @(
    "surface: runner",
    "package: runner-only (no Studio editing UI)",
    "entry: oasd.exe runner --port 8789 --config config/live.json --ui ui",
    "requires: PR1 oasd runner subcommand + ui/runner.html/js/css",
    "models: PP-OCRv6_rec_small.onnx, PP-OCRv6_det_small.onnx",
    "adb: bundled Windows adb (adb/adb.exe)",
    "data: $DataSummary"
  )
$buildLines | Write-Utf8NoBom -Path (Join-Path $Stage "BUILD-INFO.txt")

# ------------------------------------------------------- stage audit
$forbidden = Get-ChildItem $Stage -Recurse -File -Force | Where-Object {
  ($ForbiddenUiNames -contains $_.Name) -or ($ForbiddenPackageNames -contains $_.Name)
}
if ($forbidden) {
  $names = ($forbidden | ForEach-Object { $_.FullName }) -join "; "
  throw "Runner stage contains Studio-only files: $names"
}

$requiredStage = @(
  "oasd.exe", "ui\runner.html", "ui\runner.js", "ui\runner.css", "ui\style.css",
  "models\PP-OCRv6_rec_small.onnx", "models\PP-OCRv6_det_small.onnx",
  "adb\adb.exe", "config\live.example.json", "config\live.json",
  "start-runner.bat", "stop-runner.bat", "README-RUNNER.txt",
  "scheduler.json", "tasks\README.txt", "packages\.keep", "runs\.keep",
  "logs\.keep", "assets\.keep", "VERSION", "BUILD-INFO.txt"
)
if (-not $NoData) {
  $requiredStage += @("bundled-data\manifest.json", "bundled-data\tasks", "bundled-data\assets\assets.json")
}
foreach ($rel in $requiredStage) {
  if (-not (Test-Path (Join-Path $Stage $rel))) { throw "stage is missing $rel" }
}

# F1 regression guard: scheduler.json must parse as JSON; serde_json rejects a
# UTF-8 BOM, so assert the generated plan starts with '{' (no EF BB BF).
$schedBytes = [System.IO.File]::ReadAllBytes((Join-Path $Stage "scheduler.json"))
if ($schedBytes.Length -lt 1 -or $schedBytes[0] -ne 0x7B) {
  throw "scheduler.json must be BOM-free UTF-8 JSON (first byte must be '{')"
}

# P0-2/P0-3 audit: bundled-data must be exactly the snapshot its manifest lists
# (no runtime state, no unlisted file), or must be absent with -NoData.
$BundleStage = Join-Path $Stage "bundled-data"
if ($NoData) {
  if (Test-Path $BundleStage) { throw "-NoData stage must not contain bundled-data\" }
} else {
  $ManifestPath = Join-Path $BundleStage "manifest.json"
  if (-not (Test-Path $ManifestPath)) { throw "bundled-data\manifest.json is missing" }
  $manifest = Get-Content -Raw -Encoding UTF8 $ManifestPath | ConvertFrom-Json
  if ([int]$manifest.schema_version -ne 1) { throw "bundled-data manifest schema_version must be 1" }
  $listed = @($manifest.files)
  if ($listed.Count -lt 1) { throw "bundled-data manifest has no files" }
  foreach ($entry in $listed) {
    $rel = ([string]$entry.path).Replace('/', '\')
    $p = Join-Path $BundleStage $rel
    if (-not (Test-Path $p)) { throw "bundled-data manifest lists a missing file: $($entry.path)" }
    $h = (Get-FileHash $p -Algorithm SHA256).Hash
    if ($h -ne [string]$entry.sha256) { throw "bundled-data sha256 mismatch: $($entry.path)" }
  }
  $actual = @(Get-ChildItem $BundleStage -Recurse -File -Force | Where-Object { $_.Name -ne "manifest.json" })
  if ($actual.Count -ne $listed.Count) {
    throw "bundled-data has $($actual.Count) file(s) but manifest lists $($listed.Count)"
  }
  $stateDirs = @(Get-ChildItem $BundleStage -Recurse -Force -Directory -Filter "state" -ErrorAction SilentlyContinue)
  if ($stateDirs.Count -gt 0) { throw "bundled-data must not contain runtime state/ dirs" }
  if (@($manifest.tasks).Count -lt 1) { throw "bundled-data manifest has no tasks" }
}

Write-Host "-- stage audit OK (no Studio UI file names present; scheduler.json is BOM-free JSON; bundled-data verified)"

# --------------------------------------------------------------------- zip
if (-not (Test-Path $OutDir)) { New-Item -ItemType Directory -Force -Path $OutDir | Out-Null }
if (Test-Path $Zip) { Remove-Item $Zip -Force }
if (Test-Path $ShaFile) { Remove-Item $ShaFile -Force }
Write-Host "-- compress $ZipName"
# Write the zip by hand instead of Compress-Archive: on Windows PowerShell 5.1
# Compress-Archive stores entry names with `\`, which non-Windows unzip tools may
# extract as literal backslash filenames. The Runner package format is meant to
# stay portable, so always use `/` entry separators (APPNOTE 4.4.17).
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
Copy-Item (Join-Path $Stage "BUILD-INFO.txt") $BuildInfoOut -Force

$sizeMb = [math]::Round((Get-Item $Zip).Length / 1MB, 1)
Write-Host ""
Write-Host "OK: $Zip ($sizeMb MB)"
Write-Host "SHA256: $hash"
Write-Host "BUILD-INFO: $BuildInfoOut"
Write-Host "Contents:"
Get-ChildItem $Stage -Recurse -File -Force |
  ForEach-Object { "  " + $_.FullName.Substring($Stage.Length + 1) }
