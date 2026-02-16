[CmdletBinding()]
param(
    [string]$ProjectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path,
    [switch]$SkipBuild,
    [switch]$InstallVSCodeExtension
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Write-Step([string]$Message) {
    Write-Host "`n==> $Message" -ForegroundColor Cyan
}

function Write-Ok([string]$Message) {
    Write-Host "[OK] $Message" -ForegroundColor Green
}

function Write-WarnMsg([string]$Message) {
    Write-Host "[WARN] $Message" -ForegroundColor Yellow
}

function Get-CommandPath([string]$Name) {
    $cmd = Get-Command $Name -ErrorAction SilentlyContinue
    if ($null -eq $cmd) {
        return $null
    }
    return $cmd.Source
}

function Add-PathToCurrentAndUser([string]$PathToAdd) {
    if (-not (Test-Path $PathToAdd)) {
        return
    }

    $currentParts = @($env:PATH -split ";")
    if ($currentParts -notcontains $PathToAdd) {
        $env:PATH = "$PathToAdd;$env:PATH"
    }

    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if ([string]::IsNullOrWhiteSpace($userPath)) {
        [Environment]::SetEnvironmentVariable("Path", $PathToAdd, "User")
        return
    }

    $userParts = @($userPath -split ";")
    if ($userParts -notcontains $PathToAdd) {
        [Environment]::SetEnvironmentVariable("Path", "$PathToAdd;$userPath", "User")
    }
}

function Invoke-Checked([string]$FilePath, [string[]]$Arguments) {
    $joined = $Arguments -join " "
    Write-Host "  -> $FilePath $joined"
    & $FilePath @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "Command failed ($LASTEXITCODE): $FilePath $joined"
    }
}

function Install-WithWinget([string]$Id, [string]$DisplayName) {
    $winget = Get-CommandPath "winget"
    if (-not $winget) {
        return $false
    }

    Write-Step "Installing $DisplayName with winget..."
    Invoke-Checked $winget @(
        "install",
        "--id", $Id,
        "--exact",
        "--source", "winget",
        "--accept-package-agreements",
        "--accept-source-agreements",
        "--silent"
    )
    return $true
}

function Install-WithChoco([string]$PackageName, [string]$DisplayName) {
    $choco = Get-CommandPath "choco"
    if (-not $choco) {
        return $false
    }

    Write-Step "Installing $DisplayName with Chocolatey..."
    Invoke-Checked $choco @("install", $PackageName, "-y")
    return $true
}

function Ensure-RustToolchain {
    Write-Step "Checking Rust toolchain..."
    $rustc = Get-CommandPath "rustc"
    $cargo = Get-CommandPath "cargo"

    if ($rustc -and $cargo) {
        Write-Ok "Rust is already installed."
    } else {
        $installed = Install-WithWinget "Rustlang.Rustup" "Rust (rustup)"
        if (-not $installed) {
            $installed = Install-WithChoco "rustup.install" "Rust (rustup)"
        }
        if (-not $installed) {
            throw "Rust is missing and no supported installer (winget/choco) was found."
        }
    }

    Add-PathToCurrentAndUser (Join-Path $env:USERPROFILE ".cargo\bin")

    $rustup = Get-CommandPath "rustup"
    if ($rustup) {
        Write-Step "Configuring default Rust toolchain..."
        Invoke-Checked $rustup @("default", "stable")
    } else {
        Write-WarnMsg "rustup was not found. Skipping rustup configuration."
    }

    $rustc = Get-CommandPath "rustc"
    $cargo = Get-CommandPath "cargo"
    if (-not $rustc -or -not $cargo) {
        throw "Rust installation did not complete correctly (rustc/cargo missing)."
    }

    Invoke-Checked $rustc @("--version")
    Invoke-Checked $cargo @("--version")
}

function Resolve-LlvmBinFromClang([string]$ClangPath) {
    return Split-Path -Parent $ClangPath
}

function Resolve-LlvmPrefixFromClang([string]$ClangPath) {
    $binDir = Resolve-LlvmBinFromClang $ClangPath
    return Split-Path -Parent $binDir
}

function Ensure-LlvmToolchain {
    Write-Step "Checking LLVM/Clang toolchain..."
    $clang = Get-CommandPath "clang"
    $lli = Get-CommandPath "lli"

    if (-not $clang -or -not $lli) {
        $installed = Install-WithWinget "LLVM.LLVM" "LLVM"
        if (-not $installed) {
            $installed = Install-WithChoco "llvm" "LLVM"
        }
        if (-not $installed) {
            throw "LLVM is missing and no supported installer (winget/choco) was found."
        }
    }

    $defaultLlvmBin = "C:\Program Files\LLVM\bin"
    Add-PathToCurrentAndUser $defaultLlvmBin

    $clang = Get-CommandPath "clang"
    $lli = Get-CommandPath "lli"
    if (-not $clang -or -not $lli) {
        throw "LLVM install finished but clang/lli still not found in PATH."
    }

    $llvmPrefix = Resolve-LlvmPrefixFromClang $clang
    $env:LLVM_SYS_180_PREFIX = $llvmPrefix
    [Environment]::SetEnvironmentVariable("LLVM_SYS_180_PREFIX", $llvmPrefix, "User")

    Write-Ok "LLVM detected."
    Write-Host "  clang: $clang"
    Write-Host "  lli:   $lli"
    Write-Host "  LLVM_SYS_180_PREFIX=$llvmPrefix"
    Invoke-Checked $clang @("--version")
}

function Ensure-ProjectLayout([string]$Root) {
    if (-not (Test-Path (Join-Path $Root "Cargo.toml"))) {
        throw "Invalid project root: Cargo.toml not found under '$Root'."
    }
    if (-not (Test-Path (Join-Path $Root "tools\sgc\Cargo.toml"))) {
        throw "Invalid project root: tools/sgc not found under '$Root'."
    }
}

function Build-SengooTools([string]$Root) {
    Write-Step "Building Sengoo tools (release)..."
    Push-Location $Root
    try {
        Invoke-Checked "cargo" @("build", "-p", "sgc", "--release")
        Invoke-Checked "cargo" @("build", "-p", "sglsp", "--release")
    } finally {
        Pop-Location
    }
    Write-Ok "Build completed."
}

function Update-VSCodeWorkspaceSettings([string]$Root) {
    Write-Step "Configuring workspace .vscode/settings.json..."
    $vscodeDir = Join-Path $Root ".vscode"
    $settingsPath = Join-Path $vscodeDir "settings.json"
    $sgcPath = Join-Path $Root "target\release\sgc.exe"
    $sglspPath = Join-Path $Root "target\release\sglsp.exe"

    if (-not (Test-Path $vscodeDir)) {
        New-Item -Path $vscodeDir -ItemType Directory -Force | Out-Null
    }

    $settings = @{}
    if (Test-Path $settingsPath) {
        try {
            $raw = Get-Content $settingsPath -Raw -Encoding UTF8
            if (-not [string]::IsNullOrWhiteSpace($raw)) {
                $parsed = $raw | ConvertFrom-Json
                if ($parsed) {
                    foreach ($prop in $parsed.PSObject.Properties) {
                        $settings[$prop.Name] = $prop.Value
                    }
                }
            }
        } catch {
            Write-WarnMsg "Existing settings.json is not strict JSON. Recreating it."
            $backup = "$settingsPath.bak"
            Copy-Item $settingsPath $backup -Force
            Write-WarnMsg "Backup written to: $backup"
            $settings = @{}
        }
    }

    $settings["sengoo.sgc.path"] = $sgcPath
    if (Test-Path $sglspPath) {
        $settings["sengoo.lsp.path"] = $sglspPath
        $settings["sengoo.lsp.enabled"] = $true
    }

    $json = $settings | ConvertTo-Json -Depth 8
    Set-Content -Path $settingsPath -Value $json -Encoding UTF8
    Write-Ok "Workspace settings updated: $settingsPath"
}

function Install-VSCodeExtensionFromVsix([string]$Root) {
    Write-Step "Installing VSCode extension from local VSIX..."
    $code = Get-CommandPath "code"
    if (-not $code) {
        Write-WarnMsg "VS Code CLI 'code' not found. Skipping VSIX install."
        return
    }

    $pluginDir = Join-Path $Root "插件"
    if (-not (Test-Path $pluginDir)) {
        Write-WarnMsg "Plugin directory not found: $pluginDir"
        return
    }

    $vsix = Get-ChildItem $pluginDir -Filter "sengoo-*.vsix" -File |
        Sort-Object LastWriteTime -Descending |
        Select-Object -First 1

    if (-not $vsix) {
        Write-WarnMsg "No VSIX found in: $pluginDir"
        return
    }

    Invoke-Checked $code @("--install-extension", $vsix.FullName, "--force")
    Write-Ok "VSIX installed: $($vsix.Name)"
}

Write-Step "Sengoo environment bootstrap started."
Write-Host "Project root: $ProjectRoot"

if ($PSVersionTable.PSVersion.Major -lt 5) {
    throw "PowerShell 5.1+ is required."
}

Ensure-ProjectLayout $ProjectRoot
Ensure-RustToolchain
Ensure-LlvmToolchain

if (-not $SkipBuild) {
    Build-SengooTools $ProjectRoot
} else {
    Write-WarnMsg "Skip build enabled. sgc/sglsp build was skipped."
}

Update-VSCodeWorkspaceSettings $ProjectRoot

if ($InstallVSCodeExtension) {
    Install-VSCodeExtensionFromVsix $ProjectRoot
}

Write-Step "All done."
Write-Host "Next steps:"
Write-Host "  1) Open the folder in VSCode."
Write-Host "  2) Run a .sg file with command: Sengoo: 运行当前文件"
Write-Host "  3) CLI test: .\target\release\sgc.exe run .\tests\noi_algorithm.sg"
