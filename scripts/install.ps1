param(
    [string]$Archive = "",
    [string]$Version = "",
    [string]$Target = "",
    [string]$BaseUrl = "https://github.com/Hyper66666/Sengoo/releases/download",
    [string]$InstallDir = (Join-Path $HOME ".sengoo"),
    [switch]$AddToPath
)

$ErrorActionPreference = "Stop"

function Is-WindowsHost {
    if ($PSVersionTable.PSEdition -eq "Desktop") {
        return $true
    }
    return [bool]$IsWindows
}

function Default-Target {
    if (Is-WindowsHost) {
        return "x86_64-pc-windows-msvc"
    }
    if ($IsMacOS) {
        return "x86_64-apple-darwin"
    }
    return "x86_64-unknown-linux-gnu"
}

function Download-File($Url, $Destination) {
    Write-Host "Downloading $Url"
    Invoke-WebRequest -Uri $Url -OutFile $Destination
}

if (-not $Archive -and -not $Version) {
    throw "provide -Archive PATH or -Version VERSION"
}
if ($Archive -and $Version) {
    throw "provide only one of -Archive or -Version"
}

$InstallDir = [System.IO.Path]::GetFullPath($InstallDir)
$tempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("sengoo-install-" + [System.Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $tempRoot | Out-Null

try {
    if ($Version) {
        if (-not $Target) {
            $Target = Default-Target
        }
        $extension = if ($Target -like "*windows*") { "zip" } else { "tar.gz" }
        $archiveName = "sengoo-$Version-$Target.$extension"
        $releaseBase = "$($BaseUrl.TrimEnd('/'))/v$Version"
        $Archive = Join-Path $tempRoot $archiveName
        Download-File "$releaseBase/$archiveName" $Archive
        Download-File "$releaseBase/$archiveName.sha256" "$Archive.sha256"
    }

    $archivePath = (Resolve-Path $Archive).Path
    $checksumPath = "$archivePath.sha256"
    if (-not (Test-Path -LiteralPath $checksumPath)) {
        throw "checksum file not found: $checksumPath"
    }
    $expectedHash = ((Get-Content -LiteralPath $checksumPath | Select-Object -First 1) -split "\s+")[0].ToLowerInvariant()
    $actualHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $archivePath).Hash.ToLowerInvariant()
    if ($expectedHash -ne $actualHash) {
        throw "checksum mismatch for $archivePath"
    }

    if ($archivePath.EndsWith(".zip")) {
        Expand-Archive -LiteralPath $archivePath -DestinationPath $tempRoot -Force
    } elseif ($archivePath.EndsWith(".tar.gz") -or $archivePath.EndsWith(".tgz")) {
        & tar -xzf $archivePath -C $tempRoot
        if ($LASTEXITCODE -ne 0) {
            throw "tar extraction failed"
        }
    } else {
        throw "unsupported archive type: $archivePath"
    }

    $payload = Get-ChildItem -LiteralPath $tempRoot -Directory |
        Where-Object { Test-Path -LiteralPath (Join-Path $_.FullName "manifest.json") } |
        Select-Object -First 1
    if (-not $payload) {
        throw "archive does not contain a Sengoo manifest.json"
    }

    Remove-Item -LiteralPath $InstallDir -Recurse -Force -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    Copy-Item -Path (Join-Path $payload.FullName "*") -Destination $InstallDir -Recurse -Force

    $binDir = Join-Path $InstallDir "bin"
    $sgc = Join-Path $binDir "sgc.exe"
    if (-not (Test-Path -LiteralPath $sgc)) {
        $sgc = Join-Path $binDir "sgc"
    }
    if (Test-Path -LiteralPath $sgc) {
        & $sgc --version
    }

    if ($AddToPath) {
        $currentPath = [Environment]::GetEnvironmentVariable("Path", "User")
        $pathParts = @($currentPath -split ';' | Where-Object { $_ })
        if ($pathParts -notcontains $binDir) {
            $nextPath = (@($pathParts) + $binDir) -join ';'
            [Environment]::SetEnvironmentVariable("Path", $nextPath, "User")
            Write-Host "Added $binDir to the user PATH. Open a new shell to use it."
        }
    } else {
        Write-Host "Add this directory to PATH: $binDir"
    }

    Write-Host "Installed Sengoo to $InstallDir"
} finally {
    Remove-Item -LiteralPath $tempRoot -Recurse -Force -ErrorAction SilentlyContinue
}
