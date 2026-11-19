# Build MermaidCraft for Windows.
#
#   target\windows-package\MermaidCraft\          MermaidCraft.exe, mermaidcraft-cli.exe, licences
#   target\windows-package\MermaidCraft-<version>-windows-<arch>.zip
#
# Usage: powershell -ExecutionPolicy Bypass -File packaging\windows\package.ps1 [-SkipBuild]
#
# Put the folder anywhere (for example %LOCALAPPDATA%\Programs\MermaidCraft) and add it to PATH
# to use mermaidcraft-cli from a terminal. The app keeps its preferences in %APPDATA%\MermaidCraft.
param([switch]$SkipBuild)
$ErrorActionPreference = 'Stop'

$Root = Resolve-Path (Join-Path $PSScriptRoot '..\..')
$TargetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $Root 'target' }
# A relative CARGO_TARGET_DIR is taken from where the script was called (cargo runs from $Root).
if (-not [System.IO.Path]::IsPathRooted($TargetDir)) { $TargetDir = Join-Path (Get-Location).Path $TargetDir }
$env:CARGO_TARGET_DIR = $TargetDir

# Version from [workspace.package] in Cargo.toml.
$Version = $null
$InSection = $false
foreach ($line in Get-Content (Join-Path $Root 'Cargo.toml')) {
    if ($line -match '^\[workspace\.package\]') { $InSection = $true; continue }
    if ($line -match '^\[') { $InSection = $false }
    if ($InSection -and $line -match '^version\s*=\s*"([^"]+)"') { $Version = $Matches[1]; break }
}
if (-not $Version) { throw 'version not found in Cargo.toml' }
$Arch = if ($env:PROCESSOR_ARCHITECTURE -eq 'ARM64') { 'arm64' } else { 'x64' }
Write-Host "==> MermaidCraft $Version for Windows ($Arch)"

if (-not $SkipBuild) {
    Push-Location $Root
    # cargo writes its progress to stderr: under 'Stop', Windows PowerShell 5.1 with redirection
    # would take the first line as an error. The exit code decides.
    $ErrorActionPreference = 'Continue'
    try {
        cargo build --release --locked -p mermaidcraft -p mermaidcraft-cli
        if ($LASTEXITCODE -ne 0) { throw "cargo build failed ($LASTEXITCODE)" }
    } finally { Pop-Location; $ErrorActionPreference = 'Stop' }
}
$Bin = Join-Path $TargetDir 'release'

$Work = Join-Path $TargetDir 'windows-package'
$Dir = Join-Path $Work 'MermaidCraft'
if (Test-Path $Work) { Remove-Item -Recurse -Force $Work }
New-Item -ItemType Directory -Force (Join-Path $Dir 'licenses') | Out-Null
Copy-Item (Join-Path $Bin 'mermaidcraft.exe') (Join-Path $Dir 'MermaidCraft.exe')
Copy-Item (Join-Path $Bin 'mermaidcraft-cli.exe') (Join-Path $Dir 'mermaidcraft-cli.exe')
Copy-Item (Join-Path $Root 'LICENSE') (Join-Path $Dir 'LICENSE.txt')
Copy-Item (Join-Path $Root 'README.md') (Join-Path $Dir 'README.md')
Get-ChildItem (Join-Path $Root 'assets\fonts') -Filter 'OFL*.txt' | ForEach-Object {
    Copy-Item $_.FullName (Join-Path $Dir "licenses\fonts-$($_.Name)")
}

$Zip = Join-Path $Work "MermaidCraft-$Version-windows-$Arch.zip"
Compress-Archive -Path $Dir -DestinationPath $Zip -Force
Write-Host "==> $Zip"
