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
- Required method evidence:
  - `reports/source-file-manifest.md`
  - `reports/toolchain-capabilities.md`
  - `reports/verification-escalation.md`
  - `reports/coverage-ledger.md`
  - `reports/scan-completion.md`
  - one or more `plans/scan-wave-NNN.md` files until candidate space is exhausted
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
  -> probe local verification toolchain
  -> prepare sanitizer and runtime fallback escalation paths
  -> map attack surfaces and suspicious points
  -> extract deterministic SAST candidates
  -> seed coverage ledger and first scan wave
  -> write scan wave checkpoint
  -> run bounded black-box LLM source review, skeptic pass, and variant/family search
  -> generate AI-authored runtime tests
  -> run tests and capture reports/verification-output.txt
  -> reject unverified candidates
  -> repeat until every generated attack-surface entry and SAST candidate is verified or rejected
  -> generate deliverables
  -> run final_verify.py
  -> update result/output.md
```

Never stop because some vulnerability count has been reached. The goal is to
find all runtime-verifiable vulnerabilities exposed by the generated candidate
space. A vulnerability can appear in `vulnerability_list.md` only after runtime
verification evidence exists, and the run is not complete until the coverage
ledger and scan-completion report show that all generated attack-surface entries
and SAST candidates have been triaged to a final state.

## Scan Completion Requirements

The final gate checks method evidence, not only output format:

1. Generate `reports/source-file-manifest.md` from a deterministic full-tree
   traversal. This manifest is the proof that all source files were considered
   by automation before LLM review.
2. Review every generated entry from `reports/attack-surface-map.md` and every
   generated entry from `reports/sast-candidates.md`. Do not use a fixed top-N
   cap as the completion standard.
3. Create `reports/coverage-ledger.md` before LLM review and keep it current.
   Use this exact header block:

```markdown
# Coverage Ledger

coverage-budget: all generated attack-surface and SAST candidate entries
minimum-reviewed-targets: N
candidate-space-exhausted: no
```

   Then maintain a table with these columns:

```markdown
| Target | Domain | Source | Sink | Sanitizer status | Status | Evidence |
|---|---|---|---|---|---|---|
```

   Count a target as reviewed only when `Status` is `verified` or `rejected`
   and `Evidence` points to a prompt, hypothesis entry, test, command output,
   or source-based rejection reason.
4. Keep writing `plans/scan-wave-NNN.md` files until no unreviewed ledger rows
   remain. Each wave should cover a bounded batch and may include variant or
   family searches, but completion is based on candidate exhaustion, not on
   reaching a specific number of vulnerabilities.
5. Set `candidate-space-exhausted: yes` only after all ledger rows are
   `verified` or `rejected` and all accepted hypotheses have runtime proof or
   have been moved to rejected.
6. For every high-risk target, record one of:
   - `verified`: runtime evidence reproduced the issue;
   - `rejected`: source evidence, sanitizer/precondition, or runtime test
     disproved the hypothesis.
7. Write `reports/scan-completion.md` with these exact completion assertions:
   - `all source files inventoried`
   - `candidate extraction ran over full manifest`
   - `all attack-surface entries reviewed`
   - `all sast candidates triaged`
   - `no unverified accepted hypotheses remain`
   - `all runtime-verified vulnerabilities listed`

Do not use `deferred`, `unknown`, or `needs follow-up` as final statuses for
budget targets.

## Large-Repository Strategy

Do not try to make the LLM read the whole repository line by line. For a large
target, "scan all code" means:

1. Deterministic scripts traverse every source file and write
   `reports/source-file-manifest.md`.
2. Deterministic scripts rank every file/function that matches source, sink,
   invariant, parser, native-boundary, arithmetic, or memory-safety patterns.
3. The LLM reviews bounded batches from the generated candidate space, with a
   skeptic pass for every accepted hypothesis.
4. Runtime tests are generated only for hypotheses that survive source review.
5. Variant/family search expands from verified or accepted patterns until that
   family is also verified or rejected.
6. The run finishes only when the coverage ledger proves no generated candidate
   or accepted hypothesis remains open.

This is not a mathematical proof that no vulnerability exists in unreachable or
unmodeled code. It is a repeatable whole-repository scan over the generated
candidate space, with explicit residual risk documented in the report.

## Missing Sanitizer Policy

Run `scripts/probe_verification_tools.py` before verification. It writes
`reports/toolchain-capabilities.md` with ASAN, UBSAN, compiler, and build-tool
availability.
Then run `scripts/escalate_verification_tools.py`. It must try alternate
compiler locations, sanitizer flag combinations, build-system sanitizer
injection paths, and non-sanitizer runtime fallbacks such as Valgrind,
Guard Malloc, Python debug runtime, or another real target command.

- If ASAN or UBSAN is available, use it for native memory-safety and
  undefined-behavior claims.
- If ASAN or UBSAN is unavailable in the first probe, do not stop. Use
  `reports/verification-escalation.md` to retry with alternate compilers,
  project build-system flags, and runtime fallback tools.
- If the target package cannot be imported directly from the source tree, do
  not stop. Try a real command-line tool, Python subprocess with adjusted
  `PYTHONPATH`, existing tests, native reproducer, parser entry point, generated
  minimal model/input, or build-system test target.
- Toolchain limits, sanitizer absence, and source-tree import failures are
  verification-path failures, not scan-completion reasons. Continue scanning
  other candidates and alternate runtime surfaces until a runtime-verified
  finding exists and the candidate space is exhausted.
- Do not list memory-safety or undefined-behavior hypotheses merely from source
  reasoning or a toy program.
- Without sanitizer support, a candidate can still be listed only if a real
  target execution path produces concrete runtime evidence: crash, signal,
  exception, fatal check, or equivalent observable failure.
- If neither sanitizer evidence nor real target runtime proof exists, reject the
  hypothesis only after the escalation attempts are recorded in
  `reports/verification-escalation.md`, `reports/hypotheses.md`, and
  `vulnerability_report.md`.

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
4. For every verified bug and every accepted hypothesis, search variants by the
   same source/sink/sanitizer pattern across neighboring files and similarly
   named functions until that variant family is verified or rejected.
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
python3 work/skills/vuln-mining-autonomous/scripts/probe_verification_tools.py
python3 work/skills/vuln-mining-autonomous/scripts/escalate_verification_tools.py
python3 work/skills/vuln-mining-autonomous/scripts/attack_surface_map.py
python3 work/skills/vuln-mining-autonomous/scripts/sast_candidates.py
python3 work/skills/vuln-mining-autonomous/scripts/init_coverage_ledger.py
```

These scripts must write:

- `reports/source-inventory.md`
- `reports/source-file-manifest.md`
- `reports/toolchain-capabilities.md`
- `reports/verification-escalation.md`
- `reports/attack-surface-map.md`
- `reports/sast-candidates.md`
- `reports/coverage-ledger.md`

The coverage ledger is initialized from the union of generated attack-surface
and SAST candidate entries. Leave `candidate-space-exhausted` as `no` until
every generated entry has a final `verified` or `rejected` status.

Then write `plans/scan-wave-001.md` with:

- the next bounded batch of candidate files or functions;
- source/sink/sanitizer hypothesis for each candidate;
- why each candidate is high risk and how a malformed input could reach it;
- the exact black-box prompt planned for the first LLM review;
- test strategy ideas, including expected runtime signal if the bug is real.

Do not keep inventory findings only in chat. Every candidate selected or
rejected during the run must be reflected in `reports/coverage-ledger.md`.

## Phase 2: LLM Review Logging

For each candidate wave:

1. Read a bounded source slice. Prefer one file or one function family at a time.
2. Build a prompt that contains only:
   - generic role: C/C++/Python security audit;
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
7. Update `reports/coverage-ledger.md` for each reviewed target with `verified`
   or `rejected`, and link the relevant prompt, hypothesis, source evidence, or
   verification output.

Reject any candidate immediately if the evidence depends on memory of preseeded findings
or vulnerability databases.

When a wave leaves unreviewed ledger rows, write the next
`plans/scan-wave-NNN.md` and continue. When a verified finding appears, add
variant/family candidates to the ledger and triage them too. Do not set
`candidate-space-exhausted: yes` and do not move to final reports until there
are no unreviewed rows, no unverified accepted hypotheses, and no untriaged
variant-family candidates.

## Phase 3: Runtime Test Generation

All tests in `verify/run_test.py` must be AI-generated during this run.

Required properties:

- one test function per candidate vulnerability;
- runnable from the project root with `python3 verify/run_test.py`;
- capture exceptions, process exit codes, stderr, stdout, and signals;
- write `reports/verification-output.txt`;
- clearly print `VERIFIED` only for candidates with runtime evidence.
- call a real target API, command, parser, test helper, imported module, or
  compile a native reproducer that includes or links target source under
  `TARGET_ROOT`;
- never verify by compiling or running a toy snippet that merely imitates a
  pattern such as division by zero, out-of-bounds indexing, or overflow.

Verification ladder:

1. Try the smallest existing runtime surface: imported module, command-line tool,
   unit test helper, or example binary already present in the validation tree.
2. If needed, generate a small native repro under `verify/` and compile it with
   available local compilers. It must include or link target source/header files.
   Use `-fsanitize=address,undefined` when supported, and record compiler
   support or failures in `reports/verification-output.txt`.
3. If the target accepts bytes or serialized structures, generate a bounded
   fuzz-style loop with empty, tiny, malformed, max-size, negative, zero, and
   overflow-adjacent seeds. Keep runtime bounded.
4. If the reproducer cannot execute, reject the candidate and return to Phase 2.

If a runtime package or build artifact is unavailable, do not claim the bug.
Generate a narrower test or choose another candidate.

For memory-safety or undefined-behavior claims, sanitizer output is preferred.
If sanitizer tooling is unavailable, the report must state that fact and the
runtime evidence must still come from a real target execution path.

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
risk. It must also summarize:

- coverage budget and reviewed target count from `reports/coverage-ledger.md`;
- the candidate-space exhaustion rationale from `reports/scan-completion.md`;
- negative conclusions for rejected high-risk targets;
- whether ASAN/UBSAN or equivalent runtime instrumentation was used for native
  memory and undefined-behavior claims.

`result/output.md` must state the final status, verified vulnerability count,
coverage ledger status, verification command, and report paths.

## Phase 5: Final Gate

Run:

```bash
python3 work/skills/vuln-mining-autonomous/scripts/final_verify.py
```

If it fails, fix the exact failure and rerun. Do not report completion until it
passes.
