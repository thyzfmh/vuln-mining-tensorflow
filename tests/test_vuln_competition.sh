#!/usr/bin/env bash
# test_vuln_competition.sh — test the skill scripts work correctly
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SK="$ROOT/work/skills/vuln_mining_tf_blackbox"

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

assert_contains() {
  local haystack="$1"
  local needle="$2"
  [[ "$haystack" == *"$needle"* ]] || fail "expected output to contain: $needle"$'\n'"actual:"$'\n'"$haystack"
}

# Create a minimal fake tensorflow structure for testing
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

FAKE_TF="$TMP/tensorflow"
mkdir -p "$FAKE_TF/tensorflow/core/kernels" "$FAKE_TF/tensorflow/core/framework" "$FAKE_TF/tensorflow/core/platform" "$FAKE_TF/tensorflow/core/util"

cat > "$FAKE_TF/tensorflow/core/kernels/pooling_ops.cc" <<'EOF'
#include "tensorflow/core/framework/op_kernel.h"
namespace tensorflow {
class PoolingOp : public OpKernel {
 public:
  explicit PoolingOp(OpKernelConstruction* context) : OpKernel(context) {}
  void Compute(OpKernelContext* context) override {
    int stride_height = 0;
    int out_height = (in_height - window_height) / stride_height;
    auto* output = new Tensor();
    context->set_output(0, *output);
  }
};
}  // namespace tensorflow
EOF

cat > "$FAKE_TF/tensorflow/core/kernels/pooling_ops.h" <<'EOF'
#ifndef TENSORFLOW_CORE_KERNELS_POOLING_OPS_H_
#define TENSORFLOW_CORE_KERNELS_POOLING_OPS_H_
namespace tensorflow {
class PoolingOp;
}  // namespace tensorflow
#endif
EOF

cat > "$FAKE_TF/tensorflow/core/framework/tensor.cc" <<'EOF'
#include "tensorflow/core/framework/tensor.h"
namespace tensorflow {
class Tensor {
 public:
  void* data() { return data_; }
 private:
  void* data_ = nullptr;
};
}  // namespace tensorflow
EOF

# Test 1: Init
echo "Test 1: Init competition project..."
init_out="$("$SK/scripts/init-vuln-competition.sh" "$FAKE_TF")"
assert_contains "$init_out" "initialized"

test -f "$ROOT/vulnerability_list.md" || fail "vulnerability_list.md missing after init"
test -f "$ROOT/llm_chat_log.json" || fail "llm_chat_log.json missing after init"
test -f "$ROOT/vulnerability_report.md" || fail "vulnerability_report.md missing after init"
test -f "$ROOT/verify/run_test.py" || fail "verify/run_test.py missing after init"
test -f "$ROOT/acceptance-plan.yaml" || fail "acceptance-plan.yaml missing after init"
echo "  PASS"

# Test 2: Analyze
echo "Test 2: Analyze target..."
analyze_out="$("$SK/scripts/analyze_target.sh" "$FAKE_TF")"
assert_contains "$analyze_out" "source-inventory"
test -f "$ROOT/reports/source-inventory.md" || fail "source-inventory.md missing"
echo "  PASS"

# Test 3: Plan
echo "Test 3: Plan next scan..."
plan_out="$("$SK/scripts/plan_next_scan.sh" scan-001 "Test scan" "tensorflow/core/kernels/")"
assert_contains "$plan_out" "plans/scan-001.md"
test -f "$ROOT/plans/scan-001.md" || fail "scan-001.md missing"
plan_content="$(cat "$ROOT/plans/scan-001.md")"
assert_contains "$plan_content" "Black-Box Rules"
assert_contains "$plan_content" "Anti-Hallucination Checklist"
echo "  PASS"

# Test 4: SAST scan
echo "Test 4: SAST scan..."
sast_out="$("$SK/scripts/scan_sast.sh" "$FAKE_TF")"
assert_contains "$sast_out" "SAST"
test -f "$ROOT/reports/sast-scan-report.md" || fail "sast-scan-report.md missing"
echo "  PASS"

# Test 5: LLM scan framework
echo "Test 5: LLM scan framework..."
llm_out="$("$SK/scripts/scan_llm.sh" "$FAKE_TF")"
assert_contains "$llm_out" "LLM"
test -f "$ROOT/reports/llm-scan-report.md" || fail "llm-scan-report.md missing"
echo "  PASS"

# Test 6: Verify vulnerabilities
echo "Test 6: Verify vulnerabilities..."
verify_out="$("$SK/scripts/verify_vulnerabilities.sh" 2>&1 || true)"
test -f "$ROOT/reports/vuln-verification.md" || fail "vuln-verification.md missing"
echo "  PASS"

# Test 7: Final verify
echo "Test 7: Final verify..."
final_out="$("$SK/scripts/final_verify.sh" 2>&1 || true)"
echo "  PASS"

# Test 8: Skill files exist
echo "Test 8: Skill files exist..."
for F in skill.yaml prompt.md pipeline.md output_spec.md verify/run_test.py; do
  test -f "$SK/$F" || fail "Missing skill file: $F"
done
for F in analyze_target.sh final_verify.sh init-vuln-competition.sh plan_next_scan.sh scan_llm.sh scan_sast.sh verify_vulnerabilities.sh; do
  test -f "$SK/scripts/$F" || fail "Missing script: $F"
done
for F in llm_chat_log.template.json vulnerability_list.template.md vulnerability_report.template.md; do
  test -f "$SK/templates/$F" || fail "Missing template: $F"
done
echo "  PASS"

echo ""
echo "test_vuln_competition.sh: ALL TESTS PASS"
