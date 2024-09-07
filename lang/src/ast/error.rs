use crate::{
    error::ErrorSpan,
    tokens::{Token, TokenError},
};

pub type ParseResult<T> = Result<T, ErrorSpan<ParseError>>;

#[derive(Debug)]
pub enum ParseError {
    TokenError(TokenError),
    ExpectedToken(&'static [Token]),
    ExpectedExpression(Option<Token>),
}

impl From<TokenError> for ParseError {
    fn from(err: TokenError) -> Self {
        Self::TokenError(err)
    }
}

impl From<ErrorSpan<TokenError>> for ErrorSpan<ParseError> {
    fn from(ErrorSpan { err, span }: ErrorSpan<TokenError>) -> Self {
        Self {
            err: err.into(),
            span,
        }
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TokenError(err) => std::fmt::Display::fmt(err, f),
            Self::ExpectedToken(tokens) => f.write_fmt(format_args!("expected one of {tokens:?}")),
            Self::ExpectedExpression(token) => {
                #[allow(clippy::option_if_let_else)]
                let token: &str = if let Some(t) = token {
                    &format!("{t:?}")
                } else {
                    "EOF"
                };
                f.write_fmt(format_args!("unexpected {token:?}, expected expression"))
            }
        }
    }
}

impl std::error::Error for ParseError {}
