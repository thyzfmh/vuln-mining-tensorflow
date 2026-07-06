---
name: shared
description: Shared methodology for AI-driven vulnerability mining.
---

# Shared Methodology

## Core Principles

1. **Black-box standard**: Never reveal project identity, version, or known bugs to the LLM
2. **Evidence over claims**: Every vulnerability claim needs source code evidence
3. **Systematic over random**: Follow scan waves, not random poking
4. **Verify before reporting**: Anti-hallucination checks before adding to deliverables
5. **Complete logging**: Every LLM interaction recorded in llm_chat_log.json

## Vulnerability Classes

| Class | Key Indicators | Typical Severity |
|-------|---------------|-----------------|
| FPE | Division by zero, modulo by zero | High |
| Null Deref | Unchecked pointer after allocation | High |
| Heap Overflow | Integer overflow in size calculation | Critical |
| OOB | Unchecked array index | High |
| UAF | Accessing freed memory | Critical |
| Type Confusion | Unsafe downcast | Critical |
| DoS | Unbounded allocation, infinite loop | Medium |
| Info Leak | Uninitialized memory read | Medium |

## Prompt Engineering (Black-Box)

### Good Prompts (Generic)
- "Analyze this C++ code for potential division-by-zero vulnerabilities"
- "Review this function for missing input validation"
- "Check this code for memory safety issues"

### Bad Prompts (Leak Information)
- "TensorFlow v2.11.0 has a bug in pooling ops"
- "This version has a CVE for integer overflow"
- "Find the division-by-zero bug in the conv2d kernel"

## Data Flow

```
Source Code → SAST Scan → LLM Scan → Evidence Check → Anti-Hallucination → Verified Vulnerability → Deliverable
```

All intermediate results stored in `reports/`. Final deliverables in `result/`.
