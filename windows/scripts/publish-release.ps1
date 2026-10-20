# Seals, verifies and publishes the Windows release built by publish.ps1 for every architecture.
# Mirrors script/package_release.sh: version assets are immutable, and the update
# feed advances only after the packages it points to are available.
param(
    [Parameter(Mandatory)][string]$Version,
    # Holds windows-x64/ and windows-arm64/ from publish.ps1.
    [Parameter(Mandatory)][string]$AssetsDirectory
)
$ErrorActionPreference = 'Stop'
$Repo = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$Project = Join-Path $Repo 'windows/ChipDock/ChipDock.csproj'
$Repository = if ($env:GITHUB_REPOSITORY) { $env:GITHUB_REPOSITORY } else { 'tangxiangong/chipdock' }
$Tag = "v$Version"
$Architectures = @('x64', 'arm64')
if (-not $env:WINDOWS_UPDATE_PRIVATE_KEY) { throw 'Configure the WINDOWS_UPDATE_PRIVATE_KEY release secret before publishing.' }
if (-not $env:GH_TOKEN) { throw 'GH_TOKEN is required for publication.' }

$Work = Join-Path ([IO.Path]::GetTempPath()) "chipdock-windows-release-$([guid]::NewGuid().ToString('N'))"
New-Item -ItemType Directory $Work | Out-Null
try {
    & dotnet build (Join-Path $Repo 'windows/UpdateTool/ChipDock.UpdateTool.csproj') -c Release -o (Join-Path $Work 'tool')
    if ($LASTEXITCODE -ne 0) { throw 'Update tool build failed.' }
    function Invoke-UpdateTool {
        & dotnet (Join-Path $Work 'tool/ChipDock.UpdateTool.dll') @args
        if ($LASTEXITCODE -ne 0) { throw "Update tool failed: $($args[0])" }
    }

    # Returns the release JSON, or $null when GitHub reports 404.
    function Get-Release([string]$Name) {
        $Output = & gh api "repos/$Repository/releases/tags/$Name" 2>&1
        if ($LASTEXITCODE -eq 0) { return ($Output -join "`n") | ConvertFrom-Json }
        if (($Output -join "`n") -match 'HTTP 404') { return $null }
        throw "Could not read release ${Name}: $Output"
    }

    $FeedRelease = Get-Release 'update-feed'
    $Uploads = @()
    $Feeds = @()
    foreach ($Arch in $Architectures) {
        $Directory = Join-Path $AssetsDirectory "windows-$Arch"
        $Channel = "win-$Arch"
        $Signed = "releases.$Channel.signed.json"
        Invoke-UpdateTool seal $Directory $Arch $Project
        $Previous = @()
        if ($FeedRelease -and ($FeedRelease.assets | Where-Object name -eq $Signed)) {
            $PreviousDirectory = Join-Path $Work "previous-$Arch"
            & gh release download update-feed --repo $Repository --pattern $Signed --dir $PreviousDirectory
            if ($LASTEXITCODE -ne 0) { throw "Could not download the published $Signed." }
            $Previous = @(Join-Path $PreviousDirectory $Signed)
        }
        # Refuses downgrades and replacing a published package with different bytes.
        Invoke-UpdateTool verify $Directory $Arch $Version $Project @Previous
        $Uploads += @(Get-ChildItem $Directory -File | Where-Object { $_.Extension -in @('.exe', '.msi', '.nupkg') })
        $Feeds += Join-Path $Directory $Signed
    }

    $Release = Get-Release $Tag
    if (-not $Release) { throw "Release $Tag does not exist." }
    foreach ($File in $Uploads) {
        $Existing = $Release.assets | Where-Object name -eq $File.Name
        if (-not $Existing) {
            & gh release upload $Tag $File.FullName --repo $Repository
            if ($LASTEXITCODE -ne 0) { throw "Upload failed: $($File.Name)" }
            continue
        }
        $Digest = "sha256:$((Get-FileHash -Algorithm SHA256 $File.FullName).Hash.ToLowerInvariant())"
        if ($Existing.digest -ne $Digest) { throw "Refusing to overwrite the published $($File.Name) with different bytes." }
    }

    if (-not $FeedRelease) {
        $Target = if ($env:GITHUB_SHA) { $env:GITHUB_SHA } else { (& git -C $Repo rev-parse HEAD).Trim() }
        & gh release create update-feed --repo $Repository --target $Target `
            --title 'ChipDock update feed' --notes 'Sparkle and Velopack update metadata.' --prerelease --latest=false
        if ($LASTEXITCODE -ne 0) { throw 'Could not create the update-feed release.' }
    }
    # Publish the feeds only after the immutable packages are available.
    & gh release upload update-feed @Feeds --repo $Repository --clobber
    if ($LASTEXITCODE -ne 0) { throw 'Feed upload failed.' }
    Write-Host "Published ChipDock $Version for Windows: $($Uploads.Name -join ', ')"
}
finally { Remove-Item -Recurse -Force $Work -ErrorAction SilentlyContinue }
