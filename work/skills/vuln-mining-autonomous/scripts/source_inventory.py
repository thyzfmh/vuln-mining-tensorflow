#!/usr/bin/env python3
from __future__ import annotations

import pathlib

ROOT = pathlib.Path.cwd()
CODE = ROOT / "code"
REPORT = ROOT / "reports" / "source-inventory.md"
EXTS = {".c", ".cc", ".cpp", ".cxx", ".h", ".hh", ".hpp", ".hxx", ".cu", ".py"}


def detect_target() -> pathlib.Path:
    if not CODE.exists():
        raise SystemExit("code/ directory is missing")
    children = [p for p in CODE.iterdir() if p.is_dir() and not p.name.startswith(".")]
    if len(children) == 1:
        return children[0]
    if any(p.suffix in EXTS for p in CODE.rglob("*") if p.is_file()):
        return CODE
    raise SystemExit("could not detect target source tree under code/")


def main() -> None:
    target = detect_target()
    files = [p for p in target.rglob("*") if p.is_file() and p.suffix in EXTS]
    by_ext: dict[str, int] = {}
    for path in files:
        by_ext[path.suffix] = by_ext.get(path.suffix, 0) + 1

    dirs: dict[str, int] = {}
    for path in files:
        parts = path.relative_to(target).parts
        key = "/".join(parts[:3]) if len(parts) >= 3 else "/".join(parts[:-1]) or "."
        dirs[key] = dirs.get(key, 0) + 1

    REPORT.parent.mkdir(parents=True, exist_ok=True)
    lines = ["# Source Inventory", "", "- Target alias: `TARGET_ROOT`", f"- Source files: {len(files)}", "", "## File Counts", ""]
    for ext, count in sorted(by_ext.items()):
        lines.append(f"- `{ext}`: {count}")
    lines.extend(["", "## High-Volume Directories", ""])
    for directory, count in sorted(dirs.items(), key=lambda item: item[1], reverse=True)[:30]:
        lines.append(f"- `{directory}`: {count} files")
    lines.extend(["", "## Suggested First Waves", ""])
    for directory, _ in sorted(dirs.items(), key=lambda item: item[1], reverse=True)[:8]:
        lines.append(f"- `{directory}`: input validation, arithmetic safety, memory safety")
    REPORT.write_text("\n".join(lines) + "\n")
    print(f"wrote {REPORT}")


if __name__ == "__main__":
    main()
