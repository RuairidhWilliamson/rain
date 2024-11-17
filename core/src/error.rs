use crate::{afs::file::File, local_span::LocalSpan};

#[derive(Debug)]
pub struct ResolvedError<'a> {
    pub err: &'a dyn std::error::Error,
    pub file: &'a File,
    pub src: &'a str,
    pub span: LocalSpan,
}

impl std::error::Error for ResolvedError<'_> {}

impl std::fmt::Display for ResolvedError<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use colored::Colorize;
        let Self {
            err,
            file,
            src,
            span,
        } = self;
        let (line, col) = span.line_col(src);
        let location = format!("{file}:{line}:{col}\n").blue();
        f.write_fmt(format_args!("{location}"))?;
        let [before, contents, after] = span.surrounding_lines(src, 2);
        let before = before.replace('\n', "\n| ");
        let contents = contents.replace('\n', "\\n");
        f.write_fmt(format_args!("| {before}{contents}{after}\n"))?;
        let arrows = span.arrow_line(src, 2).red();
        let err = format!("{err}").red();
        f.write_fmt(format_args!("  {arrows} {err}"))?;
        Ok(())
    }
}
