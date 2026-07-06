#!/usr/bin/env bash
set -euo pipefail
mkdir -p reports
{
  echo "# Test Report"
  echo ""
  cargo test --all-targets -- --nocapture
} 2>&1 | tee reports/test-report.log
