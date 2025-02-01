use crate::{
    local_span::ErrorLocalSpan,
    tokens::{Token, TokenError},
};

pub type ParseResult<T> = Result<T, ErrorLocalSpan<ParseError>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    TokenError(TokenError),
    ExpectedToken(&'static [Token]),
    ExpectedTokenAfter(&'static [Token]),
    UnmatchedPair(Token),
    ExpectedExpression,
    InputNotFullConsumed,
}

impl From<TokenError> for ParseError {
    fn from(err: TokenError) -> Self {
        Self::TokenError(err)
    }
}

impl From<ErrorLocalSpan<TokenError>> for ErrorLocalSpan<ParseError> {
    fn from(ErrorLocalSpan { err, span }: ErrorLocalSpan<TokenError>) -> Self {
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
            Self::ExpectedToken(tokens) if tokens.len() == 1 => {
                f.write_fmt(format_args!("bad syntax: expected {:?}", tokens[0]))
            }
            Self::ExpectedToken(tokens) => {
                f.write_fmt(format_args!("bad syntax: expected one of {tokens:?}"))
            }
            Self::ExpectedTokenAfter(tokens) => {
                f.write_fmt(format_args!("bad syntax: expected one of {tokens:?} after"))
            }
            Self::UnmatchedPair(token) => {
                f.write_fmt(format_args!("bad syntax: unmatched pair {token:?}"))
            }
            Self::ExpectedExpression => f.write_str("bad syntax: expected expression"),
            Self::InputNotFullConsumed => f.write_str("bad syntax: input not fully consumed"),
        }
    }
}

impl std::error::Error for ParseError {}
