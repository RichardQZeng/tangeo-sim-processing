use std::collections::HashMap;

use anyhow::{anyhow, Result};

use crate::constraints::validate_simplicity;
use crate::geometry::{Coord, RbGeom};
use crate::spatial_index::{bbox_for_coords, GsCollection};

use super::bend::Bend;

pub fn try_validate_alternate_bend(
    collection: &GsCollection,
    rb_geom: &RbGeom,
    bend_map: &mut HashMap<usize, Vec<Bend>>,
    ind: usize,
    zero_relative: f64,
) -> Result<bool> {
    let geom_id = rb_geom.id;
    let bends = bend_map
        .get(&geom_id)
        .ok_or_else(|| anyhow!("missing bend list for alternate validation"))?;
    if ind >= bends.len() {
        return Ok(false);
    }

    let base_bend = bends[ind].clone();
    let alternate_bends = find_alternate_bends(&base_bend, rb_geom);
    for alt in alternate_bends {
        let old = alt.old_subline_geometry()?;
        let new_subline = alt.new_subline_geometry()?;
        let bbox = bbox_for_coords(&alt.old_subline_coords);
        let (with_itself, _) = collection.get_segment_intersect(rb_geom.id, bbox, &old)?;

        if validate_simplicity(&with_itself, &new_subline, zero_relative)? {
            if let Some(bends_mut) = bend_map.get_mut(&geom_id) {
                bends_mut[ind] = alt;
                return Ok(true);
            }
        }
    }

    Ok(false)
}

fn find_alternate_bends(bend: &Bend, rb_geom: &RbGeom) -> Vec<Bend> {
    let mut out = Vec::new();
    let points = &rb_geom.coords;

    let mut j = bend.j;
    while j > 1 {
        let mut i = bend.i;
        while j >= i + 2 {
            let sub = extract_subline(points, i, j, rb_geom.is_closed());
            if sub.len() >= 2 {
                out.push((
                    Bend::new(i, j, sub).area,
                    Bend::new(i, j, extract_subline(points, i, j, rb_geom.is_closed())),
                ));
            }
            i += 1;
        }
        j -= 1;
    }
    out.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    out.into_iter().map(|(_, bend)| bend).collect()
}

fn extract_subline(points: &[Coord], start: usize, end: usize, is_closed: bool) -> Vec<Coord> {
    if points.is_empty() {
        return Vec::new();
    }
    if !is_closed {
        if start <= end && end < points.len() {
            return points[start..=end].to_vec();
        }
        return Vec::new();
    }

    if start < end {
        points[start..=end].to_vec()
    } else if start > end {
        let mut out = points[start..].to_vec();
        out.extend_from_slice(&points[..=end]);
        out
    } else {
        points[..points.len().saturating_sub(1)].to_vec()
    }
}
