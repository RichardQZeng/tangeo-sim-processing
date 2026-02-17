use std::f64::consts::PI;

use anyhow::Result;
use geos::Geometry;

use crate::geometry::{Coord, SimpleGeometry};

#[derive(Debug, Clone)]
pub struct Bend {
    pub i: usize,
    pub j: usize,
    pub area: f64,
    pub perimeter: f64,
    pub adj_area: f64,
    pub to_reduce: bool,
    pub old_subline_coords: Vec<Coord>,
    pub new_subline_coords: Vec<Coord>,
    pub bend_polygon_coords: Vec<Coord>,
}

impl Bend {
    pub fn calculate_min_adj_area(diameter_tol: f64) -> f64 {
        0.75 * PI * (diameter_tol / 2.0).powi(2)
    }

    pub fn calculate_adj_area(area: f64, perimeter: f64) -> f64 {
        if perimeter == 0.0 {
            return 0.0;
        }
        let compactness_index = 4.0 * area * PI / perimeter.powi(2);
        if compactness_index == 0.0 {
            return 0.0;
        }
        area * (0.75 / compactness_index)
    }

    pub fn new(i: usize, j: usize, points: Vec<Coord>) -> Self {
        let mut ring = points.clone();
        if !ring.is_empty() && ring.first() != ring.last() {
            ring.push(ring[0]);
        }
        let area = polygon_area(&ring);
        let perimeter = polyline_len(&ring);
        let adj_area = Self::calculate_adj_area(area, perimeter);
        let new_subline_coords = if points.len() >= 2 {
            vec![points[0], points[points.len() - 1]]
        } else {
            points.clone()
        };

        Self {
            i,
            j,
            area,
            perimeter,
            adj_area,
            to_reduce: false,
            old_subline_coords: points,
            new_subline_coords,
            bend_polygon_coords: ring,
        }
    }

    pub fn old_subline_geometry(&self) -> Result<Geometry> {
        SimpleGeometry::LineString(self.old_subline_coords.clone()).to_geos()
    }

    pub fn new_subline_geometry(&self) -> Result<Geometry> {
        SimpleGeometry::LineString(self.new_subline_coords.clone()).to_geos()
    }

    pub fn bend_polygon_geometry(&self) -> Result<Geometry> {
        SimpleGeometry::Polygon {
            outer: self.bend_polygon_coords.clone(),
            inners: Vec::new(),
        }
        .to_geos()
    }
}

fn polyline_len(coords: &[Coord]) -> f64 {
    coords
        .windows(2)
        .map(|w| {
            let dx = w[1].x - w[0].x;
            let dy = w[1].y - w[0].y;
            (dx * dx + dy * dy).sqrt()
        })
        .sum()
}

fn polygon_area(coords: &[Coord]) -> f64 {
    if coords.len() < 4 {
        return 0.0;
    }
    let mut area2 = 0.0;
    for w in coords.windows(2) {
        area2 += w[0].x * w[1].y - w[1].x * w[0].y;
    }
    area2.abs() * 0.5
}
