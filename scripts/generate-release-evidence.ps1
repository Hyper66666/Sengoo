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

    [Parameter(Mandatory = $true)]
    [string]$GateRunsPath,

    [Parameter(Mandatory = $true)]
    [string]$CompatibilityFixturePath,

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

$gateRuns = Get-Content -LiteralPath $GateRunsPath -Raw | ConvertFrom-Json
if ([int]$gateRuns.schema_version -ne 1) {
    throw "unsupported required gate evidence schema: $($gateRuns.schema_version)"
}
if ([string]$gateRuns.source_revision -ne $SourceRevision) {
    throw "required gate evidence source revision mismatch"
}
$requiredWorkflowNames = @(
    "core-conformance",
    "native-safety",
    "hardening-fuzz",
    "compatibility-prerelease",
    "realworld-e2e",
    "perf-smoke"
)
$requiredGateRuns = @($gateRuns.required_runs)
foreach ($workflowName in $requiredWorkflowNames) {
    $matches = @($requiredGateRuns | Where-Object { [string]$_.workflow -eq $workflowName })
    if ($matches.Count -ne 1) {
        throw "required gate evidence must contain exactly one $workflowName run"
    }
    $run = $matches[0]
    if (
        [string]$run.head_sha -ne $SourceRevision -or
        [string]$run.head_branch -ne "main" -or
        [string]$run.event -ne "push" -or
        [string]$run.status -ne "completed" -or
        [string]$run.conclusion -ne "success"
    ) {
        throw "required gate run is not a successful main-push run for ${SourceRevision}: $workflowName"
    }
    $jobs = @($run.jobs)
    if ($jobs.Count -eq 0) {
        throw "required gate run has no retained jobs: $workflowName"
    }
    $failedJobs = @($jobs | Where-Object {
        [string]$_.status -ne "completed" -or [string]$_.conclusion -ne "success"
    })
    if ($failedJobs.Count -ne 0) {
        throw "required gate run contains non-success jobs: $workflowName"
    }
    $expiredArtifacts = @($run.artifacts | Where-Object { [bool]$_.expired })
    if ($expiredArtifacts.Count -ne 0) {
        throw "expired evidence artifact in ${workflowName}: $($expiredArtifacts.name -join ', ')"
    }
}

$distributionPrerequisites = $gateRuns.distribution_prerequisites
if ([string]$distributionPrerequisites.head_sha -ne $SourceRevision) {
    throw "distribution prerequisite source revision mismatch"
}
foreach ($jobSetName in @("package_jobs", "transition_jobs")) {
    $jobs = @($distributionPrerequisites.$jobSetName)
    if ($jobs.Count -ne 4) {
        throw "distribution prerequisite must contain four $jobSetName"
    }
    $failedJobs = @($jobs | Where-Object {
        [string]$_.status -ne "completed" -or [string]$_.conclusion -ne "success"
    })
    if ($failedJobs.Count -ne 0) {
        throw "distribution prerequisite contains non-success $jobSetName"
    }
}
$expiredDistributionArtifacts = @(
    $distributionPrerequisites.artifacts | Where-Object { [bool]$_.expired }
)
if ($expiredDistributionArtifacts.Count -ne 0) {
    throw "expired evidence artifact in distribution prerequisites: $($expiredDistributionArtifacts.name -join ', ')"
}

$compatibilityRoot = (Resolve-Path -LiteralPath $CompatibilityFixturePath).Path
$compatibilityFiles = [ordered]@{
    manifest_sha256 = "Sengoo.toml"
    lockfile_sha256 = "Sengoo.lock"
    source_sha256 = "src/lib.sg"
    test_sha256 = "tests/smoke.sg"
}
$compatibilityHashes = [ordered]@{}
foreach ($entry in $compatibilityFiles.GetEnumerator()) {
    $path = Join-Path $compatibilityRoot $entry.Value
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "compatibility fixture is missing $($entry.Value)"
    }
    $compatibilityHashes[$entry.Key] = (
        Get-FileHash -LiteralPath $path -Algorithm SHA256
    ).Hash.ToLowerInvariant()
}
$packageVersionLine = Select-String -LiteralPath (Join-Path $compatibilityRoot "Sengoo.toml") `
    -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1
if ($null -eq $packageVersionLine) {
    throw "compatibility fixture package version is missing"
}
$compatibilityFixture = [ordered]@{
    path = $CompatibilityFixturePath.Replace('\', '/')
    package_version = $packageVersionLine.Matches[0].Groups[1].Value
}
foreach ($entry in $compatibilityHashes.GetEnumerator()) {
    $compatibilityFixture[$entry.Key] = $entry.Value
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
        tool_versions = $manifest.tool_versions
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
    required_gate_runs = $requiredGateRuns
    distribution_prerequisites = $distributionPrerequisites
    compatibility_fixture = $compatibilityFixture
    known_platform_skips = @($gateRuns.known_platform_skips)
    provenance_url = $ProvenanceUrl
    artifacts = $artifacts
    generated_at_utc = (Get-Date).ToUniversalTime().ToString("o")
}

$parent = Split-Path -Parent $OutputPath
if (-not [string]::IsNullOrWhiteSpace($parent)) {
    New-Item -ItemType Directory -Force -Path $parent | Out-Null
}
$json = $evidence | ConvertTo-Json -Depth 12
$utf8NoBom = [System.Text.UTF8Encoding]::new($false)
[System.IO.File]::WriteAllText([System.IO.Path]::GetFullPath($OutputPath), $json, $utf8NoBom)
Write-Output (Resolve-Path -LiteralPath $OutputPath).Path
