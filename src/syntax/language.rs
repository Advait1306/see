use gpui::{App, AppContext as _, Entity, Global};
use std::path::Path;
use std::sync::Arc;
use tree_sitter::{Language as TSLanguage, Query};

pub struct GlobalLanguageRegistry(pub Entity<LanguageRegistry>);

impl Global for GlobalLanguageRegistry {}

pub struct Language {
    #[allow(dead_code)]
    pub name: &'static str,
    pub extensions: &'static [&'static str],
    grammar: TSLanguage,
    pub highlights_query: Query,
}

impl Language {
    pub fn new(
        name: &'static str,
        extensions: &'static [&'static str],
        grammar: TSLanguage,
        highlights_scm: &str,
    ) -> Option<Self> {
        let highlights_query = match Query::new(&grammar, highlights_scm) {
            Ok(q) => q,
            Err(e) => {
                eprintln!("[SYNTAX] Failed to compile query for {}: {:?}", name, e);
                return None;
            }
        };
        Some(Self {
            name,
            extensions,
            grammar,
            highlights_query,
        })
    }

    pub fn grammar(&self) -> TSLanguage {
        self.grammar.clone()
    }
}

pub struct LanguageRegistry {
    languages: Vec<Arc<Language>>,
}

impl LanguageRegistry {
    pub fn init(cx: &mut App) {
        let registry = cx.new(|_cx| Self::new());
        cx.set_global(GlobalLanguageRegistry(registry));
    }

    pub fn global(cx: &App) -> Entity<Self> {
        cx.global::<GlobalLanguageRegistry>().0.clone()
    }

    pub fn new() -> Self {
        let mut registry = Self { languages: Vec::new() };
        registry.register_builtin_languages();
        registry
    }

    fn register_builtin_languages(&mut self) {
        // Rust
        if let Some(lang) = Language::new(
            "rust",
            &["rs"],
            tree_sitter_rust::LANGUAGE.into(),
            include_str!("queries/rust.scm"),
        ) {
            eprintln!("[DEBUG] Registered language: rust");
            self.languages.push(Arc::new(lang));
        } else {
            eprintln!("[DEBUG] Failed to register language: rust");
        }

        // JavaScript
        if let Some(lang) = Language::new(
            "javascript",
            &["js", "mjs", "cjs", "jsx"],
            tree_sitter_javascript::LANGUAGE.into(),
            include_str!("queries/javascript.scm"),
        ) {
            self.languages.push(Arc::new(lang));
        }

        // TypeScript
        if let Some(lang) = Language::new(
            "typescript",
            &["ts", "mts", "cts"],
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            include_str!("queries/typescript.scm"),
        ) {
            self.languages.push(Arc::new(lang));
        }

        // TSX
        if let Some(lang) = Language::new(
            "tsx",
            &["tsx"],
            tree_sitter_typescript::LANGUAGE_TSX.into(),
            include_str!("queries/tsx.scm"),
        ) {
            self.languages.push(Arc::new(lang));
        }

        // Python
        if let Some(lang) = Language::new(
            "python",
            &["py", "pyw", "pyi"],
            tree_sitter_python::LANGUAGE.into(),
            include_str!("queries/python.scm"),
        ) {
            self.languages.push(Arc::new(lang));
        }

        // Go
        if let Some(lang) = Language::new(
            "go",
            &["go"],
            tree_sitter_go::LANGUAGE.into(),
            include_str!("queries/go.scm"),
        ) {
            self.languages.push(Arc::new(lang));
        }

        // C
        if let Some(lang) = Language::new(
            "c",
            &["c", "h"],
            tree_sitter_c::LANGUAGE.into(),
            include_str!("queries/c.scm"),
        ) {
            self.languages.push(Arc::new(lang));
        }

        // C++
        if let Some(lang) = Language::new(
            "cpp",
            &["cpp", "cc", "cxx", "hpp", "hh", "hxx"],
            tree_sitter_cpp::LANGUAGE.into(),
            include_str!("queries/cpp.scm"),
        ) {
            self.languages.push(Arc::new(lang));
        }

        // Java
        if let Some(lang) = Language::new(
            "java",
            &["java"],
            tree_sitter_java::LANGUAGE.into(),
            include_str!("queries/java.scm"),
        ) {
            self.languages.push(Arc::new(lang));
        }

        // Ruby
        if let Some(lang) = Language::new(
            "ruby",
            &["rb", "rake", "gemspec"],
            tree_sitter_ruby::LANGUAGE.into(),
            include_str!("queries/ruby.scm"),
        ) {
            self.languages.push(Arc::new(lang));
        }

        // Swift
        if let Some(lang) = Language::new(
            "swift",
            &["swift"],
            tree_sitter_swift::LANGUAGE.into(),
            include_str!("queries/swift.scm"),
        ) {
            self.languages.push(Arc::new(lang));
        }

        // JSON
        if let Some(lang) = Language::new(
            "json",
            &["json", "jsonc"],
            tree_sitter_json::LANGUAGE.into(),
            include_str!("queries/json.scm"),
        ) {
            self.languages.push(Arc::new(lang));
        }

        // TOML
        if let Some(lang) = Language::new(
            "toml",
            &["toml"],
            tree_sitter_toml_ng::LANGUAGE.into(),
            include_str!("queries/toml.scm"),
        ) {
            self.languages.push(Arc::new(lang));
        }

        // YAML
        if let Some(lang) = Language::new(
            "yaml",
            &["yml", "yaml"],
            tree_sitter_yaml::LANGUAGE.into(),
            include_str!("queries/yaml.scm"),
        ) {
            self.languages.push(Arc::new(lang));
        }

        // HTML
        if let Some(lang) = Language::new(
            "html",
            &["html", "htm"],
            tree_sitter_html::LANGUAGE.into(),
            include_str!("queries/html.scm"),
        ) {
            self.languages.push(Arc::new(lang));
        }

        // CSS
        if let Some(lang) = Language::new(
            "css",
            &["css"],
            tree_sitter_css::LANGUAGE.into(),
            include_str!("queries/css.scm"),
        ) {
            self.languages.push(Arc::new(lang));
        }

        // Bash
        if let Some(lang) = Language::new(
            "bash",
            &["sh", "bash", "zsh"],
            tree_sitter_bash::LANGUAGE.into(),
            include_str!("queries/bash.scm"),
        ) {
            self.languages.push(Arc::new(lang));
        }
    }

    pub fn language_for_path(&self, path: &Path) -> Option<Arc<Language>> {
        let extension = path.extension()?.to_str()?;
        self.languages
            .iter()
            .find(|lang| lang.extensions.contains(&extension))
            .cloned()
    }

}

impl Default for LanguageRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_language_for_path_rust() {
        let registry = LanguageRegistry::new();
        let lang = registry.language_for_path(Path::new("main.rs"));
        assert!(lang.is_some());
        assert_eq!(lang.unwrap().name, "rust");
    }

    #[test]
    fn test_language_for_path_unknown_extension() {
        let registry = LanguageRegistry::new();
        let lang = registry.language_for_path(Path::new("file.xyz"));
        assert!(lang.is_none());
    }

    #[test]
    fn test_language_for_path_no_extension() {
        let registry = LanguageRegistry::new();
        let lang = registry.language_for_path(Path::new("Makefile"));
        assert!(lang.is_none());
    }

    #[test]
    fn test_all_builtin_languages_register() {
        let registry = LanguageRegistry::new();
        let expected_extensions = ["rs", "js", "ts", "tsx", "py", "go", "c", "cpp", "java", "rb", "swift", "json", "toml", "yml", "html", "css", "sh"];
        for ext in &expected_extensions {
            let path_str = format!("test.{}", ext);
            let lang = registry.language_for_path(Path::new(&path_str));
            assert!(lang.is_some(), "Expected language for extension '{}' but got None", ext);
        }
    }
}
