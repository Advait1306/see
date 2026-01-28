; Variables
(identifier) @variable

; Types
(type (identifier) @type)

; Properties/Attributes
(attribute attribute: (identifier) @property)

; Function calls
(call
  function: (attribute attribute: (identifier) @function))
(call
  function: (identifier) @function)

; Decorators
(decorator "@" @punctuation.special)
(decorator
  (identifier) @function)
(decorator
  (call function: (identifier) @function))

; Function definitions
(function_definition
  name: (identifier) @function)

; Class definitions
(class_definition
  name: (identifier) @type)

; Strings
(string) @string
(escape_sequence) @string

; Numbers
[
  (integer)
  (float)
] @number

; Booleans and special values
[
  (true)
  (false)
] @boolean

[
  (none)
] @constant

; Comments
(comment) @comment

; Punctuation
[
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
  "-="
  "!="
  "*"
  "**"
  "**="
  "*="
  "/"
  "//"
  "//="
  "/="
  "&"
  "%"
  "%="
  "^"
  "+"
  "->"
  "+="
  "<"
  "<<"
  "<="
  "="
  ":="
  "=="
  ">"
  ">="
  ">>"
  "|"
  "~"
  "@"
  "@="
] @operator

; Keyword operators
[
  "and"
  "in"
  "is"
  "not"
  "or"
] @keyword

; Keywords
[
  "as"
  "assert"
  "async"
  "await"
  "break"
  "class"
  "continue"
  "def"
  "del"
  "elif"
  "else"
  "except"
  "finally"
  "for"
  "from"
  "global"
  "if"
  "import"
  "lambda"
  "nonlocal"
  "pass"
  "raise"
  "return"
  "try"
  "while"
  "with"
  "yield"
  "match"
  "case"
] @keyword
