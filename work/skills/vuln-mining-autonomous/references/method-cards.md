# Vulnerability Mining Method Cards

Use one card per scan wave. Keep prompts black-box and refer to the source tree
as `TARGET_ROOT`.

## Card 1: Source To Sink Trace

1. Pick one suspicious point from `reports/attack-surface-map.md`.
2. Identify attacker-controlled or malformed inputs.
3. Trace propagation into allocation size, index, memory copy, division, cast,
   fatal assertion, or native boundary.
4. List every sanitizer or guard on that path.
5. Ask the LLM for one concrete input that bypasses or satisfies the guards.

## Card 2: Boundary Invariant Attack

Use this for math, tensor/array, shape, parser, and runtime dispatch code.

Probe these values: zero, negative, one, maximum rank/count, empty input,
single-element input, mismatched dimensions, duplicated axes, sortedness
violations, overflow-adjacent sizes, and malformed serialized fields.

Accept the hypothesis only if the source shows the value can reach the sink
without a blocking guard.

## Card 3: Crash-To-Security Triage

Not every crash is a reportable vulnerability. Treat it as reportable only when:

1. The input can plausibly be controlled by a user, model file, request payload,
   plugin, dataset, or serialized artifact.
2. The crash crosses a service/runtime boundary or can abort a long-lived process.
3. A test generated during this run reproduces the crash or sanitizer finding.

Reject crashes that require editing production code, debugger-only state, or
impossible internal invariants.

## Card 4: Variant Search

After one verified bug:

1. Extract the pattern as `source -> missing sanitizer -> sink`.
2. Search the same sink and sanitizer vocabulary across neighboring directories.
3. Prefer similarly named functions with different dtype, device, parser, or
   shape-specialization branches.
4. Generate one new bounded prompt per variant and require independent runtime
   evidence before listing it.

## Card 5: Verification Builder

Build the narrowest proof available:

1. Existing script/module/API call with malformed arguments.
2. Existing test runner with a new AI-generated test.
3. Small native reproducer compiled under `verify/`.
4. Sanitized native build using `-fsanitize=address,undefined` when supported.
5. Bounded fuzz-style loop with a fixed seed corpus and timeout.

The verification output must include command, exit status, stdout, stderr, and
the `VERIFIED` marker only for reproduced findings.

## Card 6: Entrypoint And Corpus Route

1. Read `reports/runtime-entrypoints.md` before generating a reproducer.
2. Prefer an existing test target, command-line binary, parser, or language
   binding over importing an unbuilt source tree directly.
3. Establish one valid baseline input, then run a bounded corpus containing
   empty, tiny, malformed, zero, negative, boundary, and overflow-adjacent
   inputs through the same real target surface.
4. When available, use libFuzzer or coverage tooling to rank seeds that reach
   new code. Coverage is a prioritization signal, not runtime proof by itself.
5. Treat an expected input-validation error as rejection evidence. Accept only
   an unexpected crash, signal, fatal check, sanitizer finding, or other
   concrete failure on a real target path.

## Card 7: Semantic Flow Expansion

1. If `reports/toolchain-capabilities.md` shows CodeQL or Semgrep is available,
   run the applicable source-to-sink or taint rules over the full target tree.
2. For compiled code, let semantic extraction observe the normal build rather
   than guessing include paths or preprocessor definitions.
3. Add every static flow result to `reports/coverage-ledger.md` as a candidate,
   including its source, propagator, sanitizer, and sink evidence.
4. Use those paths to select the smallest route from
   `reports/runtime-entrypoints.md`; do not treat a static match as verified.
