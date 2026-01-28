; Variables
(identifier) @variable

; Methods
(method_declaration
  name: (identifier) @function)

(method_invocation
  name: (identifier) @function)

(super) @function

; Parameters
(formal_parameter
  name: (identifier) @variable)

(catch_formal_parameter
  name: (identifier) @variable)

(spread_parameter
  (variable_declarator
    name: (identifier) @variable))

; Lambda parameter
(inferred_parameters
  (identifier) @variable)

(lambda_expression
  parameters: (identifier) @variable)

; Operators
[
  "+"
  ":"
  "++"
  "-"
  "--"
  "&"
  "&&"
  "|"
  "||"
  "!"
  "!="
  "=="
  "*"
  "/"
  "%"
  "<"
  "<="
  ">"
  ">="
  "="
  "-="
  "+="
  "*="
  "/="
  "%="
  "->"
  "^"
  "^="
  "&="
  "|="
  "~"
  ">>"
  ">>>"
  "<<"
  "::"
] @operator

; Types
(interface_declaration
  name: (identifier) @type)

(annotation_type_declaration
  name: (identifier) @type)

(class_declaration
  name: (identifier) @type)

(record_declaration
  name: (identifier) @type)

(enum_declaration
  name: (identifier) @type)

(enum_constant
  name: (identifier) @constant)

(constructor_declaration
  name: (identifier) @function)

(type_identifier) @type

((type_identifier) @type
  (#eq? @type "var"))

(object_creation_expression
  type: (type_identifier) @function)

((method_invocation
  object: (identifier) @type)
  (#match? @type "^[A-Z]"))

((method_reference
  .
  (identifier) @type)
  (#match? @type "^[A-Z]"))

((field_access
  object: (identifier) @type)
  (#match? @type "^[A-Z]"))

(scoped_identifier
  (identifier) @type
  (#match? @type "^[A-Z]"))

; Fields
(field_declaration
  declarator:
    (variable_declarator
      name: (identifier) @property))

(field_access
  field: (identifier) @property)

[
  (boolean_type)
  (integral_type)
  (floating_point_type)
  (void_type)
] @type

; Variables
((identifier) @constant
  (#match? @constant "^[A-Z_$][A-Z\\d_$]*$"))

(this) @variable

; Annotations
(annotation
  "@" @punctuation.delimiter
  name: (identifier) @type)

(marker_annotation
  "@" @punctuation.delimiter
  name: (identifier) @type)

; Literals
(string_literal) @string

(escape_sequence) @string

(character_literal) @string

[
  (hex_integer_literal)
  (decimal_integer_literal)
  (octal_integer_literal)
  (binary_integer_literal)
  (decimal_floating_point_literal)
  (hex_floating_point_literal)
] @number

[
  (true)
  (false)
] @boolean

(null_literal) @constant

; Keywords
[
  "assert"
  "class"
  "record"
  "default"
  "enum"
  "extends"
  "implements"
  "instanceof"
  "interface"
  "@interface"
  "permits"
  "to"
  "with"
  "new"
  "abstract"
  "final"
  "native"
  "non-sealed"
  "open"
  "private"
  "protected"
  "public"
  "sealed"
  "static"
  "strictfp"
  "synchronized"
  "transitive"
  "transient"
  "volatile"
  "return"
  "yield"
  "if"
  "else"
  "switch"
  "case"
  "when"
  "for"
  "while"
  "do"
  "continue"
  "break"
  "exports"
  "import"
  "module"
  "opens"
  "package"
  "provides"
  "requires"
  "uses"
  "throw"
  "throws"
  "finally"
  "try"
  "catch"
] @keyword

(ternary_expression
  [
    "?"
    ":"
  ] @operator)

; Punctuation
[
  ";"
  "."
  "..."
  ","
] @punctuation.delimiter

[
  "{"
  "}"
  "["
  "]"
  "("
  ")"
] @punctuation.bracket

(type_arguments
  [
    "<"
    ">"
  ] @punctuation.bracket)

(type_parameters
  [
    "<"
    ">"
  ] @punctuation.bracket)

; Labels
(labeled_statement
  (identifier) @type)

; Comments
[
  (line_comment)
  (block_comment)
] @comment
