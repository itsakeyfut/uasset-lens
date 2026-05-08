# `crates/cli` — `report` command

## Summary

Implement the `report` command that generates an HTML or Markdown analysis report
from the indexed asset data.
Complete when `uasset-lens report ./Project --format html -o report.html` produces
a file that opens correctly in a browser.

## Design Notes

**Options:**

```
report <project_dir>
  --format   <html|markdown>  (default: markdown)
  -o / --output <path>        (default: uasset-lens-report.md or .html)
  --sections <s1,s2,...>      (default: all sections)
```

**Flow:**

```
load all required data from DB:
  db.all_assets()
  dead_asset_detector::detect(&graph)
  dependency_graph::find_cycles()
  (other sections if requested)

→ build ReportData
→ generate_markdown() or generate_html() depending on --format
→ write to output path
```

**Overwrite protection:** if the output file already exists and `-y` is not set,
prompt `"Overwrite <path>? [y/N]"`.

**Exit codes:** success → 0; execution error → 2.

## Requirements

- [ ] Implement `report` command handler
- [ ] Parse `--format`, `--output`, `--sections` options
- [ ] Load dead assets, cycles, and other requested data from DB
- [ ] Build `ReportData` struct
- [ ] Call `generate_markdown()` or `generate_html()` based on `--format`
- [ ] Write output to file at `--output` path
- [ ] Prompt overwrite confirmation when file exists and `-y` not set
- [ ] Exit code 0 on success, 2 on error

## Related

- Depends on: #3 and #4 (report-generator), Phase 2 Issues #4, #6 (dead assets, cycles)
- Docs: `docs/roadmap/phase6/ROADMAP.md` — Task 3
