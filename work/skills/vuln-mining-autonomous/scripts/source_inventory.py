#!/usr/bin/env python3
from __future__ import annotations

import pathlib

ROOT = pathlib.Path.cwd()
CODE = ROOT / "code"
REPORT = ROOT / "reports" / "source-inventory.md"
MANIFEST = ROOT / "reports" / "source-file-manifest.md"
EXTS = {".c", ".cc", ".cpp", ".cxx", ".h", ".hh", ".hpp", ".hxx", ".cu", ".py"}
DOMAIN_RULES = [
    ("python-api", ("python/", ".py")),
    ("native-kernel-runtime", ("kernel", "runtime", "op", "lite")),
    ("parser-serialization", ("parse", "proto", "reader", "loader", "import", "serialize", "deserialize")),
    ("compiler-graph", ("compiler", "graph", "optimizer", "simplifier", "mlir", "hlo")),
    ("accelerator-backend", ("cuda", "gpu", "opencl", "metal", "delegate", "mkl", "neon", "simd")),
]


def domains_for_path(rel: str) -> list[str]:
    rel_lower = rel.lower()
    return [domain for domain, keywords in DOMAIN_RULES if any(keyword in rel_lower for keyword in keywords)] or ["uncategorized"]


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
    by_domain: dict[str, int] = {}
    for path in files:
        by_ext[path.suffix] = by_ext.get(path.suffix, 0) + 1
        rel_lower = path.relative_to(target).as_posix().lower()
        for domain, keywords in DOMAIN_RULES:
            if any(keyword in rel_lower for keyword in keywords):
                by_domain[domain] = by_domain.get(domain, 0) + 1

    dirs: dict[str, int] = {}
    for path in files:
        parts = path.relative_to(target).parts
        key = "/".join(parts[:3]) if len(parts) >= 3 else "/".join(parts[:-1]) or "."
        dirs[key] = dirs.get(key, 0) + 1

    REPORT.parent.mkdir(parents=True, exist_ok=True)
    lines = ["# Source Inventory", "", "- Target alias: `TARGET_ROOT`", f"- Source files: {len(files)}", "", "## File Counts", ""]
    for ext, count in sorted(by_ext.items()):
        lines.append(f"- `{ext}`: {count}")
    lines.extend(["", "## Domain Coverage Targets", ""])
    for domain, count in sorted(by_domain.items(), key=lambda item: item[1], reverse=True):
        lines.append(f"- `{domain}`: {count} files")
    lines.extend(["", "## High-Volume Directories", ""])
    for directory, count in sorted(dirs.items(), key=lambda item: item[1], reverse=True)[:30]:
        lines.append(f"- `{directory}`: {count} files")
    lines.extend(["", "## Suggested First Waves", ""])
    for directory, _ in sorted(dirs.items(), key=lambda item: item[1], reverse=True)[:8]:
        lines.append(f"- `{directory}`: input validation, arithmetic safety, memory safety")
    REPORT.write_text("\n".join(lines) + "\n")

    manifest_lines = [
        "# Source File Manifest",
        "",
        "- Target alias: `TARGET_ROOT`",
        f"- Source files: {len(files)}",
        "",
        "| Path | Ext | Bytes | Domains |",
        "|---|---|---:|---|",
    ]
    for path in sorted(files, key=lambda p: p.relative_to(target).as_posix()):
        rel = path.relative_to(target).as_posix()
        try:
            size = path.stat().st_size
        except OSError:
            size = 0
        manifest_lines.append(f"| `TARGET_ROOT/{rel}` | `{path.suffix}` | {size} | {', '.join(domains_for_path(rel))} |")
    MANIFEST.write_text("\n".join(manifest_lines) + "\n")
    print(f"wrote {REPORT}")
    print(f"wrote {MANIFEST}")


if __name__ == "__main__":
    main()
