use super::{
    binary_infix_operator::BinaryInfixOperator, bool_literal::BoolLiteral, dot::Dot,
    function_call::FnCall, helpers::NextTokenSpanHelpers, ident::Ident, if_condition::IfCondition,
    list_literal::ListLiteral, match_expr::Match, string_literal::StringLiteral,
    unary_prefix_operator::UnaryPrefixOperator, Ast, ParseError,
};
use crate::{
    error::RainError,
    span::Span,
    tokens::{peek_stream::PeekTokenStream, NextTokenSpan, Token, TokenKind, TokenSpan},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    Ident(Ident),
    Dot(Dot),
    FnCall(FnCall),
    BoolLiteral(BoolLiteral),
    StringLiteral(StringLiteral),
    ListLiteral(ListLiteral),
    IfCondition(IfCondition),
    Match(Match),
    UnaryPrefixOperator(UnaryPrefixOperator),
    BinaryInfixOperator(BinaryInfixOperator),
}

impl From<Ident> for Expr {
    fn from(inner: Ident) -> Self {
        Self::Ident(inner)
    }
}

impl From<Dot> for Expr {
    fn from(inner: Dot) -> Self {
        Self::Dot(inner)
    }
}

impl From<FnCall> for Expr {
    fn from(inner: FnCall) -> Self {
        Self::FnCall(inner)
    }
}

impl From<BoolLiteral> for Expr {
    fn from(inner: BoolLiteral) -> Self {
        Self::BoolLiteral(inner)
    }
}

impl From<StringLiteral> for Expr {
    fn from(inner: StringLiteral) -> Self {
        Self::StringLiteral(inner)
    }
}

impl From<ListLiteral> for Expr {
    fn from(inner: ListLiteral) -> Self {
        Self::ListLiteral(inner)
    }
}

impl From<IfCondition> for Expr {
    fn from(inner: IfCondition) -> Self {
        Self::IfCondition(inner)
    }
}

impl From<Match> for Expr {
    fn from(inner: Match) -> Self {
        Self::Match(inner)
    }
}

impl Expr {
    pub fn parse_stream(stream: &mut PeekTokenStream) -> Result<Self, RainError> {
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
                Self::BoolLiteral(BoolLiteral { value: true, span })
            }
            Token::FalseLiteral => {
                let span = peeking
                    .consume()
                    .expect_next(crate::tokens::TokenKind::FalseLiteral)?
                    .span;
                Self::BoolLiteral(BoolLiteral { value: false, span })
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
                Self::StringLiteral(StringLiteral { value, span })
            }
            Token::LBracket => Self::ListLiteral(ListLiteral::parse_stream(stream)?),
            Token::If => Self::IfCondition(IfCondition::parse_stream(stream)?),
            Token::Match => Self::Match(Match::parse_stream(stream)?),
            Token::Dot => Self::Dot(Dot::parse_stream(None, stream)?),
            Token::Exclamation => {
                Self::UnaryPrefixOperator(UnaryPrefixOperator::parse_stream(stream)?)
            }
            Token::Ident(_) => Self::Ident(Ident::parse(
                peeking.consume().expect_next(TokenKind::Ident)?,
            )?),
            _ => {
                tracing::error!("unexpected tokens {:?}", first_token_span);
                return Err(RainError::new(
                    ParseError::UnexpectedTokens,
                    first_token_span.span,
                ));
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
                TokenKind::Dot => Self::Dot(Dot::parse_stream(Some(expr), stream)?),
                TokenKind::LParen => Self::FnCall(FnCall::parse_stream(expr, stream)?),
                _ => return Ok(expr),
            };
        }
    }
}

impl Ast for Expr {
    fn span(&self) -> Span {
        match self {
            Self::Ident(inner) => inner.span(),
            Self::Dot(inner) => inner.span(),
            Self::FnCall(inner) => inner.span(),
            Self::BoolLiteral(inner) => inner.span(),
            Self::StringLiteral(inner) => inner.span(),
            Self::ListLiteral(inner) => inner.span(),
            Self::IfCondition(inner) => inner.span(),
            Self::Match(inner) => inner.span(),
            Self::UnaryPrefixOperator(inner) => inner.span(),
            Self::BinaryInfixOperator(inner) => inner.span(),
        }
    }

    fn reset_spans(&mut self) {
        match self {
            Self::Ident(inner) => inner.reset_spans(),
            Self::Dot(inner) => inner.reset_spans(),
            Self::FnCall(inner) => inner.reset_spans(),
            Self::BoolLiteral(inner) => inner.reset_spans(),
            Self::StringLiteral(inner) => inner.reset_spans(),
            Self::ListLiteral(inner) => inner.reset_spans(),
            Self::IfCondition(inner) => inner.reset_spans(),
            Self::Match(inner) => inner.reset_spans(),
            Self::UnaryPrefixOperator(inner) => inner.reset_spans(),
            Self::BinaryInfixOperator(inner) => inner.reset_spans(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::{
        block::Block, function_call::FnCallArg, ident::Ident, if_condition::ElseCondition,
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
            Some(Expr::Ident(Ident::nosp("core"))),
            Ident::nosp("print"),
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
            vec![FnCallArg::nosp(None, Expr::Ident(Ident::nosp("a")))],
        ))
    );

    parse_expr_test!(
        parse_fn_call_args,
        "foo(a, b, c)",
        FnCall::nosp(
            Ident::nosp("foo").into(),
            vec![
                FnCallArg::nosp(None, Ident::nosp("a").into()),
                FnCallArg::nosp(None, Ident::nosp("b").into()),
                FnCallArg::nosp(None, Ident::nosp("c").into()),
            ],
        )
        .into()
    );

    parse_expr_test!(
        parse_print_call,
        "core.print(a, b)",
        FnCall::nosp(
            Dot::nosp(Some(Ident::nosp("core").into()), Ident::nosp("print")).into(),
            vec![
                FnCallArg::nosp(None, Ident::nosp("a").into()),
                FnCallArg::nosp(None, Ident::nosp("b").into())
            ],
        )
        .into()
    );

    parse_expr_test!(
        parse_print_hello_world,
        "core.print(\"hello world\")",
        Expr::FnCall(FnCall::nosp(
            Dot::nosp(Some(Ident::nosp("core").into()), Ident::nosp("print")).into(),
            vec![FnCallArg::nosp(
                None,
                StringLiteral::nosp("hello world").into()
            )],
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

    parse_expr_test!(
        parse_empty_array_literal,
        "[]",
        ListLiteral::nosp(vec![]).into()
    );

    parse_expr_test!(
        parse_multi_array_literal,
        "[\"hi\", true, foo(), [a]]",
        ListLiteral::nosp(vec![
            StringLiteral::nosp("hi").into(),
            BoolLiteral::nosp(true).into(),
            FnCall::nosp(Expr::Ident(Ident::nosp("foo")), vec![]).into(),
            ListLiteral::nosp(vec![Ident::nosp("a").into()]).into(),
        ])
        .into()
    );

    parse_expr_test!(
        parse_named_arg,
        "foo(a, b = c)",
        FnCall::nosp(
            Ident::nosp("foo").into(),
            vec![
                FnCallArg::nosp(None, Ident::nosp("a").into()),
                FnCallArg::nosp(Some(Ident::nosp("b")), Ident::nosp("c").into())
            ]
        )
        .into()
    );

    parse_expr_test!(
        parse_string_literal,
        "\"abc\\ndef\"",
        StringLiteral::nosp("abc\ndef").into()
    );

    // parse_expr_test!(parse_extra_parens, "((a))", Ident::nosp("a").into());

    // parse_expr_test!(parse_equals, "a == a", Ident::nosp("a").into());
}
