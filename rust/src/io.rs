use std::path::Path;

use anyhow::{anyhow, Result};
use gdal::vector::{
    Defn, FieldValue as OgrFieldValue, Geometry, LayerAccess, LayerOptions, OGRFieldType,
    OGRwkbGeometryType,
};
use gdal::{Dataset, DriverManager};

use crate::geometry::{FeatureRecord, FieldValue, GeoJsonGeometry, SimpleGeometry};

#[derive(Debug, Clone)]
pub struct SchemaField {
    pub name: String,
    pub field_type: OGRFieldType::Type,
    pub width: Option<i32>,
    pub precision: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct LayerSchema {
    pub layer_name: String,
    pub fields: Vec<SchemaField>,
    pub geom_type: OGRwkbGeometryType::Type,
    pub srs_wkt: Option<String>,
}

pub fn read_gpkg(
    path: &str,
    layer_name: Option<&str>,
) -> Result<(Vec<FeatureRecord>, LayerSchema)> {
    let ds = Dataset::open(Path::new(path))?;
    let mut layer = if let Some(name) = layer_name {
        ds.layer_by_name(name)?
    } else {
        ds.layer(0)?
    };

    let defn = layer.defn();
    let field_schemas: Vec<SchemaField> = defn
        .fields()
        .map(|f| SchemaField {
            name: f.name(),
            field_type: f.field_type(),
            width: Some(f.width()),
            precision: Some(f.precision()),
        })
        .collect();

    let geom_type = defn
        .geom_fields()
        .next()
        .map(|gf| gf.field_type())
        .unwrap_or(OGRwkbGeometryType::wkbUnknown);

    let srs_wkt = layer.spatial_ref().and_then(|s| s.to_wkt().ok());

    let schema = LayerSchema {
        layer_name: layer.name(),
        fields: field_schemas,
        geom_type,
        srs_wkt,
    };

    let mut records = Vec::new();
    for (stable_idx, feature) in layer.features().enumerate() {
        let mut attrs = Vec::new();
        for fld in &schema.fields {
            attrs.push(match feature.field(&fld.name)? {
                Some(OgrFieldValue::IntegerValue(v)) => FieldValue::Integer(v as i64),
                Some(OgrFieldValue::Integer64Value(v)) => FieldValue::Integer64(v),
                Some(OgrFieldValue::RealValue(v)) => FieldValue::Real(v),
                Some(OgrFieldValue::StringValue(v)) => FieldValue::Text(v),
                _ => FieldValue::Null,
            });
        }

        let geom_ref = feature
            .geometry()
            .ok_or_else(|| anyhow!("feature {stable_idx} has no geometry"))?;
        let json = geom_ref.json()?;
        let gj: GeoJsonGeometry = serde_json::from_str(&json)?;
        let geometry = SimpleGeometry::try_from(gj)?;

        records.push(FeatureRecord {
            stable_id: stable_idx,
            fid: feature.fid(),
            attrs,
            geometry,
        });
    }

    Ok((records, schema))
}

pub fn write_gpkg(path: &str, records: &[FeatureRecord], schema: &LayerSchema) -> Result<()> {
    if Path::new(path).exists() {
        std::fs::remove_file(path)?;
    }

    let driver = DriverManager::get_driver_by_name("GPKG")?;
    let mut ds = driver.create_vector_only(path)?;

    let srs = if let Some(wkt) = &schema.srs_wkt {
        Some(gdal::spatial_ref::SpatialRef::from_wkt(wkt)?)
    } else {
        None
    };

    let layer_options = LayerOptions {
        name: &schema.layer_name,
        srs: srs.as_ref(),
        ty: schema.geom_type,
        options: None,
    };

    let layer = ds.create_layer(layer_options)?;

    for fld in &schema.fields {
        layer.create_defn_fields(&[(fld.name.as_str(), fld.field_type)])?;
    }

    let defn = Defn::from_layer(&layer);

    for rec in records {
        let mut feat = gdal::vector::Feature::new(&defn)?;
        let geom_wkt = rec.geometry.to_wkt();
        let geom = Geometry::from_wkt(&geom_wkt)?;
        feat.set_geometry(geom)?;

        for (i, val) in rec.attrs.iter().enumerate() {
            let field_name = &schema.fields[i].name;
            match val {
                FieldValue::Integer(v) => feat.set_field_integer(field_name, *v as i32)?,
                FieldValue::Integer64(v) => feat.set_field_integer64(field_name, *v)?,
                FieldValue::Real(v) => feat.set_field_double(field_name, *v)?,
                FieldValue::Text(v) => feat.set_field_string(field_name, v)?,
                FieldValue::Null => feat.set_field_null(field_name)?,
            }
        }

        feat.create(&layer)?;
    }

    Ok(())
}
