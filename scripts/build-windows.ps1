param(
  [string]$Version = '0.1.0',
  [switch]$SkipVerification
)

$ErrorActionPreference = 'Stop'
$RepositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location $RepositoryRoot

if (-not $SkipVerification) {
  & (Join-Path $PSScriptRoot 'verify.ps1')
  if ($LASTEXITCODE -ne 0) { throw 'Verification failed; Windows package was not built.' }
}

Write-Host 'Building Windows NSIS installer and portable executable...' -ForegroundColor Cyan
& npm run tauri -- build --config apps/desktop/src-tauri/tauri.conf.json -- --locked
if ($LASTEXITCODE -ne 0) { throw "Tauri build failed with exit code $LASTEXITCODE" }

$Executable = Join-Path $RepositoryRoot 'apps/desktop/src-tauri/target/release/smart-workspace-launcher.exe'
if (-not (Test-Path $Executable)) { throw "Expected executable was not produced: $Executable" }

$OutputDirectory = Join-Path $RepositoryRoot 'releases/local'
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
$Archive = Join-Path $OutputDirectory "smart-workspace-launcher-v$Version-win-x64-portable.zip"
$Stage = Join-Path $OutputDirectory "stage-$Version"
Remove-Item $Stage -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $Stage | Out-Null
Copy-Item $Executable (Join-Path $Stage 'Smart Workspace Launcher.exe')
Copy-Item (Join-Path $RepositoryRoot 'docs/RELEASE.md') (Join-Path $Stage 'Release and runtime requirements.md')
Compress-Archive -Path (Join-Path $Stage '*') -DestinationPath $Archive -Force
Remove-Item $Stage -Recurse -Force
$Digest = (Get-FileHash -Algorithm SHA256 $Archive).Hash.ToLowerInvariant()
Set-Content -Path "$Archive.sha256" -Value "$Digest  $(Split-Path $Archive -Leaf)" -Encoding ascii
Write-Host "Created local, unverified package: $Archive"
Write-Host "SHA256: $Digest"
Write-Host 'This build is not a V1 release until independent Windows 11 acceptance and fresh-download smoke tests pass.' -ForegroundColor Yellow
