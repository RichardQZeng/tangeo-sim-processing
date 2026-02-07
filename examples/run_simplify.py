"""Example: run Simplify core algorithm from Python."""

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from core.simplify import Simplify

from examples.common import init_qgis, make_feedback, make_line_feature


def main():
    app = init_qgis()
    try:
        features = [make_line_feature([(0.0, 0.0), (1.0, 0.2), (2.0, 0.0), (3.0, 0.0)])]
        feedback = make_feedback()

        result = Simplify.douglas_peucker(
            qgs_in_features=features,
            tolerance=0.25,
            validate_structure=False,
            feedback=feedback,
        )

        print(f"input features: {result.in_nbr_features}")
        print(f"output features: {result.out_nbr_features}")
        print(f"vertices deleted: {result.nbr_vertice_deleted}")
        print("output WKT:")
        for feature in result.qgs_features_out:
            print(feature.geometry().asWkt())
    finally:
        app.exitQgis()


if __name__ == "__main__":
    main()
