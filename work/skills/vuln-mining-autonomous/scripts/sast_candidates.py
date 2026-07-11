#!/usr/bin/env python3
from __future__ import annotations

import pathlib
import re
from dataclasses import dataclass

try:
    from platform_assets import discover_target_root, resolve_work_root, output_path
except ImportError:  # pragma: no cover - direct execution fallback
    import sys
    sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
    from platform_assets import discover_target_root, resolve_work_root, output_path

EXTS = {".c", ".cc", ".cpp", ".cxx", ".h", ".hh", ".hpp", ".hxx", ".cu", ".py"}
DOMAIN_RULES = [
    ("python-api", ("python/", ".py")),
    ("native-kernel-runtime", ("kernel", "runtime", "op", "lite")),
    ("parser-serialization", ("parse", "proto", "reader", "loader", "import", "serialize", "deserialize")),
    ("compiler-graph", ("compiler", "graph", "optimizer", "simplifier", "mlir", "hlo")),
    ("accelerator-backend", ("cuda", "gpu", "opencl", "metal", "delegate", "mkl", "neon", "simd")),
]


@dataclass(frozen=True)
class Pattern:
    name: str
    category: str
    weight: int
    regex: re.Pattern[str]
    prompt_focus: str


PATTERNS = [
    Pattern(
        "division by variable",
        "arithmetic",
        8,
        re.compile(r"(?<!/)/(?![/*])\s*[A-Za-z_][A-Za-z0-9_]*(?!\s*[=])"),
        "zero and sign checks on denominator",
    ),
    Pattern(
        "modulo by variable",
        "arithmetic",
        8,
        re.compile(r"%\s*[A-Za-z_][A-Za-z0-9_]*"),
        "zero checks on modulo divisor",
    ),
    Pattern(
        "size multiplication/addition",
        "integer-overflow",
        7,
        re.compile(r"\b(size|len|count|num|dim|bytes|width|height|stride|offset)\w*\b[^;\n]{0,80}[+*][^;\n]{0,80}\b(size|len|count|num|dim|bytes|width|height|stride|offset)\w*\b", re.I),
        "overflow before allocation or indexing",
    ),
    Pattern(
        "raw allocation",
        "memory",
        7,
        re.compile(r"\b(new|malloc|calloc|realloc)\b"),
        "allocation size derived from untrusted or shape values",
    ),
    Pattern(
        "unchecked cast",
        "type-confusion",
        6,
        re.compile(r"\b(reinterpret_cast|static_cast|const_cast)\s*<"),
        "runtime type, alignment, and lifetime checks before cast",
    ),
    Pattern(
        "array/vector indexing",
        "bounds",
        6,
        re.compile(r"\[[^\]]*[A-Za-z_][A-Za-z0-9_]*[^\]]*\]"),
        "index range checks and negative-to-unsigned conversions",
    ),
    Pattern(
        "crash assertion",
        "dos",
        6,
        re.compile(r"\b(CHECK|DCHECK|assert|abort|LOG\s*\(\s*FATAL\s*\))\s*\("),
        "attacker-controlled path to process abort",
    ),
    Pattern(
        "memory copy/fill",
        "memory",
        9,
        re.compile(r"\b(memcpy|memmove|memset|strcpy|strncpy|std::copy)\s*\("),
        "source/destination size relationship",
    ),
    Pattern(
        "pointer arithmetic",
        "memory",
        5,
        re.compile(r"\b[A-Za-z_][A-Za-z0-9_]*\s*[+-]\s*(size|len|count|num|dim|bytes|offset|index)\w*\b", re.I),
        "bounds after pointer offset calculation",
    ),
    Pattern(
        "external input parser",
        "source",
        5,
        re.compile(r"\b(Parse|Decode|Deserialize|Read|Load|FromString|FromProto|FromJson)\b"),
        "malformed or adversarial input reaching sinks",
    ),
    Pattern(
        "shape/index vocabulary",
        "invariant",
        4,
        re.compile(r"\b(shape|rank|axis|dim|stride|padding|window|offset|index|slice|size|num_elements)\b", re.I),
        "shape/rank/axis invariant enforcement",
    ),
    Pattern(
        "native boundary",
        "boundary",
        5,
        re.compile(r"\b(extern\s+\"C\"|PyObject|JNIEnv|dlopen|dlsym)\b"),
        "cross-language ownership and input validation",
    ),
    Pattern(
        "python dynamic boundary",
        "python",
        5,
        re.compile(r"\b(eval|exec|__import__|getattr|setattr|ctypes|pywrap)\b"),
        "Python-layer input validation before dynamic or native dispatch",
    ),
]


def main() -> None:
    target = discover_target_root()
    work_root = resolve_work_root()
    REPORT = output_path(work_root, "reports", "sast-candidates.md")

    candidates: list[tuple[int, str, list[str], list[str], list[str], list[str]]] = []
    domain_counts: dict[str, int] = {}
    for path in target.rglob("*"):
        if not path.is_file() or path.suffix not in EXTS:
            continue
        rel = path.relative_to(target).as_posix()
        rel_lower = rel.lower()
        domains = [domain for domain, keywords in DOMAIN_RULES if any(keyword in rel_lower for keyword in keywords)]
        try:
            text = path.read_text(errors="replace")
        except OSError:
            continue
        hits: list[str] = []
        examples: list[str] = []
        prompt_focus: set[str] = set()
        score = 0
        for pattern in PATTERNS:
            count = len(pattern.regex.findall(text))
            if count:
                bounded = min(count, 30)
                hits.append(f"{pattern.category} / {pattern.name}: {count} (+{bounded * pattern.weight})")
                score += bounded * pattern.weight
                prompt_focus.add(pattern.prompt_focus)
                line_count = 0
                for number, line in enumerate(text.splitlines(), start=1):
                    if pattern.regex.search(line):
                        compact = " ".join(line.strip().split())
                        if compact:
                            examples.append(f"L{number} [{pattern.category}]: {compact[:180]}")
                        line_count += 1
                    if line_count >= 2:
                        break
        if score:
            for domain in domains or ["uncategorized"]:
                domain_counts[domain] = domain_counts.get(domain, 0) + 1
            candidates.append((score, rel, hits, examples[:10], sorted(prompt_focus), domains or ["uncategorized"]))

    candidates.sort(key=lambda item: (-item[0], item[1]))
    lines = [
        "# SAST Candidate Extraction",
        "",
        "- Target alias: `TARGET_ROOT`",
        "- These are hints, not accepted vulnerabilities.",
        "- Ranking uses sink density, source adjacency, and invariant vocabulary.",
        "",
        "## Domain Coverage Summary",
        "",
    ]
    for domain, count in sorted(domain_counts.items(), key=lambda item: item[1], reverse=True):
        lines.append(f"- `{domain}`: {count} candidate files")
    lines.extend([
        "",
        "## Top Candidates",
        "",
    ])
    for score, path, hits, examples, focuses, domains in candidates:
        lines.append(f"### `{path}`")
        lines.append(f"- Score: {score}")
        lines.append(f"- Domain: {', '.join(domains)}")
        if focuses:
            lines.append("- Prompt focus:")
            for focus in focuses[:8]:
                lines.append(f"  - {focus}")
        lines.append("- Pattern hits:")
        for hit in hits:
            lines.append(f"- {hit}")
        if examples:
            lines.append("- Evidence lines:")
            for example in examples:
                lines.append(f"  - `{example}`")
        lines.append("")
    REPORT.write_text("\n".join(lines))
    print(f"wrote {REPORT}")


if __name__ == "__main__":
    main()
