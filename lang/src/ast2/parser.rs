use crate::{
    ast::error::{ParseError, ParseResult},
    local_span::ErrorLocalSpan,
    tokens::{peek::PeekTokenStream, Token, TokenLocalSpan},
};

use super::{
    AlternateCondition, BinaryOp, BinaryOperatorKind, Block, FnCall, FnDeclare, FnDeclareArg,
    IfCondition, LetDeclare, Module, ModuleRoot, Node, NodeId, StringLiteral,
};

pub fn parse_module<'a>(stream: &mut PeekTokenStream<'a>) -> ParseResult<Module> {
    let mut m = ModuleParser {
        nodes: Vec::new(),
        stream,
    };
    let module_root = m.parse_module_root()?;
    let root = m.push(Node::ModuleRoot(module_root));
    let ModuleParser { nodes, stream: _ } = m;
    Ok(Module { nodes, root })
}

struct ModuleParser<'src, 'stream> {
    nodes: Vec<Node>,
    stream: &'stream mut PeekTokenStream<'src>,
}

impl<'src, 'stream> ModuleParser<'src, 'stream> {
    fn push(&mut self, elem: impl Into<Node>) -> NodeId {
        let index = self.nodes.len();
        self.nodes.push(elem.into());
        NodeId(index)
    }

    fn parse_module_root(&mut self) -> ParseResult<ModuleRoot> {
        let mut declarations = Vec::new();
        while let Some(peek) = self.stream.peek()? {
            match peek.token {
                Token::NewLine | Token::Comment => {
                    self.stream.parse_next()?;
                    continue;
                }
                Token::Let => {
                    declarations.push(self.parse_let_declare()?);
                }
                Token::Fn => {
                    declarations.push(self.parse_fn_declare()?);
                }
                _ => {
                    return Err(peek
                        .span
                        .with_error(ParseError::ExpectedToken(&[Token::Fn, Token::Let])))
                }
            }
        }
        Ok(ModuleRoot { declarations })
    }

    fn parse_let_declare(&mut self) -> ParseResult<NodeId> {
        let let_token = expect_token(self.stream.parse_next()?, &[Token::Let])?;
        let name = expect_token(self.stream.parse_next()?, &[Token::Ident])?;
        let equals_token = expect_token(self.stream.parse_next()?, &[Token::Assign])?;
        let expr = self.parse_expr()?;
        Ok(self.push(LetDeclare {
            let_token,
            name,
            equals_token,
            expr,
        }))
    }

    fn parse_fn_declare(&mut self) -> ParseResult<NodeId> {
        let fn_token = expect_token(self.stream.parse_next()?, &[Token::Fn])?;
        let name = expect_token(self.stream.parse_next()?, &[Token::Ident])?;
        let lparen_token = expect_token(self.stream.parse_next()?, &[Token::LParen])?;
        let mut args = Vec::new();
        loop {
            let t = expect_token(self.stream.peek()?, &[Token::RParen, Token::Ident])?;
            match t.token {
                Token::RParen => break,
                Token::Ident => {}
                _ => unreachable!(),
            }
            self.stream.parse_next()?;
            args.push(FnDeclareArg { name: t });
            let t = expect_token(self.stream.peek()?, &[Token::RParen, Token::Comma])?;
            match t.token {
                Token::RParen => break,
                Token::Comma => {
                    self.stream.parse_next()?;
                }
                _ => unreachable!(),
            }
        }

        let rparen_token = expect_token(self.stream.parse_next()?, &[Token::RParen])?;
        let block = self.parse_block()?;
        Ok(self.push(FnDeclare {
            fn_token,
            name,
            lparen_token,
            args,
            rparen_token,
            block,
        }))
    }

    fn parse_block(&mut self) -> ParseResult<NodeId> {
        let lbrace_token = expect_token(self.stream.parse_next()?, &[Token::LBrace])?;
        let mut statements = Vec::new();
        while let Some(peek) = self.stream.peek()? {
            match peek.token {
                Token::NewLine => {
                    self.stream.parse_next()?;
                    continue;
                }
                Token::RBrace => break,
                _ => {
                    statements.push(self.parse_statement()?);
                }
            }
        }
        let rbrace_token = expect_token(self.stream.parse_next()?, &[Token::RBrace])?;
        Ok(self.push(Block {
            lbrace_token,
            statements,
            rbrace_token,
        }))
    }

    fn parse_statement(&mut self) -> ParseResult<NodeId> {
        if let Some([first, second]) = self.stream.peek_many()? {
            if first.token == Token::Ident && second.token == Token::Assign {
                return self.parse_assignment();
            }
        }
        self.parse_expr()
    }

    fn parse_assignment(&mut self) -> ParseResult<NodeId> {
        todo!("assignment")
    }

    fn parse_expr(&mut self) -> ParseResult<NodeId> {
        let lhs = self.parse_expr_primary()?;
        self.parse_expr_ops(lhs, 0)
    }

    fn parse_expr_primary(&mut self) -> ParseResult<NodeId> {
        let Some(t) = self.stream.parse_next()? else {
            return Err(ErrorLocalSpan::new(
                ParseError::ExpectedExpression(None),
                None,
            ));
        };
        let expr = match t.token {
            Token::Ident => self.push(Node::Ident(t)),
            Token::Number => self.push(Node::IntegerLiteral(t)),
            Token::DoubleQuoteLiteral(_) => self.push(StringLiteral(t)),
            Token::True => self.push(Node::TrueLiteral(t)),
            Token::False => self.push(Node::FalseLiteral(t)),
            Token::Internal => self.push(Node::Internal(t)),
            Token::LParen => {
                let expr = self.parse_expr()?;
                expect_token(self.stream.parse_next()?, &[Token::RParen])?;
                expr
            }
            Token::If => self.parse_if_condition(t)?,
            _ => {
                return Err(t
                    .span
                    .with_error(ParseError::ExpectedExpression(Some(t.token))))
            }
        };
        Ok(expr)
    }

    fn parse_expr_ops(&mut self, mut lhs: NodeId, min_precedence: usize) -> ParseResult<NodeId> {
        while let Some((t, precedence)) = check_op(self.stream.peek()?, min_precedence) {
            if t.token == Token::LParen {
                lhs = self.parse_fn_call(lhs)?;
                continue;
            }
            self.stream.parse_next()?;
            let mut rhs = self.parse_expr_primary()?;
            while let Some((_, next_op_precedence)) = check_op(self.stream.peek()?, precedence) {
                let next_precedence = precedence + usize::from(next_op_precedence > precedence);
                rhs = self.parse_expr_ops(rhs, next_precedence)?;
            }
            let Some(op) = BinaryOperatorKind::new_from_token(t.token) else {
                unreachable!()
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
        if let Some(peek) = self.stream.peek()? {
            if peek.token == Token::Else {
                let _ = self.stream.parse_next()?;
                alternate = Some(self.parse_alternate()?);
            }
        }
        Ok(self.push(IfCondition {
            condition,
            then_block,
            alternate,
        }))
    }

    fn parse_alternate(&mut self) -> ParseResult<AlternateCondition> {
        let peek = expect_token(self.stream.peek()?, &[Token::If, Token::LBrace])?;
        match peek.token {
            Token::If => {
                let _ = self.stream.parse_next()?;
                Ok(AlternateCondition::IfElseCondition(
                    self.parse_if_condition(peek)?,
                ))
            }
            Token::LBrace => Ok(AlternateCondition::ElseBlock(self.parse_block()?)),
            _ => unreachable!(),
        }
    }

    fn parse_fn_call(&mut self, lhs: NodeId) -> ParseResult<NodeId> {
        let lparen_token = self.stream.parse_next()?.unwrap();
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
        let rparen_token = expect_token(self.stream.parse_next()?, &[Token::RParen])?;
        Ok(self.push(FnCall {
            callee: lhs,
            lparen_token,
            args,
            rparen_token,
        }))
    }
}

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

#[derive(PartialEq, Eq)]
pub enum Associativity {
    Left,
    Right,
}

pub type Precedence = usize;

pub fn get_token_precedence_associativity(token: Token) -> Option<(Precedence, Associativity)> {
    let precedence = match token {
        Token::Dot => Some(70),
        Token::LParen => Some(60),
        Token::Star | Token::Slash => Some(50),
        Token::Plus | Token::Subtract => Some(40),
        Token::Equals | Token::NotEquals => Some(30),
        Token::LogicalAnd => Some(20),
        Token::LogicalOr => Some(10),
        _ => None,
    }?;
    let associativity = Associativity::Left;
    Some((precedence, associativity))
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
