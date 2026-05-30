param(
    [ValidateSet("soft", "hard")]
    [string]$Mode = "soft",
    [Parameter(Mandatory = $true)]
    [string]$Sample,
    [string]$Baseline = "baseline.json",
    [string]$DecisionOut = ""
)

$ErrorActionPreference = "Stop"
$benchRoot = Resolve-Path (Join-Path $PSScriptRoot "..")

function Resolve-InputPath {
    param(
        [Parameter(Mandatory = $true)]
        [string]$PathValue
    )
    if (Test-Path -LiteralPath $PathValue) {
        return (Resolve-Path $PathValue).Path
    }
    $candidate = Join-Path $benchRoot $PathValue
    if (Test-Path -LiteralPath $candidate) {
        return (Resolve-Path $candidate).Path
    }
    return $PathValue
}

function Format-Number {
    param(
        [Parameter(Mandatory = $true)]
        [double]$Value
    )
    return [string]::Format("{0:N2}", $Value)
}

$samplePath = Resolve-InputPath -PathValue $Sample
$baselinePath = Resolve-InputPath -PathValue $Baseline

if (-not (Test-Path -LiteralPath $samplePath)) {
    Write-Error "sample report not found: $samplePath"
    exit 2
}
if (-not (Test-Path -LiteralPath $baselinePath)) {
    Write-Error "baseline file not found: $baselinePath"
    exit 2
}

$reportDoc = Get-Content -LiteralPath $samplePath -Raw | ConvertFrom-Json
$baselineDoc = Get-Content -LiteralPath $baselinePath -Raw | ConvertFrom-Json

$baselineCaseByKey = @{}
if ($null -ne $baselineDoc -and $null -ne $baselineDoc.cases) {
    foreach ($entry in $baselineDoc.cases.PSObject.Properties) {
        $baselineCaseByKey[$entry.Name] = $entry.Value
    }
}

$targets = $baselineDoc.targets
$summaries = @()
$violations = @()
$comparisons = @()
$checked = 0

$kind = [string]$reportDoc.kind
$suite = [string]$reportDoc.suite
$reportCases = @($reportDoc.cases)

foreach ($reportCase in $reportCases) {
    $caseName = [string]$reportCase.name
    $caseKey = "$kind/$suite/$caseName"
    $baselineCase = $baselineCaseByKey[$caseKey]

    switch -Exact ($kind) {
        "runtime" {
            if ($null -eq $baselineCase -or $null -eq $reportCase.p50_ms -or $null -eq $baselineCase.p50_ms) {
                continue
            }
            $checked++
            $improvement = (($baselineCase.p50_ms - $reportCase.p50_ms) / $baselineCase.p50_ms) * 100.0
            $target = $targets.runtime_median_improvement_pct
            $targetText = "n/a"
            if ($null -ne $target) {
                $targetText = "$(Format-Number -Value ([double]$target))%"
            }
            $deltaPct = (($reportCase.p50_ms - $baselineCase.p50_ms) / $baselineCase.p50_ms) * 100.0
            $passed = $true
            if ($null -ne $target -and $improvement -lt [double]$target) {
                $passed = $false
            }
            $comparisons += [pscustomobject]@{
                case_key = $caseKey
                kind = $kind
                suite = $suite
                case = $caseName
                metric = "p50_ms"
                baseline = [double]$baselineCase.p50_ms
                measured = [double]$reportCase.p50_ms
                delta_pct = [double]$deltaPct
                objective = "improvement_pct"
                objective_pct = [double]$improvement
                target_pct = if ($null -eq $target) { $null } else { [double]$target }
                pass = $passed
            }
            $summaries += "runtime/${caseName}: improvement=$(Format-Number -Value $improvement)% target=$targetText"
            if (-not $passed) {
                $violations += "runtime/$caseName below target ($(Format-Number -Value $improvement)% < $(Format-Number -Value ([double]$target))%)"
            }
        }
        "compile" {
            if ($null -eq $baselineCase -or $null -eq $reportCase.total_ms -or $null -eq $baselineCase.total_ms) {
                continue
            }
            $checked++
            $reduction = (($baselineCase.total_ms - $reportCase.total_ms) / $baselineCase.total_ms) * 100.0
            $target = $targets.full_compile_reduction_pct
            $targetText = "n/a"
            if ($null -ne $target) {
                $targetText = "$(Format-Number -Value ([double]$target))%"
            }
            $deltaPct = (($reportCase.total_ms - $baselineCase.total_ms) / $baselineCase.total_ms) * 100.0
            $passed = $true
            if ($null -ne $target -and $reduction -lt [double]$target) {
                $passed = $false
            }
            $comparisons += [pscustomobject]@{
                case_key = $caseKey
                kind = $kind
                suite = $suite
                case = $caseName
                metric = "total_ms"
                baseline = [double]$baselineCase.total_ms
                measured = [double]$reportCase.total_ms
                delta_pct = [double]$deltaPct
                objective = "reduction_pct"
                objective_pct = [double]$reduction
                target_pct = if ($null -eq $target) { $null } else { [double]$target }
                pass = $passed
            }
            $summaries += "compile/${caseName}: reduction=$(Format-Number -Value $reduction)% target=$targetText"
            if (-not $passed) {
                $violations += "compile/$caseName below target ($(Format-Number -Value $reduction)% < $(Format-Number -Value ([double]$target))%)"
            }
        }
        "incremental" {
            if ($null -eq $reportCase.before_ms -or $null -eq $reportCase.after_ms -or $reportCase.before_ms -le 0) {
                continue
            }
            $checked++
            $reduction = (($reportCase.before_ms - $reportCase.after_ms) / $reportCase.before_ms) * 100.0
            $target = $targets.incremental_compile_reduction_pct
            $targetText = "n/a"
            if ($null -ne $target) {
                $targetText = "$(Format-Number -Value ([double]$target))%"
            }
            $deltaPct = (($reportCase.after_ms - $reportCase.before_ms) / $reportCase.before_ms) * 100.0
            $passed = $true
            if ($null -ne $target -and $reduction -lt [double]$target) {
                $passed = $false
            }
            $comparisons += [pscustomobject]@{
                case_key = $caseKey
                kind = $kind
                suite = $suite
                case = $caseName
                metric = "after_ms"
                baseline = [double]$reportCase.before_ms
                measured = [double]$reportCase.after_ms
                delta_pct = [double]$deltaPct
                objective = "reduction_pct"
                objective_pct = [double]$reduction
                target_pct = if ($null -eq $target) { $null } else { [double]$target }
                pass = $passed
            }
            $summaries += "incremental/${caseName}: reduction=$(Format-Number -Value $reduction)% target=$targetText"
            if (-not $passed) {
                $violations += "incremental/$caseName below target ($(Format-Number -Value $reduction)% < $(Format-Number -Value ([double]$target))%)"
            }
        }
        default {
            continue
        }
    }
}

if ($checked -eq 0) {
    Write-Error "no comparable benchmark metrics found in $samplePath"
    exit 2
}

Write-Output "perf-gate mode=$Mode sample=$samplePath baseline=$baselinePath"
foreach ($line in $summaries) {
    Write-Output "  - $line"
}

$decisionPath = $null
if ([string]::IsNullOrWhiteSpace($DecisionOut)) {
    $sampleDir = Split-Path -Parent $samplePath
    $sampleStem = [System.IO.Path]::GetFileNameWithoutExtension($samplePath)
    $decisionPath = Join-Path $sampleDir "$sampleStem-perf-gate.json"
} elseif ([System.IO.Path]::IsPathRooted($DecisionOut)) {
    $decisionPath = $DecisionOut
} else {
    if ($DecisionOut.StartsWith(".\") -or $DecisionOut.StartsWith("..\")) {
        $decisionPath = [System.IO.Path]::GetFullPath((Join-Path (Get-Location) $DecisionOut))
    } else {
        $decisionPath = Join-Path $benchRoot $DecisionOut
    }
}

$decisionDir = Split-Path -Parent $decisionPath
if (-not [string]::IsNullOrWhiteSpace($decisionDir) -and -not (Test-Path -LiteralPath $decisionDir)) {
    New-Item -ItemType Directory -Path $decisionDir -Force | Out-Null
}

$decisionPayload = [ordered]@{
    schema_version = 1
    mode = $Mode
    sample = $samplePath
    baseline = $baselinePath
    kind = $kind
    suite = $suite
    comparisons = $comparisons
    violations = $violations
    gate_decision = if ($violations.Count -eq 0) { "pass" } else { "fail" }
}
$decisionPayload | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $decisionPath -Encoding utf8
Write-Output "  decision-artifact=$decisionPath"

if ($violations.Count -eq 0) {
    Write-Output "perf-gate PASS"
    exit 0
}

Write-Output "perf-gate found $($violations.Count) target violation(s):"
foreach ($line in $violations) {
    Write-Output "  ! $line"
}

if ($Mode -eq "hard") {
    Write-Error "perf-gate HARD failure"
    exit 1
}

Write-Output "perf-gate SOFT warning (not failing build)"
exit 0
