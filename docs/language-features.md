# Sengoo 高级语言特性实现文档

## 1. 已完成的实现

### 1.1 枚举类型 (Enum)

**MIR 层表示** (`compiler/src/mir/mod.rs`):
```rust
pub enum MIRType {
    // ... 其他类型
    Enum {
        discr_type: Box<MIRType>,  // 判别值类型
        variants: Vec<(u32, Option<MIRType>)>,  // (判别值, 变体数据类型)
    },
}
```

**支持的变体类型**:
- 单元变体: `None` → `Enum { variants: [(0, None)] }`
- 元组变体: `Some(T)` → `Enum { variants: [(1, Some(T))] }`
- 结构体变体: `Ok { value: T }` → `Enum { variants: [(0, Some(StructType))] }`

### 1.2 模式匹配 (Match Expression)

**HIR 表示** (`compiler/src/hir/expr.rs`):
```rust
Match {
    scrutinee: Box<HIRExpr>,
    arms: Vec<HIRMatchArm>,
}
```

**MIR Lowering** (`compiler/src/mir/lowering.rs`):
- 枚举模式匹配 → `Switch` 终止符
- 字面量模式匹配 → `If-Else` 链
- 通配符模式 `_` → 直接执行

**LLVM 代码生成** (`compiler/src/codegen/mod.rs`):
- `Switch` → LLVM `switch` 指令
- `Discriminant` → `extractvalue` 指令
- `EnumConstruct` → `insertvalue` 指令

### 1.3 新增 MIR 指令

```rust
// 获取枚举判别值
Discriminant { destination: Local, source: Local }

// 构造枚举变体
EnumConstruct {
    destination: Local,
    discriminant: u32,
    payload: Option<Local>,
    enum_type: MIRType,
}

// 提取枚举载荷
ExtractPayload { destination: Local, source: Local }
```

## 2. Trait 系统架构

### 2.1 HIR 层定义

**Trait 定义** (`compiler/src/hir/item.rs`):
```rust
pub struct HIRTrait {
    pub name: String,
    pub type_params: Vec<String>,
    pub items: Vec<HIRTraitItem>,  // 方法、常量、类型别名
    pub is_pub: bool,
}

pub enum HIRTraitItem {
    Function(HIRFunction),
    Const(String, HIRType),
    Type(String),
}
```

**Impl 定义**:
```rust
pub struct HIRImpl {
    pub target_type: HIRType,      // impl 的目标类型
    pub trait_name: Option<String>, // impl 的 trait (None = 固有 impl)
    pub items: Vec<HIRFunction>,    // impl 的方法
}
```

### 2.2 Trait 注册表实现 (`compiler/src/typeck/trait.rs`)

**TraitRegistry**: 收集和管理 Trait 定义
```rust
pub struct TraitRegistry {
    traits: HashMap<String, Arc<TraitInfo>>,
}

pub struct TraitInfo {
    pub name: String,
    pub type_params: Vec<String>,
    pub methods: HashMap<String, MethodSig>,
    pub consts: HashMap<String, Ty>,
    pub assoc_types: Vec<String>,
    pub is_pub: bool,
}

pub struct MethodSig {
    pub has_self: bool,
    pub param_types: Vec<Ty>,
    pub return_type: Ty,
}
```

**ImplRegistry**: 收集和管理 Impl 块
```rust
pub struct ImplRegistry {
    // 固有 impl (type_key -> Vec<ImplInfo>)
    inherent_impls: HashMap<String, Vec<ImplInfo>>,
    // Trait impl (trait_name -> type_key -> ImplInfo)
    trait_impls: HashMap<String, HashMap<String, ImplInfo>>,
}

pub struct ImplInfo {
    pub target_type: Ty,
    pub trait_name: Option<String>,
    pub methods: HashMap<String, FunctionTy>,
    pub consts: HashMap<String, Ty>,
    pub assoc_types: HashMap<String, Ty>,
}
```

### 2.3 方法调用

**HIR 表示**:
```rust
MethodCall {
    receiver: Box<HIRExpr>,
    method: String,
    args: Vec<HIRExpr>,
}
```

**类型检查实现** (`compiler/src/typeck/check.rs:check_method_call`):
1. 获取接收者类型
2. 查找固有 impl 的方法
3. 查找 Trait impl 的方法
4. 验证参数类型和数量
5. 返回方法返回类型

### 2.4 Trait 解析流程

```
1. 收集所有 Trait 定义 → TraitRegistry
2. 收集所有 Impl 定义 → ImplRegistry
3. 方法调用时:
   a. 确定接收者类型
   b. 查找匹配的 Impl 块
   c. 解析方法调用为静态函数调用
   d. 对于 Trait 对象，使用动态分发
```

### 2.4 动态分发实现

对于 Trait 对象 `dyn Trait`:
```rust
// Trait 对象表示
struct TraitObject {
    vtable: *const VTable,  // 虚函数表指针
    data: *const u8,        // 数据指针
}

// 虚函数表
struct VTable {
    drop: fn(*const u8),
    size: usize,
    align: usize,
    methods: [*const fn()],  // 方法指针数组
}
```

## 3. 标准库扩展设计

### 3.1 集合模块

```rust
// 核心 trait
pub trait Iter {
    type Item;
    fn next(&mut self) -> Option<Self::Item>;
}

pub trait IntoIterator {
    type Item;
    type IntoIter: Iter<Item = Self::Item>;
    fn into_iter(self) -> Self::IntoIter;
}

// 容器
pub struct Vec<T> { /* ... */ }
pub struct HashMap<K, V> { /* ... */ }
```

### 3.2 异步支持

```rust
// async/await 语法 lowering
async fn fetch_data() -> String { ... }

// 降低为状态机
enum FetchDataStateMachine {
    Start,
    AwaitingRequest,
    Complete(String),
}
```

### 3.3 序列化

```rust
pub trait Serialize {
    fn serialize(&self) -> Vec<u8>;
}

pub trait Deserialize: Sized {
    fn deserialize(data: &[u8]) -> Result<Self>;
}
```

## 4. 实现状态

| 特性 | AST | HIR | 类型检查 | MIR | 代码生成 |
|------|-----|-----|---------|-----|---------|
| 枚举定义 | ✅ | ✅ | ✅ | ✅ | ✅ |
| 模式匹配 | ✅ | ✅ | ⚠️ | ✅ | ✅ |
| Trait 定义 | ✅ | ✅ | ✅ | ❌ | ❌ |
| Impl 块 | ✅ | ✅ | ✅ | ❌ | ❌ |
| Trait 注册表 | ✅ | ✅ | ✅ | ✅ | ✅ |
| Impl 注册表 | ✅ | ✅ | ✅ | ✅ | ✅ |
| 方法调用解析 | ✅ | ✅ | ✅ | ❌ | ❌ |
| 宏系统 | ⚠️ | ❌ | ❌ | ❌ | ❌ |

**说明**: ✅ 已实现, ⚠️ 部分实现, ❌ 未实现

**最新更新**:
- ✅ 实现了 `TraitRegistry` 用于收集和管理 Trait 定义
- ✅ 实现了 `ImplRegistry` 用于收集和管理 Impl 块
- ✅ 实现了 `check_method_call` 方法调用的类型检查
- ✅ 支持固有 impl (inherent impl) 方法查找
- ✅ 支持 Trait impl 方法查找

## 5. 下一步工作

### 5.1 优先级 1: MIR Lowering 和代码生成

1. 将方法调用降低到 MIR 层
2. 实现 Trait 方法的代码生成
3. 处理动态分发（Trait 对象）

### 5.2 优先级 2: 标准库核心

实现最常用的 Trait 和集合类型：
- `Display`, `Debug` Trait
- `Iterator` Trait 和相关适配器
- `Vec<T>`, `HashMap<K, V>` 基础实现

### 5.3 优先级 3: 异步支持

实现 async/await 语法 lowering 到状态机
