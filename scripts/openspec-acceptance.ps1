param(
    [string]$Capability = "all"
)

$capabilityCommands = [ordered]@{
    "lsp-tooling-sglsp" = @(
        "cargo test -p sglsp"
    )
    "formatter-tooling-sgfmt" = @(
        "cargo test -p sgfmt"
    )
    "package-management-sgpm" = @(
        "cargo test -p sgpm"
    )
    "generics-core" = @(
        "cargo test -p sengoo-compiler generic_"
    )
    "async-concurrency-model" = @(
        "cargo test -p sengoo-compiler async_tests",
        "cargo test -p sengoo-runtime async_runtime"
    )
    "macro-system" = @(
        "cargo test -p sengoo-compiler macro_tests",
        "cargo test -p sengoo-compiler derive_macro_tests"
    )
    "incremental-compilation-accuracy" = @(
        "cargo test -p sgc edit_classifier_detects_",
        "cargo test -p sgc interface_change_propagates_"
    )
    "jit-aot-execution-modes" = @(
        "cargo test -p sgc cranelift",
        "cargo test -p sgc build_aot_package_flag_parses"
    )
    "python-interop-embedding" = @(
        '$env:PYO3_USE_ABI3_FORWARD_COMPATIBILITY = ''1''; cargo test -p sengoo-runtime --features python python_',
        "cargo test -p sgc build_python_extension_flag_parses"
    )
    "docs-and-api-reference" = @(
        "cargo test -p sgc doc_command_",
        "cargo test -p sgc example_validation_scripts_cover_core_cases"
    )
    "stdlib-core-collections" = @(
        "cargo test -p sengoo-compiler stdlib_surface_",
        "cargo test -p sgc stdlib_surface_runtime_",
        "cargo test -p sgc stdlib_runtime_exports_"
    )
}

if ($Capability -eq "list") {
    $capabilityCommands.Keys
    exit 0
}

$targets = if ($Capability -eq "all") {
    $capabilityCommands.Keys
} elseif ($capabilityCommands.Contains($Capability)) {
    @($Capability)
} else {
    Write-Error "Unknown capability: $Capability"
    Write-Host "Use -Capability list to see all capabilities."
    exit 1
}

foreach ($target in $targets) {
    Write-Host "==> $target"
    foreach ($cmd in $capabilityCommands[$target]) {
        Write-Host " -> $cmd"
        if ($cmd.TrimStart().StartsWith('$env:')) {
            Invoke-Expression $cmd
        } else {
            cmd /c $cmd
        }
        if ($LASTEXITCODE -ne 0) {
            exit $LASTEXITCODE
        }
    }
}

Write-Host "OpenSpec acceptance suites completed."


