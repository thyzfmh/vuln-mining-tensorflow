#!/usr/bin/env python3
from __future__ import annotations

import json
import os
import pathlib
import shutil
import subprocess
from collections import defaultdict
from dataclasses import dataclass

try:
    from platform_assets import discover_target_root, resolve_work_root, output_path
except ImportError:  # pragma: no cover - direct execution fallback
    import sys
    sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
    from platform_assets import discover_target_root, resolve_work_root, output_path

AST_GREP_PACKAGE = "@ast-grep/cli@0.44.1"


@dataclass(frozen=True)
class Rule:
    identifier: str
    language: str
    pattern: str
    domain: str
    focus: str


RULES = [
    Rule("cpp-memory-copy", "Cpp", "memcpy($DST, $SRC, $SIZE)", "memory", "source/destination size relationship"),
    Rule("cpp-memory-move", "Cpp", "memmove($DST, $SRC, $SIZE)", "memory", "source/destination size relationship"),
    Rule("cpp-allocation", "Cpp", "malloc($SIZE)", "memory", "allocation size and overflow checks"),
    Rule("cpp-reallocation", "Cpp", "realloc($PTR, $SIZE)", "memory", "reallocation size and ownership checks"),
    Rule("cpp-division", "Cpp", "$LEFT / $RIGHT", "arithmetic", "zero and sign checks on divisor"),
    Rule("cpp-modulo", "Cpp", "$LEFT % $RIGHT", "arithmetic", "zero checks on modulo divisor"),
    Rule("python-eval", "Python", "eval($EXPR)", "python", "untrusted input reaching dynamic evaluation"),
    Rule("python-exec", "Python", "exec($EXPR)", "python", "untrusted input reaching dynamic execution"),
    Rule("python-import", "Python", "__import__($MODULE)", "python", "untrusted input reaching dynamic import"),
    Rule("python-ctypes", "Python", "ctypes.CDLL($LIB)", "boundary", "untrusted path reaching native-library loading"),
]


def run(argv: list[str], timeout: int = 300, env_overrides: dict[str, str] | None = None) -> subprocess.CompletedProcess[str]:
    env = os.environ.copy()
    env.update({
        "CI": "1",
        "npm_config_yes": "true",
        "npm_config_update_notifier": "false",
        "npm_config_fund": "false",
        "npm_config_audit": "false",
    })
    if env_overrides:
        env.update(env_overrides)
    return subprocess.run(
        argv,
        env=env,
        stdin=subprocess.DEVNULL,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=timeout,
    )


def scanner_prefix() -> tuple[list[str] | None, str]:
    npx = shutil.which("npx")
    if npx:
        return [npx, "--yes", "--package", AST_GREP_PACKAGE, "ast-grep"], "npx"
    binary = shutil.which("ast-grep")
    if binary:
        return [binary], "local binary"
    return None, "unavailable"


def source_file_count(target: pathlib.Path) -> int:
    extensions = {".c", ".cc", ".cpp", ".cxx", ".h", ".hh", ".hpp", ".hxx", ".cu", ".py"}
    return sum(1 for path in target.rglob("*") if path.is_file() and path.suffix in extensions)


def inline_rules() -> str:
    return "\n---\n".join(
        "\n".join([
            f"id: {rule.identifier}",
            f"language: {rule.language}",
            "rule:",
            f"  pattern: {rule.pattern}",
            "severity: warning",
            f"message: {rule.identifier} structural candidate",
        ])
        for rule in RULES
    )


def parse_match(line: str, target: pathlib.Path) -> tuple[str, int, str, str] | None:
    try:
        record = json.loads(line)
        raw_path = pathlib.Path(record["file"])
        relative = raw_path.relative_to(target).as_posix()
        line_number = int(record["range"]["start"]["line"]) + 1
        text = " ".join(str(record.get("text", "")).strip().split())[:180]
        rule_id = str(record["ruleId"])
    except (KeyError, TypeError, ValueError, json.JSONDecodeError):
        return None
    return relative, line_number, text, rule_id


def main() -> None:
    target = discover_target_root().resolve()
    work_root = resolve_work_root()
    REPORT = output_path(work_root, "reports", "npm-ast-candidates.md")
    temp_root = work_root / "verify" / ".tmp"
    npm_cache = work_root / "verify" / ".npm-cache"
    temp_root.mkdir(parents=True, exist_ok=True)
    npm_cache.mkdir(parents=True, exist_ok=True)
    child_env = {"TMPDIR": str(temp_root), "npm_config_cache": str(npm_cache)}
    prefix, source = scanner_prefix()
    files = source_file_count(target)
    lines = [
        "# NPM AST Candidate Extraction",
        "",
        "- Target alias: `TARGET_ROOT`",
        f"- Source files considered: {files}",
        f"- Package: `{AST_GREP_PACKAGE}`",
        f"- Scanner source: {source}",
    ]
    if prefix is None:
        lines.extend([
            "- Scanner status: unavailable",
            "- Detail: neither `npx` nor `ast-grep` is available; continue with deterministic extractors and runtime entrypoints.",
        ])
        REPORT.write_text("\n".join(lines) + "\n")
        print(f"wrote {REPORT}")
        return

    try:
        version = run([*prefix, "--version"], timeout=120, env_overrides=child_env)
    except Exception as exc:  # pragma: no cover - external tool failure report
        lines.extend([
            "- Scanner status: unavailable",
            f"- Detail: scanner bootstrap error: {exc!r}",
        ])
        REPORT.write_text("\n".join(lines) + "\n")
        print(f"wrote {REPORT}")
        return
    if version.returncode != 0:
        lines.extend([
            "- Scanner status: unavailable",
            f"- Detail: scanner bootstrap returncode={version.returncode}; stderr={version.stderr.strip()[:500]}",
        ])
        REPORT.write_text("\n".join(lines) + "\n")
        print(f"wrote {REPORT}")
        return

    matches: dict[str, list[tuple[Rule, int, str]]] = defaultdict(list)
    rules_by_id = {rule.identifier: rule for rule in RULES}
    rule_counts = {rule.identifier: 0 for rule in RULES}
    try:
        scan = run([
            *prefix,
            "scan",
            "--inline-rules",
            inline_rules(),
            "--json=stream",
            "--color",
            "never",
            "--no-ignore",
            "vcs",
            str(target),
        ], env_overrides=child_env)
    except Exception as exc:  # pragma: no cover - external tool failure report
        scan = None
        failure = f"scanner error={exc!r}"
    else:
        failure = "" if scan.returncode in {0, 1} else f"returncode={scan.returncode}; stderr={scan.stderr.strip()[:500]}"

    if scan is not None and not failure:
        for raw in scan.stdout.splitlines():
            parsed = parse_match(raw, target)
            if parsed is None:
                continue
            relative, line_number, text, rule_id = parsed
            rule = rules_by_id.get(rule_id)
            if rule is None:
                continue
            matches[relative].append((rule, line_number, text))
            rule_counts[rule.identifier] += 1

    status = "completed" if not failure else "partial"
    lines.extend([
        f"- Scanner version: {version.stdout.strip() or 'unknown'}",
        f"- Scanner status: {status}",
        f"- Rules executed: {len(RULES) if not failure else 0}/{len(RULES)}",
        "- Method: NPM-provisioned structural scans run over `TARGET_ROOT`; matches are candidates only and require source review plus real runtime proof.",
        "",
        "## Rule Summary",
        "",
    ])
    for rule in RULES:
        lines.append(f"- `{rule.identifier}`: {rule_counts.get(rule.identifier, 0)} matches; focus: {rule.focus}")
    if failure:
        lines.extend(["", "## Scanner Failures", ""])
        lines.append(f"- {failure}")
    lines.extend(["", "## Candidates", ""])
    if not matches:
        lines.append("- No structural matches found. This is not a scan-completion conclusion; continue with other candidate sources.")
    for path in sorted(matches):
        file_matches = matches[path]
        domains = sorted({rule.domain for rule, _, _ in file_matches})
        counts: dict[str, int] = defaultdict(int)
        examples: list[str] = []
        for rule, line_number, text in file_matches:
            counts[rule.identifier] += 1
            if len(examples) < 10:
                examples.append(f"L{line_number} [{rule.identifier}]: {text}")
        lines.append(f"### `{path}`")
        lines.append(f"- Domain: npm-ast-structural, {', '.join(domains)}")
        lines.append(f"- Match count: {len(file_matches)}")
        lines.append("- Pattern hits:")
        for identifier, count in sorted(counts.items()):
            lines.append(f"  - `{identifier}`: {count}")
        lines.append("- Evidence lines:")
        for example in examples:
            lines.append(f"  - `{example}`")
        lines.append("")
    REPORT.write_text("\n".join(lines) + "\n")
    print(f"wrote {REPORT}")


if __name__ == "__main__":
    main()
