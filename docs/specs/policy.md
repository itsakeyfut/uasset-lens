# Core Values and Project Policy

## Core Values

### Primary: Instant Impact Visibility

> "Answer 'what breaks if I delete or rename this asset?' without opening the UE editor."

UE's built-in Reference Viewer, Asset Audit, and Size Map all require the editor to be running.
uasset-lens analyzes dependency relationships and impact scope from the CLI alone — no editor required.
It directly addresses **"afraid to delete / afraid to rename"**, a problem every UE developer faces daily.

### Future: CI-Integrable UE Quality Gate

> "Automatically check asset quality before merging a PR."

Existing UE tools cannot be integrated into CI. uasset-lens fills that gap,
becoming a UE-specific quality gate that can be embedded in GitHub Actions and similar pipelines.
This value is nearly achieved automatically once the Primary goal is working.

---

## Project Policy

### Publication Policy

Publication readiness is assessed once the tool is working. Even before publication,
**documentation (README, specs) is maintained at production quality from day one.**

| Item | Policy |
|------|--------|
| License | Decided at publication time (MIT / Apache-2.0 are candidates) |
| Documentation | Maintained from the start, regardless of publication status |
| Distribution | Decided at publication time (`cargo install` / GitHub Releases are candidates) |

### Primary Target User

**Indie and solo developers** are the top priority.

- Prioritize zero-setup, ready-to-use CLI design
- Emphasize working out-of-the-box without a config file
- Team-oriented features (CI integration, multi-user config sharing, etc.) are added after solo features are stable

### Supported Platforms

**Cross-platform from day one** (Windows / macOS / Linux).

- Use `std::path::PathBuf` for all file paths; never mix in OS-specific separators
- CI runs multi-OS tests via GitHub Actions (Windows / macOS / Linux)
- Primary development is on Windows, but no OS-specific implementation is used

### GUI (Phase 6) Positioning

Treated as a **best-effort goal**. Not started until the CLI reaches sufficient completeness.

- Do not begin GUI implementation until Phase 1–4 CLI features are stable
- Using egui minimizes architectural constraints
- Visual value (e.g. dependency visualization) is maximized by the GUI, so it is not descoped
