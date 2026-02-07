"""Example: run Chordal Axis from Python."""

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from chordal_axis_algorithm import ChordalAxis, GenUtil, tessellate_polygon

from examples.common import init_qgis, make_feedback, make_polygon_layer


def main():
    app = init_qgis()
    try:
        polygon_layer = make_polygon_layer(
            [(0.0, 0.0), (8.0, 0.0), (8.0, 2.0), (0.0, 2.0), (0.0, 0.0)]
        )
        feedback = make_feedback()

        triangle_features = tessellate_polygon(polygon_layer, feedback)
        if not triangle_features:
            print("No tessellation produced.")
            return

        chordal = ChordalAxis(triangle_features[0], GenUtil.ZERO)
        centre_lines = chordal.get_skeleton()

        print(f"triangles feature count: {len(triangle_features)}")
        print(f"centre line count: {len(centre_lines)}")
        print("centre line WKT:")
        for line in centre_lines:
            print(line.asWkt())
    finally:
        app.exitQgis()


if __name__ == "__main__":
    main()
