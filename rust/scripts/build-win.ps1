$ErrorActionPreference = 'Stop'

if (-not $env:CONDA_PREFIX) {
    throw "CONDA_PREFIX is not set. Run 'conda activate data' first."
}

$repoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$manifest = Join-Path $repoRoot "rust\Cargo.toml"
$lib = Join-Path $env:CONDA_PREFIX "Library\lib"
$include = Join-Path $env:CONDA_PREFIX "Library\include"
$gdalHome = Join-Path $env:CONDA_PREFIX "Library"

if (-not (Test-Path $manifest)) {
    throw "Cargo.toml not found at $manifest"
}

if (-not (Test-Path (Join-Path $lib "gdal.lib"))) {
    throw "Missing gdal.lib in $lib"
}

if (-not (Test-Path (Join-Path $lib "geos_c.lib"))) {
    throw "Missing geos_c.lib in $lib"
}

# Clear problematic overrides if present
Remove-Item Env:GDAL_DYNAMIC -ErrorAction SilentlyContinue
Remove-Item Env:GDAL_STATIC -ErrorAction SilentlyContinue
Remove-Item Env:GDAL_NO_PKG_CONFIG -ErrorAction SilentlyContinue

# GDAL / GEOS setup
$env:GDAL_HOME = $gdalHome
$env:GDAL_LIB_DIR = $lib
$env:GDAL_INCLUDE_DIR = $include

# Do NOT set GDAL_NO_PKG_CONFIG for gdal-sys v0.10.0.
# On Windows/MSVC this can still cause gdal-sys to invoke pkg-config for
# metadata/version probing, and if GDAL_NO_PKG_CONFIG=1 it aborts the build.

$env:GEOS_LIB_DIR = $lib
$env:GEOS_INCLUDE_DIR = $include
$env:GEOS_NO_PKG_CONFIG = "1"
$env:GEOS_VERSION = "3.13.1"

$env:PATH = "$(Join-Path $env:CONDA_PREFIX 'Library\bin');$env:PATH"

# Compatibility aliases for crates/build scripts
if (-not (Test-Path (Join-Path $lib "gdal_i.lib"))) {
    Copy-Item (Join-Path $lib "gdal.lib") (Join-Path $lib "gdal_i.lib")
}

if (-not (Test-Path (Join-Path $lib "gdal.dll.lib"))) {
    Copy-Item (Join-Path $lib "gdal.lib") (Join-Path $lib "gdal.dll.lib")
}

Write-Host "Using CONDA_PREFIX: $env:CONDA_PREFIX"
Write-Host "Running cargo clean + cargo test..."

cargo clean --manifest-path $manifest
cargo test --manifest-path $manifest
