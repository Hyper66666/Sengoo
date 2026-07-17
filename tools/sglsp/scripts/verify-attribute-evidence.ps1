param(
    [string]$Manifest = (Join-Path $PSScriptRoot '..\attribute-evidence.json')
)

$ErrorActionPreference = 'Stop'
& cargo run -p sglsp --bin attribute-evidence-verifier -- $Manifest
if ($LASTEXITCODE -ne 0) {
    throw "attribute evidence verifier failed with exit code $LASTEXITCODE"
}
