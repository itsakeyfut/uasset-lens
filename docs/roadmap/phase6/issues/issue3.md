# `crates/report-generator` — Markdown report generation

## Summary

Create the `report-generator` crate with report configuration types and implement
Markdown report generation.
Complete when `generate_markdown()` produces a valid GitHub Flavored Markdown report
with a summary section and dead asset table.

## Design Notes

**Types:**

```rust
pub enum ReportFormat { Html, Markdown }

pub enum ReportSection {
    Summary, DeadAssets, Cycles, BlueprintMetrics, Budget, Duplicates, Levels,
}

pub struct ReportConfig {
    pub format:           ReportFormat,
    pub output_path:      PathBuf,
    pub include_sections: Vec<ReportSection>,
}

pub struct ReportData {
    pub project_name:  String,
    pub generated_at:  String,          // ISO 8601
    pub total_assets:  usize,
    pub dead_assets:   Vec<AssetRecord>,
    pub cycles:        Vec<Vec<AssetPath>>,
    // ... (other sections filled in as needed)
}
```

**Markdown output structure:**

```markdown
# uasset-lens Report — MyProject
Generated: 2026-05-08T12:34:56Z  |  Total assets: 523

## Summary
| Metric | Value |
|--------|-------|
| Dead assets | 12 |
| Cycles | 2 |

## Dead Assets (12)
| Asset Path | Type | Size |
|---|---|---|
| /Game/Textures/T_Old | Texture2D | 2.1 MB |
```

Use only string formatting — no external template engine.

## Requirements

- [ ] Create `crates/report-generator` crate
- [ ] Define `ReportFormat`, `ReportSection`, `ReportConfig`, `ReportData` structs
- [ ] Implement `generate_markdown(data: &ReportData, config: &ReportConfig) -> String`
- [ ] Include Summary section (total assets, dead count, cycle count) as GFM table
- [ ] Include Dead Assets section as GFM table (path / type / size)
- [ ] Include Cycles section as numbered list
- [ ] Omit sections not listed in `config.include_sections`
- [ ] Unit test: empty data → report string is non-empty and contains the project name
- [ ] Unit test: 2 dead assets → Dead Assets section has 2 rows

## Related

- Next: #4 — HTML report generation
- Used by: #5 (report command)
