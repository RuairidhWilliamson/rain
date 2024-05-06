use rain_lang::ast::ParseError;
use rain_lang::error::RainError;
use rain_lang::error::ResolvedError;
use rain_lang::exec::ExecError;
use rain_lang::executor::builder::ExecutorBuilder;
use rain_lang::path::Workspace;
use rain_lang::span::{Place, Span};
use rain_lang::tokens::{TokenError, TokenKind};
use rain_lang::RunError;

macro_rules! script_errors_test {
    ($name:ident, $source:expr, $expected:expr) => {
        #[test]
        fn $name() {
            let workspace = Workspace::new_local_cwd().unwrap();
            let mut executor_builder = ExecutorBuilder::default().build(workspace);
            let workspace = Workspace::Local(std::env::current_dir().unwrap());
            let source = rain_lang::source::Source::new_evaluated(
                workspace.new_path("."),
                String::from($source),
            );
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
    "fn main() { core.print(abc) }",
    RainError::new(
        ExecError::UnknownItem(String::from("abc")),
        Span::new(
            Place {
                index: 23,
                line: 0,
                column: 23,
            },
            Place {
                index: 26,
                line: 0,
                column: 26,
            },
        ),
    )
);

script_errors_test!(
    unknown_function,
    "fn main() { foo(\"\") }",
    RainError::new(
        ExecError::UnknownItem(String::from("foo")),
        Span::new(
            Place {
                index: 12,
                line: 0,
                column: 12,
            },
            Place {
                index: 15,
                line: 0,
                column: 15,
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
    infinite_recursion,
    "fn main() { main() }",
    RainError::new(
        ExecError::CallDepthLimit,
        Span::new(
            Place {
                index: 12,
                line: 0,
                column: 12,
            },
            Place {
                index: 18,
                line: 0,
                column: 18,
            },
        ),
    )
);

script_errors_test!(
    same_fn_declare_name,
    "fn foo() { }\nfn foo() { }",
    RainError::new(
        ExecError::DuplicateDeclare(Span::new(
            Place {
                index: 0,
                line: 0,
                column: 0,
            },
            Place {
                index: 12,
                line: 0,
                column: 12,
            },
        )),
        Span::new(
            Place {
                index: 13,
                line: 1,
                column: 0,
            },
            Place {
                index: 25,
                line: 1,
                column: 12,
            },
        ),
    )
);
