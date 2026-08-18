//! Shared Sengoo source formatter.

use miette::{IntoDiagnostic, Result};

use sengoo_compiler::ast::{
    AssociatedTypeDecl, Class, ClassMember, Const, Decl, DeclKind, Enum, EnumVariant, ExprKind,
    ExternBlock, ExternFunction, ExternItem, ExternStatic, FieldName, FieldValue, Function, Impl,
    Import, ImportKind, Module, Param, Path as AstPath, Program, SelfParam, Static, Struct,
    StructField, Trait, TraitBound, TraitItem, Type, TypeAlias, TypeKind, TypeParam, VariantField,
    Visibility,
};
use sengoo_compiler::Parser as SgParser;

mod expressions;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatOptions {
    pub max_width: usize,
    pub indent_width: usize,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            max_width: 100,
            indent_width: 4,
        }
    }
}

struct Formatter {
    options: FormatOptions,
}

impl Formatter {
    fn new(options: FormatOptions) -> Self {
        Self { options }
    }

    fn format_program(&self, program: &Program) -> String {
        program
            .decls
            .iter()
            .map(|d| self.format_decl(d, 0))
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    fn format_decl(&self, decl: &Decl, indent: usize) -> String {
        match &decl.kind {
            DeclKind::Function(func) => self.format_function(func, indent),
            DeclKind::Struct(s) => self.format_struct_decl(s, indent),
            DeclKind::Enum(e) => self.format_enum_decl(e, indent),
            DeclKind::Class(c) => self.format_class_decl(c, indent),
            DeclKind::Trait(t) => self.format_trait_decl(t, indent),
            DeclKind::Impl(i) => self.format_impl_decl(i, indent),
            DeclKind::ExternBlock(e) => self.format_extern_block_decl(e, indent),
            DeclKind::Module(m) => self.format_module_decl(m, indent),
            DeclKind::TypeAlias(t) => self.format_type_alias_decl(t, indent),
            DeclKind::Const(c) => self.format_const_decl(c, indent),
            DeclKind::Static(s) => self.format_static_decl(s, indent),
            DeclKind::Import(i) => self.format_import_decl(i, indent),
        }
    }

    fn format_function(&self, func: &Function, indent: usize) -> String {
        let mut lines = Vec::new();
        if func.no_mangle {
            lines.push(format!("{}#[no_mangle]", self.pad(indent)));
        }
        if let Some(export_name) = &func.export_name {
            lines.push(format!(
                "{}#[export_name = \"{}\"]",
                self.pad(indent),
                escape_string(export_name)
            ));
        }

        let mut head = String::new();
        head.push_str(Self::visibility_prefix(func.vis));
        if func.is_async {
            head.push_str("async ");
        }
        if func.is_unsafe {
            head.push_str("unsafe ");
        }
        if let Some(abi) = &func.abi {
            head.push_str("extern \"");
            head.push_str(&escape_string(abi));
            head.push_str("\" ");
        }
        head.push_str("def ");
        head.push_str(&func.name.name);
        head.push_str(&self.format_type_params(&func.type_params));
        head.push('(');
        let mut params = Vec::new();
        if let Some(self_param) = func.self_param {
            params.push(Self::format_self_param(self_param).to_string());
        }
        params.extend(func.params.iter().map(|p| self.format_param(p)));
        head.push_str(&params.join(", "));
        head.push(')');
        if let Some(ret) = &func.return_type {
            head.push_str(" -> ");
            head.push_str(&self.format_type(ret));
        }
        if let Some(pre) = &func.precondition {
            head.push_str(" requires ");
            head.push_str(&self.format_expr(pre));
        }
        if let Some(post) = &func.postcondition {
            head.push_str(" ensures ");
            head.push_str(&self.format_expr(post));
        }

        lines.push(format!(
            "{}{} {}",
            self.pad(indent),
            head,
            self.format_block(&func.body, indent)
        ));
        lines.join("\n")
    }

    fn format_struct_decl(&self, s: &Struct, indent: usize) -> String {
        let head = format!(
            "{}{}struct {}{}",
            self.pad(indent),
            Self::visibility_prefix(s.vis),
            s.name.name,
            self.format_type_params(&s.type_params)
        );
        if s.fields.is_empty() {
            return format!("{};", head);
        }
        let all_named = s.fields.iter().all(|f| f.name.is_some());
        if all_named {
            let mut lines = vec![format!("{} {{", head)];
            for field in &s.fields {
                lines.push(format!(
                    "{}{},",
                    self.pad(indent + 1),
                    self.format_struct_field(field)
                ));
            }
            lines.push(format!("{}}}", self.pad(indent)));
            return lines.join("\n");
        }
        let fields = s
            .fields
            .iter()
            .map(|f| self.format_struct_field(f))
            .collect::<Vec<_>>()
            .join(", ");
        format!("{}({});", head, fields)
    }

    fn format_enum_decl(&self, e: &Enum, indent: usize) -> String {
        let mut lines = vec![format!(
            "{}{}enum {}{} {{",
            self.pad(indent),
            Self::visibility_prefix(e.vis),
            e.name.name,
            self.format_type_params(&e.type_params)
        )];
        for v in &e.variants {
            lines.push(format!(
                "{}{}",
                self.pad(indent + 1),
                self.format_enum_variant(v)
            ));
        }
        lines.push(format!("{}}}", self.pad(indent)));
        lines.join("\n")
    }

    fn format_type_alias_decl(&self, t: &TypeAlias, indent: usize) -> String {
        format!(
            "{}{}type {}{} = {};",
            self.pad(indent),
            Self::visibility_prefix(t.vis),
            t.name.name,
            self.format_type_params(&t.type_params),
            self.format_type(&t.ty)
        )
    }

    fn format_associated_type_decl(&self, item: &AssociatedTypeDecl, indent: usize) -> String {
        format!("{}type {};", self.pad(indent), item.name.name)
    }

    fn format_const_decl(&self, c: &Const, indent: usize) -> String {
        format!(
            "{}{}const {}: {} == {};",
            self.pad(indent),
            Self::visibility_prefix(c.vis),
            c.name.name,
            self.format_type(&c.ty),
            self.format_expr(&c.value)
        )
    }

    fn format_static_decl(&self, s: &Static, indent: usize) -> String {
        let mut out = format!(
            "{}{}static ",
            self.pad(indent),
            Self::visibility_prefix(s.vis)
        );
        if s.is_mut {
            out.push_str("mut ");
        }
        out.push_str(&format!(
            "{}: {} == {};",
            s.name.name,
            self.format_type(&s.ty),
            self.format_expr(&s.value)
        ));
        out
    }

    fn format_import_decl(&self, i: &Import, indent: usize) -> String {
        let path = self.format_path(&i.path);
        match &i.kind {
            ImportKind::Simple => {
                if let Some(alias) = &i.alias {
                    format!("{}import {} as {};", self.pad(indent), path, alias.name)
                } else {
                    format!("{}import {};", self.pad(indent), path)
                }
            }
            ImportKind::Selective(items) => format!(
                "{}import {} {{ {} }};",
                self.pad(indent),
                path,
                items
                    .iter()
                    .map(|x| x.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            ImportKind::Wildcard => format!("{}import {} * from;", self.pad(indent), path),
        }
    }

    fn format_class_decl(&self, class_decl: &Class, indent: usize) -> String {
        let mut head = format!(
            "{}{}class {}{}",
            self.pad(indent),
            Self::visibility_prefix(class_decl.vis),
            class_decl.name.name,
            self.format_type_params(&class_decl.type_params)
        );
        if let Some(parent) = &class_decl.extends {
            head.push_str(": ");
            head.push_str(&self.format_path(parent));
        }
        if !class_decl.implements.is_empty() {
            head.push_str(" implements ");
            head.push_str(
                &class_decl
                    .implements
                    .iter()
                    .map(|bound| self.format_trait_bound(bound))
                    .collect::<Vec<_>>()
                    .join(", "),
            );
        }

        let mut lines = vec![format!("{} {{", head)];
        for member in &class_decl.members {
            match member {
                ClassMember::Field(field) => {
                    let field_name = field
                        .name
                        .as_ref()
                        .map(|name| name.name.as_str())
                        .unwrap_or("_");
                    lines.push(format!(
                        "{}{}: {};",
                        self.pad(indent + 1),
                        field_name,
                        self.format_type(&field.ty)
                    ));
                }
                ClassMember::Method(method) => lines.push(self.format_function(method, indent + 1)),
            }
        }
        lines.push(format!("{}}}", self.pad(indent)));
        lines.join("\n")
    }

    fn format_trait_decl(&self, trait_decl: &Trait, indent: usize) -> String {
        let mut head = format!(
            "{}{}trait {}{}",
            self.pad(indent),
            Self::visibility_prefix(trait_decl.vis),
            trait_decl.name.name,
            self.format_type_params(&trait_decl.type_params)
        );
        if !trait_decl.bounds.is_empty() {
            head.push_str(": ");
            head.push_str(
                &trait_decl
                    .bounds
                    .iter()
                    .map(|bound| self.format_trait_bound(bound))
                    .collect::<Vec<_>>()
                    .join(", "),
            );
        }

        let mut lines = vec![format!("{} {{", head)];
        for item in &trait_decl.items {
            match item {
                TraitItem::Function(func) => lines.push(self.format_function(func, indent + 1)),
                TraitItem::Const(const_decl) => {
                    lines.push(self.format_const_decl(const_decl, indent + 1))
                }
                TraitItem::Type(associated_type) => {
                    lines.push(self.format_associated_type_decl(associated_type, indent + 1))
                }
            }
        }
        lines.push(format!("{}}}", self.pad(indent)));
        lines.join("\n")
    }

    fn format_impl_decl(&self, impl_decl: &Impl, indent: usize) -> String {
        let mut head = format!(
            "{}impl{} ",
            self.pad(indent),
            self.format_type_params(&impl_decl.type_params)
        );
        if let Some(trait_path) = &impl_decl.trait_path {
            head.push_str(&self.format_path(trait_path));
            head.push_str(" for ");
        }
        head.push_str(&self.format_type(&impl_decl.target_type));

        let mut lines = vec![format!("{} {{", head)];
        for associated_type in &impl_decl.associated_types {
            lines.push(self.format_type_alias_decl(associated_type, indent + 1));
        }
        for method in &impl_decl.items {
            lines.push(self.format_function(method, indent + 1));
        }
        lines.push(format!("{}}}", self.pad(indent)));
        lines.join("\n")
    }

    fn format_extern_block_decl(&self, extern_block: &ExternBlock, indent: usize) -> String {
        let mut lines = Vec::new();
        if let Some(link_name) = &extern_block.link_name {
            lines.push(format!(
                "{}#[link(name = \"{}\")]",
                self.pad(indent),
                escape_string(link_name)
            ));
        }

        lines.push(format!(
            "{}extern \"{}\" {{",
            self.pad(indent),
            escape_string(&extern_block.abi)
        ));

        for item in &extern_block.items {
            match item {
                ExternItem::Function(func) => lines.push(format!(
                    "{}{}",
                    self.pad(indent + 1),
                    self.format_extern_function_item(func)
                )),
                ExternItem::Static(static_decl) => lines.push(format!(
                    "{}{}",
                    self.pad(indent + 1),
                    self.format_extern_static_item(static_decl)
                )),
            }
        }

        lines.push(format!("{}}}", self.pad(indent)));
        lines.join("\n")
    }

    fn format_module_decl(&self, module_decl: &Module, indent: usize) -> String {
        let mut lines = vec![format!(
            "{}{}mod {} {{",
            self.pad(indent),
            Self::visibility_prefix(module_decl.vis),
            module_decl.name.name
        )];

        for (index, item) in module_decl.items.iter().enumerate() {
            lines.push(self.format_decl(item, indent + 1));
            if index + 1 < module_decl.items.len() {
                lines.push(String::new());
            }
        }

        lines.push(format!("{}}}", self.pad(indent)));
        lines.join("\n")
    }

    fn format_extern_function_item(&self, func: &ExternFunction) -> String {
        let mut out = String::new();
        out.push_str(Self::visibility_prefix(func.vis));
        if func.is_unsafe {
            out.push_str("unsafe ");
        }
        out.push_str("fn ");
        out.push_str(&func.name.name);
        out.push('(');
        out.push_str(
            &func
                .params
                .iter()
                .map(|param| self.format_param(param))
                .collect::<Vec<_>>()
                .join(", "),
        );
        out.push(')');
        if let Some(return_type) = &func.return_type {
            out.push_str(" -> ");
            out.push_str(&self.format_type(return_type));
        }
        out.push(';');
        out
    }

    fn format_extern_static_item(&self, static_decl: &ExternStatic) -> String {
        let mut out = String::new();
        out.push_str(Self::visibility_prefix(static_decl.vis));
        out.push_str("static ");
        if static_decl.is_mut {
            out.push_str("mut ");
        }
        out.push_str(&static_decl.name.name);
        out.push_str(": ");
        out.push_str(&self.format_type(&static_decl.ty));
        out.push(';');
        out
    }
    fn format_field_value(&self, field: &FieldValue) -> String {
        if let (FieldName::Ident(name), ExprKind::Ident(value)) = (&field.name, &field.value.kind) {
            if name.name == value.name {
                return name.name.clone();
            }
        }

        format!(
            "{}: {}",
            self.format_field_name(&field.name),
            self.format_expr(&field.value)
        )
    }

    fn format_field_name(&self, name: &FieldName) -> String {
        match name {
            FieldName::Ident(ident) => ident.name.clone(),
            FieldName::String(value) => format!("\"{}\"", escape_string(value)),
        }
    }

    fn format_type(&self, ty: &Type) -> String {
        match &ty.kind {
            TypeKind::SelfType => "Self".to_string(),
            TypeKind::Path(path) => self.format_path(path),
            TypeKind::PathWithArgs { path, args } => format!(
                "{}<{}>",
                self.format_path(path),
                args.iter()
                    .map(|a| self.format_type(a))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            TypeKind::Tuple(types) => format!(
                "({})",
                types
                    .iter()
                    .map(|t| self.format_type(t))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            TypeKind::Array(elem, len) => format!("[{}; {}]", self.format_type(elem), len),
            TypeKind::Slice(elem) => format!("[{}]", self.format_type(elem)),
            TypeKind::Ptr { base, is_mut } => {
                if *is_mut {
                    format!("*mut {}", self.format_type(base))
                } else {
                    format!("*const {}", self.format_type(base))
                }
            }
            TypeKind::Ref { base, is_mut } => {
                if *is_mut {
                    format!("&mut {}", self.format_type(base))
                } else {
                    format!("&{}", self.format_type(base))
                }
            }
            TypeKind::Fn { params, ret } => {
                let p = params
                    .iter()
                    .map(|t| self.format_type(t))
                    .collect::<Vec<_>>()
                    .join(", ");
                if let Some(ret) = ret {
                    format!("fn({}) -> {}", p, self.format_type(ret))
                } else {
                    format!("fn({})", p)
                }
            }
            TypeKind::Never => "!".to_string(),
            TypeKind::Infer => "_".to_string(),
            TypeKind::Dyn(bounds) => format!(
                "dyn {}",
                bounds
                    .iter()
                    .map(|b| self.format_trait_bound(b))
                    .collect::<Vec<_>>()
                    .join(" + ")
            ),
            TypeKind::ImplTrait(bounds) => format!(
                "impl {}",
                bounds
                    .iter()
                    .map(|b| self.format_trait_bound(b))
                    .collect::<Vec<_>>()
                    .join(" + ")
            ),
        }
    }

    fn format_trait_bound(&self, bound: &TraitBound) -> String {
        if bound.params.is_empty() {
            self.format_path(&bound.path)
        } else {
            format!(
                "{}<{}>",
                self.format_path(&bound.path),
                bound
                    .params
                    .iter()
                    .map(|p| self.format_type(p))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
    }

    fn format_path(&self, path: &AstPath) -> String {
        path.segments
            .iter()
            .map(|seg| seg.name.as_str())
            .collect::<Vec<_>>()
            .join("::")
    }

    fn format_type_params(&self, params: &[TypeParam]) -> String {
        if params.is_empty() {
            String::new()
        } else {
            format!(
                "<{}>",
                params
                    .iter()
                    .map(|p| self.format_type_param(p))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
    }

    fn format_type_param(&self, param: &TypeParam) -> String {
        let mut s = param.name.name.clone();
        if !param.bounds.is_empty() {
            s.push_str(": ");
            s.push_str(
                &param
                    .bounds
                    .iter()
                    .map(|b| self.format_trait_bound(b))
                    .collect::<Vec<_>>()
                    .join(" + "),
            );
        }
        if let Some(default) = &param.default {
            s.push_str(" = ");
            s.push_str(&self.format_type(default));
        }
        s
    }

    fn format_param(&self, param: &Param) -> String {
        if param.is_mut {
            format!("mut {}: {}", param.name.name, self.format_type(&param.ty))
        } else {
            format!("{}: {}", param.name.name, self.format_type(&param.ty))
        }
    }

    fn format_struct_field(&self, field: &StructField) -> String {
        match &field.name {
            Some(name) => format!(
                "{}{}: {}",
                Self::visibility_prefix(field.vis),
                name.name,
                self.format_type(&field.ty)
            ),
            None => format!(
                "{}{}",
                Self::visibility_prefix(field.vis),
                self.format_type(&field.ty)
            ),
        }
    }

    fn format_enum_variant(&self, variant: &EnumVariant) -> String {
        let mut s = variant.name.name.clone();
        if !variant.fields.is_empty() {
            let all_named = variant
                .fields
                .iter()
                .all(|f| matches!(f, VariantField::Named(_, _)));
            if all_named {
                let fields = variant
                    .fields
                    .iter()
                    .map(|f| match f {
                        VariantField::Named(name, ty) => {
                            format!("{}: {}", name.name, self.format_type(ty))
                        }
                        VariantField::Unnamed(_) => unreachable!(),
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                s.push_str(&format!(" {{ {} }}", fields));
            } else {
                let fields = variant
                    .fields
                    .iter()
                    .map(|f| match f {
                        VariantField::Named(_, ty) | VariantField::Unnamed(ty) => {
                            self.format_type(ty)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                s.push_str(&format!("({})", fields));
            }
        }
        if let Some(discriminant) = &variant.discriminant {
            s.push_str(" == ");
            s.push_str(&self.format_expr(discriminant));
        }
        s
    }

    fn format_self_param(self_param: SelfParam) -> &'static str {
        match self_param {
            SelfParam::Borrowed => "&self",
            SelfParam::BorrowedMut => "&mut self",
            SelfParam::Owned => "self",
            SelfParam::OwnedMut => "mut self",
        }
    }

    fn visibility_prefix(vis: Visibility) -> &'static str {
        if vis.is_public() {
            "pub "
        } else {
            ""
        }
    }

    fn pad(&self, indent: usize) -> String {
        " ".repeat(indent * self.options.indent_width)
    }
}

fn escape_string(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out
}

fn escape_char(value: char) -> String {
    match value {
        '\\' => "\\\\".to_string(),
        '\'' => "\\'".to_string(),
        '\n' => "\\n".to_string(),
        '\r' => "\\r".to_string(),
        '\t' => "\\t".to_string(),
        other => other.to_string(),
    }
}

/// Comment captured from original source so AST round-trips can reinject it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct SourceComment {
    /// Full comment text including `//` / `/*` / `*/` delimiters (no trailing `\n`).
    text: String,
    /// Non-comment code on the same line before a trailing `//` comment (trimmed).
    /// Empty when the comment occupies the whole line (or is a block comment).
    leading_code: String,
    /// Next non-empty non-comment source line (trimmed) after a full-line/block
    /// comment. Used as an insertion anchor in the formatted output.
    following_code: Option<String>,
}

/// Extract line and block comments while respecting string/char literals.
fn extract_source_comments(source: &str) -> Vec<SourceComment> {
    let bytes = source.as_bytes();
    let mut comments = Vec::new();
    let mut i = 0usize;
    let mut line_start = 0usize;
    // Stack of (comment_start, line_start_at_comment)
    while i < bytes.len() {
        let ch = bytes[i] as char;
        // Strings (regular + multiline """ ... """)
        if ch == '"' {
            if bytes.get(i..i + 3) == Some(b"\"\"\"") {
                i += 3;
                while i + 2 < bytes.len() && bytes.get(i..i + 3) != Some(b"\"\"\"") {
                    i += 1;
                }
                i = (i + 3).min(bytes.len());
                continue;
            }
            i += 1;
            while i < bytes.len() {
                match bytes[i] as char {
                    '\\' if i + 1 < bytes.len() => i += 2,
                    '"' => {
                        i += 1;
                        break;
                    }
                    _ => i += 1,
                }
            }
            continue;
        }
        // Char literals
        if ch == '\'' {
            i += 1;
            while i < bytes.len() {
                match bytes[i] as char {
                    '\\' if i + 1 < bytes.len() => i += 2,
                    '\'' => {
                        i += 1;
                        break;
                    }
                    _ => i += 1,
                }
            }
            continue;
        }
        // Line comment
        if ch == '/' && bytes.get(i + 1) == Some(&b'/') {
            let comment_start = i;
            let this_line_start = line_start;
            i += 2;
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            let text = source[comment_start..i].to_string();
            let leading =
                normalize_code_anchor(source[this_line_start..comment_start].trim());
            let following = if leading.is_empty() {
                next_code_line(source, i)
            } else {
                None
            };
            comments.push(SourceComment {
                text,
                leading_code: leading,
                following_code: following,
            });
            // leave i at newline (or EOF) so outer loop advances line_start
            if i < bytes.len() && bytes[i] == b'\n' {
                i += 1;
                line_start = i;
            }
            continue;
        }
        // Block comment
        if ch == '/' && bytes.get(i + 1) == Some(&b'*') {
            let comment_start = i;
            let this_line_start = line_start;
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                if bytes[i] == b'\n' {
                    line_start = i + 1;
                }
                i += 1;
            }
            if i + 1 < bytes.len() {
                i += 2; // consume */
            }
            let text = source[comment_start..i].to_string();
            let leading =
                normalize_code_anchor(source[this_line_start..comment_start].trim());
            let following = if leading.is_empty() {
                next_code_line(source, i)
            } else {
                None
            };
            comments.push(SourceComment {
                text,
                leading_code: leading,
                following_code: following,
            });
            continue;
        }
        if ch == '\n' {
            i += 1;
            line_start = i;
            continue;
        }
        i += 1;
    }
    comments
}

fn normalize_code_anchor(code: &str) -> String {
    code.trim()
        .trim_end_matches(';')
        .trim()
        .to_string()
}

fn next_code_line(source: &str, from: usize) -> Option<String> {
    let rest = &source[from.min(source.len())..];
    for line in rest.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with("//") || trimmed.starts_with("/*") {
            continue;
        }
        // Strip trailing line comment for a stable anchor.
        let code = match trimmed.find("//") {
            Some(idx) => trimmed[..idx].trim(),
            None => trimmed,
        };
        let anchor = normalize_code_anchor(code);
        if !anchor.is_empty() {
            return Some(anchor);
        }
    }
    None
}

fn reinject_source_comments(formatted: &str, comments: &[SourceComment]) -> String {
    if comments.is_empty() {
        return formatted.to_string();
    }
    let mut lines: Vec<String> = formatted.lines().map(str::to_string).collect();
    // Track which formatted line indices already received a trailing comment.
    let mut used_trailing = vec![false; lines.len()];

    for comment in comments {
        if !comment.leading_code.is_empty() {
            // Trailing line comment: attach to the first matching code line.
            if let Some(idx) = lines.iter().enumerate().position(|(idx, line)| {
                !used_trailing[idx]
                    && normalize_code_anchor(line.trim()) == comment.leading_code
            }) {
                if !lines[idx].contains(&comment.text) {
                    // Preserve single-space before // comments.
                    let base = lines[idx].trim_end().to_string();
                    lines[idx] = format!("{base} {}", comment.text.trim_start());
                    used_trailing[idx] = true;
                }
                continue;
            }
            // Fallback: append as a free-standing line at end if no anchor found.
            lines.push(comment.text.clone());
            continue;
        }

        // Full-line or block comment: insert before the following code anchor.
        if let Some(anchor) = &comment.following_code {
            if let Some(idx) = lines.iter().position(|line| {
                let key = normalize_code_anchor(line.trim());
                key == *anchor || key.starts_with(anchor.as_str())
            }) {
                // Avoid duplicating if already present immediately above.
                let already = idx > 0 && lines[idx - 1].trim() == comment.text.trim();
                if !already {
                    lines.insert(idx, comment.text.clone());
                    used_trailing.insert(idx, false);
                }
                continue;
            }
        }
        // Leading / trailing file comments with no anchor: keep at top then bottom.
        if comment.following_code.is_none() {
            // Prefer end of file for trailing orphan comments.
            if !lines
                .last()
                .is_some_and(|l| l.trim() == comment.text.trim())
            {
                lines.push(comment.text.clone());
            }
        } else {
            lines.insert(0, comment.text.clone());
            used_trailing.insert(0, false);
        }
    }

    let mut out = lines.join("\n");
    if formatted.ends_with('\n') && !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

pub fn format_source(source: &str, options: &FormatOptions) -> Result<String> {
    let comments = extract_source_comments(source);
    let program = SgParser::parse(source).into_diagnostic()?;
    let formatter = Formatter::new(options.clone());
    let formatted = formatter.format_program(&program);
    let with_comments = reinject_source_comments(&formatted, &comments);

    // Safety net: never emit syntactically invalid source.
    SgParser::parse(&with_comments).into_diagnostic()?;
    Ok(with_comments)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};

    fn format_test_source(src: &str, options: FormatOptions) -> String {
        format_source(src, &options).expect("format source")
    }

    fn collect_sg_files(root: &Path) -> Vec<PathBuf> {
        let mut files = Vec::new();
        let mut stack = vec![root.to_path_buf()];

        while let Some(dir) = stack.pop() {
            let entries = match fs::read_dir(&dir) {
                Ok(entries) => entries,
                Err(_) => continue,
            };

            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().and_then(|ext| ext.to_str()) == Some("sg") {
                    files.push(path);
                }
            }
        }

        files.sort();
        files
    }

    #[test]
    fn formatter_is_idempotent() {
        let src = "def main()->i64{\nlet x=1+2\nif x>2 {x}else{0}\nx\n}";
        let first = format_test_source(src, FormatOptions::default());
        let second = format_test_source(&first, FormatOptions::default());
        assert_eq!(first, second);
    }

    #[test]
    fn preserves_line_and_block_comments_through_format() {
        // RED regression: AST round-trip previously dropped all comments because
        // the lexer skips them (TokenKind logos skip) and the formatter rebuilds
        // source from a comment-free AST.
        let src = r#"// leading ownership note
def main() -> i64 {
    // body: by-value String params skip auto-Drop
    return 0  // trailing
}
/* block: owns request end-to-end on unsupported path */
"#;
        let formatted = format_test_source(src, FormatOptions::default());
        assert!(
            formatted.contains("// leading ownership note"),
            "leading comment lost:\n{formatted}"
        );
        assert!(
            formatted.contains("// body: by-value String params skip auto-Drop"),
            "body comment lost:\n{formatted}"
        );
        assert!(
            formatted.contains("// trailing"),
            "trailing comment lost:\n{formatted}"
        );
        assert!(
            formatted.contains("/* block: owns request end-to-end on unsupported path */"),
            "block comment lost:\n{formatted}"
        );
        // Idempotent with comments present.
        let second = format_test_source(&formatted, FormatOptions::default());
        assert_eq!(formatted, second);
    }

    #[test]
    fn does_not_treat_comment_markers_inside_strings_as_comments() {
        let src = r#"def main() -> i64 {
    let s = "// not a comment";
    let t = "/* also not */";
    0
}
"#;
        let formatted = format_test_source(src, FormatOptions::default());
        assert!(formatted.contains("\"// not a comment\"") || formatted.contains("// not a comment"));
        // No free-standing comment lines invented from string contents.
        assert!(
            !formatted.lines().any(|l| l.trim() == "// not a comment"),
            "string content must not become a real comment:\n{formatted}"
        );
    }

    #[test]
    fn formats_unsigned_literals_without_losing_their_type() {
        let src = "def main() -> u64 { 7u64 }";
        let first = format_test_source(src, FormatOptions::default());
        SgParser::parse(&first).expect("formatted unsigned literal must parse");
        let second = format_test_source(&first, FormatOptions::default());
        assert_eq!(first, second);
        assert!(first.contains("7 as u64") || first.contains("7u64"));
    }

    #[test]
    fn preserves_mutable_local_bindings() {
        let src = "def main()->i64{let mut value=1;value=value+1;value}";
        let formatted = format_test_source(src, FormatOptions::default());
        assert!(formatted.contains("let mut value = 1;"));
    }

    #[test]
    fn formats_keyword_logical_operators_idempotently() {
        let src = "def main() -> i64 {\nlet a = true;\nlet b = false;\nlet ok = a && !b || b;\nif ok { 0 } else { 1 }\n}";
        let first = format_test_source(src, FormatOptions::default());
        let second = format_test_source(&first, FormatOptions::default());
        assert_eq!(first, second);
        assert!(first.contains("a and not b or b"));
    }

    #[test]
    fn formats_decls_beyond_functions() {
        let src = "struct Pair(i64, i64);\nenum E { A, B(i64) }\ntype Id = i64;\nimport std::io;";
        let first = format_test_source(src, FormatOptions::default());
        let second = format_test_source(&first, FormatOptions::default());
        assert_eq!(first, second);
        assert!(first.contains("struct Pair"));
        assert!(first.contains("enum E"));
        assert!(first.contains("type Id = i64;"));
    }

    #[test]
    fn import_forms_fixture_is_parser_accepted_and_formatter_canonical() {
        let source = include_str!("../tests/fixtures/import_forms.sg");
        let formatted = format_test_source(source, FormatOptions::default());
        assert_eq!(
            formatted,
            include_str!("../tests/fixtures/import_forms.formatted.sg").trim_end()
        );
        SgParser::parse(&formatted).expect("canonical import fixture must parse");
    }

    #[test]
    fn formats_associated_types_and_self_type_without_losing_semantics() {
        let src = r#"
trait Iterator {
    type Item;
    def duplicate(self) -> Self { self }
}

struct Counter {
    value: i64,
}

impl Iterator for Counter {
    type Item = i64;
    def duplicate(self) -> Self { self }
}
"#;
        let first = format_test_source(src, FormatOptions::default());
        let second = format_test_source(&first, FormatOptions::default());

        assert_eq!(first, second);
        assert!(first.contains("type Item;"));
        assert!(first.contains("type Item = i64;"));
        assert!(first.contains("-> Self"));
    }

    #[test]
    fn formats_match_struct_lambda_and_patterns() {
        let src = "struct Point { x: i64, y: i64 }\n\ndef main() -> i64 {\n    let base = Point { x: 0, y: 0 };\n    let x = 1;\n    let y2 = 2;\n    let point = Point { x, y: y2, ..base };\n    let add = |lhs, rhs| lhs + rhs;\n    let values = [1, 2, 3];\n    let picked = match point {\n        Point { x, y: yv, .. } if x > 0 => add(x, yv),\n        _ => match values {\n            [head, ..tail] => head,\n            _ => 0,\n        },\n    };\n    let bucket = match x {\n        1..3 => 1,\n        ..10 => 2,\n        _ => 3,\n    };\n    picked + bucket\n}";
        let first = format_test_source(src, FormatOptions::default());
        let second = format_test_source(&first, FormatOptions::default());
        assert_eq!(first, second);
        assert!(first.contains("Point { x, y: y2, ..base }"));
        assert!(first.contains("|lhs, rhs| lhs + rhs"));
        assert!(first.contains("match point {"));
        assert!(first.contains("[head, ..tail]"));
        assert!(first.contains("1..3"));
        assert!(first.contains("..10"));
    }
    fn width_options(max_width: usize) -> FormatOptions {
        FormatOptions {
            max_width,
            ..FormatOptions::default()
        }
    }

    fn longest_line(source: &str) -> usize {
        source
            .lines()
            .map(|line| line.chars().count())
            .max()
            .unwrap_or(0)
    }

    #[test]
    fn long_conditional_body_is_rendered_across_lines() {
        let src = "def classify(total: i64, limit: i64) -> i64 {\n    if total > limit { let overflow = total - limit; let penalty = overflow * 3; return penalty + 1; };\n    total\n}";
        let formatted = format_test_source(src, FormatOptions::default());

        assert!(
            formatted.contains("    if total > limit {\n        let overflow = total - limit;\n"),
            "long conditional body was not broken across lines:\n{formatted}"
        );
        assert!(
            longest_line(&formatted) <= 100,
            "formatted output still exceeds the default width:\n{formatted}"
        );
        assert_eq!(
            formatted,
            format_test_source(&formatted, FormatOptions::default())
        );
    }

    #[test]
    fn short_blocks_stay_inline() {
        let src = "def small(n: i64) -> i64 {\n    if n < 0 { return 0; };\n    let mut i = 0;\n    while i < n { i = i + 1; };\n    n\n}";
        let formatted = format_test_source(src, FormatOptions::default());

        assert!(formatted.contains("    if n < 0 { return 0; };"));
        assert!(formatted.contains("    while i < n { i = i + 1; };"));
    }

    #[test]
    fn max_width_moves_where_blocks_break() {
        let src = "def tiers(total: i64, limit: i64) -> i64 {\n    if total > limit { let a = total - limit; return a * 3; };\n    if total < 0 { return 0; };\n    total\n}";

        // 62 characters wide with its indent, so it survives 100 and breaks at 60.
        let wide = format_test_source(src, width_options(100));
        assert!(wide.contains("    if total > limit { let a = total - limit; return a * 3; };"));
        assert!(wide.contains("    if total < 0 { return 0; };"));

        let narrow = format_test_source(src, width_options(60));
        assert!(
            narrow.contains("    if total > limit {\n        let a = total - limit;\n"),
            "width 60 did not break the wider block:\n{narrow}"
        );
        // The 31-character block still fits at 60 and only breaks at 25.
        assert!(narrow.contains("    if total < 0 { return 0; };"));

        let narrowest = format_test_source(src, width_options(25));
        assert!(
            narrowest.contains("    if total < 0 {\n        return 0;\n    };"),
            "width 25 did not break the narrower block:\n{narrowest}"
        );

        assert_ne!(wide, narrow);
        assert_ne!(narrow, narrowest);
    }

    #[test]
    fn every_block_form_breaks_on_width() {
        let cases = [
            (
                "if",
                "if total > limit { let a = total * 2; let b = a + limit; acc = b; };",
                "if total > limit {",
            ),
            (
                "while",
                "while acc < total { let a = acc * 7; let b = a + limit; acc = b; };",
                "while acc < total {",
            ),
            (
                "for",
                "for step in 0..3 { let a = step * limit; let b = a + 17; acc = acc + b; };",
                "for step in 0..3 {",
            ),
            (
                "loop",
                "loop { let a = acc * 2; let b = a - limit; acc = b; break acc; };",
                "loop {",
            ),
            (
                "async",
                "let d = async { let a = acc + 1; let b = a * 2; b };",
                "let d = async {",
            ),
            (
                "parallel",
                "let p = parallel { let a = acc + 3; let b = a * 4; b };",
                "let p = parallel {",
            ),
            (
                "try",
                "let t = try { let a = acc + 5; let b = a * 6; b };",
                "let t = try {",
            ),
            (
                "block",
                "{ let a = acc + 7; let b = a * 8; acc = b; };",
                "{",
            ),
        ];

        for (label, body, opener) in cases {
            let src =
                format!("def probe(total: i64, limit: i64) -> i64 {{\n    let mut acc = 0;\n    {body}\n    acc\n}}");

            let inline = format_test_source(&src, width_options(200));
            assert!(
                !inline.contains(&format!("    {opener}\n")),
                "{label} block should stay inline at width 200:\n{inline}"
            );

            let broken = format_test_source(&src, width_options(30));
            assert!(
                broken.contains(&format!("    {opener}\n")),
                "{label} block was not broken at width 30:\n{broken}"
            );
            assert_eq!(broken, format_test_source(&broken, width_options(30)));
        }
    }

    #[test]
    fn match_arms_break_one_per_line_and_carry_block_bodies() {
        let src = "def pick(total: i64, limit: i64) -> i64 {\n    let acc = 1;\n    let tag = match total { 0 => 0, 1 => { let doubled = limit * 2; let raised = doubled + acc; raised }, _ => acc };\n    tag\n}";

        let broken = format_test_source(src, width_options(60));
        assert!(broken.contains("    let tag = match total {\n"));
        assert!(broken.contains("\n        0 => 0,\n"));
        assert!(
            broken.contains("        1 => {\n            let doubled = limit * 2;\n"),
            "match arm body was not broken:\n{broken}"
        );
        assert!(broken.contains("\n        _ => acc,\n"));
        assert_eq!(broken, format_test_source(&broken, width_options(60)));
    }

    #[test]
    fn else_if_chains_break_as_a_whole() {
        let src = "def chain(total: i64, limit: i64) -> i64 {\n    let mut acc = 0;\n    if total > limit { let inner = total - limit; acc = acc + inner; } else if total < 0 { acc = 0 - total; } else { acc = total; };\n    acc\n}";
        let formatted = format_test_source(src, FormatOptions::default());

        assert!(formatted.contains("    if total > limit {\n"));
        assert!(formatted.contains("\n    } else if total < 0 {\n"));
        assert!(formatted.contains("\n    } else {\n"));
        assert_eq!(
            formatted,
            format_test_source(&formatted, FormatOptions::default())
        );
    }

    #[test]
    fn nested_blocks_keep_their_indentation() {
        let src = "def nested(total: i64, limit: i64) -> i64 {\n    let mut acc = 0;\n    if total > limit { if total > limit + 10 { let inner = total - limit; acc = acc + inner; } else { acc = acc + 1; }; acc = acc * 2; };\n    acc\n}";
        let formatted = format_test_source(src, FormatOptions::default());

        assert!(formatted.contains("    if total > limit {\n        if total > limit + 10 {\n            let inner = total - limit;\n"));
        assert!(formatted.contains("\n        } else {\n            acc = acc + 1;\n        };\n"));
        assert!(
            longest_line(&formatted) <= 100,
            "nested output still exceeds the default width:\n{formatted}"
        );
    }

    #[test]
    fn multi_line_conditional_source_is_already_formatted() {
        let src = "def classify(total: i64, limit: i64) -> i64 {\n    if total > limit {\n        let overflow = total - limit;\n        let penalty = overflow * 3;\n        return penalty + 1;\n    };\n    total;\n}";
        assert_eq!(format_test_source(src, FormatOptions::default()), src);
    }

    #[test]
    fn empty_blocks_stay_inline_even_when_the_prefix_is_too_wide() {
        let src =
            "def spin(total: i64, limit: i64) -> i64 {\n    while total > limit {};\n    total\n}";
        let formatted = format_test_source(src, width_options(10));

        assert!(
            formatted.contains("    while total > limit {};"),
            "empty block should not be split:\n{formatted}"
        );
    }

    #[test]
    fn bench_samples_are_idempotent() {
        let bench_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("bench");
        assert!(
            bench_root.exists(),
            "bench root not found: {}",
            bench_root.display()
        );

        let files = collect_sg_files(&bench_root);
        assert!(
            !files.is_empty(),
            "no .sg files found in {}",
            bench_root.display()
        );

        for file in files {
            let src = fs::read_to_string(&file).expect("read bench sample");
            let first = format_test_source(&src, FormatOptions::default());
            SgParser::parse(&first).expect("formatted bench sample must parse");
            let second = format_test_source(&first, FormatOptions::default());
            assert_eq!(first, second, "not idempotent: {}", file.display());
        }
    }
}
