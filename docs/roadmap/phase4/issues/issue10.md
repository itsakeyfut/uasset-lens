# `.uasset-lens.toml` — `[lint]` configuration section

## Summary

Extend `.uasset-lens.toml` with a `[lint]` section that configures naming prefixes
and Blueprint complexity thresholds, and wire these values into the lint rules.
Complete when custom prefixes and thresholds from the config file are applied by the
`lint` command.

## Design Notes

**New TOML schema:**

```toml
[lint]
naming_prefix.Texture2D  = "T_"
naming_prefix.Material   = "M_"
naming_prefix.StaticMesh = "SM_"
naming_prefix.Blueprint  = "BP_"

blueprint_max_nodes      = 200
blueprint_max_event_tick = 1
blueprint_max_cast_count = 10
```

**Extend `ConfigFile`:**

```rust
#[derive(Default, serde::Deserialize)]
pub struct ConfigFile {
    pub scan: ScanConfig,
    pub lint: LintConfig,
}

#[derive(Default, serde::Deserialize)]
pub struct LintConfig {
    pub naming_prefix:          HashMap<String, String>,
    pub blueprint_max_nodes:    Option<u32>,
    pub blueprint_max_event_tick: Option<u32>,
    pub blueprint_max_cast_count: Option<u32>,
}
```

The `lint` command reads `LintConfig` and constructs lint rules with overridden values,
falling back to defaults where not specified.

## Requirements

- [ ] Add `LintConfig` struct with `naming_prefix` map and Blueprint threshold options to `ConfigFile`
- [ ] Update `load_config()` to deserialize the `[lint]` section
- [ ] In `lint` command: build `NamingPrefixRule` using `config.lint.naming_prefix` (merged with defaults)
- [ ] In `lint` command: build `BlueprintComplexityRule` using `config.lint.blueprint_max_*` (with Option fallback to defaults)
- [ ] Unit test: config with `blueprint_max_nodes = 50` → threshold applied in lint run
- [ ] Unit test: config with custom `naming_prefix.Texture2D = "TX_"` → `TX_Rock` passes, `T_Rock` fails

## Related

- Depends on: Phase 3 Issue #2 (ConfigFile), #7 (naming rules), #9 (blueprint rules)
- Used by: #14 (lint command)
