use std::collections::HashMap;

use anyhow::Result;
use geos::{Geom, Geometry};

use crate::constraints::{validate_intersection, validate_sidedness, validate_simplicity};
use crate::geometry::{Coord, RbGeom};
use crate::spatial_index::{bbox_for_coords, GsCollection};

use super::detection::angle_between_three_points;

#[derive(Debug, Clone)]
pub struct BendReduced {
    pub rb_geom_id: usize,
    pub i: usize,
    pub j: usize,
    pub start: Coord,
    pub end: Coord,
    pub bend_polygon_coords: Vec<Coord>,
}

pub fn manage_smooth_line(
    collection: &mut GsCollection,
    rb_geoms: &mut [RbGeom],
    rb_geom_index: &HashMap<usize, usize>,
    bends_reduced: &[BendReduced],
    diameter_tol: f64,
    zero_relative: f64,
) -> Result<usize> {
    let mut nbr_line_smooth = 0usize;

    for reduced in bends_reduced {
        if dist(reduced.start, reduced.end) <= diameter_tol * (2.0 / 3.0) {
            continue;
        }

        let Some(&idx) = rb_geom_index.get(&reduced.rb_geom_id) else {
            continue;
        };
        let Some(rb_geom) = rb_geoms.get_mut(idx) else {
            continue;
        };

        let i = extract_vertex_index(&rb_geom.coords, reduced.start, zero_relative);
        let j = extract_vertex_index(&rb_geom.coords, reduced.end, zero_relative);
        let (Some(i), Some(j)) = (i, j) else {
            continue;
        };
        if i + 1 != j {
            continue;
        }
        if i < 1 || j > rb_geom.coords.len().saturating_sub(2) {
            continue;
        }

        let Some(smooth_line_coords) =
            calculate_smooth_line(rb_geom, i, j, &reduced.bend_polygon_coords)
        else {
            continue;
        };

        let Some(smooth_polygon) = resolve_non_valid_polygon(&smooth_line_coords, zero_relative)?
        else {
            continue;
        };

        if validate_constraints_smooth(
            collection,
            rb_geom,
            reduced,
            &smooth_line_coords,
            &smooth_polygon,
            zero_relative,
        )? {
            collection.add_vertex(rb_geom, i, j, &smooth_line_coords)?;
            nbr_line_smooth += 1;
        }
    }

    Ok(nbr_line_smooth)
}

fn validate_constraints_smooth(
    collection: &GsCollection,
    rb_geom: &RbGeom,
    reduced: &BendReduced,
    smooth_line_coords: &[Coord],
    smooth_polygon: &Geometry,
    zero_relative: f64,
) -> Result<bool> {
    let old_subline =
        crate::geometry::SimpleGeometry::LineString(vec![reduced.start, reduced.end]).to_geos()?;
    let smooth_line =
        crate::geometry::SimpleGeometry::LineString(smooth_line_coords.to_vec()).to_geos()?;
    let bbox = bbox_for_coords(smooth_line_coords);

    let (with_itself, with_others) =
        collection.get_segment_intersect(rb_geom.id, bbox, &old_subline)?;

    if !validate_simplicity(&with_itself, &smooth_line, zero_relative)? {
        return Ok(false);
    }
    if !validate_intersection(&with_others, &smooth_line)? {
        return Ok(false);
    }
    validate_sidedness(&with_others, smooth_polygon)
}

fn calculate_smooth_line(
    rb_geom: &RbGeom,
    i: usize,
    j: usize,
    bend_polygon_coords: &[Coord],
) -> Option<Vec<Coord>> {
    if i == 0 || j + 1 >= rb_geom.coords.len() {
        return None;
    }

    let p0 = rb_geom.coords[i - 1];
    let p1 = rb_geom.coords[i];
    let p2 = rb_geom.coords[j];
    let p3 = rb_geom.coords[j + 1];

    let centroid = centroid_of_polygon(bend_polygon_coords)?;

    let t0 = translate(p0, p1);
    let t1 = translate(p1, p1);
    let t2 = translate(p2, p1);
    let t3 = translate(p3, p1);
    let tcent = translate(centroid, p1);

    let base_length = dist(t1, t2);
    if base_length == 0.0 {
        return None;
    }

    let angle_x_axis = (t2.y - t1.y).atan2(t2.x - t1.x);
    let r0 = rotate(t0, -angle_x_axis);
    let r1 = rotate(t1, -angle_x_axis);
    let r2 = rotate(t2, -angle_x_axis);
    let r3 = rotate(t3, -angle_x_axis);
    let rcent = rotate(tcent, -angle_x_axis);

    let base = r2.x;
    if base <= 0.0 {
        return None;
    }

    let p0_x = base * (1.0 / 3.0);
    let p1_x = base * (2.0 / 3.0);

    let smooth_case = if r0.y * r3.y > 0.0 {
        if r0.y * rcent.y < 0.0 {
            1
        } else {
            2
        }
    } else {
        3
    };

    let angle_i = angle_between_three_points(r0, r1, r2);
    let angle_j = angle_between_three_points(r1, r2, r3);
    let angle_smooth = calculate_angle(angle_i, angle_j, smooth_case);

    let mut p0_y = angle_smooth.tan() * p0_x;
    let (s0, s1) = if smooth_case == 1 || smooth_case == 2 {
        if r0.y > 0.0 {
            p0_y *= -1.0;
        }
        (Coord { x: p0_x, y: p0_y }, Coord { x: p1_x, y: p0_y })
    } else {
        if r0.y > 0.0 {
            p0_y *= -1.0;
        }
        let a = Coord { x: p0_x, y: p0_y };
        let b = Coord { x: p1_x, y: -p0_y };
        (a, b)
    };

    let local = vec![r1, s0, s1, r2];
    let mut world = local
        .into_iter()
        .map(|p| untranslate(rotate(p, angle_x_axis), p1))
        .collect::<Vec<_>>();

    world[0] = p1;
    let last = world.len() - 1;
    world[last] = p2;
    Some(world)
}

fn resolve_non_valid_polygon(
    smooth_line_coords: &[Coord],
    epsilon: f64,
) -> Result<Option<Geometry>> {
    if smooth_line_coords.len() < 3 {
        return Ok(None);
    }

    let mut ring = smooth_line_coords.to_vec();
    if ring.first() != ring.last() {
        ring.push(ring[0]);
    }

    let poly = crate::geometry::SimpleGeometry::Polygon {
        outer: ring.clone(),
        inners: Vec::new(),
    }
    .to_geos()?;

    if poly.is_valid() {
        return Ok(Some(poly));
    }

    let close_line = crate::geometry::SimpleGeometry::LineString(ring).to_geos()?;
    let unary = close_line.unary_union()?;
    let polygonized = Geometry::polygonize(&[unary])?;

    if polygonized.area()? <= epsilon {
        return Ok(None);
    }
    if !polygonized.is_valid() {
        return Ok(None);
    }

    Ok(Some(polygonized))
}

fn extract_vertex_index(coords: &[Coord], target: Coord, eps: f64) -> Option<usize> {
    coords.iter().enumerate().find_map(|(i, c)| {
        if dist(*c, target) <= eps {
            Some(i)
        } else {
            None
        }
    })
}

fn calculate_angle(mut angle_i: f64, mut angle_j: f64, smooth_case: i32) -> f64 {
    let pi = std::f64::consts::PI;
    if angle_i > pi {
        angle_i = (2.0 * pi) - angle_i;
    }
    if angle_j > pi {
        angle_j = (2.0 * pi) - angle_j;
    }

    let mut angle_smooth = pi - angle_i.max(angle_j);
    match smooth_case {
        1 => {
            angle_smooth /= 1.5;
            if angle_smooth.to_degrees() > 30.0 {
                angle_smooth = 30.0_f64.to_radians();
            }
        }
        2 => {
            angle_smooth /= 2.5;
            if angle_smooth.to_degrees() > 20.0 {
                angle_smooth = 20.0_f64.to_radians();
            }
        }
        _ => {
            angle_smooth /= 3.0;
            if angle_smooth.to_degrees() > 20.0 {
                angle_smooth = 20.0_f64.to_radians();
            }
        }
    }
    angle_smooth
}

fn centroid_of_polygon(coords: &[Coord]) -> Option<Coord> {
    if coords.len() < 3 {
        return None;
    }
    let mut ring = coords.to_vec();
    if ring.first() != ring.last() {
        ring.push(ring[0]);
    }

    let mut a = 0.0;
    let mut cx = 0.0;
    let mut cy = 0.0;
    for w in ring.windows(2) {
        let cross = w[0].x * w[1].y - w[1].x * w[0].y;
        a += cross;
        cx += (w[0].x + w[1].x) * cross;
        cy += (w[0].y + w[1].y) * cross;
    }
    a *= 0.5;
    if a.abs() < 1e-15 {
        let sx: f64 = ring.iter().map(|p| p.x).sum();
        let sy: f64 = ring.iter().map(|p| p.y).sum();
        let n = ring.len() as f64;
        return Some(Coord {
            x: sx / n,
            y: sy / n,
        });
    }

    Some(Coord {
        x: cx / (6.0 * a),
        y: cy / (6.0 * a),
    })
}

fn translate(p: Coord, origin: Coord) -> Coord {
    Coord {
        x: p.x - origin.x,
        y: p.y - origin.y,
    }
}

fn untranslate(p: Coord, origin: Coord) -> Coord {
    Coord {
        x: p.x + origin.x,
        y: p.y + origin.y,
    }
}

fn rotate(p: Coord, angle_rad: f64) -> Coord {
    let c = angle_rad.cos();
    let s = angle_rad.sin();
    Coord {
        x: p.x * c - p.y * s,
        y: p.x * s + p.y * c,
    }
}

fn dist(a: Coord, b: Coord) -> f64 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    (dx * dx + dy * dy).sqrt()
}

#[cfg(test)]
mod tests {
    use super::{calculate_angle, calculate_smooth_line, resolve_non_valid_polygon};
    use crate::geometry::{Coord, GeomType, RbGeom};

    fn c(x: f64, y: f64) -> Coord {
        Coord { x, y }
    }

    #[test]
    fn calculate_angle_respects_case_caps() {
        let angle_i = 0.1;
        let angle_j = 0.2;

        let a1 = calculate_angle(angle_i, angle_j, 1);
        let a2 = calculate_angle(angle_i, angle_j, 2);
        let a3 = calculate_angle(angle_i, angle_j, 3);

        assert!(a1 <= 30.0_f64.to_radians() + 1e-12);
        assert!(a2 <= 20.0_f64.to_radians() + 1e-12);
        assert!(a3 <= 20.0_f64.to_radians() + 1e-12);
    }

    #[test]
    fn calculate_smooth_line_builds_intermediate_points() {
        let rb = RbGeom {
            id: 1,
            original_type: GeomType::LineString,
            is_simplest: false,
            need_pivot: false,
            coords: vec![c(-1.0, 1.0), c(0.0, 0.0), c(3.0, 0.0), c(4.0, 1.0)],
        };
        let bend_polygon = vec![c(0.0, 0.0), c(1.5, 1.0), c(3.0, 0.0), c(0.0, 0.0)];

        let smooth = calculate_smooth_line(&rb, 1, 2, &bend_polygon)
            .expect("smooth line should be computed");
        assert_eq!(smooth.len(), 4);
        assert!((smooth[0].x - 0.0).abs() < 1e-9 && (smooth[0].y - 0.0).abs() < 1e-9);
        assert!((smooth[3].x - 3.0).abs() < 1e-9 && (smooth[3].y - 0.0).abs() < 1e-9);
    }

    #[test]
    fn resolve_non_valid_polygon_accepts_valid_smooth_line() {
        let smooth_line = vec![c(0.0, 0.0), c(1.0, 0.5), c(2.0, 0.5), c(3.0, 0.0)];
        let out =
            resolve_non_valid_polygon(&smooth_line, 1e-9).expect("polygon resolution should run");
        assert!(out.is_some());
    }
}
