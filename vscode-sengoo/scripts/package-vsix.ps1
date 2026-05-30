Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
$tempRoot = Join-Path $root '.vsix-build'
$baseVsix = Join-Path $tempRoot 'base.vsix'
$finalVsix = Join-Path $root 'sengoo-1.0.0.vsix'
$tempZip = Join-Path $tempRoot 'base.zip'
$extractRoot = Join-Path $tempRoot 'extract'

function Invoke-Checked([string] $command) {
    Write-Host "> $command"
    cmd /c $command
    if ($LASTEXITCODE -ne 0) {
        throw "Command failed with exit code ${LASTEXITCODE}: $command"
    }
}

function Get-RelativePathCompat([string] $basePath, [string] $targetPath) {
    $baseUri = New-Object System.Uri(([System.IO.Path]::GetFullPath($basePath).TrimEnd('\') + '\'))
    $targetUri = New-Object System.Uri([System.IO.Path]::GetFullPath($targetPath))
    return [System.Uri]::UnescapeDataString($baseUri.MakeRelativeUri($targetUri).ToString()).Replace('/', '\')
}

if (Test-Path $tempRoot) {
    Remove-Item -Recurse -Force $tempRoot
}
New-Item -ItemType Directory -Path $tempRoot | Out-Null

Invoke-Checked 'npm run compile'
Invoke-Checked "npx @vscode/vsce package --no-yarn --no-dependencies --allow-missing-repository --out ""$baseVsix"""

Copy-Item $baseVsix $tempZip -Force
Expand-Archive -Path $tempZip -DestinationPath $extractRoot -Force

$extensionRoot = Join-Path $extractRoot 'extension'
$depRoots = @(
    cmd /c 'npm list --production --parseable --depth=99999 --loglevel=error' |
        Where-Object { $_ -and (Test-Path $_) }
)

foreach ($depRoot in $depRoots) {
    if ((Resolve-Path $depRoot).Path -eq (Resolve-Path $root).Path) {
        continue
    }

    $relativePath = Get-RelativePathCompat $root (Resolve-Path $depRoot).Path
    $targetPath = Join-Path $extensionRoot $relativePath
    $targetParent = Split-Path -Parent $targetPath

    if (-not (Test-Path $targetParent)) {
        New-Item -ItemType Directory -Path $targetParent -Force | Out-Null
    }

    Copy-Item -Recurse -Force $depRoot $targetPath
}

if (Test-Path $finalVsix) {
    Remove-Item -Force $finalVsix
}

Add-Type -AssemblyName System.IO.Compression.FileSystem
[System.IO.Compression.ZipFile]::CreateFromDirectory($extractRoot, $finalVsix)

if (Test-Path $tempRoot) {
    Remove-Item -Recurse -Force $tempRoot
}

Write-Host "Packaged: $finalVsix"
