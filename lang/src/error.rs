use serde::{Deserialize, Serialize};

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

impl std::error::Error for ResolvedError<'_> {}

impl std::fmt::Display for ResolvedError<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.into_owned().fmt(f)
    }
}

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

impl std::fmt::Display for OwnedResolvedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use colored::Colorize as _;
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
        let location = format!("{file_name}:{line}:{col}\n").blue();
        f.write_fmt(format_args!("{location}"))?;
        let contents = contents.red();
        f.write_fmt(format_args!("| {before}{contents}{after}\n"))?;
        let err = err.red();
        let arrows = arrows.red();
        f.write_fmt(format_args!("  {arrows} {err}"))?;
        Ok(())
    }
}

impl std::error::Error for OwnedResolvedError {}
