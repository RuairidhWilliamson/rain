#[cfg(test)]
mod test;

mod display;
pub mod error;
pub mod parser;

use crate::{
    local_span::LocalSpan,
    tokens::{StringLiteralPrefix, Token, TokenLocalSpan},
};

#[derive(Debug)]
pub struct Module {
    pub root: NodeId,
    nodes: NodeList,
}

impl Module {
    pub fn display(&self, src: &str) -> String {
        self.nodes.display(src, self.root)
    }

    pub fn get(&self, id: NodeId) -> &Node {
        self.nodes.get(id)
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

    fn display(&self, src: &str, id: NodeId) -> String {
        let node = self.get(id);
        let mut buf = String::new();
        let mut f = display::AstFormatter::new(src, &mut buf, &self);
        node.ast_display(&mut f).unwrap();
        buf
    }

    fn push(&mut self, node: impl Into<Node>) -> NodeId {
        let index = self.nodes.len();
        self.nodes.push(node.into());
        NodeId(index)
    }

    fn get(&self, id: NodeId) -> &Node {
        let Some(node) = self.nodes.get(id.0) else {
            unreachable!()
        };
        node
    }
}

#[derive(Debug, Clone, Copy)]
pub struct NodeId(usize);

#[derive(Debug)]
pub enum Node {
    ModuleRoot(ModuleRoot),
    LetDeclare(LetDeclare),
    FnDeclare(FnDeclare),
    Block(Block),
    IfCondition(IfCondition),
    FnCall(FnCall),
    Assignment(Assignment),
    BinaryOp(BinaryOp),
    Ident(TokenLocalSpan),
    Internal(TokenLocalSpan),
    StringLiteral(StringLiteral),
    IntegerLiteral(TokenLocalSpan),
    TrueLiteral(TokenLocalSpan),
    FalseLiteral(TokenLocalSpan),
}

impl Node {
    fn ast_display(&self, f: &mut display::AstFormatter) -> std::fmt::Result {
        match self {
            Self::ModuleRoot(module_root) => f
                .node("ModuleRoot")
                .children(&module_root.declarations)
                .finish(),
            Self::LetDeclare(let_declare) => f
                .node("LetDeclare")
                .child_contents(let_declare.name.span)
                .child(let_declare.expr)
                .finish(),
            Self::FnDeclare(fn_declare) => f
                .node("FnDeclare")
                .child_contents(fn_declare.name.span)
                .child(fn_declare.block)
                .finish(),
            Self::Block(block) => f.node("Block").children(&block.statements).finish(),
            Self::IfCondition(if_condition) => {
                let mut b = f.node("IfCondition");
                b.child(if_condition.condition)
                    .child(if_condition.then_block);
                match &if_condition.alternate {
                    Some(AlternateCondition::IfElseCondition(id)) => b.child(*id),
                    Some(AlternateCondition::ElseBlock(id)) => b.child(*id),
                    None => &mut b,
                }
                .finish()
            }
            Self::FnCall(fn_call) => f
                .node("FnCall")
                .child(fn_call.callee)
                .children(&fn_call.args)
                .finish(),
            Self::Assignment(assignment) => f
                .node("Assignment")
                .child_contents(assignment.name.span)
                .child(assignment.expr)
                .finish(),
            Self::BinaryOp(op) => f
                .node("BinaryOp")
                .child(op.left)
                .child_contents(op.op_span)
                .child(op.right)
                .finish(),
            Self::Ident(tls) => f.node("Ident").child_contents(tls.span).finish(),
            Self::Internal(tls) => f.node("Internal").child_contents(tls.span).finish(),
            Self::StringLiteral(lit) => f.node("StringLiteral").child_contents(lit.0.span).finish(),
            Self::IntegerLiteral(tls) => f.node("IntegerLiteral").child_contents(tls.span).finish(),
            Self::TrueLiteral(_) => f.node("TrueLiteral").finish(),
            Self::FalseLiteral(_) => f.node("FalseLiteral").finish(),
        }
    }
}

#[derive(Debug)]
pub struct ModuleRoot {
    pub declarations: Vec<NodeId>,
}

impl From<ModuleRoot> for Node {
    fn from(inner: ModuleRoot) -> Self {
        Self::ModuleRoot(inner)
    }
}

#[derive(Debug)]
pub struct LetDeclare {
    pub let_token: TokenLocalSpan,
    pub name: TokenLocalSpan,
    pub equals_token: TokenLocalSpan,
    pub expr: NodeId,
}

impl From<LetDeclare> for Node {
    fn from(inner: LetDeclare) -> Self {
        Self::LetDeclare(inner)
    }
}

#[derive(Debug)]
pub struct FnDeclare {
    pub fn_token: TokenLocalSpan,
    pub name: TokenLocalSpan,
    pub lparen_token: TokenLocalSpan,
    pub args: Vec<FnDeclareArg>,
    pub rparen_token: TokenLocalSpan,
    pub block: NodeId,
}

impl From<FnDeclare> for Node {
    fn from(inner: FnDeclare) -> Self {
        Self::FnDeclare(inner)
    }
}

#[derive(Debug)]
pub struct FnDeclareArg {
    pub name: TokenLocalSpan,
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

#[derive(Debug)]
pub struct StringLiteral(pub TokenLocalSpan);

impl StringLiteral {
    pub fn prefix(&self) -> Option<StringLiteralPrefix> {
        let Token::DoubleQuoteLiteral(prefix) = self.0.token else {
            unreachable!()
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

#[derive(Debug)]
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
            _ => None,
        }
    }
}
