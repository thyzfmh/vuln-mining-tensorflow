#!/usr/bin/env python3
"""Verify that judge key paths are present in the submitted Git archive."""

from __future__ import annotations

import io
import importlib.util
import json
import os
import pathlib
import subprocess
import sys
import tarfile
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[4]
REQUIRED = [
    "work/vulnerability_list.md",
    "work/llm_chat_log.json",
    "work/vulnerability_report.md",
    "work/verify/run_test.py",
    "work/reports/source-inventory.md",
    "work/reports/source-file-manifest.md",
    "work/reports/attack-surface-map.md",
    "work/reports/sast-candidates.md",
    "work/reports/npm-ast-candidates.md",
    "work/reports/toolchain-capabilities.md",
    "work/reports/verification-escalation.md",
    "work/reports/runtime-entrypoints.md",
    "work/reports/coverage-ledger.md",
    "work/reports/scan-completion.md",
    "work/reports/hypotheses.md",
    "work/reports/verification-output.txt",
    "work/plans/scan-wave-001.md",
    "work/result/output.md",
]


def run(argv: list[str], **kwargs) -> subprocess.CompletedProcess:
    return subprocess.run(argv, cwd=ROOT, check=False, **kwargs)


def main() -> None:
    missing = [path for path in REQUIRED if not (ROOT / path).is_file()]
    if missing:
        raise SystemExit(f"missing submission key paths: {', '.join(missing)}")

    ignored = run(
        ["git", "check-ignore", "--no-index", "--stdin"],
        input="\n".join(REQUIRED) + "\n",
        text=True,
        stdout=subprocess.PIPE,
    ).stdout.splitlines()
    if ignored:
        raise SystemExit(f"submission key paths are ignored: {', '.join(ignored)}")

    tree = run(["git", "write-tree"], stdout=subprocess.PIPE, text=True)
    if tree.returncode != 0:
        raise SystemExit("could not build a submission tree from the Git index")
    archive = run(["git", "archive", "--format=tar", tree.stdout.strip()], stdout=subprocess.PIPE)
    if archive.returncode != 0:
        raise SystemExit("could not create a submission archive")
    with tarfile.open(fileobj=io.BytesIO(archive.stdout), mode="r:") as bundle:
        archived = set(bundle.getnames())
    absent = [path for path in REQUIRED if path not in archived]
    if absent:
        raise SystemExit(f"key paths absent from Git archive: {', '.join(absent)}")

    json.loads((ROOT / "work/llm_chat_log.json").read_text())
    compile_check = run([sys.executable, "-m", "py_compile", "work/verify/run_test.py"])
    if compile_check.returncode != 0:
        raise SystemExit("work/verify/run_test.py is not valid Python")

    with tempfile.TemporaryDirectory() as td:
        temp = pathlib.Path(td)
        target = temp / "TARGET_ROOT"
        target.mkdir()
        (target / "probe.c").write_text("int main(void) { return 0; }\n")

        spec = importlib.util.spec_from_file_location(
            "run_vulnerability_mining",
            ROOT / "work/run_vulnerability_mining.py",
        )
        if spec is None or spec.loader is None:
            raise SystemExit("could not load the entry orchestrator")
        launcher = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(launcher)
        prepared = temp / "prepared-work"
        for relative in [
            "vulnerability_list.md",
            "vulnerability_report.md",
            "llm_chat_log.json",
            "verify/run_test.py",
        ]:
            path = prepared / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("BOOTSTRAP_PENDING\n")
        launcher.prepare_output_root(prepared, target)
        stale = [
            relative
            for relative in [
                "vulnerability_list.md",
                "vulnerability_report.md",
                "llm_chat_log.json",
                "verify/run_test.py",
            ]
            if "BOOTSTRAP_PENDING" in (prepared / relative).read_text()
        ]
        if stale:
            raise SystemExit(f"orchestrator did not refresh bootstrap files: {', '.join(stale)}")

        ledger_work = temp / "ledger-work"
        reports = ledger_work / "reports"
        reports.mkdir(parents=True)
        candidate = "### `parser.cc`\n- Domain: parser\n"
        for name in ["attack-surface-map.md", "sast-candidates.md", "npm-ast-candidates.md"]:
            (reports / name).write_text(candidate)
        (reports / "coverage-ledger.md").write_text("bootstrap-state: pending\n")
        env = dict(os.environ, VULN_WORK_ROOT=str(ledger_work))
        ledger = run(
            [sys.executable, "work/skills/vuln-mining-autonomous/scripts/init_coverage_ledger.py"],
            env=env,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
        )
        ledger_body = (reports / "coverage-ledger.md").read_text()
        if ledger.returncode != 0 or "bootstrap-state: pending" in ledger_body or "TARGET_ROOT/parser.cc" not in ledger_body:
            raise SystemExit(f"coverage ledger bootstrap was not replaced:\n{ledger.stdout}")

        verify = run(
            [
                sys.executable,
                "work/skills/vuln-mining-autonomous/scripts/final_verify.py",
                "--work-root",
                "work",
                "--target-root",
                str(target),
            ],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
        )
    if verify.returncode == 0 or "FINAL_VERIFY_FAIL" not in verify.stdout:
        raise SystemExit("untouched bootstrap artifacts must fail final verification")

    print("submission_archive_test.py: PASS")


if __name__ == "__main__":
    main()
