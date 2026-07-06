#!/usr/bin/env bash
# analyze_target.sh — inventory the target codebase for vulnerability scanning
set -euo pipefail

TARGET="${1:?usage: $0 <target_path>}"

if [ ! -d "$TARGET" ]; then
  echo "ERROR: target path does not exist: $TARGET" >&2
  exit 1
fi

mkdir -p reports

CC_COUNT="$(find "$TARGET" -type f -name '*.cc' 2>/dev/null | wc -l | tr -d ' ')"
H_COUNT="$(find "$TARGET" -type f -name '*.h' 2>/dev/null | wc -l | tr -d ' ')"
CU_COUNT="$(find "$TARGET" -type f -name '*.cu' 2>/dev/null | wc -l | tr -d ' ')"
PY_COUNT="$(find "$TARGET" -type f -name '*.py' 2>/dev/null | wc -l | tr -d ' ')"
TOTAL=$((CC_COUNT + H_COUNT + CU_COUNT))

# Module breakdown
KERNELS_COUNT="0"
FRAMEWORK_COUNT="0"
PLATFORM_COUNT="0"
UTIL_COUNT="0"
if [ -d "$TARGET/tensorflow/core/kernels" ]; then
  KERNELS_COUNT="$(find "$TARGET/tensorflow/core/kernels" -type f -name '*.cc' 2>/dev/null | wc -l | tr -d ' ')"
fi
if [ -d "$TARGET/tensorflow/core/framework" ]; then
  FRAMEWORK_COUNT="$(find "$TARGET/tensorflow/core/framework" -type f -name '*.cc' 2>/dev/null | wc -l | tr -d ' ')"
fi
if [ -d "$TARGET/tensorflow/core/platform" ]; then
  PLATFORM_COUNT="$(find "$TARGET/tensorflow/core/platform" -type f -name '*.cc' 2>/dev/null | wc -l | tr -d ' ')"
fi
if [ -d "$TARGET/tensorflow/core/util" ]; then
  UTIL_COUNT="$(find "$TARGET/tensorflow/core/util" -type f -name '*.cc' 2>/dev/null | wc -l | tr -d ' ')"
fi

{
  echo "# Target Codebase Inventory"
  echo ""
  echo "- Target: \`$TARGET\`"
  echo "- .cc files: $CC_COUNT"
  echo "- .h files: $H_COUNT"
  echo "- .cu files: $CU_COUNT"
  echo "- Total C++ source files: $TOTAL"
  echo "- .py files: $PY_COUNT"
  echo ""
  echo "## Module Breakdown"
  echo ""
  echo "| Module | .cc Files | Risk Level |"
  echo "|--------|-----------|------------|"
  echo "| core/kernels/ | $KERNELS_COUNT | HIGH — processes external input |"
  echo "| core/framework/ | $FRAMEWORK_COUNT | HIGH — memory management |"
  echo "| core/platform/ | $PLATFORM_COUNT | MEDIUM — I/O operations |"
  echo "| core/util/ | $UTIL_COUNT | MEDIUM — utility functions |"
  echo ""
  echo "## High-Risk Areas"
  echo ""
  echo "### 1. Kernel Operations (Priority: Critical)"
  echo "- Path: \`tensorflow/core/kernels/\`"
  echo "- Risk: Input validation gaps, integer overflow, division by zero"
  echo "- Key files: pooling, conv, matmul, slice, gather ops"
  echo ""
  echo "### 2. Framework Layer (Priority: High)"
  echo "- Path: \`tensorflow/core/framework/\`"
  echo "- Risk: Null deref, use-after-free, type confusion"
  echo "- Key files: tensor, allocation, variant"
  echo ""
  echo "### 3. Platform/I/O (Priority: Medium)"
  echo "- Path: \`tensorflow/core/platform/\`, \`tensorflow/core/util/\`"
  echo "- Risk: Path traversal, resource exhaustion, parsing errors"
  echo ""
  echo "## SAST Pattern Counts"
  echo ""

  # Count high-risk patterns (use || true to prevent set -e failure on no matches)
  DIV_ZERO="$(grep -rn '/ [^=/]' "$TARGET/tensorflow/core/kernels/" --include='*.cc' 2>/dev/null | wc -l | tr -d ' ' || true)"
  NEW_ALLOC="$(grep -rn 'new ' "$TARGET/tensorflow/core/kernels/" --include='*.cc' 2>/dev/null | wc -l | tr -d ' ' || true)"
  UNCHECKED_STATUS="$(grep -rn 'Status' "$TARGET/tensorflow/core/kernels/" --include='*.cc' 2>/dev/null | wc -l | tr -d ' ' || true)"
  STATIC_CAST="$(grep -rn 'static_cast<' "$TARGET/tensorflow/core/kernels/" --include='*.cc' 2>/dev/null | wc -l | tr -d ' ' || true)"

  echo "| Pattern | Count in core/kernels/ |"
  echo "|---------|----------------------|"
  echo "| Division operations | $DIV_ZERO |"
  echo "| Raw allocations (new) | $NEW_ALLOC |"
  echo "| Status-related | $UNCHECKED_STATUS |"
  echo "| static_cast usage | $STATIC_CAST |"
  echo ""
  echo "## Recommended Scan Waves"
  echo ""
  echo "1. **Wave 1**: core/kernels/ (pooling, conv, matmul) — input validation, arithmetic"
  echo "2. **Wave 2**: core/kernels/ (slice, gather, sparse) — bounds checking, OOB"
  echo "3. **Wave 3**: core/framework/ — memory management, type safety"
  echo "4. **Wave 4**: core/platform/, core/util/ — I/O, parsing, resource management"
  echo "5. **Wave 5**: Cross-module analysis — data flow, API boundary validation"
} > reports/source-inventory.md

echo "Wrote reports/source-inventory.md"
