#!/usr/bin/env python3
from __future__ import annotations

import json
import pathlib
import re
import sys

ROOT = pathlib.Path.cwd()
FAILURES: list[str] = []


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
    if len(history) < 5:
        fail("llm_chat_log.json must contain at least 5 turns")
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


def verify_vulnerability_list() -> None:
    body = text("vulnerability_list.md")
    required = ["漏洞 #", "漏洞类型", "严重级别", "问题源码路径", "成因简述", "验证结果", "与LLM交互中哪句提示词发现了bug"]
    for item in required:
        if item not in body:
            fail(f"vulnerability_list.md missing section: {item}")
    if body.count("漏洞 #") < 1:
        fail("vulnerability_list.md must include at least one vulnerability")
    if not re.search(r"\b(VERIFIED|CRASH|EXCEPTION|SIGNAL)\b", body, re.I):
        fail("vulnerability_list.md must include runtime verification evidence")


def verify_run_test() -> None:
    body = text("verify/run_test.py")
    if "AI-generated" not in body and "AI generated" not in body:
        fail("verify/run_test.py must identify tests as AI-generated")
    if not re.search(r"def\s+test_", body):
        fail("verify/run_test.py must contain at least one test_ function")
    for item in ["TODO", "placeholder", "pass  #", "NotImplemented"]:
        if item.lower() in body.lower():
            fail(f"verify/run_test.py contains placeholder marker: {item}")


def verify_reports() -> None:
    body = text("reports/verification-output.txt")
    if not re.search(r"\b(VERIFIED|CRASH|EXCEPTION|SIGNAL)\b", body, re.I):
        fail("reports/verification-output.txt must contain runtime evidence")
    report = text("vulnerability_report.md")
    for item in ["Black-Box", "method", "verification", "reproduc"]:
        if item.lower() not in report.lower():
            fail(f"vulnerability_report.md missing expected discussion: {item}")


def verify_result_output() -> None:
    body = text("result/output.md")
    for item in ["COMPLETED", "Final verification", "PASSED"]:
        if item not in body:
            fail(f"result/output.md missing final status marker: {item}")


def main() -> int:
    verify_chat_log()
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
