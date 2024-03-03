use crate::{
    error::RainError,
    span::Span,
    tokens::{peek_stream::PeekTokenStream, NextTokenSpan, Token, TokenKind, TokenSpan},
};

use super::{ident::Ident, ParseError};

#[derive(Debug, PartialEq, Eq)]
pub struct Item<'a> {
    pub idents: Vec<Ident<'a>>,
    pub span: Span,
}

impl<'a> Item<'a> {
    pub fn parse(tokens: &[TokenSpan<'a>]) -> Result<Self, RainError> {
        let idents = tokens
            .iter()
            .filter_map(|t| match &t.token {
                Token::Ident(name) => Some(Ident { name, span: t.span }),
                Token::Dot => None,
                token => panic!("unexpected token {token:?}"),
            })
            .collect();
        let span = tokens
            .iter()
            .map(|ts| ts.span)
            .reduce(Span::combine)
            .unwrap();
        Ok(Self { idents, span })
    }

    pub fn parse_stream(stream: &mut PeekTokenStream<'a>) -> Result<Self, RainError> {
        let mut idents = Vec::default();
        let token = match stream.parse_next()? {
            NextTokenSpan::Next(token) => token,
            NextTokenSpan::End(span) => {
                return Err(RainError::new(ParseError::Expected(TokenKind::Ident), span));
            }
        };
        idents.push(Ident::parse(token)?);

        loop {
            let NextTokenSpan::Next(token) = stream.peek()? else {
                break;
            };
            if TokenKind::from(&token.token) != TokenKind::Dot {
                break;
            }
            // Consume the Dot we have just peeked
            stream.parse_next()?;
            let ident_token = match stream.parse_next()? {
                NextTokenSpan::Next(ident_token) => ident_token,
                NextTokenSpan::End(span) => {
                    return Err(RainError::new(ParseError::Expected(TokenKind::Ident), span));
                }
            };
            idents.push(Ident::parse(ident_token)?);
        }
        let span = Span::combine(idents.first().unwrap().span, idents.last().unwrap().span);
        Ok(Self { idents, span })
    }

    pub fn nosp(idents: Vec<Ident<'a>>) -> Self {
        Self {
            idents,
            span: Span::default(),
        }
    }

    pub fn reset_spans(&mut self) {
        self.idents.iter_mut().for_each(|ident| ident.span_reset());
        self.span.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_item(source: &str) -> Result<Item, RainError> {
        let mut stream = PeekTokenStream::new(source);
        let mut item = super::Item::parse_stream(&mut stream)?;
        item.reset_spans();
        Ok(item)
    }

    #[test]
    fn parse_single_ident() -> Result<(), RainError> {
        let item = parse_item("foo")?;
        assert_eq!(item, Item::nosp(vec![Ident::nosp("foo")],));
        Ok(())
    }

    #[test]
    fn parse_two_ident() -> Result<(), RainError> {
        let item = parse_item("foo.bar")?;
        assert_eq!(
            item,
            Item::nosp(vec![Ident::nosp("foo"), Ident::nosp("bar")],)
        );
        Ok(())
    }

    #[test]
    fn parse_three_ident() -> Result<(), RainError> {
        let item = parse_item("foo.bar.baz")?;
        assert_eq!(
            item,
            Item::nosp(vec![
                Ident::nosp("foo"),
                Ident::nosp("bar"),
                Ident::nosp("baz")
            ],)
        );
        Ok(())
    }
}
