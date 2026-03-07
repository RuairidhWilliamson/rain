use tree_sitter::TreeCursor;

use crate::{
    ast::{
        AlternateCondition, Assignment, BinaryOp, BinaryOperatorKind, Block, Closure,
        ClosureReturnTypeSpec, Declare, DeclareName, DeclareNameListElement, DeclareNameSingle,
        DeclareNamedDestructure, FnCall, FnDeclareArg, Ident, IfCondition, IntegerLiteral, List,
        ListElement, Module, ModuleRoot, NodeId, NodeList, Not, Record, RecordField,
        SimpleLiteralKind, StringLiteral, TypeSpec,
    },
    local_span::LocalSpan,
};

const DEPTH_LIMIT: u32 = 50;

#[derive(Debug)]
pub enum Error {
    TreeSitter,
    DepthLimit,
    ParseErrors(Vec<(LocalSpan, String)>),
}

struct Walker<'a, 'cursor> {
    cursor: &'a mut TreeCursor<'cursor>,
    nodes: &'a mut NodeList,
}

impl<'a, 'cursor> Walker<'a, 'cursor> {
    fn new(cursor: &'a mut TreeCursor<'cursor>, nodes: &'a mut NodeList) -> Result<Self, Error> {
        if cursor.depth() > DEPTH_LIMIT {
            return Err(Error::DepthLimit);
        }
        assert!(
            cursor.goto_first_child(),
            "cannot get child of {}",
            cursor.node().kind()
        );
        Ok(Self { cursor, nodes })
    }

    fn kind(&self) -> &'static str {
        self.cursor.node().kind()
    }

    fn next(&mut self) {
        assert!(self.cursor.goto_next_sibling());
    }

    fn span(&self) -> LocalSpan {
        let node = self.cursor.node();
        LocalSpan::new(node.start_byte(), node.end_byte())
    }

    fn span_expect(&self, kind: &str) -> LocalSpan {
        let node = self.cursor.node();
        assert_eq!(node.kind(), kind);
        LocalSpan::new(node.start_byte(), node.end_byte())
    }

    fn maybe_next(&mut self) -> bool {
        self.cursor.goto_next_sibling()
    }

    fn child<'current, 'child>(&'current mut self) -> Result<Walker<'child, 'cursor>, Error>
    where
        'current: 'child,
    {
        Walker::new(self.cursor, self.nodes)
    }
}

impl Drop for Walker<'_, '_> {
    fn drop(&mut self) {
        assert!(self.cursor.goto_parent());
    }
}

pub fn parse(source: &str) -> tree_sitter::Tree {
    let mut parser = tree_sitter::Parser::new();
    let language = tree_sitter_rain::LANGUAGE;
    parser
        .set_language(&language.into())
        .expect("error loading Rain parser");
    parser.parse(source, None).expect("no language")
}

pub fn parse_module(source: &str) -> Result<Module, Error> {
    let tree = parse(source);
    let mut cursor = tree.walk();
    let mut nodes = NodeList::new();
    let mut declarations = Vec::new();
    if cursor.node().has_error() {
        let parse_errors: Vec<(LocalSpan, String)> = tree_errors(&tree)
            .map(|node| {
                let span = LocalSpan::new(node.start_byte(), node.end_byte());
                (span, node.to_sexp())
            })
            .collect();
        return Err(Error::ParseErrors(parse_errors));
    }
    if cursor.goto_first_child() {
        while cursor.goto_next_sibling() {
            let node = cursor.node();
            match node.kind() {
                "declaration" => {
                    declarations.push(parse_declaration(Walker::new(&mut cursor, &mut nodes)?)?);
                }
                "line_comment" => {}
                kind => unreachable!("{kind}"),
            }
        }
    }
    Ok(Module {
        root: ModuleRoot { declarations },
        nodes,
    })
}

fn tree_errors(tree: &tree_sitter::Tree) -> impl Iterator<Item = tree_sitter::Node<'_>> {
    let mut cursor = tree.root_node().walk();
    (0..tree.root_node().descendant_count()).filter_map(move |i| {
        cursor.goto_descendant(i);
        if cursor.node().is_error() && cursor.node().child_count() == 1 {
            Some(cursor.node())
        } else {
            None
        }
    })
}

fn parse_declaration(mut walker: Walker) -> Result<Declare, Error> {
    let pub_token = match walker.kind() {
        "pub" => {
            let s = walker.span_expect("pub");
            walker.next();
            Some(s)
        }
        "let" => None,
        _ => unreachable!(),
    };
    let let_token = walker.span_expect("let");
    walker.next();
    let assignment = parse_assignment(walker.child()?)?;
    Ok(Declare {
        pub_token,
        let_token,
        assignment,
    })
}

fn parse_assignment(mut walker: Walker) -> Result<Assignment, Error> {
    let name = parse_declare_name(walker.child()?)?;
    walker.next();
    let equals_token = walker.span_expect("=");
    walker.next();
    let expr = parse_expr(walker.child()?)?;
    Ok(Assignment {
        name,
        equals_token,
        expr,
    })
}

fn parse_declare_name(mut walker: Walker) -> Result<DeclareName, Error> {
    match walker.kind() {
        "declare_single_name" => {
            let mut declare_single_walker = walker.child()?;
            let name = declare_single_walker.span_expect("identifier");
            let type_spec =
                if declare_single_walker.maybe_next() && declare_single_walker.kind() == ":" {
                    let colon_token = declare_single_walker.span_expect(":");
                    declare_single_walker.next();
                    let type_expr = parse_expr(declare_single_walker.child()?.child()?)?;
                    Some(TypeSpec {
                        colon_token,
                        type_expr,
                    })
                } else {
                    None
                };
            Ok(DeclareName::Single(DeclareNameSingle { name, type_spec }))
        }
        "declare_named_destructure" => {
            let mut walker = walker.child()?;
            let lbrace = walker.span_expect("{");
            walker.next();
            let mut elements = Vec::new();
            loop {
                match walker.kind() {
                    "line_comment" | "," => {
                        walker.next();
                    }
                    "declare_single_name" => {
                        let mut declare_single_walker = walker.child()?;
                        let name = declare_single_walker.span_expect("identifier");
                        let type_spec = if declare_single_walker.maybe_next()
                            && declare_single_walker.kind() == ":"
                        {
                            let colon_token = declare_single_walker.span_expect(":");
                            declare_single_walker.next();
                            let type_expr = parse_expr(declare_single_walker.child()?.child()?)?;
                            Some(TypeSpec {
                                colon_token,
                                type_expr,
                            })
                        } else {
                            None
                        };
                        drop(declare_single_walker);
                        let comma = if walker.maybe_next() && walker.kind() == "comma" {
                            let comma = Some(walker.span());
                            walker.next();
                            comma
                        } else {
                            None
                        };
                        elements.push(DeclareNameListElement {
                            name,
                            type_spec,
                            comma,
                        });
                    }
                    "}" => break,
                    kind => unreachable!("{kind}"),
                }
            }
            let rbrace = walker.span_expect("}");
            Ok(DeclareName::NamedDestructure(DeclareNamedDestructure {
                lbrace,
                elements,
                rbrace,
            }))
        }
        _ => unreachable!(),
    }
}

fn parse_expr(mut walker: Walker) -> Result<NodeId, Error> {
    match walker.kind() {
        "fn_call" => {
            let mut walker = walker.child()?;
            let callee = parse_expr(walker.child()?)?;
            walker.next();
            let mut walker = walker.child()?;
            let lparen_token = walker.span_expect("(");
            let mut args = Vec::new();
            loop {
                walker.next();
                match walker.kind() {
                    "expr" => args.push(parse_expr(walker.child()?)?),
                    "," => {}
                    ")" => break,
                    _ => unreachable!(),
                }
            }
            let rparen_token = walker.span_expect(")");
            Ok(walker.nodes.push(FnCall {
                callee,
                lparen_token,
                args,
                rparen_token,
            }))
        }
        "identifier" => Ok(walker.nodes.push(Ident(walker.span_expect("identifier")))),
        "string_literal" => Ok(walker.nodes.push(StringLiteral {
            prefix: None,
            contents: walker.span(),
        })),
        "raw_string_literal" => Ok(walker.nodes.push(StringLiteral {
            prefix: Some(crate::tokens::StringLiteralPrefix::Raw),
            contents: walker.span(),
        })),
        "format_string_literal" => Ok(walker.nodes.push(StringLiteral {
            prefix: Some(crate::tokens::StringLiteralPrefix::Format),
            contents: walker.span(),
        })),
        "fn_declare_expr" => parse_closure(walker.child()?),
        "namespace" => {
            let mut walker = walker.child()?;
            let left = parse_expr(walker.child()?)?;
            walker.next();
            let op_span = walker.span_expect(".");
            walker.next();
            let right = walker.nodes.push(Ident(walker.span_expect("identifier")));
            let binary_op = BinaryOp {
                left,
                op: BinaryOperatorKind::Dot,
                op_span,
                right,
            };
            Ok(walker.nodes.push(binary_op))
        }
        "internal" => {
            let internal = SimpleLiteralKind::Internal.with(walker.span_expect("internal"));
            Ok(walker.nodes.push(internal))
        }
        "bool_literal" => {
            let walker = walker.child()?;
            match walker.kind() {
                "true" => Ok(walker
                    .nodes
                    .push(SimpleLiteralKind::True.with(walker.span()))),
                "false" => Ok(walker
                    .nodes
                    .push(SimpleLiteralKind::False.with(walker.span()))),
                _ => unreachable!(),
            }
        }
        "list_literal" => parse_list_literal(walker.child()?),
        "record_literal" => parse_record_literal(walker.child()?),
        "number_literal" => Ok(walker.nodes.push(IntegerLiteral(walker.span()))),
        "if_condition" => {
            let mut walker = walker.child()?;
            parse_if_condition(&mut walker)
        }
        "unary_expr" => {
            let mut walker = walker.child()?;
            let exclamation = walker.span_expect("!");
            walker.next();
            let inner = parse_expr(walker.child()?)?;
            Ok(walker.nodes.push(Not { exclamation, inner }))
        }
        "binary_expr" => parse_binary_expr(walker.child()?),
        "(" => {
            walker.next();
            parse_expr(walker.child()?)
        }
        kind => unreachable!("expr: {kind}"),
    }
}

fn parse_binary_expr(mut walker: Walker<'_, '_>) -> Result<NodeId, Error> {
    let left = parse_expr(walker.child()?)?;
    walker.next();
    let op = match walker.kind() {
        "*" => BinaryOperatorKind::Multiplication,
        "/" => BinaryOperatorKind::Division,
        "+" => BinaryOperatorKind::Addition,
        "-" => BinaryOperatorKind::Subtraction,
        "==" => BinaryOperatorKind::Equals,
        "!=" => BinaryOperatorKind::NotEquals,
        "||" => BinaryOperatorKind::LogicalOr,
        "&&" => BinaryOperatorKind::LogicalAnd,
        "%" => BinaryOperatorKind::Modulo,
        "^" => BinaryOperatorKind::Pow,
        "&" => BinaryOperatorKind::BitwiseAnd,
        "|" => BinaryOperatorKind::BitwiseOr,
        "<" => BinaryOperatorKind::LessThan,
        ">" => BinaryOperatorKind::GreaterThan,
        "<=" => BinaryOperatorKind::LessThanEquals,
        ">=" => BinaryOperatorKind::GreaterThanEquals,
        kind => unreachable!("binary op: {kind}"),
    };
    let op_span = walker.span();
    walker.next();
    let right = parse_expr(walker.child()?)?;
    let binary_op = BinaryOp {
        left,
        op,
        op_span,
        right,
    };
    Ok(walker.nodes.push(binary_op))
}

fn parse_record_literal(mut walker: Walker<'_, '_>) -> Result<NodeId, Error> {
    let lbrace = walker.span_expect("{");
    walker.next();
    let mut fields = Vec::new();
    loop {
        match walker.kind() {
            "record_element" => {
                let mut element_walker = walker.child()?;
                let key = element_walker.span_expect("identifier");
                element_walker.next();
                let equals = element_walker.span_expect("=");
                element_walker.next();
                let expr = parse_expr(element_walker.child()?)?;
                drop(element_walker);
                let comma = if walker.maybe_next() && walker.kind() == "comma" {
                    let comma = Some(walker.span());
                    walker.next();
                    comma
                } else {
                    None
                };
                fields.push(RecordField {
                    key,
                    equals,
                    value: expr,
                    comma,
                });
            }
            "}" => {
                break;
            }
            "," | "line_comment" => {
                walker.next();
            }
            kind => unreachable!("{kind}"),
        }
    }
    let rbrace = walker.span_expect("}");
    Ok(walker.nodes.push(Record {
        lbrace,
        fields,
        rbrace,
    }))
}

fn parse_list_literal(mut walker: Walker<'_, '_>) -> Result<NodeId, Error> {
    let lsqbracket = walker.span_expect("[");
    walker.next();
    let mut elements = Vec::new();
    loop {
        match walker.kind() {
            "expr" => {
                let expr = parse_expr(walker.child()?)?;
                let comma = if walker.maybe_next() && walker.kind() == "comma" {
                    let comma = Some(walker.span());
                    walker.next();
                    comma
                } else {
                    None
                };
                elements.push(ListElement { value: expr, comma });
            }
            "]" => {
                break;
            }
            "," | "line_comment" => {
                walker.next();
            }
            kind => unreachable!("{kind}"),
        }
    }
    let rsqbracket = walker.span_expect("]");
    Ok(walker.nodes.push(List {
        lsqbracket,
        elements,
        rsqbracket,
    }))
}

fn parse_closure(mut walker: Walker<'_, '_>) -> Result<NodeId, Error> {
    let fn_token = walker.span_expect("fn");
    walker.next();
    let mut arg_walker = walker.child()?;
    let lparen_token = arg_walker.span_expect("(");
    arg_walker.next();
    let mut args = Vec::new();
    loop {
        match arg_walker.kind() {
            "fn_declare_arg" => {
                let element_walker = arg_walker.child()?;
                let name = element_walker.span_expect("identifier");
                drop(element_walker);
                arg_walker.next();
                args.push(FnDeclareArg {
                    name,
                    type_spec: None,
                });
            }
            "," => {
                arg_walker.next();
            }
            ")" => break,
            kind => unreachable!("{kind}"),
        }
    }
    let rparen_token = arg_walker.span_expect(")");
    drop(arg_walker);
    walker.next();
    let return_type = if walker.kind() == "->" {
        let return_type_arrow = walker.span_expect("->");
        walker.next();
        let expr = parse_expr(walker.child()?.child()?)?;
        walker.next();
        Some(ClosureReturnTypeSpec {
            return_type_arrow,
            type_expr: expr,
        })
    } else {
        None
    };
    let block = parse_block(walker.child()?)?;
    let block = walker.nodes.push(block);

    Ok(walker.nodes.push(Closure {
        fn_token,
        lparen_token,
        args,
        rparen_token,
        return_type,
        block,
    }))
}

fn parse_if_condition(walker: &mut Walker) -> Result<NodeId, Error> {
    // let if_span = walker.span_expect("if");
    walker.next();
    let condition = parse_expr(walker.child()?)?;
    walker.next();
    let then_block = parse_block(walker.child()?)?;
    let then_block = walker.nodes.push(then_block);
    let alternate = if walker.maybe_next() && walker.kind() == "else" {
        // let else_span = walker.span();
        walker.next();
        if walker.kind() == "if" {
            Some(AlternateCondition::IfElseCondition(parse_if_condition(
                walker,
            )?))
        } else {
            let block = parse_block(walker.child()?)?;
            Some(AlternateCondition::ElseBlock(walker.nodes.push(block)))
        }
    } else {
        None
    };
    Ok(walker.nodes.push(IfCondition {
        condition,
        then_block,
        alternate,
    }))
}

fn parse_block(mut walker: Walker) -> Result<Block, Error> {
    let lbrace_token = walker.span_expect("{");
    walker.next();
    let mut statements = Vec::new();
    while walker.kind() != "}" {
        if let Some(s) = parse_statement(walker.child()?)? {
            statements.push(s);
        }
        walker.next();
    }
    let rbrace_token = walker.span_expect("}");

    Ok(Block {
        lbrace_token,
        statements,
        rbrace_token,
    })
}

fn parse_statement(mut walker: Walker) -> Result<Option<NodeId>, Error> {
    match walker.kind() {
        "assignment" => {
            let assignment = parse_assignment(walker.child()?)?;
            Ok(Some(walker.nodes.push(assignment)))
        }
        "expr" => Ok(Some(parse_expr(walker.child()?)?)),
        "line_comment" => Ok(None),
        _ => unreachable!(),
    }
}
