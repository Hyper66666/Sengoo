//! 类型 interner
//!
//! 会话级的结构性类型 ID 分配与查找。
//!
//! Phase 1 baseline 范围：
//! - 仅做新增；不改动 `Ty` / `TyKind` / `Subst` / `TypeEnv` 等现有 API。
//! - 通过 `intern_ty` / `materialize` 在 owned `Ty` 与 `InternedTyId` 之间相互转换，
//!   作为后续 phase 中渐进迁移 substitution map、checkpoint、symbol 存储的兼容边界。
//!
//! 当前 `compiler/src/typeck/ty.rs` 中的 `pub type TyId = usize` 是 per-instance 的
//! 「来源 tag」（由 `TypeEnv::fresh_ty_id` 分配，subst 时被原样传递，调用方常以 `0`
//! 当哨兵）。它并不表示结构性相等，因此本模块引入独立的 `InternedTyId` newtype，
//! 避免与既有 `TyId` 语义混淆。

use std::collections::HashMap;

use super::ty::{FloatKind, IntKind, Ty, TyKind, TyVarId};

/// 结构性类型句柄。
///
/// 仅在分配它的 [`TyInterner`] 内有效；跨 interner 比较在语义上未定义。
/// 使用 `u32` 而非 `usize`：单次类型检查会话不会产生 40 亿种结构性 shape，
/// 而 4 字节句柄能让上层 map/Vec 更紧凑。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InternedTyId(u32);

impl InternedTyId {
    /// 仅用于调试 / 测试，业务代码不应假设此数值具有任何稳定含义。
    #[inline]
    pub fn as_u32(self) -> u32 {
        self.0
    }
}

/// Interner 友好的类型种类。
///
/// 与 [`TyKind`] 镜像，但所有 nested 子类型用 [`InternedTyId`] 表示，使得：
/// 1. 整个 `InternedTyKind` 本身浅小，可直接 `Hash` 做去重；
/// 2. 嵌套结构靠 id 引用，避免递归 `Vec<Ty>` / `Box<Ty>` 深拷贝。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum InternedTyKind {
    Error,
    Unit,
    Never,
    Bool,
    Int(IntKind),
    Float(FloatKind),
    Char,
    Str,
    Byte,
    Bytes,
    Tuple(Vec<InternedTyId>),
    Array(InternedTyId, usize),
    Slice(InternedTyId),
    Ref(bool, InternedTyId),
    Ptr(InternedTyId),
    Fn {
        params: Vec<InternedTyId>,
        ret: InternedTyId,
        is_variadic: bool,
    },
    Var(TyVarId),
    Adt {
        name: String,
        args: Vec<InternedTyId>,
    },
    Dyn(Vec<String>),
    ImplTrait(Vec<String>),
    Future(InternedTyId),
    SelfType,
    Inferred,
}

/// 会话级类型 interner。
///
/// 设计要点：
/// - `arena`：id → kind 反向查找，按插入顺序追加，元素位置即 id；
/// - `lookup`：kind → id 正向去重；
/// - 不打算做线程安全；同一时间只能被单线程访问；
/// - 一次类型检查会话独占一个 interner，结束即释放，不存在跨 session 复用。
#[derive(Debug, Default, Clone)]
pub struct TyInterner {
    arena: Vec<InternedTyKind>,
    lookup: HashMap<InternedTyKind, InternedTyId>,
}

impl TyInterner {
    /// 创建一个空 interner。
    pub fn new() -> Self {
        Self::default()
    }

    /// 已分配的不同 type shape 数量。
    pub fn len(&self) -> usize {
        self.arena.len()
    }

    /// 是否还没有任何已分配的 type shape。
    pub fn is_empty(&self) -> bool {
        self.arena.is_empty()
    }

    /// 主入口：把任意 [`InternedTyKind`] 写入 interner，去重后返回 id。
    ///
    /// 若 `kind` 已存在，复用旧 id 且不增加 arena 长度；否则追加并建立映射。
    pub fn intern(&mut self, kind: InternedTyKind) -> InternedTyId {
        if let Some(&id) = self.lookup.get(&kind) {
            return id;
        }
        let id = InternedTyId(self.arena.len() as u32);
        self.arena.push(kind.clone());
        self.lookup.insert(kind, id);
        id
    }

    /// 通过 id 查找对应的 kind；id 不属于此 interner 时返回 `None`。
    ///
    /// 对应 spec scenario：Looking up an invalid type ID。
    pub fn try_lookup(&self, id: InternedTyId) -> Option<&InternedTyKind> {
        self.arena.get(id.0 as usize)
    }

    /// 类似 [`try_lookup`](Self::try_lookup)，但 invalid id 直接 panic。
    ///
    /// 用于内部不变量被破坏时早期发现；普通调用方应优先选择 `try_lookup`。
    pub fn lookup(&self, id: InternedTyId) -> &InternedTyKind {
        self.try_lookup(id)
            .unwrap_or_else(|| panic!("InternedTyId {:?} not owned by this TyInterner", id))
    }

    // ---------------- 兼容层：Ty ↔ InternedTyId ----------------

    /// 递归 intern 一棵 owned `Ty` 树，返回 root 的 [`InternedTyId`]。
    ///
    /// 现有 `Ty.id`（per-instance 来源 tag）在此被丢弃 —— 结构性相等不依赖它。
    pub fn intern_ty(&mut self, ty: &Ty) -> InternedTyId {
        let kind = self.lower_kind(&ty.kind);
        self.intern(kind)
    }

    fn lower_kind(&mut self, kind: &TyKind) -> InternedTyKind {
        match kind {
            TyKind::Error => InternedTyKind::Error,
            TyKind::Unit => InternedTyKind::Unit,
            TyKind::Never => InternedTyKind::Never,
            TyKind::Bool => InternedTyKind::Bool,
            TyKind::Int(int_kind) => InternedTyKind::Int(*int_kind),
            TyKind::Float(float_kind) => InternedTyKind::Float(*float_kind),
            TyKind::Char => InternedTyKind::Char,
            TyKind::Str => InternedTyKind::Str,
            TyKind::Byte => InternedTyKind::Byte,
            TyKind::Bytes => InternedTyKind::Bytes,
            TyKind::Tuple(types) => {
                let ids = types.iter().map(|t| self.intern_ty(t)).collect();
                InternedTyKind::Tuple(ids)
            }
            TyKind::Array(elem, n) => {
                let elem_id = self.intern_ty(elem);
                InternedTyKind::Array(elem_id, *n)
            }
            TyKind::Slice(elem) => InternedTyKind::Slice(self.intern_ty(elem)),
            TyKind::Ref(is_mut, inner) => InternedTyKind::Ref(*is_mut, self.intern_ty(inner)),
            TyKind::Ptr(inner) => InternedTyKind::Ptr(self.intern_ty(inner)),
            TyKind::Fn {
                params,
                ret,
                is_variadic,
            } => {
                let param_ids = params.iter().map(|t| self.intern_ty(t)).collect();
                let ret_id = self.intern_ty(ret);
                InternedTyKind::Fn {
                    params: param_ids,
                    ret: ret_id,
                    is_variadic: *is_variadic,
                }
            }
            TyKind::Var(var_id) => InternedTyKind::Var(*var_id),
            TyKind::Adt { name, args } => {
                let arg_ids = args.iter().map(|t| self.intern_ty(t)).collect();
                InternedTyKind::Adt {
                    name: name.clone(),
                    args: arg_ids,
                }
            }
            TyKind::Dyn(traits) => InternedTyKind::Dyn(traits.clone()),
            TyKind::ImplTrait(traits) => InternedTyKind::ImplTrait(traits.clone()),
            TyKind::Future(inner) => InternedTyKind::Future(self.intern_ty(inner)),
            TyKind::SelfType => InternedTyKind::SelfType,
            TyKind::Inferred => InternedTyKind::Inferred,
        }
    }

    /// 反向兼容：把 id 还原成 owned `Ty`，所有 origin tag 都填 `0`
    /// （哨兵值，与现有调用约定一致 —— 例如 `check.rs` 里大量直接 `Ty::new(0, ...)`）。
    ///
    /// 若调用方需要保留特定 origin id，请改用 [`materialize_with_origin`](Self::materialize_with_origin)。
    pub fn materialize(&self, id: InternedTyId) -> Ty {
        self.materialize_with_origin(id, 0)
    }

    /// 反向兼容：还原 owned `Ty`，可指定 root 的 origin tag；
    /// nested 子类型的 origin 全为 `0`（baseline 阶段不保留递归 tag 历史）。
    pub fn materialize_with_origin(&self, id: InternedTyId, origin: usize) -> Ty {
        let kind = self.materialize_kind(id);
        Ty::new(origin, kind)
    }

    fn materialize_kind(&self, id: InternedTyId) -> TyKind {
        match self.lookup(id) {
            InternedTyKind::Error => TyKind::Error,
            InternedTyKind::Unit => TyKind::Unit,
            InternedTyKind::Never => TyKind::Never,
            InternedTyKind::Bool => TyKind::Bool,
            InternedTyKind::Int(int_kind) => TyKind::Int(*int_kind),
            InternedTyKind::Float(float_kind) => TyKind::Float(*float_kind),
            InternedTyKind::Char => TyKind::Char,
            InternedTyKind::Str => TyKind::Str,
            InternedTyKind::Byte => TyKind::Byte,
            InternedTyKind::Bytes => TyKind::Bytes,
            InternedTyKind::Tuple(ids) => {
                TyKind::Tuple(ids.iter().map(|&id| self.materialize(id)).collect())
            }
            InternedTyKind::Array(elem_id, n) => {
                TyKind::Array(Box::new(self.materialize(*elem_id)), *n)
            }
            InternedTyKind::Slice(elem_id) => TyKind::Slice(Box::new(self.materialize(*elem_id))),
            InternedTyKind::Ref(is_mut, inner_id) => {
                TyKind::Ref(*is_mut, Box::new(self.materialize(*inner_id)))
            }
            InternedTyKind::Ptr(inner_id) => TyKind::Ptr(Box::new(self.materialize(*inner_id))),
            InternedTyKind::Fn {
                params,
                ret,
                is_variadic,
            } => TyKind::Fn {
                params: params.iter().map(|&id| self.materialize(id)).collect(),
                ret: Box::new(self.materialize(*ret)),
                is_variadic: *is_variadic,
            },
            InternedTyKind::Var(var_id) => TyKind::Var(*var_id),
            InternedTyKind::Adt { name, args } => TyKind::Adt {
                name: name.clone(),
                args: args.iter().map(|&id| self.materialize(id)).collect(),
            },
            InternedTyKind::Dyn(traits) => TyKind::Dyn(traits.clone()),
            InternedTyKind::ImplTrait(traits) => TyKind::ImplTrait(traits.clone()),
            InternedTyKind::Future(inner_id) => {
                TyKind::Future(Box::new(self.materialize(*inner_id)))
            }
            InternedTyKind::SelfType => TyKind::SelfType,
            InternedTyKind::Inferred => TyKind::Inferred,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typeck::ty::{IntKind, Ty, TyKind};

    fn ty(kind: TyKind) -> Ty {
        Ty::new(0, kind)
    }

    /// Spec: Reusing an existing structural type → same id; arena 不增长。
    #[test]
    fn canonical_reuse_returns_same_id() {
        let mut interner = TyInterner::new();
        let id1 = interner.intern(InternedTyKind::Int(IntKind::I32));
        let id2 = interner.intern(InternedTyKind::Int(IntKind::I32));
        assert_eq!(id1, id2);
        assert_eq!(interner.len(), 1);
    }

    /// Spec: Distinguishing different structural types → 不同 id。
    #[test]
    fn distinct_kinds_get_distinct_ids() {
        let mut interner = TyInterner::new();
        let int_id = interner.intern(InternedTyKind::Int(IntKind::I32));
        let bool_id = interner.intern(InternedTyKind::Bool);
        let i64_id = interner.intern(InternedTyKind::Int(IntKind::I64));
        let ref_bool_id = interner.intern(InternedTyKind::Ref(false, bool_id));
        let mut_ref_bool_id = interner.intern(InternedTyKind::Ref(true, bool_id));
        assert_ne!(int_id, bool_id);
        assert_ne!(int_id, i64_id);
        assert_ne!(ref_bool_id, mut_ref_bool_id);
        assert_eq!(interner.len(), 5);
    }

    /// Spec: Looking up a composite type → 子类型靠 id 表达，无需 owned Ty。
    #[test]
    fn nested_composite_lookup_uses_handles() {
        let mut interner = TyInterner::new();
        let i32_id = interner.intern(InternedTyKind::Int(IntKind::I32));
        let bool_id = interner.intern(InternedTyKind::Bool);
        let tuple_id = interner.intern(InternedTyKind::Tuple(vec![i32_id, bool_id]));

        match interner.lookup(tuple_id) {
            InternedTyKind::Tuple(ids) => {
                assert_eq!(ids.as_slice(), &[i32_id, bool_id]);
            }
            other => panic!("expected Tuple, got {:?}", other),
        }
    }

    /// Spec: Looking up an invalid type ID → 不被静默当作另一类型。
    #[test]
    fn invalid_id_returns_none() {
        let mut interner = TyInterner::new();
        let id = interner.intern(InternedTyKind::Unit);
        assert!(interner.try_lookup(id).is_some());

        let bogus = InternedTyId(9_999);
        assert!(interner.try_lookup(bogus).is_none());

        // 跨 interner 不应巧合命中。
        let other = TyInterner::new();
        assert!(other.try_lookup(id).is_none());
    }

    /// Spec: Existing tests continue to pass via compat layer
    ///       → intern_ty(materialize(id)) 必须是 id 自身。
    #[test]
    fn intern_ty_round_trip_preserves_structure_and_id() {
        let mut interner = TyInterner::new();
        let i32_ty = ty(TyKind::Int(IntKind::I32));
        let bool_ty = ty(TyKind::Bool);
        let tuple_ty = ty(TyKind::Tuple(vec![i32_ty, bool_ty]));

        let id = interner.intern_ty(&tuple_ty);
        let materialized = interner.materialize(id);

        // Display 一致：兼容层不应丢失结构性信息。
        assert_eq!(format!("{}", materialized), format!("{}", tuple_ty));

        // 再 intern 相同的 materialized 形态应返回同一 id（确认幂等）。
        let id2 = interner.intern_ty(&materialized);
        assert_eq!(id, id2);
    }

    /// 现有 Ty.id 是 per-instance 来源 tag，不应影响结构性相等。
    #[test]
    fn structurally_equal_tys_with_different_origin_share_id() {
        let mut interner = TyInterner::new();
        let a = Ty::new(7, TyKind::Int(IntKind::I32));
        let b = Ty::new(42, TyKind::Int(IntKind::I32));
        let id_a = interner.intern_ty(&a);
        let id_b = interner.intern_ty(&b);
        assert_eq!(id_a, id_b);
        assert_eq!(interner.len(), 1);
    }

    /// 深度嵌套的函数类型 round-trip：fn(i32, [bool; 3]) -> &mut Future<()>。
    #[test]
    fn deeply_nested_fn_intern_and_materialize() {
        let mut interner = TyInterner::new();
        let original = ty(TyKind::Fn {
            params: vec![
                ty(TyKind::Int(IntKind::I32)),
                ty(TyKind::Array(Box::new(ty(TyKind::Bool)), 3)),
            ],
            ret: Box::new(ty(TyKind::Ref(
                true,
                Box::new(ty(TyKind::Future(Box::new(ty(TyKind::Unit))))),
            ))),
            is_variadic: false,
        });
        let id = interner.intern_ty(&original);
        let materialized = interner.materialize(id);
        assert_eq!(format!("{}", materialized), format!("{}", original));
    }
}
