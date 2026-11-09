# Agent SDK 开发预览

这些 API 目前为 `0.1.0` 实验包，独立于标准库。尚未公开发布安装包；
本地预览产物与验收进度见 [实施账本](agent-ecosystem-progress.md)。
当前实际验收主机为 Windows x64，其他平台不能仅凭源码可移植性标为已支持。

## 包与职责

| 包或工具 | 负责什么 | 应用仍负责什么 |
| --- | --- | --- |
| sgllm | HTTP/SSE、三协议、工具参数流、用量、错误阶段 | 连接配置、密钥、重试策略、权限和工具执行 |
| sgmcp | stdio 协商、工具发现/调用、进度、取消、进程回收；服务器协议 | 业务 handler、目录权限、是否批准调用 |
| sgschema | 明确支持的工具 Schema 子集、路径化失败信息 | 选择合法 Schema，解析前的原始重复 JSON 键检查 |
| sgagent | 可嵌入动作核心、调度、权限策略、SQLite 检查点与工具账本 | 实际执行、审批、可信清理证明与未知结果核实 |
| sgeval | 离线录制脱敏、数据回放、断言、JSON 报告 | 真实模型任务、评分数据、成本与最终验收 |

当前 SDK 不读取 Sencode 私有数据库或自动获取其凭据。调用者提供配置与密钥，
避免将密钥写入录制文件。网络错误不等于工具失败；重试模型请求不能重放已完成
或结果未知的写工具。

## 离线依赖

构建和校验开发预览 bundle（编译工具由开发者准备）：

```text
python scripts/package_ecosystem.py build --sgpm target/debug/sgpm.exe --profile agents --output target/agent-preview
python scripts/package_ecosystem.py verify target/agent-preview --profile agents
```

将校验后的 registry bundle 解压到项目旁；保持 `registry/` 与 manifest 相对路径：

```toml
[package]
name = "my_agent"
version = "0.1.0"
edition = "2026"

[bin]
path = "src/main.sg"

[registries.official]
path = "registry"

[dependencies]
sgllm = { version = "=0.1.0", registry = "official" }
sgmcp = { version = "=0.1.0", registry = "official" }
sgschema = { version = "=0.1.0", registry = "official" }
sgagent = { version = "=0.1.0", registry = "official" }
```

```text
sgpm update --runtime-mode source-development
sgpm check --locked --runtime-mode source-development
sgpm build --locked --runtime-mode source-development
```

上面选用 source-development 是本仓库预览工具链的构建方式。使用本地 stdlib、
runtime 覆盖时必须显式选择该模式。真正独立安装版的命令与二进制应以其安装
验收记录为准，不能靠把任意旧版本工具的文件名改掉来满足 SDK 的最低能力。

preview 打包会将已知 sibling 开发路径依赖转换为精确的 official registry 版本，
仅修改归档中的 manifest，不改源码 checkout。清单同时保留源归档哈希、发布
归档哈希与是否转换。基础三包的默认分发 profile 保留。

## 从已检查的示例开始

- 模型流：`packages/sgllm/examples/stream.sg`，订阅流、取消和失败信息。
- MCP 服务：`packages/sgmcp/examples/echo-server.sg`，业务 handler 与协议循环分离。
- MCP 客户端：`packages/sgmcp/examples/echo-client.sg`，初始化、调用、轮询和回收。
- Schema：`packages/sgschema/tests/schema.sg`，正反例包含 Unicode 和精确 i64。
- 离线评估：`tools/sgeval/README.md`，录制与回放命令、断言格式和退出状态。
- 双模型审查：`examples/realworld/agent-audit`，两个连接读取同一份显式文件快照。
- 本地知识服务：`examples/realworld/agent-knowledge`，MCP 工具和 SQLite 索引。
- 可恢复维护：`packages/sgagent/examples/maintenance.sg`，读取、修改、验证和未知副作用恢复。

这些是可编译的源文件，具体命令行参数以各包 README 为准。SDK 测试中的 HTTP
服务和模型答案是夹具，不把它们作为真实供应商效果、性能或 API 可用性的证据。

## 生命周期约定

`Result<T,E>` 的失败值保持无资源的占位值；不要从 Err.value 读取拥有型数据。
需要保留详细失败内容时，用宿主或客户端上的显式拥有型 accessor，例如 MCP
的 `rpc_response()`。这样既保留原始错误，也让失败路径可恢复到资源基线。

原生 SDK 采用单个 Sengoo 状态所有者加可轮询 I/O。调用者应持续轮询并消费
有界通知，不在一个回合中阻塞输入与取消。MCP 的取消只有在进程树静止后才
完成清理，不能把“已发取消通知”当作工具没有产生副作用。

模型 SDK、MCP、Agent 核心、三个样板、教程与 Python 工具已有固定工具链的独立
验收，资源检查与仓库外锁定构建见[Windows 验收记录](agent-ecosystem-acceptance.md)。
服务器输出使用通用的有界 StdWriter，目前限 Windows 本地字节管道；其他端点明确
返回不支持。Python 样例只覆盖稳定小签名函数，不导出整个 Agent 对象。
CLI 安装和其他平台的状态以[实施账本](agent-ecosystem-progress.md)为准。
