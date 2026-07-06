#!/usr/bin/env bash
# verify_vulnerabilities.sh — verify all reported vulnerabilities have complete evidence
set -euo pipefail

mkdir -p reports

echo "=== Vulnerability Evidence Verification ==="
echo ""

ERRORS=0
WARNINGS=0

# Check vulnerability_list.md exists
if [ ! -f "result/vulnerability_list.md" ]; then
  echo "ERROR: result/vulnerability_list.md not found" >&2
  ERRORS=$((ERRORS + 1))
else
  # Count vulnerabilities
  VULN_COUNT="$(grep -c '^## 漏洞 #' result/vulnerability_list.md 2>/dev/null || echo "0")"
  echo "Vulnerabilities found: $VULN_COUNT"

  # Check each vulnerability has required fields
  VULN_NUM=0
  while IFS= read -r line; do
    if [[ "$line" == "## 漏洞 #"* ]]; then
      VULN_NUM=$((VULN_NUM + 1))
    fi
  done < result/vulnerability_list.md

  # Verify required sections
  for SECTION in "漏洞类型" "严重级别" "问题源码路径" "成因简述" "与LLM交互中哪句提示词发现了bug" "为什么选择此提示词" "潜在业务危害"; do
    if ! grep -q "$SECTION" result/vulnerability_list.md; then
      echo "WARNING: Missing section '$SECTION' in vulnerability_list.md" >&2
      WARNINGS=$((WARNINGS + 1))
    fi
  done
fi

# Check llm_chat_log.json exists and is valid JSON
if [ ! -f "result/llm_chat_log.json" ]; then
  echo "ERROR: result/llm_chat_log.json not found" >&2
  ERRORS=$((ERRORS + 1))
else
  if ! python3 -c "import json; json.load(open('result/llm_chat_log.json'))" 2>/dev/null; then
    echo "ERROR: llm_chat_log.json is not valid JSON" >&2
    ERRORS=$((ERRORS + 1))
  else
    TURN_COUNT="$(python3 -c "import json; d=json.load(open('result/llm_chat_log.json')); print(len(d.get('chat_history', [])))" 2>/dev/null || echo "0")"
    echo "Chat turns: $TURN_COUNT"

    # Check for black-box compliance (no version hints)
    if grep -qiE 'v2\.11|tensorflow.*2\.11|version.*2\.11' result/llm_chat_log.json 2>/dev/null; then
      echo "ERROR: Black-box violation — version hints found in llm_chat_log.json" >&2
      ERRORS=$((ERRORS + 1))
    fi

    if grep -qiE 'CVE-[0-9]{4}-[0-9]+' result/llm_chat_log.json 2>/dev/null; then
      echo "ERROR: Black-box violation — CVE references found in llm_chat_log.json" >&2
      ERRORS=$((ERRORS + 1))
    fi
  fi
fi

# Check vulnerability_report.md exists
if [ ! -f "result/vulnerability_report.md" ]; then
  echo "ERROR: result/vulnerability_report.md not found" >&2
  ERRORS=$((ERRORS + 1))
else
  for SECTION in "概要" "方法论" "漏洞详细分析" "修复建议"; do
    if ! grep -q "$SECTION" result/vulnerability_report.md; then
      echo "WARNING: Missing section '$SECTION' in vulnerability_report.md" >&2
      WARNINGS=$((WARNINGS + 1))
    fi
  done
fi

# Summary
echo ""
echo "=== Verification Summary ==="
echo "Errors: $ERRORS"
echo "Warnings: $WARNINGS"

{
  echo "# Vulnerability Verification Report"
  echo ""
  echo "Date: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "Errors: $ERRORS"
  echo "Warnings: $WARNINGS"
  echo "Status: $([ "$ERRORS" -eq 0 ] && echo 'PASS' || echo 'FAIL')"
} > reports/vuln-verification.md

if [ "$ERRORS" -gt 0 ]; then
  exit 1
fi

echo "Vulnerability verification passed."
