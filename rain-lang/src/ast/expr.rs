use super::{
    bool_literal::BoolLiteral, dot::Dot, function_call::FnCall, helpers::NextTokenSpanHelpers,
    ident::Ident, if_condition::IfCondition, match_expr::Match, string_literal::StringLiteral, Ast,
    ParseError,
};
use crate::{
    error::RainError,
    span::Span,
    tokens::{peek_stream::PeekTokenStream, NextTokenSpan, Token, TokenKind, TokenSpan},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr<'a> {
    Ident(Ident<'a>),
    Dot(Dot<'a>),
    FnCall(FnCall<'a>),
    BoolLiteral(BoolLiteral),
    StringLiteral(StringLiteral),
    IfCondition(IfCondition<'a>),
    Match(Match<'a>),
}

impl<'a> From<Ident<'a>> for Expr<'a> {
    fn from(inner: Ident<'a>) -> Self {
        Self::Ident(inner)
    }
}

impl<'a> From<Dot<'a>> for Expr<'a> {
    fn from(inner: Dot<'a>) -> Self {
        Self::Dot(inner)
    }
}

impl<'a> From<FnCall<'a>> for Expr<'a> {
    fn from(inner: FnCall<'a>) -> Self {
        Self::FnCall(inner)
    }
}

impl<'a> From<BoolLiteral> for Expr<'a> {
    fn from(inner: BoolLiteral) -> Self {
        Self::BoolLiteral(inner)
    }
}

impl<'a> From<StringLiteral> for Expr<'a> {
    fn from(inner: StringLiteral) -> Self {
        Self::StringLiteral(inner)
    }
}

impl<'a> From<IfCondition<'a>> for Expr<'a> {
    fn from(inner: IfCondition<'a>) -> Self {
        Self::IfCondition(inner)
    }
}

impl<'a> From<Match<'a>> for Expr<'a> {
    fn from(inner: Match<'a>) -> Self {
        Self::Match(inner)
    }
}

impl<'a> Expr<'a> {
    pub fn parse_stream(stream: &mut PeekTokenStream<'a>) -> Result<Self, RainError> {
        let peeking = stream.peek()?;
        let first_token_span = match peeking.value() {
            NextTokenSpan::Next(token_span) => token_span,
            NextTokenSpan::End(span) => {
                return Err(RainError::new(ParseError::EmptyExpression, *span));
            }
        };
        let mut expr = match &first_token_span.token {
            Token::TrueLiteral => {
                let span = peeking
                    .consume()
                    .expect_next(crate::tokens::TokenKind::TrueLiteral)?
                    .span;
                Expr::BoolLiteral(BoolLiteral { value: true, span })
            }
            Token::FalseLiteral => {
                let span = peeking
                    .consume()
                    .expect_next(crate::tokens::TokenKind::FalseLiteral)?
                    .span;
                Expr::BoolLiteral(BoolLiteral { value: false, span })
            }
            Token::DoubleQuoteLiteral(_) => {
                let TokenSpan {
                    token: Token::DoubleQuoteLiteral(value),
                    span,
                } = peeking
                    .consume()
                    .expect_next(crate::tokens::TokenKind::DoubleQuoteLiteral)?
                else {
                    unreachable!("we already have checked this is a double quote literal");
                };
                Expr::StringLiteral(StringLiteral { value, span })
            }
            Token::If => Expr::IfCondition(IfCondition::parse_stream(stream)?),
            Token::Match => Expr::Match(Match::parse_stream(stream)?),
            Token::Dot => Expr::Dot(Dot::parse_stream(None, stream)?),
            Token::Ident(_) => Expr::Ident(Ident::parse(
                peeking.consume().expect_next(TokenKind::Ident)?,
            )?),
            _ => {
                return Err(RainError::new(
                    ParseError::UnexpectedTokens,
                    first_token_span.span,
                ))
            }
        };
        // After the initial expression we can also add .<ident> or make a function call
        loop {
            let peeking = stream.peek()?;
            let NextTokenSpan::Next(token_span) = peeking.value() else {
                // No more tokens so must be the end of the expression
                return Ok(expr);
            };
            let kind = token_span.token.kind();
            expr = match kind {
                TokenKind::Dot => Expr::Dot(Dot::parse_stream(Some(expr), stream)?),
                TokenKind::LParen => Expr::FnCall(FnCall::parse_stream(expr, stream)?),
                _ => return Ok(expr),
            };
        }
    }
}

impl Ast for Expr<'_> {
    fn span(&self) -> Span {
        match self {
            Expr::Ident(inner) => inner.span(),
            Expr::Dot(inner) => inner.span(),
            Expr::FnCall(inner) => inner.span(),
            Expr::BoolLiteral(inner) => inner.span(),
            Expr::StringLiteral(inner) => inner.span(),
            Expr::IfCondition(inner) => inner.span(),
            Expr::Match(inner) => inner.span(),
        }
    }

    fn reset_spans(&mut self) {
        match self {
            Expr::Ident(inner) => inner.reset_spans(),
            Expr::Dot(inner) => inner.reset_spans(),
            Expr::FnCall(inner) => inner.reset_spans(),
            Expr::BoolLiteral(inner) => inner.reset_spans(),
            Expr::StringLiteral(inner) => inner.reset_spans(),
            Expr::IfCondition(inner) => inner.reset_spans(),
            Expr::Match(inner) => inner.reset_spans(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        ast::{block::Block, ident::Ident, if_condition::ElseCondition},
        span::Span,
    };

    use super::*;

    macro_rules! parse_expr_test {
        ($name:ident, $source:expr, $expected:expr) => {
            #[test]
            fn $name() -> Result<(), RainError> {
                let mut token_stream = PeekTokenStream::new($source);
                let mut expr = Expr::parse_stream(&mut token_stream)?;
                expr.reset_spans();
                assert_eq!(expr, $expected);
                Ok(())
            }
        };
    }

    parse_expr_test!(parse_single_ident, "std", Expr::Ident(Ident::nosp("std")));

    parse_expr_test!(
        parse_item,
        "core.print",
        Expr::Dot(Dot::nosp(
            Some(Expr::Ident(Ident {
                name: "core",
                span: Span::default()
            })),
            Ident {
                name: "print",
                span: Span::default()
            },
        ),)
    );

    parse_expr_test!(
        parse_item3,
        "a.b.c",
        Expr::Dot(Dot::nosp(
            Some(Expr::Dot(Dot::nosp(
                Some(Expr::Ident(Ident::nosp("a"))),
                Ident::nosp("b"),
            ))),
            Ident::nosp("c")
        ))
    );

    parse_expr_test!(
        parse_fn_call,
        "foo()",
        Expr::FnCall(FnCall::nosp(Expr::Ident(Ident::nosp("foo")), vec![]))
    );

    parse_expr_test!(
        parse_fn_call_arg,
        "foo(a)",
        Expr::FnCall(FnCall::nosp(
            Expr::Ident(Ident::nosp("foo")),
            vec![Expr::Ident(Ident::nosp("a"))],
        ))
    );

    parse_expr_test!(
        parse_fn_call_args,
        "foo(a, b, c)",
        FnCall::nosp(
            Ident::nosp("foo").into(),
            vec![
                Ident::nosp("a").into(),
                Ident::nosp("b").into(),
                Ident::nosp("c").into()
            ],
        )
        .into()
    );

    parse_expr_test!(
        parse_print_call,
        "core.print(a, b)",
        FnCall::nosp(
            Dot::nosp(Some(Ident::nosp("core").into()), Ident::nosp("print")).into(),
            vec![Ident::nosp("a").into(), Ident::nosp("b").into()],
        )
        .into()
    );

    parse_expr_test!(
        parse_print_hello_world,
        "core.print(\"hello world\")",
        Expr::FnCall(FnCall::nosp(
            Dot::nosp(Some(Ident::nosp("core").into()), Ident::nosp("print")).into(),
            vec![StringLiteral::nosp("hello world").into()],
        ))
    );

    parse_expr_test!(
        parse_if,
        "if true {}",
        Expr::IfCondition(IfCondition::nosp(
            Expr::BoolLiteral(BoolLiteral::nosp(true)),
            Block::nosp(vec![]),
            None,
        ))
    );

    parse_expr_test!(
        parse_if_else,
        "if false {} else {}",
        Expr::IfCondition(IfCondition::nosp(
            Expr::BoolLiteral(BoolLiteral::nosp(false)),
            Block::nosp(vec![]),
            Some(ElseCondition::nosp(Block::nosp(vec![])))
        ))
    );

    parse_expr_test!(
        parse_fn_dot,
        "foo().a",
        Dot::nosp(
            Some(FnCall::nosp(Expr::Ident(Ident::nosp("foo")), vec![]).into()),
            Ident::nosp("a")
        )
        .into()
    );

    parse_expr_test!(
        parse_string_dot,
        "\"hi\".foo",
        Dot::nosp(Some(StringLiteral::nosp("hi").into()), Ident::nosp("foo")).into()
    );
}
