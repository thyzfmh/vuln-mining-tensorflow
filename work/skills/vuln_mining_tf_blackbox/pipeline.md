# Vulnerability Discovery Pipeline

## Overview

This system performs black-box vulnerability discovery on a large C++ codebase using AI-driven semantic analysis.

## Pipeline Stages

### 1. Code Ingestion
- Clone/checkout the target codebase at the specified version
- Run `./work/skills/vuln_mining_tf_blackbox/scripts/analyze_target.sh code/tensorflow` to generate inventory
- Output: `reports/source-inventory.md`

### 2. Function Decomposition
- Identify high-risk functions in kernel ops and framework layer
- Map function signatures, parameters, and return types
- Trace data flow from external inputs to internal computation

### 3. Semantic Analysis
- Read source code and understand the logic
- Identify missing validation, unsafe operations, and error handling gaps
- Generate vulnerability hypotheses based on code patterns

### 4. Hypothesis Generation
- For each semantic finding, formulate a testable hypothesis
- Each hypothesis must specify:
  - The vulnerable code path
  - The trigger condition (what input reaches the vulnerability)
  - The expected failure mode (crash, exception, incorrect output)

### 5. AI Test Generation
- Generate Python test cases using the project's Python API
- Each test case targets a specific vulnerability hypothesis
- Tests must be self-contained and executable without manual setup

### 6. Runtime Verification
- Execute all generated tests via `python3 verify/run_test.py`
- Capture: exit codes, stderr, exception traces, signal types (SIGFPE, SIGSEGV, etc.)
- Only accept findings with runtime evidence (crash, exception, assertion failure)

### 7. Evidence Collection
- Compile verified findings into `vulnerability_list.md`
- Record all LLM interactions into `llm_chat_log.json`
- Generate `vulnerability_report.md`
- Package all test cases into `verify/run_test.py`

## Constraints

- No CVE usage
- No external vulnerability database
- Pure semantic reasoning only
- All test cases AI-generated
- All findings runtime-verified

## Failure Handling

- If a test does not crash: hypothesis is rejected, not included in deliverables
- If a test requires unavailable dependencies: skip and document in report
- If runtime environment lacks the target library: fall back to source-level evidence with explicit note
