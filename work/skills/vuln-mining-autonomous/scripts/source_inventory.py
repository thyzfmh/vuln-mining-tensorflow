#!/usr/bin/env python3
from __future__ import annotations

import pathlib

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


def domains_for_path(rel: str) -> list[str]:
    rel_lower = rel.lower()
    return [domain for domain, keywords in DOMAIN_RULES if any(keyword in rel_lower for keyword in keywords)] or ["uncategorized"]


def main() -> None:
    target = discover_target_root()
    work_root = resolve_work_root()
    REPORT = output_path(work_root, "reports", "source-inventory.md")
    MANIFEST = output_path(work_root, "reports", "source-file-manifest.md")

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
