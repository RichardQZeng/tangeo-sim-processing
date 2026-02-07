# -*- coding: utf-8 -*-

import inspect
import os

from qgis.PyQt.QtCore import QCoreApplication
from qgis.PyQt.QtGui import QIcon
from qgis.core import QgsProcessingAlgorithm


class GeoSimBaseAlgorithm(QgsProcessingAlgorithm):
    def tr(self, string):  # pylint: disable=no-self-use
        return QCoreApplication.translate("Processing", string)

    def createInstance(self):  # pylint: disable=no-self-use
        return type(self)()

    def group(self):
        return self.tr(self.groupId())

    def groupId(self):  # pylint: disable=no-self-use
        return ""

    def icon(self):  # pylint: disable=no-self-use
        cmd_folder = os.path.split(inspect.getfile(inspect.currentframe()))[0]
        return QIcon(os.path.join(cmd_folder, "logo.png"))

    @staticmethod
    def normalize_in_vector_layer(in_vector_layer, feedback):
        import processing

        feedback.pushInfo("Start normalizing input layer")
        params = {"INPUT": in_vector_layer, "OUTPUT": "memory:"}
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
        result_drop_zm = processing.run(
            "native:dropmzvalues", params, feedback=feedback
        )
        drop_zm_layer = result_drop_zm["OUTPUT"]

        qgs_in_features = []
        for qgs_feature in drop_zm_layer.getFeatures():
            qgs_in_features.append(qgs_feature)
        if len(qgs_in_features) > 1:
            geom_type = qgs_in_features[0].geometry().wkbType()
        else:
            geom_type = drop_zm_layer.wkbType()
        feedback.pushInfo("End normalizing input layer")

        return qgs_in_features, geom_type

    @staticmethod
    def reset_geometry_counters():
        from .geo_sim_util import GsFeature, RbGeom, SimGeom

        GsFeature._id_counter = 0
        RbGeom._id_counter = 0
        SimGeom._id_counter = 0
