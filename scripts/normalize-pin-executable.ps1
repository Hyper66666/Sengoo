# Normalize residual non-content identity in pin-grade package executables so
# independent dual builds can share bit-identical payload hashes when the
# functional content matches. Used by package-senline-worker/http.ps1.
#
# - PE (Windows): zero COFF TimeDateStamp and optional-header CheckSum.
# - ELF (Linux): strip .note.gnu.build-id and .comment when objcopy is available.
#
# Returns a hashtable describing tools used (path, version string, exit codes)
# so package provenance can record pin-normalization inputs.

function Get-ToolVersion([string]$ToolPath) {
    if (-not $ToolPath) { return $null }
    try {
        $out = & $ToolPath --version 2>&1 | Out-String
        return ($out -replace '\s+', ' ').Trim()
    } catch {
        return $null
    }
}

function Normalize-PinExecutable {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path
    )
    if (-not (Test-Path -LiteralPath $Path)) {
        throw "Normalize-PinExecutable: missing file $Path"
    }
    $provenance = [ordered]@{
        path = $Path
        pe_timestamp_zeroed = $false
        elf_objcopy = $null
        elf_strip = $null
    }
    $bytes = [IO.File]::ReadAllBytes($Path)
    if ($bytes.Length -lt 64) {
        return $provenance
    }

    # PE: MZ ... PE\0\0
    if ($bytes[0] -eq 0x4D -and $bytes[1] -eq 0x5A) {
        $peOff = [BitConverter]::ToInt32($bytes, 0x3C)
        if ($peOff -le 0 -or ($peOff + 24) -ge $bytes.Length) {
            return $provenance
        }
        if ($bytes[$peOff] -ne 0x50 -or $bytes[$peOff + 1] -ne 0x45) {
            return $provenance
        }
        # COFF TimeDateStamp at PE+8
        $bytes[$peOff + 8] = 0
        $bytes[$peOff + 9] = 0
        $bytes[$peOff + 10] = 0
        $bytes[$peOff + 11] = 0
        # Optional header starts at PE+24; CheckSum is at optional+64 for PE32/PE32+
        $optOff = $peOff + 24
        if (($optOff + 68) -lt $bytes.Length) {
            $magic = [BitConverter]::ToUInt16($bytes, $optOff)
            if ($magic -eq 0x10B -or $magic -eq 0x20B) {
                $checkOff = $optOff + 64
                $bytes[$checkOff] = 0
                $bytes[$checkOff + 1] = 0
                $bytes[$checkOff + 2] = 0
                $bytes[$checkOff + 3] = 0
            }
        }
        [IO.File]::WriteAllBytes($Path, $bytes)
        $provenance.pe_timestamp_zeroed = $true
        return $provenance
    }

    # ELF: \x7fELF — strip non-content note/comment sections and optional
    # full symbol strip so dual independent release builds hash-match.
    if ($bytes[0] -eq 0x7F -and $bytes[1] -eq 0x45 -and $bytes[2] -eq 0x4C -and $bytes[3] -eq 0x46) {
        $objcopy = $null
        foreach ($name in @("llvm-objcopy", "llvm-objcopy-19", "llvm-objcopy-18", "objcopy")) {
            $cmd = Get-Command $name -ErrorAction SilentlyContinue
            if ($cmd) { $objcopy = $cmd.Source; break }
        }
        if ($objcopy) {
            & $objcopy --remove-section=.note.gnu.build-id --remove-section=.comment $Path 2>$null
            $code = $LASTEXITCODE
            # Missing sections may yield non-zero; only fail hard on tool not found (already resolved).
            $provenance.elf_objcopy = [ordered]@{
                path = $objcopy
                version = Get-ToolVersion $objcopy
                exit_code = $code
            }
            if ($code -gt 1) {
                throw "Normalize-PinExecutable: objcopy failed exit=$code path=$objcopy"
            }
        }
        $strip = $null
        foreach ($name in @("llvm-strip", "llvm-strip-19", "llvm-strip-18", "strip")) {
            $cmd = Get-Command $name -ErrorAction SilentlyContinue
            if ($cmd) { $strip = $cmd.Source; break }
        }
        if ($strip) {
            & $strip -s $Path 2>$null
            $code = $LASTEXITCODE
            $provenance.elf_strip = [ordered]@{
                path = $strip
                version = Get-ToolVersion $strip
                exit_code = $code
            }
            if ($code -ne 0) {
                throw "Normalize-PinExecutable: strip failed exit=$code path=$strip"
            }
        }
    }
    return $provenance
}
