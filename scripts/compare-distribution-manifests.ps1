param(
    [Parameter(Mandatory = $true)]
    [string]$LeftManifest,
    [Parameter(Mandatory = $true)]
    [string]$RightManifest,
    [Parameter(Mandatory = $true)]
    [string]$OutputDir
)

$ErrorActionPreference = "Stop"

$ManifestFields = @(
    "schema_version", "version", "target", "build_hash", "source_revision",
    "source_dirty", "artifact_provenance", "release_eligible", "build_manifest_id",
    "tools", "tool_versions", "stdlib_modules", "runtime_sources", "native_runtime",
    "payload_checksum_file", "payloads", "archive_file", "checksum_file", "runner_os",
    "runner_image", "smoke_evidence", "license_included", "generated_at_utc"
)
$NativeRuntimeFields = @(
    "abi_version", "target", "library", "sha256", "link_args", "dynamic_dependencies"
)
$PayloadFields = @("path", "sha256", "size")
$ExcludedFields = @("generated_at_utc", "runner_os", "runner_image", "smoke_evidence")

function Read-JsonObject([string]$Path, [string]$Label) {
    $resolved = (Resolve-Path -LiteralPath $Path).Path
    try {
        $value = Get-Content -LiteralPath $resolved -Raw | ConvertFrom-Json
    } catch {
        throw "$Label is not valid JSON: $($_.Exception.Message)"
    }
    if ($null -eq $value -or $value -isnot [pscustomobject]) {
        throw "$Label must contain one JSON object"
    }
    return $value
}

function Assert-ExactFields($Value, [string[]]$Expected, [string]$Label) {
    if ($null -eq $Value -or $Value -isnot [pscustomobject]) {
        throw "$Label must be an object"
    }
    $actual = @($Value.PSObject.Properties.Name)
    $missing = @($Expected | Where-Object { $_ -notin $actual })
    if ($missing.Count -ne 0) {
        throw "missing manifest field at ${Label}: $($missing -join ', ')"
    }
    $unknown = @($actual | Where-Object { $_ -notin $Expected })
    if ($unknown.Count -ne 0) {
        throw "unknown manifest field at ${Label}: $($unknown -join ', ')"
    }
}

function Require-String($Value, [string]$Label, [switch]$AllowEmpty) {
    if ($Value -isnot [string] -or (-not $AllowEmpty -and $Value.Length -eq 0)) {
        throw "$Label must be a non-empty string"
    }
    return [string]$Value
}

function Require-NullableString($Value, [string]$Label) {
    if ($null -ne $Value -and $Value -isnot [string]) {
        throw "$Label must be a string or null"
    }
    return $Value
}

function Require-Bool($Value, [string]$Label) {
    if ($Value -isnot [bool]) {
        throw "$Label must be a boolean"
    }
    return [bool]$Value
}

function Require-Integer($Value, [string]$Label, [long]$Minimum = [long]::MinValue) {
    if ($Value -isnot [byte] -and $Value -isnot [int16] -and $Value -isnot [int32] -and
        $Value -isnot [int64] -and $Value -isnot [uint16] -and $Value -isnot [uint32]) {
        throw "$Label must be an integer"
    }
    $integer = [long]$Value
    if ($integer -lt $Minimum) {
        throw "$Label must be at least $Minimum"
    }
    return $integer
}

function Require-Array($Value, [string]$Label) {
    if ($Value -isnot [System.Array]) {
        throw "$Label must be an array"
    }
    return ,$Value
}

function Normalize-Hex($Value, [int]$Length, [string]$Label) {
    $text = Require-String $Value $Label
    if ($text.Length -ne $Length -or $text -notmatch "^[0-9a-fA-F]{$Length}$") {
        throw "$Label must be exactly $Length hexadecimal characters"
    }
    return $text.ToLowerInvariant()
}

function Normalize-RelativePath($Value, [string]$Label) {
    $path = Require-String $Value $Label
    if ($path.Contains('\') -or $path.StartsWith('/') -or $path.EndsWith('/') -or
        $path -match '^[A-Za-z]:' -or $path.Contains('//')) {
        throw "$Label must be a normalized relative path"
    }
    foreach ($segment in $path.Split('/')) {
        if ($segment.Length -eq 0 -or $segment -eq '.' -or $segment -eq '..') {
            throw "$Label must be a normalized relative path"
        }
    }
    return $path
}

function Sort-Ordinal([string[]]$Values) {
    $copy = [string[]]@($Values)
    [Array]::Sort($copy, [StringComparer]::Ordinal)
    return ,$copy
}

function Normalize-StringSet($Value, [string]$Label) {
    $items = Require-Array $Value $Label
    $seen = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    $normalized = @()
    for ($index = 0; $index -lt $items.Count; $index++) {
        $item = Require-String $items[$index] "$Label[$index]"
        if (-not $seen.Add($item)) {
            throw "$Label contains duplicate value: $item"
        }
        $normalized += $item
    }
    return ,(Sort-Ordinal $normalized)
}

function Normalize-OrderedStrings($Value, [string]$Label) {
    $items = Require-Array $Value $Label
    $normalized = @()
    for ($index = 0; $index -lt $items.Count; $index++) {
        $normalized += Require-String $items[$index] "$Label[$index]"
    }
    return ,$normalized
}

function Normalize-ToolVersions($Value, [string[]]$Tools, [string]$Label) {
    Assert-ExactFields $Value $Tools $Label
    $result = [ordered]@{}
    foreach ($tool in (Sort-Ordinal $Tools)) {
        $result[$tool] = Require-String $Value.PSObject.Properties[$tool].Value "$Label.$tool"
    }
    return [pscustomobject]$result
}

function Normalize-Payloads($Value, [string]$Label) {
    $items = Require-Array $Value $Label
    if ($items.Count -eq 0) {
        throw "$Label must contain at least one payload"
    }
    $byPath = [Collections.Generic.SortedDictionary[string, object]]::new([StringComparer]::Ordinal)
    $caseFolded = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    for ($index = 0; $index -lt $items.Count; $index++) {
        $itemLabel = "$Label[$index]"
        Assert-ExactFields $items[$index] $PayloadFields $itemLabel
        $path = Normalize-RelativePath $items[$index].path "$itemLabel.path"
        if (-not $caseFolded.Add($path)) {
            throw "duplicate payload path: $path"
        }
        $payload = [pscustomobject][ordered]@{
            path = $path
            sha256 = Normalize-Hex $items[$index].sha256 64 "$itemLabel.sha256"
            size = Require-Integer $items[$index].size "$itemLabel.size" 0
        }
        $byPath.Add($path, $payload)
    }
    return ,@($byPath.Values)
}

function Normalize-Manifest($Manifest, [string]$Label) {
    Assert-ExactFields $Manifest $ManifestFields $Label
    $schemaVersion = Require-Integer $Manifest.schema_version "$Label.schema_version" 0
    if ($schemaVersion -ne 2) {
        throw "$Label.schema_version must be 2"
    }
    $version = Require-String $Manifest.version "$Label.version"
    $target = Require-String $Manifest.target "$Label.target"
    $sourceRevision = Normalize-Hex $Manifest.source_revision 40 "$Label.source_revision"
    $buildHash = Require-String $Manifest.build_hash "$Label.build_hash"
    if ($buildHash -notmatch '^[0-9a-fA-F]{7,40}$' -or
        -not $sourceRevision.StartsWith($buildHash.ToLowerInvariant(), [StringComparison]::Ordinal)) {
        throw "$Label.build_hash must be a hexadecimal prefix of source_revision"
    }
    $tools = Normalize-StringSet $Manifest.tools "$Label.tools"
    $payloads = Normalize-Payloads $Manifest.payloads "$Label.payloads"

    Assert-ExactFields $Manifest.native_runtime $NativeRuntimeFields "$Label.native_runtime"
    $nativeTarget = Require-String $Manifest.native_runtime.target "$Label.native_runtime.target"
    if ($nativeTarget -ne $target) {
        throw "$Label.native_runtime.target must equal target"
    }
    $nativeLibrary = Normalize-RelativePath $Manifest.native_runtime.library "$Label.native_runtime.library"
    $nativeHash = Normalize-Hex $Manifest.native_runtime.sha256 64 "$Label.native_runtime.sha256"
    $runtimePayload = @($payloads | Where-Object { $_.path -eq $nativeLibrary })
    if ($runtimePayload.Count -ne 1 -or $runtimePayload[0].sha256 -ne $nativeHash) {
        throw "$Label.native_runtime library must have one matching payload hash"
    }

    $normalized = [ordered]@{
        schema_version = $schemaVersion
        version = $version
        target = $target
        build_hash = $buildHash.ToLowerInvariant()
        source_revision = $sourceRevision
        source_dirty = Require-Bool $Manifest.source_dirty "$Label.source_dirty"
        artifact_provenance = Require-String $Manifest.artifact_provenance "$Label.artifact_provenance"
        release_eligible = Require-Bool $Manifest.release_eligible "$Label.release_eligible"
        build_manifest_id = Normalize-Hex $Manifest.build_manifest_id 64 "$Label.build_manifest_id"
        tools = @($tools)
        tool_versions = Normalize-ToolVersions $Manifest.tool_versions $tools "$Label.tool_versions"
        stdlib_modules = @(Normalize-StringSet $Manifest.stdlib_modules "$Label.stdlib_modules")
        runtime_sources = @(Normalize-StringSet $Manifest.runtime_sources "$Label.runtime_sources")
        native_runtime = [ordered]@{
            abi_version = Require-Integer $Manifest.native_runtime.abi_version "$Label.native_runtime.abi_version" 0
            target = $nativeTarget
            library = $nativeLibrary
            sha256 = $nativeHash
            link_args = @(Normalize-OrderedStrings $Manifest.native_runtime.link_args "$Label.native_runtime.link_args")
            dynamic_dependencies = @(Normalize-StringSet $Manifest.native_runtime.dynamic_dependencies "$Label.native_runtime.dynamic_dependencies")
        }
        payload_checksum_file = Normalize-RelativePath $Manifest.payload_checksum_file "$Label.payload_checksum_file"
        payloads = @($payloads)
        archive_file = Normalize-RelativePath $Manifest.archive_file "$Label.archive_file"
        checksum_file = Normalize-RelativePath $Manifest.checksum_file "$Label.checksum_file"
        license_included = Require-Bool $Manifest.license_included "$Label.license_included"
    }

    $excluded = [ordered]@{
        generated_at_utc = Require-String $Manifest.generated_at_utc "$Label.generated_at_utc"
        runner_os = Require-NullableString $Manifest.runner_os "$Label.runner_os"
        runner_image = Require-NullableString $Manifest.runner_image "$Label.runner_image"
        smoke_evidence = Require-String $Manifest.smoke_evidence "$Label.smoke_evidence" -AllowEmpty
    }
    return [pscustomobject][ordered]@{
        normalized = [pscustomobject]$normalized
        excluded = [pscustomobject]$excluded
    }
}

function Canonical-Json($Value, [switch]$Pretty) {
    if ($null -eq $Value) {
        return "null"
    }
    $json = if ($Pretty) {
        $Value | ConvertTo-Json -Depth 12
    } else {
        $Value | ConvertTo-Json -Depth 12 -Compress
    }
    if ($null -eq $json) {
        return "null"
    }
    return $json.Replace("`r`n", "`n")
}

function Write-Utf8NoBom([string]$Path, [string]$Text) {
    $parent = Split-Path -Parent $Path
    if ($parent) {
        New-Item -ItemType Directory -Force -Path $parent | Out-Null
    }
    $encoding = [Text.UTF8Encoding]::new($false)
    [IO.File]::WriteAllText($Path, $Text.TrimEnd("`r", "`n") + "`n", $encoding)
}

function String-Sha256([string]$Text) {
    $sha = [Security.Cryptography.SHA256]::Create()
    try {
        $bytes = [Text.UTF8Encoding]::new($false).GetBytes($Text.TrimEnd("`r", "`n") + "`n")
        return ([BitConverter]::ToString($sha.ComputeHash($bytes))).Replace("-", "").ToLowerInvariant()
    } finally {
        $sha.Dispose()
    }
}

function Json-Equal($Left, $Right) {
    return (Canonical-Json $Left) -ceq (Canonical-Json $Right)
}

$leftRaw = Read-JsonObject $LeftManifest "left manifest"
$rightRaw = Read-JsonObject $RightManifest "right manifest"
$left = Normalize-Manifest $leftRaw "left manifest"
$right = Normalize-Manifest $rightRaw "right manifest"

New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null
$leftJson = Canonical-Json $left.normalized -Pretty
$rightJson = Canonical-Json $right.normalized -Pretty
Write-Utf8NoBom (Join-Path $OutputDir "normalized-a.json") $leftJson
Write-Utf8NoBom (Join-Path $OutputDir "normalized-b.json") $rightJson
$leftSha = String-Sha256 $leftJson
$rightSha = String-Sha256 $rightJson

$differences = @()
foreach ($field in @(
    "schema_version", "version", "target", "build_hash", "source_revision", "source_dirty",
    "artifact_provenance", "release_eligible", "build_manifest_id", "tools", "tool_versions",
    "stdlib_modules", "runtime_sources", "payload_checksum_file", "payloads", "archive_file",
    "checksum_file", "license_included"
)) {
    if (-not (Json-Equal $left.normalized.$field $right.normalized.$field)) {
        $differences += $field
    }
}
foreach ($field in $NativeRuntimeFields) {
    if (-not (Json-Equal $left.normalized.native_runtime.$field $right.normalized.native_runtime.$field)) {
        $differences += "native_runtime.$field"
    }
}

$excludedDifferences = @()
foreach ($field in $ExcludedFields) {
    if (-not (Json-Equal $left.excluded.$field $right.excluded.$field)) {
        $excludedDifferences += [pscustomobject][ordered]@{
            field = $field
            left = $left.excluded.$field
            right = $right.excluded.$field
        }
    }
}

$status = if ($differences.Count -eq 0) { "reproducible" } else { "mismatch" }
$comparison = [pscustomobject][ordered]@{
    schema_version = 1
    status = $status
    left = [ordered]@{
        manifest = [IO.Path]::GetFileName($LeftManifest)
        normalized_file = "normalized-a.json"
        normalized_sha256 = $leftSha
    }
    right = [ordered]@{
        manifest = [IO.Path]::GetFileName($RightManifest)
        normalized_file = "normalized-b.json"
        normalized_sha256 = $rightSha
    }
    excluded_fields = $ExcludedFields
    excluded_differences = @($excludedDifferences)
    mismatched_fields = @($differences)
}
Write-Utf8NoBom (Join-Path $OutputDir "comparison.json") (Canonical-Json $comparison -Pretty)

if ($differences.Count -ne 0) {
    throw "distribution manifests differ: $($differences -join ', ')"
}
if ($leftSha -ne $rightSha) {
    throw "normalized manifest SHA-256 differs despite no field mismatch"
}

Write-Host "Distribution manifests are reproducible: $leftSha"
