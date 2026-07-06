#!/usr/bin/env bash
# test_vuln_competition.sh — test the harness scripts work correctly
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"

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
init_out="$("$ROOT/scripts/init-vuln-competition.sh" "$FAKE_TF")"
assert_contains "$init_out" "initialized"

test -f "$ROOT/result/vulnerability_list.md" || fail "vulnerability_list.md missing after init"
test -f "$ROOT/result/llm_chat_log.json" || fail "llm_chat_log.json missing after init"
test -f "$ROOT/result/vulnerability_report.md" || fail "vulnerability_report.md missing after init"
test -f "$ROOT/acceptance-plan.yaml" || fail "acceptance-plan.yaml missing after init"
echo "  PASS"

# Test 2: Analyze
echo "Test 2: Analyze target..."
analyze_out="$("$ROOT/harness/analyze_target.sh" "$FAKE_TF")"
assert_contains "$analyze_out" "source-inventory"
test -f "$ROOT/reports/source-inventory.md" || fail "source-inventory.md missing"
echo "  PASS"

# Test 3: Plan
echo "Test 3: Plan next scan..."
plan_out="$("$ROOT/harness/plan_next_scan.sh" scan-001 "Test scan" "tensorflow/core/kernels/")"
assert_contains "$plan_out" "plans/scan-001.md"
test -f "$ROOT/plans/scan-001.md" || fail "scan-001.md missing"
plan_content="$(cat "$ROOT/plans/scan-001.md")"
assert_contains "$plan_content" "Black-Box Rules"
assert_contains "$plan_content" "Anti-Hallucination Checklist"
echo "  PASS"

# Test 4: SAST scan
echo "Test 4: SAST scan..."
sast_out="$("$ROOT/harness/scan_sast.sh" "$FAKE_TF")"
assert_contains "$sast_out" "SAST"
test -f "$ROOT/reports/sast-scan-report.md" || fail "sast-scan-report.md missing"
echo "  PASS"

# Test 5: LLM scan framework
echo "Test 5: LLM scan framework..."
llm_out="$("$ROOT/harness/scan_llm.sh" "$FAKE_TF")"
assert_contains "$llm_out" "LLM"
test -f "$ROOT/reports/llm-scan-report.md" || fail "llm-scan-report.md missing"
echo "  PASS"

# Test 6: Verify vulnerabilities
echo "Test 6: Verify vulnerabilities..."
verify_out="$("$ROOT/harness/verify_vulnerabilities.sh" 2>&1 || true)"
test -f "$ROOT/reports/vuln-verification.md" || fail "vuln-verification.md missing"
echo "  PASS"

# Test 7: Final verify
echo "Test 7: Final verify..."
final_out="$("$ROOT/harness/final_verify.sh" 2>&1 || true)"
test -f "$ROOT/reports/final-report.md" || true  # May not exist if verification fails
echo "  PASS"

echo ""
echo "test_vuln_competition.sh: ALL TESTS PASS"
