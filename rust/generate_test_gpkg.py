"""
Generate test GPKG files for Rust migration of the Douglas-Peucker simplify algorithm.

This script creates input and expected-output GeoPackage files for each test case
extracted from tests/simplify_unittest.py.

Usage:
    Run inside a QGIS Python console or with QGIS Python environment:
        python generate_test_gpkg.py

Output:
    tests/data/test_XX_input.gpkg   — input geometries
    tests/data/test_XX_expected.gpkg — expected output geometries
    tests/data/test_manifest.json    — metadata for all test cases
"""

import json
import os
import sys

# Standalone QGIS/conda bootstrap
QGIS_PREFIX_PATH = os.environ.get(
    "QGIS_PREFIX_PATH",
    r"C:\Users\xxx\miniconda3\envs\data",
)

if os.name == "nt":
    qgis_bin = os.path.join(QGIS_PREFIX_PATH, "Library", "bin")
    if os.path.isdir(qgis_bin):
        os.environ["PATH"] = qgis_bin + os.pathsep + os.environ.get("PATH", "")

from qgis.core import (
    QgsApplication,
    QgsFeature,
    QgsField,
    QgsFields,
    QgsGeometry,
    QgsLineString,
    QgsPoint,
    QgsPolygon,
    QgsProcessingFeedback,
    QgsVectorFileWriter,
    QgsVectorLayer,
    QgsWkbTypes,
    QgsCoordinateReferenceSystem,
    QgsCoordinateTransformContext,
)
from qgis.PyQt.QtCore import QVariant

# Add parent directory to path so we can import the algorithm
sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from simplify import Simplify


def create_point(coord):
    return QgsPoint(coord[0], coord[1])


def create_line_geom(coords):
    pts = [create_point(c) for c in coords]
    return QgsGeometry(QgsLineString(pts))


def create_polygon_geom(outer, inners=None):
    outer_ls = QgsLineString([create_point(c) for c in outer])
    pol = QgsPolygon()
    pol.setExteriorRing(outer_ls)
    if inners:
        for inner in inners:
            inner_ls = QgsLineString([create_point(c) for c in inner])
            pol.addInteriorRing(inner_ls)
    return QgsGeometry(pol)


def make_features(geom_list):
    features = []
    for geom in geom_list:
        f = QgsFeature()
        f.setGeometry(geom)
        features.append(f)
    return features


def run_simplify(geom_list, tolerance):
    features = make_features(geom_list)
    feedback = QgsProcessingFeedback()
    results = Simplify.douglas_peucker(features, tolerance, False, feedback)
    return [f.geometry() for f in results.qgs_features_out]


def write_gpkg(filepath, geometries, geom_type=QgsWkbTypes.Unknown):
    if not geometries:
        fields = QgsFields()
        fields.append(QgsField("id", QVariant.Int))
        opts = QgsVectorFileWriter.SaveVectorOptions()
        opts.driverName = "GPKG"
        opts.fileEncoding = "UTF-8"
        writer = QgsVectorFileWriter.create(
            filepath,
            fields,
            QgsWkbTypes.LineString,
            QgsCoordinateReferenceSystem(),
            QgsCoordinateTransformContext(),
            opts,
        )
        del writer
        return

    if geom_type == QgsWkbTypes.Unknown:
        geom_type = geometries[0].wkbType()

    fields = QgsFields()
    fields.append(QgsField("id", QVariant.Int))
    fields.append(QgsField("tolerance", QVariant.Double))

    opts = QgsVectorFileWriter.SaveVectorOptions()
    opts.driverName = "GPKG"
    opts.fileEncoding = "UTF-8"

    writer = QgsVectorFileWriter.create(
        filepath,
        fields,
        geom_type,
        QgsCoordinateReferenceSystem(),
        QgsCoordinateTransformContext(),
        opts,
    )

    for i, geom in enumerate(geometries):
        feat = QgsFeature()
        feat.setFields(fields)
        feat.setAttribute("id", i)
        feat.setGeometry(geom)
        writer.addFeature(feat)

    del writer


# ---- Test Case Definitions ----

TEST_CASES = [
    {
        "id": 1,
        "title": "Empty file",
        "inputs": [],
        "tolerance": 2,
    },
    {
        "id": 2,
        "title": "Open line with 2 vertices",
        "inputs": [{"type": "line", "coords": [(0, 0), (10, 0)]}],
        "tolerance": 5,
    },
    {
        "id": 3,
        "title": "Open line with 3 vertices",
        "inputs": [{"type": "line", "coords": [(0, 0), (5, 1), (10, 0)]}],
        "tolerance": 5,
    },
    {
        "id": 4,
        "title": "Open line with 4 vertices, farthest=2",
        "inputs": [{"type": "line", "coords": [(0, 0), (5, 3), (10, 4), (20, 0)]}],
        "tolerance": 5,
    },
    {
        "id": 5,
        "title": "Open line with 4 vertices, farthest=1",
        "inputs": [{"type": "line", "coords": [(0, 0), (5, 4), (10, 3), (20, 0)]}],
        "tolerance": 5,
    },
    {
        "id": 6,
        "title": "Open line with 5 vertices, farthest in middle",
        "inputs": [
            {"type": "line", "coords": [(0, 0), (5, 3), (10, 4), (15, 3), (20, 0)]}
        ],
        "tolerance": 5,
    },
    {
        "id": 7,
        "title": "Open line with 5 vertices, partial simplify",
        "inputs": [
            {"type": "line", "coords": [(0, 0), (5, 3), (10, 4), (15, 3), (20, 0)]}
        ],
        "tolerance": 3.5,
    },
    {
        "id": 8,
        "title": "Open line with 5 vertices, below tolerance",
        "inputs": [
            {"type": "line", "coords": [(0, 0), (5, 3), (10, 4), (15, 3), (20, 0)]}
        ],
        "tolerance": 0.1,
    },
    {
        "id": 9,
        "title": "Triangle polygon, no simplification",
        "inputs": [
            {"type": "polygon", "outer": [(0, 0), (5, 5), (10, 0), (0, 0)], "inners": []}
        ],
        "tolerance": 10,
    },
    {
        "id": 10,
        "title": "Square polygon",
        "inputs": [
            {
                "type": "polygon",
                "outer": [(0, 0), (0, 5), (5, 5), (5, 0), (0, 0)],
                "inners": [],
            }
        ],
        "tolerance": 10,
    },
    {
        "id": 11,
        "title": "Pentagon simplified to square",
        "inputs": [
            {
                "type": "polygon",
                "outer": [(0, 0), (0, 5), (3, 6), (5, 5), (5, 0), (0, 0)],
                "inners": [],
            }
        ],
        "tolerance": 10,
    },
    {
        "id": 12,
        "title": "Pentagon simplified to square (bottom bump)",
        "inputs": [
            {
                "type": "polygon",
                "outer": [(0, 0), (0, 5), (5, 5), (5, 0), (2, 1), (0, 0)],
                "inners": [],
            }
        ],
        "tolerance": 10,
    },
    {
        "id": 13,
        "title": "Collinear point removal",
        "inputs": [
            {
                "type": "polygon",
                "outer": [(0, 0), (0, 5), (3, 5), (5, 5), (5, 0), (0, 0)],
                "inners": [],
            }
        ],
        "tolerance": 2,
    },
    {
        "id": 14,
        "title": "Small bump removal on polygon",
        "inputs": [
            {
                "type": "polygon",
                "outer": [(0, 0), (0, 5), (5, 5), (5, 0), (2, 1), (0, 0)],
                "inners": [],
            }
        ],
        "tolerance": 2,
    },
    {
        "id": 15,
        "title": "Self-intersecting open line",
        "inputs": [
            {
                "type": "line",
                "coords": [
                    (0, 0), (5, 0), (5, 2), (10, 2), (10, 0),
                    (50, 0), (50, -5), (7, -5), (7, 1),
                ],
            }
        ],
        "tolerance": 3,
    },
    {
        "id": 16,
        "title": "Cross-feature intersection blocks simplification",
        "inputs": [
            {"type": "line", "coords": [(0, 0), (2, 2), (4, 0)]},
            {"type": "line", "coords": [(2, -1), (2, 1)]},
        ],
        "tolerance": 3,
    },
    {
        "id": 17,
        "title": "Two parallel lines simplified",
        "inputs": [
            {"type": "line", "coords": [(0, 1), (3, 3), (6, 1)]},
            {"type": "line", "coords": [(0, 0), (3, 1.5), (6, 0)]},
        ],
        "tolerance": 3,
    },
    {
        "id": 18,
        "title": "Two parallel lines simplified (reversed order)",
        "inputs": [
            {"type": "line", "coords": [(0, 0), (3, 1.5), (6, 0)]},
            {"type": "line", "coords": [(0, 1), (3, 3), (6, 1)]},
        ],
        "tolerance": 3,
    },
    {
        "id": 19,
        "title": "Partial simplify with blocker line",
        "inputs": [
            {"type": "line", "coords": [(0, 0), (2, 2), (4, 0), (6, 2), (8, 0)]},
            {"type": "line", "coords": [(2, -1), (2, 0.5)]},
        ],
        "tolerance": 3,
    },
    {
        "id": 20,
        "title": "Partial simplify with blocker line (2)",
        "inputs": [
            {"type": "line", "coords": [(0, 0), (2, 2), (4, 0), (6, 2.5), (8, 0)]},
            {"type": "line", "coords": [(6, -1), (6, 0.5)]},
        ],
        "tolerance": 3,
    },
    {
        "id": 21,
        "title": "Sidedness constraint prevents simplification",
        "inputs": [
            {"type": "line", "coords": [(0, 0), (2, 2), (4, 0)]},
            {"type": "line", "coords": [(2, 0.1), (2, 0.2)]},
        ],
        "tolerance": 3,
    },
    {
        "id": 22,
        "title": "Sidedness, partial simplification",
        "inputs": [
            {"type": "line", "coords": [(0, 0), (2, 2), (4, 0), (6, 2.5), (8, 0)]},
            {"type": "line", "coords": [(2, 0.1), (2, 0.2)]},
        ],
        "tolerance": 3,
    },
    {
        "id": 23,
        "title": "Two disjoint lines simplified",
        "inputs": [
            {"type": "line", "coords": [(0, 0), (2, 2), (4, 0)]},
            {"type": "line", "coords": [(6, 0), (8, 2), (10, 0)]},
        ],
        "tolerance": 3,
    },
    {
        "id": 24,
        "title": "Two touching lines simplified",
        "inputs": [
            {"type": "line", "coords": [(0, 0), (2, 2), (4, 0)]},
            {"type": "line", "coords": [(4, 0), (6, 2), (8, 0)]},
        ],
        "tolerance": 3,
    },
    {
        "id": 25,
        "title": "Endpoint touching middle of other line",
        "inputs": [
            {"type": "line", "coords": [(0, 0), (2, 2), (4, 0)]},
            {"type": "line", "coords": [(-2, 0), (2, 0)]},
        ],
        "tolerance": 3,
    },
    {
        "id": 26,
        "title": "Superimposed middle sections",
        "inputs": [
            {"type": "line", "coords": [(0, 0), (2, 2), (4, 0)]},
            {"type": "line", "coords": [(1, 0), (3, 0)]},
        ],
        "tolerance": 3,
    },
    {
        "id": 27,
        "title": "Duplicate points simplified",
        "inputs": [
            {"type": "line", "coords": [(0, 0), (2, 2), (2, 2), (4, 0)]}
        ],
        "tolerance": 3,
    },
    {
        "id": 28,
        "title": "Multiple duplicate points simplified",
        "inputs": [
            {
                "type": "line",
                "coords": [(0, 0), (0, 0), (2, 2), (2, 2), (2, 2), (4, 0), (4, 0)],
            }
        ],
        "tolerance": 3,
    },
    {
        "id": 29,
        "title": "Degenerate lines (all identical points)",
        "inputs": [
            {"type": "line", "coords": [(0, 0), (0, 0)]},
            {"type": "line", "coords": [(10, 10), (10, 10), (10, 10)]},
            {"type": "line", "coords": [(20, 20), (20, 20), (20, 20), (20, 20)]},
            {
                "type": "line",
                "coords": [(30, 30), (30, 30), (30, 30), (30, 30), (30, 30)],
            },
            {
                "type": "line",
                "coords": [
                    (40, 40), (40, 40), (40, 40), (40, 40), (40, 40), (40, 40),
                ],
            },
        ],
        "tolerance": 15,
    },
]


def build_geom(spec):
    if spec["type"] == "line":
        return create_line_geom(spec["coords"])
    elif spec["type"] == "polygon":
        return create_polygon_geom(spec["outer"], spec.get("inners", []))
    raise ValueError(f"Unknown type: {spec['type']}")


def main():
    QgsApplication.setPrefixPath(QGIS_PREFIX_PATH, True)
    app = QgsApplication([], False)
    app.initQgis()

    data_dir = os.path.join(os.path.dirname(__file__), "tests", "data")
    os.makedirs(data_dir, exist_ok=True)

    manifest = []

    for tc in TEST_CASES:
        tid = tc["id"]
        print(f"Generating test {tid:02d}: {tc['title']}")

        input_geoms = [build_geom(spec) for spec in tc["inputs"]]
        tolerance = tc["tolerance"]

        input_path = os.path.join(data_dir, f"test_{tid:02d}_input.gpkg")
        expected_path = os.path.join(data_dir, f"test_{tid:02d}_expected.gpkg")

        write_gpkg(input_path, input_geoms)

        if input_geoms:
            output_geoms = run_simplify(input_geoms, tolerance)
        else:
            output_geoms = []

        write_gpkg(expected_path, output_geoms)

        manifest.append(
            {
                "id": tid,
                "title": tc["title"],
                "tolerance": tolerance,
                "input_file": f"test_{tid:02d}_input.gpkg",
                "expected_file": f"test_{tid:02d}_expected.gpkg",
                "num_input_features": len(input_geoms),
                "num_output_features": len(output_geoms),
            }
        )

    manifest_path = os.path.join(data_dir, "test_manifest.json")
    with open(manifest_path, "w") as f:
        json.dump(manifest, f, indent=2)

    print(f"\nGenerated {len(TEST_CASES)} test cases in {data_dir}")
    print(f"Manifest: {manifest_path}")

    app.exitQgis()


if __name__ == "__main__":
    main()
