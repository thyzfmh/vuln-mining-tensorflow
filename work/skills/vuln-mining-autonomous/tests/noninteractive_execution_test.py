#!/usr/bin/env python3
"""Enforce the unattended, executor-workspace-only execution contract."""

from __future__ import annotations

import ast
import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[4]
SKILL = ROOT / "work/skills/vuln-mining-autonomous"
PRODUCTION_PYTHON = [
    ROOT / "work/run_vulnerability_mining.py",
    SKILL / "scripts/probe_verification_tools.py",
    SKILL / "scripts/escalate_verification_tools.py",
    SKILL / "scripts/npm_ast_candidates.py",
    SKILL / "templates/verify_run_test.py",
]
TEMP_USERS = {
    SKILL / "scripts/probe_verification_tools.py",
    SKILL / "scripts/escalate_verification_tools.py",
    SKILL / "templates/verify_run_test.py",
}


def dotted_name(node: ast.AST | None) -> str:
    if isinstance(node, ast.Name):
        return node.id
    if isinstance(node, ast.Attribute):
        prefix = dotted_name(node.value)
        return f"{prefix}.{node.attr}" if prefix else node.attr
    return ""


def main() -> None:
    instruction = (ROOT / "INSTRUCTION.md").read_text()
    for pattern in [r"/tmp(?:/|\b)", r"\bmktemp\b", r"python3?\s+-c\b", r"(?m)^\s*sudo\b"]:
        if re.search(pattern, instruction, re.I):
            raise SystemExit(f"INSTRUCTION.md contains approval-prone command pattern: {pattern}")
    for marker in ["EXECUTOR_ROOT", '$(dirname "$EXECUTOR_ROOT")/package', "不得请求授权", "子进程必须关闭标准输入"]:
        if marker not in instruction:
            raise SystemExit(f"INSTRUCTION.md missing unattended marker: {marker}")

    skill_text = (SKILL / "SKILL.md").read_text()
    for marker in [
        "Never request approval",
        "standard input closed",
        "inline Python commands",
        "work/verify/.tmp",
    ]:
        if marker not in skill_text:
            raise SystemExit(f"SKILL.md missing unattended rule: {marker}")

    for path in PRODUCTION_PYTHON:
        tree = ast.parse(path.read_text(), filename=str(path))
        for node in ast.walk(tree):
            if not isinstance(node, ast.Call):
                continue
            name = dotted_name(node.func)
            if name == "subprocess.run":
                keywords = {keyword.arg: keyword.value for keyword in node.keywords if keyword.arg}
                if dotted_name(keywords.get("stdin")) != "subprocess.DEVNULL":
                    raise SystemExit(f"{path.relative_to(ROOT)} has subprocess.run without closed stdin at line {node.lineno}")
            if path in TEMP_USERS and name == "tempfile.TemporaryDirectory":
                if not any(keyword.arg == "dir" for keyword in node.keywords):
                    raise SystemExit(f"{path.relative_to(ROOT)} uses a system temp directory at line {node.lineno}")

    template = (SKILL / "templates/verify_run_test.py").read_text()
    if re.search(r"sys\.executable\s*,\s*['\"]-c['\"]", template):
        raise SystemExit("verification template contains an inline Python subprocess")

    npm = (SKILL / "scripts/npm_ast_candidates.py").read_text()
    for marker in ["--yes", '"CI": "1"', '"npm_config_yes": "true"', '"npm_config_cache"']:
        if marker not in npm:
            raise SystemExit(f"npm scanner missing non-interactive marker: {marker}")

    print("noninteractive_execution_test.py: PASS")


if __name__ == "__main__":
    main()
