# Sengoo 编译器开发文档

> **版本**: v0.2.0
> **更新日期**: 2025-01-19
> **目标**: AI Agent 接手继续开发的完整指南

---

## 目录

1. [项目概览](#1-项目概览)
2. [快速开始](#2-快速开始)
3. [编译流程架构](#3-编译流程架构)
4. [核心模块详解](#4-核心模块详解)
5. [Trait 系统实现](#5-trait-系统实现)
6. [MIR 设计精髓](#6-mir-设计精髓)
7. [代码生成指南](#7-代码生成指南)
8. [工具链状态](#8-工具链状态)
9. [待完成工作](#9-待完成工作)
10. [开发规范](#10-开发规范)

---

## 1. 项目概览

### 1.1 项目定位

**Sengoo** 是一门现代化编程语言，目标成为 Python 的现代替代：

| 特性 | 说明 |
|------|------|
| 语法 | 类 Python，使用 `{}` 替代缩进 |
| 类型系统 | 渐进式类型，默认推导 |
| 并发 | 无 GIL，真正多线程 |
| 目标 | LLVM IR 生成，编译为原生代码 |

### 1.2 当前实现状态

| 模块 | 状态 | 说明 |
|------|------|------|
| Lexer | ✅ | 基于 `logos` |
| Parser | ✅ | 基于 `chumsky` |
| AST | ✅ | 完整定义 |
| Type Checker | ✅ | 基础类型检查 + Trait 注册表 |
| HIR | ✅ | 高级中间表示 |
| MIR | ✅ | SSA 形式，含枚举支持 |
| LLVM Codegen | ✅ | 可生成有效 LLVM IR |
| Trait Registry | ✅ | 收集和管理 Trait 定义 |
| Impl Registry | ✅ | 收集和管理 Impl 块 |
| Method Call Resolution | ✅ | 类型检查层方法解析 |

### 1.3 目录结构

```
compiler/src/
├── lib.rs              # 库入口
├── lexer/              # 词法分析 (logos)
│   ├── mod.rs          # Lexer 结构
│   └── token.rs        # Token 定义
├── parser/             # 语法分析 (chumsky)
│   ├── mod.rs          # Parser 入口
│   ├── expr.rs         # 表达式解析
│   ├── stmt.rs         # 语句解析
│   └── decl.rs         # 声明解析
├── ast/                # 抽象语法树
│   ├── mod.rs          # AST 根定义
│   ├── expr.rs         # 表达式节点
│   ├── stmt.rs         # 语句节点
│   ├── decl.rs         # 声明节点
│   ├── ty.rs           # 类型节点
│   └── pattern.rs      # 模式节点
├── typeck/             # 类型检查
│   ├── mod.rs          # 模块入口
│   ├── check.rs        # 类型检查器
│   ├── ty.rs           # 类型系统定义
│   ├── env.rs          # 类型环境
│   ├── infer.rs        # 类型推断
│   ├── borrow.rs       # 借用检查
│   └── trait.rs        # Trait/Impl 注册表 ⭐
├── hir/                # 高级中间表示
│   ├── mod.rs          # HIR 入口
│   ├── lowering.rs     # AST → HIR
│   ├── expr.rs         # HIR 表达式
│   └── item.rs         # HIR 项定义
├── mir/                # 中级中间表示 (SSA)
│   ├── mod.rs          # MIR 类型、函数定义
│   ├── lowering.rs     # HIR → MIR 转换
│   ├── inst.rs         # MIR 指令
│   ├── bb.rs           # 基本块、终止符
│   └── op.rs           # MIR 运算符
├── codegen/            # 代码生成
│   ├── mod.rs          # LLVM IR 文本生成
│   └── jit.rs          # JIT 代码生成
└── error.rs            # 错误类型

tools/
├── sgc/                # 编译器 CLI
├── sgfmt/              # 代码格式化器 (⚠️ 待修复)
├── sgpy/               # 包管理器 ✅
└── sglsp/              # LSP 语言服务器 (⚠️ 待修复)
```

---

## 2. 快速开始

### 2.1 构建项目

```bash
# 克隆项目
cd C:\Users\tomi\Desktop\Gemini\Sengoo

# 构建编译器和工具
cargo build --release

# 或只构建特定组件
cargo build -p sengoo-compiler
cargo build -p sgpy
```

### 2.2 运行测试

```bash
# 所有测试
cargo test

# 特定模块
cargo test --test lexer
cargo test --test parser
```

### 2.3 编译示例程序

```bash
# 使用编译器 CLI
cargo run --bin sgc -- build examples/01_hello.sg

# 查看生成的 LLVM IR
cat output.ll
```

### 2.4 当前可编译的示例

| 文件 | 功能 | 测试特性 |
|------|------|----------|
| `01_hello.sg` | 返回常量 | 基础返回值 |
| `02_arithmetic.sg` | 四则运算 | Binary 指令 |
| `03_variables.sg` | 变量绑定 | let 绑定 |
| `04_array.sg` | 数组初始化和访问 | Aggregate, IndexAddr |
| `05_loop.sg` | for 循环 | 循环控制流 |
| `06_lambda.sg` | Lambda 表达式 | 闭包捕获 |
| `07_if.sg` | if-else | 条件分支 |
| `08_struct.sg` | 结构体 | 字段访问 |
| `09_enum.sg` | 枚举和模式匹配 | Enum, Match |

---

## 3. 编译流程架构

### 3.1 完整流程图

```
源代码 (.sg)
    │
    ▼
┌─────────┐
│  Lexer  │ → Vec<Token>
└─────────┘
    │
    ▼
┌─────────┐
│ Parser  │ → Program (AST)
└─────────┘
    │
    ▼
┌──────────────┐
│ TypeChecker  │ → Program (带类型)
│               │   + Trait/Impl 注册
└──────────────┘
    │
    ▼
┌─────────────┐
│ HIR Lowering  │ → Vec<HIRItem>
└─────────────┘
    │
    ▼
┌─────────────┐
│ MIR Lowering  │ → Vec<MirFunction>
└─────────────┘
    │
    ▼
┌─────────────┐
│  Codegen     │ → String (LLVM IR)
└─────────────┘
```

### 3.2 使用编译器 API

```rust
use sengoo_compiler::{Lexer, Parser, TypeChecker, lower_ast, lower_hir, Codegen};

fn compile(source: &str) -> Result<String> {
    // 1. 词法分析
    let lexer = Lexer::new(source);
    let tokens = lexer.tokenize();

    // 2. 语法分析
    let parser = Parser::new(source);
    let program = parser.parse_program()?;

    // 3. 类型检查
    let mut checker = TypeChecker::new();
    checker.check_program(&program)?;

    // 4. HIR lowering
    let hir_items = lower_ast(&program)?;

    // 5. MIR lowering
    let mir_fns = lower_hir(&hir_items)?;

    // 6. 代码生成
    let mut codegen = Codegen::new();
    let llvm_ir = codegen.codegen(&mir_fns)?;

    Ok(llvm_ir)
}
```

---

## 4. 核心模块详解

### 4.1 Token 系统 (`lexer/token.rs`)

```rust
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

pub enum TokenKind {
    // 关键字
    DefKw, StructKw, EnumKw, TraitKw, ImplKw,
    LetKw, IfKw, ElseKw, ForKw, WhileKw, MatchKw,
    ReturnKw, BreakKw, ContinueKw,

    // 字面量
    Int(Option<i64>),
    Float(Option<f64>),
    String(Option<String>),
    Char(Option<char>),
    Bool(Option<bool>),

    // 标识符和运算符
    Ident,
    Plus, Minus, Star, Slash, Percent,
    Eq, NotEq, Lt, Le, Gt, Ge,
    And, Or, Not,

    // 分隔符
    LParen, RParen, LBrace, RBrace,
    LBracket, RBracket, Comma, Semicolon,
    Colon, Arrow, FatArrow,

    // 末尾
    Newline,
    Indent,
    Dedent,
    EOF,
}
```

### 4.2 AST 结构 (`ast/`)

**程序根节点**:
```rust
pub struct Program {
    pub decls: Vec<Decl>,
}
```

**声明类型** (`decl.rs`):
```rust
pub enum DeclKind {
    Function(Function),
    Struct(Struct),
    Enum(Enum),
    Class(Class),
    Trait(Trait),
    Impl(Impl),
    TypeAlias(TypeAlias),
    Const(Const),
    Static(Static),
    Import(Import),
    Module(Module),
}
```

**表达式类型** (`expr.rs`):
```rust
pub enum ExprKind {
    Literal(Literal),
    Ident(Ident),
    Unary { op: UnOp, operand: Box<Expr> },
    Binary { op: BinOp, left: Box<Expr>, right: Box<Expr> },
    Assign { target: Box<Expr>, value: Box<Expr> },
    AssignOp { op: BinOp, target: Box<Expr>, value: Box<Expr> },
    Index { base: Box<Expr>, index: Box<Expr> },
    Field { base: Box<Expr>, field: Ident },
    Call { func: Box<Expr>, args: Vec<Expr> },
    MethodCall { receiver: Box<Expr>, method: Ident, args: Vec<Expr> },
    Tuple(Vec<Expr>),
    Array(Vec<Expr>),
    Block(Block),
    If { cond: Box<Expr>, then_branch: Box<Expr>, else_branch: Option<Box<Expr>> },
    While { cond: Box<Expr>, body: Box<Stmt> },
    For { pattern: Box<Pattern>, iter: Box<Expr>, body: Box<Stmt> },
    Loop(Box<Stmt>),
    Match { scrutinee: Box<Expr>, arms: Vec<MatchArm> },
    Return(Option<Box<Expr>>),
    Break(Option<Box<Expr>>),
    Continue,
    Path(Path),
    Lambda { params: Vec<Param>, body: Box<Expr> },
}
```

**语句类型** (`stmt.rs`):
```rust
pub enum StmtKind {
    Let { name: Ident, ty: Option<Type>, value: Option<Expr> },
    Expr(Expr),
    Semi(Expr, Option<Semicolon>),
}
```

### 4.3 类型系统 (`typeck/ty.rs`)

```rust
pub struct Ty {
    pub id: TyId,
    pub kind: TyKind,
}

pub enum TyKind {
    Error,
    Unit,
    Never,
    Bool,
    Char,
    Str,
    Byte,
    Bytes,
    Int(IntKind),
    Float(FloatKind),
    Tuple(Vec<Ty>),
    Array(Box<Ty>, usize),
    Slice(Box<Ty>),
    Ref(bool, Box<Ty>),  // (mutability, inner)
    Ptr(Box<Ty>),
    Fn { params: Vec<Ty>, ret: Box<Ty>, is_variadic: bool },
    Var(TyVarId),
    Adt { name: String, args: Vec<Ty> },
    Dyn(Vec<String>),       // Trait 对象
    ImplTrait(Vec<String>),  // impl Trait
    SelfType,
    Inferred,
}

pub enum IntKind {
    I8, I16, I32, I64, I128, ISize,
    U8, U16, U32, U64, U128, USize,
}

pub enum FloatKind {
    F32, F64,
}
```

---

## 5. Trait 系统实现

### 5.1 Trait 注册表 (`typeck/trait.rs`)

**核心数据结构**:
```rust
/// Trait 信息
pub struct TraitInfo {
    pub name: String,
    pub type_params: Vec<String>,
    pub methods: HashMap<String, MethodSig>,
    pub consts: HashMap<String, Ty>,
    pub assoc_types: Vec<String>,
    pub is_pub: bool,
}

/// 方法签名
pub struct MethodSig {
    pub has_self: bool,
    pub param_types: Vec<Ty>,
    pub return_type: Ty,
}

/// Trait 注册表
pub struct TraitRegistry {
    traits: HashMap<String, Arc<TraitInfo>>,
}
```

### 5.2 Impl 注册表

```rust
/// Impl 信息
pub struct ImplInfo {
    pub target_type: Ty,
    pub trait_name: Option<String>,  // None = 固有 impl
    pub methods: HashMap<String, FunctionTy>,
    pub consts: HashMap<String, Ty>,
    pub assoc_types: HashMap<String, Ty>,
}

/// 函数类型
pub struct FunctionTy {
    pub has_self: bool,
    pub param_types: Vec<Ty>,
    pub return_type: Ty,
}

/// Impl 注册表
pub struct ImplRegistry {
    // 固有 impl
    inherent_impls: HashMap<String, Vec<ImplInfo>>,
    // Trait impl (trait_name -> type_key -> ImplInfo)
    trait_impls: HashMap<String, HashMap<String, ImplInfo>>,
}
```

### 5.3 方法调用解析 (`typeck/check.rs`)

```rust
fn check_method_call(&mut self, receiver: &Expr, method: &Ident, args: &[Expr]) -> TyResult<Ty> {
    // 1. 获取接收者类型
    let receiver_ty = self.check_expr(receiver)?;
    let receiver_key = type_key(&receiver_ty);

    // 2. 检查参数类型
    let mut arg_types = Vec::new();
    for arg in args {
        arg_types.push(self.check_expr(arg)?);
    }

    // 3. 查找固有 impl 方法
    if let Some(fn_ty) = self.impl_registry.lookup_inherent_method(&receiver_key, &method.name) {
        // 验证参数
        if fn_ty.param_types.len() != args.len() {
            return Err(TypeckError::ArgumentCountMismatch { ... });
        }
        for (expected, actual) in fn_ty.param_types.iter().zip(arg_types.iter()) {
            self.infer.unify(expected, actual)?;
        }
        return Ok(fn_ty.return_type.clone());
    }

    // 4. 查找 Trait impl 方法
    for trait_name in self.trait_registry.all_traits() {
        if let Some(fn_ty) = self.impl_registry.lookup_trait_method(
            &trait_name, &receiver_key, &method.name
        ) {
            // 验证参数
            ...
            return Ok(fn_ty.return_type.clone());
        }
    }

    // 5. 未找到方法
    Err(TypeckError::MethodNotFound { ... })
}
```

### 5.4 类型键生成

```rust
pub fn type_key(ty: &Ty) -> String {
    match &ty.kind {
        TyKind::Unit => "()".to_string(),
        TyKind::Bool => "bool".to_string(),
        TyKind::Int(int_kind) => int_kind.to_string(),
        TyKind::Adt { name, args } if args.is_empty() => name.clone(),
        TyKind::Ref(_, inner) => format!("&{}", type_key(inner)),
        TyKind::Array(elem, len) => format!("[{}; {}]", type_key(elem), len),
        // ...
    }
}
```

---

## 6. MIR 设计精髓

### 6.1 LocalKind 系统 ⚠️ **关键**

这是 MIR 最重要的概念：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LocalKind {
    /// 返回值 (索引 0)
    Return,
    /// 函数参数
    Param,
    /// 临时变量 - 寄存器值，不需要 alloca
    Temp,
    /// 用户变量 - 需要内存分配
    User,
}
```

**决策规则**:

| LocalKind | 使用场景 | 内存分配 | 访问方式 |
|-----------|----------|----------|----------|
| `Return` | 函数返回值 | 无 | 直接寄存器 |
| `Param` | 函数参数 | 无 | 直接寄存器 |
| `Temp` | 中间计算结果 | 无 | 直接寄存器 |
| `User` | `let x = ...` | `alloca` | `store`/`load` |

### 6.2 MIR 函数结构

```rust
pub struct MirFunction {
    pub name: String,
    pub params: Vec<MIRType>,
    pub return_type: MIRType,
    pub locals: Vec<(Local, MIRType)>,
    pub basic_blocks: Vec<BasicBlock>,
    pub start_block: usize,
}
```

### 6.3 MIR 指令集

```rust
pub enum Instruction {
    // 常量赋值
    Assign { destination: Local, value: MirConstant },

    // 运算
    Unary { destination: Local, op: MirUnOp, operand: Local },
    Binary { destination: Local, op: MirBinOp, left: Local, right: Local },

    // 内存操作
    Load { destination: Local, source: Local },
    Store { destination: Local, value: Local },
    AddrOf { destination: Local, source: Local },

    // 聚合类型
    Aggregate { destination: Local, fields: Vec<Local>, ty: MIRType },
    IndexAddr { destination: Local, base: Local, index: Local },
    FieldAddr { destination: Local, base: Local, field: u32 },

    // 枚举操作
    Discriminant { destination: Local, source: Local },
    EnumConstruct { destination: Local, discriminant: u32, payload: Option<Local>, enum_type: MIRType },
    ExtractPayload { destination: Local, source: Local },

    // 函数调用
    Call { destination: Local, func: String, args: Vec<Local> },

    // 其他
    Nop,
}
```

### 6.4 枚举类型 MIR 表示

```rust
pub enum MIRType {
    Enum {
        discr_type: Box<MIRType>,     // 判别值类型
        variants: Vec<(u32, Option<MIRType>)>,  // (判别值, 载荷类型)
    },
    // ...
}
```

**枚举值表示**: `{ i64, i64 }` - 第一个 i64 是判别值，第二个 i64 是载荷

---

## 7. 代码生成指南

### 7.1 Local 命名规则

```rust
fn local_name(&self, local: Local) -> String {
    match local.kind {
        LocalKind::Param  => format!("%l_{}", local.id),
        LocalKind::Temp   => format!("%t_{}", local.id),
        LocalKind::User   => format!("%u_{}", local.id),
        LocalKind::Return => format!("%ret_{}", local.id),
    }
}
```

### 7.2 MIR 到 LLVM 类型映射

| MIR 类型 | LLVM 类型 |
|----------|-----------|
| `Int(8)` | `i8` |
| `Int(16)` | `i16` |
| `Int(32)` | `i32` |
| `Int(64)` | `i64` |
| `Bool` | `i1` |
| `Float(32)` | `float` |
| `Float(64)` | `double` |
| `Enum { .. }` | `{ i64, i64 }` |
| `Array(T, n)` | `[n x T]` |
| `Ptr(T)` / `Ref(T)` | `i8*` (或具体类型指针) |

### 7.3 关键代码生成模式

**Load 指令**:
```rust
mir::Instruction::Load { destination, source } => {
    let dest = self.local_name(*destination);
    let src = self.local_name(*source);

    // 判断是否需要真的 load
    let needs_load = local_info.kind == LocalKind::User
        || matches!(src_ty, MIRType::Ptr(_) | MIRType::Ref(_));

    if needs_load {
        self.ir.push_str(&format!("{} = load {}, {}* {}\n", dest, ty, ty, src));
    } else {
        // Temp 寄存器直接复制
        self.ir.push_str(&format!("{} = add i64 0, {}\n", dest, src));
    }
}
```

**枚举构造**:
```rust
mir::Instruction::EnumConstruct { destination, discriminant, payload, .. } => {
    let dest = self.local_name(*destination);

    // 初始化为 undef
    self.ir.push_str(&format!("{} = insertvalue {{ i64, i64 }} undef, i64 {}, 0\n",
        dest, discriminant));

    // 插入载荷
    if let Some(payload_local) = payload {
        let payload_val = self.operand_value(*payload_local, mir_fn);
        self.ir.push_str(&format!("{} = insertvalue {{ i64, i64 }} {}, i64 {}, 1\n",
            dest, dest, payload_val));
    }
}
```

**判别值提取**:
```rust
mir::Instruction::Discriminant { destination, source } => {
    let dest = self.local_name(*destination);
    let src = self.local_name(*source);
    self.ir.push_str(&format!("{} = extractvalue {{ i64, i64 }} {}, 0\n", dest, src));
}
```

---

## 8. 工具链状态

### 8.1 当前状态

| 工具 | 状态 | 说明 |
|------|------|------|
| `sgc` | ✅ | 编译器 CLI，工作正常 |
| `sgpy` | ✅ | 包管理器，已修复编译错误 |
| `sgfmt` | ⚠️ | 代码格式化器，API 不匹配待修复 |
| `sglsp` | ⚠️ | LSP 服务器，tower_lsp API 兼容性问题待修复 |

### 8.2 sgpy 功能

```bash
# 初始化项目
sgpy init

# 添加依赖
sgpy add serde="1.0"

# 构建项目
sgpy build

# 发布包
sgpy publish
```

---

## 9. 待完成工作

### 9.1 高优先级 - MIR 层

- [ ] 方法调用降低到 MIR
- [ ] Trait 方法代码生成
- [ ] 动态分发 (Trait 对象 `dyn Trait`)

### 9.2 中优先级 - 标准库

- [ ] `Display`, `Debug` Trait
- [ ] `Iterator` Trait
- [ ] `Vec<T>`, `HashMap<K, V>` 基础实现

### 9.3 低优先级 - 工具链修复

- [ ] 修复 `sgfmt` API 不匹配
- [ ] 修复 `sglsp` tower_lsp 兼容性

### 9.4 长期规划

- [ ] 泛型支持
- [ ] async/await 语法
- [ ] 宏系统
- [ ] 模块系统

---

## 10. 开发规范

### 10.1 添加新语言特性流程

1. **AST 层** (`ast/`): 定义新的 AST 节点
2. **Parser 层** (`parser/`): 添加解析逻辑
3. **HIR 层** (`hir/`): 添加 HIR 转换
4. **TypeCheck 层** (`typeck/`): 添加类型检查
5. **MIR 层** (`mir/`): 添加 MIR 指令和 lowering
6. **Codegen 层** (`codegen/`): 添加 LLVM IR 生成

### 10.2 调试技巧

**打印 MIR**:
```rust
println!("=== MIR Function: {} ===", mir_fn.name);
for bb in &mir_fn.basic_blocks {
    println!("Block {}:", bb.id);
    for inst in &bb.instructions {
        println!("  {:?}", inst);
    }
    println!("  Terminator: {:?}", bb.terminator);
}
```

**验证 LLVM IR**:
```bash
# 保存 IR 到文件
sgc build example.sg -o output.ll

# 验证 IR
opt -verify output.ll

# 查看汇编
llc -filetype=asm output.ll -o output.s
```

### 10.3 关键文件清单

| 文件 | 作用 | 修改频率 |
|------|------|----------|
| `mir/lowering.rs` | HIR → MIR | 高 |
| `codegen/mod.rs` | LLVM IR 生成 | 高 |
| `typeck/trait.rs` | Trait 注册表 | 中 |
| `typeck/check.rs` | 类型检查 | 中 |
| `mir/inst.rs` | MIR 指令定义 | 低 |

---

## 附录

### A. 编译器版本信息

```toml
[package]
name = "sengoo-compiler"
version = "0.1.0"
edition = "2021"

[dependencies]
logos = "0.15"
chumsky = "1.0"
inkwell = { version = "0.5", features = ["llvm18-0"] }
```

### B. 参考资源

- [LLVM Language Reference](https://llvm.org/docs/LangRef.html)
- [SSA 论文](https://www.cs.cornell.edu/~fischer/papers/ssa-slp-tr.pdf)
- [Rust 编译器开发书](https://rustc-dev-guide.rust-lang.org/)

### C. 联系与支持

- 项目路径: `C:\Users\tomi\Desktop\Gemini\Sengoo`
- 主编译器: `compiler/`
- 工具链: `tools/`

---

**文档维护**: 每次重大架构变更后请更新本文档。
