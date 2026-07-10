#!/usr/bin/env python3
from __future__ import annotations

import pathlib
import re
from dataclasses import dataclass

ROOT = pathlib.Path.cwd()
CODE = ROOT / "code"
REPORT = ROOT / "reports" / "runtime-entrypoints.md"
SOURCE_EXTENSIONS = {".c", ".cc", ".cpp", ".cxx", ".h", ".hh", ".hpp", ".hxx", ".cu", ".py"}
BUILD_FILES = {"BUILD", "BUILD.bazel", "CMakeLists.txt"}

NATIVE_TEST = re.compile(r"\b(?:TEST|TEST_F|TEST_P|TYPED_TEST|BENCHMARK|TEST_CASE)\s*\(")
PYTHON_TEST = re.compile(r"^\s*def\s+test_[A-Za-z0-9_]*\s*\(")
NATIVE_MAIN = re.compile(r"\b(?:int|auto)\s+main\s*\(")
PYTHON_MAIN = re.compile(r"__name__\s*==\s*[\"']__main__[\"']")
EXTENSION_MODULE = re.compile(r"\b(?:PYBIND11_MODULE|PyInit_[A-Za-z0-9_]+)\b")
BUILD_RULE = re.compile(r"^\s*(cc_test|py_test|tf_cc_test|cuda_test|cc_binary|py_binary)\s*\(")
BUILD_NAME = re.compile(r"^\s*name\s*=\s*[\"']([^\"']+)[\"']")
CMAKE_TEST = re.compile(r"\badd_test\s*\(\s*(?:NAME\s+)?([^\s)]+)", re.I)
CMAKE_BINARY = re.compile(r"\badd_executable\s*\(\s*([^\s)]+)", re.I)


@dataclass(frozen=True)
class Entrypoint:
    kind: str
    target: str
    surface: str
    evidence: str
    verification: str


def detect_target() -> pathlib.Path:
    if not CODE.is_dir():
        raise SystemExit("code/ directory is missing")
    children = [path for path in CODE.iterdir() if path.is_dir() and not path.name.startswith(".")]
    return children[0] if len(children) == 1 else CODE


def relative(path: pathlib.Path, target: pathlib.Path) -> str:
    return path.relative_to(target).as_posix()


def compact(line: str) -> str:
    return " ".join(line.strip().split())[:180]


def scan_source_file(path: pathlib.Path, target: pathlib.Path) -> list[Entrypoint]:
    try:
        lines = path.read_text(errors="replace").splitlines()
    except OSError:
        return []

    rel = relative(path, target)
    entries: list[Entrypoint] = []
    for number, line in enumerate(lines, start=1):
        evidence = f"`{rel}:L{number}` {compact(line)}"
        if path.suffix == ".py" and PYTHON_TEST.search(line):
            entries.append(Entrypoint(
                "python-test",
                f"`{rel}:L{number}`",
                "existing Python test function",
                evidence,
                "extend the existing test with an adversarial input and compare against a valid baseline",
            ))
        elif path.suffix != ".py" and NATIVE_TEST.search(line):
            entries.append(Entrypoint(
                "native-test",
                f"`{rel}:L{number}`",
                "existing native test case",
                evidence,
                "extend the existing test target with a bounded malformed-input seed matrix",
            ))
        if path.suffix == ".py" and PYTHON_MAIN.search(line):
            entries.append(Entrypoint(
                "python-cli",
                f"`{rel}:L{number}`",
                "Python command-line entry point",
                evidence,
                "run the real command in a subprocess with a valid baseline and malformed input",
            ))
        elif path.suffix != ".py" and NATIVE_MAIN.search(line):
            entries.append(Entrypoint(
                "native-cli",
                f"`{rel}:L{number}`",
                "native command-line entry point",
                evidence,
                "run the linked binary with sanitizer or runtime fallback instrumentation",
            ))
        if path.suffix != ".py" and EXTENSION_MODULE.search(line):
            entries.append(Entrypoint(
                "extension-module",
                f"`{rel}:L{number}`",
                "native language-binding module",
                evidence,
                "exercise the built module through its public binding or existing binding test",
            ))
    return entries


def scan_build_file(path: pathlib.Path, target: pathlib.Path) -> list[Entrypoint]:
    try:
        lines = path.read_text(errors="replace").splitlines()
    except OSError:
        return []

    rel = relative(path, target)
    directory = path.parent.relative_to(target).as_posix()
    package = "" if directory == "." else directory
    entries: list[Entrypoint] = []
    current_rule = ""
    current_line = 0
    for number, line in enumerate(lines, start=1):
        rule = BUILD_RULE.search(line)
        if rule:
            current_rule = rule.group(1)
            current_line = number
            continue
        name = BUILD_NAME.search(line)
        if name and current_rule:
            label = f"//{package}:{name.group(1)}" if package else f"//:{name.group(1)}"
            is_test = current_rule.endswith("test")
            entries.append(Entrypoint(
                "build-test-target" if is_test else "build-binary-target",
                f"`{label}`",
                f"{current_rule} build rule",
                f"`{rel}:L{current_line}-L{number}` {current_rule}(name={name.group(1)})",
                "run the existing build target with an AI-generated test or reproducer" if is_test else "build and run the real binary or depend on it from a focused test target",
            ))
            current_rule = ""
            continue
        if current_rule and line.strip() == ")":
            current_rule = ""

    if path.name == "CMakeLists.txt":
        for number, line in enumerate(lines, start=1):
            for pattern, kind, surface, verification in [
                (CMAKE_TEST, "cmake-test-target", "CMake registered test", "run the registered test with an AI-generated adversarial case"),
                (CMAKE_BINARY, "cmake-binary-target", "CMake executable", "build and run the real executable with a valid baseline and malformed input"),
            ]:
                match = pattern.search(line)
                if match:
                    entries.append(Entrypoint(
                        kind,
                        f"`{match.group(1)}`",
                        surface,
                        f"`{rel}:L{number}` {compact(line)}",
                        verification,
                    ))
    return entries


def main() -> None:
    target = detect_target()
    source_files = 0
    entries: list[Entrypoint] = []
    for path in target.rglob("*"):
        if not path.is_file():
            continue
        if path.suffix in SOURCE_EXTENSIONS:
            source_files += 1
            entries.extend(scan_source_file(path, target))
        if path.name in BUILD_FILES:
            entries.extend(scan_build_file(path, target))

    entries = sorted(set(entries), key=lambda entry: (entry.kind, entry.target, entry.evidence))
    by_kind: dict[str, int] = {}
    for entry in entries:
        by_kind[entry.kind] = by_kind.get(entry.kind, 0) + 1

    REPORT.parent.mkdir(parents=True, exist_ok=True)
    lines = [
        "# Runtime Entrypoint Map",
        "",
        "- Target alias: `TARGET_ROOT`",
        f"- Source files traversed: {source_files}",
        f"- Discovered runtime entrypoints: {len(entries)}",
        "- Purpose: map static candidates to existing real execution surfaces before creating a custom reproducer.",
        "",
        "## Entrypoint Summary",
        "",
    ]
    if by_kind:
        for kind, count in sorted(by_kind.items()):
            lines.append(f"- `{kind}`: {count}")
    else:
        lines.append("- No conventional test, CLI, module, or build entrypoint was detected; inspect build metadata manually before rejecting candidates.")
    lines.extend([
        "",
        "## Entrypoints",
        "",
        "| Kind | Target | Runtime surface | Evidence | Preferred verification |",
        "|---|---|---|---|---|",
    ])
    for entry in entries:
        lines.append(f"| {entry.kind} | {entry.target} | {entry.surface} | {entry.evidence} | {entry.verification} |")
    REPORT.write_text("\n".join(lines) + "\n")
    print(f"wrote {REPORT}")


if __name__ == "__main__":
    main()
