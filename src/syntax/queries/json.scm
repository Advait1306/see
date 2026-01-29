; Comments (for jsonc)
(comment) @comment

; Strings
(string) @string
(escape_sequence) @string

; Property keys
(pair
  key: (string) @property)

; Numbers
(number) @number

; Booleans and null
[
  (true)
  (false)
] @boolean

(null) @constant

; Punctuation
[
  ","
  ":"
] @punctuation.delimiter

[
  "{"
  "}"
  "["
  "]"
] @punctuation.bracket
