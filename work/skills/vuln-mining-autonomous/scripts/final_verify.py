#!/usr/bin/env python3
"""Final verification gate.

Runs against the ``work/`` output root and an externally supplied target root.

Usage::

    python3 final_verify.py [--work-root <path>] [--target-root <path>]

Defaults:
  --work-root   <repo>/work
  --target-root discovered by platform_assets.discover_target_root()
"""
from __future__ import annotations

import argparse
import json
import pathlib
import re
import sys

try:
    from platform_assets import discover_target_root, resolve_work_root, target_blacklist_names, REPO_ROOT
except ImportError:  # pragma: no cover - direct execution fallback
    sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
    from platform_assets import discover_target_root, resolve_work_root, target_blacklist_names, REPO_ROOT

FAILURES: list[str] = []
SIMULATION_MARKERS = [
    "toy snippet",
    "pattern-only",
    "standalone pattern",
    "simulated vulnerability",
    "simulate the bug",
    "imitates",
    "does not call target",
    "\u6a21\u62df\u6f0f\u6d1e",
    "\u6a21\u62df\u6d4b\u8bd5",
    "\u6f14\u793a\u4ee3\u7801",
]
PREMATURE_STOP_PATTERNS = [
    r"NO_RUNTIME_VERIFIED_VULNERABILITIES",
    r"verified vulnerabilit(y|ies) count:\s*0",
    r"verified vulnerabilities:\s*0",
    r"no runtime-verified vulnerabilities",
    r"no verified vulnerabilities",
    r"\u672a\u53d1\u73b0\u4efb\u4f55\u53ef\u901a\u8fc7\u8fd0\u884c\u65f6\u9a8c\u8bc1\u7684\u6f0f\u6d1e",
    r"(\u7531\u4e8e|\u56e0).{0,20}(\u5de5\u5177\u94fe|asan|ubsan).{0,80}(\u672a\u80fd\u53d1\u73b0|\u672a\u53d1\u73b0|0\s*\u4e2a)",
    r"(\u7531\u4e8e|\u56e0).{0,20}\u65e0\u6cd5\u4ece\u6e90\u7801\u6811.{0,80}(\u672a\u80fd\u53d1\u73b0|\u672a\u53d1\u73b0|0\s*\u4e2a)",
    r"(because of|due to).{0,40}(toolchain|asan|ubsan|source-tree import).{0,80}(no|zero|0).{0,40}(verified|runtime)",
]

# Module-level globals set by main() / parse_args
WORK_ROOT: pathlib.Path = pathlib.Path()
TARGET_ROOT: pathlib.Path = pathlib.Path()


def fail(message: str) -> None:
    FAILURES.append(message)


def require_file(rel: str) -> pathlib.Path:
    p = WORK_ROOT / rel
    if not p.is_file():
        fail(f"missing required file: {rel}")
    return p


def text(rel: str) -> str:
    p = require_file(rel)
    return p.read_text(errors="replace") if p.is_file() else ""


def markdown_heading_paths(body: str) -> list[str]:
    paths: list[str] = []
    for line in body.splitlines():
        match = re.match(r"^### `(.+)`", line)
        if match:
            paths.append(match.group(1))
    return paths


def reviewed_coverage_rows(body: str) -> list[tuple[str, str]]:
    rows: list[tuple[str, str]] = []
    for line in body.splitlines():
        stripped = line.strip()
        if not stripped.startswith("|") or set(stripped.replace("|", "").strip()) <= {"-"}:
            continue
        cells = [cell.strip() for cell in stripped.strip("|").split("|")]
        if len(cells) < 7 or cells[0].lower() == "target":
            continue
        status = cells[5].lower().strip("` ")
        evidence = cells[6].strip("` ")
        if status in {"verified", "rejected"} and evidence and evidence not in {"-", "n/a"}:
            rows.append((status, evidence))
    return rows


def contains_simulation_marker(body: str) -> str | None:
    lower = body.lower()
    for marker in SIMULATION_MARKERS:
        if marker.lower() in lower:
            return marker
    return None


def verify_no_premature_toolchain_stop() -> None:
    combined = "\n".join([
        text("vulnerability_list.md"),
        text("vulnerability_report.md"),
        text("reports/verification-output.txt"),
        text("result/output.md"),
    ]).lower()
    for pattern in PREMATURE_STOP_PATTERNS:
        if re.search(pattern, combined, re.I):
            fail("toolchain or import limitations cannot be used as the final no-finding status; continue with escalation and alternative real target paths")


def verify_chat_log() -> None:
    p = require_file("llm_chat_log.json")
    if not p.is_file():
        return
    try:
        data = json.loads(p.read_text())
    except json.JSONDecodeError as exc:
        fail(f"llm_chat_log.json is invalid JSON: {exc}")
        return
    history = data.get("chat_history")
    if not isinstance(history, list):
        fail("llm_chat_log.json chat_history must be a list")
        return
    if len(history) < 8:
        fail("llm_chat_log.json must contain at least 8 turns across review, skeptic, and exhaustive triage waves")
    for index, entry in enumerate(history, start=1):
        if not isinstance(entry, dict):
            fail(f"llm_chat_log.json chat_history[{index}] must be an object")
            continue
        role = entry.get("role")
        content = entry.get("content")
        if role not in {"user", "assistant", "system"}:
            fail(f"llm_chat_log.json chat_history[{index}] has invalid role: {role!r}")
        if not isinstance(content, str) or not content.strip():
            fail(f"llm_chat_log.json chat_history[{index}] must contain non-empty content")
        if role == "user" and "TARGET_ROOT" not in content:
            fail(f"llm_chat_log.json user prompt {index} must use TARGET_ROOT alias")
    content = json.dumps(data, ensure_ascii=False).lower()
    forbidden = [
        r"v2\.?\d+",
        r"version\s+\d",
        r"cve-\d",
        r"\bnvd\b",
        r"\bghsa\b",
        r"known\s+bug",
        r"known\s+vulnerability",
        "\u5df2\u77e5\u6f0f\u6d1e",
    ]
    for name in target_blacklist_names(TARGET_ROOT):
        escaped = re.escape(name.lower())
        forbidden.append(rf"(?<![a-z0-9_]){escaped}(?![a-z0-9_])")
    for pattern in forbidden:
        if re.search(pattern, content):
            fail(f"llm_chat_log.json contains black-box forbidden pattern: {pattern}")
    if not re.search(r"\b(skeptic|false[- ]positive)\b|\u53cd\u8bc1|\u8bef\u62a5", content):
        fail("llm_chat_log.json must include a skeptic or false-positive rejection pass")
    if not re.search(r"\b(variant|all remaining|remaining candidate|candidate[- ]space|exhaustive)\b|\u53d8\u4f53|\u5269\u4f59|\u5168\u90e8\u5019\u9009", content):
        fail("llm_chat_log.json must include exhaustive remaining-candidate or variant-family search prompts")


def verify_vulnerability_list() -> None:
    body = text("vulnerability_list.md")
    required = [
        "漏洞 #",
        "漏洞类型",
        "严重级别",
        "问题源码路径",
        "成因简述",
        "AI生成的测试用例",
        "验证结果",
        "与LLM交互中哪句提示词发现了bug",
        "为什么选择此提示词",
        "潜在业务危害",
    ]
    for item in required:
        if item not in body:
            fail(f"vulnerability_list.md missing section: {item}")
    placeholder_markers = [
        "Generated during the OpenCode run",
        "[简短直观",
        "[说明",
        "[必须",
        "[粘贴",
        "[DoS",
        "TARGET_ROOT/...",
    ]
    for marker in placeholder_markers:
        if marker in body:
            fail(f"vulnerability_list.md still contains template marker: {marker}")
    marker = contains_simulation_marker(body)
    if marker:
        fail(f"vulnerability_list.md appears to rely on simulated proof: {marker}")
    if body.count("漏洞 #") < 1:
        fail("vulnerability_list.md must include at least one vulnerability")
    if not re.search(r"\bVERIFIED\b", body, re.I):
        fail("vulnerability_list.md must include VERIFIED runtime evidence")


def verify_run_test() -> None:
    body = text("verify/run_test.py")
    if "AI-generated" not in body and "AI generated" not in body:
        fail("verify/run_test.py must identify tests as AI-generated")
    if "TARGET_ROOT" not in body:
        fail("verify/run_test.py must reference TARGET_ROOT/code paths instead of standalone toy code")
    if "reports/verification-output.txt" not in body:
        fail("verify/run_test.py must write reports/verification-output.txt")
    if not re.search(r"def\s+test_", body):
        fail("verify/run_test.py must contain at least one test_ function")
    marker = contains_simulation_marker(body)
    if marker:
        fail(f"verify/run_test.py appears to rely on simulated proof: {marker}")
    for item in ["TODO", "placeholder", "pass  #", "NotImplemented"]:
        if item.lower() in body.lower():
            fail(f"verify/run_test.py contains placeholder marker: {item}")


def verify_reports() -> None:
    body = text("reports/verification-output.txt")
    if not re.search(r"\bVERIFIED\b", body, re.I):
        fail("reports/verification-output.txt must contain VERIFIED runtime evidence")
    if not re.search(r"\b(argv|command|cmd)=", body, re.I):
        fail("reports/verification-output.txt must include the exact runtime command or argv")
    if not re.search(r"\breturncode=|\bsignal\b|traceback|AddressSanitizer|UndefinedBehaviorSanitizer", body, re.I):
        fail("reports/verification-output.txt must include return code, signal, traceback, or sanitizer output")
    marker = contains_simulation_marker(body)
    if marker:
        fail(f"reports/verification-output.txt appears to rely on simulated proof: {marker}")
    report = text("vulnerability_report.md")
    if "Generated during the OpenCode run" in report:
        fail("vulnerability_report.md still contains template marker: Generated during the OpenCode run")
    for item in [
        "Black-Box",
        "method",
        "attack surface",
        "candidate",
        "npm ast",
        "hypoth",
        "verification",
        "reproduc",
        "coverage",
        "coverage-ledger",
        "scan-completion",
        "candidate-space-exhausted",
        "rejected",
        "sanitizer",
    ]:
        if item.lower() not in report.lower():
            fail(f"vulnerability_report.md missing expected discussion: {item}")
    claims = text("vulnerability_list.md").lower()
    needs_sanitizer = [
        "heap overflow",
        "oob",
        "uaf",
        "type confusion",
        "integer overflow",
        "\u8d8a\u754c",
        "\u6ea2\u51fa",
    ]
    if any(term in claims for term in needs_sanitizer):
        combined = "\n".join([body, report])
        if not re.search(r"AddressSanitizer|UndefinedBehaviorSanitizer|-fsanitize|sanitizer unavailable", combined, re.I):
            fail("memory/overflow claims must include sanitizer evidence or an explicit sanitizer-unavailable note")
        if "sanitizer unavailable" in combined.lower() and not re.search(r"\b(Valgrind|Guard Malloc|DYLD_INSERT_LIBRARIES|PYTHONMALLOC=debug|signal|returncode=-|fatal|crash)\b", combined, re.I):
            fail("memory/overflow claims with unavailable sanitizer must include escalated real-target proof, not only an unavailable note")


def verify_method_artifacts() -> None:
    for path in [
        "reports/source-inventory.md",
        "reports/source-file-manifest.md",
        "reports/attack-surface-map.md",
        "reports/sast-candidates.md",
        "reports/npm-ast-candidates.md",
        "reports/toolchain-capabilities.md",
        "reports/verification-escalation.md",
        "reports/runtime-entrypoints.md",
        "reports/coverage-ledger.md",
        "reports/scan-completion.md",
        "reports/hypotheses.md",
        "plans/scan-wave-001.md",
    ]:
        require_file(path)

    plan = text("plans/scan-wave-001.md").lower()
    for item in ["target_root", "source", "sink", "sanitizer", "prompt", "test"]:
        if item not in plan:
            fail(f"plans/scan-wave-001.md missing expected scan planning field: {item}")

    hypotheses = text("reports/hypotheses.md").lower()
    for item in ["source", "sink", "sanitizer"]:
        if item not in hypotheses:
            fail(f"reports/hypotheses.md missing expected triage field: {item}")
    if not re.search(r"\b(accepted|rejected|verified)\b", hypotheses):
        fail("reports/hypotheses.md must record accepted, rejected, or verified hypotheses")

    verify_coverage_ledger()
    verify_scan_waves()
    verify_scan_completion()
    verify_toolchain_capabilities()
    verify_verification_escalation()
    verify_runtime_entrypoints()
    verify_npm_ast_candidates()


def verify_coverage_ledger() -> None:
    body = text("reports/coverage-ledger.md")
    lower = body.lower()
    for item in ["# coverage ledger", "coverage-budget:", "minimum-reviewed-targets:", "candidate-space-exhausted:", "| target | domain | source | sink | sanitizer status | status | evidence |"]:
        if item not in lower:
            fail(f"reports/coverage-ledger.md missing required field: {item}")

    if "candidate-space-exhausted: yes" not in lower:
        fail("reports/coverage-ledger.md must set candidate-space-exhausted: yes")

    minimum_match = re.search(r"minimum-reviewed-targets:\s*(\d+)", lower)
    if not minimum_match:
        fail("reports/coverage-ledger.md must declare numeric minimum-reviewed-targets")
        minimum = 0
    else:
        minimum = int(minimum_match.group(1))

    attack_paths = markdown_heading_paths(text("reports/attack-surface-map.md"))
    sast_paths = markdown_heading_paths(text("reports/sast-candidates.md"))
    npm_ast_paths = markdown_heading_paths(text("reports/npm-ast-candidates.md"))
    all_paths = sorted(set(attack_paths + sast_paths + npm_ast_paths))
    expected = len(all_paths) if all_paths else minimum
    if minimum < expected:
        fail(f"reports/coverage-ledger.md minimum-reviewed-targets must be at least {expected}")

    ledger_lower = lower
    missing_paths = [path for path in all_paths if f"target_root/{path.lower()}" not in ledger_lower]
    if missing_paths:
        preview = ", ".join(missing_paths[:5])
        fail(f"reports/coverage-ledger.md missing generated candidate targets: {preview}")

    rows = reviewed_coverage_rows(body)
    if len(rows) < minimum:
        fail(f"reports/coverage-ledger.md has {len(rows)} reviewed rows, expected at least {minimum}")
    statuses = {status for status, _ in rows}
    if "verified" not in statuses:
        fail("reports/coverage-ledger.md must include at least one verified target")
    if "rejected" not in statuses:
        fail("reports/coverage-ledger.md must include rejected high-risk targets with evidence")
    if re.search(r"\|\s*(deferred|unknown|pending|todo|needs follow-up)\s*\|", lower):
        fail("reports/coverage-ledger.md cannot use deferred/unknown/pending/todo statuses for budget targets")


def verify_scan_waves() -> None:
    wave_files = sorted((WORK_ROOT / "plans").glob("scan-wave-*.md"))
    if not wave_files:
        fail("plans/ must contain at least scan-wave-001.md")
        return
    for path in wave_files:
        body = path.read_text(errors="replace").lower()
        for item in ["target_root", "source", "sink", "sanitizer", "prompt", "test"]:
            if item not in body:
                fail(f"{path.relative_to(WORK_ROOT)} missing expected scan planning field: {item}")


def verify_scan_completion() -> None:
    body = text("reports/scan-completion.md").lower()
    required = [
        "all source files inventoried",
        "candidate extraction ran over full manifest",
        "all attack-surface entries reviewed",
        "all sast candidates triaged",
        "all npm ast candidates triaged",
        "no unverified accepted hypotheses remain",
        "all runtime-verified vulnerabilities listed",
    ]
    for item in required:
        if item not in body:
            fail(f"reports/scan-completion.md missing completion assertion: {item}")


def verify_toolchain_capabilities() -> None:
    body = text("reports/toolchain-capabilities.md")
    lower = body.lower()
    for item in ["# verification toolchain capabilities", "- asan:", "- ubsan:", "## policy"]:
        if item not in lower:
            fail(f"reports/toolchain-capabilities.md missing required field: {item}")
    if "sanitizer unavailable" in "\n".join([
        text("reports/verification-output.txt"),
        text("vulnerability_report.md"),
    ]).lower():
        if "asan: unavailable" not in lower and "ubsan: unavailable" not in lower:
            fail("sanitizer-unavailable claims must be supported by reports/toolchain-capabilities.md")


def verify_verification_escalation() -> None:
    body = text("reports/verification-escalation.md")
    lower = body.lower()
    for item in ["# verification escalation", "## sanitizer attempts", "## runtime fallback options", "## mandatory escalation policy"]:
        if item not in lower:
            fail(f"reports/verification-escalation.md missing required field: {item}")
    if not re.search(r"\b(asan|ubsan).*\b(available|unavailable)\b", lower):
        fail("reports/verification-escalation.md must record ASAN/UBSAN escalation attempts")
    if "only reject after recording" not in lower:
        fail("reports/verification-escalation.md must include the reject-after-escalation policy")


def verify_runtime_entrypoints() -> None:
    body = text("reports/runtime-entrypoints.md").lower()
    for item in [
        "# runtime entrypoint map",
        "source files traversed:",
        "discovered runtime entrypoints:",
        "| kind | target | runtime surface | evidence | preferred verification |",
    ]:
        if item not in body:
            fail(f"reports/runtime-entrypoints.md missing required field: {item}")


def verify_npm_ast_candidates() -> None:
    body = text("reports/npm-ast-candidates.md").lower()
    for item in [
        "# npm ast candidate extraction",
        "source files considered:",
        "package:",
        "scanner source:",
        "scanner status:",
    ]:
        if item not in body:
            fail(f"reports/npm-ast-candidates.md missing required field: {item}")


def verify_result_output() -> None:
    body = text("result/output.md")
    for item in ["COMPLETED", "Final verification", "PASSED"]:
        if item not in body:
            fail(f"result/output.md missing final status marker: {item}")
    if "coverage" not in body.lower():
        fail("result/output.md must state coverage ledger status")


def main() -> int:
    global WORK_ROOT, TARGET_ROOT

    parser = argparse.ArgumentParser(description="Final verification gate for the vulnerability mining competition.")
    parser.add_argument("--work-root", default=None, help="path to the work output root (default: <repo>/work)")
    parser.add_argument("--target-root", default=None, help="path to the target source tree (default: auto-discovered)")
    args = parser.parse_args()

    WORK_ROOT = resolve_work_root(args.work_root)
    TARGET_ROOT = discover_target_root(args.target_root)

    print(f"[final_verify] WORK_ROOT={WORK_ROOT}")
    print(f"[final_verify] TARGET_ROOT={TARGET_ROOT}")

    verify_chat_log()
    verify_method_artifacts()
    verify_vulnerability_list()
    require_file("vulnerability_report.md")
    verify_run_test()
    verify_reports()
    verify_result_output()
    verify_no_premature_toolchain_stop()
    if FAILURES:
        print("FINAL_VERIFY_FAIL")
        for failure in FAILURES:
            print(f"- {failure}")
        return 1
    print("FINAL_VERIFY_PASS")
    return 0


if __name__ == "__main__":
    sys.exit(main())
