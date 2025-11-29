use std::ops::{Add, AddAssign};

use crate::{
    afs::file::File,
    error::{ResolvedError, ResolvedSpan},
    ir::ModuleId,
    span::{ErrorSpan, Span},
};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct LocalSpan {
    pub start: usize,
    pub end: usize,
}

impl LocalSpan {
    pub fn byte(index: usize) -> Self {
        Self {
            start: index,
            end: index + 1,
        }
    }

    /// Create a span from the zero based line column
    pub fn byte_from_line_colz(src: &str, mut line: usize, col: usize) -> Option<Self> {
        let mut iter = src.char_indices();
        while line > 0 {
            if iter.next()?.1 == '\n' {
                line -= 1;
            }
        }
        Some(Self::byte(iter.next()?.0 + col))
    }

    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn rng(&self) -> std::ops::Range<usize> {
        self.start..self.end
    }

    pub fn contents<'a>(&self, src: &'a str) -> &'a str {
        &src[self.rng()]
    }

    pub fn surrounding_lines<'a>(&self, src: &'a str, before_lines: usize) -> [&'a str; 3] {
        let mut new_line_count = 0;
        let start_offset: usize = src[..self.start]
            .chars()
            .rev()
            .take_while(|&c| {
                if c == '\n' {
                    new_line_count += 1;
                    new_line_count <= before_lines
                } else {
                    true
                }
            })
            .map(char::len_utf8)
            .sum();
        let end_offset: usize = src[self.end..]
            .chars()
            .take_while(|&c| c != '\n')
            .map(char::len_utf8)
            .sum();

        [
            &src[self.start - start_offset..self.start],
            self.contents(src),
            &src[self.end..self.end + end_offset],
        ]
    }

    pub fn arrow_line(&self, src: &str, new_line_size: usize) -> String {
        let a: String = src[..self.start]
            .chars()
            .rev()
            .take_while(|&c| c != '\n')
            .map(|c| match c {
                '\t' => c,
                _ => ' ',
            })
            .collect::<String>()
            .chars()
            .rev()
            .collect();
        let contents = self.contents(src);
        let extra_len = contents.chars().filter(|&c| c == '\n').count() * (new_line_size - 1);
        let len = self.len() + extra_len;
        let b: String = std::iter::once('^').cycle().take(len).collect();
        format!("{a}{b}")
    }

    fn start_line(&self, src: &str) -> usize {
        src[..self.start].chars().filter(|&c| c == '\n').count()
    }

    fn start_col(&self, src: &str) -> usize {
        src[..self.start]
            .chars()
            .rev()
            .take_while(|&c| c != '\n')
            .count()
    }

    fn end_line(&self, src: &str) -> usize {
        src[..self.end].chars().filter(|&c| c == '\n').count()
    }

    fn end_col(&self, src: &str) -> usize {
        src[..self.end]
            .chars()
            .rev()
            .take_while(|&c| c != '\n')
            .count()
    }

    /// Get the 0 based line and column
    pub fn start_line_colz(&self, src: &str) -> (usize, usize) {
        let line = self.start_line(src);
        let col = self.start_col(src);
        (line, col)
    }

    /// Get the 1 based line and column
    pub fn start_line_colo(&self, src: &str) -> (usize, usize) {
        let (line, col) = self.start_line_colz(src);
        (line + 1, col + 1)
    }

    /// Get the 0 based line and column
    pub fn end_line_colz(&self, src: &str) -> (usize, usize) {
        let line = self.end_line(src);
        let col = self.end_col(src);
        (line, col)
    }

    pub fn is_empty(&self) -> bool {
        self.end == self.start
    }

    pub fn len(&self) -> usize {
        self.end - self.start
    }

    pub fn contains(&self, other: &Self) -> bool {
        self.start <= other.start && self.end >= other.end
    }

    pub fn span_iter(iter: impl Iterator<Item = Self>) -> Self {
        let mut acc = Self::default();
        for s in iter {
            acc += s;
        }
        acc
    }

    pub const fn with_module(self, module_id: ModuleId) -> Span {
        Span {
            module: module_id,
            span: self,
        }
    }

    pub const fn with_error<E: std::error::Error>(self, err: E) -> ErrorLocalSpan<E> {
        ErrorLocalSpan { err, span: self }
    }
}

impl From<&Self> for LocalSpan {
    fn from(value: &Self) -> Self {
        *value
    }
}

impl Add for LocalSpan {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            start: self.start.min(rhs.start),
            end: self.end.max(rhs.end),
        }
    }
}

impl AddAssign for LocalSpan {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorLocalSpan<E: std::error::Error> {
    pub err: E,
    pub span: LocalSpan,
}

impl<E: std::error::Error> ErrorLocalSpan<E> {
    pub fn resolve<'a>(&'a self, file: Option<&'a File>, src: &'a str) -> ResolvedError<'a> {
        ResolvedError {
            err: &self.err,
            trace: vec![ResolvedSpan {
                file,
                src,
                call_span: self.span,
                declaration_span: None,
            }],
        }
    }

    pub fn upgrade(self, module_id: ModuleId) -> ErrorSpan<E> {
        self.span.with_module(module_id).with_error(self.err)
    }
}
