use crate::{
    span::LocalSpan,
    tokens::{peek::PeekTokenStream, Token},
};

use super::{expect_token, ParseError};

#[derive(Debug)]
pub enum Expr {
    Ident(LocalSpan),
    StringLiteral(LocalSpan),
    IntegerLiteral(LocalSpan),
    Dot(Dot),
    FnCall(FnCall),
}

impl Expr {
    pub fn parse(stream: &mut PeekTokenStream) -> Result<Self, ParseError> {
        if let Some(t) = stream.peek()? {
            match t.token {
                Token::Number => {
                    stream.parse_next()?;
                    return Ok(Self::IntegerLiteral(t.span));
                }
                Token::DoubleQuoteLiteral => {
                    stream.parse_next()?;
                    return Ok(Self::StringLiteral(t.span));
                }
                _ => (),
            }
        }
        let ident = expect_token(stream.parse_next()?, &[Token::Ident])?.span;
        let lparen_token = expect_token(stream.parse_next()?, &[Token::LParen])?.span;
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
        let rparen_token = expect_token(stream.parse_next()?, &[Token::RParen])?.span;
        Ok(Self::FnCall(FnCall {
            callee: Box::new(Self::Ident(ident)),
            args: FnCallArgs {
                lparen_token,
                args,
                rparen_token,
            },
        }))
    }
}

impl super::display::AstDisplay for Expr {
    fn fmt(&self, f: &mut super::display::AstFormatter<'_>) -> std::fmt::Result {
        let mut builder = f.node("Expr");
        match self {
            Self::Ident(inner) => builder.named_child("Ident", inner),
            Self::StringLiteral(inner) => builder.named_child("StringLiteral", inner),
            Self::IntegerLiteral(inner) => builder.named_child("IntegerLiteral", inner),
            Self::Dot(inner) => builder.child(inner),
            Self::FnCall(inner) => builder.child(inner),
        };
        builder.finish()
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
    pub lparen_token: LocalSpan,
    pub args: Vec<Expr>,
    pub rparen_token: LocalSpan,
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
pub struct Dot {
    pub parent: Box<Expr>,
    pub name: LocalSpan,
}

impl super::display::AstDisplay for Dot {
    fn fmt(&self, f: &mut super::display::AstFormatter<'_>) -> std::fmt::Result {
        f.node("Dot")
            .child(self.parent.as_ref())
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
    fn fn_call_no_args() {
        insta::assert_snapshot!(parse_display_expr("foo()"));
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
}
