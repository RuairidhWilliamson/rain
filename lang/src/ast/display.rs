use std::fmt::{Result, Write};

use crate::{span::LocalSpan, tokens::TokenLocalSpan};

/// # Panics
/// Panics if a formatter error occurs
pub fn display_ast(ast: &dyn AstDisplay, src: &str) -> String {
    let mut buf = String::new();
    let mut f = AstFormatter {
        src,
        indent: 0,
        buf: &mut buf,
    };
    ast.fmt(&mut f)
        .expect("a AstDisplay implementation returned an error unexpectedly");
    buf
}

pub trait AstDisplay {
    fn span(&self) -> LocalSpan;
    fn fmt(&self, f: &mut AstFormatter<'_>) -> Result;
}

pub struct AstFormatter<'a> {
    src: &'a str,
    indent: usize,
    buf: &'a mut (dyn Write + 'a),
}

impl<'b> AstFormatter<'b> {
    pub fn node<'a>(&'a mut self, name: &str) -> NodeBuilder<'a, 'b>
    where
        'b: 'a,
    {
        NodeBuilder::new(self, name)
    }

    pub fn node_single_child(&mut self, name: &str, child: &dyn AstDisplay) -> Result {
        self.buf.write_str(name)?;
        self.buf.write_char('(')?;
        child.fmt(self)?;
        self.buf.write_char(')')
    }

    fn write_indent(&mut self) -> Result {
        for _ in 0..self.indent {
            self.buf.write_char(' ')?;
        }
        Ok(())
    }
}

pub struct NodeBuilder<'a, 'b: 'a> {
    fmt: &'a mut AstFormatter<'b>,
    has_children: bool,
    result: Result,
}

impl<'a, 'b> NodeBuilder<'a, 'b> {
    pub fn new(fmt: &'a mut AstFormatter<'b>, name: &str) -> Self {
        let result = fmt
            .buf
            .write_str(name)
            .and_then(|()| fmt.buf.write_str("(\n"));
        fmt.indent += 1;
        Self {
            fmt,
            has_children: false,
            result,
        }
    }

    fn child_fn(&mut self, f: impl FnOnce(&mut AstFormatter<'b>) -> Result) -> &mut Self {
        self.result = self.result.and_then(|()| {
            self.fmt.write_indent()?;
            f(self.fmt)?;
            self.fmt.buf.write_str("\n")
        });
        self.has_children = true;
        self
    }

    pub fn child(&mut self, child: &dyn AstDisplay) -> &mut Self {
        self.child_fn(|f| child.fmt(f))
    }

    pub fn children<T: AstDisplay + 'a>(
        &mut self,
        children: impl IntoIterator<Item = &'a T>,
    ) -> &mut Self {
        self.result = self.result.and_then(|()| {
            self.fmt.write_indent()?;
            self.fmt.buf.write_str("(\n")?;
            self.fmt.indent += 1;
            for c in children {
                self.fmt.write_indent()?;
                c.fmt(self.fmt)?;
                self.fmt.buf.write_str("\n")?;
            }
            self.fmt.indent -= 1;
            self.fmt.write_indent()?;
            self.fmt.buf.write_str(")\n")
        });
        self.has_children = true;
        self
    }

    pub fn named_child(&mut self, name: &str, child: &dyn AstDisplay) -> &mut Self {
        self.child_fn(|f| f.node(name).child(child).finish())
    }

    pub fn finish(&mut self) -> Result {
        self.result?;
        self.fmt.indent -= 1;
        self.fmt.write_indent()?;
        self.fmt.buf.write_str(")")?;
        Ok(())
    }
}

impl AstDisplay for LocalSpan {
    fn span(&self) -> LocalSpan {
        *self
    }

    fn fmt(&self, f: &mut AstFormatter<'_>) -> Result {
        f.buf.write_str(self.contents(f.src))
    }
}

impl AstDisplay for TokenLocalSpan {
    fn span(&self) -> LocalSpan {
        self.span
    }

    fn fmt(&self, f: &mut AstFormatter<'_>) -> Result {
        f.node_single_child(&format!("{:?}", self.token), &self.span)
    }
}
