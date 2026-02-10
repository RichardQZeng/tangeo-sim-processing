param(
    [ValidateSet("check", "build", "test")]
    [string]$Command = "test",

    [switch]$Clean,

    [switch]$Release,

    [switch]$DebugOnly
)

$ErrorActionPreference = 'Stop'

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Split-Path -Parent (Split-Path -Parent $scriptDir)
$manifest = Join-Path $repoRoot "rust\Cargo.toml"
$envScript = Join-Path $scriptDir "env-win.ps1"

if (-not (Test-Path $manifest)) {
    throw "Cargo.toml not found at $manifest"
}

if (-not (Test-Path $envScript)) {
    throw "env-win.ps1 not found at $envScript"
}

. $envScript

Write-Host "Using CONDA_PREFIX: $env:CONDA_PREFIX"

if ($Clean) {
    Write-Host "Running cargo clean..."
    cargo clean --manifest-path $manifest
}

Write-Host "Running cargo $Command..."
if ($Release) {
    cargo $Command --manifest-path $manifest --release
}
elseif ($DebugOnly) {
    cargo $Command --manifest-path $manifest
}
else {
    Write-Host "Default mode: running both debug and release"
    cargo $Command --manifest-path $manifest
    cargo $Command --manifest-path $manifest --release
}
