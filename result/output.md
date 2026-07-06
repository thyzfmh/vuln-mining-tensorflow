# 自验证输出

本目录用于记录作品运行成功的输出信息。

当前交付件已经整理为平台要求的目录结构：

- `/INSTRUCTION.md`
- `/work/skills/vuln_mining_tf_blackbox/SKILL.md`
- `/work/skills/vuln_mining_tf_blackbox/skill.yaml`
- `/work/skills/vuln_mining_tf_blackbox/prompt.md`
- `/work/skills/vuln_mining_tf_blackbox/pipeline.md`
- `/work/skills/vuln_mining_tf_blackbox/output_spec.md`
- `/work/skills/vuln_mining_tf_blackbox/verify/run_test.py`
- `/result/output.md`
- `/result/screenshot`
- `/logs/interaction.md`
- `/logs/trace`

验证记录见：

- `logs/trace/self-verify.log`

## 本次自验证结果

已使用一个最小 C++ 源码目录完成端到端自验证：

```text
SELF_VERIFY_PASS
```

验证覆盖：

- 生成漏洞挖掘工程结构；
- 运行目标代码分析（`harness/analyze_target.sh`）；
- 生成扫描计划（`harness/plan_next_scan.sh`）；
- 运行 SAST 模式扫描（`harness/scan_sast.sh`）；
- 运行 LLM 扫描框架（`harness/scan_llm.sh`）；
- 运行漏洞验证（`harness/verify_vulnerabilities.sh`）；
- 运行最终验证（`harness/final_verify.sh`）；
- 生成 `reports/` 下的所有报告文件；
- 8 项测试全部通过。
