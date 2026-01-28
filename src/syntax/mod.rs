mod highlight;
mod language;
mod theme;

pub use highlight::{HighlightSpan, highlights_for_lines};
pub use language::{Language, LanguageRegistry};
pub use theme::SyntaxTheme;
