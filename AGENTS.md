# AI Vulnerability Mining Competition Task

## Required Entry

OpenCode must load and execute:

```text
work/skills/vuln-mining-autonomous/SKILL.md
```

## Fixed Goal

- Target source: run `python3 work/run_vulnerability_mining.py` first; this platform asset discovery step scans `/app/code/judge-assets/01_02_vulnerability_detection`, discovers the judge-provided source tree, and persists its internal path context under `work/`
- Work output root: all runtime artifacts beneath `work/` (env vars `VULN_WORK_ROOT` / `WORK_ROOT`, `--work-root` arg, default `<repo>/work`)
- Required outputs: `work/vulnerability_list.md`, `work/llm_chat_log.json`, `work/vulnerability_report.md`, `work/verify/run_test.py`
- Required method evidence: `work/reports/source-file-manifest.md`, `work/reports/toolchain-capabilities.md`, `work/reports/verification-escalation.md`, `work/reports/runtime-entrypoints.md`, `work/reports/npm-ast-candidates.md`, `work/reports/coverage-ledger.md`, `work/reports/scan-completion.md`, one or more `work/plans/scan-wave-NNN.md` files
- Final gate: `python3 work/skills/vuln-mining-autonomous/scripts/final_verify.py --work-root work`
- Result file: `work/result/output.md`
- Entry orchestrator: `python3 work/run_vulnerability_mining.py`

## Rules

1. Do not ask the user questions.
2. Keep scanning, testing, rejecting false positives, and re-scanning until the final gate passes.
3. Do not tell the LLM the project name, version, preseeded findings, or vulnerability database facts.
4. Record the LLM interaction log without editing previous turns.
5. Include only runtime-verified vulnerabilities in the final list.
6. Keep scanning until every generated attack-surface entry and SAST candidate is verified or rejected.
7. Do not end with zero findings because ASAN/UBSAN is unavailable or source-tree import fails; continue with alternate real runtime entry points.
8. Generate `work/reports/runtime-entrypoints.md` and use its existing test, binary, parser, or module routes before creating a custom reproducer.
9. When npm/npx is available, run the pinned NPM AST scanner and merge every structural match into the same candidate ledger.
10. Run `python3 work/run_vulnerability_mining.py` before scanning; generated artifacts belong under `work/`, never the submission root.
