use unicode_width::UnicodeWidthStr;

#[derive(Debug)]
pub struct Padding {
    pub tabs: usize,
    pub spaces: usize,
}

impl Padding {
    /// Creates new padding that matches the length of the string so that when they are printed on lines they have the same length
    pub fn new_matching_string(s: &str) -> Self {
        let tabs = s.chars().filter(|&c| c == '\t').count();
        let spaces = UnicodeWidthStr::width(s);
        Self { tabs, spaces }
    }

    pub fn pad_with_whitespace(&self) -> String {
        "\t".repeat(self.tabs) + &" ".repeat(self.spaces)
    }

    pub fn pad_with_char(&self, c: char, tab_size: usize) -> String {
        c.to_string().repeat(self.spaces + self.tabs * tab_size)
    }
}
