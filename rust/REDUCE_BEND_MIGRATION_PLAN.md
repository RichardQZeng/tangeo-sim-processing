# Rust Migration Status: `core/reduce_bend.py` → Topology-Aware Bend Reduction in Rust

## 1. Executive Summary

### Status: **Core reduce-bend migration implemented; parity hardening and test-policy follow-up remain**

This document serves as an **as-built architecture + parity checklist** for `core/reduce_bend.py` (and dependencies from `geo_sim_util.py`) in Rust. The existing crate `dp-simplify` reuses significant infrastructure from the Douglas-Peucker migration. The bend-reduction algorithm is substantially more complex than Douglas-Peucker and introduces additional concepts (bend detection, adjusted-area flagging, alternate bends, line smoothing, polygon/hole deletion).

---

## 2. Current Implementation Snapshot (As Built)

### 2.0 Reuse Assessment: Existing Rust Modules

### 2.1 Fully Reusable (no changes needed)

| Module | What it provides | Used by reduce_bend? |
|---|---|---|
| `epsilon.rs` | `Epsilon::from_features` — computes `zero_relative`, `zero_absolute`, `zero_angle` | ✅ Same epsilon logic |
| `geometry.rs` | `Coord`, `FeatureRecord`, `SimpleGeometry`, `FieldValue`, `GeomType`, `RbGeom`, `GsFeature`, `GeoJsonGeometry` | ✅ Same coordinate/feature/geometry model. `RbGeom` is unchanged — bend state is engine-owned (see §4.1). |
| `io.rs` | `read_gpkg`, `write_gpkg`, `LayerSchema`, `SchemaField` | ✅ Same I/O path |

### 2.2 Reusable With Extensions Needed

| Module | Current state | Extension needed for reduce_bend |
|---|---|---|
| `spatial_index.rs` → `GsCollection` | Has `add_features`, `get_segment_intersect`, `delete_vertex`, `validate_integrity`, `add_vertex`. | `add_vertex` implemented in Phase 1 with runtime invariants. For smoothing-heavy phases, switch endpoint/ring closure checks to epsilon-based comparisons to avoid false negatives from floating-point drift. |
| `constraints.rs` | `validate_simplicity`, `validate_intersection`, `validate_sidedness`, `validate_sidedness_from_coords`. | Refactor completed in Phase 1. Generic `validate_sidedness(&Geometry)` path is available for reduce-bend and smoothing; DP simplify now uses `validate_sidedness_from_coords` preserving `is_simple` guard semantics. |

**Design decision — polygon/hole deletion:** Pre-filter at the `FeatureRecord` level (remove inner rings from `SimpleGeometry::Polygon` or remove entire records) *before* calling `GsFeature::from_records`. This avoids post-hoc ring-ID deletion issues in `GsFeature::Polygon.rb_geom_ids` and `rebuild_record`. No changes needed to `geometry.rs` → `GsFeature`.

**Design decision — bend state ownership:** Bend data (`Vec<Bend>`) is **not** stored on `RbGeom`. Instead, the engine owns a `HashMap<usize, Vec<Bend>>` keyed by `rb_geom.id`. This keeps `RbGeom` as clean shared infrastructure used by both DP simplify and bend reduction, avoiding algorithm-specific coupling. An `rb_geom_id → index` lookup map (`HashMap<usize, usize>`) is also maintained by the engine for O(1) lookups during bend processing and constraint validation (it does **not** affect `GsFeature::rebuild_record`, which uses `iter().find()` and is only called once at output time).

### 2.3 New Reduce-Bend Code (Implemented)

| Concept | Python source | Notes |
|---|---|---|
| `Bend` struct | `geo_sim_util.py:Bend` | area, perimeter, adj_area, to_reduce flag, i/j indices, geometry properties |
| `Inflexion` struct | `reduce_bend.py:Inflexion` | Simple start/end dataclass |
| `BendReduced` struct + smoothing logic | `reduce_bend.py:BendReduced` | Translation/rotation smoothing, polygon repair via unary union + polygonize |
| Bend detection | `ReduceBend.detect_bends`, `get_angles`, `delete_co_linear` | Angle computation, inflexion detection, co-linear removal |
| Bend flagging | `ReduceBend.flag_bend_to_reduce` | Sorted adj_area, adjacent-bend exclusion |
| Alternate bend search | `ReduceBend.find_alternate_bends`, `validate_alternate_bend` | Fallback when simplicity fails |
| Polygon/hole deletion | `ReduceBend.del_outer_inner_ring` | Pre-processing step |
| Bend processing loop | `ReduceBend._manage_reduce_bend`, `process_bends` | Iterative multi-pass with closed-line special handling |
| Smooth constraint validation | `ReduceBend.validate_constraints_smooth` | Like regular constraints but against smooth polygon |
| `RbResults` (reduce_bend variant) | `reduce_bend.py:RbResults` | Additional fields: `nbr_bend_reduced`, `nbr_bend_detected`, `nbr_hole_del`, `nbr_pol_del`, `nbr_line_smooth` |

---

## 3. Current File Layout (As Built)

Reduce-bend is implemented in a `reduce_bend/` submodule inside the existing crate. Shared modules remain in `src/`.

```text
rust/src/
├── main.rs                    # Legacy simplify mode + `simplify`/`reduce-bend` subcommands
├── lib.rs                     # Exposes simplify and reduce_bend modules
├── epsilon.rs                 # Shared epsilon logic
├── geometry.rs                # Shared coordinate/feature/geometry model
├── io.rs                      # Shared GPKG I/O
├── spatial_index.rs           # Includes delete_vertex/add_vertex
├── constraints.rs             # Generic sidedness + coords helper for DP
├── simplify/
│   ├── mod.rs
│   ├── dp.rs
│   └── engine.rs
└── reduce_bend/
    ├── mod.rs                 # ReduceBendEngine, params/stats/output, pass loop
    ├── bend.rs                # Bend struct (area, perimeter, adj_area, to_reduce, i, j)
    ├── detection.rs           # get_angles, delete_co_linear, detect_bends, Inflexion
    ├── flagging.rs            # flag_bend_to_reduce, min_adj_area, adjacent-bend exclusion
    ├── alternate.rs           # find_alternate_bends, validate_alternate_bend
    ├── smoothing.rs           # BendReduced, smoothing/repair logic
    └── pre_process.rs         # pre_filter_records, remove_duplicate_nodes
```

---

## 4. Python-to-Rust Mapping

### 4.1 Core Engine — `ReduceBend` → `ReduceBendEngine`

Python `ReduceBend.__init__` / `reduce_bends` → Rust `ReduceBendEngine::new` / `run_with_progress`

Parameters to carry:

| Python param | Rust field | Type |
|---|---|---|
| `diameter_tol` | `diameter_tol` | `f64` |
| `smooth_line` | `smooth_line` | `bool` |
| `flag_del_outer` | `flag_del_outer` | `bool` |
| `flag_del_inner` | `flag_del_inner` | `bool` |
| `validate_structure` | `validate_structure` | `bool` |

**Engine-owned state** (not on shared types):

| Field | Type | Purpose |
|---|---|---|
| `bend_map` | `HashMap<usize, Vec<Bend>>` | Bend state per `rb_geom.id`; replaces Python's `rb_geom.bends` attribute |
| `rb_geom_index` | `HashMap<usize, usize>` | Maps `rb_geom.id` → position in `Vec<RbGeom>` for O(1) lookups within the engine (bend processing, constraint validation). Note: this does **not** accelerate `GsFeature::rebuild_record` which still uses `iter().find()` — that API is only called once at output time, so the O(n) cost is acceptable there. |
| `bends_reduced` | `Vec<BendReduced>` | Accumulated reduced bends for post-processing smoothing |

### 4.2 Pre-Processing — `pre_reduction_process` / `del_outer_inner_ring`

Python flow:
1. Optionally delete small outer polygons or inner holes (by adjusted area vs `min_adj_area`)
2. Collect `RbGeom` list from features
3. Remove duplicate nodes (`removeDuplicateNodes`)

Rust: Implemented in `pre_process.rs`. **Polygon/hole deletion operates on `Vec<FeatureRecord>` before `GsFeature::from_records` is called:**
- To delete a small outer polygon: remove the entire `FeatureRecord` from the vector.
- To delete a small inner hole: remove the corresponding ring from `SimpleGeometry::Polygon { outer, inners }` by filtering `inners`.
- This avoids ring-ID consistency issues in `GsFeature::Polygon.rb_geom_ids` and `rebuild_record`.
- Duplicate-node removal is pure coordinate math on `FeatureRecord.geometry` coords (consecutive points within epsilon).
- **Post-removal validity guards** (apply after both hole deletion and duplicate-node removal):
  - Closed rings: verify `coords.first() == coords.last()` still holds; re-close if broken.
  - Minimum vertex count: closed rings must retain ≥ 4 coordinates (triangle + closure point); open lines must retain ≥ 2. For parity with Python behavior, do **not** drop degenerate features by default at this stage. Keep them in `Vec<FeatureRecord>` and let `GsFeature::from_records` + `setup_rb_geom_flags` classify them as simplest/non-processable downstream.

### 4.3 Angle Computation — `get_angles`

Python uses `QgsGeometryUtils.angleBetweenThreePoints` to compute the angle at each vertex, then classifies as `CLOCK_WISE(1)`, `ANTI_CLOCK_WISE(-1)`, or `FLAT_ANGLE(0)`.

Rust: Pure math — `atan2` for angle between three points. No GEOS/GDAL dependency. Implement in `detection.rs`.

### 4.4 Co-linear Vertex Deletion — `delete_co_linear`

Removes vertices with `FLAT_ANGLE`, using existing `GsCollection::delete_vertex`. Adjusts angles list after each deletion.

Rust: Implemented in `detection.rs`. Reuses `GsCollection::delete_vertex`.

### 4.5 Bend Detection — `detect_bends`

Python flow:
1. Walk angle list, find inflexion points (where direction sign flips: CW ↔ ACW)
2. Handle circular (closed) vs open line differences
3. Create `Bend` objects from each inflexion range

Rust: Implemented in `detection.rs`. Returns `Vec<Bend>` stored in the engine's `bend_map` keyed by `rb_geom.id`.

### 4.6 Bend Flagging — `flag_bend_to_reduce`

Python flow:
1. Compute `min_adj_area = 0.75 * π * (diameter_tol / 2)²`
2. Collect bends with `area < min_adj_area`, sort by `adj_area`
3. From smallest: flag `to_reduce = true` unless adjacent bend is already flagged

Rust: Implemented in `flagging.rs`. Pure logic, no geometry dependencies.

### 4.7 Constraint Validation — `validate_constraints`

Same three-step pattern as Douglas-Peucker (simplicity → intersection → sidedness), but with two key differences:

1. **Alternate bend fallback**: If simplicity fails, calls `find_alternate_bends` to search for a smaller sub-bend that doesn't self-intersect
2. **Sidedness uses pre-built bend polygon** (`bend.qgs_geom_bend`) instead of constructing one from old subline coords

Rust: Uses `constraints::validate_simplicity` and `constraints::validate_intersection`. It also uses the refactored `validate_sidedness`, which accepts a generic `&Geometry` (Polygon or MultiPolygon), covering both bend polygon and smooth polygon inputs.

### 4.8 Alternate Bend Search — `find_alternate_bends` / `validate_alternate_bend`

Python flow:
1. From the failing bend's `(i, j)` range, enumerate all sub-ranges `(i', j')` where `j' < j` and `i' > i`
2. Create `Bend` objects for each, sort by area descending (bigger = better candidate)
3. Validate simplicity for each until one passes (Python uses DE-9IM pattern; Rust uses `crosses()` + `interior_boundary_intersects()` — same semantic check, see DP MIGRATION_PLAN §4.3)
4. Replace the original bend with the valid alternate

Rust: Implemented in `alternate.rs`. Reuses `constraints::validate_simplicity`.

### 4.9 Bend Processing — `process_bends` / `_manage_reduce_bend`

Python flow:
- Outer `while True` loop (multi-pass until no bends reduced)
- Per-pass: iterate `rb_geoms`, recalculate bends if `bends is None`, call `flag_bend_to_reduce`, process each bend
- Per-geometry: for closed lines, process inner bends first (reversed), then first/last bend only if no others reduced
- Per-bend: validate constraints → record `BendReduced` → `delete_vertex(bend.i+1, bend.j-1)`
- If any bend reduced in geometry, set `bends = None` to force recalculation next pass

Rust: Implemented in `mod.rs` (`ReduceBendEngine`). It reuses `GsCollection::delete_vertex`. Bend recalculation state is managed via `bend_map.remove(rb_geom_id)` to trigger re-detection on the next pass.

**Borrow-checker strategy:** The main loop needs simultaneous access to `bend_map` (read bend state), `rb_geoms` (mutate coords via `delete_vertex`), and `collection` (mutate spatial index). Since all three are fields of `ReduceBendEngine`, calling `&mut self` methods while borrowing `self.bend_map` will cause aliasing conflicts. Recommended approach:
- Use index-based iteration (`for i in 0..self.rb_geoms.len()`) instead of iterator-based (`for rb_geom in &mut self.rb_geoms`).
- Extract bend data by value/clone from `bend_map` before the mutable `delete_vertex` call (bends are small structs; cloning `(i, j)` pairs is cheap).
- If needed, temporarily `take()` the bend vec out of the map (`bend_map.remove(id)`) before processing, then re-insert or drop it after.
- For `validate_constraints` (read-only: queries `collection` and `rb_geoms` but does not mutate them), use a free function or method that takes explicit shared references (`&self.collection`, `&self.rb_geoms[idx]`, `&bend`). This avoids borrowing `&self` while a separate `&mut self` is needed later for `delete_vertex`.
- The mutation sequence per bend is: validate (read-only) → if valid, `delete_vertex` (`&mut collection`, `&mut rb_geoms[idx]`). Splitting read and write into separate borrow scopes avoids conflicts.

### 4.10 Line Smoothing — `BendReduced` / `manage_smooth_line`

Python flow (post-processing, after all bend reduction):
1. For each `BendReduced`, check if the base length > `diameter_tol * 2/3`
2. Locate `i` and `j` indices in current geometry (vertex positions may have shifted)
3. Translate bend base to origin, rotate to align with x-axis
4. Classify smoothing case (CASE_1/2/3) based on relative position of adjacent segments and bend centroid
5. Compute smooth angle, place two intermediate points at 1/3 and 2/3 along base
6. Rotate and translate back
7. Repair polygon if self-intersecting (unary union + polygonize)
8. Validate spatial constraints on smooth line
9. If valid, call `GsCollection::add_vertex` to insert smooth points

Rust: Implemented in `smoothing.rs`. It uses **`GsCollection::add_vertex`** from `spatial_index.rs`.

### 4.11 `GsCollection::add_vertex` — Implemented Method

Python `GsCollection.add_vertex(rb_geom, bend_i, bend_j, qgs_geom_new_subline)`:
1. Delete the segment `(bend_i, bend_j)` from the spatial index
2. Extract new points from the new subline (drop first/last which are the existing endpoints)
3. Insert new vertices into the geometry at position `bend_j` (in reverse order)
4. Add new segments to the spatial index

Rust: Implemented in `spatial_index.rs`. It mirrors the Python logic over `rb_geom.coords` and `Coord` vectors.

**Required invariants** (enforce via `Result` errors at runtime, not `debug_assert!` — release builds must not skip these checks since silent index corruption is worse than a visible error):
- `bend_j == bend_i + 1` (endpoints must be adjacent) → return `Err` if violated
- `bend_i < rb_geom.coords.len()` and `bend_j < rb_geom.coords.len()` (endpoints must exist) → return `Err` if out of bounds
- For closed lines: after insertion, `coords.first() == coords.last()` must still hold → return `Err` if ring closure is broken

---

## 5. Dependencies

No new crate dependencies needed. All required functionality is covered by the existing `Cargo.toml`:
- `anyhow`, `thiserror` — error handling
- `geos` — topology predicates, polygonize, unary_union
- `gdal` — I/O
- `rstar` — spatial indexing
- `clap` — CLI
- `serde`, `serde_json` — test manifest

---

## 6. CLI Behavior (As Built)

`main.rs` supports both algorithms with backward compatibility: the existing flat-flag invocation for DP simplify continues to work unchanged.

Current approach: subcommands are available, while legacy flat-flag mode remains the default when no subcommand is provided.

```bash
# Legacy invocation (MUST keep working, no subcommand)
dp-simplify --input in.gpkg --output out.gpkg --tolerance 5.0 [--layer NAME] [--validate-structure]

# Explicit subcommand form (equivalent to legacy)
dp-simplify simplify --input in.gpkg --output out.gpkg --tolerance 5.0 [--layer NAME] [--validate-structure]

# New subcommand
dp-simplify reduce-bend --input in.gpkg --output out.gpkg --diameter 100.0 \
    [--layer NAME] [--smooth-line] [--del-outer] [--del-inner] [--validate-structure]
```

Concrete `clap` struct shape for mixed legacy/subcommand parsing:

```rust
#[derive(Debug, Parser)]
#[command(name = "dp-simplify")]
struct Cli {
    // Legacy flat args (optional — used when no subcommand given)
    #[arg(long)]
    input: Option<String>,
    #[arg(long)]
    output: Option<String>,
    #[arg(long)]
    tolerance: Option<f64>,
    #[arg(long)]
    layer: Option<String>,
    #[arg(long, default_value_t = false)]
    validate_structure: bool,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    Simplify(SimplifyArgs),
    ReduceBend(ReduceBendArgs),
}
```

At runtime: if `command` is `None`, treat top-level `input`/`output`/`tolerance` as a legacy simplify invocation. This keeps backward compat while allowing explicit subcommands.

Legacy-mode validation rule: `--input`, `--output`, and `--tolerance` are required together when no subcommand is provided. If any are missing, return a clear CLI error instead of guessing defaults.

Note: shared args like `--layer`, `--validate-structure`, `--input`, and `--output` currently appear in both legacy mode and subcommands. Extracting them into a common `SharedArgs` struct remains an optional cleanup.

---

## 7. Test Strategy

### 7.1 Test Data Generation

`generate_test_reduce_bend_gpkg.py` (analogous to `generate_test_gpkg.py`) is the fixture-generation path that:
1. Runs the Python `ReduceBend.reduce()` on curated input GPKG files
2. Saves expected output GPKG files
3. Generates a `test_manifest_reduce_bend.json` with cases covering:
   - Open line with bends
   - Closed line with bends (polygon rings)
   - Mixed features (polygon + line + point)
   - Alternate bend fallback triggers
   - Line smoothing (with and without)
   - Polygon/hole deletion (`flag_del_outer`, `flag_del_inner`)
   - Multi-pass convergence
   - Edge cases: single-bend closed line, degenerate geometries

### 7.2 Unit Tests (Per-Module)

Write focused unit tests for each module before full integration. This de-risks debugging significantly:
- `detection.rs`: Test `angle_between_three_points` against known Python `QgsGeometryUtils.angleBetweenThreePoints` outputs. Test CW/CCW/FLAT classification for edge cases (near-π angles, near-0 angles, exactly flat).
- `detection.rs`: Test `detect_bends` on hand-crafted coordinate sequences (open + closed lines).
- `flagging.rs`: Test `flag_bend_to_reduce` adjacency exclusion logic with mock bend lists.
- `alternate.rs`: Test `find_alternate_bends` sub-range enumeration.
- `pre_process.rs`: Test polygon/hole deletion at `FeatureRecord` level.
- **Closed-ring invariant tests**: Dedicated tests that verify `coords.first() == coords.last()` after `add_vertex` on closed lines and after `pre_process` hole deletion. Ring closure is easy to accidentally break and causes subtle downstream failures in sidedness/polygonize.

### 7.3 Angle Parity Gate

Angle computation parity with Python should be validated with a fixed set of test triples (three-point sequences) and known `angleBetweenThreePoints` outputs. This remains the highest-risk divergence point in the pipeline.

### 7.4 Integration Tests

`tests/integration_tests_reduce_bend.rs` is implemented and currently follows the same strict assertion pattern as DP tests:
1. Load manifest from `tests/bend/test_manifest_reduce_bend.json`
2. Run Rust `ReduceBendEngine`
3. Compare output vs expected with:
   - GEOS topological equality (`equals`)
   - coordinate-level equality with epsilon (`1e-10`)

Note: tolerant smoothing/repair thresholds (Hausdorff/area-delta) are not yet implemented in the current test harness.

---

## 8. Implementation Status

### Completed
1. Shared infrastructure extensions are in place: generic sidedness path, `validate_sidedness_from_coords`, and `GsCollection::add_vertex` with runtime checks.
2. Core reduce-bend modules are implemented in `src/reduce_bend/` (`bend`, `detection`, `flagging`, `alternate`, `pre_process`, `smoothing`, engine in `mod.rs`).
3. CLI support is implemented with backward-compatible legacy simplify mode plus `simplify` and `reduce-bend` subcommands.
4. Reduce-bend integration tests and manifest-backed GPKG test corpus are present under `tests/bend/`.

### Remaining
5. Add and enforce explicit smoothing-tolerant assertions (Hausdorff/area-delta policy) if exact coordinate parity remains too strict for repair/smoothing cases.
6. Add focused angle-parity regression fixtures against Python `QgsGeometryUtils.angleBetweenThreePoints` outputs.

---

## 9. Key Risks & Remaining Decisions

### Resolved Decisions

- ~~`RbGeom` extension~~ → **Resolved**: Bend state is engine-owned (`HashMap`), `RbGeom` is unchanged. No risk to `simplify.rs`.
- ~~Feature deletion semantics~~ → **Resolved**: Pre-filter at `FeatureRecord` level before `GsFeature::from_records`.
- ~~Sidedness overload~~ → **Resolved**: Refactor to generic `&Geometry` input, update both call sites.
- ~~CLI naming~~ → **Resolved**: Keep `dp-simplify` binary name, add subcommands with legacy flat-flag backward compat.

### Active Risks

1. **Angle computation parity** (HIGH): Python uses `QgsGeometryUtils.angleBetweenThreePoints` which returns angles in `[0, 2π]`. Rust must replicate this exact convention or the CW/ACW/FLAT classification will diverge. **Mitigated by Phase 2 angle parity gate.**

2. **Rotation/translation numerics** (MEDIUM): The smoothing logic in `BendReduced._calculate_smooth_line` involves translate → rotate → compute → rotate back → translate back. Floating-point accumulation may cause micro-differences vs Python. **Mitigation pending:** add tolerant smoothing assertions (Hausdorff/area-delta).

3. **Polygon repair in smoothing** (MEDIUM): `_resolve_non_valid_polygon` uses unary union + polygonize to repair self-intersecting smooth polygons. GEOS version differences may produce different repair results. **Mitigation pending:** adopt Hausdorff/area-delta assertions for smoothing/repair cases.

4. **`validate_sidedness` refactor** (LOW): The generic function has a new signature, but the `validate_sidedness_from_coords` helper preserves the exact same behavior for DP simplify, including the current `is_simple` guard — `simplify.rs` call site changes from `validate_sidedness(others, coords)` to `validate_sidedness_from_coords(others, coords)` with identical semantics. Must re-run existing DP integration tests to confirm no regression.

---

## 10. Out of Scope for v1

The following are intentionally out of scope for the first reduce-bend migration milestone unless explicitly promoted:

1. Multipolygon-specific enhancements beyond current single-geometry processing assumptions in existing `SimpleGeometry`/`GsFeature` design.
2. Schema/FID roundtrip hardening beyond current Rust baseline (tracked separately from geometry/topology parity).
3. Performance benchmarking/optimization beyond parity and correctness gates.

---

## 11. Historical Notes

Phase 1 is complete. The detailed patch-by-patch implementation checklist was removed to keep this document focused on current behavior and remaining parity tasks.

For historical implementation details, see the current code and tests:
- `src/constraints.rs` (`validate_sidedness`, `validate_sidedness_from_coords`)
- `src/spatial_index.rs` (`GsCollection::add_vertex` + invariants/tests)
- `src/simplify/engine.rs` (DP sidedness call-site)
- `tests/integration_tests.rs` and `tests/integration_tests_reduce_bend.rs`
