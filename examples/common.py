"""Shared helpers for running algorithms from a plain Python script."""

import os

from qgis.analysis import QgsNativeAlgorithms
from qgis.core import (
    QgsApplication,
    QgsFeature,
    QgsGeometry,
    QgsLineString,
    QgsPoint,
    QgsPolygon,
    QgsProcessingFeedback,
    QgsVectorLayer,
)
from processing.core.Processing import Processing

try:
    from qgis._3d import Qgs3DAlgorithms
except ImportError:
    Qgs3DAlgorithms = None


def init_qgis():
    """Initialize QGIS providers for script usage."""

    prefix_path = os.environ.get("QGIS_PREFIX_PATH")
    if prefix_path:
        QgsApplication.setPrefixPath(prefix_path, True)

    app = QgsApplication([], False)
    app.initQgis()

    Processing.initialize()
    registry = QgsApplication.processingRegistry()
    if registry.providerById("native") is None:
        registry.addProvider(QgsNativeAlgorithms())
    if Qgs3DAlgorithms is not None and registry.providerById("3d") is None:
        registry.addProvider(Qgs3DAlgorithms(registry))

    return app


def make_line_feature(coords):
    points = [QgsPoint(x, y) for x, y in coords]
    feature = QgsFeature()
    feature.setGeometry(QgsGeometry(QgsLineString(points)))
    return feature


def make_polygon_feature(outer_ring_coords):
    outer_points = [QgsPoint(x, y) for x, y in outer_ring_coords]
    polygon = QgsPolygon()
    polygon.setExteriorRing(QgsLineString(outer_points))
    feature = QgsFeature()
    feature.setGeometry(QgsGeometry(polygon))
    return feature


def make_polygon_layer(outer_ring_coords):
    layer = QgsVectorLayer("Polygon", "example_input", "memory")
    provider = layer.dataProvider()
    provider.addFeatures([make_polygon_feature(outer_ring_coords)])
    layer.updateExtents()
    return layer


def make_feedback():
    return QgsProcessingFeedback()
