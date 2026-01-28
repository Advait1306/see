(identifier) @variable
(field_identifier) @property
(namespace_identifier) @type

(call_expression
  function: (qualified_identifier
    name: (identifier) @function))

(call_expression
  function: (identifier) @function)

(call_expression
  function: (field_expression
    field: (field_identifier) @function))

(preproc_function_def
  name: (identifier) @function)

(template_function
  name: (identifier) @function)

(template_method
  name: (field_identifier) @function)

(function_declarator
  declarator: (identifier) @function)

(function_declarator
  declarator: (qualified_identifier
    name: (identifier) @function))

(function_declarator
  declarator: (field_identifier) @function)

(destructor_name (identifier) @function)

(auto) @type
(type_identifier) @type
(primitive_type) @type
(sized_type_specifier) @type

((identifier) @constant
 (#match? @constant "^_*[A-Z][A-Z\\d_]*$"))

(statement_identifier) @type
(this) @variable

[
  "alignas"
  "alignof"
  "class"
  "decltype"
  "delete"
  "enum"
  "explicit"
  "extern"
  "final"
  "friend"
  "inline"
  "namespace"
  "new"
  "noexcept"
  "operator"
  "override"
  "private"
  "protected"
  "public"
  "sizeof"
  "struct"
  "template"
  "thread_local"
  "typedef"
  "typename"
  "union"
  "using"
  "virtual"
  "static"
  "register"
  "const"
  "volatile"
  "restrict"
  "mutable"
] @keyword

[
  "break"
  "case"
  "catch"
  "continue"
  "default"
  "do"
  "else"
  "for"
  "goto"
  "if"
  "return"
  "switch"
  "throw"
  "try"
  "while"
] @keyword

[
  "#define"
  "#elif"
  "#else"
  "#endif"
  "#if"
  "#ifdef"
  "#ifndef"
  "#include"
  (preproc_directive)
] @keyword

(comment) @comment

[
  (true)
  (false)
] @boolean

(null) @constant
"nullptr" @constant

(number_literal) @number

[
  (string_literal)
  (system_lib_string)
  (char_literal)
  (raw_string_literal)
] @string

(escape_sequence) @string

[
  ","
  ":"
  "::"
  ";"
] @punctuation.delimiter

[
  "{"
  "}"
  "("
  ")"
  "["
  "]"
] @punctuation.bracket

[
  "."
  ".*"
  "->*"
  "~"
  "-"
  "--"
  "-="
  "->"
  "="
  "!"
  "!="
  "|"
  "|="
  "||"
  "^"
  "^="
  "&"
  "&="
  "&&"
  "+"
  "++"
  "+="
  "*"
  "*="
  "/"
  "/="
  "%"
  "%="
  "<<"
  "<<="
  ">>"
  ">>="
  "<"
  "=="
  ">"
  "<="
  ">="
  "?"
] @operator
