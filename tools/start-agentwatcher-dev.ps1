$ErrorActionPreference = 'Stop'

$RepoRoot = Split-Path -Parent $PSScriptRoot
$NodeCandidates = @(
  'C:\Program Files\nodejs\node.exe',
  (Join-Path $env:USERPROFILE '.cache\codex-runtimes\codex-primary-runtime\dependencies\node\bin\node.exe')
)
$NodeExe = $NodeCandidates | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $NodeExe) {
  throw 'Node.js not found. Install Node.js LTS or restore the Codex bundled runtime.'
}

$PathParts = @(
  (Split-Path -Parent $NodeExe),
  (Join-Path $env:USERPROFILE '.cargo\bin'),
  (Join-Path $env:USERPROFILE '.unrealworkflow\bin'),
  (Join-Path $env:USERPROFILE '.unrealdevflow\bin'),
  $env:Path
) | Where-Object { $_ }
$env:Path = ($PathParts -join ';')
$env:AGENTWATCHER_DEV_SILENT = '1'
$env:AGENTWATCHER_DEBUG_SILENT = '1'
$env:WEBVIEW2_USER_DATA_FOLDER = Join-Path $RepoRoot '.tmp\tauri-dev-webview2'

Set-Location $RepoRoot
& $NodeExe (Join-Path $RepoRoot 'tools\tauri-dev.mjs') --restart
