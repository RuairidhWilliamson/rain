(internal) @variable.builtin
(string) @string
(number) @constant.numeric
(bool) @constant.builtin.boolean

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

"," @punctuation.delimiter

"(" @punctuation.bracket
")" @punctuation.bracket
"[" @punctuation.bracket
"]" @punctuation.bracket
"{" @punctuation.bracket
"}" @punctuation.bracket

(fn_declare (identifier) @function)
(fn_call (expr) @function)

(identifier) @variable
(line_comment) @comment.line
