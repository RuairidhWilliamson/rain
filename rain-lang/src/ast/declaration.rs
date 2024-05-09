use crate::{
    error::RainError,
    span::Span,
    tokens::{peek_stream::PeekTokenStream, Token, TokenKind},
};

use super::{
    block::Block,
    expr::Expr,
    helpers::{NextTokenSpanHelpers, PeekTokenStreamHelpers as _},
    ident::Ident,
    visibility_specifier::VisibilitySpecifier,
    Ast, ParseError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declaration {
    pub comment: Option<String>,
    pub attributes: Vec<(Ident, Expr)>,
    pub visibility: Option<VisibilitySpecifier>,
    pub inner: InnerDeclaration,
}

impl Declaration {
    pub fn parse_stream(
        comment: Option<String>,
        visibility: Option<VisibilitySpecifier>,
        stream: &mut PeekTokenStream,
    ) -> Result<Self, RainError> {
        let peeking = stream.peek()?;
        let peeking_token = peeking
            .value()
            .ref_expect_not_end(ParseError::ExpectedStmt)?;
        match TokenKind::from(&peeking_token.token) {
            TokenKind::Let => {
                let let_lazy_token = peeking.consume().expect_next(TokenKind::Let)?;
                Ok(Self {
                    comment,
                    attributes: Vec::new(),
                    visibility,
                    inner: InnerDeclaration::Let(LetDeclaration::parse_stream(
                        let_lazy_token.span,
                        stream,
                    )?),
                })
            }
            TokenKind::Lazy => {
                let let_lazy_token = peeking.consume().expect_next(TokenKind::Lazy)?;
                Ok(Self {
                    comment,
                    attributes: Vec::new(),
                    visibility,
                    inner: InnerDeclaration::Lazy(LetDeclaration::parse_stream(
                        let_lazy_token.span,
                        stream,
                    )?),
                })
            }
            TokenKind::Fn => {
                let fn_token = peeking.consume().expect_next(TokenKind::Fn)?;
                Ok(Self {
                    comment,
                    attributes: Vec::new(),
                    visibility,
                    inner: InnerDeclaration::Function(FunctionDeclaration::parse_stream(
                        fn_token.span,
                        stream,
                    )?),
                })
            }
            TokenKind::Pub => {
                let visibility = VisibilitySpecifier::parse_stream(stream)?;
                Self::parse_stream(comment, Some(visibility), stream)
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
}

impl Ast for Declaration {
    fn span(&self) -> Span {
        self.inner.span()
    }

    fn reset_spans(&mut self) {
        for (k, v) in &mut self.attributes {
            k.reset_spans();
            v.reset_spans();
        }
        for v in &mut self.visibility {
            v.reset_spans();
        }
        self.inner.reset_spans();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InnerDeclaration {
    Let(LetDeclaration),
    Lazy(LetDeclaration),
    Function(FunctionDeclaration),
}

impl InnerDeclaration {
    pub fn name(&self) -> &str {
        match self {
            Self::Let(inner) => &inner.name.name,
            Self::Lazy(inner) => &inner.name.name,
            Self::Function(inner) => &inner.name.name,
        }
    }
}

impl Ast for InnerDeclaration {
    fn span(&self) -> Span {
        match self {
            Self::Let(inner) => inner.span(),
            Self::Lazy(inner) => inner.span(),
            Self::Function(inner) => inner.span(),
        }
    }

    fn reset_spans(&mut self) {
        match self {
            Self::Let(inner) => inner.reset_spans(),
            Self::Lazy(inner) => inner.reset_spans(),
            Self::Function(inner) => inner.reset_spans(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LetDeclaration {
    pub let_lazy_token: Span,
    pub name: Ident,
    pub equals_token: Span,
    pub value: Expr,
}

impl LetDeclaration {
    pub fn parse_stream(
        let_lazy_token: Span,
        stream: &mut PeekTokenStream,
    ) -> Result<Self, RainError> {
        let name = Ident::parse(stream.expect_parse_next(TokenKind::Ident)?)?;
        let equals_token = stream.expect_parse_next(TokenKind::Equals)?.span;
        let value = Expr::parse_stream(stream)?;
        Ok(Self {
            let_lazy_token,
            name,
            equals_token,
            value,
        })
    }
}

impl Ast for LetDeclaration {
    fn span(&self) -> Span {
        self.let_lazy_token.combine(self.equals_token)
    }

    fn reset_spans(&mut self) {
        self.let_lazy_token.reset();
        self.name.reset_spans();
        self.equals_token.reset();
        self.value.reset_spans();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionDeclaration {
    pub fn_token: Span,
    pub name: Ident,
    pub lparen_token: Span,
    pub args: Vec<FunctionArg>,
    pub rparen_token: Span,
    pub block: Block,
}

impl FunctionDeclaration {
    pub fn parse_stream(fn_token: Span, stream: &mut PeekTokenStream) -> Result<Self, RainError> {
        let name = Ident::parse(stream.expect_parse_next(TokenKind::Ident)?)?;
        let lparen_token = stream.expect_parse_next(TokenKind::LParen)?.span;
        let mut args = Vec::new();
        let rparen_token: Span;
        loop {
            let peeking = stream.peek()?;
            let token_span = peeking
                .consume()
                .expect_not_end(ParseError::Expected(TokenKind::RParen))?;
            match token_span.token.kind() {
                TokenKind::RParen => {
                    rparen_token = token_span.span;
                    break;
                }
                TokenKind::Ident => {
                    args.push(FunctionArg {
                        name: Ident::parse(token_span)?,
                    });
                }
                _ => {
                    return Err(RainError::new(
                        ParseError::ExpectedAny(&[TokenKind::Ident, TokenKind::RParen]),
                        token_span.span,
                    ));
                }
            }
            let peeking = stream.peek()?;
            let peeking_token_span = peeking
                .value()
                .ref_expect_not_end(ParseError::Expected(TokenKind::RParen))?;
            if peeking_token_span.token == Token::RParen {
                rparen_token = peeking.consume().expect_next(TokenKind::RParen)?.span;
                break;
            } else if peeking_token_span.token == Token::Comma {
                peeking.consume();
            }
        }
        let block = Block::parse_stream(stream)?;
        Ok(Self {
            fn_token,
            name,
            lparen_token,
            args,
            rparen_token,
            block,
        })
    }

    pub fn nosp(name: Ident, args: Vec<FunctionArg>, block: Block) -> Self {
        Self {
            fn_token: Span::default(),
            name,
            lparen_token: Span::default(),
            args,
            rparen_token: Span::default(),
            block,
        }
    }
}

impl Ast for FunctionDeclaration {
    fn span(&self) -> Span {
        self.fn_token.combine(self.block.span())
    }

    fn reset_spans(&mut self) {
        self.fn_token.reset();
        self.name.reset_spans();
        self.lparen_token.reset();
        for a in &mut self.args {
            a.reset_spans();
        }
        self.rparen_token.reset();
        self.block.reset_spans();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionArg {
    pub name: Ident,
}

impl FunctionArg {
    pub fn nosp(name: Ident) -> Self {
        Self { name }
    }
}

impl Ast for FunctionArg {
    fn span(&self) -> Span {
        self.name.span
    }

    fn reset_spans(&mut self) {
        self.name.reset_spans();
    }
}
