param(
    [Parameter(Mandatory = $true)]
    [string]$SgcPath,
    [Parameter(Mandatory = $true)]
    [string]$OutputDir,
    [string]$SgpmPath = "",
    [string]$WorkerRoot = "",
    [string]$Version = "0.1.0-worker"
)

$ErrorActionPreference = "Stop"

if (-not $WorkerRoot) {
    $WorkerRoot = Join-Path $PSScriptRoot "..\examples\realworld\senline-domain-worker"
}
$WorkerRoot = (Resolve-Path -LiteralPath $WorkerRoot).Path
$SgcPath = (Resolve-Path -LiteralPath $SgcPath).Path
if (-not $SgpmPath) {
    $SgpmPath = Join-Path (Split-Path -Parent $SgcPath) $(if ($env:OS -eq "Windows_NT" -or $IsWindows) { "sgpm.exe" } else { "sgpm" })
}
$SgpmPath = (Resolve-Path -LiteralPath $SgpmPath).Path

$OutputDir = if ([IO.Path]::IsPathRooted($OutputDir)) {
    [IO.Path]::GetFullPath($OutputDir)
} else {
    [IO.Path]::GetFullPath((Join-Path (Get-Location) $OutputDir))
}
Remove-Item -LiteralPath $OutputDir -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null

$binDir = Split-Path -Parent $SgcPath
$pathSep = [IO.Path]::PathSeparator
$env:PATH = "$binDir$pathSep" + $env:PATH
$env:SGPM_SGC = $SgcPath
$sgfmtCandidate = Join-Path $binDir $(if ($env:OS -eq "Windows_NT" -or $IsWindows) { "sgfmt.exe" } else { "sgfmt" })
if (Test-Path -LiteralPath $sgfmtCandidate) {
    $env:SGPM_SGFMT = $sgfmtCandidate
}

# Release packaging must regenerate build identity (not ship fixture-mode all-1s).
$repoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
$sourceRevision = (git -C $repoRoot rev-parse HEAD).Trim().ToLowerInvariant()
if ($sourceRevision -cnotmatch '^[0-9a-f]{40}$') {
    throw "git rev-parse HEAD did not return a 40-char lowercase revision"
}
$sgcVersionRaw = (& $SgcPath --version).Trim()
# Keep portable identifier characters only for embedded identity fields.
$toolchainVersion = ($sgcVersionRaw -replace '[^0-9A-Za-z.+-]', '')
if (-not $toolchainVersion) { $toolchainVersion = "0.0.0" }
if ($toolchainVersion.Length -gt 64) { $toolchainVersion = $toolchainVersion.Substring(0, 64) }
$applicationVersion = $Version
$identitySeed = "{0}|{1}|{2}|senline-domain-worker" -f $sourceRevision, $toolchainVersion, $applicationVersion
$identityBytes = [Text.Encoding]::UTF8.GetBytes($identitySeed)
$sha = [Security.Cryptography.SHA256]::Create()
$buildManifestId = ($sha.ComputeHash($identityBytes) | ForEach-Object { $_.ToString("x2") }) -join ""
$generateIdentity = Join-Path $WorkerRoot "scripts\generate-build-identity.ps1"
$identityOut = Join-Path $WorkerRoot "packages\senline-build-identity\src\lib.sg"
$handshakeOut = Join-Path $WorkerRoot "fixtures\v1\handshake\ready.json"
# Preserve fixture-mode sources; packaging rewrites them for the release binary
# then restores so the worktree is not left with a non-fixture identity.
$identityBackup = [IO.Path]::GetTempFileName()
$handshakeBackup = [IO.Path]::GetTempFileName()
Copy-Item -LiteralPath $identityOut -Destination $identityBackup -Force
Copy-Item -LiteralPath $handshakeOut -Destination $handshakeBackup -Force
try {
    & $generateIdentity `
        -SourceRevision $sourceRevision `
        -ToolchainVersion $toolchainVersion `
        -ApplicationVersion $applicationVersion `
        -BuildManifestId $buildManifestId `
        -OutputPath $identityOut `
        -HandshakeOutputPath $handshakeOut
    if ($LASTEXITCODE -ne 0) {
        throw "generate-build-identity.ps1 failed"
    }

    Push-Location $WorkerRoot
    try {
        # sgpm resolves locked path deps and module maps; use installed tools only.
        # Force installed runtime mode so packaging never falls back to checkout
        # source-development runtime or cargo.
        & $SgpmPath --runtime-mode installed build --locked --release
        if ($LASTEXITCODE -ne 0) {
            throw "sgpm --runtime-mode installed build --locked --release failed for senline-domain-worker"
        }
    } finally {
        Pop-Location
    }

    # Copy the release binary and *regenerated* fixtures into the package BEFORE
    # restoring fixture-mode identity sources. Otherwise package handshake JSON
    # would be the all-1s fixture while the binary embeds the release identity.
    $hostIsWindowsInner = ($env:OS -eq "Windows_NT") -or ((Get-Variable -Name IsWindows -ErrorAction SilentlyContinue) -and $IsWindows)
    $exeNameInner = if ($hostIsWindowsInner) { "senline_domain_worker.exe" } else { "senline_domain_worker" }
    $builtInner = Join-Path $WorkerRoot (Join-Path "target" (Join-Path "release" $exeNameInner))
    if (-not (Test-Path -LiteralPath $builtInner)) {
        throw "missing built worker executable: $builtInner"
    }
    Copy-Item -LiteralPath $builtInner -Destination (Join-Path $OutputDir $exeNameInner) -Force
    Copy-Item -LiteralPath (Join-Path $WorkerRoot "fixtures") -Destination (Join-Path $OutputDir "fixtures") -Recurse -Force
    Copy-Item -LiteralPath (Join-Path $WorkerRoot "README.md") -Destination (Join-Path $OutputDir "README.md") -Force
    Copy-Item -LiteralPath (Join-Path $WorkerRoot "Sengoo.toml") -Destination (Join-Path $OutputDir "Sengoo.toml") -Force
    Copy-Item -LiteralPath (Join-Path $WorkerRoot "Sengoo.lock") -Destination (Join-Path $OutputDir "Sengoo.lock") -Force
} finally {
    Copy-Item -LiteralPath $identityBackup -Destination $identityOut -Force
    Copy-Item -LiteralPath $handshakeBackup -Destination $handshakeOut -Force
    Remove-Item -LiteralPath $identityBackup, $handshakeBackup -Force -ErrorAction SilentlyContinue
}

# Avoid $isWindows: PowerShell is case-insensitive and $IsWindows is read-only.
$hostIsWindows = ($env:OS -eq "Windows_NT") -or ((Get-Variable -Name IsWindows -ErrorAction SilentlyContinue) -and $IsWindows)
$exeName = if ($hostIsWindows) { "senline_domain_worker.exe" } else { "senline_domain_worker" }
if (-not (Test-Path -LiteralPath (Join-Path $OutputDir $exeName))) {
    throw "package missing worker executable after identity-aware copy: $(Join-Path $OutputDir $exeName)"
}

function Get-Sha256([string]$Path) {
    return (Get-FileHash -Algorithm SHA256 -LiteralPath $Path).Hash.ToLowerInvariant()
}

$sgcVersion = $sgcVersionRaw
$arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString().ToLowerInvariant()
$osName = if ($hostIsWindows) { "windows" } else { "linux" }
$abi = if ($hostIsWindows) { "msvc" } else { "gnu" }
$utf8 = [Text.UTF8Encoding]::new($false)

# Ship license + SBOM input files before hashing package payloads.
$licenseText = @(
    "Senline domain worker package ($Version)",
    "Source revision: $sourceRevision",
    "Build-manifest id: $buildManifestId",
    "License: UNLICENSED pending repository SPDX publication.",
    "See Sengoo.toml package metadata and the parent Sengoo distribution license."
) -join "`n"
[IO.File]::WriteAllText((Join-Path $OutputDir "LICENSE.txt"), $licenseText + "`n", $utf8)

function Get-PackagePayloads([string]$Root, [string[]]$ExcludeNames) {
    $items = New-Object System.Collections.Generic.List[object]
    Get-ChildItem -LiteralPath $Root -Recurse -File | ForEach-Object {
        $rel = $_.FullName.Substring($Root.Length).TrimStart('\', '/').Replace('\', '/')
        if ($ExcludeNames -contains $rel) { return }
        $items.Add([ordered]@{
            path = $rel
            sha256 = Get-Sha256 $_.FullName
            size = $_.Length
        }) | Out-Null
    }
    return @($items | Sort-Object -Property path)
}

$componentPayloads = Get-PackagePayloads -Root $OutputDir -ExcludeNames @("sbom-inputs.json", "worker-manifest.json")
$sbom = [ordered]@{
    schema_version = 1
    package = "senline-domain-worker"
    version = $Version
    source_revision = $sourceRevision
    build_manifest_id = $buildManifestId
    components = @($componentPayloads)
}
[IO.File]::WriteAllText((Join-Path $OutputDir "sbom-inputs.json"), (($sbom | ConvertTo-Json -Depth 6) + "`n"), $utf8)

# Manifest payloads include every package file except the manifest itself.
$payloads = Get-PackagePayloads -Root $OutputDir -ExcludeNames @("worker-manifest.json")
$manifest = [ordered]@{
    schema_version = 1
    package = "senline-domain-worker"
    version = $Version
    built_with_sgc = $sgcVersion
    source_revision = $sourceRevision
    source_tree = "examples/realworld/senline-domain-worker"
    build_manifest_id = $buildManifestId
    target = [ordered]@{
        os = $osName
        arch = $arch
        abi = $abi
        triple = if ($hostIsWindows) { "x86_64-pc-windows-msvc" } else { "x86_64-unknown-linux-gnu" }
    }
    protocols = @("senline-worker-v1")
    runtime_dependencies = @(
        [ordered]@{ name = "sengoo_runtime"; role = "installed-native-runtime"; note = "Provided by installed Sengoo toolchain for the package target" }
    )
    build_tools = @(
        [ordered]@{ name = "sgc"; version = $sgcVersion; role = "installed-toolchain-build" }
        [ordered]@{ name = "sgpm"; role = "installed-package-manager" }
    )
    license = [ordered]@{
        spdx_expression = "UNLICENSED"
        file = "LICENSE.txt"
        note = "Package ships LICENSE.txt; repository root may not yet publish a SPDX license."
    }
    provenance = [ordered]@{
        built_with_installed_toolchain_only = $true
        generate_build_identity = $true
        cargo_forbidden_at_package_time = $true
        sbom_inputs = "sbom-inputs.json"
    }
    payloads = $payloads
    notes = @(
        "Built with installed toolchain binaries only (sgpm + sgc).",
        "Build identity regenerated from source revision before release build.",
        "No Sengoo compiler checkout path is required at runtime."
    )
}
$manifestPath = Join-Path $OutputDir "worker-manifest.json"
[IO.File]::WriteAllText($manifestPath, ($manifest | ConvertTo-Json -Depth 6), $utf8)
Write-Host "Worker package written to $OutputDir"
Write-Host "  executable: $(Join-Path $OutputDir $exeName)"
Write-Host "  manifest:   $manifestPath"
