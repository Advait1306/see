; Booleans and null
(boolean_scalar) @boolean
(null_scalar) @constant

; Strings
[
  (double_quote_scalar)
  (single_quote_scalar)
  (block_scalar)
  (string_scalar)
] @string

(escape_sequence) @string

; Numbers
[
  (integer_scalar)
  (float_scalar)
] @number

; Keys
(block_mapping_pair
  key: (flow_node
    (plain_scalar
      (string_scalar) @property)))

; Comments
(comment) @comment

; Anchors and aliases
[
  (anchor_name)
  (alias_name)
  (tag)
] @type

; Punctuation
[
 ","
 "-"
 ":"
 ">"
 "?"
 "|"
] @punctuation.delimiter

[
 "["
 "]"
 "{"
 "}"
] @punctuation.bracket

; Special markers
[
 "*"
 "&"
 "---"
 "..."
] @punctuation.special
