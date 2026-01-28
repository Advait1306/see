; Based on Zed's Rust highlights.scm

; Identifiers and types
(identifier) @variable
(type_identifier) @type
(primitive_type) @type
(self) @variable
(field_identifier) @property

; Function calls
(call_expression
  function: [
    (identifier) @function
    (scoped_identifier
      name: (identifier) @function)
    (field_expression
      field: (field_identifier) @function)
  ])

; Function definitions
(function_item name: (identifier) @function)

; Macro invocations
(macro_invocation
  macro: [
    (identifier) @function
    (scoped_identifier
      name: (identifier) @function)
  ])

; Punctuation
[
  "("
  ")"
  "{"
  "}"
  "["
  "]"
] @punctuation.bracket

[
  "."
  ";"
  ","
  "::"
] @punctuation.delimiter

; Keywords
[
  "as"
  "async"
  "const"
  "dyn"
  "enum"
  "extern"
  "fn"
  "impl"
  "let"
  "mod"
  "move"
  "pub"
  "ref"
  "static"
  "struct"
  "for"
  "trait"
  "type"
  "unsafe"
  "use"
  "where"
  (crate)
  (mutable_specifier)
  (super)
] @keyword

; Control flow keywords
[
  "await"
  "break"
  "continue"
  "else"
  "if"
  "in"
  "loop"
  "match"
  "return"
  "while"
] @keyword

; Strings
[
  (string_literal)
  (raw_string_literal)
  (char_literal)
] @string

; Numbers
[
  (integer_literal)
  (float_literal)
] @number

; Booleans
(boolean_literal) @constant

; Comments
[
  (line_comment)
  (block_comment)
] @comment

; Operators
[
  "!="
  "%"
  "&"
  "&&"
  "*"
  "+"
  "-"
  "->"
  ".."
  "/"
  ":"
  "<"
  "<<"
  "<="
  "="
  "=="
  "=>"
  ">"
  ">="
  ">>"
  "@"
  "^"
  "|"
  "||"
  "?"
] @operator

; Lifetimes
(lifetime
  "'" @variable
  (identifier) @variable)

; Parameters
(parameter (identifier) @variable)

; Attributes
(attribute_item (attribute (identifier) @attribute))
