# AI Vulnerability Mining Competition

This package is a black-box vulnerability-mining harness for OpenCode.

## Platform Entry

```text
INSTRUCTION.md
work/skills/vuln-mining-autonomous/SKILL.md
```

The skill must generate:

- `vulnerability_list.md`
- `llm_chat_log.json`
- `vulnerability_report.md`
- `verify/run_test.py`
- `result/output.md`

## Black-Box Constraint

The interaction log must not tell the LLM the project name, version, preseeded findings,
or vulnerability database facts. Prompts should refer to the target as
`TARGET_ROOT` and should use source snippets, relative file paths, and semantic
questions only.

## Methodology

The skill combines deterministic engineering and LLM analysis:

1. inventory the source tree;
2. run local SAST-style candidate extraction;
3. review bounded source slices with black-box prompts;
4. generate AI-authored runtime tests;
5. reject unverified candidates;
6. report only runtime-verified bugs.
