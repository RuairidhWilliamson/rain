(internal) @variable.builtin
(raw_string_literal) @string
(format_string_literal) @string
(string_literal) @string
(number_literal) @constant.numeric
(bool_literal) @constant.builtin.boolean

"let" @keyword.storage.type
"fn" @keyword.function
"pub" @keyword

"if" @keyword.control.conditional
"else" @keyword.control.conditional

"=" @operator
"!" @operator
"==" @operator
"!=" @operator
"<" @operator
">" @operator
"<=" @operator
">=" @operator
"&&" @operator
"||" @operator
"+" @operator
"*" @operator
"-" @operator
"/" @operator
"->" @operator

"," @punctuation.delimiter

"(" @punctuation.bracket
")" @punctuation.bracket
"[" @punctuation.bracket
"]" @punctuation.bracket
"{" @punctuation.bracket
"}" @punctuation.bracket

(fn_declare_arg (identifier) @variable.parameter)
(fn_declare_expr) @function
(fn_call (expr) @function)
(type_constraint) @type

(block) @variable
; (identifier) @variable
(line_comment) @comment.line
