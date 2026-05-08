# CI integration documentation

## Summary

Write the GitHub Actions sample workflow and supporting documentation for integrating
`uasset-lens` into a CI pipeline.
Complete when the sample workflow file runs `lint` and `graph --cycles-only` and
fails the pipeline on exit code 1.

## Design Notes

**Files to create:**

1. `docs/ci/github-actions.yml` — copy-pasteable workflow example:

```yaml
name: Asset Quality Gate

on: [push, pull_request]

jobs:
  asset-lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          lfs: true          # required if .uasset files are stored in Git LFS

      - name: Install uasset-lens
        run: cargo install uasset-lens

      - name: Scan assets
        run: uasset-lens scan ./YourProject

      - name: Check circular dependencies
        run: uasset-lens graph --cycles-only ./YourProject

      - name: Lint assets
        run: uasset-lens lint ./YourProject
```

2. `docs/ci/git-lfs-guide.md` — guidance on `.uasset` storage options:
   - Option A: Commit `.uasset` files directly (simple, works for small projects)
   - Option B: Git LFS (recommended for projects with many/large assets)
   - `.gitattributes` example for LFS tracking

3. `README.md` — add a "CI Integration" section linking to the above docs.

> **Note**: The workflow assumes `uasset-lens` is published to crates.io.
> Before Phase 5 ships, verify that `cargo install uasset-lens` works end-to-end.

## Requirements

- [ ] Create `docs/ci/github-actions.yml` with a working workflow example
- [ ] Create `docs/ci/git-lfs-guide.md` covering direct commit vs LFS trade-offs
- [ ] Add `## CI Integration` section to `README.md` linking to the docs
- [ ] Verify the example workflow runs on a real repository (test in a GitHub repo)
- [ ] Document that `lint` exits with code 1 on violations (enabling pipeline failure)

## Related

- Closes Phase 5
- Docs: `docs/roadmap/phase5/ROADMAP.md` — Task 4
