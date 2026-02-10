$ErrorActionPreference = 'Stop'

if (-not $env:CONDA_PREFIX) {
    throw "CONDA_PREFIX is not set. Run 'conda activate data' first."
}

$repoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$manifest = Join-Path $repoRoot "rust\Cargo.toml"
$lib = Join-Path $env:CONDA_PREFIX "Library\lib"
$include = Join-Path $env:CONDA_PREFIX "Library\include"
$gdalHome = Join-Path $env:CONDA_PREFIX "Library"
$bin = Join-Path $env:CONDA_PREFIX "Library\bin"

if (-not (Test-Path $manifest)) {
    throw "Cargo.toml not found at $manifest"
}

if (-not (Test-Path (Join-Path $lib "gdal.lib"))) {
    throw "Missing gdal.lib in $lib"
}

if (-not (Test-Path (Join-Path $lib "geos_c.lib"))) {
    throw "Missing geos_c.lib in $lib"
}

if (-not (Test-Path (Join-Path $include "gdal.h"))) {
    throw "Missing gdal.h in $include"
}

if (-not (Test-Path (Join-Path $include "geos_c.h"))) {
    throw "Missing geos_c.h in $include"
}

# Clear problematic overrides if present.
Remove-Item Env:GDAL_DYNAMIC -ErrorAction SilentlyContinue
Remove-Item Env:GDAL_STATIC -ErrorAction SilentlyContinue
Remove-Item Env:GEOS_DYNAMIC -ErrorAction SilentlyContinue
Remove-Item Env:GEOS_STATIC -ErrorAction SilentlyContinue

# Avoid pkg-config metadata leaking GNU-style deps into MSVC builds.
Remove-Item Env:PKG_CONFIG -ErrorAction SilentlyContinue
Remove-Item Env:PKG_CONFIG_PATH -ErrorAction SilentlyContinue
Remove-Item Env:PKG_CONFIG_LIBDIR -ErrorAction SilentlyContinue
Remove-Item Env:PKG_CONFIG_SYSROOT_DIR -ErrorAction SilentlyContinue
Remove-Item Env:PKG_CONFIG_ALLOW_SYSTEM_LIBS -ErrorAction SilentlyContinue
Remove-Item Env:PKG_CONFIG_ALLOW_SYSTEM_CFLAGS -ErrorAction SilentlyContinue

# GDAL / GEOS setup for windows-msvc.
$env:GDAL_HOME = $gdalHome
$env:GDAL_LIB_DIR = $lib
$env:GDAL_INCLUDE_DIR = $include
$env:GDAL_VERSION = "3.9.3"
$env:GDAL_NO_PKG_CONFIG = "1"

$env:GEOS_LIB_DIR = $lib
$env:GEOS_INCLUDE_DIR = $include
$env:GEOS_VERSION = "3.13.1"
$env:GEOS_NO_PKG_CONFIG = "1"

if ($env:PATH -notlike "$bin*") {
    $env:PATH = "$bin;$env:PATH"
}

# gdal-sys expects gdal_i.lib on windows-msvc.
if (-not (Test-Path (Join-Path $lib "gdal_i.lib"))) {
    Copy-Item (Join-Path $lib "gdal.lib") (Join-Path $lib "gdal_i.lib")
}

Write-Host "Configured MSVC GDAL/GEOS environment from CONDA_PREFIX: $env:CONDA_PREFIX"
Write-Host "GDAL_LIB_DIR=$env:GDAL_LIB_DIR"
Write-Host "GEOS_LIB_DIR=$env:GEOS_LIB_DIR"
