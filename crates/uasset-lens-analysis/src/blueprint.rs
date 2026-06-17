use uasset_lens_scanner::AssetMetadata;
pub use uasset_lens_scanner::BlueprintMetrics;

/// Returns `None` for non-Blueprint assets; the scanner populates `blueprint_metrics`
/// only for Blueprint/AnimBlueprint/UserWidget.
pub fn analyze(metadata: &AssetMetadata) -> Option<BlueprintMetrics> {
    metadata.blueprint_metrics.clone()
}

#[derive(Debug, Clone)]
pub struct ComplexityThresholds {
    pub max_node_count: u32,
    pub max_event_tick_count: u32,
    pub max_cast_count: u32,
    pub max_dependency_depth: u32,
}

impl Default for ComplexityThresholds {
    fn default() -> Self {
        Self {
            max_node_count: 200,
            max_event_tick_count: 1,
            max_cast_count: 10,
            // UE5 Manny/Quinn PostProcess ABPs reach depth 16 due to the per-joint PoseAsset
            // chain (~64 pose assets) in the IK rig. 20 is chosen as a safe default that
            // accommodates standard Epic template content without false positives.
            max_dependency_depth: 20,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Warning {
    pub rule: &'static str,
    pub message: String,
}

pub fn is_complex(metrics: &BlueprintMetrics, thresholds: &ComplexityThresholds) -> Vec<Warning> {
    let mut warnings = Vec::new();
    if metrics.node_count > thresholds.max_node_count {
        warnings.push(Warning {
            rule: "blueprint/node-count",
            message: format!(
                "node count {} exceeds limit {}",
                metrics.node_count, thresholds.max_node_count
            ),
        });
    }
    if metrics.event_tick_count > thresholds.max_event_tick_count {
        warnings.push(Warning {
            rule: "blueprint/event-tick",
            message: format!(
                "event tick count {} exceeds limit {}",
                metrics.event_tick_count, thresholds.max_event_tick_count
            ),
        });
    }
    if metrics.cast_count > thresholds.max_cast_count {
        warnings.push(Warning {
            rule: "blueprint/cast-count",
            message: format!(
                "cast count {} exceeds limit {}",
                metrics.cast_count, thresholds.max_cast_count
            ),
        });
    }
    if metrics.dependency_depth > thresholds.max_dependency_depth {
        warnings.push(Warning {
            rule: "blueprint/dependency-depth",
            message: format!(
                "dependency depth {} exceeds limit {}",
                metrics.dependency_depth, thresholds.max_dependency_depth
            ),
        });
    }
    warnings
}

#[cfg(test)]
mod tests {
    use super::*;
    use uasset_lens_shared::AssetType;

    fn below_threshold_metrics() -> BlueprintMetrics {
        BlueprintMetrics {
            node_count: 5,
            event_tick_count: 0,
            cast_count: 2,
            dependency_depth: 1,
        }
    }

    #[test]
    fn analyze_should_return_some_for_blueprint_asset() {
        let metrics = below_threshold_metrics();
        let meta = AssetMetadata {
            blueprint_metrics: Some(metrics.clone()),
            ..uasset_lens_scanner::make_meta("/Game/Test", AssetType::Blueprint)
        };
        assert_eq!(analyze(&meta), Some(metrics));
    }

    #[test]
    fn analyze_should_return_none_for_non_blueprint_asset() {
        let meta = uasset_lens_scanner::make_meta("/Game/Test", AssetType::Texture2D);
        assert!(analyze(&meta).is_none());
    }

    #[test]
    fn is_complex_should_return_empty_when_all_metrics_below_thresholds() {
        let warnings = is_complex(&below_threshold_metrics(), &ComplexityThresholds::default());
        assert!(warnings.is_empty());
    }

    #[test]
    fn is_complex_should_return_warning_for_each_threshold_exceeded() {
        let metrics = BlueprintMetrics {
            node_count: 201,
            event_tick_count: 2,
            cast_count: 11,
            dependency_depth: 21,
        };
        let warnings = is_complex(&metrics, &ComplexityThresholds::default());
        assert_eq!(warnings.len(), 4);
        let rules: Vec<&str> = warnings.iter().map(|w| w.rule).collect();
        assert!(rules.contains(&"blueprint/node-count"));
        assert!(rules.contains(&"blueprint/event-tick"));
        assert!(rules.contains(&"blueprint/cast-count"));
        assert!(rules.contains(&"blueprint/dependency-depth"));
    }

    #[test]
    fn is_complex_should_return_node_count_warning_when_only_node_count_exceeds_threshold() {
        let metrics = BlueprintMetrics {
            node_count: 201,
            event_tick_count: 0,
            cast_count: 0,
            dependency_depth: 0,
        };
        let warnings = is_complex(&metrics, &ComplexityThresholds::default());
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].rule, "blueprint/node-count");
    }

    #[test]
    fn is_complex_should_not_warn_when_metrics_equal_thresholds() {
        let thresholds = ComplexityThresholds::default();
        let metrics = BlueprintMetrics {
            node_count: thresholds.max_node_count,
            event_tick_count: thresholds.max_event_tick_count,
            cast_count: thresholds.max_cast_count,
            dependency_depth: thresholds.max_dependency_depth,
        };
        // equal to threshold is not over — no warnings
        assert!(is_complex(&metrics, &thresholds).is_empty());
    }
}
