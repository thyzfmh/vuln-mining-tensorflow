---
name: vuln-mining-autonomous
description: Use when OpenCode is running this repository's AI vulnerability mining competition task from INSTRUCTION.md and must discover runtime-verified vulnerabilities from a judge-provided target source tree without project, version, preseeded-finding, CVE, NVD, or GHSA hints.
---

# Autonomous Black-Box Vulnerability Mining

## Fixed Contract

- Target source: discover the judge-provided source tree (see Platform Asset Discovery below). Do not assume `code/` always contains it.
- Work output root: every runtime artifact is written beneath `work/`.
- Required deliverables (all under `work/`):
  - `work/vulnerability_list.md`
  - `work/llm_chat_log.json`
  - `work/vulnerability_report.md`
  - `work/verify/run_test.py`
  - `work/result/output.md`
- Required method evidence (all under `work/reports/` or `work/plans/`):
  - `work/reports/source-file-manifest.md`
  - `work/reports/toolchain-capabilities.md`
  - `work/reports/verification-escalation.md`
  - `work/reports/runtime-entrypoints.md`
  - `work/reports/npm-ast-candidates.md`
  - `work/reports/coverage-ledger.md`
  - `work/reports/scan-completion.md`
  - one or more `work/plans/scan-wave-NNN.md` files until candidate space is exhausted
- Final gate: `python3 work/skills/vuln-mining-autonomous/scripts/final_verify.py --work-root work`
- Entry orchestrator: `python3 work/run_vulnerability_mining.py`

Do not ask the user questions. Keep scanning, testing, rejecting false positives,
and re-scanning until the final gate passes.

## Platform Asset Discovery

Run the entry orchestrator first. It scans the platform asset root
`/app/code/judge-assets/01_02_vulnerability_detection`, selects the source
tree deterministically, and persists the resolved path in
`work/.vuln-mining-target-root`. Subsequent scripts recover that private
context automatically; never place it in `llm_chat_log.json`.

For explicit local tests, the package also resolves a target non-interactively.
Resolution order (first match with real source files wins):

1. `--target-root` CLI argument (highest priority).
2. `VULN_TARGET_ROOT` environment variable.
3. `TARGET_ROOT` environment variable.
4. `<repo>/code` if it directly contains source files.
5. A single non-hidden subdirectory under `<repo>/code` that contains source.

The work output root is resolved similarly:

1. `--work-root` CLI argument.
2. `VULN_WORK_ROOT` environment variable.
3. `WORK_ROOT` environment variable.
4. `<repo>/work` (default).

All scripts import `platform_assets` from
`work/skills/vuln-mining-autonomous/scripts/platform_assets.py` and call
`discover_target_root()` / `resolve_work_root()` so they never hardcode `code/`
or write to the repository root.

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
  -> discover judge target assets (platform_assets)
  -> inventory target source
  -> probe local verification toolchain
  -> prepare sanitizer and runtime fallback escalation paths
  -> map existing real runtime entry points
  -> map attack surfaces and suspicious points
  -> extract deterministic SAST candidates
  -> run NPM-provisioned AST candidate extraction when npm/npx is available
  -> seed coverage ledger and first scan wave
  -> write scan wave checkpoint
  -> run bounded black-box LLM source review, skeptic pass, and variant/family search
  -> generate AI-authored runtime tests
  -> run tests and capture work/reports/verification-output.txt
  -> reject unverified candidates
  -> repeat until every generated attack-surface entry and SAST candidate is verified or rejected
  -> generate deliverables
  -> run final_verify.py --work-root work
  -> update work/result/output.md
```

Never stop because some vulnerability count has been reached. The goal is to
find all runtime-verifiable vulnerabilities exposed by the generated candidate
space. A vulnerability can appear in `work/vulnerability_list.md` only after
runtime verification evidence exists, and the run is not complete until the
coverage ledger and scan-completion report show that all generated
attack-surface entries and SAST candidates have been triaged to a final state.

## Scan Completion Requirements

The final gate checks method evidence, not only output format:

1. Generate `work/reports/source-file-manifest.md` from a deterministic
   full-tree traversal. This manifest is the proof that all source files were
   considered by automation before LLM review.
2. Review every generated entry from `work/reports/attack-surface-map.md` and
   every generated entry from `work/reports/sast-candidates.md`. Do not use a
   fixed top-N cap as the completion standard.
3. Create `work/reports/coverage-ledger.md` before LLM review and keep it
   current. Use this exact header block:

```markdown
# Coverage Ledger

coverage-budget: all generated attack-surface, SAST, and NPM AST candidate entries
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
4. Keep writing `work/plans/scan-wave-NNN.md` files until no unreviewed ledger
   rows remain. Each wave should cover a bounded batch and may include variant
   or family searches, but completion is based on candidate exhaustion, not on
   reaching a specific number of vulnerabilities.
5. Set `candidate-space-exhausted: yes` only after all ledger rows are
   `verified` or `rejected` and all accepted hypotheses have runtime proof or
   have been moved to rejected.
6. For every high-risk target, record one of:
   - `verified`: runtime evidence reproduced the issue;
   - `rejected`: source evidence, sanitizer/precondition, or runtime test
     disproved the hypothesis.
7. Write `work/reports/scan-completion.md` with these exact completion
   assertions:
   - `all source files inventoried`
   - `candidate extraction ran over full manifest`
   - `all attack-surface entries reviewed`
   - `all sast candidates triaged`
   - `all npm ast candidates triaged`
   - `no unverified accepted hypotheses remain`
   - `all runtime-verified vulnerabilities listed`

Do not use `deferred`, `unknown`, or `needs follow-up` as final statuses for
budget targets.

## Large-Repository Strategy

Do not try to make the LLM read the whole repository line by line. For a large
target, "scan all code" means:

1. Deterministic scripts traverse every source file and write
   `work/reports/source-file-manifest.md`.
2. Deterministic scripts rank every file/function that matches source, sink,
   invariant, parser, native-boundary, arithmetic, or memory-safety patterns.
3. A deterministic runtime-entrypoint map links candidates to existing tests,
   binaries, parsers, bindings, and build targets before a custom reproducer
   is considered.
4. When a local semantic scanner is available, use source-to-sink or taint-flow
   results to expand the same candidate ledger across the full target tree.
5. When npm/npx is available, run the pinned `@ast-grep/cli` through `npx` to
   add structural C/C++ and Python matches to the same candidate ledger.
6. The LLM reviews bounded batches from the generated candidate space, with a
   skeptic pass for every accepted hypothesis.
7. Runtime tests are generated only for hypotheses that survive source review.
8. Variant/family search expands from verified or accepted patterns until that
   family is also verified or rejected.
9. The run finishes only when the coverage ledger proves no generated candidate
   or accepted hypothesis remains open.

This is not a mathematical proof that no vulnerability exists in unreachable or
unmodeled code. It is a repeatable whole-repository scan over the generated
candidate space, with explicit residual risk documented in the report.

## Missing Sanitizer Policy

Run `scripts/probe_verification_tools.py` before verification. It writes
`work/reports/toolchain-capabilities.md` with ASAN, UBSAN, compiler, and
build-tool availability.
Then run `scripts/escalate_verification_tools.py`. It must try alternate
compiler locations, sanitizer flag combinations, build-system sanitizer
injection paths, and non-sanitizer runtime fallbacks such as Valgrind,
Guard Malloc, Python debug runtime, or another real target command.

- If ASAN or UBSAN is available, use it for native memory-safety and
  undefined-behavior claims.
- If ASAN or UBSAN is unavailable in the first probe, do not stop. Use
  `work/reports/verification-escalation.md` to retry with alternate compilers,
  project build-system flags, and runtime fallback tools.
- If the target package cannot be imported directly from the source tree, do
  not stop. Try a real command-line tool, Python subprocess with adjusted
  `PYTHONPATH`, existing tests, native reproducer, parser entry point, generated
  minimal model/input, or build-system test target.
- Read `work/reports/runtime-entrypoints.md` before choosing a fallback. Prefer
  an existing test, binary, parser, module binding, or registered build target
  over an unbuilt source-tree import.
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
  `work/reports/verification-escalation.md`, `work/reports/hypotheses.md`, and
  `work/vulnerability_report.md`.

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

1. Prepare the `work/` output root and run the mandatory first inventory pass:

```bash
python3 work/run_vulnerability_mining.py
```

   The orchestrator performs platform-asset discovery, copies the required
   output templates when absent, executes the Phase 1 extraction scripts, and
   writes the private target context under `work/`. Do not write the real
   target basename into `llm_chat_log.json`.

2. Confirm the required directories now exist under `work/`:

```bash
WORK_ROOT="work"
SKILL_DIR="work/skills/vuln-mining-autonomous"
mkdir -p "$WORK_ROOT/reports" "$WORK_ROOT/plans" "$WORK_ROOT/verify" "$WORK_ROOT/result"
```

3. Copy templates only when outputs do not exist:

```bash
[ -f "$WORK_ROOT/llm_chat_log.json" ] || cp "$SKILL_DIR/templates/llm_chat_log.json" "$WORK_ROOT/llm_chat_log.json"
[ -f "$WORK_ROOT/vulnerability_list.md" ] || cp "$SKILL_DIR/templates/vulnerability_list.md" "$WORK_ROOT/vulnerability_list.md"
[ -f "$WORK_ROOT/vulnerability_report.md" ] || cp "$SKILL_DIR/templates/vulnerability_report.md" "$WORK_ROOT/vulnerability_report.md"
[ -f "$WORK_ROOT/verify/run_test.py" ] || cp "$SKILL_DIR/templates/verify_run_test.py" "$WORK_ROOT/verify/run_test.py"
chmod +x "$WORK_ROOT/verify/run_test.py"
```

Only append to `llm_chat_log.json`; do not rewrite or summarize previous turns.

## Phase 1: Inventory And Candidate Extraction

The orchestrator already runs this once. Re-run an individual extractor only
when a later scan wave needs refreshed evidence:

```bash
python3 work/skills/vuln-mining-autonomous/scripts/source_inventory.py
python3 work/skills/vuln-mining-autonomous/scripts/probe_verification_tools.py
python3 work/skills/vuln-mining-autonomous/scripts/escalate_verification_tools.py
python3 work/skills/vuln-mining-autonomous/scripts/runtime_entrypoints.py
python3 work/skills/vuln-mining-autonomous/scripts/attack_surface_map.py
python3 work/skills/vuln-mining-autonomous/scripts/sast_candidates.py
python3 work/skills/vuln-mining-autonomous/scripts/npm_ast_candidates.py
python3 work/skills/vuln-mining-autonomous/scripts/init_coverage_ledger.py
```

These scripts must write (all under `WORK_ROOT`):

- `reports/source-inventory.md`
- `reports/source-file-manifest.md`
- `reports/toolchain-capabilities.md`
- `reports/verification-escalation.md`
- `reports/runtime-entrypoints.md`
- `reports/attack-surface-map.md`
- `reports/sast-candidates.md`
- `reports/npm-ast-candidates.md`
- `reports/coverage-ledger.md`

The coverage ledger is initialized from the union of generated attack-surface,
SAST, and NPM AST candidate entries. Leave `candidate-space-exhausted` as `no`
until every generated entry has a final `verified` or `rejected` status.

Then write `plans/scan-wave-001.md` with:

- the next bounded batch of candidate files or functions;
- source/sink/sanitizer hypothesis for each candidate;
- why each candidate is high risk and how a malformed input could reach it;
- the exact black-box prompt planned for the first LLM review;
- test strategy ideas, including expected runtime signal if the bug is real.

Do not keep inventory findings only in chat. Every candidate selected or
rejected during the run must be reflected in `reports/coverage-ledger.md`.

If npm/npx is available, `npm_ast_candidates.py` must invoke the pinned
`@ast-grep/cli` package and merge every reported file into the same coverage
ledger. An NPM download failure is a recorded tool-path failure, not a reason to
end scanning or report zero findings.

If `reports/toolchain-capabilities.md` shows CodeQL or Semgrep is available,
run its source-to-sink or taint analysis as an additional full-tree candidate
source. Add every resulting path to the same coverage ledger before declaring
the candidate space exhausted. For compiled code, let semantic extraction use
the normal project build instead of hand-written include-path guesses.

## Phase 2: LLM Review Logging

For each candidate wave:

1. Read a bounded source slice. Prefer one file or one function family at a time.
2. Build a prompt that contains only:
   - generic role: C/C++/Python security audit;
   - `TARGET_ROOT` relative source path;
   - source snippet or summarized code facts;
   - vulnerability patterns to consider.
3. Append the exact user prompt to `work/llm_chat_log.json`.
4. Write the assistant analysis as the next assistant turn in `work/llm_chat_log.json`.
5. Append a skeptic prompt asking whether the candidate is a false positive,
   which sanitizer or precondition blocks it, and what runtime input would prove
   or disprove it. Record the answer in `work/llm_chat_log.json`.
6. Write accepted or rejected hypotheses to `work/reports/hypotheses.md` with:
   relative path, suspicious point, source, sink, sanitizer status, planned
   reproducer, and rejection reason if rejected.
7. Update `work/reports/coverage-ledger.md` for each reviewed target with
   `verified` or `rejected`, and link the relevant prompt, hypothesis, source
   evidence, or verification output.

Reject any candidate immediately if the evidence depends on memory of preseeded
findings or vulnerability databases.

When a wave leaves unreviewed ledger rows, write the next
`work/plans/scan-wave-NNN.md` and continue. When a verified finding appears,
add variant/family candidates to the ledger and triage them too. Do not set
`candidate-space-exhausted: yes` and do not move to final reports until there
are no unreviewed rows, no unverified accepted hypotheses, and no untriaged
variant-family candidates.

## Phase 3: Runtime Test Generation

All tests in `work/verify/run_test.py` must be AI-generated during this run.

Required properties:

- one test function per candidate vulnerability;
- runnable from the project root with `python3 work/verify/run_test.py`;
- capture exceptions, process exit codes, stderr, stdout, and signals;
- write `work/reports/verification-output.txt`;
- clearly print `VERIFIED` only for candidates with runtime evidence.
- call a real target API, command, parser, test helper, imported module, or
  compile a native reproducer that includes or links target source under
  `TARGET_ROOT`;
- never verify by compiling or running a toy snippet that merely imitates a
  pattern such as division by zero, out-of-bounds indexing, or overflow.

Verification ladder:

1. Try the smallest existing runtime surface: imported module, command-line
   tool, unit test helper, registered build target, parser, language binding, or
   example binary listed in `work/reports/runtime-entrypoints.md`.
2. Establish a valid baseline and run a bounded malformed-input seed matrix
   through that same real target surface. Record every command, seed outcome,
   crash, and available coverage signal in `work/reports/verification-output.txt`.
3. If needed, generate a small native repro under `work/verify/` and compile it
   with available local compilers. It must include or link target source/header
   files. Use `-fsanitize=address,undefined` when supported, and record compiler
   support or failures in `work/reports/verification-output.txt`.
4. If the target accepts bytes or serialized structures, generate a bounded
   fuzz-style loop with empty, tiny, malformed, max-size, negative, zero, and
   overflow-adjacent seeds. Use libFuzzer and coverage tooling when available,
   while treating coverage as prioritization evidence rather than proof. Keep
   runtime bounded.
5. If the reproducer cannot execute, reject the candidate and return to Phase 2.

If a runtime package or build artifact is unavailable, do not claim the bug.
Generate a narrower test or choose another candidate.

For memory-safety or undefined-behavior claims, sanitizer output is preferred.
If sanitizer tooling is unavailable, the report must state that fact and the
runtime evidence must still come from a real target execution path.

## Phase 4: Deliverables

`work/vulnerability_list.md` must contain at least one runtime-verified
vulnerability and follow this structure:

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

`work/vulnerability_report.md` must describe the engineering method, SAST
candidates, NPM AST candidates, LLM review waves, rejected false positives,
runtime verification, and remaining risk. It must also summarize:

- coverage budget and reviewed target count from `work/reports/coverage-ledger.md`;
- the candidate-space exhaustion rationale from `work/reports/scan-completion.md`;
- negative conclusions for rejected high-risk targets;
- whether ASAN/UBSAN or equivalent runtime instrumentation was used for native
  memory and undefined-behavior claims.

`work/result/output.md` must state the final status, verified vulnerability
count, coverage ledger status, verification command, and report paths.

## Phase 5: Final Gate

Run:

```bash
python3 work/skills/vuln-mining-autonomous/scripts/final_verify.py \
  --work-root work
```

If it fails, fix the exact failure and rerun. Do not report completion until it
passes.
