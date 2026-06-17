use std::collections::HashMap;

use uasset_lens_analysis::{LintRule, Severity};
use uasset_lens_shared::AssetType;

use crate::config::{CheckSeverity, LintConfig};

fn parse_asset_type(s: &str) -> Option<AssetType> {
    match s {
        "Texture2D" => Some(AssetType::Texture2D),
        "Material" => Some(AssetType::Material),
        "StaticMesh" => Some(AssetType::StaticMesh),
        "Blueprint" => Some(AssetType::Blueprint),
        "SkeletalMesh" => Some(AssetType::SkeletalMesh),
        "AnimBlueprint" => Some(AssetType::AnimBlueprint),
        "UserWidget" => Some(AssetType::UserWidget),
        "SoundWave" => Some(AssetType::SoundWave),
        "NiagaraSystem" => Some(AssetType::NiagaraSystem),
        "NiagaraEmitter" => Some(AssetType::NiagaraEmitter),
        "IKRigDefinition" => Some(AssetType::IKRigDefinition),
        "IKRetargeter" => Some(AssetType::IKRetargeter),
        "DialogueWave" => Some(AssetType::DialogueWave),
        _ => None,
    }
}

// Maps a configured `CheckSeverity` to the lint engine's `Severity`. `Off` never
// reaches here (the caller skips the rule), so it falls back to `default`.
fn to_severity(configured: Option<CheckSeverity>, default: Severity) -> Severity {
    match configured {
        Some(CheckSeverity::Error) => Severity::Error,
        Some(CheckSeverity::Warn) => Severity::Warning,
        _ => default,
    }
}

// A sub-rule runs only when enabled and not configured `off`.
fn rule_active(enabled: bool, severity: Option<CheckSeverity>) -> bool {
    enabled && severity != Some(CheckSeverity::Off)
}

pub fn build_lint_rules(cfg: &LintConfig) -> Vec<Box<dyn LintRule>> {
    let mut rules: Vec<Box<dyn LintRule>> = Vec::new();

    if rule_active(cfg.naming.enabled, cfg.naming.severity) {
        let mut naming = uasset_lens_analysis::NamingPrefixRule {
            severity: to_severity(cfg.naming.severity, Severity::Warning),
            ..Default::default()
        };
        // Each configured `*_prefix` replaces the built-in default Vec for that type.
        let overrides = [
            (&cfg.naming.texture_prefix, AssetType::Texture2D),
            (&cfg.naming.material_prefix, AssetType::Material),
            (
                &cfg.naming.material_function_prefix,
                AssetType::MaterialFunction,
            ),
            (&cfg.naming.static_mesh_prefix, AssetType::StaticMesh),
            (&cfg.naming.skeletal_mesh_prefix, AssetType::SkeletalMesh),
            (&cfg.naming.blueprint_prefix, AssetType::Blueprint),
            (&cfg.naming.widget_prefix, AssetType::UserWidget),
            (&cfg.naming.anim_bp_prefix, AssetType::AnimBlueprint),
            (&cfg.naming.sound_prefix, AssetType::SoundWave),
        ];
        for (prefix, asset_type) in overrides {
            if let Some(p) = prefix {
                naming.prefixes.insert(asset_type, vec![p.clone()]);
            }
        }
        rules.push(Box::new(naming));
    }

    if rule_active(cfg.blueprint.enabled, cfg.blueprint.severity) {
        let defaults = uasset_lens_analysis::ComplexityThresholds::default();
        let depth_by_type: HashMap<AssetType, u32> = cfg
            .blueprint
            .depth_by_type
            .iter()
            .filter_map(|(k, &v)| parse_asset_type(k).map(|t| (t, v)))
            .collect();
        let complexity = uasset_lens_analysis::BlueprintComplexityRule {
            thresholds: uasset_lens_analysis::ComplexityThresholds {
                max_node_count: cfg.blueprint.node_limit.unwrap_or(defaults.max_node_count),
                max_event_tick_count: cfg
                    .blueprint
                    .event_tick_limit
                    .unwrap_or(defaults.max_event_tick_count),
                max_cast_count: cfg.blueprint.cast_limit.unwrap_or(defaults.max_cast_count),
                max_dependency_depth: cfg
                    .blueprint
                    .dependency_depth_limit
                    .unwrap_or(defaults.max_dependency_depth),
            },
            depth_by_type,
            severity: to_severity(cfg.blueprint.severity, Severity::Error),
        };
        rules.push(Box::new(complexity));
    }

    rules
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{LintBlueprintConfig, LintConfig, LintNamingConfig};
    use std::collections::HashMap;
    use uasset_lens_shared::AssetType;

    #[test]
    fn build_lint_rules_should_use_defaults_when_naming_prefix_is_empty() {
        let rules = build_lint_rules(&LintConfig::default());
        let t_rock =
            uasset_lens_asset_db::make_record("/Game/Textures/T_Rock", AssetType::Texture2D);
        let bp_player =
            uasset_lens_asset_db::make_record("/Game/Blueprints/BP_Player", AssetType::Blueprint);
        for rule in &rules {
            assert!(rule.check(&t_rock, None).is_empty());
            assert!(rule.check(&bp_player, None).is_empty());
        }
    }

    #[test]
    fn build_lint_rules_should_apply_custom_naming_prefix_when_set_in_config() {
        let cfg = LintConfig {
            naming: LintNamingConfig {
                texture_prefix: Some("TX_".to_owned()),
                ..Default::default()
            },
            ..LintConfig::default()
        };
        let rules = build_lint_rules(&cfg);
        let tx_rock =
            uasset_lens_asset_db::make_record("/Game/Textures/TX_Rock", AssetType::Texture2D);
        let t_rock =
            uasset_lens_asset_db::make_record("/Game/Textures/T_Rock", AssetType::Texture2D);
        let ok_violations: Vec<_> = rules.iter().flat_map(|r| r.check(&tx_rock, None)).collect();
        let naming_violations: Vec<_> = rules.iter().flat_map(|r| r.check(&t_rock, None)).collect();
        assert!(ok_violations.is_empty());
        assert!(!naming_violations.is_empty());
    }

    #[test]
    fn build_lint_rules_should_apply_blueprint_max_nodes_threshold_when_set_in_config() {
        let cfg = LintConfig {
            blueprint: LintBlueprintConfig {
                node_limit: Some(50),
                ..Default::default()
            },
            ..LintConfig::default()
        };
        let rules = build_lint_rules(&cfg);
        let bp = uasset_lens_asset_db::make_record("/Game/BP_Complex", AssetType::Blueprint);
        let metrics = uasset_lens_scanner::BlueprintMetrics {
            node_count: 51,
            event_tick_count: 0,
            cast_count: 0,
            dependency_depth: 0,
        };
        let violations: Vec<_> = rules
            .iter()
            .flat_map(|r| r.check(&bp, Some(&metrics)))
            .collect();
        assert!(!violations.is_empty());
    }

    #[test]
    fn build_lint_rules_should_ignore_unknown_asset_type_keys_in_depth_by_type() {
        let cfg = LintConfig {
            blueprint: LintBlueprintConfig {
                depth_by_type: HashMap::from([("UnknownType".to_owned(), 1u32)]),
                ..Default::default()
            },
            ..LintConfig::default()
        };
        // Unknown key is filtered out; rules still build and a well-named asset is clean.
        let rules = build_lint_rules(&cfg);
        let t_rock =
            uasset_lens_asset_db::make_record("/Game/Textures/T_Rock", AssetType::Texture2D);
        let ok_violations: Vec<_> = rules.iter().flat_map(|r| r.check(&t_rock, None)).collect();
        assert!(ok_violations.is_empty());
    }

    #[test]
    fn build_lint_rules_should_apply_blueprint_max_dependency_depth_when_set_in_config() {
        let cfg = LintConfig {
            blueprint: LintBlueprintConfig {
                dependency_depth_limit: Some(5),
                ..Default::default()
            },
            ..LintConfig::default()
        };
        let rules = build_lint_rules(&cfg);
        let bp = uasset_lens_asset_db::make_record("/Game/BP_Deep", AssetType::Blueprint);
        let metrics = uasset_lens_scanner::BlueprintMetrics {
            node_count: 0,
            event_tick_count: 0,
            cast_count: 0,
            dependency_depth: 6,
        };
        let violations: Vec<_> = rules
            .iter()
            .flat_map(|r| r.check(&bp, Some(&metrics)))
            .collect();
        assert!(
            violations
                .iter()
                .any(|v| v.rule_id == "blueprint/dependency-depth")
        );
    }

    #[test]
    fn build_lint_rules_should_apply_per_type_depth_when_blueprint_depth_by_type_set_in_config() {
        let cfg = LintConfig {
            blueprint: LintBlueprintConfig {
                depth_by_type: HashMap::from([("Blueprint".to_owned(), 5u32)]),
                ..Default::default()
            },
            ..LintConfig::default()
        };
        let rules = build_lint_rules(&cfg);
        let bp = uasset_lens_asset_db::make_record("/Game/BP_Deep", AssetType::Blueprint);
        let metrics = uasset_lens_scanner::BlueprintMetrics {
            node_count: 0,
            event_tick_count: 0,
            cast_count: 0,
            dependency_depth: 6, // exceeds per-type threshold of 5
        };
        let violations: Vec<_> = rules
            .iter()
            .flat_map(|r| r.check(&bp, Some(&metrics)))
            .collect();
        assert!(
            violations
                .iter()
                .any(|v| v.rule_id == "blueprint/dependency-depth")
        );
    }

    #[test]
    fn build_lint_rules_should_skip_blueprint_when_disabled() {
        let cfg = LintConfig {
            blueprint: LintBlueprintConfig {
                enabled: false,
                ..Default::default()
            },
            ..LintConfig::default()
        };
        let rules = build_lint_rules(&cfg);
        let bp = uasset_lens_asset_db::make_record("/Game/BP_Complex", AssetType::Blueprint);
        let metrics = uasset_lens_scanner::BlueprintMetrics {
            node_count: 9999,
            event_tick_count: 99,
            cast_count: 99,
            dependency_depth: 99,
        };
        let violations: Vec<_> = rules
            .iter()
            .flat_map(|r| r.check(&bp, Some(&metrics)))
            .collect();
        assert!(
            violations.is_empty(),
            "blueprint.enabled = false must produce no Blueprint violations"
        );
    }

    #[test]
    fn build_lint_rules_should_skip_naming_when_severity_is_off() {
        let cfg = LintConfig {
            naming: LintNamingConfig {
                severity: Some(CheckSeverity::Off),
                ..Default::default()
            },
            ..LintConfig::default()
        };
        let rules = build_lint_rules(&cfg);
        // Wrongly-named texture would normally violate the naming rule.
        let bad = uasset_lens_asset_db::make_record("/Game/Textures/Rock", AssetType::Texture2D);
        let violations: Vec<_> = rules.iter().flat_map(|r| r.check(&bad, None)).collect();
        assert!(
            violations.is_empty(),
            "naming severity = off must skip the naming rule"
        );
    }

    #[test]
    fn build_lint_rules_should_report_naming_violation_with_configured_severity() {
        let cfg = LintConfig {
            naming: LintNamingConfig {
                severity: Some(CheckSeverity::Warn),
                ..Default::default()
            },
            ..LintConfig::default()
        };
        let rules = build_lint_rules(&cfg);
        let bad = uasset_lens_asset_db::make_record("/Game/Textures/Rock", AssetType::Texture2D);
        let violations: Vec<_> = rules.iter().flat_map(|r| r.check(&bad, None)).collect();
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].severity, Severity::Warning);
    }

    #[test]
    fn build_lint_rules_should_report_naming_violation_with_error_severity_when_configured() {
        let cfg = LintConfig {
            naming: LintNamingConfig {
                severity: Some(CheckSeverity::Error),
                ..Default::default()
            },
            ..LintConfig::default()
        };
        let rules = build_lint_rules(&cfg);
        let bad = uasset_lens_asset_db::make_record("/Game/Textures/Rock", AssetType::Texture2D);
        let violations: Vec<_> = rules.iter().flat_map(|r| r.check(&bad, None)).collect();
        assert_eq!(violations.len(), 1);
        // Default naming severity is Warning, so Error proves the config value is applied.
        assert_eq!(violations[0].severity, Severity::Error);
    }

    #[test]
    fn build_lint_rules_should_apply_event_tick_limit_threshold() {
        let cfg = LintConfig {
            blueprint: LintBlueprintConfig {
                event_tick_limit: Some(2),
                ..Default::default()
            },
            ..LintConfig::default()
        };
        let rules = build_lint_rules(&cfg);
        let bp = uasset_lens_asset_db::make_record("/Game/BP_Tick", AssetType::Blueprint);
        let event_tick_violations = |n: u32| -> usize {
            let metrics = uasset_lens_scanner::BlueprintMetrics {
                node_count: 0,
                event_tick_count: n,
                cast_count: 0,
                dependency_depth: 0,
            };
            rules
                .iter()
                .flat_map(|r| r.check(&bp, Some(&metrics)))
                .filter(|v| v.rule_id == "blueprint/event-tick")
                .count()
        };
        // At the configured limit there is no violation. This is discriminating: the
        // built-in default of 1 would fire at count == 2, so passing proves the
        // configured limit of 2 is actually applied.
        assert_eq!(event_tick_violations(2), 0);
        // Above the configured limit a violation is reported.
        assert_eq!(event_tick_violations(3), 1);
    }
}
