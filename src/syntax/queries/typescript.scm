; Variables
(identifier) @variable

; Types
(type_identifier) @type
(predefined_type) @type

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
  "abstract"
  "as"
  "async"
  "await"
  "class"
  "const"
  "debugger"
  "declare"
  "default"
  "delete"
  "enum"
  "export"
  "extends"
  "from"
  "function"
  "get"
  "implements"
  "import"
  "in"
  "infer"
  "instanceof"
  "interface"
  "keyof"
  "let"
  "module"
  "namespace"
  "new"
  "of"
  "override"
  "private"
  "protected"
  "public"
  "readonly"
  "set"
  "static"
  "type"
  "typeof"
  "var"
  "void"
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
  "?"
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
