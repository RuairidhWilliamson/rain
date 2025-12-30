use std::path::Path;

use crate::afs::file::File;

fn parse_display_script(src: &str) -> String {
    let file = File::new_local(Path::new(file!())).unwrap();
    let s = match super::parser::parse_module(src) {
        Ok(s) => s,
        Err(err) => {
            panic!("parse error:\n{}", err.resolve(Some(&file), src));
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
    match super::parser::parse_module(src) {
        Ok(_) => panic!("expected parse error"),
        Err(_err) => {}
    };
}

// #[test]
// fn destructure_single_item() {
//     let src = "let {a} = {a}";
//     insta::assert_snapshot!(parse_display_script(src));
// }
