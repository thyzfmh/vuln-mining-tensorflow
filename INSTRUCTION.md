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
- 最终必须生成 `vulnerability_list.md`、`llm_chat_log.json`、`vulnerability_report.md`、`verify/run_test.py`
- 直到 `python3 work/skills/vuln-mining-autonomous/scripts/final_verify.py` 通过
- 最后必须更新 `result/output.md`
