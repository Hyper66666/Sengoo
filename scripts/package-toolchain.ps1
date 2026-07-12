param(
    [string]$Version = "0.1.0-dev",
    [string]$OutputDir = "target/dist",
    [string]$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path,
    [switch]$NoBuild,
    [string]$SmokeEvidence = ""
)

$ErrorActionPreference = "Stop"

function Is-WindowsHost {
    if ($PSVersionTable.PSEdition -eq "Desktop") {
        return $true
    }
    return [bool]$IsWindows
}

function Build-Hash {
    if ($env:SENGOO_BUILD_HASH) {
        return $env:SENGOO_BUILD_HASH
    }
    $hash = (& git -C $RepoRoot rev-parse --short=12 HEAD 2>$null)
    if ($LASTEXITCODE -eq 0 -and $hash) {
        return $hash.Trim()
    }
    return "unknown"
}

function Target-Label {
    if ($env:SENGOO_DIST_TARGET) {
        return $env:SENGOO_DIST_TARGET
    }
    if (Is-WindowsHost) {
        return "x86_64-pc-windows-msvc"
    }
    $architecture = [System.Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture.ToString()
    if ($IsMacOS) {
        if ($architecture -eq "Arm64") {
            return "aarch64-apple-darwin"
        }
        return "x86_64-apple-darwin"
    }
    if ($architecture -eq "Arm64") {
        return "aarch64-unknown-linux-gnu"
    }
    return "x86_64-unknown-linux-gnu"
}

if (-not $NoBuild) {
    & cargo build -p sgc -p sgpm -p sgfmt -p sglsp --release
    if ($LASTEXITCODE -ne 0) {
        throw "cargo release build failed"
    }
}

$target = Target-Label
$buildHash = Build-Hash
$distRoot = Join-Path $RepoRoot $OutputDir
$stageName = "sengoo-$Version-$target"
$stage = Join-Path $distRoot $stageName
$binDir = Join-Path $stage "bin"
$scriptsDir = Join-Path $stage "scripts"
$stdlibDir = Join-Path $stage "share/sengoo/stdlib"
$runtimeDir = Join-Path $stage "share/sengoo/runtime"

Remove-Item -LiteralPath $stage -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $binDir, $scriptsDir, $stdlibDir, $runtimeDir | Out-Null

$exeSuffix = if (Is-WindowsHost) { ".exe" } else { "" }
$tools = @("sgc", "sgpm", "sgfmt", "sglsp")
$toolVersions = [ordered]@{}
foreach ($tool in $tools) {
    $binary = "$tool$exeSuffix"
    $source = Join-Path $RepoRoot "target/release/$binary"
    if (-not (Test-Path -LiteralPath $source)) {
        throw "missing release binary: $source"
    }
    Copy-Item -LiteralPath $source -Destination (Join-Path $binDir $binary)
    $toolVersions[$tool] = (& $source --version).Trim()
}

$readme = Join-Path $RepoRoot "README.md"
if (Test-Path -LiteralPath $readme) {
    Copy-Item -LiteralPath $readme -Destination (Join-Path $stage "README.md")
}
$distReadme = Join-Path $RepoRoot "README-dist.md"
if (Test-Path -LiteralPath $distReadme) {
    Copy-Item -LiteralPath $distReadme -Destination (Join-Path $stage "README-dist.md")
}

$license = Get-ChildItem -LiteralPath $RepoRoot -File -Force |
    Where-Object { $_.Name -match '^LICENSE(\..*)?$' -or $_.Name -match '^COPYING(\..*)?$' } |
    Select-Object -First 1
if ($license) {
    Copy-Item -LiteralPath $license.FullName -Destination (Join-Path $stage "LICENSE")
}

foreach ($script in @("install.ps1", "install.sh")) {
    $source = Join-Path $RepoRoot "scripts/$script"
    if (Test-Path -LiteralPath $source) {
        Copy-Item -LiteralPath $source -Destination (Join-Path $scriptsDir $script)
    }
}

$stdlibSource = Join-Path $RepoRoot "tools/stdlib"
Get-ChildItem -LiteralPath $stdlibSource -File | Where-Object {
    $_.Extension -eq ".sg" -or $_.Name -eq "README.md"
} | ForEach-Object {
    Copy-Item -LiteralPath $_.FullName -Destination (Join-Path $stdlibDir $_.Name)
}

Get-ChildItem -LiteralPath $stdlibSource -File | Where-Object {
    $_.Name -like "runtime*.c" -or $_.Name -eq "runtime_shared.h"
} | ForEach-Object {
    Copy-Item -LiteralPath $_.FullName -Destination (Join-Path $runtimeDir $_.Name)
    Copy-Item -LiteralPath $_.FullName -Destination (Join-Path $stdlibDir $_.Name)
}

$archiveBase = Join-Path $distRoot $stageName
$archive = if (Is-WindowsHost) { "$archiveBase.zip" } else { "$archiveBase.tar.gz" }
$archiveLeaf = Split-Path -Leaf $archive
$checksumLeaf = "$archiveLeaf.sha256"

$manifest = [ordered]@{
    schema_version = 1
    version = $Version
    target = $target
    build_hash = $buildHash
    tools = $tools
    tool_versions = $toolVersions
    stdlib_modules = @(Get-ChildItem -LiteralPath $stdlibDir -Filter "*.sg" | Sort-Object Name | ForEach-Object { $_.Name })
    runtime_sources = @(Get-ChildItem -LiteralPath $runtimeDir -File | Sort-Object Name | ForEach-Object { $_.Name })
    archive_file = $archiveLeaf
    checksum_file = $checksumLeaf
    runner_os = $env:RUNNER_OS
    runner_image = $env:ImageOS
    smoke_evidence = $SmokeEvidence
    license_included = [bool]$license
    generated_at_utc = (Get-Date).ToUniversalTime().ToString("o")
}
$manifest | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $stage "manifest.json") -Encoding UTF8

Remove-Item -LiteralPath $archive -Force -ErrorAction SilentlyContinue
Remove-Item -LiteralPath "$archive.sha256" -Force -ErrorAction SilentlyContinue

if (Is-WindowsHost) {
    Compress-Archive -Path $stage -DestinationPath $archive -Force
} else {
    & tar -czf $archive -C $distRoot $stageName
    if ($LASTEXITCODE -ne 0) {
        throw "tar archive creation failed"
    }
}

$sha = (Get-FileHash -Algorithm SHA256 -LiteralPath $archive).Hash.ToLowerInvariant()
"$sha  $archiveLeaf" | Set-Content -LiteralPath "$archive.sha256" -Encoding ASCII
$archive | Set-Content -LiteralPath (Join-Path $distRoot "latest-archive.txt") -Encoding UTF8

Write-Host "Packaged Sengoo toolchain:"
Write-Host "  archive: $archive"
Write-Host "  sha256:  $archive.sha256"
