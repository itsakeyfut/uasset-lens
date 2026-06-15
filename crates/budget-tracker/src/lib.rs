use std::collections::HashMap;

use asset_db::AssetRecord;
use shared::{AssetPath, AssetType};

#[derive(Default, serde::Deserialize)]
pub struct BudgetConfig {
    #[serde(flatten)]
    pub limits: HashMap<String, AssetBudget>,
}

impl BudgetConfig {
    pub fn effective(user: &BudgetConfig) -> BudgetConfig {
        let mut limits = HashMap::new();
        limits.insert(
            "Texture2D".to_owned(),
            AssetBudget {
                max_size: 4 * 1024 * 1024,
            },
        );
        for (k, v) in &user.limits {
            limits.insert(
                k.clone(),
                AssetBudget {
                    max_size: v.max_size,
                },
            );
        }
        BudgetConfig { limits }
    }
}

#[derive(serde::Deserialize)]
pub struct AssetBudget {
    pub max_size: u64,
}

#[derive(Debug)]
pub struct BudgetReport {
    pub violations: Vec<BudgetViolation>,
}

#[derive(Debug)]
pub struct BudgetViolation {
    pub asset_path: AssetPath,
    pub asset_type: AssetType,
    pub file_size: u64,
    pub max_size: u64,
}

pub fn check_budget(assets: &[AssetRecord], config: &BudgetConfig) -> BudgetReport {
    let violations = assets
        .iter()
        .filter_map(|asset| {
            let key = asset.asset_type.to_string();
            let budget = config.limits.get(&key)?;
            if asset.file_size > budget.max_size {
                Some(BudgetViolation {
                    asset_path: asset.asset_path.clone(), // clone required: BudgetViolation owns the path
                    asset_type: asset.asset_type.clone(), // clone required: BudgetViolation owns the type
                    file_size: asset.file_size,
                    max_size: budget.max_size,
                })
            } else {
                None
            }
        })
        .collect();

    BudgetReport { violations }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn budget(max_size: u64) -> AssetBudget {
        AssetBudget { max_size }
    }

    #[test]
    fn check_budget_should_return_no_violations_when_all_assets_are_within_budget() {
        let config = BudgetConfig {
            limits: [("Texture2D".to_owned(), budget(4096))].into(),
        };
        let assets = vec![asset_db::AssetRecord {
            file_size: 4096,
            ..asset_db::make_record("/Game/T_Rock", AssetType::Texture2D)
        }];

        let report = check_budget(&assets, &config);

        assert!(report.violations.is_empty());
    }

    #[test]
    fn check_budget_should_report_violation_when_asset_is_one_byte_over_budget() {
        let config = BudgetConfig {
            limits: [("Texture2D".to_owned(), budget(4096))].into(),
        };
        let assets = vec![asset_db::AssetRecord {
            file_size: 4097,
            ..asset_db::make_record("/Game/T_Large", AssetType::Texture2D)
        }];

        let report = check_budget(&assets, &config);

        assert_eq!(report.violations.len(), 1);
        let v = &report.violations[0];
        assert_eq!(v.asset_path.as_str(), "/Game/T_Large");
        assert_eq!(v.file_size, 4097);
        assert_eq!(v.max_size, 4096);
    }

    #[test]
    fn check_budget_should_not_report_violation_when_asset_type_has_no_budget() {
        let config = BudgetConfig {
            limits: [("Texture2D".to_owned(), budget(4096))].into(),
        };
        let assets = vec![asset_db::AssetRecord {
            file_size: 1_000_000,
            ..asset_db::make_record("/Game/SM_Rock", AssetType::StaticMesh)
        }];

        let report = check_budget(&assets, &config);

        assert!(report.violations.is_empty());
    }

    #[test]
    fn check_budget_should_report_violations_per_type_with_independent_budgets() {
        let config = BudgetConfig {
            limits: [
                ("Texture2D".to_owned(), budget(4096)),
                ("SoundWave".to_owned(), budget(2048)),
            ]
            .into(),
        };
        let assets = vec![
            asset_db::AssetRecord {
                file_size: 4096,
                ..asset_db::make_record("/Game/T_OK", AssetType::Texture2D)
            },
            asset_db::AssetRecord {
                file_size: 5000,
                ..asset_db::make_record("/Game/T_Big", AssetType::Texture2D)
            },
            asset_db::AssetRecord {
                file_size: 2048,
                ..asset_db::make_record("/Game/SW_OK", AssetType::SoundWave)
            },
            asset_db::AssetRecord {
                file_size: 3000,
                ..asset_db::make_record("/Game/SW_Big", AssetType::SoundWave)
            },
        ];

        let report = check_budget(&assets, &config);

        assert_eq!(report.violations.len(), 2);
        assert!(
            report
                .violations
                .iter()
                .any(|v| v.asset_path.as_str() == "/Game/T_Big")
        );
        assert!(
            report
                .violations
                .iter()
                .any(|v| v.asset_path.as_str() == "/Game/SW_Big")
        );
    }

    #[test]
    fn effective_should_apply_texture2d_default_when_user_config_is_empty() {
        let user = BudgetConfig::default();
        let effective = BudgetConfig::effective(&user);
        assert_eq!(
            effective.limits.get("Texture2D").map(|b| b.max_size),
            Some(4 * 1024 * 1024)
        );
    }

    #[test]
    fn effective_should_override_default_when_texture2d_is_in_user_config() {
        let user = BudgetConfig {
            limits: [(
                "Texture2D".to_owned(),
                AssetBudget {
                    max_size: 8 * 1024 * 1024,
                },
            )]
            .into(),
        };
        let effective = BudgetConfig::effective(&user);
        assert_eq!(
            effective.limits.get("Texture2D").map(|b| b.max_size),
            Some(8 * 1024 * 1024)
        );
    }

    #[test]
    fn effective_should_include_both_default_texture2d_and_user_defined_types() {
        let user = BudgetConfig {
            limits: [(
                "SoundWave".to_owned(),
                AssetBudget {
                    max_size: 2 * 1024 * 1024,
                },
            )]
            .into(),
        };
        let effective = BudgetConfig::effective(&user);
        assert_eq!(
            effective.limits.get("Texture2D").map(|b| b.max_size),
            Some(4 * 1024 * 1024)
        );
        assert_eq!(
            effective.limits.get("SoundWave").map(|b| b.max_size),
            Some(2 * 1024 * 1024)
        );
    }
}
