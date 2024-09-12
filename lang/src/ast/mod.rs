pub mod binary_op;
pub mod display;
pub mod error;
pub mod expr;

#[cfg(test)]
mod test;

use error::{ParseError, ParseResult};

use crate::{
    error::ErrorLocalSpan,
    local_span::LocalSpan,
    tokens::{peek::PeekTokenStream, Token, TokenLocalSpan},
};

#[derive(Debug)]
pub struct Script {
    pub declarations: Vec<Declaration>,
}

impl display::AstDisplay for Script {
    fn span(&self) -> LocalSpan {
        LocalSpan::span_iter(self.declarations.iter().map(display::AstDisplay::span))
    }

    fn fmt(&self, f: &mut display::AstFormatter<'_>) -> std::fmt::Result {
        let mut builder = f.node("Script");
        for d in &self.declarations {
            builder.child(d);
        }
        builder.finish()
    }
}

impl Script {
    pub fn parse(stream: &mut PeekTokenStream) -> ParseResult<Self> {
        let mut declarations = Vec::new();
        while let Some(peek) = stream.peek()? {
            match peek.token {
                Token::NewLine | Token::Comment => {
                    stream.parse_next()?;
                    continue;
                }
                Token::Let => {
                    declarations.push(LetDeclare::parse(stream)?.into());
                }
                Token::Fn => {
                    declarations.push(FnDeclare::parse(stream)?.into());
                }
                _ => {
                    return Err(peek
                        .span
                        .with_error(ParseError::ExpectedToken(&[Token::Fn, Token::Let])))
                }
            }
        }
        Ok(Self { declarations })
    }
}

#[derive(Debug)]
pub enum Declaration {
    LetDeclare(LetDeclare),
    FnDeclare(FnDeclare),
}

impl Declaration {
    pub fn name(&self) -> TokenLocalSpan {
        match self {
            Self::LetDeclare(declare) => declare.name,
            Self::FnDeclare(declare) => declare.name,
        }
    }
}

impl display::AstDisplay for Declaration {
    fn span(&self) -> LocalSpan {
        match self {
            Self::LetDeclare(inner) => inner.span(),
            Self::FnDeclare(inner) => inner.span(),
        }
    }

    fn fmt(&self, f: &mut display::AstFormatter<'_>) -> std::fmt::Result {
        let inner: &dyn display::AstDisplay = match self {
            Self::LetDeclare(inner) => inner,
            Self::FnDeclare(inner) => inner,
        };
        inner.fmt(f)
    }
}

impl From<LetDeclare> for Declaration {
    fn from(value: LetDeclare) -> Self {
        Self::LetDeclare(value)
    }
}

impl From<FnDeclare> for Declaration {
    fn from(value: FnDeclare) -> Self {
        Self::FnDeclare(value)
    }
}

#[derive(Debug)]
pub struct LetDeclare {
    pub let_token: TokenLocalSpan,
    pub name: TokenLocalSpan,
    pub equals_token: TokenLocalSpan,
    pub expr: expr::Expr,
}

impl LetDeclare {
    fn parse(stream: &mut PeekTokenStream) -> ParseResult<Self> {
        let let_token = expect_token(stream.parse_next()?, &[Token::Let])?;
        let name = expect_token(stream.parse_next()?, &[Token::Ident])?;
        let equals_token = expect_token(stream.parse_next()?, &[Token::Assign])?;
        let expr = expr::Expr::parse(stream)?;
        Ok(Self {
            let_token,
            name,
            equals_token,
            expr,
        })
    }
}

impl display::AstDisplay for LetDeclare {
    fn span(&self) -> LocalSpan {
        self.let_token.span + self.expr.span()
    }

    fn fmt(&self, f: &mut display::AstFormatter<'_>) -> std::fmt::Result {
        f.node("LetDeclare")
            .child(&self.name)
            .child(&self.expr)
            .finish()
    }
}

#[derive(Debug)]
pub struct FnDeclare {
    pub fn_token: TokenLocalSpan,
    pub name: TokenLocalSpan,
    pub lparen_token: TokenLocalSpan,
    pub args: Vec<FnDeclareArg>,
    pub rparen_token: TokenLocalSpan,
    pub block: Block,
}

impl FnDeclare {
    fn parse(stream: &mut PeekTokenStream) -> ParseResult<Self> {
        let fn_token = expect_token(stream.parse_next()?, &[Token::Fn])?;
        let name = expect_token(stream.parse_next()?, &[Token::Ident])?;
        let lparen_token = expect_token(stream.parse_next()?, &[Token::LParen])?;
        let mut args = Vec::new();
        loop {
            let t = expect_token(stream.peek()?, &[Token::RParen, Token::Ident])?;
            match t.token {
                Token::RParen => break,
                Token::Ident => {}
                _ => unreachable!(),
            }
            stream.parse_next()?;
            args.push(FnDeclareArg { name: t });
            let t = expect_token(stream.peek()?, &[Token::RParen, Token::Comma])?;
            match t.token {
                Token::RParen => break,
                Token::Comma => {
                    stream.parse_next()?;
                }
                _ => unreachable!(),
            }
        }

        let rparen_token = expect_token(stream.parse_next()?, &[Token::RParen])?;
        let block = Block::parse(stream)?;
        Ok(Self {
            fn_token,
            name,
            lparen_token,
            args,
            rparen_token,
            block,
        })
    }
}

impl display::AstDisplay for FnDeclare {
    fn span(&self) -> LocalSpan {
        self.fn_token.span + self.block.span()
    }

    fn fmt(&self, f: &mut display::AstFormatter<'_>) -> std::fmt::Result {
        f.node("FnDeclare")
            .child(&self.name)
            .children(&self.args)
            .child(&self.block)
            .finish()
    }
}

#[derive(Debug)]
pub struct FnDeclareArg {
    pub name: TokenLocalSpan,
}

impl display::AstDisplay for FnDeclareArg {
    fn span(&self) -> LocalSpan {
        self.name.span
    }

    fn fmt(&self, f: &mut display::AstFormatter<'_>) -> std::fmt::Result {
        f.node("Arg").child(&self.name).finish()
    }
}

#[derive(Debug)]
pub struct Block {
    pub lbrace_token: LocalSpan,
    pub statements: Vec<Statement>,
    pub rbrace_token: LocalSpan,
}

impl Block {
    fn parse(stream: &mut PeekTokenStream) -> ParseResult<Self> {
        let lbrace_token = expect_token(stream.parse_next()?, &[Token::LBrace])?.span;
        let mut statements = Vec::new();
        while let Some(peek) = stream.peek()? {
            match peek.token {
                Token::NewLine => {
                    stream.parse_next()?;
                    continue;
                }
                Token::RBrace => break,
                _ => {
                    statements.push(Statement::parse(stream)?);
                }
            }
        }
        let rbrace_token = expect_token(stream.parse_next()?, &[Token::RBrace])?.span;
        Ok(Self {
            lbrace_token,
            statements,
            rbrace_token,
        })
    }
}

impl display::AstDisplay for Block {
    fn span(&self) -> LocalSpan {
        self.lbrace_token + self.rbrace_token
    }

    fn fmt(&self, f: &mut display::AstFormatter<'_>) -> std::fmt::Result {
        let mut builder = f.node("Block");
        for s in &self.statements {
            builder.child(s);
        }
        builder.finish()
    }
}

#[derive(Debug)]
pub enum Statement {
    Expr(expr::Expr),
    Assignment(Assignment),
}

impl Statement {
    fn parse(stream: &mut PeekTokenStream) -> ParseResult<Self> {
        if let Some([first, second]) = stream.peek_many()? {
            if first.token == Token::Ident && second.token == Token::Assign {
                return Assignment::parse(stream).map(Self::Assignment);
            }
        }
        expr::Expr::parse(stream).map(Self::Expr)
    }
}

impl display::AstDisplay for Statement {
    fn span(&self) -> LocalSpan {
        match self {
            Self::Expr(inner) => inner.span(),
            Self::Assignment(inner) => inner.span(),
        }
    }

    fn fmt(&self, f: &mut display::AstFormatter<'_>) -> std::fmt::Result {
        let inner: &dyn display::AstDisplay = match self {
            Self::Expr(inner) => inner,
            Self::Assignment(inner) => inner,
        };
        inner.fmt(f)
    }
}

#[derive(Debug)]
pub struct Assignment {
    pub name: TokenLocalSpan,
    pub equals_token: TokenLocalSpan,
    pub expr: expr::Expr,
}

impl Assignment {
    fn parse(stream: &mut PeekTokenStream) -> ParseResult<Self> {
        let name = expect_token(stream.parse_next()?, &[Token::Ident])?;
        let equals_token = expect_token(stream.parse_next()?, &[Token::Assign])?;
        let expr = expr::Expr::parse(stream)?;
        Ok(Self {
            name,
            equals_token,
            expr,
        })
    }
}

impl display::AstDisplay for Assignment {
    fn span(&self) -> LocalSpan {
        self.name.span + self.expr.span()
    }

    fn fmt(&self, f: &mut display::AstFormatter<'_>) -> std::fmt::Result {
        f.node("Assignment")
            .child(&self.name)
            .child(&self.expr)
            .finish()
    }
}

fn expect_token(
    tls: Option<TokenLocalSpan>,
    expect: &'static [Token],
) -> ParseResult<TokenLocalSpan> {
    let Some(token) = tls else {
        return Err(ErrorLocalSpan::new(ParseError::ExpectedToken(expect), None));
    };
    if expect.contains(&token.token) {
        Ok(token)
    } else {
        Err(token.span.with_error(ParseError::ExpectedToken(expect)))
    }
}
