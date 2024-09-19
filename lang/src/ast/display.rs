use std::fmt::{Result, Write};

use crate::local_span::LocalSpan;

use super::{NodeId, NodeList};

pub struct AstFormatter<'a> {
    src: &'a str,
    indent: usize,
    buf: &'a mut (dyn Write + 'a),
    nodes: &'a NodeList,
}
impl<'a> AstFormatter<'a> {
    pub fn new(src: &'a str, buf: &'a mut (dyn Write + 'a), nodes: &'a NodeList) -> Self {
        Self {
            src,
            indent: 0,
            buf,
            nodes,
        }
    }

    pub fn node<'b>(&'b mut self, name: &str) -> NodeBuilder<'b, 'a>
    where
        'a: 'b,
    {
        {
            let result = self
                .buf
                .write_str(name)
                .and_then(|()| self.buf.write_str("(\n"));
            self.indent += 1;
            NodeBuilder {
                fmt: self,
                has_children: false,
                result,
            }
        }
    }

    fn write_indent(&mut self) -> Result {
        for _ in 0..self.indent {
            self.buf.write_char(' ')?;
        }
        Ok(())
    }
}

pub struct NodeBuilder<'b, 'a: 'b> {
    fmt: &'b mut AstFormatter<'a>,
    has_children: bool,
    result: Result,
}

impl<'b, 'a> NodeBuilder<'b, 'a> {
    pub fn child_fn(&mut self, f: impl FnOnce(&mut AstFormatter<'a>) -> Result) -> &mut Self {
        self.result = self.result.and_then(|()| {
            self.fmt.write_indent()?;
            f(self.fmt)?;
            self.fmt.buf.write_str("\n")
        });
        self.has_children = true;
        self
    }

    pub fn child_contents(&mut self, span: LocalSpan) -> &mut Self {
        self.child_fn(|f| f.buf.write_str(span.contents(self.fmt.src)))
    }

    pub fn child(&mut self, id: NodeId) -> &mut Self {
        let child = self.fmt.nodes.get(id);
        self.child_fn(|f| child.ast_display(f))
    }

    pub fn children<'id>(&mut self, children: impl IntoIterator<Item = &'id NodeId>) -> &mut Self {
        for c in children.into_iter().copied() {
            let child = self.fmt.nodes.get(c);
            self.child_fn(|f| child.ast_display(f));
        }
        self
    }

    pub fn finish(&mut self) -> Result {
        self.result?;
        self.fmt.indent -= 1;
        self.fmt.write_indent()?;
        self.fmt.buf.write_str(")")?;
        Ok(())
    }
}
