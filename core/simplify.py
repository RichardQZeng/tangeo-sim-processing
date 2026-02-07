# -*- coding: utf-8 -*-
# pylint: disable=no-name-in-module
# pylint: disable=too-many-lines
# pylint: disable=useless-return
# pylint: disable=too-few-public-methods
# pylint: disable=relative-beyond-top-level

from qgis.core import QgsLineString, QgsWkbTypes, QgsGeometry

from ..geo_sim_util import Epsilon, GsCollection, GeoSimUtil, GsFeature, ProgressBar


# --------------------------------------------------------
# Start of the algorithm
# --------------------------------------------------------

# Define global constant


class RbResults:
    """Class defining the stats and results"""

    __slots__ = ('in_nbr_features', 'out_nbr_features', 'nbr_vertice_deleted',  'qgs_features_out', 'nbr_pass',
                 'is_structure_valid')

    def __init__(self):
        """Constructor that initialize a RbResult object.

        :param: None
        :return: None
        :rtype: None
        """

        self.in_nbr_features = None
        self.out_nbr_features = None
        self.nbr_vertice_deleted = 0
        self.qgs_features_out = None
        self.nbr_pass = 0
        self.is_structure_valid = None


class Simplify:
    """Main class for bend reduction"""

    @staticmethod
    def douglas_peucker(qgs_in_features, tolerance, validate_structure=False, feedback=None):
        """Main static method used to launch the simplification of the Douglas-Peucker algorithm.

        :param: qgs_features: List of QgsFeatures to process.
        :param: tolerance: Simplification tolerance in ground unit.
        :param: validate_structure: Validate internal data structure after processing (for debugging only)
        :param: feedback: QgsFeedback handle for interaction with QGIS.
        :return: Statistics and results object.
        :rtype: RbResults
        """

        dp = Simplify(qgs_in_features, tolerance, validate_structure, feedback)
        results = dp.reduce()

        return results

    @staticmethod
    def find_farthest_point(qgs_points, first, last, ):
        """Returns a tuple with the farthest point's index and it's distance from a subline section

        :param: qgs_points: List of QgsPoint defining the line to process
        :first: int: Index of the first point in qgs_points
        :last: int: Index of the last point in qgs_points
        :return: distance from the farthest point; index of the farthest point
        :rtype: tuple of 2 values
        """

        if last - first >= 2:
            qgs_geom_first_last = QgsLineString(qgs_points[first], qgs_points[last])
            qgs_geom_engine = QgsGeometry.createGeometryEngine(qgs_geom_first_last)
            distances = [qgs_geom_engine.distance(qgs_points[i]) for i in range(first + 1, last)]
            farthest_dist = max(distances)
            farthest_index = distances.index(farthest_dist) + first + 1
        else:
            # Not enough vertice to calculate the farthest distance
            farthest_dist = -1.
            farthest_index = first

        return farthest_index, farthest_dist

    __slots__ = ('tolerance', 'validate_structure', 'feedback', 'rb_collection', 'eps', 'rb_results', 'rb_geoms',
                 'gs_features')

    def __init__(self, qgs_in_features, tolerance, validate_structure, feedback):
        """Constructor for Simplify algorithm.

       :param: qgs_in_features: List of features to process.
       :param: tolerance: Float tolerance distance of the Douglas Peucker algorithm.
       :param: validate_structure: flag to validate internal data structure after processing (for debugging)
       :param: feedback: QgsFeedback handle for interaction with QGIS.
       """

        self.tolerance = tolerance
        self.validate_structure = validate_structure
        self.feedback = feedback

        # Calculates the epsilon and initialize some stats and results value
        self.eps = Epsilon(qgs_in_features)
        self.eps.set_class_variables()
        self.rb_results = RbResults()

        # Create the list of GsPolygon, GsLineString and GsPoint to process
        self.rb_results.in_nbr_features = len(qgs_in_features)
        self.gs_features = GsFeature.create_gs_feature(qgs_in_features)

    def reduce(self):
        """Main method to reduce line string.

        :return: Statistics and result object.
        :rtype: RbResult
        """

        #  Code used for the profiler (uncomment if needed)
#        import cProfile, pstats, io
#        from pstats import SortKey
#        pr = cProfile.Profile()
#        pr.enable()

        # Calculates the epsilon and initialize some stats and results value
#        self.eps = Epsilon(self.qgs_in_features)
#        self.eps.set_class_variables()
#        self.rb_results = RbResults()

#        # Create the list of GsPolygon, GsLineString and GsPoint to process
#        self.gs_features = GeoSimUtil.create_gs_feature(self.qgs_in_features)

        # Pre process the LineString: remove to close point and co-linear points
        self.rb_geoms = self.pre_simplification_process()

        # Create the GsCollection a spatial index to accelerate search for spatial relationships
        self.rb_collection = GsCollection()
        self.rb_collection.add_features(self.rb_geoms, self.feedback)

        # Execute the line simplification for each LineString
        self._simplify_lines()

        # Recreate the QgsFeature
        qgs_features_out = [gs_feature.get_qgs_feature() for gs_feature in self.gs_features]

        # Set return values
        self.rb_results.out_nbr_features = len(qgs_features_out)
        self.rb_results.qgs_features_out = qgs_features_out

        # Validate inner spatial structure. For debug purpose only
        if self.rb_results.is_structure_valid:
            self.rb_collection.validate_integrity(self.rb_geoms)

        #  Code used for the profiler (uncomment if needed)
 #       pr.disable()
 #       s = io.StringIO()
 #       sortby = SortKey.CUMULATIVE
 #       ps = pstats.Stats(pr, stream=s).sort_stats(sortby)
 #       ps.print_stats()
 #       print(s.getvalue())

        return self.rb_results

    def pre_simplification_process(self):
        """This method execute the pre simplification process

        Pre simplification process applies only to closed line string and is used to find the 2 points that are
        the distant from each other using the oriented bounding box

        :return: List of rb_geom
        :rtype: [RbGeom]
        """

        # Create the list of RbGeom ==> List of geometry to simplify
        sim_geoms = []
        for gs_feature in self.gs_features:
            sim_geoms += gs_feature.get_rb_geom()

        return sim_geoms

    def _simplify_lines(self):
        """Loop over the geometry until there is no more subline to simplify

        An iterative process for line simplification is applied in order to maximise line simplification.  The process
        will always stabilize and exit when there are no more simplification to do.
        """

        while True:
            progress_bar_value = 0
            self.rb_results.nbr_pass += 1
            progress_bar = ProgressBar(self.feedback, len(self.rb_geoms),
                                       "Iteration: {0}".format(self.rb_results.nbr_pass))
            nbr_vertice_deleted = 0
            for i, rb_geom in enumerate(self.rb_geoms):
                if self.feedback.isCanceled():
                    break
                progress_bar.set_value(i)
                if not rb_geom.is_simplest:  # Only process geometry that are not at simplest form
                    nbr_vertice_deleted += self.process_line(rb_geom)

            self.feedback.pushInfo("Vertice deleted: {0}".format(nbr_vertice_deleted))

            # While loop breaking condition (when no vertice deleted in a loop)
            if nbr_vertice_deleted == 0:
                break
            self.rb_results.nbr_vertice_deleted += nbr_vertice_deleted

        return

    def validate_constraints(self, sim_geom, first, last):
        """Validate the spatial relationship in order maintain topological structure

        Three distinct spatial relation are tested in order to assure that each bend reduce will continue to maintain
        the topological structure in a feature between the features:
         - Simplicity: Adequate validation is done to make sure that the bend reduction will not cause the feature
                       to cross  itself.
         - Intersection : Adequate validation is done to make sure that a line from other features will not intersect
                          the bend being reduced
         - Sidedness: Adequate validation is done to make sure that a line is not completely contained in the bend.
                      This situation can happen when a ring in a polygon complete;y lie in a bend ans after bend
                      reduction, the the ring falls outside the polygon which make it invalid.

        Note if the topological structure is wrong before the bend correction no correction will be done on these
        errors.

        :param: sim_geom: Geometry used to validate constraints
        :param: first: Index of the start vertice of the subline
        :param: last: Index of the last vertice of the subline
        :return: Flag indicating if the spatial constraints are valid for this subline simplification
        :rtype: Bool
        """

        constraints_valid = True

        qgs_points = [sim_geom.qgs_geom.vertexAt(i) for i in range(first, last+1)]
        qgs_geom_new_subline = QgsGeometry(QgsLineString(qgs_points[0], qgs_points[-1]))
        qgs_geom_old_subline = QgsGeometry(QgsLineString(qgs_points))
        qgs_geoms_with_itself, qgs_geoms_with_others = \
            self.rb_collection.get_segment_intersect(sim_geom.id, qgs_geom_old_subline.boundingBox(),
                                                     qgs_geom_old_subline)

        # First: check if the bend reduce line string is an OGC simple line
        constraints_valid = GeoSimUtil.validate_simplicity(qgs_geoms_with_itself, qgs_geom_new_subline)

        # Second: check that the new line does not intersect with any other line or points
        if constraints_valid and len(qgs_geoms_with_others) >= 1:
            constraints_valid = GeoSimUtil.validate_intersection(qgs_geoms_with_others, qgs_geom_new_subline)

        # Third: check that inside the subline to simplify there is no feature completely inside it.  This would cause a
        # sidedness or relative position error
        if constraints_valid and len(qgs_geoms_with_others) >= 1:
            qgs_ls_old_subline = QgsLineString(qgs_points)
            qgs_ls_old_subline.addVertex(qgs_points[0])  # Close the line with the start point
            qgs_geom_old_subline = QgsGeometry(qgs_ls_old_subline.clone())

            # Next two lines used to transform a self intersecting line into a valid MultiPolygon
            qgs_geom_unary = QgsGeometry.unaryUnion([qgs_geom_old_subline])
            qgs_geom_polygonize = QgsGeometry.polygonize([qgs_geom_unary])

            if qgs_geom_polygonize.isSimple():
                constraints_valid = GeoSimUtil.validate_sidedness(qgs_geoms_with_others, qgs_geom_polygonize)
            else:
                print("Polygonize not valid")
                constraints_valid = False

        return constraints_valid

    @staticmethod
    def init_process_line_stack(is_line_closed, qgs_points):
        """Method that initialize the stack used to simulate recursivity to simplify the line

        :param: is_closed: Boolean to indicate if the feature is closed or open
        :param: qgs_points: List of QgsPoints forming the line string to simplify
        :return: Stack used to initiate the line simplification process
        :rtype: List of tuple
        """

        stack = []
        last_index = len(qgs_points) - 1
        if is_line_closed:
            # Initialize stack for a closed line string
            if last_index >= 4:
                x = qgs_points[0].x()
                y = qgs_points[0].y()
                lst_distance = [qgs_point.distance(x, y) for qgs_point in qgs_points]
                mid_index = lst_distance.index(max(lst_distance))  # Most distant vertex position

                (farthest_index_a, farthest_dist_a) = Simplify.find_farthest_point(qgs_points, 0, mid_index)
                (farthest_index_b, farthest_dist_b) = Simplify.find_farthest_point(qgs_points, mid_index, last_index)
                if farthest_dist_a > 0.:
                    stack.append((0, farthest_index_a))
                    stack.append((farthest_index_a, mid_index))
                if farthest_dist_b > 0.:
                    stack.append((mid_index, farthest_index_b))
                    stack.append((farthest_index_b, last_index))
            else:
                # Not enough vertice... nothing to simplify
                pass
        else:
            # Initialize stack for an open line string
            stack.append((0, last_index))

        return stack

    def process_line(self, sim_geom):
        """This method is simplifying a line with the Douglas Peucker algorithm and spatial constraints.

        Important note: The line is always simplified for the end of the line to the start of the line. This helps
        maintain the relative position of the vertice in the line

        :param: sim_geom: GeoSim object to simplify
        :return: Number of vertice deleted
        :rtype: int
        """

        qgs_line_string = sim_geom.qgs_geom.constGet()
        qgs_points = qgs_line_string.points()

        # Initialize the stack that simulate recursivity
        stack = Simplify.init_process_line_stack(qgs_line_string.isClosed(), qgs_points)

        # Loop over the stack to simplify the line
        sim_geom.is_simplest = True
        nbr_vertice_deleted = 0
        while stack:
            (first, last) = stack.pop()
            if first + 1 < last:  # The segment to check has only 2 points
                (farthest_index, farthest_dist) = Simplify.find_farthest_point(qgs_points, first, last)
                if farthest_dist <= self.tolerance:
                    if self.validate_constraints(sim_geom, first, last):
                        nbr_vertice_deleted += last - first - 1
                        self.rb_collection.delete_vertex(sim_geom, first + 1, last - 1)
                    else:
                        sim_geom.is_simplest = False  # The line string is not at its simplest form
                        # In case of non respect of spatial constraints split and stack again the sub lines
                        (farthest_index, farthest_dist) = Simplify.find_farthest_point(qgs_points, first, last)
                        if farthest_dist <= self.tolerance:
                            # Stack for the net iteration
                            stack.append((first, farthest_index))
                            stack.append((farthest_index, last))
                else:
                    # Stack for the iteration
                    stack.append((first, farthest_index))
                    stack.append((farthest_index, last))

        return nbr_vertice_deleted
