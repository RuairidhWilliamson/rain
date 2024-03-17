use crate::utils::Padding;

#[derive(Default, Clone, Copy)]
pub struct Place {
    // Zero based byte index of the place in the file
    pub index: usize,
    // Zero based line number of the place in the file
    pub line: usize,
    // Zero based column number of the place in the line
    pub column: usize,
}

impl std::fmt::Debug for Place {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "({:?} {:?}:{:?})",
            self.index, self.line, self.column
        ))
    }
}

impl PartialEq for Place {
    fn eq(&self, other: &Self) -> bool {
        self.index.eq(&other.index)
    }
}

impl Eq for Place {}

impl PartialOrd for Place {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.index.cmp(&other.index))
    }
}

impl Ord for Place {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.index.cmp(&other.index)
    }
}

#[derive(Default, PartialEq, Eq, Clone, Copy)]
pub struct Span {
    // Inclusive start of span
    pub start: Place,
    // Exclusive end of span
    pub end: Place,
}

impl std::fmt::Debug for Span {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("[{:?} -> {:?}]", self.start, self.end))
    }
}

impl Span {
    pub fn new(start: Place, end: Place) -> Self {
        assert!(start <= end);
        Self { start, end }
    }

    pub fn new_single_byte(place: Place) -> Self {
        Self::new(
            place,
            Place {
                index: place.index + 1,
                line: place.line,
                column: place.column + 1,
            },
        )
    }

    pub fn len(&self) -> usize {
        self.end.index - self.start.index
    }

    pub fn is_empty(&self) -> bool {
        self.end.index == self.start.index
    }

    pub fn lines(&self) -> usize {
        self.end.line - self.start.line
    }

    pub fn is_multiline(&self) -> bool {
        self.end.line != self.start.line
    }

    pub fn combine(self, other: Self) -> Self {
        assert!(self.start <= other.end);
        Self {
            start: self.start,
            end: other.end,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn extract<'a>(&self, source: &'a str) -> Extract<'a> {
        assert_eq!(self.start.line, self.end.line);

        let line_start = source[..self.start.index]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);
        let line_end = source[self.end.index..]
            .find('\n')
            .map(|i| i + self.end.index)
            .unwrap_or(source.len());
        let line = &source[line_start..line_end];

        Extract {
            line,
            exact: self.start.index - line_start..self.end.index - line_start,
        }
    }
}

#[derive(Debug)]
pub struct Extract<'a> {
    pub line: &'a str,
    /// The index of the extract within the line
    pub exact: std::ops::Range<usize>,
}

impl Extract<'_> {
    pub fn before_exact(&self) -> &str {
        &self.line[..self.exact.start]
    }

    pub fn exact_str(&self) -> &str {
        &self.line[self.exact.clone()]
    }

    pub fn under_arrows(&self) -> String {
        let padding = Padding::new_matching_string(self.before_exact());
        let arrows = Padding::new_matching_string(self.exact_str());
        padding.pad_with_whitespace() + &arrows.pad_with_char('^', 4)
    }
}
