; Variables
(identifier) @variable

; Types
(type_identifier) @type
(field_identifier) @property
(package_identifier) @namespace

; Function calls
(call_expression
  function: (identifier) @function)
(call_expression
  function: (selector_expression
    field: (field_identifier) @function))

; Function definitions
(function_declaration
  name: (identifier) @function)
(method_declaration
  name: (field_identifier) @function)

; Strings
[
  (interpreted_string_literal)
  (raw_string_literal)
  (rune_literal)
] @string

(escape_sequence) @string

; Numbers
[
  (int_literal)
  (float_literal)
  (imaginary_literal)
] @number

; Booleans
[
  (true)
  (false)
] @boolean

; Special constants
[
  (nil)
  (iota)
] @constant

; Comments
(comment) @comment

; Punctuation
[
  ";"
  "."
  ","
  ":"
] @punctuation.delimiter

[
  "("
  ")"
  "{"
  "}"
  "["
  "]"
] @punctuation.bracket

; Operators
[
  "--"
  "-"
  "-="
  ":="
  "!"
  "!="
  "..."
  "*"
  "*="
  "/"
  "/="
  "&"
  "&&"
  "&="
  "%"
  "%="
  "^"
  "^="
  "+"
  "++"
  "+="
  "<-"
  "<"
  "<<"
  "<<="
  "<="
  "="
  "=="
  ">"
  ">="
  ">>"
  ">>="
  "|"
  "|="
  "||"
  "~"
] @operator

; Keywords
[
  "break"
  "case"
  "chan"
  "const"
  "continue"
  "default"
  "defer"
  "else"
  "fallthrough"
  "for"
  "func"
  "go"
  "goto"
  "if"
  "import"
  "interface"
  "map"
  "package"
  "range"
  "return"
  "select"
  "struct"
  "switch"
  "type"
  "var"
] @keyword
