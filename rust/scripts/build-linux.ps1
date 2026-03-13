param(
    [ValidateSet("check", "build", "test")]
    [string]$Command = "test",

    [switch]$Clean,

    [switch]$Release,

    [switch]$DebugOnly,

    [switch]$SkipCliSmoke
)

$ErrorActionPreference = 'Stop'

if ($Release -and $DebugOnly) {
    throw "Use either -Release or -DebugOnly, not both."
}

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Split-Path -Parent (Split-Path -Parent $scriptDir)
$manifestWin = Join-Path $repoRoot "rust\Cargo.toml"
$linuxScriptWin = Join-Path $scriptDir "build-linux.sh"

if (-not (Test-Path $manifestWin)) {
    throw "Cargo.toml not found at $manifestWin"
}

if (-not (Test-Path $linuxScriptWin)) {
    throw "build-linux.sh not found at $linuxScriptWin"
}

function Convert-ToWslPath {
    param([string]$WindowsPath)

    $resolved = (Resolve-Path -LiteralPath $WindowsPath).Path
    if ($resolved -notmatch '^([A-Za-z]):\\(.*)$') {
        throw "Unable to convert path to WSL format: $WindowsPath"
    }

    $drive = $matches[1].ToLowerInvariant()
    $tail = $matches[2] -replace '\\', '/'
    "/mnt/$drive/$tail"
}

function Invoke-Wsl {
    param([string[]]$Arguments)

    & wsl.exe @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "WSL command failed: wsl.exe $($Arguments -join ' ')"
    }
}

$manifestWsl = Convert-ToWslPath $manifestWin
$linuxScriptWsl = Convert-ToWslPath $linuxScriptWin

if ($Clean) {
    Write-Host "Running cargo clean in WSL..."
    Invoke-Wsl @("cargo", "clean", "--manifest-path", $manifestWsl)
}

$mode = if ($DebugOnly) { "debug" } else { "release" }
$skipSmoke = if ($SkipCliSmoke) { "1" } else { "0" }

Write-Host "Running Linux build via WSL ($mode)..."
Invoke-Wsl @("bash", $linuxScriptWsl, $Command, $mode, $skipSmoke, $manifestWsl)
