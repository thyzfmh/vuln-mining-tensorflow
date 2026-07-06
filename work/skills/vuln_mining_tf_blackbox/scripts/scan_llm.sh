#!/usr/bin/env bash
# scan_llm.sh — placeholder for LLM semantic analysis (to be executed by the AI agent)
set -euo pipefail

TARGET="${1:?usage: $0 <target_path>}"

if [ ! -d "$TARGET" ]; then
  echo "ERROR: target path does not exist: $TARGET" >&2
  exit 1
fi

mkdir -p reports

echo "=== LLM Semantic Analysis ==="
echo "Target: $TARGET"
echo ""
echo "The LLM scan is executed by the AI agent following work/skills/vuln-mining/SKILL.md."
echo "This script records the scan metadata."
echo ""

{
  echo "# LLM Scan Report"
  echo ""
  echo "Target: \`$TARGET\`"
  echo "Date: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo ""
  echo "## Scan Configuration"
  echo ""
  echo "- Model: [filled by AI agent during execution]"
  echo "- System prompt: Generic C++ security analysis (no project/version hints)"
  echo "- Scan scope: [filled by AI agent]"
  echo "- Turns completed: [filled by AI agent]"
  echo ""
  echo "## Prompt Strategy"
  echo ""
  echo "### Wave 1: Input Validation"
  echo "- Focus: Division by zero, missing null checks, negative values"
  echo "- Modules: core/kernels/ (pooling, conv, matmul)"
  echo ""
  echo "### Wave 2: Bounds Checking"
  echo "- Focus: OOB access, integer overflow in index calculation"
  echo "- Modules: core/kernels/ (slice, gather, sparse)"
  echo ""
  echo "### Wave 3: Memory Safety"
  echo "- Focus: Use-after-free, heap overflow, type confusion"
  echo "- Modules: core/framework/"
  echo ""
  echo "### Wave 4: Resource Management"
  echo "- Focus: Leaks, unbounded allocation, parsing errors"
  echo "- Modules: core/platform/, core/util/"
  echo ""
  echo "### Wave 5: Cross-Module"
  echo "- Focus: Data flow from external input, API boundary validation"
  echo "- Scope: Cross-cutting"
  echo ""
  echo "## Findings"
  echo ""
  echo "[Filled by AI agent as vulnerabilities are discovered]"
} > reports/llm-scan-report.md

echo "Wrote reports/llm-scan-report.md"
echo ""
echo "LLM scan framework ready. The AI agent will fill in findings during execution."
