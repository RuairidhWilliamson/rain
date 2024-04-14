use rain_lang::error::RainError;
use rain_lang::exec::executor::ExecutorBuilder;
use rain_lang::exec::ExecError;
use rain_lang::span::{Place, Span};

macro_rules! script_errors_test {
    ($name:ident, $source:expr, $expected:expr) => {
        #[test]
        fn $name() {
            let mut executor_builder = ExecutorBuilder::default().build();
            let source = rain_lang::source::Source::from($source);
            match rain_lang::run(source, &mut executor_builder) {
                Ok(_) => panic!("expected error"),
                Err(err) => match *err {
                    rain_lang::error::ResolvedError { err, .. } => assert_eq!(err, $expected),
                },
            }
        }
    };
}

script_errors_test!(
    unknown_var,
    "core.print(abc)",
    RainError::new(
        ExecError::UnknownItem(String::from("abc")),
        Span::new(
            Place {
                index: 11,
                line: 0,
                column: 11,
            },
            Place {
                index: 14,
                line: 0,
                column: 14,
            },
        ),
    )
);

script_errors_test!(
    unknown_function,
    "foo(\"\")",
    RainError::new(
        ExecError::UnknownItem(String::from("foo")),
        Span::new(
            Place {
                index: 0,
                line: 0,
                column: 0,
            },
            Place {
                index: 3,
                line: 0,
                column: 3,
            },
        ),
    )
);
