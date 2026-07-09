#!/usr/bin/env python3
from __future__ import annotations

import pathlib
import shutil
import subprocess
import sys
import tempfile

ROOT = pathlib.Path.cwd()
REPORT = ROOT / "reports" / "toolchain-capabilities.md"


def which(name: str) -> str:
    return shutil.which(name) or ""


def run(argv: list[str], cwd: pathlib.Path | None = None, timeout: int = 30) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        argv,
        cwd=cwd,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=timeout,
    )


def compiler_version(compiler: str) -> str:
    if not compiler:
        return "unavailable"
    try:
        proc = run([compiler, "--version"], timeout=10)
    except Exception as exc:  # pragma: no cover - defensive report path
        return f"error: {exc!r}"
    first = (proc.stdout or proc.stderr).splitlines()
    return first[0] if first else f"returncode={proc.returncode}"


def sanitizer_probe(compiler: str, sanitizer: str) -> tuple[bool, str]:
    if not compiler:
        return False, "compiler unavailable"
    source = r"""
int main() {
  volatile int x = 1;
  volatile int y = 1;
  return x - y;
}
"""
    with tempfile.TemporaryDirectory() as td:
        tmp = pathlib.Path(td)
        src = tmp / "probe.cc"
        binary = tmp / "probe"
        src.write_text(source)
        cmd = [compiler, "-std=c++17", f"-fsanitize={sanitizer}", str(src), "-o", str(binary)]
        try:
            compile_proc = run(cmd, cwd=tmp)
        except Exception as exc:  # pragma: no cover - defensive report path
            return False, f"compile error: {exc!r}"
        if compile_proc.returncode != 0:
            return False, f"compile returncode={compile_proc.returncode}; stderr={compile_proc.stderr.strip()[:300]}"
        try:
            run_proc = run([str(binary)], cwd=tmp)
        except Exception as exc:  # pragma: no cover - defensive report path
            return False, f"run error: {exc!r}"
        if run_proc.returncode != 0:
            return False, f"run returncode={run_proc.returncode}; stderr={run_proc.stderr.strip()[:300]}"
        return True, "compile-and-run succeeded"


def main() -> None:
    REPORT.parent.mkdir(parents=True, exist_ok=True)
    clang = which("clang++")
    gxx = which("g++")
    compiler = clang or gxx
    asan_ok, asan_detail = sanitizer_probe(compiler, "address")
    ubsan_ok, ubsan_detail = sanitizer_probe(compiler, "undefined")

    lines = [
        "# Verification Toolchain Capabilities",
        "",
        f"- Python: `{sys.executable}`",
        f"- clang++: `{clang or 'unavailable'}`",
        f"- clang++ version: {compiler_version(clang)}",
        f"- g++: `{gxx or 'unavailable'}`",
        f"- g++ version: {compiler_version(gxx)}",
        f"- ASAN: {'available' if asan_ok else 'unavailable'}",
        f"  - Detail: {asan_detail}",
        f"- UBSAN: {'available' if ubsan_ok else 'unavailable'}",
        f"  - Detail: {ubsan_detail}",
        f"- bazel: `{which('bazel') or 'unavailable'}`",
        f"- cmake: `{which('cmake') or 'unavailable'}`",
        f"- ninja: `{which('ninja') or 'unavailable'}`",
        f"- valgrind: `{which('valgrind') or 'unavailable'}`",
        "",
        "## Policy",
        "",
        "- Memory-safety and undefined-behavior claims should use sanitizer output when ASAN or UBSAN is available.",
        "- If sanitizer unavailable, do not list memory-safety or undefined-behavior hypotheses unless a real target execution path still produces a runtime crash, signal, exception, or equivalent concrete failure.",
        "- If neither sanitizer nor real target execution proof exists, reject the hypothesis and record the reason in `reports/hypotheses.md`.",
    ]
    REPORT.write_text("\n".join(lines) + "\n")
    print(f"wrote {REPORT}")


if __name__ == "__main__":
    main()
