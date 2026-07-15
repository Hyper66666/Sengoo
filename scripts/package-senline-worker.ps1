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

$isWindows = ($env:OS -eq "Windows_NT") -or $IsWindows
$exeName = if ($isWindows) { "senline_domain_worker.exe" } else { "senline_domain_worker" }
$built = Join-Path $WorkerRoot (Join-Path "target" (Join-Path "release" $exeName))
if (-not (Test-Path -LiteralPath $built)) {
    throw "missing built worker executable: $built"
}
Copy-Item -LiteralPath $built -Destination (Join-Path $OutputDir $exeName) -Force

# Ship fixtures and protocol docs needed by supervisors/tests.
Copy-Item -LiteralPath (Join-Path $WorkerRoot "fixtures") -Destination (Join-Path $OutputDir "fixtures") -Recurse -Force
Copy-Item -LiteralPath (Join-Path $WorkerRoot "README.md") -Destination (Join-Path $OutputDir "README.md") -Force
Copy-Item -LiteralPath (Join-Path $WorkerRoot "Sengoo.toml") -Destination (Join-Path $OutputDir "Sengoo.toml") -Force
Copy-Item -LiteralPath (Join-Path $WorkerRoot "Sengoo.lock") -Destination (Join-Path $OutputDir "Sengoo.lock") -Force

function Get-Sha256([string]$Path) {
    return (Get-FileHash -Algorithm SHA256 -LiteralPath $Path).Hash.ToLowerInvariant()
}

$sgcVersion = (& $SgcPath --version).Trim()
$payloads = @(
    Get-ChildItem -LiteralPath $OutputDir -Recurse -File | ForEach-Object {
        $rel = $_.FullName.Substring($OutputDir.Length).TrimStart('\', '/').Replace('\', '/')
        [ordered]@{
            path = $rel
            sha256 = Get-Sha256 $_.FullName
            size = $_.Length
        }
    }
)

$manifest = [ordered]@{
    schema_version = 1
    package = "senline-domain-worker"
    version = $Version
    built_with_sgc = $sgcVersion
    source_tree = "examples/realworld/senline-domain-worker"
    protocols = @("senline-worker-v1")
    payloads = $payloads
    notes = @(
        "Built with installed toolchain binaries only (sgpm + sgc).",
        "No Sengoo compiler checkout path is required at runtime."
    )
}
$manifestPath = Join-Path $OutputDir "worker-manifest.json"
$json = $manifest | ConvertTo-Json -Depth 6
$utf8 = [Text.UTF8Encoding]::new($false)
[IO.File]::WriteAllText($manifestPath, $json, $utf8)
Write-Host "Worker package written to $OutputDir"
Write-Host "  executable: $(Join-Path $OutputDir $exeName)"
Write-Host "  manifest:   $manifestPath"
