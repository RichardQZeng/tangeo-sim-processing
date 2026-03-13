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
geo-simplify --input input.gpkg --output output.gpkg --tolerance 5.0 [--in-layer INPUT_LAYER] [--out-layer OUTPUT_LAYER] [--validate-structure]

# Explicit simplify subcommand
geo-simplify simplify --input input.gpkg --output output.gpkg --tolerance 5.0 [--in-layer INPUT_LAYER] [--out-layer OUTPUT_LAYER] [--validate-structure]

# Reduce-bend subcommand
geo-simplify reduce-bend --input input.gpkg --output output.gpkg --diameter 100.0 \
  [--in-layer INPUT_LAYER] [--out-layer OUTPUT_LAYER] [--smooth-line] [--del-outer] [--del-inner] [--validate-structure]
```

## Build scripts (`rust/scripts`)

The `rust/scripts` folder contains helper scripts for both Windows-native and Linux builds:

- `build-win.ps1`: wrapper to run Cargo commands (`check`, `build`, `test`) with Windows MSVC environment setup.
- `env-win.ps1`: configures GDAL/GEOS environment variables from `CONDA_PREFIX` for `windows-msvc` builds.
- `build-linux.ps1`: Windows PowerShell wrapper that forwards build/test/check to WSL (Linux x64 output).
- `build-linux.sh`: Linux build runner used by `build-linux.ps1` inside WSL.

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
  - default behavior runs release only

- For `build` and `test`, the script also runs CLI smoke checks for:
  - `geo-simplify --help`
  - `geo-simplify simplify --help`
  - `geo-simplify reduce-bend --help`
- For `build` and `test`, the script copies the Windows binary to `rust/dist`:
  - release: `rust/dist/geo-simplify-win-x64.exe`
  - debug-only: `rust/dist/geo-simplify-win-x64-debug.exe`

Example:

```powershell
cd d:\BERATools\tangeo-sim-processing\rust
conda activate data
.\scripts\build-win.ps1 -Command test -Clean
```

### `build-linux.ps1` + `build-linux.sh` (cross-build on Windows via WSL)

Purpose:

- Builds Linux x64 binaries from a Windows host by running Cargo in WSL.
- Preserves the same command style as `build-win.ps1` (`check|build|test`, `-Clean`, `-Release`, `-DebugOnly`, `-SkipCliSmoke`).
- Runs the same CLI smoke checks (`--help`, `simplify --help`, `reduce-bend --help`) for `build` and `test`.
- Copies the built Linux binary to `rust/dist/geo-simplify-linux-x64` after successful `build` or `test`.

WSL prerequisites (Ubuntu):

```bash
sudo apt update
sudo apt install -y build-essential pkg-config libgdal-dev libgeos-dev
```

Example (from Windows PowerShell):

```powershell
cd d:\BERATools\tangeo-sim-processing\rust
.\scripts\build-linux.ps1 -Command build -Release
.\scripts\build-linux.ps1 -Command test -Clean
```

Output binary path (Linux ELF):

- build artifact: `rust/target/release/geo-simplify`
- copied distribution artifact: `rust/dist/geo-simplify-linux-x64`

### Verification

Run these commands to validate the CLI and tests locally:

```powershell
cd d:\BERATools\tangeo-sim-processing\rust
conda activate data
.\scripts\build-win.ps1 -Command check
.\scripts\build-win.ps1 -Command test
cargo run -- --help
cargo run -- simplify --help
cargo run -- reduce-bend --help
.\scripts\build-linux.ps1 -Command check -Release
.\scripts\build-linux.ps1 -Command test -Release
```

## Notes

- Reduce-bend preserves degenerate inputs (for example, zero-length lines) as simplest geometries and skips further bend reduction on them.
- Reduce-bend test-data generation: `generate_test_reduce_bend_gpkg.py`
