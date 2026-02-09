use anyhow::{anyhow, Result};
use geos::{Geom, Geometry};

use crate::geometry::{Coord, SimpleGeometry};
use crate::spatial_index::SegmentGeometry;

fn interior_boundary_intersects(new_subline: &Geometry, cand_geom: &Geometry) -> Result<bool> {
    let start = cand_geom.get_start_point().map_err(|e| anyhow!(e.to_string()))?;
    let end = cand_geom.get_end_point().map_err(|e| anyhow!(e.to_string()))?;

    let new_start = new_subline.get_start_point().map_err(|e| anyhow!(e.to_string()))?;
    let new_end = new_subline.get_end_point().map_err(|e| anyhow!(e.to_string()))?;

    for pt in [&start, &end] {
        if new_subline.intersects(pt).map_err(|e| anyhow!(e.to_string()))? {
            let at_start = pt.equals(&new_start).map_err(|e| anyhow!(e.to_string()))?;
            let at_end = pt.equals(&new_end).map_err(|e| anyhow!(e.to_string()))?;
            if !at_start && !at_end {
                return Ok(true);
            }
        }
    }

    Ok(false)
}

pub fn validate_simplicity(candidates: &[SegmentGeometry], new_subline: &Geometry, zero_relative: f64) -> Result<bool> {
    if new_subline.length().map_err(|e| anyhow!(e.to_string()))? <= zero_relative {
        return Ok(true);
    }

    for cand in candidates {
        let cand_geom = cand.to_geos()?;

        if new_subline.crosses(&cand_geom).map_err(|e| anyhow!(e.to_string()))? {
            return Ok(false);
        }

        if interior_boundary_intersects(new_subline, &cand_geom)? {
            return Ok(false);
        }
    }

    Ok(true)
}

pub fn validate_intersection(candidates: &[SegmentGeometry], new_subline: &Geometry) -> Result<bool> {
    for cand in candidates {
        let cand_geom = cand.to_geos()?;

        if new_subline.crosses(&cand_geom).map_err(|e| anyhow!(e.to_string()))? {
            return Ok(false);
        }
    }

    Ok(true)
}

pub fn validate_sidedness(candidates: &[SegmentGeometry], old_subline_coords: &[Coord]) -> Result<bool> {
    if old_subline_coords.len() < 3 {
        return Ok(true);
    }

    let mut ring = old_subline_coords.to_vec();
    if ring.first() != ring.last() {
        ring.push(ring[0]);
    }

    let bend_line = SimpleGeometry::LineString(ring).to_geos()?;

    let unary = bend_line.unary_union().map_err(|e| anyhow!(e.to_string()))?;
    let polygonized = Geometry::polygonize(&[unary]).map_err(|e| anyhow!(e.to_string()))?;

    if !polygonized.is_simple().map_err(|e| anyhow!(e.to_string()))? {
        return Ok(false);
    }

    for cand in candidates {
        let cand_geom = cand.to_geos()?;
        if polygonized
            .contains(&cand_geom)
            .map_err(|e| anyhow!(e.to_string()))?
        {
            return Ok(false);
        }
    }

    Ok(true)
}
