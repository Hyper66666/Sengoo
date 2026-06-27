//! 类型环境
//!
//! 管理符号表和作用域。

use crate::typeck::interner::TyInterner;
use crate::typeck::r#trait::type_key;
use crate::typeck::ty::{Ty, TyKind, TyVarId};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

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
    Var { ty: Ty, is_mut: bool },
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
        Self::var_with_mutability(name, ty, false)
    }

    pub fn var_with_mutability(name: String, ty: Ty, is_mut: bool) -> Self {
        Self {
            name,
            kind: SymbolKind::Var { ty, is_mut },
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
            SymbolKind::Var { ty, .. } => Some(ty),
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
    /// 会话级共享类型 interner。
    ///
    /// 用 `Rc<RefCell<_>>` 包装：Clone 此 env 时（`TypeChecker::new` 把 env clone
    /// 进 `TypeInfer`、`borrow_check` 把 env clone 进 `BorrowChecker` 等场景）
    /// 通过 `Rc` 浅复制共享同一 arena 和同一套 [`crate::typeck::interner::InternedTyId`] 编号。
    interner: Rc<RefCell<TyInterner>>,
    /// Canonical stdlib `String { handle: i64 }` type identity for move rules.
    pub owned_string_ty: Option<Ty>,
    /// Type keys for non-Copy values with compiler-managed ownership.
    drop_owned_type_keys: HashSet<String>,
    /// Resolved field layouts used by ownership analysis after type checking.
    struct_field_types: HashMap<String, StructFieldTypes>,
}

#[derive(Debug, Clone)]
struct StructFieldTypes {
    type_params: Vec<TyVarId>,
    fields: Vec<(String, Ty)>,
}

impl TypeEnv {
    pub fn new() -> Self {
        let mut env = Self {
            scopes: Vec::new(),
            current: 0,
            next_ty_id: 0,
            next_ty_var_id: 0,
            interner: Rc::new(RefCell::new(TyInterner::new())),
            owned_string_ty: None,
            drop_owned_type_keys: HashSet::new(),
            struct_field_types: HashMap::new(),
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
        // 全部走 `new_ty` 入口，以便同时 intern 进共享 arena。
        let unit = self.new_ty(TyKind::Unit);
        let never = self.new_ty(TyKind::Never);
        let bool_ = self.new_ty(TyKind::Bool);
        let char_ = self.new_ty(TyKind::Char);
        let str_ = self.new_ty(TyKind::Str);
        let byte = self.new_ty(TyKind::Byte);
        let bytes = self.new_ty(TyKind::Bytes);

        use crate::typeck::ty::{FloatKind, IntKind};

        // 整数类型
        let i8 = self.new_ty(TyKind::Int(IntKind::I8));
        let i16 = self.new_ty(TyKind::Int(IntKind::I16));
        let i32 = self.new_ty(TyKind::Int(IntKind::I32));
        let i64 = self.new_ty(TyKind::Int(IntKind::I64));
        let i128 = self.new_ty(TyKind::Int(IntKind::I128));
        let isize = self.new_ty(TyKind::Int(IntKind::ISize));

        // U8 仍插入 arena（后续有机会被使用），但语言中 "u8" 名字映射到 TyKind::Byte。
        let _u8 = self.new_ty(TyKind::Int(IntKind::U8));
        let u16 = self.new_ty(TyKind::Int(IntKind::U16));
        let u32 = self.new_ty(TyKind::Int(IntKind::U32));
        let u64 = self.new_ty(TyKind::Int(IntKind::U64));
        let u128 = self.new_ty(TyKind::Int(IntKind::U128));
        let usize = self.new_ty(TyKind::Int(IntKind::USize));

        // 浮点类型
        let f32 = self.new_ty(TyKind::Float(FloatKind::F32));
        let f64 = self.new_ty(TyKind::Float(FloatKind::F64));

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

    pub fn insert_var_with_mutability(&mut self, name: String, ty: Ty, is_mut: bool) {
        let symbol = Symbol::var_with_mutability(name.clone(), ty, is_mut);
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

    pub fn mark_drop_owned_type(&mut self, ty: &Ty) {
        if !ty.is_copy_value() {
            self.drop_owned_type_keys.insert(type_key(ty));
        }
    }

    pub fn is_drop_owned_type(&self, ty: &Ty) -> bool {
        !ty.is_copy_value() && self.drop_owned_type_keys.contains(&type_key(ty))
    }

    pub fn register_struct_field_types(
        &mut self,
        name: String,
        type_params: Vec<TyVarId>,
        fields: Vec<(String, Ty)>,
    ) {
        self.struct_field_types.insert(
            name,
            StructFieldTypes {
                type_params,
                fields,
            },
        );
    }

    pub fn struct_field_type(&self, owner: &Ty, field_name: &str) -> Option<Ty> {
        let TyKind::Adt { name, args } = &owner.kind else {
            return None;
        };
        let layout = self.struct_field_types.get(name)?;
        let field_ty = layout
            .fields
            .iter()
            .find(|(name, _)| name == field_name)?
            .1
            .clone();
        let subst = layout
            .type_params
            .iter()
            .copied()
            .zip(args.iter().cloned())
            .collect::<HashMap<_, _>>();
        Some(Self::substitute_ty_vars(&field_ty, &subst))
    }

    pub fn type_contains_drop_owned_value(&self, ty: &Ty) -> bool {
        self.type_contains_drop_owned_value_inner(ty, &mut HashSet::new())
    }

    pub fn is_legacy_idempotent_handle_type(&self, ty: &Ty) -> bool {
        match &ty.kind {
            TyKind::Adt { name, .. } => matches!(
                name.as_str(),
                "Buffer"
                    | "Vec"
                    | "JsonDoc"
                    | "ProcessCommand"
                    | "ProcessOutput"
                    | "ProcessHandle"
                    | "TcpStream"
                    | "UdpSocket"
                    | "HttpClient"
                    | "HttpServer"
                    | "HttpServerRequest"
                    | "WsClient"
            ),
            _ => false,
        }
    }

    fn type_contains_drop_owned_value_inner(
        &self,
        ty: &Ty,
        visiting_adts: &mut HashSet<String>,
    ) -> bool {
        if ty.is_copy_value() {
            return false;
        }
        if self.is_drop_owned_type(ty) {
            return true;
        }
        match &ty.kind {
            TyKind::Tuple(types) => types
                .iter()
                .any(|field| self.type_contains_drop_owned_value_inner(field, visiting_adts)),
            TyKind::Array(elem, _) => {
                self.type_contains_drop_owned_value_inner(elem, visiting_adts)
            }
            TyKind::Adt { name, .. } => self.struct_field_types.get(name).is_some_and(|layout| {
                if !visiting_adts.insert(name.clone()) {
                    return false;
                }
                let contains = layout.fields.iter().any(|(field_name, _)| {
                    self.struct_field_type(ty, field_name).is_some_and(|field| {
                        self.type_contains_drop_owned_value_inner(&field, visiting_adts)
                    })
                });
                visiting_adts.remove(name);
                contains
            }),
            _ => false,
        }
    }

    fn substitute_ty_vars(ty: &Ty, subst: &HashMap<TyVarId, Ty>) -> Ty {
        let kind = match &ty.kind {
            TyKind::Var(var_id) => {
                return subst.get(var_id).cloned().unwrap_or_else(|| ty.clone());
            }
            TyKind::Tuple(types) => TyKind::Tuple(
                types
                    .iter()
                    .map(|inner| Self::substitute_ty_vars(inner, subst))
                    .collect(),
            ),
            TyKind::Array(elem, len) => {
                TyKind::Array(Box::new(Self::substitute_ty_vars(elem, subst)), *len)
            }
            TyKind::Slice(elem) => TyKind::Slice(Box::new(Self::substitute_ty_vars(elem, subst))),
            TyKind::Ref(is_mut, inner) => {
                TyKind::Ref(*is_mut, Box::new(Self::substitute_ty_vars(inner, subst)))
            }
            TyKind::Ptr(inner) => TyKind::Ptr(Box::new(Self::substitute_ty_vars(inner, subst))),
            TyKind::Fn {
                params,
                ret,
                is_variadic,
            } => TyKind::Fn {
                params: params
                    .iter()
                    .map(|param| Self::substitute_ty_vars(param, subst))
                    .collect(),
                ret: Box::new(Self::substitute_ty_vars(ret, subst)),
                is_variadic: *is_variadic,
            },
            TyKind::Adt { name, args } => TyKind::Adt {
                name: name.clone(),
                args: args
                    .iter()
                    .map(|arg| Self::substitute_ty_vars(arg, subst))
                    .collect(),
            },
            TyKind::AssocProjection {
                base,
                trait_name,
                name,
            } => TyKind::AssocProjection {
                base: Box::new(Self::substitute_ty_vars(base, subst)),
                trait_name: trait_name.clone(),
                name: name.clone(),
            },
            TyKind::Future(inner) => {
                TyKind::Future(Box::new(Self::substitute_ty_vars(inner, subst)))
            }
            _ => return ty.clone(),
        };
        Ty { id: ty.id, kind }
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
    ///
    /// 会同时 intern 进共享 [`TyInterner`]；builtin / composite / new\_ty 类 helper
    /// 都以此为唯一入口，从而保证 arena 在 `TypeEnv::new` 后便包含全部 primitive shape。
    pub fn new_ty(&mut self, kind: TyKind) -> Ty {
        let id = self.fresh_ty_id();
        let ty = Ty::new(id, kind);
        // RefCell::borrow_mut 仅需 &self.interner，与上述 fresh_ty_id 调用已释放的 &mut self 不冲突。
        self.interner.borrow_mut().intern_ty(&ty);
        ty
    }

    /// 创建新的类型变量
    ///
    /// 每个 fresh var 都占一个独立 [`crate::typeck::interner::InternedTyId`]，
    /// 因为 `TyKind::Var(id)` 的结构性相等以 `id` 为锰。
    pub fn new_ty_var(&mut self) -> Ty {
        let var_id = self.fresh_ty_var_id();
        self.new_ty(TyKind::Var(var_id))
    }

    /// 创建错误类型
    ///
    /// `TyKind::Error` 仅有一种结构形状，多次调用会被 interner 去重为同一 id，
    /// 但 origin tag 仍递增以保留现有的 per-instance 诊断语义。
    pub fn error_ty(&mut self) -> Ty {
        self.new_ty(TyKind::Error)
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

    /// 返回共享的类型 interner 句柄。
    ///
    /// 返回 [`Rc`] clone，让调用方可独立持有；通过 `.borrow()` /
    /// `.borrow_mut()` 借用底层 [`TyInterner`]。
    pub fn interner(&self) -> Rc<RefCell<TyInterner>> {
        Rc::clone(&self.interner)
    }

    /// 把 owned `Ty` intern 进会话 arena 并返回结构性 id。
    ///
    /// Slice F: 这是 `env.interner().borrow_mut().intern_ty(ty)` 的便捷封装；
    /// 对所有通过 [`Self::new_ty`] 构造出的 `Ty`，这次调用是 HashMap 命中，
    /// 不会增长 arena。
    pub fn intern_ty(&self, ty: &crate::typeck::ty::Ty) -> crate::typeck::interner::InternedTyId {
        self.interner.borrow_mut().intern_ty(ty)
    }

    /// 复合查找：返回符号 `name` 对应类型的结构性 id（如果存在且符号有类型）。
    ///
    /// Slice F (Task 3.4)：Phase 1 baseline 保留 [`Symbol`] 存储为 owned `Ty`
    /// 以避免修改 ~6 处 `symbol.get_ty()` / `match &symbol.kind` 调用点；本 helper
    /// 让需要做结构性比较的新代码可以 O(1) 拿到 id（因为 builtin 已在
    /// `TypeEnv::new` 时预 intern；用户类型也通过 `new_ty` 路径预 intern）。
    pub fn symbol_ty_id(&self, name: &str) -> Option<crate::typeck::interner::InternedTyId> {
        let symbol = self.lookup(name)?;
        let ty = symbol.get_ty()?;
        Some(self.intern_ty(ty))
    }
}

impl Default for TypeEnv {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typeck::interner::InternedTyKind;
    use crate::typeck::ty::IntKind;

    /// Slice F: `env.symbol_ty_id` 应该对所有 builtin primitive name 返回 Some，
    /// 且重复查询同名 symbol 不应增长 arena（命中已 intern 的 id）。
    #[test]
    fn symbol_ty_id_returns_pre_interned_id_for_builtin_primitives() {
        let env = TypeEnv::new();
        let initial_len = env.interner().borrow().len();

        // 一组覆盖性 builtin：unit / bool / i32 / f64 / str。
        let probed: Vec<(&str, Option<crate::typeck::interner::InternedTyId>)> = vec![
            ("()", env.symbol_ty_id("()")),
            ("bool", env.symbol_ty_id("bool")),
            ("i32", env.symbol_ty_id("i32")),
            ("f64", env.symbol_ty_id("f64")),
            ("str", env.symbol_ty_id("str")),
        ];
        for (name, id) in &probed {
            assert!(id.is_some(), "expected symbol_ty_id({:?}) to be Some", name);
        }
        // arena 不应因为查询而增长。
        assert_eq!(env.interner().borrow().len(), initial_len);

        // 未注册的符号返回 None。
        assert!(env.symbol_ty_id("_NonExistentSymbol_xyz").is_none());
        assert_eq!(env.interner().borrow().len(), initial_len);
    }

    /// Slice D 之后，`TypeEnv::new` 会预 intern 全部 primitive shape。
    /// 重复 intern 同样的 kind 不应增长 arena。
    #[test]
    fn type_env_init_interns_primitive_builtins() {
        let env = TypeEnv::new();

        let initial_len = env.interner().borrow().len();
        // 7 个非整数/浮点 primitive（Unit/Never/Bool/Char/Str/Byte/Bytes）+ 12 个 Int variant + 2 个 Float variant
        // + insert_builtin_functions 里额外产生的 Ref(false, Str) / Fn(...) 等。这里只断言下界以便后续
        // 引入 fresh primitive 时不必同步调整该数字。
        assert!(
            initial_len >= 20,
            "expected primitive builtins to be pre-interned, got len = {}",
            initial_len
        );

        // Re-intern known primitives: 必复用现有 id，arena 不增长。
        let interner_rc = env.interner();
        let mut interner = interner_rc.borrow_mut();
        interner.intern(InternedTyKind::Bool);
        interner.intern(InternedTyKind::Int(IntKind::I32));
        interner.intern(InternedTyKind::Unit);
        interner.intern(InternedTyKind::Never);
        interner.intern(InternedTyKind::Float(crate::typeck::ty::FloatKind::F64));
        assert_eq!(
            interner.len(),
            initial_len,
            "primitive kinds should already be present after TypeEnv::new"
        );
    }

    /// 验证 `Rc<RefCell<TyInterner>>` 在 env clone 后共享同一 arena —— 这正是
    /// Slice C 选用此包装的核心动机：避免 `TypeChecker.env` / `TypeChecker.infer.env` /
    /// `BorrowChecker._env` 三处 clone 各持独立 arena 的回归。
    #[test]
    fn cloned_type_envs_share_one_interner_arena() {
        let env1 = TypeEnv::new();
        let env2 = env1.clone();

        let initial_len = env1.interner().borrow().len();
        assert_eq!(
            initial_len,
            env2.interner().borrow().len(),
            "fresh + cloned env should both observe the same arena length"
        );

        // 选用一个不会与 builtin 冲突的 ADT 名作为「新」 shape。
        let novel_kind_a = InternedTyKind::Adt {
            name: "_SliceCSharedArenaTestAdt_alpha".to_string(),
            args: vec![],
        };
        let novel_kind_b = InternedTyKind::Adt {
            name: "_SliceCSharedArenaTestAdt_beta".to_string(),
            args: vec![],
        };

        // 通过 env1 写入一个新 shape。
        let id_a = env1.interner().borrow_mut().intern(novel_kind_a.clone());
        assert_eq!(env1.interner().borrow().len(), initial_len + 1);

        // env2 立即看到。
        assert_eq!(env2.interner().borrow().len(), initial_len + 1);
        assert_eq!(
            env2.interner().borrow().try_lookup(id_a),
            Some(&novel_kind_a)
        );

        // 反向：通过 env2 写入，env1 立刻可见，且 id 不重复。
        let id_b = env2.interner().borrow_mut().intern(novel_kind_b);
        assert_eq!(env1.interner().borrow().len(), initial_len + 2);
        assert_ne!(id_a, id_b);

        // 幂等：重复 intern 同样 kind 返回旧 id 且 arena 不增长。
        let id_a_again = env1.interner().borrow_mut().intern(novel_kind_a);
        assert_eq!(id_a, id_a_again);
        assert_eq!(env2.interner().borrow().len(), initial_len + 2);
    }
}
