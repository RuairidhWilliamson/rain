use tree_sitter::TreeCursor;

use crate::{
    ast::{
        AlternateCondition, Assignment, BinaryOp, BinaryOperatorKind, Block, Closure,
        ClosureReturnTypeSpec, Declare, DeclareName, DeclareNameListElement, DeclareNameSingle,
        DeclareNamedDestructure, FnCall, FnDeclareArg, Ident, IfCondition, IntegerLiteral, List,
        ListElement, Module, ModuleRoot, NodeId, NodeList, Not, Record, RecordField,
        SimpleLiteralKind, StringLiteral, TypeSpec, error::ParseError,
    },
    local_span::{ErrorLocalSpan, LocalSpan},
};

struct Walker<'a, 'cursor> {
    cursor: &'a mut TreeCursor<'cursor>,
    nodes: &'a mut NodeList,
}

impl<'a, 'cursor> Walker<'a, 'cursor> {
    fn new(cursor: &'a mut TreeCursor<'cursor>, node_list: &'a mut NodeList) -> Self {
        assert!(
            cursor.goto_first_child(),
            "cannot get child of {}",
            cursor.node().kind()
        );
        Self {
            cursor,
            nodes: node_list,
        }
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

    fn child<'current, 'child>(&'current mut self) -> Walker<'child, 'cursor>
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

pub fn parse_module(source: &str) -> Result<Module, ErrorLocalSpan<ParseError>> {
    let tree = parse(source);
    let mut cursor = tree.walk();
    let mut nodes = NodeList::new();
    let mut declarations = Vec::new();
    if cursor.goto_first_child() {
        while cursor.goto_next_sibling() {
            let node = cursor.node();
            match node.kind() {
                "declaration" => {
                    declarations.push(parse_declaration(Walker::new(&mut cursor, &mut nodes)));
                }
                "line_comment" => {}
                _ => unreachable!(),
            }
        }
    }
    Ok(Module {
        root: ModuleRoot { declarations },
        nodes,
    })
}

fn parse_declaration(mut walker: Walker) -> Declare {
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
    let assignment = parse_assignment(walker.child());
    Declare {
        pub_token,
        let_token,
        assignment,
    }
}

fn parse_assignment(mut walker: Walker) -> Assignment {
    let name = parse_declare_name(walker.child());
    walker.next();
    let equals_token = walker.span_expect("=");
    walker.next();
    let expr = parse_expr(walker.child());
    Assignment {
        name,
        equals_token,
        expr,
    }
}

fn parse_declare_name(mut walker: Walker) -> DeclareName {
    match walker.kind() {
        "declare_single_name" => {
            let mut declare_single_walker = walker.child();
            let name = declare_single_walker.span_expect("identifier");
            let type_spec =
                if declare_single_walker.maybe_next() && declare_single_walker.kind() == ":" {
                    let colon_token = declare_single_walker.span_expect(":");
                    declare_single_walker.next();
                    let type_expr = parse_expr(declare_single_walker.child().child());
                    Some(TypeSpec {
                        colon_token,
                        type_expr,
                    })
                } else {
                    None
                };
            DeclareName::Single(DeclareNameSingle { name, type_spec })
        }
        "declare_named_destructure" => {
            let mut walker = walker.child();
            let lbrace = walker.span_expect("{");
            walker.next();
            let mut elements = Vec::new();
            loop {
                match walker.kind() {
                    "line_comment" | "," => {
                        walker.next();
                    }
                    "declare_single_name" => {
                        let mut declare_single_walker = walker.child();
                        let name = declare_single_walker.span_expect("identifier");
                        let type_spec = if declare_single_walker.maybe_next()
                            && declare_single_walker.kind() == ":"
                        {
                            let colon_token = declare_single_walker.span_expect(":");
                            declare_single_walker.next();
                            let type_expr = parse_expr(declare_single_walker.child().child());
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
            DeclareName::NamedDestructure(DeclareNamedDestructure {
                lbrace,
                elements,
                rbrace,
            })
        }
        _ => unreachable!(),
    }
}

fn parse_expr(mut walker: Walker) -> NodeId {
    match walker.kind() {
        "fn_call" => {
            let mut walker = walker.child();
            let callee = parse_expr(walker.child());
            walker.next();
            let mut walker = walker.child();
            let lparen_token = walker.span_expect("(");
            let mut args = Vec::new();
            loop {
                walker.next();
                match walker.kind() {
                    "expr" => args.push(parse_expr(walker.child())),
                    "," => continue,
                    ")" => break,
                    _ => unreachable!(),
                }
            }
            let rparen_token = walker.span_expect(")");
            walker.nodes.push(FnCall {
                callee,
                lparen_token,
                args,
                rparen_token,
            })
        }
        "identifier" => walker.nodes.push(Ident(walker.span_expect("identifier"))),
        "string_literal" => walker.nodes.push(StringLiteral {
            prefix: None,
            contents: walker.span(),
        }),
        "fn_declare_expr" => {
            let mut walker = walker.child();
            let fn_token = walker.span_expect("fn");
            walker.next();
            let mut arg_walker = walker.child();
            let lparen_token = arg_walker.span_expect("(");
            arg_walker.next();
            let mut args = Vec::new();
            loop {
                match arg_walker.kind() {
                    "fn_declare_arg" => {
                        let element_walker = arg_walker.child();
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
                let expr = parse_expr(walker.child().child());
                walker.next();
                Some(ClosureReturnTypeSpec {
                    return_type_arrow,
                    type_expr: expr,
                })
            } else {
                None
            };
            let block = parse_block(walker.child());
            let block = walker.nodes.push(block);

            walker.nodes.push(Closure {
                fn_token,
                lparen_token,
                args,
                rparen_token,
                return_type,
                block,
            })
        }
        "namespace" => {
            let mut walker = walker.child();
            let left = parse_expr(walker.child());
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
            walker.nodes.push(binary_op)
        }
        "internal" => {
            let internal = SimpleLiteralKind::Internal.with(walker.span_expect("internal"));
            walker.nodes.push(internal)
        }
        "bool_literal" => {
            let walker = walker.child();
            match walker.kind() {
                "true" => walker
                    .nodes
                    .push(SimpleLiteralKind::True.with(walker.span())),
                "false" => walker
                    .nodes
                    .push(SimpleLiteralKind::False.with(walker.span())),
                _ => unreachable!(),
            }
        }
        "list_literal" => {
            let mut walker = walker.child();
            let lsqbracket = walker.span_expect("[");
            walker.next();
            let mut elements = Vec::new();
            loop {
                match walker.kind() {
                    "expr" => {
                        let expr = parse_expr(walker.child());
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
                    "," => {
                        walker.next();
                    }
                    kind => unreachable!("{kind}"),
                }
            }
            let rsqbracket = walker.span_expect("]");
            walker.nodes.push(List {
                lsqbracket,
                elements,
                rsqbracket,
            })
        }
        "record_literal" => {
            let mut walker = walker.child();
            let lbrace = walker.span_expect("{");
            walker.next();
            let mut fields = Vec::new();
            loop {
                match walker.kind() {
                    "record_element" => {
                        let mut element_walker = walker.child();
                        let key = element_walker.span_expect("identifier");
                        element_walker.next();
                        let equals = element_walker.span_expect("=");
                        element_walker.next();
                        let expr = parse_expr(element_walker.child());
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
                    "," => {
                        walker.next();
                    }
                    kind => unreachable!("{kind}"),
                }
            }
            let rbrace = walker.span_expect("}");
            walker.nodes.push(Record {
                lbrace,
                fields,
                rbrace,
            })
        }
        "number_literal" => walker.nodes.push(IntegerLiteral(walker.span())),
        "if_condition" => {
            let mut walker = walker.child();
            parse_if_condition(&mut walker)
        }
        "unary_expr" => {
            let mut walker = walker.child();
            let exclamation = walker.span_expect("!");
            walker.next();
            let inner = parse_expr(walker.child());
            walker.nodes.push(Not { exclamation, inner })
        }
        "binary_expr" => {
            let mut walker = walker.child();
            let left = parse_expr(walker.child());
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
                kind => todo!("binary op: {kind}"),
            };
            let op_span = walker.span();
            walker.next();
            let right = parse_expr(walker.child());
            let binary_op = BinaryOp {
                left,
                op,
                op_span,
                right,
            };
            walker.nodes.push(binary_op)
        }
        kind => todo!("expr: {kind}"),
    }
}

fn parse_if_condition(walker: &mut Walker) -> NodeId {
    // let if_span = walker.span_expect("if");
    walker.next();
    let condition = parse_expr(walker.child());
    walker.next();
    let then_block = parse_block(walker.child());
    let then_block = walker.nodes.push(then_block);
    let alternate = if walker.maybe_next() && walker.kind() == "else" {
        // let else_span = walker.span();
        walker.next();
        if walker.kind() == "if" {
            Some(AlternateCondition::IfElseCondition(parse_if_condition(
                walker,
            )))
        } else {
            let block = parse_block(walker.child());
            Some(AlternateCondition::ElseBlock(walker.nodes.push(block)))
        }
    } else {
        None
    };
    walker.nodes.push(IfCondition {
        condition,
        then_block,
        alternate,
    })
}

fn parse_block(mut walker: Walker) -> Block {
    let lbrace_token = walker.span_expect("{");
    walker.next();
    let mut statements = Vec::new();
    while walker.kind() != "}" {
        if let Some(s) = parse_statement(walker.child()) {
            statements.push(s);
        }
        walker.next();
    }
    let rbrace_token = walker.span_expect("}");

    Block {
        lbrace_token,
        statements,
        rbrace_token,
    }
}

fn parse_statement(mut walker: Walker) -> Option<NodeId> {
    match walker.kind() {
        "assignment" => {
            let assignment = parse_assignment(walker.child());
            Some(walker.nodes.push(assignment))
        }
        "expr" => Some(parse_expr(walker.child())),
        "line_comment" => None,
        _ => unreachable!(),
    }
}
