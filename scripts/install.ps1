param(
    [string]$Archive = "",
    [string]$Version = "",
    [string]$Target = "",
    [string]$BaseUrl = "https://github.com/Hyper66666/Sengoo/releases/download",
    [string]$InstallDir = (Join-Path $HOME ".sengoo"),
    [switch]$AddToPath,
    [switch]$PrintTarget
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

function Download-File($Url, $Destination) {
    Write-Host "Downloading $Url"
    Invoke-WebRequest -Uri $Url -OutFile $Destination
}

function Copy-OrDownloadFile($Source, $Destination) {
    if (Test-Path -LiteralPath $Source) {
        Write-Host "Copying $Source"
        Copy-Item -LiteralPath $Source -Destination $Destination
        return
    }
    Download-File $Source $Destination
}

if ($PrintTarget) {
    Write-Output (Default-Target)
    return
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
        $Archive = Join-Path $tempRoot $archiveName
        if (Test-Path -LiteralPath $BaseUrl) {
            $releaseBase = Join-Path $BaseUrl "v$Version"
            $Source = Join-Path $releaseBase $archiveName
            $ChecksumSource = "$Source.sha256"
        } else {
            $releaseBase = "$($BaseUrl.TrimEnd('/'))/v$Version"
            $Source = "$releaseBase/$archiveName"
            $ChecksumSource = "$Source.sha256"
        }
        Copy-OrDownloadFile $Source $Archive
        Copy-OrDownloadFile $ChecksumSource "$Archive.sha256"
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

    $manifestPath = Join-Path $payload.FullName "manifest.json"
    $manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
    if ($manifest.schema_version -ne 2) {
        throw "unsupported Sengoo manifest schema: $($manifest.schema_version)"
    }
    $payloadChecksums = Join-Path $payload.FullName "payloads.sha256"
    if (-not (Test-Path -LiteralPath $payloadChecksums)) {
        throw "archive does not contain payloads.sha256"
    }
    $payloadRoot = [System.IO.Path]::GetFullPath($payload.FullName).TrimEnd([char[]]@('\', '/'))
    $verifiedPayloads = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
    foreach ($line in Get-Content -LiteralPath $payloadChecksums) {
        if ($line -notmatch '^([0-9a-fA-F]{64})  (.+)$') {
            throw "invalid payload checksum entry: $line"
        }
        $expectedPayloadHash = $Matches[1].ToLowerInvariant()
        $relativePayloadPath = $Matches[2].Replace('/', [System.IO.Path]::DirectorySeparatorChar)
        $payloadPath = [System.IO.Path]::GetFullPath((Join-Path $payloadRoot $relativePayloadPath))
        if (-not $payloadPath.StartsWith($payloadRoot + [System.IO.Path]::DirectorySeparatorChar, [System.StringComparison]::OrdinalIgnoreCase)) {
            throw "payload checksum path escapes archive root: $relativePayloadPath"
        }
        if (-not (Test-Path -LiteralPath $payloadPath -PathType Leaf)) {
            throw "payload file is missing: $relativePayloadPath"
        }
        $actualPayloadHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $payloadPath).Hash.ToLowerInvariant()
        if ($expectedPayloadHash -ne $actualPayloadHash) {
            throw "payload checksum mismatch for $relativePayloadPath"
        }
        $null = $verifiedPayloads.Add($payloadPath)
    }
    $unlistedPayloads = @(
        Get-ChildItem -LiteralPath $payloadRoot -Recurse -File | Where-Object {
            $_.FullName -ne $manifestPath -and
            $_.FullName -ne $payloadChecksums -and
            -not $verifiedPayloads.Contains([System.IO.Path]::GetFullPath($_.FullName))
        }
    )
    if ($unlistedPayloads.Count -ne 0) {
        throw "archive contains payload files missing from payloads.sha256: $($unlistedPayloads.FullName -join ', ')"
    }
    $actualBuildManifestId = (Get-FileHash -Algorithm SHA256 -LiteralPath $payloadChecksums).Hash.ToLowerInvariant()
    if ($manifest.build_manifest_id -ne $actualBuildManifestId) {
        throw "payload checksum manifest identity mismatch"
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
