use std::ops::{Add, AddAssign};

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

    fn line(&self, src: &str) -> usize {
        src[..self.start].chars().filter(|&c| c == '\n').count()
    }

    fn col(&self, src: &str) -> usize {
        src[..self.start]
            .chars()
            .rev()
            .take_while(|&c| c != '\n')
            .count()
    }

    /// Get the 1 based line and column
    pub fn line_col(&self, src: &str) -> (usize, usize) {
        let line = self.line(src) + 1;
        let col = self.col(src) + 1;
        (line, col)
    }

    pub fn is_empty(&self) -> bool {
        self.end == self.start
    }

    pub fn len(&self) -> usize {
        self.end - self.start
    }

    pub fn span_iter(iter: impl Iterator<Item = Self>) -> Self {
        let mut acc = Self::default();
        for s in iter {
            acc += s;
        }
        acc
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
