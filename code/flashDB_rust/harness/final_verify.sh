#!/usr/bin/env bash
set -euo pipefail
mkdir -p reports

./harness/build_check.sh
./harness/test_all.sh
./harness/unsafe_audit.sh 10

PLACEHOLDERS="$(grep -RInE 'todo!\(|unimplemented!\(|panic!\("TODO|TODO: fake|placeholder' src tests 2>/dev/null || true)"
if [ -n "$PLACEHOLDERS" ]; then
  {
    echo "# Placeholder Failure"
    echo ""
    echo "$PLACEHOLDERS"
  } > reports/placeholder-failure.md
  echo "ERROR: placeholders found. See reports/placeholder-failure.md" >&2
  exit 1
fi

{
  echo "# Final Verification Report"
  echo ""
  echo "- Build: passed"
  echo "- Tests: passed"
  echo "- Unsafe audit: passed"
  echo "- Placeholder audit: passed"
} > reports/final-report.md

echo "Final verification passed. Wrote reports/final-report.md"
