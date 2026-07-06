#!/usr/bin/env bash
set -euo pipefail
THRESHOLD="${1:-10}"
mkdir -p reports

python3 - "$THRESHOLD" <<'PYEOF'
import pathlib, re, sys

threshold = float(sys.argv[1])
files = sorted(pathlib.Path("src").rglob("*.rs"))
code_lines = 0
unsafe_hits = 0
details = []

for path in files:
    text = path.read_text()
    local_lines = 0
    local_unsafe = 0
    for line in text.splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("//"):
            continue
        local_lines += 1
        local_unsafe += len(re.findall(r"\bunsafe\b", line))
    code_lines += local_lines
    unsafe_hits += local_unsafe
    if local_unsafe:
        details.append((str(path), local_unsafe))

ratio = (unsafe_hits / code_lines * 100.0) if code_lines else 0.0
report = pathlib.Path("reports/unsafe-report.md")
with report.open("w") as f:
    f.write("# Unsafe Audit\n\n")
    f.write(f"- Rust source files: {len(files)}\n")
    f.write(f"- Non-empty production code lines: {code_lines}\n")
    f.write(f"- unsafe keyword hits: {unsafe_hits}\n")
    f.write(f"- unsafe ratio: {ratio:.2f}%\n")
    f.write(f"- threshold: < {threshold:.2f}%\n\n")
    if details:
        f.write("## Files with unsafe\n\n")
        for path, count in details:
            f.write(f"- `{path}`: {count}\n")

print(f"unsafe ratio: {ratio:.2f}% (threshold < {threshold:.2f}%)")
if ratio >= threshold:
    sys.exit(1)
PYEOF
