use crate::{
    span::LocalSpan,
    tokens::{peek::PeekTokenStream, Token, TokenLocalSpan},
};

use super::{expect_token, ParseError};

#[derive(Debug)]
pub enum Expr {
    Ident(TokenLocalSpan),
    StringLiteral(TokenLocalSpan),
    IntegerLiteral(TokenLocalSpan),
    BinaryOp(BinaryOp),
    Dot(Dot),
    FnCall(FnCall),
}

enum Associativity {
    Left,
    Right,
}

impl Expr {
    pub fn parse(stream: &mut PeekTokenStream) -> Result<Self, ParseError> {
        let mut output_stack: Vec<TokenLocalSpan> = Vec::new();
        let mut op_stack: Vec<TokenLocalSpan> = Vec::new();
        let t = expect_token(
            stream.parse_next()?,
            &[Token::Ident, Token::Number, Token::DoubleQuoteLiteral],
        )?;
        output_stack.push(t);
        loop {
            let Some(t) = stream.peek()? else {
                break;
            };
            match t.token {
                Token::Ident | Token::Number | Token::DoubleQuoteLiteral => {
                    stream.parse_next()?;
                    output_stack.push(t);
                }
                Token::Plus | Token::Star => {
                    stream.parse_next()?;
                    if let Some(op) = op_stack.last() {
                        if Self::operator_precedence(op.token) > Self::operator_precedence(t.token)
                            || Self::operator_precedence(op.token)
                                == Self::operator_precedence(t.token)
                                && matches!(
                                    Self::operator_associtivity(t.token),
                                    Associativity::Left
                                )
                        {
                            let Some(op) = op_stack.pop() else {
                                unreachable!();
                            };
                            output_stack.push(op);
                        }
                    }
                    op_stack.push(t);
                }
                _ => break,
            }
        }
        while let Some(op) = op_stack.pop() {
            output_stack.push(op);
        }
        Self::convert_output_stack(&mut output_stack)
    }

    fn convert_output_stack(output_stack: &mut Vec<TokenLocalSpan>) -> Result<Self, ParseError> {
        let t = output_stack.pop().unwrap();
        match t.token {
            Token::Ident => Ok(Self::Ident(t)),
            Token::Number => Ok(Self::IntegerLiteral(t)),
            Token::DoubleQuoteLiteral => Ok(Self::StringLiteral(t)),
            Token::Plus | Token::Star => {
                let right = Box::new(Self::convert_output_stack(output_stack)?);
                let left = Box::new(Self::convert_output_stack(output_stack)?);
                Ok(Self::BinaryOp(BinaryOp { left, op: t, right }))
            }
            _ => unreachable!("output stack contained {:?}", t),
        }
    }

    fn operator_precedence(t: Token) -> usize {
        match t {
            Token::Star | Token::Slash => 3,
            Token::Plus | Token::Dash => 2,
            _ => panic!("operator precedence not defined for {t:?}"),
        }
    }

    fn operator_associtivity(t: Token) -> Associativity {
        match t {
            Token::Star | Token::Slash | Token::Plus | Token::Dash => Associativity::Left,
            _ => panic!("operator associativity not defined for {t:?}"),
        }
    }

    pub fn parse2(stream: &mut PeekTokenStream) -> Result<Self, ParseError> {
        let t = expect_token(
            stream.parse_next()?,
            &[Token::Ident, Token::Number, Token::DoubleQuoteLiteral],
        )?;
        let left = match t.token {
            Token::Ident => Self::Ident(t),
            Token::Number => Self::IntegerLiteral(t),
            Token::DoubleQuoteLiteral => Self::StringLiteral(t),
            _ => unreachable!(),
        };
        let Some(t) = stream.peek()? else {
            return Ok(left);
        };
        match t.token {
            Token::Plus | Token::Star => {
                stream.parse_next()?;
                let op = t;
                let right = Self::parse(stream)?;
                Ok(Self::BinaryOp(BinaryOp {
                    left: Box::new(left),
                    op,
                    right: Box::new(right),
                }))
            }
            Token::Dot => {
                stream.parse_next()?;
                let name = expect_token(stream.parse_next()?, &[Token::Ident])?.span;
                Ok(Self::Dot(Dot {
                    left: Box::new(left),
                    dot_token: t.span,
                    name,
                }))
            }
            Token::LParen => {
                let lparen_token = expect_token(stream.parse_next()?, &[Token::LParen])?;
                let mut args = Vec::new();
                loop {
                    if let Some(t) = stream.peek()? {
                        if t.token == Token::RParen {
                            break;
                        }
                    }
                    let arg = Self::parse(stream)?;
                    args.push(arg);
                    if let Some(t) = stream.peek()? {
                        if t.token == Token::Comma {
                            stream.parse_next()?;
                        }
                    }
                }
                let rparen_token = expect_token(stream.parse_next()?, &[Token::RParen])?;
                Ok(Self::FnCall(FnCall {
                    callee: Box::new(left),
                    args: FnCallArgs {
                        lparen_token,
                        args,
                        rparen_token,
                    },
                }))
            }
            _ => Ok(left),
        }
    }
}

impl super::display::AstDisplay for Expr {
    fn fmt(&self, f: &mut super::display::AstFormatter<'_>) -> std::fmt::Result {
        let inner: &dyn super::display::AstDisplay = match self {
            Self::Ident(inner) | Self::StringLiteral(inner) | Self::IntegerLiteral(inner) => inner,
            Self::BinaryOp(inner) => inner,
            Self::Dot(inner) => inner,
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
    pub op: TokenLocalSpan,
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

    #[test]
    fn number_add() {
        insta::assert_snapshot!(parse_display_expr("1 + 1"));
    }

    #[test]
    fn number_add_left_associative() {
        insta::assert_snapshot!(parse_display_expr("1 + 2 + 3"));
    }

    #[test]
    fn number_star() {
        insta::assert_snapshot!(parse_display_expr("1 * 2"));
    }

    #[test]
    fn number_star_left_associative() {
        insta::assert_snapshot!(parse_display_expr("1 * 2 * 3"));
    }

    #[test]
    fn number_star_add_precedence() {
        insta::assert_snapshot!(parse_display_expr("5 * 2 + 3"));
    }

    #[test]
    fn number_star_add_precedence() {
        insta::assert_snapshot!(parse_display_expr("5 * 2 + 3"));
    }

    #[ignore]
    #[test]
    fn ident_dot_ident() {
        insta::assert_snapshot!(parse_display_expr("foo.bar"));
    }

    #[ignore]
    #[test]
    fn ident_dot_ident_dot_ident() {
        insta::assert_snapshot!(parse_display_expr("foo.bar.baz"));
    }
}
