#!/usr/bin/env python3
from __future__ import annotations

import os
import pathlib
import shutil
import subprocess
import sys
import tempfile

try:
    from platform_assets import discover_target_root, resolve_work_root, output_path
except ImportError:  # pragma: no cover - direct execution fallback
    sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
    from platform_assets import discover_target_root, resolve_work_root, output_path

COMMON_COMPILERS = [
    "clang++",
    "g++",
    "c++",
    "/opt/homebrew/opt/llvm/bin/clang++",
    "/usr/local/opt/llvm/bin/clang++",
]


def run(argv: list[str], cwd: pathlib.Path | None = None, env: dict[str, str] | None = None, timeout: int = 30) -> subprocess.CompletedProcess[str]:
    merged_env = os.environ.copy()
    if env:
        merged_env.update(env)
    return subprocess.run(
        argv,
        cwd=cwd,
        env=merged_env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=timeout,
    )


def compiler_candidates() -> list[str]:
    found: list[str] = []
    for candidate in COMMON_COMPILERS:
        path = shutil.which(candidate) if "/" not in candidate else candidate
        if path and pathlib.Path(path).exists() and path not in found:
            found.append(path)
    return found


def sanitizer_probe(compiler: str, flags: list[str]) -> tuple[bool, str]:
    source = r"""
#include <cstddef>
int main() {
  volatile int x = 1;
  volatile int y = 1;
  return static_cast<int>(x - y);
}
"""
    with tempfile.TemporaryDirectory() as td:
        tmp = pathlib.Path(td)
        src = tmp / "probe.cc"
        out = tmp / "probe"
        src.write_text(source)
        cmd = [compiler, "-std=c++17", "-O1", "-g", "-fno-omit-frame-pointer", *flags, str(src), "-o", str(out)]
        try:
            compile_proc = run(cmd, cwd=tmp)
        except Exception as exc:  # pragma: no cover - defensive report path
            return False, f"error={exc!r}"
        if compile_proc.returncode != 0:
            return False, f"argv={cmd}; returncode={compile_proc.returncode}; stderr={compile_proc.stderr.strip()[:500]}"
        run_proc = run([str(out)], cwd=tmp)
        return run_proc.returncode == 0, f"argv={cmd}; run_returncode={run_proc.returncode}; stderr={run_proc.stderr.strip()[:500]}"


def detect_build_systems(target: pathlib.Path) -> list[str]:
    systems: list[str] = []
    roots = [target]
    roots.extend([p for p in target.iterdir() if p.is_dir() and not p.name.startswith(".")])
    for root in roots:
        if not root.exists():
            continue
        rel = "TARGET_ROOT" if root == target else f"TARGET_ROOT/{root.name}"
        checks = [
            ("Bazel", ["WORKSPACE", "WORKSPACE.bazel", "MODULE.bazel", "BUILD", "BUILD.bazel"]),
            ("CMake", ["CMakeLists.txt"]),
            ("Make", ["Makefile", "makefile"]),
            ("Python package", ["pyproject.toml", "setup.py", "setup.cfg"]),
        ]
        for name, files in checks:
            if any((root / file).exists() for file in files):
                systems.append(f"{name} at `{rel}`")
    return sorted(set(systems))


def available_runtime_alternatives() -> list[str]:
    alternatives: list[str] = []
    if shutil.which("valgrind"):
        alternatives.append("Valgrind available: use `valgrind --error-exitcode=99 --track-origins=yes <real-target-command>`")
    if sys.platform == "darwin" and pathlib.Path("/usr/lib/libgmalloc.dylib").exists():
        alternatives.append("Guard Malloc available: run real target command with `DYLD_INSERT_LIBRARIES=/usr/lib/libgmalloc.dylib MallocScribble=1 MallocPreScribble=1`")
    alternatives.append("Python debug runtime available: run Python target paths with `PYTHONMALLOC=debug` and `python3 -X dev -X faulthandler`")
    return alternatives


def main() -> None:
    target = discover_target_root()
    work_root = resolve_work_root()
    REPORT = output_path(work_root, "reports", "verification-escalation.md")

    compilers = compiler_candidates()
    sanitizer_attempts: list[tuple[str, str, bool, str]] = []
    for compiler in compilers:
        for name, flags in [
            ("ASAN", ["-fsanitize=address"]),
            ("UBSAN", ["-fsanitize=undefined"]),
            ("ASAN+UBSAN", ["-fsanitize=address,undefined"]),
        ]:
            ok, detail = sanitizer_probe(compiler, flags)
            sanitizer_attempts.append((compiler, name, ok, detail))

    any_sanitizer = any(ok for _, _, ok, _ in sanitizer_attempts)
    build_systems = detect_build_systems(target)
    alternatives = available_runtime_alternatives()

    lines = [
        "# Verification Escalation",
        "",
        f"- status: {'sanitizer-ready' if any_sanitizer else 'fallback-required'}",
        f"- python: `{sys.executable}`",
        f"- compiler-candidates: {len(compilers)}",
        "",
        "## Sanitizer Attempts",
        "",
    ]
    if sanitizer_attempts:
        for compiler, name, ok, detail in sanitizer_attempts:
            lines.append(f"- {name} via `{compiler}`: {'available' if ok else 'unavailable'}")
            lines.append(f"  - Detail: {detail}")
    else:
        lines.append("- No C++ compiler candidates found.")

    lines.extend(["", "## Build-System Injection Options", ""])
    if build_systems:
        for system in build_systems:
            lines.append(f"- {system}")
        lines.extend(
            [
                "- Bazel native sanitizer attempt: `bazel test --copt=-fsanitize=address --linkopt=-fsanitize=address --strip=never <target>`",
                "- Bazel UB sanitizer attempt: `bazel test --copt=-fsanitize=undefined --linkopt=-fsanitize=undefined --strip=never <target>`",
                "- CMake sanitizer attempt: configure with `-DCMAKE_C_FLAGS=-fsanitize=address,undefined -DCMAKE_CXX_FLAGS=-fsanitize=address,undefined -DCMAKE_EXE_LINKER_FLAGS=-fsanitize=address,undefined`",
                "- Make sanitizer attempt: run with `CC=clang CXX=clang++ CFLAGS=-fsanitize=address,undefined CXXFLAGS=-fsanitize=address,undefined LDFLAGS=-fsanitize=address,undefined`",
            ]
        )
    else:
        lines.append("- No common build system detected at TARGET_ROOT root candidates.")

    lines.extend(["", "## Runtime Fallback Options", ""])
    for alternative in alternatives:
        lines.append(f"- {alternative}")

    lines.extend(
        [
            "",
            "## Mandatory Escalation Policy",
            "",
            "- If ASAN/UBSAN is unavailable in the initial probe, run this escalation script before rejecting any memory-safety or undefined-behavior hypothesis.",
            "- If any sanitizer attempt succeeds here, use that compiler or build-system injection path for native reproducers.",
            "- If no sanitizer path succeeds, try Valgrind, Guard Malloc, Python debug runtime, or another real target command that exercises the vulnerable path.",
            "- Do not list memory-safety or undefined-behavior vulnerabilities from source reasoning alone.",
            "- Only reject after recording the attempted sanitizer/compiler/build-system/runtime escalation and the concrete reason no real proof was produced.",
        ]
    )
    REPORT.write_text("\n".join(lines) + "\n")
    print(f"wrote {REPORT}")


if __name__ == "__main__":
    main()
