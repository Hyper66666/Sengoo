# Sengoo 编程语言示例

## 从零开始学习 Sengoo

### 01_hello.sg - Hello World
最简单的程序，返回一个常量值。

### 02_arithmetic.sg - 算术运算
演示加法运算：`10 + 20 = 30`

### 03_variables.sg - 变量绑定
演示变量声明和使用：`x + y = 300`

### 04_array.sg - 数组索引
演示数组创建和元素访问。

### 05_loop.sg - 循环遍历
演示 `for-in` 循环，计算数组元素之和。

### 06_lambda.sg - Lambda 闭包
演示闭包捕获外部变量：`x + y = 15`

### 07_if.sg - 条件分支
演示 `if-else` 条件语句。

### 08_struct.sg - 结构体
演示结构体定义和字段访问。

### ffi/ - C FFI 双向调用
演示 `extern "C"` 声明、导出符号（`export_name`）以及 Sengoo <-> C 的最小闭环。

## 运行示例

```bash
# 编译并查看所有示例
cargo run --bin test_all_examples

# 或手动编译单个文件
./target/release/sgc build examples/01_hello.sg --emit-llvm
```
