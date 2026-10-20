# Builds the Windows installers and the Velopack update package for one architecture.
# The output is unsigned: publish-release.ps1 seals the feed after verifying it.
param(
    [Parameter(Mandatory)][ValidateSet('x64', 'ARM64')][string]$Architecture,
    [Parameter(Mandatory)][string]$Version,
    [Parameter(Mandatory)][uint32]$BuildNumber,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [string]$ReleaseNotes
)
$ErrorActionPreference = 'Stop'
$Repo = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$Project = Join-Path $Repo 'windows/ChipDock/ChipDock.csproj'
$Arch = $Architecture.ToLowerInvariant()
$Rid = "win-$Arch"
# The installer's OS guard matches TargetPlatformMinVersion.
$InstallerRuntime = "win10.0.17763-$Arch"

if ($Version -notmatch '^\d+\.\d+\.\d+$') { throw "Release version must be a stable x.y.z version: $Version" }
if ((Test-Path $OutputDirectory) -and @(Get-ChildItem -Force $OutputDirectory).Count -ne 0) {
    throw 'OutputDirectory must be empty; refusing to mix release artifacts.'
}

# One version for the Rust Core, the app and the tag, as on macOS.
$Metadata = & cargo metadata --format-version 1 --no-deps --locked --manifest-path (Join-Path $Repo 'Cargo.toml') | ConvertFrom-Json
if ($LASTEXITCODE -ne 0) { throw 'cargo metadata failed.' }
$CoreVersion = ($Metadata.packages | Where-Object name -eq 'chipdock-core').version
if ($CoreVersion -ne $Version) { throw "Release version $Version does not match chipdock-core $CoreVersion" }
$ProjectXml = [xml](Get-Content -Raw $Project)
function Get-ProjectProperty([string]$Name) {
    @($ProjectXml.SelectNodes("//$Name") | ForEach-Object { $_.InnerText.Trim() } | Where-Object { $_ }) | Select-Object -First 1
}
$AppVersion = Get-ProjectProperty 'Version'
if ($AppVersion -ne $Version) { throw "Release version $Version does not match ChipDock.csproj $AppVersion" }
if (-not (Get-ProjectProperty 'UpdatePublicKey')) { throw 'UpdatePublicKey is missing from ChipDock.csproj.' }

# Isolate publish output so obsolete files from another build cannot enter a package.
$Publish = Join-Path $Repo "target/publish/$Rid-$([guid]::NewGuid().ToString('N'))"
$NotesArguments = @()
if ($ReleaseNotes) {
    $FilteredNotes = "$Publish-notes.md"
    & cargo run --quiet --locked --manifest-path (Join-Path $Repo 'Cargo.toml') -p chipdock-release-notes -- `
        windows (Resolve-Path $ReleaseNotes).Path $FilteredNotes
    if ($LASTEXITCODE -ne 0) { throw 'Release note platform filtering failed.' }
    $NotesArguments = @('--releaseNotes', $FilteredNotes)
}
& dotnet publish $Project -c Release "-p:Platform=$Architecture" "-p:RuntimeIdentifier=$Rid" "-p:Version=$Version" -o $Publish
if ($LASTEXITCODE -ne 0) { throw 'WinUI publish failed.' }
foreach ($BundledRuntime in @('coreclr.dll', 'Microsoft.ui.xaml.dll')) {
    if (Test-Path (Join-Path $Publish $BundledRuntime)) { throw "Unexpected bundled runtime: $BundledRuntime" }
}
$RuntimeConfig = Get-Content -Raw (Join-Path $Publish 'ChipDock.runtimeconfig.json') | ConvertFrom-Json
if ($RuntimeConfig.runtimeOptions.framework.name -ne 'Microsoft.NETCore.App') { throw 'Expected framework-dependent .NET publish output.' }
foreach ($Required in @('Microsoft.WindowsAppRuntime.Bootstrap.dll', 'chipdock_csharp.dll', 'Velopack.dll')) {
    if (-not (Test-Path (Join-Path $Publish $Required))) { throw "$Required is missing from the publish output." }
}

$Tools = Join-Path $Repo 'target/release-tools'
$Vpk = Join-Path $Tools 'vpk.exe'
if (-not (Test-Path $Vpk)) {
    & dotnet tool install vpk --version 1.2.161 --tool-path $Tools
    if ($LASTEXITCODE -ne 0) { throw 'Velopack tool installation failed.' }
}
New-Item -ItemType Directory -Force $OutputDirectory | Out-Null
# MSI versions are numeric; the CI run number keeps them increasing independently of SemVer.
if ($BuildNumber -ge 16777216) { throw 'MSI build sequence exhausted.' }
$MsiVersion = "1.$([math]::Floor($BuildNumber / 65536)).$($BuildNumber % 65536)"
# Keep the deployed package ID while the product and executable use ChipDock.
& $Vpk pack --packId FlashDesk --packVersion $Version --packTitle ChipDock --packAuthors tangxiangong `
    --packDir $Publish --mainExe ChipDock.exe --icon (Join-Path $Repo 'windows/ChipDock/Assets/ChipDock.ico') `
    --runtime $InstallerRuntime --framework "net10.0-$Arch-runtime,vcredist143-$Arch" --channel $Rid `
    @NotesArguments --outputDir $OutputDirectory --msi --msiVersion $MsiVersion
if ($LASTEXITCODE -ne 0) { throw 'Velopack packaging failed.' }

foreach ($Extension in @('exe', 'msi')) {
    $Installers = @(Get-ChildItem $OutputDirectory -File -Filter "*.$Extension")
    if ($Installers.Count -ne 1) { throw "Expected one $Extension installer." }
    Move-Item $Installers[0].FullName (Join-Path $OutputDirectory "ChipDock-$Version-windows-$Arch.$Extension")
}
# Updates consume the full package through the signed feed; portable archives and
# Velopack's other index files are not part of this distribution.
$Feed = "releases.$Rid.json"
Get-ChildItem $OutputDirectory -File |
    Where-Object { $_.Extension -notin @('.exe', '.msi', '.nupkg') -and $_.Name -ne $Feed } |
    Remove-Item
$Packages = @(Get-ChildItem $OutputDirectory -File -Filter '*.nupkg')
if ($Packages.Count -ne 1 -or $Packages[0].Name -ne "FlashDesk-$Version-$Rid-full.nupkg") {
    throw "Expected exactly FlashDesk-$Version-$Rid-full.nupkg, found: $($Packages.Name -join ', ')"
}
if (-not (Test-Path (Join-Path $OutputDirectory $Feed))) { throw "Velopack did not write $Feed." }
Remove-Item -Recurse -Force $Publish
if ($ReleaseNotes) { Remove-Item -Force $FilteredNotes }
Get-ChildItem $OutputDirectory | Format-Table Name, Length
