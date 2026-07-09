#!/usr/bin/env python3
from __future__ import annotations

import json
import pathlib
import re
import sys

ROOT = pathlib.Path.cwd()
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


def fail(message: str) -> None:
    FAILURES.append(message)


def require_file(path: str) -> pathlib.Path:
    p = ROOT / path
    if not p.is_file():
        fail(f"missing required file: {path}")
    return p


def text(path: str) -> str:
    p = require_file(path)
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
    code_dir = ROOT / "code"
    if code_dir.is_dir():
        for child in code_dir.iterdir():
            if child.is_dir() and not child.name.startswith("."):
                name = re.escape(child.name.lower())
                forbidden.append(rf"(?<![a-z0-9_]){name}(?![a-z0-9_])")
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
        "reports/toolchain-capabilities.md",
        "reports/verification-escalation.md",
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
    all_paths = sorted(set(attack_paths + sast_paths))
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
    wave_files = sorted((ROOT / "plans").glob("scan-wave-*.md"))
    if not wave_files:
        fail("plans/ must contain at least scan-wave-001.md")
        return
    for path in wave_files:
        body = path.read_text(errors="replace").lower()
        for item in ["target_root", "source", "sink", "sanitizer", "prompt", "test"]:
            if item not in body:
                fail(f"{path.relative_to(ROOT)} missing expected scan planning field: {item}")


def verify_scan_completion() -> None:
    body = text("reports/scan-completion.md").lower()
    required = [
        "all source files inventoried",
        "candidate extraction ran over full manifest",
        "all attack-surface entries reviewed",
        "all sast candidates triaged",
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


def verify_result_output() -> None:
    body = text("result/output.md")
    for item in ["COMPLETED", "Final verification", "PASSED"]:
        if item not in body:
            fail(f"result/output.md missing final status marker: {item}")
    if "coverage" not in body.lower():
        fail("result/output.md must state coverage ledger status")


def main() -> int:
    verify_chat_log()
    verify_method_artifacts()
    verify_vulnerability_list()
    require_file("vulnerability_report.md")
    verify_run_test()
    verify_reports()
    verify_result_output()
    if FAILURES:
        print("FINAL_VERIFY_FAIL")
        for failure in FAILURES:
            print(f"- {failure}")
        return 1
    print("FINAL_VERIFY_PASS")
    return 0


if __name__ == "__main__":
    sys.exit(main())
