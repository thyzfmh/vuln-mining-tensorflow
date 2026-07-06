#!/usr/bin/env bash
# final_verify.sh — final verification gate for the competition deliverables
set -euo pipefail

mkdir -p reports

echo "========================================="
echo "  AI Vulnerability Mining — Final Verify"
echo "========================================="
echo ""

ERRORS=0

# 1. Deliverable existence check
echo "1. Checking deliverable existence..."

for F in result/vulnerability_list.md result/llm_chat_log.json result/vulnerability_report.md; do
  if [ ! -f "$F" ]; then
    echo "   FAIL: $F not found"
    ERRORS=$((ERRORS + 1))
  else
    echo "   OK: $F exists"
  fi
done

# 2. Format compliance
echo ""
echo "2. Checking format compliance..."

# vulnerability_list.md must have at least 1 vulnerability
if [ -f "result/vulnerability_list.md" ]; then
  VULN_COUNT="$(grep -c '^## 漏洞 #' result/vulnerability_list.md 2>/dev/null || echo "0")"
  if [ "$VULN_COUNT" -lt 1 ]; then
    echo "   FAIL: vulnerability_list.md has 0 vulnerabilities (need ≥ 1)"
    ERRORS=$((ERRORS + 1))
  else
    echo "   OK: vulnerability_list.md has $VULN_COUNT vulnerabilities"
  fi
fi

# llm_chat_log.json must be valid JSON
if [ -f "result/llm_chat_log.json" ]; then
  if python3 -c "import json; d=json.load(open('result/llm_chat_log.json')); assert 'metadata' in d; assert 'chat_history' in d; assert len(d['chat_history']) >= 1" 2>/dev/null; then
    TURN_COUNT="$(python3 -c "import json; d=json.load(open('result/llm_chat_log.json')); print(len(d.get('chat_history', [])))")"
    echo "   OK: llm_chat_log.json is valid JSON with $TURN_COUNT turns"
  else
    echo "   FAIL: llm_chat_log.json is invalid or missing required fields"
    ERRORS=$((ERRORS + 1))
  fi
fi

# vulnerability_report.md must have required sections
if [ -f "result/vulnerability_report.md" ]; then
  for SECTION in "概要" "方法论" "漏洞详细分析"; do
    if ! grep -q "$SECTION" result/vulnerability_report.md; then
      echo "   FAIL: vulnerability_report.md missing section '$SECTION'"
      ERRORS=$((ERRORS + 1))
    fi
  done
  echo "   OK: vulnerability_report.md has required sections"
fi

# 3. Evidence chain check
echo ""
echo "3. Checking evidence chain..."

if [ -f "result/vulnerability_list.md" ]; then
  # Every vulnerability should reference a source file
  if grep -q '问题源码路径' result/vulnerability_list.md; then
    echo "   OK: Source paths referenced"
  else
    echo "   FAIL: No source paths found in vulnerability_list.md"
    ERRORS=$((ERRORS + 1))
  fi
fi

# 4. Black-box compliance
echo ""
echo "4. Checking black-box compliance..."

if [ -f "result/llm_chat_log.json" ]; then
  # No version hints
  if grep -qiE 'v2\.11|tensorflow.*2\.11|version.*2\.11' result/llm_chat_log.json 2>/dev/null; then
    echo "   FAIL: Version hints found in chat log (black-box violation)"
    ERRORS=$((ERRORS + 1))
  else
    echo "   OK: No version hints in chat log"
  fi

  # No CVE references
  if grep -qiE 'CVE-[0-9]{4}-[0-9]+' result/llm_chat_log.json 2>/dev/null; then
    echo "   FAIL: CVE references found in chat log (black-box violation)"
    ERRORS=$((ERRORS + 1))
  else
    echo "   OK: No CVE references in chat log"
  fi

  # No explicit project name hints
  if grep -qiE '"这个项目是.*TensorFlow"|这是.*版本|这个版本有.*bug' result/llm_chat_log.json 2>/dev/null; then
    echo "   FAIL: Explicit project/version hints found in chat log (black-box violation)"
    ERRORS=$((ERRORS + 1))
  else
    echo "   OK: No explicit project hints in chat log"
  fi
fi

# 5. Interaction completeness
echo ""
echo "5. Checking interaction completeness..."

if [ -f "result/llm_chat_log.json" ]; then
  TURN_COUNT="$(python3 -c "import json; d=json.load(open('result/llm_chat_log.json')); print(len(d.get('chat_history', [])))" 2>/dev/null || echo "0")"
  if [ "$TURN_COUNT" -lt 5 ]; then
    echo "   WARN: Only $TURN_COUNT chat turns (recommended ≥ 5 for 90% reproducibility)"
  else
    echo "   OK: $TURN_COUNT chat turns (sufficient for reproducibility)"
  fi
fi

# 6. System prompt check
echo ""
echo "6. Checking system prompt..."

if [ -f "result/llm_chat_log.json" ]; then
  SYS_PROMPT="$(python3 -c "import json; d=json.load(open('result/llm_chat_log.json')); print(d.get('metadata', {}).get('system_prompt', ''))" 2>/dev/null || echo "")"
  if [ -z "$SYS_PROMPT" ]; then
    echo "   FAIL: No system_prompt in metadata"
    ERRORS=$((ERRORS + 1))
  elif echo "$SYS_PROMPT" | grep -qiE 'tensorflow|v2\.11'; then
    echo "   FAIL: System prompt contains project/version hints"
    ERRORS=$((ERRORS + 1))
  else
    echo "   OK: System prompt is generic (black-box compliant)"
  fi
fi

# Final summary
echo ""
echo "========================================="
if [ "$ERRORS" -eq 0 ]; then
  echo "  FINAL VERIFICATION: PASSED"
  echo "========================================="
  echo ""

  {
    echo "# Final Verification Report"
    echo ""
    echo "Date: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "Status: PASSED"
    echo ""
    echo "## Deliverables"
    echo ""
    echo "- vulnerability_list.md: $(grep -c '^## 漏洞 #' result/vulnerability_list.md 2>/dev/null || echo '0') vulnerabilities"
    echo "- llm_chat_log.json: $(python3 -c "import json; d=json.load(open('result/llm_chat_log.json')); print(len(d.get('chat_history', [])))" 2>/dev/null || echo '0') turns"
    echo "- vulnerability_report.md: Complete"
    echo ""
    echo "## Compliance"
    echo ""
    echo "- Format compliance: PASS"
    echo "- Evidence chain: PASS"
    echo "- Black-box compliance: PASS"
    echo "- Interaction completeness: PASS"
  } > reports/final-report.md

  echo "Wrote reports/final-report.md"
  exit 0
else
  echo "  FINAL VERIFICATION: FAILED ($ERRORS errors)"
  echo "========================================="
  echo ""
  echo "Fix the errors above and re-run ./harness/final_verify.sh"
  exit 1
fi
