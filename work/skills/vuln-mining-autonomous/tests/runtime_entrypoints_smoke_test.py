#!/usr/bin/env python3
from __future__ import annotations

import os
import pathlib
import runpy
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
        target = workspace / "code" / "sample"
        write(target / "CMakeLists.txt", "add_executable(tool tool.cc)\nadd_test(NAME smoke COMMAND tool)\n")
        write(target / "pkg" / "BUILD", "cc_test(\n    name = \"native_case\",\n)\npy_binary(\n    name = \"runner\",\n)\n")
        write(target / "tool.cc", "int main() { return 0; }\nTEST(Parser, RejectsBadInput) {}\nPYBIND11_MODULE(sample, m) {}\n")
        write(target / "test_tool.py", "def test_parser():\n    pass\n\nif __name__ == \"__main__\":\n    pass\n")

        proc = subprocess.run(
            [sys.executable, str(SCRIPT)],
            cwd=workspace,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=30,
        )
        if proc.returncode != 0:
            raise SystemExit(f"runtime_entrypoints.py failed:\n{proc.stdout}\n{proc.stderr}")

        report = (workspace / "reports" / "runtime-entrypoints.md").read_text()
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
            workspace / "reports" / "npm-ast-candidates.md",
            "# NPM AST Candidate Extraction\n\n"
            "- Source files considered: 1\n"
            "- Package: `@ast-grep/cli@0.44.1`\n"
            "- Scanner source: npx\n"
            "- Scanner status: completed\n",
        )

        old_cwd = pathlib.Path.cwd()
        try:
            os.chdir(workspace)
            final_verify = runpy.run_path(str(FINAL_VERIFY))
            final_verify["verify_runtime_entrypoints"]()
            final_verify["verify_npm_ast_candidates"]()
            if final_verify["FAILURES"]:
                raise SystemExit(f"runtime entrypoint final-gate check failed: {final_verify['FAILURES']}")
        finally:
            os.chdir(old_cwd)
    print("runtime_entrypoints_smoke_test.py: PASS")


if __name__ == "__main__":
    main()
