use std::fs;
use std::path::PathBuf;

use dp_simplify::geometry::SimpleGeometry;
use dp_simplify::io::read_gpkg;
use dp_simplify::{SimplifyEngine, SimplifyParams};
use geos::Geom;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct ManifestCase {
    id: u32,
    title: String,
    tolerance: f64,
    input_file: String,
    expected_file: String,
}

#[test]
fn test_point_to_segment_distance_basic() {
    use dp_simplify::geometry::Coord;
    use dp_simplify::simplify::point_to_segment_dist;

    let d = point_to_segment_dist(
        Coord { x: 5.0, y: 2.0 },
        Coord { x: 0.0, y: 0.0 },
        Coord { x: 10.0, y: 0.0 },
    );
    assert!((d - 2.0).abs() < 1e-12);
}

#[test]
fn test_manifest_cases_if_available() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let data_dir = root.join("tests").join("data");
    let manifest_path = data_dir.join("test_manifest.json");

    if !manifest_path.exists() {
        eprintln!("Skipping integration data tests: {} not found", manifest_path.display());
        return;
    }

    let manifest_text = fs::read_to_string(&manifest_path).expect("read manifest");
    let cases: Vec<ManifestCase> = serde_json::from_str(&manifest_text).expect("parse manifest");

    for case in cases {
        let input_path = data_dir.join(case.input_file);
        let expected_path = data_dir.join(case.expected_file);

        let (input_records, _schema) = read_gpkg(input_path.to_string_lossy().as_ref(), None)
            .unwrap_or_else(|e| panic!("case {} {} input read failed: {e}", case.id, case.title));

        let engine = SimplifyEngine::new(
            &input_records,
            SimplifyParams {
                tolerance: case.tolerance,
                validate_structure: false,
            },
        )
        .unwrap_or_else(|e| panic!("case {} {} engine init failed: {e}", case.id, case.title));

        let output = engine
            .run()
            .unwrap_or_else(|e| panic!("case {} {} simplify failed: {e}", case.id, case.title));

        let (expected_records, _) = read_gpkg(expected_path.to_string_lossy().as_ref(), None)
            .unwrap_or_else(|e| panic!("case {} {} expected read failed: {e}", case.id, case.title));

        assert_eq!(
            output.features.len(),
            expected_records.len(),
            "case {} {} feature count mismatch",
            case.id,
            case.title
        );

        for (idx, (got, exp)) in output.features.iter().zip(expected_records.iter()).enumerate() {
            let got_geos = got
                .geometry
                .to_geos()
                .unwrap_or_else(|e| panic!("case {} geom {} invalid output geos: {e}", case.id, idx));
            let exp_geos = exp
                .geometry
                .to_geos()
                .unwrap_or_else(|e| panic!("case {} geom {} invalid expected geos: {e}", case.id, idx));

            let equal = got_geos
                .equals(&exp_geos)
                .unwrap_or_else(|e| panic!("case {} geom {} equals failed: {e}", case.id, idx));
            assert!(equal, "case {} {} feature {} topological mismatch", case.id, case.title, idx);

            assert_coords_equal(&got.geometry, &exp.geometry, 1e-10, case.id, &case.title, idx);
        }
    }
}

fn assert_coords_equal(got: &SimpleGeometry, exp: &SimpleGeometry, eps: f64, case_id: u32, title: &str, feature_idx: usize) {
    let g = got.all_coords();
    let e = exp.all_coords();

    assert_eq!(
        g.len(),
        e.len(),
        "case {} {} feature {} coordinate count mismatch",
        case_id,
        title,
        feature_idx
    );

    for (i, (gc, ec)) in g.iter().zip(e.iter()).enumerate() {
        assert!(
            (gc.x - ec.x).abs() <= eps && (gc.y - ec.y).abs() <= eps,
            "case {} {} feature {} coordinate {} mismatch: got=({}, {}), expected=({}, {})",
            case_id,
            title,
            feature_idx,
            i,
            gc.x,
            gc.y,
            ec.x,
            ec.y
        );
    }
}
