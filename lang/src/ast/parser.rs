use crate::{
    ast::{
        AlternateCondition, Assignment, BinaryOp, BinaryOperatorKind, Block, Closure, Declare,
        DeclareName, DeclareNameListElement, DeclareNameSingle, DeclareNamedDestructure, FnCall,
        FnDeclareArg, Ident, IfCondition, IntegerLiteral, List, ListElement, Module, ModuleRoot,
        Node, NodeId, NodeList, Not, Record, RecordField, SimpleLiteralKind, StringLiteral,
        TypeSpec,
        error::{ParseError, ParseResult},
    },
    local_span::ErrorLocalSpan,
    tokens::{Token, TokenLocalSpan, peek::PeekTokenStream},
};

pub fn parse_module_inner(source: &str) -> ParseResult<Module> {
    let mut parser = ModuleParser::new(source);
    let root = parser.parse_module_root()?;
    let nodes = parser.complete()?;
    Ok(Module { root, nodes })
}

struct ModuleParser<'src> {
    nodes: NodeList,
    stream: PeekTokenStream<'src>,
}

impl<'src> ModuleParser<'src> {
    pub fn new(s: &'src str) -> Self {
        Self {
            nodes: NodeList::new(),
            stream: PeekTokenStream::new(s),
        }
    }

    pub fn complete(mut self) -> Result<NodeList, ErrorLocalSpan<ParseError>> {
        if let Some(tls) = self.stream.parse_next()? {
            Err(tls.span.with_error(ParseError::InputNotFullyConsumed))
        } else {
            Ok(self.nodes)
        }
    }

    fn push(&mut self, node: impl Into<Node>) -> NodeId {
        self.nodes.push(node)
    }

    fn parse_module_root(&mut self) -> ParseResult<ModuleRoot> {
        let mut declarations = Vec::new();
        while let Some([peek1, peek2]) = self.stream.peek_many::<2>()? {
            let peek = if peek1.token == Token::Pub {
                peek2
            } else {
                peek1
            };
            match peek.token {
                Token::NewLine | Token::Comment => {
                    self.stream.parse_next()?;
                }
                Token::Let => {
                    declarations.push(self.parse_let_declare()?);
                }
                _ => {
                    return Err(peek
                        .span
                        .with_error(ParseError::ExpectedToken(&[Token::Let])));
                }
            }
        }
        // Consume trailing new line
        if let Some(t) = self.stream.peek()?
            && let Token::NewLine | Token::Comment = t.token
        {
            self.stream.parse_next()?;
        }
        Ok(ModuleRoot { declarations })
    }

    fn parse_let_declare(&mut self) -> ParseResult<Declare> {
        let token = self.stream.expect_parse_next(&[Token::Pub, Token::Let])?;
        let (pub_token, let_token) = if token.token == Token::Pub {
            (
                Some(token.span),
                self.stream.expect_parse_next(&[Token::Let])?.span,
            )
        } else {
            (None, token.span)
        };

        let assignment = self.parse_assignment()?;
        Ok(Declare {
            pub_token,
            let_token,
            assignment,
        })
    }

    fn parse_fn_declare(&mut self, fn_token: TokenLocalSpan) -> ParseResult<NodeId> {
        let lparen_token = self.stream.expect_parse_next(&[Token::LParen])?.span;
        let mut args = Vec::new();
        loop {
            let t = self.stream.expect_peek(&[Token::RParen, Token::Ident])?;
            match t.token {
                Token::RParen => break,
                Token::Ident => {}
                _ => unreachable!("parse fn declare rparen"),
            }
            self.stream.parse_next()?;
            let name = t.span;
            let mut t = self
                .stream
                .expect_peek(&[Token::RParen, Token::Comma, Token::Colon])?;
            if t.token == Token::Colon {
                self.stream.parse_next()?;
                let expr = self.parse_expr()?;
                args.push(FnDeclareArg {
                    name,
                    type_spec: Some(TypeSpec {
                        colon_token: t.span,
                        type_expr: expr,
                    }),
                });
                t = self.stream.expect_peek(&[Token::RParen, Token::Comma])?;
            } else {
                args.push(FnDeclareArg {
                    name,
                    type_spec: None,
                });
            }
            match t.token {
                Token::RParen => {
                    break;
                }
                Token::Comma => {
                    self.stream.parse_next()?;
                }
                _ => unreachable!("parse fn declare rparen"),
            }
        }

        let rparen_token = self.stream.expect_parse_next(&[Token::RParen])?.span;
        let mut return_type = None;
        if let Some(peek) = self.stream.peek()?
            && peek.token == Token::ReturnType
        {
            let Some(arrow) = self.stream.parse_next()? else {
                unreachable!()
            };
            debug_assert_eq!(arrow.token, Token::ReturnType);
            let expr = self.parse_expr()?;
            return_type = Some(super::ClosureReturnTypeSpec {
                return_type_arrow: arrow.span,
                type_expr: expr,
            });
        }
        let block = self.parse_block()?;
        Ok(self.push(Closure {
            fn_token: fn_token.span,
            lparen_token,
            args,
            rparen_token,
            return_type,
            block,
        }))
    }

    fn parse_block(&mut self) -> ParseResult<NodeId> {
        let lbrace_token = self.stream.expect_parse_next(&[Token::LBrace])?.span;
        let mut statements = Vec::new();
        let mut expecting_statement = true;
        while let Some(peek) = self.stream.peek()? {
            match peek.token {
                Token::NewLine | Token::Comment => {
                    self.stream.parse_next()?;
                    expecting_statement = true;
                }
                Token::RBrace => break,
                _ if expecting_statement => {
                    statements.push(self.parse_statement()?);
                    expecting_statement = false;
                }
                _ => {
                    return Err(peek
                        .span
                        .with_error(ParseError::ExpectedToken(&[Token::NewLine, Token::RBrace])));
                }
            }
        }
        let rbrace_token = self.stream.expect_parse_next(&[Token::RBrace])?.span;
        Ok(self.push(Block {
            lbrace_token,
            statements,
            rbrace_token,
        }))
    }

    fn parse_statement(&mut self) -> ParseResult<NodeId> {
        if let Some([first, second]) = self.stream.peek_many()?
            && first.token == Token::Ident
            && second.token == Token::Assign
        {
            let assignment = self.parse_assignment()?;
            return Ok(self.push(assignment));
        }
        self.parse_expr()
    }

    fn parse_assignment(&mut self) -> ParseResult<Assignment> {
        let token = self
            .stream
            .expect_parse_next(&[Token::Ident, Token::LBrace])?;
        let name = match token.token {
            Token::Ident => {
                let name = token.span;
                let peek = self.stream.expect_peek(&[Token::Colon, Token::Assign])?;
                let type_spec = if peek.token == Token::Colon {
                    let colon_token = self.stream.expect_parse_next(&[Token::Colon])?.span;
                    Some(TypeSpec {
                        colon_token,
                        type_expr: self.parse_expr()?,
                    })
                } else {
                    None
                };
                DeclareName::Single(DeclareNameSingle { name, type_spec })
            }
            Token::LBrace => {
                let lbrace = token.span;
                let mut elements = Vec::new();
                loop {
                    self.stream.skip_if_newline_or_comment()?;
                    let Some(peek) = self.stream.peek()? else {
                        break;
                    };
                    if peek.token == Token::RBrace {
                        break;
                    }
                    let name = self.stream.expect_parse_next(&[Token::Ident])?.span;
                    let peek =
                        self.stream
                            .expect_peek(&[Token::Colon, Token::Comma, Token::RBrace])?;
                    let type_spec = if peek.token == Token::Colon {
                        let colon_token = self.stream.expect_parse_next(&[Token::Colon])?.span;
                        Some(TypeSpec {
                            colon_token,
                            type_expr: self.parse_expr()?,
                        })
                    } else {
                        None
                    };
                    let peek = self.stream.expect_peek(&[Token::Comma, Token::RBrace])?;
                    match peek.token {
                        Token::RBrace => {
                            elements.push(DeclareNameListElement {
                                name,
                                type_spec,
                                comma: None,
                            });
                            break;
                        }
                        Token::Comma => {
                            let comma = Some(self.stream.expect_parse_next(&[Token::Comma])?.span);
                            elements.push(DeclareNameListElement {
                                name,
                                type_spec,
                                comma,
                            });
                        }
                        _ => unreachable!(),
                    }
                }
                let rbrace = self.stream.expect_parse_next(&[Token::RBrace])?.span;
                DeclareName::NamedDestructure(DeclareNamedDestructure {
                    lbrace,
                    elements,
                    rbrace,
                })
            }
            _ => unreachable!(),
        };
        let equals_token = self.stream.expect_parse_next(&[Token::Assign])?.span;
        let expr = self.parse_expr()?;
        Ok(Assignment {
            name,
            equals_token,
            expr,
        })
    }

    fn parse_expr(&mut self) -> ParseResult<NodeId> {
        let (mut prefixes, mut lhs) = self.parse_expr_primary()?;
        loop {
            let min_precedence = prefixes.last().map_or(0, |t| {
                get_token_precedence_associativity(t.token)
                    .expect("not has precedence")
                    .0
            });
            lhs = self.parse_expr_ops(lhs, min_precedence)?;
            if let Some(prefix) = prefixes.pop() {
                debug_assert_eq!(prefix.token, Token::Excalmation);
                lhs = self.push(Not {
                    exclamation: prefix.span,
                    inner: lhs,
                });
                continue;
            }
            break;
        }
        Ok(lhs)
    }

    fn parse_expr_primary(&mut self) -> ParseResult<(Vec<TokenLocalSpan>, NodeId)> {
        let Some(t) = self.stream.parse_next()? else {
            return Err(self
                .stream
                .last_span()
                .with_error(ParseError::ExpectedExpression));
        };
        let expr = match t.token {
            Token::Fn => self.parse_fn_declare(t)?,
            Token::Ident => self.push(Ident(t.span)),
            Token::Number => self.push(IntegerLiteral(t.span)),
            Token::DoubleQuoteLiteral(prefix) => {
                let mut contents = t.span;
                if prefix.is_some() {
                    contents.start += 2;
                } else {
                    contents.start += 1;
                }
                contents.end -= 1;
                self.push(StringLiteral { prefix, contents })
            }
            Token::True => self.push(SimpleLiteralKind::True.with(t.span)),
            Token::False => self.push(SimpleLiteralKind::False.with(t.span)),
            Token::Internal => self.push(SimpleLiteralKind::Internal.with(t.span)),
            Token::Import => self.push(SimpleLiteralKind::Import.with(t.span)),
            Token::Stdlib => self.push(SimpleLiteralKind::Stdlib.with(t.span)),
            Token::ThisFile => self.push(SimpleLiteralKind::ThisFile.with(t.span)),
            Token::LParen => {
                let expr = self.parse_expr()?;
                self.stream.expect_parse_next(&[Token::RParen])?;
                expr
            }
            Token::If => self.parse_if_condition(t)?,
            Token::LBrace => self.parse_record(t)?,
            Token::LSqBracket => self.parse_list(t)?,
            Token::Excalmation => {
                let exclamation = t;
                let (mut prefix, inner) = self.parse_expr_primary()?;
                prefix.push(exclamation);
                return Ok((prefix, inner));
            }
            _ => return Err(t.span.with_error(ParseError::ExpectedExpression)),
        };
        Ok((Vec::new(), expr))
    }

    fn parse_expr_ops(&mut self, mut lhs: NodeId, min_precedence: usize) -> ParseResult<NodeId> {
        fn check_op(
            t: Option<TokenLocalSpan>,
            min_precedence: usize,
        ) -> Option<(TokenLocalSpan, Precedence)> {
            let t = t?;
            let (precedence, associativity) = get_token_precedence_associativity(t.token)?;
            if precedence > min_precedence
                || precedence == min_precedence && associativity == Associativity::Right
            {
                Some((t, precedence))
            } else {
                None
            }
        }
        while let Some((t, precedence)) = check_op(self.stream.peek()?, min_precedence) {
            if t.token == Token::LParen {
                lhs = self.parse_fn_call(lhs)?;
                continue;
            }
            self.stream.parse_next()?;
            let (mut prefixes, mut rhs) = self.parse_expr_primary()?;
            loop {
                if let Some(prefix) = prefixes.last()
                    && let Some((prefix_precedence, _)) =
                        get_token_precedence_associativity(prefix.token)
                    && let Some((_, precedence)) = check_op(self.stream.peek()?, min_precedence)
                    && prefix_precedence > precedence
                {
                    debug_assert_eq!(prefix.token, Token::Excalmation);
                    rhs = self.push(Not {
                        exclamation: prefix.span,
                        inner: rhs,
                    });
                    prefixes.pop();
                    continue;
                }
                break;
            }
            while let Some((_, next_op_precedence)) = check_op(self.stream.peek()?, precedence) {
                let next_precedence = precedence + usize::from(next_op_precedence > precedence);
                rhs = self.parse_expr_ops(rhs, next_precedence)?;
            }
            for prefix in prefixes {
                debug_assert_eq!(prefix.token, Token::Excalmation);
                rhs = self.push(Not {
                    exclamation: prefix.span,
                    inner: rhs,
                });
            }
            let Some(op) = BinaryOperatorKind::new_from_token(t.token) else {
                return Err(t.span.with_error(ParseError::InvalidBinaryOperator));
            };
            lhs = self.push(BinaryOp {
                left: lhs,
                op,
                op_span: t.span,
                right: rhs,
            });
        }
        Ok(lhs)
    }

    fn parse_if_condition(&mut self, if_token: TokenLocalSpan) -> ParseResult<NodeId> {
        debug_assert_eq!(if_token.token, Token::If);
        let condition = self.parse_expr()?;
        let then_block = self.parse_block()?;
        let mut alternate = None;
        if let Some(peek) = self.stream.peek()?
            && peek.token == Token::Else
        {
            let _ = self.stream.parse_next()?;
            alternate = Some(self.parse_alternate()?);
        }
        Ok(self.push(IfCondition {
            condition,
            then_block,
            alternate,
        }))
    }

    fn parse_alternate(&mut self) -> ParseResult<AlternateCondition> {
        let peek = self.stream.expect_peek(&[Token::If, Token::LBrace])?;
        match peek.token {
            Token::If => {
                let _ = self.stream.parse_next()?;
                Ok(AlternateCondition::IfElseCondition(
                    self.parse_if_condition(peek)?,
                ))
            }
            Token::LBrace => Ok(AlternateCondition::ElseBlock(self.parse_block()?)),
            _ => unreachable!("parse_alternate"),
        }
    }

    fn parse_fn_call(&mut self, lhs: NodeId) -> ParseResult<NodeId> {
        let lparen_token = self.stream.expect_parse_next(&[Token::LParen])?.span;
        let mut args = Vec::new();
        loop {
            let Some(t) = self.stream.peek()? else {
                break;
            };
            if t.token == Token::RParen {
                break;
            }
            args.push(self.parse_expr()?);
            let Some(t) = self.stream.peek()? else {
                break;
            };
            match t.token {
                Token::Comma => {
                    self.stream.parse_next()?;
                }
                _ => break,
            }
        }
        let rparen_token = self.stream.expect_parse_next(&[Token::RParen])?.span;
        Ok(self.push(FnCall {
            callee: lhs,
            lparen_token,
            args,
            rparen_token,
        }))
    }

    fn parse_record(&mut self, lbrace: TokenLocalSpan) -> ParseResult<NodeId> {
        let lbrace = lbrace.span;
        let mut fields = Vec::new();
        loop {
            self.stream.skip_if_newline_or_comment()?;
            let Some(peek) = self.stream.peek()? else {
                break;
            };
            if peek.token == Token::RBrace {
                break;
            }
            let key = self.stream.expect_parse_next(&[Token::Ident])?.span;
            let equals = self.stream.expect_parse_next(&[Token::Assign])?.span;
            let value = self.parse_expr()?;
            let mut comma = None;
            if let Some(tls) = self.stream.peek()?
                && tls.token == Token::Comma
            {
                comma = Some(self.stream.expect_parse_next(&[Token::Comma])?.span);
            }
            fields.push(RecordField {
                key,
                equals,
                value,
                comma,
            });
        }
        let rbrace = self.stream.expect_parse_next(&[Token::RBrace])?.span;
        Ok(self.push(Record {
            lbrace,
            fields,
            rbrace,
        }))
    }

    fn parse_list(&mut self, lbracket: TokenLocalSpan) -> ParseResult<NodeId> {
        let lbracket = lbracket.span;
        let mut elements = Vec::new();
        loop {
            self.stream.skip_if_newline_or_comment()?;
            let Some(peek) = self.stream.peek()? else {
                break;
            };
            if peek.token == Token::RSqBracket {
                break;
            }
            let value = self.parse_expr()?;
            self.stream.skip_if_newline()?;
            let Some(tls) = self.stream.peek()? else {
                break;
            };
            if tls.token == Token::Comma {
                let comma = Some(self.stream.expect_parse_next(&[Token::Comma])?.span);
                elements.push(ListElement { value, comma });
            } else {
                elements.push(ListElement { value, comma: None });
                break;
            }
        }
        let rbracket = self.stream.expect_parse_next(&[Token::RSqBracket])?.span;
        Ok(self.push(List {
            lsqbracket: lbracket,
            elements,
            rsqbracket: rbracket,
        }))
    }
}

#[derive(PartialEq, Eq)]
pub enum Associativity {
    Left,
    Right,
}

pub type Precedence = usize;

pub fn get_token_precedence_associativity(token: Token) -> Option<(Precedence, Associativity)> {
    let precedence = match token {
        Token::Dot => Some(120),
        Token::LParen => Some(110),
        Token::Excalmation => Some(100),
        Token::Star | Token::Slash => Some(50),
        Token::Plus | Token::Subtract => Some(40),
        Token::LAngle | Token::RAngle | Token::LessEq | Token::GreaterEq => Some(35),
        Token::Equals | Token::NotEquals => Some(30),
        Token::LogicalAnd => Some(20),
        Token::LogicalOr => Some(10),
        _ => None,
    }?;
    let associativity = Associativity::Left;
    Some((precedence, associativity))
}
