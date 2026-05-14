# Storing `.uasset` Files in Git

Unreal Engine binary assets (`.uasset`, `.umap`) are large binary files. How you store
them in Git affects CI checkout speed and repository size.

---

## Option A — Direct Commit

Commit `.uasset` files directly into the Git object store.

**When to use:**
- Small to medium projects (total asset binary size under ~500 MB)
- Teams without Git LFS infrastructure set up
- Fastest local setup — no extra tooling needed

**CI checkout:**

```yaml
- uses: actions/checkout@v4
# No lfs: true needed
```

**Trade-offs:**

| Pro | Con |
|-----|-----|
| Zero extra setup | Repository size grows with every asset commit |
| Works with any Git host | `git clone` gets slower as history accumulates |
| | Older asset versions are never pruned automatically |

---

## Option B — Git LFS (Recommended for Large Projects)

Store binary files in Git LFS, keeping the Git object store lean.

**When to use:**
- Projects with many or large assets (> ~500 MB total)
- Teams already using Git LFS for other binary files

**Setup:**

```bash
git lfs install
```

Add to `.gitattributes` at the project root:

```
*.uasset filter=lfs diff=lfs merge=lfs -text
*.umap   filter=lfs diff=lfs merge=lfs -text
```

**CI checkout:**

```yaml
- uses: actions/checkout@v4
  with:
    lfs: true   # downloads LFS objects during checkout
```

**Trade-offs:**

| Pro | Con |
|-----|-----|
| Git history stays small | Requires Git LFS on every developer machine |
| Faster `git clone` for non-asset work | LFS storage costs money on most hosts |
| Old versions can be pruned with `git lfs prune` | Extra setup for new contributors |

---

## Exit Codes

Understanding exit codes is important for CI pipeline design:

| Command | Exit 0 | Exit 1 | Exit 2 |
|---------|--------|--------|--------|
| `uasset-lens scan` | Always (indexing only) | — | IO / parse error |
| `uasset-lens graph --cycles-only` | No cycles | Cycles found | IO / DB error |
| `uasset-lens lint` | No violations | Violations found | IO / DB error |
| `uasset-lens dead-assets` | No dead assets | Dead assets found | IO / DB error |

`exit 1` causes a GitHub Actions step to fail, which fails the job and blocks the PR.
`exit 2` indicates a tool error (missing DB, IO failure) and also fails the job.

Use `scan` first to build the index, then run analysis commands. `scan` itself never
exits 1 — it always succeeds or errors with exit 2.
