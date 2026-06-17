//! SARIF 2.1.0 output for the `check`, `lint`, and `budget` commands.
//!
//! Each command converts its violations into format-agnostic [`SarifFinding`]s and calls
//! [`to_sarif_json`], which builds a SARIF document suitable for upload to GitHub Advanced
//! Security. See `docs/specs/integrations/sarif.md`.

use anyhow::Context;

const SCHEMA: &str = "https://schemastore.azurewebsites.net/schemas/json/sarif-2.1.0-rtm.5.json";
const VERSION: &str = "2.1.0";
const DRIVER_NAME: &str = "uasset-lens";
const INFORMATION_URI: &str = "https://github.com/itsakeyfut/uasset-lens";
const SRC_ROOT: &str = "%SRCROOT%";

/// SARIF result severity. Maps directly from a finding's own severity.
#[derive(Debug, Clone, Copy)]
pub(crate) enum SarifLevel {
    Error,
    Warning,
}

impl SarifLevel {
    fn as_str(self) -> &'static str {
        match self {
            SarifLevel::Error => "error",
            SarifLevel::Warning => "warning",
        }
    }
}

/// One violation in command-agnostic form. `uri` is the project-root-relative, forward-slash
/// filesystem path; `None` when the finding has no resolvable file (e.g. a duplicate group).
pub(crate) struct SarifFinding {
    pub rule_id: String,
    pub level: SarifLevel,
    pub message: String,
    pub uri: Option<String>,
}

#[derive(serde::Serialize)]
struct SarifLog {
    #[serde(rename = "$schema")]
    schema: &'static str,
    version: &'static str,
    runs: Vec<Run>,
}

#[derive(serde::Serialize)]
struct Run {
    tool: Tool,
    results: Vec<SarifResult>,
}

#[derive(serde::Serialize)]
struct Tool {
    driver: Driver,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct Driver {
    name: &'static str,
    version: &'static str,
    information_uri: &'static str,
    rules: Vec<ReportingDescriptor>,
}

#[derive(serde::Serialize)]
struct ReportingDescriptor {
    id: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifResult {
    rule_id: String,
    level: &'static str,
    message: Message,
    locations: Vec<Location>,
}

#[derive(serde::Serialize)]
struct Message {
    text: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct Location {
    physical_location: PhysicalLocation,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct PhysicalLocation {
    artifact_location: ArtifactLocation,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ArtifactLocation {
    uri: String,
    uri_base_id: &'static str,
}

/// Serializes `findings` into a pretty-printed SARIF 2.1.0 document. `rules` lists each distinct
/// rule id that produced a result; a zero-finding run still yields a valid document.
pub(crate) fn to_sarif_json(findings: &[SarifFinding]) -> anyhow::Result<String> {
    let mut rule_ids: Vec<&str> = findings.iter().map(|f| f.rule_id.as_str()).collect();
    rule_ids.sort_unstable();
    rule_ids.dedup();
    let rules = rule_ids
        .into_iter()
        .map(|id| ReportingDescriptor { id: id.to_owned() })
        .collect();

    let results = findings
        .iter()
        .map(|f| SarifResult {
            rule_id: f.rule_id.clone(),
            level: f.level.as_str(),
            message: Message {
                text: f.message.clone(),
            },
            locations: f
                .uri
                .as_ref()
                .map(|uri| {
                    vec![Location {
                        physical_location: PhysicalLocation {
                            artifact_location: ArtifactLocation {
                                uri: uri.clone(),
                                uri_base_id: SRC_ROOT,
                            },
                        },
                    }]
                })
                .unwrap_or_default(),
        })
        .collect();

    let log = SarifLog {
        schema: SCHEMA,
        version: VERSION,
        runs: vec![Run {
            tool: Tool {
                driver: Driver {
                    name: DRIVER_NAME,
                    version: env!("CARGO_PKG_VERSION"),
                    information_uri: INFORMATION_URI,
                    rules,
                },
            },
            results,
        }],
    };

    serde_json::to_string_pretty(&log).context("Failed to serialize SARIF output")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(findings: &[SarifFinding]) -> serde_json::Value {
        serde_json::from_str(&to_sarif_json(findings).unwrap()).unwrap()
    }

    #[test]
    fn to_sarif_json_should_build_a_valid_document_for_findings() {
        let findings = vec![
            SarifFinding {
                rule_id: "naming/prefix".to_owned(),
                level: SarifLevel::Error,
                message: "Asset 'Character' is missing required prefix 'BP_'".to_owned(),
                uri: Some("Content/Characters/Character.uasset".to_owned()),
            },
            SarifFinding {
                rule_id: "budget/texture2d".to_owned(),
                level: SarifLevel::Error,
                message: "Texture2D exceeds budget".to_owned(),
                uri: Some("Content/T_Rock.uasset".to_owned()),
            },
        ];
        let v = parse(&findings);

        assert_eq!(
            v["$schema"],
            "https://schemastore.azurewebsites.net/schemas/json/sarif-2.1.0-rtm.5.json"
        );
        assert_eq!(v["version"], "2.1.0");
        let driver = &v["runs"][0]["tool"]["driver"];
        assert_eq!(driver["name"], "uasset-lens");
        assert_eq!(driver["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(
            driver["informationUri"],
            "https://github.com/itsakeyfut/uasset-lens"
        );
        // Distinct rule ids, sorted.
        assert_eq!(driver["rules"][0]["id"], "budget/texture2d");
        assert_eq!(driver["rules"][1]["id"], "naming/prefix");

        let result = &v["runs"][0]["results"][0];
        assert_eq!(result["ruleId"], "naming/prefix");
        assert_eq!(result["level"], "error");
        assert_eq!(
            result["message"]["text"],
            "Asset 'Character' is missing required prefix 'BP_'"
        );
        let loc = &result["locations"][0]["physicalLocation"]["artifactLocation"];
        assert_eq!(loc["uri"], "Content/Characters/Character.uasset");
        assert_eq!(loc["uriBaseId"], "%SRCROOT%");
    }

    #[test]
    fn to_sarif_json_should_map_warning_level() {
        let findings = vec![SarifFinding {
            rule_id: "dead-assets".to_owned(),
            level: SarifLevel::Warning,
            message: "unreferenced asset".to_owned(),
            uri: Some("Content/Unused/T_Old.uasset".to_owned()),
        }];
        let v = parse(&findings);
        assert_eq!(v["runs"][0]["results"][0]["level"], "warning");
    }

    #[test]
    fn to_sarif_json_should_produce_empty_results_and_rules_when_no_findings() {
        let v = parse(&[]);
        assert_eq!(v["version"], "2.1.0");
        assert_eq!(v["runs"][0]["results"].as_array().unwrap().len(), 0);
        assert_eq!(
            v["runs"][0]["tool"]["driver"]["rules"]
                .as_array()
                .unwrap()
                .len(),
            0
        );
    }

    #[test]
    fn to_sarif_json_should_omit_locations_when_uri_is_none() {
        let findings = vec![SarifFinding {
            rule_id: "duplicate-assets".to_owned(),
            level: SarifLevel::Warning,
            message: "[same-name] Rock".to_owned(),
            uri: None,
        }];
        let v = parse(&findings);
        assert_eq!(
            v["runs"][0]["results"][0]["locations"]
                .as_array()
                .unwrap()
                .len(),
            0
        );
    }
}
