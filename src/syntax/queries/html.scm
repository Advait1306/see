; Tags
(tag_name) @tag

; Attributes
(attribute_name) @attribute

; Attribute values
[
  (attribute_value)
  (quoted_attribute_value)
] @string

; Comments
(comment) @comment

; Doctype
(doctype) @keyword

; Punctuation
[
  "<"
  ">"
  "</"
  "/>"
] @punctuation.bracket

"=" @punctuation.delimiter

; Text content
(text) @text
