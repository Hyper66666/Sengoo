# Sengoo Language for VS Code

Sengoo 编程语言的 VS Code 扩展，提供语法高亮、代码片段和 LSP 集成。

## 功能

- **语法高亮** - 完整的 Sengoo 语法着色支持
- **代码片段** - 常用代码模板（`def`, `struct`, `impl`, `for`, `match` 等）
- **LSP 集成** - 连接 `sglsp` 语言服务器，提供补全、跳转定义、悬停提示
- **括号匹配** - 自动匹配 `{}`, `[]`, `()`, `||`（闭包）

## 安装

### 从 VSIX 安装

```bash
code --install-extension sengoo-1.0.0.vsix
```

### 开发模式

```bash
npm install
npm run compile
# 按 F5 启动调试
```

## LSP 配置

安装 `sglsp` 后，插件会自动连接语言服务器：

```json
{
  "sengoo.lsp.enabled": true,
  "sengoo.lsp.path": "/path/to/sglsp"
}
```

## 代码片段

| 前缀 | 描述 |
|------|------|
| `main` | main 函数 |
| `def` | 函数定义 |
| `struct` | 结构体 |
| `impl` | 实现块 |
| `trait` | Trait 定义 |
| `if` / `ife` | 条件表达式 |
| `for` / `while` | 循环 |
| `let` | 变量绑定 |
| `lambda` | Lambda 闭包 |
| `match` | 模式匹配 |
