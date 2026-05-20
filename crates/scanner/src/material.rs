use shared::AssetType;

pub(crate) fn is_material_asset(asset_type: &AssetType) -> bool {
    matches!(asset_type, AssetType::Material)
}

// Counts MaterialExpressionTextureSample nodes by scanning export ClassIndex values.
//
// Issue #40 design note: the spec requested ArrayProperty "Expressions" parsing to count
// texture sample entries. The exact property name requires fixture investigation; exact
// property offsets differ between Material asset versions. This function uses the proven
// ClassIndex → import ObjectName route (same approach as extract_blueprint_metrics) which
// is reliable without knowing per-export property offsets.
pub(crate) fn extract_texture_sample_count(
    data: &[u8],
    export_offset: u64,
    export_count: usize,
    depends_offset: u64,
    import_class_name_idxs: &[usize],
    name_table: &[String],
) -> u32 {
    if export_count == 0 || depends_offset <= export_offset {
        return 0;
    }
    let bytes_per_entry = (depends_offset - export_offset) / export_count as u64;
    if bytes_per_entry < 4 {
        return 0;
    }
    let mut count = 0u32;
    for i in 0..export_count {
        let entry_pos = (export_offset + i as u64 * bytes_per_entry) as usize;
        if entry_pos + 4 > data.len() {
            break;
        }
        let class_index = i32::from_le_bytes([
            data[entry_pos],
            data[entry_pos + 1],
            data[entry_pos + 2],
            data[entry_pos + 3],
        ]);
        if class_index >= 0 {
            continue;
        }
        let imp_i = (-class_index - 1) as usize;
        let matches = import_class_name_idxs
            .get(imp_i)
            .and_then(|&idx| name_table.get(idx))
            .map(|s| s == "MaterialExpressionTextureSample")
            .unwrap_or(false);
        if matches {
            count += 1;
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_export_table(class_indices: &[i32], bytes_per_entry: usize) -> Vec<u8> {
        let mut data = Vec::new();
        for &ci in class_indices {
            let mut entry = ci.to_le_bytes().to_vec();
            entry.resize(bytes_per_entry, 0);
            data.extend(entry);
        }
        data
    }

    #[test]
    fn extract_texture_sample_count_should_count_only_texture_sample_class_exports() {
        let name_table = vec![
            "Material".to_string(),
            "MaterialExpressionTextureSample".to_string(),
            "MaterialExpressionMultiply".to_string(),
        ];
        let import_class_name_idxs: Vec<usize> = vec![0, 1, 2];
        // -1 → import[0]→Material (skip), -2 → TextureSample (count), -3 → Multiply (skip), -2 (count)
        let bytes_per: usize = 4;
        let class_indices = [-1i32, -2, -3, -2];
        let data = make_export_table(&class_indices, bytes_per);
        let export_count = class_indices.len();
        let depends_offset = export_count as u64 * bytes_per as u64;

        let result = extract_texture_sample_count(
            &data,
            0,
            export_count,
            depends_offset,
            &import_class_name_idxs,
            &name_table,
        );
        assert_eq!(result, 2);
    }

    #[test]
    fn extract_texture_sample_count_should_return_zero_when_no_exports_match() {
        let name_table = vec!["Material".to_string()];
        let import_class_name_idxs: Vec<usize> = vec![0];
        let bytes_per: usize = 4;
        let data = make_export_table(&[-1i32], bytes_per);
        let result = extract_texture_sample_count(
            &data,
            0,
            1,
            bytes_per as u64,
            &import_class_name_idxs,
            &name_table,
        );
        assert_eq!(result, 0);
    }

    #[test]
    fn is_material_asset_should_return_true_for_material_and_false_for_others() {
        assert!(is_material_asset(&AssetType::Material));
        assert!(!is_material_asset(&AssetType::MaterialInstance));
        assert!(!is_material_asset(&AssetType::Blueprint));
        assert!(!is_material_asset(&AssetType::Texture2D));
    }
}
