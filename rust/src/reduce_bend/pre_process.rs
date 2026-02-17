use crate::geometry::{Coord, FeatureRecord, GeomType, RbGeom, SimpleGeometry};

use super::bend::Bend;

pub fn pre_filter_records(
    records: &[FeatureRecord],
    diameter_tol: f64,
    flag_del_outer: bool,
    flag_del_inner: bool,
) -> (Vec<FeatureRecord>, usize, usize) {
    let mut out = Vec::new();
    let mut nbr_pol_del = 0usize;
    let mut nbr_hole_del = 0usize;
    let min_adj_area = Bend::calculate_min_adj_area(diameter_tol);

    for rec in records {
        match &rec.geometry {
            SimpleGeometry::Polygon { outer, inners } => {
                let (outer_area, outer_perim) = polygon_metrics(outer);
                let outer_adj = Bend::calculate_adj_area(outer_area, outer_perim);
                if flag_del_outer && outer_adj < min_adj_area {
                    nbr_pol_del += 1;
                    continue;
                }

                let mut kept_holes = Vec::new();
                for hole in inners {
                    let (a, p) = polygon_metrics(hole);
                    let adj = Bend::calculate_adj_area(a, p);
                    if flag_del_inner && adj < min_adj_area {
                        nbr_hole_del += 1;
                    } else {
                        kept_holes.push(hole.clone());
                    }
                }

                let mut new_rec = rec.clone();
                new_rec.geometry = SimpleGeometry::Polygon {
                    outer: outer.clone(),
                    inners: kept_holes,
                };
                out.push(new_rec);
            }
            _ => out.push(rec.clone()),
        }
    }

    (out, nbr_pol_del, nbr_hole_del)
}

pub fn remove_duplicate_nodes(rb_geom: &mut RbGeom, zero_relative: f64) {
    if rb_geom.original_type == GeomType::Point || rb_geom.coords.len() < 2 {
        return;
    }

    let is_closed = rb_geom.is_closed();
    let mut dedup = Vec::with_capacity(rb_geom.coords.len());
    for c in &rb_geom.coords {
        let keep = dedup
            .last()
            .map(|p: &Coord| dist(*p, *c) > zero_relative)
            .unwrap_or(true);
        if keep {
            dedup.push(*c);
        }
    }

    if is_closed && dedup.len() >= 2 && dedup.first() != dedup.last() {
        dedup.push(dedup[0]);
    }

    rb_geom.coords = dedup;

    if rb_geom.is_closed() {
        if rb_geom.coords.len() < 4 {
            rb_geom.is_simplest = true;
        }
    } else if rb_geom.coords.len() < 2 {
        rb_geom.is_simplest = true;
    }
}

fn polygon_metrics(ring: &[Coord]) -> (f64, f64) {
    if ring.len() < 3 {
        return (0.0, 0.0);
    }
    let mut closed = ring.to_vec();
    if closed.first() != closed.last() {
        closed.push(closed[0]);
    }

    let mut area2 = 0.0;
    let mut perimeter = 0.0;
    for w in closed.windows(2) {
        area2 += w[0].x * w[1].y - w[1].x * w[0].y;
        perimeter += dist(w[0], w[1]);
    }
    (area2.abs() * 0.5, perimeter)
}

fn dist(a: Coord, b: Coord) -> f64 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    (dx * dx + dy * dy).sqrt()
}
