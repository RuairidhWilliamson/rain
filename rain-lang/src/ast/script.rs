use ordered_hash_map::OrderedHashMap;

use crate::{
    error::RainError,
    span::Span,
    tokens::{peek_stream::PeekTokenStream, NextTokenSpan, TokenKind},
};

use super::{
    function_def::FnDef, helpers::NextTokenSpanHelpers, let_declare::LetDeclare,
    visibility_specifier::VisibilitySpecifier, Ast, ParseError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Script {
    pub declarations: OrderedHashMap<String, Declaration>,
}

impl Script {
    pub fn parse_stream(stream: &mut PeekTokenStream) -> Result<Self, RainError> {
        let mut declarations = OrderedHashMap::<String, Declaration>::new();
        loop {
            let peeking = stream.peek()?;
            let NextTokenSpan::Next(token) = peeking.value() else {
                break;
            };
            match TokenKind::from(&token.token) {
                TokenKind::NewLine => {
                    peeking.consume();
                    continue;
                }
                _ => {
                    let d = Declaration::parse_stream(stream)?;
                    if let Some(existing_declare) = declarations.get(&d.name()) {
                        return Err(RainError::new(
                            ParseError::DuplicateDeclare(existing_declare.span()),
                            d.span(),
                        ));
                    }
                    declarations.insert(d.name(), d);
                }
            }
        }
        Ok(Self { declarations })
    }

    pub fn nosp(declarations: OrderedHashMap<String, Declaration>) -> Self {
        Self { declarations }
    }

    pub fn get(&self, name: &str) -> Option<&Declaration> {
        self.declarations.get(name)
    }
}

impl Ast for Script {
    fn span(&self) -> Span {
        todo!("script span")
    }

    fn reset_spans(&mut self) {
        for d in self.declarations.values_mut() {
            d.reset_spans();
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Declaration {
    LetDeclare(LetDeclare),
    LazyDeclare(LetDeclare),
    FnDeclare(FnDef),
}

impl Declaration {
    pub fn parse_stream(stream: &mut PeekTokenStream) -> Result<Self, RainError> {
        let peeking = stream.peek()?;
        let peeking_token = peeking
            .value()
            .ref_expect_not_end(ParseError::ExpectedStmt)?;
        match TokenKind::from(&peeking_token.token) {
            TokenKind::Let => Ok(Self::LetDeclare(LetDeclare::parse_stream_let(
                None, stream,
            )?)),
            TokenKind::Lazy => Ok(Self::LazyDeclare(LetDeclare::parse_stream_lazy(
                None, stream,
            )?)),
            TokenKind::Fn => Ok(Self::FnDeclare(FnDef::parse_stream(None, stream)?)),
            TokenKind::Pub => {
                let visibility = VisibilitySpecifier::parse_stream(stream)?;
                let peeking = stream.peek()?;
                let peeking_token =
                    peeking
                        .value()
                        .ref_expect_not_end(ParseError::ExpectedAny(&[
                            TokenKind::Let,
                            TokenKind::Lazy,
                            TokenKind::Fn,
                        ]))?;
                match TokenKind::from(&peeking_token.token) {
                    TokenKind::Let => Ok(Self::LetDeclare(LetDeclare::parse_stream_let(
                        Some(visibility),
                        stream,
                    )?)),
                    TokenKind::Lazy => Ok(Self::LazyDeclare(LetDeclare::parse_stream_lazy(
                        Some(visibility),
                        stream,
                    )?)),
                    TokenKind::Fn => Ok(Self::FnDeclare(FnDef::parse_stream(
                        Some(visibility),
                        stream,
                    )?)),
                    _ => Err(RainError::new(
                        ParseError::ExpectedAny(&[TokenKind::Let, TokenKind::Lazy, TokenKind::Fn]),
                        peeking_token.span,
                    )),
                }
            }
            _ => Err(RainError::new(
                ParseError::ExpectedAny(&[
                    TokenKind::Let,
                    TokenKind::Lazy,
                    TokenKind::Pub,
                    TokenKind::Fn,
                ]),
                peeking_token.span,
            )),
        }
    }

    pub fn name(&self) -> String {
        match self {
            Declaration::LetDeclare(inner) => inner.name(),
            Declaration::LazyDeclare(inner) => inner.name(),
            Declaration::FnDeclare(inner) => inner.name(),
        }
    }
}

impl Ast for Declaration {
    fn span(&self) -> Span {
        match self {
            Self::LetDeclare(inner) => inner.span(),
            Self::LazyDeclare(inner) => inner.span(),
            Self::FnDeclare(inner) => inner.span(),
        }
    }

    fn reset_spans(&mut self) {
        match self {
            Self::LetDeclare(inner) => inner.reset_spans(),
            Self::LazyDeclare(inner) => inner.reset_spans(),
            Self::FnDeclare(inner) => inner.reset_spans(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::{
        block::Block,
        dot::Dot,
        expr::Expr,
        function_call::{FnCall, FnCallArg},
        ident::Ident,
        let_declare::LetDeclare,
        statement::Statement,
        string_literal::StringLiteral,
    };

    use super::*;

    #[test]
    fn parse_script() {
        let source = "fn main() {
        core.print(\"hello world\")
        let msg = \"okie\"
        core.print(msg)
        core.print(\"goodbye\")
        }
        ";
        let mut token_stream = PeekTokenStream::new(source);
        let mut script = Script::parse_stream(&mut token_stream).unwrap();
        script.reset_spans();
        assert_eq!(
            script,
            Script::nosp(OrderedHashMap::from_iter(std::iter::once((
                String::from("main"),
                Declaration::FnDeclare(FnDef::nosp(
                    None,
                    Ident::nosp("main"),
                    vec![],
                    Block::nosp(vec![
                        Statement::Expr(Expr::FnCall(FnCall::nosp(
                            Dot::nosp(Some(Ident::nosp("core").into()), Ident::nosp("print"))
                                .into(),
                            vec![FnCallArg::nosp(
                                None,
                                StringLiteral::nosp("hello world").into()
                            )],
                        ))),
                        Statement::LetDeclare(LetDeclare::nosp(
                            None,
                            Ident::nosp("msg"),
                            StringLiteral::nosp("okie").into(),
                        )),
                        Statement::Expr(Expr::FnCall(FnCall::nosp(
                            Dot::nosp(Some(Ident::nosp("core").into()), Ident::nosp("print"))
                                .into(),
                            vec![FnCallArg::nosp(None, Ident::nosp("msg").into())],
                        ))),
                        Statement::Expr(Expr::FnCall(FnCall::nosp(
                            Dot::nosp(Some(Ident::nosp("core").into()), Ident::nosp("print"))
                                .into(),
                            vec![FnCallArg::nosp(None, StringLiteral::nosp("goodbye").into())],
                        )))
                    ])
                ))
            ))))
        );
    }
}
