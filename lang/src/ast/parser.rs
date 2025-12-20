use crate::{
    ast::{
        ArgTypeSpec, Declaration, FnDeclare,
        error::{ParseError, ParseResult},
    },
    local_span::ErrorLocalSpan,
    tokens::{Token, TokenLocalSpan, peek::PeekTokenStream},
};

use super::{
    AlternateCondition, Assignment, BinaryOp, BinaryOperatorKind, Block, Closure, FalseLiteral,
    FnCall, FnDeclareArg, Ident, IfCondition, IntegerLiteral, InternalLiteral, LetDeclare, List,
    ListElement, Module, ModuleRoot, Node, NodeId, NodeList, Not, Record, RecordField,
    StringLiteral, TrueLiteral,
};

pub fn parse_module(source: &str) -> ParseResult<Module> {
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
                Token::Fn => {
                    declarations.push(self.parse_fn_declare()?);
                }
                _ => {
                    return Err(peek
                        .span
                        .with_error(ParseError::ExpectedToken(&[Token::Fn, Token::Let])));
                }
            }
        }
        // Consume trailing new line
        if let Some(t) = self.stream.peek()? {
            if let Token::NewLine | Token::Comment = t.token {
                self.stream.parse_next()?;
            }
        }
        Ok(ModuleRoot { declarations })
    }

    fn parse_let_declare(&mut self) -> ParseResult<Declaration> {
        let token = self.stream.expect_parse_next(&[Token::Pub, Token::Let])?;
        let (pub_token, let_token) = if token.token == Token::Pub {
            (Some(token), self.stream.expect_parse_next(&[Token::Let])?)
        } else {
            (None, token)
        };
        let name = self.stream.expect_parse_next(&[Token::Ident])?;
        let token = self
            .stream
            .expect_parse_next(&[Token::Colon, Token::Assign])?;
        let (type_spec, equals_token) = if token.token == Token::Colon {
            (
                Some(ArgTypeSpec {
                    colon_token: token,
                    type_expr: self.parse_expr()?,
                }),
                self.stream.expect_parse_next(&[Token::Assign])?,
            )
        } else {
            (None, token)
        };
        let expr = self.parse_expr()?;
        Ok(Declaration::LetDeclare(LetDeclare {
            pub_token,
            let_token,
            name,
            type_spec,
            equals_token,
            expr,
        }))
    }

    fn parse_fn_declare(&mut self) -> ParseResult<Declaration> {
        let token = self.stream.expect_parse_next(&[Token::Pub, Token::Fn])?;
        let (pub_token, fn_token) = if token.token == Token::Pub {
            (Some(token), self.stream.expect_parse_next(&[Token::Fn])?)
        } else {
            (None, token)
        };
        let name = self.stream.expect_parse_next(&[Token::Ident])?;
        let lparen_token = self.stream.expect_parse_next(&[Token::LParen])?;
        let mut args = Vec::new();
        loop {
            let t = self.stream.expect_peek(&[Token::RParen, Token::Ident])?;
            match t.token {
                Token::RParen => break,
                Token::Ident => {}
                _ => unreachable!("parse fn declare rparen"),
            }
            self.stream.parse_next()?;
            let name = t;
            let mut t = self
                .stream
                .expect_peek(&[Token::RParen, Token::Comma, Token::Colon])?;
            if t.token == Token::Colon {
                self.stream.parse_next()?;
                let expr = self.parse_expr()?;
                args.push(FnDeclareArg {
                    name,
                    type_spec: Some(ArgTypeSpec {
                        colon_token: t,
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

        let rparen_token = self.stream.expect_parse_next(&[Token::RParen])?;
        let block = self.parse_block()?;
        Ok(Declaration::FnDeclare(FnDeclare {
            pub_token,
            fn_token,
            name,
            lparen_token,
            args,
            rparen_token,
            block,
        }))
    }

    fn parse_anonymous_fn_declare(&mut self, fn_token: TokenLocalSpan) -> ParseResult<NodeId> {
        let lparen_token = self.stream.expect_parse_next(&[Token::LParen])?;
        let mut args = Vec::new();
        loop {
            let t = self.stream.expect_peek(&[Token::RParen, Token::Ident])?;
            match t.token {
                Token::RParen => break,
                Token::Ident => {}
                _ => unreachable!("parse fn declare rparen"),
            }
            self.stream.parse_next()?;
            let name = t;
            let mut t = self
                .stream
                .expect_peek(&[Token::RParen, Token::Comma, Token::Colon])?;
            if t.token == Token::Colon {
                self.stream.parse_next()?;
                let expr = self.parse_expr()?;
                args.push(FnDeclareArg {
                    name,
                    type_spec: Some(ArgTypeSpec {
                        colon_token: t,
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

        let rparen_token = self.stream.expect_parse_next(&[Token::RParen])?;
        let mut return_type = None;
        if let Some(peek) = self.stream.peek()?
            && peek.token == Token::ReturnType
        {
            let Some(arrow) = self.stream.parse_next()? else {
                unreachable!()
            };
            let expr = self.parse_expr()?;
            return_type = Some(super::ClosureReturnType {
                return_type_arrow: arrow,
                type_expr: expr,
            });
        }
        let block = self.parse_block()?;
        Ok(self.push(Closure {
            fn_token,
            lparen_token,
            args,
            rparen_token,
            return_type,
            block,
        }))
    }

    fn parse_block(&mut self) -> ParseResult<NodeId> {
        let lbrace_token = self.stream.expect_parse_next(&[Token::LBrace])?;
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
        let rbrace_token = self.stream.expect_parse_next(&[Token::RBrace])?;
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
        let name = self.stream.expect_parse_next(&[Token::Ident])?;
        let equals_token = self.stream.expect_parse_next(&[Token::Assign])?;
        let expr = self.parse_expr()?;
        Ok(self.push(Assignment {
            name,
            equals_token,
            expr,
        }))
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
            Token::Fn => self.parse_anonymous_fn_declare(t)?,
            Token::Ident => self.push(Ident(t)),
            Token::Number => self.push(IntegerLiteral(t)),
            Token::DoubleQuoteLiteral(_) => self.push(StringLiteral(t)),
            Token::True => self.push(TrueLiteral(t)),
            Token::False => self.push(FalseLiteral(t)),
            Token::Internal => self.push(InternalLiteral(t)),
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
                if let Some(prefix) = prefixes.last() {
                    if let Some((prefix_precedence, _)) =
                        get_token_precedence_associativity(prefix.token)
                    {
                        if let Some((_, precedence)) = check_op(self.stream.peek()?, min_precedence)
                        {
                            if prefix_precedence > precedence {
                                debug_assert_eq!(prefix.token, Token::Excalmation);
                                rhs = self.push(Not {
                                    exclamation: prefix.span,
                                    inner: rhs,
                                });
                                prefixes.pop();
                                continue;
                            }
                        }
                    }
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
                unreachable!("parse_expr_ops")
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
        let lparen_token = self.stream.expect_parse_next(&[Token::LParen])?;
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
        let rparen_token = self.stream.expect_parse_next(&[Token::RParen])?;
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
            let key = self.stream.expect_parse_next(&[Token::Ident])?;
            let equals = self.stream.expect_parse_next(&[Token::Assign])?.span;
            let value = self.parse_expr()?;
            let mut comma = None;
            if let Some(tls) = self.stream.peek()? {
                if tls.token == Token::Comma {
                    comma = Some(self.stream.expect_parse_next(&[Token::Comma])?.span);
                }
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

#[cfg(test)]
mod test {
    use std::path::Path;

    use crate::{
        afs::file::File,
        ast::{error::ParseError, parser::ModuleParser},
        local_span::{ErrorLocalSpan, LocalSpan},
    };

    use super::parse_module;

    fn parse_display_expr(src: &str) -> String {
        let file = File::new_local(Path::new(file!())).unwrap();
        let mut parser = ModuleParser::new(src);
        let id = match parser.parse_expr() {
            Ok(s) => s,
            Err(err) => {
                eprintln!("{}", err.resolve(Some(&file), src));
                panic!("parse error");
            }
        };
        let nodes = match parser.complete() {
            Ok(nodes) => nodes,
            Err(err) => {
                eprintln!("{}", err.resolve(Some(&file), src));
                panic!("parse error");
            }
        };
        nodes.display(src, nodes.get(id).ast_node())
    }

    #[test]
    fn number_literal() {
        insta::assert_snapshot!(parse_display_expr("4"));
    }

    #[test]
    fn false_literal() {
        insta::assert_snapshot!(parse_display_expr("false"));
    }

    #[test]
    fn true_literal() {
        insta::assert_snapshot!(parse_display_expr("true"));
    }

    #[test]
    fn string_literal() {
        insta::assert_snapshot!(parse_display_expr("\"asldjf\""));
    }

    #[test]
    fn number_add() {
        insta::assert_snapshot!(parse_display_expr("1 + 1"));
    }

    #[test]
    fn number_add_left_associative() {
        insta::assert_snapshot!(parse_display_expr("1 + 2 + 3"));
    }

    #[test]
    fn number_multiply() {
        insta::assert_snapshot!(parse_display_expr("1 * 2"));
    }

    #[test]
    fn number_multiply_left_associative() {
        insta::assert_snapshot!(parse_display_expr("1 * 2 * 3"));
    }

    #[test]
    fn number_multiply_add_precedence1() {
        insta::assert_snapshot!(parse_display_expr("5 * 2 + 3"));
    }

    #[test]
    fn number_multiply_add_precedence2() {
        insta::assert_snapshot!(parse_display_expr("5 + 2 * 3"));
    }

    #[test]
    fn number_add_subtract_precedence() {
        insta::assert_snapshot!(parse_display_expr("5 - 2 + 3 - 4"));
    }

    #[test]
    fn number_add_subtract_multiply_precedence() {
        insta::assert_snapshot!(parse_display_expr("5 * 2 + 3 - 4"));
    }

    #[test]
    fn number_add_subtrace_multiply_divide_precedence() {
        insta::assert_snapshot!(parse_display_expr("1 - 3 / 2 + 4 * 3"));
    }

    #[test]
    fn ident_maths() {
        insta::assert_snapshot!(parse_display_expr("a + b - c * d / e"));
    }

    #[test]
    fn ident_dot_ident() {
        insta::assert_snapshot!(parse_display_expr("foo.bar"));
    }

    #[test]
    fn ident_dot_ident_dot_ident() {
        insta::assert_snapshot!(parse_display_expr("foo.bar.baz"));
    }

    #[test]
    fn ident_dot_maths() {
        insta::assert_snapshot!(parse_display_expr("a.b.c + 3 * d.e"));
    }

    #[test]
    fn maths_parens1() {
        insta::assert_snapshot!(parse_display_expr("1 - (a + 3) * 4"));
    }

    #[test]
    fn maths_parens2() {
        insta::assert_snapshot!(parse_display_expr("(3 - b) * c"));
    }

    #[test]
    fn fn_call_no_args() {
        insta::assert_snapshot!(parse_display_expr("foo()"));
    }

    #[test]
    fn fn_call_no_args_call_no_args() {
        insta::assert_snapshot!(parse_display_expr("foo()()"));
    }

    #[test]
    fn fn_call_no_args_precedence() {
        insta::assert_snapshot!(parse_display_expr("foo.bar()"));
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

    #[test]
    fn logical_operators() {
        insta::assert_snapshot!(parse_display_expr(
            "true || a == b && 1 != 1 && (false || a != b)"
        ));
    }

    #[test]
    fn record_constructor() {
        insta::assert_snapshot!(parse_display_expr("{a = 1, b = 2, c = \"ajlsdkf\"}"));
    }

    #[test]
    fn record_constructor_nested() {
        insta::assert_snapshot!(parse_display_expr("{a = {b = {c = 5}},}"));
    }

    #[test]
    fn record_constructor_nls() {
        insta::assert_snapshot!(parse_display_expr("{\na = b, \n// comment \n c = 4\n}"));
    }

    #[test]
    fn list_constructor_nested() {
        insta::assert_snapshot!(parse_display_expr("[a, b, 123, [567, d]]"));
    }

    #[test]
    fn list_constructor_nested_nls() {
        insta::assert_snapshot!(parse_display_expr(
            "[a\n, b,\n 123, // comment \n [\n567, d]\n]"
        ));
    }

    #[test]
    fn invalid_exprs() {
        assert!(ModuleParser::new("4.").parse_expr().is_err());
        assert!(ModuleParser::new(".4").parse_expr().is_err());
        assert!(ModuleParser::new("()").parse_expr().is_err());
        assert_eq!(
            ModuleParser::new("()").parse_expr(),
            Err(LocalSpan::byte(1).with_error(ParseError::ExpectedExpression))
        );
        assert_eq!(
            ModuleParser::new("(").parse_expr(),
            Err(LocalSpan::byte(0).with_error(ParseError::ExpectedExpression))
        );
        assert_eq!(
            ModuleParser::new(")").parse_expr(),
            Err(LocalSpan::byte(0).with_error(ParseError::ExpectedExpression))
        );
    }

    #[test]
    fn invalid_scripts() {
        fn parse_display_module(src: &str) -> Result<(), ErrorLocalSpan<ParseError>> {
            parse_module(src).map(|m| {
                log::error!("{}", m.display(src));
            })
        }
        assert!(parse_display_module("fn foo() {5 6}").is_err());
        assert!(parse_display_module("fn foo() {a b c}").is_err());
    }

    #[test]
    fn not_and_operation() {
        insta::assert_snapshot!(parse_display_expr("false && !!!true || !false"));
    }

    #[test]
    fn not_paren_operation() {
        insta::assert_snapshot!(parse_display_expr("!(!a || !b)"));
    }

    #[test]
    fn not_dot_operation() {
        insta::assert_snapshot!(parse_display_expr("!a.b"));
    }

    #[test]
    fn not_or_operation() {
        insta::assert_snapshot!(parse_display_expr("!a || b"));
    }

    #[test]
    fn not_not_or_operation() {
        insta::assert_snapshot!(parse_display_expr("!!a || b"));
    }

    #[test]
    fn or_not_dot_operation() {
        insta::assert_snapshot!(parse_display_expr("a || !b.c"));
    }

    #[test]
    fn not_dot_plus_expr() {
        insta::assert_snapshot!(parse_display_expr("!a.b + d"));
    }

    #[test]
    fn plus_not_dot_plus_expr() {
        insta::assert_snapshot!(parse_display_expr("f + !a.b + d"));
    }

    #[test]
    fn dot_not_plus_dot_plus_expr() {
        insta::assert_snapshot!(parse_display_expr("a.b + !c + !d.e"));
    }

    #[test]
    fn less_than() {
        insta::assert_snapshot!(parse_display_expr("b < c && d > e"));
    }

    #[test]
    fn greater_than_eq() {
        insta::assert_snapshot!(parse_display_expr("a >= b"));
    }

    #[test]
    fn closure() {
        insta::assert_snapshot!(parse_display_expr("fn () {}"));
    }

    #[test]
    fn closure_args() {
        insta::assert_snapshot!(parse_display_expr("fn (a: A, b: B) { 5 }(a, b)"));
    }
}
