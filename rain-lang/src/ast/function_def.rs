use crate::{
    error::RainError,
    span::Span,
    tokens::{peek_stream::PeekTokenStream, Token, TokenKind},
};

use super::{
    block::Block,
    helpers::{NextTokenSpanHelpers, PeekTokenStreamHelpers},
    ident::Ident,
    visibility_specifier::VisibilitySpecifier,
    Ast, ParseError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FnDef {
    pub visibility: Option<VisibilitySpecifier>,
    pub fn_token: Span,
    pub name: Ident,
    pub lparen_token: Span,
    pub args: Vec<FnDefArg>,
    pub rparen_token: Span,
    pub block: Block,
}

impl FnDef {
    pub fn parse_stream(
        visibility: Option<VisibilitySpecifier>,
        stream: &mut PeekTokenStream,
    ) -> Result<Self, RainError> {
        let fn_token = stream.expect_parse_next(TokenKind::Fn)?.span;
        let name = Ident::parse(stream.expect_parse_next(TokenKind::Ident)?)?;
        let lparen_token = stream.expect_parse_next(TokenKind::LParen)?.span;
        let mut args = Vec::new();
        let rparen_token: Span;
        loop {
            let peeking = stream.peek()?;
            let token_span = peeking
                .consume()
                .expect_not_end(ParseError::Expected(TokenKind::RParen))?;
            match token_span.token.kind() {
                TokenKind::RParen => {
                    rparen_token = token_span.span;
                    break;
                }
                TokenKind::Ident => {
                    args.push(FnDefArg {
                        name: Ident::parse(token_span)?,
                    });
                }
                _ => {
                    return Err(RainError::new(
                        ParseError::ExpectedAny(&[TokenKind::Ident, TokenKind::RParen]),
                        token_span.span,
                    ));
                }
            }
            let peeking = stream.peek()?;
            let peeking_token_span = peeking
                .value()
                .ref_expect_not_end(ParseError::Expected(TokenKind::RParen))?;
            if peeking_token_span.token == Token::RParen {
                rparen_token = peeking.consume().expect_next(TokenKind::RParen)?.span;
                break;
            } else if peeking_token_span.token == Token::Comma {
                peeking.consume();
            }
        }
        let block = Block::parse_stream(stream)?;
        Ok(Self {
            visibility,
            fn_token,
            name,
            lparen_token,
            args,
            rparen_token,
            block,
        })
    }

    pub fn nosp(
        visibility: Option<VisibilitySpecifier>,
        name: Ident,
        args: Vec<FnDefArg>,
        block: Block,
    ) -> Self {
        Self {
            visibility,
            fn_token: Span::default(),
            name,
            lparen_token: Span::default(),
            args,
            rparen_token: Span::default(),
            block,
        }
    }
}

impl Ast for FnDef {
    fn span(&self) -> Span {
        self.fn_token.combine(self.block.span())
    }

    fn reset_spans(&mut self) {
        for v in &mut self.visibility {
            v.reset_spans();
        }
        self.fn_token.reset();
        self.name.reset_spans();
        self.lparen_token.reset();
        for a in &mut self.args {
            a.reset_spans();
        }
        self.rparen_token.reset();
        self.block.reset_spans();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FnDefArg {
    pub name: Ident,
}

impl FnDefArg {
    pub fn nosp(name: Ident) -> Self {
        Self { name }
    }
}

impl Ast for FnDefArg {
    fn span(&self) -> Span {
        self.name.span
    }

    fn reset_spans(&mut self) {
        self.name.reset_spans();
    }
}

#[cfg(test)]
mod tests {
    use crate::error::RainError;

    use super::*;

    fn parse_fn_def(source: &str) -> Result<FnDef, RainError> {
        let mut stream = PeekTokenStream::new(source);
        let mut fn_def = super::FnDef::parse_stream(None, &mut stream)?;
        fn_def.reset_spans();
        Ok(fn_def)
    }

    #[test]
    fn parse_no_args() -> Result<(), RainError> {
        let fn_def = parse_fn_def("fn foo() {}")?;
        assert_eq!(
            fn_def,
            FnDef::nosp(None, Ident::nosp("foo"), vec![], Block::nosp(vec![])),
        );
        Ok(())
    }

    #[test]
    fn parse_single_arg() -> Result<(), RainError> {
        let fn_def = parse_fn_def("fn foo(a) {}")?;
        assert_eq!(
            fn_def,
            FnDef::nosp(
                None,
                Ident::nosp("foo"),
                vec![FnDefArg::nosp(Ident::nosp("a"))],
                Block::nosp(vec![])
            ),
        );
        Ok(())
    }
}
