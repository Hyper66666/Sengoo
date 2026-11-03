//! Source-mapped lexical bindings. Facts are built once per indexed revision;
//! queries never parse or scan the workspace and never substitute by spelling.
use std::collections::HashMap;

use sengoo_compiler::ast::pattern::Pattern;
use sengoo_compiler::ast::{pattern::*, *};
use sengoo_compiler::lexer::{Lexer, Span, TokenKind};
use sengoo_compiler::Parser;
use tower_lsp::lsp_types::*;

use crate::protocol::SymbolOrigin;
use crate::text_editing::{position_to_byte_index, span_to_range};
use crate::workspace_index::{module_identity_from_snapshot, WorkspaceSnapshot};

#[derive(Debug, Clone)]
struct Binding {
    name: String,
    span: Span,
    scope: usize,
    container: Option<Vec<String>>,
    ty: Option<Vec<String>>,
    alias: Option<Vec<String>>,
    trait_member: bool,
}

#[derive(Debug, Clone)]
enum Lookup {
    Declaration(usize),
    Name(Vec<String>),
    Member(Option<Vec<String>>),
}

#[derive(Debug, Clone)]
struct Occurrence {
    name: String,
    span: Span,
    visible: Vec<usize>,
    lookup: Lookup,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct BindingFacts {
    safe: bool,
    bindings: Vec<Binding>,
    occurrences: Vec<Occurrence>,
    imports: Vec<(String, Vec<String>)>,
    wildcards: Vec<Vec<String>>,
}

impl BindingFacts {
    pub(crate) fn parse(source: &str) -> Self {
        let tokens = Lexer::new(source).collect::<Vec<_>>();
        // Surface attributes and macros can delete, insert, or generate AST
        // nodes. They need compiler expansion source maps before safe edits.
        if tokens.iter().any(|t| matches!(t.kind, TokenKind::Hash))
            || tokens.windows(2).any(|p| {
                matches!(p[0].kind, TokenKind::Ident) && matches!(p[1].kind, TokenKind::Not)
            })
        {
            return Self::default();
        }
        let Ok(program) = Parser::parse(source) else {
            return Self::default();
        };
        let mut walker = Walker {
            source,
            facts: Self {
                safe: true,
                ..Self::default()
            },
            visible: Vec::new(),
            scope: 0,
            next_scope: 1,
            owner: None,
        };
        walker.declarations(&program.decls, &[]);
        // Multiple meanings at one source token (for example shorthand fields)
        // require a structured rewrite. Keep navigation, but refuse such edits.
        walker.facts
    }
}

struct Walker<'a> {
    source: &'a str,
    facts: BindingFacts,
    visible: Vec<usize>,
    scope: usize,
    next_scope: usize,
    owner: Option<Vec<String>>,
}

fn path_names(path: &Path) -> Vec<String> {
    path.segments.iter().map(|id| id.name.clone()).collect()
}

fn type_path(ty: &Type) -> Option<Vec<String>> {
    match &ty.kind {
        TypeKind::Path(p) | TypeKind::PathWithArgs { path: p, .. } => Some(path_names(p)),
        TypeKind::Ref { base, .. } | TypeKind::Ptr { base, .. } => type_path(base),
        TypeKind::SelfType => Some(vec!["Self".into()]),
        _ => None,
    }
}

impl Walker<'_> {
    fn mapped(&mut self, id: &Ident) -> bool {
        let valid = self.source.get(id.span.lo as usize..id.span.hi as usize) == Some(&id.name);
        self.facts.safe &= valid;
        valid
    }

    fn occurrence(&mut self, id: &Ident, lookup: Lookup) {
        if self.mapped(id) {
            self.facts.occurrences.push(Occurrence {
                name: id.name.clone(),
                span: id.span,
                visible: self.visible.clone(),
                lookup,
            });
        }
    }

    fn bind(
        &mut self,
        id: &Ident,
        container: Option<Vec<String>>,
        ty: Option<Vec<String>>,
        lexical: bool,
    ) -> usize {
        let index = self.facts.bindings.len();
        self.facts.bindings.push(Binding {
            name: id.name.clone(),
            span: id.span,
            scope: self.scope,
            container,
            ty,
            alias: None,
            trait_member: false,
        });
        self.occurrence(id, Lookup::Declaration(index));
        if lexical {
            self.visible.push(index);
        }
        index
    }

    fn scoped(&mut self, f: impl FnOnce(&mut Self)) {
        let saved_len = self.visible.len();
        let previous = self.scope;
        self.scope = self.next_scope;
        self.next_scope += 1;
        f(self);
        self.visible.truncate(saved_len);
        self.scope = previous;
    }

    fn path(&mut self, path: &Path) {
        for (n, id) in path.segments.iter().enumerate() {
            self.occurrence(
                id,
                Lookup::Name(path.segments[..=n].iter().map(|x| x.name.clone()).collect()),
            );
        }
    }

    fn ty(&mut self, ty: &Type) {
        match &ty.kind {
            TypeKind::Path(path) => self.path(path),
            TypeKind::PathWithArgs { path, args } => {
                self.path(path);
                for t in args {
                    self.ty(t);
                }
            }
            TypeKind::Tuple(types) => {
                for t in types {
                    self.ty(t);
                }
            }
            TypeKind::Array(t, _)
            | TypeKind::Slice(t)
            | TypeKind::Ptr { base: t, .. }
            | TypeKind::Ref { base: t, .. } => self.ty(t),
            TypeKind::Fn { params, ret } => {
                for t in params {
                    self.ty(t);
                }
                if let Some(t) = ret {
                    self.ty(t);
                }
            }
            TypeKind::Dyn(bounds) | TypeKind::ImplTrait(bounds) => {
                for b in bounds {
                    self.path(&b.path);
                    for t in &b.params {
                        self.ty(t);
                    }
                }
            }
            _ => {}
        }
    }

    fn type_params(&mut self, params: &[TypeParam]) {
        for p in params {
            self.bind(&p.name, None, None, true);
        }
        for p in params {
            for b in &p.bounds {
                self.path(&b.path);
                for t in &b.params {
                    self.ty(t);
                }
            }
            if let Some(t) = &p.default {
                self.ty(t);
            }
        }
    }

    fn declarations(&mut self, decls: &[Decl], container: &[String]) {
        for decl in decls {
            if let Some(name) = decl.name() {
                let ty = match &decl.kind {
                    DeclKind::Function(f) => f.return_type.as_ref().and_then(type_path),
                    DeclKind::Const(c) => type_path(&c.ty),
                    DeclKind::Static(s) => type_path(&s.ty),
                    _ => None,
                };
                self.bind(name, Some(container.to_vec()), ty, true);
            }
            match &decl.kind {
                DeclKind::Struct(s) => {
                    let owner = [container, std::slice::from_ref(&s.name.name)].concat();
                    for f in &s.fields {
                        if let Some(id) = &f.name {
                            self.bind(id, Some(owner.clone()), type_path(&f.ty), false);
                        }
                    }
                }
                DeclKind::Class(c) => {
                    let owner = [container, std::slice::from_ref(&c.name.name)].concat();
                    for m in &c.members {
                        match m {
                            ClassMember::Field(f) => {
                                if let Some(id) = &f.name {
                                    self.bind(id, Some(owner.clone()), type_path(&f.ty), false);
                                }
                            }
                            ClassMember::Method(f) => {
                                self.bind(
                                    &f.name,
                                    Some(owner.clone()),
                                    f.return_type.as_ref().and_then(type_path),
                                    false,
                                );
                            }
                        }
                    }
                }
                DeclKind::Impl(i) => {
                    if let Some(owner) = type_path(&i.target_type) {
                        for f in &i.items {
                            let index = self.bind(
                                &f.name,
                                Some(owner.clone()),
                                f.return_type.as_ref().and_then(type_path),
                                false,
                            );
                            self.facts.bindings[index].trait_member = i.trait_path.is_some();
                        }
                    }
                }
                DeclKind::Enum(e) => {
                    let owner = [container, std::slice::from_ref(&e.name.name)].concat();
                    for v in &e.variants {
                        self.bind(&v.name, Some(owner.clone()), None, false);
                    }
                }
                DeclKind::Import(i) => {
                    let path = path_names(&i.path);
                    if let Some(alias) = &i.alias {
                        let index = self.bind(alias, None, None, true);
                        self.facts.bindings[index].alias = Some(path.clone());
                    } else {
                        match &i.kind {
                            ImportKind::Simple => {
                                if let Some(last) = path.last() {
                                    self.facts.imports.push((last.clone(), path.clone()));
                                }
                            }
                            ImportKind::Selective(ids) => {
                                for id in ids {
                                    let mut full = path.clone();
                                    full.push(id.name.clone());
                                    self.facts.imports.push((id.name.clone(), full));
                                }
                            }
                            ImportKind::Wildcard => self.facts.wildcards.push(path.clone()),
                        }
                    }
                }
                DeclKind::ExternBlock(e) => {
                    for item in &e.items {
                        match item {
                            ExternItem::Function(f) => {
                                self.bind(
                                    &f.name,
                                    Some(container.to_vec()),
                                    f.return_type.as_ref().and_then(type_path),
                                    true,
                                );
                            }
                            ExternItem::Static(s) => {
                                self.bind(
                                    &s.name,
                                    Some(container.to_vec()),
                                    type_path(&s.ty),
                                    true,
                                );
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        for decl in decls {
            self.declaration(decl, container);
        }
    }

    fn declaration(&mut self, decl: &Decl, container: &[String]) {
        match &decl.kind {
            DeclKind::Function(f) => self.function(f, None),
            DeclKind::Struct(s) => self.scoped(|w| {
                w.type_params(&s.type_params);
                for f in &s.fields {
                    w.ty(&f.ty);
                }
            }),
            DeclKind::Class(c) => self.scoped(|w| {
                w.type_params(&c.type_params);
                if let Some(p) = &c.extends {
                    w.path(p);
                }
                let owner = Some([container, std::slice::from_ref(&c.name.name)].concat());
                for m in &c.members {
                    match m {
                        ClassMember::Field(f) => w.ty(&f.ty),
                        ClassMember::Method(f) => w.function(f, owner.clone()),
                    }
                }
            }),
            DeclKind::Impl(i) => self.scoped(|w| {
                w.type_params(&i.type_params);
                w.ty(&i.target_type);
                if let Some(p) = &i.trait_path {
                    w.path(p);
                }
                // Trait dispatch does not identify a unique implementation.
                let owner = if i.trait_path.is_none() {
                    type_path(&i.target_type)
                } else {
                    None
                };
                for f in &i.items {
                    w.function(f, owner.clone());
                }
            }),
            DeclKind::Enum(e) => self.scoped(|w| {
                w.type_params(&e.type_params);
                for v in &e.variants {
                    for f in &v.fields {
                        match f {
                            VariantField::Named(_, t) | VariantField::Unnamed(t) => w.ty(t),
                        }
                    }
                    if let Some(e) = &v.discriminant {
                        w.expr(e);
                    }
                }
            }),
            DeclKind::Trait(t) => self.scoped(|w| {
                w.type_params(&t.type_params);
                // Declarations in a trait can participate in dynamic dispatch;
                // do not pretend these are uniquely resolved concrete members.
                for item in &t.items {
                    match item {
                        TraitItem::Function(f) => w.function(f, None),
                        TraitItem::Const(c) => {
                            w.ty(&c.ty);
                            w.expr(&c.value);
                        }
                        _ => {}
                    }
                }
            }),
            DeclKind::Const(c) => {
                self.ty(&c.ty);
                self.expr(&c.value);
            }
            DeclKind::Static(s) => {
                self.ty(&s.ty);
                self.expr(&s.value);
            }
            DeclKind::TypeAlias(t) => self.scoped(|w| {
                w.type_params(&t.type_params);
                w.ty(&t.ty);
            }),
            DeclKind::Import(i) => {
                self.path(&i.path);
                if let ImportKind::Selective(ids) = &i.kind {
                    for id in ids {
                        let mut path = path_names(&i.path);
                        path.push(id.name.clone());
                        self.occurrence(id, Lookup::Name(path));
                    }
                }
            }
            DeclKind::Module(m) => self.scoped(|w| {
                let owner = [container, std::slice::from_ref(&m.name.name)].concat();
                w.declarations(&m.items, &owner);
            }),
            DeclKind::ExternBlock(e) => {
                for item in &e.items {
                    match item {
                        ExternItem::Function(f) => self.scoped(|w| {
                            for p in &f.params {
                                w.ty(&p.ty);
                                w.bind(&p.name, None, type_path(&p.ty), true);
                            }
                            if let Some(t) = &f.return_type {
                                w.ty(t);
                            }
                        }),
                        ExternItem::Static(s) => self.ty(&s.ty),
                    }
                }
            }
        }
    }

    fn function(&mut self, f: &Function, owner: Option<Vec<String>>) {
        let old_owner = self.owner.clone();
        self.owner = owner;
        self.scoped(|w| {
            w.type_params(&f.type_params);
            for p in &f.params {
                w.ty(&p.ty);
                w.bind(&p.name, None, type_path(&p.ty), true);
            }
            if let Some(t) = &f.return_type {
                w.ty(t);
            }
            if let Some(e) = &f.precondition {
                w.expr(e);
            }
            if let Some(e) = &f.postcondition {
                w.expr(e);
            }
            w.block(&f.body);
        });
        self.owner = old_owner;
    }

    fn block(&mut self, block: &Block) {
        self.scoped(|w| {
            for stmt in &block.stmts {
                match &stmt.kind {
                    StmtKind::Let {
                        name, ty, value, ..
                    } => {
                        if let Some(t) = ty {
                            w.ty(t);
                        }
                        if let Some(e) = value {
                            w.expr(e);
                        }
                        let inferred = ty
                            .as_ref()
                            .and_then(type_path)
                            .or_else(|| value.as_deref().and_then(|e| w.expr_type(e)));
                        w.bind(name, None, inferred, true);
                    }
                    StmtKind::Const { name, ty, value } => {
                        w.ty(ty);
                        w.expr(value);
                        w.bind(name, None, type_path(ty), true);
                    }
                    StmtKind::Expr(e) => w.expr(e),
                    StmtKind::Item(decl) => {
                        w.declarations(std::slice::from_ref(decl.as_ref()), &[])
                    }
                }
            }
        });
    }

    fn expr_type(&self, e: &Expr) -> Option<Vec<String>> {
        if let ExprKind::Path(path) = &e.kind {
            if let Some(id) = path.as_simple() {
                if id.name == "self" {
                    return self.owner.clone();
                }
                return self
                    .visible
                    .iter()
                    .rev()
                    .find(|n| self.facts.bindings[**n].name == id.name)
                    .and_then(|n| self.facts.bindings[*n].ty.clone());
            }
        }
        match &e.kind {
            ExprKind::Ident(id) if id.name == "self" => self.owner.clone(),
            ExprKind::Ident(id) => self
                .visible
                .iter()
                .rev()
                .find(|n| self.facts.bindings[**n].name == id.name)
                .and_then(|n| self.facts.bindings[*n].ty.clone()),
            ExprKind::Struct { path, .. } => Some(path_names(path)),
            ExprKind::Call { func, .. } => self.expr_type(func),
            ExprKind::Unary { operand, .. } | ExprKind::Paren(operand) => self.expr_type(operand),
            ExprKind::Cast { ty, .. } => type_path(ty),
            _ => None,
        }
    }

    fn pattern(&mut self, p: &Pattern) {
        match &p.kind {
            PatternKind::Ident(id) => {
                self.bind(id, None, None, true);
            }
            PatternKind::Path(p) => self.path(p),
            PatternKind::Struct { path, fields, .. } => {
                self.path(path);
                for f in fields {
                    self.occurrence(&f.name, Lookup::Member(Some(path_names(path))));
                    self.pattern(&f.pattern);
                }
            }
            PatternKind::TupleStruct { path, patterns } => {
                self.path(path);
                for p in patterns {
                    self.pattern(p);
                }
            }
            PatternKind::Tuple(ps) => {
                for p in ps {
                    self.pattern(p);
                }
            }
            PatternKind::Or(ps) => {
                self.facts.safe = false;
                for p in ps {
                    self.pattern(p);
                }
            }
            PatternKind::Slice(ps, rest) => {
                for p in ps {
                    self.pattern(p);
                }
                if let Some(p) = rest {
                    self.pattern(p);
                }
            }
            PatternKind::Range(a, b, _) => {
                self.pattern(a);
                self.pattern(b);
            }
            _ => {}
        }
    }

    fn expr(&mut self, e: &Expr) {
        match &e.kind {
            ExprKind::Ident(id) => self.occurrence(id, Lookup::Name(vec![id.name.clone()])),
            ExprKind::Path(p) => self.path(p),
            ExprKind::Unary { operand, .. }
            | ExprKind::Await(operand)
            | ExprKind::Try(operand)
            | ExprKind::Paren(operand) => self.expr(operand),
            ExprKind::Binary { left, right, .. }
            | ExprKind::Assign {
                target: left,
                value: right,
            }
            | ExprKind::AssignOp {
                target: left,
                value: right,
                ..
            }
            | ExprKind::Index {
                base: left,
                index: right,
            } => {
                self.expr(left);
                self.expr(right);
            }
            ExprKind::Call { func, args } => {
                self.expr(func);
                for a in args {
                    self.expr(a);
                }
            }
            ExprKind::MethodCall {
                receiver,
                method,
                args,
            } => {
                self.expr(receiver);
                self.occurrence(method, Lookup::Member(self.expr_type(receiver)));
                for a in args {
                    self.expr(a);
                }
            }
            ExprKind::Field { base, field } => {
                self.expr(base);
                self.occurrence(field, Lookup::Member(self.expr_type(base)));
            }
            ExprKind::Block(b)
            | ExprKind::Loop(b)
            | ExprKind::AsyncBlock(b)
            | ExprKind::ParallelBlock(b)
            | ExprKind::TryBlock(b) => self.block(b),
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.expr(cond);
                self.block(then_branch);
                if let Some(e) = else_branch {
                    self.expr(e);
                }
            }
            ExprKind::While { cond, body } => {
                self.expr(cond);
                self.block(body);
            }
            ExprKind::For {
                pattern,
                iter,
                body,
            } => {
                self.expr(iter);
                self.scoped(|w| {
                    w.pattern(pattern);
                    w.block(body);
                });
            }
            ExprKind::Match { scrutinee, arms } => {
                self.expr(scrutinee);
                for arm in arms {
                    self.scoped(|w| {
                        // Alternative patterns bind the same logical names, but their
                        // source declarations need a unified identity before editing.
                        if arm.patterns.len() > 1 {
                            w.facts.safe = false;
                        }
                        for p in &arm.patterns {
                            w.pattern(p);
                        }
                        if let Some(e) = &arm.guard {
                            w.expr(e);
                        }
                        w.expr(&arm.body);
                    });
                }
            }
            ExprKind::Return(e) | ExprKind::Break(e) | ExprKind::Yield(e) => {
                if let Some(e) = e {
                    self.expr(e);
                }
            }
            ExprKind::Array(es) | ExprKind::Tuple(es) => {
                for e in es {
                    self.expr(e);
                }
            }
            ExprKind::Struct { path, fields, base } => {
                self.path(path);
                for f in fields {
                    if let FieldName::Ident(id) = &f.name {
                        self.occurrence(id, Lookup::Member(Some(path_names(path))));
                    }
                    self.expr(&f.value);
                }
                if let Some(e) = base {
                    self.expr(e);
                }
            }
            ExprKind::Range { start, end, .. } => {
                if let Some(e) = start {
                    self.expr(e);
                }
                if let Some(e) = end {
                    self.expr(e);
                }
            }
            ExprKind::Lambda { params, body } => self.scoped(|w| {
                for p in params {
                    w.bind(p, None, None, true);
                }
                w.expr(body);
            }),
            ExprKind::Cast { expr, ty } | ExprKind::Is { expr, ty } => {
                self.expr(expr);
                self.ty(ty);
            }
            ExprKind::Literal(_) | ExprKind::Continue => {}
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Key {
    uri: Url,
    binding: usize,
}

struct Resolver<'a> {
    snapshot: &'a WorkspaceSnapshot,
    renamed: Option<(&'a Key, &'a str)>,
}

impl Resolver<'_> {
    fn name(&self, key: &Key, binding: &Binding) -> String {
        if let Some((target, name)) = self.renamed {
            if target == key {
                return name.into();
            }
        }
        binding.name.clone()
    }

    fn binding(&self, key: &Key) -> Option<&Binding> {
        self.snapshot
            .documents
            .get(&key.uri)?
            .bindings
            .bindings
            .get(key.binding)
    }

    fn module(&self, uri: &Url) -> Vec<String> {
        module_identity_from_snapshot(self.snapshot, uri)
            .map(|m| m.import_path.split("::").map(str::to_string).collect())
            .unwrap_or_default()
    }

    fn adjusted_path(&self, uri: &Url, visible: &[usize], path: &[String]) -> Vec<String> {
        let Some((target, new_name)) = self.renamed else {
            return path.to_vec();
        };
        let original = Resolver {
            snapshot: self.snapshot,
            renamed: None,
        };
        let mut changed = path.to_vec();
        for n in 0..path.len() {
            if original.lookup_path(uri, visible, &path[..=n]).as_ref() == Some(target) {
                changed[n] = new_name.into();
            }
        }
        changed
    }

    fn globals(&self, uri: &Url, path: &[String]) -> Vec<Key> {
        let current_module = self.module(uri);
        let current_identity = module_identity_from_snapshot(self.snapshot, uri);
        let mut results = Vec::new();
        for (doc_uri, doc) in &self.snapshot.documents {
            if !doc.parse_valid || !doc.bindings.safe {
                continue;
            }
            let module = self.module(doc_uri);
            for (n, b) in doc.bindings.bindings.iter().enumerate() {
                let Some(container) = &b.container else {
                    continue;
                };
                let key = Key {
                    uri: doc_uri.clone(),
                    binding: n,
                };
                let original_full = [
                    module.as_slice(),
                    container.as_slice(),
                    std::slice::from_ref(&b.name),
                ]
                .concat();
                let full = self.adjusted_path(doc_uri, &[], &original_full);
                let local = full[module.len()..].to_vec();
                let same_package = match (
                    &current_identity,
                    module_identity_from_snapshot(self.snapshot, doc_uri),
                ) {
                    (Some(a), Some(b)) => a.package_name == b.package_name && a.origin == b.origin,
                    _ => {
                        doc.origin == SymbolOrigin::Workspace
                            && self
                                .snapshot
                                .documents
                                .get(uri)
                                .is_some_and(|d| d.origin == SymbolOrigin::Workspace)
                    }
                };
                if path == full
                    || (same_package && path == local && (path.len() > 1 || container.is_empty()))
                    || (!current_module.is_empty()
                        && full == [current_module.as_slice(), path].concat())
                {
                    results.push(key);
                }
            }
        }
        results
    }

    fn unique(mut keys: Vec<Key>) -> Option<Key> {
        keys.sort_by(|a, b| {
            a.uri
                .as_str()
                .cmp(b.uri.as_str())
                .then(a.binding.cmp(&b.binding))
        });
        keys.dedup();
        (keys.len() == 1).then(|| keys.remove(0))
    }

    fn lookup_path(&self, uri: &Url, visible: &[usize], path: &[String]) -> Option<Key> {
        let first = path.first()?;
        let facts = &self.snapshot.documents.get(uri)?.bindings;
        for n in visible.iter().rev() {
            let key = Key {
                uri: uri.clone(),
                binding: *n,
            };
            let binding = &facts.bindings[*n];
            if self.name(&key, binding) != *first {
                continue;
            }
            if path.len() == 1 {
                return Some(key);
            }
            let expanded = if let Some(alias) = &binding.alias {
                [self.adjusted_path(uri, &[], alias).as_slice(), &path[1..]].concat()
            } else if let Some(container) = &binding.container {
                [
                    self.module(uri).as_slice(),
                    container.as_slice(),
                    &[self.name(&key, binding)],
                    &path[1..],
                ]
                .concat()
            } else {
                return None;
            };
            return Self::unique(self.globals(uri, &expanded));
        }
        let mut imports = Vec::new();
        for (name, imported) in &facts.imports {
            let adjusted = self.adjusted_path(uri, &[], imported);
            let effective_name = if imported.last() == Some(name) {
                adjusted.last().unwrap_or(name)
            } else {
                name
            };
            if effective_name == first {
                let expanded = [adjusted.as_slice(), &path[1..]].concat();
                imports.extend(self.globals(uri, &expanded));
            }
            // A simple module import exposes that module's declarations using
            // the compiler's established unqualified-import convention.
            imports.extend(self.globals(uri, &[adjusted.as_slice(), path].concat()));
        }
        for imported in &facts.wildcards {
            imports.extend(self.globals(uri, &[imported.as_slice(), path].concat()));
        }
        if !imports.is_empty() {
            return Self::unique(imports);
        }
        Self::unique(self.globals(uri, path))
    }

    fn resolve(&self, uri: &Url, occurrence: &Occurrence) -> Option<Key> {
        match &occurrence.lookup {
            Lookup::Declaration(n) => Some(Key {
                uri: uri.clone(),
                binding: *n,
            }),
            Lookup::Name(path) => {
                let path = self.adjusted_path(uri, &occurrence.visible, path);
                self.lookup_path(uri, &occurrence.visible, &path)
            }
            Lookup::Member(Some(owner)) => {
                let owner = self.adjusted_path(uri, &occurrence.visible, owner);
                let mut owner_key = self.lookup_path(uri, &occurrence.visible, &owner)?;
                if let Some(alias) = &self.binding(&owner_key)?.alias {
                    owner_key = self.lookup_path(uri, &[], &self.adjusted_path(uri, &[], alias))?;
                }
                let owner_binding = self.binding(&owner_key)?;
                let mut name = occurrence.name.clone();
                if let Some((target, new_name)) = self.renamed {
                    if (Resolver {
                        snapshot: self.snapshot,
                        renamed: None,
                    })
                    .resolve(uri, occurrence)
                    .as_ref()
                        == Some(target)
                    {
                        name = new_name.into();
                    }
                }
                let owner_name = self.name(&owner_key, owner_binding);
                let full = [
                    self.module(&owner_key.uri),
                    owner_binding.container.clone().unwrap_or_default(),
                    vec![owner_name, name],
                ]
                .concat();
                Self::unique(self.globals(uri, &full))
            }
            Lookup::Member(None) => None,
        }
    }

    fn cursor(&self, uri: &Url, position: Position) -> Option<(&Occurrence, Key)> {
        let doc = self.snapshot.documents.get(uri)?;
        debug_assert_eq!(uri, &doc.uri);
        if !doc.parse_valid || !doc.bindings.safe {
            return None;
        }
        let byte = position_to_byte_index(&doc.content, position)? as u32;
        let mut candidates = doc
            .bindings
            .occurrences
            .iter()
            .filter(|o| o.span.lo <= byte && byte < o.span.hi)
            .collect::<Vec<_>>();
        if candidates.is_empty() {
            candidates = doc
                .bindings
                .occurrences
                .iter()
                .filter(|o| o.span.hi == byte)
                .collect();
        }
        let first = *candidates.first()?;
        let target = self.resolve(uri, first)?;
        for occurrence in candidates {
            if self.resolve(uri, occurrence).as_ref() != Some(&target) {
                return None;
            }
        }
        Some((first, target))
    }

    fn writable(&self, key: &Key) -> bool {
        self.snapshot.documents.get(&key.uri).is_some_and(|d| {
            matches!(
                d.origin,
                SymbolOrigin::Workspace | SymbolOrigin::CurrentDocument
            )
        })
    }

    fn location(&self, uri: &Url, span: Span) -> Option<Location> {
        let doc = self.snapshot.documents.get(uri)?;
        Some(Location::new(
            uri.clone(),
            span_to_range(&doc.content, span.lo, span.hi),
        ))
    }

    fn occurrences(&self, key: &Key, include_declaration: bool) -> Vec<Location> {
        let mut locations = Vec::new();
        for (uri, doc) in &self.snapshot.documents {
            if !doc.parse_valid || !doc.bindings.safe {
                continue;
            }
            for o in &doc.bindings.occurrences {
                if !include_declaration && matches!(o.lookup, Lookup::Declaration(_)) {
                    continue;
                }
                if self.resolve(uri, o).as_ref() == Some(key) {
                    if let Some(loc) = self.location(uri, o.span) {
                        locations.push(loc);
                    }
                }
            }
        }
        locations.sort_by(|a, b| {
            a.uri
                .as_str()
                .cmp(b.uri.as_str())
                .then(a.range.start.line.cmp(&b.range.start.line))
                .then(a.range.start.character.cmp(&b.range.start.character))
        });
        locations.dedup();
        locations
    }
}

pub(crate) fn definition(
    snapshot: &WorkspaceSnapshot,
    uri: &Url,
    position: Position,
) -> Option<GotoDefinitionResponse> {
    let resolver = Resolver {
        snapshot,
        renamed: None,
    };
    let (_, target) = resolver.cursor(uri, position)?;
    Some(GotoDefinitionResponse::Scalar(
        resolver.location(&target.uri, resolver.binding(&target)?.span)?,
    ))
}

pub(crate) fn references(
    snapshot: &WorkspaceSnapshot,
    uri: &Url,
    position: Position,
    include_declaration: bool,
) -> Option<Vec<Location>> {
    let resolver = Resolver {
        snapshot,
        renamed: None,
    };
    let (_, target) = resolver.cursor(uri, position)?;
    Some(resolver.occurrences(&target, include_declaration))
}

pub(crate) fn prepare(
    snapshot: &WorkspaceSnapshot,
    uri: &Url,
    position: Position,
) -> Option<PrepareRenameResponse> {
    let resolver = Resolver {
        snapshot,
        renamed: None,
    };
    let (occurrence, target) = resolver.cursor(uri, position)?;
    if !resolver.writable(&target) || resolver.binding(&target)?.trait_member {
        return None;
    }
    Some(PrepareRenameResponse::RangeWithPlaceholder {
        range: resolver.location(uri, occurrence.span)?.range,
        placeholder: occurrence.name.clone(),
    })
}

pub(crate) fn rename(
    snapshot: &WorkspaceSnapshot,
    uri: &Url,
    position: Position,
    new_name: &str,
    versioned: bool,
) -> Result<Option<WorkspaceEdit>, String> {
    // The compiler lexer is the authority for keywords and identifier syntax.
    let tokens = Lexer::new(new_name).collect::<Vec<_>>();
    if new_name == "_"
        || tokens.len() != 1
        || !matches!(tokens[0].kind, TokenKind::Ident)
        || tokens[0].span != Span::new(0, new_name.len() as u32)
    {
        return Err("new name must be a non-keyword Sengoo identifier".into());
    }
    let resolver = Resolver {
        snapshot,
        renamed: None,
    };
    let Some((_, target)) = resolver.cursor(uri, position) else {
        return Err("symbol is ambiguous, incomplete, or lacks a reliable source mapping".into());
    };
    if !resolver.writable(&target) {
        return Err("dependency and standard-library declarations are read-only".into());
    }
    let binding = resolver.binding(&target).unwrap();
    if binding.trait_member {
        return Err(
            "trait implementation members require linked trait/dispatch identities; rename refused"
                .into(),
        );
    }
    let changed = Resolver {
        snapshot,
        renamed: Some((&target, new_name)),
    };
    for (doc_uri, doc) in &snapshot.documents {
        if !doc.parse_valid || !doc.bindings.safe {
            // A broken workspace document may contain references that cannot
            // safely be updated. Local bindings are confined to their own URI.
            if binding.container.is_some() || *doc_uri == target.uri {
                return Err(
                    "workspace contains incomplete or unmapped source; repair it before renaming"
                        .into(),
                );
            }
            continue;
        }
        for (n, other) in doc.bindings.bindings.iter().enumerate() {
            if other.name == new_name
                && (Key {
                    uri: doc_uri.clone(),
                    binding: n,
                }) != target
                && ((doc_uri == &target.uri
                    && other.scope == binding.scope
                    && other.container == binding.container)
                    || (binding.container.is_some()
                        && other.container == binding.container
                        && resolver.module(doc_uri) == resolver.module(&target.uri)))
            {
                return Err("new name conflicts with another declaration".into());
            }
        }
        for o in &doc.bindings.occurrences {
            let before = resolver.resolve(doc_uri, o);
            if before.is_none()
                && o.name == binding.name
                && binding.container.is_some()
                && matches!(o.lookup, Lookup::Name(_))
            {
                return Err("an occurrence cannot be resolved uniquely; rename refused".into());
            }
            if before.is_none()
                && matches!(o.lookup, Lookup::Member(_))
                && o.name == binding.name
                && binding.container.as_ref().is_some_and(|c| !c.is_empty())
            {
                return Err(
                    "a receiver cannot be resolved statically; member rename refused".into(),
                );
            }
            if changed.resolve(doc_uri, o) != before {
                return Err(
                    "new name would capture a reference or change symbol resolution".into(),
                );
            }
        }
    }
    let mut changes: HashMap<Url, Vec<TextEdit>> = HashMap::new();
    for location in resolver.occurrences(&target, true) {
        if !resolver.writable(&Key {
            uri: location.uri.clone(),
            binding: 0,
        }) {
            return Err("rename would modify a read-only dependency".into());
        }
        let document = &snapshot.documents[&location.uri];
        for o in &document.bindings.occurrences {
            if span_to_range(&document.content, o.span.lo, o.span.hi) == location.range
                && resolver.resolve(&location.uri, o).as_ref() != Some(&target)
            {
                return Err(
                    "shorthand token has multiple meanings; expand it before renaming".into(),
                );
            }
        }
        changes.entry(location.uri).or_default().push(TextEdit {
            range: location.range,
            new_text: new_name.into(),
        });
    }
    if versioned {
        let mut documents = changes
            .into_iter()
            .map(|(uri, edits)| TextDocumentEdit {
                text_document: OptionalVersionedTextDocumentIdentifier {
                    version: snapshot.documents[&uri].revision,
                    uri,
                },
                edits: edits.into_iter().map(OneOf::Left).collect(),
            })
            .collect::<Vec<_>>();
        documents.sort_by(|a, b| {
            a.text_document
                .uri
                .as_str()
                .cmp(b.text_document.uri.as_str())
        });
        Ok(Some(WorkspaceEdit {
            document_changes: Some(DocumentChanges::Edits(documents)),
            ..WorkspaceEdit::default()
        }))
    } else {
        Ok(Some(WorkspaceEdit {
            changes: Some(changes),
            ..WorkspaceEdit::default()
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace_index::{IndexCancellation, WorkspaceIndex};
    use std::sync::Arc;

    fn fixture(source: &str) -> (WorkspaceIndex, Url) {
        let index = WorkspaceIndex::default();
        let uri = Url::parse("file:///bindings.sg").unwrap();
        assert!(index.open(uri.clone(), 7, source.into(), &IndexCancellation::default()));
        assert!(
            index.document(&uri).unwrap().parse_valid,
            "fixture must parse: {source}"
        );
        assert!(
            index.document(&uri).unwrap().bindings.safe,
            "fixture must have mapped spans: {source}"
        );
        (index, uri)
    }

    fn position(source: &str, needle: &str) -> Position {
        let offset = source.find(needle).unwrap() as u32;
        span_to_range(source, offset, offset).start
    }

    fn edits(
        index: &WorkspaceIndex,
        uri: &Url,
        source: &str,
        needle: &str,
        name: &str,
    ) -> Vec<TextEdit> {
        index
            .rename(uri, position(source, needle), name, false)
            .unwrap()
            .unwrap()
            .changes
            .unwrap()
            .remove(uri)
            .unwrap()
    }

    #[test]
    fn rename_local_never_touches_another_functions_same_name() {
        let source = "def first(value: i64) -> i64 { value + value }\ndef second(value: i64) -> i64 { value }\n";
        let (index, uri) = fixture(source);
        let edits = edits(&index, &uri, source, "value +", "answer");
        assert_eq!(edits.len(), 3);
        assert!(edits.iter().all(|edit| edit.range.start.line == 0));
        let references = index
            .references(&uri, position(source, "value +"), false)
            .unwrap();
        assert_eq!(
            references.len(),
            2,
            "exclude the declaration, not the cursor use"
        );
        let GotoDefinitionResponse::Scalar(definition) = index
            .goto_definition(&uri, position(source, "value +"))
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(definition.range.start.character, 10);
    }

    #[test]
    fn rename_respects_block_shadowing_and_initializer_scope() {
        let source =
            "def main() -> i64 { let value = 1; { let value = value + 1; value; }; value }";
        let (index, uri) = fixture(source);
        let edits = edits(&index, &uri, source, "value = 1", "outer");
        assert_eq!(
            edits.len(),
            3,
            "outer declaration, inner initializer and final outer use"
        );
        assert!(!edits
            .iter()
            .any(|edit| edit.range.start == position(source, "value = value")));
    }

    #[test]
    fn rename_tracks_closure_capture_and_lambda_parameter() {
        let source = "def main() -> i64 { let value = 1; let f = |other| value + other; value }";
        let (index, uri) = fixture(source);
        assert_eq!(edits(&index, &uri, source, "value =", "captured").len(), 3);
        assert_eq!(edits(&index, &uri, source, "other|", "argument").len(), 2);
    }

    #[test]
    fn nested_closure_capture_preserves_each_lexical_binding() {
        let source = "def main(value: i64) -> i64 { let outer = |other| { let inner = |last| value + other + last; value }; value }";
        let (index, uri) = fixture(source);
        assert_eq!(edits(&index, &uri, source, "value:", "captured").len(), 4);
        assert_eq!(edits(&index, &uri, source, "other|", "argument").len(), 2);
        assert_eq!(edits(&index, &uri, source, "last|", "leaf").len(), 2);
    }

    #[test]
    fn rename_tracks_loop_and_match_bindings() {
        let source =
            "def main() -> i64 { for item in [1, 2] { item; }; match 1 { bound => bound } }";
        let (index, uri) = fixture(source);
        assert_eq!(edits(&index, &uri, source, "item in", "element").len(), 2);
        assert_eq!(edits(&index, &uri, source, "bound =>", "matched").len(), 2);
    }

    #[test]
    fn rename_rejects_declaration_conflict_and_both_capture_directions() {
        let source = "def main(left: i64, right: i64) -> i64 { left + right }";
        let (index, uri) = fixture(source);
        assert!(index
            .rename(&uri, position(source, "left +"), "right", false)
            .is_err());
        let source = "def main(outer: i64) -> i64 { { let inner = 1; outer + inner }; outer }";
        let (index, uri) = fixture(source);
        assert!(index
            .rename(&uri, position(source, "outer:"), "inner", false)
            .is_err());
        assert!(index
            .rename(&uri, position(source, "inner ="), "outer", false)
            .is_err());
        for invalid in ["if", "let", "1x", "x-y", ""] {
            assert!(
                index
                    .rename(&uri, position(source, "outer:"), invalid, false)
                    .is_err(),
                "{invalid}"
            );
        }
    }

    #[test]
    fn rename_fields_uses_owner_identity_and_rejects_unknown_receivers() {
        let source = "struct A { value: i64 }\nstruct B { value: i64 }\ndef use(a: A, b: B) -> i64 { a.value + b.value }";
        let (index, uri) = fixture(source);
        let edits = edits(&index, &uri, source, "value: i64", "count");
        assert_eq!(edits.len(), 2);
        assert!(edits.iter().all(|edit| edit.range.start.line != 1));
        let source = "struct A { value: i64 }\ndef use(a: A) -> i64 { unknown.value + a.value }";
        let (index, uri) = fixture(source);
        assert!(index
            .rename(&uri, position(source, "value:"), "count", false)
            .is_err());
    }

    #[test]
    fn rename_unicode_uses_utf16_and_preserves_comments_and_strings() {
        let source =
            "def main() -> i64 { let text = \"你好😀 value\"; let value = 1; /* value */ value }";
        let (index, uri) = fixture(source);
        let edits = edits(&index, &uri, source, "value =", "answer");
        assert_eq!(edits.len(), 2);
        assert_eq!(edits[0].range.start, position(source, "value ="));
        assert_eq!(
            edits[0].range.end.character - edits[0].range.start.character,
            5
        );
    }

    #[test]
    fn rename_binds_edits_to_document_version_and_rejects_stale_snapshot() {
        let source = "def main(value: i64) -> i64 { value }";
        let (index, uri) = fixture(source);
        let snapshot = index.snapshot();
        let response = index
            .rename(&uri, position(source, "value }"), "answer", true)
            .unwrap()
            .unwrap();
        assert!(response.changes.is_none());
        let DocumentChanges::Edits(documents) = response.document_changes.unwrap() else {
            panic!()
        };
        assert_eq!(documents[0].text_document.version, Some(7));
        assert!(index.open(
            uri.clone(),
            8,
            source.replace("value", "current"),
            &IndexCancellation::default()
        ));
        assert!(index
            .rename_from_snapshot(&snapshot, &uri, position(source, "value }"), "answer", true)
            .is_err());
    }

    #[test]
    fn rename_refuses_received_changes_before_their_snapshot_is_published() {
        let source = "def main(value: i64) -> i64 { value }";
        let (index, uri) = fixture(source);
        let snapshot = index.snapshot();
        let cursor = position(source, "value }");
        // Models didChange arriving before its blocking parser is scheduled.
        index.observe_document_revision(&uri, 8);
        assert!(Arc::ptr_eq(&snapshot, &index.snapshot()));
        assert!(index.prepare_rename(&uri, cursor).is_none());
        for versioned in [false, true] {
            assert!(index.rename(&uri, cursor, "answer", versioned).is_err());
        }
        assert!(index.open(uri.clone(), 8, source.into(), &IndexCancellation::default()));
        assert!(index.prepare_rename(&uri, cursor).is_some());
        assert_eq!(edits(&index, &uri, source, "value }", "answer").len(), 2);
    }

    #[test]
    fn closing_clears_pending_versions_and_reopening_can_restart_versions() {
        let project = Project::new();
        let source = "def main(value: i64) -> i64 { value }";
        let uri = project.source("main.sg", source);
        let index = project.index();
        let cursor = position(source, "value }");
        assert!(index.open(uri.clone(), 7, source.into(), &IndexCancellation::default()));
        index.observe_document_revision(&uri, 8);
        assert!(index.prepare_rename(&uri, cursor).is_none());
        assert!(index.close(&uri));
        assert!(index.prepare_rename(&uri, cursor).is_some());
        assert_eq!(edits(&index, &uri, source, "value }", "answer").len(), 2);
        assert!(index.open(uri.clone(), 1, source.into(), &IndexCancellation::default()));
        assert!(index.prepare_rename(&uri, cursor).is_some());
        assert_eq!(edits(&index, &uri, source, "value }", "answer").len(), 2);
        let unknown = Url::parse("file:///unpublished.sg").unwrap();
        index.observe_document_revision(&unknown, 100);
        assert!(index.prepare_rename(&uri, cursor).is_none());
        assert!(!index.close(&unknown));
        assert!(index.prepare_rename(&uri, cursor).is_some());
    }

    #[test]
    fn rename_refuses_incomplete_overlays_instead_of_using_last_good_spans() {
        let source = "def main(value: i64) -> i64 { value }";
        let (index, uri) = fixture(source);
        assert!(index.open(
            uri.clone(),
            8,
            "def main(value:".into(),
            &IndexCancellation::default()
        ));
        assert!(index.prepare_rename(&uri, Position::new(0, 10)).is_none());
        assert!(index
            .rename(&uri, Position::new(0, 10), "answer", true)
            .is_err());
        assert!(index.references(&uri, Position::new(0, 10), true).is_none());
    }

    #[test]
    fn warm_symbol_queries_do_not_reparse_or_scan() {
        let source = "def main(value: i64) -> i64 { value }";
        let (index, uri) = fixture(source);
        let before = index.metrics();
        for _ in 0..100 {
            assert!(index
                .prepare_rename(&uri, position(source, "value }"))
                .is_some());
            assert!(index
                .references(&uri, position(source, "value }"), true)
                .is_some());
            assert!(index
                .goto_definition(&uri, position(source, "value }"))
                .is_some());
        }
        let after = index.metrics();
        assert_eq!(before.parsed_documents, after.parsed_documents);
        assert_eq!(before.disk_reads, after.disk_reads);
        assert_eq!(before.recursive_scans, after.recursive_scans);
    }

    struct Project(std::path::PathBuf);

    impl Project {
        fn new() -> Self {
            static NEXT_PROJECT: std::sync::atomic::AtomicU64 =
                std::sync::atomic::AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "sglsp-bindings-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT_PROJECT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            std::fs::create_dir_all(path.join("src")).unwrap();
            std::fs::write(path.join("Sengoo.toml"), "[package]\nname = \"example\"\n").unwrap();
            Self(path)
        }

        fn source(&self, name: &str, source: &str) -> Url {
            let path = self.0.join("src").join(name);
            std::fs::write(&path, source).unwrap();
            Url::from_file_path(path).unwrap()
        }

        fn index(&self) -> WorkspaceIndex {
            WorkspaceIndex::build(std::slice::from_ref(&self.0), IndexCancellation::default())
                .unwrap()
        }
    }

    impl Drop for Project {
        fn drop(&mut self) {
            assert!(self.0.starts_with(std::env::temp_dir()));
            assert!(self
                .0
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("sglsp-bindings-"));
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn import_alias_is_a_binding_independent_from_the_original_declaration() {
        let project = Project::new();
        let original = project.source(
            "library.sg",
            "pub def calculate(value: i64) -> i64 { value }\n",
        );
        project.source(
            "unrelated.sg",
            "pub def calculate(value: i64) -> i64 { value + 1 }\n",
        );
        let source = "import example::library::calculate as work;\ndef main() -> i64 { work(3) }";
        let uri = project.source("main.sg", source);
        let index = project.index();
        let changes = index
            .rename(&uri, position(source, "work;"), "compute", false)
            .unwrap()
            .unwrap()
            .changes
            .unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[&uri].len(), 2);
        assert!(!changes.contains_key(&original));
        let changes = index
            .rename(&uri, position(source, "calculate as"), "evaluate", false)
            .unwrap()
            .unwrap()
            .changes
            .unwrap();
        assert_eq!(changes[&original].len(), 1);
        assert_eq!(
            changes[&uri].len(),
            1,
            "only original import path changes; alias uses remain stable"
        );
    }

    #[test]
    fn selective_import_and_qualified_calls_track_exact_module_identity() {
        let project = Project::new();
        let original = project.source(
            "library.sg",
            "pub def calculate(value: i64) -> i64 { value }\n",
        );
        let other = project.source(
            "unrelated.sg",
            "pub def calculate(value: i64) -> i64 { value + 1 }\n",
        );
        let source = "import example::library { calculate };\ndef main() -> i64 { calculate(3) + example::library::calculate(4) }";
        let uri = project.source("main.sg", source);
        let index = project.index();
        let changes = index
            .rename(&uri, position(source, "calculate(3)"), "evaluate", false)
            .unwrap()
            .unwrap()
            .changes
            .unwrap();
        assert_eq!(changes[&original].len(), 1);
        assert_eq!(changes[&uri].len(), 3);
        assert!(!changes.contains_key(&other));
    }

    #[test]
    fn aliased_type_member_uses_its_original_module_identity() {
        let project = Project::new();
        let original = project.source("library.sg", "pub struct Counter { value: i64 }\n");
        let unrelated = project.source("unrelated.sg", "pub struct Counter { value: i64 }\n");
        let source = "import example::library::Counter as Model;\ndef main(counter: Model) -> i64 { counter.value }";
        let uri = project.source("main.sg", source);
        let index = project.index();
        let changes = index
            .rename(&uri, position(source, "value }"), "count", false)
            .unwrap()
            .unwrap()
            .changes
            .unwrap();
        assert_eq!(changes[&original].len(), 1);
        assert_eq!(changes[&uri].len(), 1);
        assert!(!changes.contains_key(&unrelated));
    }

    #[test]
    fn dependencies_are_navigable_but_read_only_and_local_aliases_remain_editable() {
        let project = Project::new();
        let dependency = project.0.join("dependency");
        std::fs::create_dir_all(dependency.join("src")).unwrap();
        std::fs::write(
            dependency.join("Sengoo.toml"),
            "[package]\nname = \"dependency\"\n",
        )
        .unwrap();
        std::fs::write(
            dependency.join("src/lib.sg"),
            "pub def answer() -> i64 { 42 }\n",
        )
        .unwrap();
        std::fs::write(project.0.join("Sengoo.toml"), "[package]\nname = \"example\"\n[dependencies]\ndep = { package = \"dependency\", path = \"dependency\" }\n").unwrap();
        std::fs::write(project.0.join("Sengoo.lock"), "version = 1\nroot = \"example\"\n[[package]]\nname = \"dependency\"\nversion = \"0.1.0\"\nsource = \"path+dependency\"\nmanifest = \"dependency/Sengoo.toml\"\n").unwrap();
        let source = "import dep::answer as external;\ndef main() -> i64 { external() }";
        let uri = project.source("main.sg", source);
        let index = project.index();
        let position = position(source, "answer as");
        let GotoDefinitionResponse::Scalar(definition) =
            index.goto_definition(&uri, position).unwrap()
        else {
            panic!()
        };
        assert!(definition.uri.as_str().ends_with("/dependency/src/lib.sg"));
        assert!(index.prepare_rename(&uri, position).is_none());
        assert!(index.rename(&uri, position, "changed", false).is_err());
        assert_eq!(
            edits(&index, &uri, source, "external;", "local_alias").len(),
            2
        );
    }

    #[test]
    fn concrete_methods_and_type_rename_preserve_member_identity() {
        let source = "struct Counter { value: i64 }\nimpl Counter { def read(self) -> i64 { self.value } }\ndef main(counter: Counter) -> i64 { counter.read() }";
        let (index, uri) = fixture(source);
        assert_eq!(edits(&index, &uri, source, "read(self)", "count").len(), 2);
        assert_eq!(edits(&index, &uri, source, "Counter {", "Meter").len(), 3);
    }

    #[test]
    fn trait_implementation_methods_refuse_unlinked_trait_renames() {
        let source = "trait Read { def read(self) -> i64 { 0 } }\nstruct Counter { value: i64 }\nimpl Read for Counter { def read(self) -> i64 { self.value } }\ndef main(counter: Counter) -> i64 { counter.read() }";
        let (index, uri) = fixture(source);
        for needle in ["read(self) -> i64 { self.value", "read()"] {
            let cursor = position(source, needle);
            assert!(index.prepare_rename(&uri, cursor).is_none());
            assert!(index.rename(&uri, cursor, "count", false).is_err());
        }
    }

    #[test]
    fn macros_and_shorthand_tokens_refuse_edits_without_source_rewrites() {
        let source = "macro_rules! number { () => { 1 }; }\ndef main() -> i64 { let value = number!(); value }";
        let index = WorkspaceIndex::default();
        let uri = Url::parse("file:///macro.sg").unwrap();
        index.open(uri.clone(), 1, source.into(), &IndexCancellation::default());
        assert!(index
            .prepare_rename(&uri, position(source, "value ="))
            .is_none());
        assert!(index
            .rename(&uri, position(source, "value ="), "answer", false)
            .is_err());
        let source = "struct A { value: i64 }\ndef make(value: i64) -> A { A { value } }";
        let (index, uri) = fixture(source);
        assert!(index
            .rename(&uri, position(source, "value: i64)"), "answer", false)
            .is_err());
    }
}
