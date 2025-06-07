(internal) @variable.builtin
(string) @string
(number) @constant.numeric
(bool) @constant.builtin.boolean

"let" @keyword.storage.type
"fn" @keyword.function
["if" "else"] @keyword.control.conditional
["=" "!" "==" "!=" "<" ">" "<=" ">=" "&&" "||" "+" "*" "-" "/"] @operator
[","] @punctuation.delimiter
["(" ")" "[" "]" "{" "}"] @punctuation.bracket

(fn_declare (identifier) @function)

(identifier) @variable
; (comment) @comment.line
