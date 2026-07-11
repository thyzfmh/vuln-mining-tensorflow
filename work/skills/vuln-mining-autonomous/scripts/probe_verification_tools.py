#!/usr/bin/env python3
from __future__ import annotations

import pathlib
import shutil
import subprocess
import sys
import tempfile

try:
    from platform_assets import resolve_work_root, output_path
except ImportError:  # pragma: no cover - direct execution fallback
    sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
    from platform_assets import resolve_work_root, output_path


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


def tool_version(tool: str) -> str:
    if not tool:
        return "unavailable"
    try:
        proc = run([tool, "--version"], timeout=10)
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


def fuzzing_probe(compiler: str) -> tuple[bool, str]:
    if not compiler:
        return False, "compiler unavailable"
    source = r"""
#include <cstddef>
#include <cstdint>
extern "C" int LLVMFuzzerTestOneInput(const uint8_t*, size_t) { return 0; }
"""
    with tempfile.TemporaryDirectory() as td:
        tmp = pathlib.Path(td)
        src = tmp / "probe.cc"
        binary = tmp / "probe"
        src.write_text(source)
        cmd = [compiler, "-std=c++17", "-fsanitize=fuzzer,address", str(src), "-o", str(binary)]
        try:
            compile_proc = run(cmd, cwd=tmp)
        except Exception as exc:  # pragma: no cover - defensive report path
            return False, f"compile error: {exc!r}"
        if compile_proc.returncode != 0:
            return False, f"compile returncode={compile_proc.returncode}; stderr={compile_proc.stderr.strip()[:300]}"
        run_proc = run([str(binary), "-runs=1"], cwd=tmp)
        if run_proc.returncode != 0:
            return False, f"run returncode={run_proc.returncode}; stderr={run_proc.stderr.strip()[:300]}"
        return True, "compile-and-run succeeded"


def main() -> None:
    work_root = resolve_work_root()
    REPORT = output_path(work_root, "reports", "toolchain-capabilities.md")

    clang = which("clang++")
    gxx = which("g++")
    npm = which("npm")
    npx = which("npx")
    compiler = clang or gxx
    asan_ok, asan_detail = sanitizer_probe(compiler, "address")
    ubsan_ok, ubsan_detail = sanitizer_probe(compiler, "undefined")
    fuzz_ok, fuzz_detail = fuzzing_probe(clang)

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
        f"- libFuzzer with ASAN: {'available' if fuzz_ok else 'unavailable'}",
        f"  - Detail: {fuzz_detail}",
        f"- bazel: `{which('bazel') or 'unavailable'}`",
        f"- cmake: `{which('cmake') or 'unavailable'}`",
        f"- ninja: `{which('ninja') or 'unavailable'}`",
        f"- npm: `{npm or 'unavailable'}`",
        f"- npm version: {tool_version(npm)}",
        f"- npx: `{npx or 'unavailable'}`",
        f"- npx version: {tool_version(npx)}",
        f"- codeql: `{which('codeql') or 'unavailable'}`",
        f"- semgrep: `{which('semgrep') or 'unavailable'}`",
        f"- valgrind: `{which('valgrind') or 'unavailable'}`",
        f"- llvm-cov: `{which('llvm-cov') or 'unavailable'}`",
        f"- llvm-profdata: `{which('llvm-profdata') or 'unavailable'}`",
        f"- gcov: `{which('gcov') or 'unavailable'}`",
        "",
        "## Policy",
        "",
        "- Memory-safety and undefined-behavior claims should use sanitizer output when ASAN or UBSAN is available.",
        "- If sanitizer unavailable, do not list memory-safety or undefined-behavior hypotheses unless a real target execution path still produces a runtime crash, signal, exception, or equivalent concrete failure.",
        "- If neither sanitizer nor real target execution proof exists, reject the hypothesis and record the reason in `reports/hypotheses.md`.",
        "- When libFuzzer and coverage tooling are available, use a bounded seed corpus and record whether the target builds, executes, crashes, and reaches additional code before claiming a finding.",
        "- When CodeQL or Semgrep is available, use its source-to-sink or taint results to expand the candidate ledger; static results remain hypotheses until a real target path verifies them.",
        "- When npm and npx are available, run `npm_ast_candidates.py`; it provisions the pinned `@ast-grep/cli` package without a global install and adds full-tree structural matches to the candidate ledger.",
    ]
    REPORT.write_text("\n".join(lines) + "\n")
    print(f"wrote {REPORT}")


if __name__ == "__main__":
    main()
