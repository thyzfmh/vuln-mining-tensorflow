---
name: vuln_mining_tf_blackbox
version: 1.0

mode: batch
interaction: false

entry: vuln_mining_tf_blackbox/prompt.md

output:
  dir: .

constraints:
  black_box: true
  no_cve: true
  no_known_vuln_db: true
  ai_generated_tests: true
  require_verification: true

deliverables:
  - vulnerability_list.md
  - llm_chat_log.json
  - vulnerability_report.md
  - verify/run_test.py
---

# AI Vulnerability Mining — Black-Box

Autonomous black-box vulnerability discovery system for a large C++ codebase.

## Entry Point

See `vuln_mining_tf_blackbox/prompt.md` for the full 5-step pipeline.

## Pipeline

1. Code Semantic Analysis — 源码语义分析
2. Hypothesis Generation — 漏洞假设生成
3. AI Test Generation — AI 测试用例生成
4. Runtime Verification — 运行时验证
5. Evidence Collection — 证据收集

## Constraints

- No CVE usage
- No external vulnerability database
- Pure semantic reasoning only
- All test cases AI-generated
- All vulnerabilities runtime-verified

## Deliverables

| File | Description |
|------|-------------|
| `vulnerability_list.md` | Complete vulnerability list with evidence chains |
| `llm_chat_log.json` | Complete LLM interaction log (no edits, 90% reproducible) |
| `vulnerability_report.md` | Detailed engineering report |
| `verify/run_test.py` | AI-generated runtime verification script |

## Black-Box Declaration

All results are derived purely from code semantic analysis.
No CVE or vulnerability database is used.
All test cases are AI generated.
All vulnerabilities are validated through runtime execution.
