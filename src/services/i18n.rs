use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::Deserialize;

/// Translation entry in TOML format
#[derive(Debug, Clone, Deserialize)]
pub struct TranslationValue {
    #[serde(flatten)]
    values: HashMap<String, toml::Value>,
}

/// i18n service for managing translations
#[derive(Debug, Clone)]
pub struct I18n {
    /// Current language code (e.g., "en_US", "zh_CN")
    current_lang: Arc<RwLock<String>>,
    /// All loaded translations: lang -> key -> value
    translations: Arc<RwLock<HashMap<String, HashMap<String, String>>>>,
    /// Default fallback language
    default_lang: String,
}

impl Default for I18n {
    fn default() -> Self {
        Self::new()
    }
}

impl I18n {
    /// Create a new i18n instance
    pub fn new() -> Self {
        Self {
            current_lang: Arc::new(RwLock::new("en_US".to_string())),
            translations: Arc::new(RwLock::new(HashMap::new())),
            default_lang: "en_US".to_string(),
        }
    }

    /// Load translations from embedded files
    pub async fn load_translations(&self) -> anyhow::Result<()> {
        let mut translations = self.translations.write().await;

        // Load all translation files
        let langs = [
            "en_US", "zh_CN", "zh_TW", "ru_RU", "vi_VN", "fa_IR",
            "ar_EG", "es_ES", "ja_JP", "id_ID", "pt_BR", "tr_TR", "uk_UA"
        ];

        for lang in langs {
            if let Ok(content) = Self::load_translation_file(lang) {
                if let Ok(map) = Self::parse_toml_translations(&content) {
                    translations.insert(lang.to_string(), map);
                    tracing::info!("Loaded {} translations for {}", translations.get(lang).map(|m| m.len()).unwrap_or(0), lang);
                }
            }
        }

        Ok(())
    }

    /// Load a translation file by language code
    fn load_translation_file(lang: &str) -> anyhow::Result<String> {
        let content = match lang {
            "en_US" => include_str!("../../web/translation/translate.en_US.toml"),
            "zh_CN" => include_str!("../../web/translation/translate.zh_CN.toml"),
            "zh_TW" => include_str!("../../web/translation/translate.zh_TW.toml"),
            "ru_RU" => include_str!("../../web/translation/translate.ru_RU.toml"),
            "vi_VN" => include_str!("../../web/translation/translate.vi_VN.toml"),
            "fa_IR" => include_str!("../../web/translation/translate.fa_IR.toml"),
            "ar_EG" => include_str!("../../web/translation/translate.ar_EG.toml"),
            "es_ES" => include_str!("../../web/translation/translate.es_ES.toml"),
            "ja_JP" => include_str!("../../web/translation/translate.ja_JP.toml"),
            "id_ID" => include_str!("../../web/translation/translate.id_ID.toml"),
            "pt_BR" => include_str!("../../web/translation/translate.pt_BR.toml"),
            "tr_TR" => include_str!("../../web/translation/translate.tr_TR.toml"),
            "uk_UA" => include_str!("../../web/translation/translate.uk_UA.toml"),
            _ => return Err(anyhow::anyhow!("Unknown language: {}", lang)),
        };
        Ok(content.to_string())
    }

    /// Parse TOML translation file into a flat key-value map
    fn parse_toml_translations(content: &str) -> anyhow::Result<HashMap<String, String>> {
        let mut map = HashMap::new();
        let value: toml::Value = toml::from_str(content)?;

        Self::flatten_toml(&value, "", &mut map);

        Ok(map)
    }

    /// Recursively flatten TOML structure into dot-notation keys
    fn flatten_toml(value: &toml::Value, prefix: &str, map: &mut HashMap<String, String>) {
        match value {
            toml::Value::Table(table) => {
                for (key, val) in table {
                    let new_prefix = if prefix.is_empty() {
                        key.clone()
                    } else {
                        format!("{}.{}", prefix, key)
                    };
                    Self::flatten_toml(val, &new_prefix, map);
                }
            }
            toml::Value::String(s) => {
                map.insert(prefix.to_string(), s.clone());
            }
            toml::Value::Integer(i) => {
                map.insert(prefix.to_string(), i.to_string());
            }
            toml::Value::Boolean(b) => {
                map.insert(prefix.to_string(), b.to_string());
            }
            _ => {}
        }
    }

    /// Set the current language
    pub async fn set_language(&self, lang: &str) {
        let mut current = self.current_lang.write().await;
        *current = lang.to_string();
    }

    /// Get the current language
    pub async fn get_language(&self) -> String {
        self.current_lang.read().await.clone()
    }

    /// Get a translation by key
    pub async fn t(&self, key: &str) -> String {
        let translations = self.translations.read().await;
        let current_lang = self.current_lang.read().await.clone();

        // Try current language first
        if let Some(lang_map) = translations.get(&current_lang) {
            if let Some(value) = lang_map.get(key) {
                return value.clone();
            }
        }

        // Fallback to default language
        if let Some(lang_map) = translations.get(&self.default_lang) {
            if let Some(value) = lang_map.get(key) {
                return value.clone();
            }
        }

        // Return key if not found
        key.to_string()
    }

    /// Get a translation synchronously (for template rendering)
    pub fn t_sync(&self, key: &str, lang: &str) -> String {
        // This is a synchronous version that requires pre-loaded translations
        // Used in template rendering where we can't use async
        key.to_string()
    }

    /// Check if a language is supported
    pub async fn is_language_supported(&self, lang: &str) -> bool {
        let translations = self.translations.read().await;
        translations.contains_key(lang)
    }

    /// Get all supported languages
    pub async fn get_supported_languages(&self) -> Vec<LanguageInfo> {
        let translations = self.translations.read().await;
        translations.keys().map(|code| {
            let (name, icon) = match code.as_str() {
                "en_US" => ("English", "🇺🇸"),
                "zh_CN" => ("简体中文", "🇨🇳"),
                "zh_TW" => ("繁體中文", "🇹🇼"),
                "ru_RU" => ("Русский", "🇷🇺"),
                "vi_VN" => ("Tiếng Việt", "🇻🇳"),
                "fa_IR" => ("فارسی", "🇮🇷"),
                "ar_EG" => ("العربية", "🇪🇬"),
                "es_ES" => ("Español", "🇪🇸"),
                "ja_JP" => ("日本語", "🇯🇵"),
                "id_ID" => ("Bahasa Indonesia", "🇮🇩"),
                "pt_BR" => ("Português", "🇧🇷"),
                "tr_TR" => ("Türkçe", "🇹🇷"),
                "uk_UA" => ("Українська", "🇺🇦"),
                _ => (code.as_str(), "🌐"),
            };
            LanguageInfo {
                value: code.clone(),
                name: name.to_string(),
                icon: icon.to_string(),
            }
        }).collect()
    }
}

/// Language information for UI
#[derive(Debug, Clone, serde::Serialize)]
pub struct LanguageInfo {
    pub value: String,
    pub name: String,
    pub icon: String,
}

/// Global i18n instance
static mut I18N_INSTANCE: Option<I18n> = None;

/// Get or create the global i18n instance
pub fn get_i18n() -> I18n {
    unsafe {
        if I18N_INSTANCE.is_none() {
            I18N_INSTANCE = Some(I18n::new());
        }
        I18N_INSTANCE.clone().unwrap()
    }
}

/// Initialize the global i18n instance
pub async fn init_i18n() -> anyhow::Result<()> {
    let i18n = get_i18n();
    i18n.load_translations().await?;
    Ok(())
}
