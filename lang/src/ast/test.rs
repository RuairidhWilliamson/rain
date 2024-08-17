use crate::{ast::display::display_ast, tokens::peek::PeekTokenStream};

use super::Script;

fn parse_display_script(src: &str) -> String {
    let mut stream = PeekTokenStream::new(src);
    let s = Script::parse(&mut stream).unwrap();
    assert_eq!(
        stream.parse_next().unwrap(),
        None,
        "input not fully consumed"
    );
    display_ast(&s, src)
}

#[test]
fn hello_world() {
    insta::assert_snapshot!(parse_display_script(
        "
        fn main() {
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
        fn factorial(n) {
        	factorial(n - 1) * n
        }
        "
    ));
}
