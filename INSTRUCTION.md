# AI 漏洞挖掘：运行入口

## 执行方式

加载并执行 Skill：

```text
work/skills/vuln_mining_tf_blackbox/SKILL.md
```

按 SKILL.md 中的 8 步流水线依次执行，不中断，不提问，直到 `final_verify.sh` 通过。

## 交付件

| 文件 | 说明 |
|------|------|
| `vulnerability_list.md` | 漏洞清单（含证据链） |
| `llm_chat_log.json` | LLM 交互记录（不得编辑，90%可复现） |
| `vulnerability_report.md` | 漏洞审查工程化报告 |
| `verify/run_test.py` | AI 生成的运行时验证脚本 |
| `result/output.md` | **作品运行成功的输出信息（必选）** |

## 核心规则

- 全程自动执行，不询问用户
- 黑盒标准：不得在 LLM 交互中透露版本号、已知漏洞、CVE 编号
- 每个漏洞必须有源码证据和可复现的触发方式
- 所有测试用例必须由 AI 生成
- 所有漏洞必须通过运行时验证（`verify/run_test.py`）
- **执行完成后必须将运行结果写入 `result/output.md`**
- 验证不通过则立即修复，持续修复直到通过

## 完成条件

全部满足才算完成：

1. `final_verify.sh` 通过（exit code 0）
2. `vulnerability_list.md` 有 ≥ 1 个运行时验证的漏洞
3. `llm_chat_log.json` 有 ≥ 5 轮交互
4. `result/output.md` 已更新执行结果
5. 所有交付件无黑盒违规
