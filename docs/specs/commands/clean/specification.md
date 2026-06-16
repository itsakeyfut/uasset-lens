# `clean` Command — Specification

## Purpose

Delete confirmed dead assets from disk. Requires confirmation before deleting unless
`--yes` is given. Use `--dry-run` to preview what would be deleted without touching any files.

```bash
uasset-lens clean ./Project
uasset-lens clean ./Project --dry-run
```

---

## Exit Codes

| Code | Meaning |
|---|---|
| `0` | Always (unless execution error) |
| `2` | Execution error |

---

## Safety

`clean` only deletes assets that `dead-assets` would also report — assets with no
incoming references. It does not delete assets that are referenced by other assets.

The command always prompts for confirmation before deleting unless `-y` is given.
Use `--dry-run` to review the deletion list before committing.

---

## Text Output (dry run)

```
$ uasset-lens clean ./Project --dry-run

Would delete (3 assets, 10.8 MB):
  /Game/Unused/T_OldRock       (Texture2D,   2.1 MB)
  /Game/Unused/SK_OldEnemy     (SkeletalMesh, 8.4 MB)
  /Game/Unused/BP_Test         (Blueprint,    0.3 MB)

Dry run: no files deleted.
```

---

## Text Output (with confirmation)

```
$ uasset-lens clean ./Project

Will delete (3 assets, 10.8 MB):
  /Game/Unused/T_OldRock       (Texture2D,   2.1 MB)
  /Game/Unused/SK_OldEnemy     (SkeletalMesh, 8.4 MB)
  /Game/Unused/BP_Test         (Blueprint,    0.3 MB)

Delete these 3 assets from disk? [y/N]: y

Deleted: /Game/Unused/T_OldRock
Deleted: /Game/Unused/SK_OldEnemy
Deleted: /Game/Unused/BP_Test

Cleaned: 3 assets deleted (10.8 MB freed)
```

With `-y`, the confirmation prompt is skipped.

---

## Filtering

`--min-size`, `--exclude`, and `--path` apply the same filters as `dead-assets` to
narrow the deletion target set.

```bash
# Delete only Texture2D dead assets 1 MB or larger, outside Dev/
uasset-lens clean ./Project --min-size 1048576 --exclude Dev
```
