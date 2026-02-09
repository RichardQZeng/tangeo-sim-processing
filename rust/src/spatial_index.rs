use anyhow::{anyhow, Result};
use geos::{Geom, Geometry};
use rstar::{AABB, PointDistance, RTree, RTreeObject};

use crate::geometry::{Coord, GeomType, RbGeom, SimpleGeometry};

#[derive(Debug, Clone)]
pub enum SegmentGeometry {
    Segment(Coord, Coord),
    Point(Coord),
}

impl SegmentGeometry {
    pub fn to_simple_geometry(&self) -> SimpleGeometry {
        match self {
            Self::Segment(a, b) => SimpleGeometry::LineString(vec![*a, *b]),
            Self::Point(p) => SimpleGeometry::Point(*p),
        }
    }

    pub fn to_geos(&self) -> Result<Geometry> {
        self.to_simple_geometry().to_geos()
    }

    pub fn bbox(&self, grow: f64) -> AABB<[f64; 2]> {
        match self {
            Self::Point(p) => AABB::from_corners([p.x - grow, p.y - grow], [p.x + grow, p.y + grow]),
            Self::Segment(a, b) => {
                let min_x = a.x.min(b.x) - grow;
                let min_y = a.y.min(b.y) - grow;
                let max_x = a.x.max(b.x) + grow;
                let max_y = a.y.max(b.y) + grow;
                AABB::from_corners([min_x, min_y], [max_x, max_y])
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct SegmentEntry {
    segment_id: usize,
    owner_geom_id: usize,
    envelope: AABB<[f64; 2]>,
}

impl RTreeObject for SegmentEntry {
    type Envelope = AABB<[f64; 2]>;

    fn envelope(&self) -> Self::Envelope {
        self.envelope
    }
}

impl PointDistance for SegmentEntry {
    fn distance_2(&self, point: &[f64; 2]) -> f64 {
        self.envelope.distance_2(point)
    }
}

#[derive(Debug, Clone)]
struct SegmentRecord {
    owner_geom_id: usize,
    geom: SegmentGeometry,
}

#[derive(Debug)]
pub struct GsCollection {
    rtree: RTree<SegmentEntry>,
    records: Vec<Option<SegmentRecord>>,
    next_segment_id: usize,
    zero_relative: f64,
}

impl GsCollection {
    pub fn new(zero_relative: f64) -> Self {
        Self {
            rtree: RTree::new(),
            records: Vec::new(),
            next_segment_id: 1,
            zero_relative,
        }
    }

    pub fn add_features(&mut self, rb_geoms: &[RbGeom]) {
        for rb_geom in rb_geoms {
            match rb_geom.original_type {
                GeomType::Point => {
                    if let Some(p) = rb_geom.coords.first() {
                        self.insert_record(rb_geom.id, SegmentGeometry::Point(*p));
                    }
                }
                _ => {
                    for pair in rb_geom.coords.windows(2) {
                        self.insert_record(rb_geom.id, SegmentGeometry::Segment(pair[0], pair[1]));
                    }
                }
            }
        }
    }

    fn insert_record(&mut self, owner_geom_id: usize, geom: SegmentGeometry) {
        let segment_id = self.next_segment_id;
        self.next_segment_id += 1;

        let envelope = geom.bbox(0.0);
        let record = SegmentRecord { owner_geom_id, geom };
        if self.records.len() <= segment_id {
            self.records.resize_with(segment_id + 1, || None);
        }
        self.records[segment_id] = Some(record);
        self.rtree.insert(SegmentEntry {
            segment_id,
            owner_geom_id,
            envelope,
        });
    }

    pub fn get_segment_intersect(
        &self,
        owner_geom_id: usize,
        subline_bbox: AABB<[f64; 2]>,
        old_subline: &Geometry,
    ) -> Result<(Vec<SegmentGeometry>, Vec<SegmentGeometry>)> {
        let grow = self.zero_relative * 100.0;
        let envelope = AABB::from_corners(
            [subline_bbox.lower()[0] - grow, subline_bbox.lower()[1] - grow],
            [subline_bbox.upper()[0] + grow, subline_bbox.upper()[1] + grow],
        );

        let mut with_itself = Vec::new();
        let mut with_others = Vec::new();

        for candidate in self.rtree.locate_in_envelope_intersecting(&envelope) {
            let Some(record) = self.records.get(candidate.segment_id).and_then(|r| r.as_ref()) else {
                continue;
            };

            if record.owner_geom_id == owner_geom_id {
                let g = record.geom.to_geos()?;
                if !g.within(old_subline).map_err(|e| anyhow!(e.to_string()))? {
                    with_itself.push(record.geom.clone());
                }
            } else {
                with_others.push(record.geom.clone());
            }
        }

        Ok((with_itself, with_others))
    }

    fn delete_segment(&mut self, owner_geom_id: usize, p0: Coord, p1: Coord) -> Result<()> {
        let target = SegmentGeometry::Segment(p0, p1);
        let mid_x = (p0.x + p1.x) * 0.5;
        let mid_y = (p0.y + p1.y) * 0.5;
        let grow = self.zero_relative * 100.0;
        let envelope = AABB::from_corners([mid_x - grow, mid_y - grow], [mid_x + grow, mid_y + grow]);

        let mut found_entry: Option<SegmentEntry> = None;
        for candidate in self.rtree.locate_in_envelope_intersecting(&envelope) {
            let Some(record) = self.records.get(candidate.segment_id).and_then(|r| r.as_ref()) else {
                continue;
            };
            if record.owner_geom_id != owner_geom_id {
                continue;
            }

            if segments_equal(&record.geom, &target, self.zero_relative)? {
                found_entry = Some(candidate.clone());
                break;
            }
        }

        let entry = found_entry.ok_or_else(|| anyhow!("segment not found for deletion"))?;
        self.records[entry.segment_id] = None;
        let removed = self.rtree.remove(&entry);
        if removed.is_none() {
            return Err(anyhow!("internal index corruption while deleting segment"));
        }

        Ok(())
    }

    fn _delete_vertex(&mut self, rb_geom: &mut RbGeom, v_id_start: usize, v_id_end: usize) -> Result<()> {
        let is_closed = rb_geom.is_closed();
        let mut ids_to_delete: Vec<usize> = (v_id_start..=v_id_end).collect();
        if v_id_start == 0 && is_closed {
            let n = rb_geom.coords.len();
            ids_to_delete.insert(0, n - 2);
        } else {
            ids_to_delete.insert(0, v_id_start - 1);
        }
        ids_to_delete.push(v_id_end + 1);

        for pair in ids_to_delete.windows(2) {
            let p0 = rb_geom.coords[pair[0]];
            let p1 = rb_geom.coords[pair[1]];
            self.delete_segment(rb_geom.id, p0, p1)?;
        }

        let p0 = rb_geom.coords[ids_to_delete[0]];
        let p1 = rb_geom.coords[*ids_to_delete.last().unwrap_or(&ids_to_delete[0])];
        self.insert_record(rb_geom.id, SegmentGeometry::Segment(p0, p1));

        for idx in (v_id_start..=v_id_end).rev() {
            rb_geom.coords.remove(idx);
            if v_id_start == 0 && is_closed {
                let first = rb_geom.coords[0];
                let n = rb_geom.coords.len();
                rb_geom.coords.insert(n - 1, first);
                rb_geom.coords.remove(n);
            }
        }

        Ok(())
    }

    pub fn delete_vertex(&mut self, rb_geom: &mut RbGeom, mut v_id_start: usize, mut v_id_end: usize) -> Result<()> {
        let num_points = rb_geom.coords.len();
        if v_id_start == num_points - 1 {
            v_id_start = 0;
        }
        if num_points > 1 && v_id_end == usize::MAX {
            v_id_end = num_points - 2;
        }

        if v_id_start <= v_id_end {
            self._delete_vertex(rb_geom, v_id_start, v_id_end)
        } else {
            self._delete_vertex(rb_geom, v_id_start, num_points - 2)?;
            self._delete_vertex(rb_geom, 0, 0)?;
            if v_id_end > 0 {
                self._delete_vertex(rb_geom, 1, v_id_end)?;
            }
            Ok(())
        }
    }

    pub fn validate_integrity(&self, rb_geoms: &[RbGeom]) -> Result<bool> {
        for rb_geom in rb_geoms {
            if rb_geom.original_type == GeomType::Point {
                continue;
            }
            for pair in rb_geom.coords.windows(2) {
                let target = SegmentGeometry::Segment(pair[0], pair[1]);
                let bbox = target.bbox(self.zero_relative * 100.0);
                let mut found = false;
                for candidate in self.rtree.locate_in_envelope_intersecting(&bbox) {
                    let Some(record) = self.records.get(candidate.segment_id).and_then(|r| r.as_ref()) else {
                        continue;
                    };
                    if record.owner_geom_id == rb_geom.id
                        && segments_equal(&record.geom, &target, self.zero_relative)?
                    {
                        found = true;
                        break;
                    }
                }
                if !found {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }
}

fn segments_equal(a: &SegmentGeometry, b: &SegmentGeometry, eps: f64) -> Result<bool> {
    let ga = a.to_geos()?;
    let gb = b.to_geos()?;

    if ga.equals(&gb).map_err(|e| anyhow!(e.to_string()))? {
        return Ok(true);
    }

    let d = ga.hausdorff_distance(&gb).map_err(|e| anyhow!(e.to_string()))?;
    Ok(d <= eps)
}

pub fn bbox_for_coords(coords: &[Coord]) -> AABB<[f64; 2]> {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;

    for c in coords {
        min_x = min_x.min(c.x);
        min_y = min_y.min(c.y);
        max_x = max_x.max(c.x);
        max_y = max_y.max(c.y);
    }

    if !min_x.is_finite() {
        return AABB::from_corners([0.0, 0.0], [0.0, 0.0]);
    }

    AABB::from_corners([min_x, min_y], [max_x, max_y])
}
