#[cfg(test)]
mod test;

mod display;
pub mod error;
pub mod parser;

use crate::{
    local_span::LocalSpan,
    tokens::{StringLiteralPrefix, Token, TokenLocalSpan},
};

trait AstNode {
    fn span(&self, list: &NodeList) -> LocalSpan;
    fn ast_display(&self, f: &mut display::AstFormatter) -> std::fmt::Result;
}

#[derive(Debug)]
pub struct Module {
    pub root: ModuleRoot,
    nodes: NodeList,
}

impl Module {
    pub fn display(&self, src: &str) -> String {
        self.nodes.display(src, &self.root)
    }

    pub fn get(&self, id: NodeId) -> &Node {
        self.nodes.get(id)
    }

    pub fn span(&self, id: NodeId) -> LocalSpan {
        self.nodes.get(id).span(&self.nodes)
    }

    pub fn find_node_by_span(&self, search_span: LocalSpan) -> Option<NodeId> {
        (0..self.nodes.nodes.len())
            .map(NodeId)
            .map(|id| (id, self.nodes.span(id)))
            .filter(|(_id, span)| span.contains(&search_span))
            .min_by_key(|(_id, span)| span.len())
            .map(|(id, _span)| id)
    }

    pub fn display_node(&self, src: &str, id: NodeId) -> String {
        self.nodes.display(src, self.nodes.get(id).ast_node())
    }
}

#[derive(Debug)]
struct NodeList {
    nodes: Vec<Node>,
}

impl NodeList {
    const fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    fn display(&self, src: &str, target: &dyn AstNode) -> String {
        let mut buf = String::new();
        let mut f = display::AstFormatter::new(src, &mut buf, self);
        target.ast_display(&mut f).expect("display write");
        buf
    }

    fn push(&mut self, node: impl Into<Node>) -> NodeId {
        let index = self.nodes.len();
        let node = node.into();
        self.nodes.push(node);
        NodeId(index)
    }

    fn get(&self, id: NodeId) -> &Node {
        let Some(node) = self.nodes.get(id.0) else {
            unreachable!("unknown_node")
        };
        node
    }

    fn span(&self, id: NodeId) -> LocalSpan {
        self.get(id).span(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NodeId(usize);

impl From<&Self> for NodeId {
    fn from(value: &Self) -> Self {
        *value
    }
}

#[derive(Debug)]
pub enum Declaration {
    LetDeclare(LetDeclare),
    FnDeclare(FnDeclare),
}

impl Declaration {
    fn ast_node(&self) -> &dyn AstNode {
        match self {
            Self::FnDeclare(inner) => inner,
            Self::LetDeclare(inner) => inner,
        }
    }
}

#[derive(Debug)]
pub enum Node {
    AnonymousFnDeclare(AnonymousFnDeclare),
    Block(Block),
    IfCondition(IfCondition),
    FnCall(FnCall),
    Assignment(Assignment),
    BinaryOp(BinaryOp),
    Not(Not),
    Ident(Ident),
    StringLiteral(StringLiteral),
    IntegerLiteral(IntegerLiteral),
    TrueLiteral(TrueLiteral),
    FalseLiteral(FalseLiteral),
    InternalLiteral(InternalLiteral),
    Record(Record),
    List(List),
}

impl Node {
    fn ast_node(&self) -> &dyn AstNode {
        match self {
            Self::Block(inner) => inner,
            Self::IfCondition(inner) => inner,
            Self::FnCall(inner) => inner,
            Self::Assignment(inner) => inner,
            Self::BinaryOp(inner) => inner,
            Self::Not(inner) => inner,
            Self::Ident(inner) => inner,
            Self::StringLiteral(inner) => inner,
            Self::IntegerLiteral(inner) => inner,
            Self::TrueLiteral(inner) => inner,
            Self::FalseLiteral(inner) => inner,
            Self::InternalLiteral(inner) => inner,
            Self::Record(inner) => inner,
            Self::List(inner) => inner,
            Self::AnonymousFnDeclare(inner) => inner,
        }
    }

    fn span(&self, list: &NodeList) -> LocalSpan {
        self.ast_node().span(list)
    }

    fn ast_display(&self, f: &mut display::AstFormatter) -> std::fmt::Result {
        self.ast_node().ast_display(f)
    }
}

#[derive(Debug)]
pub struct ModuleRoot {
    pub declarations: Vec<Declaration>,
}

impl AstNode for ModuleRoot {
    fn span(&self, list: &NodeList) -> LocalSpan {
        LocalSpan::span_iter(self.declarations.iter().map(|d| d.ast_node().span(list)))
    }

    fn ast_display(&self, f: &mut display::AstFormatter) -> std::fmt::Result {
        f.node("ModuleRoot")
            .children_ast(self.declarations.iter().map(Declaration::ast_node))
            .finish()
    }
}

#[derive(Debug)]
pub struct LetDeclare {
    pub pub_token: Option<TokenLocalSpan>,
    pub let_token: TokenLocalSpan,
    pub name: TokenLocalSpan,
    pub type_spec: Option<TypeSpec>,
    pub equals_token: TokenLocalSpan,
    pub expr: NodeId,
}

impl AstNode for LetDeclare {
    fn span(&self, list: &NodeList) -> LocalSpan {
        let first = if let Some(t) = &self.pub_token {
            t.span
        } else {
            self.let_token.span
        };
        first + list.span(self.expr)
    }

    fn ast_display(&self, f: &mut display::AstFormatter) -> std::fmt::Result {
        let mut b = f.node("LetDeclare");
        if let Some(t) = &self.pub_token {
            b.child_contents(t.span);
        } else {
            b.child_str("private");
        }
        b.child_contents(self.name.span);
        if let Some(t) = &self.type_spec {
            b.child_fn(|f| f.node("TypeSpec").child(t.type_expr).finish());
        }
        b.child(self.expr).finish()
    }
}

#[derive(Debug)]
pub struct TypeSpec {
    pub colon_token: TokenLocalSpan,
    pub type_expr: NodeId,
}

#[derive(Debug)]
pub struct AnonymousFnDeclare {
    pub fn_token: TokenLocalSpan,
    pub lparen_token: TokenLocalSpan,
    pub args: Vec<FnDeclareArg>,
    pub rparen_token: TokenLocalSpan,
    pub block: NodeId,
}

impl From<AnonymousFnDeclare> for Node {
    fn from(inner: AnonymousFnDeclare) -> Self {
        Self::AnonymousFnDeclare(inner)
    }
}

impl AstNode for AnonymousFnDeclare {
    fn span(&self, list: &NodeList) -> LocalSpan {
        self.fn_token.span + list.span(self.block)
    }

    fn ast_display(&self, f: &mut display::AstFormatter) -> std::fmt::Result {
        let mut b = f.node("AnonymousFnDeclare");
        b.child_fn(|f| {
            let mut b = f.node("Args");
            for arg in &self.args {
                b.child_contents(arg.name.span);
                if let Some(t) = &arg.type_spec {
                    b.child_fn(|f| f.node("TypeSpec").child(t.type_expr).finish());
                }
            }
            b.finish()
        });
        b.child(self.block).finish()
    }
}

#[derive(Debug)]
pub struct FnDeclare {
    pub pub_token: Option<TokenLocalSpan>,
    pub fn_token: TokenLocalSpan,
    pub name: TokenLocalSpan,
    pub lparen_token: TokenLocalSpan,
    pub args: Vec<FnDeclareArg>,
    pub rparen_token: TokenLocalSpan,
    pub block: NodeId,
}

impl AstNode for FnDeclare {
    fn span(&self, list: &NodeList) -> LocalSpan {
        let first = if let Some(t) = &self.pub_token {
            t.span
        } else {
            self.fn_token.span
        };
        first + list.span(self.block)
    }

    fn ast_display(&self, f: &mut display::AstFormatter) -> std::fmt::Result {
        let mut b = f.node("FnDeclare");
        if let Some(t) = &self.pub_token {
            b.child_contents(t.span);
        } else {
            b.child_str("private");
        }
        b.child_contents(self.name.span);
        b.child_fn(|f| {
            let mut b = f.node("Args");
            for arg in &self.args {
                b.child_contents(arg.name.span);
                if let Some(t) = &arg.type_spec {
                    b.child_fn(|f| f.node("TypeSpec").child(t.type_expr).finish());
                }
            }
            b.finish()
        });
        b.child(self.block).finish()
    }
}

#[derive(Debug)]
pub struct FnDeclareArg {
    pub name: TokenLocalSpan,
    pub type_spec: Option<TypeSpec>,
}

#[derive(Debug)]
pub struct Block {
    pub lbrace_token: TokenLocalSpan,
    pub statements: Vec<NodeId>,
    pub rbrace_token: TokenLocalSpan,
}

impl From<Block> for Node {
    fn from(inner: Block) -> Self {
        Self::Block(inner)
    }
}

impl AstNode for Block {
    fn span(&self, _list: &NodeList) -> LocalSpan {
        self.lbrace_token.span + self.rbrace_token.span
    }

    fn ast_display(&self, f: &mut display::AstFormatter) -> std::fmt::Result {
        f.node("Block").children(&self.statements).finish()
    }
}

#[derive(Debug)]
pub struct StringLiteral(pub TokenLocalSpan);

impl StringLiteral {
    pub fn prefix(&self) -> Option<StringLiteralPrefix> {
        let Token::DoubleQuoteLiteral(prefix) = self.0.token else {
            unreachable!("double_quote_literal")
        };
        prefix
    }

    pub fn content_span(&self) -> LocalSpan {
        let mut s = self.0.span;
        if self.prefix().is_some() {
            s.start += 1;
        }
        s.start += 1;
        s.end -= 1;
        s
    }
}

impl From<StringLiteral> for Node {
    fn from(inner: StringLiteral) -> Self {
        Self::StringLiteral(inner)
    }
}

impl AstNode for StringLiteral {
    fn span(&self, _list: &NodeList) -> LocalSpan {
        self.0.span
    }

    fn ast_display(&self, f: &mut display::AstFormatter) -> std::fmt::Result {
        f.node("StringLiteral").child_contents(self.0.span).finish()
    }
}

#[derive(Debug)]
pub struct BinaryOp {
    pub left: NodeId,
    pub op: BinaryOperatorKind,
    pub op_span: LocalSpan,
    pub right: NodeId,
}

impl From<BinaryOp> for Node {
    fn from(inner: BinaryOp) -> Self {
        Self::BinaryOp(inner)
    }
}

impl AstNode for BinaryOp {
    fn span(&self, list: &NodeList) -> LocalSpan {
        list.span(self.left) + list.span(self.right)
    }

    fn ast_display(&self, f: &mut display::AstFormatter) -> std::fmt::Result {
        f.node("BinaryOp")
            .child(self.left)
            .child_contents(self.op_span)
            .child(self.right)
            .finish()
    }
}

#[derive(Debug)]
pub struct Not {
    pub exclamation: LocalSpan,
    pub inner: NodeId,
}

impl From<Not> for Node {
    fn from(inner: Not) -> Self {
        Self::Not(inner)
    }
}

impl AstNode for Not {
    fn span(&self, list: &NodeList) -> LocalSpan {
        self.exclamation + list.span(self.inner)
    }

    fn ast_display(&self, f: &mut display::AstFormatter) -> std::fmt::Result {
        f.node("Not").child(self.inner).finish()
    }
}

#[derive(Debug)]
pub struct FnCall {
    pub callee: NodeId,
    pub lparen_token: TokenLocalSpan,
    pub args: Vec<NodeId>,
    pub rparen_token: TokenLocalSpan,
}

impl From<FnCall> for Node {
    fn from(inner: FnCall) -> Self {
        Self::FnCall(inner)
    }
}

impl AstNode for FnCall {
    fn span(&self, list: &NodeList) -> LocalSpan {
        list.span(self.callee) + self.rparen_token.span
    }

    fn ast_display(&self, f: &mut display::AstFormatter) -> std::fmt::Result {
        f.node("FnCall")
            .child(self.callee)
            .children(&self.args)
            .finish()
    }
}

#[derive(Debug)]
pub struct IfCondition {
    pub condition: NodeId,
    pub then_block: NodeId,
    pub alternate: Option<AlternateCondition>,
}

#[derive(Debug)]
pub enum AlternateCondition {
    IfElseCondition(NodeId),
    ElseBlock(NodeId),
}

impl From<IfCondition> for Node {
    fn from(inner: IfCondition) -> Self {
        Self::IfCondition(inner)
    }
}

impl AstNode for IfCondition {
    fn span(&self, list: &NodeList) -> LocalSpan {
        list.span(self.condition)
            + match &self.alternate {
                Some(
                    AlternateCondition::IfElseCondition(nid) | AlternateCondition::ElseBlock(nid),
                ) => list.span(*nid),
                None => list.span(self.then_block),
            }
    }

    fn ast_display(&self, f: &mut display::AstFormatter) -> std::fmt::Result {
        let mut b = f.node("IfCondition");
        b.child(self.condition).child(self.then_block);
        match &self.alternate {
            Some(AlternateCondition::IfElseCondition(id) | AlternateCondition::ElseBlock(id)) => {
                b.child(*id)
            }
            None => &mut b,
        }
        .finish()
    }
}

#[derive(Debug)]
pub struct Assignment {
    pub name: TokenLocalSpan,
    pub equals_token: TokenLocalSpan,
    pub expr: NodeId,
}

impl From<Assignment> for Node {
    fn from(inner: Assignment) -> Self {
        Self::Assignment(inner)
    }
}

impl AstNode for Assignment {
    fn span(&self, list: &NodeList) -> LocalSpan {
        self.name.span + list.span(self.expr)
    }

    fn ast_display(&self, f: &mut display::AstFormatter) -> std::fmt::Result {
        f.node("Assignment")
            .child_contents(self.name.span)
            .child(self.expr)
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOperatorKind {
    Addition,
    Subtraction,
    Multiplication,
    Division,
    Dot,
    LogicalAnd,
    LogicalOr,
    Equals,
    NotEquals,
    LessThan,
    LessThanEquals,
    GreaterThan,
    GreaterThanEquals,
}

impl BinaryOperatorKind {
    fn new_from_token(t: Token) -> Option<Self> {
        match t {
            Token::Star => Some(Self::Multiplication),
            Token::Slash => Some(Self::Division),
            Token::Plus => Some(Self::Addition),
            Token::Subtract => Some(Self::Subtraction),
            Token::Dot => Some(Self::Dot),
            Token::Equals => Some(Self::Equals),
            Token::NotEquals => Some(Self::NotEquals),
            Token::LogicalAnd => Some(Self::LogicalAnd),
            Token::LogicalOr => Some(Self::LogicalOr),
            Token::LAngle => Some(Self::LessThan),
            Token::LessEq => Some(Self::LessThanEquals),
            Token::RAngle => Some(Self::GreaterThan),
            Token::GreaterEq => Some(Self::GreaterThanEquals),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct IntegerLiteral(pub TokenLocalSpan);

impl From<IntegerLiteral> for Node {
    fn from(inner: IntegerLiteral) -> Self {
        Self::IntegerLiteral(inner)
    }
}

impl AstNode for IntegerLiteral {
    fn span(&self, _list: &NodeList) -> LocalSpan {
        self.0.span
    }

    fn ast_display(&self, f: &mut display::AstFormatter) -> std::fmt::Result {
        f.node("IntegerLiteral")
            .child_contents(self.0.span)
            .finish()
    }
}

#[derive(Debug)]
pub struct InternalLiteral(pub TokenLocalSpan);

impl From<InternalLiteral> for Node {
    fn from(inner: InternalLiteral) -> Self {
        Self::InternalLiteral(inner)
    }
}

impl AstNode for InternalLiteral {
    fn span(&self, _list: &NodeList) -> LocalSpan {
        self.0.span
    }

    fn ast_display(&self, f: &mut display::AstFormatter) -> std::fmt::Result {
        f.node("IntegerLiteral")
            .child_contents(self.0.span)
            .finish()
    }
}

#[derive(Debug)]
pub struct TrueLiteral(pub TokenLocalSpan);

impl From<TrueLiteral> for Node {
    fn from(inner: TrueLiteral) -> Self {
        Self::TrueLiteral(inner)
    }
}

impl AstNode for TrueLiteral {
    fn span(&self, _list: &NodeList) -> LocalSpan {
        self.0.span
    }

    fn ast_display(&self, f: &mut display::AstFormatter) -> std::fmt::Result {
        f.node("TrueLiteral").finish()
    }
}

#[derive(Debug)]
pub struct FalseLiteral(pub TokenLocalSpan);

impl From<FalseLiteral> for Node {
    fn from(inner: FalseLiteral) -> Self {
        Self::FalseLiteral(inner)
    }
}

impl AstNode for FalseLiteral {
    fn span(&self, _list: &NodeList) -> LocalSpan {
        self.0.span
    }

    fn ast_display(&self, f: &mut display::AstFormatter) -> std::fmt::Result {
        f.node("FalseLiteral").finish()
    }
}

#[derive(Debug)]
pub struct Ident(pub TokenLocalSpan);

impl From<Ident> for Node {
    fn from(inner: Ident) -> Self {
        Self::Ident(inner)
    }
}

impl AstNode for Ident {
    fn span(&self, _list: &NodeList) -> LocalSpan {
        self.0.span
    }

    fn ast_display(&self, f: &mut display::AstFormatter) -> std::fmt::Result {
        f.node("Ident").child_contents(self.0.span).finish()
    }
}

#[derive(Debug)]
pub struct Record {
    pub lbrace: LocalSpan,
    pub fields: Vec<RecordField>,
    pub rbrace: LocalSpan,
}

impl From<Record> for Node {
    fn from(inner: Record) -> Self {
        Self::Record(inner)
    }
}

impl AstNode for Record {
    fn span(&self, _list: &NodeList) -> LocalSpan {
        self.lbrace + self.rbrace
    }

    fn ast_display(&self, f: &mut display::AstFormatter) -> std::fmt::Result {
        let mut b = f.node("Record");
        for e in &self.fields {
            b.child_fn(|f| e.ast_display(f));
        }
        b.finish()
    }
}

#[derive(Debug)]
pub struct RecordField {
    pub key: TokenLocalSpan,
    pub equals: LocalSpan,
    pub value: NodeId,
    pub comma: Option<LocalSpan>,
}

impl AstNode for RecordField {
    fn span(&self, list: &NodeList) -> LocalSpan {
        self.key.span + self.comma.unwrap_or_else(|| list.span(self.value))
    }

    fn ast_display(&self, f: &mut display::AstFormatter) -> std::fmt::Result {
        f.node("RecordEntry")
            .child_contents(self.key.span)
            .child(self.value)
            .finish()
    }
}

#[derive(Debug)]
pub struct List {
    pub lsqbracket: LocalSpan,
    pub elements: Vec<ListElement>,
    pub rsqbracket: LocalSpan,
}

impl From<List> for Node {
    fn from(inner: List) -> Self {
        Self::List(inner)
    }
}

impl AstNode for List {
    fn span(&self, _list: &NodeList) -> LocalSpan {
        self.lsqbracket + self.rsqbracket
    }

    fn ast_display(&self, f: &mut display::AstFormatter) -> std::fmt::Result {
        let mut b = f.node("List");
        for e in &self.elements {
            b.child_fn(|f| e.ast_display(f));
        }
        b.finish()
    }
}

#[derive(Debug)]
pub struct ListElement {
    pub value: NodeId,
    pub comma: Option<LocalSpan>,
}

impl AstNode for ListElement {
    fn span(&self, list: &NodeList) -> LocalSpan {
        let value_span = list.span(self.value);
        if let Some(&s) = self.comma.as_ref() {
            value_span + s
        } else {
            value_span
        }
    }

    fn ast_display(&self, f: &mut display::AstFormatter) -> std::fmt::Result {
        f.node("ListElement").child(self.value).finish()
    }
}
