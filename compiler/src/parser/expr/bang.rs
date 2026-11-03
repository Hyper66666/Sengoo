use crate::ast::*;
use crate::error::{CompileError, ParseError};
use crate::lexer::TokenKind;
use crate::Result;
use miette::SourceSpan;

use super::super::Parser;

impl<'source> Parser<'source> {
    pub(super) fn is_bang_form(&mut self) -> bool {
        self.check(TokenKind::Not) && self.check_peek(TokenKind::LBracket)
    }

    pub(super) fn parse_bang_form(&mut self, path: Path) -> Result<Expr> {
        let bang = self.expect(TokenKind::Not)?;
        let name = path
            .segments
            .last()
            .map(|segment| segment.name.as_str())
            .unwrap_or("");
        if path.segments.len() != 1 || name != "vec" {
            return Err(CompileError::ParseError(ParseError::InvalidPatternAt {
                message: format!(
                    "`{name}!` is not a user-definable macro; only the pinned `vec!` form is available"
                ),
                span: source_span(bang.span),
            }));
        }
        self.parse_vec_bang(path.span().lo)
    }

    fn parse_vec_bang(&mut self, lo: u32) -> Result<Expr> {
        self.expect(TokenKind::LBracket)?;
        if self.consume(TokenKind::RBracket).is_some() {
            return Ok(Expr::vec_bang(Vec::new(), None, self.span_at(lo)));
        }

        let first = self.parse_expr()?;
        if self.consume(TokenKind::Semicolon).is_some() {
            let count = self.parse_expr()?;
            self.expect(TokenKind::RBracket)?;
            return Ok(Expr::vec_bang(vec![first], Some(count), self.span_at(lo)));
        }

        let mut elements = vec![first];
        while self.consume(TokenKind::Comma).is_some() {
            if self.check(TokenKind::RBracket) {
                break;
            }
            elements.push(self.parse_expr()?);
        }
        self.expect(TokenKind::RBracket)?;
        Ok(Expr::vec_bang(elements, None, self.span_at(lo)))
    }
}

fn source_span(span: crate::lexer::Span) -> SourceSpan {
    (span.lo as usize, span.len() as usize).into()
}
