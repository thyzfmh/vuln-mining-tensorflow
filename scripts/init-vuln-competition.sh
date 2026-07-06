#!/usr/bin/env bash
# init-vuln-competition.sh — initialize the AI vulnerability mining competition project
set -euo pipefail

TARGET_PATH="${1:?usage: $0 <target_codebase_path>}"

if [ ! -d "$TARGET_PATH" ]; then
  echo "ERROR: target path does not exist: $TARGET_PATH" >&2
  exit 1
fi

TODAY="$(date +%Y-%m-%d)"

mkdir -p reports plans submission/verify

# Generate submission deliverable stubs
if [ ! -f submission/vulnerability_list.md ]; then
  cp templates/vulnerability_list.template.md submission/vulnerability_list.md
  echo "Created submission/vulnerability_list.md from template"
fi

if [ ! -f submission/llm_chat_log.json ]; then
  cp templates/llm_chat_log.template.json submission/llm_chat_log.json
  echo "Created submission/llm_chat_log.json from template"
fi

if [ ! -f submission/vulnerability_report.md ]; then
  cp templates/vulnerability_report.template.md submission/vulnerability_report.md
  echo "Created submission/vulnerability_report.md from template"
fi

if [ ! -f submission/verify/run_test.py ]; then
  cp work/skills/vuln_mining_tf_blackbox/verify/run_test.py submission/verify/run_test.py
  echo "Created submission/verify/run_test.py from skill template"
fi

# Generate acceptance-plan.yaml
cat > acceptance-plan.yaml <<EOF
# AI Vulnerability Mining Competition Acceptance Plan
created_at: "$TODAY"

scope:
  target_codebase: "$TARGET_PATH"
  target_language: cpp
  scan_scope:
    - "$TARGET_PATH/tensorflow/core/kernels/"
    - "$TARGET_PATH/tensorflow/core/framework/"
    - "$TARGET_PATH/tensorflow/core/platform/"
    - "$TARGET_PATH/tensorflow/core/util/"

requirements:
  black_box_standard: true
  no_version_hints: true
  no_cve_references: true
  evidence_required: true
  ai_generated_tests: true
  runtime_verification: true
  complete_interaction_log: true
  min_vulnerabilities: 1
  min_chat_turns: 5

verification:
  primary_commands:
    - "./harness/verify_vulnerabilities.sh"
    - "./harness/final_verify.sh"
  runtime_verification:
    - "python3 submission/verify/run_test.py"
  black_box_check:
    - "No project name/version in LLM prompts"
    - "No CVE references in chat log"
    - "System prompt is generic"
  evidence_check:
    - "Source file path exists in codebase"
    - "Trigger path is reachable from external input"
    - "Runtime test can demonstrate the vulnerability"

deliverables:
  - "submission/vulnerability_list.md — Complete vulnerability list with evidence"
  - "submission/llm_chat_log.json — Complete LLM interaction log (no edits)"
  - "submission/vulnerability_report.md — Detailed vulnerability audit report"
  - "submission/verify/run_test.py — AI-generated runtime verification script"
EOF

# Generate initial scan plan
cat > plans/scan-000-orientation.md <<EOF
# scan-000: Orientation

## Goal

Understand the target codebase and prepare for systematic vulnerability scanning.

## Steps

1. Run \`./harness/analyze_target.sh "$TARGET_PATH"\`.
2. Read \`reports/source-inventory.md\`.
3. Identify the first high-risk module for scanning.
4. Generate a concrete scan plan:

\`\`\`bash
./harness/plan_next_scan.sh scan-001 "Semantic analysis for input validation in kernel ops" "$TARGET_PATH/tensorflow/core/kernels/"
\`\`\`

5. Execute the scan following work/skills/vuln_mining_tf_blackbox/prompt.md.
EOF

chmod +x harness/*.sh

echo ""
echo "AI Vulnerability Mining competition initialized."
echo ""
echo "Target: $TARGET_PATH"
echo ""
echo "Next:"
echo "  ./harness/analyze_target.sh \"$TARGET_PATH\""
echo "  ./harness/plan_next_scan.sh scan-001 \"Semantic analysis for input validation\" \"$TARGET_PATH/tensorflow/core/kernels/\""
