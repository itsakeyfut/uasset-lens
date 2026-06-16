# Perforce Integration

## Overview

`uasset-lens` supports Perforce (Helix Core) as an alternative to git for version control
integration. Perforce is common in game studios with large binary assets.

Currently, Perforce integration covers:
- Changelist-aware scanning (scan only checked-out or recently synced files)
- Reporting violations as Perforce shelf annotations

---

## Changelist-Based Diff Scanning

Instead of `git diff HEAD`, Perforce users can scope analysis to the current default
changelist or a named changelist.

```bash
uasset-lens scan ./Project --p4-changelist default
uasset-lens scan ./Project --p4-changelist 12345
uasset-lens check ./Project --p4-changelist 12345
```

The `--p4-changelist` flag:
1. Runs `p4 opened -c <CL>` to get the list of files in the changelist.
2. Filters scan to only those `.uasset` / `.umap` files (ignores source code changes).
3. Runs the normal mtime delta scan on the filtered file list.

If `p4` is not on `PATH`, the flag returns an error: `exit 2, "p4 command not found"`.

---

## Workspace Root Detection

`uasset-lens` detects a Perforce workspace by looking for `p4config` or `.p4config` in
the directory tree above `<project_dir>`. When found:

- The P4ROOT is used as the workspace root for path normalization.
- `p4 where` is called to map depot paths to local paths when needed.

If no Perforce workspace is detected, Perforce features are silently disabled.

---

## Sync Change Detection (`--p4-after-sync`)

After a `p4 sync`, `mtime` is reset to the sync time — not the original edit time.
This makes mtime-based delta scan unreliable.

```bash
uasset-lens scan ./Project --p4-after-sync
```

`--p4-after-sync` forces `--full-scan` behavior but only on files that `p4 have` reports
as having changed since the previous `p4 have` snapshot stored in the DB.

Implementation: The DB stores the last known P4 change number. On `--p4-after-sync`,
`uasset-lens` runs `p4 changes //depot/...@<last_cl>,#head` to find changed files.

---

## Config Integration

```toml
[scan]
# Enable Perforce-aware mode globally (auto-detects workspace)
perforce = true

# Map depot paths to content paths (optional override)
perforce_depot_path = "//depot/MyProject/Content/..."
```

When `perforce = true`:
- Perforce workspace detection is run on startup.
- `--p4-after-sync` behavior is applied automatically if the workspace has changed
  since the last scan.

---

## Relationship to git Integration

Perforce and git modes are mutually exclusive per project directory:
- If a `.git` directory is found above `<project_dir>`, git mode is used.
- If a `.p4config` / `P4CONFIG` file is found, Perforce mode is used.
- If both are found, git takes precedence (with a warning on stderr).
- If neither is found, version control integration features are unavailable.

---

## Limitations (v0.4.0)

- Only read-only Perforce operations are used (`p4 opened`, `p4 have`, `p4 changes`).
  `uasset-lens` never submits, checks out, or modifies depot state.
- Streams workspaces are supported; classic branch-spec workspaces are not tested.
- SSL-enabled Perforce servers (P4PORT starting with `ssl:`) are supported if the
  `p4` CLI is configured with the correct trust fingerprint.
