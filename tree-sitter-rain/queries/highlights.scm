(internal) @variable.builtin
(string) @string
(number) @constant.numeric
(bool) @constant.builtin.boolean

"let" @keyword.storage.type
"fn" @keyword.function
"pub" @keyword
["if" "else"] @keyword.control.conditional
["=" "!" "==" "!=" "<" ">" "<=" ">=" "&&" "||" "+" "*" "-" "/"] @operator
[","] @punctuation.delimiter
["(" ")" "[" "]" "{" "}"] @punctuation.bracket

(fn_declare (identifier) @function)
(fn_call (expr) @function)

(identifier) @variable
(line_comment) @comment.line
