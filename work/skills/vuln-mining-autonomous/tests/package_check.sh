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

for dir in "$ROOT/.agents" "$ROOT/.agent" "$ROOT/config" "$ROOT/templates" "$ROOT/scripts" "$ROOT/tests" "$ROOT/harness" "$ROOT/plans" "$ROOT/reports" "$ROOT/verify"; do
  [ ! -e "$dir" ] || fail "root package should not contain $(basename "$dir")"
done

instruction_text="$(cat "$ROOT/INSTRUCTION.md")"
assert_contains "$instruction_text" "work/skills/vuln-mining-autonomous/SKILL.md"
assert_contains "$instruction_text" "Skill 名称：vuln-mining-autonomous"

for file in "$ROOT/INSTRUCTION.md" "$ROOT/AGENTS.md" "$ROOT/README.md" "$expected_skill_dir/SKILL.md"; do
  if grep -Eiq 'v2\.?11|known[[:space:]_-]*bug|known[[:space:]_-]*vulnerability|CVE-[0-9]' "$file"; then
    fail "black-box forbidden wording found in ${file#$ROOT/}"
  fi
done

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

for file in \
  "$expected_skill_dir/scripts/source_inventory.py" \
  "$expected_skill_dir/scripts/attack_surface_map.py" \
  "$expected_skill_dir/scripts/sast_candidates.py" \
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
  "$expected_skill_dir/scripts/source_inventory.py" \
  "$expected_skill_dir/scripts/attack_surface_map.py" \
  "$expected_skill_dir/scripts/sast_candidates.py" \
  "$expected_skill_dir/scripts/final_verify.py"
rm -rf "$expected_skill_dir/scripts/__pycache__"
bash -n "$expected_skill_dir/scripts/self_check.sh"
python3 -m json.tool "$expected_skill_dir/templates/llm_chat_log.json" >/dev/null

echo "package_check.sh: PASS"
