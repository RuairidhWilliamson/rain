use crate::{ast::display::display_ast, tokens::peek::PeekTokenStream};

use super::Script;

#[test]
fn hello_world() {
    let src = "
        fn main() {
            print(\"Hello world\")
        }
    ";
    let mut stream = PeekTokenStream::new(src);
    let s = Script::parse(&mut stream).unwrap();
    insta::assert_snapshot!(display_ast(&s, src));
}
