# `crates/cli` — `lint` command

## Summary

Implement the `lint` command that runs all lint rules against indexed assets and
reports violations. Exit code 1 when violations are found, enabling CI gate usage.
Complete when `uasset-lens lint ./Project` reports naming and complexity violations
and exits with code 1.

## Design Notes

**Flow:**

```
load_config(project_dir)
→ build LintEngine with configured rules:
    NamingPrefixRule (from config.lint.naming_prefix)
    TextureSizeRule (from config.lint.max_sizes if present, else default)
    BlueprintComplexityRule (from config.lint.blueprint_max_*)
→ db.all_assets() + load BlueprintMetrics from DB
→ lint_engine.run(&assets, &metrics_map)
→ output violations
```

**Text output:**

```
Lint Results
============
WARNING  naming/prefix      /Game/Textures/Rock_D             expected prefix "T_"
ERROR    blueprint/node-count  /Game/Characters/BP_Boss        312 nodes (limit: 200)

2 violations found.
```

**JSON output:**

```json
[
  {"severity": "warning", "rule_id": "naming/prefix", "asset_path": "...", "message": "..."}
]
```

**Exit codes:** violations found → 1; clean → 0; execution error → 2.

This exit code behavior is essential for CI pipeline integration (`docs/roadmap/phase5/ROADMAP.md`).

## Requirements

- [ ] Implement `lint` command handler
- [ ] Load config, construct `LintEngine` with all 3 rule types
- [ ] Fetch all assets and their Blueprint metrics from DB
- [ ] Run `lint_engine.run()` and collect `Vec<LintViolation>`
- [ ] Implement text output (severity / rule_id / path / message table + total count)
- [ ] Implement JSON output (array of violation objects)
- [ ] Exit code 1 when violations > 0, 0 when clean

## Related

- Depends on: #6–#10 (lint-engine + rules + config)
- Docs: `docs/roadmap/phase4/ROADMAP.md` completion criteria
