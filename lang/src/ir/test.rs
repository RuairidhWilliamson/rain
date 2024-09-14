use crate::{ast::ModuleRoot, tokens::peek::PeekTokenStream};

use super::Rir;

#[test]
fn let_deps() {
    let src = "
        let a = 1
        let b = a
        let c = a + b
    "
    .to_string();
    let mut stream = PeekTokenStream::new(&src);
    let ast = ModuleRoot::parse(&mut stream).unwrap();
    let mut ir = Rir::new();
    let module_id = ir.insert_module(None, src, ast);
    let a = ir.resolve_global_declaration(module_id, "a").unwrap();
    let b = ir.resolve_global_declaration(module_id, "b").unwrap();
    let c = ir.resolve_global_declaration(module_id, "c").unwrap();
    assert_eq!(ir.declaration_deps(a).unwrap(), vec![]);
    assert_eq!(ir.declaration_deps(b).unwrap(), vec![a]);
    assert_eq!(ir.declaration_deps(c).unwrap(), vec![a, b]);
}

#[test]
fn fn_deps() {
    let src = String::from(
        "
        let a = 1

        fn main() {
            a * 5 + 3
        }
    ",
    );
    let mut stream = PeekTokenStream::new(&src);
    let ast = ModuleRoot::parse(&mut stream).unwrap();
    let mut ir = Rir::new();
    let module_id = ir.insert_module(None, src, ast);
    let a = ir.resolve_global_declaration(module_id, "a").unwrap();
    let main = ir.resolve_global_declaration(module_id, "main").unwrap();
    assert_eq!(ir.declaration_deps(a).unwrap(), vec![]);
    assert_eq!(ir.declaration_deps(main).unwrap(), vec![a]);
}
