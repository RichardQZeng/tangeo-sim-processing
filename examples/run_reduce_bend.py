"""Example: run Reduce Bend core algorithm from Python."""

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from core.reduce_bend import ReduceBend

from examples.common import init_qgis, make_feedback, make_line_feature


def main():
    app = init_qgis()
    try:
        features = [make_line_feature([(0.0, 0.0), (2.0, 2.0), (4.0, 0.0), (6.0, 0.0)])]
        feedback = make_feedback()

        result = ReduceBend.reduce(
            qgs_in_features=features,
            diameter_tol=3.0,
            smooth_line=False,
            flag_del_outer=False,
            flag_del_inner=False,
            validate_structure=False,
            feedback=feedback,
        )

        print(f"input features: {result.in_nbr_features}")
        print(f"output features: {result.out_nbr_features}")
        print(f"bends detected: {result.nbr_bend_detected}")
        print(f"bends reduced: {result.nbr_bend_reduced}")
        print("output WKT:")
        for feature in result.qgs_features_out:
            print(feature.geometry().asWkt())
    finally:
        app.exitQgis()


if __name__ == "__main__":
    main()
