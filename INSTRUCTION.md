# AI 漏洞挖掘：运行入口

## 目标

- **目标代码**: `code/tensorflow` (C++, v2.11.0)
- **交付件**: `vulnerability_list.md`, `llm_chat_log.json`, `vulnerability_report.md`

## 执行方式

加载并执行 Skill：

```text
work/skills/vuln-mining/SKILL.md
```

该 Skill 包含从项目初始化到最终验证的完整自主流程。目标代码路径已固定，加载后按步骤执行，不中断，不提问，直到 `final_verify.sh` 通过。

## 核心规则

- 全程自动执行，不询问用户
- 黑盒标准：不得在 LLM 交互中透露版本号、已知漏洞、CVE 编号
- 每个漏洞必须有源码证据和可复现的触发方式
- 验证不通过则立即修复，持续修复直到通过
- 仅在以下情况升级给用户：源码无法获取、连续 3 轮扫描未发现新漏洞、发现零日漏洞需要决策
