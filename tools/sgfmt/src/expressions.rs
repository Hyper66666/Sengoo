use sengoo_compiler::ast::pattern::{Pattern, PatternKind, RangeEnd};
use sengoo_compiler::ast::{Block, Expr, ExprKind, MatchArm, Stmt, StmtKind};
use sengoo_compiler::Literal;

use super::{escape_char, escape_string, Formatter};

impl Formatter {
    pub(super) fn format_block(&self, block: &Block, indent: usize) -> String {
        let mut lines = vec!["{".to_string()];
        for stmt in &block.stmts {
            lines.push(self.format_stmt(stmt, indent + 1));
        }
        lines.push(format!("{}}}", self.pad(indent)));
        lines.join("\n")
    }

    fn format_block_inline(&self, block: &Block) -> String {
        if block.stmts.is_empty() {
            return "{}".to_string();
        }
        let body = block
            .stmts
            .iter()
            .map(|s| self.format_stmt_inline(s))
            .collect::<Vec<_>>()
            .join(" ");
        format!("{{ {} }}", body)
    }

    /// Multi-line rendering used when the inline form would exceed `max_width`.
    ///
    /// An empty block has nothing to spread over lines, so it keeps the inline
    /// form even when its prefix alone is already too wide.
    fn format_block_broken(&self, block: &Block, indent: usize) -> String {
        if block.stmts.is_empty() {
            return "{}".to_string();
        }
        self.format_block(block, indent)
    }

    fn fits(&self, line: &str) -> bool {
        line.chars().count() <= self.options.max_width
    }

    fn format_stmt(&self, stmt: &Stmt, indent: usize) -> String {
        match &stmt.kind {
            StmtKind::Let {
                name,
                ty,
                value,
                is_mut,
            } => {
                let mut s = format!(
                    "{}let {}{}",
                    self.pad(indent),
                    if *is_mut { "mut " } else { "" },
                    name.name
                );
                if let Some(ty) = ty {
                    s.push_str(": ");
                    s.push_str(&self.format_type(ty));
                }
                let Some(value) = value else {
                    s.push(';');
                    return s;
                };
                s.push_str(" = ");
                self.finish_stmt(s, value, indent)
            }
            StmtKind::Const { name, ty, value } => self.finish_stmt(
                format!(
                    "{}const {}: {} = ",
                    self.pad(indent),
                    name.name,
                    self.format_type(ty)
                ),
                value,
                indent,
            ),
            StmtKind::Expr(expr) => self.finish_stmt(self.pad(indent), expr, indent),
            StmtKind::Item(item) => self.format_decl(item, indent),
        }
    }

    /// Renders `head` followed by `expr` and the statement terminator.
    ///
    /// The width budget is measured against the whole line, so the leading
    /// indent, any statement head (`let x = `) and any expression prefix
    /// (`if cond `, `while cond `, ...) all count against `max_width`.
    fn finish_stmt(&self, head: String, expr: &Expr, indent: usize) -> String {
        let inline = format!("{}{};", head, self.format_expr(expr));
        if self.fits(&inline) {
            return inline;
        }
        format!("{}{};", head, self.format_expr_broken(expr, indent))
    }

    /// Re-renders `expr` with its blocks spread across lines.
    ///
    /// Expression forms that carry no block keep their inline rendering: there
    /// is nothing to break, so the line simply stays long.
    fn format_expr_broken(&self, expr: &Expr, indent: usize) -> String {
        match &expr.kind {
            ExprKind::Block(block) => self.format_block_broken(block, indent),
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                let mut s = format!(
                    "if {} {}",
                    self.format_expr(cond),
                    self.format_block_broken(then_branch, indent)
                );
                if let Some(else_branch) = else_branch {
                    s.push_str(" else ");
                    s.push_str(&self.format_expr_broken(else_branch, indent));
                }
                s
            }
            ExprKind::IfLet {
                pattern,
                expr,
                then_branch,
                else_branch,
            } => {
                let mut s = format!(
                    "if let {} = {} {}",
                    self.format_pattern(pattern),
                    self.format_expr(expr),
                    self.format_block_broken(then_branch, indent)
                );
                if let Some(else_branch) = else_branch {
                    s.push_str(" else ");
                    s.push_str(&self.format_expr_broken(else_branch, indent));
                }
                s
            }
            ExprKind::While { cond, body } => format!(
                "while {} {}",
                self.format_expr(cond),
                self.format_block_broken(body, indent)
            ),
            ExprKind::For {
                pattern,
                iter,
                body,
            } => format!(
                "for {} in {} {}",
                self.format_pattern(pattern),
                self.format_expr(iter),
                self.format_block_broken(body, indent)
            ),
            ExprKind::Loop(body) => format!("loop {}", self.format_block_broken(body, indent)),
            ExprKind::Match { scrutinee, arms } => {
                if arms.is_empty() {
                    return format!("match {} {{}}", self.format_expr(scrutinee));
                }
                let mut lines = vec![format!("match {} {{", self.format_expr(scrutinee))];
                for arm in arms {
                    lines.push(format!(
                        "{}{},",
                        self.pad(indent + 1),
                        self.format_match_arm_broken(arm, indent + 1)
                    ));
                }
                lines.push(format!("{}}}", self.pad(indent)));
                lines.join("\n")
            }
            ExprKind::AsyncBlock(block) => {
                format!("async {}", self.format_block_broken(block, indent))
            }
            ExprKind::ParallelBlock(block) => {
                format!("parallel {}", self.format_block_broken(block, indent))
            }
            ExprKind::TryBlock(block) => format!("try {}", self.format_block_broken(block, indent)),
            ExprKind::Return(Some(value)) => {
                format!("return {}", self.format_expr_broken(value, indent))
            }
            ExprKind::Break(Some(value)) => {
                format!("break {}", self.format_expr_broken(value, indent))
            }
            ExprKind::Yield(Some(value)) => {
                format!("yield {}", self.format_expr_broken(value, indent))
            }
            ExprKind::Assign { target, value } => format!(
                "{} = {}",
                self.format_expr(target),
                self.format_expr_broken(value, indent)
            ),
            ExprKind::AssignOp { op, target, value } => format!(
                "{} {} {}",
                self.format_expr(target),
                op.as_str(),
                self.format_expr_broken(value, indent)
            ),
            ExprKind::Lambda { params, body } => format!(
                "|{}| {}",
                params
                    .iter()
                    .map(|param| param.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
                self.format_expr_broken(body, indent)
            ),
            _ => self.format_expr(expr),
        }
    }

    fn format_stmt_inline(&self, stmt: &Stmt) -> String {
        match &stmt.kind {
            StmtKind::Let {
                name,
                ty,
                value,
                is_mut,
            } => {
                let mut s = format!("let {}{}", if *is_mut { "mut " } else { "" }, name.name);
                if let Some(ty) = ty {
                    s.push_str(": ");
                    s.push_str(&self.format_type(ty));
                }
                if let Some(value) = value {
                    s.push_str(" = ");
                    s.push_str(&self.format_expr(value));
                }
                s.push(';');
                s
            }
            StmtKind::Const { name, ty, value } => format!(
                "const {}: {} = {};",
                name.name,
                self.format_type(ty),
                self.format_expr(value)
            ),
            StmtKind::Expr(expr) => format!("{};", self.format_expr(expr)),
            StmtKind::Item(_) => "/* item */".to_string(),
        }
    }

    pub(super) fn format_expr(&self, expr: &Expr) -> String {
        match &expr.kind {
            ExprKind::Literal(lit) => self.format_literal(lit),
            ExprKind::Ident(ident) => ident.name.clone(),
            ExprKind::Path(path) => self.format_path(path),
            ExprKind::Binary { op, left, right } => format!(
                "{} {} {}",
                self.format_expr(left),
                op,
                self.format_expr(right)
            ),
            ExprKind::Unary { op, operand } => {
                let op = op.to_string();
                if op.chars().all(|c| c.is_alphabetic()) || op.ends_with("mut") {
                    format!("{} {}", op, self.format_expr(operand))
                } else {
                    format!("{}{}", op, self.format_expr(operand))
                }
            }
            ExprKind::Call { func, args } => format!(
                "{}({})",
                self.format_expr(func),
                args.iter()
                    .map(|a| self.format_expr(a))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            ExprKind::MethodCall {
                receiver,
                method,
                args,
            } => format!(
                "{}.{}({})",
                self.format_expr(receiver),
                method.name,
                args.iter()
                    .map(|a| self.format_expr(a))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            ExprKind::Block(block) => self.format_block_inline(block),
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                let mut s = format!(
                    "if {} {}",
                    self.format_expr(cond),
                    self.format_block_inline(then_branch)
                );
                if let Some(else_branch) = else_branch {
                    s.push_str(" else ");
                    s.push_str(&self.format_expr(else_branch));
                }
                s
            }
            ExprKind::IfLet {
                pattern,
                expr,
                then_branch,
                else_branch,
            } => {
                let mut s = format!(
                    "if let {} = {} {}",
                    self.format_pattern(pattern),
                    self.format_expr(expr),
                    self.format_block_inline(then_branch)
                );
                if let Some(else_branch) = else_branch {
                    s.push_str(" else ");
                    s.push_str(&self.format_expr(else_branch));
                }
                s
            }
            ExprKind::While { cond, body } => format!(
                "while {} {}",
                self.format_expr(cond),
                self.format_block_inline(body)
            ),
            ExprKind::For {
                pattern,
                iter,
                body,
            } => format!(
                "for {} in {} {}",
                self.format_pattern(pattern),
                self.format_expr(iter),
                self.format_block_inline(body)
            ),
            ExprKind::Loop(body) => format!("loop {}", self.format_block_inline(body)),
            ExprKind::Match { scrutinee, arms } => {
                let arms = arms
                    .iter()
                    .map(|arm| self.format_match_arm(arm))
                    .collect::<Vec<_>>();
                if arms.is_empty() {
                    format!("match {} {{}}", self.format_expr(scrutinee))
                } else {
                    format!(
                        "match {} {{ {} }}",
                        self.format_expr(scrutinee),
                        arms.join(", ")
                    )
                }
            }
            ExprKind::Return(value) => value
                .as_ref()
                .map(|v| format!("return {}", self.format_expr(v)))
                .unwrap_or_else(|| "return".to_string()),
            ExprKind::Break(value) => value
                .as_ref()
                .map(|v| format!("break {}", self.format_expr(v)))
                .unwrap_or_else(|| "break".to_string()),
            ExprKind::Continue => "continue".to_string(),
            ExprKind::Yield(value) => value
                .as_ref()
                .map(|v| format!("yield {}", self.format_expr(v)))
                .unwrap_or_else(|| "yield".to_string()),
            ExprKind::Await(base) => format!("await {}", self.format_expr(base)),
            ExprKind::AsyncBlock(block) => format!("async {}", self.format_block_inline(block)),
            ExprKind::ParallelBlock(block) => {
                format!("parallel {}", self.format_block_inline(block))
            }
            ExprKind::Index { base, index } => {
                format!("{}[{}]", self.format_expr(base), self.format_expr(index))
            }
            ExprKind::Field { base, field } => format!("{}.{}", self.format_expr(base), field.name),
            ExprKind::Array(items) => format!(
                "[{}]",
                items
                    .iter()
                    .map(|i| self.format_expr(i))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            ExprKind::VecBang { elements, count } => {
                if let Some(count) = count {
                    format!(
                        "vec![{}; {}]",
                        elements
                            .first()
                            .map(|value| self.format_expr(value))
                            .unwrap_or_else(|| "_".to_string()),
                        self.format_expr(count)
                    )
                } else {
                    format!(
                        "vec![{}]",
                        elements
                            .iter()
                            .map(|i| self.format_expr(i))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                }
            }
            ExprKind::Tuple(items) => {
                let rendered = items
                    .iter()
                    .map(|i| self.format_expr(i))
                    .collect::<Vec<_>>()
                    .join(", ");
                if items.is_empty() {
                    "()".to_string()
                } else if items.len() == 1 {
                    format!("({},)", rendered)
                } else {
                    format!("({})", rendered)
                }
            }
            ExprKind::Struct { path, fields, base } => {
                let mut items = fields
                    .iter()
                    .map(|field| self.format_field_value(field))
                    .collect::<Vec<_>>();
                if let Some(base) = base {
                    items.push(format!("..{}", self.format_expr(base)));
                }
                if items.is_empty() {
                    format!("{} {{}}", self.format_path(path))
                } else {
                    format!("{} {{ {} }}", self.format_path(path), items.join(", "))
                }
            }
            ExprKind::Assign { target, value } => {
                format!("{} = {}", self.format_expr(target), self.format_expr(value))
            }
            ExprKind::AssignOp { op, target, value } => format!(
                "{} {} {}",
                self.format_expr(target),
                op.as_str(),
                self.format_expr(value)
            ),
            ExprKind::Range {
                start,
                end,
                inclusive,
            } => {
                let s = start
                    .as_ref()
                    .map(|v| self.format_expr(v))
                    .unwrap_or_default();
                let e = end
                    .as_ref()
                    .map(|v| self.format_expr(v))
                    .unwrap_or_default();
                if *inclusive {
                    format!("{}..={}", s, e)
                } else {
                    format!("{}..{}", s, e)
                }
            }
            ExprKind::Lambda { params, body } => {
                let params = params
                    .iter()
                    .map(|param| param.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("|{}| {}", params, self.format_expr(body))
            }
            ExprKind::Try(base) => format!("{}?", self.format_expr(base)),
            ExprKind::TryBlock(block) => format!("try {}", self.format_block_inline(block)),
            ExprKind::Cast { expr, ty } => {
                format!("{} as {}", self.format_expr(expr), self.format_type(ty))
            }
            ExprKind::Is { expr, ty } => {
                format!("{} is {}", self.format_expr(expr), self.format_type(ty))
            }
            ExprKind::Paren(inner) => format!("({})", self.format_expr(inner)),
        }
    }

    fn format_pattern(&self, pattern: &Pattern) -> String {
        match &pattern.kind {
            PatternKind::Wildcard => "_".to_string(),
            PatternKind::Literal(lit) => self.format_literal(lit),
            PatternKind::Ident(ident) => ident.name.clone(),
            PatternKind::Path(path) => self.format_path(path),
            PatternKind::Struct { path, fields, rest } => {
                let mut members = fields
                    .iter()
                    .map(|field| {
                        if field.shorthand {
                            field.name.name.clone()
                        } else {
                            format!(
                                "{}: {}",
                                field.name.name,
                                self.format_pattern(&field.pattern)
                            )
                        }
                    })
                    .collect::<Vec<_>>();
                if *rest {
                    members.push("..".to_string());
                }
                if members.is_empty() {
                    format!("{} {{}}", self.format_path(path))
                } else {
                    format!("{} {{ {} }}", self.format_path(path), members.join(", "))
                }
            }
            PatternKind::TupleStruct { path, patterns } => format!(
                "{}({})",
                self.format_path(path),
                patterns
                    .iter()
                    .map(|pattern| self.format_pattern(pattern))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            PatternKind::Tuple(patterns) => {
                let rendered = patterns
                    .iter()
                    .map(|pattern| self.format_pattern(pattern))
                    .collect::<Vec<_>>()
                    .join(", ");
                if patterns.is_empty() {
                    "()".to_string()
                } else if patterns.len() == 1 {
                    format!("({},)", rendered)
                } else {
                    format!("({})", rendered)
                }
            }
            PatternKind::Slice(patterns, rest) => {
                let mut items = patterns
                    .iter()
                    .map(|pattern| self.format_pattern(pattern))
                    .collect::<Vec<_>>();
                if let Some(rest) = rest {
                    items.push(format!("..{}", self.format_pattern(rest)));
                }
                format!("[{}]", items.join(", "))
            }
            PatternKind::Range(start, end, range_end) => {
                let op = match range_end {
                    RangeEnd::Inclusive => "..=",
                    RangeEnd::Exclusive | RangeEnd::HalfOpen => "..",
                };
                let start_is_synth_open = matches!(start.kind, PatternKind::Wildcard)
                    && start.span.lo == end.span.lo
                    && start.span.hi == end.span.hi;
                let left = if start_is_synth_open {
                    String::new()
                } else {
                    self.format_pattern(start)
                };
                format!("{}{}{}", left, op, self.format_pattern(end))
            }
            PatternKind::Or(patterns) => {
                if patterns.is_empty() {
                    "_".to_string()
                } else {
                    patterns
                        .iter()
                        .map(|pattern| self.format_pattern(pattern))
                        .collect::<Vec<_>>()
                        .join(" | ")
                }
            }
        }
    }

    fn format_match_arm(&self, arm: &MatchArm) -> String {
        format!(
            "{}{}",
            self.format_match_arm_head(arm),
            self.format_expr(&arm.body)
        )
    }

    /// Renders one arm of a match that is already broken across lines, spreading
    /// the arm body too when the arm's own line would exceed `max_width`.
    fn format_match_arm_broken(&self, arm: &MatchArm, indent: usize) -> String {
        let head = self.format_match_arm_head(arm);
        let inline = format!(
            "{}{}{},",
            self.pad(indent),
            head,
            self.format_expr(&arm.body)
        );
        if self.fits(&inline) {
            return format!("{}{}", head, self.format_expr(&arm.body));
        }
        format!("{}{}", head, self.format_expr_broken(&arm.body, indent))
    }

    fn format_match_arm_head(&self, arm: &MatchArm) -> String {
        let mut rendered = if arm.patterns.is_empty() {
            "_".to_string()
        } else {
            arm.patterns
                .iter()
                .map(|pattern| self.format_pattern(pattern))
                .collect::<Vec<_>>()
                .join(" | ")
        };

        if let Some(guard) = &arm.guard {
            rendered.push_str(" if ");
            rendered.push_str(&self.format_expr(guard));
        }
        rendered.push_str(" => ");
        rendered
    }

    fn format_literal(&self, literal: &Literal) -> String {
        match literal {
            Literal::Int(v) => v.to_string(),
            Literal::Uint(v) => v.to_string(),
            Literal::Float(v) => {
                let mut s = v.to_string();
                if !s.contains('.') && !s.contains('e') && !s.contains('E') {
                    s.push_str(".0");
                }
                s
            }
            Literal::String(v) => format!("\"{}\"", escape_string(v)),
            Literal::Char(v) => format!("'{}'", escape_char(*v)),
            Literal::Bytes(v) => format!("b\"{}\"", String::from_utf8_lossy(v)),
            Literal::Bool(v) => v.to_string(),
            Literal::Null => "null".to_string(),
            Literal::Unit => "()".to_string(),
        }
    }
}
