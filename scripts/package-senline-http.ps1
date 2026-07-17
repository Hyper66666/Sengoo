param(
    [Parameter(Mandatory = $true)]
    [string]$SgcPath,
    [Parameter(Mandatory = $true)]
    [string]$OutputDir,
    [string]$SgpmPath = "",
    [string]$HttpRoot = "",
    [string]$Version = "0.1.0-http-dogfood"
)

$ErrorActionPreference = "Stop"

if (-not $HttpRoot) {
    $HttpRoot = Join-Path $PSScriptRoot "..\examples\realworld\senline-http-dogfood"
}
$HttpRoot = (Resolve-Path -LiteralPath $HttpRoot).Path
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

$repoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
$sourceRevision = (git -C $repoRoot rev-parse HEAD).Trim().ToLowerInvariant()
if ($sourceRevision -cnotmatch '^[0-9a-f]{40}$') {
    throw "git rev-parse HEAD did not return a 40-char lowercase revision"
}

Push-Location $HttpRoot
try {
    & $SgpmPath --runtime-mode installed build --locked --release
    if ($LASTEXITCODE -ne 0) {
        throw "sgpm --runtime-mode installed build --locked --release failed for senline-http-dogfood"
    }
} finally {
    Pop-Location
}

# Avoid $isWindows: PowerShell is case-insensitive and $IsWindows is read-only.
$hostIsWindows = ($env:OS -eq "Windows_NT") -or ((Get-Variable -Name IsWindows -ErrorAction SilentlyContinue) -and $IsWindows)
$exeName = if ($hostIsWindows) { "senline_http_dogfood.exe" } else { "senline_http_dogfood" }
$built = Join-Path $HttpRoot "target\release\$exeName"
if (-not (Test-Path -LiteralPath $built)) {
    # Linux/mac path separator fallback
    $builtUnix = Join-Path $HttpRoot "target/release/$exeName"
    if (Test-Path -LiteralPath $builtUnix) {
        $built = $builtUnix
    } else {
        throw "missing built HTTP dogfood executable: $built"
    }
}
Copy-Item -LiteralPath $built -Destination (Join-Path $OutputDir $exeName) -Force
Copy-Item -LiteralPath (Join-Path $HttpRoot "README.md") -Destination (Join-Path $OutputDir "README.md") -Force
Copy-Item -LiteralPath (Join-Path $HttpRoot "Sengoo.toml") -Destination (Join-Path $OutputDir "Sengoo.toml") -Force
Copy-Item -LiteralPath (Join-Path $HttpRoot "Sengoo.lock") -Destination (Join-Path $OutputDir "Sengoo.lock") -Force

function Get-Sha256([string]$Path) {
    return (Get-FileHash -Algorithm SHA256 -LiteralPath $Path).Hash.ToLowerInvariant()
}

$sgcVersion = (& $SgcPath --version).Trim()
$arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString().ToLowerInvariant()
$osName = if ($hostIsWindows) { "windows" } else { "linux" }
$abi = if ($hostIsWindows) { "msvc" } else { "gnu" }
$utf8 = [Text.UTF8Encoding]::new($false)

# Ship license + SBOM input files before hashing package payloads.
$licenseText = @(
    "Senline HTTP dogfood package ($Version)",
    "Source revision: $sourceRevision",
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

$componentPayloads = Get-PackagePayloads -Root $OutputDir -ExcludeNames @("sbom-inputs.json", "http-manifest.json")
$sbom = [ordered]@{
    schema_version = 1
    package = "senline-http-dogfood"
    version = $Version
    source_revision = $sourceRevision
    components = @($componentPayloads)
}
[IO.File]::WriteAllText((Join-Path $OutputDir "sbom-inputs.json"), (($sbom | ConvertTo-Json -Depth 6) + "`n"), $utf8)

$payloads = Get-PackagePayloads -Root $OutputDir -ExcludeNames @("http-manifest.json")
$manifest = [ordered]@{
    schema_version = 1
    package = "senline-http-dogfood"
    version = $Version
    built_with_sgc = $sgcVersion
    source_revision = $sourceRevision
    source_tree = "examples/realworld/senline-http-dogfood"
    target = [ordered]@{
        os = $osName
        arch = $arch
        abi = $abi
        triple = if ($hostIsWindows) { "x86_64-pc-windows-msvc" } else { "x86_64-unknown-linux-gnu" }
    }
    protocols = @("senline-worker-v1", "http-loopback-dogfood-v1")
    runtime_dependencies = @(
        [ordered]@{ name = "sengoo_runtime"; role = "installed-native-runtime"; note = "Provided by installed Sengoo toolchain for the package target" }
        [ordered]@{ name = "senline-domain-worker"; role = "loopback-planner-backend"; note = "HTTP dogfood spawns/forwards to the framed domain worker contract" }
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
        cargo_forbidden_at_package_time = $true
        sbom_inputs = "sbom-inputs.json"
    }
    payloads = $payloads
    notes = @(
        "Built with installed toolchain binaries only (sgpm + sgc).",
        "Loopback-only synthetic harness; not TLS, ingress, or client routing.",
        "No Sengoo compiler checkout path is required at runtime."
    )
}
$manifestPath = Join-Path $OutputDir "http-manifest.json"
[IO.File]::WriteAllText($manifestPath, ($manifest | ConvertTo-Json -Depth 6), $utf8)
Write-Host "HTTP dogfood package written to $OutputDir"
Write-Host "  executable: $(Join-Path $OutputDir $exeName)"
Write-Host "  manifest:   $manifestPath"
