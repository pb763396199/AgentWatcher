param(
  [string]$WorkflowHome = ""
)

$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$cli = Join-Path $repo 'src-tauri\target\debug\agentwatcher-cli.exe'
if (-not (Test-Path -LiteralPath $cli)) {
  throw "CLI 不存在，请先运行 cargo build --manifest-path src-tauri/Cargo.toml --bin agentwatcher-cli"
}
if ([string]::IsNullOrWhiteSpace($WorkflowHome)) {
  $WorkflowHome = Join-Path $repo (".tmp\workflow-manual-fixture\" + [guid]::NewGuid().ToString())
}
New-Item -ItemType Directory -Force -Path $WorkflowHome | Out-Null

function Invoke-Workflow([string]$Command, [hashtable]$Payload) {
  $request = @{
    schemaVersion = 1
    requestId = [guid]::NewGuid().ToString()
    command = $Command
    payload = $Payload
  } | ConvertTo-Json -Depth 20 -Compress
  $result = $request | & $cli --workflow-home $WorkflowHome workflow apply --file - | ConvertFrom-Json
  if (-not $result.ok) { throw ($result | ConvertTo-Json -Depth 20) }
  return $result.data
}

Invoke-Workflow 'project.create' @{ id = 'manual-project'; title = 'Manual workflow fixture' } | Out-Null
Invoke-Workflow 'roadmap.publish' @{
  id = 'manual-roadmap'
  projectId = 'manual-project'
  title = 'Manual acceptance route'
  expectedRevision = 0
  humanConfirmed = $true
  nodes = @(
    @{ id = 'first'; title = 'First fixture node'; goal = 'Provide a handoff for manual acceptance'; acceptance = @('fixture evidence') },
    @{ id = 'downstream'; title = 'Downstream fixture node'; goal = 'Stay blocked until selected'; dependencies = @('first'); acceptance = @('downstream selected') }
  )
} | Out-Null
$claim = Invoke-Workflow 'node.claim' @{ roadmapId = 'manual-roadmap'; nodeId = 'first'; expectedRevision = 1; sessionId = 'codex:manual-fixture' }
$handoff = Invoke-Workflow 'execution.handoff' @{
  executionId = $claim.executionId
  token = $claim.token
  expectedRevision = 1
  summary = 'Synthetic handoff for manual acceptance only'
  verification = @(@{ criterion = 'fixture evidence'; outcome = 'passed'; evidence = 'manual fixture created by workflow-manual-fixture.ps1' })
  artifacts = @()
}
[pscustomobject]@{
  workflowHome = $WorkflowHome
  handoffId = $handoff.handoffId
  next = @(
    "`$env:AGENTWATCHER_WORKFLOW_HOME='$WorkflowHome'"
    'npm run dev'
    '按 manual-test.md 的 M-001 到 M-012 执行'
  )
} | ConvertTo-Json -Depth 5
