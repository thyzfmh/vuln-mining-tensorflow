---
name: status-dashboard
description: Vulnerability mining progress dashboard. Tracks scan waves, findings, and verification status.
---

# Status Dashboard

Tracks vulnerability mining progress across scan waves.

## Metrics

- Total scan waves completed
- Modules scanned per wave
- Candidate vulnerabilities found
- Verified vulnerabilities (passed anti-hallucination)
- Unverified vulnerabilities
- Deliverable completion status

## Usage

Check `reports/` directory for current status:
- `reports/source-inventory.md` — Codebase analysis
- `reports/sast-scan-report.md` — SAST scan results
- `reports/llm-scan-report.md` — LLM scan results
- `reports/vuln-verification.md` — Verification status
- `reports/final-report.md` — Final verification report
