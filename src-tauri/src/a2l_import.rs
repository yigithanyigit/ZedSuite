use crate::models::{AxisEncoding, DataType, DetectedMap, MapDimensions};
use a2lfile::{
    A2lObjectName, AddrType, AxisDescrAttribute, ByteOrderEnum, Characteristic, CharacteristicType,
    ConversionType, DataType as Storage, IndexMode, IndexOrder, Module, RecordLayout,
};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Serialize)]
pub struct ImportResult {
    pub maps: Vec<DetectedMap>,
    pub rejected: Vec<String>,
}

fn storage(kind: Storage) -> Result<(usize, DataType), String> {
    Ok(match kind {
        Storage::Ubyte => (1, DataType::UInt8),
        Storage::Sbyte => (1, DataType::Int8),
        Storage::Uword => (2, DataType::UInt16),
        Storage::Sword => (2, DataType::Int16),
        Storage::Ulong => (4, DataType::UInt32),
        Storage::Slong => (4, DataType::Int32),
        Storage::Float32Ieee => (4, DataType::Float32),
        _ => return Err(format!("unsupported storage {kind:?}")),
    })
}

fn big_endian(order: Option<&a2lfile::ByteOrder>) -> Result<(), String> {
    if !order.is_some_and(|o| {
        matches!(
            o.byte_order,
            ByteOrderEnum::MsbFirst | ByteOrderEnum::BigEndian
        )
    }) {
        return Err("reference requires explicit big-endian byte order".into());
    }
    Ok(())
}

#[derive(Clone)]
struct Conversion {
    factor: f64,
    offset: f64,
    units: String,
}

fn conversion(module: &Module, name: &str) -> Result<Conversion, String> {
    let method = module
        .compu_method
        .get(name)
        .ok_or_else(|| format!("missing conversion {name}"))?;
    let coeffs = method
        .coeffs
        .as_ref()
        .ok_or_else(|| format!("non-numeric conversion {name}"))?;
    if method.conversion_type != ConversionType::RatFunc
        || coeffs.a != 0.0
        || coeffs.d != 0.0
        || coeffs.e != 0.0
        || coeffs.b == 0.0
        || coeffs.f == 0.0
    {
        return Err(format!("unsupported conversion {name}"));
    }
    let factor = coeffs.f / coeffs.b;
    let offset = -coeffs.c / coeffs.b;
    if !factor.is_finite() || !offset.is_finite() {
        return Err("invalid conversion".into());
    }
    Ok(Conversion {
        factor,
        offset,
        units: method.unit.clone(),
    })
}

#[derive(Clone)]
struct Field {
    offset: u32,
    kind: Storage,
}

fn layout_fields(
    layout: &RecordLayout,
    address: u32,
    dimensions: [usize; 2],
    bin: &[u8],
) -> Result<BTreeMap<&'static str, Field>, String> {
    let mut fields: Vec<(u16, &'static str, Storage, usize)> = Vec::new();
    for (axis, value, count) in [
        ("x", &layout.axis_pts_x, dimensions[0]),
        ("y", &layout.axis_pts_y, dimensions[1]),
    ] {
        if let Some(value) = value {
            if value.addressing != AddrType::Direct || value.index_incr != IndexOrder::IndexIncr {
                return Err("indirect or descending axis layout".into());
            }
            fields.push((value.position, axis, value.datatype, count));
        }
    }
    for (axis, value) in [("nx", &layout.no_axis_pts_x), ("ny", &layout.no_axis_pts_y)] {
        if let Some(value) = value {
            fields.push((value.position, axis, value.datatype, 1));
        }
    }
    if let Some(value) = &layout.fnc_values {
        if value.address_type != AddrType::Direct
            || !matches!(value.index_mode, IndexMode::RowDir | IndexMode::ColumnDir)
        {
            return Err("unsupported function layout".into());
        }
        fields.push((
            value.position,
            "z",
            value.datatype,
            dimensions[0] * dimensions[1],
        ));
    }
    for value in &layout.reserved {
        let kind = match value.data_size {
            a2lfile::DataTypeSize::Byte => Storage::Ubyte,
            a2lfile::DataTypeSize::Word => Storage::Uword,
            a2lfile::DataTypeSize::Long => Storage::Ulong,
        };
        fields.push((value.position, "reserved", kind, 1));
    }
    fields.sort_by_key(|f| f.0);
    if fields.windows(2).any(|w| w[0].0 == w[1].0) {
        return Err("duplicate layout position".into());
    }
    let mut offset = address
        .checked_sub(0x09000000)
        .ok_or("address below reference image")? as usize;
    let mut result = BTreeMap::new();
    for (_, name, kind, count) in fields {
        let (width, _) = storage(kind)?;
        let alignment = match kind {
            Storage::Float32Ieee => layout
                .alignment_float32_ieee
                .as_ref()
                .map(|a| a.alignment_border),
            _ if width == 1 => layout.alignment_byte.as_ref().map(|a| a.alignment_border),
            _ if width == 2 => layout.alignment_word.as_ref().map(|a| a.alignment_border),
            _ => layout.alignment_long.as_ref().map(|a| a.alignment_border),
        }
        .filter(|n| *n > 0)
        .ok_or("missing layout alignment")? as usize;
        offset = offset.div_ceil(alignment) * alignment;
        let end = offset.checked_add(width * count).ok_or("layout overflow")?;
        if offset < 0x680000 || end > bin.len() {
            return Err("layout outside calibration image".into());
        }
        if name == "nx" || name == "ny" {
            let count = bin[offset..end]
                .iter()
                .fold(0usize, |v, b| (v << 8) | *b as usize);
            if count != dimensions[usize::from(name == "ny")] {
                return Err(format!(
                    "{name} header {count} disagrees with declared dimension"
                ));
            }
        }
        result.insert(
            name,
            Field {
                offset: offset as u32,
                kind,
            },
        );
        offset = end;
    }
    Ok(result)
}

fn describe(module: &Module, item: &Characteristic, bin: &[u8]) -> Result<DetectedMap, String> {
    if matches!(
        item.get_name(),
        "KLIHDR"
            | "KLPHDR"
            | "KLVSTMSVG"
            | "IKCtl_FacKnockDetThd0_GM"
            | "IKCtl_FacKnockDetThd1_GM"
            | "IKCtl_FacKnockDetThd2_GM"
            | "IKCtl_FacKnockDetThd3_GM"
            | "IKCtl_FacKnockDetThd4_GM"
            | "IKCtl_FacKnockDetThd5_GM"
    ) {
        return Err("reference A2L and XDF disagree on axis address; unresolved".into());
    }
    big_endian(item.byte_order.as_ref())?;
    if item.bit_mask.is_some() || item.read_only.is_some() || item.ecu_address_extension.is_some() {
        return Err("masked, read-only or extended-address characteristic".into());
    }
    let axis_count = match item.characteristic_type {
        CharacteristicType::Value => 0,
        CharacteristicType::Curve => 1,
        CharacteristicType::Map => 2,
        _ => return Err("unsupported characteristic kind".into()),
    };
    if item.axis_descr.len() != axis_count {
        return Err("axis count mismatch".into());
    }
    let dims = [
        item.axis_descr
            .first()
            .map_or(1, |a| a.max_axis_points as usize),
        item.axis_descr
            .get(1)
            .map_or(1, |a| a.max_axis_points as usize),
    ];
    if dims.iter().any(|n| *n == 0 || *n > 4096) {
        return Err("invalid dimensions".into());
    }
    let layout = module
        .record_layout
        .get(&item.deposit)
        .ok_or("missing record layout")?;
    let fields = layout_fields(layout, item.address, dims, bin)?;
    let value = fields.get("z").ok_or("missing values")?;
    let (width, kind) = storage(value.kind)?;
    let conv = conversion(module, &item.conversion)?;
    let mut map = DetectedMap::new(
        value.offset,
        width * dims[0] * dims[1],
        MapDimensions::TwoDimensional {
            rows: dims[1],
            cols: dims[0],
        },
        kind,
    );
    map.name = Some(item.long_identifier.clone());
    map.description = Some(item.get_name().to_string());
    map.external_source = Some("A2L".into());
    map.category = Some("Calibration".into());
    map.subcategory = map.category.clone();
    map.is_little_endian = Some(false);
    map.column_major = Some(layout.fnc_values.as_ref().unwrap().index_mode == IndexMode::ColumnDir);
    map.unit = Some(conv.units);
    map.correction_factor = Some(conv.factor);
    map.offset = Some(conv.offset);
    map.confidence = 1.0;
    for (i, axis) in item.axis_descr.iter().enumerate() {
        let field = match axis.attribute {
            AxisDescrAttribute::StdAxis => fields
                .get(if i == 0 { "x" } else { "y" })
                .cloned()
                .ok_or("missing inline axis")?,
            AxisDescrAttribute::ComAxis => {
                let name = &axis
                    .axis_pts_ref
                    .as_ref()
                    .ok_or("missing shared axis reference")?
                    .axis_points;
                let shared = module
                    .axis_pts
                    .get(name)
                    .ok_or_else(|| format!("missing shared axis {name}"))?;
                big_endian(shared.byte_order.as_ref())?;
                if shared.max_axis_points != axis.max_axis_points
                    || shared.conversion != axis.conversion
                {
                    return Err(format!("shared axis {name} disagrees with characteristic"));
                }
                let layout = module
                    .record_layout
                    .get(&shared.deposit_record)
                    .ok_or("missing shared axis layout")?;
                layout_fields(layout, shared.address, [dims[i], 1], bin)?
                    .get("x")
                    .cloned()
                    .ok_or("missing shared axis data")?
            }
            _ => return Err("unsupported axis kind".into()),
        };
        let (_, kind) = storage(field.kind)?;
        let conv = conversion(module, &axis.conversion)?;
        let encoding = Some(AxisEncoding {
            data_type: kind,
            is_little_endian: false,
        });
        if i == 0 {
            map.x_axis_address = Some(field.offset);
            map.x_axis_encoding = encoding;
            map.x_axis_correction = Some(conv.factor);
            map.x_axis_offset = Some(conv.offset);
            map.x_label = Some(conv.units);
        } else {
            map.y_axis_address = Some(field.offset);
            map.y_axis_encoding = encoding;
            map.y_axis_correction = Some(conv.factor);
            map.y_axis_offset = Some(conv.offset);
            map.y_label = Some(conv.units);
        }
    }
    Ok(map)
}

pub fn parse_reference(data: &[u8], bin: &[u8]) -> Result<ImportResult, String> {
    use sha2::{Digest, Sha256};
    if format!("{:x}", Sha256::digest(data))
        != "f5d56122110007ec120f67427108be2ed82e09bd730ec9a131266847fe3de0e4"
    {
        return Err("A2L is not the verified R0R9A005B reference definition".into());
    }
    if !crate::mg_custom::matches_reference_software(bin) {
        return Err("BIN does not match the reference MG1 software".into());
    }
    let text = crate::xdf_import::decode_text(data);
    let (file, warnings) =
        a2lfile::load_from_string(&text, None, false).map_err(|e| e.to_string())?;
    let mut result = ImportResult {
        maps: Vec::new(),
        rejected: warnings.iter().map(ToString::to_string).collect(),
    };
    for module in &file.project.module {
        for item in &module.characteristic {
            match describe(module, item, bin) {
                Ok(map) => result.maps.push(map),
                Err(reason) => result
                    .rejected
                    .push(format!("{}: {reason}", item.get_name())),
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unverified_definition() {
        assert!(parse_reference(b"not the reference", &[]).is_err());
    }

    #[test]
    fn affine_rat_func_is_inverted() {
        let mut module = Module::new("test".into(), "".into());
        let mut method = a2lfile::CompuMethod::new(
            "scale".into(),
            "".into(),
            ConversionType::RatFunc,
            "%6.2".into(),
            "unit".into(),
        );
        method.coeffs = Some(a2lfile::Coeffs::new(0.0, 2.0, -4.0, 0.0, 0.0, 8.0));
        module.compu_method.push(method);
        let scale = conversion(&module, "scale").unwrap();
        assert_eq!((scale.factor, scale.offset), (4.0, 2.0));
        assert!(conversion(&module, "missing").is_err());
    }

    #[test]
    fn inline_layout_honors_count_alignment_and_bounds() {
        let mut layout = RecordLayout::new("test".into());
        layout.alignment_byte = Some(a2lfile::AlignmentByte::new(1));
        layout.alignment_word = Some(a2lfile::AlignmentWord::new(2));
        layout.no_axis_pts_x = Some(a2lfile::NoAxisPtsDim::new(1, Storage::Ubyte));
        layout.axis_pts_x = Some(a2lfile::AxisPtsDim::new(
            2,
            Storage::Uword,
            IndexOrder::IndexIncr,
            AddrType::Direct,
        ));
        layout.fnc_values = Some(a2lfile::FncValues::new(
            3,
            Storage::Sword,
            IndexMode::ColumnDir,
            AddrType::Direct,
        ));
        let mut bin = vec![0; 0x68000e];
        bin[0x680000] = 3;
        let fields = layout_fields(&layout, 0x09680000, [3, 1], &bin).unwrap();
        assert_eq!(fields["x"].offset, 0x680002);
        assert_eq!(fields["z"].offset, 0x680008);
        assert!(layout_fields(&layout, 0x09680000, [2, 1], &bin).is_err());
        assert!(layout_fields(&layout, 0x09680000, [3, 1], &bin[..bin.len() - 1]).is_err());
        assert!(layout_fields(&layout, 0x09670000, [3, 1], &bin).is_err());
    }
    #[test]
    #[ignore = "requires private reference files outside the repository"]
    fn imports_private_reference_binaries() {
        use base64::{engine::general_purpose::STANDARD, Engine};
        let root =
            std::path::PathBuf::from(std::env::var("MG_REFERENCE_DIR").expect("MG_REFERENCE_DIR"));
        let data = std::fs::read(root.join("analysis/definitions/R0R9A005B_7706-000_011_005.a2l"))
            .unwrap();
        assert!(parse_reference(&data, &vec![0; 0x780000]).is_err());
        let mut binaries: Vec<_> = ["250", "280", "300"]
            .iter()
            .map(|tune| root.join(format!("analysis/decoded/dbj_{tune}.bin")))
            .collect();
        binaries.extend(
            std::fs::read_dir(&root)
                .unwrap()
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| {
                    path.file_name()
                        .unwrap()
                        .to_string_lossy()
                        .starts_with("WBA")
                        && path.extension().is_some_and(|extension| extension == "bin")
                }),
        );
        assert_eq!(binaries.len(), 4);
        for path in binaries {
            let binary = std::fs::read(path).unwrap();
            let result = crate::commands::import_map_definitions(
                STANDARD.encode(&data),
                "reference.a2l".into(),
                binary.len() as u32,
                Some(STANDARD.encode(&binary)),
            )
            .unwrap();
            assert_eq!(result.maps.len(), 357);
            assert_eq!(result.rejected.len(), 42);
            assert!(
                result
                    .maps
                    .iter()
                    .filter(|map| matches!(map.data_type, DataType::Float32))
                    .count()
                    == 3
            );
            let lambda = result
                .maps
                .iter()
                .find(|map| map.description.as_deref() == Some("KF_LABAS_1"))
                .unwrap();
            assert_eq!(lambda.correction_factor, Some(1.0 / 4096.0));
            for map in &result.maps {
                assert!(map.address as usize + map.size <= binary.len());
                assert_eq!(map.is_little_endian, Some(false));
            }
        }
    }
}
