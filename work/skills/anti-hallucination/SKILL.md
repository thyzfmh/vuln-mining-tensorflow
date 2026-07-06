---
name: anti-hallucination
description: Evidence verification during vulnerability analysis. Prevents LLM from fabricating source code locations, trigger paths, or vulnerability claims.
---

# Anti-Hallucination for Vulnerability Mining

When analyzing vulnerabilities reported by an LLM, apply these checks before accepting any claim:

## 6-Question Checklist

1. **Source existence**: Does the cited source code actually exist at the claimed file path and line number?
2. **Trigger path reality**: Does the call chain from external input to the vulnerability point actually exist?
3. **Verifiability**: Can the claimed behavior be verified from the source code alone, without relying on LLM "memory"?
4. **False positive check**: Could this be a pattern-match false positive where the LLM recognized a surface pattern but the actual code is safe?
5. **Severity justification**: Is the severity rating supported by the actual impact (not just the vulnerability class)?
6. **PoC triggerability**: Can the proposed PoC actually trigger the bug, or does it assume conditions not present in the code?

## Red Flags

- LLM claims a file exists without providing exact content
- LLM describes behavior not visible in the provided source
- LLM references functions/types that don't appear in the codebase
- LLM proposes a PoC that requires impossible input conditions
- LLM assigns Critical severity to a bug that only causes a logged error

## Action

If any checklist item fails, mark the vulnerability as "unverified" and re-scan with a more specific prompt targeting the exact code section.
