use anyhow::{anyhow, Result};

use crate::constraints::{validate_intersection, validate_sidedness, validate_simplicity};
use crate::epsilon::Epsilon;
use crate::geometry::{Coord, FeatureRecord, GsFeature, RbGeom, SimpleGeometry};
use crate::spatial_index::{bbox_for_coords, GsCollection};

#[derive(Debug, Clone, Copy)]
pub struct SimplifyParams {
    pub tolerance: f64,
    pub validate_structure: bool,
}

#[derive(Debug, Clone)]
pub struct SimplifyStats {
    pub in_nbr_features: usize,
    pub out_nbr_features: usize,
    pub nbr_vertice_deleted: usize,
    pub nbr_pass: usize,
    pub is_structure_valid: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct SimplifyOutput {
    pub features: Vec<FeatureRecord>,
    pub stats: SimplifyStats,
}

#[derive(Debug, Clone, Copy)]
pub struct SimplifyProgress {
    pub current_feature: usize,
    pub total_features: usize,
    pub current_pass: usize,
}

pub struct SimplifyEngine {
    tolerance: f64,
    validate_structure: bool,
    eps: Epsilon,
    collection: GsCollection,
    gs_features: Vec<GsFeature>,
    rb_geoms: Vec<RbGeom>,
}

impl SimplifyEngine {
    pub fn new(records: &[FeatureRecord], params: SimplifyParams) -> Result<Self> {
        let eps = Epsilon::from_features(records);
        let (gs_features, rb_geoms) = GsFeature::from_records(records, eps.zero_relative)?;

        let mut collection = GsCollection::new(eps.zero_relative);
        collection.add_features(&rb_geoms);

        Ok(Self {
            tolerance: params.tolerance,
            validate_structure: params.validate_structure,
            eps,
            collection,
            gs_features,
            rb_geoms,
        })
    }

    pub fn run(mut self) -> Result<SimplifyOutput> {
        self.run_with_progress(|_| {})
    }

    pub fn run_with_progress<F>(mut self, mut on_progress: F) -> Result<SimplifyOutput>
    where
        F: FnMut(SimplifyProgress),
    {
        let in_count = self.gs_features.len();
        let mut total_deleted = 0usize;
        let mut pass = 0usize;

        loop {
            pass += 1;
            let mut deleted_in_pass = 0usize;

            for i in 0..self.rb_geoms.len() {
                on_progress(SimplifyProgress {
                    current_feature: i + 1,
                    total_features: self.rb_geoms.len(),
                    current_pass: pass,
                });

                if !self.rb_geoms[i].is_simplest {
                    deleted_in_pass += self.process_line(i)?;
                }
            }

            if deleted_in_pass == 0 {
                break;
            }
            total_deleted += deleted_in_pass;
        }

        let mut out_features = self
            .gs_features
            .iter()
            .map(|f| f.rebuild_record(&self.rb_geoms))
            .collect::<Result<Vec<_>>>()?;

        out_features.sort_by_key(|f| (f.stable_id, f.fid.unwrap_or(u64::MAX)));

        let structure_valid = if self.validate_structure {
            Some(self.collection.validate_integrity(&self.rb_geoms)?)
        } else {
            None
        };

        let stats = SimplifyStats {
            in_nbr_features: in_count,
            out_nbr_features: out_features.len(),
            nbr_vertice_deleted: total_deleted,
            nbr_pass: pass,
            is_structure_valid: structure_valid,
        };

        Ok(SimplifyOutput {
            features: out_features,
            stats,
        })
    }

    fn process_line(&mut self, rb_geom_idx: usize) -> Result<usize> {
        let mut stack = init_process_line_stack(self.rb_geoms[rb_geom_idx].is_closed(), &self.rb_geoms[rb_geom_idx].coords);

        let mut deleted = 0usize;
        self.rb_geoms[rb_geom_idx].is_simplest = true;

        while let Some((first, last)) = stack.pop() {
            if first + 1 >= last {
                continue;
            }

            let coords_snapshot = self.rb_geoms[rb_geom_idx].coords.clone();
            if last >= coords_snapshot.len() {
                continue;
            }

            let (farthest_index, farthest_dist) = find_farthest_point(&coords_snapshot, first, last);

            if farthest_dist <= self.tolerance {
                if self.validate_constraints(rb_geom_idx, first, last)? {
                    deleted += last - first - 1;
                    let rb_geom = self
                        .rb_geoms
                        .get_mut(rb_geom_idx)
                        .ok_or_else(|| anyhow!("invalid rb geom index"))?;
                    self.collection.delete_vertex(rb_geom, first + 1, last - 1)?;
                } else {
                    self.rb_geoms[rb_geom_idx].is_simplest = false;
                    let now = &self.rb_geoms[rb_geom_idx].coords;
                    if first < now.len() && last < now.len() {
                        let (next_farthest, next_dist) = find_farthest_point(now, first, last);
                        if next_dist <= self.tolerance {
                            stack.push((first, next_farthest));
                            stack.push((next_farthest, last));
                        }
                    }
                }
            } else {
                stack.push((first, farthest_index));
                stack.push((farthest_index, last));
            }
        }

        Ok(deleted)
    }

    fn validate_constraints(&self, rb_geom_idx: usize, first: usize, last: usize) -> Result<bool> {
        let sim_geom = self
            .rb_geoms
            .get(rb_geom_idx)
            .ok_or_else(|| anyhow!("invalid rb geom index for constraint validation"))?;

        if last >= sim_geom.coords.len() {
            return Ok(false);
        }

        let sub_coords = sim_geom.coords[first..=last].to_vec();
        if sub_coords.len() < 2 {
            return Ok(false);
        }

        let new_subline = SimpleGeometry::LineString(vec![sub_coords[0], *sub_coords.last().unwrap()]).to_geos()?;
        let old_subline = SimpleGeometry::LineString(sub_coords.clone()).to_geos()?;
        let bbox = bbox_for_coords(&sub_coords);

        let (with_itself, with_others) = self
            .collection
            .get_segment_intersect(sim_geom.id, bbox, &old_subline)?;

        if !validate_simplicity(&with_itself, &new_subline, self.eps.zero_relative)? {
            return Ok(false);
        }

        if !with_others.is_empty() && !validate_intersection(&with_others, &new_subline)? {
            return Ok(false);
        }

        if !with_others.is_empty() {
            return validate_sidedness(&with_others, &sub_coords);
        }

        Ok(true)
    }
}

pub fn init_process_line_stack(is_line_closed: bool, points: &[Coord]) -> Vec<(usize, usize)> {
    let mut stack = Vec::new();
    if points.is_empty() {
        return stack;
    }

    let last_index = points.len() - 1;

    if is_line_closed {
        if last_index >= 4 {
            let origin = points[0];
            let mut max_d = f64::NEG_INFINITY;
            let mut mid_index = 0usize;
            for (idx, p) in points.iter().enumerate() {
                let dx = p.x - origin.x;
                let dy = p.y - origin.y;
                let d = (dx * dx + dy * dy).sqrt();
                if d > max_d {
                    max_d = d;
                    mid_index = idx;
                }
            }

            let (farthest_a, dist_a) = find_farthest_point(points, 0, mid_index);
            let (farthest_b, dist_b) = find_farthest_point(points, mid_index, last_index);

            if dist_a > 0.0 {
                stack.push((0, farthest_a));
                stack.push((farthest_a, mid_index));
            }
            if dist_b > 0.0 {
                stack.push((mid_index, farthest_b));
                stack.push((farthest_b, last_index));
            }
        }
    } else {
        stack.push((0, last_index));
    }

    stack
}

pub fn find_farthest_point(points: &[Coord], first: usize, last: usize) -> (usize, f64) {
    if last < first + 2 {
        return (first, -1.0);
    }

    let a = points[first];
    let b = points[last];
    let mut farthest_index = first;
    let mut farthest_dist = f64::NEG_INFINITY;

    for (idx, p) in points.iter().enumerate().take(last).skip(first + 1) {
        let d = point_to_segment_dist(*p, a, b);
        if d > farthest_dist {
            farthest_dist = d;
            farthest_index = idx;
        }
    }

    (farthest_index, farthest_dist)
}

pub fn point_to_segment_dist(p: Coord, a: Coord, b: Coord) -> f64 {
    let vx = b.x - a.x;
    let vy = b.y - a.y;
    let wx = p.x - a.x;
    let wy = p.y - a.y;

    let seg_len2 = vx * vx + vy * vy;
    if seg_len2 == 0.0 {
        let dx = p.x - a.x;
        let dy = p.y - a.y;
        return (dx * dx + dy * dy).sqrt();
    }

    let t = ((wx * vx) + (wy * vy)) / seg_len2;
    let t = t.clamp(0.0, 1.0);

    let proj_x = a.x + t * vx;
    let proj_y = a.y + t * vy;

    let dx = p.x - proj_x;
    let dy = p.y - proj_y;

    (dx * dx + dy * dy).sqrt()
}
