#!/usr/bin/env python3
"""Tests for work output layout.

Proves that all runtime artifacts (reports, plans, verify, deliverables,
result) are rooted at the work output root — never the repository root —
and that the final gate runs against the work root with an externally
supplied target root.
"""
from __future__ import annotations

import json
import os
import pathlib
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[4]
SCRIPT_DIR = ROOT / "work" / "skills" / "vuln-mining-autonomous" / "scripts"
sys.path.insert(0, str(SCRIPT_DIR))


def write(path: pathlib.Path, body: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(body)


def run_script(name: str, env: dict[str, str]) -> subprocess.CompletedProcess[str]:
    script = SCRIPT_DIR / name
    return subprocess.run(
        [sys.executable, str(script)],
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=60,
    )


def test_source_inventory_writes_under_work():
    """source_inventory.py writes reports under WORK_ROOT, not repo root."""
    with tempfile.TemporaryDirectory() as td:
        target = pathlib.Path(td) / "target"
        write(target / "src" / "main.c", "int main() { return 0; }\n")
        work = pathlib.Path(td) / "work"

        env = os.environ.copy()
        env["VULN_TARGET_ROOT"] = str(target)
        env["VULN_WORK_ROOT"] = str(work)

        proc = run_script("source_inventory.py", env)
        assert proc.returncode == 0, f"source_inventory.py failed:\n{proc.stdout}\n{proc.stderr}"

        assert (work / "reports" / "source-inventory.md").is_file(), "report not under work/"
        assert (work / "reports" / "source-file-manifest.md").is_file(), "manifest not under work/"
        # Nothing should be at the temp dir root (which would be "repo root")
        assert not (pathlib.Path(td) / "reports").exists(), "report leaked to repo root"
    print("test_source_inventory_writes_under_work: PASS")


def test_attack_surface_map_writes_under_work():
    with tempfile.TemporaryDirectory() as td:
        target = pathlib.Path(td) / "target"
        write(target / "parse.cc", "int main() { memcpy(0,0,0); return 0; }\n")
        work = pathlib.Path(td) / "work"

        env = os.environ.copy()
        env["VULN_TARGET_ROOT"] = str(target)
        env["VULN_WORK_ROOT"] = str(work)

        proc = run_script("attack_surface_map.py", env)
        assert proc.returncode == 0, f"attack_surface_map.py failed:\n{proc.stdout}\n{proc.stderr}"
        assert (work / "reports" / "attack-surface-map.md").is_file()
        assert not (pathlib.Path(td) / "reports").exists()
    print("test_attack_surface_map_writes_under_work: PASS")


def test_sast_candidates_writes_under_work():
    with tempfile.TemporaryDirectory() as td:
        target = pathlib.Path(td) / "target"
        write(target / "risk.c", "int x = 1 / y;\n")
        work = pathlib.Path(td) / "work"

        env = os.environ.copy()
        env["VULN_TARGET_ROOT"] = str(target)
        env["VULN_WORK_ROOT"] = str(work)

        proc = run_script("sast_candidates.py", env)
        assert proc.returncode == 0, f"sast_candidates.py failed:\n{proc.stdout}\n{proc.stderr}"
        assert (work / "reports" / "sast-candidates.md").is_file()
        assert not (pathlib.Path(td) / "reports").exists()
    print("test_sast_candidates_writes_under_work: PASS")


def test_runtime_entrypoints_writes_under_work():
    with tempfile.TemporaryDirectory() as td:
        target = pathlib.Path(td) / "target"
        write(target / "tool.cc", "int main() { return 0; }\nTEST(A, B) {}\n")
        work = pathlib.Path(td) / "work"

        env = os.environ.copy()
        env["VULN_TARGET_ROOT"] = str(target)
        env["VULN_WORK_ROOT"] = str(work)

        proc = run_script("runtime_entrypoints.py", env)
        assert proc.returncode == 0, f"runtime_entrypoints.py failed:\n{proc.stdout}\n{proc.stderr}"
        assert (work / "reports" / "runtime-entrypoints.md").is_file()
        assert not (pathlib.Path(td) / "reports").exists()
    print("test_runtime_entrypoints_writes_under_work: PASS")


def test_probe_verification_tools_writes_under_work():
    with tempfile.TemporaryDirectory() as td:
        work = pathlib.Path(td) / "work"
        env = os.environ.copy()
        env["VULN_WORK_ROOT"] = str(work)
        env.pop("VULN_TARGET_ROOT", None)
        env.pop("TARGET_ROOT", None)

        proc = run_script("probe_verification_tools.py", env)
        assert proc.returncode == 0, f"probe_verification_tools.py failed:\n{proc.stdout}\n{proc.stderr}"
        assert (work / "reports" / "toolchain-capabilities.md").is_file()
        assert not (pathlib.Path(td) / "reports").exists()
    print("test_probe_verification_tools_writes_under_work: PASS")


def test_init_coverage_ledger_writes_under_work():
    with tempfile.TemporaryDirectory() as td:
        target = pathlib.Path(td) / "target"
        write(target / "a.c", "int main() { return 0; }\n")
        work = pathlib.Path(td) / "work"

        env = os.environ.copy()
        env["VULN_TARGET_ROOT"] = str(target)
        env["VULN_WORK_ROOT"] = str(work)

        for script in ["source_inventory.py", "attack_surface_map.py", "sast_candidates.py", "npm_ast_candidates.py"]:
            proc = run_script(script, env)
            assert proc.returncode == 0, f"{script} failed:\n{proc.stdout}\n{proc.stderr}"

        proc = run_script("init_coverage_ledger.py", env)
        assert proc.returncode == 0, f"init_coverage_ledger.py failed:\n{proc.stdout}\n{proc.stderr}"
        assert (work / "reports" / "coverage-ledger.md").is_file()
        assert not (pathlib.Path(td) / "reports").exists()
    print("test_init_coverage_ledger_writes_under_work: PASS")


def test_final_verify_accepts_work_and_target_args():
    """final_verify.py accepts --work-root and --target-root and checks files under work/."""
    with tempfile.TemporaryDirectory() as td:
        target = pathlib.Path(td) / "target"
        write(target / "secret_pkg" / "main.c", "int main() { return 0; }\n")
        work = pathlib.Path(td) / "work"

        env = os.environ.copy()
        env["VULN_TARGET_ROOT"] = str(target)
        env["VULN_WORK_ROOT"] = str(work)

        # Run Phase 1 scripts to populate reports
        for script in [
            "source_inventory.py", "probe_verification_tools.py",
            "escalate_verification_tools.py", "runtime_entrypoints.py",
            "attack_surface_map.py", "sast_candidates.py",
            "npm_ast_candidates.py", "init_coverage_ledger.py",
        ]:
            proc = run_script(script, env)
            assert proc.returncode == 0, f"{script} failed:\n{proc.stdout}\n{proc.stderr}"

        # Run final_verify — it should fail (no deliverables yet) but it must
        # run against the work root, not the repo root.
        final_verify = SCRIPT_DIR / "final_verify.py"
        proc = subprocess.run(
            [sys.executable, str(final_verify), "--work-root", str(work), "--target-root", str(target)],
            env=env,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            timeout=30,
        )
        # It will fail because deliverables are missing, but the failure must
        # be about missing files under work/, not a crash.
        assert proc.returncode == 1, f"expected final_verify to fail (rc=1) but got rc={proc.returncode}"
        assert "FINAL_VERIFY_FAIL" in proc.stdout
        assert "missing required file" in proc.stdout
        # Ensure nothing leaked to the temp dir root
        assert not (pathlib.Path(td) / "reports").exists()
        assert not (pathlib.Path(td) / "vulnerability_list.md").exists()
    print("test_final_verify_accepts_work_and_target_args: PASS")


def test_final_verify_work_root_not_repo_root():
    """final_verify.py does NOT read from the repo root when --work-root is set."""
    with tempfile.TemporaryDirectory() as td:
        target = pathlib.Path(td) / "target"
        write(target / "pkg" / "a.py", "x = 1\n")
        work = pathlib.Path(td) / "work"

        env = os.environ.copy()
        env["VULN_TARGET_ROOT"] = str(target)
        env["VULN_WORK_ROOT"] = str(work)

        # Create a fake llm_chat_log.json at the "repo root" (td) that should NOT be read
        write(pathlib.Path(td) / "llm_chat_log.json", json.dumps({"chat_history": []}))
        # The work root has no llm_chat_log.json
        final_verify = SCRIPT_DIR / "final_verify.py"
        proc = subprocess.run(
            [sys.executable, str(final_verify), "--work-root", str(work), "--target-root", str(target)],
            env=env,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            timeout=30,
        )
        # Should report missing llm_chat_log.json under work/, not find the one at td/
        assert "missing required file: llm_chat_log.json" in proc.stdout
    print("test_final_verify_work_root_not_repo_root: PASS")


def test_black_box_no_target_name_in_reports():
    """Reports use TARGET_ROOT alias, not the real target directory name."""
    with tempfile.TemporaryDirectory() as td:
        target = pathlib.Path(td) / "real_secret_target"
        write(target / "main.c", "int main() { return 0; }\n")
        work = pathlib.Path(td) / "work"

        env = os.environ.copy()
        env["VULN_TARGET_ROOT"] = str(target)
        env["VULN_WORK_ROOT"] = str(work)

        proc = run_script("source_inventory.py", env)
        assert proc.returncode == 0

        inventory = (work / "reports" / "source-inventory.md").read_text()
        assert "TARGET_ROOT" in inventory
        assert "real_secret_target" not in inventory, "target name leaked into report"
    print("test_black_box_no_target_name_in_reports: PASS")


def test_run_vulnerability_mining_persists_context():
    """The orchestrator writes a private target context under its work root."""
    with tempfile.TemporaryDirectory() as td:
        target = pathlib.Path(td) / "judge_target"
        write(target / "main.py", "x = 1\n")
        work = pathlib.Path(td) / "work"

        runner = ROOT / "work" / "run_vulnerability_mining.py"
        env = os.environ.copy()
        env["VULN_MINING_TARGET_ROOT"] = str(target)
        proc = subprocess.run(
            [sys.executable, str(runner), "--output-root", str(work), "--skip-npm"],
            env=env,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            timeout=30,
        )
        assert proc.returncode == 0, f"orchestrator failed:\n{proc.stdout}"
        context = work / ".vuln-mining-target-root"
        assert context.read_text().strip() == str(target.resolve())
        assert target.name not in proc.stdout
    print("test_run_vulnerability_mining_persists_context: PASS")


def main() -> None:
    test_source_inventory_writes_under_work()
    test_attack_surface_map_writes_under_work()
    test_sast_candidates_writes_under_work()
    test_runtime_entrypoints_writes_under_work()
    test_probe_verification_tools_writes_under_work()
    test_init_coverage_ledger_writes_under_work()
    test_final_verify_accepts_work_and_target_args()
    test_final_verify_work_root_not_repo_root()
    test_black_box_no_target_name_in_reports()
    test_run_vulnerability_mining_persists_context()
    print("work_output_layout_test.py: PASS")


if __name__ == "__main__":
    main()
