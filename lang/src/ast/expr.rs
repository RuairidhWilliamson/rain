use crate::{
    error::ErrorSpan,
    span::LocalSpan,
    tokens::{peek::PeekTokenStream, StringLiteralPrefix, Token, TokenLocalSpan},
};

use super::{
    binary_op::{
        get_token_precedence_associativity, Associativity, BinaryOp, BinaryOperator, Precedence,
    },
    error::{ParseError, ParseResult},
    expect_token, Block,
};

#[derive(Debug)]
pub enum Expr {
    Ident(TokenLocalSpan),
    StringLiteral(StringLiteral),
    IntegerLiteral(TokenLocalSpan),
    TrueLiteral(TokenLocalSpan),
    FalseLiteral(TokenLocalSpan),
    BinaryOp(BinaryOp),
    FnCall(FnCall),
    If(IfCondition),
    Internal(TokenLocalSpan),
}

impl Expr {
    pub fn parse(stream: &mut PeekTokenStream) -> ParseResult<Self> {
        let lhs = Self::parse_primary(stream)?;
        Self::parse_expr_ops(stream, lhs, 0)
    }

    fn parse_primary(stream: &mut PeekTokenStream) -> ParseResult<Self> {
        let Some(t) = stream.parse_next()? else {
            return Err(ErrorSpan::new(ParseError::ExpectedExpression(None), None));
        };
        let expr = match t.token {
            Token::Ident => Self::Ident(t),
            Token::Number => Self::IntegerLiteral(t),
            Token::DoubleQuoteLiteral(_) => Self::StringLiteral(StringLiteral(t)),
            Token::True => Self::TrueLiteral(t),
            Token::False => Self::FalseLiteral(t),
            Token::Internal => Self::Internal(t),
            Token::LParen => {
                let expr = Self::parse(stream)?;
                expect_token(stream.parse_next()?, &[Token::RParen])?;
                expr
            }
            Token::If => Self::If(Self::parse_if_condition(t, stream)?),
            _ => {
                return Err(t
                    .span
                    .with_error(ParseError::ExpectedExpression(Some(t.token))))
            }
        };
        Ok(expr)
    }

    fn parse_if_condition(
        if_token: TokenLocalSpan,
        stream: &mut PeekTokenStream,
    ) -> ParseResult<IfCondition> {
        debug_assert_eq!(if_token.token, Token::If);
        let condition = Box::new(Self::parse(stream)?);
        let then = Block::parse(stream)?;
        let mut alternate = None;
        if let Some(peek) = stream.peek()? {
            if peek.token == Token::Else {
                let _ = stream.parse_next()?;
                alternate = Some(Self::parse_alternate(stream)?);
            }
        }
        Ok(IfCondition {
            condition,
            then,
            alternate,
        })
    }

    fn parse_alternate(stream: &mut PeekTokenStream) -> ParseResult<AlternateCondition> {
        let peek = expect_token(stream.peek()?, &[Token::If, Token::LBrace])?;
        match peek.token {
            Token::If => {
                let _ = stream.parse_next()?;
                Ok(AlternateCondition::IfElse(Box::new(
                    Self::parse_if_condition(peek, stream)?,
                )))
            }
            Token::LBrace => Ok(AlternateCondition::Else(Block::parse(stream)?)),
            _ => unreachable!(),
        }
    }

    fn parse_expr_ops(
        stream: &mut PeekTokenStream,
        mut lhs: Self,
        min_precedence: usize,
    ) -> ParseResult<Self> {
        while let Some((t, precedence)) = Self::check_op(stream.peek()?, min_precedence) {
            if t.token == Token::LParen {
                lhs = Self::FnCall(Self::parse_fn_call(stream, lhs)?);
                continue;
            }
            stream.parse_next()?;
            let mut rhs = Self::parse_primary(stream)?;
            while let Some((_, next_op_precedence)) = Self::check_op(stream.peek()?, precedence) {
                let next_precedence = precedence + usize::from(next_op_precedence > precedence);
                rhs = Self::parse_expr_ops(stream, rhs, next_precedence)?;
            }
            let Some(op) = BinaryOperator::new_from_token(t) else {
                unreachable!()
            };
            lhs = Self::BinaryOp(BinaryOp {
                left: Box::new(lhs),
                op,
                right: Box::new(rhs),
            });
        }
        Ok(lhs)
    }

    fn check_op(
        t: Option<TokenLocalSpan>,
        min_precedence: usize,
    ) -> Option<(TokenLocalSpan, Precedence)> {
        let t = t?;
        let (precedence, associativity) = get_token_precedence_associativity(t.token)?;
        if precedence > min_precedence
            || precedence == min_precedence && associativity == Associativity::Right
        {
            Some((t, precedence))
        } else {
            None
        }
    }

    fn parse_fn_call(stream: &mut PeekTokenStream, lhs: Self) -> ParseResult<FnCall> {
        let lparen_token = stream.parse_next()?.unwrap();
        let mut args = Vec::new();
        loop {
            let Some(t) = stream.peek()? else {
                break;
            };
            if t.token == Token::RParen {
                break;
            }
            args.push(Self::parse(stream)?);
            let Some(t) = stream.peek()? else {
                break;
            };
            match t.token {
                Token::Comma => {
                    stream.parse_next()?;
                }
                _ => break,
            }
        }
        let rparen_token = expect_token(stream.parse_next()?, &[Token::RParen])?;
        Ok(FnCall {
            callee: Box::new(lhs),
            args: FnCallArgs {
                lparen_token,
                args,
                rparen_token,
            },
        })
    }
}

impl super::display::AstDisplay for Expr {
    fn span(&self) -> LocalSpan {
        match self {
            Self::Ident(inner)
            | Self::IntegerLiteral(inner)
            | Self::TrueLiteral(inner)
            | Self::FalseLiteral(inner)
            | Self::Internal(inner) => inner.span,
            Self::StringLiteral(inner) => inner.span(),
            Self::BinaryOp(inner) => inner.span(),
            Self::FnCall(inner) => inner.span(),
            Self::If(inner) => inner.span(),
        }
    }

    fn fmt(&self, f: &mut super::display::AstFormatter<'_>) -> std::fmt::Result {
        let inner: &dyn super::display::AstDisplay = match self {
            Self::Ident(inner)
            | Self::IntegerLiteral(inner)
            | Self::TrueLiteral(inner)
            | Self::FalseLiteral(inner)
            | Self::Internal(inner) => inner,
            Self::StringLiteral(inner) => &inner.0,
            Self::BinaryOp(inner) => inner,
            Self::FnCall(inner) => inner,
            Self::If(inner) => inner,
        };
        inner.fmt(f)
    }
}

#[derive(Debug)]
pub struct StringLiteral(pub TokenLocalSpan);

impl StringLiteral {
    pub fn prefix(&self) -> Option<StringLiteralPrefix> {
        let Token::DoubleQuoteLiteral(prefix) = self.0.token else {
            unreachable!()
        };
        prefix
    }

    pub fn content_span(&self) -> LocalSpan {
        let mut s = self.0.span;
        if self.prefix().is_some() {
            s.start += 1;
        }
        s.start += 1;
        s.end -= 1;
        s
    }
}

impl super::display::AstDisplay for StringLiteral {
    fn span(&self) -> LocalSpan {
        self.0.span
    }

    fn fmt(&self, f: &mut super::display::AstFormatter<'_>) -> std::fmt::Result {
        f.node("StringLiteral").child(&self.0.span).finish()
    }
}

#[derive(Debug)]
pub struct FnCall {
    pub callee: Box<Expr>,
    pub args: FnCallArgs,
}

impl super::display::AstDisplay for FnCall {
    fn span(&self) -> LocalSpan {
        self.callee.span() + self.args.span()
    }

    fn fmt(&self, f: &mut super::display::AstFormatter<'_>) -> std::fmt::Result {
        let mut builder = f.node("FnCall");
        builder.child(self.callee.as_ref());
        builder.child(&self.args);
        builder.finish()
    }
}

#[derive(Debug)]
pub struct FnCallArgs {
    pub lparen_token: TokenLocalSpan,
    pub args: Vec<Expr>,
    pub rparen_token: TokenLocalSpan,
}

impl super::display::AstDisplay for FnCallArgs {
    fn span(&self) -> LocalSpan {
        self.lparen_token.span() + self.rparen_token.span()
    }

    fn fmt(&self, f: &mut super::display::AstFormatter<'_>) -> std::fmt::Result {
        let mut builder = f.node("FnCallArgs");
        for a in &self.args {
            builder.child(a);
        }
        builder.finish()
    }
}

#[derive(Debug)]
pub struct IfCondition {
    pub condition: Box<Expr>,
    pub then: super::Block,
    pub alternate: Option<AlternateCondition>,
}

impl super::display::AstDisplay for IfCondition {
    fn span(&self) -> LocalSpan {
        let mut s = self.condition.span() + self.then.span();
        if let Some(alternate) = &self.alternate {
            s += alternate.span();
        }
        s
    }

    fn fmt(&self, f: &mut super::display::AstFormatter<'_>) -> std::fmt::Result {
        let mut builder = f.node("If");
        builder.child(self.condition.as_ref()).child(&self.then);
        if let Some(alternate) = &self.alternate {
            builder.child(alternate);
        }
        builder.finish()
    }
}

#[derive(Debug)]
pub enum AlternateCondition {
    IfElse(Box<IfCondition>),
    Else(super::Block),
}

impl super::display::AstDisplay for AlternateCondition {
    fn span(&self) -> LocalSpan {
        match self {
            Self::IfElse(inner) => inner.span(),
            Self::Else(inner) => inner.span(),
        }
    }

    fn fmt(&self, f: &mut super::display::AstFormatter<'_>) -> std::fmt::Result {
        match self {
            Self::IfElse(alternate) => f.node("IfElse").child(alternate.as_ref()).finish(),
            Self::Else(alternate) => f.node("Else").child(alternate).finish(),
        }
    }
}

#[cfg(test)]
mod test {
    use crate::{ast::display::display_ast, tokens::peek::PeekTokenStream};

    use super::Expr;

    fn parse_display_expr(src: &str) -> String {
        let mut stream = PeekTokenStream::new(src);
        let s = match Expr::parse(&mut stream) {
            Ok(s) => s,
            Err(err) => {
                eprintln!("{}", err.resolve(None, src));
                panic!("parse error");
            }
        };
        assert_eq!(
            stream.parse_next().unwrap(),
            None,
            "input not fully consumed"
        );
        display_ast(&s, src)
    }

    #[test]
    fn number_literal() {
        insta::assert_snapshot!(parse_display_expr("4"));
    }

    #[test]
    fn string_literal() {
        insta::assert_snapshot!(parse_display_expr("\"asldjf\""));
    }

    #[test]
    fn number_add() {
        insta::assert_snapshot!(parse_display_expr("1 + 1"));
    }

    #[test]
    fn number_add_left_associative() {
        insta::assert_snapshot!(parse_display_expr("1 + 2 + 3"));
    }

    #[test]
    fn number_multiply() {
        insta::assert_snapshot!(parse_display_expr("1 * 2"));
    }

    #[test]
    fn number_multiply_left_associative() {
        insta::assert_snapshot!(parse_display_expr("1 * 2 * 3"));
    }

    #[test]
    fn number_multiply_add_precedence1() {
        insta::assert_snapshot!(parse_display_expr("5 * 2 + 3"));
    }

    #[test]
    fn number_multiply_add_precedence2() {
        insta::assert_snapshot!(parse_display_expr("5 + 2 * 3"));
    }

    #[test]
    fn number_add_subtract_precedence() {
        insta::assert_snapshot!(parse_display_expr("5 - 2 + 3 - 4"));
    }

    #[test]
    fn number_add_subtract_multiply_precedence() {
        insta::assert_snapshot!(parse_display_expr("5 * 2 + 3 - 4"));
    }

    #[test]
    fn number_add_subtrace_multiply_divide_precedence() {
        insta::assert_snapshot!(parse_display_expr("1 - 3 / 2 + 4 * 3"));
    }

    #[test]
    fn ident_maths() {
        insta::assert_snapshot!(parse_display_expr("a + b - c * d / e"));
    }

    #[test]
    fn ident_dot_ident() {
        insta::assert_snapshot!(parse_display_expr("foo.bar"));
    }

    #[test]
    fn ident_dot_ident_dot_ident() {
        insta::assert_snapshot!(parse_display_expr("foo.bar.baz"));
    }

    #[test]
    fn ident_dot_maths() {
        insta::assert_snapshot!(parse_display_expr("a.b.c + 3 * d.e"));
    }

    #[test]
    fn maths_parens1() {
        insta::assert_snapshot!(parse_display_expr("1 - (a + 3) * 4"));
    }

    #[test]
    fn maths_parens2() {
        insta::assert_snapshot!(parse_display_expr("(3 - b) * c"));
    }

    #[test]
    fn fn_call_no_args() {
        insta::assert_snapshot!(parse_display_expr("foo()"));
    }

    #[test]
    fn fn_call_no_args_call_no_args() {
        insta::assert_snapshot!(parse_display_expr("foo()()"));
    }

    #[test]
    fn fn_call_no_args_precedence() {
        insta::assert_snapshot!(parse_display_expr("foo.bar()"));
    }

    #[test]
    fn fn_call_one_arg() {
        insta::assert_snapshot!(parse_display_expr("foo(1)"));
    }

    #[test]
    fn fn_call_two_arg() {
        insta::assert_snapshot!(parse_display_expr("foo(1, 2)"));
    }

    #[test]
    fn fn_call_two_arg_trailing_comma() {
        insta::assert_snapshot!(parse_display_expr("foo(1, 2,)"));
    }

    #[test]
    fn logical_operators() {
        insta::assert_snapshot!(parse_display_expr(
            "true || a == b && 1 != 1 && (false || a != b)"
        ));
    }
}
