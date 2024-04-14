use rain_lang::ast::ParseError;
use rain_lang::error::RainError;
use rain_lang::error::ResolvedError;
use rain_lang::exec::executor::ExecutorBuilder;
use rain_lang::exec::ExecError;
use rain_lang::span::{Place, Span};
use rain_lang::tokens::{TokenError, TokenKind};
use rain_lang::RunError;

macro_rules! script_errors_test {
    ($name:ident, $source:expr, $expected:expr) => {
        #[test]
        fn $name() {
            let mut executor_builder = ExecutorBuilder::default().build();
            let source = rain_lang::source::Source::from($source);
            match rain_lang::run(source, &mut executor_builder) {
                Ok(_) => panic!("expected error"),
                Err(RunError::ResolvedRainError(ResolvedError { err, .. })) => {
                    assert_eq!(err, $expected)
                }
                Err(rain_lang::RunError::RuntimeError(_)) => {
                    panic!("unexpected runtime error")
                }
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

script_errors_test!(
    unclosed_double_quote,
    "let a = \"",
    RainError::new(
        TokenError {
            kind: rain_lang::tokens::TokenErrorKind::UnclosedDoubleQuote,
            place: Place {
                index: 8,
                line: 0,
                column: 8,
            },
        },
        Span::new(
            Place {
                index: 8,
                line: 0,
                column: 8,
            },
            Place {
                index: 9,
                line: 0,
                column: 9,
            },
        ),
    )
);

script_errors_test!(
    very_bad_function_def,
    "fn foo(/",
    RainError::new(
        ParseError::ExpectedAny(&[TokenKind::Ident, TokenKind::RParen]),
        Span::new(
            Place {
                index: 7,
                line: 0,
                column: 7,
            },
            Place {
                index: 8,
                line: 0,
                column: 8,
            },
        ),
    )
);

script_errors_test!(
    top_level_return,
    "return false",
    RainError::new(
        ExecError::ReturnOutsideFunction,
        Span::new(
            Place {
                index: 0,
                line: 0,
                column: 0,
            },
            Place {
                index: 11,
                line: 0,
                column: 11,
            },
        ),
    )
);

script_errors_test!(
    infinite_recursion,
    "fn foo() { foo() }\nfoo()",
    RainError::new(
        ExecError::CallDepthLimit,
        Span::new(
            Place {
                index: 11,
                line: 0,
                column: 11,
            },
            Place {
                index: 16,
                line: 0,
                column: 16,
            },
        ),
    )
);
