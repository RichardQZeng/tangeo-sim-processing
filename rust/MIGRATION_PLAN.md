# Rust Migration Plan Status: `core/simplify.py` → Topology-Aware Douglas-Peucker in Rust

## 1. Executive Summary

### Status: **Core migration implemented; parity hardening/documentation follow-up remains**

The Rust implementation is no longer a greenfield migration. Core modules, CLI wiring, and integration tests are in place and functional in `rust/src` and `rust/tests`.

This document now serves as an **as-built architecture + parity checklist** for remaining alignment with Python behavior from `core/simplify.py`.

---

## 2. Current Implementation Snapshot (As Built)

### 2.1 Implemented Modules

The following modules are implemented and wired through `lib.rs`:

- `epsilon.rs`
- `geometry.rs`
- `io.rs`
- `spatial_index.rs`
- `constraints.rs`
- `simplify.rs`
- `main.rs`

### 2.2 Current CLI

`src/main.rs` exposes:

```bash
dp_simplify --input input.gpkg --output output.gpkg --tolerance 5.0 [--layer LAYER_NAME] [--validate-structure]
```

### 2.3 Current Test Harness

`tests/integration_tests.rs` currently:

1. Loads `tests/data/test_manifest.json`
2. Runs all 29 generated GPKG cases
3. Compares output vs expected with:
   - GEOS topological equality (`equals`)
   - coordinate-level equality with epsilon (`1e-10`)

This is already aligned with the intended two-level assertion strategy.

---

## 3. Dependency Baseline (Matches Current `Cargo.toml`)

```toml
[dependencies]
anyhow = "1"
clap = { version = "4", features = ["derive"] }
gdal = "0.17"
geos = "10"
rstar = "0.12"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"

[dev-dependencies]
serde_json = "1"
```

---

## 4. Python-to-Rust Mapping (Current Behavior)

Reference source: `core/simplify.py`

### 4.1 Core Simplification Loop

Python `Simplify._simplify_lines` / `process_line` maps to Rust `SimplifyEngine::run_with_progress` / `process_line` with iterative pass processing and stack-based splitting.

### 4.2 Farthest-Point Distance (Hot Path)

Python uses QGIS geometry engine distance in `find_farthest_point`.

Rust uses pure math:

- `find_farthest_point`
- `point_to_segment_dist`

This is intentional for performance.

### 4.3 Constraint Validation

Python `validate_constraints` checks:

1. Simplicity
2. Intersection with others
3. Sidedness (close line → unary union → polygonize → `isSimple()` guard → contains)

Rust currently mirrors this flow at module level (`constraints.rs`) with GEOS APIs:

- `validate_simplicity`
- `validate_intersection`
- `validate_sidedness`

Implementation note: current Rust code uses GEOS predicates (`crosses`, `contains`) and polygonize guard rather than explicit DE-9IM pattern string parsing.

### 4.4 Spatial Index & Segment Mutation

Python `GsCollection` behavior is represented by Rust `GsCollection` (`spatial_index.rs`) with:

- R-tree query for intersecting candidates
- delete-and-reinsert segment update pattern on vertex removal
- point geometry indexing support (for blockers)
- optional integrity validation (`validate_integrity`)

---

## 5. I/O and Feature Fidelity Status

### Implemented

- Read GPKG via GDAL (`read_gpkg`)
- Carry attributes, geometry, stable ordering info, optional FID in memory
- Write output GPKG with schema name/type and geometry rebuilt from internal representation
- Preserve layer CRS WKT in schema object and apply on output layer creation

### Follow-up Items

1. **FID write-back parity**
   - FID is read and preserved in `FeatureRecord` but not explicitly restored on write.

2. **Field width/precision write-back parity**
   - Width/precision are captured in `LayerSchema`, but field creation currently applies `(name, type)` only.

3. **Broader field-type coverage**
   - Current mapping focuses on integer/int64/real/string/null; additional OGR field variants may need explicit handling for strict schema parity.

---

## 6. Current File Layout

```text
rust/
├── BUILD_WINDOWS.md
├── Cargo.toml
├── generate_test_gpkg.py
├── MIGRATION_PLAN.md
├── src/
│   ├── main.rs
│   ├── lib.rs
│   ├── epsilon.rs
│   ├── geometry.rs
│   ├── io.rs
│   ├── spatial_index.rs
│   ├── constraints.rs
│   └── simplify.rs
└── tests/
    ├── integration_tests.rs
    └── data/
        ├── test_01_input.gpkg
        ├── test_01_expected.gpkg
        ├── ...
        ├── test_29_input.gpkg
        ├── test_29_expected.gpkg
        └── test_manifest.json
```

---

## 7. Remaining Work (Parity Hardening)

### High Priority

1. Ensure output FID behavior is explicitly defined and implemented.
2. Reapply width/precision (and any required field metadata) when creating output layer fields.
3. Confirm whether exact DE-9IM string-pattern parity is required, or if predicate-based equivalence is accepted.

### Medium Priority

4. Add explicit tests for attribute/schema roundtrip behavior (not just geometry equivalence).
5. Add targeted regression tests for edge cases around sidedness/polygonize behavior across GEOS versions.

### Optional

6. Add benchmark harness for hot path (`find_farthest_point`) and end-to-end pass counts.

---

## 8. Performance Positioning

Rust implementation avoids the Python/Shapely overhead observed previously:

- pure Rust arithmetic in the hottest distance loop
- native `rstar` indexing
- GEOS calls reserved for topology predicates and polygonize logic

This keeps expensive FFI usage on lower-frequency operations while preserving topology checks.

---

## 9. Open Decisions

1. Should this remain CLI-only, or later be wrapped for QGIS plugin usage?
2. Should CI standardize on conda-forge GDAL/GEOS (per `BUILD_WINDOWS.md`) or adopt vcpkg for portability?
3. Is strict schema/FID roundtrip parity mandatory for v1 acceptance, or can it be staged after geometry/topology parity?
