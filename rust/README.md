# Rust Simplify Rewrite

This directory contains the Rust rewrite of the **simplify** algorithm from the Python implementation (`core/simplify.py`).

The Rust implementation provides a **topology-aware Douglas-Peucker** simplification workflow for GeoPackage line layers. It is designed to keep simplification fast while preserving key topological constraints (simplicity, intersections, and sidedness checks).

## Current scope

- Core simplify engine implemented in `src/simplify.rs`
- Constraint validation in `src/constraints.rs`
- GeoPackage I/O in `src/io.rs`
- CLI entrypoint in `src/main.rs`
- Integration tests in `tests/integration_tests.rs`

## CLI usage

```bash
dp_simplify --input input.gpkg --output output.gpkg --tolerance 5.0 [--layer LAYER_NAME] [--validate-structure]
```

## Build scripts (`rust/scripts`)

The `rust/scripts` folder contains two helper scripts for Windows MSVC builds:

- `build-win.ps1`: wrapper to run Cargo commands (`check`, `build`, `test`) with consistent environment setup.
- `env-win.ps1`: configures GDAL/GEOS environment variables from `CONDA_PREFIX` for `windows-msvc` builds.

### `env-win.ps1`

Purpose:

- Validates required GDAL/GEOS headers and libraries in `%CONDA_PREFIX%\Library`.
- Clears potentially conflicting GDAL/GEOS and `pkg-config` environment overrides.
- Sets required variables such as `GDAL_HOME`, `GDAL_LIB_DIR`, `GEOS_LIB_DIR`, `GDAL_NO_PKG_CONFIG=1`, and `GEOS_NO_PKG_CONFIG=1`.
- Prepends `%CONDA_PREFIX%\Library\bin` to `PATH` for runtime DLL discovery.
- Ensures `gdal_i.lib` exists (copies from `gdal.lib` if needed) for `gdal-sys` on MSVC.

### `build-win.ps1`

Purpose:

- Sources `env-win.ps1` first.
- Runs Cargo with the Rust manifest at `rust/Cargo.toml`.
- Supports:
  - `-Command check|build|test` (default: `test`)
  - `-Clean` to run `cargo clean` first
  - `-Release` to run release only
  - `-DebugOnly` to run debug only
  - default behavior runs both debug and release

Example:

```powershell
cd d:\BERATools\tangeo-sim-processing\rust
conda activate data
.\scripts\build-win.ps1 -Command test -Clean
```

## Notes

- Migration status and parity checklist: see `MIGRATION_PLAN.md`
- Windows build/setup details: see `BUILD_WINDOWS.md`