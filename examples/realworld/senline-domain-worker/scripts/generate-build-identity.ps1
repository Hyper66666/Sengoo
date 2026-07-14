param(
    [Parameter(Mandatory = $true)]
    [string]$SourceRevision,
    [Parameter(Mandatory = $true)]
    [string]$ToolchainVersion,
    [Parameter(Mandatory = $true)]
    [string]$ApplicationVersion,
    [Parameter(Mandatory = $true)]
    [string]$BuildManifestId,
    [Parameter(Mandatory = $true)]
    [string]$OutputPath,
    [Parameter(Mandatory = $true)]
    [string]$HandshakeOutputPath
)

$ErrorActionPreference = "Stop"

if ($SourceRevision -cnotmatch '^[0-9a-f]{40}$') {
    throw "SourceRevision must be exactly 40 lowercase hexadecimal characters"
}
if ($BuildManifestId -cnotmatch '^[0-9a-f]{64}$') {
    throw "BuildManifestId must be exactly 64 lowercase hexadecimal characters"
}
foreach ($version in @($ToolchainVersion, $ApplicationVersion)) {
    if ($version -cnotmatch '^[0-9A-Za-z][0-9A-Za-z.+-]{0,63}$') {
        throw "versions must be 1..64 portable ASCII identifier characters"
    }
}

$handshake = '{"kind":"handshake","protocol_version":1,"sengoo_source_revision":"' +
    $SourceRevision + '","toolchain_version":"' + $ToolchainVersion +
    '","application_version":"' + $ApplicationVersion +
    '","build_manifest_id":"' + $BuildManifestId + '"}'
$escapedHandshake = $handshake.Replace('\', '\\').Replace('"', '\"')
$source = @"
def senline_build_source_revision() -> &str {
    "$SourceRevision";
}

def senline_build_toolchain_version() -> &str {
    "$ToolchainVersion";
}

def senline_build_application_version() -> &str {
    "$ApplicationVersion";
}

def senline_build_manifest_id() -> &str {
    "$BuildManifestId";
}

def senline_build_handshake_payload() -> &str {
    "$escapedHandshake\n";
}
"@.Replace("`r`n", "`n")
$source = $source + "`n"

$utf8NoBom = [System.Text.UTF8Encoding]::new($false)
foreach ($path in @($OutputPath, $HandshakeOutputPath)) {
    $parent = Split-Path -Parent ([System.IO.Path]::GetFullPath($path))
    if ($parent) {
        [System.IO.Directory]::CreateDirectory($parent) | Out-Null
    }
}
[System.IO.File]::WriteAllText([System.IO.Path]::GetFullPath($OutputPath), $source, $utf8NoBom)
[System.IO.File]::WriteAllText(
    [System.IO.Path]::GetFullPath($HandshakeOutputPath),
    $handshake + "`n",
    $utf8NoBom
)
