#!/usr/bin/env bash
# verify_vulnerabilities.sh — verify all reported vulnerabilities have complete evidence
set -euo pipefail

mkdir -p reports

echo "=== Vulnerability Evidence Verification ==="
echo ""

ERRORS=0
WARNINGS=0

if [ -f "submission/vulnerability_list.md" ]; then
  VULN_COUNT="$(grep -c '^## 漏洞 #' submission/vulnerability_list.md 2>/dev/null || echo "0")"
  echo "Vulnerabilities found: $VULN_COUNT"

  for SECTION in "漏洞类型" "严重级别" "问题源码路径" "成因简述" "验证结果"; do
    if ! grep -q "$SECTION" submission/vulnerability_list.md; then
      echo "WARNING: Missing section '$SECTION' in vulnerability_list.md" >&2
      WARNINGS=$((WARNINGS + 1))
    fi
  done
else
  echo "ERROR: submission/vulnerability_list.md not found" >&2
  ERRORS=$((ERRORS + 1))
fi

if [ -f "submission/llm_chat_log.json" ]; then
  if ! python3 -c "import json; json.load(open('submission/llm_chat_log.json'))" 2>/dev/null; then
    echo "ERROR: llm_chat_log.json is not valid JSON" >&2
    ERRORS=$((ERRORS + 1))
  else
    TURN_COUNT="$(python3 -c "import json; d=json.load(open('submission/llm_chat_log.json')); print(len(d.get('chat_history', [])))" 2>/dev/null || echo "0")"
    echo "Chat turns: $TURN_COUNT"

    if grep -qiE 'v2\.11|tensorflow.*2\.11|version.*2\.11' submission/llm_chat_log.json 2>/dev/null; then
      echo "ERROR: Black-box violation — version hints found in llm_chat_log.json" >&2
      ERRORS=$((ERRORS + 1))
    fi

    if grep -qiE 'CVE-[0-9]{4}-[0-9]+' submission/llm_chat_log.json 2>/dev/null; then
      echo "ERROR: Black-box violation — CVE references found in llm_chat_log.json" >&2
      ERRORS=$((ERRORS + 1))
    fi
  fi
else
  echo "ERROR: submission/llm_chat_log.json not found" >&2
  ERRORS=$((ERRORS + 1))
fi

if [ -f "submission/vulnerability_report.md" ]; then
  for SECTION in "黑盒\|black.box" "方法论\|methodology" "pipeline\|流程"; do
    if ! grep -qi "$SECTION" submission/vulnerability_report.md; then
      echo "WARNING: Missing section matching '$SECTION' in vulnerability_report.md" >&2
      WARNINGS=$((WARNINGS + 1))
    fi
  done
else
  echo "ERROR: submission/vulnerability_report.md not found" >&2
  ERRORS=$((ERRORS + 1))
fi

if [ -f "submission/verify/run_test.py" ]; then
  if ! python3 -c "import ast; ast.parse(open('submission/verify/run_test.py').read())" 2>/dev/null; then
    echo "ERROR: verify/run_test.py has syntax errors" >&2
    ERRORS=$((ERRORS + 1))
  else
    echo "verify/run_test.py: valid Python syntax"
  fi
else
  echo "ERROR: submission/verify/run_test.py not found" >&2
  ERRORS=$((ERRORS + 1))
fi

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
