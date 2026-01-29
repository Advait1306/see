; Punctuation
[ "." ";" ":" "," ] @punctuation.delimiter
[ "\\(" "(" ")" "[" "]" "{" "}" ] @punctuation.bracket

; Identifiers
(attribute) @variable
(type_identifier) @type
(self_expression) @variable
(user_type (type_identifier) @type)

; Declarations
"func" @keyword

[
  (visibility_modifier)
  (member_modifier)
  (function_modifier)
  (property_modifier)
  (parameter_modifier)
  (inheritance_modifier)
  (mutation_modifier)
] @keyword

(function_declaration (simple_identifier) @function)
(init_declaration ["init" @function])
(deinit_declaration ["deinit" @function])
(throws) @keyword
"async" @keyword
"await" @keyword
(where_keyword) @keyword
(parameter external_name: (simple_identifier) @property)
(parameter name: (simple_identifier) @property)
(type_parameter (type_identifier) @property)
(inheritance_constraint (identifier (simple_identifier) @property))
(equality_constraint (identifier (simple_identifier) @property))
(pattern bound_identifier: (simple_identifier)) @variable

[
  "typealias"
  "struct"
  "class"
  "actor"
  "enum"
  "protocol"
  "extension"
  "indirect"
  "nonisolated"
  "override"
  "convenience"
  "required"
  "mutating"
  "nonmutating"
  "associatedtype"
] @keyword

(opaque_type ["some" @keyword])
(existential_type ["any" @keyword])

[
  (getter_specifier)
  (setter_specifier)
  (modify_specifier)
] @keyword

(class_body (property_declaration (pattern (simple_identifier) @variable)))
(protocol_property_declaration (pattern (simple_identifier) @variable))

(value_argument
  name: (value_argument_label) @property)

(import_declaration
  "import" @keyword)

(enum_entry
  "case" @keyword)

; Function calls
(call_expression (simple_identifier) @function)
(call_expression
  (navigation_expression
    (navigation_suffix (simple_identifier) @function)))

(try_operator) @operator
(try_operator ["try" @keyword])

(directive) @function
(diagnostic) @function

; Statements
(for_statement ["for" @keyword])
(for_statement ["in" @keyword])
(for_statement (pattern) @variable)
(else) @keyword
(as_operator) @keyword

["while" "repeat" "continue" "break"] @keyword

["let" "var"] @keyword

(guard_statement
  "guard" @keyword)

(if_statement
  "if" @keyword)

(switch_statement
  "switch" @keyword)

(switch_entry
  "case" @keyword)

(switch_entry
  "fallthrough" @keyword)

(switch_entry
  (default_keyword) @keyword)

"return" @keyword

(ternary_expression
  ["?" ":"] @operator)

["do" (throw_keyword) (catch_keyword)] @keyword

(statement_label) @type

; Comments
[
  (comment)
  (multiline_comment)
] @comment

; String literals
(line_str_text) @string
(str_escaped_char) @string
(multi_line_str_text) @string
(raw_str_part) @string
(raw_str_end_part) @string
(raw_str_interpolation_start) @punctuation.bracket
["\"" "\"\"\""] @string

; Lambda literals
(lambda_literal ["in" @keyword])

; Basic literals
[
  (integer_literal)
  (hex_literal)
  (oct_literal)
  (bin_literal)
] @number
(real_literal) @number
(boolean_literal) @boolean
"nil" @constant

; Regex literals
(regex_literal) @string

; Operators
(custom_operator) @operator

[
  "!"
  "?"
  "+"
  "-"
  "*"
  "/"
  "%"
  "="
  "+="
  "-="
  "*="
  "/="
  "<"
  ">"
  "<="
  ">="
  "++"
  "--"
  "&"
  "~"
  "%="
  "!="
  "!=="
  "=="
  "==="
  "??"
  "->"
  "..<"
  "..."
  (bang)
] @operator

(value_parameter_pack ["each" @keyword])
(value_pack_expansion ["repeat" @keyword])
(type_parameter_pack ["each" @keyword])
(type_pack_expansion ["repeat" @keyword])
