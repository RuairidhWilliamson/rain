use crate::{
    error::RainError,
    span::Span,
    tokens::{peek_stream::PeekTokenStream, NextTokenSpan, TokenKind},
};

use super::{helpers::PeekTokenStreamHelpers, ident::Ident, Ast, ParseError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item<'a> {
    pub idents: Vec<Ident<'a>>,
    pub span: Span,
}

impl<'a> Item<'a> {
    pub fn parse_stream(stream: &mut PeekTokenStream<'a>) -> Result<Self, RainError> {
        let mut idents = Vec::default();
        let token = stream.expect_parse_next(TokenKind::Ident)?;
        idents.push(Ident::parse(token)?);

        loop {
            let peeking = stream.peek()?;
            let NextTokenSpan::Next(token) = peeking.value() else {
                break;
            };
            if TokenKind::from(&token.token) != TokenKind::Dot {
                break;
            }
            // Consume the Dot we have just peeked
            peeking.consume();
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
}

impl Ast for Item<'_> {
    fn reset_spans(&mut self) {
        for i in &mut self.idents {
            i.reset_spans();
        }
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
