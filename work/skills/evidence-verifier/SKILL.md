---
name: evidence-verifier
description: Verifies vulnerability evidence chains and runs anti-hallucination checks.
---

# Evidence Verifier

Verifies that each reported vulnerability has:
1. A source file path that exists in the codebase
2. A trigger path reachable from external input
3. An AI-generated PoC
4. Passed the anti-hallucination 6-question checklist

## Anti-Hallucination Checklist

1. Does the cited source code actually exist at the claimed location?
2. Does the trigger path actually exist in the call graph?
3. Is the claimed behavior verifiable from source code alone?
4. Could this be a false positive from LLM pattern matching?
5. Is the severity assessment justified by actual impact?
6. Is the proposed PoC actually triggerable?

## Usage

```bash
./work/skills/vuln_mining_tf_blackbox/scripts/verify_vulnerabilities.sh
```

Output: `reports/vuln-verification.md`
