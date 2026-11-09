# Agent few-shot · v1

与 [Sengoo Agent 短参考](../../docs/agent-language-reference.md) 配套，参考标识
`sengoo-agent-reference/1`。三个程序使用公共实验 SDK `0.1.0`，edition 2026。

| 源文件 | 输入 | 正常退出 |
| --- | --- | --- |
| `tool_schema.sg` | 第一个参数为 JSON，例如 `{"count":3}` | 0；契约不符 2；解析/运行时失败 1 |
| `llm_poll.sg` | API base、模型、密钥文件；可选第四参数触发取消 | 0；失败/取消 1 |
| `persist_action.sg` | SQLite 数据库路径 | 0；错误/暂停 1 |

`llm_poll` 使用 Responses，凭据来自文件并由宿主负责保管，不作为命令行明文参数。
它输出增量文本及最终输入/输出 token 数；10 秒示例期限或第四参数会取消。生产宿主
应把取消信号与自己的事件循环连接。程序不自动重试，不自动执行模型工具。

`persist_action` 使用确定性模型结果，仅演示真实 SQLite 提交与恢复，不调用供应商。
相同路径第二次启动从已完成状态继续，保留累计请求数。它不绕过 store 的锁和
fencing，也不执行任意 shell；未知副作用的跨进程恢复见完整维护 Agent 样例。

根 manifest 默认编译 Schema 示例。复制到自己的项目时使用路径或离线 registry
依赖，执行 `sgpm update`，随后 check/build 使用 `--locked`。开发源码覆盖应显式
指定 source-development 模式。不要把相邻仓库路径当作安装用户的运行时依赖。

独立验证需要明确传入 `--compiler`、`--clang`、`--formatter`、`--sgpm` 和
`--sengoo` 到 `verify.py`，以及 JSON 证据输出 `--evidence`。验证器将各程序复制到
仓库外单独锁定，检查格式、编译并运行；测试 Schema 错误 JSON/退出码、实际本地
HTTP 流与取消、SQLite 再次启动，以及 `repair.json` 的真实编译错误和修复后输出。
运行阶段移除编译工具 PATH。所有联网验证只针对回环夹具，不使用 API Key。

`target/acceptance.json` 是本机执行证据，不随源码提交；工具 SHA、源文件哈希和
每步退出码由验证器保存。文档本身不是尚未运行场景的通过证明。

2026-11-09 在 Windows x64 实际通过 23 项，报告 SHA256
`746fc45d168a07951fd1bb7eabb722f754bb68622c53a39028f935e1242b6ed9`。
编译器 SHA256 为
`69a42135586f5fa56debe10f35e51e4423a926463e88c3f3bfa5f5e1f596ddce`。
最初使用内建 `print` 逐事件输出产生额外空行，完整 stdout 断言拒绝了该次运行；
改用公共 `std::io` 原样写入后通过，失败报告另存于
`target/builtin-print-newline-failed-acceptance.json`。SDK 当前实验版本的完整发布
还要通过合流候选的资源和安装门禁。
