; Variables
[
  (identifier)
  (global_variable)
] @variable

; Keywords
[
  "alias"
  "and"
  "begin"
  "break"
  "case"
  "class"
  "def"
  "do"
  "else"
  "elsif"
  "end"
  "ensure"
  "for"
  "if"
  "in"
  "module"
  "next"
  "or"
  "rescue"
  "retry"
  "return"
  "then"
  "unless"
  "until"
  "when"
  "while"
  "yield"
] @keyword

((identifier) @keyword
  (#match? @keyword "^(private|protected|public)$"))

; Function calls
(call
  method: [
    (identifier)
    (constant)
  ] @function)

((identifier) @keyword
  (#any-of? @keyword "require" "require_relative" "load"))

"defined?" @function

; Function definitions
(alias
  (identifier) @function)

(setter
  (identifier) @function)

(method
  name: [
    (identifier)
    (constant)
  ] @function)

(singleton_method
  name: [
    (identifier)
    (constant)
  ] @function)

(method_parameters
  [
    (identifier) @variable
    (optional_parameter
      name: (identifier) @variable)
    (keyword_parameter
      [
        name: (identifier)
        ":"
      ] @variable)
  ])

(block_parameters
  (identifier) @variable)

; Identifiers
((identifier) @constant
  (#match? @constant "^__(FILE|LINE|ENCODING)__$"))

(file) @constant

(line) @constant

(encoding) @constant

(hash_splat_nil
  "**" @operator) @constant

(constant) @type

((constant) @constant
  (#match? @constant "^[A-Z\\d_]+$"))

(superclass
  (constant) @type)

(superclass
  (scope_resolution
    (constant) @type))

(superclass
  (scope_resolution
    (scope_resolution
      (constant) @type)))

(self) @variable

(super) @variable

[
  (class_variable)
  (instance_variable)
] @variable

((call
  !receiver
  method: (identifier) @function)
  (#any-of? @function "include" "extend" "prepend" "refine" "using"))

((identifier) @keyword
  (#any-of? @keyword "raise" "fail" "catch" "throw"))

; Literals
[
  (string)
  (bare_string)
  (subshell)
  (heredoc_body)
  (heredoc_beginning)
] @string

[
  (simple_symbol)
  (delimited_symbol)
  (hash_key_symbol)
  (bare_symbol)
] @string

(regex) @string

(escape_sequence) @string

[
  (integer)
  (float)
] @number

[
  (true)
  (false)
] @boolean

(nil) @constant

; Comments
(comment) @comment

; Operators
[
  "!"
  "~"
  "+"
  "-"
  "**"
  "*"
  "/"
  "%"
  "<<"
  ">>"
  "&"
  "|"
  "^"
  ">"
  "<"
  "<="
  ">="
  "=="
  "!="
  "=~"
  "!~"
  "<=>"
  "||"
  "&&"
  ".."
  "..."
  "="
  "**="
  "*="
  "/="
  "%="
  "+="
  "-="
  "<<="
  ">>="
  "&&="
  "&="
  "||="
  "|="
  "^="
  "=>"
  "->"
  (operator)
] @operator

[
  ","
  ";"
  "."
  "::"
] @punctuation.delimiter

[
  "("
  ")"
  "["
  "]"
  "{"
  "}"
  "%w("
  "%i("
] @punctuation.bracket

(interpolation
  "#{" @punctuation.bracket
  "}" @punctuation.bracket)
