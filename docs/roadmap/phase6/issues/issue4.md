# `crates/report-generator` — HTML report generation

## Summary

Implement HTML report generation in `report-generator`, producing a self-contained
HTML file with inline CSS that works offline without any CDN dependency.
Complete when `generate_html()` produces a valid HTML file renderable in a browser
with no network requests.

## Design Notes

**Self-contained requirement:** all CSS must be inlined in `<style>` tags.
No `<link>` to external stylesheets, no `<script src="...">` pointing to CDNs.

**Structure:**

```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <title>uasset-lens Report — {project_name}</title>
  <style>/* inline CSS here */</style>
</head>
<body>
  <h1>uasset-lens Report — {project_name}</h1>
  <p>Generated: {timestamp} | Total assets: {total}</p>

  <section id="summary">...</section>
  <section id="dead-assets">...</section>
  <section id="cycles">...</section>
</body>
</html>
```

Keep CSS minimal — clean typography, a table style, and a color for violation severity.
Use `const CSS: &str = "..."` embedded directly in the Rust source.

**HTML escaping:** escape `<`, `>`, `&`, `"` in all user-supplied strings.

## Requirements

- [ ] Implement `generate_html(data: &ReportData, config: &ReportConfig) -> String`
- [ ] Embed minimal CSS as a compile-time `&str` constant (no file reads at runtime)
- [ ] HTML-escape all asset paths, names, and other user-supplied strings
- [ ] Include same sections as Markdown (Summary, Dead Assets, Cycles) as HTML tables
- [ ] Unit test: output contains `<!DOCTYPE html>` and `</html>` (valid structure)
- [ ] Unit test: no `http://` or `https://` URLs in the output (fully offline)
- [ ] Unit test: asset path containing `<` is HTML-escaped in the output

## Related

- Depends on: #3 (ReportData, ReportConfig types)
- Used by: #5 (report command)
