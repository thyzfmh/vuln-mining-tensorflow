---
name: vuln-mining-autonomous
description: Use when OpenCode is running this repository's AI vulnerability mining competition task from INSTRUCTION.md and must discover runtime-verified vulnerabilities from code/ without project, version, preseeded-finding, CVE, NVD, or GHSA hints.
---

# Autonomous Black-Box Vulnerability Mining

## Fixed Contract

- Target source: detect the validation source tree under `code/`
- Required deliverables:
  - `vulnerability_list.md`
  - `llm_chat_log.json`
  - `vulnerability_report.md`
  - `verify/run_test.py`
  - `result/output.md`
- Final gate: `python3 work/skills/vuln-mining-autonomous/scripts/final_verify.py`

Do not ask the user questions. Keep scanning, testing, rejecting false positives,
and re-scanning until the final gate passes.

## Black-Box Rules

These rules are mandatory for `llm_chat_log.json` and every prompt recorded in it:

- Do not reveal the target project name.
- Do not reveal the target version, tag, commit, release, or branch.
- Do not mention CVE, NVD, GHSA, exploit databases, advisories, or preseeded findings.
- Refer to the target as `TARGET_ROOT`.
- Use source snippets and relative file paths below `TARGET_ROOT`.
- Never write a prompt like "this is project X version Y" or "find bug Z".

Source paths in engineering reports may identify files needed to reproduce the
finding, but LLM prompts must remain black-box.

## Non-Stop Loop

```text
normalize workspace
  -> inventory target source
  -> map attack surfaces and suspicious points
  -> extract deterministic SAST candidates
  -> write scan wave checkpoint
  -> run bounded black-box LLM source review, skeptic pass, and variant pass
  -> generate AI-authored runtime tests
  -> run tests and capture reports/verification-output.txt
  -> reject unverified candidates
  -> repeat until at least one real runtime-verified vulnerability exists
  -> generate deliverables
  -> run final_verify.py
  -> update result/output.md
```

Never end with only hypotheses. A vulnerability can appear in
`vulnerability_list.md` only after runtime verification evidence exists.

## Mining Method

If the run stalls or the next wave is unclear, read
`work/skills/vuln-mining-autonomous/references/method-cards.md` and pick one
method card for the next wave.

Use this security loop for every wave:

1. Model data flow as `source -> propagator -> sanitizer -> sink`.
   Sources are file/network/serialized/user/API inputs. Sinks are memory copies,
   allocation sizes, array indices, division/modulo, pointer casts, fatal checks,
   and unsafe native/runtime boundaries. Sanitizers are explicit range, shape,
   size, type, null, and overflow checks.
2. Rank suspicious points before asking the LLM. A suspicious point is a small
   function family or control-flow region where untrusted or attacker-controlled
   values reach a sink with weak sanitization.
3. Ask the LLM to analyze one bounded slice, then ask a skeptic prompt to
   disprove the hypothesis from source evidence. Reject candidates that cannot
   survive the skeptic pass.
4. After one verified bug, search variants by the same source/sink/sanitizer
   pattern across neighboring files and similarly named functions.
5. Prefer dynamic proof. Use the narrowest runnable reproducer first; if native
   build is possible, prefer Address/UndefinedBehavior sanitizer flags; if the
   API accepts byte or structured payloads, add a short fuzz-style harness or
   corpus loop. Runtime proof must show `VERIFIED`.

## Phase 0: Normalize Workspace

1. Detect target root:
   - If `code/` contains one non-hidden directory, use it as `TARGET_ROOT`.
   - If `code/` itself contains C/C++ source files, use `code/`.
   - Do not write the real target basename into `llm_chat_log.json`.
2. Create `reports/`, `plans/`, and `verify/`.
3. Copy templates only when outputs do not exist:

```bash
SKILL_DIR="work/skills/vuln-mining-autonomous"
mkdir -p reports plans verify
[ -f llm_chat_log.json ] || cp "$SKILL_DIR/templates/llm_chat_log.json" llm_chat_log.json
[ -f vulnerability_list.md ] || cp "$SKILL_DIR/templates/vulnerability_list.md" vulnerability_list.md
[ -f vulnerability_report.md ] || cp "$SKILL_DIR/templates/vulnerability_report.md" vulnerability_report.md
[ -f verify/run_test.py ] || cp "$SKILL_DIR/templates/verify_run_test.py" verify/run_test.py
chmod +x verify/run_test.py
```

Only append to `llm_chat_log.json`; do not rewrite or summarize previous turns.

## Phase 1: Inventory And Candidate Extraction

Run:

```bash
python3 work/skills/vuln-mining-autonomous/scripts/source_inventory.py
python3 work/skills/vuln-mining-autonomous/scripts/attack_surface_map.py
python3 work/skills/vuln-mining-autonomous/scripts/sast_candidates.py
```

These scripts must write:

- `reports/source-inventory.md`
- `reports/attack-surface-map.md`
- `reports/sast-candidates.md`

Then write `plans/scan-wave-001.md` with:

- top 5 candidate files or functions;
- source/sink/sanitizer hypothesis for each candidate;
- why each candidate is high risk and how a malformed input could reach it;
- the exact black-box prompt planned for the first LLM review;
- test strategy ideas, including expected runtime signal if the bug is real.

Do not keep inventory findings only in chat.

## Phase 2: LLM Review Logging

For each candidate wave:

1. Read a bounded source slice. Prefer one file or one function family at a time.
2. Build a prompt that contains only:
   - generic role: C/C++ security audit;
   - `TARGET_ROOT` relative source path;
   - source snippet or summarized code facts;
   - vulnerability patterns to consider.
3. Append the exact user prompt to `llm_chat_log.json`.
4. Write the assistant analysis as the next assistant turn in `llm_chat_log.json`.
5. Append a skeptic prompt asking whether the candidate is a false positive,
   which sanitizer or precondition blocks it, and what runtime input would prove
   or disprove it. Record the answer in `llm_chat_log.json`.
6. Write accepted or rejected hypotheses to `reports/hypotheses.md` with:
   relative path, suspicious point, source, sink, sanitizer status, planned
   reproducer, and rejection reason if rejected.

Reject any candidate immediately if the evidence depends on memory of preseeded findings
or vulnerability databases.

## Phase 3: Runtime Test Generation

All tests in `verify/run_test.py` must be AI-generated during this run.

Required properties:

- one test function per candidate vulnerability;
- runnable from the project root with `python3 verify/run_test.py`;
- capture exceptions, process exit codes, stderr, stdout, and signals;
- write `reports/verification-output.txt`;
- clearly print `VERIFIED` only for candidates with runtime evidence.

Verification ladder:

1. Try the smallest existing runtime surface: imported module, command-line tool,
   unit test helper, or example binary already present in the validation tree.
2. If needed, generate a small native repro under `verify/` and compile it with
   available local compilers. Use `-fsanitize=address,undefined` when supported.
3. If the target accepts bytes or serialized structures, generate a bounded
   fuzz-style loop with empty, tiny, malformed, max-size, negative, zero, and
   overflow-adjacent seeds. Keep runtime bounded.
4. If the reproducer cannot execute, reject the candidate and return to Phase 2.

If a runtime package or build artifact is unavailable, do not claim the bug.
Generate a narrower test or choose another candidate.

## Phase 4: Deliverables

`vulnerability_list.md` must contain at least one runtime-verified vulnerability
and follow this structure:

```markdown
## 漏洞 #1：[简短直观的漏洞名称]

- **漏洞类型**：[DoS / FPE / Heap Overflow / OOB / UAF / Type Confusion / Integer Overflow / Null Deref]
- **严重级别**：[高危 (High) / 中危 (Medium) / 低危 (Low)]
- **问题源码路径**：[relative source path]

### 成因简述
...

### AI生成的测试用例
...

### 验证结果
...

### 与LLM交互中哪句提示词发现了bug
...

### 为什么选择此提示词
...

### 潜在业务危害
...
```

`vulnerability_report.md` must describe the engineering method, SAST candidates,
LLM review waves, rejected false positives, runtime verification, and remaining
risk.

`result/output.md` must state the final status, verified vulnerability count,
verification command, and report paths.

## Phase 5: Final Gate

Run:

```bash
python3 work/skills/vuln-mining-autonomous/scripts/final_verify.py
```

If it fails, fix the exact failure and rerun. Do not report completion until it
passes.
