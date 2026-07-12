# Sengoo 编程语言 - 完整开发规范

> **项目代号**: Sengoo
> **版本**: v0.1.0
> **状态**: 设计阶段
> **文档用途**: 开发团队/AI Agent的完整技术规范

---

## 📋 目录

1. [项目概述](#1-项目概述)
2. [语言设计](#2-语言设计)
3. [技术架构](#3-技术架构)
4. [编译器前端规范](#4-编译器前端规范)
5. [编译器后端规范](#5-编译器后端规范)
6. [运行时设计](#6-运行时设计)
7. [标准库设计](#7-标准库设计)
8. [工具链设计](#8-工具链设计)
9. [Python互操作](#9-python互操作)
10. [实现路线图](#10-实现路线图)
11. [测试策略](#11-测试策略)
12. [编码规范](#12-编码规范)

---

## 1. 项目概述

### 1.1 项目定位

**Sengoo** 是一门现代化的编程语言，定位如下：

| 维度 | 描述 |
|------|------|
| **目标领域** | IDE开发、Web开发、后端服务、中小型项目 |
| **设计目标** | 保持Python的易读性，解决其核心痛点 |
| **核心特性** | `{}块语法 + 渐进式类型 + 无GIL并行 + Python互操作 |
| **对标关系** | 类似TypeScript之于JavaScript |

### 1.2 解决的Python痛点

| 痛点 | Sengoo解决方案 |
|------|----------------|
| **GIL限制** | 无GIL设计，真正的多线程并行 |
| **缩进错误** | 使用`{}`替代缩进划分块 |
| **类型安全** | 渐进式类型系统，可选类型注解 |
| **性能问题** | JIT + AOT编译，LLVM后端 |
| **启动慢** | 支持字节码快速启动 |
| **依赖混乱** | 统一包管理器，锁定文件 |

### 1.3 技术决策总览

| 决策点 | 选择 | 理由 |
|--------|------|------|
| **实现语言** | Rust | 安全、并发、零成本抽象 |
| **内存管理** | RC + 循环GC | 平衡安全性与易用性 |
| **类型系统** | 渐进式类型，默认推导 | 保持Python的简洁性 |
| **编译模式** | JIT + AOT | 开发体验与生产性能兼顾 |
| **Python集成** | 嵌入CPython | 性能与兼容性最优 |
| **类型推导** | 局部推导 | 实用主义，避免复杂化 |
| **self语法** | 保持 | 降低迁移成本 |

### 1.4 项目结构

```
sengoo/
├── compiler/              # 编译器核心
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── lexer/         # 词法分析器
│       ├── parser/        # 语法分析器
│       ├── ast/           # AST定义
│       ├── typeck/        # 类型检查器
│       ├── hir/           # 高级IR
│       ├── mir/           # 中级IR
│       └── codegen/       # 代码生成
│           ├── llvm/      # LLVM后端
│           ├── bytecode/  # 字节码后端
│           └── wasm/      # WASM后端
│
├── runtime/               # 运行时
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── memory/        # 内存管理
│       ├── value/         # 值表示
│       ├── sync/          # 并发原语
│       └── python/        # Python互操作
│
├── stdlib/                # 标准库
│   ├── core/             # 核心类型
│   ├── collections/      # 集合类型
│   ├── io/               # IO操作
│   ├── net/              # 网络库
│   ├── concurrent/       # 并发库
│   ├── json/             # JSON
│   └── testing/          # 测试框架
│
├── tools/                 # 工具链
│   ├── sgc/              # 编译器CLI (sengoo compiler)
│   ├── sgfmt/            # 格式化工具
│   ├── sgpy/             # 包管理器 (sengoo package manager)
│   └── sglsp/            # LSP服务器
│
├── tests/                 # 测试套件
│   ├── lexer/            # 词法测试
│   ├── parser/           # 语法测试
│   ├── typeck/           # 类型测试
│   └── runtime/          # 运行时测试
│
├── docs/                  # 文档
│   ├── language/         # 语言文档
│   ├── stdlib/           # 标准库文档
│   └── internals/        # 内部文档
│
├── Cargo.toml             # 工作空间配置
├── README.md
└── this_file.md          # 本文档
```

---

## 2. 语言设计

### 2.1 文件扩展名

| 扩展名 | 用途 |
|--------|------|
| `.sg` | Sengoo源代码文件（推荐） |
| `.sgoo` | Sengoo源代码文件（完整） |

### 2.2 Hello World

```python
# hello.sg

fn main() {
    print("Hello, Sengoo!")
}
```

### 2.3 语法规范

#### 2.3.1 注释

```python
// 单行注释

/*
 * 多行注释
 * 可以跨多行
 */

/// 文档注释（用于函数、类等）
fn example() {
    // TODO注释
    // FIXME注释
}
```

#### 2.3.2 标识符命名

```python
// 变量和函数：snake_case
let my_variable = 42
fn calculate_sum() { }

// 类和结构体：PascalCase
class MyClass { }
struct MyStruct { }

// 常量：SCREAMING_SNAKE_CASE
const MAX_SIZE = 100

// 私有成员：前缀下划线（约定）
class Example {
    let _private_value = 1
}
```

#### 2.3.3 字面量

```python
// 整数
let a = 42           // i32
let b = 42i64        // i64
let c = 42u8         // u8
let d = 0x2A         // 十六进制
let e = 0o52         // 八进制
let f = 0b101010     // 二进制

// 浮点数
let x = 3.14         // f64
let y = 3.14f32      // f32
let z = 1.0e10       // 科学计数法

// 字符串
let s1 = "hello"
let s2 = "hello\nworld"     // 转义序列
let s3 = r"raw\nstring"     // 原始字符串

// 字符串插值
let name = "Sengoo"
let greeting = f"Hello, {name}!"

// 多行字符串
let multiline = """
    This is a
    multi-line string
    No indentation issues
"""

// 字节串
let b1 = b"hello"
let b2 = b"\x00\x01\x02"

// 布尔值
let flag1 = true
let flag2 = false

// 空值
let nothing = null
```

#### 2.3.4 运算符

**优先级（从高到低）：**

```python
// 1. 路径、字段、调用
a.b, a(), a[i], a..b, a..=b

// 2. 一元运算符
-, !, ~, *, &, await

// 3. 乘除类
*, /, %

// 4. 加减类
+, -

// 5. 移位
<<, >>

// 6. 比较
==, !=, <, >, <=, >=

// 7. 逻辑与
&&

// 8. 逻辑或
||

// 9. 范围
.., ..=

// 10. 赋值
=, +=, -=, *=, /=, %=, &=, |=, ^=, <<=, >>=

// 11. Lambda
|args| expr

// 12. 模式匹配分支
=>
```

### 2.4 变量与常量

```python
// let绑定（可变）
let x = 42
x = 10  // OK，可变

// const绑定（编译时常量）
const MAX: i32 = 100

// 静态变量
static COUNTER: i32 = 0

// 类型注解
let name: str = "Sengoo"
let count: i32 = 42

// 可选类型
let maybe: Option[i32] = Some(42)
let nothing: Option[i32] = None

// 解构赋值
let (x, y) = (1, 2)
let Point { x, y } = point
```

### 2.5 函数定义

```python
// 基本函数
fn add(x, y) {
    return x + y
}

// 带类型注解
fn calculate(x: i32, y: i32) -> i32 {
    return x + y
}

// 隐式返回（表达式结尾）
fn add(x: i32, y: i32) -> i32 {
    x + y
}

// 多返回值（元组）
fn divmod(a: i32, b: i32) -> (i32, i32) {
    (a / b, a % b)
}

// 泛型函数
fn identity<T>(x: T) -> T {
    x
}

// 可变参数
fn sum(nums: i32...) -> i32 {
    let total = 0
    for n in nums {
        total += n
    }
    total
}

// Lambda/箭头函数
let add = |x, y| x + y
let square = |x| x * x

// 高阶函数
fn apply(f: fn(i32) -> i32, x: i32) -> i32 {
    f(x)
}
```

### 2.6 控制流

#### 2.6.1 if/else

```python
// 基本用法
if condition {
    print("true")
}

// if/else
if condition {
    print("true")
} else {
    print("false")
}

// if/else if/else
if x > 0 {
    print("positive")
} else if x < 0 {
    print("negative")
} else {
    print("zero")
}

// if表达式（三元运算符替代）
let abs = if x >= 0 { x } else { -x }

// 条件可以是let绑定（模式匹配）
if let Some(value) = optional {
    print(value)
}
```

#### 2.6.2 循环

```python
// while循环
while condition {
    // ...
}

// for循环（范围）
for i in 0..10 {
    print(i)
}

// for循环（迭代器）
for item in list {
    print(item)
}

// 带索引的for
for (index, item) in list.enumerate() {
    print(f"{index}: {item}")
}

// 无限循环
loop {
    if should_break {
        break
    }
}

// break和continue
for i in 0..10 {
    if i % 2 == 0 {
        continue
    }
    if i > 7 {
        break
    }
    print(i)
}

// 带标签的break/continue
'outer: for i in 0..10 {
    for j in 0..10 {
        if i * j > 50 {
            break 'outer
        }
    }
}
```

#### 2.6.3 match（模式匹配）

```python
// 基本匹配
match value {
    1 => "one",
    2 => "two",
    _ => "other"
}

// 匹配范围
match x {
    0..=10 => "small",
    11..=100 => "medium",
    _ => "large"
}

// 匹配结构
match point {
    Point { x: 0, y } => f"on y-axis at {y}",
    Point { x, y: 0 } => f"on x-axis at {x}",
    Point { x, y } => f"at ({x}, {y})"
}

// 匹配Option
match optional {
    Some(value) => print(value),
    None => print("nothing")
}

// 匹配Result
match result {
    Ok(value) => print(f"Success: {value}"),
    Err(error) => print(f"Error: {error}")
}

// 守卫条件
match x {
    n if n % 2 == 0 => "even",
    n if n % 2 == 1 => "odd",
    _ => "unknown"
}
```

### 2.7 类与结构体

#### 2.7.1 结构体

```python
// 定义结构体
struct Point {
    x: f64
    y: f64
}

// 创建实例
let p = Point { x: 1.0, y: 2.0 }

// 访问字段
print(p.x)

// 更新语法
let p2 = Point { x: 3.0, ..p }

// 元组结构体
struct Color(f64, f64, f64)
let red = Color(1.0, 0.0, 0.0)

// 单元结构体
struct UnitStruct
```

#### 2.7.2 impl块

```python
// 为结构体添加方法
impl Point {
    // 构造函数（约定使用new）
    fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    // 方法
    fn distance(&self, other: Point) -> f64 {
        let dx = self.x - other.x
        let dy = self.y - other.y
        (dx * dx + dy * dy).sqrt()
    }

    // 静态方法
    fn origin() -> Self {
        Self { x: 0.0, y: 0.0 }
    }
}
```

#### 2.7.3 类（class）

```python
// 类定义
class Counter {
    // 私有字段
    let _count: i32

    // 构造函数
    fn new(init: i32) -> Self {
        Self { _count: init }
    }

    // 方法
    fn increment(&self) {
        self._count += 1
    }

    fn get(&self) -> i32 {
        self._count
    }
}

// 使用
let counter = Counter::new(0)
counter.increment()
print(counter.get())
```

#### 2.7.4 继承与Trait

```python
// Trait定义
trait Display {
    fn to_string(&self) -> str
}

// Trait实现
impl Display for Point {
    fn to_string(&self) -> str {
        f"Point({self.x}, {self.y})"
    }
}

// 带默认方法的Trait
trait Animal {
    fn speak(&self) -> str {
        "..."
    }

    fn name(&self) -> str
}

// Trait约束（泛型）
fn print_display<T: Display>(value: T) {
    print(value.to_string())
}

// where子句（更复杂的约束）
fn process<T, U>(t: T, u: U) -> str
    where T: Display,
          U: Clone
{
    t.to_string()
}
```

### 2.8 集合类型

```python
// 列表（可变数组）
let nums = [1, 2, 3, 4, 5]
let empty: List[i32] = []

// 操作
nums.push(6)
let first = nums[0]
let slice = nums[1..3]

// 字典
let scores = {
    "Alice": 100,
    "Bob": 95
}

// 操作
scores["Charlie"] = 88
let alice_score = scores.get("Alice")

// 集合
let set = {1, 2, 3, 4}
set.insert(5)

// 元组
let tuple = (1, "hello", true)
let (a, b, c) = tuple
```

### 2.9 异步编程

```python
// 异步函数
async fn fetch_user(id: i32) -> Result[User] {
    let response = await http.get(f"/api/users/{id}")
    return response.json()
}

// 调用异步函数
async fn main() {
    let user = await fetch_user(1)
    print(user.name)
}

// 并行执行（无GIL）
parallel {
    fetch_user(1),
    fetch_user(2),
    fetch_user(3)
}

// 异步迭代
async fn process_items() {
    for await item in stream {
        print(item)
    }
}

// 生成器
fn range(n: i32) {
    let i = 0
    while i < n {
        yield i
        i += 1
    }
}

// 消费生成器
for n in range(10) {
    print(n)
}
```

### 2.10 错误处理

```python
// Result类型
enum Result<T, E> {
    Ok(T),
    Err(E)
}

// Option类型
enum Option<T> {
    Some(T),
    None
}

// 使用?
fn divide(a: i32, b: i32) -> Result<f64, str] {
    if b == 0 {
        return Err("Division by zero")
    }
    return Ok(a as f64 / b as f64)
}

// 链式调用
fn calculate() -> Result<f64, str] {
    let x = divide(10, 2)?
    let y = divide(x as i32, 5)?
    return Ok(y)
}

// try/except（兼容Python风格）
try {
    let result = risky_operation()
} except Error as e {
    print(f"Error: {e}")
} finally {
    cleanup()
}

// throw/raise
fn validate(x: i32) {
    if x < 0 {
        throw Error("Invalid value")
    }
}
```

### 2.11 模块系统

```python
// 导入单个名称
import math
print(math.sqrt(16))

// 导入多个
import math, json, random

// 导入并重命名
import numpy as np

// 从模块导入
from math import sqrt, sin, cos

// 从模块导入所有
from collections import *

// 相对导入
from .utils import helper
from ..parent import Parent
from ..sibling.module import func

// 导出
pub fn public_function() { }
priv fn private_function() { }

// 模块定义（文件即为模块）
// utils/sg
pub fn helper() {
    print("helping...")
}

// 子模块（目录）
// mymodule/
//     mod.sg
//     utils/
//         helper.sg
```

### 2.12 装饰器

```python
// 函数装饰器
fn log_decorator(func) {
    fn wrapper(*args, **kwargs) {
        print(f"Calling {func.name}")
        let result = func(*args, **kwargs)
        print(f"Done")
        return result
    }
    return wrapper
}

@log_decorator
fn my_function() {
    print("Inside function")
}

// 带参数的装饰器
fn repeat(times: i32) {
    fn decorator(func) {
        fn wrapper(*args, **kwargs) {
            for _ in 0..times {
                func(*args, **kwargs)
            }
        }
        return wrapper
    }
    return decorator
}

@repeat(3)
fn say_hello() {
    print("Hello!")
}

// 类装饰器
@dataclass
class User {
    name: str
    age: i32
}
```

### 2.13 类型系统完整定义

#### 2.13.1 基础类型

```python
// 整数
i8, i16, i32, i64, i128      // 有符号整数
u8, u16, u32, u64, u128      // 无符号整数
isize, usize                 // 指针大小的整数

// 浮点数
f32, f64

// 其他
bool, str, bytes
```

#### 2.13.2 复合类型

```python
// 列表
List[T]

// 元组
(T1, T2, ...)                // 固定大小
(T1, T2, T3, ...)            // 任意大小

// 字典
Dict[K, V]

// 集合
Set[T]
```

#### 2.13.3 特殊类型

```python
// Option（可能有值也可能无）
Option[T] = Some(T) | None

// Result（可能成功或失败）
Result<T, E> = Ok(T) | Err(E)

// 函数类型
fn(Args) -> ReturnType

// 单元类型
()                           // 空元组

// Never类型（永不返回）
!
```

#### 2.13.4 类型推导规则

```python
// 局部推导
let x = 42                   // 推导为 i32
let y = 3.14                 // 推导为 f64
let z = x + y                // 错误：类型不匹配

// 函数返回值推导
fn add(x, y) {               // 参数为 Any
    return x + y            // 返回值推导
}

// 泛型推导
fn identity<T>(x: T) -> T {
    x
}
let result = identity(42)    // T推导为 i32

// 约束推导
fn process<T: Display>(x: T) {
    print(x.to_string())
}
```

---

## 3. 技术架构

### 3.1 编译架构总览

```
┌─────────────────────────────────────────────────────────────────┐
│                      Sengoo 源代码 (.sg)                         │
└─────────────────────────────┬───────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  前端 (Frontend)                                                 │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────────────────┐ │
│  │ Lexer        │→ │ Parser       │→ │ Name Resolution        │ │
│  │ 词法分析      │  │ 语法分析      │  │ 名称解析                │ │
│  └──────────────┘  └──────────────┘  └────────────────────────┘ │
│                                          │                       │
│                                          ▼                       │
│                                ┌────────────────────────────────┐│
│                                │ Type Checker                  ││
│                                │ 类型推导 + 类型检查            ││
│                                └────────────────────────────────┘│
└─────────────────────────────┬───────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  中端 (Middle-end)                                              │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ HIR (High-Level IR)                                        │ │
│  │ - 接近源代码的抽象                                          │ │
│  │ - 保留高级语义                                              │ │
│  └────────────────────────────────────────────────────────────┘ │
│                              │                                  │
│                              ▼                                  │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ MIR (Mid-Level IR)                                         │ │
│  │ - SSA形式                                                  │ │
│  │ - 类型擦除                                                 │ │
│  │ - 数据流分析                                               │ │
│  └────────────────────────────────────────────────────────────┘ │
└─────────────────────────────┬───────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  后端 (Backend)                                                 │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐  │
│  │ LLVM IR      │  │ Bytecode     │  │ WASM                 │  │
│  │ 原生二进制    │  │ 快速启动      │  │ Web/移动端           │  │
│  └──────────────┘  └──────────────┘  └──────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

### 3.2 技术栈选择

| 组件 | 技术选择 | 备选方案 |
|------|----------|----------|
| **实现语言** | Rust | C++, C |
| **词法分析** | logos | 自建 |
| **语法分析** | chumsky / 自建 | pest, lalrpop |
| **IR** | 自定义HIR/MIR | LLVM IR直接 |
| **后端** | LLVM | Cranelift |
| **Python集成** | pyo3 | cpython-c-api |
| **测试框架** | criterion | 自建 |
| **错误处理** | miette | anyhow, eyre |
| **日志** | tracing | log |

### 3.3 编译模式

| 模式 | 编译速度 | 运行速度 | 调试 | 热重载 | 适用场景 |
|------|----------|----------|------|--------|----------|
| **jit** | 快 | 中 | 可 | 支持 | 开发、REPL |
| **aot-native** | 慢 | 最快 | 可 | 不支持 | 生产环境 |
| **aot-bytecode** | 最快 | 慢 | 可 | 支持 | 脚本、CLI |

---

## 4. 编译器前端规范

### 4.1 Lexer（词法分析器）

#### 4.1.1 Token定义

```rust
// compiler/lexer/src/token.rs

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TokenKind {
    // 标识符
    Ident(Symbol),
    Keyword(Keyword),

    // 字面量
    Literal(LiteralKind),

    // 运算符 - 算术
    Plus, Minus, Star, Slash, Percent,

    // 运算符 - 位运算
    BitAnd, BitOr, BitXor, Shl, Shr,
    BitNot, // ~

    // 运算符 - 逻辑
    And, Or, Not,
    Eq, NotEq,

    // 运算符 - 比较
    Lt, Gt, Le, Ge,

    // 运算符 - 赋值
    Assign, AddAssign, SubAssign,
    MulAssign, DivAssign, ModAssign,
    BitAndAssign, BitOrAssign, BitXorAssign,
    ShlAssign, ShrAssign,

    // 分隔符
    LParen, RParen,        // ( )
    LBrace, RBrace,        // { }
    LBracket, RBracket,    // [ ]
    Colon, ColonColon,     // : ::
    Semicolon, Comma, Dot,
    DotDot, DotDotDot,     // .. ...

    // 箭头
    Arrow,     // ->
    FatArrow,  // =>

    // 其他
    At,       // @ 装饰器
    Question, // ? 可选类型
    Dollar,   // $ 字符串插值

    // 工具
    Whitespace,
    Comment,
    Newline,
    Shebang,

    // 结束
    EOF,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Keyword {
    // 声明
    Fn, Class, Struct, Enum, Impl, Trait, Type, Const, Static, Let,

    // 控制流
    If, Else, Match, Case, Default,
    For, While, Loop, Break, Continue,

    // 返回与跳转
    Return, Yield, Await,

    // 异步
    Async, Parallel,

    // 模块
    Import, From, As, Export,

    // 异常
    Try, Except, Finally, Raise, Throw,

    // 可见性
    Pub, Priv,

    // 类型相关
    Where, SelfKw, SelfLower,

    // 字面量
    True, False, Null,

    // 其他
    In, Is, NotIn, IsNot,
}
```

#### 4.1.2 字面量类型

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum LiteralKind {
    Int(i64),
    Float(f64),
    String(String),
    RawString(String),
    Bytes(Vec<u8>),
    Char(char),
    Bool(bool),
}
```

#### 4.1.3 Lexer实现要求

- 使用 `logos` 作为实现基础
- 支持Unicode标识符
- 保留位置信息（行号、列号）
- 支持shebang (`#!/usr/bin/env sgc`)
- 支持字符串插值标记
- 支持原始字符串（r"..."）

### 4.2 Parser（语法分析器）

#### 4.2.1 AST定义

```rust
// compiler/ast/src/lib.rs

use std::collections::HashMap;
use crate::symbol::Symbol;

/// AST根节点
#[derive(Debug, Clone)]
pub struct Ast {
    pub files: Vec<File>,
}

/// 源文件
#[derive(Debug, Clone)]
pub struct File {
    pub shebang: Option<String>,
    pub attrs: Vec<Attribute>,
    pub items: Vec<Item>,
    pub span: Span,
}

/// 属性
#[derive(Debug, Clone)]
pub struct Attribute {
    pub name: Ident,
    pub args: Vec<Expr>,
    pub span: Span,
}

/// 顶层项
#[derive(Debug, Clone)]
pub enum Item {
    Fn(FnItem),
    Struct(StructItem),
    Class(ClassItem),
    Enum(EnumItem),
    Trait(TraitItem),
    Impl(ImplItem),
    Const(ConstItem),
    Static(StaticItem),
    TypeAlias(TypeAliasItem),
    Import(ImportItem),
    Module(ModuleItem),
}

/// 函数定义
#[derive(Debug, Clone)]
pub struct FnItem {
    pub attrs: Vec<Attribute>,
    pub vis: Visibility,
    pub name: Ident,
    pub type_params: Vec<TypeParam>,
    pub params: Vec<Param>,
    pub return_type: Option<Type>,
    pub body: Block,
    pub is_async: bool,
    pub span: Span,
}

/// 参数
#[derive(Debug, Clone)]
pub struct Param {
    pub attrs: Vec<Attribute>,
    pub name: Ident,
    pub ty: Option<Type>,
    pub default: Option<Expr>,
    pub span: Span,
}

/// 块
#[derive(Debug, Clone)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub expr: Option<Expr>,
    pub span: Span,
}

/// 语句
#[derive(Debug, Clone)]
pub enum Stmt {
    Local(LocalStmt),
    Item(Item),
    Expr(ExprStmt),
    Semi( SemiStmt),
    Return(ReturnStmt),
    Yield(YieldStmt),
    Await(AwaitStmt),
    If(IfStmt),
    While(WhileStmt),
    For(ForStmt),
    Loop(LoopStmt),
    Match(MatchStmt),
    Try(TryStmt),
    Break(BreakStmt),
    Continue(ContinueStmt),
}

/// 语句：let绑定
#[derive(Debug, Clone)]
pub struct LocalStmt {
    pub attrs: Vec<Attribute>,
    pub pat: Pattern,
    pub ty: Option<Type>,
    pub init: Option<Expr>,
    pub span: Span,
}

/// 表达式语句
#[derive(Debug, Clone)]
pub struct ExprStmt {
    pub expr: Expr,
    pub span: Span,
}

/// 分号语句
#[derive(Debug, Clone)]
pub struct SemiStmt {
    pub expr: Expr,
    pub span: Span,
}

/// return语句
#[derive(Debug, Clone)]
pub struct ReturnStmt {
    pub value: Option<Expr>,
    pub span: Span,
}

/// yield语句
#[derive(Debug, Clone)]
pub struct YieldStmt {
    pub value: Option<Expr>,
    pub span: Span,
}

/// await语句
#[derive(Debug, Clone)]
pub struct AwaitStmt {
    pub expr: Expr,
    pub span: Span,
}

/// if语句
#[derive(Debug, Clone)]
pub struct IfStmt {
    pub cond: Expr,
    pub then_block: Block,
    pub else_ifs: Vec<(Expr, Block)>,
    pub else_block: Option<Block>,
    pub span: Span,
}

/// while语句
#[derive(Debug, Clone)]
pub struct WhileStmt {
    pub label: Option<Label>,
    pub cond: Expr,
    pub body: Block,
    pub span: Span,
}

/// for语句
#[derive(Debug, Clone)]
pub struct ForStmt {
    pub label: Option<Label>,
    pub pat: Pattern,
    pub iter: Expr,
    pub body: Block,
    pub span: Span,
}

/// loop语句
#[derive(Debug, Clone)]
pub struct LoopStmt {
    pub label: Option<Label>,
    pub body: Block,
    pub span: Span,
}

/// match语句
#[derive(Debug, Clone)]
pub struct MatchStmt {
    pub scrutinee: Expr,
    pub arms: Vec<MatchArm>,
    pub span: Span,
}

/// match分支
#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pat: Pattern,
    pub guard: Option<Expr>,
    pub body: Expr,
    pub span: Span,
}

/// try语句
#[derive(Debug, Clone)]
pub struct TryStmt {
    pub body: Block,
    pub catches: Vec<CatchClause>,
    pub finally_block: Option<Block>,
    pub span: Span,
}

/// catch子句
#[derive(Debug, Clone)]
pub struct CatchClause {
    pub pat: Pattern,
    pub cond: Option<Expr>,
    pub body: Block,
    pub span: Span,
}

/// break语句
#[derive(Debug, Clone)]
pub struct BreakStmt {
    pub label: Option<Label>,
    pub value: Option<Expr>,
    pub span: Span,
}

/// continue语句
#[derive(Debug, Clone)]
pub struct ContinueStmt {
    pub label: Option<Label>,
    pub span: Span,
}

/// 标签
#[derive(Debug, Clone)]
pub struct Label {
    pub name: Ident,
}

/// 表达式
#[derive(Debug, Clone)]
pub enum Expr {
    // 字面量
    Lit(Literal),

    // 路径
    Path(PathExpr),

    // 运算符
    Unary(UnaryOp, Box<Expr>),
    Binary(BinaryOp, Box<Expr>, Box<Expr>),

    // 复合字面量
    Array(Vec<Expr>),
    Tuple(Vec<Expr>),
    Dict(Vec<(Expr, Expr)>),

    // 控制流
    If(Box<IfExpr>),
    Match(Box<MatchExpr>),
    Block(Block),
    Loop(Box<LoopExpr>),

    // 函数相关
    Call(Box<Expr>, Vec<Expr>),
    MethodCall(Box<Expr>, Ident, Vec<Expr>),
    Lambda(LambdaExpr),

    // 结构体
    Struct(StructExpr),

    // 索引和切片
    Index(Box<Expr>, Box<Expr>),
    Slice(Box<Expr>, Option<Box<Expr>>, Option<Box<Expr>>),

    // 引用
    Ref(Box<Expr>),
    Deref(Box<Expr>),

    // 异步
    Await(Box<Expr>),
    Yield(Box<Expr>),

    // 其他
    Field(Box<Expr>, Ident),
    Paren(Box<Expr>),
    Try(Box<Expr>),
    TypeAscription(Box<Expr>, Type),
    Assign(Box<Expr>, AssignOp, Box<Expr>),

    // 错误占位
    Error,
}

/// 字面量
#[derive(Debug, Clone)]
pub enum Literal {
    Int(i64),
    Float(f64),
    String(String),
    Bytes(Vec<u8>),
    Bool(bool),
    Null,
    Char(char),
}

/// 路径表达式
#[derive(Debug, Clone)]
pub struct PathExpr {
    pub segments: Vec<PathSegment>,
    pub span: Span,
}

/// 路径段
#[derive(Debug, Clone)]
pub struct PathSegment {
    pub name: Ident,
    pub args: Option<PathArgs>,
}

/// 路径参数
#[derive(Debug, Clone)]
pub enum PathArgs {
    TypeArgs(Vec<Type>),
    ConstArgs(Vec<Expr>),
}

/// 一元运算符
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,   // -
    Not,   // !
    BitNot,// ~
    Ref,   // &
    Deref, // *
}

/// 二元运算符
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    // 算术
    Add, Sub, Mul, Div, Mod,

    // 位运算
    BitAnd, BitOr, BitXor, Shl, Shr,

    // 逻辑
    And, Or,

    // 比较
    Eq, NotEq, Lt, Gt, Le, Ge,

    // 赋值
    Assign, AddAssign, SubAssign,
    MulAssign, DivAssign, ModAssign,
    BitAndAssign, BitOrAssign, BitXorAssign,
    ShlAssign, ShrAssign,
}

/// if表达式
#[derive(Debug, Clone)]
pub struct IfExpr {
    pub cond: Expr,
    pub then_block: Block,
    pub else_block: Option<Box<Expr>>,
    pub span: Span,
}

/// match表达式
#[derive(Debug, Clone)]
pub struct MatchExpr {
    pub scrutinee: Expr,
    pub arms: Vec<MatchArm>,
    pub span: Span,
}

/// loop表达式
#[derive(Debug, Clone)]
pub struct LoopExpr {
    pub label: Option<Label>,
    pub kind: LoopKind,
    pub body: Block,
    pub span: Span,
}

/// loop类型
#[derive(Debug, Clone)]
pub enum LoopKind {
    Loop,
    While(Expr),
    For(Pattern, Expr),
}

/// Lambda表达式
#[derive(Debug, Clone)]
pub struct LambdaExpr {
    pub params: Vec<Param>,
    pub return_type: Option<Type>,
    pub body: Box<Expr>,
    pub span: Span,
}

/// 结构体表达式
#[derive(Debug, Clone)]
pub struct StructExpr {
    pub path: Path,
    pub fields: Vec<StructField>,
    pub rest: Option<Box<Expr>>,
    pub span: Span,
}

/// 结构体字段
#[derive(Debug, Clone)]
pub struct StructField {
    pub name: Ident,
    pub value: Expr,
}

/// 赋值运算符
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignOp {
    Assign, Add, Sub, Mul, Div, Mod,
    BitAnd, BitOr, BitXor, Shl, Shr,
}

/// 模式
#[derive(Debug, Clone)]
pub enum Pattern {
    // 通配符
    Wild,

    // 字面量
    Lit(Literal),

    // 路径
    Path(Path),

    // 元组
    Tuple(Vec<Pattern>),

    // 结构体
    Struct(Path, Vec<StructFieldPat>),

    // 数组
    Array(Vec<Pattern>),

    // 切片
    Slice(Vec<Pattern>, Box<Pattern>, Vec<Pattern>),

    // 或模式
    Or(Box<Pattern>, Box<Pattern>),

    // 错误
    Error,
}

/// 结构体字段模式
#[derive(Debug, Clone)]
pub struct StructFieldPat {
    pub name: Ident,
    pub pat: Option<Box<Pattern>>,
}

/// 类型
#[derive(Debug, Clone)]
pub enum Type {
    // 基础类型
    Unit,
    Never,
    Bool,
    Int(IntKind),
    Float(FloatKind),
    String,
    Bytes,
    Char,

    // 复合类型
    Array(Box<Type>),
    Slice(Box<Type>),
    Tuple(Vec<Type>),
    Dict(Box<Type>, Box<Type>),

    // 函数
    Fn(Vec<Type>, Box<Type>),

    // 特殊类型
    Option(Box<Type>),
    Result(Box<Type>, Box<Type>),

    // 路径
    Path(Path),

    // 泛型
    Generic(Box<Type>, Vec<Type>),

    // 特征边界
    TraitBound(Vec<TraitBound>),

    // 类型变量
    Var(Symbol),

    // 未知
    Unknown,
}

/// 整数类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntKind {
    I8, I16, I32, I64, I128,
    U8, U16, U32, U64, U128,
    Isize, Usize,
}

/// 浮点类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatKind {
    F32, F64,
}

/// 类型参数
#[derive(Debug, Clone)]
pub struct TypeParam {
    pub name: Ident,
    pub bounds: Vec<TraitBound>,
    pub default: Option<Type>,
}

/// 特征边界
#[derive(Debug, Clone)]
pub enum TraitBound {
    Trait(Path),
    Out(Type),
}

/// 结构体定义
#[derive(Debug, Clone)]
pub struct StructItem {
    pub attrs: Vec<Attribute>,
    pub vis: Visibility,
    pub name: Ident,
    pub type_params: Vec<TypeParam>,
    pub fields: Vec<StructFieldDef>,
    pub span: Span,
}

/// 结构体字段定义
#[derive(Debug, Clone)]
pub struct StructFieldDef {
    pub attrs: Vec<Attribute>,
    pub vis: Visibility,
    pub name: Ident,
    pub ty: Type,
}

/// 类定义
#[derive(Debug, Clone)]
pub struct ClassItem {
    pub attrs: Vec<Attribute>,
    pub vis: Visibility,
    pub name: Ident,
    pub type_params: Vec<TypeParam>,
    pub extends: Option<Path>,
    pub fields: Vec<StructFieldDef>,
    pub methods: Vec<FnItem>,
    pub span: Span,
}

/// 枚举定义
#[derive(Debug, Clone)]
pub struct EnumItem {
    pub attrs: Vec<Attribute>,
    pub vis: Visibility,
    pub name: Ident,
    pub type_params: Vec<TypeParam>,
    pub variants: Vec<EnumVariant>,
    pub span: Span,
}

/// 枚举变体
#[derive(Debug, Clone)]
pub struct EnumVariant {
    pub attrs: Vec<Attribute>,
    pub name: Ident,
    pub fields: Vec<VariantField>,
}

/// 变体字段
#[derive(Debug, Clone)]
pub enum VariantField {
    Unit,
    Tuple(Vec<Type>),
    Struct(Vec<StructFieldDef>),
}

/// Trait定义
#[derive(Debug, Clone)]
pub struct TraitItem {
    pub attrs: Vec<Attribute>,
    pub vis: Visibility,
    pub name: Ident,
    pub type_params: Vec<TypeParam>,
    pub bounds: Vec<TraitBound>,
    pub items: Vec<AssocItem>,
    pub span: Span,
}

/// 关联项
#[derive(Debug, Clone)]
pub enum AssocItem {
    Fn(FnItem),
    Type(TypeAliasItem),
    Const(ConstItem),
}

/// impl块
#[derive(Debug, Clone)]
pub struct ImplItem {
    pub attrs: Vec<Attribute>,
    pub type_params: Vec<TypeParam>,
    pub trait_path: Option<Path>,
    pub self_ty: Type,
    pub items: Vec<AssocItem>,
    pub span: Span,
}

/// 常量定义
#[derive(Debug, Clone)]
pub struct ConstItem {
    pub attrs: Vec<Attribute>,
    pub vis: Visibility,
    pub name: Ident,
    pub ty: Type,
    pub init: Expr,
    pub span: Span,
}

/// 静态变量定义
#[derive(Debug, Clone)]
pub struct StaticItem {
    pub attrs: Vec<Attribute>,
    pub vis: Visibility,
    pub is_mut: bool,
    pub name: Ident,
    pub ty: Type,
    pub init: Expr,
    pub span: Span,
}

/// 类型别名
#[derive(Debug, Clone)]
pub struct TypeAliasItem {
    pub attrs: Vec<Attribute>,
    pub vis: Visibility,
    pub name: Ident,
    pub type_params: Vec<TypeParam>,
    pub ty: Type,
    pub span: Span,
}

/// 导入项
#[derive(Debug, Clone)]
pub struct ImportItem {
    pub attrs: Vec<Attribute>,
    pub path: Path,
    pub kind: ImportKind,
    pub span: Span,
}

/// 导入类型
#[derive(Debug, Clone)]
pub enum ImportKind {
    Simple(Option<Ident>),
    Glob,
    List(Vec<(Ident, Option<Ident>)>),
}

/// 模块项
#[derive(Debug, Clone)]
pub struct ModuleItem {
    pub attrs: Vec<Attribute>,
    pub vis: Visibility,
    pub name: Ident,
    pub items: Vec<Item>,
    pub span: Span,
}

/// 可见性
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Public,
    Private,
    Restricted(Option<Path>),
}

/// 标识符
#[derive(Debug, Clone)]
pub struct Ident {
    pub name: Symbol,
    pub span: Span,
}

/// 路径
#[derive(Debug, Clone)]
pub struct Path {
    pub segments: Vec<PathSegment>,
    pub span: Span,
    pub is_absolute: bool,
}

/// 位置信息
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    pub lo: u32,
    pub hi: u32,
}

impl Span {
    pub fn new(lo: u32, hi: u32) -> Self {
        Self { lo, hi }
    }

    pub fn dummy() -> Self {
        Self { lo: 0, hi: 0 }
    }
}
```

#### 4.2.2 Parser实现要求

- 使用递归下降解析或解析器组合子库（chumsky）
- 支持错误恢复
- 提供清晰的错误信息
- 保留语法糖以便后续处理

### 4.3 Type Checker（类型检查器）

#### 4.3.1 类型定义

```rust
// compiler/typeck/src/ty.rs

use crate::symbol::Symbol;

/// 类型表示
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Ty {
    // 基础类型
    Unit,
    Never,
    Bool,
    Int(IntKind),
    Float(FloatKind),
    String,
    Bytes,
    Char,

    // 复合类型
    Array(Box<Ty>),
    Slice(Box<Ty>),
    Tuple(Vec<Ty>),
    Dict(Box<Ty>, Box<Ty>),

    // 函数类型
    Fn(Vec<Ty>, Box<Ty>),

    // 特殊类型
    Option(Box<Ty>),
    Result(Box<Ty>, Box<Ty>),

    // 特征对象
    Dyn(Vec<TraitBound>),

    // 类型参数
    Param(Symbol),

    // 类型变量（用于推导）
    Var(TyVar),

    // 未知类型
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IntKind {
    I8, I16, I32, I64, I128,
    U8, U16, U32, U64, U128,
    Isize, Usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FloatKind {
    F32, F64,
}

/// 类型变量
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TyVar(pub usize);

/// 特征边界
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitBound {
    pub trait_name: Symbol,
    pub args: Vec<Ty>,
}
```

#### 4.3.2 类型环境

```rust
// compiler/typeck/src/env.rs

use std::collections::HashMap;
use crate::ty::{Ty, TraitBound};
use crate::symbol::Symbol;

/// 类型环境
pub struct TypeEnv {
    /// 值绑定：变量 -> 类型
    pub values: HashMap<Symbol, Ty>,

    /// 类型绑定：类型名 -> 类型定义
    pub types: HashMap<Symbol, TypeDef>,

    /// 特征绑定
    pub traits: HashMap<Symbol, TraitDef>,

    /// impl绑定：(特征, 类型) -> impl
    pub impls: HashMap<(Symbol, Ty), ImplDef>,

    /// 类型变量绑定
    pub ty_vars: HashMap<usize, Ty>,

    /// 父环境
    pub parent: Option<Box<TypeEnv>>,
}

/// 类型定义
#[derive(Debug, Clone)]
pub struct TypeDef {
    pub name: Symbol,
    pub kind: TypeKind,
}

#[derive(Debug, Clone)]
pub enum TypeKind {
    Struct { fields: Vec<(Symbol, Ty)> },
    Enum { variants: Vec<(Symbol, Ty)> },
    Alias { ty: Ty },
    Trait,
}

/// 特征定义
#[derive(Debug, Clone)]
pub struct TraitDef {
    pub name: Symbol,
    pub items: Vec<TraitItem>,
}

#[derive(Debug, Clone)]
pub enum TraitItem {
    Fn { name: Symbol, sig: FnSig },
    Type { name: Symbol },
    Const { name: Symbol, ty: Ty },
}

/// 函数签名
#[derive(Debug, Clone)]
pub struct FnSig {
    pub params: Vec<Ty>,
    pub return_type: Ty,
}

/// impl定义
#[derive(Debug, Clone)]
pub struct ImplDef {
    pub trait_name: Option<Symbol>,
    pub self_ty: Ty,
    pub items: Vec<TraitItem>,
}
```

#### 4.3.3 类型检查器

```rust
// compiler/typeck/src/lib.rs

use crate::ty::Ty;
use crate::env::TypeEnv;
use crate::error::TypeError;
use ast::*;

pub struct TypeChecker {
    /// 类型环境
    pub env: TypeEnv,

    /// 类型变量计数器
    pub ty_var_counter: usize,

    /// 约束
    pub constraints: Vec<Constraint>,

    /// 错误
    pub errors: Vec<TypeError>,
}

/// 约束
#[derive(Debug, Clone)]
pub enum Constraint {
    Equate(Ty, Ty),
    Trait(Ty, TraitBound),
}

impl TypeChecker {
    pub fn new() -> Self {
        let mut env = TypeEnv::new();
        Self::init_builtins(&mut env);

        Self {
            env,
            ty_var_counter: 0,
            constraints: Vec::new(),
            errors: Vec::new(),
        }
    }

    /// 初始化内置类型
    fn init_builtins(env: &mut TypeEnv) {
        // 整数类型
        env.types.insert("i8".into(), TypeDef::simple("i8", Ty::Int(IntKind::I8)));
        env.types.insert("i16".into(), TypeDef::simple("i16", Ty::Int(IntKind::I16)));
        env.types.insert("i32".into(), TypeDef::simple("i32", Ty::Int(IntKind::I32)));
        env.types.insert("i64".into(), TypeDef::simple("i64", Ty::Int(IntKind::I64)));
        env.types.insert("u8".into(), TypeDef::simple("u8", Ty::Int(IntKind::U8)));
        env.types.insert("u32".into(), TypeDef::simple("u32", Ty::Int(IntKind::U32)));
        env.types.insert("usize".into(), TypeDef::simple("usize", Ty::Int(IntKind::Usize)));

        // 浮点类型
        env.types.insert("f32".into(), TypeDef::simple("f32", Ty::Float(FloatKind::F32)));
        env.types.insert("f64".into(), TypeDef::simple("f64", Ty::Float(FloatKind::F64)));

        // 其他基础类型
        env.types.insert("bool".into(), TypeDef::simple("bool", Ty::Bool));
        env.types.insert("str".into(), TypeDef::simple("str", Ty::String));
        env.types.insert("bytes".into(), TypeDef::simple("bytes", Ty::Bytes));
        env.types.insert("char".into(), TypeDef::simple("char", Ty::Char));

        // 特殊类型
        env.types.insert("Option".into(), TypeDef::generic("Option", 1));
        env.types.insert("Result".into(), TypeDef::generic("Result", 2));
        env.types.insert("List".into(), TypeDef::generic("List", 1));
        env.types.insert("Dict".into(), TypeDef::generic("Dict", 2));
        env.types.insert("Set".into(), TypeDef::generic("Set", 1));
        env.types.insert("Tuple".into(), TypeDef::generic("Tuple", usize::MAX));

        // 单元类型
        env.types.insert("()".into(), TypeDef::simple("()", Ty::Unit));
    }

    /// 推导表达式类型
    pub fn infer_expr(&mut self, expr: &Expr) -> Ty {
        match expr {
            Expr::Lit(lit) => self.infer_literal(lit),
            Expr::Path(path) => self.infer_path(path),
            Expr::Unary(op, expr) => self.infer_unary(op, expr),
            Expr::Binary(op, left, right) => self.infer_binary(op, left, right),
            Expr::Array(elems) => self.infer_array(elems),
            Expr::Tuple(elems) => self.infer_tuple(elems),
            Expr::Dict(pairs) => self.infer_dict(pairs),
            Expr::Block(block) => self.infer_block(block),
            Expr::If(if_expr) => self.infer_if(if_expr),
            Expr::Match(match_expr) => self.infer_match(match_expr),
            Expr::Call(func, args) => self.infer_call(func, args),
            Expr::MethodCall(obj, name, args) => self.infer_method_call(obj, name, args),
            Expr::Lambda(lambda) => self.infer_lambda(lambda),
            Expr::Index(arr, idx) => self.infer_index(arr, idx),
            Expr::Field(obj, name) => self.infer_field(obj, name),
            Expr::Struct(struct_expr) => self.infer_struct(struct_expr),
            Expr::Assign(target, _, value) => self.infer_assign(target, value),
            _ => Ty::Unknown,
        }
    }

    /// 检查表达式类型
    pub fn check_expr(&mut self, expr: &Expr, expected: &Ty) -> Result<(), TypeError> {
        let actual = self.infer_expr(expr);
        self.unify(&actual, expected)
    }

    /// 类型统一
    pub fn unify(&mut self, t1: &Ty, t2: &Ty) -> Result<(), TypeError> {
        match (t1, t2) {
            // 相同类型
            (ty1, ty2) if ty1 == ty2 => Ok(()),

            // 类型变量
            (Ty::Var(v), ty) | (ty, Ty::Var(v)) => {
                self.bind_ty_var(*v, ty.clone());
                Ok(())
            }

            // 数字类型可以转换
            (Ty::Int(_), Ty::Int(_)) => Ok(()),
            (Ty::Float(_), Ty::Float(_)) => Ok(()),
            (Ty::Int(_), Ty::Float(_)) | (Ty::Float(_), Ty::Int(_)) => {
                // 允许数字类型间的转换（带警告）
                Ok(())
            }

            // 数组
            (Ty::Array(t1), Ty::Array(t2)) => self.unify(t1, t2),

            // 字典
            (Ty::Dict(k1, v1), Ty::Dict(k2, v2)) => {
                self.unify(k1, k2)?;
                self.unify(v1, v2)
            }

            // 元组
            (Ty::Tuple(elems1), Ty::Tuple(elems2)) if elems1.len() == elems2.len() => {
                for (e1, e2) in elems1.iter().zip(elems2.iter()) {
                    self.unify(e1, e2)?;
                }
                Ok(())
            }

            // Option
            (Ty::Option(t1), Ty::Option(t2)) => self.unify(t1, t2),

            // Result
            (Ty::Result(ok1, err1), Ty::Result(ok2, err2)) => {
                self.unify(ok1, ok2)?;
                self.unify(err1, err2)
            }

            // 函数
            (Ty::Fn(params1, ret1), Ty::Fn(params2, ret2)) => {
                if params1.len() != params2.len() {
                    return Err(TypeError::ArgCountMismatch {
                        expected: params1.len(),
                        found: params2.len(),
                    });
                }
                for (p1, p2) in params1.iter().zip(params2.iter()) {
                    self.unify(p1, p2)?;
                }
                self.unify(ret1, ret2)
            }

            (t1, t2) => Err(TypeError::Mismatch {
                expected: t2.clone(),
                found: t1.clone(),
            }),
        }
    }

    fn bind_ty_var(&mut self, var: usize, ty: Ty) {
        self.env.ty_vars.insert(var, ty);
    }

    fn fresh_ty_var(&mut self) -> Ty {
        let v = self.ty_var_counter;
        self.ty_var_counter += 1;
        Ty::Var(TyVar(v))
    }

    fn infer_literal(&self, lit: &Literal) -> Ty {
        match lit {
            Literal::Int(_) => Ty::Int(IntKind::I32),
            Literal::Float(_) => Ty::Float(FloatKind::F64),
            Literal::String(_) => Ty::String,
            Literal::Bytes(_) => Ty::Bytes,
            Literal::Bool(_) => Ty::Bool,
            Literal::Null => Ty::Unit,
            Literal::Char(_) => Ty::Char,
        }
    }

    fn infer_path(&self, path: &PathExpr) -> Ty {
        // 在环境中查找
        let name = path.segments.last().map(|s| s.name.to_string());
        if let Some(name) = name {
            if let Some(ty) = self.env.values.get(&name.into()) {
                return ty.clone();
            }
        }
        Ty::Unknown
    }

    fn infer_unary(&mut self, op: &UnaryOp, expr: &Expr) -> Ty {
        let ty = self.infer_expr(expr);
        match op {
            UnaryOp::Neg | UnaryOp::BitNot => {
                match ty {
                    Ty::Int(_) | Ty::Float(_) => ty,
                    _ => Ty::Unknown,
                }
            }
            UnaryOp::Not => Ty::Bool,
            UnaryOp::Ref | UnaryOp::Deref => ty,
        }
    }

    fn infer_binary(&mut self, op: &BinaryOp, left: &Expr, right: &Expr) -> Ty {
        let left_ty = self.infer_expr(left);
        let right_ty = self.infer_expr(right);

        // 尝试统一左右类型
        let _ = self.unify(&left_ty, &right_ty);

        match op {
            // 算术运算返回操作数类型
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => {
                left_ty
            }

            // 位运算返回整数类型
            BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor | BinaryOp::Shl | BinaryOp::Shr => {
                match left_ty {
                    Ty::Int(_) => left_ty,
                    _ => Ty::Unknown,
                }
            }

            // 比较运算返回布尔类型
            BinaryOp::Eq | BinaryOp::NotEq | BinaryOp::Lt | BinaryOp::Gt
            | BinaryOp::Le | BinaryOp::Ge => Ty::Bool,

            // 逻辑运算返回布尔类型
            BinaryOp::And | BinaryOp::Or => Ty::Bool,

            // 赋值返回单元类型
            BinaryOp::Assign | BinaryOp::AddAssign | BinaryOp::SubAssign
            | BinaryOp::MulAssign | BinaryOp::DivAssign | BinaryOp::ModAssign
            | BinaryOp::BitAndAssign | BinaryOp::BitOrAssign | BinaryOp::BitXorAssign
            | BinaryOp::ShlAssign | BinaryOp::ShrAssign => Ty::Unit,
        }
    }

    fn infer_array(&mut self, elems: &[Expr]) -> Ty {
        if elems.is_empty() {
            // 空数组，推导为 Array<Unknown>
            return Ty::Array(Box::new(Ty::Unknown));
        }

        // 推导第一个元素类型
        let elem_ty = self.infer_expr(&elems[0]);

        // 检查其他元素
        for elem in &elems[1..] {
            let _ = self.check_expr(elem, &elem_ty);
        }

        Ty::Array(Box::new(elem_ty))
    }

    fn infer_tuple(&mut self, elems: &[Expr]) -> Ty {
        let tys: Vec<Ty> = elems.iter().map(|e| self.infer_expr(e)).collect();
        Ty::Tuple(tys)
    }

    fn infer_dict(&mut self, pairs: &[(Expr, Expr)]) -> Ty {
        if pairs.is_empty() {
            return Ty::Dict(Box::new(Ty::Unknown), Box::new(Ty::Unknown));
        }

        let key_ty = self.infer_expr(&pairs[0].0);
        let val_ty = self.infer_expr(&pairs[0].1);

        for (k, v) in &pairs[1..] {
            let _ = self.check_expr(k, &key_ty);
            let _ = self.check_expr(v, &val_ty);
        }

        Ty::Dict(Box::new(key_ty), Box::new(val_ty))
    }

    fn infer_block(&mut self, block: &Block) -> Ty {
        // 创建新的作用域
        // self.env.push_scope();

        // 检查语句
        for stmt in &block.stmtss {
            self.check_stmt(stmt);
        }

        // 推导最后的表达式（如果有）
        let ty = block.expr.as_ref()
            .map(|e| self.infer_expr(e))
            .unwrap_or(Ty::Unit);

        // self.env.pop_scope();
        ty
    }

    fn infer_if(&mut self, if_expr: &IfExpr) -> Ty {
        // 条件必须是布尔类型
        let _ = self.check_expr(&if_expr.cond, &Ty::Bool);

        // 推导then块
        let then_ty = self.infer_block(&if_expr.then_block);

        // 推导else块（如果有）
        let else_ty = if_expr.else_block.as_ref()
            .map(|e| self.infer_expr(e))
            .unwrap_or(Ty::Unit);

        // 统一两个分支的类型
        let _ = self.unify(&then_ty, &else_ty);

        then_ty
    }

    fn infer_match(&mut self, match_expr: &MatchExpr) -> Ty {
        // 推导被匹配的表达式
        let _ = self.infer_expr(&match_expr.scrutinee);

        if match_expr.arms.is_empty() {
            return Ty::Never;
        }

        // 推导第一个臂的类型
        let first_ty = self.infer_expr(&match_expr.arms[0].body);

        // 统一所有臂的类型
        for arm in &match_expr.arms[1..] {
            let arm_ty = self.infer_expr(&arm.body);
            let _ = self.unify(&first_ty, &arm_ty);
        }

        first_ty
    }

    fn infer_call(&mut self, func: &Expr, args: &[Expr]) -> Ty {
        // 推导函数类型
        let func_ty = self.infer_expr(func);

        match func_ty {
            Ty::Fn(params, ret) => {
                if params.len() != args.len() {
                    // 参数数量不匹配
                }

                // 检查每个参数
                for (param_ty, arg) in params.iter().zip(args.iter()) {
                    let _ = self.check_expr(arg, param_ty);
                }

                *ret
            }
            _ => Ty::Unknown,
        }
    }

    fn infer_method_call(&mut self, obj: &Expr, _name: &Ident, args: &[Expr]) -> Ty {
        // 推导对象类型
        let _ = self.infer_expr(obj);

        // 查找方法（简化版）
        // 实际实现需要在类型定义中查找方法

        // 推导参数
        for arg in args {
            let _ = self.infer_expr(arg);
        }

        // 返回未知类型（实际应该返回方法的返回类型）
        Ty::Unknown
    }

    fn infer_lambda(&mut self, lambda: &LambdaExpr) -> Ty {
        // 创建新的作用域
        // self.env.push_scope();

        // 绑定参数
        for param in &lambda.params {
            let ty = param.ty.clone().unwrap_or_else(|| Ty::Unknown);
            // self.env.values.insert(param.name, ty);
        }

        // 推导返回类型
        let ret_ty = self.infer_expr(&lambda.body);

        // self.env.pop_scope();

        // 构建函数类型
        let param_tys: Vec<Ty> = lambda.params.iter()
            .map(|p| p.ty.clone().unwrap_or(Ty::Unknown))
            .collect();

        Ty::Fn(param_tys, Box::new(ret_ty))
    }

    fn infer_index(&mut self, arr: &Expr, _idx: &Expr) -> Ty {
        let arr_ty = self.infer_expr(arr);
        match arr_ty {
            Ty::Array(elem_ty) => *elem_ty,
            Ty::Slice(elem_ty) => *elem_ty,
            Ty::Tuple(_) => Ty::Unknown, // 索引具体元素
            Ty::Dict(_, val_ty) => *val_ty,
            _ => Ty::Unknown,
        }
    }

    fn infer_field(&mut self, obj: &Expr, _name: &Ident) -> Ty {
        let obj_ty = self.infer_expr(obj);

        // 查找字段（简化版）
        // 实际实现需要在类型定义中查找字段

        match obj_ty {
            Ty::Unknown => Ty::Unknown,
            _ => Ty::Unknown,
        }
    }

    fn infer_struct(&mut self, _struct_expr: &StructExpr) -> Ty {
        // 查找结构体定义
        // 返回结构体类型
        Ty::Unknown
    }

    fn infer_assign(&mut self, target: &Expr, value: &Expr) -> Ty {
        let target_ty = self.infer_expr(target);
        let _ = self.check_expr(value, &target_ty);
        Ty::Unit
    }

    fn check_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Local(local) => {
                let init_ty = local.init.as_ref()
                    .map(|e| self.infer_expr(e))
                    .unwrap_or(Ty::Unit);
                // 绑定变量
            }
            Stmt::Expr(expr_stmt) => {
                self.infer_expr(&expr_stmt.expr);
            }
            Stmt::Semi(semi_stmt) => {
                self.infer_expr(&semi_stmt.expr);
            }
            Stmt::Return(ret) => {
                if let Some(value) = &ret.value {
                    self.infer_expr(value);
                }
            }
            Stmt::If(if_stmt) => {
                let _ = self.check_expr(&if_stmt.cond, &Ty::Bool);
                self.infer_block(&if_stmt.then_block);
                for (_, block) in &if_stmt.else_ifs {
                    self.infer_block(block);
                }
                if let Some(else_block) = &if_stmt.else_block {
                    self.infer_block(else_block);
                }
            }
            Stmt::While(while_stmt) => {
                let _ = self.check_expr(&while_stmt.cond, &Ty::Bool);
                self.infer_block(&while_stmt.body);
            }
            Stmt::For(for_stmt) => {
                let _ = self.infer_expr(&for_stmt.iter);
                self.infer_block(&for_stmt.body);
            }
            Stmt::Loop(loop_stmt) => {
                self.infer_block(&loop_stmt.body);
            }
            Stmt::Match(match_stmt) => {
                self.infer_expr(&match_stmt.scrutinee);
                for arm in &match_stmt.arms {
                    self.infer_expr(&arm.body);
                }
            }
            _ => {}
        }
    }
}
```

#### 4.3.4 类型错误

```rust
// compiler/typeck/src/error.rs

use crate::ty::Ty;
use miette::Diagnostic;
use thiserror::Error;

#[derive(Debug, Diagnostic, Error)]
pub enum TypeError {
    #[error("类型不匹配: 期望 {expected}, 找到 {found}")]
    #[diagnostic(code(typeck::mismatch))]
    Mismatch {
        expected: Ty,
        found: Ty,
    },

    #[error("参数数量不匹配: 期望 {expected} 个, 找到 {found} 个")]
    #[diagnostic(code(typeck::arg_count_mismatch))]
    ArgCountMismatch {
        expected: usize,
        found: usize,
    },

    #[error("未定义的变量: {name}")]
    #[diagnostic(code(typeck::undefined_var))]
    UndefinedVar {
        name: String,
    },

    #[error("未定义的类型: {name}")]
    #[diagnostic(code(typeck::undefined_type))]
    UndefinedType {
        name: String,
    },

    #[error("未定义的方法: {method}")]
    #[diagnostic(code(typeck::undefined_method))]
    UndefinedMethod {
        method: String,
    },

    #[error("未定义的字段: {field}")]
    #[diagnostic(code(typeck::undefined_field))]
    UndefinedField {
        field: String,
    },

    #[error("类型不支持该操作")]
    #[diagnostic(code(typeck::invalid_operation))]
    InvalidOperation,

    #[error("特征未实现: {trait_name}")]
    #[diagnostic(code(typeck::trait_not_implemented))]
    TraitNotImplemented {
        trait_name: String,
    },
}
```

---

## 5. 编译器后端规范

### 5.1 HIR（高级中间表示）

```rust
// compiler/hir/src/lib.rs

use crate::ty::Ty;

/// HIR模块
pub struct Module {
    pub name: String,
    pub items: Vec<HIRItem>,
}

/// HIR项
pub enum HIRItem {
    Function(HIRFunction),
    Struct(HIRStruct),
    Enum(HIREnum),
    Trait(HIRTrait),
    Impl(HIRImpl),
    Const(HIRConst),
    Static(HIRStatic),
}

/// HIR函数
pub struct HIRFunction {
    pub name: String,
    pub type_params: Vec<String>,
    pub params: Vec<(String, Ty)>,
    pub return_type: Ty,
    pub body: HIRBody,
    pub is_async: bool,
}

/// HIR表达式
pub enum HIRExpr {
    // 字面量
    Lit(HIRLiteral),

    // 变量
    Var(String),

    // 运算
    Unary(HIRUnaryOp, Box<HIRExpr>),
    Binary(HIRBinaryOp, Box<HIRExpr>, Box<HIRExpr>),

    // 控制流
    If(Box<HIRExpr>, Box<HIRBody>, Option<Box<HIRBody>>),
    Match(Box<HIRExpr>, Vec<HIRMatchArm>),
    Loop(Box<HIRBody>),
    While(Box<HIRExpr>, Box<HIRBody>),
    For(String, Box<HIRExpr>, Box<HIRBody>),

    // 函数调用
    Call(Box<HIRExpr>, Vec<HIRExpr>),
    MethodCall(Box<HIRExpr>, String, Vec<HIRExpr>),

    // 结构体
    Struct(String, Vec<(String, HIRExpr)>),

    // 数组
    Array(Vec<HIRExpr>),
    Index(Box<HIRExpr>, Box<HIRExpr>),

    // 字段访问
    Field(Box<HIRExpr>, String),

    // 赋值
    Assign(Box<HIRExpr>, Box<HIRExpr>),

    // 返回
    Return(Option<Box<HIRExpr>>),

    // 块
    Block(HIRBody),

    // 类型转换
    Cast(Box<HIRExpr>, Ty),
}

/// HIR字面量
pub enum HIRLiteral {
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
    Null,
}

/// HIR一元运算符
pub enum HIRUnaryOp {
    Neg, Not, BitNot,
}

/// HIR二元运算符
pub enum HIRBinaryOp {
    Add, Sub, Mul, Div, Mod,
    BitAnd, BitOr, BitXor, Shl, Shr,
    And, Or,
    Eq, NotEq, Lt, Gt, Le, Ge,
}

/// HIR块
pub struct HIRBody {
    pub stmts: Vec<HIRStmt>,
    pub expr: Option<Box<HIRExpr>>,
}

/// HIR语句
pub enum HIRStmt {
    Let(String, Ty, Option<HIRExpr>),
    Expr(HIRExpr),
}

/// HIR match分支
pub struct HIRMatchArm {
    pub pat: HIRPattern,
    pub guard: Option<HIRExpr>,
    pub body: HIRExpr,
}

/// HIR模式
pub enum HIRPattern {
    Wild,
    Lit(HIRLiteral),
    Var(String),
    Struct(String, Vec<(String, Option<HIRPattern>)>),
    Tuple(Vec<HIRPattern>),
    Or(Box<HIRPattern>, Box<HIRPattern>),
}
```

### 5.2 MIR（中级中间表示）

```rust
// compiler/mir/src/lib.rs

/// MIR函数
pub struct MirFunction {
    pub name: String,
    pub params: Vec<MirLocal>,
    pub locals: Vec<MirLocal>,
    pub blocks: Vec<MirBasicBlock>,
}

/// MIR局部变量
pub struct MirLocal {
    pub name: String,
    pub ty: Ty,
}

/// MIR基本块
pub struct MirBasicBlock {
    pub statements: Vec<MirStmt>,
    pub terminator: Option<MirTerminator>,
}

/// MIR语句
pub enum MirStmt {
    // 赋值
    Assign(MirPlace, MirRvalue),

    // 存储到内存
    Store(MirPlace, MirOperand),

    // 设置字段
    SetField(MirPlace, String, MirOperand),

    // 断言
    Assert(MirOperand, String),
}

/// MIR终止符
pub enum MirTerminator {
    // 返回
    Return(Option<MirOperand>),

    // 跳转
    Goto(MirBasicBlockId),

    // 函数调用
    Call {
        func: MirOperand,
        args: Vec<MirOperand>,
        destination: MirPlace,
        target: MirBasicBlockId,
        cleanup: Option<MirBasicBlockId>,
    },

    // 条件跳转
    Switch {
        cond: MirOperand,
        true_block: MirBasicBlockId,
        false_block: MirBasicBlockId,
    },

    // 匹配跳转
    Match {
        scrutinee: MirOperand,
        targets: Vec<(MirPattern, MirBasicBlockId)>,
        default: MirBasicBlockId,
    },

    // 不可达
    Unreachable,
}

/// MIR位置
pub enum MirPlace {
    Local(MirLocalId),
    Field(Box<MirPlace>, String),
    Index(Box<MirPlace>, Box<MirOperand>),
    Deref(Box<MirOperand>),
}

/// MIR右值
pub enum MirRvalue {
    // 运算
    UnaryOp(MirUnaryOp, MirOperand),
    BinaryOp(MirBinaryOp, MirOperand, MirOperand),

    // 聚合
    Array(Vec<MirOperand>),
    Tuple(Vec<MirOperand>),
    Struct(String, Vec<(String, MirOperand)>),

    // 引用
    Ref(MirOperand),
    Deref(MirOperand),

    // 转换
    Cast(MirOperand, Ty),

    // 长度
    Len(MirOperand),
}

/// MIR操作数
pub enum MirOperand {
    Copy(MirPlace),
    Move(MirPlace),
    Constant(MirConstant),
}

/// MIR常量
pub enum MirConstant {
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
    Null,
    Fn(String),
}

/// MIR一元运算符
pub enum MirUnaryOp {
    Neg, Not, BitNot,
}

/// MIR二元运算符
pub enum MirBinaryOp {
    Add, Sub, Mul, Div, Mod,
    BitAnd, BitOr, BitXor, Shl, Shr,
    And, Or,
    Eq, NotEq, Lt, Gt, Le, Ge,
}
```

### 5.3 代码生成

#### 5.3.1 LLVM后端

```rust
// compiler/codegen/llvm/src/lib.rs

use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::builder::Builder;
use inkwell::values::FunctionValue;

pub struct LLVMCodeGen {
    context: Context,
    module: Module,
    builder: Builder,
}

impl LLVMCodeGen {
    pub fn new(module_name: &str) -> Self {
        let context = Context::create();
        let module = context.create_module(module_name);
        let builder = context.create_builder();

        Self {
            context,
            module,
            builder,
        }
    }

    pub fn compile(&mut self, mir: &MirFunction) {
        // 生成函数
        let fn_type = self.fn_type(mir);
        let function = self.module.add_function(&mir.name, fn_type, None);

        // 创建基本块
        let entry = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(entry);

        // 编译基本块
        for block in &mir.blocks {
            self.compile_basic_block(function, block);
        }
    }

    fn fn_type(&self, mir: &MirFunction) -> inkwell::types::FunctionType {
        // 构建函数类型
        // ...
    }

    fn compile_basic_block(&mut self, function: FunctionValue, block: &MirBasicBlock) {
        // 编译基本块
    }
}
```

#### 5.3.2 字节码后端

```rust
// compiler/codegen/bytecode/src/lib.rs

pub struct BytecodeCodeGen {
    pub instructions: Vec<Instruction>,
    pub constants: Vec<Constant>,
    pub labels: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum Instruction {
    // 栈操作
    PushConstant(usize),
    Pop,

    // 变量操作
    LoadLocal(usize),
    StoreLocal(usize),
    LoadGlobal(String),
    StoreGlobal(String),

    // 运算
    Add, Sub, Mul, Div, Mod,
    BitAnd, BitOr, BitXor, Shl, Shr,
    And, Or, Not,
    Eq, NotEq, Lt, Gt, Le, Ge,
    Neg,

    // 函数调用
    Call(usize),
    Return,

    // 控制流
    Jump(String),
    JumpIfFalse(String),

    // 其他
    Dup,
    Swap,
    Print,
}

#[derive(Debug, Clone)]
pub enum Constant {
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
    Null,
}

impl BytecodeCodeGen {
    pub fn new() -> Self {
        Self {
            instructions: Vec::new(),
            constants: Vec::new(),
            labels: Vec::new(),
        }
    }

    pub fn compile(&mut self, mir: &MirFunction) {
        // 编译MIR到字节码
    }
}
```

---

## 6. 运行时设计

### 6.1 值表示

```rust
// runtime/value/src/lib.rs
use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;

/// Sengoo值表示
#[derive(Debug, Clone)]
pub enum Value {
    Unit,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(Rc<String>),
    Bytes(Rc<Vec<u8>>),
    Array(Rc<RefCell<Vec<Value>>>),
    Dict(Rc<RefCell<HashMap<Value, Value>>>),
    Tuple(Rc<Vec<Value>>),
    Fn(Function),
    Struct(StructValue),
    Option(Option<Box<Value>>),
    Result(Result<Box<Value>, Box<Value>>),
    Null,
}

/// 函数值
#[derive(Debug, Clone)]
pub enum Function {
    Native fn(Value) -> Value),
    Closure {
        params: Vec<String>,
        body: Expr,
        env: Environment,
    },
}

/// 结构体值
#[derive(Debug, Clone)]
pub struct StructValue {
    pub type_name: String,
    pub fields: HashMap<String, Value>,
}

/// 环境（用于闭包）
#[derive(Debug, Clone)]
pub struct Environment {
    pub parent: Option<Rc<Environment>>,
    pub bindings: HashMap<String, Value>,
}

impl Value {
    /// 类型检查
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::Null => false,
            Value::Unit => false,
            Value::Int(n) => *n != 0,
            Value::Float(f) => *f != 0.0,
            Value::Option(None) => false,
            Value::Result(Err(_)) => false,
            _ => true,
        }
    }

    /// 类型名称
    pub fn type_name(&self) -> &str {
        match self {
            Value::Unit => "()",
            Value::Bool(_) => "bool",
            Value::Int(_) => "int",
            Value::Float(_) => "float",
            Value::String(_) => "str",
            Value::Bytes(_) => "bytes",
            Value::Array(_) => "list",
            Value::Dict(_) => "dict",
            Value::Tuple(_) => "tuple",
            Value::Fn(_) => "fn",
            Value::Struct(s) => &s.type_name,
            Value::Option(_) => "Option",
            Value::Result(_) => "Result",
            Value::Null => "null",
        }
    }
}
```

### 6.2 内存管理

```rust
// runtime/memory/src/lib.rs
use std::rc::Rc;
use std::cell::RefCell;
use std::collections::HashSet;

/// 内存管理器
pub struct MemoryManager {
    /// 已分配的对象
    allocated: HashSet<Rc<RefCell<dyn Traceable>>>,

    /// GC阈值
    gc_threshold: usize,
}

/// 可追踪的对象
pub trait Traceable {
    fn trace(&self, visitor: &mut dyn Visitor);
}

/// 访问者接口
pub trait Visitor {
    fn visit(&mut self, value: &Rc<RefCell<dyn Traceable>>);
}

impl MemoryManager {
    pub fn new() -> Self {
        Self {
            allocated: HashSet::new(),
            gc_threshold: 1000,
        }
    }

    /// 分配对象
    pub fn allocate<T: Traceable + 'static>(&mut self, value: T) -> Rc<RefCell<T>> {
        let obj = Rc::new(RefCell::new(value));
        self.allocated.insert(obj.clone());
        obj
    }

    /// 触发垃圾回收
    pub fn collect_garbage(&mut self, roots: &[Rc<RefCell<dyn Traceable>>]) {
        // 标记阶段
        let mut marker = Marker::new();
        for root in roots {
            root.borrow().trace(&mut marker);
        }

        // 清理阶段
        self.allocated.retain(|obj| obj.borrow().is_marked());
    }
}

struct Marker {
    marked: HashSet<usize>,
}
```

### 6.3 Python互操作

```rust
// runtime/python/src/lib.rs
use pyo3::prelude::*;
use pyo3::types::PyDict;
use crate::value::Value;

/// Python互操作层
pub struct PythonInterop {
    py: Python,
    module: Option<PyObject>,
}

impl PythonInterop {
    /// 初始化Python解释器
    pub fn new() -> Self {
        // pyo3::prepare_freethreaded_python();
        Self {
            py: Python::acquire_gil(),
            module: None,
        }
    }

    /// 导入Python模块
    pub fn import(&mut self, name: &str) -> Result<(), PyErr> {
        let module: PyObject = self.py.import(name)?.into();
        self.module = Some(module);
        Ok(())
    }

    /// 调用Python函数
    pub fn call(&self, func_name: &str, args: Vec<Value>) -> Result<Value, PyErr> {
        if let Some(module) = &self.module {
            let func = module.getattr(self.py, func_name)?;
            let py_args: Vec<PyObject> = args.into_iter()
                .map(|v| self.value_to_py(self.py, v))
                .collect::<Result<_, _>>()?;

            let result = func.call(self.py, py_args.as_slice(), None)?;
            Ok(self.py_to_value(self.py, result)?)
        } else {
            Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                "No module imported"
            ))
        }
    }

    /// Sengoo值转Python值
    fn value_to_py(&self, py: Python, value: Value) -> Result<PyObject, PyErr> {
        match value {
            Value::Unit => Ok(py.None()),
            Value::Bool(b) => Ok(b.to_object(py)),
            Value::Int(n) => Ok(n.to_object(py)),
            Value::Float(f) => Ok(f.to_object(py)),
            Value::String(s) => Ok(s.to_object(py)),
            Value::Array(arr) => {
                let py_list = pyo3::types::PyList::empty(py);
                for item in arr.borrow().iter() {
                    py_list.append(self.value_to_py(py, item.clone())?)?;
                }
                Ok(py_list.into())
            }
            Value::Dict(dict) => {
                let py_dict = PyDict::new(py);
                for (k, v) in dict.borrow().iter() {
                    py_dict.set_item(
                        self.value_to_py(py, k.clone())?,
                        self.value_to_py(py, v.clone())?
                    )?;
                }
                Ok(py_dict.into())
            }
            _ => Ok(py.None()),
        }
    }

    /// Python值转Sengoo值
    fn py_to_value(&self, py: Python, obj: PyObject) -> Result<Value, PyErr> {
        if obj.is_none(py) {
            Ok(Value::Null)
        } else if let Ok(b) = obj.extract::<bool>(py) {
            Ok(Value::Bool(b))
        } else if let Ok(n) = obj.extract::<i64>(py) {
            Ok(Value::Int(n))
        } else if let Ok(f) = obj.extract::<f64>(py) {
            Ok(Value::Float(f))
        } else if let Ok(s) = obj.extract::<String>(py) {
            Ok(Value::String(Rc::new(s)))
        } else if let Ok(list) = obj.cast_as::<pyo3::types::PyList>(py) {
            let mut arr = Vec::new();
            for item in list.iter() {
                arr.push(self.py_to_value(py, item.into())?);
            }
            Ok(Value::Array(Rc::new(RefCell::new(arr))))
        } else {
            Ok(Value::Null)
        }
    }
}
```

---

## 7. 标准库设计

### 7.1 核心模块

```
stdlib/
├── core/
│   ├── builtins.sg     # 内置函数
│   ├── types.sg        # 类型定义
│   └── traits.sg       # 核心trait
├── collections/
│   ├── list.sg         # 列表
│   ├── dict.sg         # 字典
│   ├── set.sg          # 集合
│   └── tuple.sg        # 元组
├── io/
│   ├── file.sg         # 文件操作
│   ├── stdin.sg
│   ├── stdout.sg
│   └── stderr.sg
├── net/
│   ├── http.sg         # HTTP客户端
│   ├── tcp.sg          # TCP
│   └── udp.sg          # UDP
├── concurrent/
│   ├── async.sg        # 异步运行时
│   ├── thread.sg       # 线程
│   ├── channel.sg      # 通道
│   └── sync.sg         # 同步原语
├── json/
│   └── json.sg
├── datetime/
│   └── datetime.sg
└── testing/
    ├── test.sg         # 测试框架
    └── assert.sg       # 断言
```

### 7.2 标准库示例

```python
// stdlib/core/builtins.sg

// print函数
pub fn print(args: ...) {
    std::io::stdout.write(format(args))
}

// input函数
pub fn input(prompt: str) -> str {
    if prompt != "" {
        print(prompt, end="")
    }
    return std::io::stdin.read_line()
}

// len函数
pub fn len(obj: T) -> i32 {
    match obj {
        List(items) => items.len(),
        Dict(items) => items.len(),
        Set(items) => items.len(),
        Tuple(items) => items.len(),
        Str(s) => s.len(),
        Bytes(b) => b.len(),
        _ => raise TypeError("len() unsupported type")
    }
}

// range函数
pub fn range(stop: i32) -> Range {
    Range { start: 0, stop, step: 1 }
}

pub fn range(start: i32, stop: i32) -> Range {
    Range { start, stop, step: 1 }
}

pub fn range(start: i32, stop: i32, step: i32) -> Range {
    Range { start, stop, step }
}

// 类型检查
pub fn isinstance(obj: T, type: Type) -> bool {
    obj.type_of() == type
}

pub fn type_of(obj: T) -> str {
    obj.type_name()
}

// 枚举
pub fn enumerate(iter: I) -> Enumerate {
    Enumerate { iter, index: 0 }
}

// zip
pub fn zip(*iterators) -> Zip {
    Zip { iterators }
}

// map
pub fn map(fn: F, iter: I) -> Map {
    Map { fn, iter }
}

// filter
pub fn filter(fn: F, iter: I) -> Filter {
    Filter { fn, iter }
}

// min/max
pub fn min(iter: I) -> T {
    let mut min_val = null
    for item in iter {
        if min_val == null or item < min_val {
            min_val = item
        }
    }
    return min_val
}

pub fn max(iter: I) -> T {
    let mut max_val = null
    for item in iter {
        if max_val == null or item > max_val {
            max_val = item
        }
    }
    return max_val
}

// sum
pub fn sum(iter: I) -> T {
    let total = 0
    for item in iter {
        total += item
    }
    return total
}

// any/all
pub fn any(iter: I) -> bool {
    for item in iter {
        if item {
            return true
        }
    }
    return false
}

pub fn all(iter: I) -> bool {
    for item in iter {
        if !item {
            return false
        }
    }
    return true
}

// reversed
pub fn reversed(seq: S) -> Reversed {
    Reversed { seq }
}

// sorted
pub fn sorted(iter: I, key: fn(T) -> K = |x| x, reverse: bool = false) -> List {
    let mut items = list(iter)
    items.sort_by(key)
    if reverse {
        items.reverse()
    }
    return items
}
```

```python
// stdlib/testing/test.sg

// 测试框架
pub struct TestCase {
    name: str
    setup: fn() -> ()
    teardown: fn() -> ()
    tests: List[fn() -> ()]
}

pub fn test(name: str) -> TestBuilder {
    TestBuilder {
        name,
        setup: || {},
        teardown: || {},
        tests: []
    }
}

pub struct TestBuilder {
    name: str
    setup: fn() -> ()
    teardown: fn() -> ()
    tests: List[fn() -> ()]
}

impl TestBuilder {
    pub fn setup(self, fn: fn() -> ()) -> Self {
        Self { setup: fn, ..self }
    }

    pub fn teardown(self, fn: fn() -> ()) -> Self {
        Self { teardown: fn, ..self }
    }

    pub fn test(self, name: str, fn: fn() -> ()) -> Self {
        Self { tests: self.tests + [fn], ..self }
    }

    pub fn run(self) {
        print(f"Running test suite: {self.name}")
        let passed = 0
        let failed = 0

        for test in self.tests {
            self.setup()
            let result = try {
                test()
                Ok(())
            } except e {
                Err(e)
            }
            self.teardown()

            match result {
                Ok(_) => {
                    print(f"  ✓ {test.name}")
                    passed += 1
                }
                Err(e) => {
                    print(f"  ✗ {test.name}: {e}")
                    failed += 1
                }
            }
        }

        print(f"\nResults: {passed} passed, {failed} failed")
    }
}

// 断言
pub fn assert(condition: bool, message: str = "assertion failed") {
    if !condition {
        raise AssertionError(message)
    }
}

pub fn assert_eq<T>(left: T, right: T, message: str = "") {
    if left != right {
        let msg = if message == "" {
            f"assertion failed: {left} == {right}"
        } else {
            message
        }
        raise AssertionError(msg)
    }
}

pub fn assert_ne<T>(left: T, right: T, message: str = "") {
    if left == right {
        let msg = if message == "" {
            f"assertion failed: {left} != {right}"
        } else {
            message
        }
        raise AssertionError(msg)
    }
}

pub fn assert_true(condition: bool, message: str = "") {
    if !condition {
        let msg = if message == "" {
            "assertion failed: expected true"
        } else {
            message
        }
        raise AssertionError(msg)
    }
}

pub fn assert_false(condition: bool, message: str = "") {
    if condition {
        let msg = if message == "" {
            "assertion failed: expected false"
        } else {
            message
        }
        raise AssertionError(msg)
    }
}

pub fn assert_raises<E: Exception>(fn: fn() -> (), message: str = "") {
    let result = try {
        fn()
        Ok(())
    } except e {
        Err(e)
    }

    match result {
        Err(e) if e is E => {},
        Err(e) => raise AssertionError(f"wrong exception: {e}"),
        Ok(_) => raise AssertionError("no exception raised"),
    }
}
```

---

## 8. 工具链设计

### 8.1 编译器CLI (sgc)

```bash
# 编译
sgc build main.sg                # 编译为二进制
sgc build --bytecode main.sg    # 编译为字节码
sgc build --wasm main.sg        # 编译为WASM

# 运行
sgc run main.sg                 # JIT运行
sgc run --release main.sg       # 优化运行

# 检查
sgc check main.sg               # 类型检查
sgc fmt main.sg                 # 格式化代码
sgc lint main.sg                # 静态分析

# REPL
sgc repl                        # 交互式环境
sgc repl --with numpy           # 带Python库的REPL

# 信息
sgc --version
sgc --help
sgc info                        # 项目信息
```

### 8.2 包管理器 (sgpy)

```bash
# 项目初始化
sgpy init myproject             # 创建新项目
sgpy init --lib mylib           # 创建库项目

# 依赖管理
sgpy add requests               # 添加依赖
sgpy add --dev pytest           # 添加开发依赖
sgpy add --git https://...      # 添加git依赖

# 依赖操作
sgpy install                    # 安装依赖
sgpy update                     # 更新依赖
sgpy remove requests            # 移除依赖
sgpy tree                       # 查看依赖树

# 构建
sgpy build                      # 构建项目
sgpy build --release            # 发布构建
sgpy test                       # 运行测试

# 发布
sgpy publish                    # 发布到包仓库
```

### 8.3 项目配置

```toml
# sgpy.toml (项目配置)

[package]
name = "myproject"
version = "0.1.0"
description = "My Sengoo project"
authors = ["Your Name <you@example.com>"]
license = "MIT"
repository = "https://github.com/username/myproject"
homepage = "https://myproject.dev"
keywords = ["sengoo", "example"]
categories = ["command-line-utilities"]
readme = "README.md"

[dependencies]
# Sengoo原生依赖
sengoo-async = "^0.5"
sengoo-json = "^0.3"

# Python依赖（明确标记）
python.numpy = "^2.0"
python.requests = "^2.31"

[dev-dependencies]
sengoo-test = "^0.1"
python.pytest = "^8.0"

[build-dependencies]
# 构建时依赖

[bin]
# 二进制输出
name = "myapp"
path = "src/main.sg"

[lib]
# 库配置（可选）
name = "mylib"
path = "src/lib.sg"

[[bin]]
# 多个二进制文件
name = "tool1"
path = "src/tool1.sg"

[[bin]]
name = "tool2"
path = "src/tool2.sg"

[profile.dev]
opt-level = 0
debug = true
debug-assertions = true

[profile.release]
opt-level = 3
debug = false
lto = true
strip = true

[profile.test]
opt-level = 1
debug = true
```

### 8.4 LSP服务器 (sglsp)

```json
// LSP能力
{
  "capabilities": {
    "textDocumentSync": {
      "openClose": true,
      "change": "incremental"
    },
    "completionProvider": {
      "triggerCharacters": [".", "(", " ", "@"],
      "resolveProvider": true
    },
    "hoverProvider": true,
    "definitionProvider": true,
    "typeDefinitionProvider": true,
    "implementationProvider": true,
    "referencesProvider": true,
    "documentHighlightProvider": true,
    "documentSymbolProvider": true,
    "workspaceSymbolProvider": true,
    "codeActionProvider": {
      "codeActionKinds": ["quickfix", "refactor"]
    },
    "codeLensProvider": {},
    "documentFormattingProvider": true,
    "documentRangeFormattingProvider": true,
    "documentOnTypeFormattingProvider": {
      "firstTriggerCharacter": "}"
    },
    "renameProvider": {
      "prepareProvider": true
    },
    "diagnosticProvider": {
      "interFileDependencies": true,
      "workspaceDiagnostics": true
    },
    "signatureHelpProvider": {
      "triggerCharacters": ["(", ","]
    },
    "inlayHintProvider": {
      "resolveProvider": true
    },
    "semanticTokensProvider": {
      "full": true,
      "legend": {
        "tokenTypes": ["function", "variable", "parameter", ...],
        "tokenModifiers": ["declaration", "definition", "readonly", ...]
      }
    }
  }
}
```

---

## 9. Python互操作

### 9.1 导入语法

```python
// 导入Python模块
import python.numpy as np
import python.pandas as pd
import python.requests as requests

// 从Python模块导入
from python.numpy import array, arange
from python.math import sin, cos, tan

// 使用Python库
fn main() {
    // 创建numpy数组
    let arr = np.array([1, 2, 3, 4, 5])

    // numpy操作（返回Sengoo类型）
    let sum = arr.sum()
    print(f"Sum: {sum}")

    // 范围
    let nums = np.arange(0, 10, 2)
    print(nums)

    // pandas DataFrame
    let df = pd.DataFrame({
        "A": [1, 2, 3],
        "B": [4, 5, 6]
    })
    print(df.describe())

    // HTTP请求
    let response = requests.get("https://api.example.com/data")
    let data = response.json()
    print(data)
}
```

### 9.2 类型桥接

| Sengoo 类型 | Python 类型 | 转换方式 |
|-------------|-------------|----------|
| `i32, i64` | `int` | 直接转换 |
| `f64` | `float` | 直接转换 |
| `str` | `str` | 零拷贝（共享） |
| `bytes` | `bytes` | 零拷贝 |
| `List[T]` | `list` | 遍历转换 |
| `Dict[K,V]` | `dict` | 遍历转换 |
| `Set[T]` | `set` | 遍历转换 |
| `Tuple[T,...]` | `tuple` | 遍历转换 |
| `Option[T]` | `Optional[T]` | None/值映射 |
| `Result[T,E]` | - | 异常机制映射 |
| `bool` | `bool` | 直接转换 |

### 9.3 异常处理

```python
// Python异常转Sengoo Result
import python.requests as requests

fn fetch_data(url: str) -> Result[dict, str] {
    try {
        let response = requests.get(url)
        if response.status_code == 200 {
            return Ok(response.json())
        } else {
            return Err(f"HTTP {response.status_code}")
        }
    } except requests.HTTPError as e {
        return Err(e.to_string())
    } except requests.ConnectionError as e {
        return Err("Connection failed")
    }
}

// 使用
match fetch_data("https://api.example.com") {
    Ok(data) => print(data),
    Err(e) => print(f"Error: {e}")
}
```

---

## 10. 实现路线图

### 10.1 阶段划分

#### Phase 0: 项目初始化 (1-2周)

- [ ] 创建Cargo工作空间
- [ ] 设置项目结构
- [ ] 配置开发工具（pre-commit, CI/CD）
- [ ] 编写基础文档

#### Phase 1: Lexer (1-2周)

- [ ] Token类型定义
- [ ] Lexer实现（使用logos）
- [ ] 单元测试
- [ ] 错误处理

#### Phase 2: Parser (2-3周)

- [ ] AST定义
- [ ] Parser实现（使用chumsky或自建）
- [ ] 语法测试
- [ ] 错误恢复

#### Phase 3: 类型检查 (2-3周)

- [ ] 类型定义
- [ ] 类型环境
- [ ] 类型推导
- [ ] 类型统一
- [ ] 错误诊断

#### Phase 4: HIR/MIR (2-3周)

- [ ] HIR定义
- [ ] AST到HIR转换
- [ ] MIR定义
- [ ] HIR到MIR转换
- [ ] MIR验证

#### Phase 5: LLVM后端 (3-4周)

- [ ] LLVM集成
- [ ] 类型代码生成
- [ ] 函数代码生成
- [ ] 控制流代码生成
- [ ] 数据结构代码生成

#### Phase 6: 运行时 (2-3周)

- [ ] 值表示
- [ ] 内存管理（RC + GC）
- [ ] 基本操作实现

#### Phase 7: 标准库基础 (2-3周)

- [ ] 核心类型实现
- [ ] IO操作
- [ ] 集合类型
- [ ] 字符串操作

#### Phase 8: Python互操作 (3-4周)

- [ ] pyo3集成
- [ ] 类型转换
- [ ] 模块导入
- [ ] 函数调用

#### Phase 9: 工具链基础 (2-3周)

- [ ] 编译器CLI
- [ ] REPL
- [ ] 格式化工具
- [ ] 测试框架

#### Phase 10: LSP (3-4周)

- [ ] LSP服务器
- [ ] 代码补全
- [ ] 跳转定义
- [ ] 诊断
- [ ] 代码格式化

#### Phase 11: 优化与测试 (持续)

- [ ] 性能优化
- [ ] 基准测试
- [ ] 集成测试
- [ ] 文档完善

### 10.2 MVP范围定义

**最小可行产品（MVP）应该包含：**

1. **语言核心**
   - 基础语法（{}块）
   - 变量绑定（let）
   - 函数定义与调用
   - 基本控制流（if/else, while, for）
   - 基础类型（int, float, bool, str）
   - 数组和字典字面量

2. **编译器**
   - 词法分析
   - 语法分析
   - 基础类型推导
   - LLVM代码生成
   - 可执行二进制输出

3. **运行时**
   - 基本值表示
   - 内存管理（RC）
   - 字符串操作
   - 打印函数

4. **工具链**
   - 编译器CLI（sgc）
   - 基础REPL

**MVP不包含（后续版本）：**
- 类和trait
- async/await
- Python互操作
- LSP
- 包管理器

---

## 11. 测试策略

### 11.1 测试层次

```
tests/
├── unit/                   # 单元测试
│   ├── lexer/
│   ├── parser/
│   ├── typeck/
│   └── runtime/
├── integration/            # 集成测试
│   ├── compiler/
│   └── stdlib/
├── regression/             # 回归测试
└── perf/                   # 性能测试
```

### 11.2 测试框架

```rust
// 使用 criterion 进行基准测试
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn fibonacci(n: u64) -> u64 {
    match n {
        0 => 1,
        1 => 1,
        _ => fibonacci(n - 1) + fibonacci(n - 2),
    }
}

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("fib 20", |b| b.iter(|| fibonacci(black_box(20))));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
```

### 11.3 语言测试

```python
// tests/parser/test_basic.sg

// 测试函数定义
fn add(x: i32, y: i32) -> i32 {
    return x + y
}

// 测试if/else
fn max(x: i32, y: i32) -> i32 {
    if x > y {
        return x
    } else {
        return y
    }
}

// 测试循环
fn sum(n: i32) -> i32 {
    let total = 0
    for i in 0..n {
        total += i
    }
    return total
}

// 测试模式匹配
fn describe(n: i32) -> str {
    match n {
        0 => "zero",
        1 => "one",
        2 | 3 | 5 | 7 => "prime",
        _ => "other"
    }
}
```

---

## 12. 编码规范

### 12.1 Rust代码规范

```rust
// 使用标准的Rust命名约定
mod lexer;        // 模块名: snake_case
struct Token;     // 结构体: PascalCase
enum TokenKind;   // 枚举: PascalCase
fn tokenize();    // 函数: snake_case
const MAX_SIZE;   // 常量: SCREAMING_SNAKE_CASE
```

### 12.2 错误处理

```rust
// 使用 thiserror 定义错误
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CompileError {
    #[error("语法错误: {0}")]
    Syntax(#[from] SyntaxError),

    #[error("类型错误: {0}")]
    Type(#[from] TypeError),

    #[error("IO错误: {0}")]
    Io(#[from] std::io::Error),
}

// 使用 miette 提供好的错误信息
use miette::Diagnostic;

#[derive(Debug, Diagnostic, Error)]
#[error("类型不匹配")]
#[diagnostic(
    code(typeck::mismatch),
    help("尝试使用显式类型转换")
)]
pub struct TypeMismatch {
    #[source_code]
    src: String,

    #[label("期望 {}", expected)]
    span: Span,

    expected: String,
    found: String,
}
```

### 12.3 文档注释

```rust
//! # Sengoo 编译器
//!
//! 这是Sengoo语言的编译器实现。

/// 词法分析器
///
/// 将源代码字符串转换为Token流
///
/// # 示例
///
/// ```
/// use sengoo_compiler::lexer::Lexer;
///
/// let lexer = Lexer::new("let x = 42");
/// let tokens = lexer.tokenize();
/// ```
pub struct Lexer {
    // ...
}
```

---

## 附录A: 语法BNF

```
# 文件
file       = (shebang)? (item)*

# 顶层项
item       = fn_item | struct_item | class_item | enum_item
           | trait_item | impl_item | const_item | static_item
           | type_alias_item | import_item | mod_item

# 函数
fn_item    = attribute* visibility? "fn" ident type_params? "(" param* ")"
             ("->" type)? block

# 参数
param      = attribute* ident (":" type)? ("=" expr)?

# 类型参数
type_params = "<" type_param ("," type_param)* ">"

type_param = ident (":" trait_bound)? ("=" type)?

# 块
block      = "{" stmt* expr? "}"

# 语句
stmt       = let_stmt | expr_stmt | semi_stmt
           | return_stmt | yield_stmt | await_stmt
           | if_stmt | while_stmt | for_stmt | loop_stmt
           | match_stmt | try_stmt | break_stmt | continue_stmt

# let绑定
let_stmt   = attribute* "let" pattern (":" type)? ("=" expr)? ";"

# 表达式
expr       = literal | path | unary_expr | binary_expr
           | array_expr | tuple_expr | dict_expr
           | if_expr | match_expr | block_expr | loop_expr
           | call_expr | method_call_expr | lambda_expr
           | struct_expr | index_expr | slice_expr
           | field_expr | paren_expr | await_expr | yield_expr
           | try_expr | assign_expr

# 运算符优先级（从低到高）
assign     = expr "=" expr
           | expr "+=" expr | expr "-=" expr | expr "*=" expr
           | expr "/=" expr | expr "%=" expr
logic_or   = expr "||" expr
logic_and  = expr "&&" expr
equality   = expr ("==" | "!=") expr
compare    = expr ("<" | ">" | "<=" | ">=") expr
bit_or     = expr "|" expr
bit_xor    = expr "^" expr
bit_and    = expr "&" expr
shift      = expr ("<<" | ">>") expr
additive   = expr ("+" | "-") expr
multiplicative = expr ("*" | "/" | "%") expr
unary      = ("-" | "!" | "~" | "&" | "*") expr
           | "await" expr

# 函数调用
call_expr  = expr "(" expr* ")"

# 方法调用
method_call_expr = expr "." ident "(" expr* ")"

# Lambda
lambda     = "|" param* "|" expr
           | "fn" "(" param* ")" ("->" type)? block

# 数组
array_expr = "[" expr* "]"

# 字典
dict_expr  = "{" (expr ":" expr)* "}"

# 结构体
struct_expr = path "{" (ident ":" expr)* ".." expr? "}"

# if表达式
if_expr    = "if" expr block ("else" if_else)?
if_else    = block | "if" expr block

# match表达式
match_expr = "match" expr "{" match_arm* "}"
match_arm  = pattern ("if" expr)? "=>" expr ","?

# 循环
loop_expr  = "loop" block
           | "while" expr block
           | "for" pattern "in" expr block

# 索引
index_expr = expr "[" expr "]"
slice_expr = expr "[" expr? ".." expr? "]"

# 字段访问
field_expr = expr "." ident

# 模式
pattern    = "_"
           | literal
           | ident
           | path "{" (ident ":" pattern?)? ".." ident? "}"
           | "(" pattern* ")"
           | "[" pattern* "]"
           | pattern "|" pattern

# 类型
type       = "(" ")"
           | "bool" | "str" | "bytes"
           | "i8" | "i16" | "i32" | "i64" | "i128"
           | "u8" | "u16" | "u32" | "u64" | "u128"
           | "isize" | "usize"
           | "f32" | "f64"
           | "[" type "]"
           | "(" type* ")"
           | "{" type ":" type "}"
           | "Option" "[" type "]"
           | "Result" "[" type "," type "]"
           | "fn" "(" type* ")" ("->" type)?
           | path ("[" type* "]")?

# 特征边界
trait_bound = path
           | path "(" type* ")"

# 导入
import_item = "import" import_spec
import_spec = path ("as" ident)?
           | path "::" "*"
           | "{" (ident ("as" ident)?)* "}"

# 可见性
visibility = "pub" | "pub" "(" "crate" | "self" | "super" ident* ")"
```

---

## 附录B: 术语表

| 术语 | 英文 | 说明 |
|------|------|------|
| 词法分析 | Lexical Analysis | 将源代码分解为Token流 |
| 语法分析 | Syntax Analysis | 将Token流转换为AST |
| 语义分析 | Semantic Analysis | 检查语义正确性 |
| 类型推导 | Type Inference | 自动推导表达式类型 |
| 类型统一 | Type Unification | 统一两个类型 |
| 中间表示 | Intermediate Representation | 编译器中间表示 |
| SSA | Static Single Assignment | 静态单一赋值形式 |
| HIR | High-Level IR | 高级中间表示 |
| MIR | Mid-Level IR | 中级中间表示 |
| LLVM | Low Level Virtual Machine | 编译器基础设施 |
| JIT | Just-In-Time | 即时编译 |
| AOT | Ahead-Of-Time | 预先编译 |
| GIL | Global Interpreter Lock | 全局解释器锁 |
| FFI | Foreign Function Interface | 外部函数接口 |
| REPL | Read-Eval-Print Loop | 交互式环境 |
| LSP | Language Server Protocol | 语言服务协议 |
| AST | Abstract Syntax Tree | 抽象语法树 |
| RC | Reference Counted | 引用计数 |
| GC | Garbage Collection | 垃圾回收 |

---

## 附录C: 参考资源

### 编译器设计
- *Crafting Interpreters* by Robert Nystrom
- *Engineering a Compiler* by Keith Cooper & Linda Torczon
- *Modern Compiler Implementation in ML* by Andrew Appel

### Rust编译器
- Rustc源码: https://github.com/rust-lang/rust
- Chumsky: https://github.com/zesterer/chumsky
- Logos: https://github.com/maciejhirsz/logos

### LLVM
- LLVM documentation: https://llvm.org/docs/
- Inkwell: https://github.com/TheDan64/inkwell

### Python集成
- PyO3: https://pyo3.rs/
- CPython API: https://docs.python.org/3/c-api/index.html

### 其他
- TypeScript编译器: https://github.com/microsoft/TypeScript
- Swift编译器: https://github.com/apple/swift

---

**文档版本**: v0.1.0
**最后更新**: 2025-01-16
**维护者**: Sengoo项目组

---

## 2026-02-16 Non-Normative Performance Snapshot

This section is an implementation-status snapshot and does not modify language syntax or semantic rules.

### 10k-1000k three-language e2e compile baseline

Source: `bench/results/1771252338862-advanced-pipeline.json`

| LOC | Sengoo (ms) | C++ (ms) | Rust (ms) |
|---|---:|---:|---:|
| 10k | 666.99 | 830.18 | 1225.40 |
| 100k | 1054.08 | 1145.91 | 4135.55 |
| 1000k | 6482.95 | 3373.79 | 35292.84 |

Sengoo 1000k stage split:
- Frontend: `5869.79ms` (`90.54%`)
- Codegen object: `56.19ms` (`0.87%`)
- Link: `556.97ms` (`8.59%`)

### Demo evidence

- Hot-path runtime (`Sengoo vs Python`): `bench/demos/hotpath-risk-scoring/results/1771254169774-risk-scoring-demo.json`
- Reflection ergonomics (`Sengoo auto vs C++ manual`): `bench/demos/reflection-auto-vs-cpp/results/1771255074700-reflection-auto-vs-cpp.json`
<!--
Historical design draft.

This file is preserved as project context and is not the authoritative language
reference. Parts of it are encoding-corrupted and parts describe planned
features that are not implemented. Use docs/language-reference.md as the
current source of truth for supported language behavior.
-->
