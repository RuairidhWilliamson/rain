use std::io::{Write as _, stderr};

use rain_core::rain_lang::driver::monitoring::Call;
use termcolor::{Color, ColorSpec, StandardStream, WriteColor as _};

use crate::{GlobalOptions, ReportMode, remote::msg::run::RunProgress};

pub trait Reporter {
    fn update(&mut self, progress: RunProgress);
}

pub fn new_reporter(options: &GlobalOptions) -> Box<dyn Reporter> {
    match options.report {
        ReportMode::Basic => Box::new(Basic::default()),
        ReportMode::Verbose => Box::new(Verbose::default()),
        ReportMode::Tree => Box::new(Tree::new(options)),
        ReportMode::None => Box::new(Noop),
    }
}

pub struct Noop;

impl Reporter for Noop {
    fn update(&mut self, _progress: RunProgress) {}
}

#[derive(Default)]
pub struct Basic {
    stack: Vec<String>,
}

impl Reporter for Basic {
    fn update(&mut self, progress: RunProgress) {
        match progress {
            RunProgress::Print(s) => {
                let mut stderr = StandardStream::stderr(termcolor::ColorChoice::Auto);
                let _ = stderr.set_color(ColorSpec::new().set_fg(Some(Color::Rgb(127, 127, 127))));
                let _ = writeln!(stderr, "{s}");
                let _ = stderr.reset();
            }
            RunProgress::EnterCall(Call::Custom(s)) => {
                let mut stderr = StandardStream::stderr(termcolor::ColorChoice::Auto);
                let _ = stderr.set_color(ColorSpec::new().set_fg(Some(Color::Rgb(200, 200, 200))));
                let _ = writeln!(stderr, "{s}");
                let _ = stderr.reset();
            }
            _ => {}
        }
        if let Some(last) = self.stack.last() {
            eprintln!("{last}");
        }
        let _ = stderr().flush();
    }
}

#[derive(Default)]
pub struct Verbose {
    stack: Vec<Call>,
}

impl Reporter for Verbose {
    fn update(&mut self, progress: RunProgress) {
        match progress {
            RunProgress::Print(s) => eprintln!("{s}"),
            RunProgress::EnterCall(s) => {
                self.stack.push(s);
            }
            RunProgress::ExitCall(_) => {
                self.stack.pop();
            }
        }
        if let Some(last) = self.stack.last() {
            eprintln!("{last:?}");
        }
        let _ = stderr().flush();
    }
}

#[derive(Default)]
pub struct Tree {
    stack: Vec<Call>,
    depth: usize,
}

impl Tree {
    fn new(options: &GlobalOptions) -> Self {
        Self {
            stack: Vec::new(),
            depth: options.tree_depth,
        }
    }
}

impl Reporter for Tree {
    fn update(&mut self, progress: RunProgress) {
        match progress {
            RunProgress::Print(s) => eprintln!("{s}"),
            RunProgress::EnterCall(s) => {
                if self.stack.len() <= self.depth {
                    for _ in 0..self.stack.len() {
                        eprint!(" ");
                    }
                    eprintln!("{s:?}");
                }
                self.stack.push(s);
            }
            RunProgress::ExitCall(s) => {
                let popped = self.stack.pop();
                debug_assert_eq!(popped, Some(s));
            }
        }
        let _ = stderr().flush();
    }
}
