#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../../../.." && pwd)"

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

assert_contains() {
  local haystack="$1"
  local needle="$2"
  [[ "$haystack" == *"$needle"* ]] || fail "expected text to contain: $needle"
}

expected_skill_dir="$ROOT/work/skills/vuln-mining-autonomous"
skill_dirs="$(find "$ROOT/work/skills" -mindepth 1 -maxdepth 1 -type d | sort)"
if [ "$skill_dirs" != "$expected_skill_dir" ]; then
  fail "work/skills must contain only vuln-mining-autonomous"$'\n'"actual:"$'\n'"$skill_dirs"
fi

skill_files="$(find "$ROOT/work/skills" -name SKILL.md -type f | sort)"
if [ "$skill_files" != "$expected_skill_dir/SKILL.md" ]; then
  fail "expected only one SKILL.md at work/skills/vuln-mining-autonomous/SKILL.md"
fi

# Root package must not contain runtime artifact directories
for dir in "$ROOT/.agents" "$ROOT/.agent" "$ROOT/config" "$ROOT/templates" "$ROOT/scripts" "$ROOT/tests" "$ROOT/harness" "$ROOT/plans" "$ROOT/reports" "$ROOT/verify"; do
  [ ! -e "$dir" ] || fail "root package should not contain $(basename "$dir")"
done

# work/run_vulnerability_mining.py must exist as the entry orchestrator
[ -f "$ROOT/work/run_vulnerability_mining.py" ] || fail "missing work/run_vulnerability_mining.py"

# platform_assets.py must exist
[ -f "$expected_skill_dir/scripts/platform_assets.py" ] || fail "missing scripts/platform_assets.py"

instruction_text="$(cat "$ROOT/INSTRUCTION.md")"
assert_contains "$instruction_text" "work/skills/vuln-mining-autonomous/SKILL.md"
assert_contains "$instruction_text" "Skill 名称：vuln-mining-autonomous"
assert_contains "$instruction_text" "VULN_TARGET_ROOT"
assert_contains "$instruction_text" "只读目录"
assert_contains "$instruction_text" "cp -R"
assert_contains "$instruction_text" 'FINAL_VERIFY_PASS'

agents_text="$(cat "$ROOT/AGENTS.md")"
assert_contains "$agents_text" "work/"
assert_contains "$agents_text" "platform asset discovery"

readme_text="$(cat "$ROOT/README.md")"
assert_contains "$readme_text" "work/"
assert_contains "$readme_text" "platform asset discovery"

# Black-box forbidden wording
for file in "$ROOT/INSTRUCTION.md" "$ROOT/AGENTS.md" "$ROOT/README.md" "$expected_skill_dir/SKILL.md"; do
  if grep -Eiq 'v2\.?11|known[[:space:]_-]*bug|known[[:space:]_-]*vulnerability|CVE-[0-9]' "$file"; then
    fail "black-box forbidden wording found in ${file#$ROOT/}"
  fi
done

# Target directory names must not appear in docs
if [ -d "$ROOT/code" ]; then
  for target_dir in "$ROOT/code"/*; do
    [ -d "$target_dir" ] || continue
    target_name="$(basename "$target_dir")"
    [ "$target_name" = "*" ] && continue
    for file in "$ROOT/INSTRUCTION.md" "$ROOT/AGENTS.md" "$ROOT/README.md" "$expected_skill_dir/SKILL.md"; do
      if grep -Fqi "$target_name" "$file"; then
        fail "target directory name appears in ${file#$ROOT/}"
      fi
    done
  done
fi

# Required skill resources
for file in \
  "$expected_skill_dir/scripts/platform_assets.py" \
  "$expected_skill_dir/scripts/source_inventory.py" \
  "$expected_skill_dir/scripts/attack_surface_map.py" \
  "$expected_skill_dir/scripts/sast_candidates.py" \
  "$expected_skill_dir/scripts/npm_ast_candidates.py" \
  "$expected_skill_dir/scripts/init_coverage_ledger.py" \
  "$expected_skill_dir/scripts/probe_verification_tools.py" \
  "$expected_skill_dir/scripts/escalate_verification_tools.py" \
  "$expected_skill_dir/scripts/runtime_entrypoints.py" \
  "$expected_skill_dir/scripts/final_verify.py" \
  "$expected_skill_dir/scripts/self_check.sh" \
  "$expected_skill_dir/references/method-cards.md" \
  "$expected_skill_dir/templates/llm_chat_log.json" \
  "$expected_skill_dir/templates/vulnerability_list.md" \
  "$expected_skill_dir/templates/vulnerability_report.md" \
  "$expected_skill_dir/templates/verify_run_test.py"
do
  [ -f "$file" ] || fail "missing skill resource: ${file#$ROOT/}"
done

python3 -m py_compile \
  "$ROOT/work/run_vulnerability_mining.py" \
  "$expected_skill_dir/scripts/platform_assets.py" \
  "$expected_skill_dir/scripts/source_inventory.py" \
  "$expected_skill_dir/scripts/attack_surface_map.py" \
  "$expected_skill_dir/scripts/sast_candidates.py" \
  "$expected_skill_dir/scripts/npm_ast_candidates.py" \
  "$expected_skill_dir/scripts/init_coverage_ledger.py" \
  "$expected_skill_dir/scripts/probe_verification_tools.py" \
  "$expected_skill_dir/scripts/escalate_verification_tools.py" \
  "$expected_skill_dir/scripts/runtime_entrypoints.py" \
  "$expected_skill_dir/scripts/final_verify.py"
rm -rf "$expected_skill_dir/scripts/__pycache__"

# Run tests
python3 "$expected_skill_dir/tests/platform_asset_discovery_test.py"
python3 "$expected_skill_dir/tests/work_output_layout_test.py"
python3 "$expected_skill_dir/tests/runtime_entrypoints_smoke_test.py"
python3 "$expected_skill_dir/tests/platform_execution_smoke_test.py"
python3 "$expected_skill_dir/tests/submission_archive_test.py"

bash -n "$expected_skill_dir/scripts/self_check.sh"
python3 -m json.tool "$expected_skill_dir/templates/llm_chat_log.json" >/dev/null

echo "package_check.sh: PASS"
