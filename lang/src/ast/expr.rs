use crate::{
    error::{ErrorSpan, ErrorSpanExt},
    span::LocalSpan,
    tokens::{peek::PeekTokenStream, Token, TokenLocalSpan},
};

use super::{
    display::AstDisplay,
    error::{ParseError, ParseResult},
    expect_token,
};

#[derive(PartialEq, Eq)]
enum Associativity {
    Left,
    Right,
}

#[derive(Debug)]
pub struct BinaryOperator {
    pub kind: BinaryOperatorKind,
    pub span: LocalSpan,
}

impl AstDisplay for BinaryOperator {
    fn fmt(&self, f: &mut super::display::AstFormatter<'_>) -> std::fmt::Result {
        f.node_single_child(&format!("{:?}", self.kind), &self.span)
    }
}

impl BinaryOperator {
    fn new_from_token(t: TokenLocalSpan) -> Option<Self> {
        Some(Self {
            kind: BinaryOperatorKind::new_from_token(t.token)?,
            span: t.span,
        })
    }
}

#[derive(Debug)]
pub enum BinaryOperatorKind {
    Addition,
    Subtraction,
    Multiplication,
    Division,
    Dot,
    LogicalAnd,
    LogicalOr,
    Equals,
    NotEquals,
}

impl BinaryOperatorKind {
    fn new_from_token(t: Token) -> Option<Self> {
        match t {
            Token::Star => Some(Self::Multiplication),
            Token::Slash => Some(Self::Division),
            Token::Plus => Some(Self::Addition),
            Token::Subtract => Some(Self::Subtraction),
            Token::Dot => Some(Self::Dot),
            Token::Equals => Some(Self::Equals),
            Token::NotEquals => Some(Self::NotEquals),
            Token::LogicalAnd => Some(Self::LogicalAnd),
            Token::LogicalOr => Some(Self::LogicalOr),
            _ => None,
        }
    }
}

type Precedence = usize;

fn get_token_precedence_associativity(token: Token) -> Option<(Precedence, Associativity)> {
    let precedence = match token {
        Token::Dot => Some(70),
        Token::LParen => Some(60),
        Token::Star | Token::Slash => Some(50),
        Token::Plus | Token::Subtract => Some(40),
        Token::Equals | Token::NotEquals => Some(30),
        Token::LogicalAnd => Some(20),
        Token::LogicalOr => Some(10),
        _ => None,
    }?;
    let associativity = Associativity::Left;
    Some((precedence, associativity))
}

#[derive(Debug)]
pub enum Expr {
    Ident(TokenLocalSpan),
    StringLiteral(TokenLocalSpan),
    IntegerLiteral(TokenLocalSpan),
    TrueLiteral(TokenLocalSpan),
    FalseLiteral(TokenLocalSpan),
    BinaryOp(BinaryOp),
    FnCall(FnCall),
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
            Token::DoubleQuoteLiteral => Self::StringLiteral(t),
            Token::True => Self::TrueLiteral(t),
            Token::False => Self::FalseLiteral(t),
            Token::LParen => {
                let expr = Self::parse(stream)?;
                expect_token(stream.parse_next()?, &[Token::RParen])?;
                expr
            }
            _ => {
                return Err(t
                    .span
                    .with_error(ParseError::ExpectedExpression(Some(t.token))))
            }
        };
        Ok(expr)
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
    fn fmt(&self, f: &mut super::display::AstFormatter<'_>) -> std::fmt::Result {
        let inner: &dyn super::display::AstDisplay = match self {
            Self::Ident(inner)
            | Self::StringLiteral(inner)
            | Self::IntegerLiteral(inner)
            | Self::TrueLiteral(inner)
            | Self::FalseLiteral(inner) => inner,
            Self::BinaryOp(inner) => inner,
            Self::FnCall(inner) => inner,
        };
        inner.fmt(f)
    }
}

#[derive(Debug)]
pub struct FnCall {
    pub callee: Box<Expr>,
    pub args: FnCallArgs,
}

impl super::display::AstDisplay for FnCall {
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
    fn fmt(&self, f: &mut super::display::AstFormatter<'_>) -> std::fmt::Result {
        let mut builder = f.node("FnCallArgs");
        for a in &self.args {
            builder.child(a);
        }
        builder.finish()
    }
}

#[derive(Debug)]
pub struct BinaryOp {
    pub left: Box<Expr>,
    pub op: BinaryOperator,
    pub right: Box<Expr>,
}

impl super::display::AstDisplay for BinaryOp {
    fn fmt(&self, f: &mut super::display::AstFormatter<'_>) -> std::fmt::Result {
        f.node("BinaryOp")
            .child(self.left.as_ref())
            .child(&self.op)
            .child(self.right.as_ref())
            .finish()
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
