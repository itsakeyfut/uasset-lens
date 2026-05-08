# `crates/cli` — `duplicates` command

## Summary

Implement the `duplicates` command that lists same-name and texture duplicate asset groups.
Complete when `uasset-lens duplicates ./Project` outputs grouped duplicate assets.

## Design Notes

**Flow:**

```
db.all_assets()
→ duplicate_detector::detect_by_name(&assets) → Vec<DuplicateGroup>
→ duplicate_detector::detect_texture_duplicates(&assets) → Vec<DuplicateGroup>
→ merge and deduplicate groups
→ output
```

**Text output:**

```
Duplicate Assets
================
[Same name] T_Rock (3 copies)
  /Game/Characters/T_Rock
  /Game/Environment/T_Rock
  /Game/Shared/T_Rock

[Texture duplicate] T_Ground_D (2.1 MB × 2)
  /Game/Landscape/T_Ground_D
  /Game/Outdoor/T_Ground_D

4 duplicate groups found.
```

**JSON output:**

```json
[
  {"type": "same-name",  "name": "T_Rock",     "assets": ["/Game/Characters/T_Rock", ...]},
  {"type": "texture-dup","name": "T_Ground_D", "assets": [...]}
]
```

**Exit codes:** duplicates found → 1; none → 0; execution error → 2.

## Requirements

- [ ] Implement `duplicates` command handler
- [ ] Run both `detect_by_name()` and `detect_texture_duplicates()`
- [ ] Merge results, labeling each group with its detection type
- [ ] Implement text output grouped by detection type
- [ ] Implement JSON output
- [ ] Exit code 1 when duplicates found, 0 when none

## Related

- Depends on: #4 (detect_by_name), #5 (detect_texture_duplicates)
- Closes Phase 4
