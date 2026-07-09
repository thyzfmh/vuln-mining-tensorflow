#!/usr/bin/env python3
"""AI-generated vulnerability verification script.

The OpenCode run must replace this template with concrete AI-generated tests.
"""

from __future__ import annotations

import pathlib
import subprocess
import sys
import traceback

REPORT = pathlib.Path("reports/verification-output.txt")
RESULTS: list[tuple[str, str, str]] = []


def run_test(name, fn):
    try:
        fn()
        RESULTS.append((name, "NO_CRASH", ""))
    except Exception as exc:
        RESULTS.append((name, "EXCEPTION", repr(exc)))
        traceback.print_exc()


def run_subprocess_test(name, code):
    proc = subprocess.run(
        [sys.executable, "-c", code],
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=60,
    )
    detail = f"returncode={proc.returncode}\nstdout={proc.stdout}\nstderr={proc.stderr}"
    if proc.returncode == 0:
        RESULTS.append((name, "NO_CRASH", detail))
    elif proc.returncode < 0:
        RESULTS.append((name, "SIGNAL", detail))
    else:
        RESULTS.append((name, "EXCEPTION", detail))


def main():
    REPORT.parent.mkdir(parents=True, exist_ok=True)
    # AI-generated test cases are appended here during the run.

    lines = ["# Verification Output", ""]
    for name, status, detail in RESULTS:
        marker = "VERIFIED" if status in {"EXCEPTION", "SIGNAL", "CRASH"} else "REJECTED"
        lines.append(f"- {name}: {marker} ({status})")
        if detail:
            lines.append("```")
            lines.append(detail)
            lines.append("```")
    lines.append("")
    lines.append(f"Total tests: {len(RESULTS)}")
    lines.append(f"Verified findings: {sum(1 for _, s, _ in RESULTS if s in {'EXCEPTION', 'SIGNAL', 'CRASH'})}")
    REPORT.write_text("\n".join(lines) + "\n")
    print(REPORT.read_text())


if __name__ == "__main__":
    main()
