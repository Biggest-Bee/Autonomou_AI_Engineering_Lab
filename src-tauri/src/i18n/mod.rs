// Multi-Language Support Module
// Programming language registry, file-type detection, and UI localization

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

/// A supported programming language
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageInfo {
    pub language_id: String,
    pub name: String,
    pub extensions: Vec<String>,
    pub tree_sitter_grammar: Option<String>,
    pub comment_syntax: CommentSyntax,
    pub category: LanguageCategory,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommentSyntax {
    pub line: String,
    pub block_start: Option<String>,
    pub block_end: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LanguageCategory {
    System,
    Scripting,
    Web,
    Data,
    Markup,
    Config,
    Other,
}

/// Detect the programming language for a file path
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageDetection {
    pub file_path: String,
    pub language_id: Option<String>,
    pub confidence: f64, // 1.0 = extension match, lower = heuristic
    pub method: DetectionMethod,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DetectionMethod {
    Extension,
    Shebang,
    Filename,
    Content,
    Unknown,
}

/// UI localization entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocaleString {
    pub key: String,
    pub translations: HashMap<String, String>, // locale -> text
}

/// Multi-language support manager
pub struct LanguageManager {
    /// language_id -> LanguageInfo
    languages: Arc<RwLock<HashMap<String, LanguageInfo>>>,
    /// extension -> language_id index
    extension_index: Arc<RwLock<HashMap<String, String>>>,
    /// filename -> language_id index (e.g. "Dockerfile", "Makefile")
    filename_index: Arc<RwLock<HashMap<String, String>>>,
    /// UI locale strings
    locales: Arc<RwLock<HashMap<String, HashMap<String, String>>>>, // locale -> key -> text
    current_locale: Arc<RwLock<String>>,
}

impl LanguageManager {
    pub fn new() -> Self {
        let mgr = Self {
            languages: Arc::new(RwLock::new(HashMap::new())),
            extension_index: Arc::new(RwLock::new(HashMap::new())),
            filename_index: Arc::new(RwLock::new(HashMap::new())),
            locales: Arc::new(RwLock::new(HashMap::new())),
            current_locale: Arc::new(RwLock::new("en".to_string())),
        };
        mgr.register_defaults();
        mgr
    }

    fn register_defaults(&self) {
        // This is a sync fn; register via blocking is awkward — instead we
        // populate lazily in `ensure_defaults` on first async use.
        // For simplicity, defaults are registered in `initialize`.
    }

    /// Initialize with the default language set (async context)
    pub async fn initialize(&self) -> Result<()> {
        let defaults = Self::default_languages();
        let mut langs = self.languages.write().await;
        let mut ext_idx = self.extension_index.write().await;
        let mut fn_idx = self.filename_index.write().await;

        for lang in defaults {
            for ext in &lang.extensions {
                ext_idx.insert(ext.clone(), lang.language_id.clone());
            }
            // Special filenames
            match lang.language_id.as_str() {
                "dockerfile" => { fn_idx.insert("dockerfile".to_string(), "dockerfile".to_string()); }
                "makefile" => { fn_idx.insert("makefile".to_string(), "makefile".to_string()); }
                _ => {}
            }
            langs.insert(lang.language_id.clone(), lang);
        }

        // Default UI strings (en)
        let mut en = HashMap::new();
        en.insert("app.title".to_string(), "Autonomous AI Engineering Lab".to_string());
        en.insert("menu.file".to_string(), "File".to_string());
        en.insert("menu.edit".to_string(), "Edit".to_string());
        en.insert("menu.view".to_string(), "View".to_string());
        en.insert("menu.help".to_string(), "Help".to_string());
        en.insert("terminal.title".to_string(), "Terminal".to_string());
        en.insert("ide.files".to_string(), "Files".to_string());
        en.insert("ide.editor".to_string(), "Editor".to_string());
        en.insert("common.save".to_string(), "Save".to_string());
        en.insert("common.cancel".to_string(), "Cancel".to_string());
        en.insert("common.run".to_string(), "Run".to_string());
        self.locales.write().await.insert("en".to_string(), en);

        // Spanish
        let mut es = HashMap::new();
        es.insert("app.title".to_string(), "Laboratorio de Ingeniería AI Autónoma".to_string());
        es.insert("menu.file".to_string(), "Archivo".to_string());
        es.insert("menu.edit".to_string(), "Editar".to_string());
        es.insert("menu.view".to_string(), "Ver".to_string());
        es.insert("menu.help".to_string(), "Ayuda".to_string());
        es.insert("terminal.title".to_string(), "Terminal".to_string());
        es.insert("ide.files".to_string(), "Archivos".to_string());
        es.insert("ide.editor".to_string(), "Editor".to_string());
        es.insert("common.save".to_string(), "Guardar".to_string());
        es.insert("common.cancel".to_string(), "Cancelar".to_string());
        es.insert("common.run".to_string(), "Ejecutar".to_string());
        self.locales.write().await.insert("es".to_string(), es);

        Ok(())
    }

    fn default_languages() -> Vec<LanguageInfo> {
        vec![
            LanguageInfo {
                language_id: "rust".into(), name: "Rust".into(),
                extensions: vec!["rs".into()],
                tree_sitter_grammar: Some("rust".into()),
                comment_syntax: CommentSyntax { line: "//".into(), block_start: Some("/*".into()), block_end: Some("*/".into()) },
                category: LanguageCategory::System, enabled: true,
            },
            LanguageInfo {
                language_id: "python".into(), name: "Python".into(),
                extensions: vec!["py".into(), "pyi".into()],
                tree_sitter_grammar: Some("python".into()),
                comment_syntax: CommentSyntax { line: "#".into(), block_start: Some("\"\"\"".into()), block_end: Some("\"\"\"".into()) },
                category: LanguageCategory::Scripting, enabled: true,
            },
            LanguageInfo {
                language_id: "typescript".into(), name: "TypeScript".into(),
                extensions: vec!["ts".into(), "tsx".into()],
                tree_sitter_grammar: Some("typescript".into()),
                comment_syntax: CommentSyntax { line: "//".into(), block_start: Some("/*".into()), block_end: Some("*/".into()) },
                category: LanguageCategory::Web, enabled: true,
            },
            LanguageInfo {
                language_id: "javascript".into(), name: "JavaScript".into(),
                extensions: vec!["js".into(), "jsx".into(), "mjs".into(), "cjs".into()],
                tree_sitter_grammar: Some("javascript".into()),
                comment_syntax: CommentSyntax { line: "//".into(), block_start: Some("/*".into()), block_end: Some("*/".into()) },
                category: LanguageCategory::Web, enabled: true,
            },
            LanguageInfo {
                language_id: "go".into(), name: "Go".into(),
                extensions: vec!["go".into()],
                tree_sitter_grammar: Some("go".into()),
                comment_syntax: CommentSyntax { line: "//".into(), block_start: None, block_end: None },
                category: LanguageCategory::System, enabled: true,
            },
            LanguageInfo {
                language_id: "java".into(), name: "Java".into(),
                extensions: vec!["java".into()],
                tree_sitter_grammar: Some("java".into()),
                comment_syntax: CommentSyntax { line: "//".into(), block_start: Some("/*".into()), block_end: Some("*/".into()) },
                category: LanguageCategory::System, enabled: true,
            },
            LanguageInfo {
                language_id: "cpp".into(), name: "C++".into(),
                extensions: vec!["cpp".into(), "cc".into(), "cxx".into(), "hpp".into(), "hh".into(), "hxx".into()],
                tree_sitter_grammar: Some("cpp".into()),
                comment_syntax: CommentSyntax { line: "//".into(), block_start: Some("/*".into()), block_end: Some("*/".into()) },
                category: LanguageCategory::System, enabled: true,
            },
            LanguageInfo {
                language_id: "c".into(), name: "C".into(),
                extensions: vec!["c".into(), "h".into()],
                tree_sitter_grammar: Some("c".into()),
                comment_syntax: CommentSyntax { line: "//".into(), block_start: Some("/*".into()), block_end: Some("*/".into()) },
                category: LanguageCategory::System, enabled: true,
            },
            LanguageInfo {
                language_id: "csharp".into(), name: "C#".into(),
                extensions: vec!["cs".into()],
                tree_sitter_grammar: Some("c_sharp".into()),
                comment_syntax: CommentSyntax { line: "//".into(), block_start: Some("/*".into()), block_end: Some("*/".into()) },
                category: LanguageCategory::System, enabled: true,
            },
            LanguageInfo {
                language_id: "ruby".into(), name: "Ruby".into(),
                extensions: vec!["rb".into(), "rake".into(), "gemspec".into()],
                tree_sitter_grammar: Some("ruby".into()),
                comment_syntax: CommentSyntax { line: "#".into(), block_start: Some("=begin".into()), block_end: Some("=end".into()) },
                category: LanguageCategory::Scripting, enabled: true,
            },
            LanguageInfo {
                language_id: "php".into(), name: "PHP".into(),
                extensions: vec!["php".into(), "phtml".into()],
                tree_sitter_grammar: Some("php".into()),
                comment_syntax: CommentSyntax { line: "//".into(), block_start: Some("/*".into()), block_end: Some("*/".into()) },
                category: LanguageCategory::Web, enabled: true,
            },
            LanguageInfo {
                language_id: "swift".into(), name: "Swift".into(),
                extensions: vec!["swift".into()],
                tree_sitter_grammar: Some("swift".into()),
                comment_syntax: CommentSyntax { line: "//".into(), block_start: Some("/*".into()), block_end: Some("*/".into()) },
                category: LanguageCategory::System, enabled: true,
            },
            LanguageInfo {
                language_id: "kotlin".into(), name: "Kotlin".into(),
                extensions: vec!["kt".into(), "kts".into()],
                tree_sitter_grammar: Some("kotlin".into()),
                comment_syntax: CommentSyntax { line: "//".into(), block_start: Some("/*".into()), block_end: Some("*/".into()) },
                category: LanguageCategory::System, enabled: true,
            },
            LanguageInfo {
                language_id: "scala".into(), name: "Scala".into(),
                extensions: vec!["scala".into(), "sc".into()],
                tree_sitter_grammar: Some("scala".into()),
                comment_syntax: CommentSyntax { line: "//".into(), block_start: Some("/*".into()), block_end: Some("*/".into()) },
                category: LanguageCategory::System, enabled: true,
            },
            LanguageInfo {
                language_id: "html".into(), name: "HTML".into(),
                extensions: vec!["html".into(), "htm".into()],
                tree_sitter_grammar: Some("html".into()),
                comment_syntax: CommentSyntax { line: "".into(), block_start: Some("<!--".into()), block_end: Some("-->".into()) },
                category: LanguageCategory::Markup, enabled: true,
            },
            LanguageInfo {
                language_id: "css".into(), name: "CSS".into(),
                extensions: vec!["css".into(), "scss".into(), "sass".into(), "less".into()],
                tree_sitter_grammar: Some("css".into()),
                comment_syntax: CommentSyntax { line: "".into(), block_start: Some("/*".into()), block_end: Some("*/".into()) },
                category: LanguageCategory::Markup, enabled: true,
            },
            LanguageInfo {
                language_id: "json".into(), name: "JSON".into(),
                extensions: vec!["json".into(), "jsonc".into()],
                tree_sitter_grammar: Some("json".into()),
                comment_syntax: CommentSyntax { line: "".into(), block_start: None, block_end: None },
                category: LanguageCategory::Data, enabled: true,
            },
            LanguageInfo {
                language_id: "yaml".into(), name: "YAML".into(),
                extensions: vec!["yaml".into(), "yml".into()],
                tree_sitter_grammar: Some("yaml".into()),
                comment_syntax: CommentSyntax { line: "#".into(), block_start: None, block_end: None },
                category: LanguageCategory::Data, enabled: true,
            },
            LanguageInfo {
                language_id: "toml".into(), name: "TOML".into(),
                extensions: vec!["toml".into()],
                tree_sitter_grammar: Some("toml".into()),
                comment_syntax: CommentSyntax { line: "#".into(), block_start: None, block_end: None },
                category: LanguageCategory::Config, enabled: true,
            },
            LanguageInfo {
                language_id: "markdown".into(), name: "Markdown".into(),
                extensions: vec!["md".into(), "markdown".into()],
                tree_sitter_grammar: Some("markdown".into()),
                comment_syntax: CommentSyntax { line: "".into(), block_start: None, block_end: None },
                category: LanguageCategory::Markup, enabled: true,
            },
            LanguageInfo {
                language_id: "sql".into(), name: "SQL".into(),
                extensions: vec!["sql".to_string()],
                tree_sitter_grammar: Some("sql".into()),
                comment_syntax: CommentSyntax { line: "--".into(), block_start: Some("/*".into()), block_end: Some("*/".into()) },
                category: LanguageCategory::Data, enabled: true,
            },
            LanguageInfo {
                language_id: "shell".into(), name: "Shell".into(),
                extensions: vec!["sh".into(), "bash".into(), "zsh".into()],
                tree_sitter_grammar: Some("bash".into()),
                comment_syntax: CommentSyntax { line: "#".into(), block_start: None, block_end: None },
                category: LanguageCategory::Scripting, enabled: true,
            },
            LanguageInfo {
                language_id: "powershell".into(), name: "PowerShell".into(),
                extensions: vec!["ps1".into(), "psm1".into(), "psd1".into()],
                tree_sitter_grammar: None,
                comment_syntax: CommentSyntax { line: "#".into(), block_start: Some("<#".into()), block_end: Some("#>".into()) },
                category: LanguageCategory::Scripting, enabled: true,
            },
            LanguageInfo {
                language_id: "dockerfile".into(), name: "Dockerfile".into(),
                extensions: vec!["dockerfile".into()],
                tree_sitter_grammar: None,
                comment_syntax: CommentSyntax { line: "#".into(), block_start: None, block_end: None },
                category: LanguageCategory::Config, enabled: true,
            },
            LanguageInfo {
                language_id: "makefile".into(), name: "Makefile".into(),
                extensions: vec!["mk".into(), "mak".into()],
                tree_sitter_grammar: None,
                comment_syntax: CommentSyntax { line: "#".into(), block_start: None, block_end: None },
                category: LanguageCategory::Config, enabled: true,
            },
        ]
    }

    /// Detect language from a file path (extension + filename heuristics)
    pub async fn detect_language(&self, file_path: &str) -> LanguageDetection {
        let path = Path::new(file_path);
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_lowercase();

        // 1. Filename match (Dockerfile, Makefile, etc.)
        {
            let fn_idx = self.filename_index.read().await;
            if let Some(lang_id) = fn_idx.get(&filename) {
                return LanguageDetection {
                    file_path: file_path.to_string(),
                    language_id: Some(lang_id.clone()),
                    confidence: 0.95,
                    method: DetectionMethod::Filename,
                };
            }
        }

        // 2. Extension match
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            let ext_lower = ext.to_lowercase();
            let ext_idx = self.extension_index.read().await;
            if let Some(lang_id) = ext_idx.get(&ext_lower) {
                return LanguageDetection {
                    file_path: file_path.to_string(),
                    language_id: Some(lang_id.clone()),
                    confidence: 1.0,
                    method: DetectionMethod::Extension,
                };
            }
        }

        LanguageDetection {
            file_path: file_path.to_string(),
            language_id: None,
            confidence: 0.0,
            method: DetectionMethod::Unknown,
        }
    }

    /// Detect language including shebang/content sniffing
    pub async fn detect_language_with_content(
        &self,
        file_path: &str,
        content: Option<&str>,
    ) -> LanguageDetection {
        // Check shebang line first
        if let Some(c) = content {
            if let Some(first_line) = c.lines().next() {
                if first_line.starts_with("#!") {
                    if first_line.contains("python") {
                        return LanguageDetection {
                            file_path: file_path.to_string(),
                            language_id: Some("python".to_string()),
                            confidence: 0.9,
                            method: DetectionMethod::Shebang,
                        };
                    }
                    if first_line.contains("bash") || first_line.contains("sh") {
                        return LanguageDetection {
                            file_path: file_path.to_string(),
                            language_id: Some("shell".to_string()),
                            confidence: 0.9,
                            method: DetectionMethod::Shebang,
                        };
                    }
                    if first_line.contains("powershell") || first_line.contains("pwsh") {
                        return LanguageDetection {
                            file_path: file_path.to_string(),
                            language_id: Some("powershell".to_string()),
                            confidence: 0.9,
                            method: DetectionMethod::Shebang,
                        };
                    }
                    if first_line.contains("node") {
                        return LanguageDetection {
                            file_path: file_path.to_string(),
                            language_id: Some("javascript".to_string()),
                            confidence: 0.9,
                            method: DetectionMethod::Shebang,
                        };
                    }
                    if first_line.contains("ruby") {
                        return LanguageDetection {
                            file_path: file_path.to_string(),
                            language_id: Some("ruby".to_string()),
                            confidence: 0.9,
                            method: DetectionMethod::Shebang,
                        };
                    }
                }
            }
        }

        self.detect_language(file_path).await
    }

    /// Get language info by ID
    pub async fn get_language(&self, language_id: &str) -> Option<LanguageInfo> {
        self.languages.read().await.get(language_id).cloned()
    }

    /// List all registered languages
    pub async fn list_languages(&self) -> Vec<LanguageInfo> {
        self.languages.read().await.values().cloned().collect()
    }

    /// List enabled languages only
    pub async fn list_enabled_languages(&self) -> Vec<LanguageInfo> {
        self.languages
            .read()
            .await
            .values()
            .filter(|l| l.enabled)
            .cloned()
            .collect()
    }

    /// Register a custom language
    pub async fn register_language(&self, lang: LanguageInfo) -> Result<()> {
        let mut ext_idx = self.extension_index.write().await;
        for ext in &lang.extensions {
            ext_idx.insert(ext.clone(), lang.language_id.clone());
        }
        self.languages
            .write()
            .await
            .insert(lang.language_id.clone(), lang);
        Ok(())
    }

    /// Enable or disable a language
    pub async fn set_language_enabled(&self, language_id: &str, enabled: bool) -> Result<bool> {
        let mut langs = self.languages.write().await;
        if let Some(lang) = langs.get_mut(language_id) {
            lang.enabled = enabled;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Check whether a language has a tree-sitter grammar available
    pub async fn has_tree_sitter_grammar(&self, language_id: &str) -> bool {
        self.languages
            .read()
            .await
            .get(language_id)
            .and_then(|l| l.tree_sitter_grammar.as_ref())
            .is_some()
    }

    // ---------- UI Localization ----------

    /// Set the current UI locale
    pub async fn set_locale(&self, locale: &str) {
        *self.current_locale.write().await = locale.to_string();
    }

    pub async fn get_locale(&self) -> String {
        self.current_locale.read().await.clone()
    }

    /// Get a localized string, falling back to English then the key itself
    pub async fn translate(&self, key: &str) -> String {
        let locale = self.current_locale.read().await.clone();
        let locales = self.locales.read().await;

        if let Some(strings) = locales.get(&locale) {
            if let Some(s) = strings.get(key) {
                return s.clone();
            }
        }
        // Fallback to English
        if let Some(strings) = locales.get("en") {
            if let Some(s) = strings.get(key) {
                return s.clone();
            }
        }
        key.to_string()
    }

    /// List available locales
    pub async fn available_locales(&self) -> Vec<String> {
        self.locales.read().await.keys().cloned().collect()
    }

    /// Add a translation for a key in a locale
    pub async fn add_translation(&self, locale: &str, key: &str, text: &str) {
        self.locales
            .write()
            .await
            .entry(locale.to_string())
            .or_insert_with(HashMap::new)
            .insert(key.to_string(), text.to_string());
    }
}

impl Default for LanguageManager {
    fn default() -> Self {
        Self::new()
    }
}
