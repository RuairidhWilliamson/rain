use crate::{
    span::LocalSpan,
    tokens::{peek::PeekTokenStream, Token, TokenLocalSpan},
};

use super::ParseError;

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

impl BinaryOperator {
    fn new_from_token(t: TokenLocalSpan) -> Option<Self> {
        Some(Self {
            kind: BinaryOperatorKind::new_from_token(t.token)?,
            span: t.span,
        })
    }

    fn precedence(&self) -> usize {
        self.kind.precedence()
    }

    fn associativity(&self) -> Associativity {
        self.kind.associativity()
    }
}

#[derive(Debug)]
pub enum BinaryOperatorKind {
    Addition,
    Subtraction,
    Multiplication,
    Division,
    Dot,
}

impl BinaryOperatorKind {
    fn new_from_token(t: Token) -> Option<Self> {
        match t {
            Token::Star => Some(Self::Multiplication),
            Token::Slash => Some(Self::Division),
            Token::Plus => Some(Self::Addition),
            Token::Subtract => Some(Self::Subtraction),
            Token::Dot => Some(Self::Dot),
            _ => None,
        }
    }

    fn associativity(&self) -> Associativity {
        match self {
            BinaryOperatorKind::Addition
            | BinaryOperatorKind::Subtraction
            | BinaryOperatorKind::Multiplication
            | BinaryOperatorKind::Division
            | BinaryOperatorKind::Dot => Associativity::Left,
        }
    }

    fn precedence(&self) -> usize {
        match self {
            Self::Dot => 40,
            Self::Multiplication | Self::Division => 30,
            Self::Addition | Self::Subtraction => 20,
        }
    }
}

#[derive(Debug)]
pub enum Expr {
    Ident(TokenLocalSpan),
    StringLiteral(TokenLocalSpan),
    IntegerLiteral(TokenLocalSpan),
    BinaryOp(BinaryOp),
    FnCall(FnCall),
}

impl Expr {
    pub fn parse(stream: &mut PeekTokenStream) -> Result<Self, ParseError> {
        let lhs = Self::parse_primary(stream)?;
        Self::parse_expr_ops(stream, lhs, 0)
    }

    fn parse_primary(stream: &mut PeekTokenStream) -> Result<Self, ParseError> {
        let Some(t) = stream.parse_next()? else {
            return Err(ParseError::ExpectedExpression(None));
        };
        match t.token {
            Token::Ident => Ok(Self::Ident(t)),
            Token::Number => Ok(Self::IntegerLiteral(t)),
            Token::DoubleQuoteLiteral => Ok(Self::StringLiteral(t)),
            _ => Err(ParseError::ExpectedExpression(Some(t))),
        }
    }

    fn parse_expr_ops(
        stream: &mut PeekTokenStream,
        mut lhs: Self,
        min_precedence: usize,
    ) -> Result<Self, ParseError> {
        while let Some(op) = Self::check_op(stream.peek()?, min_precedence) {
            stream.parse_next()?;
            let mut rhs = Self::parse_primary(stream)?;
            while let Some(next_op) = Self::check_op(stream.peek()?, op.precedence()) {
                // stream.parse_next()?;
                let next_precedence = op.precedence()
                    + if next_op.precedence() > op.precedence() {
                        1
                    } else {
                        0
                    };
                rhs = Self::parse_expr_ops(stream, rhs, next_precedence)?;
            }
            lhs = Self::BinaryOp(BinaryOp {
                left: Box::new(lhs),
                op,
                right: Box::new(rhs),
            });
        }
        Ok(lhs)
    }

    fn check_op(t: Option<TokenLocalSpan>, min_precedence: usize) -> Option<BinaryOperator> {
        let op = BinaryOperator::new_from_token(t?)?;
        if op.precedence() > min_precedence
            || op.precedence() == min_precedence && op.associativity() == Associativity::Right
        {
            Some(op)
        } else {
            None
        }
    }
}

impl super::display::AstDisplay for Expr {
    fn fmt(&self, f: &mut super::display::AstFormatter<'_>) -> std::fmt::Result {
        let inner: &dyn super::display::AstDisplay = match self {
            Self::Ident(inner) | Self::StringLiteral(inner) | Self::IntegerLiteral(inner) => inner,
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
            .child(&self.op.span)
            .child(self.right.as_ref())
            .finish()
    }
}

#[derive(Debug)]
pub struct Dot {
    pub left: Box<Expr>,
    pub dot_token: LocalSpan,
    pub name: LocalSpan,
}

impl super::display::AstDisplay for Dot {
    fn fmt(&self, f: &mut super::display::AstFormatter<'_>) -> std::fmt::Result {
        f.node("Dot")
            .child(self.left.as_ref())
            .child(&self.name)
            .finish()
    }
}

#[cfg(test)]
mod test {
    use crate::{ast::display::display_ast, tokens::peek::PeekTokenStream};

    use super::Expr;

    fn parse_display_expr(src: &str) -> String {
        let mut stream = PeekTokenStream::new(src);
        let s = Expr::parse(&mut stream).unwrap();
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

    #[ignore]
    #[test]
    fn maths_parens1() {
        insta::assert_snapshot!(parse_display_expr("1 - (a + 3) * 4"));
    }

    #[ignore]
    #[test]
    fn maths_parens2() {
        insta::assert_snapshot!(parse_display_expr("(3 - b) * c"));
    }

    #[ignore]
    #[test]
    fn fn_call_no_args() {
        insta::assert_snapshot!(parse_display_expr("foo()"));
    }

    #[ignore]
    #[test]
    fn fn_call_one_arg() {
        insta::assert_snapshot!(parse_display_expr("foo(1)"));
    }

    #[ignore]
    #[test]
    fn fn_call_two_arg() {
        insta::assert_snapshot!(parse_display_expr("foo(1, 2)"));
    }

    #[ignore]
    #[test]
    fn fn_call_two_arg_trailing_comma() {
        insta::assert_snapshot!(parse_display_expr("foo(1, 2,)"));
    }
}
