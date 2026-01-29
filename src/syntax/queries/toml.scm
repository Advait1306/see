; Properties
(bare_key) @property
(quoted_key) @property

; Literals
(boolean) @boolean
(comment) @comment
(integer) @number
(float) @number
(string) @string
(escape_sequence) @string

; Date/time types
(offset_date_time) @string
(local_date_time) @string
(local_date) @string
(local_time) @string

; Punctuation
[
  "."
  ","
] @punctuation.delimiter

"=" @operator

[
  "["
  "]"
  "[["
  "]]"
  "{"
  "}"
] @punctuation.bracket
