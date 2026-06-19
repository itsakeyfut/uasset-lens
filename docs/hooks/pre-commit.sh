#!/usr/bin/env bash
# uasset-lens git pre-commit hook.
# Runs lint and budget checks on staged UE5 assets and blocks the commit on violations.
# Install: cp docs/hooks/pre-commit.sh .git/hooks/pre-commit && chmod +x .git/hooks/pre-commit
set -euo pipefail

# UE project directory (the folder containing Content/). Override with UASSET_LENS_PROJECT_DIR.
PROJECT_DIR="${UASSET_LENS_PROJECT_DIR:-.}"

# Collect staged .uasset / .umap files (added, copied, or modified).
STAGED=$(git diff --cached --name-only --diff-filter=ACM | grep -E '\.(uasset|umap)$' || true)

if [ -z "$STAGED" ]; then
  exit 0
fi

if ! command -v uasset-lens >/dev/null 2>&1; then
  echo "uasset-lens: command not found — install it to enable asset checks." >&2
  exit 2
fi

echo "uasset-lens: checking $(echo "$STAGED" | wc -l | tr -d ' ') staged asset(s)..."

# A non-zero exit (1 = violations, 2 = execution error) propagates via set -e and blocks the commit.
uasset-lens check "$PROJECT_DIR" \
  --only lint,budget \
  --skip-scan \
  --fail-on error
