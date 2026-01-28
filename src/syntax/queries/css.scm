; Comments
(comment) @comment

; Selectors
[
  (tag_name)
  (nesting_selector)
  (universal_selector)
] @tag

(id_name) @type
(class_name) @type

; Namespace
(namespace_name) @namespace

; Properties
[
  (feature_name)
  (property_name)
] @property

; Function names
(function_name) @function

; Keywords
[
  "@media"
  "@import"
  "@charset"
  "@namespace"
  "@supports"
  "@keyframes"
  (at_keyword)
  (to)
  (from)
  (important)
] @keyword

; Strings
(string_value) @string
(color_value) @string

; Numbers
[
  (integer_value)
  (float_value)
] @number

(unit) @type

; Pseudo selectors
(pseudo_element_selector "::" (tag_name) @property)
(pseudo_class_selector ":" (class_name) @property)

; Operators
[
  "~"
  ">"
  "+"
  "-"
  "|"
  "*"
  "/"
  "="
  "^="
  "|="
  "~="
  "$="
  "*="
] @operator

; Punctuation
[
  ","
  ":"
  "::"
  ";"
  "."
] @punctuation.delimiter

[
  "{"
  ")"
  "("
  "}"
  "["
  "]"
] @punctuation.bracket
