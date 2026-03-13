param(
    [ValidateSet("check", "build", "test")]
    [string]$Command = "test",

    [switch]$Clean,

    [switch]$Release,

    [switch]$DebugOnly,

    [switch]$SkipCliSmoke
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

function Invoke-CliSmoke {
    param(
        [switch]$UseRelease
    )

    $mode = if ($UseRelease) { "release" } else { "debug" }
    Write-Host "Running CLI smoke checks ($mode)..."

    $args = @("run", "--manifest-path", $manifest)
    if ($UseRelease) {
        $args += "--release"
    }

    $helpArgs = $args + @("--", "--help")
    & cargo @helpArgs | Out-Null

    $simplifyArgs = $args + @("--", "simplify", "--help")
    & cargo @simplifyArgs | Out-Null

    $reduceBendArgs = $args + @("--", "reduce-bend", "--help")
    & cargo @reduceBendArgs | Out-Null
}

function Copy-WindowsArtifact {
    param(
        [switch]$UseRelease
    )

    $profile = if ($UseRelease) { "release" } else { "debug" }
    $source = Join-Path $repoRoot "rust\target\$profile\geo-simplify.exe"
    $distDir = Join-Path $repoRoot "rust\dist"
    $destName = if ($UseRelease) { "geo-simplify-win-x64.exe" } else { "geo-simplify-win-x64-debug.exe" }
    $destination = Join-Path $distDir $destName

    if (-not (Test-Path $source)) {
        throw "Built binary not found at $source"
    }

    if (-not (Test-Path $distDir)) {
        New-Item -ItemType Directory -Path $distDir | Out-Null
    }

    Copy-Item -Path $source -Destination $destination -Force
    Write-Host "Copied Windows x64 binary to: $destination"
}

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
    Write-Host "Default mode: running release only"
    cargo $Command --manifest-path $manifest --release
}

if (-not $SkipCliSmoke -and ($Command -eq "build" -or $Command -eq "test")) {
    if ($Release) {
        Invoke-CliSmoke -UseRelease
    }
    elseif ($DebugOnly) {
        Invoke-CliSmoke
    }
    else {
        Invoke-CliSmoke
        Invoke-CliSmoke -UseRelease
    }
}

if ($Command -eq "build" -or $Command -eq "test") {
    if ($Release) {
        Copy-WindowsArtifact -UseRelease
    }
    elseif ($DebugOnly) {
        Copy-WindowsArtifact
    }
    else {
        Copy-WindowsArtifact -UseRelease
    }
}
