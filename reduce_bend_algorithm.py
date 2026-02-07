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
from .core.reduce_bend import ReduceBend


class ReduceBendAlgorithm(GeoSimBaseAlgorithm):
    def name(self):  # pylint: disable=no-self-use
        return "reducebend"

    def displayName(self):  # pylint: disable=no-self-use
        return self.tr("Reduce bend")

    def shortHelpString(self):
        help_str = """
    Reduce bend is a geospatial simplification and generalization tool for lines and polygons. The \
    particularity of this algorithm is that for each line or polygon it analyzes its bends (curves) and \
    decides which one to reduce, trying to emulate what a cartographer would do manually \
    to simplify or generalize a line. Reduce bend will accept lines and polygons as input.  Reduce bend will \
    preserve the topology (spatial relations) within and between the features during the bend reduction. \
    Reduce bend also accept multi lines and multi polygons but will output lines and polygons.

    <b>Usage</b>
    <u>Input layer</u> : Any LineString or Polygon layer.  Multi geometry are transformed into single part geometry.
    <u>Diameter tolerance</u>: Theoretical diameter of a bend to remove.
    <u>Smooth line</u>: If you want to smooth the reduced bends (when possible).
    <u>Exclude hole</u>: If you want to exclude (delete )holes below the diameter of the bend.
    <u>Exclude polygon</u>: If you want to exclude (delete) polygon below the diameter of the bend.
    <u>Reduced bend</u> : Output layer of the algorithm.

    <b>Rule of thumb for the diameter tolerance</b>
    Reduce bend can be used for line (polygon) simplifying in the context of line (polygon) generalization. The big \
    question will often be what diameter should we use? A good starting point is the cartographic rule of \
    thumb -- the .5mm on the map -- which says that the minimum distance between two lines should be \
    greater than 0.5mm on a paper map. So to simplify (generalize) a line for representation at a scale of \
    1:50 000 for example a diameter of 25m should be a good starting point.
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
                "SMOOTH", self.tr("Smooth line"), defaultValue=False
            )
        )
        self.addParameter(
            QgsProcessingParameterBoolean(
                "EXCLUDE_POLYGON", self.tr("Exclude polygon"), defaultValue=True
            )
        )
        self.addParameter(
            QgsProcessingParameterBoolean(
                "EXCLUDE_HOLE", self.tr("Exclude hole"), defaultValue=True
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
            QgsProcessingParameterFeatureSink("OUTPUT", self.tr("Reduced bend"))
        )

    def processAlgorithm(self, parameters, context, feedback):
        context.setInvalidGeometryCheck(QgsFeatureRequest.GeometryNoCheck)
        self.reset_geometry_counters()
        source_in = self.parameterAsSource(parameters, "INPUT", context)
        diameter_tol = self.parameterAsDouble(parameters, "TOLERANCE", context)
        smooth_line = self.parameterAsBool(parameters, "SMOOTH", context)
        exclude_hole = self.parameterAsBool(parameters, "EXCLUDE_HOLE", context)
        exclude_polygon = self.parameterAsBool(parameters, "EXCLUDE_POLYGON", context)
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
        rb_return = ReduceBend.reduce(
            qgs_features_in,
            diameter_tol,
            smooth_line,
            exclude_polygon,
            exclude_hole,
            validate_structure,
            feedback,
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
            "Number of bends detected: {0}".format(rb_return.nbr_bend_detected)
        )
        feedback.pushInfo(
            "Number of bends reduced: {0}".format(rb_return.nbr_bend_reduced)
        )
        feedback.pushInfo(
            "Number of deleted polygons: {0}".format(rb_return.nbr_pol_del)
        )
        feedback.pushInfo(
            "Number of deleted polygon holes: {0}".format(rb_return.nbr_hole_del)
        )
        feedback.pushInfo(
            "Number of line smoothed: {0}".format(rb_return.nbr_line_smooth)
        )
        if validate_structure:
            status = "Valid" if rb_return.is_structure_valid else "Invalid"
            feedback.pushInfo(
                "Debug - State of the internal data structure: {0}".format(status)
            )

        return {"OUTPUT": dest_id}
