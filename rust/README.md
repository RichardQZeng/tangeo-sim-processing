# Rust Geometry Simplification Rewrite

This directory contains the Rust rewrite of geometry simplification algorithms from the Python implementation (`core/simplify.py` and `core/reduce_bend.py`).

The Rust implementation provides topology-aware simplification workflows for GeoPackage layers while preserving key topological constraints (simplicity, intersections, and sidedness checks).

## Current scope

- Core simplify engine implemented in `src/simplify/`
- Reduce-bend engine implemented in `src/reduce_bend/`
- Constraint validation in `src/constraints.rs`
- GeoPackage I/O in `src/io.rs`
- CLI entrypoint in `src/main.rs`
- Integration tests in `tests/integration_tests.rs` and `tests/integration_tests_reduce_bend.rs`

## CLI usage

```bash
# Backward-compatible simplify mode
dp-simplify --input input.gpkg --output output.gpkg --tolerance 5.0 [--layer LAYER_NAME] [--validate-structure]

# Explicit simplify subcommand
dp-simplify simplify --input input.gpkg --output output.gpkg --tolerance 5.0 [--layer LAYER_NAME] [--validate-structure]

# Reduce-bend subcommand
dp-simplify reduce-bend --input input.gpkg --output output.gpkg --diameter 100.0 \
  [--layer LAYER_NAME] [--smooth-line] [--del-outer] [--del-inner] [--validate-structure]
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
  - `-SkipCliSmoke` to skip CLI smoke checks
  - default behavior runs both debug and release

- For `build` and `test`, the script also runs CLI smoke checks for:
  - `dp-simplify --help`
  - `dp-simplify simplify --help`
  - `dp-simplify reduce-bend --help`

Example:

```powershell
cd d:\BERATools\tangeo-sim-processing\rust
conda activate data
.\scripts\build-win.ps1 -Command test -Clean
```

## Notes

- Migration status and parity checklist: see `MIGRATION_PLAN.md`
- Reduce-bend test-data generation: `generate_test_reduce_bend_gpkg.py`
- Windows build/setup details: see `BUILD_WINDOWS.md`
