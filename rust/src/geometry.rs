use anyhow::{anyhow, Result};
use geos::Geometry;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coord {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone)]
pub enum FieldValue {
    Integer(i64),
    Integer64(i64),
    Real(f64),
    Text(String),
    Null,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeomType {
    Point,
    LineString,
    Polygon,
}

#[derive(Debug, Clone)]
pub enum SimpleGeometry {
    Point(Coord),
    LineString(Vec<Coord>),
    Polygon {
        outer: Vec<Coord>,
        inners: Vec<Vec<Coord>>,
    },
}

impl SimpleGeometry {
    pub fn geom_type(&self) -> GeomType {
        match self {
            Self::Point(_) => GeomType::Point,
            Self::LineString(_) => GeomType::LineString,
            Self::Polygon { .. } => GeomType::Polygon,
        }
    }

    pub fn all_coords(&self) -> Vec<Coord> {
        match self {
            Self::Point(c) => vec![*c],
            Self::LineString(coords) => coords.clone(),
            Self::Polygon { outer, inners } => {
                let mut out = outer.clone();
                for ring in inners {
                    out.extend_from_slice(ring);
                }
                out
            }
        }
    }

    pub fn to_wkt(&self) -> String {
        match self {
            Self::Point(c) => format!("POINT({} {})", fmt(c.x), fmt(c.y)),
            Self::LineString(coords) => {
                let part = coords
                    .iter()
                    .map(|c| format!("{} {}", fmt(c.x), fmt(c.y)))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("LINESTRING({part})")
            }
            Self::Polygon { outer, inners } => {
                let mut rings = vec![ring_wkt(outer)];
                rings.extend(inners.iter().map(|ring| ring_wkt(ring)));
                format!("POLYGON({})", rings.join(", "))
            }
        }
    }

    pub fn to_geos(&self) -> Result<Geometry> {
        Geometry::new_from_wkt(&self.to_wkt()).map_err(|e| anyhow!(e.to_string()))
    }
}

fn ring_wkt(coords: &[Coord]) -> String {
    let mut local = coords.to_vec();
    if !local.is_empty() && local.first() != local.last() {
        local.push(local[0]);
    }

    let part = local
        .iter()
        .map(|c| format!("{} {}", fmt(c.x), fmt(c.y)))
        .collect::<Vec<_>>()
        .join(", ");
    format!("({part})")
}

fn fmt(v: f64) -> String {
    let mut s = format!("{v:.15}");
    while s.contains('.') && s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') {
        s.pop();
    }
    if s.is_empty() {
        "0".to_string()
    } else {
        s
    }
}

#[derive(Debug, Clone)]
pub struct FeatureRecord {
    pub stable_id: usize,
    pub fid: Option<u64>,
    pub attrs: Vec<FieldValue>,
    pub geometry: SimpleGeometry,
}

#[derive(Debug, Clone)]
pub struct RbGeom {
    pub id: usize,
    pub original_type: GeomType,
    pub is_simplest: bool,
    pub need_pivot: bool,
    pub coords: Vec<Coord>,
}

impl RbGeom {
    pub fn is_closed(&self) -> bool {
        self.coords.len() >= 2 && self.coords.first() == self.coords.last()
    }

    pub fn length_2d(&self) -> f64 {
        if self.coords.len() < 2 {
            return 0.0;
        }

        self.coords
            .windows(2)
            .map(|w| {
                let dx = w[1].x - w[0].x;
                let dy = w[1].y - w[0].y;
                (dx * dx + dy * dy).sqrt()
            })
            .sum()
    }

    pub fn to_simple_geometry(&self) -> SimpleGeometry {
        match self.original_type {
            GeomType::Point => SimpleGeometry::Point(self.coords[0]),
            _ => SimpleGeometry::LineString(self.coords.clone()),
        }
    }
}

#[derive(Debug, Clone)]
pub enum GsFeature {
    Polygon {
        stable_id: usize,
        fid: Option<u64>,
        attrs: Vec<FieldValue>,
        rb_geom_ids: Vec<usize>,
    },
    LineString {
        stable_id: usize,
        fid: Option<u64>,
        attrs: Vec<FieldValue>,
        rb_geom_id: usize,
    },
    Point {
        stable_id: usize,
        fid: Option<u64>,
        attrs: Vec<FieldValue>,
        rb_geom_id: usize,
    },
}

impl GsFeature {
    pub fn stable_id(&self) -> usize {
        match self {
            Self::Polygon { stable_id, .. } => *stable_id,
            Self::LineString { stable_id, .. } => *stable_id,
            Self::Point { stable_id, .. } => *stable_id,
        }
    }

    pub fn from_records(records: &[FeatureRecord], zero_relative: f64) -> Result<(Vec<Self>, Vec<RbGeom>)> {
        let mut features = Vec::new();
        let mut rb_geoms = Vec::new();
        let mut rb_id = 1usize;

        for rec in records {
            match &rec.geometry {
                SimpleGeometry::Polygon { outer, inners } => {
                    let mut ring_ids = Vec::new();
                    for ring in std::iter::once(outer).chain(inners.iter()) {
                        let mut rb = RbGeom {
                            id: rb_id,
                            original_type: GeomType::Polygon,
                            is_simplest: false,
                            need_pivot: false,
                            coords: ring.clone(),
                        };
                        setup_rb_geom_flags(&mut rb, zero_relative);
                        rb_geoms.push(rb);
                        ring_ids.push(rb_id);
                        rb_id += 1;
                    }

                    features.push(Self::Polygon {
                        stable_id: rec.stable_id,
                        fid: rec.fid,
                        attrs: rec.attrs.clone(),
                        rb_geom_ids: ring_ids,
                    });
                }
                SimpleGeometry::LineString(coords) => {
                    let mut rb = RbGeom {
                        id: rb_id,
                        original_type: GeomType::LineString,
                        is_simplest: false,
                        need_pivot: false,
                        coords: coords.clone(),
                    };
                    setup_rb_geom_flags(&mut rb, zero_relative);
                    rb_geoms.push(rb);

                    features.push(Self::LineString {
                        stable_id: rec.stable_id,
                        fid: rec.fid,
                        attrs: rec.attrs.clone(),
                        rb_geom_id: rb_id,
                    });
                    rb_id += 1;
                }
                SimpleGeometry::Point(pt) => {
                    let mut rb = RbGeom {
                        id: rb_id,
                        original_type: GeomType::Point,
                        is_simplest: true,
                        need_pivot: false,
                        coords: vec![*pt],
                    };
                    setup_rb_geom_flags(&mut rb, zero_relative);
                    rb_geoms.push(rb);

                    features.push(Self::Point {
                        stable_id: rec.stable_id,
                        fid: rec.fid,
                        attrs: rec.attrs.clone(),
                        rb_geom_id: rb_id,
                    });
                    rb_id += 1;
                }
            }
        }

        Ok((features, rb_geoms))
    }

    pub fn rebuild_record(&self, rb_geoms: &[RbGeom]) -> Result<FeatureRecord> {
        match self {
            Self::Polygon {
                stable_id,
                fid,
                attrs,
                rb_geom_ids,
            } => {
                let mut rings = Vec::new();
                for ring_id in rb_geom_ids {
                    let rb = rb_geoms
                        .iter()
                        .find(|g| g.id == *ring_id)
                        .ok_or_else(|| anyhow!("missing ring id {ring_id}"))?;
                    rings.push(rb.coords.clone());
                }

                let outer = rings.first().cloned().unwrap_or_default();
                let inners = if rings.len() > 1 {
                    rings[1..].to_vec()
                } else {
                    Vec::new()
                };

                Ok(FeatureRecord {
                    stable_id: *stable_id,
                    fid: *fid,
                    attrs: attrs.clone(),
                    geometry: SimpleGeometry::Polygon { outer, inners },
                })
            }
            Self::LineString {
                stable_id,
                fid,
                attrs,
                rb_geom_id,
            } => {
                let rb = rb_geoms
                    .iter()
                    .find(|g| g.id == *rb_geom_id)
                    .ok_or_else(|| anyhow!("missing line rb geom id {rb_geom_id}"))?;

                Ok(FeatureRecord {
                    stable_id: *stable_id,
                    fid: *fid,
                    attrs: attrs.clone(),
                    geometry: SimpleGeometry::LineString(rb.coords.clone()),
                })
            }
            Self::Point {
                stable_id,
                fid,
                attrs,
                rb_geom_id,
            } => {
                let rb = rb_geoms
                    .iter()
                    .find(|g| g.id == *rb_geom_id)
                    .ok_or_else(|| anyhow!("missing point rb geom id {rb_geom_id}"))?;

                let coord = rb
                    .coords
                    .first()
                    .copied()
                    .ok_or_else(|| anyhow!("point has no coordinate"))?;

                Ok(FeatureRecord {
                    stable_id: *stable_id,
                    fid: *fid,
                    attrs: attrs.clone(),
                    geometry: SimpleGeometry::Point(coord),
                })
            }
        }
    }
}

fn setup_rb_geom_flags(rb: &mut RbGeom, zero_relative: f64) {
    if rb.original_type == GeomType::Point {
        rb.is_simplest = true;
        rb.need_pivot = false;
        return;
    }

    if rb.length_2d() >= zero_relative {
        if rb.is_closed() {
            rb.need_pivot = true;
        }
    } else {
        rb.is_simplest = true;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum GeoJsonGeometry {
    Point { coordinates: [f64; 2] },
    LineString { coordinates: Vec<[f64; 2]> },
    Polygon { coordinates: Vec<Vec<[f64; 2]>> },
}

impl TryFrom<GeoJsonGeometry> for SimpleGeometry {
    type Error = anyhow::Error;

    fn try_from(value: GeoJsonGeometry) -> Result<Self> {
        match value {
            GeoJsonGeometry::Point { coordinates } => Ok(Self::Point(Coord {
                x: coordinates[0],
                y: coordinates[1],
            })),
            GeoJsonGeometry::LineString { coordinates } => Ok(Self::LineString(
                coordinates
                    .into_iter()
                    .map(|c| Coord { x: c[0], y: c[1] })
                    .collect(),
            )),
            GeoJsonGeometry::Polygon { coordinates } => {
                let mut iter = coordinates.into_iter();
                let outer = iter
                    .next()
                    .ok_or_else(|| anyhow!("polygon has no exterior ring"))?
                    .into_iter()
                    .map(|c| Coord { x: c[0], y: c[1] })
                    .collect::<Vec<_>>();

                let inners = iter
                    .map(|ring| {
                        ring.into_iter()
                            .map(|c| Coord { x: c[0], y: c[1] })
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>();

                Ok(Self::Polygon { outer, inners })
            }
        }
    }
}
