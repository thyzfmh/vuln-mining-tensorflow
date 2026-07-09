#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../../../.." && pwd)"

"$ROOT/work/skills/vuln-mining-autonomous/tests/package_check.sh"

VALIDATOR="/Users/tanghui/.codex/skills/.system/skill-creator/scripts/quick_validate.py"
if [ -f "$VALIDATOR" ]; then
  python3 "$VALIDATOR" "$ROOT/work/skills/vuln-mining-autonomous"
else
  echo "WARN: skill validator not found at $VALIDATOR"
fi

echo "self_check.sh: PASS"
