/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

export default grammar({
  name: "rain",

  rules: {
    source_file: ($) => repeat($.declaration),

    declaration: ($) => choice($.let_declare, $.line_comment),

    let_declare: ($) => seq(optional("pub"), "let", $.identifier, "=", $.expr),
    fn_declare_expr: ($) => seq("fn", $.fn_declare_args, $.block),
    fn_declare_args: ($) =>
      seq(
        "(",
        optional(seq($.fn_declare_arg, repeat(seq(",", $.fn_declare_arg)))),
        ")",
      ),
    fn_declare_arg: ($) => seq($.identifier, optional(":"), $.type_constraint),
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

    // TODO: Precedence should be different for different operators
    binary_expr: ($) =>
      choice(
        prec.left(50, seq($.expr, choice("*", "/"), $.expr)),
        prec.left(40, seq($.expr, choice("+", "-"), $.expr)),
        prec.left(35, seq($.expr, choice(">", "<", ">=", "<="), $.expr)),
        prec.left(30, seq($.expr, choice("==", "!="), $.expr)),
        prec.left(20, seq($.expr, "&&", $.expr)),
        prec.left(20, seq($.expr, "||", $.expr)),
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
      seq("[", optional(seq($.expr, repeat(seq(",", $.expr)))), "]"),
    record_literal: ($) =>
      seq(
        "{",
        optional(seq($.record_element, repeat(seq(",", $.record_element)))),
        "}",
      ),
    record_element: ($) => seq($.identifier, "=", $.expr),
    fn_call: ($) => prec(9, seq($.expr, $.arg_list)),
    arg_list: ($) =>
      seq("(", optional(seq($.expr, repeat(seq(",", $.expr)))), ")"),

    internal: () => "internal",
    string: () => /"[^"]*"/,
    number: () => /\d+/,
    bool: () => choice("true", "false"),

    identifier: () => /[a-zA-Z_][a-zA-Z0-9_]*/,
    line_comment: () => /\/\/.*/,
  },
});
