# Windows Build Setup

## Prerequisites

- **Rust** stable with MSVC toolchain (`x86_64-pc-windows-msvc`)
- **conda** (Miniconda or Anaconda)
- **QGIS** Python environment (for generating test GPKG files only)

## 1. Install GDAL + GEOS via conda-forge

conda-forge ships MSVC-compatible `.lib` files on Windows, which is what the Rust MSVC toolchain needs.

```
conda activate data
conda install -c conda-forge "libgdal=3.9.3" geos libclang
```

`libclang` is needed because the `gdal` crate uses `bindgen` to generate Rust bindings from GDAL's C headers at build time (conda-forge ships newer GDAL versions than the crate has pre-built bindings for).

`pkg-config` is intentionally not required in this workflow. The scripts force `GDAL_NO_PKG_CONFIG=1` and `GEOS_NO_PKG_CONFIG=1` to avoid GNU/MinGW metadata leaking into MSVC builds.

## 2. Configure an MSVC-safe build environment

Use the provided script in every terminal session before running `cargo`:

```
conda activate data
.\scripts\env-win.ps1
```

From `rust\`, the equivalent manual variables are:

```
set GDAL_HOME=%CONDA_PREFIX%\Library
set GDAL_LIB_DIR=%CONDA_PREFIX%\Library\lib
set GDAL_INCLUDE_DIR=%CONDA_PREFIX%\Library\include
set GDAL_VERSION=3.9.3
set GDAL_NO_PKG_CONFIG=1

set GEOS_LIB_DIR=%CONDA_PREFIX%\Library\lib
set GEOS_INCLUDE_DIR=%CONDA_PREFIX%\Library\include
set GEOS_VERSION=3.13.1
set GEOS_NO_PKG_CONFIG=1

set PATH=%CONDA_PREFIX%\Library\bin;%PATH%
```

`env-win.ps1` also creates `gdal_i.lib` from `gdal.lib` when needed, because `gdal-sys` on `windows-msvc` expects `gdal_i.lib`.

## 3. Verify the installation

These commands should show matching files:

```
dir %CONDA_PREFIX%\Library\lib\gdal*.lib
dir %CONDA_PREFIX%\Library\lib\gdal_i.lib
dir %CONDA_PREFIX%\Library\lib\geos*.lib
dir %CONDA_PREFIX%\Library\include\gdal.h
dir %CONDA_PREFIX%\Library\include\geos_c.h
```

## 4. Build and check

All `cargo` commands must be run from the `rust/` subdirectory (where `Cargo.toml` lives).

### Preferred (wrapper script)

By default, `build-win.ps1` runs **both debug and release** for the selected command.

```
cd d:\BERATools\tangeo-sim-processing\rust
.\scripts\build-win.ps1 -Command check -Clean
.\scripts\build-win.ps1 -Command build
.\scripts\build-win.ps1 -Command test
```

Optional profile switches:

```
.\scripts\build-win.ps1 -Command build -DebugOnly
.\scripts\build-win.ps1 -Command build -Release
```

### Direct cargo (after environment bootstrap)

```
cd d:\BERATools\tangeo-sim-processing\rust
.\scripts\env-win.ps1
cargo clean --manifest-path Cargo.toml
cargo check --manifest-path Cargo.toml
cargo build
cargo build --release
cargo test
cargo test --release
```

## 5. Generate test GPKG files

The `generate_test_gpkg.py` script requires QGIS Python (`qgis.core`). Run it from a QGIS Python console or OSGeo4W shell:

```
python generate_test_gpkg.py
```

This creates `tests/data/test_XX_input.gpkg` and `tests/data/test_XX_expected.gpkg` for all 29 test cases.

## 6. Run tests

After generating test GPKGs:

```
cargo test
```

## Notes

- **Why not MINGW64?** The Rust toolchain is MSVC (`x86_64-pc-windows-msvc`). MINGW64's `.a` libraries won't link with it. Switching to `x86_64-pc-windows-gnu` would work but is more disruptive. conda-forge is the simplest path.
- **`stdc++.lib` link errors** (`LNK1181`) indicate GNU/MinGW-style dependency leakage into an MSVC build (often from pkg-config metadata). Always run with `GDAL_NO_PKG_CONFIG=1` and `GEOS_NO_PKG_CONFIG=1` in this setup.
- **Runtime DLLs**: The `PATH` must include `%CONDA_PREFIX%\Library\bin` at runtime so `geos_c.dll` and `gdal*.dll` can be found.
- **CI builds**: Replicate the conda-forge setup or use vcpkg as an alternative for CI environments.
