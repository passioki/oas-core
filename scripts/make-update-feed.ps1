# OAS update feed generator (Windows PowerShell 5.1 compatible)
#
# Turns Studio-published `.oaspkg` files into a `publish-index.json` that the
# Runner can consume as an update feed (local path or HTTP/HTTPS URL, e.g. a
# GitHub raw file / GitHub Pages). This is the "software update check" source
# described by the user (external `curl` is used by the Runner to fetch it).
#
# Usage (from the oas-core repo root):
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts\make-update-feed.ps1
#     [-PackagesDir dist\packages] [-OutFile dist\feed\publish-index.json]
#     [-PackageBaseUrl https://github.com/passioki/oas-core/releases/download/v0.2.0]
#     [-CopyPackages] [-Repo https://github.com/passioki/oas-core.git] [-Version 0.1.0]
#
# Output index schema matches apps/oasd/src/exchange.rs PublishIndex (schema 1):
#   events[] = { event_id, task_id, task_version, package_id, package_file,
#                sha256, size_bytes, published_at_ms, source{repo,commit},
#                min_engine?, flow_ops?, package_url? }
# `package_file` is the bare file name; when `package_url` is absent the Runner
# resolves `<feed.package_base>/<package_file>` (local dir or URL base).
param(
  [string]$PackagesDir = "dist\packages",
  [string]$OutFile = "dist\feed\publish-index.json",
  [string]$PackageBaseUrl = "",
  [switch]$CopyPackages,
  [string]$Repo = "https://github.com/passioki/oas-core.git",
  [string]$Version = "0.1.0"
)
$ErrorActionPreference = "Stop"

$Root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
Set-Location $Root

function Write-Utf8NoBom($path, $text) {
  $dir = Split-Path -Parent $path
  if ($dir -and -not (Test-Path $dir)) { New-Item -ItemType Directory -Force -Path $dir | Out-Null }
  [System.IO.File]::WriteAllText($path, $text, (New-Object System.Text.UTF8Encoding($false)))
}

$Commit = "unknown"
try {
  $line = & git -C $Root rev-parse --short HEAD 2>$null
  if ($LASTEXITCODE -eq 0 -and $line) { $Commit = ($line | Select-Object -First 1).ToString().Trim() }
} catch { $Commit = "unknown" }

Add-Type -AssemblyName System.IO.Compression.FileSystem
$packages = @(Get-ChildItem $PackagesDir -Filter "*.oaspkg" -File -ErrorAction SilentlyContinue)
if ($packages.Count -eq 0) { throw "no .oaspkg packages found in $PackagesDir" }

$events = @()
foreach ($pkg in $packages | Sort-Object Name) {
  $zip = [System.IO.Compression.ZipFile]::OpenRead($pkg.FullName)
  try {
    $entry = $zip.Entries | Where-Object { $_.FullName -eq "manifest.json" } | Select-Object -First 1
    if (-not $entry) { throw "package $($pkg.Name) has no manifest.json" }
    $reader = New-Object System.IO.StreamReader($entry.Open())
    try { $manifest = ($reader.ReadToEnd() | ConvertFrom-Json) } finally { $reader.Dispose() }
  } finally { $zip.Dispose() }

  $sha = (Get-FileHash $pkg.FullName -Algorithm SHA256).Hash
  $packageId = [string]$manifest.package_id
  $taskId = [string]$manifest.task_id
  $taskVersion = [int]$manifest.task_version
  $fileName = $pkg.Name
  $event = [ordered]@{
    event_id = $packageId
    task_id = $taskId
    task_version = $taskVersion
    package_id = $packageId
    package_file = $fileName
    sha256 = $sha
    size_bytes = [int64]$pkg.Length
    published_at_ms = ([DateTimeOffset]$pkg.LastWriteTime).ToUnixTimeMilliseconds()
    source = [ordered]@{
      repo = $(if ($manifest.source -and $manifest.source.repo) { [string]$manifest.source.repo } else { $Repo })
      commit = $(if ($manifest.source -and $manifest.source.commit) { [string]$manifest.source.commit } else { $Commit })
    }
  }
  if ($manifest.engine) {
    if ($manifest.engine.min_engine) { $event.min_engine = [string]$manifest.engine.min_engine }
    $ops = @($manifest.engine.flow_ops)
    if ($ops.Count -gt 0) { $event.flow_ops = $ops }
  } else {
    $event.min_engine = $Version
  }
  if (-not [string]::IsNullOrWhiteSpace($PackageBaseUrl)) {
    $base = $PackageBaseUrl.TrimEnd('/')
    $event.package_url = "$base/" + [uri]::EscapeDataString($fileName)
  }
  if ($CopyPackages) {
    $feedDir = Split-Path -Parent $OutFile
    if (-not (Test-Path $feedDir)) { New-Item -ItemType Directory -Force -Path $feedDir | Out-Null }
    Copy-Item $pkg.FullName (Join-Path $feedDir $fileName) -Force
  }
  $events += [pscustomobject]$event
}

$index = [ordered]@{
  schema_version = 1
  updated_at_ms = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
  seq = $events.Count
  publisher = [ordered]@{
    surface = "feed"
    repo = $Repo
    commit = $Commit
    version = $Version
    dirty = $false
  }
  events = $events
}
$json = $index | ConvertTo-Json -Depth 8
Write-Utf8NoBom $OutFile $json

Write-Host "-- update feed: $OutFile"
Write-Host ("-- packages: {0} (base url: {1})" -f $events.Count, $(if ($PackageBaseUrl) { $PackageBaseUrl } else { "(local dir)" }))
foreach ($e in $events) { Write-Host ("   {0} v{1} sha256={2}" -f $e.task_id, $e.task_version, $e.sha256.Substring(0, 12)) }
