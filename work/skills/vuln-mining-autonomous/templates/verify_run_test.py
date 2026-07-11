#!/usr/bin/env python3
"""AI-generated vulnerability verification script.

The OpenCode run must replace this template with concrete AI-generated tests.
All output paths are relative to WORK_ROOT (default: work/).
TARGET_ROOT is resolved from the persisted work context, with environment and
code/ fallbacks for explicit local tests.
"""

from __future__ import annotations

import os
import pathlib
import subprocess
import sys
import tempfile
import traceback

WORK_ROOT = pathlib.Path(os.environ.get("VULN_WORK_ROOT", os.environ.get("WORK_ROOT", "work")))
REPORT = WORK_ROOT / "reports" / "verification-output.txt"
TARGET_CONTEXT = WORK_ROOT / ".vuln-mining-target-root"
TARGET_ROOT = pathlib.Path(
    os.environ.get(
        "VULN_TARGET_ROOT",
        os.environ.get(
            "TARGET_ROOT",
            TARGET_CONTEXT.read_text().strip() if TARGET_CONTEXT.is_file() else "code",
        ),
    )
)
RESULTS: list[tuple[str, str, str]] = []


def target_source_path(*parts):
    return TARGET_ROOT.joinpath(*parts)


def run_test(name, fn, verified_exception_markers=()):
    try:
        fn()
        RESULTS.append((name, "NO_CRASH", ""))
    except Exception as exc:
        detail = repr(exc)
        status = "EXCEPTION" if any(marker in detail for marker in verified_exception_markers) else "REJECTED"
        RESULTS.append((name, status, detail))
        traceback.print_exc()


def run_subprocess_test(name, code, verified_error_markers=()):
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
        status = "EXCEPTION" if any(marker in detail for marker in verified_error_markers) else "REJECTED"
        RESULTS.append((name, status, detail))


def run_command_test(name, argv, timeout=120, cwd=None, env=None, verified_error_markers=()):
    proc = subprocess.run(
        argv,
        cwd=cwd,
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=timeout,
    )
    detail = f"argv={argv}\nreturncode={proc.returncode}\nstdout={proc.stdout}\nstderr={proc.stderr}"
    if proc.returncode == 0:
        RESULTS.append((name, "NO_CRASH", detail))
    elif proc.returncode < 0:
        RESULTS.append((name, "SIGNAL", detail))
    else:
        crash_words = ("AddressSanitizer", "UndefinedBehaviorSanitizer", "runtime error", "Segmentation fault")
        if any(word in detail for word in crash_words):
            status = "CRASH"
        elif any(marker in detail for marker in verified_error_markers):
            status = "EXCEPTION"
        else:
            status = "REJECTED"
        RESULTS.append((name, status, detail))


def run_seed_corpus(
    name,
    command_for_seed,
    seeds,
    timeout=120,
    cwd=None,
    env=None,
    verified_error_markers=(),
):
    """Run a bounded AI-generated corpus through a real TARGET_ROOT command."""
    with tempfile.TemporaryDirectory() as td:
        corpus_dir = pathlib.Path(td)
        for label, payload in seeds:
            seed_path = corpus_dir / label
            seed_path.write_bytes(payload)
            run_command_test(
                f"{name} [{label}]",
                command_for_seed(seed_path),
                timeout=timeout,
                cwd=cwd,
                env=env,
                verified_error_markers=verified_error_markers,
            )


def compile_and_run_cxx_test(
    name,
    source_path,
    extra_flags=None,
    include_paths=None,
    timeout=120,
    allow_unsanitized_fallback=True,
):
    extra_flags = extra_flags or []
    include_paths = include_paths or [TARGET_ROOT]
    compiler = "clang++"
    binary = WORK_ROOT / "verify" / f"{pathlib.Path(source_path).stem}.bin"
    flags = ["-std=c++17", "-O1", "-g", "-fno-omit-frame-pointer"]
    sanitize_flags = ["-fsanitize=address,undefined"]
    include_flags = [flag for include_path in include_paths for flag in ("-I", str(include_path))]
    compile_cmd = [compiler, *flags, *sanitize_flags, *include_flags, *extra_flags, str(source_path), "-o", str(binary)]
    compile_proc = subprocess.run(compile_cmd, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=timeout)
    if compile_proc.returncode != 0:
        sanitizer_detail = (
            "sanitizer unavailable\n"
            f"argv={compile_cmd}\nstdout={compile_proc.stdout}\nstderr={compile_proc.stderr}"
        )
        if not allow_unsanitized_fallback:
            RESULTS.append((name, "BUILD_FAIL", sanitizer_detail))
            return
        fallback_cmd = [compiler, *flags, *include_flags, *extra_flags, str(source_path), "-o", str(binary)]
        fallback_proc = subprocess.run(
            fallback_cmd,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=timeout,
        )
        if fallback_proc.returncode != 0:
            detail = (
                f"{sanitizer_detail}\n"
                f"fallback_argv={fallback_cmd}\nstdout={fallback_proc.stdout}\nstderr={fallback_proc.stderr}"
            )
            RESULTS.append((name, "BUILD_FAIL", detail))
            return
        RESULTS.append((f"{name} sanitizer probe", "NO_CRASH", sanitizer_detail))
    run_command_test(name, [str(binary)], timeout=timeout)


def main():
    REPORT.parent.mkdir(parents=True, exist_ok=True)
    # AI-generated test cases are appended here during the run.
    # Each test must exercise a real TARGET_ROOT API, command, parser, module,
    # or native source/header path before marking a finding VERIFIED.

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
