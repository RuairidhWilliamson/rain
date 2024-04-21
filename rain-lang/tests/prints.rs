use std::{cell::RefCell, fmt::Write, rc::Rc};

use rain_lang::exec::corelib::CoreHandler;
use rain_lang::exec::executor::ExecutorBuilder;
use rain_lang::path::Workspace;

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
        fn $name() {
            let buffer = Rc::new(RefCell::new(String::new()));
            let ch = Box::new(BufferCoreHandler {
                output: buffer.clone(),
            });
            let workspace = Workspace::new_local_cwd().unwrap();
            let mut executor_builder = ExecutorBuilder {
                corelib_handler: Some(ch),
                ..ExecutorBuilder::default()
            }
            .build(workspace);
            let workspace = Workspace::Local(std::env::current_dir().unwrap());
            let source = rain_lang::source::Source::new_evaluated(
                workspace.new_path("."),
                String::from($source),
            );
            if let Err(err) = rain_lang::run(source, &mut executor_builder) {
                panic!("{err}");
            }
            assert_eq!(buffer.borrow().as_str(), $expected);
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

script_prints_test!(
    fn_args,
    "
        fn wrap(a) {
            core.print(a)
        }
        wrap(\"hello world\")
    ",
    "hello world\n"
);

script_prints_test!(
    if_early_return,
    "
        fn foo() {
            core.print(\"about to return\")
            if true {
                return false
            }
            core.print(\"unreachable\")
        }
        foo()
    ",
    "about to return\n"
);

script_prints_test!(
    named_function_args,
    "
        fn foo(a, b) {
            core.print(\"a is\", a)
            core.print(\"b is\", b)
        }
        foo(b = true, a = false)
    ",
    "a is false\nb is true\n"
);
