param(
    [string]$Version = "0.1.0-dev",
    [string]$OutputDir = "target/dist",
    [string]$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path,
    [string]$CargoTargetDir = "",
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

function Host-Target {
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

function Git-Head {
    $revision = (& git -C $RepoRoot rev-parse --verify HEAD 2>$null)
    if ($LASTEXITCODE -ne 0 -or -not $revision) {
        throw "repository HEAD is unavailable; distribution identity requires an immutable Git revision"
    }
    $revision = $revision.Trim().ToLowerInvariant()
    if ($revision -notmatch '^[0-9a-f]{40}$') {
        throw "repository HEAD must be a complete 40-character lowercase Git revision"
    }
    return $revision
}

function Git-Status {
    $status = @(& git -C $RepoRoot status --porcelain=v1 --untracked-files=all 2>$null)
    if ($LASTEXITCODE -ne 0) {
        throw "repository status is unavailable; distribution cleanliness cannot be established"
    }
    return $status
}

$RepoRoot = (Resolve-Path -LiteralPath $RepoRoot).Path
$hostTarget = Host-Target
$target = if ($env:SENGOO_DIST_TARGET) { $env:SENGOO_DIST_TARGET } else { $hostTarget }
if ($target -ne $hostTarget) {
    throw "distribution target $target does not match host target $hostTarget; cross-host packaging is unsupported"
}

$sourceRevision = Git-Head
foreach ($override in @(
    [pscustomobject]@{ name = "SENGOO_SOURCE_REVISION"; value = $env:SENGOO_SOURCE_REVISION },
    [pscustomobject]@{ name = "GITHUB_SHA"; value = $env:GITHUB_SHA }
)) {
    if ($override.value -and $override.value.ToLowerInvariant() -ne $sourceRevision) {
        throw "$($override.name) must equal repository HEAD $sourceRevision"
    }
}
$buildHash = $sourceRevision.Substring(0, 12)
if ($env:SENGOO_BUILD_HASH -and
    $env:SENGOO_BUILD_HASH.ToLowerInvariant() -ne $buildHash) {
    throw "SENGOO_BUILD_HASH must equal repository HEAD prefix $buildHash"
}
$env:SENGOO_BUILD_HASH = $buildHash

if (-not $CargoTargetDir) {
    $CargoTargetDir = Join-Path $RepoRoot "target"
} elseif (-not [IO.Path]::IsPathRooted($CargoTargetDir)) {
    $CargoTargetDir = Join-Path $RepoRoot $CargoTargetDir
}
$CargoTargetDir = [IO.Path]::GetFullPath($CargoTargetDir)
$manifestPath = Join-Path $RepoRoot "Cargo.toml"
$sourceStatusBefore = @(Git-Status)

function Set-DeterministicCargoRustflags([string]$SourceRoot, [string]$TargetDir) {
    # Unit-separator encoding required by CARGO_ENCODED_RUSTFLAGS.
    $sep = [char]0x1f
    $flags = [System.Collections.Generic.List[string]]::new()
    $sourceRoot = [IO.Path]::GetFullPath($SourceRoot).TrimEnd([char[]]@('\', '/'))
    $targetDir = [IO.Path]::GetFullPath($TargetDir).TrimEnd([char[]]@('\', '/'))
    # Remap both source checkout and cargo target so two independent package
    # builds do not bake different absolute paths into payloads.
    $flags.Add("--remap-path-prefix=$sourceRoot=/sengoo-build/src")
    $flags.Add("--remap-path-prefix=$targetDir=/sengoo-build/target")
    if (Is-WindowsHost) {
        # MSVC PE/COFF timestamps and incremental leftovers are otherwise
        # non-deterministic across independent target directories.
        $flags.Add("-C")
        $flags.Add("debuginfo=0")
        $flags.Add("-C")
        $flags.Add("link-arg=/Brepro")
        $flags.Add("-C")
        $flags.Add("link-arg=/INCREMENTAL:NO")
        $flags.Add("-C")
        $flags.Add("link-arg=/DEBUG:NONE")
    }
    $env:CARGO_ENCODED_RUSTFLAGS = ($flags -join $sep)
    Remove-Item Env:RUSTFLAGS -ErrorAction SilentlyContinue
    Write-Host "deterministic cargo rustflags: source->$sourceRoot target->$targetDir windows=$(Is-WindowsHost)"
}

if (-not $NoBuild) {
    Set-DeterministicCargoRustflags -SourceRoot $RepoRoot -TargetDir $CargoTargetDir
    & cargo build --manifest-path $manifestPath --target-dir $CargoTargetDir --locked `
        -p sgc -p sgpm -p sgfmt -p sglsp --release
    if ($LASTEXITCODE -ne 0) {
        throw "cargo release build failed"
    }
    & cargo build --manifest-path $manifestPath --target-dir $CargoTargetDir --locked `
        -p sengoo-runtime --lib --features native-bridge --profile staticlib
    if ($LASTEXITCODE -ne 0) {
        throw "native runtime static library build failed"
    }
}

$sourceStatusAfter = @(Git-Status)
if ($sourceStatusBefore.Count -eq 0 -and $sourceStatusAfter.Count -ne 0) {
    throw "package build dirtied the source tree: $($sourceStatusAfter -join '; ')"
}
$sourceDirty = $sourceStatusBefore.Count -ne 0 -or $sourceStatusAfter.Count -ne 0
$artifactProvenance = if ($NoBuild) { "prebuilt-unverified" } else { "built-by-package-toolchain" }
$releaseEligible = -not $NoBuild -and -not $sourceDirty
$distRoot = if ([IO.Path]::IsPathRooted($OutputDir)) {
    [IO.Path]::GetFullPath($OutputDir)
} else {
    [IO.Path]::GetFullPath((Join-Path $RepoRoot $OutputDir))
}
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
    $source = Join-Path $CargoTargetDir "release/$binary"
    if (-not (Test-Path -LiteralPath $source)) {
        throw "missing release binary: $source"
    }
    Copy-Item -LiteralPath $source -Destination (Join-Path $binDir $binary)
    $toolVersions[$tool] = (& $source --version).Trim()
    $expectedVersionSuffix = "($buildHash)"
    if (-not $toolVersions[$tool].EndsWith($expectedVersionSuffix, [StringComparison]::Ordinal)) {
        throw "$tool version identity does not match repository HEAD prefix ${buildHash}: $($toolVersions[$tool])"
    }
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

$runtimeLibraryName = if (Is-WindowsHost) { "sengoo_runtime.lib" } else { "libsengoo_runtime.a" }
$runtimeLibrarySource = Join-Path $CargoTargetDir "staticlib/$runtimeLibraryName"
if (-not (Test-Path -LiteralPath $runtimeLibrarySource)) {
    throw "missing target native runtime static library: $runtimeLibrarySource"
}
$runtimeTargetDir = Join-Path $runtimeDir $target
New-Item -ItemType Directory -Force -Path $runtimeTargetDir | Out-Null
$runtimeLibraryDestination = Join-Path $runtimeTargetDir $runtimeLibraryName
Copy-Item -LiteralPath $runtimeLibrarySource -Destination $runtimeLibraryDestination

Get-ChildItem -LiteralPath $stdlibSource -File | Where-Object {
    $_.Name -like "runtime*.c" -or $_.Name -eq "runtime_shared.h"
} | ForEach-Object {
    Copy-Item -LiteralPath $_.FullName -Destination (Join-Path $runtimeDir $_.Name)
    Copy-Item -LiteralPath $_.FullName -Destination (Join-Path $stdlibDir $_.Name)
}

function Relative-PayloadPath($Root, $Path) {
    $rootFull = [System.IO.Path]::GetFullPath($Root).TrimEnd([char[]]@('\', '/'))
    $pathFull = [System.IO.Path]::GetFullPath($Path)
    if (-not $pathFull.StartsWith($rootFull + [System.IO.Path]::DirectorySeparatorChar, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "payload path escapes stage root: $pathFull"
    }
    return $pathFull.Substring($rootFull.Length + 1).Replace('\', '/')
}

$payloadPaths = @(
    Get-ChildItem -LiteralPath $stage -Recurse -File | ForEach-Object {
        Relative-PayloadPath $stage $_.FullName
    }
)
[Array]::Sort($payloadPaths, [StringComparer]::Ordinal)
$payloadEntries = @(
    $payloadPaths | ForEach-Object {
        $relativePath = $_
        $payloadPath = Join-Path $stage $relativePath.Replace('/', [IO.Path]::DirectorySeparatorChar)
        [ordered]@{
            path = $relativePath
            sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $payloadPath).Hash.ToLowerInvariant()
            size = (Get-Item -LiteralPath $payloadPath).Length
        }
    }
)
$payloadChecksumPath = Join-Path $stage "payloads.sha256"
$payloadChecksumText = (($payloadEntries | ForEach-Object { "$($_.sha256)  $($_.path)" }) -join "`n") + "`n"
[IO.File]::WriteAllText($payloadChecksumPath, $payloadChecksumText, [Text.Encoding]::ASCII)
$buildManifestId = (Get-FileHash -Algorithm SHA256 -LiteralPath $payloadChecksumPath).Hash.ToLowerInvariant()
$runtimeRelativePath = Relative-PayloadPath $stage $runtimeLibraryDestination
$runtimeHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $runtimeLibraryDestination).Hash.ToLowerInvariant()
$runtimeLinkArgs = if (Is-WindowsHost) {
    @(
        "kernel32.lib", "ntdll.lib", "userenv.lib", "ws2_32.lib", "dbghelp.lib",
        "advapi32.lib", "bcrypt.lib", "crypt32.lib", "ncrypt.lib", "secur32.lib",
        "legacy_stdio_definitions.lib", "msvcrt.lib", "vcruntime.lib", "ucrt.lib"
    )
} elseif ($target -like "*apple-darwin") {
    @("-framework", "Security", "-framework", "CoreFoundation")
} else {
    @("-lm")
}

$archiveBase = Join-Path $distRoot $stageName
$archive = if (Is-WindowsHost) { "$archiveBase.zip" } else { "$archiveBase.tar.gz" }
$archiveLeaf = Split-Path -Leaf $archive
$checksumLeaf = "$archiveLeaf.sha256"

$manifest = [ordered]@{
    schema_version = 2
    version = $Version
    target = $target
    build_hash = $buildHash
    source_revision = $sourceRevision
    source_dirty = $sourceDirty
    artifact_provenance = $artifactProvenance
    release_eligible = $releaseEligible
    build_manifest_id = $buildManifestId
    tools = $tools
    tool_versions = $toolVersions
    stdlib_modules = @(Get-ChildItem -LiteralPath $stdlibDir -Filter "*.sg" | Sort-Object Name | ForEach-Object { $_.Name })
    runtime_sources = @(Get-ChildItem -LiteralPath $runtimeDir -File | Sort-Object Name | ForEach-Object { $_.Name })
    native_runtime = [ordered]@{
        abi_version = 1
        target = $target
        library = $runtimeRelativePath
        sha256 = $runtimeHash
        link_args = $runtimeLinkArgs
        dynamic_dependencies = @()
    }
    payload_checksum_file = "payloads.sha256"
    payloads = $payloadEntries
    archive_file = $archiveLeaf
    checksum_file = $checksumLeaf
    runner_os = $env:RUNNER_OS
    runner_image = $env:ImageOS
    smoke_evidence = $SmokeEvidence
    license_included = [bool]$license
    generated_at_utc = (Get-Date).ToUniversalTime().ToString("o")
}
function Format-JsonString([string]$Value) {
    $escaped = $Value.
        Replace('\', '\\').
        Replace('"', '\"').
        Replace("`r", '\r').
        Replace("`n", '\n').
        Replace("`t", '\t')
    return '"' + $escaped + '"'
}

function Format-JsonStringArray([string[]]$Values) {
    $parts = @(@($Values) | ForEach-Object { Format-JsonString ([string]$_) })
    return '[' + ($parts -join ', ') + ']'
}

function Set-JsonArrayProperty([string]$Json, [string]$Property, [string[]]$Values) {
    $arrayJson = Format-JsonStringArray $Values
    $name = [regex]::Escape($Property)
    $replacementText = '"' + $Property + '": ' + $arrayJson
    $patterns = @(
        "`"$name`"\s*:\s*`"(?:\\.|[^`"])*`"",
        "`"$name`"\s*:\s*\[[^\]]*\]",
        "`"$name`"\s*:\s*null"
    )
    $replaced = $false
    foreach ($pattern in $patterns) {
        $regex = [regex]::new($pattern)
        if ($regex.IsMatch($Json)) {
            $Json = $regex.Replace($Json, { param($m) $replacementText }, 1)
            $replaced = $true
            break
        }
    }
    if (-not $replaced) {
        throw "failed to force JSON array property: $Property"
    }
    return $Json
}

# ConvertTo-Json collapses single-element and empty arrays; force the
# contract-critical arrays back to true JSON arrays before writing.
$manifestJson = $manifest | ConvertTo-Json -Depth 8
$manifestJson = Set-JsonArrayProperty $manifestJson "link_args" ([string[]]@($runtimeLinkArgs))
$manifestJson = Set-JsonArrayProperty $manifestJson "dynamic_dependencies" @()
$utf8NoBom = [System.Text.UTF8Encoding]::new($false)
[System.IO.File]::WriteAllText((Join-Path $stage "manifest.json"), $manifestJson, $utf8NoBom)

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
