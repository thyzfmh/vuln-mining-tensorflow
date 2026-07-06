#!/usr/bin/env bash
# plan_next_scan.sh — generate a scan plan for the next wave
set -euo pipefail

SCAN_ID="${1:-scan-001}"
GOAL="${2:-Scan the next high-risk module for vulnerabilities}"
SCOPE="${3:-tensorflow/core/kernels/}"

mkdir -p plans
PLAN="plans/${SCAN_ID}.md"

cat > "$PLAN" <<EOF
# ${SCAN_ID}: ${GOAL}

## Goal

${GOAL}

## Target Scope

${SCOPE}

## Black-Box Rules

- Do NOT mention the project name, version, or known CVEs in LLM prompts
- All prompts must be generic: "Analyze this C++ source file for potential security issues"
- The LLM must discover vulnerabilities autonomously from the source code alone

## Scan Strategy

| Strategy | Description |
|----------|-------------|
| SAST Pattern Scan | Regex-based pattern matching for common vulnerability patterns |
| LLM Semantic Scan | AI-driven code understanding for logic-level vulnerabilities |
| Evidence Verification | Anti-hallucination check for each candidate vulnerability |

## Vulnerability Patterns to Scan

| Pattern | Regex / Indicator | Vulnerability Type |
|---------|-------------------|-------------------|
| Division without zero-check | Division near shape/size calculations | FPE / DoS |
| Unchecked pointer deref | Pointer access without null check | Null deref / DoS |
| Integer overflow | int32 used for size then cast to size_t | Heap overflow |
| Missing error handling | Returned Status not checked | Logic error |
| Unsafe cast | static_cast / reinterpret_cast without validation | Type confusion |
| Uncapped allocation | Allocate/new with user-controlled size | DoS / OOM |

## Prompt Templates (Black-Box Compliant)

### Input Validation Scan
\`\`\`
Analyze this C++ source file for missing input validation that could
lead to crashes or undefined behavior. Focus on: (1) parameters that
could be zero causing division errors, (2) negative values causing
buffer underflow, (3) missing null checks after allocation.
\`\`\`

### Arithmetic Safety Scan
\`\`\`
Review this C++ code for arithmetic safety issues: (1) integer
overflow in size calculations, (2) signed/unsigned mismatch, (3)
truncation when casting between types.
\`\`\`

### Memory Safety Scan
\`\`\`
Examine this C++ code for memory safety issues: (1) use-after-free,
(2) heap buffer overflow, (3) null pointer dereference.
\`\`\`

## Anti-Hallucination Checklist

For each candidate vulnerability:
- [ ] Does the cited source code actually exist at the claimed location?
- [ ] Does the trigger path actually exist in the call graph?
- [ ] Is the claimed behavior verifiable from source code alone?
- [ ] Could this be a false positive from LLM pattern matching?
- [ ] Is the severity assessment justified by actual impact?
- [ ] Is the proposed PoC actually triggerable?

## Verification Commands

\`\`\`bash
./harness/verify_vulnerabilities.sh
./harness/final_verify.sh
\`\`\`

## Review Gate

- [ ] All interactions recorded in llm_chat_log.json
- [ ] Black-box compliance verified (no version/CVE hints)
- [ ] Anti-hallucination checklist passed for each finding
- [ ] Evidence chain complete for each vulnerability
EOF

echo "Created $PLAN"
