#!/usr/bin/env python3
from __future__ import annotations

import pathlib
import re

ROOT = pathlib.Path.cwd()
ATTACK_MAP = ROOT / "reports" / "attack-surface-map.md"
SAST_CANDIDATES = ROOT / "reports" / "sast-candidates.md"
LEDGER = ROOT / "reports" / "coverage-ledger.md"


def parse_candidate_report(path: pathlib.Path, missing_message: str) -> list[tuple[str, str]]:
    if not path.is_file():
        raise SystemExit(missing_message)

    entries: list[tuple[str, str]] = []
    current_path: str | None = None
    current_domain = ""
    for line in path.read_text(errors="replace").splitlines():
        heading = re.match(r"^### `(.+)`", line)
        if heading:
            if current_path:
                entries.append((current_path, current_domain))
            current_path = heading.group(1)
            current_domain = ""
            continue
        if current_path and line.startswith("- Domain:"):
            current_domain = line.split(":", 1)[1].strip()
    if current_path:
        entries.append((current_path, current_domain))
    return entries


def merged_candidates() -> list[tuple[str, str]]:
    merged: dict[str, set[str]] = {}
    ordered: list[str] = []
    for path, domain in parse_candidate_report(
        ATTACK_MAP,
        "reports/attack-surface-map.md is missing; run attack_surface_map.py first",
    ) + parse_candidate_report(
        SAST_CANDIDATES,
        "reports/sast-candidates.md is missing; run sast_candidates.py first",
    ):
        if path not in merged:
            merged[path] = set()
            ordered.append(path)
        for item in domain.split(","):
            item = item.strip()
            if item:
                merged[path].add(item)
    return [(path, ", ".join(sorted(merged[path])) or "uncategorized") for path in ordered]


def main() -> None:
    entries = merged_candidates()
    minimum = len(entries)
    LEDGER.parent.mkdir(parents=True, exist_ok=True)
    if LEDGER.exists():
        print(f"kept existing {LEDGER}")
        return

    lines = [
        "# Coverage Ledger",
        "",
        "coverage-budget: all generated attack-surface and SAST candidate entries",
        f"minimum-reviewed-targets: {minimum}",
        "candidate-space-exhausted: no",
        "",
        "| Target | Domain | Source | Sink | Sanitizer status | Status | Evidence |",
        "|---|---|---|---|---|---|---|",
    ]
    for path, domain in entries:
        lines.append(f"| `TARGET_ROOT/{path}` | {domain or 'uncategorized'} |  |  |  |  |  |")
    LEDGER.write_text("\n".join(lines) + "\n")
    print(f"wrote {LEDGER}")


if __name__ == "__main__":
    main()
