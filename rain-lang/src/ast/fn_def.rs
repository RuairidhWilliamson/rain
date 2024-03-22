use crate::{
    error::RainError,
    tokens::{peek_stream::PeekTokenStream, Token, TokenKind},
};

use super::{
    block::Block,
    helpers::{NextTokenSpanHelpers, PeekNextTokenHelpers, PeekTokenStreamHelpers},
    ident::Ident,
    Ast, ParseError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FnDef<'a> {
    pub name: Ident<'a>,
    pub args: Vec<FnDefArg<'a>>,
    pub block: Block<'a>,
}

impl<'a> FnDef<'a> {
    pub fn parse_stream(stream: &mut PeekTokenStream<'a>) -> Result<Self, RainError> {
        stream.expect_parse_next(TokenKind::Fn)?;
        let name = Ident::parse(stream.expect_parse_next(TokenKind::Ident)?)?;
        stream.expect_parse_next(TokenKind::LParen)?;
        let mut args = Vec::new();
        loop {
            let peeking = stream.peek()?;
            let peeking_token_span =
                peeking.expect_not_end(ParseError::Expected(TokenKind::RParen))?;
            if peeking_token_span.token == Token::RParen {
                peeking.consume();
                break;
            }
            if TokenKind::from(&peeking_token_span.token) == TokenKind::Ident {
                let ident = Ident::parse(peeking.consume().expect_next(TokenKind::Ident)?)?;
                args.push(FnDefArg { name: ident });
            }
            let peeking = stream.peek()?;
            let peeking_token_span =
                peeking.expect_not_end(ParseError::Expected(TokenKind::RParen))?;
            if peeking_token_span.token == Token::RParen {
                peeking.consume();
                break;
            } else if peeking_token_span.token == Token::Comma {
                peeking.consume();
            }
        }
        let block = Block::parse_stream(stream)?;
        Ok(Self { name, args, block })
    }

    pub fn nosp(name: Ident<'a>, args: Vec<FnDefArg<'a>>, block: Block<'a>) -> Self {
        Self { name, args, block }
    }
}

impl Ast for FnDef<'_> {
    fn reset_spans(&mut self) {
        self.name.reset_spans();
        for a in &mut self.args {
            a.reset_spans();
        }
        self.block.reset_spans();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FnDefArg<'a> {
    pub name: Ident<'a>,
}

impl Ast for FnDefArg<'_> {
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
        let mut fn_def = super::FnDef::parse_stream(&mut stream)?;
        fn_def.reset_spans();
        Ok(fn_def)
    }

    #[test]
    fn parse_no_args() -> Result<(), RainError> {
        let fn_def = parse_fn_def("fn foo() {}")?;
        assert_eq!(
            fn_def,
            FnDef::nosp(Ident::nosp("foo"), vec![], Block::nosp(vec![]))
        );
        Ok(())
    }
}
