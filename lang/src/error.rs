use serde::{Deserialize, Serialize};
use termcolor::{NoColor, WriteColor};

use crate::{afs::file::File, local_span::LocalSpan};

#[derive(Debug)]
pub struct ResolvedError<'a> {
    pub err: &'a dyn std::error::Error,
    pub file: &'a File,
    pub src: &'a str,
    pub span: LocalSpan,
}

impl ResolvedError<'_> {
    pub fn into_owned(&self) -> OwnedResolvedError {
        let Self {
            err,
            file,
            src,
            span,
        } = self;
        let (line, col) = span.line_col(src);
        let [before, contents, after] = span.surrounding_lines(src, 2);
        let before = before.replace('\n', "\n| ");
        let contents = contents.replace('\n', "\\n");
        let after = after.to_string();
        let arrows = span.arrow_line(src, 2);
        let err = err.to_string();
        OwnedResolvedError {
            file_name: format!("{file}"),
            line,
            col,
            before,
            contents,
            after,
            arrows,
            err,
        }
    }
}

impl ResolvedError<'_> {
    pub fn write_color(&self, bufwtr: &mut impl WriteColor) -> std::io::Result<()> {
        self.into_owned().write_color(bufwtr)
    }
}

impl std::fmt::Display for ResolvedError<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.into_owned().fmt(f)
    }
}

impl std::error::Error for ResolvedError<'_> {}

#[derive(Debug, Serialize, Deserialize)]
pub struct OwnedResolvedError {
    pub file_name: String,
    pub line: usize,
    pub col: usize,
    pub before: String,
    pub contents: String,
    pub after: String,
    pub arrows: String,
    pub err: String,
}

impl OwnedResolvedError {
    pub fn write_color(&self, writer: &mut impl WriteColor) -> std::io::Result<()> {
        use termcolor::{Color, ColorSpec};
        let Self {
            file_name,
            line,
            col,
            before,
            contents,
            after,
            arrows,
            err,
        } = self;
        writer.set_color(ColorSpec::new().set_fg(Some(Color::Blue)))?;
        writeln!(writer, "{file_name}:{line}:{col}")?;
        writer.set_color(ColorSpec::new().set_fg(None))?;
        writeln!(writer, "| {before}{contents}{after}")?;
        writer.set_color(ColorSpec::new().set_fg(Some(Color::Red)))?;
        writeln!(writer, "  {arrows} {err}")?;
        Ok(())
    }
}

impl std::fmt::Display for OwnedResolvedError {
    #[expect(clippy::unwrap_used)]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut data = Vec::new();
        let mut buf = NoColor::new(&mut data);
        self.write_color(&mut buf).unwrap();
        let s = std::str::from_utf8(&data).unwrap();
        f.write_str(s)
    }
}

impl std::error::Error for OwnedResolvedError {}
