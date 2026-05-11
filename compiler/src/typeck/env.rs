//! 类型环境
//!
//! 管理符号表和作用域。

use crate::typeck::ty::{Ty, TyKind};
use std::collections::HashMap;

/// 符号
#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
}

/// 符号种类
#[derive(Debug, Clone)]
pub enum SymbolKind {
    /// 变量
    Var(Ty),
    /// 函数
    Function { ty: Ty },
    /// 类型（结构体、枚举等）
    Type { ty: Ty },
    /// Trait
    Trait { name: String },
    /// 常量
    Const { ty: Ty },
    /// 静态变量
    Static { ty: Ty, is_mut: bool },
    /// 模块
    Module { name: String },
    /// 类型参数
    TypeParam { name: String },
    /// Lifetime 参数
    LifetimeParam { name: String },
}

impl Symbol {
    pub fn var(name: String, ty: Ty) -> Self {
        Self {
            name,
            kind: SymbolKind::Var(ty),
        }
    }

    pub fn function(name: String, ty: Ty) -> Self {
        Self {
            name,
            kind: SymbolKind::Function { ty },
        }
    }

    pub fn type_symbol(name: String, ty: Ty) -> Self {
        Self {
            name,
            kind: SymbolKind::Type { ty },
        }
    }

    pub fn get_ty(&self) -> Option<&Ty> {
        match &self.kind {
            SymbolKind::Var(ty) => Some(ty),
            SymbolKind::Function { ty } => Some(ty),
            SymbolKind::Type { ty } => Some(ty),
            SymbolKind::Const { ty } => Some(ty),
            SymbolKind::Static { ty, .. } => Some(ty),
            _ => None,
        }
    }
}

/// 作用域层级
#[derive(Debug, Clone)]
pub struct Scope {
    /// 当前作用域中的符号
    symbols: HashMap<String, Symbol>,
    /// 父作用域的索引
    parent: Option<usize>,
}

impl Scope {
    pub fn new(parent: Option<usize>) -> Self {
        Self {
            symbols: HashMap::new(),
            parent,
        }
    }

    pub fn insert(&mut self, name: String, symbol: Symbol) {
        self.symbols.insert(name, symbol);
    }

    pub fn get(&self, name: &str) -> Option<&Symbol> {
        self.symbols.get(name)
    }
}

/// 类型环境
#[derive(Debug, Clone)]
pub struct TypeEnv {
    /// 所有作用域
    scopes: Vec<Scope>,
    /// 当前作用域索引
    current: usize,
    /// 下一个类型 ID
    next_ty_id: usize,
    /// 下一个类型变量 ID
    next_ty_var_id: usize,
}

impl TypeEnv {
    pub fn new() -> Self {
        let mut env = Self {
            scopes: Vec::new(),
            current: 0,
            next_ty_id: 0,
            next_ty_var_id: 0,
        };
        // 创建全局作用域
        env.push_scope();
        // 插入内置类型
        env.insert_builtin_types();
        // 插入内置函数
        env.insert_builtin_functions();
        env
    }

    /// 插入内置类型
    fn insert_builtin_types(&mut self) {
        let unit = Ty::new(self.fresh_ty_id(), TyKind::Unit);
        let never = Ty::new(self.fresh_ty_id(), TyKind::Never);
        let bool_ = Ty::new(self.fresh_ty_id(), TyKind::Bool);
        let char_ = Ty::new(self.fresh_ty_id(), TyKind::Char);
        let str_ = Ty::new(self.fresh_ty_id(), TyKind::Str);
        let byte = Ty::new(self.fresh_ty_id(), TyKind::Byte);
        let bytes = Ty::new(self.fresh_ty_id(), TyKind::Bytes);

        // 整数类型
        let i8 = Ty::new(
            self.fresh_ty_id(),
            TyKind::Int(crate::typeck::ty::IntKind::I8),
        );
        let i16 = Ty::new(
            self.fresh_ty_id(),
            TyKind::Int(crate::typeck::ty::IntKind::I16),
        );
        let i32 = Ty::new(
            self.fresh_ty_id(),
            TyKind::Int(crate::typeck::ty::IntKind::I32),
        );
        let i64 = Ty::new(
            self.fresh_ty_id(),
            TyKind::Int(crate::typeck::ty::IntKind::I64),
        );
        let i128 = Ty::new(
            self.fresh_ty_id(),
            TyKind::Int(crate::typeck::ty::IntKind::I128),
        );
        let isize = Ty::new(
            self.fresh_ty_id(),
            TyKind::Int(crate::typeck::ty::IntKind::ISize),
        );

        let _u8 = Ty::new(
            self.fresh_ty_id(),
            TyKind::Int(crate::typeck::ty::IntKind::U8),
        );
        let u16 = Ty::new(
            self.fresh_ty_id(),
            TyKind::Int(crate::typeck::ty::IntKind::U16),
        );
        let u32 = Ty::new(
            self.fresh_ty_id(),
            TyKind::Int(crate::typeck::ty::IntKind::U32),
        );
        let u64 = Ty::new(
            self.fresh_ty_id(),
            TyKind::Int(crate::typeck::ty::IntKind::U64),
        );
        let u128 = Ty::new(
            self.fresh_ty_id(),
            TyKind::Int(crate::typeck::ty::IntKind::U128),
        );
        let usize = Ty::new(
            self.fresh_ty_id(),
            TyKind::Int(crate::typeck::ty::IntKind::USize),
        );

        // 浮点类型
        let f32 = Ty::new(
            self.fresh_ty_id(),
            TyKind::Float(crate::typeck::ty::FloatKind::F32),
        );
        let f64 = Ty::new(
            self.fresh_ty_id(),
            TyKind::Float(crate::typeck::ty::FloatKind::F64),
        );

        // 插入类型符号
        for (name, ty) in [
            ("()", unit),
            ("!", never),
            ("bool", bool_),
            ("char", char_),
            ("str", str_),
            ("u8", byte),
            ("[u8]", bytes),
            ("i8", i8),
            ("i16", i16),
            ("i32", i32),
            ("i64", i64),
            ("i128", i128),
            ("isize", isize),
            ("u16", u16),
            ("u32", u32),
            ("u64", u64),
            ("u128", u128),
            ("usize", usize),
            ("f32", f32),
            ("f64", f64),
        ] {
            self.insert_type(name.to_string(), ty);
        }
    }

    /// 插入内置函数
    fn insert_builtin_functions(&mut self) {
        let unit = self.unit_ty();
        let i64 = self.int_ty(crate::typeck::ty::IntKind::I64);
        let str_ty = self.str_ty();
        let str_ref = self.ref_ty(false, str_ty);

        // print函数：打印字符串
        // print(s: &str) -> ()
        self.declare_fn("print".to_string(), vec![str_ref], unit.clone());

        // print函数的整数版本：print(n: i64) -> ()
        let _print_fn_i64 = self.fn_ty(vec![i64.clone()], unit.clone());
        // 注意：这里使用同一个名字，但实际类型检查时会根据参数类型选择
        // 为了简化，我们先只支持字符串版本
    }

    /// 推入新的作用域
    pub fn push_scope(&mut self) {
        let parent = if self.scopes.is_empty() {
            None
        } else {
            Some(self.current)
        };
        self.scopes.push(Scope::new(parent));
        self.current = self.scopes.len() - 1;
    }

    /// 弹出当前作用域
    pub fn pop_scope(&mut self) -> bool {
        if self.current == 0 {
            return false; // 不能弹出全局作用域
        }
        if let Some(scope) = self.scopes.get(self.current) {
            if let Some(parent) = scope.parent {
                self.current = parent;
                return true;
            }
        }
        false
    }

    /// 插入符号
    pub fn insert(&mut self, name: String, symbol: Symbol) {
        if let Some(scope) = self.scopes.get_mut(self.current) {
            scope.insert(name, symbol);
        }
    }

    /// 插入变量
    pub fn insert_var(&mut self, name: String, ty: Ty) {
        let symbol = Symbol::var(name.clone(), ty);
        self.insert(name, symbol);
    }

    /// 插入函数
    pub fn insert_fn(&mut self, name: String, ty: Ty) {
        let symbol = Symbol::function(name.clone(), ty);
        self.insert(name, symbol);
    }

    /// Build the function type from `params`/`ret` and insert it as a Function
    /// symbol in one step, avoiding the redundant clones that result from
    /// calling `fn_ty` and `insert_fn` separately at the call site.
    pub fn declare_fn(&mut self, name: String, params: Vec<Ty>, ret: Ty) {
        let ty = self.fn_ty(params, ret);
        self.insert_fn(name, ty);
    }

    /// 插入类型
    pub fn insert_type(&mut self, name: String, ty: Ty) {
        let symbol = Symbol::type_symbol(name.clone(), ty);
        self.insert(name, symbol);
    }

    /// 查找符号（在当前及父作用域中查找）
    pub fn lookup(&self, name: &str) -> Option<&Symbol> {
        let mut current = self.current;
        while let Some(scope) = self.scopes.get(current) {
            if let Some(symbol) = scope.get(name) {
                return Some(symbol);
            }
            if let Some(parent) = scope.parent {
                current = parent;
            } else {
                break;
            }
        }
        None
    }

    /// 在当前作用域查找符号
    pub fn lookup_in_current(&self, name: &str) -> Option<&Symbol> {
        if let Some(scope) = self.scopes.get(self.current) {
            scope.get(name)
        } else {
            None
        }
    }

    /// 检查符号是否存在于任何作用域
    pub fn contains(&self, name: &str) -> bool {
        self.lookup(name).is_some()
    }

    /// 生成新的类型 ID
    pub fn fresh_ty_id(&mut self) -> usize {
        let id = self.next_ty_id;
        self.next_ty_id += 1;
        id
    }

    /// 生成新的类型变量 ID
    pub fn fresh_ty_var_id(&mut self) -> usize {
        let id = self.next_ty_var_id;
        self.next_ty_var_id += 1;
        id
    }

    /// 创建新的类型
    pub fn new_ty(&mut self, kind: TyKind) -> Ty {
        Ty::new(self.fresh_ty_id(), kind)
    }

    /// 创建新的类型变量
    pub fn new_ty_var(&mut self) -> Ty {
        let id = self.fresh_ty_var_id();
        Ty::new(self.fresh_ty_id(), TyKind::Var(id))
    }

    /// 创建错误类型
    pub fn error_ty(&mut self) -> Ty {
        Ty::new(self.fresh_ty_id(), TyKind::Error)
    }

    /// 创建单元类型
    pub fn unit_ty(&mut self) -> Ty {
        // 查找已存在的单元类型
        if let Some(sym) = self.lookup("()") {
            if let Some(ty) = sym.get_ty() {
                return ty.clone();
            }
        }
        self.new_ty(TyKind::Unit)
    }

    /// 创建 Never 类型
    pub fn never_ty(&mut self) -> Ty {
        // 查找已存在的 Never 类型
        if let Some(sym) = self.lookup("!") {
            if let Some(ty) = sym.get_ty() {
                return ty.clone();
            }
        }
        self.new_ty(TyKind::Never)
    }

    /// 创建布尔类型
    pub fn bool_ty(&mut self) -> Ty {
        if let Some(sym) = self.lookup("bool") {
            if let Some(ty) = sym.get_ty() {
                return ty.clone();
            }
        }
        self.new_ty(TyKind::Bool)
    }

    /// 创建整数类型
    pub fn int_ty(&mut self, kind: crate::typeck::ty::IntKind) -> Ty {
        let name = kind.to_string();
        if let Some(sym) = self.lookup(&name) {
            if let Some(ty) = sym.get_ty() {
                return ty.clone();
            }
        }
        self.new_ty(TyKind::Int(kind))
    }

    /// 创建浮点类型
    pub fn float_ty(&mut self, kind: crate::typeck::ty::FloatKind) -> Ty {
        let name = kind.to_string();
        if let Some(sym) = self.lookup(&name) {
            if let Some(ty) = sym.get_ty() {
                return ty.clone();
            }
        }
        self.new_ty(TyKind::Float(kind))
    }

    /// 创建字符串类型
    pub fn str_ty(&mut self) -> Ty {
        if let Some(sym) = self.lookup("str") {
            if let Some(ty) = sym.get_ty() {
                return ty.clone();
            }
        }
        self.new_ty(TyKind::Str)
    }

    /// 创建引用类型
    pub fn ref_ty(&mut self, mutability: bool, inner: Ty) -> Ty {
        self.new_ty(TyKind::Ref(mutability, Box::new(inner)))
    }

    /// 创建函数类型
    pub fn fn_ty(&mut self, params: Vec<Ty>, ret: Ty) -> Ty {
        self.new_ty(TyKind::Fn {
            params,
            ret: Box::new(ret),
            is_variadic: false,
        })
    }

    /// 创建元组类型
    pub fn tuple_ty(&mut self, types: Vec<Ty>) -> Ty {
        self.new_ty(TyKind::Tuple(types))
    }

    /// 创建数组类型
    pub fn array_ty(&mut self, elem: Ty, len: usize) -> Ty {
        self.new_ty(TyKind::Array(Box::new(elem), len))
    }

    /// 创建切片类型
    pub fn slice_ty(&mut self, elem: Ty) -> Ty {
        self.new_ty(TyKind::Slice(Box::new(elem)))
    }

    /// 获取当前作用域深度
    pub fn depth(&self) -> usize {
        let mut depth = 0;
        let mut current = self.current;
        while let Some(scope) = self.scopes.get(current) {
            depth += 1;
            if let Some(parent) = scope.parent {
                current = parent;
            } else {
                break;
            }
        }
        depth
    }

    /// 获取当前作用域索引
    pub fn current_scope(&self) -> usize {
        self.current
    }
}

impl Default for TypeEnv {
    fn default() -> Self {
        Self::new()
    }
}
