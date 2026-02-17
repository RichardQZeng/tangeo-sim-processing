use std::f64::consts::PI;

use anyhow::Result;

use crate::geometry::{Coord, RbGeom};
use crate::spatial_index::GsCollection;

use super::bend::Bend;

pub const ANTI_CLOCK_WISE: i8 = -1;
pub const CLOCK_WISE: i8 = 1;
pub const FLAT_ANGLE: i8 = 0;

#[derive(Debug, Clone, Copy)]
struct Inflexion {
    start: usize,
    end: usize,
}

pub fn angle_between_three_points(a: Coord, b: Coord, c: Coord) -> f64 {
    let angle1 = (a.y - b.y).atan2(a.x - b.x);
    let angle2 = (c.y - b.y).atan2(c.x - b.x);
    let mut angle = angle1 - angle2;
    if angle < 0.0 {
        angle += 2.0 * PI;
    }
    angle
}

pub fn get_angles(rb_geom: &RbGeom, zero_angle: f64) -> Vec<i8> {
    let mut xy = rb_geom.coords.clone();
    if xy.len() < 3 {
        return Vec::new();
    }

    if rb_geom.is_closed() && xy.len() >= 2 {
        let pivot = xy[xy.len() - 2];
        xy.insert(0, pivot);
    }

    let mut out = Vec::new();
    for i in 1..(xy.len() - 1) {
        let angle = angle_between_three_points(xy[i - 1], xy[i], xy[i + 1]);
        if (angle - PI).abs() <= zero_angle || angle.abs() <= zero_angle {
            out.push(FLAT_ANGLE);
        } else if angle >= PI {
            out.push(CLOCK_WISE);
        } else {
            out.push(ANTI_CLOCK_WISE);
        }
    }

    out
}

pub fn delete_co_linear(
    collection: &mut GsCollection,
    rb_geom: &mut RbGeom,
    angles: &mut Vec<i8>,
    zero_relative: f64,
) -> Result<()> {
    let is_closed = rb_geom.is_closed();
    let mut num_points = rb_geom.coords.len();

    for i in (0..angles.len()).rev() {
        if angles[i] != FLAT_ANGLE {
            continue;
        }
        num_points -= 1;
        if is_closed && num_points <= 3 {
            rb_geom.is_simplest = true;
            break;
        }

        if is_closed {
            collection.delete_vertex(rb_geom, i, i)?;
        } else {
            collection.delete_vertex(rb_geom, i + 1, i + 1)?;
        }
        angles.remove(i);
    }

    if !is_closed && !angles.is_empty() {
        let first = angles[0];
        let last = angles[angles.len() - 1];
        angles.insert(0, first);
        angles.push(last);
    }

    if rb_geom.length_2d() <= zero_relative {
        rb_geom.is_simplest = true;
    }

    Ok(())
}

pub fn detect_bends(angles: &[i8], rb_geom: &mut RbGeom) -> Vec<Bend> {
    if angles.is_empty() {
        rb_geom.is_simplest = true;
        return Vec::new();
    }

    let mut inflexions = vec![Inflexion { start: 0, end: 0 }];
    for k in 0..(angles.len().saturating_sub(1)) {
        if angles[k] * angles[k + 1] == -1 {
            inflexions.last_mut().map(|i| i.end = k + 1);
            inflexions.push(Inflexion {
                start: k,
                end: k + 1,
            });
        }
    }

    let points = &rb_geom.coords;
    if rb_geom.is_closed() {
        if angles[angles.len() - 1] * angles[0] == -1 {
            inflexions.last_mut().map(|i| i.end = 0);
            inflexions.push(Inflexion {
                start: angles.len() - 1,
                end: 0,
            });
        }
        if let Some(last) = inflexions.last().copied() {
            inflexions[0].start = last.start;
            inflexions.pop();
        }
    } else {
        inflexions[0].start = 0;
        if let Some(last) = inflexions.last_mut() {
            last.end = points.len().saturating_sub(1);
        }
    }

    let mut bends = Vec::new();
    for inf in inflexions {
        let sub = extract_subline(points, inf.start, inf.end, rb_geom.is_closed());
        if sub.len() >= 2 {
            bends.push(Bend::new(inf.start, inf.end, sub));
        }
    }

    bends
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
