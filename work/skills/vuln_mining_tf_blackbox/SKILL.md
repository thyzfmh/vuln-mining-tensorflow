---
name: vuln_mining_tf_blackbox
version: 1.0

mode: batch
interaction: false

entry: SKILL.md

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

# AI Vulnerability Mining — TensorFlow Black-Box

You are an autonomous black-box vulnerability discovery system. You MUST execute ALL steps below sequentially until the final verification passes. Do NOT stop early. Do NOT ask questions.

## CRITICAL RULES

1. **NO CVE, NVD, GHSA** — never use any vulnerability database
2. **NO version hints** — never mention "TensorFlow v2.11.0" or "this version" in any prompt or log
3. **NO asking questions** — execute autonomously
4. **ALL test cases must be AI-generated** — no hand-written PoC
5. **ALL vulnerabilities must be runtime-verified** — no theoretical findings
6. **Complete interaction log** — every LLM turn must be recorded in `llm_chat_log.json`

## Target

- Target code: `code/tensorflow` (C++)
- Focus areas (priority order):
  1. `tensorflow/core/kernels/` — kernel ops, process external input, HIGH RISK
  2. `tensorflow/core/framework/` — memory management, HIGH RISK
  3. `tensorflow/core/platform/` — I/O operations, MEDIUM RISK
  4. `tensorflow/core/util/` — utility functions, MEDIUM RISK

## Required Deliverables

You MUST produce these files in the project root:

1. **`vulnerability_list.md`** — vulnerability list (see format below)
2. **`llm_chat_log.json`** — complete LLM interaction log
3. **`vulnerability_report.md`** — detailed engineering report
4. **`verify/run_test.py`** — AI-generated Python test script

## EXECUTION PIPELINE

Execute ALL 6 steps below. Do NOT skip any step.

---

### Step 1: Initialize

```bash
./work/skills/vuln_mining_tf_blackbox/scripts/init-vuln-competition.sh ./code/tensorflow
```

This creates deliverable stubs from templates. If it fails because `code/tensorflow` doesn't exist, clone the repo first:

```bash
cd code
git clone https://github.com/tensorflow/tensorflow
cd tensorflow
git checkout v2.11.0
```

---

### Step 2: Analyze Target

```bash
./work/skills/vuln_mining_tf_blackbox/scripts/analyze_target.sh ./code/tensorflow
```

Read `reports/source-inventory.md` to understand the codebase structure and identify high-risk files.

Then READ the actual source files. For each high-risk file:
- Read the full source code
- Identify functions that process external input (shapes, dimensions, strides, sizes)
- Note missing validation (division by zero, null checks, bounds checks, integer overflow)

Focus on these vulnerability patterns:
1. **Division without zero guard** — divisor from user input (shape dims, strides, ksize)
2. **Unchecked allocation** — pointer used without null check after `new`/`Allocate`
3. **Integer overflow in size calc** — `int32` for buffer size then cast to `size_t`
4. **Missing bounds check** — array access with user-controlled index
5. **Type confusion** — `static_cast`/`reinterpret_cast` without validation
6. **Uncapped resource** — allocation size from user input without upper bound

---

### Step 3: Generate Vulnerability Hypotheses

For EACH high-risk function found in Step 2, generate a vulnerability hypothesis:

- **What**: which vulnerability type (FPE, null deref, overflow, OOB, etc.)
- **Where**: exact file path and function name
- **Why**: what validation is missing
- **How to trigger**: what input would reach the vulnerable code path

Record each hypothesis. You need at least 1 valid vulnerability for submission.

---

### Step 4: Generate AI Test Cases

For EACH hypothesis from Step 3, write a Python test function in `verify/run_test.py`:

```python
def test_case_N():
    import tensorflow as tf
    # Construct input that triggers the vulnerability
    # Let the exception propagate — do NOT catch it here
```

Add each test to the runner:

```python
run_test("test_case_N", test_case_N)
```

The test must:
- Use `import tensorflow as tf` to reach the vulnerable C++ code via Python API
- Construct specific input that triggers the vulnerability
- NOT catch the exception (let `run_test()` handle it)

---

### Step 5: Runtime Verification

```bash
pip install tensorflow==2.11.0
python3 verify/run_test.py
```

Capture the output. Only vulnerabilities that produce a CRASH, EXCEPTION, or SIGNAL are accepted.

If no crash is detected for a hypothesis, that hypothesis is REJECTED — do NOT include it in deliverables.

If no test cases crash, try different inputs or examine more source files. You MUST find at least 1 runtime-verified vulnerability.

---

### Step 6: Generate Deliverables

#### 6.1 Write `vulnerability_list.md`

Use this EXACT format for each vulnerability:

```markdown
## 漏洞 #1：[简短直观的漏洞名称]

- **漏洞类型**：[FPE / Null Deref / Heap Overflow / OOB / UAF / Type Confusion / DoS / Info Leak / Integer Overflow]
- **严重级别**：[高危 (High) / 中危 (Medium) / 低危 (Low)]
- **问题源码路径**：[exact .cc file path]

### 成因简述
[Reasoning chain from source code semantics]

### AI生成的测试用例
[The Python test code from Step 4]

### 验证结果
[Runtime output from Step 5 — crash signal, exception trace, etc.]

### 与LLM交互中哪句提示词发现了bug
[The exact prompt that led to this finding]

### 为什么选择此提示词
[Why this prompt was effective]

### 潜在业务危害
[Business impact if exploited]
```

#### 6.2 Write `llm_chat_log.json`

```json
{
  "metadata": {
    "llm_model_used": "[your model name]",
    "total_turns": [number],
    "system_prompt": "You are an expert in C++ source code auditing and security vulnerability analysis."
  },
  "chat_history": [
    {"turn": 1, "role": "user", "content": "[prompt]"},
    {"turn": 1, "role": "assistant", "content": "[response]"}
  ]
}
```

IMPORTANT: The system prompt and user prompts must NOT contain "TensorFlow", "v2.11.0", or any CVE reference.

#### 6.3 Write `vulnerability_report.md`

Must include:
1. 概要 (summary table)
2. 黑盒方法论 (black-box methodology description)
3. Pipeline 描述 (5-step pipeline description)
4. 每个漏洞的详细分析 (per-vulnerability detailed analysis)
5. 验证方法 (verification approach — runtime execution)
6. 可复现性 (reproducibility instructions)
7. **Black-box declaration** (MUST include this exact text):

```
All results are derived purely from code semantic analysis.
No CVE or vulnerability database is used.
All test cases are AI generated.
All vulnerabilities are validated through runtime execution.
```

#### 6.4 Write `verify/run_test.py`

Must already exist from Step 4. Verify it contains all test functions and runs correctly.

---

### Step 7: Final Verification

```bash
./work/skills/vuln_mining_tf_blackbox/scripts/final_verify.sh
```

This checks:
- All 4 deliverable files exist
- `vulnerability_list.md` has at least 1 vulnerability
- `llm_chat_log.json` is valid JSON with ≥ 5 turns
- `vulnerability_report.md` has required sections
- `verify/run_test.py` is valid Python
- No black-box violations (version hints, CVE references)

**If verification fails**, fix the specific error and re-run. Keep fixing until it passes.

---

### Step 8: Write Output

**IMPORTANT: You MUST write the execution result to `/result/output.md`.**

Append to `result/output.md`:

```markdown
## Execution Result

- Status: COMPLETED
- Vulnerabilities found: [N]
- Runtime verified: [N]
- Final verification: PASSED
- Date: [current date]

### Deliverables
- vulnerability_list.md: [N] vulnerabilities
- llm_chat_log.json: [N] turns
- vulnerability_report.md: Complete
- verify/run_test.py: [N] test cases
```

This is REQUIRED by the competition platform to confirm the work executed successfully.

---

## Completion Criteria

You are DONE when ALL of the following are true:

1. ✅ `final_verify.sh` passes (exit code 0)
2. ✅ `vulnerability_list.md` has ≥ 1 runtime-verified vulnerability
3. ✅ `llm_chat_log.json` has ≥ 5 turns
4. ✅ `result/output.md` has been updated with execution result
5. ✅ No black-box violations in any deliverable
