# Sengoo Agent 短参考 v1

参考标识：`sengoo-agent-reference/1`。适用 edition 2026、Sengoo
`0.2.1-dev.0` 和实验包 `sgschema`、`sgllm`、`sgagent` `0.1.0`。
开发快照还需使用本轮验收记录中的完整编译器 SHA；版本号相同不等于二进制相同。
本页供模型生成代码时加入上下文。完整语义以[语言参考](language-reference.md)
和各包公开接口为准，不推断不存在的 API。

## 生成代码约定

| 要写的内容 | 使用方式 |
| --- | --- |
| 函数与入口 | `def name(...) -> Type { ... }`；程序入口是 `def main() -> i64` |
| 可变绑定 | `let mut total = 0;`；随后才可给 `total` 赋值 |
| 布尔与控制流 | `bool`、`true`、`false`、`if`、`while`、`loop`；逻辑运算可用 `and/or/not` |
| 拥有文本 | `String` 拥有 UTF-8 资源；`&str` 是借用文本，`&String` 借用拥有者 |
| 失败传播 | 返回 `Result<T, i64>` 的函数可用 `?`；`main -> i64` 自行处理 Result 并返回退出码 |
| Result 构造 | `Result { is_ok: true, value: value, error: 0 }`；错误分支不可携带非零拥有型资源占位值 |
| 导入 | `import std::json;`、`import sgllm;` 等真实模块；依赖在 manifest 中声明并锁定 |
| JSON | `json_parse_limits`、`JsonDoc.root()`、`object_get`、类型访问器、`serialize_string` |
| JSON 子节点 | `JsonValue` 借用所属 `JsonDoc`，需要保留文档；跨文档使用 `destination.copy(value)` |
| 原样流式输出 | `std::io` 的 `io_stdout_write` 与 `io_stdout_flush`；非空增量才写出，不能把逐事件 `print` 当作原样字节写入 |

拥有值移动后不继续使用原绑定。优先借用、显式拥有局部变量和自动 Drop；不要为
绕过诊断重建 raw handle、手动重复释放，或返回局部变量的引用。标准库中部分历史
按值包装具有经审计的借用契约，不表示任意按值资源参数都可以再次使用。
完整生命周期系统和任意资源型异步取消均不在当前支持声明内。

控制名称、ID、路径、模型名必须先验证 `owned.len() == owned.as_str().len()`，再
缩窄为 C 字符串视图；该检查拒绝内嵌 NUL。工具参数中的合法 JSON 字符串业务值
可包含转义 Unicode，不能靠字符串拼接解释参数或权限。

## 三个可编译 few-shot

1. [Schema 工具](../examples/agent-learning/tool_schema.sg)：解析有界输入，用
   `schema_validate` 校验工具参数，输出 JSON；契约拒绝返回退出码 2，运行时失败为 1。
   Schema 校验成功不授予执行权限；未知 Schema 关键字按定义错误处理。
2. [LLM 轮询](../examples/agent-learning/llm_poll.sg)：从宿主提供的密钥文件读取凭据，
   `llm_request_config` → `llm_request_start` → `poll` → `result`，不阻塞状态所有者。
   取消调用 `job.cancel()`，失败读取分类阶段与 HTTP 状态，凭据不写日志。
   HTTP 200 不能替代完整 Provider 回合验证；增量工具参数不直接执行。
3. [SQLite persist/action](../examples/agent-learning/persist_action.sg)：接收 `persist`
   后由真实 SQLite 事务提交，再用匹配 receipt 确认；之后才处理 `model_request`。
   此短例使用明确的确定性模型适配器，不发网络请求。重启读取同一检查点；更完整的
   [维护 Agent](../packages/sgagent/examples/maintenance.sg) 演示真实流、修改、命令及崩溃。

模型请求、工具调用、逻辑任务和存储 revision 使用各自稳定身份。网络重试不能重跑
已完成工具；未知副作用暂停，不能从“进程锁已释放”推断旧工具已静止。内核动作
是宿主协议，模型不能伪造 receipt、授权自己或确认进程树清理。请求上限为 0
表示不限请求次数；显式费用预算与有界输入、内存、并发属于不同约束。

## 根据 JSON 诊断修复

执行 `sgc --error-format json check path/to/main.sg`，同时保留真实进程退出码。
错误对象含 `schema_version`、`ok`、`kind`、`stage`、`code`、`input`、
`message`、`details` 和可能存在的 `location`；按实际返回字段处理，不能凭空补位置。
部分非编译器错误可能没有相同结构。修复后重新 check/build/run，并检查退出码和输出。

[repair.json](../examples/agent-learning/repair.json) 保存一个最小前后修复：不可变
局部变量被赋值，收到 `immutable-assignment` 后在原声明加 `mut`。它是测试输入，
实际编译诊断由[验证器](../examples/agent-learning/verify.py)生成，不冒充静态示例为执行结果。

源码开发环境设置 `SENGOO_STDLIB` / `SENGOO_RUNTIME` / 模块映射后，必须在子命令前
使用 `--runtime-mode source-development`。默认 installed 模式拒绝源码覆盖；重复
同一条错误配置命令不会修好。`rg` 退出 1 表示无匹配，不能冒称执行成功或工具崩溃。

当前示例的 Windows 实际结果与工具 SHA 记录在
`examples/agent-learning/target/acceptance.json`。无该次运行证据不推断其他平台可用，
离线 HTTP 夹具不等于真实供应商验证。

2026-11-09 的 Windows x64 记录已通过 23 项：三个锁定的外部消费程序、精确
流式输出/取消、SQLite 再次启动、错误 JSON 和修复后的原生运行。报告 SHA256：
`746fc45d168a07951fd1bb7eabb722f754bb68622c53a39028f935e1242b6ed9`。
该报告绑定编译器
`69a42135586f5fa56debe10f35e51e4423a926463e88c3f3bfa5f5e1f596ddce`，
不能据此代替另一候选编译器的资源正确性验收。
