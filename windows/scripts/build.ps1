param(
    [ValidateSet('x64', 'ARM64')][string]$Architecture = 'x64',
    [ValidateSet('Debug', 'Release')][string]$Configuration = 'Release'
)
$ErrorActionPreference = 'Stop'
$Repo = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
& dotnet build (Join-Path $Repo 'windows/ChipDock.sln') -c $Configuration "-p:Platform=$Architecture"
if ($LASTEXITCODE -ne 0) { throw 'ChipDock build failed.' }
