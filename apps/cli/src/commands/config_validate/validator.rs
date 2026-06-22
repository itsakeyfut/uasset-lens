use super::{ConfigError, ConfigWarning, closest, find_line};

// Known field sets, maintained alongside `crate::config::*` and `uasset_lens_analysis::BudgetConfig`.
const SECTIONS: &[&str] = &["scan", "rules", "lint", "budget", "diff", "check"];
const SCAN_KEYS: &[&str] = &["exclude_paths", "external_roots", "content_root"];
const RULES_KEYS: &[&str] = &[
    "dead-assets",
    "circular-deps",
    "duplicate-assets",
    "redirectors",
];
const LINT_KEYS: &[&str] = &["naming", "blueprint"];
const NAMING_KEYS: &[&str] = &[
    "enabled",
    "severity",
    "texture_prefix",
    "material_prefix",
    "material_function_prefix",
    "static_mesh_prefix",
    "skeletal_mesh_prefix",
    "blueprint_prefix",
    "widget_prefix",
    "anim_bp_prefix",
    "sound_prefix",
];
const BLUEPRINT_KEYS: &[&str] = &[
    "enabled",
    "severity",
    "event_tick_limit",
    "cast_limit",
    "node_limit",
    "dependency_depth_limit",
    "depth_by_type",
];
const DIFF_KEYS: &[&str] = &["size_increase_threshold_pct"];
const CHECK_KEYS: &[&str] = &["baseline_path"];
const SEVERITIES: &[&str] = &["error", "warn", "off"];

struct Validator<'a> {
    raw: &'a str,
    errors: Vec<ConfigError>,
    warnings: Vec<ConfigWarning>,
}

impl<'a> Validator<'a> {
    fn err(&mut self, key: &str, section: &str, message: String) {
        self.errors.push(ConfigError {
            line: find_line(self.raw, key),
            section: section.to_string(),
            message,
        });
    }

    fn unknown(&mut self, key: &str, section: &str, known: &[&str]) {
        self.warnings.push(ConfigWarning {
            line: find_line(self.raw, key),
            section: section.to_string(),
            message: format!("unknown field '{key}'"),
            suggestion: closest(key, known).map(str::to_string),
        });
    }

    fn expect_bool(&mut self, key: &str, section: &str, val: &toml::Value) {
        if !val.is_bool() {
            self.err(
                key,
                section,
                format!("expected boolean, got {}", val.type_str()),
            );
        }
    }

    fn expect_str(&mut self, key: &str, section: &str, val: &toml::Value) {
        if !val.is_str() {
            self.err(
                key,
                section,
                format!("expected string, got {}", val.type_str()),
            );
        }
    }

    fn expect_int(&mut self, key: &str, section: &str, val: &toml::Value) {
        if val.as_integer().is_none() {
            self.err(
                key,
                section,
                format!("expected integer, got {}", val.type_str()),
            );
        }
    }

    fn expect_str_array(&mut self, key: &str, section: &str, val: &toml::Value) {
        match val.as_array() {
            Some(arr) if arr.iter().all(|e| e.is_str()) => {}
            Some(_) => self.err(key, section, "expected an array of strings".to_string()),
            None => self.err(
                key,
                section,
                format!("expected an array, got {}", val.type_str()),
            ),
        }
    }

    fn expect_severity(&mut self, key: &str, section: &str, val: &toml::Value) {
        match val.as_str() {
            Some(s) if SEVERITIES.contains(&s) => {}
            Some(s) => self.err(
                key,
                section,
                format!("invalid severity '{s}' (expected one of error, warn, off)"),
            ),
            None => self.err(
                key,
                section,
                format!("expected a severity string, got {}", val.type_str()),
            ),
        }
    }

    fn validate_root(&mut self, root: &toml::map::Map<String, toml::Value>) {
        for (key, val) in root {
            match key.as_str() {
                "scan" => self.validate_scan(key, val),
                "rules" => self.validate_rules(key, val),
                "lint" => self.validate_lint(key, val),
                "budget" => self.validate_budget(key, val),
                "diff" => self.validate_table(key, val, DIFF_KEYS, Self::validate_diff_key),
                "check" => self.validate_table(key, val, CHECK_KEYS, Self::validate_check_key),
                _ => self.unknown(key, "(top level)", SECTIONS),
            }
        }
    }

    /// Validates a fixed-key table section: known keys go through `each`, unknown keys warn.
    fn validate_table(
        &mut self,
        section: &str,
        val: &toml::Value,
        known: &[&str],
        each: fn(&mut Self, &str, &str, &toml::Value),
    ) {
        let Some(table) = val.as_table() else {
            self.err(
                section,
                section,
                format!("expected a table, got {}", val.type_str()),
            );
            return;
        };
        for (k, v) in table {
            if known.contains(&k.as_str()) {
                each(self, k, section, v);
            } else {
                self.unknown(k, section, known);
            }
        }
    }

    fn validate_scan(&mut self, section: &str, val: &toml::Value) {
        self.validate_table(section, val, SCAN_KEYS, |s, k, sec, v| match k {
            "exclude_paths" | "external_roots" => s.expect_str_array(k, sec, v),
            "content_root" => s.expect_str(k, sec, v),
            _ => {}
        });
    }

    fn validate_rules(&mut self, section: &str, val: &toml::Value) {
        self.validate_table(section, val, RULES_KEYS, |s, k, sec, v| {
            s.expect_severity(k, sec, v)
        });
    }

    fn validate_diff_key(&mut self, key: &str, section: &str, val: &toml::Value) {
        if key == "size_increase_threshold_pct" {
            self.expect_int(key, section, val);
        }
    }

    fn validate_check_key(&mut self, key: &str, section: &str, val: &toml::Value) {
        if key == "baseline_path" {
            self.expect_str(key, section, val);
        }
    }

    fn validate_lint(&mut self, section: &str, val: &toml::Value) {
        let Some(table) = val.as_table() else {
            self.err(
                section,
                section,
                format!("expected a table, got {}", val.type_str()),
            );
            return;
        };
        for (k, v) in table {
            match k.as_str() {
                "naming" => self.validate_naming("lint.naming", v),
                "blueprint" => self.validate_blueprint("lint.blueprint", v),
                _ => self.unknown(k, section, LINT_KEYS),
            }
        }
    }

    fn validate_naming(&mut self, section: &str, val: &toml::Value) {
        self.validate_table(section, val, NAMING_KEYS, |s, k, sec, v| match k {
            "enabled" => s.expect_bool(k, sec, v),
            "severity" => s.expect_severity(k, sec, v),
            _ => s.expect_str(k, sec, v), // the *_prefix fields
        });
    }

    fn validate_blueprint(&mut self, section: &str, val: &toml::Value) {
        let Some(table) = val.as_table() else {
            self.err(
                section,
                section,
                format!("expected a table, got {}", val.type_str()),
            );
            return;
        };
        for (k, v) in table {
            match k.as_str() {
                "enabled" => self.expect_bool(k, section, v),
                "severity" => self.expect_severity(k, section, v),
                "event_tick_limit" | "cast_limit" | "node_limit" | "dependency_depth_limit" => {
                    self.expect_int(k, section, v)
                }
                "depth_by_type" => self.validate_depth_by_type("lint.blueprint.depth_by_type", v),
                _ => self.unknown(k, section, BLUEPRINT_KEYS),
            }
        }
    }

    /// `[lint.blueprint.depth_by_type]` — dynamic asset-type keys, each an integer.
    fn validate_depth_by_type(&mut self, section: &str, val: &toml::Value) {
        let Some(table) = val.as_table() else {
            self.err(
                section,
                section,
                format!("expected a table, got {}", val.type_str()),
            );
            return;
        };
        for (asset, v) in table {
            self.expect_int(asset, &format!("{section}.{asset}"), v);
        }
    }

    /// `[budget]` — dynamic asset-type keys, each a table with a required `max_size` integer > 0.
    fn validate_budget(&mut self, section: &str, val: &toml::Value) {
        let Some(table) = val.as_table() else {
            self.err(
                section,
                section,
                format!("expected a table, got {}", val.type_str()),
            );
            return;
        };
        for (asset, v) in table {
            let sec = format!("{section}.{asset}");
            let Some(entry) = v.as_table() else {
                self.err(
                    asset,
                    &sec,
                    format!("expected a table with 'max_size', got {}", v.type_str()),
                );
                continue;
            };
            for (k, vv) in entry {
                if k == "max_size" {
                    match vv.as_integer() {
                        Some(n) if n > 0 => {}
                        Some(n) => self.err(asset, &sec, format!("value '{n}' must be > 0")),
                        None => self.err(
                            asset,
                            &sec,
                            format!("expected integer, got {}", vv.type_str()),
                        ),
                    }
                } else {
                    self.unknown(k, &sec, &["max_size"]);
                }
            }
            if !entry.contains_key("max_size") {
                self.err(asset, &sec, "missing required field 'max_size'".to_string());
            }
        }
    }
}

pub(super) fn validate(value: &toml::Value, raw: &str) -> (Vec<ConfigError>, Vec<ConfigWarning>) {
    let mut v = Validator {
        raw,
        errors: Vec::new(),
        warnings: Vec::new(),
    };
    if let Some(root) = value.as_table() {
        v.validate_root(root);
    } else {
        v.err("", "(top level)", "config root must be a table".to_string());
    }
    v.errors.sort_by_key(|e| e.line.unwrap_or(u32::MAX));
    v.warnings.sort_by_key(|w| w.line.unwrap_or(u32::MAX));
    (v.errors, v.warnings)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(raw: &str) -> (Vec<ConfigError>, Vec<ConfigWarning>) {
        let value: toml::Value = toml::from_str(raw).expect("test toml must parse");
        validate(&value, raw)
    }

    #[test]
    fn validate_should_accept_a_well_formed_config() {
        let (errors, warnings) = run(
            "[scan]\nexclude_paths = [\"Content/Dev/\"]\n\n[rules]\ndead-assets = \"warn\"\n\n[lint]\nnaming.enabled = true\nblueprint.node_limit = 200\n\n[budget]\nTexture2D.max_size = 8388608\n",
        );
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
    }

    #[test]
    fn validate_should_warn_on_unknown_top_level_section_with_suggestion() {
        let (errors, warnings) = run("[scn]\nexclude_paths = []\n");
        assert!(errors.is_empty());
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].suggestion.as_deref(), Some("scan"));
        assert_eq!(warnings[0].line, Some(1));
    }

    #[test]
    fn validate_should_warn_on_unknown_scan_key_with_suggestion() {
        let (errors, warnings) = run("[scan]\nexclude_path = []\n");
        assert!(errors.is_empty());
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].section, "scan");
        assert_eq!(warnings[0].suggestion.as_deref(), Some("exclude_paths"));
    }

    #[test]
    fn validate_should_error_on_zero_budget() {
        let (errors, _) = run("[budget]\nTexture2D.max_size = 0\n");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].section, "budget.Texture2D");
        assert!(
            errors[0].message.contains("must be > 0"),
            "{}",
            errors[0].message
        );
    }

    #[test]
    fn validate_should_error_on_budget_missing_max_size() {
        let (errors, _) = run("[budget]\nTexture2D = { foo = 1 }\n");
        assert!(
            errors
                .iter()
                .any(|e| e.message.contains("missing required field 'max_size'")),
            "{errors:?}"
        );
    }

    #[test]
    fn validate_should_error_on_invalid_severity() {
        let (errors, _) = run("[rules]\ndead-assets = \"errorr\"\n");
        assert_eq!(errors.len(), 1);
        assert!(
            errors[0].message.contains("invalid severity"),
            "{}",
            errors[0].message
        );
    }

    #[test]
    fn validate_should_error_on_wrong_type() {
        let (errors, _) = run("[lint]\nnaming.enabled = \"yes\"\n");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].section, "lint.naming");
        assert!(
            errors[0].message.contains("expected boolean"),
            "{}",
            errors[0].message
        );
    }

    #[test]
    fn validate_should_accept_dynamic_budget_and_depth_keys() {
        let (errors, warnings) = run(
            "[budget]\nMyCustomType.max_size = 1024\n\n[lint.blueprint.depth_by_type]\nAnimBlueprint = 20\n",
        );
        assert!(errors.is_empty(), "{errors:?}");
        assert!(warnings.is_empty(), "{warnings:?}");
    }
}
