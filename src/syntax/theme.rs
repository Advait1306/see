use gpui::Hsla;
use std::collections::HashMap;

pub struct SyntaxTheme {
    colors: HashMap<&'static str, Hsla>,
}

/// Convert hex color string to Hsla (e.g., "#74ade8" or "#74ade8ff")
fn hex_to_hsla(hex: &str) -> Hsla {
    let hex = hex.trim_start_matches('#');
    let (r, g, b) = match hex.len() {
        6 | 8 => {
            let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0) as f32 / 255.0;
            let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0) as f32 / 255.0;
            let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0) as f32 / 255.0;
            (r, g, b)
        }
        _ => (0.5, 0.5, 0.5),
    };

    // RGB to HSL conversion
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;

    if (max - min).abs() < 0.0001 {
        return Hsla { h: 0.0, s: 0.0, l, a: 1.0 };
    }

    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };

    let h = if (max - r).abs() < 0.0001 {
        let mut h = (g - b) / d;
        if g < b {
            h += 6.0;
        }
        h / 6.0
    } else if (max - g).abs() < 0.0001 {
        ((b - r) / d + 2.0) / 6.0
    } else {
        ((r - g) / d + 4.0) / 6.0
    };

    Hsla { h, s, l, a: 1.0 }
}

impl SyntaxTheme {
    pub fn new() -> Self {
        // One Light theme colors from Zed
        let mut colors = HashMap::new();

        // Keywords - purple/magenta
        let keyword = hex_to_hsla("#a449ab");
        colors.insert("keyword", keyword);
        colors.insert("keyword.control", keyword);
        colors.insert("keyword.function", keyword);
        colors.insert("keyword.operator", keyword);
        colors.insert("keyword.return", keyword);
        colors.insert("keyword.directive", keyword);
        colors.insert("keyword.definition", keyword);

        // Types - blue
        let type_color = hex_to_hsla("#3882b7");
        colors.insert("type", type_color);
        colors.insert("type.builtin", type_color);
        colors.insert("type.class", type_color);
        colors.insert("type.unit", type_color);

        // Functions - blue
        let function = hex_to_hsla("#5b79e3");
        colors.insert("function", function);
        colors.insert("function.method", function);
        colors.insert("function.builtin", function);
        colors.insert("function.call", function);
        colors.insert("function.decorator", function);
        colors.insert("function.special", function);
        colors.insert("constructor", function);

        // Strings - green
        let string = hex_to_hsla("#649f57");
        colors.insert("string", string);
        colors.insert("text.literal", string);

        // String escape - gray
        let string_escape = hex_to_hsla("#7c7e86");
        colors.insert("string.escape", string_escape);

        // String special/regex - orange
        let string_special = hex_to_hsla("#ad6e26");
        colors.insert("string.special", string_special);
        colors.insert("string.regex", string_special);
        colors.insert("string.special.symbol", string_special);

        // Numbers - orange
        let number = hex_to_hsla("#ad6e25");
        colors.insert("number", number);
        colors.insert("number.float", number);

        // Booleans - orange
        colors.insert("boolean", number);

        // Comments - gray
        let comment = hex_to_hsla("#a2a3a7");
        colors.insert("comment", comment);
        colors.insert("comment.line", comment);
        colors.insert("comment.block", comment);

        // Doc comments - darker gray
        let comment_doc = hex_to_hsla("#7c7e86");
        colors.insert("comment.doc", comment_doc);

        // Operators - blue
        let operator = hex_to_hsla("#3882b7");
        colors.insert("operator", operator);
        colors.insert("operator.spaceship", operator);

        // Punctuation - dark gray
        let punctuation = hex_to_hsla("#242529");
        colors.insert("punctuation", punctuation);

        let punctuation_bracket = hex_to_hsla("#4d4f52");
        colors.insert("punctuation.bracket", punctuation_bracket);
        colors.insert("punctuation.delimiter", punctuation_bracket);

        // Punctuation special - red
        let punctuation_special = hex_to_hsla("#b92b46");
        colors.insert("punctuation.special", punctuation_special);

        // Variables - dark gray
        let variable = hex_to_hsla("#242529");
        colors.insert("variable", variable);
        colors.insert("variable.parameter", variable);
        colors.insert("text", variable);

        // Variable special (self, this) - orange
        let variable_special = hex_to_hsla("#ad6e25");
        colors.insert("variable.special", variable_special);
        colors.insert("variable.builtin", variable_special);

        // Constants - yellow/gold
        let constant = hex_to_hsla("#c18401");
        colors.insert("constant", constant);
        colors.insert("constant.builtin", constant);

        // Properties - red/orange
        let property = hex_to_hsla("#d3604f");
        colors.insert("property", property);
        colors.insert("property.json_key", property);

        // Tags - blue
        let tag = hex_to_hsla("#5c78e2");
        colors.insert("tag", tag);
        colors.insert("tag.jsx", tag);

        // Attributes - blue
        let attribute = hex_to_hsla("#5c78e2");
        colors.insert("attribute", attribute);
        colors.insert("attribute.jsx", attribute);
        colors.insert("attribute.builtin", attribute);

        // Labels - blue
        colors.insert("label", attribute);

        // Namespace - dark
        let namespace = hex_to_hsla("#242529");
        colors.insert("namespace", namespace);

        // Embedded - dark
        colors.insert("embedded", namespace);

        // Enum - red/orange
        let enum_color = hex_to_hsla("#d3604f");
        colors.insert("enum", enum_color);

        // Preproc/Directive - dark
        colors.insert("preproc", namespace);

        // Symbol (Ruby) - orange
        colors.insert("symbol", string_special);

        // Selector (CSS) - green
        let selector = hex_to_hsla("#669f59");
        colors.insert("selector", selector);
        colors.insert("selector.id", selector);
        colors.insert("selector.class", selector);
        colors.insert("selector.pseudo", attribute);

        // Title - red/orange
        colors.insert("title", property);

        // Variant - blue
        colors.insert("variant", function);

        Self { colors }
    }

    pub fn color_for_capture(&self, capture_name: &str) -> Option<Hsla> {
        // Try exact match first
        if let Some(&color) = self.colors.get(capture_name) {
            return Some(color);
        }

        // Try parent match (e.g., "function.method" -> "function")
        if let Some(dot_idx) = capture_name.find('.') {
            let parent = &capture_name[..dot_idx];
            if let Some(&color) = self.colors.get(parent) {
                return Some(color);
            }
        }

        None
    }
}

impl Default for SyntaxTheme {
    fn default() -> Self {
        Self::new()
    }
}
