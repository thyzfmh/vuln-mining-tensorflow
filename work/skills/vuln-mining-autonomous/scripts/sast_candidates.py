#!/usr/bin/env python3
from __future__ import annotations

import pathlib
import re

ROOT = pathlib.Path.cwd()
CODE = ROOT / "code"
REPORT = ROOT / "reports" / "sast-candidates.md"
EXTS = {".c", ".cc", ".cpp", ".cxx", ".h", ".hh", ".hpp", ".hxx", ".cu"}

PATTERNS = [
    ("division", re.compile(r"/\s*[A-Za-z_][A-Za-z0-9_]*(?!\s*[=])")),
    ("modulo", re.compile(r"%\s*[A-Za-z_][A-Za-z0-9_]*")),
    ("raw allocation", re.compile(r"\b(new|malloc|calloc|realloc)\b")),
    ("unchecked cast", re.compile(r"\b(reinterpret_cast|static_cast)\s*<")),
    ("indexing", re.compile(r"\[[^\]]*[A-Za-z_][A-Za-z0-9_]*[^\]]*\]")),
    ("assert/check crash", re.compile(r"\b(CHECK|DCHECK|assert)\s*\(")),
    ("memcpy/memmove", re.compile(r"\b(memcpy|memmove|memset)\s*\(")),
]


def detect_target() -> pathlib.Path:
    children = [p for p in CODE.iterdir() if p.is_dir() and not p.name.startswith(".")]
    if len(children) == 1:
        return children[0]
    return CODE


def main() -> None:
    target = detect_target()
    candidates: list[tuple[int, str, list[str]]] = []
    for path in target.rglob("*"):
        if not path.is_file() or path.suffix not in EXTS:
            continue
        try:
            text = path.read_text(errors="replace")
        except OSError:
            continue
        hits: list[str] = []
        score = 0
        for name, pattern in PATTERNS:
            count = len(pattern.findall(text))
            if count:
                hits.append(f"{name}: {count}")
                score += count
        if score:
            candidates.append((score, path.relative_to(target).as_posix(), hits))

    candidates.sort(reverse=True)
    REPORT.parent.mkdir(parents=True, exist_ok=True)
    lines = ["# SAST Candidate Extraction", "", "- Target alias: `TARGET_ROOT`", "- These are hints, not accepted vulnerabilities.", "", "## Top Candidates", ""]
    for score, path, hits in candidates[:80]:
        lines.append(f"### `{path}`")
        lines.append(f"- Score: {score}")
        for hit in hits:
            lines.append(f"- {hit}")
        lines.append("")
    REPORT.write_text("\n".join(lines))
    print(f"wrote {REPORT}")


if __name__ == "__main__":
    main()
