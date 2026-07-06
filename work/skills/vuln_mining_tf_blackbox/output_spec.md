# Output Specification

All deliverables go into `submission/`.

## submission/vulnerability_list.md

Must include for each vulnerability:

1. **Vulnerability name** — concise, descriptive (e.g., "算子非正数输入引发的浮点异常")
2. **Type** — FPE / Null Deref / Heap Overflow / OOB / UAF / Type Confusion / DoS / Info Leak / Integer Overflow
3. **Severity** — High / Medium / Low (self-assessed with justification)
4. **Source path** — exact .cc/.h file path
5. **Reasoning chain** — step-by-step logical derivation from source code
6. **AI-generated test case** — the Python code that triggers the vulnerability
7. **Verification result** — runtime output (crash signal, exception trace, or error message)
8. **Evidence chain** — source code excerpt → trigger path → runtime result

Template:

```markdown
## 漏洞 #N：[名称]

- **漏洞类型**：[type]
- **严重级别**：[severity]
- **问题源码路径**：[path]

### 成因简述
[reasoning chain]

### AI生成的测试用例
[python test code]

### 验证结果
[runtime output]

### 证据链
[source code] → [trigger path] → [runtime crash/exception]

### 与LLM交互中哪句提示词发现了bug
[prompt excerpt]

### 为什么选择此提示词
[explanation]

### 潜在业务危害
[impact assessment]
```

## submission/llm_chat_log.json

Must include:

1. **metadata** — model name, total turns, system prompt
2. **chat_history** — array of turns, each with:
   - `turn` (integer)
   - `role` ("user" or "assistant")
   - `content` (full text, no edits)

The chat log must demonstrate:
- Multi-step reasoning (not single-shot)
- Hypothesis generation (proposing then testing)
- Test generation (writing Python code)
- Verification step (running tests and interpreting results)

System prompt must be generic (no project name, version, or known bugs).

```json
{
  "metadata": {
    "llm_model_used": "[model]",
    "total_turns": 0,
    "system_prompt": "You are an expert in C++ source code auditing and security vulnerability analysis."
  },
  "chat_history": []
}
```

## submission/vulnerability_report.md

Must include:

1. **Black-box methodology** — description of the approach used
2. **Pipeline description** — how the discovery process worked
3. **Verification approach** — how findings were validated
4. **Reproducibility** — how to reproduce the results

Additional sections:
- Executive summary
- Per-vulnerability detailed analysis
- Engineering lessons learned
- Comparison with traditional methods (SAST/SCA)

Must contain the black-box constraint declaration:

```markdown
All results are derived purely from code semantic analysis.
No CVE or vulnerability database is used.
All test cases are AI generated.
All vulnerabilities are validated through runtime execution.
```

## submission/verify/run_test.py

Must:

1. Import the target project's Python API (e.g., `import tensorflow as tf`)
2. Define test functions, one per vulnerability hypothesis
3. Execute each test and capture:
   - Exceptions (try/except)
   - Crash signals (SIGFPE, SIGSEGV via subprocess)
   - Error messages
4. Output reproducible results to stdout
5. Return exit code 0 if all tests run (even if vulnerabilities found)
6. Print a summary of findings

Template:

```python
#!/usr/bin/env python3
"""AI-generated vulnerability verification script."""

import sys
import traceback

results = []

def run_test(name, fn):
    """Run a test case and record the result."""
    try:
        fn()
        results.append((name, "NO_CRASH", ""))
    except Exception as e:
        results.append((name, "CRASH", str(e)))
        traceback.print_exc()

def main():
    # Test cases will be added by the AI agent
    print("Running vulnerability verification...")
    # run_test("test_case_1", test_case_1)

    print("\n=== Results ===")
    for name, status, detail in results:
        print(f"  {name}: {status}")
        if detail:
            print(f"    {detail}")

    crashed = sum(1 for _, s, _ in results if s == "CRASH")
    print(f"\nTotal: {len(results)} tests, {crashed} crashes detected")

if __name__ == "__main__":
    main()
```
