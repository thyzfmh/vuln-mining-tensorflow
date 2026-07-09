# OpenCode 执行入口

请 OpenCode 加载以下 Skill 文件：

```text
work/skills/vuln-mining-autonomous/SKILL.md
```

Skill 名称：vuln-mining-autonomous

执行规则：

- 目标源码固定从 `code/` 目录自动识别
- 不询问用户，不中断执行
- 严格黑盒：不得在 LLM 交互记录中透露项目身份、版本、预置答案或外部漏洞库信息
- 所有测试必须由 AI 生成
- 只接受运行时验证通过的真实漏洞
- 目标是找出候选空间内所有可运行时验证的漏洞，不能以达到某个漏洞数量作为停止条件
- 必须生成 `reports/source-file-manifest.md`，证明确定性扫描遍历了所有源码文件
- 必须生成 `reports/toolchain-capabilities.md`，记录 ASAN/UBSAN 等验证工具是否可用
- 必须生成 `reports/verification-escalation.md`，记录 sanitizer 缺失时的替代验证路径和实际尝试
- 必须维护 `reports/coverage-ledger.md`，覆盖所有生成的攻击面和候选，并记录 rejected/verified 结论
- 必须生成 `reports/scan-completion.md`，声明候选空间已耗尽且无未验证的 accepted 假设
- 最终必须生成 `vulnerability_list.md`、`llm_chat_log.json`、`vulnerability_report.md`、`verify/run_test.py`
- 直到 `python3 work/skills/vuln-mining-autonomous/scripts/final_verify.py` 通过
- 最后必须更新 `result/output.md`
