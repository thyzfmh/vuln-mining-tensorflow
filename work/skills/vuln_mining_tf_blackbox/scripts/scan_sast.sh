#!/usr/bin/env bash
# scan_sast.sh — run SAST static pattern scanning on the target
set -euo pipefail

TARGET="${1:?usage: $0 <target_path>}"

if [ ! -d "$TARGET" ]; then
  echo "ERROR: target path does not exist: $TARGET" >&2
  exit 1
fi

mkdir -p reports

echo "=== SAST Static Pattern Scan ==="
echo "Target: $TARGET"
echo ""

{
  echo "# SAST Scan Report"
  echo ""
  echo "Target: \`$TARGET\`"
  echo "Date: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo ""

  # Scan patterns
  echo "## Pattern: Division without zero-check"
  echo ""
  echo "\`\`\`"
  grep -rn 'shape\.dim_size\|ksize\|strides\|window\|stride' "$TARGET/tensorflow/core/kernels/" --include='*.cc' -l 2>/dev/null | head -20 || echo "(no matches)"
  echo "\`\`\`"
  echo ""

  echo "## Pattern: Unchecked pointer dereference"
  echo ""
  echo "\`\`\`"
  grep -rn '->' "$TARGET/tensorflow/core/kernels/" --include='*.cc' -l 2>/dev/null | head -20 || echo "(no matches)"
  echo "\`\`\`"
  echo ""

  echo "## Pattern: Integer overflow in size calculation"
  echo ""
  echo "\`\`\`"
  grep -rn 'static_cast<int\|static_cast<int32\|reinterpret_cast' "$TARGET/tensorflow/core/kernels/" --include='*.cc' -l 2>/dev/null | head -20 || echo "(no matches)"
  echo "\`\`\`"
  echo ""

  echo "## Pattern: Uncapped allocation"
  echo ""
  echo "\`\`\`"
  grep -rn 'Allocate\|->New\|new ' "$TARGET/tensorflow/core/kernels/" --include='*.cc' -l 2>/dev/null | head -20 || echo "(no matches)"
  echo "\`\`\`"
  echo ""

  echo "## Pattern: Missing error handling"
  echo ""
  echo "\`\`\`"
  grep -rn 'Status ' "$TARGET/tensorflow/core/kernels/" --include='*.cc' -l 2>/dev/null | head -20 || echo "(no matches)"
  echo "\`\`\`"
  echo ""

  TOTAL_CC="$(find "$TARGET/tensorflow/core/kernels/" -name '*.cc' 2>/dev/null | wc -l | tr -d ' ' || true)"
  echo "## Summary"
  echo ""
  echo "- Total .cc files in core/kernels/: $TOTAL_CC"
  echo "- Next step: Run LLM semantic analysis on high-signal files"

} > reports/sast-scan-report.md

echo "Wrote reports/sast-scan-report.md"
echo ""
echo "SAST scan complete. Review reports/sast-scan-report.md for high-signal files."
