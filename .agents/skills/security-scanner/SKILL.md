---
name: security-scanner
description: SAST static pattern scanner for C++ vulnerability patterns.
---

# Security Scanner

Performs regex-based pattern matching for common vulnerability patterns in C++ code.

## Patterns Scanned

| Pattern | Indicator | Vulnerability Type |
|---------|-----------|-------------------|
| Division without zero-check | Division near shape/size calculations | FPE / DoS |
| Unchecked pointer deref | Pointer access without null check | Null deref / DoS |
| Integer overflow | int32 used for size then cast to size_t | Heap overflow |
| Missing error handling | Returned Status not checked | Logic error |
| Unsafe cast | static_cast/reinterpret_cast without validation | Type confusion |
| Uncapped allocation | Allocate/new with user-controlled size | DoS / OOM |

## Usage

```bash
./harness/scan_sast.sh code/tensorflow
```

Output: `reports/sast-scan-report.md`
