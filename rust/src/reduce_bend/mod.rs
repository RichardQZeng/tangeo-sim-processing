use std::collections::HashMap;

use anyhow::{anyhow, Result};

use crate::constraints::{validate_intersection, validate_sidedness, validate_simplicity};
use crate::epsilon::Epsilon;
use crate::geometry::{FeatureRecord, GsFeature, RbGeom};
use crate::spatial_index::{bbox_for_coords, GsCollection};

use self::alternate::try_validate_alternate_bend;
use self::bend::Bend;
use self::detection::{delete_co_linear, detect_bends, get_angles};
use self::flagging::flag_bend_to_reduce;
use self::pre_process::{pre_filter_records, remove_duplicate_nodes};
use self::smoothing::{manage_smooth_line, BendReduced};

pub mod alternate;
pub mod bend;
pub mod detection;
pub mod flagging;
pub mod pre_process;
pub mod smoothing;

#[derive(Debug, Clone, Copy)]
pub struct ReduceBendParams {
    pub diameter_tol: f64,
    pub smooth_line: bool,
    pub flag_del_outer: bool,
    pub flag_del_inner: bool,
    pub validate_structure: bool,
}

#[derive(Debug, Clone)]
pub struct ReduceBendStats {
    pub in_nbr_features: usize,
    pub out_nbr_features: usize,
    pub nbr_bend_reduced: usize,
    pub nbr_bend_detected: usize,
    pub nbr_hole_del: usize,
    pub nbr_pol_del: usize,
    pub nbr_line_smooth: usize,
    pub nbr_pass: usize,
    pub is_structure_valid: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct ReduceBendOutput {
    pub features: Vec<FeatureRecord>,
    pub stats: ReduceBendStats,
}

#[derive(Debug, Clone, Copy)]
pub struct ReduceBendProgress {
    pub current_feature: usize,
    pub total_features: usize,
    pub current_pass: usize,
}

pub struct ReduceBendEngine {
    params: ReduceBendParams,
    eps: Epsilon,
    collection: GsCollection,
    gs_features: Vec<GsFeature>,
    rb_geoms: Vec<RbGeom>,
    rb_geom_index: HashMap<usize, usize>,
    bend_map: HashMap<usize, Vec<Bend>>,
    bends_reduced: Vec<BendReduced>,
    nbr_hole_del: usize,
    nbr_pol_del: usize,
}

impl ReduceBendEngine {
    pub fn new(records: &[FeatureRecord], params: ReduceBendParams) -> Result<Self> {
        let eps = Epsilon::from_features(records);

        let (filtered, nbr_pol_del, nbr_hole_del) = pre_filter_records(
            records,
            params.diameter_tol,
            params.flag_del_outer,
            params.flag_del_inner,
        );

        let (gs_features, mut rb_geoms) = GsFeature::from_records(&filtered, eps.zero_relative)?;
        for rb in &mut rb_geoms {
            if !rb.is_simplest {
                remove_duplicate_nodes(rb, eps.zero_relative);
            }
        }

        let mut collection = GsCollection::new(eps.zero_relative);
        collection.add_features(&rb_geoms);

        let rb_geom_index = rb_geoms
            .iter()
            .enumerate()
            .map(|(idx, g)| (g.id, idx))
            .collect::<HashMap<_, _>>();

        Ok(Self {
            params,
            eps,
            collection,
            gs_features,
            rb_geoms,
            rb_geom_index,
            bend_map: HashMap::new(),
            bends_reduced: Vec::new(),
            nbr_hole_del,
            nbr_pol_del,
        })
    }

    pub fn run(self) -> Result<ReduceBendOutput> {
        self.run_with_progress(|_| {})
    }

    pub fn run_with_progress<F>(mut self, mut on_progress: F) -> Result<ReduceBendOutput>
    where
        F: FnMut(ReduceBendProgress),
    {
        let in_count = self.gs_features.len();

        let mut stats = ReduceBendStats {
            in_nbr_features: in_count,
            out_nbr_features: 0,
            nbr_bend_reduced: 0,
            nbr_bend_detected: 0,
            nbr_hole_del: self.nbr_hole_del,
            nbr_pol_del: self.nbr_pol_del,
            nbr_line_smooth: 0,
            nbr_pass: 0,
            is_structure_valid: None,
        };

        loop {
            stats.nbr_pass += 1;
            let mut nbr_bend_reduced = 0usize;
            let mut nbr_bend_detected = 0usize;
            let total_features = self.rb_geoms.len();

            for idx in 0..self.rb_geoms.len() {
                on_progress(ReduceBendProgress {
                    current_feature: idx + 1,
                    total_features,
                    current_pass: stats.nbr_pass,
                });

                if self.rb_geoms[idx].is_simplest {
                    continue;
                }

                let geom_id = self.rb_geoms[idx].id;
                if !self.bend_map.contains_key(&geom_id) {
                    let mut angles = get_angles(&self.rb_geoms[idx], self.eps.zero_angle);
                    delete_co_linear(
                        &mut self.collection,
                        &mut self.rb_geoms[idx],
                        &mut angles,
                        self.eps.zero_relative,
                    )?;
                    let bends = detect_bends(&angles, &mut self.rb_geoms[idx]);
                    nbr_bend_detected += bends.len();
                    let mut bends = bends;
                    flag_bend_to_reduce(&mut bends, &self.rb_geoms[idx], self.params.diameter_tol)?;
                    self.bend_map.insert(geom_id, bends);
                }

                nbr_bend_reduced += self.process_bends(idx)?;
            }

            stats.nbr_bend_reduced += nbr_bend_reduced;
            if stats.nbr_pass == 1 {
                stats.nbr_bend_detected = nbr_bend_detected;
            }

            if nbr_bend_reduced == 0 {
                break;
            }
        }

        if self.params.smooth_line {
            stats.nbr_line_smooth = manage_smooth_line(
                &mut self.collection,
                &mut self.rb_geoms,
                &self.rb_geom_index,
                &self.bends_reduced,
                self.params.diameter_tol,
                self.eps.zero_relative,
            )?;
        }

        let mut out_features = self
            .gs_features
            .iter()
            .map(|f| f.rebuild_record(&self.rb_geoms))
            .collect::<Result<Vec<_>>>()?;

        out_features.sort_by_key(|f| (f.stable_id, f.fid.unwrap_or(u64::MAX)));

        if self.params.validate_structure {
            stats.is_structure_valid = Some(self.collection.validate_integrity(&self.rb_geoms)?);
        }
        stats.out_nbr_features = out_features.len();

        Ok(ReduceBendOutput {
            features: out_features,
            stats,
        })
    }

    fn process_bends(&mut self, rb_geom_idx: usize) -> Result<usize> {
        let mut nbr_bend_reduced = 0usize;
        let geom_id = self.rb_geoms[rb_geom_idx].id;
        let Some(bends_snapshot) = self.bend_map.get(&geom_id).cloned() else {
            return Ok(0);
        };
        if bends_snapshot.is_empty() {
            self.rb_geoms[rb_geom_idx].is_simplest = true;
            return Ok(0);
        }

        let is_closed = self.rb_geoms[rb_geom_idx].is_closed();

        let mut indices = Vec::new();
        if is_closed && bends_snapshot.len() >= 2 {
            indices.extend((1..(bends_snapshot.len() - 1)).rev());
        } else {
            indices.extend((0..bends_snapshot.len()).rev());
        }

        for bend_idx in indices {
            let to_reduce = self
                .bend_map
                .get(&geom_id)
                .and_then(|b| b.get(bend_idx))
                .map(|b| b.to_reduce)
                .unwrap_or(false);
            if !to_reduce {
                continue;
            }

            if self.validate_constraints(rb_geom_idx, bend_idx)? {
                let bend = self
                    .bend_map
                    .get(&geom_id)
                    .and_then(|b| b.get(bend_idx))
                    .cloned()
                    .ok_or_else(|| anyhow!("missing bend after validation"))?;
                nbr_bend_reduced += 1;

                let start = self.rb_geoms[rb_geom_idx]
                    .coords
                    .get(bend.i)
                    .copied()
                    .ok_or_else(|| anyhow!("bend start index out of bounds"))?;
                let end = self.rb_geoms[rb_geom_idx]
                    .coords
                    .get(bend.j)
                    .copied()
                    .ok_or_else(|| anyhow!("bend end index out of bounds"))?;
                self.bends_reduced.push(BendReduced {
                    rb_geom_id: geom_id,
                    i: bend.i,
                    j: bend.j,
                    start,
                    end,
                    bend_polygon_coords: bend.bend_polygon_coords.clone(),
                });

                let v_start = bend.i + 1;
                let v_end = bend.j.wrapping_sub(1);
                self.collection
                    .delete_vertex(&mut self.rb_geoms[rb_geom_idx], v_start, v_end)?;
            }
        }

        if is_closed && nbr_bend_reduced == 0 && bends_snapshot.len() >= 1 {
            for bend_idx in [0usize, bends_snapshot.len() - 1] {
                let to_reduce = self
                    .bend_map
                    .get(&geom_id)
                    .and_then(|b| b.get(bend_idx))
                    .map(|b| b.to_reduce)
                    .unwrap_or(false);
                if !to_reduce {
                    continue;
                }
                if self.validate_constraints(rb_geom_idx, bend_idx)? {
                    let bend = self
                        .bend_map
                        .get(&geom_id)
                        .and_then(|b| b.get(bend_idx))
                        .cloned()
                        .ok_or_else(|| anyhow!("missing bend after validation"))?;
                    nbr_bend_reduced += 1;

                    let start = self.rb_geoms[rb_geom_idx]
                        .coords
                        .get(bend.i)
                        .copied()
                        .ok_or_else(|| anyhow!("bend start index out of bounds"))?;
                    let end = self.rb_geoms[rb_geom_idx]
                        .coords
                        .get(bend.j)
                        .copied()
                        .ok_or_else(|| anyhow!("bend end index out of bounds"))?;
                    self.bends_reduced.push(BendReduced {
                        rb_geom_id: geom_id,
                        i: bend.i,
                        j: bend.j,
                        start,
                        end,
                        bend_polygon_coords: bend.bend_polygon_coords.clone(),
                    });

                    let v_start = bend.i + 1;
                    let v_end = bend.j.wrapping_sub(1);
                    self.collection.delete_vertex(
                        &mut self.rb_geoms[rb_geom_idx],
                        v_start,
                        v_end,
                    )?;
                    break;
                }
            }
        }

        if nbr_bend_reduced > 0 {
            self.bend_map.remove(&geom_id);
        }

        Ok(nbr_bend_reduced)
    }

    fn validate_constraints(&mut self, rb_geom_idx: usize, bend_idx: usize) -> Result<bool> {
        let geom_id = self.rb_geoms[rb_geom_idx].id;
        let bend = self
            .bend_map
            .get(&geom_id)
            .and_then(|b| b.get(bend_idx))
            .cloned()
            .ok_or_else(|| anyhow!("missing bend to validate"))?;

        let old_subline = bend.old_subline_geometry()?;
        let new_subline = bend.new_subline_geometry()?;
        let bend_polygon = bend.bend_polygon_geometry()?;
        let bbox = bbox_for_coords(&bend.old_subline_coords);

        let (with_itself, with_others) = self.collection.get_segment_intersect(
            self.rb_geoms[rb_geom_idx].id,
            bbox,
            &old_subline,
        )?;

        let mut constraints_valid =
            validate_simplicity(&with_itself, &new_subline, self.eps.zero_relative)?;
        if !constraints_valid {
            constraints_valid = try_validate_alternate_bend(
                &self.collection,
                &self.rb_geoms[rb_geom_idx],
                &mut self.bend_map,
                bend_idx,
                self.eps.zero_relative,
            )?;
        }

        if constraints_valid {
            constraints_valid = validate_intersection(&with_others, &new_subline)?;
        }
        if constraints_valid {
            constraints_valid = validate_sidedness(&with_others, &bend_polygon)?;
        }

        Ok(constraints_valid)
    }
}
