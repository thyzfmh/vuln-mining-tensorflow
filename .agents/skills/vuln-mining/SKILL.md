---
name: vuln-mining
description: AI-driven vulnerability mining for large C++ codebases. Systematic SAST + LLM scanning with evidence verification.
---

# Vulnerability Mining Skill

See `work/skills/vuln-mining/SKILL.md` for the full execution workflow.

This skill provides:
- SAST static pattern scanning
- LLM semantic vulnerability analysis
- Evidence chain verification
- Anti-hallucination checks
- Black-box compliant prompt engineering

## Quick Reference

```bash
./harness/analyze_target.sh code/tensorflow
./harness/plan_next_scan.sh scan-001 "Goal" "scope/"
./harness/scan_sast.sh code/tensorflow
./harness/verify_vulnerabilities.sh
./harness/final_verify.sh
```
