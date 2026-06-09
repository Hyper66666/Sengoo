//! 词法分析器 (Lexer)
//!
//! 使用 `logos` 将 Sengoo 源代码转换为 Token 流。

pub use token::{Keyword, LiteralKind, Span, Symbol, Token, TokenKind};

mod token;

use logos::Logos;

/// 词法分析器
pub struct Lexer<'source> {
    source: &'source str,
    lexer: logos::Lexer<'source, TokenKind>,
}

impl<'source> Lexer<'source> {
    /// 创建一个新的词法分析器
    pub fn new(source: &'source str) -> Self {
        let lexer = TokenKind::lexer(source);
        Self { source, lexer }
    }

    /// 获取源代码
    pub fn source(&self) -> &'source str {
        self.source
    }

    /// Tokenize 源代码，返回 Token 流（跳过空白和注释）
    pub fn tokenize(source: &'source str) -> Vec<Token> {
        let mut lexer = TokenKind::lexer(source);
        // 预留容量以避免大文件下 token 向量反复扩容/搬迁。
        // 经验估计：平均每个 token 约占 4 字节源码，宁可略多于反复 realloc。
        let mut tokens = Vec::with_capacity(estimated_token_capacity(source.len()));

        while let Some(result) = lexer.next() {
            if let Ok(kind) = result {
                let span = lexer.span();
                tokens.push(Token::with_span(kind, span.start as u32, span.end as u32));
            }
        }

        tokens
    }
}

fn estimated_token_capacity(source_len: usize) -> usize {
    let margin = (source_len / 64).clamp(1024, 65_536);
    source_len / 3 + margin + 16
}

impl<'source> Iterator for Lexer<'source> {
    type Item = Token;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let result = self.lexer.next()?;
            if let Ok(kind) = result {
                let span = self.lexer.span();
                return Some(Token::with_span(kind, span.start as u32, span.end as u32));
            }
            // 跳过错误 Token
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_span() {
        let span = Span::new(0, 10);
        assert_eq!(span.lo, 0);
        assert_eq!(span.hi, 10);
        assert_eq!(span.len(), 10);
    }

    #[test]
    fn test_tokenize_integers() {
        let tokens = Lexer::tokenize("42 100");
        assert!(matches!(tokens[0].kind, TokenKind::Int(Some(42))));
        assert!(matches!(tokens[1].kind, TokenKind::Int(Some(100))));
    }

    #[test]
    fn test_tokenize_floats() {
        let tokens = Lexer::tokenize("3.14 2.0");
        assert!(matches!(tokens[0].kind, TokenKind::Float(Some(_))));
        assert!(matches!(tokens[1].kind, TokenKind::Float(Some(_))));
    }

    #[test]
    fn test_tokenize_keywords() {
        let tokens = Lexer::tokenize("fn let if else");
        assert!(tokens[0].is_keyword(Keyword::Fn));
        assert!(tokens[1].is_keyword(Keyword::Let));
        assert!(tokens[2].is_keyword(Keyword::If));
        assert!(tokens[3].is_keyword(Keyword::Else));
    }

    #[test]
    fn test_tokenize_operators() {
        let tokens = Lexer::tokenize("+ - * / %");
        assert_eq!(tokens[0].kind, TokenKind::Plus);
        assert_eq!(tokens[1].kind, TokenKind::Minus);
        assert_eq!(tokens[2].kind, TokenKind::Star);
        assert_eq!(tokens[3].kind, TokenKind::Slash);
        assert_eq!(tokens[4].kind, TokenKind::Percent);
    }

    #[test]
    fn test_tokenize_braces() {
        let tokens = Lexer::tokenize("{}()[]");
        assert_eq!(tokens[0].kind, TokenKind::LBrace);
        assert_eq!(tokens[1].kind, TokenKind::RBrace);
        assert_eq!(tokens[2].kind, TokenKind::LParen);
        assert_eq!(tokens[3].kind, TokenKind::RParen);
        assert_eq!(tokens[4].kind, TokenKind::LBracket);
        assert_eq!(tokens[5].kind, TokenKind::RBracket);
    }

    #[test]
    fn test_tokenize_identifiers() {
        let tokens = Lexer::tokenize("foo bar baz123");
        assert!(tokens[0].is_ident());
        assert!(tokens[1].is_ident());
        assert!(tokens[2].is_ident());
    }

    #[test]
    fn test_tokenize_string() {
        let tokens = Lexer::tokenize("\"hello\"");
        assert!(matches!(tokens[0].kind, TokenKind::String(Some(_))));
    }

    #[test]
    fn test_tokenize_function() {
        let tokens = Lexer::tokenize("fn add(x, y) { x + y }");
        assert!(tokens[0].is_keyword(Keyword::Fn));
        assert!(tokens[1].is_ident());
        assert_eq!(tokens[2].kind, TokenKind::LParen);
    }

    #[test]
    fn test_tokenize_skips_comments() {
        let tokens = Lexer::tokenize("42 // comment\n100");
        assert_eq!(tokens.len(), 2); // 42, 100
    }

    #[test]
    fn test_tokenize_all_keywords() {
        let source = "fn class struct enum impl trait type const static let \
            if else match case default for while loop break continue \
            return yield await async parallel \
            import from as export \
            extern unsafe \
            try except finally raise throw \
            pub priv where Self self \
            true false null in is";
        let tokens = Lexer::tokenize(source);
        // 应该有所有关键字
        assert!(tokens.iter().any(|t| t.is_keyword(Keyword::Fn)));
        assert!(tokens.iter().any(|t| t.is_keyword(Keyword::Async)));
        assert!(tokens.iter().any(|t| t.is_keyword(Keyword::Parallel)));
    }

    #[test]
    fn test_tokenize_comparison() {
        let tokens = Lexer::tokenize("== != < > <=");
        assert_eq!(tokens[0].kind, TokenKind::Eq);
        assert_eq!(tokens[1].kind, TokenKind::NotEq);
        assert_eq!(tokens[2].kind, TokenKind::Lt);
        assert_eq!(tokens[3].kind, TokenKind::Gt);
        assert_eq!(tokens[4].kind, TokenKind::Le);
    }

    #[test]
    fn test_large_source_token_capacity_avoids_scale_realloc() {
        // `advanced_pipeline_bench.py::make_scale_source_sengoo(1_000_000)`
        // emits about 250k tiny functions: roughly 10.4 MB and 3.5M tokens.
        // Keep the lexer estimate above that token count so the production
        // scale gate avoids a Vec growth that doubles token storage.
        assert!(estimated_token_capacity(10_389_039) >= 3_500_042);
    }
}
