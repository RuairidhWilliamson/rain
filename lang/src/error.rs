use std::path::Path;

use crate::local_span::LocalSpan;

#[derive(Debug, Clone)]
pub struct ErrorLocalSpan<E: std::error::Error> {
    pub err: E,
    pub span: Option<LocalSpan>,
}

impl<E: std::error::Error> ErrorLocalSpan<E> {
    pub fn new(err: E, span: Option<LocalSpan>) -> Self {
        Self { err, span }
    }
}

impl<E: std::error::Error> ErrorLocalSpan<E> {
    pub fn resolve<'a>(&'a self, path: Option<&'a Path>, src: &'a str) -> ResolvedError<'a> {
        ResolvedError {
            err: &self.err,
            path,
            src,
            span: self.span,
        }
    }
}

impl LocalSpan {
    pub const fn with_error<E: std::error::Error>(self, err: E) -> ErrorLocalSpan<E> {
        ErrorLocalSpan {
            err,
            span: Some(self),
        }
    }
}

#[derive(Debug)]
pub struct ResolvedError<'a> {
    err: &'a dyn std::error::Error,
    path: Option<&'a Path>,
    src: &'a str,
    span: Option<LocalSpan>,
}

impl std::fmt::Display for ResolvedError<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use colored::Colorize;
        let Self {
            err,
            path,
            src,
            span,
            ..
        } = self;
        let span = span.unwrap();
        let (line, col) = span.line_col(src);
        let path = path.unwrap_or(Path::new("<unknown>"));
        let location = format!("{}:{}:{}\n", path.display(), line, col).blue();
        f.write_fmt(format_args!("{location}"))?;
        let [before, contents, after] = span.surrounding_lines(src, 2);
        let before = before.replace('\n', "\n| ");
        let contents = contents.replace('\n', "\\n");
        f.write_fmt(format_args!("| {before}{contents}{after}\n"))?;
        let arrows = span.arrow_line(src, 2).red();
        let err = format!("{err}").red();
        f.write_fmt(format_args!("| {arrows} {err}"))?;
        Ok(())
    }
}
