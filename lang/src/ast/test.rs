use test_log::test;

use crate::{
    ast::{Module, error::ParseError},
    local_span::ErrorLocalSpan,
};

fn parse_display_script(src: &str) -> String {
    let s = match Module::parse(src) {
        Ok(s) => s,
        Err(err) => {
            panic!("parse error:\n{}", err.resolve(None, src));
        }
    };
    s.display(src)
}

#[test]
fn hello_world() {
    insta::assert_snapshot!(parse_display_script(
        "
        let main = fn() {
            print(\"Hello world\")
        }
        "
    ));
}

#[test]
fn let_declare() {
    insta::assert_snapshot!(parse_display_script(
        "
        let a = 4
        let asjldf = \"asjldf\"
        "
    ));
}

#[test]
fn fn_call() {
    insta::assert_snapshot!(parse_display_script(
        "
        let val = foo(3)
        let val = foo(bar(4))
        "
    ));
}

#[test]
fn factorial() {
    insta::assert_snapshot!(parse_display_script(
        "
        let assert = std.test.assert
        let eq = std.ops.eq

        let main = fn() {
        	assert(factorial(5), 12)
        }

        let factorial = fn(n) {
            if n == 0 {
                1
            } else {
            	factorial(n - 1) * n
        	}
        }
        "
    ));
}

#[test]
fn comment() {
    insta::assert_snapshot!(parse_display_script(
        "
        let b = 2
        // This is silly
        let a = b // Very silly
        // Hehe
        "
    ));
}

#[test]
fn pub_fn() {
    insta::assert_snapshot!(parse_display_script(
        "
        pub let foo = fn() {}
        "
    ));
}

#[test]
fn pub_let() {
    insta::assert_snapshot!(parse_display_script(
        "
        pub let foo = 5
        "
    ));
}

#[test]
fn let_type_spec() {
    insta::assert_snapshot!(parse_display_script(
        "
        let a: B = 5
        "
    ));
}

#[test]
fn fn_type_spec_args() {
    insta::assert_snapshot!(parse_display_script(
        "
        let foo = fn(a: A, b: B) {}
        "
    ));
}

#[test]
fn list_missing_comma() {
    let src = "let a = [
        a, b
        c
    ]";
    match Module::parse(src) {
        Ok(_) => panic!("expected parse error"),
        Err(_err) => {}
    }
}

#[test]
fn destructure_single_item() {
    let src = "let {a} = {a = 4}";
    insta::assert_snapshot!(parse_display_script(src));
}

#[test]
fn destructure_two_items() {
    let src = "let {a: Integer, b} = {a = 4, b = 6}";
    insta::assert_snapshot!(parse_display_script(src));
}

#[test]
fn empty_source() {
    let src = "";
    insta::assert_snapshot!(parse_display_script(src));
}

#[test]
fn comment_no_text() {
    let src = "//\0";

    assert!(Module::parse(src).is_err());
}

fn parse_expr(src: &str) -> Result<String, String> {
    // Wrap the source into a module
    let src = format!("let main = {src}");
    match Module::parse(&src) {
        Ok(module) => {
            let declaration = module.root.declarations.first().unwrap();
            let id = declaration.assignment.expr;
            Ok(module.display_node(&src, id))
        }
        Err(err) => Err(err.resolve(None, &src).into_owned().to_string()),
    }
}

fn parse_display_expr(src: &str) -> String {
    match parse_expr(src) {
        Ok(display) => display,
        Err(err) => {
            eprintln!("{err}");
            panic!("parse error");
        }
    }
}

#[test]
fn number_literal() {
    insta::assert_snapshot!(parse_display_expr("4"));
}

#[test]
fn false_literal() {
    insta::assert_snapshot!(parse_display_expr("false"));
}

#[test]
fn true_literal() {
    insta::assert_snapshot!(parse_display_expr("true"));
}

#[test]
fn string_literal() {
    insta::assert_snapshot!(parse_display_expr("\"asldjf\""));
}

#[test]
fn number_add() {
    insta::assert_snapshot!(parse_display_expr("1 + 1"));
}

#[test]
fn number_add_left_associative() {
    insta::assert_snapshot!(parse_display_expr("1 + 2 + 3"));
}

#[test]
fn number_multiply() {
    insta::assert_snapshot!(parse_display_expr("1 * 2"));
}

#[test]
fn number_multiply_left_associative() {
    insta::assert_snapshot!(parse_display_expr("1 * 2 * 3"));
}

#[test]
fn number_multiply_add_precedence1() {
    insta::assert_snapshot!(parse_display_expr("5 * 2 + 3"));
}

#[test]
fn number_multiply_add_precedence2() {
    insta::assert_snapshot!(parse_display_expr("5 + 2 * 3"));
}

#[test]
fn number_add_subtract_precedence() {
    insta::assert_snapshot!(parse_display_expr("5 - 2 + 3 - 4"));
}

#[test]
fn number_add_subtract_multiply_precedence() {
    insta::assert_snapshot!(parse_display_expr("5 * 2 + 3 - 4"));
}

#[test]
fn number_add_subtrace_multiply_divide_precedence() {
    insta::assert_snapshot!(parse_display_expr("1 - 3 / 2 + 4 * 3"));
}

#[test]
fn ident_maths() {
    insta::assert_snapshot!(parse_display_expr("a + b - c * d / e"));
}

#[test]
fn ident_dot_ident() {
    insta::assert_snapshot!(parse_display_expr("foo.bar"));
}

#[test]
fn ident_dot_ident_dot_ident() {
    insta::assert_snapshot!(parse_display_expr("foo.bar.baz"));
}

#[test]
fn ident_dot_maths() {
    insta::assert_snapshot!(parse_display_expr("a.b.c + 3 * d.e"));
}

#[test]
fn maths_parens1() {
    insta::assert_snapshot!(parse_display_expr("1 - (a + 3) * 4"));
}

#[test]
fn maths_parens2() {
    insta::assert_snapshot!(parse_display_expr("(3 - b) * c"));
}

#[test]
fn fn_call_no_args() {
    insta::assert_snapshot!(parse_display_expr("foo()"));
}

#[test]
fn fn_call_no_args_call_no_args() {
    insta::assert_snapshot!(parse_display_expr("foo()()"));
}

#[test]
fn fn_call_no_args_precedence() {
    insta::assert_snapshot!(parse_display_expr("foo.bar()"));
}

#[test]
fn fn_call_one_arg() {
    insta::assert_snapshot!(parse_display_expr("foo(1)"));
}

#[test]
fn fn_call_two_arg() {
    insta::assert_snapshot!(parse_display_expr("foo(1, 2)"));
}

#[test]
fn fn_call_two_arg_trailing_comma() {
    insta::assert_snapshot!(parse_display_expr("foo(1, 2,)"));
}

#[test]
fn logical_operators() {
    insta::assert_snapshot!(parse_display_expr(
        "true || a == b && 1 != 1 && (false || a != b)"
    ));
}

#[test]
fn record_constructor() {
    insta::assert_snapshot!(parse_display_expr("{a = 1, b = 2, c = \"ajlsdkf\"}"));
}

#[test]
fn record_constructor_nested() {
    insta::assert_snapshot!(parse_display_expr("{a = {b = {c = 5}},}"));
}

#[test]
fn record_constructor_nls() {
    insta::assert_snapshot!(parse_display_expr("{\na = b, \n// comment \n c = 4\n}"));
}

#[test]
fn list_constructor_nested() {
    insta::assert_snapshot!(parse_display_expr("[a, b, 123, [567, d]]"));
}

#[test]
fn list_constructor_nested_nls() {
    insta::assert_snapshot!(parse_display_expr(
        "[a\n, b,\n 123, // comment \n [\n567, d]\n]"
    ));
}

#[test]
fn invalid_exprs() {
    assert!(parse_expr("4.").is_err());
    assert!(parse_expr(".4").is_err());
    assert!(parse_expr("()").is_err());
}

#[test]
fn invalid_scripts() {
    fn parse_display_module(src: &str) -> Result<(), ErrorLocalSpan<ParseError>> {
        Module::parse(src).map(|m| {
            log::error!("{}", m.display(src));
        })
    }
    assert!(parse_display_module("let foo = fn() {5 6}").is_err());
    assert!(parse_display_module("let foo = fn() {a b c}").is_err());
}

#[test]
fn not_and_operation() {
    insta::assert_snapshot!(parse_display_expr("false && !!!true || !false"));
}

#[test]
fn not_paren_operation() {
    insta::assert_snapshot!(parse_display_expr("!(!a || !b)"));
}

#[test]
fn not_dot_operation() {
    insta::assert_snapshot!(parse_display_expr("!a.b"));
}

#[test]
fn not_or_operation() {
    insta::assert_snapshot!(parse_display_expr("!a || b"));
}

#[test]
fn not_not_or_operation() {
    insta::assert_snapshot!(parse_display_expr("!!a || b"));
}

#[test]
fn or_not_dot_operation() {
    insta::assert_snapshot!(parse_display_expr("a || !b.c"));
}

#[test]
fn not_dot_plus_expr() {
    insta::assert_snapshot!(parse_display_expr("!a.b + d"));
}

#[test]
fn plus_not_dot_plus_expr() {
    insta::assert_snapshot!(parse_display_expr("f + !a.b + d"));
}

#[test]
fn dot_not_plus_dot_plus_expr() {
    insta::assert_snapshot!(parse_display_expr("a.b + !c + !d.e"));
}

#[test]
fn less_than() {
    insta::assert_snapshot!(parse_display_expr("b < c && d > e"));
}

#[test]
fn greater_than_eq() {
    insta::assert_snapshot!(parse_display_expr("a >= b"));
}

#[test]
fn closure() {
    insta::assert_snapshot!(parse_display_expr("fn () {}"));
}

#[test]
fn closure_args() {
    insta::assert_snapshot!(parse_display_expr("fn (a: A, b: B) { 5 }(a, b)"));
}

#[test]
fn internal() {
    insta::assert_snapshot!(parse_display_expr("internal._print(42)"));
}
