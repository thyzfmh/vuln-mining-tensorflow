---
name: vuln-mining
description: AI-driven vulnerability mining for large C++ codebases. Executes systematic scanning — SAST, semantic analysis, evidence verification — until real vulnerabilities are found and proven.
---

# AI Vulnerability Mining — TensorFlow

This skill discovers real vulnerabilities in a large C++ codebase using AI. The target path is fixed.

## Fixed Paths

- **Target**: `code/tensorflow` (C++, v2.11.0)
- **Deliverables directory**: `result/`

## Core Rule

**Execute the ENTIRE flow below without stopping. Do NOT ask the user any questions. Do NOT pause between phases. Keep going until `final_verify.sh` passes.**

If a verification step fails, fix it immediately and re-verify. Keep fixing until it passes.

Only escalate to the user if:
1. The target code cannot be accessed
2. 3 consecutive scan rounds produce zero new vulnerabilities
3. A critical zero-day is discovered requiring disclosure decisions

## Black-Box Rules (CRITICAL)

1. **NEVER** tell the LLM the project name ("TensorFlow"), version ("v2.11.0"), or any known CVE
2. **NEVER** hint at specific bugs or vulnerability types expected
3. **NEVER** provide expected findings in prompts
4. All prompts must be generic: "Analyze this C++ source file for potential security issues"
5. The LLM must discover everything autonomously from the source code alone

## Phase 0: Intake

1. Verify `code/tensorflow` exists and contains source files. If not, report the problem and stop.
2. Tell the user what will happen:

   ```text
   我会按 AI 漏洞挖掘流程处理目标代码：

   - 目标代码：code/tensorflow
   - 扫描策略：SAST + LLM 语义分析 + 证据验证
   - 交付件：vulnerability_list.md, llm_chat_log.json, vulnerability_report.md
   - 验证方式：漏洞可复现性、证据链完整性、交付件格式合规

   接下来我会自动完成扫描，直到最终验证通过。
   ```

3. Proceed to **Phase 1** immediately.

## Phase 1: Target Analysis

### 1.1 Codebase Inventory

Run `./harness/analyze_target.sh code/tensorflow` to generate `reports/source-inventory.md`.

The inventory must include:
- Total file count by extension (.cc, .h, .cu, etc.)
- Module breakdown (kernels, ops, core, framework, etc.)
- High-risk areas identification (external input handling, memory ops, arithmetic)
- Estimated attack surface

### 1.2 Attack Surface Prioritization

Focus scanning on these high-risk areas (in priority order):

1. **Op/Kernel implementations** — process external input, shape calculations, arithmetic
   - Path: `tensorflow/core/kernels/`
   - Risk: input validation gaps, integer overflow, division by zero
2. **Framework layer** — memory management, tensor operations
   - Path: `tensorflow/core/framework/`
   - Risk: null deref, use-after-free, type confusion
3. **Platform/File I/O** — filesystem operations, parsing
   - Path: `tensorflow/core/platform/`, `tensorflow/core/util/`
   - Risk: path traversal, resource exhaustion
4. **Python/C++ boundary** — pybind11, SWIG bindings
   - Path: `tensorflow/python/`
   - Risk: type confusion, boundary validation

### 1.3 Scan Planning

Generate scan plans under `plans/`:

```bash
./harness/plan_next_scan.sh scan-001 "SAST scan for input validation in kernel ops" "tensorflow/core/kernels/"
```

## Phase 2: SAST-Led Scanning

### 2.1 Static Pattern Scan

For each high-risk module, scan for these patterns:

| Pattern | Regex / Indicator | Vulnerability Type |
|---------|-------------------|-------------------|
| Division without zero-check | `/ [^=] /` near `shape.dim_size()` or `ksize` | FPE / DoS |
| Unchecked pointer deref | `->` or `*` without null check after allocation | Null deref / DoS |
| Integer overflow | `int32` used for size calculation then cast to `size_t` | Heap overflow |
| Unchecked array index | `(` `)` `[` without bounds check | OOB read/write |
| Missing error handling | `Status` returned but not checked | Logic error |
| Raw pointer lifecycle | `new` without matching `delete` or smart pointer | Memory leak / UAF |
| Unsafe cast | `static_cast<` / `reinterpret_cast<` without validation | Type confusion |
| Uncapped allocation | `Allocate` / `new` with user-controlled size | DoS / OOM |

### 2.2 Pattern-Based Prompt Engineering

For each pattern found, craft an LLM prompt following black-box rules:

```text
BAD (leaks version info):
  "TensorFlow v2.11.0 has a division by zero bug in pooling ops"

GOOD (black-box):
  "Analyze the following C++ source code for potential division-by-zero 
   vulnerabilities. Focus on arithmetic operations where the divisor 
   could be zero. Provide the exact line numbers and explain the 
   trigger condition."
```

### 2.3 Record All Interactions

Every LLM prompt and response must be appended to `result/llm_chat_log.json`.

## Phase 3: LLM Semantic Analysis

### 3.1 Module-by-Module Deep Scan

For each high-risk module identified in Phase 1:

1. Read the source file
2. Craft a black-box prompt asking the LLM to analyze for vulnerabilities
3. Record the interaction in `llm_chat_log.json`
4. If the LLM identifies a potential vulnerability, proceed to evidence verification

### 3.2 Prompt Templates (Black-Box Compliant)

**Input validation scan:**
```text
"Analyze this C++ source file for missing input validation that could 
lead to crashes or undefined behavior. Focus on: (1) parameters that 
could be zero causing division errors, (2) negative values causing 
buffer underflow, (3) missing null checks after allocation. For each 
finding, provide: exact source line, the missing validation, and what 
input would trigger the bug."
```

**Arithmetic safety scan:**
```text
"Review this C++ code for arithmetic safety issues: (1) integer 
overflow in size calculations, (2) signed/unsigned mismatch, (3) 
truncation when casting between types. Focus on operations that 
compute buffer sizes or array indices from user-controlled inputs."
```

**Memory safety scan:**
```text
"Examine this C++ code for memory safety issues: (1) use-after-free, 
(2) heap buffer overflow, (3) stack buffer overflow, (4) null pointer 
dereference. Focus on pointer arithmetic, allocation/deallocation 
patterns, and bounds checking."
```

**Resource management scan:**
```text
"Analyze this C++ code for resource management issues: (1) memory 
leaks in error paths, (2) file descriptor leaks, (3) unbounded 
resource allocation from user input. Focus on RAII coverage and 
exception safety."
```

### 3.3 Cross-Module Analysis

After per-module scans, perform cross-module analysis:

1. Trace data flow from external inputs (Python API) to internal computation
2. Identify where validation is assumed but not enforced
3. Check for TOCTOU races in shared state
4. Verify error propagation across module boundaries

## Phase 4: Evidence Verification

### 4.1 For Each Candidate Vulnerability

1. **Source Evidence**: Verify the exact file path and line number exist in the codebase
2. **Trigger Path**: Confirm the vulnerability is reachable from an external input
3. **AI-Generated PoC**: Have the LLM generate a test case or input that triggers the bug
4. **Anti-Hallucination Check**: Run `.agents/skills/anti-hallucination/` checklist:
   - Does the cited source code actually exist at the claimed location?
   - Does the trigger path actually exist in the call graph?
   - Is the claimed behavior verifiable from the source code alone?
   - Could this be a false positive from LLM pattern matching?
   - Is the severity assessment justified by the actual impact?
   - Is the proposed PoC actually triggerable?

### 4.2 Vulnerability Classification

Classify each verified vulnerability:

| Type | Examples | Typical Severity |
|------|---------|-----------------|
| FPE (Floating Point Exception) | Division by zero, modulo by zero | High (DoS) |
| Null Deref | Dereferencing unchecked allocation result | High (DoS) |
| Heap Overflow | OOB write from integer overflow in size calc | Critical (RCE) |
| Stack Overflow | Deep recursion with user-controlled depth | High (DoS) |
| Use-After-Free | Accessing freed memory | Critical (RCE) |
| Type Confusion | Unsafe downcast without type check | Critical (RCE) |
| DoS | Unbounded allocation, infinite loop | Medium |
| Info Leak | Uninitialized memory read | Medium |

### 4.3 Deduplication

Before adding to the final list, check:
- Is this the same root cause as an existing entry? → Merge
- Is this a different trigger for the same bug? → Add as variant
- Is this genuinely independent? → Add as new entry

## Phase 5: Deliverable Generation

### 5.1 vulnerability_list.md

Generate from the template at `templates/vulnerability_list.template.md`. Each entry must include:

1. Vulnerability name (concise, descriptive)
2. Vulnerability type
3. Severity level (self-assessed: High/Medium/Low)
4. Source code path
5. Root cause description
6. The specific LLM prompt that discovered it
7. Why this prompt was chosen
8. Potential business impact

### 5.2 llm_chat_log.json

Generate from the template at `templates/llm_chat_log.template.json`. Must include:
- Metadata (model used, total turns, system prompt)
- Complete chat history (every turn, no edits)
- System prompt must be generic (no version/project hints)

### 5.3 vulnerability_report.md

Generate a detailed engineering report including:
- Executive summary
- Methodology description (SAST + LLM + evidence verification)
- Scan coverage statistics
- Per-vulnerability detailed analysis with:
  - Source code excerpt
  - Trigger path analysis
  - AI-generated PoC
  - Recommended fix
  - Severity justification
- Engineering lessons learned
- Comparison with traditional vulnerability detection methods

## Phase 6: Final Verification

Run `./harness/final_verify.sh`. This checks:

1. **Deliverable existence**: all three files exist in `result/`
2. **Format compliance**: files match required templates
3. **Evidence chain**: every vulnerability has source path, line number, and trigger
4. **Black-box compliance**: no version hints or CVE references in `llm_chat_log.json`
5. **Interaction completeness**: `llm_chat_log.json` has ≥ 5 turns
6. **PoC quality**: at least one vulnerability has an AI-generated test/PoC
7. **No false positives**: anti-hallucination checklist passed for each entry

If verification fails, fix the specific issue and re-run. Keep fixing until it passes.

## Scan Wave Pattern

Organize scanning in waves of 3-5 modules:

| Wave | Modules | Focus |
|------|---------|-------|
| 1 | core/kernels/ (pooling, conv, matmul) | Input validation, arithmetic |
| 2 | core/kernels/ (slice, gather, sparse) | Bounds checking, OOB |
| 3 | core/framework/ | Memory management, type safety |
| 4 | core/platform/, core/util/ | I/O, parsing, resource management |
| 5 | Cross-module analysis | Data flow, API boundary validation |

Each wave:
1. Scan all modules in the wave
2. Verify evidence for each finding
3. Update `vulnerability_list.md` and `llm_chat_log.json`
4. Run anti-hallucination check
5. Only proceed to next wave after verification

## Emergency Escalation

Escalate to the user if:
1. Target code not found or inaccessible
2. 3 consecutive waves produce zero new findings
3. A vulnerability is found that could affect live systems (responsible disclosure)
