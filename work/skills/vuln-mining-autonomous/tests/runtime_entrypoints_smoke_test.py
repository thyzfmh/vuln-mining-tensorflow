#!/usr/bin/env python3
from __future__ import annotations

import os
import pathlib
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[4]
SCRIPT = ROOT / "work" / "skills" / "vuln-mining-autonomous" / "scripts" / "runtime_entrypoints.py"
FINAL_VERIFY = ROOT / "work" / "skills" / "vuln-mining-autonomous" / "scripts" / "final_verify.py"


def write(path: pathlib.Path, body: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(body)


def main() -> None:
    with tempfile.TemporaryDirectory() as td:
        workspace = pathlib.Path(td)
        target = workspace / "target_src"
        work = workspace / "work"
        write(target / "CMakeLists.txt", "add_executable(tool tool.cc)\nadd_test(NAME smoke COMMAND tool)\n")
        write(target / "pkg" / "BUILD", "cc_test(\n    name = \"native_case\",\n)\npy_binary(\n    name = \"runner\",\n)\n")
        write(target / "tool.cc", "int main() { return 0; }\nTEST(Parser, RejectsBadInput) {}\nPYBIND11_MODULE(sample, m) {}\n")
        write(target / "test_tool.py", "def test_parser():\n    pass\n\nif __name__ == \"__main__\":\n    pass\n")

        env = os.environ.copy()
        env["VULN_TARGET_ROOT"] = str(target)
        env["VULN_WORK_ROOT"] = str(work)

        proc = subprocess.run(
            [sys.executable, str(SCRIPT)],
            env=env,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=30,
        )
        if proc.returncode != 0:
            raise SystemExit(f"runtime_entrypoints.py failed:\n{proc.stdout}\n{proc.stderr}")

        report = (work / "reports" / "runtime-entrypoints.md").read_text()
        for expected in [
            "native-test",
            "python-test",
            "native-cli",
            "python-cli",
            "extension-module",
            "build-test-target",
            "build-binary-target",
            "cmake-test-target",
            "cmake-binary-target",
        ]:
            if expected not in report:
                raise SystemExit(f"missing expected entrypoint kind: {expected}")
        write(
            work / "reports" / "npm-ast-candidates.md",
            "# NPM AST Candidate Extraction\n\n"
            "- Source files considered: 1\n"
            "- Package: `@ast-grep/cli@0.44.1`\n"
            "- Scanner source: npx\n"
            "- Scanner status: completed\n",
        )

        # Test final_verify individual functions with work root
        sys.path.insert(0, str(SCRIPT.parent))
        import platform_assets
        platform_assets.WORK_ROOT = work  # not used; final_verify uses globals
        # Import final_verify and set its globals
        import importlib
        if "final_verify" in sys.modules:
            del sys.modules["final_verify"]
        final_verify = importlib.import_module("final_verify")
        final_verify.WORK_ROOT = work
        final_verify.TARGET_ROOT = target
        final_verify.verify_runtime_entrypoints()
        final_verify.verify_npm_ast_candidates()
        if final_verify.FAILURES:
            raise SystemExit(f"runtime entrypoint final-gate check failed: {final_verify.FAILURES}")
    print("runtime_entrypoints_smoke_test.py: PASS")


if __name__ == "__main__":
    main()
