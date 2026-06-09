param(
    [ValidateSet("soft", "hard")]
    [string]$Mode = "hard",
    [string]$Sample = "",
    [string]$BaselineProfile = "bench/frontend-memory-baseline.json",
    [switch]$RunBench,
    [switch]$SkipAbsoluteTargets,
    [switch]$P0EvidenceOnly
)

$ErrorActionPreference = "Stop"
$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")

function Resolve-RepoPath {
    param([string]$PathValue)
    if ([System.IO.Path]::IsPathRooted($PathValue)) {
        return $PathValue
    }
    $candidate = Join-Path $repoRoot $PathValue
    if (Test-Path -LiteralPath $candidate) {
        return (Resolve-Path $candidate).Path
    }
    return $candidate
}

if ($RunBench) {
    Write-Host "==> advanced_pipeline_bench.py"
    Push-Location $repoRoot
    try {
        $benchArgs = @((Join-Path $repoRoot "bench/advanced_pipeline_bench.py"))
        if ($P0EvidenceOnly) {
            $benchArgs += "--p0-evidence-only"
        }
        python @benchArgs
        if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    } finally {
        Pop-Location
    }
}

if ([string]::IsNullOrWhiteSpace($Sample)) {
    $resultsDir = Join-Path $repoRoot "bench/results"
    $latest = Get-ChildItem -LiteralPath $resultsDir -Filter "*-advanced-pipeline.json" -File |
        Sort-Object LastWriteTime -Descending |
        Select-Object -First 1
    if ($null -eq $latest) {
        Write-Error "no advanced pipeline report found under bench/results; pass -Sample or -RunBench"
        exit 2
    }
    $Sample = $latest.FullName
}

$samplePath = Resolve-RepoPath -PathValue $Sample
$baselinePath = Resolve-RepoPath -PathValue $BaselineProfile
$gateScript = Join-Path $repoRoot "bench/scripts/advanced-kpi-gate.py"

if (-not (Test-Path -LiteralPath $samplePath)) {
    Write-Error "sample report not found: $samplePath"
    exit 2
}
if (-not (Test-Path -LiteralPath $baselinePath)) {
    Write-Error "baseline profile not found: $baselinePath"
    exit 2
}

$gateArgs = @(
    $gateScript,
    "--mode", $Mode,
    "--sample", $samplePath,
    "--baseline-profile", $baselinePath
)
if ($SkipAbsoluteTargets) {
    $gateArgs += "--skip-absolute-targets"
}
if ($P0EvidenceOnly) {
    $gateArgs += "--p0-evidence-only"
}

Write-Host "==> advanced-kpi-gate mode=$Mode"
python @gateArgs
exit $LASTEXITCODE
