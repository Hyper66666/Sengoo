# Sengoo Language for VS Code

Sengoo 的 VS Code 扩展，提供基础语言支持、`sgc` 命令集成、调试入口和 `sglsp` 语言服务器接入。

## 功能

- 语法高亮：支持 `.sg` 文件的语法着色
- 代码片段：内置常用模板，如 `def`、`struct`、`impl`、`for`、`match`
- 命令集成：可直接运行、编译、编译并运行、类型检查当前文件
- 调试支持：提供 Sengoo 调试配置和快速启动入口
- LSP 集成：连接 `sglsp`，提供补全、跳转到定义、悬停等能力

## 安装

### 从 VSIX 安装

```bash
code --install-extension sengoo-1.0.0.vsix
```

### 本地开发

```bash
npm install
npm run compile
npm run package
```

编译产物会写入 `dist/`；该目录是本地生成内容，不需要提交。

按 `F5` 可以在扩展开发宿主中调试插件。

## 可用命令

- `Sengoo: 运行当前文件`
- `Sengoo: 编译当前文件`
- `Sengoo: 编译并运行`
- `Sengoo: 类型检查当前文件`
- `Sengoo: 显示信息`

## 配置项

```json
{
  "sengoo.lsp.enabled": true,
  "sengoo.lsp.path": "",
  "sengoo.sgc.path": "",
  "sengoo.trace.server": "off"
}
```

- `sengoo.lsp.enabled`：是否启用 `sglsp`
- `sengoo.lsp.path`：`sglsp` 可执行文件路径；留空时从 `PATH` 查找
- `sengoo.sgc.path`：`sgc` 可执行文件路径；留空时优先自动探测项目内 `target/debug/sgc`
- `sengoo.trace.server`：LSP 通信日志级别，可选 `off`、`messages`、`verbose`

## 调试配置

扩展提供两种默认调试模式：

- 运行当前 Sengoo 文件
- 编译并运行当前 Sengoo 文件

如果工作区中没有 `launch.json`，按 `F5` 时会自动生成并填充默认配置。

## 依赖

- `sgc`：用于运行、编译和类型检查 Sengoo 程序
- `sglsp`：用于语言服务器功能

如果本地没有显式配置 `sengoo.sgc.path`，扩展会优先在当前项目下自动探测 `target/debug/sgc` 或 `target/release/sgc`。
