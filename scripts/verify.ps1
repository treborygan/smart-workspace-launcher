param(
  [switch]$SkipCargo
)

$ErrorActionPreference = 'Stop'
$RepositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location $RepositoryRoot
$EvidenceDirectory = Join-Path $RepositoryRoot 'artifacts/verification'
New-Item -ItemType Directory -Force -Path $EvidenceDirectory | Out-Null
$Timestamp = [DateTime]::UtcNow.ToString('yyyyMMddTHHmmssZ')
$Transcript = Join-Path $EvidenceDirectory "windows-$Timestamp.log"
Start-Transcript -Path $Transcript -Force | Out-Null

function Invoke-Checked([string]$Command, [string[]]$Arguments) {
  Write-Host "`n> $Command $($Arguments -join ' ')" -ForegroundColor Cyan
  & $Command @Arguments
  if ($LASTEXITCODE -ne 0) {
    throw "Command failed with exit code ${LASTEXITCODE}: $Command $($Arguments -join ' ')"
  }
}

try {
  Write-Host 'Smart Workspace Launcher local verification' -ForegroundColor Green
  Write-Host "Revision: $(git rev-parse HEAD)"
  Write-Host "Windows: $((Get-CimInstance Win32_OperatingSystem).Caption)"
  Write-Host "Node: $(node --version); npm: $(npm --version)"
  if (-not $SkipCargo) {
    Write-Host "Rust: $(rustc --version); Cargo: $(cargo --version)"
  }

  Invoke-Checked 'npm' @('ci')
  Invoke-Checked 'npm' @('run', 'lint')
  Invoke-Checked 'npm' @('test')
  Invoke-Checked 'python' @('-m', 'pip', 'install', '-r', 'requirements-test.txt')
  Invoke-Checked 'python' @('-m', 'unittest', 'discover', '-s', 'tests', '-v')
  Invoke-Checked 'python' @('scripts/verify_handoff.py')

  if (-not $SkipCargo) {
    Invoke-Checked 'cargo' @('fmt', '--manifest-path', 'apps/desktop/src-tauri/Cargo.toml', '--check')
    Invoke-Checked 'cargo' @('test', '--locked', '--manifest-path', 'apps/desktop/src-tauri/Cargo.toml')
  }

  Write-Host "`nCross-platform verification commands passed. Windows acceptance evidence must still be collected separately." -ForegroundColor Green
}
finally {
  Stop-Transcript | Out-Null
}

Write-Host "Verification transcript: $Transcript"
