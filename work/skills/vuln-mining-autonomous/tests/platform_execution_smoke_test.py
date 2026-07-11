#!/usr/bin/env python3
from __future__ import annotations

import pathlib
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[4]
LAUNCHER = ROOT / "work" / "run_vulnerability_mining.py"


def write(path: pathlib.Path, body: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(body)


def main() -> None:
    with tempfile.TemporaryDirectory() as td:
        temp = pathlib.Path(td)
        assets = temp / "judge-assets" / "challenge" / "source"
        write(assets / "WORKSPACE", "workspace(name = \"sample\")\n")
        write(assets / "kernel.cc", "int main() { return 0; }\n")
        output = temp / "executor" / "work"
        proc = subprocess.run(
            [sys.executable, str(LAUNCHER), "--asset-root", str(temp / "judge-assets"), "--output-root", str(output), "--skip-npm"],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=60,
        )
        if proc.returncode != 0:
            raise SystemExit(f"launcher failed:\n{proc.stdout}\n{proc.stderr}")
        context = output / ".vuln-mining-target-root"
        if not context.is_file() or pathlib.Path(context.read_text().strip()) != assets.resolve():
            raise SystemExit("launcher did not persist the discovered judge asset context under work/")
        for relative in [
            "vulnerability_list.md",
            "llm_chat_log.json",
            "vulnerability_report.md",
            "verify/run_test.py",
            "reports/source-file-manifest.md",
            "reports/toolchain-capabilities.md",
            "reports/verification-escalation.md",
            "reports/runtime-entrypoints.md",
            "reports/npm-ast-candidates.md",
            "reports/coverage-ledger.md",
        ]:
            if not (output / relative).is_file():
                raise SystemExit(f"missing work output: {relative}")
        if (temp / "executor" / "reports").exists():
            raise SystemExit("runtime reports escaped the work output root")
        subprocess.run(
            [sys.executable, str(ROOT / "work" / "skills" / "vuln-mining-autonomous" / "scripts" / "source_inventory.py")],
            cwd=output,
            check=True,
            timeout=30,
        )
    print("platform_execution_smoke_test.py: PASS")


if __name__ == "__main__":
    main()
