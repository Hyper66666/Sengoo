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
$env:PATH = "$binDir;" + $env:PATH

Push-Location $HttpRoot
try {
    & $SgpmPath --runtime-mode installed build --locked --release
    if ($LASTEXITCODE -ne 0) {
        throw "sgpm --runtime-mode installed build --locked --release failed for senline-http-dogfood"
    }
} finally {
    Pop-Location
}

$isWindows = ($env:OS -eq "Windows_NT") -or $IsWindows
$exeName = if ($isWindows) { "senline_http_dogfood.exe" } else { "senline_http_dogfood" }
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
    package = "senline-http-dogfood"
    version = $Version
    built_with_sgc = $sgcVersion
    source_tree = "examples/realworld/senline-http-dogfood"
    protocols = @("senline-worker-v1", "http-loopback-dogfood-v1")
    payloads = $payloads
    notes = @(
        "Built with installed toolchain binaries only (sgpm + sgc).",
        "Loopback-only synthetic harness; not TLS, ingress, or client routing.",
        "No Sengoo compiler checkout path is required at runtime."
    )
}
$manifestPath = Join-Path $OutputDir "http-manifest.json"
$json = $manifest | ConvertTo-Json -Depth 6
$utf8 = [Text.UTF8Encoding]::new($false)
[IO.File]::WriteAllText($manifestPath, $json, $utf8)
Write-Host "HTTP dogfood package written to $OutputDir"
Write-Host "  executable: $(Join-Path $OutputDir $exeName)"
Write-Host "  manifest:   $manifestPath"
