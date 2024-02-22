#[derive(Debug, Default, Clone, Copy)]
pub struct Place {
    // Zero based byte index of the place in the file
    pub index: usize,
    // Zero based line number of the place in the file
    pub line: usize,
    // Zero based column number of the place in the line
    pub column: usize,
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

#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
pub struct Span {
    // Inclusive start of span
    pub start: Place,
    // Exclusive end of span
    pub end: Place,
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
}
