# `crates/cli` — `redirectors` command

## Summary

Implement the `redirectors` command that lists all `ObjectRedirector` assets in the project.
Complete when `uasset-lens redirectors ./Project` outputs the redirector list and exits
with code 1 when redirectors are found.

## Design Notes

**Flow:**

```
load_graph(db) → redirector_analyzer::detect(&graph) → output
```

**Text output:**

```
ObjectRedirectors (3 found)
===========================
/Game/Characters/BP_Player_Old
/Game/Textures/T_Rock_Old
/Game/Maps/L_OldLevel_Old

Note: redirect target resolution is available in Phase 4 analysis.
```

**JSON output:**

```json
{
  "count": 3,
  "redirectors": [
    "/Game/Characters/BP_Player_Old",
    "/Game/Textures/T_Rock_Old"
  ]
}
```

**Exit codes:** redirectors found → 1; none → 0; execution error → 2.

## Requirements

- [ ] Implement `redirectors` command handler
- [ ] Call `redirector_analyzer::detect(&graph)` for the path list
- [ ] Implement text output with count header + path list
- [ ] Append `"Note: redirect target resolution is available in Phase 4 analysis."` to text output
- [ ] Implement JSON output matching the spec
- [ ] Exit code 1 when redirectors found, 0 when none

## Related

- Depends on: Issue #1 (redirector-analyzer), Phase 2 Issue #6 (load_graph)
- Docs: `docs/specs/cli-design.md` (redirectors output spec)
