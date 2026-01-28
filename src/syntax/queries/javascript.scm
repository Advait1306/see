; Variables
(identifier) @variable

; Properties
(property_identifier) @property
(shorthand_property_identifier) @property

; Function calls
(call_expression
  function: (identifier) @function)
(call_expression
  function: (member_expression
    property: (property_identifier) @function))

; Function definitions
(function_expression
  name: (identifier) @function)
(function_declaration
  name: (identifier) @function)
(method_definition
  name: (property_identifier) @function)
(variable_declarator
  name: (identifier) @function
  value: [(function_expression) (arrow_function)])

; Strings
[
  (string)
  (template_string)
] @string

; Numbers
(number) @number

; Booleans and special values
[
  (true)
  (false)
] @boolean

[
  (null)
  (undefined)
] @constant

; Comments
(comment) @comment

; Keywords
[
  "as"
  "async"
  "await"
  "class"
  "const"
  "debugger"
  "default"
  "delete"
  "export"
  "extends"
  "from"
  "function"
  "get"
  "import"
  "in"
  "instanceof"
  "let"
  "new"
  "of"
  "set"
  "static"
  "target"
  "typeof"
  "var"
  "void"
  "with"
] @keyword

; Control flow
[
  "break"
  "case"
  "catch"
  "continue"
  "do"
  "else"
  "finally"
  "for"
  "if"
  "return"
  "switch"
  "throw"
  "try"
  "while"
  "yield"
] @keyword

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
  "["
  "]"
  "{"
  "}"
] @punctuation.bracket

; Operators
[
  "-"
  "--"
  "-="
  "+"
  "++"
  "+="
  "*"
  "*="
  "/"
  "/="
  "%"
  "%="
  "<"
  "<="
  "<<"
  "="
  "=="
  "==="
  "!"
  "!="
  "!=="
  "=>"
  ">"
  ">="
  ">>"
  ">>>"
  "~"
  "^"
  "&"
  "|"
  "&&"
  "||"
  "??"
  "..."
] @operator
