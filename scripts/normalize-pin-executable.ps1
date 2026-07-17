# Normalize residual non-content identity in pin-grade package executables so
# independent dual builds can share bit-identical payload hashes when the
# functional content matches. Used by package-senline-worker/http.ps1.
#
# - PE (Windows): zero COFF TimeDateStamp and optional-header CheckSum.
# - ELF (Linux): strip .note.gnu.build-id and .comment when objcopy is available.

function Normalize-PinExecutable {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path
    )
    if (-not (Test-Path -LiteralPath $Path)) {
        throw "Normalize-PinExecutable: missing file $Path"
    }
    $bytes = [IO.File]::ReadAllBytes($Path)
    if ($bytes.Length -lt 64) {
        return
    }

    # PE: MZ ... PE\0\0
    if ($bytes[0] -eq 0x4D -and $bytes[1] -eq 0x5A) {
        $peOff = [BitConverter]::ToInt32($bytes, 0x3C)
        if ($peOff -le 0 -or ($peOff + 24) -ge $bytes.Length) {
            return
        }
        if ($bytes[$peOff] -ne 0x50 -or $bytes[$peOff + 1] -ne 0x45) {
            return
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
        return
    }

    # ELF: \x7fELF
    if ($bytes[0] -eq 0x7F -and $bytes[1] -eq 0x45 -and $bytes[2] -eq 0x4C -and $bytes[3] -eq 0x46) {
        $candidates = @(
            "llvm-objcopy",
            "llvm-objcopy-19",
            "llvm-objcopy-18",
            "objcopy"
        )
        $objcopy = $null
        foreach ($name in $candidates) {
            $cmd = Get-Command $name -ErrorAction SilentlyContinue
            if ($cmd) {
                $objcopy = $cmd.Source
                break
            }
        }
        if ($objcopy) {
            # Best-effort: leave binary intact if section is missing.
            & $objcopy --remove-section=.note.gnu.build-id --remove-section=.comment $Path 2>$null
        }
    }
}
