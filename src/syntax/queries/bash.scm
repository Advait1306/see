; Strings
[
  (string)
  (raw_string)
  (heredoc_body)
  (heredoc_start)
  (heredoc_end)
  (ansi_c_string)
] @string

; Variables
(variable_name) @variable
(special_variable_name) @variable

; Function definitions and commands
(function_definition name: (word) @function)
(command_name) @function

; Numbers
[
  (number)
] @number

; Keywords
[
  "export"
  "function"
  "unset"
  "local"
  "declare"
] @keyword

; Control flow
[
  "case"
  "do"
  "done"
  "elif"
  "else"
  "esac"
  "fi"
  "for"
  "if"
  "in"
  "select"
  "then"
  "until"
  "while"
] @keyword

; Comments
(comment) @comment

; Operators
[
  "$"
  "&&"
  ">"
  "<<"
  ">>"
  ">&"
  "<"
  "|"
  ":"
  "//"
  "/"
  "%"
  "%%"
  "#"
  "##"
  "="
  "=="
  "!="
  "||"
] @operator

; Punctuation
[
  ";"
] @punctuation.delimiter

[
  "("
  ")"
  "{"
  "}"
  "["
  "]"
  "[["
  "]]"
] @punctuation.bracket
