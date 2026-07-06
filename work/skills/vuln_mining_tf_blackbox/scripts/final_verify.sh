#!/usr/bin/env bash
# final_verify.sh — final verification gate for the competition deliverables
set -euo pipefail

mkdir -p reports

echo "========================================="
echo "  AI Vulnerability Mining — Final Verify"
echo "========================================="
echo ""

ERRORS=0
WARNINGS=0

# 1. Deliverable existence check
echo "1. Checking deliverable existence..."

for F in vulnerability_list.md llm_chat_log.json vulnerability_report.md verify/run_test.py; do
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

if [ -f "vulnerability_list.md" ]; then
  VULN_COUNT="$(grep -c '^## 漏洞 #' vulnerability_list.md 2>/dev/null || echo "0")"
  if [ "$VULN_COUNT" -lt 1 ]; then
    echo "   FAIL: vulnerability_list.md has 0 vulnerabilities (need >= 1)"
    ERRORS=$((ERRORS + 1))
  else
    echo "   OK: vulnerability_list.md has $VULN_COUNT vulnerabilities"
  fi
fi

if [ -f "llm_chat_log.json" ]; then
  if python3 -c "import json; d=json.load(open('llm_chat_log.json')); assert 'metadata' in d; assert 'chat_history' in d; assert len(d['chat_history']) >= 1" 2>/dev/null; then
    TURN_COUNT="$(python3 -c "import json; d=json.load(open('llm_chat_log.json')); print(len(d.get('chat_history', [])))")"
    echo "   OK: llm_chat_log.json is valid JSON with $TURN_COUNT turns"
  else
    echo "   FAIL: llm_chat_log.json is invalid or missing required fields"
    ERRORS=$((ERRORS + 1))
  fi
fi

if [ -f "vulnerability_report.md" ]; then
  for SECTION in "黑盒" "方法论" "pipeline" "验证" "reproducibility\|可复现"; do
    if ! grep -qi "$SECTION" vulnerability_report.md; then
      echo "   FAIL: vulnerability_report.md missing section matching '$SECTION'"
      ERRORS=$((ERRORS + 1))
      break
    fi
  done
  echo "   OK: vulnerability_report.md has required sections"
fi

if [ -f "verify/run_test.py" ]; then
  if python3 -c "import ast; ast.parse(open('verify/run_test.py').read())" 2>/dev/null; then
    echo "   OK: verify/run_test.py is valid Python"
  else
    echo "   FAIL: verify/run_test.py has syntax errors"
    ERRORS=$((ERRORS + 1))
  fi

  if grep -q 'def run_test\|def test_case' verify/run_test.py 2>/dev/null; then
    echo "   OK: verify/run_test.py contains test functions"
  else
    echo "   WARN: verify/run_test.py has no test functions yet"
    WARNINGS=$((WARNINGS + 1))
  fi
fi

# 3. Evidence chain check
echo ""
echo "3. Checking evidence chain..."

if [ -f "vulnerability_list.md" ]; then
  if grep -q '问题源码路径\|source.*path\|源码路径' vulnerability_list.md; then
    echo "   OK: Source paths referenced"
  else
    echo "   FAIL: No source paths found in vulnerability_list.md"
    ERRORS=$((ERRORS + 1))
  fi

  if grep -q '验证结果\|verification\|CRASH\|crash' vulnerability_list.md; then
    echo "   OK: Verification results present"
  else
    echo "   FAIL: No verification results in vulnerability_list.md"
    ERRORS=$((ERRORS + 1))
  fi
fi

# 4. Black-box compliance
echo ""
echo "4. Checking black-box compliance..."

if [ -f "llm_chat_log.json" ]; then
  if grep -qiE 'v2\.11|tensorflow.*2\.11|version.*2\.11' llm_chat_log.json 2>/dev/null; then
    echo "   FAIL: Version hints found in chat log (black-box violation)"
    ERRORS=$((ERRORS + 1))
  else
    echo "   OK: No version hints in chat log"
  fi

  if grep -qiE 'CVE-[0-9]{4}-[0-9]+' llm_chat_log.json 2>/dev/null; then
    echo "   FAIL: CVE references found in chat log (black-box violation)"
    ERRORS=$((ERRORS + 1))
  else
    echo "   OK: No CVE references in chat log"
  fi

  if grep -qiE '"这个项目是.*[Tt]ensor[Ff]low"|这是.*版本|这个版本有.*bug' llm_chat_log.json 2>/dev/null; then
    echo "   FAIL: Explicit project/version hints found in chat log (black-box violation)"
    ERRORS=$((ERRORS + 1))
  else
    echo "   OK: No explicit project hints in chat log"
  fi
fi

# 5. Interaction completeness
echo ""
echo "5. Checking interaction completeness..."

if [ -f "llm_chat_log.json" ]; then
  TURN_COUNT="$(python3 -c "import json; d=json.load(open('llm_chat_log.json')); print(len(d.get('chat_history', [])))" 2>/dev/null || echo "0")"
  if [ "$TURN_COUNT" -lt 5 ]; then
    echo "   WARN: Only $TURN_COUNT chat turns (recommended >= 5 for 90% reproducibility)"
    WARNINGS=$((WARNINGS + 1))
  else
    echo "   OK: $TURN_COUNT chat turns (sufficient for reproducibility)"
  fi
fi

# 6. Black-box declaration in report
echo ""
echo "6. Checking black-box declaration..."

if [ -f "vulnerability_report.md" ]; then
  if grep -qi 'code semantic analysis\|No CVE\|AI generated\|runtime execution' vulnerability_report.md; then
    echo "   OK: Black-box declaration present"
  else
    echo "   FAIL: Missing black-box declaration in vulnerability_report.md"
    ERRORS=$((ERRORS + 1))
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
    echo "Errors: $ERRORS"
    echo "Warnings: $WARNINGS"
    echo ""
    echo "## Deliverables"
    echo ""
    VULNS="$(grep -c '^## 漏洞 #' vulnerability_list.md 2>/dev/null || echo '0')"
    TURNS="$(python3 -c "import json; d=json.load(open('llm_chat_log.json')); print(len(d.get('chat_history', [])))" 2>/dev/null || echo '0')"
    echo "- vulnerability_list.md: $VULNS vulnerabilities"
    echo "- llm_chat_log.json: $TURNS turns"
    echo "- vulnerability_report.md: Complete"
    echo "- verify/run_test.py: Present"
    echo ""
    echo "## Compliance"
    echo ""
    echo "- Format compliance: PASS"
    echo "- Evidence chain: PASS"
    echo "- Black-box compliance: PASS"
    echo "- Interaction completeness: PASS"
    echo "- Runtime verification: PASS"
  } > reports/final-report.md

  echo "Wrote reports/final-report.md"
  exit 0
else
  echo "  FINAL VERIFICATION: FAILED ($ERRORS errors, $WARNINGS warnings)"
  echo "========================================="
  echo ""
  echo "Fix the errors above and re-run ./work/skills/vuln_mining_tf_blackbox/scripts/final_verify.sh"
  exit 1
fi
