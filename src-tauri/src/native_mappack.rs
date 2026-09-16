use crate::models::{AxisEncoding, DataType, DetectedMap, MapDimensions};
use serde::Deserialize;

#[derive(Deserialize)]
struct Pack {
    format: String,
    version: u32,
    software: String,
    rom_size: usize,
    maps: Vec<DetectedMap>,
    #[serde(default)]
    rejected: Vec<String>,
}

fn width(kind: &DataType) -> usize {
    match kind {
        DataType::UInt8 | DataType::Int8 => 1,
        DataType::UInt16 | DataType::Int16 => 2,
        _ => 4,
    }
}

fn range(address: u32, count: usize, width: usize, length: usize) -> Result<(), String> {
    let end = count
        .checked_mul(width)
        .and_then(|size| (address as usize).checked_add(size));
    if address < 0x680000 || !end.is_some_and(|end| end <= length) {
        return Err("definition outside reference calibration image".into());
    }
    Ok(())
}

fn scale(factor: Option<f64>, offset: Option<f64>) -> Result<(), String> {
    let factor = factor.unwrap_or(1.0);
    let offset = offset.unwrap_or(0.0);
    if !factor.is_finite() || factor == 0.0 || !offset.is_finite() {
        return Err("invalid numeric conversion".into());
    }
    Ok(())
}

fn axis(
    address: Option<u32>,
    encoding: Option<&AxisEncoding>,
    count: usize,
    factor: Option<f64>,
    offset: Option<f64>,
    length: usize,
) -> Result<(), String> {
    if let Some(address) = address.filter(|address| *address != 0) {
        let encoding = encoding.ok_or("axis encoding is missing")?;
        range(address, count, width(&encoding.data_type), length)?;
        scale(factor, offset)?;
    }
    Ok(())
}

fn validate(map: &DetectedMap, length: usize) -> Result<(), String> {
    let (rows, cols) = match map.dimensions {
        MapDimensions::TwoDimensional { rows, cols } => (rows, cols),
        MapDimensions::OneDimensional { length } => (1, length),
        MapDimensions::ThreeDimensional { .. } => {
            return Err("three-dimensional definition is unsupported".into())
        }
    };
    if rows == 0
        || cols == 0
        || rows > 4096
        || cols > 4096
        || rows * cols * width(&map.data_type) != map.size
    {
        return Err("inconsistent map dimensions or size".into());
    }
    if map.is_little_endian.is_none() {
        return Err("cell byte order is missing".into());
    }
    range(map.address, rows * cols, width(&map.data_type), length)?;
    scale(map.correction_factor, map.offset)?;
    axis(
        map.x_axis_address,
        map.x_axis_encoding.as_ref(),
        cols,
        map.x_axis_correction,
        map.x_axis_offset,
        length,
    )?;
    axis(
        map.y_axis_address,
        map.y_axis_encoding.as_ref(),
        rows,
        map.y_axis_correction,
        map.y_axis_offset,
        length,
    )?;
    Ok(())
}

pub fn parse(data: &[u8], binary: &[u8]) -> Result<(Vec<DetectedMap>, Vec<String>), String> {
    let mut pack: Pack =
        serde_json::from_slice(data).map_err(|e| format!("invalid native definition pack: {e}"))?;
    if pack.format != "ZedSuite MG1 definitions"
        || pack.version != 1
        || pack.software != "MG1CS003/R0R9A005B"
    {
        return Err("unsupported native definition format or software".into());
    }
    if pack.rom_size != binary.len() || !crate::mg_custom::matches_reference_software(binary) {
        return Err("native definition pack does not match the project software".into());
    }
    if pack.maps.is_empty() {
        return Err("native definition pack is empty".into());
    }
    for map in &mut pack.maps {
        validate(map, binary.len()).map_err(|reason| {
            format!("{}: {reason}", map.description.as_deref().unwrap_or("map"))
        })?;
        map.external_source = Some("ZedSuite".into());
    }
    Ok((pack.maps, pack.rejected))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference_map() -> DetectedMap {
        let mut map = DetectedMap::new(
            0x680100,
            24,
            MapDimensions::TwoDimensional { rows: 2, cols: 3 },
            DataType::Float32,
        );
        map.is_little_endian = Some(false);
        map.correction_factor = Some(0.5);
        map.offset = Some(-1.0);
        map.column_major = Some(true);
        map
    }

    #[test]
    fn validates_types_dimensions_and_conversions() {
        let mut map = reference_map();
        assert!(validate(&map, 0x780000).is_ok());
        map.size = 12;
        assert!(validate(&map, 0x780000).is_err());
        map.size = 24;
        map.correction_factor = Some(0.0);
        assert!(validate(&map, 0x780000).is_err());
        map.correction_factor = Some(1.0);
        map.address = 0x77fffe;
        assert!(validate(&map, 0x780000).is_err());
    }

    #[test]
    fn axes_require_explicit_encoding_and_in_bounds_data() {
        let mut map = reference_map();
        map.x_axis_address = Some(0x680000);
        assert!(validate(&map, 0x780000).is_err());
        map.x_axis_encoding = Some(AxisEncoding {
            data_type: DataType::Int8,
            is_little_endian: false,
        });
        map.x_axis_correction = Some(0.25);
        map.x_axis_offset = Some(-20.0);
        assert!(validate(&map, 0x780000).is_ok());
        map.x_axis_address = Some(0x77ffff);
        assert!(validate(&map, 0x780000).is_err());
    }
}
