param(
    [Parameter(Mandatory = $true)]
    [string]$LeftManifest,
    [Parameter(Mandatory = $true)]
    [string]$RightManifest,
    [Parameter(Mandatory = $true)]
    [string]$OutputDir,
    # Explicit opt-in only. Default pin-grade policy requires every payload hash
    # (including executables) to match across dual builds. With this switch,
    # executable hash divergence is recorded under allowed_executable_drift and
    # does not fail the comparison.
    [switch]$AllowExecutableHashDrift
)

$ErrorActionPreference = "Stop"

# Required on every pin-grade package manifest (design: payload hashes, ABI,
# dependency identities, and provenance must match across dual builds).
$ManifestRequiredFields = @(
    "schema_version", "package", "version", "built_with_sgc", "source_tree",
    "source_revision", "target", "protocols",
    "runtime_dependencies", "build_tools", "license", "provenance",
    "payloads", "notes"
)
# Optional only when a package family does not emit the field (HTTP packages
# currently omit build_manifest_id; worker packages emit it).
$ManifestOptionalFields = @(
    "build_manifest_id"
)
$PayloadFields = @("path", "sha256", "size")
$ExecutableNamePatterns = @(
    "senline_domain_worker",
    "senline_domain_worker.exe",
    "senline_http_dogfood",
    "senline_http_dogfood.exe"
)

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
    if ($null -eq $Value -or $Value -is [string] -or $Value -is [ValueType] -or $Value -is [System.Array]) {
        throw "$Label must be an object"
    }
    if ($null -eq $Value.PSObject -or $null -eq $Value.PSObject.Properties) {
        throw "$Label must be an object with properties"
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

function Assert-ManifestFields($Value, [string]$Label) {
    if ($null -eq $Value -or $Value -is [string] -or $Value -is [ValueType] -or $Value -is [System.Array]) {
        throw "$Label must be an object"
    }
    if ($null -eq $Value.PSObject -or $null -eq $Value.PSObject.Properties) {
        throw "$Label must be an object with properties"
    }
    $actual = @($Value.PSObject.Properties.Name)
    $allowed = @($ManifestRequiredFields + $ManifestOptionalFields)
    $missing = @($ManifestRequiredFields | Where-Object { $_ -notin $actual })
    if ($missing.Count -ne 0) {
        throw "missing required manifest field at ${Label}: $($missing -join ', ')"
    }
    $unknown = @($actual | Where-Object { $_ -notin $allowed })
    if ($unknown.Count -ne 0) {
        throw "unknown manifest field at ${Label}: $($unknown -join ', ')"
    }
}

function Require-String($Value, [string]$Label) {
    if ($null -eq $Value -or $Value -isnot [string] -or $Value.Length -eq 0) {
        throw "$Label must be a non-empty string"
    }
    return [string]$Value
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
    # Emit elements to the output stream (no unary-comma wrapper). Callers wrap
    # with @() so a single JSON object is one element and multi-element arrays
    # expand correctly. Unary-comma would nest arrays and break [0] indexing.
    if ($null -eq $Value) {
        return @()
    }
    if ($Value -is [string]) {
        return @($Value)
    }
    if ($Value -is [System.Array]) {
        return @($Value)
    }
    if ($Value -is [System.Collections.IEnumerable]) {
        return @($Value)
    }
    return @($Value)
}

function Normalize-Hex($Value, [string]$Label) {
    $text = Require-String $Value $Label
    if ($text.Length -ne 64 -or $text -notmatch '^[0-9a-fA-F]{64}$') {
        throw "$Label must be exactly 64 hexadecimal characters"
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

function Is-ExecutablePayload([string]$Path) {
    return $ExecutableNamePatterns -contains $Path
}

function Normalize-Payloads($Value, [string]$Label) {
    $items = @(Require-Array $Value $Label)
    if ($items.Count -eq 0) {
        throw "$Label must contain at least one payload"
    }
    $normalized = New-Object System.Collections.Generic.List[object]
    $seen = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    for ($index = 0; $index -lt $items.Count; $index++) {
        $itemLabel = "$Label[$index]"
        Assert-ExactFields $items[$index] $PayloadFields $itemLabel
        $path = Normalize-RelativePath $items[$index].path "$itemLabel.path"
        if (-not $seen.Add($path)) {
            throw "duplicate payload path: $path"
        }
        $normalized.Add([pscustomobject][ordered]@{
            path = $path
            sha256 = Normalize-Hex $items[$index].sha256 "$itemLabel.sha256"
            size = Require-Integer $items[$index].size "$itemLabel.size" 0
        }) | Out-Null
    }
    # Sort by path without collapsing the collection through pipeline unwrapping.
    return @($normalized | Sort-Object -Property path)
}

function Normalize-DependencyList($Value, [string]$Label) {
    $items = @(Require-Array $Value $Label)
    $normalized = New-Object System.Collections.Generic.List[object]
    for ($i = 0; $i -lt $items.Count; $i++) {
        $item = $items[$i]
        if ($null -eq $item -or $null -eq $item.PSObject) {
            throw "$Label[$i] must be an object"
        }
        $name = Require-String $item.name "$Label[$i].name"
        $entry = [ordered]@{ name = $name }
        foreach ($prop in @($item.PSObject.Properties.Name | Sort-Object)) {
            if ($prop -eq "name") { continue }
            $entry[$prop] = $item.$prop
        }
        $normalized.Add([pscustomobject]$entry) | Out-Null
    }
    # Sort by name so dual-build ordering cannot mask identity changes.
    return @($normalized | Sort-Object -Property name)
}

function Normalize-Manifest($Manifest, [string]$Label) {
    Assert-ManifestFields $Manifest $Label
    $schemaVersion = Require-Integer $Manifest.schema_version "$Label.schema_version" 0
    if ($schemaVersion -ne 1) {
        throw "$Label.schema_version must be 1"
    }
    # Always re-wrap with @(): a single JSON string (collapsed one-element array)
    # must not become a character-enumerable scalar.
    $protocolItems = @(Require-Array $Manifest.protocols "$Label.protocols")
    $protocols = New-Object System.Collections.Generic.List[string]
    for ($i = 0; $i -lt $protocolItems.Count; $i++) {
        $protocols.Add((Require-String $protocolItems[$i] "$Label.protocols[$i]")) | Out-Null
    }
    $noteItems = @(Require-Array $Manifest.notes "$Label.notes")
    $notes = New-Object System.Collections.Generic.List[string]
    for ($i = 0; $i -lt $noteItems.Count; $i++) {
        $notes.Add((Require-String $noteItems[$i] "$Label.notes[$i]")) | Out-Null
    }
    $normalized = [ordered]@{
        schema_version = $schemaVersion
        package = Require-String $Manifest.package "$Label.package"
        version = Require-String $Manifest.version "$Label.version"
        built_with_sgc = Require-String $Manifest.built_with_sgc "$Label.built_with_sgc"
        source_tree = Require-String $Manifest.source_tree "$Label.source_tree"
        source_revision = Require-String $Manifest.source_revision "$Label.source_revision"
        target = $Manifest.target
        protocols = @($protocols)
        runtime_dependencies = @(Normalize-DependencyList $Manifest.runtime_dependencies "$Label.runtime_dependencies")
        build_tools = @(Normalize-DependencyList $Manifest.build_tools "$Label.build_tools")
        license = $Manifest.license
        provenance = $Manifest.provenance
        payloads = @(Normalize-Payloads $Manifest.payloads "$Label.payloads")
        notes = @($notes)
    }
    if ($null -eq $normalized.target -or $null -eq $normalized.target.PSObject) {
        throw "$Label.target must be an object"
    }
    if ($null -eq $normalized.license -or $null -eq $normalized.license.PSObject) {
        throw "$Label.license must be an object"
    }
    if ($null -eq $normalized.provenance -or $null -eq $normalized.provenance.PSObject) {
        throw "$Label.provenance must be an object"
    }
    foreach ($optional in $ManifestOptionalFields) {
        if ($null -ne $Manifest.PSObject.Properties[$optional]) {
            $normalized[$optional] = $Manifest.$optional
        }
    }
    return $normalized
}

function Write-Utf8NoBom([string]$Path, [string]$Text) {
    $utf8 = [Text.UTF8Encoding]::new($false)
    [IO.File]::WriteAllText($Path, $Text, $utf8)
}

function Canonical-Json($Object) {
    return ($Object | ConvertTo-Json -Depth 12 -Compress)
}

function Compare-MetaField($Left, $Right, [string]$Field, [System.Collections.ArrayList]$Mismatches) {
    $leftHas = $Left.Contains($Field)
    $rightHas = $Right.Contains($Field)
    if (-not $leftHas -and -not $rightHas) { return }
    $leftVal = if ($leftHas) { Canonical-Json $Left[$Field] } else { "<missing>" }
    $rightVal = if ($rightHas) { Canonical-Json $Right[$Field] } else { "<missing>" }
    if ($leftVal -ne $rightVal) {
        [void]$Mismatches.Add([ordered]@{
            field = $Field
            left = if ($leftHas) { $Left[$Field] } else { $null }
            right = if ($rightHas) { $Right[$Field] } else { $null }
        })
    }
}

$left = Normalize-Manifest (Read-JsonObject $LeftManifest "left") "left"
$right = Normalize-Manifest (Read-JsonObject $RightManifest "right") "right"

# Scalar/object/array meta fields that must match for pin-grade dual builds.
$metaFields = @(
    "schema_version", "package", "version", "built_with_sgc", "source_tree",
    "source_revision", "build_manifest_id", "target",
    "runtime_dependencies", "build_tools", "license", "provenance"
)
$metaMismatches = New-Object System.Collections.ArrayList
foreach ($field in $metaFields) {
    Compare-MetaField $left $right $field $metaMismatches
}

$leftProtocols = ($left.protocols -join "`n")
$rightProtocols = ($right.protocols -join "`n")
if ($leftProtocols -ne $rightProtocols) {
    [void]$metaMismatches.Add([ordered]@{
        field = "protocols"
        left = $left.protocols
        right = $right.protocols
    })
}

$leftByPath = @{}
foreach ($payload in @($left.payloads)) {
    if ($null -eq $payload -or -not $payload.PSObject.Properties['path']) {
        throw "left payload entry missing path property"
    }
    $leftByPath[[string]$payload.path] = $payload
}
$rightByPath = @{}
foreach ($payload in @($right.payloads)) {
    if ($null -eq $payload -or -not $payload.PSObject.Properties['path']) {
        throw "right payload entry missing path property"
    }
    $rightByPath[[string]$payload.path] = $payload
}

$pathSet = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
foreach ($key in @($leftByPath.Keys)) { [void]$pathSet.Add([string]$key) }
foreach ($key in @($rightByPath.Keys)) { [void]$pathSet.Add([string]$key) }
$allPaths = @($pathSet | Sort-Object)
$payloadMismatches = @()
$allowedExecutableDrift = @()
$executableHashDivergences = @()
$identicalPayloads = 0

foreach ($path in $allPaths) {
    $lp = $leftByPath[$path]
    $rp = $rightByPath[$path]
    if ($null -eq $lp -or $null -eq $rp) {
        $payloadMismatches += [ordered]@{
            path = $path
            reason = "missing_on_one_side"
            left = $lp
            right = $rp
        }
        continue
    }
    $hashEqual = $lp.sha256 -eq $rp.sha256
    $sizeEqual = $lp.size -eq $rp.size
    if ($hashEqual -and $sizeEqual) {
        $identicalPayloads++
        continue
    }
    $isExe = Is-ExecutablePayload $path
    # Design requires payload hashes match across dual builds (including
    # executables). AllowExecutableHashDrift is an explicit opt-in only; default
    # fails closed so 8.7 cannot report green on PE/ELF hash divergence.
    if ($AllowExecutableHashDrift -and $isExe) {
        $allowedExecutableDrift += [ordered]@{
            path = $path
            left_sha256 = $lp.sha256
            right_sha256 = $rp.sha256
            left_size = $lp.size
            right_size = $rp.size
        }
        if ($isExe -and -not $hashEqual) {
            $executableHashDivergences += [ordered]@{
                path = $path
                left_sha256 = $lp.sha256
                right_sha256 = $rp.sha256
                left_size = $lp.size
                right_size = $rp.size
                policy = "allow_executable_hash_drift_opt_in"
            }
        }
        continue
    }
    $payloadMismatches += [ordered]@{
        path = $path
        reason = if ($isExe -and -not $hashEqual -and $sizeEqual) {
            "executable_hash_mismatch"
        } elseif ($isExe -and -not $sizeEqual) {
            "executable_size_mismatch"
        } else {
            "hash_or_size_mismatch"
        }
        left_sha256 = $lp.sha256
        right_sha256 = $rp.sha256
        left_size = $lp.size
        right_size = $rp.size
    }
}

$ok = ($metaMismatches.Count -eq 0) -and ($payloadMismatches.Count -eq 0)
$comparison = [ordered]@{
    schema_version = 1
    ok = $ok
    allow_executable_hash_drift = [bool]$AllowExecutableHashDrift
    pin_grade_policy = "payload_hashes_and_dependency_identities_must_match_unless_AllowExecutableHashDrift"
    identical_payload_count = $identicalPayloads
    allowed_executable_drift_count = $allowedExecutableDrift.Count
    executable_hash_divergence_count = $executableHashDivergences.Count
    meta_mismatch_count = $metaMismatches.Count
    payload_mismatch_count = $payloadMismatches.Count
    meta_mismatches = @($metaMismatches)
    allowed_executable_drift = @($allowedExecutableDrift)
    executable_hash_divergences = @($executableHashDivergences)
    payload_mismatches = @($payloadMismatches)
}

$OutputDir = if ([IO.Path]::IsPathRooted($OutputDir)) {
    [IO.Path]::GetFullPath($OutputDir)
} else {
    [IO.Path]::GetFullPath((Join-Path (Get-Location) $OutputDir))
}
New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null
Write-Utf8NoBom (Join-Path $OutputDir "normalized-a.json") (Canonical-Json $left)
Write-Utf8NoBom (Join-Path $OutputDir "normalized-b.json") (Canonical-Json $right)
Write-Utf8NoBom (Join-Path $OutputDir "comparison.json") (($comparison | ConvertTo-Json -Depth 12))

if (-not $ok) {
    Write-Error "senline package manifest comparison failed; see $OutputDir\comparison.json"
    exit 1
}

Write-Host "senline package manifests match (payloads=$identicalPayloads executable_drift=$($allowedExecutableDrift.Count))"
Write-Host "comparison: $(Join-Path $OutputDir 'comparison.json')"
