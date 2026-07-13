# OpenCode 执行入口

请 OpenCode 加载以下 Skill 文件：

```text
work/skills/vuln-mining-autonomous/SKILL.md
```

Skill 名称：vuln-mining-autonomous

## 环境准备

平台会把提交包挂载为只读目录，并为每次执行提供一个可写的 `executor_N` 工作目录。先把当前目录设置为该执行器工作目录，再从相邻的只读 `package` 复制提交包：

```bash
EXECUTOR_ROOT="$(pwd -P)"
PACKAGE_ROOT="${PACKAGE_ROOT:-$(dirname "$EXECUTOR_ROOT")/package}"
test -w "$EXECUTOR_ROOT"
test -f "$PACKAGE_ROOT/INSTRUCTION.md"
test "$EXECUTOR_ROOT" != "$PACKAGE_ROOT"
cp -R "$PACKAGE_ROOT/." "$EXECUTOR_ROOT/"
```

后续所有命令都在 `$EXECUTOR_ROOT` 中执行。不得切换到系统临时目录、不得写出执行器工作区、不得尝试修改只读的 `$PACKAGE_ROOT`。

## 执行方式

```bash
python3 work/run_vulnerability_mining.py
```

随后由 OpenCode 加载并完整执行 `work/skills/vuln-mining-autonomous/SKILL.md`，持续扫描、验证和更新 `work/`，直到最终门禁通过。

## 完成判定与结果获取

```bash
python3 work/skills/vuln-mining-autonomous/scripts/final_verify.py --work-root work
```

仅 `FINAL_VERIFY_PASS` 可视为完成。最终结果从可写副本的以下路径获取：

- `$EXECUTOR_ROOT/work/vulnerability_list.md`
- `$EXECUTOR_ROOT/work/llm_chat_log.json`
- `$EXECUTOR_ROOT/work/vulnerability_report.md`
- `$EXECUTOR_ROOT/work/verify/run_test.py`
- `$EXECUTOR_ROOT/work/reports/`
- `$EXECUTOR_ROOT/work/result/output.md`

执行规则：

- 入口编排器默认扫描平台资产根目录 `/app/code/judge-assets/01_02_vulnerability_detection`，自动定位目标源码并把路径上下文保存到 `work/.vuln-mining-target-root`；后续阶段不依赖人工导出环境变量，也不假设 `code/` 一定包含目标
- 仅在本地显式测试时，可用 `VULN_TARGET_ROOT` / `TARGET_ROOT` 或 `--target-root` 覆盖已发现的目标；平台执行不得依赖这些人工配置
- 所有运行时产物（reports、plans、verify、deliverables、result）写入 `work/` 输出根目录（环境变量 `VULN_WORK_ROOT` / `WORK_ROOT`、`--work-root` 参数，默认 `<repo>/work`）
- 提交包已预置全部评分 `key_paths`；其中 `BOOTSTRAP_PENDING` 仅用于保证只读提交包可取件，入口编排器会刷新这些初始文件，最终门禁不会接受未执行状态
- 不询问用户，不中断执行
- 全程无人值守：不得请求授权，不得使用 `sudo`、交互式安装、内联 Python 命令或执行器工作区外的写路径；子进程必须关闭标准输入
- 严格黑盒：不得在 LLM 交互记录中透露项目身份、版本、预置答案或外部漏洞库信息
- 所有测试必须由 AI 生成
- 只接受运行时验证通过的真实漏洞
- 目标是找出候选空间内所有可运行时验证的漏洞，不能以达到某个漏洞数量作为停止条件
- 必须生成 `work/reports/source-file-manifest.md`，证明确定性扫描遍历了所有源码文件
- 必须生成 `work/reports/toolchain-capabilities.md`，记录 ASAN/UBSAN 等验证工具是否可用
- 必须生成 `work/reports/verification-escalation.md`，记录 sanitizer 缺失时的替代验证路径和实际尝试
- 不得因为 ASAN/UBSAN 不可用或目标无法从源码树直接导入就结束扫描；必须继续尝试替代真实运行入口
- 必须生成 `work/reports/runtime-entrypoints.md`，把候选源码关联到现有测试、二进制、解析器或语言绑定等真实运行入口
- npm/npx 可用时必须生成 `work/reports/npm-ast-candidates.md`，将全树 AST 结构化匹配并入候选账本
- 必须维护 `work/reports/coverage-ledger.md`，覆盖所有生成的攻击面和候选，并记录 rejected/verified 结论
- 必须生成 `work/reports/scan-completion.md`，声明候选空间已耗尽且无未验证的 accepted 假设
- 最终必须生成 `work/vulnerability_list.md`、`work/llm_chat_log.json`、`work/vulnerability_report.md`、`work/verify/run_test.py`
- 直到 `python3 work/skills/vuln-mining-autonomous/scripts/final_verify.py --work-root work` 通过
- 最后必须更新 `work/result/output.md`
- 入口编排器：`python3 work/run_vulnerability_mining.py`
