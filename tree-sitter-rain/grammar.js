/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

export default grammar({
  name: "rain",

  rules: {
    source_file: ($) => repeat(choice($.line_comment, $.declaration)),

    declaration: ($) => $.let_declare,

    let_declare: ($) => seq(optional("pub"), "let", $.identifier, "=", $.expr),
    fn_declare_expr: ($) =>
      seq(
        "fn",
        $.fn_declare_args,
        optional(seq("->", $.type_constraint)),
        $.block,
      ),
    fn_declare_args: ($) =>
      seq(
        "(",
        optional(seq($.fn_declare_arg, repeat(seq(",", $.fn_declare_arg)))),
        ")",
      ),
    fn_declare_arg: ($) =>
      seq($.identifier, optional(seq(":", $.type_constraint))),
    type_constraint: ($) => $.expr,

    block: ($) => seq("{", repeat($.statement), "}"),

    statement: ($) => choice($.assignment, $.expr, $.line_comment),

    assignment: ($) => seq($.identifier, "=", $.expr),

    expr: ($) =>
      choice(
        $.namespace,
        $.unary_expr,
        $.binary_expr,
        $.if_condition,
        $.list_literal,
        $.record_literal,
        $.internal,
        $.string,
        $.number,
        $.bool,
        $.fn_declare_expr,
        $.fn_call,
        $.identifier,
        seq("(", $.expr, ")"),
      ),

    unary_expr: ($) => prec(60, choice(seq("!", $.expr))),

    binary_expr: ($) =>
      choice(
        prec.left(50, seq($.expr, choice("*", "/", "%", "&", "^"), $.expr)),
        prec.left(40, seq($.expr, choice("+", "-", "|"), $.expr)),
        prec.left(35, seq($.expr, choice(">", "<", ">=", "<="), $.expr)),
        prec.left(30, seq($.expr, choice("==", "!="), $.expr)),
        prec.left(20, seq($.expr, "&&", $.expr)),
        prec.left(10, seq($.expr, "||", $.expr)),
      ),

    namespace: ($) => seq($.expr, ".", $.identifier),

    if_condition: ($) =>
      seq(
        "if",
        $.expr,
        $.block,
        repeat(seq("else", "if", $.expr, $.block)),
        optional(seq("else", $.block)),
      ),
    list_literal: ($) =>
      seq(
        "[",
        seq(
          repeat(choice($.line_comment, seq($.expr, ","))),
          optional(seq($.expr, repeat($.line_comment))),
        ),
        "]",
      ),
    _list_literal_inner: ($) => choice($.line_comment, $.expr),
    record_literal: ($) =>
      seq(
        "{",
        seq(
          repeat(choice($.line_comment, seq($.record_element, ","))),
          optional(seq($.record_element, repeat($.line_comment))),
        ),
        "}",
      ),
    record_element: ($) => seq($.identifier, "=", $.expr),
    fn_call: ($) => prec(9, seq($.expr, $.arg_list)),
    arg_list: ($) =>
      seq("(", optional(seq($.expr, repeat(seq(",", $.expr)))), ")"),

    internal: () => "internal",
    string: () =>
      seq(
        '"',
        repeat(
          choice(/[^"\n\\]/u, seq("\\", choice("\\", '"', "n", "r", "t"))),
        ),
        '"',
      ),
    number: () => /\d+/,
    bool: () => choice("true", "false"),

    identifier: () => /[a-zA-Z_\P{ASCII}][a-zA-Z0-9_\P{ASCII}]*/u,
    line_comment: () => /\/\/.*/,
  },
});
