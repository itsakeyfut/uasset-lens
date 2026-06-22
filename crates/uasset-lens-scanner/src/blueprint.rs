use uasset_lens_shared::{AssetPath, AssetType};

#[derive(Debug, Clone, PartialEq)]
pub struct BlueprintMetrics {
    pub node_count: u32,
    pub event_tick_count: u32,
    pub cast_count: u32,
    pub dependency_depth: u32,
}

pub(crate) fn is_blueprint_asset(asset_type: &AssetType) -> bool {
    matches!(
        asset_type,
        AssetType::Blueprint | AssetType::AnimBlueprint | AssetType::UserWidget
    )
}

// Scans export table rows to count Blueprint graph nodes by class name.
// Exact UE5 property names for in-node data require fixture investigation (Issue #31 design note);
// this uses the ClassIndex → import ObjectName route already proven by parse_export_table.
pub(crate) fn extract_blueprint_metrics(
    data: &[u8],
    export_offset: u64,
    export_count: usize,
    depends_offset: u64,
    import_class_name_idxs: &[usize],
    dependencies: &[AssetPath],
    name_table: &[String],
) -> BlueprintMetrics {
    // dependency_depth counts all /Game/ package-level imports, not exclusively Blueprint-type
    // ones. Filtering to Blueprint-only imports requires ClassName data not currently exposed by
    // parse_import_entries; total /Game/ import count is the accepted approximation.
    let dependency_depth = dependencies.len() as u32;

    if export_count == 0 || depends_offset <= export_offset {
        return BlueprintMetrics {
            node_count: 0,
            event_tick_count: 0,
            cast_count: 0,
            dependency_depth,
        };
    }

    let bytes_per_entry = (depends_offset - export_offset) / export_count as u64;
    if bytes_per_entry < 4 {
        return BlueprintMetrics {
            node_count: 0,
            event_tick_count: 0,
            cast_count: 0,
            dependency_depth,
        };
    }

    let mut node_count = 0u32;
    let mut cast_count = 0u32;
    let mut k2event_count = 0u32;

    for i in 0..export_count {
        let entry_pos = (export_offset + i as u64 * bytes_per_entry) as usize;
        if entry_pos + 4 > data.len() {
            break;
        }
        let class_name =
            crate::parser::export_class_name(data, entry_pos, import_class_name_idxs, name_table)
                .unwrap_or("");
        if class_name.starts_with("K2Node_") {
            node_count += 1;
        }
        if class_name == "K2Node_DynamicCast" {
            cast_count += 1;
        }
        if class_name == "K2Node_Event" {
            k2event_count += 1;
        }
    }

    // "ReceiveTick" in name table + at least one K2Node_Event ≈ tick event present.
    // Per-export EventName property parsing deferred until property names confirmed.
    let event_tick_count = if k2event_count > 0 && name_table.iter().any(|n| n == "ReceiveTick") {
        1
    } else {
        0
    };

    BlueprintMetrics {
        node_count,
        event_tick_count,
        cast_count,
        dependency_depth,
    }
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
    fn extract_blueprint_metrics_should_count_k2node_exports() {
        // name_table: class names first, then "ReceiveTick" for event-tick detection
        let name_table: Vec<String> = vec![
            "Blueprint".into(),
            "K2Node_Event".into(),
            "K2Node_DynamicCast".into(),
            "K2Node_CallFunction".into(),
            "ReceiveTick".into(),
        ];
        // import[0..3] point to name_table[0..3]
        let import_class_name_idxs: Vec<usize> = vec![0, 1, 2, 3];
        // Exports: -1→import[0]→Blueprint, -2→K2Node_Event, -3→K2Node_DynamicCast, -4→K2Node_CallFunction
        let bytes_per_entry: usize = 4;
        let class_indices = [-1i32, -2, -3, -4];
        let data = make_export_table(&class_indices, bytes_per_entry);
        let export_count = class_indices.len();
        let depends_offset = export_count as u64 * bytes_per_entry as u64;

        let metrics = extract_blueprint_metrics(
            &data,
            0,
            export_count,
            depends_offset,
            &import_class_name_idxs,
            &[],
            &name_table,
        );

        assert_eq!(metrics.node_count, 3); // K2Node_Event + K2Node_DynamicCast + K2Node_CallFunction
        assert_eq!(metrics.cast_count, 1); // only K2Node_DynamicCast
        assert_eq!(metrics.event_tick_count, 1); // K2Node_Event present + "ReceiveTick" in name table
        assert_eq!(metrics.dependency_depth, 0);
    }

    #[test]
    fn extract_blueprint_metrics_should_not_count_non_k2node_exports() {
        let name_table: Vec<String> = vec!["Blueprint".into(), "BlueprintGeneratedClass".into()];
        let import_class_name_idxs: Vec<usize> = vec![0, 1];
        let bytes_per_entry: usize = 4;
        let class_indices = [-1i32, -2];
        let data = make_export_table(&class_indices, bytes_per_entry);
        let export_count = class_indices.len();
        let depends_offset = export_count as u64 * bytes_per_entry as u64;

        let metrics = extract_blueprint_metrics(
            &data,
            0,
            export_count,
            depends_offset,
            &import_class_name_idxs,
            &[],
            &name_table,
        );

        assert_eq!(metrics.node_count, 0);
        assert_eq!(metrics.cast_count, 0);
        assert_eq!(metrics.event_tick_count, 0);
    }

    #[test]
    fn extract_blueprint_metrics_should_set_dependency_depth_from_deps_count() {
        let dep1 = AssetPath::new("/Game/A").unwrap();
        let dep2 = AssetPath::new("/Game/B").unwrap();

        let metrics = extract_blueprint_metrics(
            &[],
            0,
            0, // export_count == 0 → early return
            0,
            &[],
            &[dep1, dep2],
            &[], // name_table unused when export_count == 0
        );

        assert_eq!(metrics.dependency_depth, 2);
        assert_eq!(metrics.node_count, 0);
    }

    #[test]
    fn extract_blueprint_metrics_should_not_set_event_tick_without_receive_tick_in_name_table() {
        let name_table: Vec<String> = vec!["K2Node_Event".into()];
        let import_class_name_idxs: Vec<usize> = vec![0];
        let bytes_per_entry: usize = 4;
        let data = make_export_table(&[-1i32], bytes_per_entry);

        let metrics = extract_blueprint_metrics(
            &data,
            0,
            1,
            bytes_per_entry as u64,
            &import_class_name_idxs,
            &[],
            &name_table, // no "ReceiveTick" in name table
        );

        assert_eq!(metrics.event_tick_count, 0);
    }

    #[test]
    fn is_blueprint_asset_should_return_true_for_blueprint_anim_blueprint_user_widget() {
        assert!(is_blueprint_asset(&AssetType::Blueprint));
        assert!(is_blueprint_asset(&AssetType::AnimBlueprint));
        assert!(is_blueprint_asset(&AssetType::UserWidget));
    }

    #[test]
    fn is_blueprint_asset_should_return_false_for_non_blueprint_types() {
        assert!(!is_blueprint_asset(&AssetType::Texture2D));
        assert!(!is_blueprint_asset(&AssetType::StaticMesh));
        assert!(!is_blueprint_asset(&AssetType::Material));
        assert!(!is_blueprint_asset(&AssetType::World));
        assert!(!is_blueprint_asset(&AssetType::ObjectRedirector));
    }
}
