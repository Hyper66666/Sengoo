# Negative tests for pin-grade dual-package comparison (task 8.7).
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
if (-not (Test-Path -LiteralPath (Join-Path $Root "scripts\compare-senline-package-manifests.ps1"))) {
    $Root = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
}
$Compare = Join-Path $Root "scripts\compare-senline-package-manifests.ps1"
$Tmp = Join-Path $env:TEMP ("senline-compare-tests-" + [guid]::NewGuid().ToString("n"))
New-Item -ItemType Directory -Force -Path $Tmp | Out-Null
$utf8 = [Text.UTF8Encoding]::new($false)

function New-BaseManifest([string]$RuntimeName = "sengoo_runtime") {
    return [ordered]@{
        schema_version = 1
        package = "senline-domain-worker"
        version = "0.0.0-test"
        built_with_sgc = "0.1.0"
        source_tree = "examples/realworld/senline-domain-worker"
        source_revision = ("a" * 40)
        build_manifest_id = ("b" * 40)
        target = [ordered]@{ os = "windows"; arch = "x86_64"; abi = "msvc"; triple = "x86_64-pc-windows-msvc" }
        protocols = @("senline-worker-v1")
        runtime_dependencies = @(
            [ordered]@{ name = $RuntimeName; role = "installed-native-runtime"; note = "test" }
        )
        build_tools = @(
            [ordered]@{ name = "sgc"; version = "0.1.0"; role = "installed-toolchain-build" }
            [ordered]@{ name = "sgpm"; role = "installed-package-manager" }
        )
        license = [ordered]@{ spdx_expression = "UNLICENSED"; file = "LICENSE.txt"; note = "test" }
        provenance = [ordered]@{
            built_with_installed_toolchain_only = $true
            generate_build_identity = $true
            cargo_forbidden_at_package_time = $true
            sbom_inputs = "sbom-inputs.json"
        }
        payloads = @(
            [ordered]@{ path = "senline_domain_worker.exe"; sha256 = ("1" * 64); size = 100 }
            [ordered]@{ path = "LICENSE.txt"; sha256 = ("2" * 64); size = 10 }
        )
        notes = @("test")
    }
}

function Write-Man([string]$Path, $Object) {
    [IO.File]::WriteAllText($Path, ($Object | ConvertTo-Json -Depth 8), $utf8)
}

function Invoke-Compare([string]$Left, [string]$Right, [string]$Out, [switch]$AllowDrift) {
    $args = @(
        "-NoProfile", "-File", $Compare,
        "-LeftManifest", $Left,
        "-RightManifest", $Right,
        "-OutputDir", $Out
    )
    if ($AllowDrift) { $args += "-AllowExecutableHashDrift" }
    $p = Start-Process -FilePath "powershell" -ArgumentList $args -Wait -PassThru -NoNewWindow
    return $p.ExitCode
}

$failed = 0

# 1) Identical manifests match.
$left = Join-Path $Tmp "left.json"
$right = Join-Path $Tmp "right.json"
Write-Man $left (New-BaseManifest)
Write-Man $right (New-BaseManifest)
$code = Invoke-Compare $left $right (Join-Path $Tmp "out-ok")
$cmp = Get-Content (Join-Path $Tmp "out-ok\comparison.json") -Raw | ConvertFrom-Json
if ($code -ne 0 -or -not $cmp.ok) {
    Write-Host "FAIL identical manifests should match (exit=$code ok=$($cmp.ok))"
    $failed++
} else {
    Write-Host "PASS identical manifests"
}

# 2) runtime_dependencies identity change must fail.
$mut = New-BaseManifest -RuntimeName "different_runtime"
Write-Man $right $mut
$code = Invoke-Compare $left $right (Join-Path $Tmp "out-dep")
$cmp = Get-Content (Join-Path $Tmp "out-dep\comparison.json") -Raw | ConvertFrom-Json
$fields = @($cmp.meta_mismatches | ForEach-Object { $_.field })
if ($code -eq 0 -or $cmp.ok -or ($fields -notcontains "runtime_dependencies")) {
    Write-Host "FAIL runtime_dependencies rename should fail closed (exit=$code ok=$($cmp.ok) fields=$($fields -join ','))"
    $failed++
} else {
    Write-Host "PASS runtime_dependencies identity mismatch fails"
}

# 3) license change must fail.
Write-Man $right (New-BaseManifest)
$mut = New-BaseManifest
$mut.license.spdx_expression = "MIT"
Write-Man $right $mut
$code = Invoke-Compare $left $right (Join-Path $Tmp "out-lic")
$cmp = Get-Content (Join-Path $Tmp "out-lic\comparison.json") -Raw | ConvertFrom-Json
$fields = @($cmp.meta_mismatches | ForEach-Object { $_.field })
if ($code -eq 0 -or $cmp.ok -or ($fields -notcontains "license")) {
    Write-Host "FAIL license change should fail (exit=$code ok=$($cmp.ok) fields=$($fields -join ','))"
    $failed++
} else {
    Write-Host "PASS license mismatch fails"
}

# 4) Equal-size executable hash divergence fails by default; opt-in allows.
$mut = New-BaseManifest
$mut.payloads[0].sha256 = ("9" * 64)
Write-Man $right $mut
$code = Invoke-Compare $left $right (Join-Path $Tmp "out-hash")
$cmp = Get-Content (Join-Path $Tmp "out-hash\comparison.json") -Raw | ConvertFrom-Json
if ($code -eq 0 -or $cmp.ok) {
    Write-Host "FAIL exe hash mismatch should fail closed"
    $failed++
} else {
    Write-Host "PASS exe hash mismatch fails closed"
}
$code = Invoke-Compare $left $right (Join-Path $Tmp "out-hash-opt") -AllowDrift
$cmp = Get-Content (Join-Path $Tmp "out-hash-opt\comparison.json") -Raw | ConvertFrom-Json
if ($code -ne 0 -or -not $cmp.ok) {
    Write-Host "FAIL AllowExecutableHashDrift should allow exe hash divergence"
    $failed++
} else {
    Write-Host "PASS AllowExecutableHashDrift opt-in"
}

Remove-Item -LiteralPath $Tmp -Recurse -Force -ErrorAction SilentlyContinue
if ($failed -ne 0) {
    Write-Error "$failed compare negative test(s) failed"
    exit 1
}
Write-Host "All compare pin-grade negative tests passed"
exit 0
