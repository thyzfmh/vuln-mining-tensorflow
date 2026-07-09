#!/usr/bin/env python3
from __future__ import annotations

import pathlib
import re

ROOT = pathlib.Path.cwd()
CODE = ROOT / "code"
REPORT = ROOT / "reports" / "attack-surface-map.md"
EXTS = {".c", ".cc", ".cpp", ".cxx", ".h", ".hh", ".hpp", ".hxx", ".cu", ".py"}

PATH_KEYWORDS = {
    "parse": 9,
    "decode": 9,
    "deserialize": 9,
    "serialize": 7,
    "reader": 8,
    "read": 6,
    "load": 6,
    "import": 6,
    "image": 6,
    "audio": 6,
    "video": 6,
    "csv": 6,
    "json": 6,
    "proto": 6,
    "tensor": 5,
    "array": 5,
    "shape": 8,
    "slice": 8,
    "gather": 8,
    "scatter": 8,
    "sparse": 7,
    "quant": 7,
    "kernel": 5,
    "op": 4,
    "runtime": 4,
    "allocator": 8,
    "memory": 8,
    "buffer": 8,
    "cuda": 5,
    "gpu": 5,
    "cpu": 3,
}

TEXT_PATTERNS = [
    ("external parser/loader", 8, re.compile(r"\b(Parse|Decode|Deserialize|Read|Load|FromString|FromProto|FromJson)\b")),
    ("shape/size/index parameter", 6, re.compile(r"\b(shape|rank|axis|dim|size|len|count|stride|offset|index|padding|window|limit)\b", re.I)),
    ("memory sink", 9, re.compile(r"\b(memcpy|memmove|memset|strcpy|strncpy|std::copy|malloc|calloc|realloc|new)\b")),
    ("crash/assert sink", 7, re.compile(r"\b(CHECK|DCHECK|assert|abort|LOG\s*\(\s*FATAL\s*\))\b")),
    ("native boundary", 6, re.compile(r"\b(extern\s+\"C\"|PyObject|JNIEnv|dlopen|dlsym)\b")),
    ("cast/pointer boundary", 5, re.compile(r"\b(reinterpret_cast|static_cast|const_cast|void\s*\*|char\s*\*)\b")),
]
DOMAIN_RULES = [
    ("python-api", ("python/", ".py")),
    ("native-kernel-runtime", ("kernel", "runtime", "op", "lite")),
    ("parser-serialization", ("parse", "proto", "reader", "loader", "import", "serialize", "deserialize")),
    ("compiler-graph", ("compiler", "graph", "optimizer", "simplifier", "mlir", "hlo")),
    ("accelerator-backend", ("cuda", "gpu", "opencl", "metal", "delegate", "mkl", "neon", "simd")),
]


def detect_target() -> pathlib.Path:
    if not CODE.exists():
        raise SystemExit("code/ directory is missing")
    children = [p for p in CODE.iterdir() if p.is_dir() and not p.name.startswith(".")]
    if len(children) == 1:
        return children[0]
    return CODE


def matching_lines(text: str, pattern: re.Pattern[str], limit: int = 3) -> list[str]:
    lines: list[str] = []
    for number, line in enumerate(text.splitlines(), start=1):
        if pattern.search(line):
            compact = " ".join(line.strip().split())
            if compact:
                lines.append(f"L{number}: {compact[:180]}")
            if len(lines) >= limit:
                break
    return lines


def main() -> None:
    target = detect_target()
    entries: list[tuple[int, str, list[str], list[str], list[str]]] = []
    domain_counts: dict[str, int] = {}
    for path in target.rglob("*"):
        if not path.is_file() or path.suffix not in EXTS:
            continue
        rel = path.relative_to(target).as_posix()
        rel_lower = rel.lower()
        score = 0
        reasons: list[str] = []
        examples: list[str] = []
        domains = [domain for domain, keywords in DOMAIN_RULES if any(keyword in rel_lower for keyword in keywords)]

        for keyword, weight in PATH_KEYWORDS.items():
            if keyword in rel_lower:
                score += weight
                reasons.append(f"path keyword `{keyword}` (+{weight})")

        try:
            text = path.read_text(errors="replace")
        except OSError:
            continue

        for name, weight, pattern in TEXT_PATTERNS:
            count = len(pattern.findall(text))
            if count:
                score += min(count, 20) * weight
                reasons.append(f"{name}: {count}")
                examples.extend(matching_lines(text, pattern, 2))

        if score:
            for domain in domains or ["uncategorized"]:
                domain_counts[domain] = domain_counts.get(domain, 0) + 1
            entries.append((score, rel, reasons[:10], examples[:8], domains or ["uncategorized"]))

    entries.sort(key=lambda item: (-item[0], item[1]))
    REPORT.parent.mkdir(parents=True, exist_ok=True)
    lines = [
        "# Attack Surface Map",
        "",
        "- Target alias: `TARGET_ROOT`",
        "- Purpose: rank source slices before LLM review.",
        "- Method: path keywords plus source/sink/sanitizer-adjacent text patterns.",
        "",
        "## Domain Coverage Summary",
        "",
    ]
    for domain, count in sorted(domain_counts.items(), key=lambda item: item[1], reverse=True):
        lines.append(f"- `{domain}`: {count} suspicious files")
    lines.extend([
        "",
        "## Top Suspicious Points",
        "",
    ])
    for score, rel, reasons, examples, domains in entries:
        lines.append(f"### `{rel}`")
        lines.append(f"- Score: {score}")
        lines.append(f"- Domain: {', '.join(domains)}")
        for reason in reasons:
            lines.append(f"- {reason}")
        if examples:
            lines.append("- Evidence lines:")
            for example in examples:
                lines.append(f"  - `{example}`")
        lines.append("")
    REPORT.write_text("\n".join(lines) + "\n")
    print(f"wrote {REPORT}")


if __name__ == "__main__":
    main()
