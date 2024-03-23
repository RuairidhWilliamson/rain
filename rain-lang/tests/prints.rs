use std::{cell::RefCell, fmt::Write, rc::Rc};

use rain_lang::{
    error::RainError,
    exec::{corelib::CoreHandler, Executable, ExecutorBuilder},
};

#[derive(Debug)]
struct BufferCoreHandler {
    output: Rc<RefCell<String>>,
}

impl CoreHandler for BufferCoreHandler {
    fn print(&mut self, s: std::fmt::Arguments) {
        self.output
            .borrow_mut()
            .write_fmt(format_args!("{s}\n"))
            .unwrap();
    }
}

macro_rules! script_prints_test {
    ($name:ident, $source:expr, $expected:expr) => {
        #[test]
        fn $name() -> Result<(), RainError> {
            let mut token_stream = rain_lang::tokens::peek_stream::PeekTokenStream::new($source);
            let script = rain_lang::ast::script::Script::parse_stream(&mut token_stream)?;
            let buffer = Rc::new(RefCell::new(String::new()));
            let ch = Box::new(BufferCoreHandler {
                output: buffer.clone(),
            });
            let mut executor = ExecutorBuilder {
                core_handler: Some(ch),
                ..ExecutorBuilder::default()
            }
            .build();
            Executable::execute(&script, &mut executor)?;
            assert_eq!(buffer.borrow().as_str(), $expected);
            Ok(())
        }
    };
}

script_prints_test!(hello_world, "core.print(\"hello world\")", "hello world\n");

script_prints_test!(
    if_else,
    "
        core.print(if true {
            \"it was true\"
        } else {
            \"unreachable\"
        })
    ",
    "it was true\n"
);

script_prints_test!(
    fn_call,
    "
        fn foo() {
            core.print(\"peeka boo\")
        }
        foo()
    ",
    "peeka boo\n"
);

script_prints_test!(
    early_return,
    "
        fn foo() {
            core.print(\"about to return\")
            return false
            core.print(\"unreachable\")
        }
        foo()
    ",
    "about to return\n"
);

// script_prints_test!(
//     if_early_return,
//     "
//         fn foo() {
//             core.print(\"about to return\")
//             if true {
//                 return false
//             }
//             core.print(\"unreachable\")
//         }
//         foo()
//     ",
//     "about to return\n"
// );
