param([switch]$Apply)
$ErrorActionPreference = 'Stop'
$repo = 'treborygan/smart-workspace-launcher'
$catalog = Get-Content (Join-Path $PSScriptRoot '..\backlog\issues.json') -Raw | ConvertFrom-Json
if ($Apply) {
  if (-not (Get-Command gh -ErrorAction SilentlyContinue)) { throw 'GitHub CLI (gh) required' }
  gh auth status
}
foreach ($issue in $catalog.issues) {
  Write-Host "$($issue.id): $($issue.title)"
  if ($Apply) {
    $existing = gh issue list --repo $repo --state all --search "$($issue.id) in:title" --json title --limit 100 | ConvertFrom-Json
    if (@($existing | Where-Object { $_.title -eq $issue.title }).Count -eq 0) {
      gh issue create --repo $repo --title $issue.title --body $issue.body
    } else { Write-Host "  Already exists; skipped" }
  }
}
if (-not $Apply) { Write-Host 'Preview only. Use -Apply with authenticated gh CLI to create issues.' }
