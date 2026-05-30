$ErrorActionPreference = "Stop"

$benchRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
if ($env:SENGOO_ROOT) {
    $projectRoot = Resolve-Path $env:SENGOO_ROOT
}
else {
    $projectRoot = Resolve-Path (Join-Path $benchRoot "..")
}
Set-Location $projectRoot

$sample = Join-Path $benchRoot "tests/hello_print.sg"
$outDir = Join-Path $benchRoot "tests/build/e2e"
$llvmOut = Join-Path $outDir "hello_print.ll"

New-Item -ItemType Directory -Force -Path $outDir | Out-Null

if (-not (Test-Path -LiteralPath $sample)) {
    throw "[e2e] sample not found: $sample"
}

Write-Host "[e2e] checking sample program..."
cargo run -q -p sgc -- check $sample

Write-Host "[e2e] building llvm ir sample..."
cargo run -q -p sgc -- build $sample --emit-llvm --output $llvmOut

if (-not (Test-Path $llvmOut)) {
    throw "[e2e] expected LLVM IR output not found: $llvmOut"
}

$daemonAddr = if ($env:SENGOO_DAEMON_ADDR) { $env:SENGOO_DAEMON_ADDR } else { "127.0.0.1:48767" }
$fallbackAddr = "127.0.0.1:59999"
$daemonOut = Join-Path $outDir "hello_print_daemon.ll"
$fallbackOut = Join-Path $outDir "hello_print_fallback.ll"
$daemonExe = Join-Path $projectRoot "target\debug\sgc.exe"

Write-Host "[e2e] starting daemon for happy-path smoke..."
$daemonProc = Start-Process -FilePath $daemonExe -ArgumentList @("daemon", "--addr", $daemonAddr) -PassThru -WindowStyle Hidden
Start-Sleep -Seconds 2

try {
    Write-Host "[e2e] build via daemon (happy path)..."
    $happyLog = (cargo run -q -p sgc -- build $sample --emit-llvm --output $daemonOut --daemon --daemon-addr $daemonAddr) | Out-String
    Write-Host $happyLog
    if ($happyLog -notmatch "daemon build: request completed by daemon") {
        throw "[e2e] daemon happy path output missing success marker"
    }
    if (-not (Test-Path $daemonOut)) {
        throw "[e2e] daemon output missing: $daemonOut"
    }
}
finally {
    Stop-Process -Id $daemonProc.Id -Force -ErrorAction SilentlyContinue
}

Write-Host "[e2e] build via daemon fallback path..."
$fallbackLog = (cargo run -q -p sgc -- build $sample --emit-llvm --output $fallbackOut --daemon --daemon-addr $fallbackAddr) | Out-String
Write-Host $fallbackLog
if ($fallbackLog -notmatch "daemon fallback \(build\):") {
    throw "[e2e] daemon fallback output missing fallback marker"
}
if (-not (Test-Path $fallbackOut)) {
    throw "[e2e] fallback output missing: $fallbackOut"
}

Write-Host "[e2e] smoke test passed"
