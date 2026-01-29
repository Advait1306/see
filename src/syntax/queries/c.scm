; Variables
(identifier) @variable

; Types
[
  (type_identifier)
  (primitive_type)
  (sized_type_specifier)
] @type

; Properties/Fields
(field_identifier) @property

; Function calls and definitions
(call_expression
  function: (identifier) @function)
(call_expression
  function: (field_expression
    field: (field_identifier) @function))
(function_declarator
  declarator: (identifier) @function)
(preproc_function_def
  name: (identifier) @function)

; Strings
[
  (string_literal)
  (system_lib_string)
  (char_literal)
] @string

(escape_sequence) @string

; Numbers
(number_literal) @number

; Booleans
[
  (true)
  (false)
] @boolean

; Null
(null) @constant

; Comments
(comment) @comment

; Keywords
[
  "const"
  "enum"
  "extern"
  "inline"
  "sizeof"
  "static"
  "struct"
  "typedef"
  "union"
  "volatile"
] @keyword

; Control flow
[
  "break"
  "case"
  "continue"
  "default"
  "do"
  "else"
  "for"
  "goto"
  "if"
  "return"
  "switch"
  "while"
] @keyword

; Preprocessor directives
[
  "#define"
  "#elif"
  "#else"
  "#endif"
  "#if"
  "#ifdef"
  "#ifndef"
  "#include"
  (preproc_directive)
] @keyword

; Punctuation
[
  "."
  ";"
  ","
] @punctuation.delimiter

[
  "{"
  "}"
  "("
  ")"
  "["
  "]"
] @punctuation.bracket

; Operators
[
  "="
  "+="
  "-="
  "*="
  "/="
  "%="
  "&="
  "|="
  "^="
  "<<="
  ">>="
  "++"
  "--"
  "+"
  "-"
  "*"
  "/"
  "%"
  "~"
  "&"
  "|"
  "^"
  "<<"
  ">>"
  "!"
  "&&"
  "||"
  "=="
  "!="
  "<"
  ">"
  "<="
  ">="
  "->"
  "?"
  ":"
] @operator
