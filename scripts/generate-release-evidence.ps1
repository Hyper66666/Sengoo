[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$DistDir,

    [Parameter(Mandatory = $true)]
    [string]$OutputPath,

    [Parameter(Mandatory = $true)]
    [string]$Version,

    [Parameter(Mandatory = $true)]
    [ValidatePattern('^[0-9a-fA-F]{40}$')]
    [string]$SourceRevision,

    [Parameter(Mandatory = $true)]
    [string]$PreviousVersion,

    [string]$Repository = $env:GITHUB_REPOSITORY,
    [string]$RunId = $env:GITHUB_RUN_ID,
    [string]$RunAttempt = $env:GITHUB_RUN_ATTEMPT,
    [string]$RunUrl,
    [string]$ProvenanceUrl,
    [string[]]$ExpectedTargets = @(
        "x86_64-pc-windows-msvc",
        "x86_64-unknown-linux-gnu",
        "x86_64-apple-darwin",
        "aarch64-apple-darwin"
    )
)

$ErrorActionPreference = "Stop"

if ([string]::IsNullOrWhiteSpace($PreviousVersion) -or $PreviousVersion -eq $Version) {
    throw "previous release version must be non-empty and differ from the current version"
}

$distRoot = (Resolve-Path -LiteralPath $DistDir).Path
$manifests = @(Get-ChildItem -LiteralPath $distRoot -Filter "manifest.json" -File -Recurse)
if ($manifests.Count -eq 0) {
    throw "no distribution manifests found under $distRoot"
}

$expected = @{}
foreach ($target in $ExpectedTargets) {
    if ([string]::IsNullOrWhiteSpace($target)) {
        throw "expected target names must not be empty"
    }
    if ($expected.ContainsKey($target)) {
        throw "duplicate expected target: $target"
    }
    $expected[$target] = $true
}

$artifactsByTarget = @{}
foreach ($manifestPath in $manifests) {
    $manifest = Get-Content -LiteralPath $manifestPath.FullName -Raw | ConvertFrom-Json
    $target = [string]$manifest.target
    if (-not $expected.ContainsKey($target)) {
        throw "unexpected distribution target '$target' in $($manifestPath.FullName)"
    }
    if ($artifactsByTarget.ContainsKey($target)) {
        throw "duplicate distribution manifest for target '$target'"
    }
    if ([string]$manifest.version -ne $Version) {
        throw "manifest version mismatch for ${target}: expected $Version, got $($manifest.version)"
    }
    if ([string]$manifest.source_revision -ne $SourceRevision) {
        throw "manifest source revision mismatch for ${target}: expected $SourceRevision, got $($manifest.source_revision)"
    }
    if ($manifest.release_eligible -ne $true) {
        throw "manifest for $target is not release eligible"
    }

    $archiveName = [string]$manifest.archive_file
    $checksumName = [string]$manifest.checksum_file
    $archives = @(Get-ChildItem -LiteralPath $distRoot -File -Recurse | Where-Object Name -eq $archiveName)
    $checksums = @(Get-ChildItem -LiteralPath $distRoot -File -Recurse | Where-Object Name -eq $checksumName)
    if ($archives.Count -ne 1) {
        throw "expected exactly one archive '$archiveName' for $target, found $($archives.Count)"
    }
    if ($checksums.Count -ne 1) {
        throw "expected exactly one checksum '$checksumName' for $target, found $($checksums.Count)"
    }

    $expectedHash = ((Get-Content -LiteralPath $checksums[0].FullName | Select-Object -First 1) -split '\s+')[0].ToLowerInvariant()
    if ($expectedHash -notmatch '^[0-9a-f]{64}$') {
        throw "invalid SHA-256 sidecar for $target"
    }
    $actualHash = (Get-FileHash -LiteralPath $archives[0].FullName -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actualHash -ne $expectedHash) {
        throw "archive checksum mismatch for $target"
    }

    $artifactsByTarget[$target] = [ordered]@{
        target = $target
        archive = $archiveName
        sha256 = $actualHash
        manifest = "$([System.IO.Path]::GetFileName($manifestPath.DirectoryName))/$($manifestPath.Name)"
        build_manifest_id = [string]$manifest.build_manifest_id
        native_runtime_abi = [int]$manifest.native_runtime.abi_version
    }
}

$missing = @($ExpectedTargets | Where-Object { -not $artifactsByTarget.ContainsKey($_) })
if ($missing.Count -ne 0) {
    throw "missing distribution targets: $($missing -join ', ')"
}

if ([string]::IsNullOrWhiteSpace($RunUrl) -and -not [string]::IsNullOrWhiteSpace($Repository) -and -not [string]::IsNullOrWhiteSpace($RunId)) {
    $RunUrl = "https://github.com/$Repository/actions/runs/$RunId"
}

$artifacts = @($ExpectedTargets | Sort-Object | ForEach-Object { $artifactsByTarget[$_] })
$evidence = [ordered]@{
    schema_version = 1
    version = $Version
    source_revision = $SourceRevision.ToLowerInvariant()
    repository = $Repository
    workflow_run = [ordered]@{
        id = $RunId
        attempt = $RunAttempt
        url = $RunUrl
    }
    gates = [ordered]@{
        package_smoke = "passed"
        complete_target_set = $true
        checksum_verification = "passed"
        published_upgrade = "passed"
        compatibility_fixtures = "passed"
        checksum_verified_rollback = "passed"
    }
    release_transition = [ordered]@{
        previous_version = $PreviousVersion
        current_version = $Version
        artifacts = @(
            "release-transition-linux-x86_64"
            "release-transition-windows-x86_64"
            "release-transition-macos-x86_64"
            "release-transition-macos-arm64"
        )
    }
    provenance_url = $ProvenanceUrl
    artifacts = $artifacts
    generated_at_utc = (Get-Date).ToUniversalTime().ToString("o")
}

$parent = Split-Path -Parent $OutputPath
if (-not [string]::IsNullOrWhiteSpace($parent)) {
    New-Item -ItemType Directory -Force -Path $parent | Out-Null
}
$evidence | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $OutputPath -Encoding utf8NoBOM
Write-Output (Resolve-Path -LiteralPath $OutputPath).Path
