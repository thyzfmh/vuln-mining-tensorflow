#!/usr/bin/env python3
from __future__ import annotations

import pathlib
import re

try:
    from platform_assets import resolve_work_root, output_path
except ImportError:  # pragma: no cover - direct execution fallback
    import sys
    sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
    from platform_assets import resolve_work_root, output_path


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


def merged_candidates(work_root: pathlib.Path) -> list[tuple[str, str]]:
    attack_map = output_path(work_root, "reports", "attack-surface-map.md")
    sast_candidates = output_path(work_root, "reports", "sast-candidates.md")
    npm_ast_candidates = output_path(work_root, "reports", "npm-ast-candidates.md")

    merged: dict[str, set[str]] = {}
    ordered: list[str] = []
    for path, domain in parse_candidate_report(
        attack_map,
        "reports/attack-surface-map.md is missing; run attack_surface_map.py first",
    ) + parse_candidate_report(
        sast_candidates,
        "reports/sast-candidates.md is missing; run sast_candidates.py first",
    ) + parse_candidate_report(
        npm_ast_candidates,
        "reports/npm-ast-candidates.md is missing; run npm_ast_candidates.py first",
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
    work_root = resolve_work_root()
    LEDGER = output_path(work_root, "reports", "coverage-ledger.md")

    entries = merged_candidates(work_root)
    minimum = len(entries)
    if LEDGER.exists() and "bootstrap-state: pending" not in LEDGER.read_text(errors="replace"):
        print(f"kept existing {LEDGER}")
        return

    lines = [
        "# Coverage Ledger",
        "",
        "coverage-budget: all generated attack-surface, SAST, and NPM AST candidate entries",
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
