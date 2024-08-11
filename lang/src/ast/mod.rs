pub mod display;
pub mod expr;

#[cfg(test)]
mod test;

use crate::{
    span::LocalSpan,
    tokens::{peek::PeekTokenStream, Token, TokenError, TokenLocalSpan},
};

#[derive(Debug)]
pub struct Script {
    pub declarations: Vec<Declaration>,
}

impl display::AstDisplay for Script {
    fn fmt(&self, f: &mut display::AstFormatter<'_>) -> std::fmt::Result {
        let mut builder = f.node("Script");
        for d in &self.declarations {
            builder.child(d);
        }
        builder.finish()
    }
}

impl Script {
    pub fn parse(stream: &mut PeekTokenStream) -> Result<Self, ParseError> {
        let mut declarations = Vec::new();
        while let Some(peek) = stream.peek()? {
            match peek.token {
                Token::NewLine => {
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
                    return Err(ParseError::ExpectedToken(
                        &[Token::Fn, Token::Let],
                        Some(peek),
                    ))
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

impl display::AstDisplay for Declaration {
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
    fn parse(stream: &mut PeekTokenStream) -> Result<Self, ParseError> {
        let let_token = expect_token(stream.parse_next()?, &[Token::Let])?;
        let name = expect_token(stream.parse_next()?, &[Token::Ident])?;
        let equals_token = expect_token(stream.parse_next()?, &[Token::Equals])?;
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
    // TODO: Args
    pub rparen_token: TokenLocalSpan,
    pub block: Block,
}

impl FnDeclare {
    fn parse(stream: &mut PeekTokenStream) -> Result<Self, ParseError> {
        let fn_token = expect_token(stream.parse_next()?, &[Token::Fn])?;
        let name = expect_token(stream.parse_next()?, &[Token::Ident])?;
        let lparen_token = expect_token(stream.parse_next()?, &[Token::LParen])?;
        let rparen_token = expect_token(stream.parse_next()?, &[Token::RParen])?;
        let block = Block::parse(stream)?;
        Ok(Self {
            fn_token,
            name,
            lparen_token,
            rparen_token,
            block,
        })
    }
}

impl display::AstDisplay for FnDeclare {
    fn fmt(&self, f: &mut display::AstFormatter<'_>) -> std::fmt::Result {
        f.node("FnDeclare")
            .child(&self.name)
            .child(&self.block)
            .finish()
    }
}

#[derive(Debug)]
pub struct Block {
    pub lbrace_token: LocalSpan,
    pub statements: Vec<Statement>,
    pub rbrace_token: LocalSpan,
}

impl Block {
    fn parse(stream: &mut PeekTokenStream) -> Result<Self, ParseError> {
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
}

impl Statement {
    fn parse(stream: &mut PeekTokenStream) -> Result<Self, ParseError> {
        expr::Expr::parse(stream).map(Self::Expr)
    }
}

impl display::AstDisplay for Statement {
    fn fmt(&self, f: &mut display::AstFormatter<'_>) -> std::fmt::Result {
        let inner: &dyn display::AstDisplay = match self {
            Self::Expr(inner) => inner,
        };
        inner.fmt(f)
    }
}

#[derive(Debug)]
pub enum ParseError {
    TokenError(TokenError),
    ExpectedToken(&'static [Token], Option<TokenLocalSpan>),
}

impl From<TokenError> for ParseError {
    fn from(err: TokenError) -> Self {
        Self::TokenError(err)
    }
}

fn expect_token(
    tls: Option<TokenLocalSpan>,
    expect: &'static [Token],
) -> Result<TokenLocalSpan, ParseError> {
    let Some(token) = tls else {
        return Err(ParseError::ExpectedToken(expect, tls));
    };
    if expect.contains(&token.token) {
        Ok(token)
    } else {
        Err(ParseError::ExpectedToken(expect, tls))
    }
}
