/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

module.exports = grammar({
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
        $.fn_call,
        $.internal,
        $.string,
        $.number,
        $.bool,
        $.fn_declare_expr,
        $.identifier,
        seq("(", $.expr, ")"),
      ),

    unary_expr: ($) => prec(2, choice(seq("!", $.expr))),

    // TODO: Precedence should be different for different operators
    binary_expr: ($) => prec.left(1, seq($.expr, $.binary_op, $.expr)),
    binary_op: () =>
      choice("==", "!=", ">", "<", ">=", "<=", "&&", "||", "+", "*", "-", "/"),

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
