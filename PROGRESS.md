# Sengoo 编译器开发进度

## 已完成的特性

### 1. 数组类型 ✅
- **数组字面量** `[1, 2, 3]`
  - MIR lowering: `HIRExpr::Array` → `Instruction::Aggregate`
  - LLVM codegen: `alloca [N x type]` + 元素存储
  - 修改文件: `compiler/src/hir/lower.rs`, `compiler/src/mir/lowering.rs`, `compiler/src/codegen/jit.rs`

- **数组索引** `arr[i]`
  - MIR lowering: `HIRExpr::Index` → `Instruction::IndexAddr` + `Load`
  - LLVM codegen: 数组退化 + `getelementptr` 计算
  - 关键修复: `infer_expr_type` 添加数组类型推断

### 2. 结构体类型 ✅
- **结构体定义** `struct Point { x: isize, y: isize }`
  - 类型检查器支持
  - MIR 使用 `Tuple` 类型表示结构体

- **结构体实例化** `Point { x: 10, y: 20 }`
  - MIR lowering: `HIRExpr::Struct` → `Instruction::Aggregate`
  - LLVM codegen: `alloca {i64, i64}` + 字段存储

- **结构体字段访问** `p.x`
  - MIR lowering: `HIRExpr::Field` → `Instruction::FieldAddr`
  - LLVM codegen: `getelementptr` + 常量字段索引
  - 字段名映射: `x/left/r → 0`, `y/right/g → 1`, `z/b → 2`, `w/a → 3`

### 3. 数组范围迭代 ✅
- **for-in 循环** `for x in arr { ... }`
  - MIR lowering: 4块结构 (cond, body, inc, exit)
  - 支持 `IntoIterator` 协议
  - 修改文件: `compiler/src/mir/lowering.rs`
  - 测试: `tests/for_array_test.sg`

### 4. 引用系统 ✅
- **引用运算符** `&x`, `&mut x`
  - HIR lowering: `HIRExpr::Unary(Ref/RefMut, ...)`
  - MIR lowering: `Instruction::AddrOf`
  - LLVM codegen: `getelementptr` + `store`
  - 修改文件: `compiler/src/hir/lower.rs`, `compiler/src/mir/lowering.rs`, `compiler/src/codegen/jit.rs`

- **解引用运算符** `*ptr`
  - HIR lowering: `HIRExpr::Unary(Deref, ...)`
  - MIR lowering: `Instruction::Load`
  - 类型推断修复: `infer_expr_type` 正确处理 Ref/Deref 运算符

### 5. Lambda/闭包 ✅
- **Lambda 解析** `|x| x + 1`
  - AST 解析器支持
  - HIR lowering: `HIRExpr::Lambda { params, body }`
  - 类型检查器支持闭包类型（函数类型）

- **Lambda 类型检查**
  - 在 `compiler/src/typeck/check.rs` 中添加 `check_lambda` 方法
  - Lambda 参数类型使用类型变量推断
  - Lambda 类型为 `Fn { params, ret }`

- **Lambda MIR lowering**
  - 修改 `compiler/src/mir/lowering.rs`
  - 创建 Lambda 辅助函数（命名: `$__lambdaN`）
  - 使用 `lambda_names: HashMap<Local, String>` 映射 Lambda 函数名
  - Call lowering 支持 Lambda 调用

- **测试示例**
  ```sengoo
  def main() -> i64 {
      let f = |x| x + 1;
      f(10)
  }
  ```
  生成的 LLVM IR:
  ```llvm
  declare i64 @main()
  declare i64 @$__lambda0(i64)

  define i64 @main() {
      ...
      %l_4 = call @$__lambda0(i64 %l_3)
      ret %dummy %l_4
  }

  define i64 @$__lambda0(i64 %l_1) {
      ...
      %l_3 = add i64 %l_3.load, %l_3.load2
      ret %dummy %l_3
  }
  ```

- **CLI 工具实现**
  - `tools/sgc/src/main.rs` 实现完整的编译流程
  - 支持 `sgc run` 和 `sgc check` 命令

## 关键代码修改

#### `compiler/src/hir/lower.rs`
```rust
// infer_expr_type 添加引用类型支持
ast::ExprKind::Unary { op, operand } => {
    match op {
        UnOp::Ref | UnOp::RefMut => {
            let inner_ty = infer_expr_type(operand);
            HIRType::pointer(inner_ty)
        }
        UnOp::Deref | UnOp::DerefMut => {
            let inner_ty = infer_expr_type(operand);
            match inner_ty.kind {
                HIRTypeKind::Ptr(inner) => *inner,
                HIRTypeKind::Ref(_, inner) => *inner,
                _ => HIRType::int(IntKind::I64),
            }
        }
        _ => HIRType::int(IntKind::I64),
    }
}
```

#### `compiler/src/mir/lowering.rs`
```rust
// 引用运算符
HIRExpr::Unary(op, operand) => {
    match op {
        HIRUnaryOp::Ref | HIRUnaryOp::RefMut => {
            let expr_local = self.lower_expr(operand);
            let expr_ty = self.get_local_type(expr_local);
            let ptr_ty = MIRType::Ptr(Box::new(expr_ty));
            let ptr_local = self.add_local(None, LocalKind::Temp, ptr_ty);
            self.push_inst(Instruction::AddrOf {
                destination: ptr_local,
                source: expr_local,
            });
            ptr_local
        }
        HIRUnaryOp::Deref => {
            let ptr_local = self.lower_expr(operand);
            let ptr_ty = self.get_local_type(ptr_local);
            let elem_ty = match ptr_ty {
                MIRType::Ptr(inner) | MIRType::Ref(inner) => (*inner).clone(),
                _ => MIRType::int(64),
            };
            let result_local = self.add_local(None, LocalKind::Temp, elem_ty);
            self.push_inst(Instruction::Load {
                destination: result_local,
                source: ptr_local,
            });
            result_local
        }
    }
}

// 数组范围迭代
HIRExpr::For { var, iter, body } => {
    // 创建4个基本块: cond, body, inc, exit
    let cond_block = self.new_block();
    let body_block = self.new_block();
    let inc_block = self.new_block();
    let exit_block = self.new_block();
    // ... 迭代器协议实现
}
```

#### `compiler/src/codegen/jit.rs`
```rust
// AddrOf 指令
mir::Instruction::AddrOf { destination, source } => {
    let dest = self.local_name(*destination);
    let src = self.local_reg(*source);
    let src_ty = self.get_local_type(mir_fn, *source);
    let llvm_ty = self.mir_type_to_llvm_str(&src_ty);

    let temp = format!("{}.addr", dest);
    self.ir.push_str(&format!(
        "{} = getelementptr {}, {}* {}, i64 0\n",
        temp, llvm_ty, llvm_ty, src
    ));

    let dest_ty = self.get_local_type(mir_fn, *destination);
    let dest_llvm_ty = self.mir_type_to_llvm_str(&dest_ty);
    let dest_ptr_ty = format!("{}*", dest_llvm_ty);
    self.ir.push_str(&format!(
        "store {} {}, {} {}\n",
        dest_llvm_ty, temp, dest_ptr_ty, dest
    ));
}

// Load 指令 - 使用 destination 类型
mir::Instruction::Load { destination, source } => {
    let dest_ty = self.get_local_type(mir_fn, *destination);
    let llvm_value_ty = self.mir_type_to_llvm_str(&dest_ty);

    let src_ty = self.get_local_type(mir_fn, *source);
    let llvm_ptr_ty = self.mir_type_to_llvm_str(&src_ty);
    let llvm_src_ptr_ty = format!("{}*", llvm_ptr_ty);

    self.ir.push_str(&format!(
        "{} = load {}, {} {}\n",
        dest, llvm_value_ty, llvm_src_ptr_ty, src
    ));
}

// 返回值处理 - Temp 类型直接使用值
let is_temp = matches!(local_kind, Some(mir::LocalKind::Temp));
if is_temp {
    // 直接使用寄存器值，不需要 load
    let reg = self.local_reg(*local);
    // ...
} else {
    // 从内存加载
    // ...
}
```

## 测试用例

### 数组迭代测试
```sengoo
// tests/for_array_test.sg
def main() -> i64 {
    let arr = [10, 20, 30];
    let sum = 0;
    for x in arr {
        sum = sum + x;
    }
    sum
}
```

### 引用系统测试
```sengoo
// tests/ref_test.sg
def main() -> i64 {
    let x = 42;
    let ref_x = &x;
    *ref_x
}
```

### 基本引用测试
```sengoo
// tests/ref_basic_test.sg
def main() -> i64 {
    let x = 42;
    &x;
    x
}
```

## 最新进展 (2025-01-17)

### 代码生成器优化 ✅
- **参数传递和返回值处理完善**
  - Temp/Param locals 作为寄存器值处理（不需要 alloca）
  - User locals 使用 alloca + store/load 模式
  - Binary/Call/Return 指令正确区分 Temp vs User locals
  - 修改文件: `compiler/src/codegen/mod.rs`

生成的 LLVM IR 示例：
```llvm
define i64 @$__lambda0(i64 %l_1) {
bb_0:
    %l_2 = add i64 0, 1
    %l_3 = add i64 %l_1, %l_2
    ret i64 %l_3
}
```

### Lambda 环境捕获 ✅
- **自由变量收集**
  - 添加 `collect_free_vars` 方法分析 Lambda body 中的自由变量
  - 递归遍历表达式树收集变量引用
  - 修改文件: `compiler/src/mir/lowering.rs`

- **环境参数支持**
  - Lambda 函数签名支持环境参数 `i64*`
  - 函数签名记录环境信息 `FunctionSig { ret_type, env }`
  - Lambda 内部从环境加载捕获的变量

- **Let 绑定创建环境**
  - 检查 Lambda 是否有需要捕获的变量
  - 创建环境数组结构 `Array(Int(64), n)`
  - 使用 `IndexAddr`, `Load`, `Store` 指令填充环境
  - 使用 `AddrOf` 获取环境指针

- **Call lowering 传递环境**
  - 检测被调用者是否是 Lambda
  - 从 `lambda_environments` 获取环境指针
  - 将环境指针作为第一个参数传递

### 借用规则检查器 ✅
- **借用检查器实现** `compiler/src/typeck/borrow.rs`
  - `BorrowKind`: 不可变借用 (`Immutable`) 和可变借用 (`Mutable`)
  - `BorrowError`: 多重可变借用、可变与不可变冲突等错误
  - `BorrowChecker`: 检查语句和表达式中的借用规则

- **检查规则**
  - 不能同时有多个可变借用
  - 可变借用与其他借用不能共存
  - 支持作用域嵌套和借用生命周期跟踪 (NLL)

## 已知问题

1. ~~**Lambda 环境捕获不完整**~~ ✅ 已完成
2. ~~**借用规则检查未实现**~~ ✅ 已完成

## 最新进展 (2025-01-17)

### Lambda 环境捕获完成 ✅
- **current_block 修复**
  - 在 Lambda lowering 中设置 `current_block` 为入口块
  - 修复: "no current block set" 运行时错误
  - 修改: `compiler/src/mir/lowering.rs:1187`

- **IndexAddr 指令 codegen**
  - 在 `compiler/src/codegen/mod.rs` 中添加 IndexAddr 处理
  - 生成 LLVM `getelementptr` 指令
  - 支持数组/指针索引地址计算

- **环境创建和传递**
  - Let 绑定时检测 Lambda 并创建环境数组
  - Call lowering 传递环境指针作为第一个参数
  - Lambda 内部从环境加载捕获的变量

生成的 LLVM IR 示例：
```llvm
define i64 @$__lambda0(i64* %l_1, i64 %l_2) {
bb_0:
    store i64* %l_1, i64** %local_1
    store i64 0, i64* %local_4
    %local_5.idx = load i64, i64* %local_4
    %local_5.ptr = load i64*, i64* %local_1
    %local_5.addr = getelementptr i64, i64* %local_5.ptr, i64 %local_5.idx
    store i64* %local_5.addr, i64** %local_5
    %local_3.ptr = load i64*, i64** %local_5
    %local_3.val = load i64, i64* %local_3.ptr
    store i64 %local_3.val, i64* %local_3
    %local_6.l = load i64, i64* %local_3
    %local_6.r = load i64, i64* %local_2
    %result = add i64 %local_6.l, %local_6.r
    ret i64 %result
}
```

### 借用检查器实现 ✅
- **文件**: `compiler/src/typeck/borrow.rs`
- **核心类型**:
  - `BorrowKind`: 不可变/可变借用
  - `Borrow`: 借用信息（类型、生命周期、位置）
  - `BorrowError`: 借用规则错误
  - `BorrowChecker`: 借用检查器

- **检查功能**:
  - `check_stmt()`: 检查语句中的借用
  - `check_expr()`: 检查表达式中的借用
  - `add_borrow()`: 记录借用并检查规则
  - `push_scope()`, `pop_scope()`: 作用域管理

- **检查规则**:
  - 同一变量不能有多个可变借用
  - 可变借用与其他借用不能共存
  - 支持嵌套作用域和生命周期跟踪
