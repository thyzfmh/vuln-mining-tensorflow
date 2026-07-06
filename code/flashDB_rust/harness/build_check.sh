#!/usr/bin/env bash
set -euo pipefail
mkdir -p reports
{
  echo "# Build Check"
  echo ""
  echo "## cargo fmt --check"
  if cargo fmt --version >/dev/null 2>&1; then
    cargo fmt --check
  else
    echo "WARN: rustfmt is not installed; skipping formatting check."
  fi
  echo ""
  echo "## cargo check --all-targets"
  cargo check --all-targets
} 2>&1 | tee reports/build-check.log
