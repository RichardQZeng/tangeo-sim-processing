# -*- coding: utf-8 -*-

from qgis.core import (
    QgsApplication,
    QgsFeature,
    QgsFeatureRequest,
    QgsFeatureSink,
    QgsFields,
    QgsGeometry,
    QgsProcessing,
    QgsProcessingException,
    QgsProcessingParameterBoolean,
    QgsProcessingParameterFeatureSink,
    QgsProcessingParameterFeatureSource,
    QgsVectorLayer,
    QgsWkbTypes,
)

from .base_algorithm import GeoSimBaseAlgorithm
from .core.chordal_axis import ChordalAxis, GenUtil, SpatialContainer, _TriangleSc


def _tessellate_polygon(source, feedback):
    import processing
    from qgis._3d import Qgs3DAlgorithms

    params = {"INPUT": source, "OUTPUT": "memory:"}
    result_ms = processing.run(
        "native:multiparttosingleparts", params, feedback=feedback
    )
    ms_part_layer = result_ms["OUTPUT"]

    params = {
        "INPUT": ms_part_layer,
        "DROP_M_VALUES": True,
        "DROP_Z_VALUES": True,
        "OUTPUT": "memory:",
    }
    result_drop_zm = processing.run("native:dropmzvalues", params, feedback=feedback)
    drop_zm_layer = result_drop_zm["OUTPUT"]

    if drop_zm_layer.wkbType() != QgsWkbTypes.Polygon:
        geom_type_str = QgsWkbTypes.geometryDisplayString(drop_zm_layer.geometryType())
        raise QgsProcessingException(
            "Unable to process geometry: {}".format(geom_type_str)
        )

    source_crs = source.sourceCrs()
    epsg_id = source_crs.authid()
    if not epsg_id:
        uri_p = "Polygon"
    else:
        uri_p = "Polygon?" + epsg_id

    QgsApplication.processingRegistry().addProvider(
        Qgs3DAlgorithms(QgsApplication.processingRegistry())
    )

    qgs_multi_triangles = []
    for qgs_feature in drop_zm_layer.getFeatures():
        single_feature_layer = QgsVectorLayer(uri_p, "temporary_polygon", "memory")
        data_provider = single_feature_layer.dataProvider()
        data_provider.addFeatures([qgs_feature])
        single_feature_layer.updateExtents()
        try:
            result_tessellate = processing.run(
                "3d:tessellate",
                {"INPUT": single_feature_layer, "OUTPUT": "memory:"},
                feedback=feedback,
            )
        except Exception as exc:
            raise QgsProcessingException(
                "Unable to tessellate feature: {}".format(qgs_feature.id())
            ) from exc

        tessellate_layer = result_tessellate["OUTPUT"]
        for tess_feature in tessellate_layer.getFeatures():
            qgs_geom = tess_feature.geometry()
            qgs_geoms = qgs_geom.coerceToType(QgsWkbTypes.MultiPolygon)
            tess_feature.setGeometry(qgs_geoms[0])
            qgs_multi_triangles.append(tess_feature)

    return qgs_multi_triangles


class ChordalAxisAlgorithm(GeoSimBaseAlgorithm):
    def name(self):  # pylint: disable=no-self-use
        return "chordalaxis"

    def displayName(self):  # pylint: disable=no-self-use
        return self.tr("Chordal axis")

    def shortHelpString(self):
        help_str = """
    <b>Chordal Axis</b>
    ChordalAxis is a geospatial tool that creates a skeleton (the center line). ChordalAxis creates a \
    triangulation (using QGIS Tessellate tools) and use it to extract the chordal axis (center line).

    <b>Usage</b>
    <u>Input</u>: A polygon layer to extract the chordal axis.

    <u>Correct skeleton</u>:  Correct the skeleton for small centre line, T junction and X junction. Useful in the case \
    of long any narrow polygon (ex.: polygonized road network)

    <b>Output</b>
    <u>Chordal axis</u>: The center line of the polygons.
    <u>Tessellate</u>: The result of the triangulation.
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
            QgsProcessingParameterBoolean(
                "CORRECTION", self.tr("Correction"), defaultValue=False
            )
        )
        self.addParameter(
            QgsProcessingParameterFeatureSink("OUTPUT", self.tr("Chordal axis"))
        )
        self.addParameter(
            QgsProcessingParameterFeatureSink("TRIANGLES", self.tr("Tessellation"))
        )

    def processAlgorithm(self, parameters, context, feedback):
        context.setInvalidGeometryCheck(QgsFeatureRequest.GeometryNoCheck)
        SpatialContainer._sc_id = 1
        _TriangleSc.id = 0
        in_source = self.parameterAsSource(parameters, "INPUT", context)
        correction = self.parameterAsBool(parameters, "CORRECTION", context)
        if in_source is None:
            raise QgsProcessingException(self.invalidSourceError(parameters, "INPUT"))

        in_vector_layer = in_source.materialize(QgsFeatureRequest(), feedback)
        sink, dest_id = self.parameterAsSink(
            parameters,
            "OUTPUT",
            context,
            QgsFields(),
            QgsWkbTypes.LineString,
            in_vector_layer.sourceCrs(),
        )
        sink_t, dest_id_t = self.parameterAsSink(
            parameters,
            "TRIANGLES",
            context,
            QgsFields(),
            QgsWkbTypes.MultiPolygon,
            in_vector_layer.sourceCrs(),
        )
        if sink is None:
            raise QgsProcessingException(self.invalidSinkError(parameters, "OUTPUT"))
        if sink_t is None:
            raise QgsProcessingException(self.invalidSinkError(parameters, "TRIANGLES"))

        nbr_polygon = in_vector_layer.featureCount()
        nbr_centre_line = 0
        total = 100.0 / nbr_polygon if nbr_polygon else 0
        qgs_multi_triangles = _tessellate_polygon(in_vector_layer, feedback)
        for i, qgs_multi_triangle in enumerate(qgs_multi_triangles):
            sink_t.addFeature(qgs_multi_triangle, QgsFeatureSink.FastInsert)
            centre_lines = []
            try:
                ca = ChordalAxis(qgs_multi_triangle, GenUtil.ZERO)
                if correction:
                    ca.correct_skeleton()
                centre_lines = ca.get_skeleton()
            except Exception:
                import traceback

                traceback.print_exc()

            for line in centre_lines:
                out_feature = QgsFeature()
                out_feature.setGeometry(QgsGeometry(line.clone()))
                sink.addFeature(out_feature, QgsFeatureSink.FastInsert)
                nbr_centre_line += 1

            if feedback.isCanceled():
                break
            feedback.setProgress(int(i * total))

        feedback.pushInfo("Number of input polygons: {0}".format(nbr_polygon))
        feedback.pushInfo("Number of centre lines: {0}".format(nbr_centre_line))
        return {"OUTPUT": dest_id, "TRIANGLES": dest_id_t}
