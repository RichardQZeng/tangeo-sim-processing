# -*- coding: utf-8 -*-
# pylint: disable=no-name-in-module

from qgis.core import (
    QgsFeatureRequest,
    QgsFeatureSink,
    QgsProcessing,
    QgsProcessingException,
    QgsProcessingParameterBoolean,
    QgsProcessingParameterDistance,
    QgsProcessingParameterFeatureSink,
    QgsProcessingParameterFeatureSource,
    QgsWkbTypes,
)

from .base_algorithm import GeoSimBaseAlgorithm
from .core.simplify import Simplify


class SimplifyAlgorithm(GeoSimBaseAlgorithm):
    def name(self):  # pylint: disable=no-self-use
        return "simplify"

    def displayName(self):  # pylint: disable=no-self-use
        return self.tr("Simplify")

    def shortHelpString(self):
        help_str = """
    Simplify is a geospatial simplification (generalization) tool for lines and polygons. Simplify \
    implements an improved version of the classic Douglas-Peucker algorithm with spatial constraints \
    validation during geometry simplification.  Simplify will preserve the following topological relationships:  \
    Simplicity (within the geometry), Intersection (with other geometries) and Sidedness (with other geometries).

    <b>Usage</b>
    <u>Input layer</u> : Any LineString or Polygon layer.  Multi geometry are transformed into single part geometry.
    <u>Tolerance</u>: Tolerance used for line simplification.
    <u>Simplified</u> : Output layer of the algorithm.

    <b>Rule of thumb for the diameter tolerance</b>
    Simplify (Douglas-Peucker) is an excellent tool to remove vertices on features with high vertex densities \
    while preserving a maximum of details within the geometries.  Try it with small tolerance value and then use \
    Reduce Bend to generalize features (generalization is needed).
    """
        return self.tr(help_str)

    def initAlgorithm(self, config=None):  # pylint: disable=unused-argument
        self.addParameter(
            QgsProcessingParameterFeatureSource(
                "INPUT",
                self.tr("Input layer"),
                types=[QgsProcessing.TypeVectorAnyGeometry],
            )
        )
        self.addParameter(
            QgsProcessingParameterDistance(
                "TOLERANCE",
                self.tr("Diameter tolerance"),
                defaultValue=0.0,
                parentParameterName="INPUT",
            )
        )
        self.addParameter(
            QgsProcessingParameterBoolean(
                "VALIDATE_STRUCTURE",
                self.tr("Validate structure (debug)"),
                defaultValue=False,
            )
        )
        self.addParameter(
            QgsProcessingParameterFeatureSink("OUTPUT", self.tr("Simplified"))
        )

    def processAlgorithm(self, parameters, context, feedback):
        context.setInvalidGeometryCheck(QgsFeatureRequest.GeometryNoCheck)
        self.reset_geometry_counters()
        source_in = self.parameterAsSource(parameters, "INPUT", context)
        tolerance = self.parameterAsDouble(parameters, "TOLERANCE", context)
        validate_structure = self.parameterAsBool(
            parameters, "VALIDATE_STRUCTURE", context
        )

        if source_in is None:
            raise QgsProcessingException(self.invalidSourceError(parameters, "INPUT"))

        vector_layer_in = source_in.materialize(QgsFeatureRequest(), feedback)
        qgs_features_in, geom_type = self.normalize_in_vector_layer(
            vector_layer_in, feedback
        )
        if geom_type not in (QgsWkbTypes.LineString, QgsWkbTypes.Polygon):
            raise QgsProcessingException(
                "Can only process: (Multi)LineString or (Multi)Polygon vector layers"
            )

        sink, dest_id = self.parameterAsSink(
            parameters,
            "OUTPUT",
            context,
            vector_layer_in.fields(),
            geom_type,
            vector_layer_in.sourceCrs(),
        )
        if sink is None:
            raise QgsProcessingException(self.invalidSinkError(parameters, "OUTPUT"))

        feedback.setProgress(1)
        rb_return = Simplify.douglas_peucker(
            qgs_features_in, tolerance, validate_structure, feedback
        )
        for qgs_feature_out in rb_return.qgs_features_out:
            sink.addFeature(qgs_feature_out, QgsFeatureSink.FastInsert)

        feedback.pushInfo(" ")
        feedback.pushInfo(
            "Number of features in: {0}".format(rb_return.in_nbr_features)
        )
        feedback.pushInfo(
            "Number of features out: {0}".format(rb_return.out_nbr_features)
        )
        feedback.pushInfo("Number of iteration needed: {0}".format(rb_return.nbr_pass))
        feedback.pushInfo(
            "Total vertice deleted: {0}".format(rb_return.nbr_vertice_deleted)
        )
        if validate_structure:
            status = "Valid" if rb_return.is_structure_valid else "Invalid"
            feedback.pushInfo(
                "Debug - State of the internal data structure: {0}".format(status)
            )

        return {"OUTPUT": dest_id}
